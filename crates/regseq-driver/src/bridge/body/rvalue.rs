//! MIR `Rvalue` bridge translation.

use regseq_model::{
    AggregateKind, BinOp, CastKind, FieldIdx, Mutability, PointerCoercion, Provenance, Rvalue,
    UnOp, VariantIdx,
};
use rustc_middle::mir;
use rustc_middle::ty;

use crate::bridge::{ty::translate_ty, BridgeCtxt, BridgeError, DslRejectionKind};

use super::{operand::translate_operand, place::translate_place, BodyCtxt};

/// Translate a rustc `mir::Rvalue` into a `regseq_model::Rvalue`. Per
/// spec.md section 21.6.
pub fn translate_rvalue<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    body_cx: &mut BodyCtxt<'tcx, '_>,
    rvalue: mir::Rvalue<'tcx>,
) -> Result<Rvalue, BridgeError> {
    let rvalue_ty = body_cx
        .local_decls()
        .map(|local_decls| rvalue.ty(local_decls, cx.input.tcx));

    match rvalue {
        mir::Rvalue::Use(operand) => Ok(Rvalue::Use(translate_operand(cx, body_cx, operand)?)),
        mir::Rvalue::Repeat(operand, count) => Ok(Rvalue::Repeat(
            translate_operand(cx, body_cx, operand)?,
            count
                .try_to_target_usize(cx.input.tcx)
                .ok_or_else(|| BridgeError::DslRejection {
                    kind: DslRejectionKind::Rvalue,
                    ty_repr: format!("Repeat count {count:?}"),
                })?,
        )),
        mir::Rvalue::Ref(_, mir::BorrowKind::Shared, place) => Ok(Rvalue::Ref(
            Provenance::SharedBorrow,
            translate_place(cx, body_cx, place)?,
        )),
        mir::Rvalue::Ref(_, mir::BorrowKind::Mut { .. }, place) => Ok(Rvalue::Ref(
            Provenance::UniqueBorrow,
            translate_place(cx, body_cx, place)?,
        )),
        mir::Rvalue::Ref(_, mir::BorrowKind::Fake(_), _) => rejected_rvalue("Fake borrow"),
        mir::Rvalue::RawPtr(kind, place) => Ok(Rvalue::RawPtr(
            translate_raw_ptr_kind(kind),
            translate_place(cx, body_cx, place)?,
        )),
        mir::Rvalue::Cast(kind, operand, ty) => {
            let cast_kind = translate_cast_kind(kind)?;
            let model_operand = reify_fn_def_operand(cx, &cast_kind, &operand)?
                .map_or_else(|| translate_operand(cx, body_cx, operand), Ok)?;
            Ok(Rvalue::Cast(
                cast_kind,
                model_operand,
                translate_ty(cx, ty)?,
            ))
        }
        mir::Rvalue::BinaryOp(op, operands) => Ok(Rvalue::BinaryOp(
            translate_bin_op(op)?,
            translate_operand(cx, body_cx, operands.0)?,
            translate_operand(cx, body_cx, operands.1)?,
        )),
        mir::Rvalue::UnaryOp(op, operand) => Ok(Rvalue::UnaryOp(
            translate_un_op(op),
            translate_operand(cx, body_cx, operand)?,
        )),
        mir::Rvalue::Discriminant(place) => {
            Ok(Rvalue::Discriminant(translate_place(cx, body_cx, place)?))
        }
        mir::Rvalue::Aggregate(kind, operands) => {
            let operands = operands
                .into_iter()
                .map(|operand| translate_operand(cx, body_cx, operand))
                .collect::<Result<Vec<_>, _>>()?;
            let Some(rvalue_ty) = rvalue_ty else {
                return rejected_rvalue("Aggregate without body local declarations");
            };
            Ok(Rvalue::Aggregate(
                translate_aggregate_kind(cx, *kind, rvalue_ty, operands.len())?,
                operands.into_boxed_slice(),
            ))
        }
        mir::Rvalue::ThreadLocalRef(_)
        | mir::Rvalue::CopyForDeref(_)
        | mir::Rvalue::ShallowInitBox(_, _)
        | mir::Rvalue::WrapUnsafeBinder(_, _) => rejected_rvalue("unsupported rvalue"),
    }
}

/// A `ReifyFnPointer` cast (`let f: fn(..) -> .. = some_fn;`) lowers in
/// rustc MIR to `Cast(PointerCoercion(ReifyFnPointer), Const::ZST, FnPtr)`:
/// the operand is the zero-sized `FnDef` constant. Lowered naively that
/// constant becomes a bare model `Const::ZST`, which discards the `FnDef`
/// identity (the model `Const::ZST` is a single canonical representative for
/// all zero-sized types) — leaving the backend's `reify_fn_pointer` unable to
/// name the target function. So at this site (where the rustc operand type is
/// still available) we recover the `FnDef` identity, materialise its Function
/// instance, and lower the operand to `Const::Reloc`, the same provenance shape
/// a function-pointer baked into a `const`/`static` already uses (see
/// [`crate::bridge::const_::instance_ref_for_function_provenance`]).
///
/// Returns `Ok(None)` for any cast that is not a `ReifyFnPointer` over a
/// `FnDef`-typed constant, so the caller falls back to the generic operand
/// translation.
fn reify_fn_def_operand<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    cast_kind: &CastKind,
    operand: &mir::Operand<'tcx>,
) -> Result<Option<regseq_model::Operand>, BridgeError> {
    if *cast_kind != CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer) {
        return Ok(None);
    }
    let mir::Operand::Constant(constant) = operand else {
        return Ok(None);
    };
    let ty::TyKind::FnDef(def_id, generic_args) = constant.const_.ty().kind() else {
        return Ok(None);
    };
    let instance = ty::Instance::try_resolve(
        cx.input.tcx,
        ty::TypingEnv::fully_monomorphized(),
        *def_id,
        generic_args,
    )
    .map_err(|err| {
        BridgeError::Phase4(format!(
            "ReifyFnPointer Instance::try_resolve failed for {def_id:?} with args {generic_args:?}: {err:?}"
        ))
    })?
    .ok_or_else(|| {
        BridgeError::Phase4(format!(
            "ReifyFnPointer could not resolve a monomorphic instance for {def_id:?} with args {generic_args:?}"
        ))
    })?;
    let reloc = crate::bridge::const_::instance_ref_for_function_provenance(cx, instance)?;
    let const_id = cx.model.consts.intern(regseq_model::Const::Reloc(reloc));
    Ok(Some(regseq_model::Operand::Constant(const_id)))
}

fn translate_raw_ptr_kind(kind: mir::RawPtrKind) -> Mutability {
    match kind {
        mir::RawPtrKind::Const | mir::RawPtrKind::FakeForPtrMetadata => Mutability::Not,
        mir::RawPtrKind::Mut => Mutability::Mut,
    }
}

fn translate_cast_kind(kind: mir::CastKind) -> Result<CastKind, BridgeError> {
    match kind {
        mir::CastKind::PointerExposeProvenance => Ok(CastKind::PointerExposeProvenance),
        mir::CastKind::PointerWithExposedProvenance => Ok(CastKind::PointerWithExposedProvenance),
        mir::CastKind::PointerCoercion(coercion, _) => Ok(CastKind::PointerCoercion(
            translate_pointer_coercion(coercion)?,
        )),
        mir::CastKind::IntToInt => Ok(CastKind::IntToInt),
        mir::CastKind::FloatToInt => Ok(CastKind::FloatToInt),
        mir::CastKind::FloatToFloat => Ok(CastKind::FloatToFloat),
        mir::CastKind::IntToFloat => Ok(CastKind::IntToFloat),
        mir::CastKind::PtrToPtr => Ok(CastKind::PtrToPtr),
        mir::CastKind::FnPtrToPtr => Ok(CastKind::FnPtrToPtr),
        mir::CastKind::Transmute => Ok(CastKind::Transmute),
        mir::CastKind::Subtype => Ok(CastKind::Subtype),
    }
}

fn translate_pointer_coercion(
    coercion: ty::adjustment::PointerCoercion,
) -> Result<PointerCoercion, BridgeError> {
    match coercion {
        ty::adjustment::PointerCoercion::ReifyFnPointer(_) => Ok(PointerCoercion::ReifyFnPointer),
        ty::adjustment::PointerCoercion::UnsafeFnPointer => Ok(PointerCoercion::UnsafeFnPointer),
        ty::adjustment::PointerCoercion::ClosureFnPointer(_) => {
            Ok(PointerCoercion::ClosureFnPointer)
        }
        ty::adjustment::PointerCoercion::Unsize => Ok(PointerCoercion::Unsize),
        ty::adjustment::PointerCoercion::MutToConstPointer
        | ty::adjustment::PointerCoercion::ArrayToPointer => rejected_rvalue("pointer coercion"),
    }
}

pub(crate) fn translate_bin_op(op: mir::BinOp) -> Result<BinOp, BridgeError> {
    Ok(match op {
        mir::BinOp::Add => BinOp::Add,
        mir::BinOp::Sub => BinOp::Sub,
        mir::BinOp::Mul => BinOp::Mul,
        mir::BinOp::Div => BinOp::Div,
        mir::BinOp::Rem => BinOp::Rem,
        mir::BinOp::AddUnchecked => BinOp::AddUnchecked,
        mir::BinOp::SubUnchecked => BinOp::SubUnchecked,
        mir::BinOp::MulUnchecked => BinOp::MulUnchecked,
        mir::BinOp::AddWithOverflow => BinOp::AddWithOverflow,
        mir::BinOp::SubWithOverflow => BinOp::SubWithOverflow,
        mir::BinOp::MulWithOverflow => BinOp::MulWithOverflow,
        mir::BinOp::BitXor => BinOp::BitXor,
        mir::BinOp::BitAnd => BinOp::BitAnd,
        mir::BinOp::BitOr => BinOp::BitOr,
        mir::BinOp::Shl => BinOp::Shl,
        mir::BinOp::ShlUnchecked => BinOp::ShlUnchecked,
        mir::BinOp::Shr => BinOp::Shr,
        mir::BinOp::ShrUnchecked => BinOp::ShrUnchecked,
        mir::BinOp::Eq => BinOp::Eq,
        mir::BinOp::Lt => BinOp::Lt,
        mir::BinOp::Le => BinOp::Le,
        mir::BinOp::Ne => BinOp::Ne,
        mir::BinOp::Ge => BinOp::Ge,
        mir::BinOp::Gt => BinOp::Gt,
        mir::BinOp::Cmp => BinOp::Cmp,
        mir::BinOp::Offset => BinOp::Offset,
    })
}

fn translate_un_op(op: mir::UnOp) -> UnOp {
    match op {
        mir::UnOp::Not => UnOp::Not,
        mir::UnOp::Neg => UnOp::Neg,
        mir::UnOp::PtrMetadata => UnOp::PtrMetadata,
    }
}

fn translate_aggregate_kind<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    kind: mir::AggregateKind<'tcx>,
    rvalue_ty: rustc_middle::ty::Ty<'tcx>,
    operand_count: usize,
) -> Result<AggregateKind, BridgeError> {
    match kind {
        mir::AggregateKind::Array(_) => Ok(AggregateKind::Array(translate_ty(cx, rvalue_ty)?)),
        mir::AggregateKind::Tuple => Ok(AggregateKind::Tuple(translate_ty(cx, rvalue_ty)?)),
        mir::AggregateKind::Adt(def_id, variant, _, _, active_field) => {
            let ty_id = translate_ty(cx, rvalue_ty)?;
            match cx.input.tcx.adt_def(def_id).adt_kind() {
                ty::AdtKind::Struct => Ok(AggregateKind::Struct(ty_id)),
                ty::AdtKind::Enum => Ok(AggregateKind::Enum(
                    ty_id,
                    VariantIdx::from_u32(variant.as_u32()),
                )),
                ty::AdtKind::Union => {
                    let Some(field) = active_field else {
                        return rejected_rvalue("union aggregate without active field");
                    };
                    Ok(AggregateKind::Union(
                        ty_id,
                        FieldIdx::from_u32(field.as_u32()),
                    ))
                }
            }
        }
        mir::AggregateKind::Closure(_, _) => {
            Ok(AggregateKind::Closure(translate_ty(cx, rvalue_ty)?))
        }
        mir::AggregateKind::RawPtr(pointee, mutability) => Ok(AggregateKind::RawPtr(
            crate::bridge::ty::translate_ptr_pointee(cx, pointee)?,
            translate_ast_mutability(mutability),
        )),
        mir::AggregateKind::Coroutine(_, _) | mir::AggregateKind::CoroutineClosure(_, _) => {
            rejected_rvalue("coroutine aggregate")
        }
    }
    .and_then(|kind| {
        if matches!(kind, AggregateKind::RawPtr(_, _)) && operand_count != 2 {
            rejected_rvalue("raw pointer aggregate operand count")
        } else {
            Ok(kind)
        }
    })
}

fn translate_ast_mutability(mutability: rustc_ast::Mutability) -> Mutability {
    match mutability {
        rustc_ast::Mutability::Not => Mutability::Not,
        rustc_ast::Mutability::Mut => Mutability::Mut,
    }
}

fn rejected_rvalue<T>(construct: impl Into<String>) -> Result<T, BridgeError> {
    Err(BridgeError::DslRejection {
        kind: DslRejectionKind::Rvalue,
        ty_repr: construct.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::{translate_bin_op, translate_cast_kind, translate_raw_ptr_kind, translate_un_op};
    use regseq_model::{BinOp, CastKind, Mutability, PointerCoercion, UnOp};
    use rustc_hir::Safety;
    use rustc_middle::mir;
    use rustc_middle::ty;

    #[test]
    fn translates_all_bin_op_variants() {
        let cases = [
            (mir::BinOp::Add, BinOp::Add),
            (mir::BinOp::Sub, BinOp::Sub),
            (mir::BinOp::Mul, BinOp::Mul),
            (mir::BinOp::Div, BinOp::Div),
            (mir::BinOp::Rem, BinOp::Rem),
            (mir::BinOp::AddUnchecked, BinOp::AddUnchecked),
            (mir::BinOp::SubUnchecked, BinOp::SubUnchecked),
            (mir::BinOp::MulUnchecked, BinOp::MulUnchecked),
            (mir::BinOp::AddWithOverflow, BinOp::AddWithOverflow),
            (mir::BinOp::SubWithOverflow, BinOp::SubWithOverflow),
            (mir::BinOp::MulWithOverflow, BinOp::MulWithOverflow),
            (mir::BinOp::BitXor, BinOp::BitXor),
            (mir::BinOp::BitAnd, BinOp::BitAnd),
            (mir::BinOp::BitOr, BinOp::BitOr),
            (mir::BinOp::Shl, BinOp::Shl),
            (mir::BinOp::ShlUnchecked, BinOp::ShlUnchecked),
            (mir::BinOp::Shr, BinOp::Shr),
            (mir::BinOp::ShrUnchecked, BinOp::ShrUnchecked),
            (mir::BinOp::Eq, BinOp::Eq),
            (mir::BinOp::Lt, BinOp::Lt),
            (mir::BinOp::Le, BinOp::Le),
            (mir::BinOp::Ne, BinOp::Ne),
            (mir::BinOp::Ge, BinOp::Ge),
            (mir::BinOp::Gt, BinOp::Gt),
            (mir::BinOp::Cmp, BinOp::Cmp),
            (mir::BinOp::Offset, BinOp::Offset),
        ];

        for (rustc, model) in cases {
            assert_eq!(translate_bin_op(rustc).unwrap(), model);
        }
    }

    #[test]
    fn translates_un_op_and_raw_pointer_sub_enums() {
        assert_eq!(translate_un_op(mir::UnOp::Not), UnOp::Not);
        assert_eq!(translate_un_op(mir::UnOp::Neg), UnOp::Neg);
        assert_eq!(translate_un_op(mir::UnOp::PtrMetadata), UnOp::PtrMetadata);

        assert_eq!(
            translate_raw_ptr_kind(mir::RawPtrKind::Const),
            Mutability::Not
        );
        assert_eq!(
            translate_raw_ptr_kind(mir::RawPtrKind::Mut),
            Mutability::Mut
        );
        assert_eq!(
            translate_raw_ptr_kind(mir::RawPtrKind::FakeForPtrMetadata),
            Mutability::Not
        );
    }

    #[test]
    fn translates_cast_kind_rows() {
        let cases = [
            (
                mir::CastKind::PointerExposeProvenance,
                CastKind::PointerExposeProvenance,
            ),
            (
                mir::CastKind::PointerWithExposedProvenance,
                CastKind::PointerWithExposedProvenance,
            ),
            (mir::CastKind::IntToInt, CastKind::IntToInt),
            (mir::CastKind::FloatToInt, CastKind::FloatToInt),
            (mir::CastKind::FloatToFloat, CastKind::FloatToFloat),
            (mir::CastKind::IntToFloat, CastKind::IntToFloat),
            (mir::CastKind::PtrToPtr, CastKind::PtrToPtr),
            (mir::CastKind::FnPtrToPtr, CastKind::FnPtrToPtr),
            (mir::CastKind::Transmute, CastKind::Transmute),
            (mir::CastKind::Subtype, CastKind::Subtype),
            (
                mir::CastKind::PointerCoercion(
                    ty::adjustment::PointerCoercion::ReifyFnPointer(Safety::Safe),
                    mir::CoercionSource::Implicit,
                ),
                CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer),
            ),
            (
                mir::CastKind::PointerCoercion(
                    ty::adjustment::PointerCoercion::UnsafeFnPointer,
                    mir::CoercionSource::Implicit,
                ),
                CastKind::PointerCoercion(PointerCoercion::UnsafeFnPointer),
            ),
            (
                mir::CastKind::PointerCoercion(
                    ty::adjustment::PointerCoercion::ClosureFnPointer(Safety::Safe),
                    mir::CoercionSource::Implicit,
                ),
                CastKind::PointerCoercion(PointerCoercion::ClosureFnPointer),
            ),
            (
                mir::CastKind::PointerCoercion(
                    ty::adjustment::PointerCoercion::Unsize,
                    mir::CoercionSource::Implicit,
                ),
                CastKind::PointerCoercion(PointerCoercion::Unsize),
            ),
        ];

        for (rustc, model) in cases {
            assert_eq!(translate_cast_kind(rustc).unwrap(), model);
        }

        assert!(translate_cast_kind(mir::CastKind::PointerCoercion(
            ty::adjustment::PointerCoercion::MutToConstPointer,
            mir::CoercionSource::Implicit,
        ))
        .is_err());
        assert!(translate_cast_kind(mir::CastKind::PointerCoercion(
            ty::adjustment::PointerCoercion::ArrayToPointer,
            mir::CoercionSource::Implicit,
        ))
        .is_err());
    }
}
