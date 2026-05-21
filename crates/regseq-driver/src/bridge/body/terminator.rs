//! MIR `Terminator` bridge translation.

use regseq_model::{
    args_key_for_generic_args_with_pending, AssertKind, BlockId, CallCallee, ClosureCallKind,
    FieldIdx, FnAbi, FnSig, FnSigId, FunctionInstanceData, FunctionInstanceFlavor, GenericArg,
    Instance, InstanceId, InstanceKind, ItemId, Mutability, Operand, Place, PlaceElem, Rvalue,
    Safety, Span, Statement, StatementKind, SwitchTargets, Terminator, TerminatorKind, Ty,
    UNIT_LAYOUT,
};
use rustc_hir::LangItem;
use rustc_middle::mir;
use rustc_middle::ty;

use crate::bridge::{
    canonicalize_call_site_callee, diag_log, synth, ty::translate_ty, BridgeCtxt, BridgeError,
    CanonicalCallSiteCallee, DslRejectionKind, ItemHandle,
};

use super::{
    operand::translate_operand, place::translate_place, rvalue::translate_bin_op,
    statement::translate_span, BodyCtxt,
};

/// Translate a rustc `mir::Terminator` into a
/// `regseq_model::Terminator`. Per spec.md section 23.7. Rustc
/// `Drop` collapses into `Call` per spec.md sections 23.4 and
/// 25.7.1 inside the atomic per-rustc-terminator translation envelope.
///
/// The caller threads the basic block's mutable statement accumulator
/// through `statements` so this function can append the Drop-as-Call
/// structural prelude (`&raw mut place` Assign) before returning the
/// synthesised Call terminator. Per audit finding regseq-driver-42-1.
pub fn translate_terminator<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx, '_>,
    statements: &mut Vec<Statement>,
    term: rustc_middle::mir::Terminator<'tcx>,
) -> Result<regseq_model::Terminator, BridgeError> {
    let span = translate_span(cx, term.source_info.span);
    let kind = match term.kind {
        mir::TerminatorKind::Goto { target } => TerminatorKind::Goto(block_id(target)),
        mir::TerminatorKind::SwitchInt { discr, targets } => TerminatorKind::SwitchInt {
            discr: translate_operand(cx, body_cx, discr)?,
            targets: translate_switch_targets(targets),
        },
        mir::TerminatorKind::Return => TerminatorKind::Return,
        mir::TerminatorKind::Unreachable => TerminatorKind::Unreachable,
        mir::TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            ..
        } => translate_call(
            cx,
            body_cx,
            statements,
            span,
            func,
            &args,
            destination,
            target,
        )?,
        mir::TerminatorKind::Assert {
            cond,
            expected,
            msg,
            target,
            ..
        } => TerminatorKind::Assert {
            cond: translate_operand(cx, body_cx, cond)?,
            expected,
            msg: translate_assert_kind(cx, body_cx, *msg)?,
            target: block_id(target),
        },
        mir::TerminatorKind::Drop { place, target, .. } => {
            translate_drop_as_call(cx, body_cx, statements, span, place, target)?
        }
        mir::TerminatorKind::FalseEdge { .. } => {
            return rejected_terminator(
                "internal error: borrowck FalseEdge survived CleanupPostBorrowck -- pipeline misconfiguration",
            );
        }
        mir::TerminatorKind::FalseUnwind { .. } => {
            return rejected_terminator(
                "internal error: borrowck FalseUnwind survived CleanupPostBorrowck -- pipeline misconfiguration",
            );
        }
        mir::TerminatorKind::Yield { .. } => {
            return rejected_terminator(
                "internal error: coroutine Yield survived StateTransform; coroutine type-rejection at stage-3 section 15.1 should have prevented reach",
            );
        }
        mir::TerminatorKind::CoroutineDrop => {
            return rejected_terminator(
                "internal error: CoroutineDrop survived StateTransform; coroutine type-rejection at stage-3 section 15.1 should have prevented reach",
            );
        }
        mir::TerminatorKind::UnwindResume => {
            return rejected_terminator(
                "internal error: UnwindResume survived panic=abort cleanup-block pruning -- pipeline misconfiguration or non-abort panic strategy",
            );
        }
        mir::TerminatorKind::UnwindTerminate(_) => {
            return rejected_terminator(
                "internal error: UnwindTerminate survived panic=abort cleanup-block pruning -- pipeline misconfiguration or non-abort panic strategy",
            );
        }
        mir::TerminatorKind::TailCall { .. } => {
            return rejected_terminator(
                "tail calls are not supported in Regseq driver source; the become expression and feature explicit_tail_calls are not part of the DSL surface",
            );
        }
        mir::TerminatorKind::InlineAsm { .. } => {
            return rejected_terminator(
                "inline assembly is not supported in Regseq driver source; use domain-specific intrinsics for low-level operations",
            );
        }
    };

    Ok(Terminator { kind, span })
}

fn block_id(block: mir::BasicBlock) -> BlockId {
    BlockId::from_u32(block.as_u32())
}

fn translate_switch_targets(targets: mir::SwitchTargets) -> SwitchTargets {
    SwitchTargets {
        cases: targets
            .iter()
            .map(|(value, target)| (value, block_id(target)))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        otherwise: block_id(targets.otherwise()),
    }
}

#[allow(clippy::too_many_arguments)]
fn translate_call<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx, '_>,
    statements: &mut Vec<Statement>,
    span: Span,
    func: mir::Operand<'tcx>,
    args: &[rustc_span::source_map::Spanned<mir::Operand<'tcx>>],
    destination: mir::Place<'tcx>,
    target: Option<mir::BasicBlock>,
) -> Result<TerminatorKind, BridgeError> {
    // Spec section 13.1 closure-call shim rewrite (audit-finding F1c,
    // regseq-driver-37 v4 task spec lines 286-299): every
    // `Fn::call` / `FnMut::call_mut` / `FnOnce::call_once` direct call
    // against a closure captures struct must be lowered into a
    // Rust-ABI `ClosureCall` synthetic Function Item with the call
    // arguments tuple-normalised.
    if let Some((callee, normalised_args)) =
        rewrite_closure_call_shim(cx, body_cx, statements, span, &func, args)?
    {
        return Ok(TerminatorKind::Call {
            callee,
            args: normalised_args.into_boxed_slice(),
            destination: translate_place(cx, body_cx, destination)?,
            target: target.map(block_id),
        });
    }
    Ok(TerminatorKind::Call {
        callee: translate_call_callee(cx, body_cx, &func)?,
        args: args
            .iter()
            .map(|arg| translate_operand(cx, body_cx, arg.node.clone()))
            .collect::<Result<Vec<_>, _>>()?
            .into_boxed_slice(),
        destination: translate_place(cx, body_cx, destination)?,
        target: target.map(block_id),
    })
}

/// Detect calls to `Fn::call` / `FnMut::call_mut` / `FnOnce::call_once`
/// against a closure receiver and rewrite them to call the
/// `synth/closure.rs`-produced `ClosureCall` Function Item directly.
/// The rustc closure-trait `RustCall` ABI passes args as a single
/// tuple operand; per spec 13.1 the synth shim takes positional
/// arguments instead, so this function untuples them.
fn rewrite_closure_call_shim<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx, '_>,
    statements: &mut Vec<Statement>,
    span: Span,
    func: &mir::Operand<'tcx>,
    args: &[rustc_span::source_map::Spanned<mir::Operand<'tcx>>],
) -> Result<Option<(CallCallee, Vec<Operand>)>, BridgeError> {
    let mir::Operand::Constant(constant) = func else {
        return Ok(None);
    };
    let ty::TyKind::FnDef(method_def_id, method_args) = constant.const_.ty().kind() else {
        return Ok(None);
    };
    let Some(trait_def_id) = cx.input.tcx.trait_of_assoc(*method_def_id) else {
        return Ok(None);
    };
    let Some(closure_kind) = cx.input.tcx.fn_trait_kind_from_def_id(trait_def_id) else {
        return Ok(None);
    };
    let receiver_ty = method_args.type_at(0);
    let ty::TyKind::Closure(closure_def_id, closure_args) = receiver_ty.kind() else {
        return Ok(None);
    };

    let call_kind = match cx.input.tcx.lang_items().from_def_id(trait_def_id) {
        Some(LangItem::Fn) => ClosureCallKind::Call,
        Some(LangItem::FnMut) => ClosureCallKind::CallMut,
        Some(LangItem::FnOnce) => ClosureCallKind::CallOnce,
        _ => return Ok(None),
    };
    let _ = closure_kind;

    let handles = synth::closure::synthesise_closure_for_def_id(
        cx,
        *closure_def_id,
        closure_args.as_closure(),
    )?;
    let Some(call_handle) = pick_closure_call_handle(cx, &handles, call_kind) else {
        return Ok(None);
    };

    let mut normalised: Vec<Operand> = Vec::new();
    if let Some(receiver) = args.first() {
        let translated_receiver = translate_operand(cx, body_cx, receiver.node.clone())?;
        // Per regseq-driver-62 + spec.md section 13.1: the closure-call
        // shim's `FnSig.inputs[0]` references the parallel captures-
        // struct `Ty::Struct { item, .. }` view of the closure (via
        // `construct_closure_call_instances` at `bridge/mod.rs:2278-2289`),
        // but the call-site MIR receiver carries the closure-type-
        // identity `Ty::Closure(item, args)` view. The two views share
        // one `Item` (and thus identical layout per spec.md:4619-4634)
        // but are distinct `Ty` interner entries, so the validator's
        // strict `CallArgTypeMismatch` check at
        // `crates/regseq-model/src/model.rs:3041-3048` rejects the
        // pairing. Bridge the gap by emitting a `Transmute` cast into
        // a fresh synthetic local typed against the shim's expected
        // captures-struct view; closure_ty and captures_struct_ty
        // share Item identity + layout, so the transmute is
        // bit-equivalent.
        if let Some(retyped) =
            cast_receiver_to_captures_view(cx, body_cx, statements, span, &translated_receiver)?
        {
            normalised.push(retyped);
        } else {
            normalised.push(translated_receiver);
        }
    }
    if let Some(tuple_arg) = args.get(1) {
        let local_decls = body_cx
            .local_decls()
            .ok_or_else(|| BridgeError::DslRejection {
                kind: DslRejectionKind::Terminator,
                ty_repr: "closure-call shim requires body local declarations to untuple args"
                    .to_string(),
            })?;
        let tuple_ty = tuple_arg.node.ty(local_decls, cx.input.tcx);
        let ty::TyKind::Tuple(tuple_fields) = tuple_ty.kind() else {
            return Ok(None);
        };
        match &tuple_arg.node {
            mir::Operand::Move(place) | mir::Operand::Copy(place) => {
                let base = translate_place(cx, body_cx, *place)?;
                let move_semantics = matches!(tuple_arg.node, mir::Operand::Move(_));
                for (idx, field_ty) in tuple_fields.iter().enumerate() {
                    let model_field_ty = translate_ty(cx, field_ty)?;
                    let mut projection: Vec<PlaceElem> = base.projection.to_vec();
                    projection.push(PlaceElem::Field(
                        FieldIdx::from_u32(idx as u32),
                        model_field_ty,
                    ));
                    let projected = Place {
                        local: base.local,
                        projection: projection.into_boxed_slice(),
                    };
                    normalised.push(if move_semantics {
                        Operand::Move(projected)
                    } else {
                        Operand::Copy(projected)
                    });
                }
            }
            _ => return Ok(None),
        }
    }

    // Audit-finding Codex review item 3: the closure-call shim's
    // direct callee MUST reference a real `model.instances` entry
    // rather than a raw `ItemHandle` cast to `InstanceId`. Resolve
    // the ItemHandle to a final ItemId post-Phase-4, then look up
    // (or lazy-create) the matching Function Instance through
    // `function_instance_for_item` keyed by
    // `(ItemId, ArgsKey, FunctionInstanceFlavor::Item)`. The synth
    // path treats every closure-call shim as a regular Function
    // body, so flavor is `Item`.
    let call_item = match cx.handles.resolve(call_handle) {
        Some(item) => item,
        None => {
            crate::bridge::assign_late_pending_items_for_terminator(cx);
            match cx.handles.resolve(call_handle) {
                Some(item) => item,
                None => {
                    return Ok(None);
                }
            }
        }
    };
    // Look up (or lazy-create) the closure-call Instance using a
    // best-effort FnSig derived from the synth-side `ClosureCall`
    // type. Closure-call shims have FnAbi::Rust per spec section
    // 13.1; the input list mirrors the rewritten positional args.
    //
    // Per regseq-driver-62: the synthetic-local allocated by
    // `cast_receiver_to_captures_view` (the closure-view -> captures-
    // view transmute) lives in the MODEL body's `local_decls` only,
    // not the rustc `local_decls` view used for tuple-untupling
    // above, so the per-operand type recovery here goes through the
    // model-side `operand_ty` helper.
    let arg_tys: Vec<regseq_model::TyId> = normalised
        .iter()
        .map(|operand| {
            let model_body = body_cx
                .model_body_mut()
                .expect("closure-call shim arg-Ty recovery requires BodyCtxt::new_with_model_body");
            match regseq_model::operand_ty(&cx.model, model_body, operand) {
                regseq_model::TypeRecovery::Recovered(ty_id) => ty_id,
                _ => cx.model.types.intern(Ty::Tuple {
                    fields: Vec::new(),
                    layout: UNIT_LAYOUT,
                }),
            }
        })
        .collect();
    let unit_ty = cx.model.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    });
    // Per regseq-driver-51, recover the closure's translated identity
    // args from the `Ty::Closure(captures_item, args)` interner entry
    // so this lazy-allocation `(ItemId, ArgsKey, FunctionInstanceFlavor)`
    // key matches the eager Instance constructed by
    // `bridge::construct_closure_call_instances`. With matching keys,
    // the `function_instance_for_item` lookup at
    // `lookup_or_create_direct_instance` (lines 513-518) returns the
    // eager Instance instead of creating a duplicate.
    let closure_model_ty = translate_ty(cx, receiver_ty)?;
    let closure_model_args = match cx.model.types.resolve(closure_model_ty) {
        Ty::Closure(_, args) => args.clone(),
        _ => Vec::new(),
    };
    let instance_id = lookup_or_create_direct_instance(
        cx,
        call_item,
        closure_model_args,
        FunctionInstanceFlavor::Item,
        None,
        move |cx| {
            Ok(cx.model.sigs.intern(FnSig {
                inputs: arg_tys,
                output: unit_ty,
                abi: FnAbi::Rust,
                c_variadic: false,
                safety: Safety::Safe,
            }))
        },
    )?;
    let callee = CallCallee::Direct(instance_id);
    Ok(Some((callee, normalised)))
}

/// Per regseq-driver-62: if the closure-call shim receiver's `Ty` in
/// the model carries the closure-type-identity `Ty::Closure(item, args)`
/// view (or `Ty::Ref(Ty::Closure(...), prov)`) and the model has a
/// parallel captures-struct `Ty::Struct { item, args, layout }` view
/// for the same `Item`, emit a `Statement::Assign` whose `Rvalue::Cast`
/// transmutes the receiver into the captures-struct view (or
/// `Ty::Ref(captures_struct, prov)` for `Call` / `CallMut`). Return
/// `Some(Operand::Move(synthetic_local))` for the caller to push into
/// `normalised` in place of the original receiver. Return `None` when
/// no view conversion is needed (e.g., the receiver already uses the
/// captures-struct view, or the operand is a constant).
///
/// The two views share one rustc `DefId` and identical layout per
/// spec.md:4619-4634, so the transmute is bit-equivalent. Without this
/// cast the validator's `CallArgTypeMismatch` check at
/// `crates/regseq-model/src/model.rs:3041-3048` rejects the pairing
/// (the closure-call shim's `FnSig.inputs[0]` is built against the
/// captures-struct view per spec.md:4590-4594 at
/// `bridge/mod.rs:2278-2289`, but the MIR receiver Place is typed
/// against the closure-type-identity view).
fn cast_receiver_to_captures_view<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx, '_>,
    statements: &mut Vec<Statement>,
    span: Span,
    receiver: &Operand,
) -> Result<Option<Operand>, BridgeError> {
    use regseq_model::CastKind;
    let receiver_place = match receiver {
        Operand::Copy(place) | Operand::Move(place) => place.clone(),
        Operand::Constant(_) => return Ok(None),
    };
    let receiver_ty_id = {
        let model_body_ref = match body_cx.model_body_mut() {
            Some(body) => body,
            None => return Ok(None),
        };
        match regseq_model::operand_ty(&cx.model, model_body_ref, receiver) {
            regseq_model::TypeRecovery::Recovered(ty_id) => ty_id,
            _ => return Ok(None),
        }
    };
    let receiver_ty = cx.model.types.resolve(receiver_ty_id).clone();
    let target_ty_id = match receiver_ty {
        Ty::Ref(pointee_ty_id, prov) => match cx
            .model
            .closure_captures_struct_ty
            .get(&pointee_ty_id)
            .copied()
        {
            Some(captures_ty_id) => cx.model.types.intern(Ty::Ref(captures_ty_id, prov)),
            None => return Ok(None),
        },
        Ty::Closure(_, _) => match cx
            .model
            .closure_captures_struct_ty
            .get(&receiver_ty_id)
            .copied()
        {
            Some(captures_ty_id) => captures_ty_id,
            None => return Ok(None),
        },
        _ => return Ok(None),
    };
    let model_body = body_cx.model_body_mut().ok_or_else(|| BridgeError::DslRejection {
        kind: DslRejectionKind::Terminator,
        ty_repr:
            "closure-call shim receiver cast requires BodyCtxt::new_with_model_body for synthetic-local allocation"
                .to_string(),
    })?;
    let synth_local = model_body.push_synthetic_local(target_ty_id);
    statements.push(Statement {
        kind: StatementKind::Assign(
            Place::from(synth_local),
            Rvalue::Cast(
                CastKind::Transmute,
                Operand::Copy(receiver_place),
                target_ty_id,
            ),
        ),
        span,
    });
    Ok(Some(Operand::Move(Place::from(synth_local))))
}

fn pick_closure_call_handle(
    cx: &BridgeCtxt<'_>,
    handles: &crate::bridge::synth::ClosureSynthHandles,
    call_kind: ClosureCallKind,
) -> Option<ItemHandle> {
    handles.calls.iter().copied().find(|handle| {
        // Per regseq-driver-62: prefer the bound final `ItemId` when
        // the handle has already been resolved (post-Phase-4). The
        // `placeholder_item_id_for_handle` fallback only applies while
        // `pending_items` still carries the synth entry (pre-Phase-4);
        // `lookup_item_synthetic_kind` cannot recover the synth kind
        // from a placeholder `ItemId` once `pending_items` has been
        // drained into `model.items` by
        // `assign_item_ids_and_populate_sidetables`. `rewrite_closure_call_shim`
        // is invoked during Phase-5 body materialisation, by which
        // time the closure-call synth handles have been bound.
        let item_id = cx
            .handles
            .resolve(*handle)
            .unwrap_or_else(|| crate::bridge::placeholder_item_id_for_handle(*handle));
        match cx.lookup_item_synthetic_kind(item_id) {
            Some(regseq_model::SyntheticItemKind::ClosureCall { call_kind: ck, .. }) => {
                ck == call_kind
            }
            _ => false,
        }
    })
}

fn translate_call_callee<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx, '_>,
    func: &mir::Operand<'tcx>,
) -> Result<CallCallee, BridgeError> {
    let func_ty = body_cx
        .local_decls()
        .map(|local_decls| func.ty(local_decls, cx.input.tcx));
    if let mir::Operand::Constant(constant) = func {
        if let ty::TyKind::FnDef(def_id, generic_args) = constant.const_.ty().kind() {
            // Audit finding regseq-driver-42-7 + spec section 15.1:
            // reject atomic / fence / volatile intrinsics BEFORE the
            // callee Item is interned so a rejection does not leave a
            // stray pending Item behind.
            reject_call_site_intrinsics(cx, *def_id)?;

            // Spec section 8.1 call-site canonicalization (regseq-driver-56):
            // resolve `ty::Instance::try_resolve` BEFORE interning the
            // callee Item so static-dispatch trait-method calls key on
            // the resolved impl DefId (not the trait declaration DefId),
            // while Virtual and shim flavors continue to use the
            // original MIR operand's (def_id, generic_args) per the
            // bridge's pre-fix behavior.
            let CanonicalCallSiteCallee {
                callee_ty,
                callee_def_id,
                callee_generic_args,
                flavor,
                virtual_idx,
            } = canonicalize_call_site_callee(cx.input.tcx, *def_id, generic_args).map_err(
                |err| {
                    BridgeError::Phase4(format!(
                        "call-site Instance::try_resolve failed for {:?} with args {:?}: {err}",
                        *def_id, generic_args,
                    ))
                },
            )?;

            // Route the canonical FnDef Ty through the same `translate_ty`
            // interning path used by other FnDef sites. This keeps the
            // resulting Item Phase-4-handle-resolvable for the `Item`
            // arm without disturbing the bare-Ty `TyKind::FnDef` arm at
            // `ty::translate_ty`.
            let model_ty = translate_ty(cx, callee_ty)?;
            let Ty::FnDef(callee_item, model_args) = cx.model.types.resolve(model_ty).clone()
            else {
                return rejected_terminator(format!("unsupported call callee {func:?}"));
            };
            // Source-derived Items carry placeholder ItemIds until
            // Phase 4 binds them (audit-finding F4); look up through
            // pending_items when the ItemId hasn't yet been assigned.
            let callee_def_path = cx
                .lookup_item_def_path(callee_item)
                .expect("FnDef callee Item must be interned")
                .clone();
            diag_log::recognise_log_intrinsic_def_id(
                cx,
                ItemHandle::from_u32(callee_item.as_u32()),
                &callee_def_path,
                callee_def_id,
            )?;
            // Audit finding regseq-driver-42-4 + spec section 23.7:
            // resolve direct callees through `function_instance_for_item`
            // keyed by `(ItemId, ArgsKey, FunctionInstanceFlavor)`. Per
            // spec section 11 + 23.7 dyn-Trait method calls surface as
            // `InstanceKind::Virtual(_, idx)`; the resulting Regseq
            // Instance is Direct-flavored Virtual with virtual_idx =
            // Some(idx) so the stage-11 vtable-indirect rewrite can
            // replace the Direct callee with a vtable load.
            let def_id_copy = callee_def_id;
            let generic_args_copy = callee_generic_args;
            let instance_id = lookup_or_create_direct_instance(
                cx,
                callee_item,
                model_args,
                flavor,
                virtual_idx,
                |cx| callsite_fn_sig(cx, def_id_copy, generic_args_copy),
            )?;
            return Ok(CallCallee::Direct(instance_id));
        }
    }
    if matches!(func_ty.map(|ty| ty.kind()), Some(ty::TyKind::FnPtr(..))) {
        // Section 15.1 call-site rejections (Codex review item 4):
        // intrinsic-style def_ids cannot appear behind an FnPtr
        // (rustc forbids taking a function pointer to an intrinsic
        // or the volatile/fence shim wrappers because they are not
        // ABI-stable concrete functions). The rejection is therefore
        // vacuous here, but the structure mirrors the FnDef branch
        // so a future surface that admits FnPtr intrinsics will
        // surface through the same path.
        return Ok(CallCallee::Indirect(translate_operand(
            cx,
            body_cx,
            func.clone(),
        )?));
    }
    rejected_terminator(format!("unsupported call callee {func:?}"))
}

/// Audit-finding regseq-driver-42-7 / spec section 15.1: reject
/// atomic/fence/volatile intrinsic call sites with
/// `DslRejectionKind::Terminator` BEFORE the callee type is interned.
fn reject_call_site_intrinsics<'tcx>(
    cx: &BridgeCtxt<'tcx>,
    def_id: rustc_span::def_id::DefId,
) -> Result<(), BridgeError> {
    if let Some(intrinsic) = cx.input.tcx.intrinsic(def_id) {
        let name = intrinsic.name.as_str();
        if name.starts_with("atomic_") || name.starts_with("volatile_") {
            return rejected_terminator(format!(
                "call to intrinsic `{name}` is rejected per spec section 15.1"
            ));
        }
    }
    // `core::sync::atomic::{fence, compiler_fence}` and
    // `core::ptr::{read,write}_volatile` are stable shim wrappers
    // around the underlying intrinsics. Reject by their stable
    // DefPath strings so DSL-source calls that go through the shim
    // surface are caught at the bridge.
    let path_str = cx.input.tcx.def_path_str(def_id);
    if matches!(
        path_str.as_str(),
        "core::sync::atomic::fence"
            | "std::sync::atomic::fence"
            | "core::sync::atomic::compiler_fence"
            | "std::sync::atomic::compiler_fence"
            | "core::ptr::read_volatile"
            | "std::ptr::read_volatile"
            | "core::ptr::write_volatile"
            | "std::ptr::write_volatile"
    ) {
        return rejected_terminator(format!(
            "call to `{path_str}` is rejected per spec section 15.1"
        ));
    }
    Ok(())
}

fn callsite_fn_sig<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    def_id: rustc_span::def_id::DefId,
    generic_args: ty::GenericArgsRef<'tcx>,
) -> Result<FnSigId, BridgeError> {
    let poly = cx
        .input
        .tcx
        .fn_sig(def_id)
        .instantiate(cx.input.tcx, generic_args);
    crate::bridge::fnsig::translate_fn_sig(cx, poly)
}

fn lookup_or_create_direct_instance<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    item: ItemId,
    generic_args: regseq_model::GenericArgs,
    flavor: FunctionInstanceFlavor,
    virtual_idx: Option<u32>,
    make_fn_sig: impl FnOnce(&mut BridgeCtxt<'tcx>) -> Result<FnSigId, BridgeError>,
) -> Result<InstanceId, BridgeError> {
    // Audit-finding regseq-driver-43-3 + spec section 3.4 D-12-5.3:
    // the callee `Item` may still be a Phase-2 placeholder at this
    // point if it was first interned during this body's
    // materialisation (e.g., a trait method's `FnDef` reached
    // through a virtual call site that the Phase-2 scan never
    // walked). Drain any outstanding `pending_items` AND resolve
    // the placeholder ItemId to its final value BEFORE keying the
    // `function_instance_for_item` table -- otherwise the
    // pre-resolution key would record a placeholder-keyed entry
    // that later lookups (always using real ItemIds) cannot find.
    let item = if item.as_u32() as usize >= cx.model.items.len() {
        crate::bridge::assign_late_pending_items_for_terminator(cx);
        let handle_idx = ItemId::MAX_AS_U32.saturating_sub(item.as_u32());
        let handle = crate::bridge::ItemHandle::from_u32(handle_idx);
        cx.handles.resolve(handle).ok_or_else(|| {
            BridgeError::Phase5(format!(
                "callee placeholder {} could not be bound to a final ItemId",
                item.as_u32()
            ))
        })?
    } else {
        item
    };
    let pending = cx.pending_def_path_hashes();
    let args_key = args_key_for_generic_args_with_pending(&cx.model, &generic_args, &pending);
    let key = (item, args_key, flavor);
    if let Some(existing) = cx.function_instance_for_item.get(&key) {
        return Ok(*existing);
    }
    let fn_sig = make_fn_sig(cx)?;
    crate::bridge::fnsig::ensure_cabi_lowering_slots(cx);
    let emission_name = cx.model.items[item].def_path.rendered;
    let span = cx.model.items[item].span;
    let instance_id = cx.model.instances.push(Instance {
        item,
        generic_args,
        emission_name,
        span,
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig,
            body: None,
            flavor,
            virtual_idx,
        }),
    });
    cx.function_instance_for_item.insert(key, instance_id);
    Ok(instance_id)
}

fn translate_drop_as_call<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx, '_>,
    statements: &mut Vec<Statement>,
    span: Span,
    place: mir::Place<'tcx>,
    target: mir::BasicBlock,
) -> Result<TerminatorKind, BridgeError> {
    let Some(local_decls) = body_cx.local_decls() else {
        return rejected_terminator("Drop terminator without body local declarations");
    };
    let place_ty = place.ty(local_decls, cx.input.tcx).ty;
    let model_ty = translate_ty(cx, place_ty)?;

    // Resolve (or eagerly synthesise) the drop-glue Item handle and
    // bind it to a final ItemId per spec section 23.4 step 1. The
    // Phase-2 cache holds an `ItemHandle`; the HandleTable resolves
    // it to a `ItemId` post-Phase-4. Cache misses during Phase 5
    // (eager synthesis at the drop site) are drained through the
    // late Phase-4 assignment path so the freshly minted handle
    // picks up a real `ItemId` before the Call is emitted (Codex
    // review fix item 1).
    let drop_glue_item = if let Some(handle) = cx.drop_glue_synth_cache.get(&model_ty).copied() {
        if let Some(item) = cx.handles.resolve(handle) {
            item
        } else {
            crate::bridge::assign_late_pending_items_for_terminator(cx);
            cx.handles.resolve(handle).ok_or_else(|| {
                BridgeError::Phase5(format!(
                    "drop-glue handle {} could not be bound after late assignment",
                    handle.as_u32()
                ))
            })?
        }
    } else {
        let handle = synth::synthesise_drop_glue(cx, model_ty)?;
        crate::bridge::assign_late_pending_items_for_terminator(cx);
        cx.handles.resolve(handle).ok_or_else(|| {
            BridgeError::Phase5(format!(
                "drop-glue handle {} could not be bound after eager synthesis",
                handle.as_u32()
            ))
        })?
    };

    // Audit finding regseq-driver-42-6 + spec section 23.4: dyn-Trait
    // drops resolve to `ty::InstanceKind::Virtual(drop_fn.def_id(), 0)`;
    // the Regseq Instance is Virtual-flavored with virtual_idx = 0.
    // Concrete drops resolve to `ty::InstanceKind::DropGlue(_, Some(ty))`;
    // the Regseq Instance is DropGlue-flavored with virtual_idx = None.
    let is_dynamic = matches!(place_ty.kind(), ty::TyKind::Dynamic(..));
    let (flavor, virtual_idx) = if is_dynamic {
        (FunctionInstanceFlavor::Virtual, Some(0u32))
    } else {
        (FunctionInstanceFlavor::DropGlue, None)
    };

    // Allocate the drop-glue fn_sig: `fn(*mut place_ty) -> ()` (or
    // `fn(*mut ()) -> ()` for the erased Virtual case) per spec
    // section 25.7.1.
    let ptr_to_place_ty = cx.model.types.intern(Ty::Ptr(model_ty, Mutability::Mut));
    let unit_ty = cx.model.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    });
    let drop_glue_input_ty = if is_dynamic {
        cx.model.types.intern(Ty::Ptr(unit_ty, Mutability::Mut))
    } else {
        ptr_to_place_ty
    };

    let instance_id = lookup_or_create_direct_instance(
        cx,
        drop_glue_item,
        vec![GenericArg::Type(model_ty)],
        flavor,
        virtual_idx,
        |cx| {
            Ok(cx.model.sigs.intern(FnSig {
                inputs: vec![drop_glue_input_ty],
                output: unit_ty,
                abi: FnAbi::Rust,
                c_variadic: false,
                safety: Safety::Unsafe,
            }))
        },
    )?;

    // Allocate the two synthetic locals through the mutable model
    // body per audit finding regseq-driver-42-2. Without a mutable
    // body slot the Drop-as-Call collapse cannot run; tests that
    // exercise the collapse must use
    // `BodyCtxt::new_with_model_body`.
    let model_body = body_cx.model_body_mut().ok_or_else(|| {
        BridgeError::DslRejection {
            kind: DslRejectionKind::Terminator,
            ty_repr:
                "Drop-as-Call collapse requires BodyCtxt::new_with_model_body for synthetic-local allocation"
                    .to_string(),
        }
    })?;
    let drop_arg = model_body.push_synthetic_local(ptr_to_place_ty);
    let drop_ret = model_body.push_synthetic_local(unit_ty);

    // Translate the drop place through the standard path AFTER
    // synthetic-local allocation so the place projections see the
    // pre-existing local table.
    let model_place = translate_place(cx, body_cx, place)?;

    // Append the structural prelude per spec section 23.4 step 4:
    // `Statement::Assign(_t_drop_arg, Rvalue::RawPtr(Mut, place))`.
    statements.push(Statement {
        kind: StatementKind::Assign(
            Place::from(drop_arg),
            Rvalue::RawPtr(Mutability::Mut, model_place),
        ),
        span,
    });

    Ok(TerminatorKind::Call {
        callee: CallCallee::Direct(instance_id),
        args: Box::new([Operand::Move(Place::from(drop_arg))]),
        destination: Place::from(drop_ret),
        target: Some(block_id(target)),
    })
}

fn translate_assert_kind<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx, '_>,
    msg: mir::AssertKind<mir::Operand<'tcx>>,
) -> Result<AssertKind, BridgeError> {
    Ok(match msg {
        mir::AssertKind::BoundsCheck { len, index } => AssertKind::BoundsCheck {
            len: translate_operand(cx, body_cx, len)?,
            index: translate_operand(cx, body_cx, index)?,
        },
        mir::AssertKind::Overflow(op, left, right) => AssertKind::Overflow(
            translate_bin_op(op)?,
            translate_operand(cx, body_cx, left)?,
            translate_operand(cx, body_cx, right)?,
        ),
        mir::AssertKind::OverflowNeg(operand) => {
            AssertKind::OverflowNeg(translate_operand(cx, body_cx, operand)?)
        }
        mir::AssertKind::DivisionByZero(operand) => {
            AssertKind::DivisionByZero(translate_operand(cx, body_cx, operand)?)
        }
        mir::AssertKind::RemainderByZero(operand) => {
            AssertKind::RemainderByZero(translate_operand(cx, body_cx, operand)?)
        }
        mir::AssertKind::MisalignedPointerDereference { required, found } => {
            AssertKind::MisalignedPointerDereference {
                required: translate_operand(cx, body_cx, required)?,
                found: translate_operand(cx, body_cx, found)?,
            }
        }
        mir::AssertKind::NullPointerDereference => AssertKind::NullPointerDereference,
        mir::AssertKind::InvalidEnumConstruction(operand) => {
            AssertKind::InvalidEnumConstruction(translate_operand(cx, body_cx, operand)?)
        }
        mir::AssertKind::ResumedAfterReturn(_)
        | mir::AssertKind::ResumedAfterPanic(_)
        | mir::AssertKind::ResumedAfterDrop(_) => {
            return rejected_terminator("coroutine assert kind survived coroutine rejection");
        }
    })
}

fn rejected_terminator<T>(construct: impl Into<String>) -> Result<T, BridgeError> {
    Err(BridgeError::DslRejection {
        kind: DslRejectionKind::Terminator,
        ty_repr: construct.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::block_id;
    use regseq_model::BlockId;
    use rustc_middle::mir;

    #[test]
    fn block_id_preserves_rustc_basic_block_index() {
        assert_eq!(block_id(mir::BasicBlock::from_u32(7)), BlockId::from_u32(7));
    }
}
