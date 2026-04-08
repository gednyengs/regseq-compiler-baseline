//! MIR Call terminator classification.
//!
//! Classifies function calls in MIR based on three inputs:
//! 1. Function path from `MirTy::Fn`
//! 2. Resolved receiver place from declaration metadata
//! 3. Known type shapes (register wrappers, resource types, intrinsics)

use regseq_model::declaration::mir_body::{MirOperand, MirPlace, MirTy};

use crate::lowering::behavior::ResourceOperation;
use crate::lowering::plan::ResourceKind;

use super::context::FnContext;
use super::place_resolver::{
    resolve_local_type_as_register, resolve_local_type_as_resource, resolve_place, ResolvedPlace,
};

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
    /// core::option::Option method (transparent control flow).
    OptionMethod { method: String },
    /// regseq::Serial trait method (lowered to SubsystemAction).
    SerialTraitMethod { method: String },
    /// regseq::Duration constructor (argument setup for delay/toggle).
    DurationConstructor,
    /// regseq framework value constructor (e.g., SerialErrors::empty).
    ///
    /// Transparent structural call that creates a framework-type value.
    /// Does not produce standalone C behavior.
    FrameworkValueConstructor { path: String },
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

    // 6. Option methods — transparent control flow.
    if let Some(opt) = try_classify_option_method(&path) {
        return opt;
    }

    // 7. Serial trait methods — lowered to SubsystemAction.
    if let Some(serial) = try_classify_serial_trait_method(&path) {
        return serial;
    }

    // 8. Duration constructors — argument setup for delay/toggle.
    if try_classify_duration_constructor(&path) {
        return CallClassification::DurationConstructor;
    }

    // 9. Framework value constructors (e.g., SerialErrors::empty, IrqResult::*).
    if is_framework_value_constructor(&path) {
        return CallClassification::FrameworkValueConstructor { path };
    }

    // 10. Helper calls — grounded in helper name set + self receiver.
    if let Some(helper) = try_classify_helper_call(ctx, &path, args) {
        return helper;
    }

    // 11. Field value constructors.
    if path.contains("field_val") || path.contains("raw_reg_val") || path.contains("field_from_raw")
    {
        return CallClassification::FieldValueConstructor { path };
    }

    // 12. Operator trait implementations (Add, BitOr, etc.)
    if path.contains("core::ops::") {
        return CallClassification::FieldValueConstructor { path };
    }

    // 13. Unknown — surfaced explicitly.
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
            MirTy::Fn { path, .. } => path.clone(),
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
///
/// First tries place-based resolution (self.regs.cr projection chains).
/// Falls back to type-based resolution for intermediate MIR locals.
fn resolve_receiver_register(ctx: &FnContext<'_>, args: &[MirOperand]) -> Option<String> {
    let first_arg = args.first()?;
    let place = operand_place(first_arg)?;
    let resolved = resolve_place(ctx, &place);
    match resolved {
        ResolvedPlace::RegisterAccess { register, .. } => Some(register),
        ResolvedPlace::Local { index, .. } => {
            // Fallback: check the local's declared type for register wrapper.
            resolve_local_type_as_register(ctx, index)
        }
        _ => None,
    }
}

/// Resolve the receiver argument to a resource field and kind.
///
/// First tries place-based resolution (self.clock projection chains).
/// Falls back to type-based resolution for intermediate MIR locals.
fn resolve_receiver_resource(
    ctx: &FnContext<'_>,
    args: &[MirOperand],
) -> Option<(String, ResourceKind)> {
    let first_arg = args.first()?;
    let place = operand_place(first_arg)?;
    let resolved = resolve_place(ctx, &place);
    match resolved {
        ResolvedPlace::ResourceField { name, kind } => Some((name, kind)),
        ResolvedPlace::Local { index, .. } => {
            // Fallback: check the local's declared type for resource type.
            resolve_local_type_as_resource(ctx, index)
        }
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

// ---------------------------------------------------------------------------
// New call classification helpers
// ---------------------------------------------------------------------------

/// Try to classify a `core::option::Option` method call.
fn try_classify_option_method(path: &str) -> Option<CallClassification> {
    if !path.contains("core::option::Option") {
        return None;
    }
    let method = path.rsplit("::").next()?;
    match method {
        "as_ref" | "map" | "unwrap_or" | "is_some" | "is_none" | "unwrap" | "expect" => {
            Some(CallClassification::OptionMethod {
                method: method.to_string(),
            })
        }
        _ => None,
    }
}

/// Try to classify a `regseq::Serial` trait method call.
fn try_classify_serial_trait_method(path: &str) -> Option<CallClassification> {
    if !path.contains("Serial::") {
        return None;
    }
    let method = path.rsplit("::").next()?;
    match method {
        "rx_char" | "tx_ready" | "tx_empty" | "tx_byte" | "push_rx_char" | "signal_tx_ready" => {
            Some(CallClassification::SerialTraitMethod {
                method: method.to_string(),
            })
        }
        _ => None,
    }
}

/// Try to classify a `regseq::Duration` constructor call.
fn try_classify_duration_constructor(path: &str) -> bool {
    path.contains("Duration::") && {
        let method = path.rsplit("::").next().unwrap_or("");
        matches!(method, "from_us" | "from_ms" | "from_ns")
    }
}

/// Check if a call is a framework value constructor.
///
/// These are `regseq::*` infrastructure calls that are structurally
/// transparent — they exist only for type-level or control-flow purposes
/// and have no standalone C semantic.
///
/// Referenced supported external callables (RxChar::new, SerialErrors::empty,
/// etc.) are NOT included here. They are materialized as first-class modeled
/// items and flow through ordinary modeled identity.
fn is_framework_value_constructor(path: &str) -> bool {
    // Clock::get_rate is a value-producing call handled by the
    // resource-property-read chain (option_lowering bridge).
    if path.contains("Clock::get_rate") {
        return true;
    }
    false
}
