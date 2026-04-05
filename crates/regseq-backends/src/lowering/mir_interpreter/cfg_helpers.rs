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

use super::call_classifier::CallClassification;
use super::context::FnContext;
use super::value_lowering::lower_operand;

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
            vec![BehaviorNode::CallHelper {
                callee: name.clone(),
            }]
        }
        CallClassification::Unknown { path } => {
            vec![BehaviorNode::Eval {
                expr: ValueExpr::MethodResult {
                    callee: path.clone(),
                    args: args.iter().map(|a| lower_operand(ctx, a)).collect(),
                },
            }]
        }
        _ => vec![],
    }
}

/// Extract field-value writes from Call arguments (for register write/modify).
fn extract_write_fields(ctx: &FnContext<'_>, args: &[MirOperand]) -> Vec<FieldWrite> {
    if args.len() >= 2 {
        let value = lower_operand(ctx, &args[1]);
        vec![FieldWrite {
            field: "value".into(),
            value: match value {
                ValueExpr::IntLit(v) => FieldWriteValue::Raw(v as u32),
                _ => FieldWriteValue::Variable(format!("{value:?}")),
            },
        }]
    } else {
        vec![]
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
    _ctx: &FnContext<'_>,
    _body: &MirBody,
    _err_bb: usize,
    _visited: &BTreeSet<usize>,
    _stop_set: &BTreeSet<usize>,
) -> Vec<BehaviorNode> {
    vec![]
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
