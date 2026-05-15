//! Best-effort `Ty` recovery for body-local content.
//!
//! These helpers implement section 22.5.2 / 23.6.2 type recovery:
//! `place_ty`, `rvalue_ty`, and `operand_ty`. Each helper returns
//! `Option<TyId>` because Constant operand types are not always
//! reachable through inline data (only scalar `Const` variants carry
//! a derivable `Ty`) and aggregate-shaped rvalues may not have a
//! matching interned type. Statement and terminator invariant checks
//! use these helpers and skip the check when a side is `None`.

use crate::body::Body;
use crate::const_value::Const;
use crate::ids::TyId;
use crate::layout::{EnumShape, LayoutKind};
use crate::model::RegseqModel;
use crate::operand::Operand;
use crate::place::{Place, PlaceElem};
use crate::rvalue::{AggregateKind, BinOp, Rvalue, UnOp};
use crate::ty::{Mutability, Ty, UintKind};
use crate::ty_interner::NominalKind;

/// Returns the post-projection type of `place` against `body`.
///
/// Walks the projection chain using the section 19.4 narrowing
/// table. Returns `None` when the head local is out of range, when
/// a projection step is structurally invalid, or when a derived
/// array subslice has no matching interned `Ty::Array`.
pub fn place_ty(model: &RegseqModel, body: &Body, place: &Place) -> Option<TyId> {
    if place.local.index() >= body.local_decls.len() {
        return None;
    }
    let mut current = body.local_decls[place.local].ty;
    let mut derived_array_elem: Option<TyId> = None;
    let mut active_variant: Option<u32> = None;

    for elem in place.projection.iter() {
        match elem {
            PlaceElem::Deref => match model.types.resolve(current) {
                Ty::Ref(inner, _) | Ty::Ptr(inner, _) => {
                    current = *inner;
                    derived_array_elem = None;
                    active_variant = None;
                }
                _ => return None,
            },
            PlaceElem::Field(_, field_ty) => {
                current = *field_ty;
                derived_array_elem = None;
                active_variant = None;
            }
            PlaceElem::Index(_) => match (derived_array_elem, model.types.resolve(current)) {
                (Some(elem), _) => {
                    current = elem;
                    derived_array_elem = None;
                }
                (None, Ty::Array(elem, _) | Ty::Slice(elem)) => {
                    current = *elem;
                }
                _ => return None,
            },
            PlaceElem::ConstantIndex {
                from_end,
                ..
            } => match (derived_array_elem, model.types.resolve(current)) {
                (Some(elem), _) => {
                    current = elem;
                    derived_array_elem = None;
                }
                (None, Ty::Array(elem, _) | Ty::Slice(elem)) if !*from_end => {
                    current = *elem;
                }
                (None, Ty::Slice(elem)) => {
                    current = *elem;
                }
                _ => return None,
            },
            PlaceElem::Subslice { from, to, from_end } => match (*from_end, model.types.resolve(current)) {
                (false, Ty::Array(elem, _)) => {
                    let len = to.checked_sub(*from)?;
                    if let Some(found) = model.types.find(&Ty::Array(*elem, len)) {
                        current = found;
                        derived_array_elem = None;
                    } else {
                        derived_array_elem = Some(*elem);
                    }
                }
                (true, Ty::Slice(_)) => {
                    // Slice stays slice; current unchanged.
                }
                _ => return None,
            },
            PlaceElem::Downcast(variant) => {
                if !matches!(model.types.resolve(current), Ty::Enum { .. }) {
                    return None;
                }
                active_variant = Some(variant.as_u32());
            }
        }
    }
    let _ = active_variant;
    Some(current)
}

/// Returns the type of `operand` against `body`.
///
/// `Copy(p)` and `Move(p)` delegate to `place_ty`. `Constant(c)`
/// returns a derived type for the scalar `Const` variants and `None`
/// for the variants whose type is not reachable through inline data.
pub fn operand_ty(model: &RegseqModel, body: &Body, operand: &Operand) -> Option<TyId> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => place_ty(model, body, place),
        Operand::Constant(c) => const_ty(model, *c),
    }
}

fn const_ty(model: &RegseqModel, id: crate::ids::ConstId) -> Option<TyId> {
    if id.index() >= model.consts.len() {
        return None;
    }
    match model.consts.resolve(id) {
        Const::Bool(_) => model.types.find(&Ty::Bool),
        Const::Char(_) => model.types.find(&Ty::Char),
        Const::Int(kind, _) => model.types.find(&Ty::Int(*kind)),
        Const::Uint(kind, _) => model.types.find(&Ty::Uint(*kind)),
        Const::Float(kind, _) => model.types.find(&Ty::Float(*kind)),
        Const::NullPtr(ty) => model.types.find(&Ty::Ptr(*ty, Mutability::Not)),
        Const::ZST
        | Const::Bytes(_)
        | Const::Aggregate(_)
        | Const::EnumValue { .. }
        | Const::Reloc(_) => None,
    }
}

/// Returns the type produced by `rvalue` against `body`.
///
/// Best-effort per the section 21.5 rvalue surface; returns `None`
/// when the produced type is not reachable through inline data
/// (uninterned shapes, missing operand types, etc.).
pub fn rvalue_ty(model: &RegseqModel, body: &Body, rvalue: &Rvalue) -> Option<TyId> {
    match rvalue {
        Rvalue::Use(op) => operand_ty(model, body, op),
        Rvalue::Repeat(op, count) => {
            let elem = operand_ty(model, body, op)?;
            model.types.find(&Ty::Array(elem, *count))
        }
        Rvalue::Ref(prov, place) => {
            let inner = place_ty(model, body, place)?;
            model.types.find(&Ty::Ref(inner, *prov))
        }
        Rvalue::RawPtr(mutability, place) => {
            let inner = place_ty(model, body, place)?;
            model.types.find(&Ty::Ptr(inner, *mutability))
        }
        Rvalue::Cast(_, _, ty) => Some(*ty),
        Rvalue::BinaryOp(op, left, _right) => match op {
            BinOp::Eq | BinOp::Lt | BinOp::Le | BinOp::Ne | BinOp::Ge | BinOp::Gt => {
                model.types.find(&Ty::Bool)
            }
            BinOp::Add
            | BinOp::Sub
            | BinOp::Mul
            | BinOp::Div
            | BinOp::Rem
            | BinOp::AddUnchecked
            | BinOp::SubUnchecked
            | BinOp::MulUnchecked
            | BinOp::BitXor
            | BinOp::BitAnd
            | BinOp::BitOr
            | BinOp::Shl
            | BinOp::ShlUnchecked
            | BinOp::Shr
            | BinOp::ShrUnchecked
            | BinOp::Offset => operand_ty(model, body, left),
            BinOp::AddWithOverflow | BinOp::SubWithOverflow | BinOp::MulWithOverflow => None,
            BinOp::Cmp => None,
        },
        Rvalue::UnaryOp(op, operand) => match op {
            UnOp::Not | UnOp::Neg => operand_ty(model, body, operand),
            UnOp::PtrMetadata => model.types.find(&Ty::Uint(UintKind::Usize)),
        },
        Rvalue::Discriminant(_) => None,
        Rvalue::Aggregate(kind, _) => match kind {
            AggregateKind::Array(ty)
            | AggregateKind::Tuple(ty)
            | AggregateKind::Struct(ty)
            | AggregateKind::Enum(ty, _)
            | AggregateKind::Union(ty, _)
            | AggregateKind::Closure(ty) => Some(*ty),
            AggregateKind::RawPtr(pointee, mutability) => {
                model.types.find(&Ty::Ptr(*pointee, *mutability))
            }
        },
    }
}

/// Returns the variant count for `place_ty(p)` when `p` resolves to
/// `Ty::Enum`. Returns `None` for non-enum types or when `place_ty`
/// itself is unrecoverable. Used by statement invariant B-2.
pub fn place_enum_variant_count(model: &RegseqModel, body: &Body, place: &Place) -> Option<u32> {
    let ty = place_ty(model, body, place)?;
    match model.types.resolve(ty) {
        Ty::Enum { layout, .. } => match &layout.kind {
            LayoutKind::Enum { shape } => Some(variant_count_of(shape)),
            _ => None,
        },
        _ => None,
    }
}

fn variant_count_of(shape: &EnumShape) -> u32 {
    match shape {
        EnumShape::Empty => 0,
        EnumShape::Single { .. } => 1,
        EnumShape::Multiple { variants, .. } => variants.len() as u32,
    }
}

/// Returns the resolved pointee type for `Ty::Ref` or `Ty::Ptr`.
/// Returns `None` when `ty` is not a deref-able pointer type. Used
/// by statement invariant B-5.
pub fn pointee_ty(model: &RegseqModel, ty: TyId) -> Option<TyId> {
    match model.types.resolve(ty) {
        Ty::Ref(inner, _) | Ty::Ptr(inner, _) => Some(*inner),
        _ => None,
    }
}

/// Looks up the closure capture struct for `closure_ty` and returns
/// its `Ty::Struct` TyId. Returns `None` when `closure_ty` is not a
/// `Ty::Closure` or when the captures struct is not yet interned.
#[allow(dead_code)]
pub(crate) fn closure_captures_ty(model: &RegseqModel, closure_ty: TyId) -> Option<TyId> {
    match model.types.resolve(closure_ty) {
        Ty::Closure(item, args) => {
            model.types.find_nominal(NominalKind::Struct, *item, args)
        }
        _ => None,
    }
}
