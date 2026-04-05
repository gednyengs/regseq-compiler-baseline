//! MIR Call terminator classification.
//!
//! Classifies function calls in MIR based on three inputs:
//! 1. Function path from `MirTy::FnDef`
//! 2. Resolved receiver place from declaration metadata
//! 3. Known type shapes (register wrappers, resource types, intrinsics)

use regseq_model::declaration::mir_body::{MirOperand, MirPlace, MirTy};

use crate::lowering::behavior::ResourceOperation;
use crate::lowering::plan::ResourceKind;

use super::context::FnContext;
use super::place_resolver::{resolve_place, ResolvedPlace};

/// Classification of a MIR Call terminator.
#[derive(Debug, Clone)]
pub enum CallClassification {
    /// Register write operation.
    RegisterWrite { register: String },
    /// Register modify operation.
    RegisterModify { register: String },
    /// Register field read operation.
    RegisterFieldRead { register: String },
    /// Register raw read operation.
    RegisterReadRaw { register: String },
    /// Optional resource operation (clock, reset, pinctrl).
    ResourceOp {
        field: String,
        kind: ResourceKind,
        operation: ResourceOperation,
    },
    /// Call to an authored helper function.
    HelperCall { name: String },
    /// Memory barrier intrinsic (from `ordered!` expansion).
    Barrier,
    /// Register field-value constructor (field_val, raw_reg_val, etc.).
    FieldValueConstructor { path: String },
    /// Transparent deref (core::ops::Deref::deref).
    Deref,
    /// Try trait branch method (Result/ControlFlow branching).
    TryBranch,
    /// Try trait from_residual (error propagation).
    TryFromResidual,
    /// Call not classified into a known category.
    Unknown { path: String },
}

/// Classify a MIR Call terminator.
///
/// Uses the function path, resolved receiver, and declaration metadata
/// to determine the semantic operation.
pub fn classify_call(
    ctx: &FnContext<'_>,
    func: &MirOperand,
    args: &[MirOperand],
) -> CallClassification {
    let path = extract_fn_path(func);

    // 1. Core::ops::Deref::deref — transparent, not emitted.
    if path.contains("core::ops::Deref::deref") {
        return CallClassification::Deref;
    }

    // 2. Try trait methods.
    if path.ends_with("::branch") && path.contains("Try") || path.contains("::branch") {
        // Check if this is actually Try::branch by looking at the path
        // more carefully. In MIR, `<Result<T,E> as Try>::branch` appears.
        if is_try_branch_path(&path) {
            return CallClassification::TryBranch;
        }
    }
    if path.contains("from_residual") {
        return CallClassification::TryFromResidual;
    }

    // 3. Memory barrier (from ordered! expansion).
    if path.contains("memory_barrier") {
        return CallClassification::Barrier;
    }

    // 4. Register operations — grounded in receiver place + function path.
    if let Some(register_op) = try_classify_register_op(ctx, &path, args) {
        return register_op;
    }

    // 5. Resource operations — grounded in receiver place + method name.
    if let Some(resource_op) = try_classify_resource_op(ctx, &path, args) {
        return resource_op;
    }

    // 6. Helper calls — grounded in helper name set + self receiver.
    if let Some(helper) = try_classify_helper_call(ctx, &path, args) {
        return helper;
    }

    // 7. Field value constructors.
    if path.contains("field_val") || path.contains("raw_reg_val") || path.contains("field_from_raw")
    {
        return CallClassification::FieldValueConstructor { path };
    }

    // 8. Operator trait implementations (Add, BitOr, etc.)
    if path.contains("core::ops::") {
        return CallClassification::FieldValueConstructor { path };
    }

    // 9. Unknown — surfaced explicitly.
    CallClassification::Unknown { path }
}

/// Check if a path is the Try::branch method.
fn is_try_branch_path(path: &str) -> bool {
    // In MIR, Try::branch appears as paths like:
    // `<core::result::Result<T, E> as core::ops::Try>::branch`
    // or shorter forms like `branch` on Result types.
    path.contains("Try") && path.contains("branch") || path.ends_with("::branch")
}

/// Extract the function path from a Call's func operand.
fn extract_fn_path(func: &MirOperand) -> String {
    match func {
        MirOperand::Constant { constant } => match &constant.ty {
            MirTy::FnDef { path, .. } => path.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

/// Try to classify as a register operation.
///
/// Checks if the function path matches a register wrapper method
/// AND the receiver resolves to a register access.
fn try_classify_register_op(
    ctx: &FnContext<'_>,
    path: &str,
    args: &[MirOperand],
) -> Option<CallClassification> {
    // Check for known register wrapper method paths.
    let method = if path.contains("::write") && !path.contains("core::") {
        Some("write")
    } else if path.contains("::modify") && !path.contains("core::") {
        Some("modify")
    } else if path.contains("::read_raw") {
        Some("read_raw")
    } else if path.contains("::read") && !path.contains("core::") {
        Some("read")
    } else {
        None
    }?;

    // Verify the path refers to a register wrapper type.
    if !ctx.shared.is_register_wrapper_type(path) {
        return None;
    }

    // Resolve the receiver to confirm it's a register access.
    let register = resolve_receiver_register(ctx, args)?;

    Some(match method {
        "write" => CallClassification::RegisterWrite { register },
        "modify" => CallClassification::RegisterModify { register },
        "read_raw" => CallClassification::RegisterReadRaw { register },
        "read" => CallClassification::RegisterFieldRead { register },
        _ => return None,
    })
}

/// Try to classify as a resource operation.
fn try_classify_resource_op(
    ctx: &FnContext<'_>,
    path: &str,
    args: &[MirOperand],
) -> Option<CallClassification> {
    // Extract method name from the end of the path.
    let method = path.rsplit("::").next()?;

    let operation = match method {
        "enable" => ResourceOperation::Enable,
        "disable" => ResourceOperation::Disable,
        "toggle" => ResourceOperation::Toggle,
        "apply" => ResourceOperation::ApplyState {
            state: "default".into(),
        },
        "get_rate" => return None, // Not a resource op node, just a value
        _ => return None,
    };

    // Check if the path references a known resource type.
    let is_resource_type = ctx
        .shared
        .resource_type_paths
        .iter()
        .any(|prefix| path.contains(prefix));
    if !is_resource_type {
        return None;
    }

    // Resolve the receiver to find the resource field.
    let (field, kind) = resolve_receiver_resource(ctx, args)?;

    Some(CallClassification::ResourceOp {
        field,
        kind,
        operation,
    })
}

/// Try to classify as a helper call.
fn try_classify_helper_call(
    ctx: &FnContext<'_>,
    path: &str,
    args: &[MirOperand],
) -> Option<CallClassification> {
    // Extract the method name from the path.
    let name = path.rsplit("::").next()?;

    if !ctx.shared.is_helper(name) {
        return None;
    }

    // Verify the first argument is a self reference.
    if let Some(first_arg) = args.first() {
        if let Some(place) = operand_place(first_arg) {
            let resolved = resolve_place(ctx, &place);
            if matches!(resolved, ResolvedPlace::SelfRef) {
                return Some(CallClassification::HelperCall {
                    name: name.to_string(),
                });
            }
        }
    }

    // Also accept if the path clearly belongs to the driver type.
    Some(CallClassification::HelperCall {
        name: name.to_string(),
    })
}

/// Resolve the receiver argument to a register name.
fn resolve_receiver_register(ctx: &FnContext<'_>, args: &[MirOperand]) -> Option<String> {
    let first_arg = args.first()?;
    let place = operand_place(first_arg)?;
    let resolved = resolve_place(ctx, &place);
    match resolved {
        ResolvedPlace::RegisterAccess { register, .. } => Some(register),
        _ => None,
    }
}

/// Resolve the receiver argument to a resource field and kind.
fn resolve_receiver_resource(
    ctx: &FnContext<'_>,
    args: &[MirOperand],
) -> Option<(String, ResourceKind)> {
    let first_arg = args.first()?;
    let place = operand_place(first_arg)?;
    let resolved = resolve_place(ctx, &place);
    match resolved {
        ResolvedPlace::ResourceField { name, kind } => Some((name, kind)),
        _ => None,
    }
}

/// Extract the place from an operand, if it has one.
fn operand_place(operand: &MirOperand) -> Option<MirPlace> {
    match operand {
        MirOperand::Copy { place } | MirOperand::Move { place } => Some(place.clone()),
        MirOperand::Constant { .. } => None,
    }
}
