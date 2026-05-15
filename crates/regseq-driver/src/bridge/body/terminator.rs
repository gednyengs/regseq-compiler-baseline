//! MIR `Terminator` bridge translation.

use regseq_model::{
    AssertKind, BlockId, CallCallee, FnAbi, FnSig, FunctionInstanceData, FunctionInstanceFlavor,
    Instance, InstanceId, InstanceKind, ItemId, Mutability, Operand, Place, Rvalue, Safety, Span,
    Statement, StatementKind, SwitchTargets, Terminator, TerminatorKind, Ty, TyId, UNIT_LAYOUT,
};
use rustc_middle::mir;
use rustc_middle::ty;

use crate::bridge::{
    synth,
    ty::{intern_function_item_for_def_id, translate_ty},
    BridgeCtxt, BridgeError, DslRejectionKind,
};

use super::{
    operand::translate_operand, place::translate_place, rvalue::translate_bin_op,
    statement::translate_span, BodyCtxt,
};

/// Translate a rustc `mir::Terminator` into a
/// `regseq_model::Terminator`. Per spec.md section 23.7. Rustc
/// `Drop` collapses into `Call` per spec.md sections 23.4 and
/// 25.7.1. The `statements` parameter is the caller's statement
/// accumulator; the Drop-as-Call collapse appends a synthetic
/// `&raw mut place` Assign to this vector before constructing the
/// Call terminator (spec.md section 23.4 step 4).
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
        } => TerminatorKind::Call {
            callee: translate_call_callee(cx, body_cx, &func)?,
            args: args
                .iter()
                .map(|arg| translate_operand(cx, body_cx, arg.node.clone()))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice(),
            destination: translate_place(cx, body_cx, destination)?,
            target: target.map(block_id),
        },
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
            translate_drop_as_call(cx, body_cx, statements, place, target, span)?
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

fn translate_call_callee<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx, '_>,
    func: &mir::Operand<'tcx>,
) -> Result<CallCallee, BridgeError> {
    let func_ty = body_cx
        .local_decls()
        .map(|local_decls| func.ty(local_decls, cx.input.tcx));
    if let mir::Operand::Constant(constant) = func {
        if let ty::TyKind::FnDef(def_id, args) = constant.const_.ty().kind() {
            // Intern the callee's Function Item without going through
            // `translate_ty(Ty::FnDef(..))`, which would re-trigger the
            // substantive-HRTB rejection on a trait method's
            // `for<'a> fn(..)` binder even though Direct callee
            // resolution only needs the post-region-erasure identity
            // of the callee (spec.md section 23.7).
            let item = intern_function_item_for_def_id(cx, *def_id);
            let instance_id =
                resolve_function_instance_for_direct_callee(cx, item, *def_id, args)?;
            return Ok(CallCallee::Direct(instance_id));
        }
    }
    if matches!(func_ty.map(|ty| ty.kind()), Some(ty::TyKind::FnPtr(..))) {
        return Ok(CallCallee::Indirect(translate_operand(
            cx,
            body_cx,
            func.clone(),
        )?));
    }
    rejected_terminator(format!("unsupported call callee {func:?}"))
}

fn translate_drop_as_call<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx, '_>,
    statements: &mut Vec<Statement>,
    place: mir::Place<'tcx>,
    target: mir::BasicBlock,
    span: Span,
) -> Result<TerminatorKind, BridgeError> {
    let Some(local_decls) = body_cx.local_decls() else {
        return rejected_terminator("Drop terminator without body local declarations");
    };
    let place_ty = place.ty(local_decls, cx.input.tcx).ty;
    let model_place_ty = translate_ty(cx, place_ty)?;
    let glue_handle = if let Some(handle) = cx.drop_glue_synth_cache.get(&model_place_ty) {
        *handle
    } else {
        synth::synthesise_drop_glue(cx, model_place_ty)?
    };
    let model_place = translate_place(cx, body_cx, place)?;

    // Synthesise the `*mut PlaceTy` and unit-typed locals on the
    // model body so the Drop-as-Call prelude reflects the spec's
    // §23.4 step 4-5 shape. Callers that exercise
    // `translate_terminator` outside `materialise_function_body` must
    // provide a model body via `BodyCtxt::new_with_model_body`.
    let raw_mut_ty = cx
        .model
        .types
        .intern(Ty::Ptr(model_place_ty, Mutability::Mut));
    let unit_ty = cx.model.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    });
    let Some(model_body) = body_cx.model_body_mut() else {
        return Err(BridgeError::Phase5(
            "Drop-as-Call collapse requires a model body for synthetic-local allocation (spec.md section 25.7.1)".to_string(),
        ));
    };
    let raw_local = model_body.push_synthetic_local(raw_mut_ty);
    let return_local = model_body.push_synthetic_local(unit_ty);

    statements.push(Statement {
        kind: StatementKind::Assign(
            Place::from(raw_local),
            Rvalue::RawPtr(Mutability::Mut, model_place),
        ),
        span,
    });

    let callee_instance = resolve_function_instance_for_drop_glue(cx, glue_handle)?;
    Ok(TerminatorKind::Call {
        callee: CallCallee::Direct(callee_instance),
        args: Box::new([Operand::Move(Place::from(raw_local))]),
        destination: Place::from(return_local),
        target: Some(block_id(target)),
    })
}

fn resolve_function_instance_for_direct_callee<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    item: ItemId,
    def_id: rustc_span::def_id::DefId,
    args: &'tcx ty::List<ty::GenericArg<'tcx>>,
) -> Result<InstanceId, BridgeError> {
    if let Some(instance) = cx.function_instance_for_item.get(&item) {
        return Ok(*instance);
    }
    // Phase 5 may encounter a Function Item that the Phase-4
    // Function-Instance pass did not pre-create (for example, an
    // Item interned by `intern_function_item_for_def_id` during body
    // translation). Translate the rustc `fn_sig` directly from
    // `def_id` with the call-site generic arguments so the Instance
    // carries the spec-mandated arity for
    // `validate_body_terminator_invariants`.
    let unit_ty = cx.model.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    });
    let (inputs, output) =
        translate_fn_sig_from_def_id(cx, def_id, args).unwrap_or_else(|| (Vec::new(), unit_ty));
    let sig = FnSig {
        inputs,
        output,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    };
    let sig_id = cx.model.sigs.intern(sig);
    while cx.model.sig_c_abi_lowerings.len() < cx.model.sigs.len() {
        cx.model.sig_c_abi_lowerings.push(None);
    }
    let emission_name = cx.model.items[item].def_path.rendered;
    let instance_id = cx.model.instances.push(Instance {
        item,
        generic_args: Vec::new(),
        emission_name,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: sig_id,
            body: None,
            flavor: FunctionInstanceFlavor::Item,
            virtual_idx: None,
        }),
    });
    cx.function_instance_for_item.insert(item, instance_id);
    Ok(instance_id)
}

fn translate_fn_sig_from_def_id<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    def_id: rustc_span::def_id::DefId,
    args: &'tcx ty::List<ty::GenericArg<'tcx>>,
) -> Option<(Vec<TyId>, TyId)> {
    let tcx = cx.input.tcx;
    let poly_sig = tcx.fn_sig(def_id).instantiate(tcx, args);
    let sig = poly_sig.skip_binder();
    let mut inputs = Vec::with_capacity(sig.inputs().len());
    for input in sig.inputs() {
        let erased = tcx.erase_and_anonymize_regions(*input);
        inputs.push(translate_ty(cx, erased).ok()?);
    }
    let erased_output = tcx.erase_and_anonymize_regions(sig.output());
    let output = translate_ty(cx, erased_output).ok()?;
    Some((inputs, output))
}

fn resolve_function_instance_for_drop_glue(
    cx: &mut BridgeCtxt<'_>,
    handle: crate::bridge::ItemHandle,
) -> Result<InstanceId, BridgeError> {
    let item_id = cx.handles.resolve(handle).ok_or_else(|| {
        BridgeError::Phase4(format!(
            "drop-glue handle {} was not assigned an ItemId",
            handle.as_u32()
        ))
    })?;
    if let Some(instance) = cx.function_instance_for_item.get(&item_id) {
        return Ok(*instance);
    }
    let instance_id = push_drop_glue_function_instance(cx, item_id);
    cx.function_instance_for_item.insert(item_id, instance_id);
    Ok(instance_id)
}

fn push_drop_glue_function_instance(cx: &mut BridgeCtxt<'_>, item: ItemId) -> InstanceId {
    let unit_ty = cx.model.types.intern(Ty::Tuple {
        fields: Vec::new(),
        layout: UNIT_LAYOUT,
    });
    let drop_target_ty = drop_glue_target_ty(cx, item).unwrap_or(unit_ty);
    let ptr_ty = cx
        .model
        .types
        .intern(Ty::Ptr(drop_target_ty, Mutability::Mut));
    let sig = FnSig {
        inputs: vec![ptr_ty],
        output: unit_ty,
        abi: FnAbi::Rust,
        c_variadic: false,
        safety: Safety::Safe,
    };
    let sig_id = cx.model.sigs.intern(sig);
    while cx.model.sig_c_abi_lowerings.len() < cx.model.sigs.len() {
        cx.model.sig_c_abi_lowerings.push(None);
    }
    let emission_name = cx.model.items[item].def_path.rendered;
    cx.model.instances.push(Instance {
        item,
        generic_args: vec![regseq_model::GenericArg::Type(drop_target_ty)],
        emission_name,
        span: Span::dummy(),
        kind: InstanceKind::Function(FunctionInstanceData {
            fn_sig: sig_id,
            body: None,
            flavor: FunctionInstanceFlavor::DropGlue,
            virtual_idx: None,
        }),
    })
}

fn drop_glue_target_ty(cx: &BridgeCtxt<'_>, item: ItemId) -> Option<TyId> {
    cx.drop_glue_synth_cache
        .iter()
        .find_map(|(ty, handle)| {
            cx.handles
                .resolve(*handle)
                .filter(|resolved| *resolved == item)
                .map(|_| *ty)
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
