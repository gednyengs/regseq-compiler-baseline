//! MIR `Terminator` bridge translation.

use regseq_model::{
    AssertKind, BlockId, CallCallee, InstanceId, LocalId, Operand, Place, SwitchTargets,
    Terminator, TerminatorKind, Ty,
};
use rustc_middle::mir;
use rustc_middle::ty;

use crate::bridge::{
    diag_log, synth, ty::translate_ty, BridgeCtxt, BridgeError, DslRejectionKind, ItemHandle,
};

use super::{
    operand::translate_operand, place::translate_place, rvalue::translate_bin_op,
    statement::translate_span, BodyCtxt,
};

/// Translate a rustc `mir::Terminator` into a
/// `regseq_model::Terminator`. Per spec.md section 23.7. Rustc
/// `Drop` collapses into `Call` per spec.md sections 23.4 and
/// 25.7.1.
pub fn translate_terminator<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx>,
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
            translate_drop_as_call(cx, body_cx, place, target)?
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
    body_cx: &mut BodyCtxt<'tcx>,
    func: &mir::Operand<'tcx>,
) -> Result<CallCallee, BridgeError> {
    let func_ty = body_cx
        .local_decls()
        .map(|local_decls| func.ty(local_decls, cx.input.tcx));
    if let mir::Operand::Constant(constant) = func {
        if let ty::TyKind::FnDef(def_id, _) = constant.const_.ty().kind() {
            let model_ty = translate_ty(cx, constant.const_.ty())?;
            let Ty::FnDef(callee_item, _) = cx.model.types.resolve(model_ty).clone() else {
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
                *def_id,
            )?;
            return Ok(CallCallee::Direct(instance_id_for_def_path(cx, *def_id)));
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
    body_cx: &mut BodyCtxt<'tcx>,
    place: mir::Place<'tcx>,
    target: mir::BasicBlock,
) -> Result<TerminatorKind, BridgeError> {
    let Some(local_decls) = body_cx.local_decls() else {
        return rejected_terminator("Drop terminator without body local declarations");
    };
    let place_ty = place.ty(local_decls, cx.input.tcx).ty;
    let model_ty = translate_ty(cx, place_ty)?;
    let handle = if let Some(handle) = cx.drop_glue_synth_cache.get(&model_ty) {
        *handle
    } else {
        synth::synthesise_drop_glue(cx, model_ty)?
    };
    let place = translate_place(cx, body_cx, place)?;
    Ok(TerminatorKind::Call {
        callee: CallCallee::Direct(InstanceId::from_u32(handle.as_u32())),
        args: Box::new([Operand::Move(place)]),
        destination: Place::from(LocalId::RETURN_PLACE),
        target: Some(block_id(target)),
    })
}

fn translate_assert_kind<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx>,
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

fn instance_id_for_def_path(cx: &BridgeCtxt<'_>, def_id: rustc_span::def_id::DefId) -> InstanceId {
    let hash = cx.input.tcx.def_path_hash(def_id).local_hash().as_u64();
    InstanceId::from_u32(hash as u32)
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
