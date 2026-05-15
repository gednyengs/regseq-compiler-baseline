//! Function-signature translation and C ABI lowering.

use std::collections::HashSet;

use regseq_model::{
    CAbiLowering, CAbiShape, FieldLayout, FnAbi, FnSig, FnSigId, IntKind, ItemId, ItemKind,
    LayoutKind, Safety, SubstReason, Ty, TyId, UintKind,
};
use rustc_abi::{ExternAbi, Integer, IntegerType};
use rustc_hir::definitions::DefPathHash as RustcDefPathHash;
use rustc_middle::ty::{self, TyCtxt};
use rustc_span::def_id::DefId;

use super::{
    ty::{erase_regions_or_reject, translate_ty},
    BridgeCtxt, BridgeError,
};

/// Translate a rustc `PolyFnSig` into a `regseq_model::FnSigId`. Per
/// spec.md section 14.3 schema. Region erasure runs first via
/// `bridge::ty::erase_regions_or_reject`.
pub fn translate_fn_sig<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    sig: rustc_middle::ty::PolyFnSig<'tcx>,
) -> Result<FnSigId, BridgeError> {
    let sig = sig.skip_binder();
    let tcx = cx.input.tcx;
    let inputs = sig
        .inputs()
        .iter()
        .map(|input| erase_regions_or_reject(tcx, *input).and_then(|input| translate_ty(cx, input)))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some((input_idx, _)) = inputs
        .iter()
        .enumerate()
        .find(|(_, input)| matches!(cx.model.types.resolve(**input), Ty::Never))
    {
        return Err(BridgeError::UnsupportedNeverInput { input_idx });
    }
    let output =
        erase_regions_or_reject(tcx, sig.output()).and_then(|output| translate_ty(cx, output))?;
    let abi = translate_abi(sig.abi)?;
    let sig = FnSig {
        inputs,
        output,
        abi,
        c_variadic: sig.c_variadic,
        safety: translate_safety(sig.safety),
    };
    let lowering = if abi == FnAbi::C {
        Some(CAbiChecker::new(cx).sig_lowering_for_sig(&sig)?)
    } else {
        None
    };
    let sig_id = cx.model.sigs.intern(sig);
    ensure_cabi_lowering_slots(cx);
    cx.model.sig_c_abi_lowerings[sig_id] = lowering;
    Ok(sig_id)
}

/// Compute the `CAbiLowering` for a translated `FnSig` per spec.md
/// section 14.5. Stores the per-position outcomes of the
/// `CAbiPositionAdmissible` and `CFieldAdmissible` predicates.
pub fn compute_cabi_lowering(cx: &mut BridgeCtxt<'_>, sig_id: FnSigId) -> CAbiLowering {
    let lowering = CAbiChecker::new(cx)
        .sig_lowering(sig_id)
        .expect("translated FnSig must be C ABI admissible");
    ensure_cabi_lowering_slots(cx);
    if cx.model.sigs.resolve(sig_id).abi == FnAbi::C {
        cx.model.sig_c_abi_lowerings[sig_id] = Some(lowering.clone());
    }
    lowering
}

fn ensure_cabi_lowering_slots(cx: &mut BridgeCtxt<'_>) {
    while cx.model.sig_c_abi_lowerings.len() < cx.model.sigs.len() {
        cx.model.sig_c_abi_lowerings.push(None);
    }
}

fn translate_abi(abi: ExternAbi) -> Result<FnAbi, BridgeError> {
    match abi {
        ExternAbi::Rust => Ok(FnAbi::Rust),
        ExternAbi::C { unwind: false } => Ok(FnAbi::C),
        _ => Err(BridgeError::UnsupportedAbi {
            abi_repr: format!("{abi:?}"),
        }),
    }
}

fn translate_safety(safety: rustc_hir::Safety) -> Safety {
    match safety {
        rustc_hir::Safety::Safe => Safety::Safe,
        rustc_hir::Safety::Unsafe => Safety::Unsafe,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum PositionKind {
    Input(usize),
    Output,
}

impl PositionKind {
    fn to_public(self) -> crate::bridge::CAbiTypePosition {
        match self {
            PositionKind::Input(idx) => crate::bridge::CAbiTypePosition::Input(idx),
            PositionKind::Output => crate::bridge::CAbiTypePosition::Output,
        }
    }
}

struct CAbiChecker<'a, 'tcx> {
    cx: &'a mut BridgeCtxt<'tcx>,
    visiting_position: HashSet<(TyId, PositionKind)>,
    visiting_field: HashSet<TyId>,
    visiting_sig: HashSet<FnSigId>,
}

impl<'a, 'tcx> CAbiChecker<'a, 'tcx> {
    fn new(cx: &'a mut BridgeCtxt<'tcx>) -> Self {
        Self {
            cx,
            visiting_position: HashSet::new(),
            visiting_field: HashSet::new(),
            visiting_sig: HashSet::new(),
        }
    }

    fn sig_lowering(&mut self, sig_id: FnSigId) -> Result<CAbiLowering, BridgeError> {
        let sig = self.cx.model.sigs.resolve(sig_id).clone();
        self.sig_lowering_for_sig(&sig)
    }

    fn sig_lowering_for_sig(&mut self, sig: &FnSig) -> Result<CAbiLowering, BridgeError> {
        Ok(CAbiLowering {
            inputs: sig
                .inputs
                .iter()
                .enumerate()
                .map(|(idx, input)| self.position_shape(*input, PositionKind::Input(idx)))
                .collect::<Result<Vec<_>, _>>()?,
            output: self.position_shape(sig.output, PositionKind::Output)?,
        })
    }

    fn position_shape(
        &mut self,
        ty_id: TyId,
        position: PositionKind,
    ) -> Result<CAbiShape, BridgeError> {
        if !self.visiting_position.insert((ty_id, position)) {
            return Ok(CAbiShape::Direct);
        }
        let shape = self.position_shape_inner(ty_id, position);
        self.visiting_position.remove(&(ty_id, position));
        shape
    }

    fn position_shape_inner(
        &mut self,
        ty_id: TyId,
        position: PositionKind,
    ) -> Result<CAbiShape, BridgeError> {
        Ok(match self.cx.model.types.resolve(ty_id).clone() {
            Ty::Bool | Ty::Int(_) | Ty::Uint(_) | Ty::Float(_) => CAbiShape::Direct,
            Ty::Never if position == PositionKind::Output => CAbiShape::Direct,
            Ty::Tuple { fields, .. } if fields.is_empty() && position == PositionKind::Output => {
                CAbiShape::Direct
            }
            Ty::Ptr(pointee, _) | Ty::Ref(pointee, _) => {
                if self.is_c_boundary_bad_pointee(pointee) {
                    return self.unsupported(ty_id, position.to_public());
                }
                CAbiShape::Direct
            }
            Ty::FnPtr(sig_id) => {
                self.check_nested_sig(sig_id, position.to_public())?;
                CAbiShape::Direct
            }
            Ty::Struct { item, layout, .. } if self.is_transparent(item) => {
                self.transparent_shape(&layout_field_tys(&layout), position)?
            }
            Ty::Enum { item, .. } if self.is_fieldless_repr_int_enum(item) => {
                self.tag_scalar_shape(item).unwrap_or(CAbiShape::Direct)
            }
            Ty::Struct { item, layout, .. } | Ty::Union { item, layout, .. }
                if self.is_repr_c(item) =>
            {
                for field in layout_field_tys(&layout) {
                    self.field_admissible(field)?;
                }
                CAbiShape::Direct
            }
            Ty::Enum { item, layout, .. } if self.is_repr_c(item) => {
                for field in layout_field_tys(&layout) {
                    self.field_admissible(field)?;
                }
                CAbiShape::Direct
            }
            _ => return self.unsupported(ty_id, position.to_public()),
        })
    }

    fn field_admissible(&mut self, ty_id: TyId) -> Result<(), BridgeError> {
        if !self.visiting_field.insert(ty_id) {
            return Ok(());
        }
        let admissible = match self.cx.model.types.resolve(ty_id).clone() {
            Ty::Bool | Ty::Int(_) | Ty::Uint(_) | Ty::Float(_) => Ok(()),
            Ty::Ptr(pointee, _) | Ty::Ref(pointee, _)
                if !self.is_c_field_bad_pointee(pointee) =>
            {
                Ok(())
            }
            Ty::FnPtr(sig_id) => {
                // FnPtr appearing as a field: the outer "FnPtr is
                // not C ABI" diagnostic carries Field position; the
                // nested sig's own inputs/outputs are surfaced with
                // their own positions if they themselves are bad.
                self.check_nested_sig(sig_id, crate::bridge::CAbiTypePosition::Field)
            }
            Ty::Array(elem, _) => self.field_admissible(elem),
            Ty::Tuple { fields, .. } if fields.is_empty() => Ok(()),
            Ty::Struct { item, layout, .. } if self.cx.model.items[item].is_phantom_data => {
                if layout.size == 0 && layout.align == 1 {
                    Ok(())
                } else {
                    Err(self.unsupported_error(ty_id, crate::bridge::CAbiTypePosition::Field))
                }
            }
            Ty::Struct { item, layout, .. } if self.is_transparent(item) => {
                if let Some(field) = layout_field_tys(&layout)
                    .into_iter()
                    .find(|field| !self.is_zst(*field))
                {
                    self.field_admissible(field)
                } else {
                    Ok(())
                }
            }
            Ty::Struct { item, layout, .. } | Ty::Union { item, layout, .. }
                if self.is_repr_c(item) =>
            {
                for field in layout_field_tys(&layout) {
                    self.field_admissible(field)?;
                }
                Ok(())
            }
            Ty::Enum { item, layout, .. }
                if self.is_repr_c(item) || self.is_fieldless_repr_int_enum(item) =>
            {
                for field in layout_field_tys(&layout) {
                    self.field_admissible(field)?;
                }
                Ok(())
            }
            _ => Err(self.unsupported_error(ty_id, crate::bridge::CAbiTypePosition::Field)),
        };
        self.visiting_field.remove(&ty_id);
        admissible
    }

    fn check_nested_sig(
        &mut self,
        sig_id: FnSigId,
        caller_position: crate::bridge::CAbiTypePosition,
    ) -> Result<(), BridgeError> {
        if !self.visiting_sig.insert(sig_id) {
            return Ok(());
        }
        let sig = self.cx.model.sigs.resolve(sig_id).clone();
        if sig.abi != FnAbi::C || self.is_never(sig.output) {
            self.visiting_sig.remove(&sig_id);
            return Err(BridgeError::UnsupportedCAbiType {
                ty_repr: format!("FnPtr({})", sig_id.as_u32()),
                position: caller_position,
            });
        }
        let mut inputs = Vec::with_capacity(sig.inputs.len());
        for (idx, input) in sig.inputs.iter().copied().enumerate() {
            inputs.push(self.position_shape(input, PositionKind::Input(idx))?);
        }
        let output = self.position_shape(sig.output, PositionKind::Output)?;
        ensure_cabi_lowering_slots(self.cx);
        self.cx.model.sig_c_abi_lowerings[sig_id] = Some(CAbiLowering { inputs, output });
        self.visiting_sig.remove(&sig_id);
        Ok(())
    }

    fn transparent_shape(
        &mut self,
        fields: &[TyId],
        outer_position: PositionKind,
    ) -> Result<CAbiShape, BridgeError> {
        let inner = fields
            .iter()
            .copied()
            .find(|field| !self.is_zst(*field))
            .or_else(|| fields.first().copied());
        let Some(inner) = inner else {
            return Ok(CAbiShape::Direct);
        };
        Ok(match self.position_shape(inner, outer_position)? {
            CAbiShape::Direct => CAbiShape::Substituted {
                emit_as: inner,
                reason: SubstReason::TransparentChain,
            },
            CAbiShape::Substituted { emit_as, reason } => CAbiShape::Substituted {
                emit_as,
                reason: compose_transparent_reason(reason),
            },
        })
    }

    fn tag_scalar_shape(&mut self, item: ItemId) -> Option<CAbiShape> {
        let int = self.adt_def(item)?.repr().int?;
        let emit_as = self.intern_integer_type(int);
        Some(CAbiShape::Substituted {
            emit_as,
            reason: SubstReason::TagScalar,
        })
    }

    fn intern_integer_type(&mut self, int: IntegerType) -> TyId {
        let ty = match int {
            IntegerType::Pointer(true) => Ty::Int(IntKind::Isize),
            IntegerType::Pointer(false) => Ty::Uint(UintKind::Usize),
            IntegerType::Fixed(integer, true) => Ty::Int(match_integer(integer)),
            IntegerType::Fixed(integer, false) => Ty::Uint(match_uinteger(integer)),
        };
        self.cx.model.types.intern(ty)
    }

    fn is_transparent(&self, item: ItemId) -> bool {
        self.adt_def(item)
            .is_some_and(|adt| adt.repr().transparent())
    }

    fn is_repr_c(&self, item: ItemId) -> bool {
        self.adt_def(item).is_some_and(|adt| adt.repr().c())
    }

    fn is_fieldless_repr_int_enum(&self, item: ItemId) -> bool {
        self.adt_def(item).is_some_and(|adt| {
            adt.repr().int.is_some()
                && adt
                    .variants()
                    .iter()
                    .all(|variant| variant.fields.is_empty())
        })
    }

    fn adt_def(&self, item: ItemId) -> Option<ty::AdtDef<'tcx>> {
        match self.cx.model.items[item].kind {
            ItemKind::Struct | ItemKind::Enum | ItemKind::Union => {}
            _ => return None,
        }
        let wanted = self.cx.model.item_def_paths[item];
        for local_def_id in self.cx.input.tcx.hir_crate_items(()).definitions() {
            let def_id = local_def_id.to_def_id();
            if def_path_hash_eq(self.cx.input.tcx, def_id, wanted) {
                return Some(self.cx.input.tcx.adt_def(def_id));
            }
        }
        None
    }

    fn is_never(&self, ty_id: TyId) -> bool {
        matches!(self.cx.model.types.resolve(ty_id), Ty::Never)
    }

    fn is_c_boundary_bad_pointee(&self, ty_id: TyId) -> bool {
        matches!(
            self.cx.model.types.resolve(ty_id),
            Ty::Never | Ty::Str | Ty::Slice(_) | Ty::Dynamic { .. }
        )
    }

    /// Field-position pointee admissibility. Diverges from the
    /// boundary rule in spec section 14.5: pointer/reference fields
    /// admit any *sized* pointee, including `Never` (which is sized
    /// even though uninhabited). Only unsized pointees (`Str`,
    /// `Slice`, `Dynamic`) remain inadmissible at field position.
    fn is_c_field_bad_pointee(&self, ty_id: TyId) -> bool {
        matches!(
            self.cx.model.types.resolve(ty_id),
            Ty::Str | Ty::Slice(_) | Ty::Dynamic { .. }
        )
    }

    fn is_zst(&self, ty_id: TyId) -> bool {
        match self.cx.model.types.resolve(ty_id) {
            Ty::Tuple { fields, layout } => fields.is_empty() && layout.size == 0,
            Ty::Struct { layout, .. } | Ty::Enum { layout, .. } | Ty::Union { layout, .. } => {
                layout.size == 0
            }
            _ => false,
        }
    }

    fn unsupported<T>(
        &self,
        ty_id: TyId,
        position: crate::bridge::CAbiTypePosition,
    ) -> Result<T, BridgeError> {
        Err(self.unsupported_error(ty_id, position))
    }

    fn unsupported_error(
        &self,
        ty_id: TyId,
        position: crate::bridge::CAbiTypePosition,
    ) -> BridgeError {
        BridgeError::UnsupportedCAbiType {
            ty_repr: format!("{:?}", self.cx.model.types.resolve(ty_id)),
            position,
        }
    }
}

fn compose_transparent_reason(reason: SubstReason) -> SubstReason {
    match reason {
        SubstReason::TransparentChain => SubstReason::TransparentChain,
        SubstReason::TagScalar | SubstReason::TransparentChainAndTagScalar => {
            SubstReason::TransparentChainAndTagScalar
        }
    }
}

fn layout_field_tys(layout: &regseq_model::Layout) -> Vec<TyId> {
    match &layout.kind {
        LayoutKind::Struct { fields } | LayoutKind::Union { fields } => {
            fields.iter().map(field_ty).collect()
        }
        LayoutKind::Enum { shape } => match shape {
            regseq_model::EnumShape::Empty => Vec::new(),
            regseq_model::EnumShape::Single { variant, .. } => {
                variant.fields.iter().map(field_ty).collect()
            }
            regseq_model::EnumShape::Multiple { variants, .. } => variants
                .iter()
                .flat_map(|variant| variant.fields.iter().map(field_ty))
                .collect(),
        },
    }
}

fn field_ty(field: &FieldLayout) -> TyId {
    field.ty
}

fn def_path_hash_eq(tcx: TyCtxt<'_>, def_id: DefId, wanted: regseq_model::DefPathHash) -> bool {
    let actual: RustcDefPathHash = tcx.def_path_hash(def_id);
    actual.stable_crate_id().as_u64() == wanted.stable_crate_id.0
        && actual.local_hash().as_u64() == wanted.local_hash
}

fn match_integer(integer: Integer) -> IntKind {
    match integer {
        Integer::I8 => IntKind::I8,
        Integer::I16 => IntKind::I16,
        Integer::I32 => IntKind::I32,
        Integer::I64 => IntKind::I64,
        Integer::I128 => IntKind::I128,
    }
}

fn match_uinteger(integer: Integer) -> UintKind {
    match integer {
        Integer::I8 => UintKind::U8,
        Integer::I16 => UintKind::U16,
        Integer::I32 => UintKind::U32,
        Integer::I64 => UintKind::U64,
        Integer::I128 => UintKind::U128,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_map_accepts_rust_and_c() {
        assert_eq!(translate_abi(ExternAbi::Rust).unwrap(), FnAbi::Rust);
        assert_eq!(
            translate_abi(ExternAbi::C { unwind: false }).unwrap(),
            FnAbi::C
        );
    }

    #[test]
    fn abi_map_rejects_unadjusted_and_c_unwind() {
        assert!(matches!(
            translate_abi(ExternAbi::Unadjusted),
            Err(BridgeError::UnsupportedAbi { .. })
        ));
        assert!(matches!(
            translate_abi(ExternAbi::RustCall),
            Err(BridgeError::UnsupportedAbi { .. })
        ));
        assert!(matches!(
            translate_abi(ExternAbi::C { unwind: true }),
            Err(BridgeError::UnsupportedAbi { .. })
        ));
    }
}
