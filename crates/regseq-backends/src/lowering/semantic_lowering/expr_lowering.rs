//! MIR operand and rvalue to semantic `Expr` lowering.
//!
//! Replaces `value_lowering.rs` output with structured `Expr` types
//! carrying `LocalId`, `DriverFieldRef`, `ConstantRef`, etc.
//!
//! All expression lowering routes through the `ReferenceResolver` for
//! canonical identity resolution. No string-based references survive.

use regseq_model::declaration::mir_body::{
    MirAggregateKind, MirBinOp, MirBody, MirConstValue, MirOperand, MirPlace, MirRvalue,
    MirStatementKind, MirTerminatorKind, MirTy, MirUnOp,
};
use regseq_model::item::cross_link::CrossLinkTable;
use regseq_model::item::identity::ItemStore;
use regseq_model::semantic::expr::Expr;
use regseq_model::semantic::ops::BinOp;
use regseq_model::semantic::types::SemanticType;

use super::local_env::LocalEnvironment;
use super::ref_resolver::ReferenceResolver;
use super::type_lowering::lower_mir_ty;
use super::SemanticLoweringError;
use crate::lowering::mir_interpreter::context::FnContext;
use crate::lowering::mir_interpreter::place_resolver::{resolve_place, ResolvedPlace};

/// Shorthand type for the hierarchy type-resolution context.
pub type TypeCtx<'a> = (&'a CrossLinkTable, &'a ItemStore);

// ---------------------------------------------------------------------------
// Operand lowering
// ---------------------------------------------------------------------------

/// Lower a MIR operand to a semantic `Expr`.
pub fn lower_operand(
    fn_ctx: &FnContext<'_>,
    local_env: &mut LocalEnvironment,
    resolver: &ReferenceResolver,
    operand: &MirOperand,
    tc: TypeCtx<'_>,
) -> Result<Expr, SemanticLoweringError> {
    match operand {
        MirOperand::Copy { place } | MirOperand::Move { place } => {
            lower_place_to_expr(fn_ctx, local_env, resolver, place, tc)
        }
        MirOperand::Constant { constant } => {
            // Check for unevaluated constant (symbolic path).
            if let MirConstValue::Unsupported { description } = &constant.value {
                if let Some(path) = description.strip_prefix("unevaluated:") {
                    // Register field value paths (e.g., "registers::LCRH_REG::FEN::DISABLED")
                    // are not authored ConstantDef items — they are register descriptor
                    // named values. Produce an IntLit placeholder.
                    if path.contains("_REG::") || path.contains("registers::") {
                        return Ok(Expr::IntLit {
                            value: 0,
                            ty: SemanticType::U32,
                        });
                    }
                    // Authored constants (e.g., driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ).
                    let constant_ref = resolver.resolve_constant(path)?;
                    return Ok(Expr::Constant {
                        constant: constant_ref,
                    });
                }
            }

            match &constant.value {
                MirConstValue::Scalar { bits, .. } => {
                    if constant.ty == MirTy::Bool {
                        Ok(Expr::BoolLit { value: *bits != 0 })
                    } else {
                        let ty = lower_mir_ty(&constant.ty, tc.0, tc.1);
                        Ok(Expr::IntLit {
                            value: *bits as i128,
                            ty,
                        })
                    }
                }
                MirConstValue::ZeroSized => Ok(Expr::IntLit {
                    value: 0,
                    ty: SemanticType::Void,
                }),
                MirConstValue::Unsupported { description } => Ok(Expr::IntLit {
                    value: 0,
                    ty: SemanticType::Opaque {
                        description: format!("unsupported constant: {description}"),
                    },
                }),
            }
        }
    }
}

/// Lower a MIR place to a semantic `Expr`.
fn lower_place_to_expr(
    fn_ctx: &FnContext<'_>,
    local_env: &mut LocalEnvironment,
    resolver: &ReferenceResolver,
    place: &MirPlace,
    tc: TypeCtx<'_>,
) -> Result<Expr, SemanticLoweringError> {
    let resolved = resolve_place(fn_ctx, place);
    match resolved {
        ResolvedPlace::ReturnPlace => {
            // Reference to the return place. If it has a semantic local, use it.
            if let Some(id) = local_env.mir_local(0) {
                Ok(Expr::LocalRef { local: id })
            } else {
                Ok(Expr::IntLit {
                    value: 0,
                    ty: SemanticType::Void,
                })
            }
        }
        ResolvedPlace::SelfRef => {
            // Reference to self. Return LocalRef to the self parameter.
            if let Some(id) = local_env.mir_local(1) {
                Ok(Expr::LocalRef { local: id })
            } else {
                Ok(Expr::IntLit {
                    value: 0,
                    ty: SemanticType::Opaque {
                        description: "self ref without local".into(),
                    },
                })
            }
        }
        ResolvedPlace::DriverField { name } => {
            // Guard: the place resolver sometimes produces type paths
            // (e.g., "core::option::Option") instead of real field names.
            if name.contains("::") {
                return Ok(Expr::IntLit {
                    value: 0,
                    ty: SemanticType::Opaque {
                        description: format!("type path in place: {name}"),
                    },
                });
            }
            let field_ref = resolver.resolve_driver_field(&name)?;
            Ok(Expr::DriverFieldRead { field: field_ref })
        }
        ResolvedPlace::MmioBlock { .. } | ResolvedPlace::RegisterAccess { .. } => {
            // Register access through place projection is not a direct
            // value expression — it is handled by call_lowering when the
            // register wrapper method call is classified.
            Ok(Expr::IntLit {
                value: 0,
                ty: SemanticType::Opaque {
                    description: "register place (handled by call lowering)".into(),
                },
            })
        }
        ResolvedPlace::ResourceField { name, .. } => {
            let field_ref = resolver.resolve_driver_field(&name)?;
            Ok(Expr::DriverFieldRead { field: field_ref })
        }
        ResolvedPlace::Local { index, .. } => {
            // Follow copy chain to find the ultimate source.
            let source = fn_ctx.resolve_copy_chain(index);

            // Check if the source has a semantic local.
            if let Some(id) = local_env.mir_local(source) {
                return Ok(Expr::LocalRef { local: id });
            }
            // Check the original local too.
            if let Some(id) = local_env.mir_local(index) {
                return Ok(Expr::LocalRef { local: id });
            }

            // On-demand promotion: if this local was not pre-materialized,
            // promote it now with its MIR-declared type.
            let ty = fn_ctx
                .local_decls
                .get(source as usize)
                .map(|d| super::type_lowering::lower_mir_ty(&d.ty, tc.0, tc.1))
                .unwrap_or(SemanticType::U32);
            let id = local_env.promote_temp(source, ty);
            Ok(Expr::LocalRef { local: id })
        }
        ResolvedPlace::Param { index, .. } => {
            if let Some(id) = local_env.mir_local(index) {
                Ok(Expr::LocalRef { local: id })
            } else {
                // On-demand promotion for params not yet materialized.
                let ty = fn_ctx
                    .local_decls
                    .get(index as usize)
                    .map(|d| super::type_lowering::lower_mir_ty(&d.ty, tc.0, tc.1))
                    .unwrap_or(SemanticType::U32);
                let id = local_env.promote_temp(index, ty);
                Ok(Expr::LocalRef { local: id })
            }
        }
        ResolvedPlace::Unknown => Ok(Expr::IntLit {
            value: 0,
            ty: SemanticType::Opaque {
                description: "unknown place".into(),
            },
        }),
    }
}

// ---------------------------------------------------------------------------
// Rvalue lowering
// ---------------------------------------------------------------------------

/// Lower a MIR rvalue to a semantic `Expr`.
pub fn lower_rvalue(
    fn_ctx: &FnContext<'_>,
    local_env: &mut LocalEnvironment,
    resolver: &ReferenceResolver,
    rvalue: &MirRvalue,
    tc: TypeCtx<'_>,
) -> Result<Expr, SemanticLoweringError> {
    match rvalue {
        MirRvalue::Use { operand } => lower_operand(fn_ctx, local_env, resolver, operand, tc),

        MirRvalue::BinaryOp { op, lhs, rhs } => {
            let sem_op = lower_bin_op(op);
            let lhs_expr = lower_operand(fn_ctx, local_env, resolver, lhs, tc)?;
            let rhs_expr = lower_operand(fn_ctx, local_env, resolver, rhs, tc)?;
            Ok(Expr::BinaryOp {
                op: sem_op,
                lhs: Box::new(lhs_expr),
                rhs: Box::new(rhs_expr),
            })
        }

        MirRvalue::UnaryOp { op, operand } => {
            let sem_op = lower_unary_op(op);
            let operand_expr = lower_operand(fn_ctx, local_env, resolver, operand, tc)?;
            Ok(Expr::UnaryOp {
                op: sem_op,
                operand: Box::new(operand_expr),
            })
        }

        MirRvalue::Cast { operand, ty, .. } => {
            let expr = lower_operand(fn_ctx, local_env, resolver, operand, tc)?;
            let target_ty = lower_mir_ty(ty, tc.0, tc.1);
            Ok(Expr::Cast {
                expr: Box::new(expr),
                ty: target_ty,
            })
        }

        MirRvalue::Ref { place, .. } | MirRvalue::CopyForDeref { place } => {
            lower_place_to_expr(fn_ctx, local_env, resolver, place, tc)
        }

        MirRvalue::Discriminant { .. } => {
            // Discriminant reads are absorbed into branch condition lowering.
            // If we reach here, it is a structural discriminant not yet handled.
            Ok(Expr::IntLit {
                value: 0,
                ty: SemanticType::Opaque {
                    description: "discriminant (handled in condition lowering)".into(),
                },
            })
        }

        MirRvalue::Aggregate { agg_kind, operands } => {
            lower_aggregate(fn_ctx, local_env, resolver, agg_kind, operands, tc)
        }

        MirRvalue::Unsupported { description } => Ok(Expr::IntLit {
            value: 0,
            ty: SemanticType::Opaque {
                description: format!("unsupported rvalue: {description}"),
            },
        }),
    }
}

/// Lower a MIR aggregate rvalue.
fn lower_aggregate(
    fn_ctx: &FnContext<'_>,
    local_env: &mut LocalEnvironment,
    resolver: &ReferenceResolver,
    agg_kind: &MirAggregateKind,
    operands: &[MirOperand],
    tc: TypeCtx<'_>,
) -> Result<Expr, SemanticLoweringError> {
    match agg_kind {
        MirAggregateKind::Adt {
            path,
            variant_index,
            ..
        } => {
            // Option::None (variant 0, no operands)
            if path.contains("Option") && *variant_index == 0 {
                return Ok(Expr::OptionNone);
            }
            // Option::Some (variant 1, one operand)
            if path.contains("Option") && *variant_index == 1 && !operands.is_empty() {
                let inner = lower_operand(fn_ctx, local_env, resolver, &operands[0], tc)?;
                return Ok(Expr::OptionSome {
                    inner: Box::new(inner),
                });
            }
            // Result::Ok (variant 0 for Result)
            if path.contains("Result") && *variant_index == 0 {
                if operands.is_empty() {
                    return Ok(Expr::IntLit {
                        value: 0,
                        ty: SemanticType::Void,
                    });
                }
                return lower_operand(fn_ctx, local_env, resolver, &operands[0], tc);
            }
            // Result::Err (variant 1 for Result)
            if path.contains("Result") && *variant_index == 1 && !operands.is_empty() {
                return lower_operand(fn_ctx, local_env, resolver, &operands[0], tc);
            }
            // Tuple or other aggregate — take first operand if available.
            if !operands.is_empty() {
                return lower_operand(fn_ctx, local_env, resolver, &operands[0], tc);
            }
            Ok(Expr::IntLit {
                value: 0,
                ty: SemanticType::Void,
            })
        }
        MirAggregateKind::Tuple => {
            if operands.is_empty() {
                Ok(Expr::IntLit {
                    value: 0,
                    ty: SemanticType::Void,
                })
            } else {
                lower_operand(fn_ctx, local_env, resolver, &operands[0], tc)
            }
        }
        _ => Ok(Expr::IntLit {
            value: 0,
            ty: SemanticType::Opaque {
                description: "unsupported aggregate kind".into(),
            },
        }),
    }
}

// ---------------------------------------------------------------------------
// Branch condition lowering
// ---------------------------------------------------------------------------

/// Lower a `SwitchInt` condition to a semantic `Expr`.
///
/// Examines the discriminant operand and preceding statements in the
/// block to determine the semantic condition. Produces structured
/// `Expr` (e.g., `IsSome`, `ResourcePresent`, `InterruptClassTest`,
/// `BinaryOp`) instead of the old `BranchCondition` enum.
pub fn lower_branch_condition(
    fn_ctx: &FnContext<'_>,
    local_env: &mut LocalEnvironment,
    resolver: &ReferenceResolver,
    bb_index: usize,
    body: &MirBody,
    tc: TypeCtx<'_>,
) -> Result<Expr, SemanticLoweringError> {
    let bb = &body.basic_blocks[bb_index];
    let term = match &bb.terminator {
        Some(t) => t,
        None => return Ok(Expr::BoolLit { value: true }),
    };

    let discr = match &term.kind {
        MirTerminatorKind::SwitchInt { discr, .. } => discr,
        _ => return Ok(Expr::BoolLit { value: true }),
    };

    // Find the local the discriminant comes from.
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
                    return lower_rvalue_to_condition(
                        fn_ctx, local_env, resolver, rvalue, body, bb, tc,
                    );
                }
            }
        }
    }

    // Fall back to lowering the discriminant operand directly.
    lower_operand(fn_ctx, local_env, resolver, discr, tc)
}

/// Lower a computed rvalue to a branch condition `Expr`.
fn lower_rvalue_to_condition(
    fn_ctx: &FnContext<'_>,
    local_env: &mut LocalEnvironment,
    resolver: &ReferenceResolver,
    rvalue: &MirRvalue,
    _body: &MirBody,
    _bb: &regseq_model::declaration::mir_body::MirBasicBlockData,
    tc: TypeCtx<'_>,
) -> Result<Expr, SemanticLoweringError> {
    match rvalue {
        // Discriminant on a Result/Option → IsSome / ResourcePresent.
        MirRvalue::Discriminant { place } => {
            let resolved = resolve_place(fn_ctx, place);
            match resolved {
                ResolvedPlace::DriverField { ref name } => {
                    // Guard: type paths are not real fields.
                    // If the "name" is actually a type path containing "Option",
                    // this is a discriminant on an Option-typed driver field.
                    // Try to resolve the real field from the discriminant place.
                    if name.contains("::") {
                        if name.contains("Option") {
                            // Attempt to resolve the real field from the
                            // discriminant place's field index.
                            if let Some(field_ref) =
                                resolve_field_from_discriminant_place(fn_ctx, resolver, place)
                            {
                                return Ok(Expr::IsSome {
                                    inner: Box::new(Expr::DriverFieldRead { field: field_ref }),
                                });
                            }
                        }
                        return Ok(Expr::BoolLit { value: true });
                    }
                    // Check if this is a resource field.
                    if resolver.is_resource_field(name) {
                        let resource_ref = resolver.resolve_resource(name)?;
                        return Ok(Expr::ResourcePresent {
                            resource: resource_ref,
                        });
                    }
                    // Check if this is an option state field.
                    if resolver.is_option_state_field(name) {
                        let field_ref = resolver.resolve_driver_field(name)?;
                        return Ok(Expr::IsSome {
                            inner: Box::new(Expr::DriverFieldRead { field: field_ref }),
                        });
                    }
                    // Generic discriminant on a driver field.
                    let field_ref = resolver.resolve_driver_field(name)?;
                    Ok(Expr::IsSome {
                        inner: Box::new(Expr::DriverFieldRead { field: field_ref }),
                    })
                }
                ResolvedPlace::ResourceField { ref name, .. } => {
                    // Resolve with attribute fallback for unattributed resource fields.
                    let resource_ref = resolver.resolve_resource(name).or_else(|_| {
                        let field_ref = resolver.resolve_driver_field(name)?;
                        Ok(regseq_model::semantic::refs::ResourceRef {
                            kind: regseq_model::semantic::types::ResourceKind::Reset,
                            field: field_ref,
                        })
                    })?;
                    Ok(Expr::ResourcePresent {
                        resource: resource_ref,
                    })
                }
                ResolvedPlace::Local { index, .. } => {
                    // Discriminant on a local — likely option-typed.
                    let source = fn_ctx.resolve_copy_chain(index);
                    if let Some(id) = local_env.mir_local(source) {
                        return Ok(Expr::IsSome {
                            inner: Box::new(Expr::LocalRef { local: id }),
                        });
                    }
                    if let Some(id) = local_env.mir_local(index) {
                        return Ok(Expr::IsSome {
                            inner: Box::new(Expr::LocalRef { local: id }),
                        });
                    }
                    Ok(Expr::BoolLit { value: true })
                }
                _ => lower_operand(
                    fn_ctx,
                    local_env,
                    resolver,
                    &MirOperand::Copy {
                        place: place.clone(),
                    },
                    tc,
                ),
            }
        }

        // Binary comparison.
        MirRvalue::BinaryOp { op, lhs, rhs } => {
            // Detect `(x & MASK) != 0` pattern for interrupt class tests.
            // In MIR, `status & RX_INTERRUPT_MASK != 0` compiles to:
            //   _13 = BitAnd(_6, RX_INTERRUPT_MASK)
            //   _12 = Ne(_13, 0)
            //   SwitchInt(_12)
            // When we see Ne with zero RHS and the LHS is a local, check if
            // that local was assigned from a BitAnd in this block.
            if matches!(op, MirBinOp::Ne | MirBinOp::Eq) && is_zero_operand(rhs) {
                if let Some(lhs_local) = operand_local(lhs) {
                    // Search for the BitAnd assignment to this local.
                    for stmt in _bb.statements.iter().rev() {
                        if let MirStatementKind::Assign {
                            place,
                            rvalue: inner_rv,
                        } = &stmt.kind
                        {
                            if place.local == lhs_local && place.projection.is_empty() {
                                if let MirRvalue::BinaryOp {
                                    op: MirBinOp::BitAnd,
                                    lhs: inner_lhs,
                                    rhs: inner_rhs,
                                } = inner_rv.as_ref()
                                {
                                    // Found the BitAnd. Try interrupt class resolution.
                                    let result = lower_binop_to_condition(
                                        fn_ctx,
                                        local_env,
                                        resolver,
                                        &MirBinOp::BitAnd,
                                        inner_lhs,
                                        inner_rhs,
                                        tc,
                                    )?;
                                    if matches!(result, Expr::InterruptClassTest { .. }) {
                                        return Ok(result);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            lower_binop_to_condition(fn_ctx, local_env, resolver, op, lhs, rhs, tc)
        }

        // Direct value (e.g., boolean).
        MirRvalue::Use { operand } => lower_operand(fn_ctx, local_env, resolver, operand, tc),

        _ => lower_rvalue(fn_ctx, local_env, resolver, rvalue, tc),
    }
}

/// Lower a binary operation to a condition `Expr`.
///
/// `bb` is the containing basic block, used to trace the source of
/// operands one level deeper (e.g., `(x & MASK) != 0`).
fn lower_binop_to_condition(
    fn_ctx: &FnContext<'_>,
    local_env: &mut LocalEnvironment,
    resolver: &ReferenceResolver,
    op: &MirBinOp,
    lhs: &MirOperand,
    rhs: &MirOperand,
    tc: TypeCtx<'_>,
) -> Result<Expr, SemanticLoweringError> {
    match op {
        MirBinOp::Eq | MirBinOp::Ne => {
            // The `(x & MASK) != 0` interrupt class pattern is detected
            // at the lower_rvalue_to_condition level, which has block access
            // to trace the BitAnd assignment. Here we just produce a
            // generic BinaryOp.
            let lhs_expr = lower_operand(fn_ctx, local_env, resolver, lhs, tc)?;
            let rhs_expr = lower_operand(fn_ctx, local_env, resolver, rhs, tc)?;
            Ok(Expr::BinaryOp {
                op: lower_bin_op(op),
                lhs: Box::new(lhs_expr),
                rhs: Box::new(rhs_expr),
            })
        }
        MirBinOp::BitAnd => {
            // status & MASK pattern — interrupt class test.
            // Try scalar constant mask first.
            if let Some(mask) = extract_constant_value(rhs) {
                let status_expr = lower_operand(fn_ctx, local_env, resolver, lhs, tc)?;
                // Try to resolve as interrupt class.
                if let Ok(class_ref) = resolver.resolve_interrupt_class_by_mask(mask) {
                    return Ok(Expr::InterruptClassTest {
                        status: Box::new(status_expr),
                        class_ref,
                    });
                }
            }
            // Try symbolic constant path.
            if let Some(path) = extract_constant_path(rhs) {
                let status_expr = lower_operand(fn_ctx, local_env, resolver, lhs, tc)?;
                // Derive class name from constant path and try resolution.
                let class_name =
                    crate::lowering::mir_interpreter::value_lowering::derive_class_name_from_constant_path(
                        &path,
                    );
                if let Ok(class_ref) = resolver.resolve_interrupt_class_by_name(&class_name) {
                    return Ok(Expr::InterruptClassTest {
                        status: Box::new(status_expr),
                        class_ref,
                    });
                }
            }
            // Generic BitAnd.
            let lhs_expr = lower_operand(fn_ctx, local_env, resolver, lhs, tc)?;
            let rhs_expr = lower_operand(fn_ctx, local_env, resolver, rhs, tc)?;
            Ok(Expr::BinaryOp {
                op: BinOp::BitAnd,
                lhs: Box::new(lhs_expr),
                rhs: Box::new(rhs_expr),
            })
        }
        _ => {
            let lhs_expr = lower_operand(fn_ctx, local_env, resolver, lhs, tc)?;
            let rhs_expr = lower_operand(fn_ctx, local_env, resolver, rhs, tc)?;
            Ok(Expr::BinaryOp {
                op: lower_bin_op(op),
                lhs: Box::new(lhs_expr),
                rhs: Box::new(rhs_expr),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Return value lowering
// ---------------------------------------------------------------------------

/// Determine the return expression for a `Return` terminator.
///
/// Examines the return type and the value assigned to local 0 to
/// produce the appropriate `Expr` for `Stmt::Return { value }`.
pub fn lower_return_value(
    fn_ctx: &FnContext<'_>,
    local_env: &mut LocalEnvironment,
    resolver: &ReferenceResolver,
    body: &MirBody,
    tc: TypeCtx<'_>,
) -> Result<Option<Expr>, SemanticLoweringError> {
    let return_ty = &local_env.return_type;
    if *return_ty == SemanticType::Void {
        return Ok(None);
    }

    // Look for the value assigned to local 0 in any block with a Return terminator.
    for bb in &body.basic_blocks {
        if let Some(ref term) = bb.terminator {
            if matches!(term.kind, MirTerminatorKind::Return) {
                for stmt in bb.statements.iter().rev() {
                    if let MirStatementKind::Assign { place, rvalue } = &stmt.kind {
                        if place.local == 0 && place.projection.is_empty() {
                            let expr = lower_rvalue(fn_ctx, local_env, resolver, rvalue, tc)?;
                            return Ok(Some(expr));
                        }
                    }
                }
            }
        }
    }

    // If we found no explicit assignment to _0, check if _0 has a local ref.
    if let Some(id) = local_env.mir_local(0) {
        return Ok(Some(Expr::LocalRef { local: id }));
    }

    Ok(None)
}

// ---------------------------------------------------------------------------
// Operator mapping
// ---------------------------------------------------------------------------

/// Map a MIR binary operator to a semantic `BinOp`.
pub fn lower_bin_op(op: &MirBinOp) -> BinOp {
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
        // Unsupported operators default to Add (caught by validation if
        // the result is semantically wrong).
        _ => BinOp::Add,
    }
}

/// Map a MIR unary operator to a semantic `UnaryOp`.
fn lower_unary_op(op: &MirUnOp) -> regseq_model::semantic::ops::UnaryOp {
    match op {
        MirUnOp::Not => regseq_model::semantic::ops::UnaryOp::Not,
        MirUnOp::Neg => regseq_model::semantic::ops::UnaryOp::Neg,
        MirUnOp::PtrMetadata => regseq_model::semantic::ops::UnaryOp::Not, // fallback
    }
}

// ---------------------------------------------------------------------------
// Constant extraction helpers
// ---------------------------------------------------------------------------

/// Extract a scalar constant value from a MIR operand.
pub fn extract_constant_value(operand: &MirOperand) -> Option<u32> {
    if let MirOperand::Constant { constant } = operand {
        if let MirConstValue::Scalar { bits, .. } = &constant.value {
            return Some(*bits as u32);
        }
    }
    None
}

/// Extract a symbolic constant path from an unsupported MIR operand.
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

/// Try to resolve a real driver field from a discriminant place by
/// extracting the field index from the projection chain.
/// Uses the resolver's driver field list (from the hierarchy enrichment)
/// which includes all driver fields, not just resource fields.
fn resolve_field_from_discriminant_place(
    _fn_ctx: &FnContext<'_>,
    resolver: &ReferenceResolver,
    place: &MirPlace,
) -> Option<regseq_model::semantic::refs::DriverFieldRef> {
    use regseq_model::declaration::mir_body::MirProjectionElem;

    // Look for a Field projection in the place.
    for elem in &place.projection {
        if let MirProjectionElem::Field { index, .. } = elem {
            // Use the resolver's driver field list which includes all fields.
            return resolver.resolve_driver_field_by_index(*index as usize).ok();
        }
    }
    None
}

/// Extract the bare local index from an operand, if it is a simple local.
fn operand_local(operand: &MirOperand) -> Option<u32> {
    match operand {
        MirOperand::Copy { place } | MirOperand::Move { place } => {
            if place.projection.is_empty() {
                Some(place.local)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Check if a MIR operand is the zero constant.
fn is_zero_operand(operand: &MirOperand) -> bool {
    if let MirOperand::Constant { constant } = operand {
        if let MirConstValue::Scalar { bits, .. } = &constant.value {
            return *bits == 0;
        }
    }
    false
}
