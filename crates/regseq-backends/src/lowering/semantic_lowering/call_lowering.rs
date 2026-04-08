//! MIR call classification to semantic `Stmt`/`Expr` lowering.
//!
//! Maps `CallClassification` variants to semantic IR with structured
//! refs resolved through the `ReferenceResolver`. The call lowering
//! module receives the full MIR function path (from `MirTy::Fn`)
//! and passes it to `resolve_call()` for canonical resolution.
//!
//! Serial trait method calls are lowered as `HelperCall` with
//! `FunctionRef`, not as `SubsystemAction`.

use regseq_model::declaration::mir_body::{MirOperand, MirPlace, MirTy};
use regseq_model::semantic::expr::Expr;
use regseq_model::semantic::ops::{FieldWrite, FieldWriteValue, ResourceOperation};
use regseq_model::semantic::refs::RegisterRef;
use regseq_model::semantic::stmt::Stmt;

use super::expr_lowering::{lower_operand, TypeCtx};
use super::local_env::LocalEnvironment;
use super::ref_resolver::ReferenceResolver;
use super::SemanticLoweringError;
use crate::lowering::mir_interpreter::call_classifier::CallClassification;
use crate::lowering::mir_interpreter::context::FnContext;

// ---------------------------------------------------------------------------
// Call lowering
// ---------------------------------------------------------------------------

/// Lower a classified MIR call to semantic `Stmt`s.
///
/// `mir_fn_path` is the full function path from `MirTy::Fn { path }`.
/// Used for canonical resolution of helper calls and serial trait methods.
///
/// Returns a list of statements (often 0 or 1). Transparent calls (Deref,
/// FieldValueConstructor, OptionMethod, TryBranch, etc.) return empty.
#[allow(clippy::too_many_arguments)]
pub fn lower_call(
    fn_ctx: &FnContext<'_>,
    local_env: &mut LocalEnvironment,
    resolver: &ReferenceResolver,
    classification: &CallClassification,
    mir_fn_path: &str,
    args: &[MirOperand],
    _destination: &MirPlace,
    tc: TypeCtx<'_>,
) -> Result<Vec<Stmt>, SemanticLoweringError> {
    match classification {
        CallClassification::RegisterWrite { register } => {
            let reg_ref = resolver.resolve_register(register)?;
            let fields =
                extract_semantic_write_fields(fn_ctx, local_env, resolver, args, &reg_ref, tc)?;
            Ok(vec![Stmt::RegWrite {
                register: reg_ref,
                fields,
            }])
        }

        CallClassification::RegisterModify { register } => {
            let reg_ref = resolver.resolve_register(register)?;
            let fields =
                extract_semantic_write_fields(fn_ctx, local_env, resolver, args, &reg_ref, tc)?;
            Ok(vec![Stmt::RegModify {
                register: reg_ref,
                fields,
            }])
        }

        CallClassification::RegisterFieldRead { .. }
        | CallClassification::RegisterReadRaw { .. } => {
            // Register reads are value-producing and handled as expressions
            // by the walker when the destination local is assigned.
            // The walker creates Stmt::Let { value: Expr::RegReadRaw/RegFieldRead }.
            Ok(vec![])
        }

        CallClassification::ResourceOp {
            field,
            kind,
            operation,
        } => {
            // Try attribute-based resolution first, then fall back to
            // constructing a ResourceRef from the MIR-derived kind and
            // the driver field identity. This handles fields like `reset`
            // that have no #[rseq::*] attribute but are resource-typed.
            let resource_ref = resolver.resolve_resource(field).or_else(|_| {
                let field_ref = resolver.resolve_driver_field(field)?;
                Ok(regseq_model::semantic::refs::ResourceRef {
                    kind: *kind,
                    field: field_ref,
                })
            })?;
            let sem_op = lower_resource_operation(operation);
            Ok(vec![Stmt::ResourceOp {
                resource: resource_ref,
                operation: sem_op,
            }])
        }

        CallClassification::HelperCall { .. } => {
            // Resolve through canonical MIR path, not short name.
            let func_ref = resolver.resolve_call(mir_fn_path)?;
            // Lower arguments (skip self receiver at args[0]).
            let lowered_args = lower_call_args(fn_ctx, local_env, resolver, args, tc)?;
            Ok(vec![Stmt::Eval {
                expr: Expr::HelperCall {
                    callee: func_ref,
                    args: lowered_args,
                },
            }])
        }

        CallClassification::SerialTraitMethod { .. } => {
            // Serial trait methods are intra-body calls to the driver's own
            // trait implementation. Lowered as HelperCall with FunctionRef.
            let func_ref = resolver.resolve_call(mir_fn_path)?;
            let lowered_args = lower_call_args(fn_ctx, local_env, resolver, args, tc)?;
            Ok(vec![Stmt::Eval {
                expr: Expr::HelperCall {
                    callee: func_ref,
                    args: lowered_args,
                },
            }])
        }

        // Transparent calls — no semantic output.
        CallClassification::Barrier => Ok(vec![]),
        CallClassification::Deref => Ok(vec![]),
        CallClassification::FieldValueConstructor { .. } => Ok(vec![]),
        CallClassification::OptionMethod { .. } => Ok(vec![]),
        CallClassification::TryBranch => Ok(vec![]),
        CallClassification::TryFromResidual => Ok(vec![]),
        CallClassification::DurationConstructor => Ok(vec![]),
        CallClassification::FrameworkValueConstructor { .. } => Ok(vec![]),

        CallClassification::Unknown { path } => {
            // Check structural core calls that have no standalone C semantic.
            const STRUCTURAL_CORE_CALLS: &[&str] = &[
                "core::ops::Deref::deref",
                "core::ops::Add::add",
                "core::ops::BitOr::bitor",
                "core::ops::Try::branch",
                "core::ops::FromResidual::from_residual",
            ];
            if STRUCTURAL_CORE_CALLS.iter().any(|s| path.contains(s)) {
                return Ok(vec![]);
            }
            Err(SemanticLoweringError::UnsupportedConstruct {
                function: "call_lowering".into(),
                description: format!("unclassified call: {path}"),
            })
        }
    }
}

/// Create a `HelperCall` expression (for value-producing calls).
///
/// Used by the walker when a call result is assigned to a local.
pub fn make_helper_call_expr(
    fn_ctx: &FnContext<'_>,
    local_env: &mut LocalEnvironment,
    resolver: &ReferenceResolver,
    mir_fn_path: &str,
    args: &[MirOperand],
    tc: TypeCtx<'_>,
) -> Result<Expr, SemanticLoweringError> {
    let func_ref = resolver.resolve_call(mir_fn_path)?;
    let lowered_args = lower_call_args(fn_ctx, local_env, resolver, args, tc)?;
    Ok(Expr::HelperCall {
        callee: func_ref,
        args: lowered_args,
    })
}

/// Create a `RegReadRaw` expression for a register read call.
pub fn make_reg_read_raw_expr(
    resolver: &ReferenceResolver,
    register: &str,
) -> Result<Expr, SemanticLoweringError> {
    let reg_ref = resolver.resolve_register(register)?;
    Ok(Expr::RegReadRaw { register: reg_ref })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Lower call arguments to semantic expressions, skipping the self receiver.
fn lower_call_args(
    fn_ctx: &FnContext<'_>,
    local_env: &mut LocalEnvironment,
    resolver: &ReferenceResolver,
    args: &[MirOperand],
    tc: TypeCtx<'_>,
) -> Result<Vec<Expr>, SemanticLoweringError> {
    // Skip the first argument (self receiver) for method calls.
    let skip = if fn_ctx.has_self_receiver { 1 } else { 0 };
    args.iter()
        .skip(skip)
        .map(|a| lower_operand(fn_ctx, local_env, resolver, a, tc))
        .collect()
}

/// Extract typed field writes from register write/modify call arguments.
fn extract_semantic_write_fields(
    fn_ctx: &FnContext<'_>,
    local_env: &mut LocalEnvironment,
    resolver: &ReferenceResolver,
    args: &[MirOperand],
    _reg_ref: &RegisterRef,
    tc: TypeCtx<'_>,
) -> Result<Vec<FieldWrite>, SemanticLoweringError> {
    if args.len() < 2 {
        return Ok(vec![]);
    }

    // The second argument is the value being written.
    let value_operand = &args[1];
    let value_expr = lower_operand(fn_ctx, local_env, resolver, value_operand, tc)?;

    // For now, produce a single raw field write. Register field-level
    // resolution requires descriptor data not yet in the hierarchy.
    // The field name is "__raw__" as a transitional representation.
    let field_write = match &value_expr {
        Expr::IntLit { value, .. } => FieldWrite {
            field: placeholder_reg_field_ref(_reg_ref),
            value: FieldWriteValue::Raw {
                value: *value as u32,
            },
        },
        Expr::Constant { constant } => {
            if let Some(v) = constant.known_value {
                FieldWrite {
                    field: placeholder_reg_field_ref(_reg_ref),
                    value: FieldWriteValue::Raw { value: v as u32 },
                }
            } else {
                FieldWrite {
                    field: placeholder_reg_field_ref(_reg_ref),
                    value: FieldWriteValue::Named {
                        name: constant.name.clone(),
                    },
                }
            }
        }
        Expr::LocalRef { local } => FieldWrite {
            field: placeholder_reg_field_ref(_reg_ref),
            value: FieldWriteValue::Local { local: *local },
        },
        _ => FieldWrite {
            field: placeholder_reg_field_ref(_reg_ref),
            value: FieldWriteValue::Computed {
                expr: Box::new(value_expr),
            },
        },
    };

    Ok(vec![field_write])
}

/// Create a placeholder register field ref for raw writes.
///
/// Register field-level resolution (individual bitfields) requires
/// register descriptor data. For the current slice, writes use the
/// whole-register `__raw__` placeholder. Field-level resolution is
/// a future refinement.
pub fn placeholder_reg_field_ref(
    reg_ref: &RegisterRef,
) -> regseq_model::semantic::refs::RegisterFieldRef {
    regseq_model::semantic::refs::RegisterFieldRef {
        register: reg_ref.clone(),
        canonical_path: regseq_model::item::identity::CanonicalPath::new(format!(
            "{}::__raw__",
            reg_ref.canonical_path.as_str()
        )),
        name: "__raw__".into(),
    }
}

/// Extract the function path from a Call's func operand.
pub fn extract_fn_path(func: &MirOperand) -> String {
    match func {
        MirOperand::Constant { constant } => match &constant.ty {
            MirTy::Fn { path, .. } => path.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

/// Map a behavior-layer resource operation to a semantic `ResourceOperation`.
fn lower_resource_operation(
    op: &crate::lowering::behavior::ResourceOperation,
) -> ResourceOperation {
    match op {
        crate::lowering::behavior::ResourceOperation::Enable => ResourceOperation::Enable,
        crate::lowering::behavior::ResourceOperation::Disable => ResourceOperation::Disable,
        crate::lowering::behavior::ResourceOperation::Toggle => {
            ResourceOperation::Toggle { duration_us: None }
        }
        crate::lowering::behavior::ResourceOperation::ApplyState { state } => {
            let pin_state = if state.to_lowercase().contains("sleep") {
                regseq_model::semantic::ops::PinCtrlState::Sleep
            } else {
                regseq_model::semantic::ops::PinCtrlState::Default
            };
            ResourceOperation::ApplyState { state: pin_state }
        }
    }
}
