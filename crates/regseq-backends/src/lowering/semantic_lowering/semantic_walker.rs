//! Semantic CFG walker.
//!
//! Walks MIR basic blocks and produces structured `Vec<Stmt>` output.
//! Mirrors `cfg_walker::walk_region()` structure but targets the
//! semantic IR types directly.
//!
//! Key differences from the old walker:
//! - Returns `Result`, not bare `Vec` (unhandled constructs are errors)
//! - Uses `LocalEnvironment` for `Let` vs `Assign` tracking
//! - Delegates to `call_lowering` with structured refs
//! - Produces `Expr` for conditions, not `BranchCondition`

use std::collections::BTreeSet;

use regseq_model::declaration::mir_body::{
    MirBody, MirOperand, MirPlace, MirRvalue, MirStatementKind, MirTerminatorKind,
};
use regseq_model::semantic::body::Block;
use regseq_model::semantic::expr::Expr;
use regseq_model::semantic::ops::AssignTarget;
use regseq_model::semantic::stmt::Stmt;
use regseq_model::semantic::types::{ResourceKind, SemanticType};

use super::call_lowering::{
    extract_fn_path, lower_call, make_helper_call_expr, make_reg_read_raw_expr,
};
use super::context::SemanticFnContext;
use super::expr_lowering::{lower_branch_condition, lower_return_value, lower_rvalue};
use super::local_env::is_value_producing_rvalue;
use super::ref_resolver::ReferenceResolver;
use super::type_lowering::lower_mir_ty;
use super::SemanticLoweringError;
use crate::lowering::mir_interpreter::call_classifier::{classify_call, CallClassification};
use crate::lowering::mir_interpreter::cfg_helpers::{
    detect_fallible_pattern, find_join_point, has_back_edge, identify_loop_structure,
    split_switch_targets,
};
use crate::lowering::mir_interpreter::place_resolver::{resolve_place, ResolvedPlace};

// ---------------------------------------------------------------------------
// Public entry
// ---------------------------------------------------------------------------

/// Walk a region of the MIR CFG and produce structured `Stmt` output.
///
/// Stops at blocks in `stop_set` or at Return/Unreachable terminators.
/// Tracks visited blocks to avoid infinite loops.
pub fn walk_semantic_region(
    ctx: &mut SemanticFnContext<'_>,
    resolver: &ReferenceResolver,
    body: &MirBody,
    start_bb: usize,
    stop_set: &BTreeSet<usize>,
) -> Result<Vec<Stmt>, SemanticLoweringError> {
    let mut stmts = Vec::new();
    let mut current = start_bb;
    let mut visited = BTreeSet::new();
    let mut barrier_active = false;
    let mut ordered_stmts: Vec<Stmt> = Vec::new();
    // Track resource property read context for unwrap_or chain collapse.
    let mut pending_resource_property: Option<String> = None;
    let tc = (&ctx.hierarchy.cross_link, &ctx.hierarchy.store);

    while current < body.basic_blocks.len()
        && !stop_set.contains(&current)
        && visited.insert(current)
    {
        let bb = &body.basic_blocks[current];

        // Skip cleanup blocks (they are handled by FallibleOp).
        if bb.is_cleanup {
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

        // Lower statements in this block.
        let block_stmts = lower_block_statements(ctx, resolver, bb, tc)?;
        let target = if barrier_active {
            &mut ordered_stmts
        } else {
            &mut stmts
        };
        target.extend(block_stmts);

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
                let value =
                    lower_return_value(&ctx.fn_ctx, &mut ctx.local_env, resolver, body, tc)?;
                let target = if barrier_active {
                    &mut ordered_stmts
                } else {
                    &mut stmts
                };
                target.push(Stmt::Return { value });
                break;
            }

            MirTerminatorKind::Call {
                func,
                args,
                destination,
                target: next_bb,
                unwind: _,
            } => {
                let classification = classify_call(&ctx.fn_ctx, func, args);
                let mir_fn_path = extract_fn_path(func);
                let next = next_bb.map(|b| b as usize);

                match &classification {
                    // Barrier handling for ordered! regions.
                    CallClassification::Barrier => {
                        if barrier_active {
                            let steps = std::mem::take(&mut ordered_stmts);
                            stmts.push(Stmt::OrderedRegion {
                                body: Block { stmts: steps },
                            });
                            barrier_active = false;
                        } else {
                            barrier_active = true;
                        }
                        current = next.unwrap_or(usize::MAX);
                    }

                    // Option unwrap_or: detect resource property + default pattern.
                    CallClassification::OptionMethod { method } if method == "unwrap_or" => {
                        // Check if we have a pending resource property read
                        // from a preceding Clock::get_rate or similar call.
                        if let Some(ref resource_field) = pending_resource_property {
                            if args.len() >= 2 {
                                let default_expr = super::expr_lowering::lower_operand(
                                    &ctx.fn_ctx,
                                    &mut ctx.local_env,
                                    resolver,
                                    &args[1],
                                    tc,
                                )?;
                                if let Ok(resource_ref) = resolver.resolve_resource(resource_field)
                                {
                                    let with_default = Expr::WithDefault {
                                        value: Box::new(Expr::ResourcePropertyRead {
                                            resource: resource_ref,
                                            property:
                                                regseq_model::semantic::ops::ResourceProperty::ClockRate,
                                        }),
                                        default: Box::new(default_expr),
                                    };
                                    pending_resource_property = None;
                                    // If destination is the return place, emit
                                    // as a Let (the return will pick it up).
                                    let with_default_clone = with_default.clone();
                                    if let Some(stmt) = assign_to_destination(
                                        ctx,
                                        resolver,
                                        destination,
                                        with_default,
                                        tc,
                                    )? {
                                        let target = if barrier_active {
                                            &mut ordered_stmts
                                        } else {
                                            &mut stmts
                                        };
                                        target.push(stmt);
                                    } else if destination.local == 0 {
                                        // Return place: emit as Eval so the
                                        // expression is visible in the body.
                                        let target = if barrier_active {
                                            &mut ordered_stmts
                                        } else {
                                            &mut stmts
                                        };
                                        target.push(Stmt::Eval {
                                            expr: with_default_clone,
                                        });
                                    }
                                    current = next.unwrap_or(usize::MAX);
                                    continue;
                                }
                            }
                        }
                        pending_resource_property = None;
                        // Fall through to transparent skip.
                        current = next.unwrap_or(usize::MAX);
                    }

                    // Track resource property reads for unwrap_or chain.
                    // When as_ref() is called on an Option<ResourceHandle<_>>
                    // or similar resource-typed value, record the unique
                    // driver resource field for unwrap_or collapse.
                    CallClassification::OptionMethod { method } if method == "as_ref" => {
                        if let Some(first) = args.first() {
                            if let Some(place) = match first {
                                MirOperand::Copy { place } | MirOperand::Move { place } => {
                                    Some(place)
                                }
                                _ => None,
                            } {
                                let resolved = resolve_place(&ctx.fn_ctx, place);
                                match resolved {
                                    ResolvedPlace::ResourceField { ref name, .. } => {
                                        pending_resource_property = Some(name.clone());
                                    }
                                    ResolvedPlace::Local { index, .. } => {
                                        if let Some(decl) =
                                            ctx.fn_ctx.local_decls.get(index as usize)
                                        {
                                            let sem_ty = lower_mir_ty(&decl.ty, tc.0, tc.1);
                                            if let Some(kind) =
                                                resource_kind_from_option_like_type(&sem_ty)
                                            {
                                                if let Some(field_name) =
                                                    resolver.unique_resource_field_name(kind)
                                                {
                                                    pending_resource_property = Some(field_name);
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        current = next.unwrap_or(usize::MAX);
                    }

                    CallClassification::FrameworkValueConstructor { path }
                        if super::option_lowering::resource_property_for_call_path(path)
                            .is_some() =>
                    {
                        // If we didn't already set pending from as_ref, derive
                        // a unique resource field from the recognized property.
                        if pending_resource_property.is_none() {
                            if let Some(property) =
                                super::option_lowering::resource_property_for_call_path(path)
                            {
                                if let Some(kind) = resource_kind_for_property(property) {
                                    if let Some(field_name) =
                                        resolver.unique_resource_field_name(kind)
                                    {
                                        pending_resource_property = Some(field_name);
                                    }
                                }
                            }
                        }
                        current = next.unwrap_or(usize::MAX);
                    }

                    // Option is_some(): produce IsSome(receiver) and assign
                    // to destination so the upcoming SwitchInt uses it.
                    CallClassification::OptionMethod { method } if method == "is_some" => {
                        // The receiver (args[0]) is the Option-typed value.
                        if let Some(first) = args.first() {
                            let receiver_expr = super::expr_lowering::lower_operand(
                                &ctx.fn_ctx,
                                &mut ctx.local_env,
                                resolver,
                                first,
                                tc,
                            )?;
                            let is_some_expr = Expr::IsSome {
                                inner: Box::new(receiver_expr),
                            };
                            if let Some(stmt) =
                                assign_to_destination(ctx, resolver, destination, is_some_expr, tc)?
                            {
                                let target = if barrier_active {
                                    &mut ordered_stmts
                                } else {
                                    &mut stmts
                                };
                                target.push(stmt);
                            }
                        }
                        current = next.unwrap_or(usize::MAX);
                    }

                    // Transparent calls — skip.
                    CallClassification::Deref
                    | CallClassification::FieldValueConstructor { .. }
                    | CallClassification::OptionMethod { .. }
                    | CallClassification::DurationConstructor
                    | CallClassification::FrameworkValueConstructor { .. }
                    | CallClassification::TryBranch
                    | CallClassification::TryFromResidual => {
                        current = next.unwrap_or(usize::MAX);
                    }

                    // Register reads — value-producing, assign to destination.
                    CallClassification::RegisterReadRaw { register } => {
                        let expr = make_reg_read_raw_expr(resolver, register)?;
                        if let Some(stmt) =
                            assign_to_destination(ctx, resolver, destination, expr, tc)?
                        {
                            let target = if barrier_active {
                                &mut ordered_stmts
                            } else {
                                &mut stmts
                            };
                            target.push(stmt);
                        }
                        current = next.unwrap_or(usize::MAX);
                    }

                    CallClassification::RegisterFieldRead { register } => {
                        let reg_ref = resolver.resolve_register(register)?;
                        let expr = Expr::RegFieldRead {
                            register: reg_ref.clone(),
                            field: super::call_lowering::placeholder_reg_field_ref(&reg_ref),
                        };
                        if let Some(stmt) =
                            assign_to_destination(ctx, resolver, destination, expr, tc)?
                        {
                            let target = if barrier_active {
                                &mut ordered_stmts
                            } else {
                                &mut stmts
                            };
                            target.push(stmt);
                        }
                        current = next.unwrap_or(usize::MAX);
                    }

                    // Helper calls and serial trait methods that produce values.
                    CallClassification::HelperCall { .. }
                    | CallClassification::SerialTraitMethod { .. } => {
                        // Check for fallible ? pattern.
                        if let Some(fallible) =
                            detect_fallible_pattern(body, next, &visited, stop_set)
                        {
                            let call_expr = make_helper_call_expr(
                                &ctx.fn_ctx,
                                &mut ctx.local_env,
                                resolver,
                                &mir_fn_path,
                                args,
                                tc,
                            )?;
                            // Derive the error type from the function's return type.
                            // For Result<T, E>, extract E. For non-Result
                            // functions, derive from the MIR return place type.
                            let err_ty = match &ctx.local_env.return_type {
                                SemanticType::Result { err, .. } => *err.clone(),
                                _ => {
                                    // Extract error type from MIR return place.
                                    extract_result_error_from_mir(body, tc)
                                        .unwrap_or(SemanticType::Void)
                                }
                            };
                            let error_local = ctx.local_env.allocate_derived("err", err_ty);
                            // Walk cleanup/error path. Propagate errors explicitly;
                            // no silent defaulting to empty cleanup.
                            let cleanup_stmts =
                                walk_semantic_region(ctx, resolver, body, fallible.err_bb, &{
                                    let mut s = stop_set.clone();
                                    s.extend(visited.iter());
                                    s
                                })?;
                            let target = if barrier_active {
                                &mut ordered_stmts
                            } else {
                                &mut stmts
                            };
                            target.push(Stmt::FallibleOp {
                                body: Block {
                                    stmts: vec![Stmt::Eval { expr: call_expr }],
                                },
                                success_local: None,
                                error_local,
                                cleanup_on_error: Block {
                                    stmts: cleanup_stmts,
                                },
                            });
                            current = fallible.ok_bb;
                        } else {
                            // Non-fallible call. Check if it produces a value.
                            let call_stmts = lower_call(
                                &ctx.fn_ctx,
                                &mut ctx.local_env,
                                resolver,
                                &classification,
                                &mir_fn_path,
                                args,
                                destination,
                                tc,
                            )?;

                            // If the destination is a semantic local, wrap as Let/Assign.
                            if call_stmts.is_empty() || matches!(call_stmts[0], Stmt::Eval { .. }) {
                                // Check if the call result is assigned to a meaningful local.
                                if destination.projection.is_empty() {
                                    let dest_local = destination.local;
                                    if ctx.local_env.has_mir_local(dest_local)
                                        || is_authored_local(&ctx.fn_ctx, dest_local)
                                    {
                                        let call_expr = make_helper_call_expr(
                                            &ctx.fn_ctx,
                                            &mut ctx.local_env,
                                            resolver,
                                            &mir_fn_path,
                                            args,
                                            tc,
                                        )?;
                                        if let Some(stmt) = assign_to_destination(
                                            ctx,
                                            resolver,
                                            destination,
                                            call_expr,
                                            tc,
                                        )? {
                                            let target = if barrier_active {
                                                &mut ordered_stmts
                                            } else {
                                                &mut stmts
                                            };
                                            target.push(stmt);
                                            current = next.unwrap_or(usize::MAX);
                                            continue;
                                        }
                                    }
                                }
                            }

                            let target = if barrier_active {
                                &mut ordered_stmts
                            } else {
                                &mut stmts
                            };
                            target.extend(call_stmts);
                            current = next.unwrap_or(usize::MAX);
                        }
                    }

                    // Unknown calls: gated recovery for supported external callables.
                    //
                    // Semantic contract: referenced supported external callables
                    // lower through ordinary modeled identity. Using Unknown as
                    // the arrival classification is a current implementation tactic
                    // after removing these paths from FrameworkValueConstructor.
                    CallClassification::Unknown { ref path } => {
                        if crate::lowering::semantic_lowering::external_items::is_supported_external_callable(path) {
                            // Supported external callable — must resolve to a
                            // materialized function.
                            if let Ok(func_ref) = resolver.resolve_call(&mir_fn_path) {
                                let lowered_args = lower_external_call_args(
                                    &ctx.fn_ctx,
                                    &mut ctx.local_env,
                                    resolver,
                                    args,
                                    tc,
                                )?;
                                let call_expr = Expr::HelperCall {
                                    callee: func_ref,
                                    args: lowered_args,
                                };
                                if let Some(stmt) = assign_to_destination(
                                    ctx,
                                    resolver,
                                    destination,
                                    call_expr,
                                    tc,
                                )? {
                                    let target = if barrier_active {
                                        &mut ordered_stmts
                                    } else {
                                        &mut stmts
                                    };
                                    target.push(stmt);
                                }
                                current = next.unwrap_or(usize::MAX);
                            } else {
                                // Supported external callable not materialized — hard error.
                                return Err(super::SemanticLoweringError::UnresolvedFunction {
                                    path: mir_fn_path.clone(),
                                    hint: "supported external callable not materialized".into(),
                                });
                            }
                        } else {
                            // Not a supported external callable — delegate to
                            // call_lowering for structural-core checks or hard error.
                            let call_stmts = lower_call(
                                &ctx.fn_ctx,
                                &mut ctx.local_env,
                                resolver,
                                &classification,
                                &mir_fn_path,
                                args,
                                destination,
                                tc,
                            )?;
                            let target = if barrier_active {
                                &mut ordered_stmts
                            } else {
                                &mut stmts
                            };
                            target.extend(call_stmts);
                            current = next.unwrap_or(usize::MAX);
                        }
                    }

                    // All other classified calls.
                    _ => {
                        let call_stmts = lower_call(
                            &ctx.fn_ctx,
                            &mut ctx.local_env,
                            resolver,
                            &classification,
                            &mir_fn_path,
                            args,
                            destination,
                            tc,
                        )?;
                        let target = if barrier_active {
                            &mut ordered_stmts
                        } else {
                            &mut stmts
                        };
                        target.extend(call_stmts);
                        current = next.unwrap_or(usize::MAX);
                    }
                }
            }

            MirTerminatorKind::SwitchInt { targets, .. } => {
                // Try option-chain collapse (Pattern A: WithDefault).
                if let Some(chain_result) = super::option_lowering::try_collapse_option_chain(
                    &ctx.fn_ctx,
                    &mut ctx.local_env,
                    resolver,
                    body,
                    current,
                    tc,
                )? {
                    // Collapsed into a WithDefault expression. Assign to
                    // destination if this feeds into a local, otherwise Eval.
                    let target = if barrier_active {
                        &mut ordered_stmts
                    } else {
                        &mut stmts
                    };
                    target.push(Stmt::Eval {
                        expr: chain_result.expr,
                    });
                    current = chain_result.resume_bb;
                    continue;
                }

                let condition = lower_branch_condition(
                    &ctx.fn_ctx,
                    &mut ctx.local_env,
                    resolver,
                    current,
                    body,
                    tc,
                )?;

                if has_back_edge(body, targets, &visited) {
                    // Loop.
                    let (body_entry, exit) = identify_loop_structure(targets, &visited);
                    let mut loop_stop = stop_set.clone();
                    loop_stop.insert(current);
                    let loop_body =
                        walk_semantic_region(ctx, resolver, body, body_entry, &loop_stop)?;
                    let target = if barrier_active {
                        &mut ordered_stmts
                    } else {
                        &mut stmts
                    };
                    target.push(Stmt::Loop {
                        condition,
                        body: Block { stmts: loop_body },
                    });
                    current = exit;
                } else {
                    // Branch.
                    let (then_bb, else_bb) = split_switch_targets(targets);
                    let join = find_join_point(body, then_bb, else_bb, stop_set);

                    let mut branch_stop = stop_set.clone();
                    if let Some(j) = join {
                        branch_stop.insert(j);
                    }

                    let mut then_stmts =
                        walk_semantic_region(ctx, resolver, body, then_bb, &branch_stop)?;
                    let else_stmts =
                        walk_semantic_region(ctx, resolver, body, else_bb, &branch_stop)?;

                    // Pattern C: if the condition is IsSome(field), inject
                    // OptionUnwrap at the start of the then-body.
                    if let Some(unwrap_inner) =
                        super::option_lowering::should_generate_option_unwrap(&condition)
                    {
                        let already_has_unwrap = then_stmts.first().is_some_and(|s| {
                            matches!(
                                s,
                                Stmt::Let {
                                    value: Expr::OptionUnwrap { .. },
                                    ..
                                }
                            )
                        });
                        if !already_has_unwrap {
                            // Derive the option payload type from the MIR
                            // discriminant place's field projection type.
                            let unwrap_ty = extract_option_inner_from_switchint(body, current, tc)
                                .ok_or_else(|| SemanticLoweringError::UnsupportedConstruct {
                                    function: ctx.func_path.clone(),
                                    description:
                                        "could not derive option payload type from discriminant"
                                            .into(),
                                })?;
                            let unwrap_local = ctx.local_env.allocate_derived("opt_val", unwrap_ty);
                            then_stmts.insert(
                                0,
                                Stmt::Let {
                                    local: unwrap_local,
                                    value: Expr::OptionUnwrap {
                                        inner: Box::new(unwrap_inner),
                                    },
                                },
                            );
                        }
                    }

                    let target = if barrier_active {
                        &mut ordered_stmts
                    } else {
                        &mut stmts
                    };
                    target.push(Stmt::Branch {
                        condition,
                        then_body: Block { stmts: then_stmts },
                        else_body: Block { stmts: else_stmts },
                    });

                    if let Some(j) = join {
                        current = j;
                    } else {
                        break;
                    }
                }
            }

            MirTerminatorKind::Drop { target, .. } => {
                current = *target as usize;
            }

            MirTerminatorKind::Assert { target, .. } => {
                current = *target as usize;
            }

            MirTerminatorKind::Unreachable
            | MirTerminatorKind::UnwindResume
            | MirTerminatorKind::UnwindTerminate { .. }
            | MirTerminatorKind::Unsupported { .. } => {
                break;
            }
        }
    }

    // Flush unmatched ordered region.
    if barrier_active && !ordered_stmts.is_empty() {
        stmts.push(Stmt::OrderedRegion {
            body: Block {
                stmts: ordered_stmts,
            },
        });
    }

    Ok(stmts)
}

// ---------------------------------------------------------------------------
// Statement lowering
// ---------------------------------------------------------------------------

/// Lower non-cleanup statements from a basic block.
fn lower_block_statements(
    ctx: &mut SemanticFnContext<'_>,
    resolver: &ReferenceResolver,
    bb: &regseq_model::declaration::mir_body::MirBasicBlockData,
    tc: super::expr_lowering::TypeCtx<'_>,
) -> Result<Vec<Stmt>, SemanticLoweringError> {
    let mut stmts = Vec::new();
    for stmt in &bb.statements {
        match &stmt.kind {
            MirStatementKind::Assign { place, rvalue } => {
                if let Some(s) = lower_assign(ctx, resolver, place, rvalue, tc)? {
                    stmts.push(s);
                }
            }
            MirStatementKind::StorageLive { .. }
            | MirStatementKind::StorageDead { .. }
            | MirStatementKind::Nop
            | MirStatementKind::SetDiscriminant { .. }
            | MirStatementKind::Unsupported { .. } => {}
        }
    }
    Ok(stmts)
}

/// Lower a MIR assignment to a semantic Stmt.
fn lower_assign(
    ctx: &mut SemanticFnContext<'_>,
    resolver: &ReferenceResolver,
    place: &MirPlace,
    rvalue: &MirRvalue,
    tc: super::expr_lowering::TypeCtx<'_>,
) -> Result<Option<Stmt>, SemanticLoweringError> {
    let resolved = resolve_place(&ctx.fn_ctx, place);

    match &resolved {
        ResolvedPlace::DriverField { name } => {
            // Guard: the place resolver sometimes produces type paths
            // (e.g., "core::option::Option") instead of real field names.
            // For Option-typed fields, resolve the real field from the
            // place's field index. For other type paths, skip.
            let field_ref = if name.contains("::") {
                // Try to resolve from the place's field projection index.
                use regseq_model::declaration::mir_body::MirProjectionElem;
                let mut resolved_field = None;
                for elem in &place.projection {
                    if let MirProjectionElem::Field { index, .. } = elem {
                        resolved_field =
                            resolver.resolve_driver_field_by_index(*index as usize).ok();
                    }
                }
                match resolved_field {
                    Some(f) => f,
                    None => return Ok(None),
                }
            } else {
                resolver.resolve_driver_field(name)?
            };
            let value = lower_rvalue(&ctx.fn_ctx, &mut ctx.local_env, resolver, rvalue, tc)?;
            Ok(Some(Stmt::FieldAssign {
                field: field_ref,
                value,
            }))
        }

        ResolvedPlace::Local { index, name } => {
            // Skip discriminant and reference assignments.
            if matches!(rvalue, MirRvalue::Discriminant { .. }) {
                return Ok(None);
            }
            if matches!(
                rvalue,
                MirRvalue::Ref { .. } | MirRvalue::CopyForDeref { .. }
            ) {
                return Ok(None);
            }

            let mir_idx = *index;

            // Check if this local already has a semantic ID.
            if let Some(local_id) = ctx.local_env.mir_local(mir_idx) {
                let value = lower_rvalue(&ctx.fn_ctx, &mut ctx.local_env, resolver, rvalue, tc)?;
                if ctx.local_env.is_initialized(local_id) {
                    return Ok(Some(Stmt::Assign {
                        target: AssignTarget::Local { local: local_id },
                        value,
                    }));
                } else {
                    ctx.local_env.mark_initialized(local_id);
                    return Ok(Some(Stmt::Let {
                        local: local_id,
                        value,
                    }));
                }
            }

            // Check if this is a _N temp that should be promoted.
            let should_promote = name.as_deref().map(is_mir_generated_name).unwrap_or(true)
                && is_value_producing_rvalue(rvalue)
                && (is_simple_scalar_local(&ctx.fn_ctx, mir_idx)
                    || is_option_carrier_rvalue(rvalue));

            if should_promote {
                let ty = ctx
                    .fn_ctx
                    .local_decls
                    .get(mir_idx as usize)
                    .map(|d| lower_mir_ty(&d.ty, tc.0, tc.1))
                    .unwrap_or(SemanticType::U32);
                let local_id = ctx.local_env.promote_temp(mir_idx, ty);
                let value = lower_rvalue(&ctx.fn_ctx, &mut ctx.local_env, resolver, rvalue, tc)?;
                ctx.local_env.mark_initialized(local_id);
                return Ok(Some(Stmt::Let {
                    local: local_id,
                    value,
                }));
            }

            // Not a semantic local — skip.
            Ok(None)
        }

        ResolvedPlace::ReturnPlace => {
            // Assignments to _0 are captured by return_value lowering.
            Ok(None)
        }

        _ => Ok(None),
    }
}

/// Lower MIR call arguments for an external function call.
///
/// Detects whether the first MIR arg is a self receiver by inspecting
/// the resolved place. Associated functions like `RxChar::new` have no
/// self receiver; their MIR args start with the first real parameter.
fn lower_external_call_args(
    fn_ctx: &crate::lowering::mir_interpreter::context::FnContext<'_>,
    local_env: &mut super::local_env::LocalEnvironment,
    resolver: &ReferenceResolver,
    args: &[regseq_model::declaration::mir_body::MirOperand],
    tc: super::expr_lowering::TypeCtx<'_>,
) -> Result<Vec<Expr>, super::SemanticLoweringError> {
    use regseq_model::declaration::mir_body::MirOperand;

    let first_is_self = match args.first() {
        Some(MirOperand::Copy { place } | MirOperand::Move { place }) => {
            matches!(resolve_place(fn_ctx, place), ResolvedPlace::SelfRef)
        }
        _ => false,
    };
    let skip = if first_is_self { 1 } else { 0 };
    args.iter()
        .skip(skip)
        .map(|a| super::expr_lowering::lower_operand(fn_ctx, local_env, resolver, a, tc))
        .collect()
}

/// Assign a value-producing expression to a destination local.
///
/// Creates `Let` or `Assign` depending on initialization state.
/// Promotes `_N` temps if needed.
fn assign_to_destination(
    ctx: &mut SemanticFnContext<'_>,
    _resolver: &ReferenceResolver,
    destination: &MirPlace,
    expr: Expr,
    tc: super::expr_lowering::TypeCtx<'_>,
) -> Result<Option<Stmt>, SemanticLoweringError> {
    if !destination.projection.is_empty() {
        return Ok(None);
    }

    let mir_idx = destination.local;
    if mir_idx == 0 {
        // Return place — handled by return lowering.
        return Ok(None);
    }

    // Check existing local or promote.
    let local_id = if let Some(id) = ctx.local_env.mir_local(mir_idx) {
        id
    } else {
        let ty = ctx
            .fn_ctx
            .local_decls
            .get(mir_idx as usize)
            .map(|d| lower_mir_ty(&d.ty, tc.0, tc.1))
            .unwrap_or(SemanticType::U32);
        ctx.local_env.promote_temp(mir_idx, ty)
    };

    if ctx.local_env.is_initialized(local_id) {
        Ok(Some(Stmt::Assign {
            target: AssignTarget::Local { local: local_id },
            value: expr,
        }))
    } else {
        ctx.local_env.mark_initialized(local_id);
        Ok(Some(Stmt::Let {
            local: local_id,
            value: expr,
        }))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_mir_generated_name(name: &str) -> bool {
    if let Some(rest) = name.strip_prefix('_') {
        !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit())
    } else {
        false
    }
}

fn is_authored_local(
    fn_ctx: &crate::lowering::mir_interpreter::context::FnContext<'_>,
    index: u32,
) -> bool {
    if let Some(name) = fn_ctx.local_name(index) {
        !is_mir_generated_name(name)
    } else {
        false
    }
}

/// Check if a MIR local is a simple scalar type suitable for promotion.
fn is_simple_scalar_local(
    fn_ctx: &crate::lowering::mir_interpreter::context::FnContext<'_>,
    index: u32,
) -> bool {
    if let Some(decl) = fn_ctx.local_decls.get(index as usize) {
        matches!(
            &decl.ty,
            regseq_model::declaration::mir_body::MirTy::Bool
                | regseq_model::declaration::mir_body::MirTy::Uint { .. }
                | regseq_model::declaration::mir_body::MirTy::Int { .. }
        )
    } else {
        false
    }
}

/// Check if a MIR rvalue is an Option carrier constructor (None/Some).
fn is_option_carrier_rvalue(rvalue: &MirRvalue) -> bool {
    matches!(
        rvalue,
        MirRvalue::Aggregate {
            agg_kind: regseq_model::declaration::mir_body::MirAggregateKind::Adt { path, .. },
            ..
        } if path.contains("Option")
    )
}

/// Extract the error type from a MIR body's return place.
///
/// If the return type is `Result<T, E>`, returns `lower_mir_ty(E)`.
fn extract_result_error_from_mir(
    body: &MirBody,
    tc: super::expr_lowering::TypeCtx<'_>,
) -> Option<SemanticType> {
    use regseq_model::declaration::mir_body::MirTy;
    let ret_decl = body.local_decls.first()?;
    match &ret_decl.ty {
        MirTy::Adt { path, args } if path.contains("Result") && args.len() >= 2 => {
            Some(lower_mir_ty(&args[1], tc.0, tc.1))
        }
        _ => None,
    }
}

/// Extract the Option inner type from a SwitchInt's discriminant source.
///
/// Traces back from the SwitchInt terminator to the `Discriminant { place }`
/// rvalue, and extracts `T` from the `Option<T>` MIR type found in the
/// place's field projection. This is structural derivation from actual
/// MIR type data — no hardcoded types.
fn extract_option_inner_from_switchint(
    body: &MirBody,
    bb_index: usize,
    tc: super::expr_lowering::TypeCtx<'_>,
) -> Option<SemanticType> {
    use regseq_model::declaration::mir_body::MirProjectionElem;

    let bb = body.basic_blocks.get(bb_index)?;
    let term = bb.terminator.as_ref()?;
    let discr_local = match &term.kind {
        MirTerminatorKind::SwitchInt { discr, .. } => {
            if let MirOperand::Copy { place } | MirOperand::Move { place } = discr {
                if place.projection.is_empty() {
                    Some(place.local)
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None,
    }?;

    // Find the assignment to the discriminant local in this block.
    for stmt in bb.statements.iter().rev() {
        if let MirStatementKind::Assign { place, rvalue } = &stmt.kind {
            if place.local == discr_local && place.projection.is_empty() {
                if let MirRvalue::Discriminant {
                    place: disc_place, ..
                } = rvalue.as_ref()
                {
                    // Extract the Option inner type from the discriminant
                    // place's field projection.
                    for elem in &disc_place.projection {
                        if let MirProjectionElem::Field { ty, .. } = elem {
                            return extract_option_inner_from_mir_ty(ty, tc);
                        }
                    }
                    // Also check the local's declared type directly.
                    let decl = body.local_decls.get(disc_place.local as usize)?;
                    return extract_option_inner_from_mir_ty(&decl.ty, tc);
                }
            }
        }
    }

    None
}

/// Extract `T` from a MIR `Option<T>` type.
fn extract_option_inner_from_mir_ty(
    ty: &regseq_model::declaration::mir_body::MirTy,
    tc: super::expr_lowering::TypeCtx<'_>,
) -> Option<SemanticType> {
    use regseq_model::declaration::mir_body::MirTy;

    match ty {
        MirTy::Adt { path, args } if path.contains("Option") && !args.is_empty() => {
            Some(lower_mir_ty(&args[0], tc.0, tc.1))
        }
        MirTy::Ref { ty: inner, .. } => extract_option_inner_from_mir_ty(inner, tc),
        _ => None,
    }
}

/// Make the `placeholder_reg_field_ref` function accessible from here.
pub use super::call_lowering::placeholder_reg_field_ref;

fn resource_kind_from_option_like_type(ty: &SemanticType) -> Option<ResourceKind> {
    match ty {
        SemanticType::Option { inner } => resource_kind_from_option_like_type(inner),
        SemanticType::Ref { inner } | SemanticType::RefMut { inner } => {
            resource_kind_from_option_like_type(inner)
        }
        SemanticType::ResourceHandle { kind } => Some(*kind),
        _ => None,
    }
}

fn resource_kind_for_property(
    property: regseq_model::semantic::ops::ResourceProperty,
) -> Option<ResourceKind> {
    match property {
        regseq_model::semantic::ops::ResourceProperty::ClockRate => Some(ResourceKind::Clock),
        regseq_model::semantic::ops::ResourceProperty::ResetState => Some(ResourceKind::Reset),
        regseq_model::semantic::ops::ResourceProperty::PinCtrlState => Some(ResourceKind::PinCtrl),
    }
}
