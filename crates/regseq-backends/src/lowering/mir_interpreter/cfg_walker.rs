//! MIR CFG walker for structured behavioral interpretation.
//!
//! Walks the MIR control-flow graph and produces structured
//! `BehaviorNode` trees. Handles linear sequences, branches, loops,
//! fallible operations, ordered regions, and cleanup paths.

use std::collections::BTreeSet;

use regseq_model::declaration::mir_body::{
    MirBody, MirPlace, MirRvalue, MirStatementKind, MirTerminatorKind, MirTy, MirUnwindAction,
};

use crate::lowering::behavior::{AssignValue, BehaviorNode, ValueExpr};

use super::call_classifier::{classify_call, CallClassification};
use super::cfg_helpers::{
    detect_fallible_pattern, emit_call_node, find_join_point, has_back_edge,
    identify_loop_structure, split_switch_targets, walk_error_path,
};
use super::context::FnContext;
use super::place_resolver::{resolve_place, ResolvedPlace};
use super::value_lowering::{
    determine_return_semantic, lower_branch_condition, lower_operand, lower_rvalue,
};

/// Walk a region of the CFG from `start_bb`, producing structured behavior nodes.
///
/// Stops at blocks in `stop_set` or when a Return/Unreachable terminator
/// is reached. Tracks visited blocks to avoid infinite loops.
pub fn walk_region(
    ctx: &FnContext<'_>,
    body: &MirBody,
    start_bb: usize,
    stop_set: &BTreeSet<usize>,
) -> Vec<BehaviorNode> {
    let mut nodes = Vec::new();
    let mut current = start_bb;
    let mut visited = BTreeSet::new();
    let mut barrier_active = false;
    let mut ordered_steps: Vec<BehaviorNode> = Vec::new();

    while current < body.basic_blocks.len()
        && !stop_set.contains(&current)
        && visited.insert(current)
    {
        let bb = &body.basic_blocks[current];

        // Process cleanup blocks — traverse them, do not skip.
        if bb.is_cleanup {
            let cleanup_nodes = lower_cleanup_block(ctx, body, current);
            if !cleanup_nodes.is_empty() {
                let target = if barrier_active {
                    &mut ordered_steps
                } else {
                    &mut nodes
                };
                target.push(BehaviorNode::CleanupRegion {
                    body: vec![],
                    cleanup: cleanup_nodes,
                });
            }
            match bb.terminator.as_ref().map(|t| &t.kind) {
                Some(MirTerminatorKind::Goto { target }) => {
                    current = *target as usize;
                    continue;
                }
                Some(MirTerminatorKind::Drop { target, .. }) => {
                    current = *target as usize;
                    continue;
                }
                _ => break,
            }
        }

        // Lower non-cleanup statements.
        let stmt_nodes = lower_statements(ctx, bb);
        let target = if barrier_active {
            &mut ordered_steps
        } else {
            &mut nodes
        };
        target.extend(stmt_nodes);

        // Process terminator.
        let term = match &bb.terminator {
            Some(t) => t,
            None => break,
        };

        match &term.kind {
            MirTerminatorKind::Goto { target } => {
                current = *target as usize;
            }

            MirTerminatorKind::Return => {
                let semantic = determine_return_semantic(ctx, body);
                let target = if barrier_active {
                    &mut ordered_steps
                } else {
                    &mut nodes
                };
                target.push(BehaviorNode::Return { semantic });
                break;
            }

            MirTerminatorKind::Call {
                func,
                args,
                destination,
                target: next_bb,
                unwind: _unwind,
            } => {
                let classification = classify_call(ctx, func, args);
                let next = next_bb.map(|b| b as usize);

                match classification {
                    CallClassification::Barrier => {
                        if barrier_active {
                            let steps = std::mem::take(&mut ordered_steps);
                            nodes.push(BehaviorNode::OrderedSequence { steps });
                            barrier_active = false;
                        } else {
                            barrier_active = true;
                        }
                        current = next.unwrap_or(usize::MAX);
                    }
                    CallClassification::Deref
                    | CallClassification::FieldValueConstructor { .. }
                    | CallClassification::OptionMethod { .. }
                    | CallClassification::DurationConstructor
                    | CallClassification::FrameworkValueConstructor { .. } => {
                        current = next.unwrap_or(usize::MAX);
                    }
                    CallClassification::TryBranch => {
                        current = next.unwrap_or(usize::MAX);
                    }
                    CallClassification::TryFromResidual => {
                        current = next.unwrap_or(usize::MAX);
                    }
                    _ => {
                        let call_node = emit_call_node(ctx, &classification, args, destination);

                        if let Some(fallible) =
                            detect_fallible_pattern(body, next, &visited, stop_set)
                        {
                            let cleanup =
                                walk_error_path(ctx, body, fallible.err_bb, &visited, stop_set);
                            let target = if barrier_active {
                                &mut ordered_steps
                            } else {
                                &mut nodes
                            };
                            target.push(BehaviorNode::FallibleOp {
                                body: call_node,
                                cleanup_on_error: cleanup,
                            });
                            current = fallible.ok_bb;
                        } else {
                            let target = if barrier_active {
                                &mut ordered_steps
                            } else {
                                &mut nodes
                            };
                            target.extend(call_node);
                            current = next.unwrap_or(usize::MAX);
                        }
                    }
                }
            }

            MirTerminatorKind::SwitchInt { discr: _, targets } => {
                let condition = lower_branch_condition(ctx, current, body);

                if has_back_edge(body, targets, &visited) {
                    let (body_entry, exit) = identify_loop_structure(targets, &visited);
                    let mut loop_stop = stop_set.clone();
                    loop_stop.insert(current);
                    let loop_body = walk_region(ctx, body, body_entry, &loop_stop);
                    let target = if barrier_active {
                        &mut ordered_steps
                    } else {
                        &mut nodes
                    };
                    target.push(BehaviorNode::Loop {
                        condition,
                        body: loop_body,
                    });
                    current = exit;
                } else {
                    let (then_bb, else_bb) = split_switch_targets(targets);
                    let join = find_join_point(body, then_bb, else_bb, stop_set);

                    let mut branch_stop = stop_set.clone();
                    if let Some(j) = join {
                        branch_stop.insert(j);
                    }
                    let then_body = walk_region(ctx, body, then_bb, &branch_stop);
                    let else_body = walk_region(ctx, body, else_bb, &branch_stop);

                    let target = if barrier_active {
                        &mut ordered_steps
                    } else {
                        &mut nodes
                    };
                    target.push(BehaviorNode::Branch {
                        condition,
                        then_body,
                        else_body,
                    });

                    if let Some(j) = join {
                        current = j;
                    } else {
                        break;
                    }
                }
            }

            MirTerminatorKind::Drop { target, unwind, .. } => {
                if let MirUnwindAction::Cleanup { target: cleanup_bb } = unwind {
                    let cleanup_nodes = lower_cleanup_block(ctx, body, *cleanup_bb as usize);
                    if !cleanup_nodes.is_empty() {
                        let target_list = if barrier_active {
                            &mut ordered_steps
                        } else {
                            &mut nodes
                        };
                        target_list.push(BehaviorNode::CleanupRegion {
                            body: vec![],
                            cleanup: cleanup_nodes,
                        });
                    }
                }
                current = *target as usize;
            }

            MirTerminatorKind::Unreachable
            | MirTerminatorKind::UnwindResume
            | MirTerminatorKind::UnwindTerminate { .. } => {
                break;
            }

            MirTerminatorKind::Assert { target, .. } => {
                current = *target as usize;
            }

            MirTerminatorKind::Unsupported { .. } => {
                break;
            }
        }
    }

    // Flush unmatched ordered region.
    if barrier_active && !ordered_steps.is_empty() {
        nodes.push(BehaviorNode::OrderedSequence {
            steps: ordered_steps,
        });
    }

    nodes
}

// ---------------------------------------------------------------------------
// Statement lowering
// ---------------------------------------------------------------------------

/// Lower non-cleanup statements from a basic block.
fn lower_statements(
    ctx: &FnContext<'_>,
    bb: &regseq_model::declaration::mir_body::MirBasicBlockData,
) -> Vec<BehaviorNode> {
    let mut nodes = Vec::new();
    for stmt in &bb.statements {
        match &stmt.kind {
            MirStatementKind::Assign { place, rvalue } => {
                if let Some(node) = lower_assign(ctx, place, rvalue) {
                    nodes.push(node);
                }
            }
            MirStatementKind::StorageLive { .. }
            | MirStatementKind::StorageDead { .. }
            | MirStatementKind::Nop
            | MirStatementKind::SetDiscriminant { .. }
            | MirStatementKind::Unsupported { .. } => {}
        }
    }
    nodes
}

/// Lower an Assign statement to a BehaviorNode, if semantically relevant.
fn lower_assign(ctx: &FnContext<'_>, place: &MirPlace, rvalue: &MirRvalue) -> Option<BehaviorNode> {
    let resolved = resolve_place(ctx, place);

    match &resolved {
        ResolvedPlace::DriverField { name } => {
            let value = match rvalue {
                MirRvalue::Use { operand } => {
                    let val = lower_operand(ctx, operand);
                    match val {
                        ValueExpr::None => AssignValue::None,
                        ValueExpr::IntLit(v) => AssignValue::Literal(v.to_string()),
                        ValueExpr::BoolLit(b) => {
                            AssignValue::Literal(if b { "1" } else { "0" }.to_string())
                        }
                        ValueExpr::Param(ref s) | ValueExpr::Local(ref s) => {
                            AssignValue::Literal(s.clone())
                        }
                        ValueExpr::FieldRef(ref s) => AssignValue::Literal(s.clone()),
                        _ => AssignValue::None,
                    }
                }
                MirRvalue::Aggregate { agg_kind, operands } => {
                    use regseq_model::declaration::mir_body::MirAggregateKind;
                    match agg_kind {
                        MirAggregateKind::Adt {
                            path,
                            variant_index,
                            ..
                        } => {
                            if path.contains("Option") && *variant_index == 0 {
                                AssignValue::None
                            } else if path.contains("Option") && *variant_index == 1 {
                                if let Some(first) = operands.first() {
                                    let val = lower_operand(ctx, first);
                                    match val {
                                        ValueExpr::Param(ref s) | ValueExpr::Local(ref s) => {
                                            AssignValue::SomeByte { source: s.clone() }
                                        }
                                        ValueExpr::IntLit(v) => AssignValue::SomeByte {
                                            source: v.to_string(),
                                        },
                                        _ => AssignValue::None,
                                    }
                                } else {
                                    AssignValue::None
                                }
                            } else {
                                AssignValue::None
                            }
                        }
                        _ => AssignValue::None,
                    }
                }
                _ => return None,
            };
            Some(BehaviorNode::FieldAssign {
                field: name.clone(),
                value,
            })
        }
        ResolvedPlace::Local { index, name } => {
            if is_semantic_local(ctx, *index, rvalue) {
                let value = lower_rvalue(ctx, rvalue);
                // Use authored param name, user-provided name, or stable tmp name.
                let binding_name = if let Some(authored) = ctx.authored_param_name(*index) {
                    authored.to_string()
                } else if let Some(ref n) = name {
                    if is_mir_generated_local_name(n) {
                        format!("tmp{index}")
                    } else {
                        n.clone()
                    }
                } else {
                    format!("tmp{index}")
                };
                Some(BehaviorNode::LocalBinding {
                    name: binding_name,
                    mutable: true,
                    value,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Check if a local name is a MIR-generated name (pattern: `_` followed by digits).
fn is_mir_generated_local_name(name: &str) -> bool {
    if let Some(rest) = name.strip_prefix('_') {
        !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
    } else {
        false
    }
}

/// Check if a local variable assignment is semantically meaningful.
///
/// For locals with user-provided names (not `_N` pattern), all
/// non-discriminant, non-reference assignments are semantic.
///
/// For `_N`-named locals (compiler temporaries), only value-producing
/// computations on simple scalar types are promoted to semantic locals.
/// Register/resource wrapper chain temporaries, discriminants, and
/// references remain structural (dropped).
fn is_semantic_local(ctx: &FnContext<'_>, index: u32, rvalue: &MirRvalue) -> bool {
    // Always reject discriminant and reference assignments.
    if matches!(rvalue, MirRvalue::Discriminant { .. }) {
        return false;
    }
    if matches!(
        rvalue,
        MirRvalue::Ref { .. } | MirRvalue::CopyForDeref { .. }
    ) {
        return false;
    }

    // Locals with no debug name at all are not semantic.
    let name = match ctx.local_name(index) {
        Some(n) => n,
        None => return false,
    };

    // If the local has a non-_N user-provided name, it is always semantic.
    if !is_mir_generated_local_name(name) {
        return true;
    }

    // For _N-named locals: only promote if value-producing AND simple scalar type.
    let is_value_producing = matches!(
        rvalue,
        MirRvalue::BinaryOp { .. } | MirRvalue::Cast { .. } | MirRvalue::UnaryOp { .. }
    );
    if !is_value_producing {
        return false;
    }

    // Check if the local's type is a simple scalar.
    if let Some(decl) = ctx.local_decls.get(index as usize) {
        is_simple_scalar_type(&decl.ty, ctx)
    } else {
        false
    }
}

/// Check if a MIR type is a simple scalar suitable for C local declaration.
fn is_simple_scalar_type(ty: &MirTy, ctx: &FnContext<'_>) -> bool {
    match ty {
        MirTy::Uint { .. } | MirTy::Int { .. } | MirTy::Bool => true,
        MirTy::Adt { path, .. } => {
            // Reject register wrappers, resource types, Option, Result, etc.
            if ctx.shared.is_register_wrapper_type(path) {
                return false;
            }
            if ctx
                .shared
                .resource_type_paths
                .iter()
                .any(|p| path.contains(p))
            {
                return false;
            }
            if path.contains("Option")
                || path.contains("Result")
                || path.contains("ControlFlow")
                || path.contains("FieldValue")
                || path.contains("Mmio")
            {
                return false;
            }
            false // Conservative: unknown ADTs are not simple scalars
        }
        MirTy::Ref { .. } | MirTy::Tuple { .. } => false,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Cleanup block lowering
// ---------------------------------------------------------------------------

/// Lower a cleanup block into behavior nodes.
fn lower_cleanup_block(ctx: &FnContext<'_>, body: &MirBody, bb_index: usize) -> Vec<BehaviorNode> {
    if bb_index >= body.basic_blocks.len() {
        return vec![];
    }
    let bb = &body.basic_blocks[bb_index];
    let mut nodes = lower_statements(ctx, bb);

    match bb.terminator.as_ref().map(|t| &t.kind) {
        Some(MirTerminatorKind::Drop { target, .. }) => {
            nodes.extend(lower_cleanup_block(ctx, body, *target as usize));
        }
        Some(MirTerminatorKind::Goto { target }) => {
            nodes.extend(lower_cleanup_block(ctx, body, *target as usize));
        }
        _ => {}
    }

    nodes
}
