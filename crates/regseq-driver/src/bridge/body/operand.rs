//! MIR `Operand` bridge translation.

use regseq_model::{Const, Operand};
use rustc_middle::mir;
use rustc_middle::ty;

use crate::bridge::{const_::translate_const, synth, ty::translate_ty, BridgeCtxt, BridgeError};

use super::{place::translate_place, BodyCtxt};

/// Translate a rustc `mir::Operand` into a `regseq_model::Operand`.
/// Per spec.md section 20.3.
pub fn translate_operand<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx, '_>,
    operand: mir::Operand<'tcx>,
) -> Result<Operand, BridgeError> {
    match operand {
        mir::Operand::Copy(place) => Ok(Operand::Copy(translate_place(cx, body_cx, place)?)),
        mir::Operand::Move(place) => {
            synthesize_move_drop_glue(cx, body_cx, place)?;
            Ok(Operand::Move(translate_place(cx, body_cx, place)?))
        }
        mir::Operand::Constant(constant) => {
            Ok(Operand::Constant(translate_const(cx, constant.const_)?))
        }
        mir::Operand::RuntimeChecks(_) => lower_runtime_check_operand(cx, body_cx, operand),
    }
}

/// Lower the RuntimeChecks operand surface per spec.md section 20.3.1.
pub fn lower_runtime_check_operand<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    _body_cx: &mut BodyCtxt<'tcx, '_>,
    check: mir::Operand<'tcx>,
) -> Result<Operand, BridgeError> {
    let mir::Operand::RuntimeChecks(checks) = check else {
        return translate_operand(cx, &mut BodyCtxt::empty(), check);
    };
    let value = checks.value(cx.input.tcx.sess);
    Ok(Operand::Constant(
        cx.model.consts.intern(Const::Bool(value)),
    ))
}

fn synthesize_move_drop_glue<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &BodyCtxt<'tcx, '_>,
    place: mir::Place<'tcx>,
) -> Result<(), BridgeError> {
    let Some(local_decls) = body_cx.local_decls() else {
        return Ok(());
    };
    let move_ty = place.ty(local_decls, cx.input.tcx).ty;
    let typing_env = ty::TypingEnv::fully_monomorphized();
    if cx
        .input
        .tcx
        .type_is_copy_modulo_regions(typing_env, move_ty)
    {
        return Ok(());
    }
    let model_ty = translate_ty(cx, move_ty)?;
    synth::synthesise_drop_glue(cx, model_ty)?;
    Ok(())
}
