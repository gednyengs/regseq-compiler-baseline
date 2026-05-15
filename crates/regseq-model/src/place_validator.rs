//! Place validation for body content.

use crate::body::Body;
use crate::ids::{BlockId, LocalId, TyId};
use crate::layout::{EnumShape, FieldLayout, LayoutKind, VariantLayout};
use crate::model::RegseqModel;
use crate::operand::Operand;
use crate::place::{Place, PlaceElem};
use crate::rvalue::{AggregateKind, CastKind, PointerCoercion, Rvalue};
use crate::statement::{CopyNonOverlapping, NonDivergingIntrinsic, StatementKind};
use crate::terminator::{AssertKind, CallCallee, TerminatorKind};
use crate::ty::Ty;
use crate::ty_interner::NominalKind;

/// First-violation report for section 19.7 place validation.
///
/// `location` carries the body coordinates for the offending place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceValidationError {
    /// Body location of the invalid place.
    pub location: PlaceLocation,
    /// Specific validation rule that failed.
    pub kind: PlaceValidationErrorKind,
}

/// Body coordinates for a place-validation failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceLocation {
    /// Basic block containing the place.
    pub block: BlockId,
    /// Statement or terminator site containing the place.
    pub site: PlaceSite,
    /// Projection element index when the violation is tied to one.
    pub projection_step: Option<usize>,
}

/// Statement or terminator site carrying a place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaceSite {
    /// Statement index within the block.
    Statement(usize),
    /// Terminator at the end of the block.
    Terminator,
}

/// Specific first-failing section 19.7 place-validation rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaceValidationErrorKind {
    /// A dereference projection appeared after index 0.
    DerefNotFirst {
        /// Offending projection index.
        index: usize,
    },
    /// A downcast was not immediately followed by a field projection.
    DowncastNotFollowedByField {
        /// Offending projection index.
        index: usize,
    },
    /// The place head local does not resolve in the body local table.
    HeadLocalOutOfRange {
        /// Offending local.
        local: LocalId,
        /// Length of `Body.local_decls`.
        locals_len: usize,
    },
    /// A dereference projection was applied to a non-pointer type.
    DerefOnNonPointer {
        /// Predecessor type.
        ty: TyId,
    },
    /// A field projection was applied to a non-aggregate predecessor.
    FieldOnNonAggregate {
        /// Predecessor type.
        ty: TyId,
        /// Active enum variant, if a prior downcast selected one.
        active_variant: Option<u32>,
    },
    /// A field index is outside the resolved field list.
    FieldIdxOutOfRange {
        /// Offending field index.
        actual: u32,
        /// Exclusive upper bound for valid field indices.
        max: u32,
    },
    /// The field projection's carried type disagrees with layout.
    FieldTypeMismatch {
        /// Type resolved from the field list.
        expected: TyId,
        /// Type carried by the projection.
        actual: TyId,
    },
    /// An index-style projection was applied to a non-indexable type.
    IndexOnNonIndexable {
        /// Predecessor type.
        ty: TyId,
    },
    /// A from-end constant index was applied to an array.
    ConstantIndexFromEndOnArray {
        /// Predecessor type.
        ty: TyId,
    },
    /// A from-end subslice was applied to an array.
    SubsliceFromEndOnArray {
        /// Predecessor type.
        ty: TyId,
    },
    /// A from-start subslice was applied to a slice.
    SubsliceFromStartOnSlice {
        /// Predecessor type.
        ty: TyId,
    },
    /// A downcast projection was applied to a non-enum type.
    DowncastOnNonEnum {
        /// Predecessor type.
        ty: TyId,
    },
    /// A variant index is outside the enum shape.
    VariantIdxOutOfRange {
        /// Offending variant index.
        actual: u32,
        /// Exclusive upper bound for valid variant indices.
        max: u32,
    },
}

#[derive(Clone, Copy)]
enum CurrentTy {
    Interned(TyId),
    DerivedArray { elem: TyId, diagnostic_ty: TyId },
}

/// Validates every `Place` reachable from the given `Body`.
///
/// Returns the first section 19.7 violation in walk order, or
/// `Ok(())` when every reachable place is well formed.
pub fn validate_body_places(model: &RegseqModel, body: &Body) -> Result<(), PlaceValidationError> {
    for (block_idx, block) in body.basic_blocks.iter().enumerate() {
        let block_id = BlockId::from_u32(block_idx as u32);
        for (stmt_idx, stmt) in block.statements.iter().enumerate() {
            let location = PlaceLocation {
                block: block_id,
                site: PlaceSite::Statement(stmt_idx),
                projection_step: None,
            };
            match &stmt.kind {
                StatementKind::Assign(place, rvalue) => {
                    validate_place(model, body, place, location.clone())?;
                    validate_rvalue(model, body, rvalue, location)?;
                }
                StatementKind::SetDiscriminant(place, _variant) => {
                    validate_place(model, body, place, location)?;
                }
                StatementKind::Intrinsic(intrinsic) => match intrinsic {
                    NonDivergingIntrinsic::Assume(operand) => {
                        validate_operand(model, body, operand, location)?;
                    }
                    NonDivergingIntrinsic::CopyNonOverlapping(payload) => {
                        let CopyNonOverlapping { src, dst, count } = payload;
                        validate_operand(model, body, src, location.clone())?;
                        validate_operand(model, body, dst, location.clone())?;
                        validate_operand(model, body, count, location)?;
                    }
                },
            }
        }

        let location = PlaceLocation {
            block: block_id,
            site: PlaceSite::Terminator,
            projection_step: None,
        };
        match &block.terminator.kind {
            TerminatorKind::Goto(_) | TerminatorKind::Return | TerminatorKind::Unreachable => {}
            TerminatorKind::SwitchInt { discr, targets: _ } => {
                validate_operand(model, body, discr, location)?;
            }
            TerminatorKind::Call {
                callee,
                args,
                destination,
                target: _,
            } => {
                validate_call_callee(model, body, callee, location.clone())?;
                for arg in args.iter() {
                    validate_operand(model, body, arg, location.clone())?;
                }
                validate_place(model, body, destination, location)?;
            }
            TerminatorKind::Assert {
                cond,
                expected: _,
                msg,
                target: _,
            } => {
                validate_operand(model, body, cond, location.clone())?;
                validate_assert_kind(model, body, msg, location)?;
            }
        }
    }

    Ok(())
}

fn validate_rvalue(
    model: &RegseqModel,
    body: &Body,
    rvalue: &Rvalue,
    location: PlaceLocation,
) -> Result<(), PlaceValidationError> {
    match rvalue {
        Rvalue::Use(operand) | Rvalue::Repeat(operand, _) | Rvalue::UnaryOp(_, operand) => {
            validate_operand(model, body, operand, location)
        }
        Rvalue::Ref(_, place) | Rvalue::RawPtr(_, place) | Rvalue::Discriminant(place) => {
            validate_place(model, body, place, location)
        }
        Rvalue::Cast(kind, operand, _) => {
            match kind {
                CastKind::PointerExposeProvenance
                | CastKind::PointerWithExposedProvenance
                | CastKind::IntToInt
                | CastKind::FloatToInt
                | CastKind::FloatToFloat
                | CastKind::IntToFloat
                | CastKind::PtrToPtr
                | CastKind::FnPtrToPtr
                | CastKind::Transmute
                | CastKind::Subtype => {}
                CastKind::PointerCoercion(coercion) => match coercion {
                    PointerCoercion::ReifyFnPointer
                    | PointerCoercion::UnsafeFnPointer
                    | PointerCoercion::ClosureFnPointer
                    | PointerCoercion::Unsize => {}
                },
            }
            validate_operand(model, body, operand, location)
        }
        Rvalue::BinaryOp(_, left, right) => {
            validate_operand(model, body, left, location.clone())?;
            validate_operand(model, body, right, location)
        }
        Rvalue::Aggregate(kind, operands) => {
            match kind {
                AggregateKind::Array(_)
                | AggregateKind::Tuple(_)
                | AggregateKind::Struct(_)
                | AggregateKind::Enum(_, _)
                | AggregateKind::Union(_, _)
                | AggregateKind::Closure(_)
                | AggregateKind::RawPtr(_, _) => {}
            }
            for operand in operands.iter() {
                validate_operand(model, body, operand, location.clone())?;
            }
            Ok(())
        }
    }
}

fn validate_assert_kind(
    model: &RegseqModel,
    body: &Body,
    msg: &AssertKind,
    location: PlaceLocation,
) -> Result<(), PlaceValidationError> {
    match msg {
        AssertKind::BoundsCheck { len, index } => {
            validate_operand(model, body, len, location.clone())?;
            validate_operand(model, body, index, location)
        }
        AssertKind::Overflow(_, left, right) => {
            validate_operand(model, body, left, location.clone())?;
            validate_operand(model, body, right, location)
        }
        AssertKind::OverflowNeg(operand)
        | AssertKind::DivisionByZero(operand)
        | AssertKind::RemainderByZero(operand)
        | AssertKind::InvalidEnumConstruction(operand) => {
            validate_operand(model, body, operand, location)
        }
        AssertKind::MisalignedPointerDereference { required, found } => {
            validate_operand(model, body, required, location.clone())?;
            validate_operand(model, body, found, location)
        }
        AssertKind::NullPointerDereference => Ok(()),
    }
}

fn validate_call_callee(
    model: &RegseqModel,
    body: &Body,
    callee: &CallCallee,
    location: PlaceLocation,
) -> Result<(), PlaceValidationError> {
    match callee {
        CallCallee::Direct(_) => Ok(()),
        CallCallee::Indirect(operand) => validate_operand(model, body, operand, location),
    }
}

fn validate_operand(
    model: &RegseqModel,
    body: &Body,
    operand: &Operand,
    location: PlaceLocation,
) -> Result<(), PlaceValidationError> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => validate_place(model, body, place, location),
        Operand::Constant(_) => Ok(()),
    }
}

fn validate_place(
    model: &RegseqModel,
    body: &Body,
    place: &Place,
    location: PlaceLocation,
) -> Result<(), PlaceValidationError> {
    if place.local.index() >= body.local_decls.len() {
        return Err(error(
            location,
            PlaceValidationErrorKind::HeadLocalOutOfRange {
                local: place.local,
                locals_len: body.local_decls.len(),
            },
        ));
    }

    let mut current_ty = CurrentTy::Interned(body.local_decls[place.local].ty);
    let mut active_variant = None;

    for (idx, elem) in place.projection.iter().enumerate() {
        let step_location = with_projection_step(&location, idx);
        match elem {
            PlaceElem::Deref if idx != 0 => {
                return Err(error(
                    step_location,
                    PlaceValidationErrorKind::DerefNotFirst { index: idx },
                ));
            }
            PlaceElem::Downcast(_)
                if !matches!(place.projection.get(idx + 1), Some(PlaceElem::Field(_, _))) =>
            {
                return Err(error(
                    step_location,
                    PlaceValidationErrorKind::DowncastNotFollowedByField { index: idx },
                ));
            }
            _ => {}
        }

        match elem {
            PlaceElem::Deref => match current_ty {
                CurrentTy::Interned(ty) => match model.types.resolve(ty) {
                    Ty::Ref(inner, _) | Ty::Ptr(inner, _) => {
                        current_ty = CurrentTy::Interned(*inner);
                        active_variant = None;
                    }
                    _ => {
                        return Err(error(
                            step_location,
                            PlaceValidationErrorKind::DerefOnNonPointer { ty },
                        ));
                    }
                },
                CurrentTy::DerivedArray { diagnostic_ty, .. } => {
                    return Err(error(
                        step_location,
                        PlaceValidationErrorKind::DerefOnNonPointer { ty: diagnostic_ty },
                    ));
                }
            },
            PlaceElem::Field(field, field_ty) => {
                let fields = field_list(model, current_ty, active_variant, step_location.clone())?;
                let field_index = field.index();
                let Some(expected) = fields.get(field_index).map(|field| field.ty) else {
                    return Err(error(
                        step_location,
                        PlaceValidationErrorKind::FieldIdxOutOfRange {
                            actual: field.as_u32(),
                            max: fields.len() as u32,
                        },
                    ));
                };
                if expected != *field_ty {
                    return Err(error(
                        step_location,
                        PlaceValidationErrorKind::FieldTypeMismatch {
                            expected,
                            actual: *field_ty,
                        },
                    ));
                }
                current_ty = CurrentTy::Interned(*field_ty);
                active_variant = None;
            }
            PlaceElem::Index(_) => match current_ty {
                CurrentTy::Interned(ty) => match model.types.resolve(ty) {
                    Ty::Array(elem, _) | Ty::Slice(elem) => {
                        current_ty = CurrentTy::Interned(*elem);
                    }
                    _ => {
                        return Err(error(
                            step_location,
                            PlaceValidationErrorKind::IndexOnNonIndexable { ty },
                        ));
                    }
                },
                CurrentTy::DerivedArray { elem, .. } => {
                    current_ty = CurrentTy::Interned(elem);
                }
            },
            PlaceElem::ConstantIndex {
                offset: _,
                min_length: _,
                from_end,
            } => match (from_end, current_ty) {
                (false, CurrentTy::Interned(ty)) => match model.types.resolve(ty) {
                    Ty::Array(elem, _) | Ty::Slice(elem) => {
                        current_ty = CurrentTy::Interned(*elem);
                    }
                    _ => {
                        return Err(error(
                            step_location,
                            PlaceValidationErrorKind::IndexOnNonIndexable { ty },
                        ));
                    }
                },
                (false, CurrentTy::DerivedArray { elem, .. }) => {
                    current_ty = CurrentTy::Interned(elem);
                }
                (true, CurrentTy::Interned(ty)) => match model.types.resolve(ty) {
                    Ty::Slice(elem) => {
                        current_ty = CurrentTy::Interned(*elem);
                    }
                    Ty::Array(_, _) => {
                        return Err(error(
                            step_location,
                            PlaceValidationErrorKind::ConstantIndexFromEndOnArray { ty },
                        ));
                    }
                    _ => {
                        return Err(error(
                            step_location,
                            PlaceValidationErrorKind::IndexOnNonIndexable { ty },
                        ));
                    }
                },
                (true, CurrentTy::DerivedArray { diagnostic_ty, .. }) => {
                    return Err(error(
                        step_location,
                        PlaceValidationErrorKind::ConstantIndexFromEndOnArray { ty: diagnostic_ty },
                    ));
                }
            },
            PlaceElem::Subslice { from, to, from_end } => match (from_end, current_ty) {
                (false, CurrentTy::Interned(ty)) => match model.types.resolve(ty) {
                    Ty::Array(elem, _) => {
                        let len = to.checked_sub(*from).expect("subslice to precedes from");
                        current_ty = CurrentTy::DerivedArray {
                            elem: *elem,
                            diagnostic_ty: ty,
                        };
                        let _ = len;
                    }
                    Ty::Slice(_) => {
                        return Err(error(
                            step_location,
                            PlaceValidationErrorKind::SubsliceFromStartOnSlice { ty },
                        ));
                    }
                    _ => {
                        return Err(error(
                            step_location,
                            PlaceValidationErrorKind::IndexOnNonIndexable { ty },
                        ));
                    }
                },
                (
                    false,
                    CurrentTy::DerivedArray {
                        elem,
                        diagnostic_ty,
                    },
                ) => {
                    let len = to.checked_sub(*from).expect("subslice to precedes from");
                    current_ty = CurrentTy::DerivedArray {
                        elem,
                        diagnostic_ty,
                    };
                    let _ = len;
                }
                (true, CurrentTy::Interned(ty)) => match model.types.resolve(ty) {
                    Ty::Slice(_) => {}
                    Ty::Array(_, _) => {
                        return Err(error(
                            step_location,
                            PlaceValidationErrorKind::SubsliceFromEndOnArray { ty },
                        ));
                    }
                    _ => {
                        return Err(error(
                            step_location,
                            PlaceValidationErrorKind::IndexOnNonIndexable { ty },
                        ));
                    }
                },
                (true, CurrentTy::DerivedArray { diagnostic_ty, .. }) => {
                    return Err(error(
                        step_location,
                        PlaceValidationErrorKind::SubsliceFromEndOnArray { ty: diagnostic_ty },
                    ));
                }
            },
            PlaceElem::Downcast(variant) => {
                let shape = match current_ty {
                    CurrentTy::Interned(ty) => match model.types.resolve(ty) {
                        Ty::Enum { layout, .. } => match &layout.kind {
                            LayoutKind::Enum { shape } => shape,
                            _ => unreachable!("Ty::Enum must carry enum layout kind"),
                        },
                        _ => {
                            return Err(error(
                                step_location,
                                PlaceValidationErrorKind::DowncastOnNonEnum { ty },
                            ));
                        }
                    },
                    CurrentTy::DerivedArray { diagnostic_ty, .. } => {
                        return Err(error(
                            step_location,
                            PlaceValidationErrorKind::DowncastOnNonEnum { ty: diagnostic_ty },
                        ));
                    }
                };
                let max = variant_count(shape);
                if variant.as_u32() >= max {
                    return Err(error(
                        step_location,
                        PlaceValidationErrorKind::VariantIdxOutOfRange {
                            actual: variant.as_u32(),
                            max,
                        },
                    ));
                }
                active_variant = Some(variant.as_u32());
            }
        }
    }

    Ok(())
}

fn field_list(
    model: &RegseqModel,
    ty: CurrentTy,
    active_variant: Option<u32>,
    location: PlaceLocation,
) -> Result<&[FieldLayout], PlaceValidationError> {
    match active_variant {
        Some(variant) => match ty {
            CurrentTy::Interned(ty) => match model.types.resolve(ty) {
                Ty::Enum { layout, .. } => {
                    let shape = match &layout.kind {
                        LayoutKind::Enum { shape } => shape,
                        _ => unreachable!("Ty::Enum must carry enum layout kind"),
                    };
                    Ok(variant_layout(shape, variant).fields.as_slice())
                }
                _ => Err(error(
                    location,
                    PlaceValidationErrorKind::FieldOnNonAggregate {
                        ty,
                        active_variant: Some(variant),
                    },
                )),
            },
            CurrentTy::DerivedArray { diagnostic_ty, .. } => Err(error(
                location,
                PlaceValidationErrorKind::FieldOnNonAggregate {
                    ty: diagnostic_ty,
                    active_variant: Some(variant),
                },
            )),
        },
        None => match ty {
            CurrentTy::Interned(ty) => match model.types.resolve(ty) {
                Ty::Struct { layout, .. } => {
                    let LayoutKind::Struct { fields } = &layout.kind else {
                        unreachable!("Ty::Struct must carry struct layout kind");
                    };
                    Ok(fields.as_slice())
                }
                Ty::Union { layout, .. } => {
                    let LayoutKind::Union { fields } = &layout.kind else {
                        unreachable!("Ty::Union must carry union layout kind");
                    };
                    Ok(fields.as_slice())
                }
                Ty::Tuple { fields, .. } => Ok(tuple_fields_as_layout(model, fields)),
                Ty::Closure(item, args) => {
                    let captures = model
                        .types
                        .find_nominal(NominalKind::Struct, *item, args)
                        .expect("closure capture struct type not found");
                    match model.types.resolve(captures) {
                        Ty::Struct { layout, .. } => {
                            let LayoutKind::Struct { fields } = &layout.kind else {
                                unreachable!(
                                    "closure capture struct must carry struct layout kind"
                                );
                            };
                            Ok(fields.as_slice())
                        }
                        _ => unreachable!("closure capture type must be Ty::Struct"),
                    }
                }
                _ => Err(error(
                    location,
                    PlaceValidationErrorKind::FieldOnNonAggregate {
                        ty,
                        active_variant: None,
                    },
                )),
            },
            CurrentTy::DerivedArray { diagnostic_ty, .. } => Err(error(
                location,
                PlaceValidationErrorKind::FieldOnNonAggregate {
                    ty: diagnostic_ty,
                    active_variant: None,
                },
            )),
        },
    }
}

fn tuple_fields_as_layout<'a>(model: &'a RegseqModel, fields: &'a [TyId]) -> &'a [FieldLayout] {
    model
        .types
        .iter()
        .find_map(|(_, ty)| match ty {
            Ty::Tuple {
                fields: candidate,
                layout,
            } if candidate.as_slice() == fields => match &layout.kind {
                LayoutKind::Struct { fields } => Some(fields.as_slice()),
                _ => unreachable!("Ty::Tuple must carry struct layout kind"),
            },
            _ => None,
        })
        .expect("tuple layout not found")
}

fn variant_layout(shape: &EnumShape, variant: u32) -> &VariantLayout {
    match shape {
        EnumShape::Empty => unreachable!("empty enum variant was range-checked"),
        EnumShape::Single {
            variant: layout, ..
        } => {
            assert_eq!(variant, 0, "single enum variant was range-checked");
            layout
        }
        EnumShape::Multiple { variants, .. } => &variants[variant as usize],
    }
}

fn variant_count(shape: &EnumShape) -> u32 {
    match shape {
        EnumShape::Empty => 0,
        EnumShape::Single { .. } => 1,
        EnumShape::Multiple { variants, .. } => variants.len() as u32,
    }
}

fn with_projection_step(location: &PlaceLocation, projection_step: usize) -> PlaceLocation {
    let mut location = location.clone();
    location.projection_step = Some(projection_step);
    location
}

fn error(location: PlaceLocation, kind: PlaceValidationErrorKind) -> PlaceValidationError {
    PlaceValidationError { location, kind }
}

#[cfg(test)]
mod tests {
    use super::{field_list, CurrentTy, PlaceLocation, PlaceSite, PlaceValidationErrorKind};
    use crate::{BlockId, RegseqModel, StableCrateId, Ty};

    #[test]
    fn active_variant_field_rejects_non_enum_predecessor() {
        let mut model = RegseqModel::with_primary("spec", StableCrateId(7));
        let bool_ty = model.types.intern(Ty::Bool);
        let location = PlaceLocation {
            block: BlockId::START_BLOCK,
            site: PlaceSite::Statement(0),
            projection_step: Some(0),
        };

        let err = field_list(&model, CurrentTy::Interned(bool_ty), Some(0), location).unwrap_err();

        assert_eq!(
            err.kind,
            PlaceValidationErrorKind::FieldOnNonAggregate {
                ty: bool_ty,
                active_variant: Some(0),
            }
        );
    }
}
