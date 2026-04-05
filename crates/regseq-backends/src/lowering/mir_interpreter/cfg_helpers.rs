//! CFG analysis helpers for the MIR interpreter.
//!
//! Provides control flow structure detection (loops, branches, join
//! points), call node emission, and CFG traversal utilities used by
//! the main `cfg_walker` module.

use std::collections::BTreeSet;

use regseq_model::declaration::mir_body::{
    MirBody, MirOperand, MirPlace, MirTerminatorKind, MirTy, MirUnwindAction,
};

use crate::lowering::behavior::{BehaviorNode, FieldWrite, FieldWriteValue, ValueExpr};

use super::call_classifier::{classify_call, CallClassification};
use super::context::FnContext;
use super::value_lowering::{determine_return_semantic, lower_operand};

// ---------------------------------------------------------------------------
// Call node emission
// ---------------------------------------------------------------------------

/// Emit behavior nodes for a classified call.
pub fn emit_call_node(
    ctx: &FnContext<'_>,
    classification: &CallClassification,
    args: &[MirOperand],
    _destination: &MirPlace,
) -> Vec<BehaviorNode> {
    match classification {
        CallClassification::RegisterWrite { register } => {
            let fields = extract_write_fields(ctx, args);
            vec![BehaviorNode::RegWrite {
                register: register.clone(),
                fields,
            }]
        }
        CallClassification::RegisterModify { register } => {
            let fields = extract_write_fields(ctx, args);
            vec![BehaviorNode::RegModify {
                register: register.clone(),
                fields,
            }]
        }
        CallClassification::RegisterFieldRead { register } => {
            vec![BehaviorNode::RegFieldRead {
                register: register.clone(),
                field: "unknown".into(),
            }]
        }
        CallClassification::RegisterReadRaw { register } => {
            vec![BehaviorNode::RegReadRaw {
                register: register.clone(),
            }]
        }
        CallClassification::ResourceOp {
            field,
            kind,
            operation,
        } => {
            vec![BehaviorNode::ResourceOp {
                resource_kind: kind.clone(),
                field_name: field.clone(),
                operation: operation.clone(),
            }]
        }
        CallClassification::HelperCall { name } => {
            // Skip the receiver (args[0]) — lower remaining arguments.
            let lowered_args = args.iter().skip(1).map(|a| lower_operand(ctx, a)).collect();
            vec![BehaviorNode::CallHelper {
                callee: name.clone(),
                args: lowered_args,
            }]
        }
        CallClassification::OptionMethod { .. } => {
            // Option methods are transparent control flow. The semantics
            // are captured by discriminant-based branching (SwitchInt) or
            // by the value-chain lowering in cfg_walker for unwrap_or.
            vec![]
        }
        CallClassification::SerialTraitMethod { method } => {
            let action = match method.as_str() {
                "rx_char" => "push_rx_char",
                "tx_ready" => "signal_tx_ready",
                "tx_byte" => "push_tx_byte",
                "tx_empty" => "check_tx_empty",
                other => other,
            };
            vec![BehaviorNode::SubsystemAction {
                subsystem: "serial".into(),
                action: action.to_string(),
            }]
        }
        CallClassification::DurationConstructor => {
            // Duration constructors are argument setup for delay/toggle
            // operations. The delay semantic is captured by the
            // ResourceOp::Toggle or Delay node.
            vec![]
        }
        CallClassification::FrameworkValueConstructor { .. } => {
            // Framework value constructors (e.g., SerialErrors::empty)
            // create framework-type values. They don't produce standalone
            // C behavior.
            vec![]
        }
        CallClassification::Unknown { path } => {
            // Explicit allowlist of structural core calls that have no
            // standalone C semantic in the supported slice.
            const STRUCTURAL_CORE_CALLS: &[&str] = &[
                "core::ops::Deref::deref",
                "core::ops::Add::add",
                "core::ops::BitOr::bitor",
                "core::ops::Try::branch",
                "core::ops::FromResidual::from_residual",
            ];

            if STRUCTURAL_CORE_CALLS.iter().any(|&s| path.contains(s)) {
                return vec![];
            }

            // Hard failure for unresolved calls — this is a producer-side
            // classification gap that must be fixed in call_classifier.rs.
            panic!(
                "unclassified call in MIR interpreter: {path}. \
                 Add classification in call_classifier.rs or add to \
                 the structural allowlist in cfg_helpers.rs."
            );
        }
        _ => vec![],
    }
}

/// Extract field-value writes from Call arguments (for register write/modify).
///
/// Produces typed `FieldWrite` entries using only `ValueExpr` variants,
/// never Debug formatting. Panics on unsupported value expressions.
fn extract_write_fields(ctx: &FnContext<'_>, args: &[MirOperand]) -> Vec<FieldWrite> {
    if args.len() < 2 {
        return vec![];
    }
    let value = lower_operand(ctx, &args[1]);
    match &value {
        ValueExpr::IntLit(v) => vec![FieldWrite {
            field: "__raw__".into(),
            value: FieldWriteValue::Raw(*v as u32),
        }],
        ValueExpr::Param(name) => {
            // Check for unevaluated named constants from MIR.
            if let Some(path) = name.strip_prefix("unsupported_unevaluated:") {
                parse_named_value_from_constant_path(path)
            } else {
                vec![FieldWrite {
                    field: "__raw__".into(),
                    value: FieldWriteValue::Variable(name.clone()),
                }]
            }
        }
        ValueExpr::Local(name) => vec![FieldWrite {
            field: "__raw__".into(),
            value: FieldWriteValue::Variable(name.clone()),
        }],
        ValueExpr::FieldRef(name) => vec![FieldWrite {
            field: "__raw__".into(),
            value: FieldWriteValue::Variable(name.clone()),
        }],
        ValueExpr::BinOp { .. } => {
            // Computed expression — emit as a variable reference to the
            // operand that produced it (the whole expression is a value).
            // The actual value will be provided by the caller's MIR local.
            vec![FieldWrite {
                field: "__raw__".into(),
                value: FieldWriteValue::Variable("expr".into()),
            }]
        }
        _ => {
            panic!(
                "extract_write_fields: unsupported value expression {value:?} — \
                 extend FieldWriteValue or add a new lowering path"
            );
        }
    }
}

/// Parse a named value from a constant path like `"registers::LCRH_REG::FEN::DISABLED"`.
///
/// Returns a `FieldWrite` with the field name and named value extracted from
/// the path components.
fn parse_named_value_from_constant_path(path: &str) -> Vec<FieldWrite> {
    let parts: Vec<&str> = path.split("::").collect();
    if parts.len() >= 4 {
        // Pattern: registers::REG_NAME::FIELD::VALUE
        let field = parts[parts.len() - 2].to_string();
        let val_name = parts[parts.len() - 1].to_string();
        vec![FieldWrite {
            field,
            value: FieldWriteValue::Named(val_name),
        }]
    } else if parts.len() >= 2 {
        // Pattern: TYPE::CONST_NAME — use as a variable reference
        let const_name = parts[parts.len() - 1].to_string();
        vec![FieldWrite {
            field: "__raw__".into(),
            value: FieldWriteValue::Variable(const_name),
        }]
    } else {
        vec![FieldWrite {
            field: "__raw__".into(),
            value: FieldWriteValue::Variable(path.to_string()),
        }]
    }
}

// ---------------------------------------------------------------------------
// Fallible ? pattern detection
// ---------------------------------------------------------------------------

/// Detected fallible ? pattern targets.
pub struct FalliblePattern {
    /// Block index of the success (Continue) path.
    pub ok_bb: usize,
    /// Block index of the error (Break) path.
    pub err_bb: usize,
}

/// Detect a fallible ? pattern following a Call terminator.
///
/// After a Call, if the next block calls `Try::branch` followed by a
/// SwitchInt on ControlFlow, this is a desugared `?` operator. Uses the
/// MIR's own control flow structure for detection.
pub fn detect_fallible_pattern(
    body: &MirBody,
    next_bb: Option<usize>,
    visited: &BTreeSet<usize>,
    _stop_set: &BTreeSet<usize>,
) -> Option<FalliblePattern> {
    let next = next_bb?;
    if next >= body.basic_blocks.len() || visited.contains(&next) {
        return None;
    }

    let next_block = &body.basic_blocks[next];
    if let Some(ref term) = next_block.terminator {
        if let MirTerminatorKind::Call {
            func,
            target: Some(switch_bb),
            ..
        } = &term.kind
        {
            let path = extract_call_path(func);
            if path.contains("branch") {
                let switch_idx = *switch_bb as usize;
                if switch_idx < body.basic_blocks.len() {
                    let switch_block = &body.basic_blocks[switch_idx];
                    if let Some(ref sw_term) = switch_block.terminator {
                        if let MirTerminatorKind::SwitchInt { targets, .. } = &sw_term.kind {
                            if targets.targets.len() >= 2 {
                                let ok_bb = targets.targets[0] as usize;
                                let err_bb = targets.targets[1] as usize;
                                return Some(FalliblePattern { ok_bb, err_bb });
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Walk the error path of a fallible ? to collect cleanup actions.
pub fn walk_error_path(
    ctx: &FnContext<'_>,
    body: &MirBody,
    err_bb: usize,
    visited: &BTreeSet<usize>,
    stop_set: &BTreeSet<usize>,
) -> Vec<BehaviorNode> {
    let mut nodes = Vec::new();
    let mut current = err_bb;
    let mut seen = visited.clone();

    while current < body.basic_blocks.len() && !stop_set.contains(&current) && seen.insert(current)
    {
        let bb = &body.basic_blocks[current];
        let Some(term) = &bb.terminator else {
            break;
        };

        match &term.kind {
            MirTerminatorKind::Goto { target } => {
                current = *target as usize;
            }
            MirTerminatorKind::Drop { target, unwind, .. } => {
                if let MirUnwindAction::Cleanup { target: cleanup_bb } = unwind {
                    let cleanup = walk_error_path(ctx, body, *cleanup_bb as usize, &seen, stop_set);
                    if !cleanup.is_empty() {
                        nodes.push(BehaviorNode::CleanupRegion {
                            body: vec![],
                            cleanup,
                        });
                    }
                }
                current = *target as usize;
            }
            MirTerminatorKind::Call {
                func,
                args,
                destination,
                target,
                unwind,
            } => {
                let classification = classify_call(ctx, func, args);
                nodes.extend(emit_call_node(ctx, &classification, args, destination));

                if let MirUnwindAction::Cleanup { target: cleanup_bb } = unwind {
                    let cleanup = walk_error_path(ctx, body, *cleanup_bb as usize, &seen, stop_set);
                    if !cleanup.is_empty() {
                        nodes.push(BehaviorNode::CleanupRegion {
                            body: vec![],
                            cleanup,
                        });
                    }
                }

                if let Some(next) = target {
                    current = *next as usize;
                } else {
                    break;
                }
            }
            MirTerminatorKind::Assert { target, unwind, .. } => {
                if let MirUnwindAction::Cleanup { target: cleanup_bb } = unwind {
                    let cleanup = walk_error_path(ctx, body, *cleanup_bb as usize, &seen, stop_set);
                    if !cleanup.is_empty() {
                        nodes.push(BehaviorNode::CleanupRegion {
                            body: vec![],
                            cleanup,
                        });
                    }
                }
                current = *target as usize;
            }
            MirTerminatorKind::Return => {
                nodes.push(BehaviorNode::Return {
                    semantic: determine_return_semantic(ctx, body),
                });
                break;
            }
            MirTerminatorKind::SwitchInt { targets, .. } => {
                for target in &targets.targets {
                    nodes.extend(walk_error_path(
                        ctx,
                        body,
                        *target as usize,
                        &seen,
                        stop_set,
                    ));
                }
                break;
            }
            MirTerminatorKind::Unreachable
            | MirTerminatorKind::UnwindResume
            | MirTerminatorKind::UnwindTerminate { .. }
            | MirTerminatorKind::Unsupported { .. } => {
                break;
            }
        }
    }

    nodes
}

// ---------------------------------------------------------------------------
// Control flow structure detection
// ---------------------------------------------------------------------------

/// Check if any SwitchInt target points to an already-visited block (back-edge).
pub fn has_back_edge(
    _body: &MirBody,
    targets: &regseq_model::declaration::mir_body::MirSwitchTargets,
    visited: &BTreeSet<usize>,
) -> bool {
    targets
        .targets
        .iter()
        .any(|t| visited.contains(&(*t as usize)))
}

/// Identify loop entry and exit blocks from SwitchInt targets.
pub fn identify_loop_structure(
    targets: &regseq_model::declaration::mir_body::MirSwitchTargets,
    visited: &BTreeSet<usize>,
) -> (usize, usize) {
    let mut body_entry = targets.targets[0] as usize;
    let mut exit = if targets.targets.len() > 1 {
        targets.targets[targets.targets.len() - 1] as usize
    } else {
        body_entry
    };

    for t in &targets.targets {
        let idx = *t as usize;
        if visited.contains(&idx) {
            body_entry = idx;
        } else if idx != body_entry {
            exit = idx;
        }
    }

    for t in &targets.targets {
        let idx = *t as usize;
        if !visited.contains(&idx) && idx != exit {
            body_entry = idx;
            break;
        }
    }

    (body_entry, exit)
}

/// Split SwitchInt targets into then/else branches.
pub fn split_switch_targets(
    targets: &regseq_model::declaration::mir_body::MirSwitchTargets,
) -> (usize, usize) {
    let then_bb = if targets.targets.len() > 1 {
        targets.targets[targets.targets.len() - 1] as usize
    } else {
        targets.targets[0] as usize
    };
    let else_bb = targets.targets[0] as usize;
    (then_bb, else_bb)
}

/// Find the join point where two branch arms converge.
pub fn find_join_point(
    body: &MirBody,
    then_bb: usize,
    else_bb: usize,
    stop_set: &BTreeSet<usize>,
) -> Option<usize> {
    let then_reachable = collect_reachable(body, then_bb, stop_set, 50);
    let else_reachable = collect_reachable(body, else_bb, stop_set, 50);

    let mut common: Vec<usize> = then_reachable
        .intersection(&else_reachable)
        .copied()
        .collect();
    common.sort();
    common.into_iter().next()
}

/// Collect blocks reachable from a starting block, up to a limit.
fn collect_reachable(
    body: &MirBody,
    start: usize,
    stop_set: &BTreeSet<usize>,
    limit: usize,
) -> BTreeSet<usize> {
    let mut reachable = BTreeSet::new();
    let mut worklist = vec![start];
    let mut steps = 0;

    while let Some(bb) = worklist.pop() {
        if steps >= limit || !reachable.insert(bb) || stop_set.contains(&bb) {
            continue;
        }
        steps += 1;

        if bb >= body.basic_blocks.len() {
            continue;
        }
        let block = &body.basic_blocks[bb];
        if let Some(ref term) = block.terminator {
            for succ in successor_blocks(&term.kind) {
                worklist.push(succ);
            }
        }
    }

    reachable
}

/// Return all successor block indices for a terminator.
pub fn successor_blocks(kind: &MirTerminatorKind) -> Vec<usize> {
    match kind {
        MirTerminatorKind::Goto { target } => vec![*target as usize],
        MirTerminatorKind::SwitchInt { targets, .. } => {
            targets.targets.iter().map(|t| *t as usize).collect()
        }
        MirTerminatorKind::Call { target, unwind, .. } => {
            let mut succs = Vec::new();
            if let Some(t) = target {
                succs.push(*t as usize);
            }
            if let MirUnwindAction::Cleanup { target } = unwind {
                succs.push(*target as usize);
            }
            succs
        }
        MirTerminatorKind::Drop { target, unwind, .. } => {
            let mut succs = vec![*target as usize];
            if let MirUnwindAction::Cleanup { target } = unwind {
                succs.push(*target as usize);
            }
            succs
        }
        MirTerminatorKind::Assert { target, unwind, .. } => {
            let mut succs = vec![*target as usize];
            if let MirUnwindAction::Cleanup { target } = unwind {
                succs.push(*target as usize);
            }
            succs
        }
        MirTerminatorKind::Return
        | MirTerminatorKind::Unreachable
        | MirTerminatorKind::UnwindResume
        | MirTerminatorKind::UnwindTerminate { .. }
        | MirTerminatorKind::Unsupported { .. } => vec![],
    }
}

/// Extract the function path from a Call operand.
pub fn extract_call_path(func: &MirOperand) -> String {
    match func {
        MirOperand::Constant { constant } => match &constant.ty {
            MirTy::FnDef { path, .. } => path.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}
