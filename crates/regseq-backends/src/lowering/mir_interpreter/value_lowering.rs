//! MIR rvalue and operand lowering to portable value expressions.
//!
//! Converts MIR operands, rvalues, and branch conditions into the
//! portable `ValueExpr` and `BranchCondition` types used by the
//! behavioral plan.

use regseq_model::declaration::mir_body::{
    MirBasicBlockData, MirBinOp, MirBody, MirConstValue, MirOperand, MirPlace, MirRvalue,
    MirStatementKind, MirTerminatorKind, MirTy,
};

use crate::lowering::behavior::{BinOp, BranchCondition, ReturnSemantic, ValueExpr};

use super::context::FnContext;
use super::place_resolver::{resolve_place, ResolvedPlace};

/// Lower a MIR operand to a `ValueExpr`.
pub fn lower_operand(ctx: &FnContext<'_>, operand: &MirOperand) -> ValueExpr {
    match operand {
        MirOperand::Copy { place } | MirOperand::Move { place } => lower_place_to_value(ctx, place),
        MirOperand::Constant { constant } => match &constant.value {
            MirConstValue::Scalar { bits, .. } => {
                if constant.ty == MirTy::Bool {
                    ValueExpr::BoolLit(*bits != 0)
                } else {
                    ValueExpr::IntLit(*bits as u64)
                }
            }
            MirConstValue::ZeroSized => ValueExpr::IntLit(0),
            MirConstValue::Unsupported { description } => {
                ValueExpr::Param(format!("unsupported_{description}"))
            }
        },
    }
}

/// Lower a MIR place reference to a `ValueExpr`.
fn lower_place_to_value(ctx: &FnContext<'_>, place: &MirPlace) -> ValueExpr {
    let resolved = resolve_place(ctx, place);
    match resolved {
        ResolvedPlace::ReturnPlace => ValueExpr::Local("_0".into()),
        ResolvedPlace::SelfRef => ValueExpr::FieldRef("self".into()),
        ResolvedPlace::DriverField { name } => ValueExpr::FieldRef(name),
        ResolvedPlace::MmioBlock { mmio_field } => ValueExpr::FieldRef(mmio_field),
        ResolvedPlace::RegisterAccess { register, .. } => ValueExpr::FieldRef(register),
        ResolvedPlace::ResourceField { name, .. } => ValueExpr::FieldRef(name),
        ResolvedPlace::Local { index, name } => {
            ValueExpr::Local(name.unwrap_or_else(|| format!("_{index}")))
        }
        ResolvedPlace::Param { name, index } => {
            ValueExpr::Param(name.unwrap_or_else(|| format!("_arg{index}")))
        }
        ResolvedPlace::Unknown => ValueExpr::IntLit(0),
    }
}

/// Lower a MIR rvalue to a `ValueExpr`.
pub fn lower_rvalue(ctx: &FnContext<'_>, rvalue: &MirRvalue) -> ValueExpr {
    match rvalue {
        MirRvalue::Use { operand } => lower_operand(ctx, operand),
        MirRvalue::BinaryOp { op, lhs, rhs } => {
            let bin_op = lower_bin_op(op);
            ValueExpr::BinOp {
                op: bin_op,
                lhs: Box::new(lower_operand(ctx, lhs)),
                rhs: Box::new(lower_operand(ctx, rhs)),
            }
        }
        MirRvalue::Cast { operand, ty, .. } => ValueExpr::Cast {
            expr: Box::new(lower_operand(ctx, operand)),
            ty: format_mir_ty(ty),
        },
        MirRvalue::Ref { place, .. } | MirRvalue::CopyForDeref { place } => {
            lower_place_to_value(ctx, place)
        }
        MirRvalue::Discriminant { place } => {
            let base = lower_place_to_value(ctx, place);
            ValueExpr::MethodResult {
                callee: "discriminant".into(),
                args: vec![base],
            }
        }
        MirRvalue::Aggregate { agg_kind, operands } => lower_aggregate(ctx, agg_kind, operands),
        MirRvalue::UnaryOp { operand, .. } => lower_operand(ctx, operand),
        MirRvalue::Unsupported { description } => {
            ValueExpr::Param(format!("unsupported_{description}"))
        }
    }
}

/// Lower a MIR aggregate rvalue.
fn lower_aggregate(
    ctx: &FnContext<'_>,
    agg_kind: &regseq_model::declaration::mir_body::MirAggregateKind,
    operands: &[MirOperand],
) -> ValueExpr {
    use regseq_model::declaration::mir_body::MirAggregateKind;
    match agg_kind {
        MirAggregateKind::Adt {
            path,
            variant_index,
            ..
        } => {
            if path.contains("Option") || path.contains("Result") {
                if *variant_index == 0 && operands.is_empty() {
                    return ValueExpr::None;
                }
                if *variant_index == 1 && operands.len() == 1 {
                    return ValueExpr::Some(Box::new(lower_operand(ctx, &operands[0])));
                }
            }
            if operands.is_empty() {
                ValueExpr::IntLit(0)
            } else {
                lower_operand(ctx, &operands[0])
            }
        }
        MirAggregateKind::Tuple => {
            if operands.is_empty() {
                ValueExpr::IntLit(0) // unit
            } else {
                lower_operand(ctx, &operands[0])
            }
        }
        _ => ValueExpr::IntLit(0),
    }
}

/// Lower a MIR binary operator.
fn lower_bin_op(op: &MirBinOp) -> BinOp {
    match op {
        MirBinOp::Add | MirBinOp::AddUnchecked => BinOp::Add,
        MirBinOp::Sub | MirBinOp::SubUnchecked => BinOp::Sub,
        MirBinOp::Mul | MirBinOp::MulUnchecked => BinOp::Mul,
        MirBinOp::Div => BinOp::Div,
        MirBinOp::Shl | MirBinOp::ShlUnchecked => BinOp::Shl,
        MirBinOp::Shr | MirBinOp::ShrUnchecked => BinOp::Shr,
        MirBinOp::BitAnd => BinOp::BitAnd,
        MirBinOp::BitOr => BinOp::BitOr,
        MirBinOp::Eq => BinOp::Eq,
        MirBinOp::Ne => BinOp::Ne,
        _ => BinOp::Add, // fallback for unsupported ops
    }
}

/// Lower a SwitchInt condition to a `BranchCondition`.
///
/// Examines the discriminant operand and preceding statements in the
/// block to determine the semantic condition.
pub fn lower_branch_condition(
    ctx: &FnContext<'_>,
    bb_index: usize,
    body: &MirBody,
) -> BranchCondition {
    let bb = &body.basic_blocks[bb_index];
    let term = match &bb.terminator {
        Some(t) => t,
        None => {
            return BranchCondition::ValueTest {
                expr: ValueExpr::BoolLit(true),
            }
        }
    };

    let (discr, _targets) = match &term.kind {
        MirTerminatorKind::SwitchInt { discr, targets } => (discr, targets),
        _ => {
            return BranchCondition::ValueTest {
                expr: ValueExpr::BoolLit(true),
            }
        }
    };

    // Check if the discriminant was computed by a preceding statement.
    let discr_local = match discr {
        MirOperand::Copy { place } | MirOperand::Move { place } => {
            if place.projection.is_empty() {
                Some(place.local)
            } else {
                None
            }
        }
        _ => None,
    };

    if let Some(local) = discr_local {
        // Look for the assignment to this local in the block's statements.
        for stmt in bb.statements.iter().rev() {
            if let MirStatementKind::Assign { place, rvalue } = &stmt.kind {
                if place.local == local && place.projection.is_empty() {
                    return lower_rvalue_to_condition(ctx, rvalue, body, bb);
                }
            }
        }
    }

    // Fall back to lowering the discriminant operand directly.
    BranchCondition::ValueTest {
        expr: lower_operand(ctx, discr),
    }
}

/// Lower a computed rvalue to a branch condition.
fn lower_rvalue_to_condition(
    ctx: &FnContext<'_>,
    rvalue: &MirRvalue,
    _body: &MirBody,
    bb: &MirBasicBlockData,
) -> BranchCondition {
    match rvalue {
        // Discriminant on a Result/Option → used for ? or Option check.
        MirRvalue::Discriminant { place } => {
            let resolved = resolve_place(ctx, place);
            if let ResolvedPlace::DriverField { ref name } = resolved {
                return BranchCondition::FieldIsNone {
                    field: name.clone(),
                };
            }
            BranchCondition::ValueTest {
                expr: lower_place_to_value(ctx, place),
            }
        }
        // Binary comparison: lhs op rhs.
        MirRvalue::BinaryOp { op, lhs, rhs } => lower_binop_to_condition(ctx, op, lhs, rhs, bb),
        // Use of a value directly (e.g., boolean).
        MirRvalue::Use { operand } => {
            let val = lower_operand(ctx, operand);
            BranchCondition::ValueTest { expr: val }
        }
        _ => BranchCondition::ValueTest {
            expr: lower_rvalue(ctx, rvalue),
        },
    }
}

/// Lower a binary operation to a branch condition.
fn lower_binop_to_condition(
    ctx: &FnContext<'_>,
    op: &MirBinOp,
    lhs: &MirOperand,
    rhs: &MirOperand,
    bb: &MirBasicBlockData,
) -> BranchCondition {
    match op {
        MirBinOp::Eq | MirBinOp::Ne => {
            let lhs_val = lower_operand(ctx, lhs);
            let rhs_val = lower_operand(ctx, rhs);

            // Check for field == 0 or field != 0 patterns (register read results).
            if is_zero(&rhs_val) {
                match *op {
                    MirBinOp::Eq => {
                        return BranchCondition::ValueTest { expr: lhs_val };
                    }
                    MirBinOp::Ne => {
                        return BranchCondition::ValueTest { expr: lhs_val };
                    }
                    _ => {}
                }
            }

            BranchCondition::ValueTest {
                expr: ValueExpr::BinOp {
                    op: lower_bin_op(op),
                    lhs: Box::new(lhs_val),
                    rhs: Box::new(rhs_val),
                },
            }
        }
        MirBinOp::BitAnd => {
            // status & MASK pattern — check if this is interrupt class testing.
            //
            // Two cases:
            // 1. Scalar constant: MIR has resolved the const value, so
            //    extract_constant_value returns Some(mask). Class name
            //    encodes the hex value (e.g., "class_0x0050").
            // 2. Unsupported/unevaluated constant: MIR has a symbolic
            //    reference. Extract the path and derive a class name
            //    from the constant name (e.g., "rx" from "RX_INTERRUPT_MASK").
            //    Mask value starts at 0 and is resolved by the post-pass.
            let mask_value = extract_constant_value(rhs);
            if let Some(mask) = mask_value {
                let source = lower_operand(ctx, lhs);
                if let ValueExpr::Local(ref name) | ValueExpr::Param(ref name) = source {
                    return BranchCondition::InterruptClassActive {
                        status_source: name.clone(),
                        class_name: derive_interrupt_class_name(mask, bb),
                    };
                }
            }
            // Try symbolic path from unevaluated constant.
            if let Some(path) = extract_constant_path(rhs) {
                let source = lower_operand(ctx, lhs);
                if let ValueExpr::Local(ref name) | ValueExpr::Param(ref name) = source {
                    return BranchCondition::InterruptClassActive {
                        status_source: name.clone(),
                        class_name: derive_class_name_from_constant_path(&path),
                    };
                }
            }
            BranchCondition::ValueTest {
                expr: ValueExpr::BinOp {
                    op: BinOp::BitAnd,
                    lhs: Box::new(lower_operand(ctx, lhs)),
                    rhs: Box::new(lower_operand(ctx, rhs)),
                },
            }
        }
        _ => BranchCondition::ValueTest {
            expr: ValueExpr::BinOp {
                op: lower_bin_op(op),
                lhs: Box::new(lower_operand(ctx, lhs)),
                rhs: Box::new(lower_operand(ctx, rhs)),
            },
        },
    }
}

/// Check if a value expression is zero.
fn is_zero(expr: &ValueExpr) -> bool {
    matches!(expr, ValueExpr::IntLit(0))
}

/// Extract a constant integer value from an operand.
pub fn extract_constant_value(operand: &MirOperand) -> Option<u32> {
    if let MirOperand::Constant { constant } = operand {
        if let MirConstValue::Scalar { bits, .. } = &constant.value {
            return Some(*bits as u32);
        }
    }
    None
}

/// Extract a symbolic constant path from an `Unsupported` operand.
///
/// When the MIR has unevaluated constant references (e.g.,
/// `"unevaluated:driver::Foo::SOME_MASK"`), this extracts the path
/// so the interpreter can still recognize interrupt class patterns.
pub fn extract_constant_path(operand: &MirOperand) -> Option<String> {
    if let MirOperand::Constant { constant } = operand {
        if let MirConstValue::Unsupported { description } = &constant.value {
            if let Some(path) = description.strip_prefix("unevaluated:") {
                return Some(path.to_string());
            }
        }
    }
    None
}

/// Derive an interrupt class name from a symbolic constant path.
///
/// Converts paths like `"driver::Foo::RX_INTERRUPT_MASK"` to class
/// names like `"rx"` by stripping the type prefix and `_INTERRUPT_MASK`
/// or `_MASK` suffix.
pub fn derive_class_name_from_constant_path(path: &str) -> String {
    // Take the last segment of the path (the const item name).
    let const_name = path.rsplit("::").next().unwrap_or(path);

    // Strip _INTERRUPT_MASK or _MASK suffix, then lowercase.
    let stripped = const_name
        .strip_suffix("_INTERRUPT_MASK")
        .or_else(|| const_name.strip_suffix("_MASK"))
        .unwrap_or(const_name);

    stripped.to_lowercase()
}

/// Derive an interrupt class name from a mask value and block context.
///
/// Uses the mask value directly from the MIR constant, producing a
/// descriptive name based on the value.
fn derive_interrupt_class_name(mask: u32, _bb: &MirBasicBlockData) -> String {
    // The class name encodes the mask value so it is deterministic
    // and identifiable without device-specific knowledge.
    format!("class_{mask:#06x}")
}

/// Determine the return semantic from the MIR function body.
///
/// Examines the return type and the value assigned to the return place
/// (local 0) to classify the return semantic.
pub fn determine_return_semantic(ctx: &FnContext<'_>, body: &MirBody) -> ReturnSemantic {
    let return_ty = &body.local_decls[0].ty;

    // Check return type shape.
    match return_ty {
        MirTy::Tuple { elements } if elements.is_empty() => ReturnSemantic::Void,
        MirTy::Bool => {
            // Look for the value assigned to _0.
            if let Some(val) = find_return_value(body) {
                if let ValueExpr::BoolLit(b) = lower_operand(ctx, &val) {
                    return ReturnSemantic::Bool(b);
                }
            }
            ReturnSemantic::Bool(false)
        }
        MirTy::Adt { path, .. } => {
            if path.contains("Result") {
                ReturnSemantic::Ok
            } else if path.contains("Option") {
                // Check if the return value is None or Some.
                ReturnSemantic::None
            } else if path.contains("IrqResult") {
                // Determine handled vs not handled from the return value.
                ReturnSemantic::ConditionalResult
            } else {
                ReturnSemantic::Void
            }
        }
        _ => ReturnSemantic::Void,
    }
}

/// Find the operand assigned to the return place before a Return terminator.
fn find_return_value(body: &MirBody) -> Option<MirOperand> {
    for bb in &body.basic_blocks {
        if let Some(ref term) = bb.terminator {
            if matches!(term.kind, MirTerminatorKind::Return) {
                // Look for the last assignment to local 0 in this block.
                for stmt in bb.statements.iter().rev() {
                    if let MirStatementKind::Assign { place, rvalue } = &stmt.kind {
                        if place.local == 0 && place.projection.is_empty() {
                            if let MirRvalue::Use { operand } = rvalue.as_ref() {
                                return Some(operand.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Format a MIR type for display in ValueExpr.
fn format_mir_ty(ty: &MirTy) -> String {
    match ty {
        MirTy::Uint { uint_ty } => format!("{uint_ty:?}").to_lowercase(),
        MirTy::Int { int_ty } => format!("{int_ty:?}").to_lowercase(),
        MirTy::Bool => "bool".into(),
        MirTy::Adt { path, .. } => path.clone(),
        _ => "unknown".into(),
    }
}
