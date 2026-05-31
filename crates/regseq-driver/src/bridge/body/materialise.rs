//! Phase-5 function body materialisation.

use regseq_model::{
    validate_body_places, AssertKind, AttrData, Body, BodyValidationError, CallCallee,
    ClosureCallKind, FunctionInstanceFlavor, GenericArg, InstanceId, InstanceKind, IntrinsicKind,
    LocalDecl, LocalId, Mutability, NonDivergingIntrinsic, Operand, Place, PlaceElem, Rvalue,
    StatementKind, SyntheticItemKind, Terminator, TerminatorKind, Ty, TyId, Visitor,
};
use rustc_hir::def_id::LocalDefId;
use rustc_middle::ty::{self, GenericArgs, Instance, TyCtxt};

use crate::bridge::{ty::translate_ty, BridgeCtxt, BridgeError, RevealOpaqueScope};

use super::{
    statement::translate_span, translate_basic_block, vtable_indirect::rewrite_vtable_indirect,
    BodyCtxt,
};

/// Phase-5 handle for a function instance whose body is being
/// materialised.
pub type InstanceHandle = InstanceId;

/// Outcome of a single `materialise_function_body` invocation that did
/// not produce a body. Used internally by `run_phase5_body_passes` to
/// distinguish "this flavor has no own MIR" (Virtual, VTable shims)
/// from a real bridge error. Per audit-finding regseq-driver-43-3.
#[derive(Debug)]
enum NoBodyReason {
    /// Virtual-flavor instance: body is genuinely None per spec
    /// section 25.7.2; the vtable-indirect rewrite substitutes the
    /// indirect call against the vtable function pointer.
    Virtual,
    /// Drop-glue, closure-call, or other shim instance whose source
    /// def_id could not be resolved (e.g., cross-crate synthetic
    /// items, flavors without rustc-recoverable MIR). The bridge
    /// leaves body None and continues per audit-finding
    /// regseq-driver-43-3 graceful-degradation rule.
    UnresolvableSource,
}

/// Materialise the body of one Function Instance during Phase 5 per
/// spec.md section 25.7. Walks rustc basic blocks, populating
/// LocalDecls via the section 25.8 allocation API and translating
/// each block via `translate_basic_block`.
pub fn materialise_function_body<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    instance_handle: InstanceHandle,
) -> Result<Body, BridgeError> {
    match materialise_function_body_inner(cx, instance_handle)? {
        Ok(body) => Ok(body),
        Err(reason) => Err(BridgeError::Phase5(format!(
            "instance {} has no materialisable body: {reason:?}",
            instance_handle.as_u32()
        ))),
    }
}

fn materialise_function_body_inner<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    instance_handle: InstanceHandle,
) -> Result<Result<Body, NoBodyReason>, BridgeError> {
    if instance_handle.index() >= cx.model.instances.len() {
        return Err(BridgeError::Phase5(format!(
            "missing instance {}",
            instance_handle.as_u32()
        )));
    }
    let instance = &cx.model.instances[instance_handle];
    let InstanceKind::Function(function_data) = &instance.kind else {
        return Err(BridgeError::Phase5(format!(
            "instance {} is not a function",
            instance_handle.as_u32()
        )));
    };
    let flavor = function_data.flavor;
    let item = instance.item;
    let generic_args = instance.generic_args.clone();
    let synthetic_kind = cx.model.item_synthetic_kinds[item].clone();

    // Per audit-finding regseq-driver-43-3, resolve the rustc
    // `Instance` (carrying both the `InstanceKind` and the
    // `GenericArgsRef` used by `tcx.instance_mir` and by the
    // monomorphizing substitution below) by explicit-dispatch over
    // each `FunctionInstanceFlavor` variant. Every flavor is matched
    // by name -- no silent "fallthrough" -- so the audit's
    // prohibition on "silently leaving required non-Item bodies
    // absent" holds. SyntheticItemKind takes precedence for
    // ClosureCall (which overlays a model-side Item flavor with a
    // closure DefPath).
    //
    // Per regseq-driver-60: the full `Instance<'tcx>` (def + args)
    // is threaded through so the post-`instance_mir` substitution
    // call can apply `instance.args` to the polymorphic MIR body
    // returned by rustc's query cache. Previous code kept only the
    // `.def` field and dropped `.args` on the floor, which caused
    // any `TyKind::Param` in `local_decls` (e.g., `Self` in a trait
    // default body) to be fed verbatim into `translate_ty` and
    // rejected as `BridgeError::NonRigidTy`.
    let rustc_instance: Instance<'tcx> =
        if let SyntheticItemKind::ClosureCall { closure, .. } = &synthetic_kind {
            // Closure-call shim per spec section 13.1 + audit-finding
            // regseq-driver-43-3: body is the closure body itself,
            // reached via Item kind on the closure's rustc DefId.
            let closure_hash = *closure;
            let Some(closure_def_id) = cx.input.tcx.hir_body_owners().find(|local_def_id| {
                def_path_hash_matches(cx.input.tcx, *local_def_id, closure_hash)
            }) else {
                return Ok(Err(NoBodyReason::UnresolvableSource));
            };
            let translated_args =
                translate_model_generic_args(cx, closure_def_id.to_def_id(), &generic_args)?;
            Instance::new_raw(closure_def_id.to_def_id(), translated_args)
        } else if matches!(synthetic_kind, SyntheticItemKind::VTableDropShim { .. }) {
            // VTable drop shim per audit-finding regseq-driver-43-3
            // bullet "VTable shim flavors": body comes from the
            // appropriate shim builder. The shim builder for a vtable
            // drop slot wraps the concrete drop-in-place glue of the
            // self_ty; we materialise it via `InstanceKind::DropGlue`
            // exactly like the concrete `FunctionInstanceFlavor::
            // DropGlue` path. The dropped TyId arrives in
            // `Instance.generic_args[0]` per
            // `terminator.rs::translate_drop_as_call`.
            let Some(impl_ty_id) = generic_args.iter().find_map(|arg| match arg {
                GenericArg::Type(ty) => Some(*ty),
                _ => None,
            }) else {
                return Ok(Err(NoBodyReason::UnresolvableSource));
            };
            let Some(impl_ty) = cx.ty_forward_cache.get(&impl_ty_id).copied() else {
                return Ok(Err(NoBodyReason::UnresolvableSource));
            };
            let drop_in_place = cx
                .input
                .tcx
                .require_lang_item(rustc_hir::LangItem::DropInPlace, rustc_span::DUMMY_SP);
            // `Instance::new_raw` would force `def =
            // InstanceKind::Item(_)`, so build the struct directly
            // per regseq-driver-60. Args carry the dropped self_ty
            // to match `DropGlue(_, Some(_))`'s monomorphic key.
            Instance {
                def: ty::InstanceKind::DropGlue(drop_in_place, Some(impl_ty)),
                args: cx.input.tcx.mk_args(&[impl_ty.into()]),
            }
        } else if let SyntheticItemKind::VTableMethodShim {
            method_def_path, ..
        } = &synthetic_kind
        {
            // VTable method shim per audit-finding regseq-driver-43-3
            // bullet "VTable shim flavors": body comes from
            // `tcx.instance_mir(VTableShim(method_def_id))` using the
            // synthetic kind's `method_def_path` for the source
            // method. We do NOT use `Item(...)` here per audit.
            let hash = *method_def_path;
            let Some(method_def_id) = cx
                .input
                .tcx
                .hir_body_owners()
                .find(|local_def_id| def_path_hash_matches(cx.input.tcx, *local_def_id, hash))
            else {
                return Ok(Err(NoBodyReason::UnresolvableSource));
            };
            // VTableShim is keyed by DefId; the shim's body is
            // generated by rustc's `mir_shims` and its args list is
            // empty per regseq-driver-60 spec hint.
            Instance {
                def: ty::InstanceKind::VTableShim(method_def_id.to_def_id()),
                args: cx.input.tcx.mk_args(&[]),
            }
        } else {
            match flavor {
                FunctionInstanceFlavor::Item => {
                    let Some(local_def_id) = local_def_id_for_item(cx.input.tcx, &cx.model, item)
                    else {
                        return Ok(Err(NoBodyReason::UnresolvableSource));
                    };
                    let translated_args =
                        translate_model_generic_args(cx, local_def_id.to_def_id(), &generic_args)?;
                    Instance::new_raw(local_def_id.to_def_id(), translated_args)
                }
                FunctionInstanceFlavor::DropGlue => {
                    // Drop-glue Function Item (concrete drop) per
                    // audit-finding regseq-driver-43-3: body via
                    // `InstanceKind::DropGlue(drop_in_place_def_id,
                    // Some(impl_ty))`. The dropped TyId arrives in
                    // `Instance.generic_args[0]` per
                    // `terminator.rs::translate_drop_as_call` (which
                    // calls `lookup_or_create_direct_instance` with
                    // `vec![GenericArg::Type(model_ty)]`).
                    let Some(impl_ty_id) = generic_args.iter().find_map(|arg| match arg {
                        GenericArg::Type(ty) => Some(*ty),
                        _ => None,
                    }) else {
                        return Ok(Err(NoBodyReason::UnresolvableSource));
                    };
                    let Some(impl_ty) = cx.ty_forward_cache.get(&impl_ty_id).copied() else {
                        return Ok(Err(NoBodyReason::UnresolvableSource));
                    };
                    let drop_in_place = cx
                        .input
                        .tcx
                        .require_lang_item(rustc_hir::LangItem::DropInPlace, rustc_span::DUMMY_SP);
                    Instance {
                        def: ty::InstanceKind::DropGlue(drop_in_place, Some(impl_ty)),
                        args: cx.input.tcx.mk_args(&[impl_ty.into()]),
                    }
                }
                FunctionInstanceFlavor::VTableShim => {
                    // Rustc-resolved vtable shim
                    // (`InstanceKind::VTableShim(def_id)`). Per
                    // audit-finding regseq-driver-43-3 we explicitly
                    // do NOT use `Item(...)` for vtable shims; the
                    // body comes from
                    // `tcx.instance_mir(VTableShim(method_def_id))`.
                    // For a `SyntheticItemKind::VTableMethodShim`
                    // owner the underlying source method's DefPath
                    // lives in the synthetic kind's `method_def_path`
                    // field (the owning Item's own DefPathHash is
                    // synthesised). For source-derived Items the
                    // owner's DefPathHash IS the method's hash.
                    let method_hash = if let SyntheticItemKind::VTableMethodShim {
                        method_def_path,
                        ..
                    } = &synthetic_kind
                    {
                        *method_def_path
                    } else {
                        cx.model.item_def_paths[item]
                    };
                    let Some(method_def_id) = cx.input.tcx.hir_body_owners().find(|local_def_id| {
                        def_path_hash_matches(cx.input.tcx, *local_def_id, method_hash)
                    }) else {
                        return Ok(Err(NoBodyReason::UnresolvableSource));
                    };
                    Instance {
                        def: ty::InstanceKind::VTableShim(method_def_id.to_def_id()),
                        args: cx.input.tcx.mk_args(&[]),
                    }
                }
                FunctionInstanceFlavor::Virtual => {
                    // Per spec section 25.7.2 + audit-finding
                    // regseq-driver-43-3: Virtual flavor has no own MIR;
                    // the vtable-indirect rewrite substitutes the
                    // indirect call against the vtable function
                    // pointer. Body remains None.
                    return Ok(Err(NoBodyReason::Virtual));
                }
                FunctionInstanceFlavor::Intrinsic
                | FunctionInstanceFlavor::ReifyShim
                | FunctionInstanceFlavor::FnPtrShim
                | FunctionInstanceFlavor::ClosureOnceShim
                | FunctionInstanceFlavor::ConstructCoroutineInClosureShim
                | FunctionInstanceFlavor::ThreadLocalShim
                | FunctionInstanceFlavor::FutureDropPollShim
                | FunctionInstanceFlavor::CloneShim
                | FunctionInstanceFlavor::FnPtrAddrShim
                | FunctionInstanceFlavor::AsyncDropGlueCtorShim
                | FunctionInstanceFlavor::AsyncDropGlue => {
                    // Flavors enumerated here are recognised explicitly
                    // (per audit-finding regseq-driver-43-3 prohibition
                    // on "silently leaving non-Item bodies absent"); the
                    // bridge currently has no shim builder for these and
                    // they have no own MIR that maps to a Regseq Body.
                    // Recognising the variant here without producing a
                    // body is the explicit "deferred to a future task"
                    // signal; the spec section 25.7 dispatch table only
                    // mandates Item / DropGlue / ClosureCall / Virtual /
                    // VTable-shim coverage and leaves these other
                    // flavors out of scope for this task.
                    return Ok(Err(NoBodyReason::UnresolvableSource));
                }
            }
        };

    // Per regseq-driver-60: fetch the polymorphic MIR via
    // `tcx.instance_mir`, then apply `instance.args` to substitute
    // every `TyKind::Param` in `local_decls` / statements /
    // terminators with the concrete `Ty` carried by the Function
    // Instance. `instantiate_mir_and_normalize_erasing_regions` is
    // a no-op for Instances whose `has_polymorphic_mir_body` is
    // false (e.g., `DropGlue(_, Some(_))`) and for Instances whose
    // args are the identity substitution. The `clone` is
    // unavoidable: `tcx.instance_mir(...)` returns `&'tcx Body<'tcx>`
    // (reference into the rustc query cache) and `EarlyBinder::bind`
    // requires an owned value to substitute into. The substituted
    // body is allocated in the tcx arena so downstream code can
    // borrow it via the existing `&'tcx LocalDecls<'tcx>` API
    // expected by `BodyCtxt::new_with_model_body`.
    let rustc_body_poly = cx.input.tcx.instance_mir(rustc_instance.def);
    let rustc_body: &rustc_middle::mir::Body<'tcx> = cx.input.tcx.arena.alloc(
        rustc_instance.instantiate_mir_and_normalize_erasing_regions(
            cx.input.tcx,
            ty::TypingEnv::fully_monomorphized(),
            ty::EarlyBinder::bind(rustc_body_poly.clone()),
        ),
    );

    let mut body = Body {
        basic_blocks: regseq_model::IndexVec::new(),
        local_decls: regseq_model::IndexVec::new(),
        arg_count: rustc_body.arg_count,
        span: translate_span(cx, rustc_body.span),
    };

    // Per regseq-driver-82: enable body-context RPIT opaque reveal for
    // the remainder of this per-Instance body materialisation -- the
    // local-decl loop below AND the basic-block walk (statements,
    // places, rvalues, terminators all route through `translate_ty`),
    // including any Instance lazily created mid-walk. The RAII guard
    // restores the prior flag on every exit path, including the `?`
    // early returns in the loops, so the reveal cannot leak past this
    // body into identity-signature translation.
    let mut reveal_guard = RevealOpaqueScope::enter(cx);
    let cx = &mut *reveal_guard;

    for local in rustc_body.local_decls.iter() {
        let ty = translate_ty(cx, local.ty)?;
        body.push_local(LocalDecl {
            ty,
            mutability: translate_local_mutability(local.mutability),
            span: translate_span(cx, local.source_info.span),
            debug_name: None,
        });
    }

    // Collect basic blocks one at a time so the mutable Body borrow
    // held by `BodyCtxt::new_with_model_body` is released between
    // iterations -- the Drop-as-Call collapse and any future
    // synthetic-local allocator inside `translate_basic_block` write
    // to `body.local_decls` directly. Per audit finding
    // regseq-driver-42-2 + spec section 25.7.1.
    let blocks: Vec<_> = rustc_body.basic_blocks.iter().cloned().collect();
    let mut translated_blocks = Vec::with_capacity(blocks.len());
    for block in &blocks {
        let translated = {
            let mut body_cx = BodyCtxt::new_with_model_body(&rustc_body.local_decls, &mut body);
            translate_basic_block(cx, &mut body_cx, block)?
        };
        translated_blocks.push(translated);
    }
    for translated in translated_blocks {
        body.basic_blocks.push(translated);
    }

    // Per regseq-driver-85: reconcile the `call_once` shim's by-value
    // receiver decl with its body projection. Rustc reuses the closure's
    // own by-reference MIR body for all three `Fn`-family call kinds, so
    // even the `call_once` shim arrives with receiver local `_1` typed
    // `&Captures` and projected through a leading `Deref`. The shim's
    // type-system `FnSig` receiver is by value (spec section 13.1), so
    // the prototype declares `_1` as a by-value struct -- the body's
    // `(*_1)...` then dereferences a non-pointer and `cc` rejects it.
    // Retype `_1` to the by-value receiver and strip the now-spurious
    // leading `Deref`, leaving `call` / `call_mut` (genuine reference
    // receivers) untouched.
    reconcile_call_once_receiver(cx, instance_handle, &synthetic_kind, &mut body);

    // Reconcile the vtable method shim's receiver level (vtable task). Rustc's
    // `VtableShim` for a reference receiver (`&self` / `&mut self`) wraps it in
    // an extra `*mut`: the body local `_1` arrives typed `*mut &Self` and the
    // forwarding call derefs it once (`read_byte((*_1))`). The trait-object
    // dispatch passes the data pointer (= `&Self`) by value -- one level less --
    // so without this the shim derefs the object pointer and reads garbage.
    // Retype `_1` to its pointee (the method's reference receiver) and strip the
    // leading `Deref`, so the shim forwards the data pointer directly.
    reconcile_vtable_shim_receiver(cx, &synthetic_kind, &mut body);

    Ok(Ok(body))
}

/// Reconcile the `call_once` closure-call shim's by-value receiver
/// declaration with its body projection (regseq-driver-85).
///
/// Rustc materialises the same by-reference closure body for `call`,
/// `call_mut`, and `call_once`, so the `call_once` shim's body receiver
/// local `_1` arrives typed as a reference (`&Captures` / `&Closure`)
/// and every use projects it through a leading `PlaceElem::Deref`. The
/// shim's `FnSig` receiver is by value (the captures struct) per spec
/// section 13.1, which the backend uses for the C prototype parameter --
/// so the body's `(*_1)` dereferences a by-value struct and the emitted
/// C fails to type-check.
///
/// When the instance is a `CallOnce` shim whose `FnSig` receiver is by
/// value (not a `Ty::Ref`) but whose materialised body receiver `_1` is
/// a reference, this retypes `_1` to the `FnSig` receiver type and
/// removes the leading `Deref` from every place rooted at `_1`. The
/// `call` / `call_mut` shims (whose `FnSig` receiver IS a reference)
/// already agree with the body and are left untouched.
fn reconcile_call_once_receiver(
    cx: &BridgeCtxt<'_>,
    instance_handle: InstanceHandle,
    synthetic_kind: &SyntheticItemKind,
    body: &mut Body,
) {
    if !matches!(
        synthetic_kind,
        SyntheticItemKind::ClosureCall {
            call_kind: ClosureCallKind::CallOnce,
            ..
        }
    ) {
        return;
    }

    // The receiver is positional argument local `_1`. A shim with no
    // arguments has no receiver to reconcile.
    if body.arg_count == 0 {
        return;
    }
    let receiver = LocalId::from_u32(1);

    // Read the shim's type-system `FnSig` receiver (its first input).
    let InstanceKind::Function(function_data) = &cx.model.instances[instance_handle].kind else {
        return;
    };
    let sig = cx.model.sigs.resolve(function_data.fn_sig);
    let Some(receiver_sig_ty) = sig.inputs.first().copied() else {
        return;
    };

    // Only reconcile when the FnSig receiver is by value (not a
    // reference) but the materialised body still declares `_1` as a
    // reference. A genuine by-value rustc body already agrees and needs
    // nothing; a reference FnSig receiver (`call` / `call_mut`) is a
    // different call kind and is excluded by the CallOnce guard above.
    if matches!(cx.model.types.resolve(receiver_sig_ty), Ty::Ref(..)) {
        return;
    }
    if !matches!(
        cx.model.types.resolve(body.local_decls[receiver].ty),
        Ty::Ref(..)
    ) {
        return;
    }

    // Retype `_1` to the by-value receiver so the prototype parameter and
    // the body local agree.
    body.local_decls[receiver].ty = receiver_sig_ty;

    // Strip the now-spurious leading `Deref` from every place rooted at
    // `_1` so the body accesses the by-value struct directly (`_1._0`).
    rewrite_receiver_places_in_body(body, receiver);
}

/// Reconcile a vtable method shim's receiver level with the trait-object
/// dispatch convention (vtable task).
///
/// Rustc materialises `InstanceKind::VTableShim` for a reference receiver
/// (`&self` / `&mut self`) with the body receiver local `_1` typed `*mut &Self`
/// -- an extra pointer level -- and every use deref's it once before forwarding
/// to the concrete method (`read_byte(move (*_1))`). The bridge's trait-object
/// dispatch (`vtable_indirect.rs`) passes the fat pointer's `data` slot, which
/// is the receiver reference itself (`&Self`), by value -- one level less. So
/// the shim, declared with the erased `*mut ()` ABI receiver but using `_1` as
/// `*mut &Self`, would dereference the object pointer and read past it.
///
/// When the shim's body receiver `_1` is `*mut <ref>` (a pointer to a reference
/// receiver), this retypes `_1` to that reference pointee and strips the leading
/// `Deref` from every place rooted at `_1`, so the shim forwards the data
/// pointer directly. By-value / `Box<Self>`-style receivers (a non-reference
/// pointee) have a different shim shape and are left untouched.
fn reconcile_vtable_shim_receiver(
    cx: &BridgeCtxt<'_>,
    synthetic_kind: &SyntheticItemKind,
    body: &mut Body,
) {
    if !matches!(synthetic_kind, SyntheticItemKind::VTableMethodShim { .. }) {
        return;
    }
    if body.arg_count == 0 {
        return;
    }
    let receiver = LocalId::from_u32(1);
    let pointee = match cx.model.types.resolve(body.local_decls[receiver].ty) {
        Ty::Ptr(pointee, _) => *pointee,
        _ => return,
    };
    // Only a reference receiver (`&self` / `&mut self`): the pointee is itself a
    // reference / pointer. A by-value or `Box<Self>` receiver has a distinct
    // shim shape this reconciliation does not model.
    if !matches!(cx.model.types.resolve(pointee), Ty::Ref(..) | Ty::Ptr(..)) {
        return;
    }
    body.local_decls[receiver].ty = pointee;
    rewrite_receiver_places_in_body(body, receiver);
}

/// Remove a leading `PlaceElem::Deref` from every place in `body` whose
/// head local is `receiver` (regseq-driver-85). Places rooted at other
/// locals are untouched.
fn rewrite_receiver_places_in_body(body: &mut Body, receiver: LocalId) {
    for block in body.basic_blocks.iter_mut() {
        for stmt in block.statements.iter_mut() {
            rewrite_receiver_places_in_statement(&mut stmt.kind, receiver);
        }
        rewrite_receiver_places_in_terminator(&mut block.terminator.kind, receiver);
    }
}

/// Strip a leading `Deref` from `place` iff it is rooted at `receiver`.
fn strip_leading_deref_if_receiver(place: &mut Place, receiver: LocalId) {
    if place.local == receiver && matches!(place.projection.first(), Some(PlaceElem::Deref)) {
        let mut elems = std::mem::take(&mut place.projection).into_vec();
        elems.remove(0);
        place.projection = elems.into_boxed_slice();
    }
}

/// Apply [`strip_leading_deref_if_receiver`] to every place reachable
/// from a statement.
fn rewrite_receiver_places_in_statement(kind: &mut StatementKind, receiver: LocalId) {
    match kind {
        StatementKind::Assign(place, rvalue) => {
            strip_leading_deref_if_receiver(place, receiver);
            rewrite_receiver_places_in_rvalue(rvalue, receiver);
        }
        StatementKind::SetDiscriminant(place, _) => {
            strip_leading_deref_if_receiver(place, receiver)
        }
        StatementKind::Intrinsic(intrinsic) => match intrinsic {
            NonDivergingIntrinsic::Assume(operand) => {
                rewrite_receiver_places_in_operand(operand, receiver)
            }
            NonDivergingIntrinsic::CopyNonOverlapping(payload) => {
                rewrite_receiver_places_in_operand(&mut payload.src, receiver);
                rewrite_receiver_places_in_operand(&mut payload.dst, receiver);
                rewrite_receiver_places_in_operand(&mut payload.count, receiver);
            }
        },
    }
}

/// Apply [`strip_leading_deref_if_receiver`] to every place reachable
/// from a terminator.
fn rewrite_receiver_places_in_terminator(kind: &mut TerminatorKind, receiver: LocalId) {
    match kind {
        TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => {}
        TerminatorKind::SwitchInt { discr, .. } => {
            rewrite_receiver_places_in_operand(discr, receiver)
        }
        TerminatorKind::Call {
            callee,
            args,
            destination,
            ..
        } => {
            if let CallCallee::Indirect(operand) = callee {
                rewrite_receiver_places_in_operand(operand, receiver);
            }
            for arg in args.iter_mut() {
                rewrite_receiver_places_in_operand(arg, receiver);
            }
            strip_leading_deref_if_receiver(destination, receiver);
        }
        TerminatorKind::Assert { cond, msg, .. } => {
            rewrite_receiver_places_in_operand(cond, receiver);
            rewrite_receiver_places_in_assert_kind(msg, receiver);
        }
    }
}

/// Apply [`strip_leading_deref_if_receiver`] to every place reachable
/// from an `AssertKind`.
fn rewrite_receiver_places_in_assert_kind(kind: &mut AssertKind, receiver: LocalId) {
    match kind {
        AssertKind::BoundsCheck { len, index } => {
            rewrite_receiver_places_in_operand(len, receiver);
            rewrite_receiver_places_in_operand(index, receiver);
        }
        AssertKind::Overflow(_, left, right) => {
            rewrite_receiver_places_in_operand(left, receiver);
            rewrite_receiver_places_in_operand(right, receiver);
        }
        AssertKind::OverflowNeg(operand)
        | AssertKind::DivisionByZero(operand)
        | AssertKind::RemainderByZero(operand)
        | AssertKind::InvalidEnumConstruction(operand) => {
            rewrite_receiver_places_in_operand(operand, receiver)
        }
        AssertKind::MisalignedPointerDereference { required, found } => {
            rewrite_receiver_places_in_operand(required, receiver);
            rewrite_receiver_places_in_operand(found, receiver);
        }
        AssertKind::NullPointerDereference => {}
    }
}

/// Apply [`strip_leading_deref_if_receiver`] to every place reachable
/// from an `Rvalue`.
fn rewrite_receiver_places_in_rvalue(rvalue: &mut Rvalue, receiver: LocalId) {
    match rvalue {
        Rvalue::Use(operand) | Rvalue::Repeat(operand, _) | Rvalue::UnaryOp(_, operand) => {
            rewrite_receiver_places_in_operand(operand, receiver)
        }
        Rvalue::Ref(_, place) | Rvalue::RawPtr(_, place) | Rvalue::Discriminant(place) => {
            strip_leading_deref_if_receiver(place, receiver)
        }
        Rvalue::Cast(_, operand, _) => rewrite_receiver_places_in_operand(operand, receiver),
        Rvalue::BinaryOp(_, left, right) => {
            rewrite_receiver_places_in_operand(left, receiver);
            rewrite_receiver_places_in_operand(right, receiver);
        }
        Rvalue::Aggregate(_, operands) => {
            for operand in operands.iter_mut() {
                rewrite_receiver_places_in_operand(operand, receiver);
            }
        }
    }
}

/// Apply [`strip_leading_deref_if_receiver`] to the place inside an
/// `Operand`, if any.
fn rewrite_receiver_places_in_operand(operand: &mut Operand, receiver: LocalId) {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            strip_leading_deref_if_receiver(place, receiver)
        }
        Operand::Constant(_) => {}
    }
}

/// Phase-5 driver: materialise every Function body, run
/// vtable-indirect rewrite, run joint validation. Per spec.md section
/// 25.7 pass ordering.
pub fn run_phase5_body_passes(cx: &mut BridgeCtxt<'_>) -> Result<(), BridgeError> {
    // Per audit-finding regseq-driver-43-1, materialise every
    // Function instance unconditionally; the previous
    // body.is_some() short-circuit violated the section-25.7
    // per-body pass-order rule.
    //
    // Per Codex review item 1: body translation can append new
    // Function instances (e.g., the drop-glue or closure-call
    // shim that `terminator.rs::lookup_or_create_direct_instance`
    // creates lazily at a call site). Those late-appended Function
    // instances MUST also flow through the per-body pass set in
    // the same Phase-5 run. The loop walks `instances` by index
    // and re-checks `instances.len()` at each iteration so that
    // any instance appended during a previous body's translation
    // is picked up before the loop ends.
    let mut idx: u32 = 0;
    loop {
        if (idx as usize) >= cx.model.instances.len() {
            break;
        }
        let instance = InstanceId::from_u32(idx);
        idx += 1;
        let is_function = cx.model.instances[instance].kind.as_function().is_some();
        if !is_function {
            continue;
        }
        let materialised = materialise_function_body_inner(cx, instance)?;
        let mut body = match materialised {
            Ok(body) => body,
            Err(_reason) => {
                // Flavor legitimately has no own MIR (Virtual,
                // VTable shims) or its source could not be resolved
                // (cross-crate synthetic items). Leave body=None
                // and skip body-content passes for this instance
                // per spec section 25.7.2 (Virtual) and audit-
                // finding regseq-driver-43-3 (graceful skip when
                // shim builders are pending).
                continue;
            }
        };
        rewrite_vtable_indirect(cx, &mut body)?;
        // Spec section 25.7.4: DiagLogPeel slots in after the
        // vtable-indirect rewrite and before joint validation so
        // B-4 / B-6 see the peeled flat-arg shape. The pass is a
        // no-op for Bodies without Log-intrinsic Calls.
        crate::bridge::diag_log::diag_log_peel_pass(cx, &mut body)?;
        // Normalise the body to DFS-RPO as the strictly-last
        // CFG-mutating step, after the vtable-indirect and
        // diag-log-peel passes (the latter can rewrite a `Call` into a
        // `Goto`, changing the CFG). rustc's drop-shim pipeline omits
        // `ReorderBasicBlocks`, so a multi-block drop-glue shim arrives
        // non-RPO; this satisfies the `BRpo`/`BReachable` pre-checks in
        // `joint_validation_walk` and is idempotent on already-RPO
        // bodies (`optimized_mir` output).
        body.reorder_blocks_to_rpo();
        joint_validation_walk(&cx.model, instance, &body)?;
        let function = cx.model.instances[instance]
            .kind
            .as_function_mut()
            .expect("instance is a function");
        function.body = Some(body);
    }

    Ok(())
}

/// Joint validation walk over a materialised Body per spec section
/// 25.6. Drives the available model-side checks plus the inline
/// closure invariants captured by audit-finding regseq-driver-43-6.
/// The full model-side surface (validate_body_ty_closure,
/// validate_body_statement_invariants,
/// validate_body_terminator_invariants) lands with regseq-model-84;
/// until then, the TyId closure walk and Direct-callee bounds check
/// are inlined here so the driver still surfaces the section 25.9
/// failure cases for these invariants.
fn joint_validation_walk(
    model: &regseq_model::RegseqModel,
    instance: InstanceId,
    body: &Body,
) -> Result<(), BridgeError> {
    // DFS-RPO body prechecks per spec section 25.6.2.
    let mut visitor = JointValidationVisitor::new(model);
    visitor.visit_body(body);
    if !visitor.body_errors.is_empty() {
        return Err(BridgeError::Phase5(format!(
            "joint validation body pre-check failed for instance {}: {:?}",
            instance.as_u32(),
            visitor.body_errors
        )));
    }
    // TyId closure walk per spec section 25.6.3 invariant 1; pending
    // model-side `validate_body_ty_closure` from regseq-model-84.
    if let Some(missing_ty) = visitor.missing_ty {
        return Err(BridgeError::Phase5(format!(
            "joint validation TyId closure failed for instance {}: ty {} not in model.types (len {})",
            instance.as_u32(),
            missing_ty.as_u32(),
            model.types.len()
        )));
    }
    // Direct-callee bounds check per spec section 25.6.3 invariant
    // 9; pending model-side `validate_body_direct_callees` (B-9)
    // exposure from regseq-model-84.
    if let Some(missing_callee) = visitor.missing_direct_callee {
        return Err(BridgeError::Phase5(format!(
            "joint validation direct-callee closure failed for instance {}: callee {} not in model.instances (len {})",
            instance.as_u32(),
            missing_callee.as_u32(),
            model.instances.len()
        )));
    }
    // Inline B-4 Call arity check per spec section 23.6 B-4;
    // pending model-side `validate_body_terminator_invariants` from
    // regseq-model-84. Spec section 26.6's diagnostic-logging
    // arity exception is regseq-driver-44 scope and is not enforced
    // here.
    if let Some((callee, expected, actual)) = visitor.call_arity_mismatch {
        return Err(BridgeError::Phase5(format!(
            "joint validation terminator B-4 (Call arity) failed for instance {}: callee {} expects {} args, got {}",
            instance.as_u32(),
            callee.as_u32(),
            expected,
            actual
        )));
    }
    // Inline B-5 Direct-callee callable check per spec section 23.6
    // B-5: `CallCallee::Direct(InstanceId)` must resolve to
    // `InstanceKind::Function(_)`. Static/Const/Struct/Enum/Union
    // instances are not callable. Coordinated with regseq-model-84.
    if let Some(non_function) = visitor.non_function_direct_callee {
        return Err(BridgeError::Phase5(format!(
            "joint validation terminator B-5 (Direct callee callable) failed for instance {}: callee {} is not InstanceKind::Function",
            instance.as_u32(),
            non_function.as_u32()
        )));
    }
    // Place walk per spec section 19.7 via regseq-model-83's
    // validator (covers B-0a + invariant 1 type-coherence walk).
    validate_body_places(model, body).map_err(|error| {
        BridgeError::Phase5(format!(
            "joint validation place walk failed for instance {}: {error:?}",
            instance.as_u32()
        ))
    })
    // Statement invariants B-1..B-5 (spec section 22.5) and the
    // remaining terminator invariants B-1, B-2, B-3, B-6..B-9 (spec
    // section 23.6) require `place_ty` / `operand_ty` / `rvalue_ty`
    // recovery helpers that live model-side. They are coordinated
    // with regseq-model-84 and wire in when those
    // `RegseqModel::validate_body_*` methods become pub. The
    // tractable invariants -- TyId closure, Direct-callee closure
    // (B-9), Direct-callee callable (B-5), Direct Call arity (B-4),
    // Place walk -- are inlined above.
}

struct JointValidationVisitor<'m> {
    model: &'m regseq_model::RegseqModel,
    body_errors: Vec<BodyValidationError>,
    missing_ty: Option<TyId>,
    missing_direct_callee: Option<InstanceId>,
    /// Captured `(callee_id, expected_arity, actual_arity)` for the
    /// first Call terminator whose arg count disagrees with its
    /// callee's `FnSig.inputs.len()` per spec section 23.6 B-4. A
    /// `c_variadic` callee admits any `actual >= expected` (the
    /// variadic tail), mirroring regseq-model-97; a non-variadic
    /// callee requires exact equality.
    call_arity_mismatch: Option<(InstanceId, usize, usize)>,
    /// First Direct callee whose `InstanceKind` is not
    /// `InstanceKind::Function(_)` per spec section 23.6 B-5.
    non_function_direct_callee: Option<InstanceId>,
}

impl<'m> JointValidationVisitor<'m> {
    fn new(model: &'m regseq_model::RegseqModel) -> Self {
        Self {
            model,
            body_errors: Vec::new(),
            missing_ty: None,
            missing_direct_callee: None,
            call_arity_mismatch: None,
            non_function_direct_callee: None,
        }
    }
}

impl<'m> Visitor for JointValidationVisitor<'m> {
    fn emit_body_error(&mut self, error: BodyValidationError) {
        self.body_errors.push(error);
    }

    fn visit_ty(&mut self, ty: TyId) {
        if self.missing_ty.is_none() && ty.index() >= self.model.types.len() {
            self.missing_ty = Some(ty);
        }
        self.super_ty(ty);
    }

    fn visit_call_callee(&mut self, callee: &CallCallee) {
        if let CallCallee::Direct(instance_id) = callee {
            if instance_id.index() >= self.model.instances.len() {
                if self.missing_direct_callee.is_none() {
                    self.missing_direct_callee = Some(*instance_id);
                }
            } else if self.non_function_direct_callee.is_none()
                && !matches!(
                    self.model.instances[*instance_id].kind,
                    InstanceKind::Function(_)
                )
            {
                self.non_function_direct_callee = Some(*instance_id);
            }
        }
        self.super_call_callee(callee);
    }

    fn visit_terminator(&mut self, term: &Terminator) {
        // Inline B-4 Call arity check for Direct callees. The
        // Indirect-callee arity check requires operand-type recovery
        // (spec 23.6 B-4 Indirect bullet); that path is coordinated
        // with regseq-model-84 and lands when the model-side B-4
        // validator becomes pub.
        //
        // Diagnostic-logging arity exception per spec.md sections
        // 23.6.2 B-4 / B-6 + 26.6 (I-DiagLog-CallArity): when the
        // callee resolves to an Instance whose Item carries
        // `AttrData::Intrinsic(IntrinsicKind::Log(_))`, the
        // post-DiagLogPeel Call shape is `[fmt_literal, expr_1,
        // ..., expr_N]` per spec.md section 26.6 and does NOT match
        // the declared callee signature `fn(&'static str,
        // &[LogArg<'_>])` (2 inputs). The peeled shape is the
        // load-bearing invariant; skip the standard arity check for
        // Log-intrinsic callees so the B-4 exception holds.
        if self.call_arity_mismatch.is_none() {
            if let TerminatorKind::Call {
                callee: CallCallee::Direct(callee_id),
                args,
                ..
            } = &term.kind
            {
                if callee_id.index() < self.model.instances.len()
                    && !self.callee_is_log_intrinsic(*callee_id)
                {
                    if let Some(sig) = self.model.instance_fn_sig(*callee_id) {
                        // B-4 arity. A `c_variadic` callee legitimately
                        // takes a variadic tail beyond its fixed inputs, so
                        // the arity relaxes to `args.len() >= inputs.len()`
                        // (the fixed inputs must still be present
                        // positionally); a non-variadic callee keeps exact
                        // equality. This mirrors the model-side B-4
                        // relaxation (regseq-model-97) so the two validators
                        // agree (regseq-driver-86).
                        let arity_ok = if sig.c_variadic {
                            args.len() >= sig.inputs.len()
                        } else {
                            args.len() == sig.inputs.len()
                        };
                        if !arity_ok {
                            self.call_arity_mismatch =
                                Some((*callee_id, sig.inputs.len(), args.len()));
                        }
                    }
                }
            }
        }
        self.super_terminator(term);
    }
}

impl<'m> JointValidationVisitor<'m> {
    /// Returns `true` when the resolved Direct callee's Item carries
    /// `AttrData::Intrinsic(IntrinsicKind::Log(_))` per spec.md
    /// section 26.6 and so qualifies for the
    /// `I-DiagLog-CallArity` B-4 / B-6 exceptions at spec.md
    /// section 23.6.2.
    fn callee_is_log_intrinsic(&self, callee_id: InstanceId) -> bool {
        if callee_id.index() >= self.model.instances.len() {
            return false;
        }
        let item = self.model.instances[callee_id].item;
        if item.index() >= self.model.domain_attrs.item_attrs.len() {
            return false;
        }
        self.model.domain_attrs.item_attrs[item]
            .iter()
            .any(|attr| matches!(attr, AttrData::Intrinsic(IntrinsicKind::Log(_))))
    }
}

fn local_def_id_for_item(
    tcx: TyCtxt<'_>,
    model: &regseq_model::RegseqModel,
    item: regseq_model::ItemId,
) -> Option<LocalDefId> {
    let wanted = model.item_def_paths[item];
    tcx.hir_body_owners()
        .find(|local_def_id| def_path_hash_matches(tcx, *local_def_id, wanted))
}

fn def_path_hash_matches(
    tcx: TyCtxt<'_>,
    local_def_id: LocalDefId,
    wanted: regseq_model::DefPathHash,
) -> bool {
    let actual = tcx.def_path_hash(local_def_id.to_def_id());
    actual.stable_crate_id().as_u64() == wanted.stable_crate_id.0
        && actual.local_hash().as_u64() == wanted.local_hash
}

/// Reconstruct a rustc `GenericArgsRef<'tcx>` from a model
/// `Instance.generic_args` slice per audit-finding regseq-driver-43-2.
/// Walks `GenericArgs::for_item(tcx, def_id, mk_kind)`, substituting:
/// - Lifetimes -> `tcx.lifetimes.re_erased`.
/// - Types -> resolved via `BridgeCtxt.ty_forward_cache` from
///   `GenericArg::Type(TyId)`.
/// - Consts -> coordinated with regseq-driver-37's
///   `translate_generic_args`. Both currently filter Const params; on
///   the rebuild side we explicitly reject so the spec gap is
///   surfaced rather than silently expanded. Per audit "MUST either
///   preserve or explicitly reject."
fn translate_model_generic_args<'tcx>(
    cx: &BridgeCtxt<'tcx>,
    def_id: rustc_span::def_id::DefId,
    model_args: &[GenericArg],
) -> Result<rustc_middle::ty::GenericArgsRef<'tcx>, BridgeError> {
    // Walk the model's type slots in order; rustc `for_item` consumes
    // them via the closure.
    let mut type_cursor = model_args
        .iter()
        .filter_map(|arg| match arg {
            GenericArg::Type(ty_id) => Some(*ty_id),
            GenericArg::Const(_) => None,
        })
        .collect::<Vec<_>>()
        .into_iter();

    let tcx = cx.input.tcx;
    let mut error: Option<BridgeError> = None;
    let args = GenericArgs::for_item(tcx, def_id, |param, _| {
        if error.is_some() {
            return tcx.lifetimes.re_erased.into();
        }
        match param.kind {
            rustc_middle::ty::GenericParamDefKind::Lifetime => tcx.lifetimes.re_erased.into(),
            rustc_middle::ty::GenericParamDefKind::Type { .. } => {
                let Some(ty_id) = type_cursor.next() else {
                    error = Some(BridgeError::Phase5(format!(
                        "model Instance generic args exhausted before rustc Type param `{}`",
                        param.name
                    )));
                    return tcx.types.unit.into();
                };
                let Some(rustc_ty) = cx.ty_forward_cache.get(&ty_id).copied() else {
                    error = Some(BridgeError::Phase5(format!(
                        "model TyId {} for generic arg has no rustc Ty in ty_forward_cache",
                        ty_id.as_u32()
                    )));
                    return tcx.types.unit.into();
                };
                rustc_ty.into()
            }
            rustc_middle::ty::GenericParamDefKind::Const { .. } => {
                // Spec gap coordinated with regseq-driver-37: both
                // forward (translate_generic_args) and backward
                // (here) currently drop Const params. Per audit
                // regseq-driver-43-2, reject explicitly so any const
                // generic forces a coordinated fix in driver-37 as
                // well, rather than silently expanding the gap.
                error = Some(BridgeError::Phase5(format!(
                    "const generic param `{}` is not supported in body materialisation",
                    param.name
                )));
                tcx.types.unit.into()
            }
        }
    });
    if let Some(err) = error {
        return Err(err);
    }
    Ok(args)
}

fn translate_local_mutability(mutability: rustc_middle::mir::Mutability) -> Mutability {
    match mutability {
        rustc_middle::mir::Mutability::Not => Mutability::Not,
        rustc_middle::mir::Mutability::Mut => Mutability::Mut,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regseq_model::{
        BasicBlock, BlockId, CallCallee, FieldIdx, FnAbi, FnSig, FunctionInstanceData, IndexVec,
        Instance, Item, ItemKind, LocalDecl, LocalId, Operand, Place, PlaceElem, RegseqModel,
        Rvalue, Safety, Span, StableCrateId, Statement, StatementKind, Terminator, TerminatorKind,
        Ty, UNIT_LAYOUT,
    };

    fn make_synthetic_unit(model: &mut RegseqModel) -> TyId {
        model.types.intern(Ty::Tuple {
            fields: Vec::new(),
            layout: UNIT_LAYOUT,
        })
    }

    fn make_function_item(model: &mut RegseqModel, name: &str) -> regseq_model::ItemId {
        use regseq_model::{
            DefPath, DefPathData, DefPathHash, DefPathSegment, InlineHint, Linkage,
        };
        let name_id = model.strings.intern(name);
        let rendered = model.strings.intern(&format!("test::{name}"));
        let item = model.items.push(Item {
            def_path: DefPath {
                crate_id: model.primary_crate,
                segments: vec![DefPathSegment {
                    data: DefPathData::ValueNs(name_id),
                    disambiguator: 0,
                }],
                rendered,
            },
            kind: ItemKind::Function,
            span: Span::dummy(),
            linkage: Linkage::Default,
            link_section: None,
            inline_hint: InlineHint::None,
            is_unsafe: false,
            is_phantom_data: false,
            is_unsafe_cell: false,
            is_manually_drop: false,
            opaque_handle_eligible: false,
        });
        model.item_def_paths.push(DefPathHash {
            stable_crate_id: StableCrateId(1),
            local_hash: item.as_u32() as u64 + 100,
        });
        model.item_synthetic_kinds.push(SyntheticItemKind::None);
        model.domain_attrs.item_attrs.push(Vec::new());
        item
    }

    fn make_function_instance(model: &mut RegseqModel, item: regseq_model::ItemId) -> InstanceId {
        let unit = make_synthetic_unit(model);
        let fn_sig = model.sigs.intern(FnSig {
            inputs: Vec::new(),
            output: unit,
            abi: FnAbi::Rust,
            c_variadic: false,
            safety: Safety::Safe,
        });
        model.sig_c_abi_lowerings.push(None);
        model.instances.push(Instance {
            item,
            generic_args: Vec::new(),
            emission_name: model.items[item].def_path.rendered,
            span: Span::dummy(),
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig,
                body: None,
                flavor: FunctionInstanceFlavor::Item,
                virtual_idx: None,
            }),
        })
    }

    /// Build a callee function instance with `fixed_inputs` fixed `unit`
    /// parameters and the given `c_variadic` flag, so the B-4 arity check
    /// in `joint_validation_walk` can be exercised against a variadic vs.
    /// non-variadic callee (regseq-driver-86).
    fn make_callee_with_sig(
        model: &mut RegseqModel,
        name: &str,
        fixed_inputs: usize,
        c_variadic: bool,
    ) -> InstanceId {
        let item = make_function_item(model, name);
        let unit = make_synthetic_unit(model);
        let fn_sig = model.sigs.intern(FnSig {
            inputs: vec![unit; fixed_inputs],
            output: unit,
            abi: if c_variadic { FnAbi::C } else { FnAbi::Rust },
            c_variadic,
            safety: Safety::Safe,
        });
        model.sig_c_abi_lowerings.push(None);
        model.instances.push(Instance {
            item,
            generic_args: Vec::new(),
            emission_name: model.items[item].def_path.rendered,
            span: Span::dummy(),
            kind: InstanceKind::Function(FunctionInstanceData {
                fn_sig,
                body: None,
                flavor: FunctionInstanceFlavor::Item,
                virtual_idx: None,
            }),
        })
    }

    /// Replace `body`'s START_BLOCK terminator with a `Call` to `callee`
    /// carrying `arg_count` constant args, so the B-4 arity check sees a
    /// specific (callee-arity, call-arity) pair (regseq-driver-86).
    fn set_call_terminator(
        model: &mut RegseqModel,
        body: &mut Body,
        callee: InstanceId,
        arg_count: usize,
    ) {
        use regseq_model::BlockId;
        let zst = model.consts.intern(regseq_model::Const::ZST);
        let args: Vec<Operand> = (0..arg_count).map(|_| Operand::Constant(zst)).collect();
        body.basic_blocks[BlockId::START_BLOCK].terminator = Terminator {
            kind: TerminatorKind::Call {
                callee: CallCallee::Direct(callee),
                args: args.into_boxed_slice(),
                destination: Place::from(LocalId::RETURN_PLACE),
                target: Some(BlockId::START_BLOCK),
            },
            span: Span::dummy(),
        };
    }

    fn make_dummy_local(ty: TyId) -> LocalDecl {
        LocalDecl {
            ty,
            mutability: Mutability::Mut,
            span: Span::dummy(),
            debug_name: None,
        }
    }

    fn make_dummy_body(model: &mut RegseqModel) -> Body {
        let unit = make_synthetic_unit(model);
        let mut local_decls = IndexVec::new();
        local_decls.push(make_dummy_local(unit));
        let mut basic_blocks = IndexVec::new();
        basic_blocks.push(BasicBlock {
            statements: Box::new([]),
            terminator: Terminator {
                kind: TerminatorKind::Return,
                span: Span::dummy(),
            },
        });
        Body {
            basic_blocks,
            local_decls,
            arg_count: 0,
            span: Span::dummy(),
        }
    }

    #[test]
    fn joint_validation_passes_for_well_formed_body() {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let item = make_function_item(&mut model, "ok_fn");
        let instance = make_function_instance(&mut model, item);
        let body = make_dummy_body(&mut model);

        joint_validation_walk(&model, instance, &body).expect("well-formed body validates");
    }

    #[test]
    fn joint_validation_fails_when_body_references_unknown_ty() {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let item = make_function_item(&mut model, "bad_ty_fn");
        let instance = make_function_instance(&mut model, item);
        let mut body = make_dummy_body(&mut model);
        // Force a TyId that exceeds model.types.len(): use a Field
        // projection whose carrier ty is out-of-range.
        let bogus_ty = TyId::from_u32(model.types.len() as u32 + 99);
        let zst = model.consts.intern(regseq_model::Const::ZST);
        let mut statements = body.basic_blocks[BlockId::START_BLOCK].statements.to_vec();
        statements.push(Statement {
            kind: StatementKind::Assign(
                Place {
                    local: LocalId::RETURN_PLACE,
                    projection: Box::new([PlaceElem::Field(FieldIdx::from_u32(0), bogus_ty)]),
                },
                Rvalue::Use(Operand::Constant(zst)),
            ),
            span: Span::dummy(),
        });
        body.basic_blocks[BlockId::START_BLOCK].statements = statements.into_boxed_slice();

        let err = joint_validation_walk(&model, instance, &body).unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("TyId closure"),
            "expected TyId closure failure, got `{message}`"
        );
    }

    #[test]
    fn joint_validation_fails_when_direct_callee_is_out_of_range() {
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let item = make_function_item(&mut model, "bad_callee_fn");
        let instance = make_function_instance(&mut model, item);
        let mut body = make_dummy_body(&mut model);
        let bogus_instance = InstanceId::from_u32(model.instances.len() as u32 + 99);
        body.basic_blocks[BlockId::START_BLOCK].terminator = Terminator {
            kind: TerminatorKind::Call {
                callee: CallCallee::Direct(bogus_instance),
                args: Box::new([]),
                destination: Place::from(LocalId::RETURN_PLACE),
                target: None,
            },
            span: Span::dummy(),
        };

        let err = joint_validation_walk(&model, instance, &body).unwrap_err();
        assert!(format!("{err}").contains("direct-callee closure"));
    }

    #[test]
    fn b4_admits_c_variadic_callee_with_extra_args() {
        // A `c_variadic` callee with 1 fixed input, called with 3 args
        // (1 fixed + 2 variadic-tail) must pass the bridge B-4 arity
        // check (regseq-driver-86, mirroring regseq-model-97).
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let owner_item = make_function_item(&mut model, "owner_fn");
        let owner = make_function_instance(&mut model, owner_item);
        let callee = make_callee_with_sig(&mut model, "variadic_callee", 1, true);
        let mut body = make_dummy_body(&mut model);
        set_call_terminator(&mut model, &mut body, callee, 3);

        joint_validation_walk(&model, owner, &body)
            .expect("variadic callee with extra args must pass B-4");
    }

    #[test]
    fn b4_rejects_non_variadic_arity_mismatch() {
        // A non-variadic callee with 1 fixed input, called with 2 args,
        // must still be rejected (the exception is variadic-only).
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let owner_item = make_function_item(&mut model, "owner_fn");
        let owner = make_function_instance(&mut model, owner_item);
        let callee = make_callee_with_sig(&mut model, "fixed_callee", 1, false);
        let mut body = make_dummy_body(&mut model);
        set_call_terminator(&mut model, &mut body, callee, 2);

        let err = joint_validation_walk(&model, owner, &body).unwrap_err();
        assert!(
            format!("{err}").contains("Call arity"),
            "expected B-4 arity rejection, got `{err}`"
        );
    }

    #[test]
    fn b4_rejects_c_variadic_callee_below_fixed_arity() {
        // A `c_variadic` callee with 2 fixed inputs, called with only 1
        // arg (fewer than its fixed params), must still be rejected: the
        // relaxation is `args.len() >= inputs.len()`, not unconditional.
        let mut model = RegseqModel::with_primary("test", StableCrateId(1));
        let owner_item = make_function_item(&mut model, "owner_fn");
        let owner = make_function_instance(&mut model, owner_item);
        let callee = make_callee_with_sig(&mut model, "variadic_callee2", 2, true);
        let mut body = make_dummy_body(&mut model);
        set_call_terminator(&mut model, &mut body, callee, 1);

        let err = joint_validation_walk(&model, owner, &body).unwrap_err();
        assert!(
            format!("{err}").contains("Call arity"),
            "expected B-4 arity rejection, got `{err}`"
        );
    }
}
