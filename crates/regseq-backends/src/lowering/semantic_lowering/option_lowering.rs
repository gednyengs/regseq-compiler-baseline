//! Option carrier pattern detection and lowering.
//!
//! Detects and collapses MIR-level Option patterns into semantic IR:
//! `WithDefault` chains, `IsSome` conditions, `OptionUnwrap` extraction,
//! and `OptionNone`/`OptionSome` construction.
//!
//! ## Supported Patterns
//!
//! **Pattern A: `as_ref().map(f).unwrap_or(d)` chain** (`uart_clock_hz`)
//!
//! After drop elaboration, this becomes a discriminant-based branch where
//! the Some-arm calls a resource property accessor and the None-arm
//! provides a constant default. The option lowering module detects this
//! at the `SwitchInt` level and collapses it into
//! `Expr::WithDefault { value: ResourcePropertyRead, default: Constant }`.
//!
//! **Pattern B: `.is_some()` as loop condition** (`service_rx_fifo`)
//!
//! A loop whose condition is a discriminant check on the result of a
//! helper call returning `Option<T>`.
//!
//! **Pattern C: `if let Some(x) = field` branching** (`service_tx_fifo`)
//!
//! A discriminant-based SwitchInt on an Option-typed driver field. The
//! then-body needs an `OptionUnwrap` to extract the payload.
//!
//! **Pattern D: `None`/`Some(v)` construction** (various)
//!
//! `Aggregate { Adt("Option", variant=0) }` -> `Expr::OptionNone`
//! `Aggregate { Adt("Option", variant=1) }` -> `Expr::OptionSome`
//! (Handled inline in `expr_lowering.rs`.)

use regseq_model::declaration::mir_body::{
    MirBody, MirOperand, MirRvalue, MirStatementKind, MirTerminatorKind, MirTy,
};
use regseq_model::semantic::expr::Expr;
use regseq_model::semantic::ops::ResourceProperty;

use super::expr_lowering::{extract_constant_path, extract_constant_value, lower_operand, TypeCtx};
use super::local_env::LocalEnvironment;
use super::ref_resolver::ReferenceResolver;
use super::SemanticLoweringError;
use crate::lowering::mir_interpreter::cfg_helpers::{find_join_point, split_switch_targets};
use crate::lowering::mir_interpreter::context::FnContext;
use crate::lowering::mir_interpreter::place_resolver::{resolve_place, ResolvedPlace};

/// Result of an option-chain collapse attempt.
pub struct OptionChainResult {
    /// The collapsed semantic expression.
    pub expr: Expr,
    /// The basic block to resume walking from after the collapsed chain.
    pub resume_bb: usize,
}

/// Attempt to detect and collapse an option-carrier value-chain pattern
/// at a `SwitchInt` on an Option discriminant.
///
/// This handles Pattern A: `as_ref().map(f).unwrap_or(d)`.
///
/// Returns `Some(OptionChainResult)` if the pattern matches, `None`
/// otherwise. The caller should use the result expression and resume
/// from `resume_bb`.
pub fn try_collapse_option_chain(
    fn_ctx: &FnContext<'_>,
    local_env: &mut LocalEnvironment,
    resolver: &ReferenceResolver,
    body: &MirBody,
    switch_bb: usize,
    tc: TypeCtx<'_>,
) -> Result<Option<OptionChainResult>, SemanticLoweringError> {
    let bb = &body.basic_blocks[switch_bb];
    let term = match &bb.terminator {
        Some(t) => t,
        None => return Ok(None),
    };

    let targets = match &term.kind {
        MirTerminatorKind::SwitchInt { targets, .. } => targets,
        _ => return Ok(None),
    };

    // Find the discriminant source — must be on a resource field.
    let discr_field = find_discriminant_resource_field(fn_ctx, resolver, bb);
    let resource_field = match discr_field {
        Some(f) => f,
        None => return Ok(None),
    };

    // Split into None-arm and Some-arm.
    let (then_bb, else_bb) = split_switch_targets(targets);
    // In MIR's Option discriminant encoding: value 0 = None, otherwise = Some.
    // SwitchInt targets: [value=0 -> targets[0], otherwise -> targets[last]]
    // split_switch_targets returns (then=last, else=first).
    // So then_bb = Some-arm, else_bb = None-arm.
    let some_bb = then_bb;
    let none_bb = else_bb;

    // Find the join point.
    let join = find_join_point(body, some_bb, none_bb, &std::collections::BTreeSet::new());
    let join_bb = match join {
        Some(j) => j,
        None => return Ok(None),
    };

    // Check if the Some-arm contains a resource property read (Clock::get_rate).
    let resource_property = find_resource_property_in_arm(body, some_bb, join_bb);

    // Check if the None-arm contains a constant default.
    let default_expr =
        find_constant_default_in_arm(fn_ctx, local_env, resolver, body, none_bb, join_bb, tc)?;

    if let (Some(property), Some(default)) = (resource_property, default_expr) {
        // Successful collapse: WithDefault { ResourcePropertyRead, default }
        let resource_ref = resolver.resolve_resource(&resource_field)?;
        let expr = Expr::WithDefault {
            value: Box::new(Expr::ResourcePropertyRead {
                resource: resource_ref,
                property,
            }),
            default: Box::new(default),
        };
        return Ok(Some(OptionChainResult {
            expr,
            resume_bb: join_bb,
        }));
    }

    Ok(None)
}

/// Check if an Option-typed IsSome branch condition should generate
/// an `OptionUnwrap` in the then-body.
///
/// For Pattern C: `if let Some(x) = field`, the then-body needs
/// `Stmt::Let { local, value: OptionUnwrap(field_read) }` at the start.
///
/// Returns the inner expression to unwrap if this is an IsSome pattern
/// that warrants an OptionUnwrap in the then-body.
pub fn should_generate_option_unwrap(condition: &Expr) -> Option<Expr> {
    match condition {
        Expr::IsSome { inner } => {
            // Only generate OptionUnwrap for driver field reads and local refs.
            match inner.as_ref() {
                Expr::DriverFieldRead { .. } | Expr::LocalRef { .. } => Some(*inner.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Recognize supported resource-property accessor call paths.
///
/// This is intentionally exact on the nominal type + method pair rather than
/// broad substring matching, which could collide with unrelated helper/library
/// methods that happen to share the same final segment.
pub(crate) fn resource_property_for_call_path(path: &str) -> Option<ResourceProperty> {
    let mut segs = path.rsplit("::");
    let method = segs.next()?;
    let ty = segs.next()?;
    match (ty, method) {
        ("Clock", "get_rate") => Some(ResourceProperty::ClockRate),
        ("ResetLine", "state") => Some(ResourceProperty::ResetState),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find the resource field name from a discriminant in a basic block.
///
/// Scans for `Rvalue::Discriminant { place }` where the place resolves
/// to a resource field.
fn find_discriminant_resource_field(
    fn_ctx: &FnContext<'_>,
    resolver: &ReferenceResolver,
    bb: &regseq_model::declaration::mir_body::MirBasicBlockData,
) -> Option<String> {
    // Check the terminator's discriminant operand.
    let term = bb.terminator.as_ref()?;
    let discr = match &term.kind {
        MirTerminatorKind::SwitchInt { discr, .. } => discr,
        _ => return None,
    };

    let discr_local = match discr {
        MirOperand::Copy { place } | MirOperand::Move { place } => {
            if place.projection.is_empty() {
                Some(place.local)
            } else {
                None
            }
        }
        _ => None,
    }?;

    // Find the discriminant assignment in this block.
    for stmt in bb.statements.iter().rev() {
        if let MirStatementKind::Assign { place, rvalue } = &stmt.kind {
            if place.local == discr_local && place.projection.is_empty() {
                if let MirRvalue::Discriminant { place: disc_place } = rvalue.as_ref() {
                    let resolved = resolve_place(fn_ctx, disc_place);
                    if let ResolvedPlace::ResourceField { name, .. } = resolved {
                        if resolver.is_resource_field(&name) {
                            return Some(name);
                        }
                    }
                }
            }
        }
    }

    None
}

/// Check if a branch arm (from start_bb up to but not including join_bb)
/// contains a resource property read.
fn find_resource_property_in_arm(
    body: &MirBody,
    start_bb: usize,
    join_bb: usize,
) -> Option<ResourceProperty> {
    let mut current = start_bb;
    let mut visited = std::collections::BTreeSet::new();

    while current < body.basic_blocks.len() && current != join_bb && visited.insert(current) {
        let bb = &body.basic_blocks[current];

        // Look for exact recognized resource property accessors.
        if let Some(ref term) = bb.terminator {
            if let MirTerminatorKind::Call { func, target, .. } = &term.kind {
                let path = extract_call_fn_path(func);
                if let Some(property) = resource_property_for_call_path(&path) {
                    return Some(property);
                }
                current = target.map(|t| t as usize).unwrap_or(usize::MAX);
                continue;
            }

            match &term.kind {
                MirTerminatorKind::Goto { target } => {
                    current = *target as usize;
                }
                _ => break,
            }
        } else {
            break;
        }
    }

    None
}

/// Check if a branch arm contains a constant default value assignment.
fn find_constant_default_in_arm(
    fn_ctx: &FnContext<'_>,
    local_env: &mut LocalEnvironment,
    resolver: &ReferenceResolver,
    body: &MirBody,
    start_bb: usize,
    join_bb: usize,
    tc: TypeCtx<'_>,
) -> Result<Option<Expr>, SemanticLoweringError> {
    let mut current = start_bb;
    let mut visited = std::collections::BTreeSet::new();

    while current < body.basic_blocks.len() && current != join_bb && visited.insert(current) {
        let bb = &body.basic_blocks[current];

        // Look for assignments that contain constant values.
        for stmt in &bb.statements {
            if let MirStatementKind::Assign { rvalue, .. } = &stmt.kind {
                if let MirRvalue::Use { operand } = rvalue.as_ref() {
                    // Check for scalar constant.
                    if extract_constant_value(operand).is_some() {
                        let expr = lower_operand(fn_ctx, local_env, resolver, operand, tc)?;
                        return Ok(Some(expr));
                    }
                    // Check for unevaluated constant path.
                    if extract_constant_path(operand).is_some() {
                        let expr = lower_operand(fn_ctx, local_env, resolver, operand, tc)?;
                        return Ok(Some(expr));
                    }
                }
            }
        }

        if let Some(ref term) = bb.terminator {
            match &term.kind {
                MirTerminatorKind::Goto { target } => {
                    current = *target as usize;
                }
                MirTerminatorKind::Call { target, .. } => {
                    current = target.map(|t| t as usize).unwrap_or(usize::MAX);
                }
                _ => break,
            }
        } else {
            break;
        }
    }

    Ok(None)
}

/// Extract the function path from a Call's func operand.
fn extract_call_fn_path(func: &MirOperand) -> String {
    match func {
        MirOperand::Constant { constant } => match &constant.ty {
            MirTy::Fn { path, .. } => path.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_generate_unwrap_for_driver_field_is_some() {
        use regseq_model::item::identity::CanonicalPath;
        use regseq_model::item::ItemId;
        use regseq_model::semantic::refs::DriverFieldRef;

        let field = DriverFieldRef {
            owner: ItemId(0),
            canonical_path: CanonicalPath::new("test::pending_tx"),
            name: "pending_tx".into(),
        };
        let condition = Expr::IsSome {
            inner: Box::new(Expr::DriverFieldRead {
                field: field.clone(),
            }),
        };
        let result = should_generate_option_unwrap(&condition);
        assert!(result.is_some());
        assert!(matches!(result.unwrap(), Expr::DriverFieldRead { .. }));
    }

    #[test]
    fn should_not_generate_unwrap_for_non_is_some() {
        let condition = Expr::BoolLit { value: true };
        assert!(should_generate_option_unwrap(&condition).is_none());
    }

    #[test]
    fn should_generate_unwrap_for_local_is_some() {
        use regseq_model::semantic::local::LocalId;

        let condition = Expr::IsSome {
            inner: Box::new(Expr::LocalRef { local: LocalId(5) }),
        };
        let result = should_generate_option_unwrap(&condition);
        assert!(result.is_some());
    }

    #[test]
    fn resource_property_call_path_matches_exact_clock_get_rate() {
        assert_eq!(
            resource_property_for_call_path("regseq::Clock::get_rate"),
            Some(ResourceProperty::ClockRate)
        );
    }

    #[test]
    fn resource_property_call_path_rejects_generic_get_rate_name() {
        assert_eq!(
            resource_property_for_call_path("crate::helpers::get_rate"),
            None
        );
    }
}
