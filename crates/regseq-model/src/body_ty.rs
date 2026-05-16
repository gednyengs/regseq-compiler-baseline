//! Body-local `Ty` recovery helpers for statement / terminator
//! invariant checks.
//!
//! Spec sections 19.4 (place narrowing), 21 (rvalue type recovery),
//! 22.5.2 (statement-site invariants), 23.6.2 (terminator-site
//! invariants).
//!
//! Each helper returns a `TypeRecovery`:
//!
//! - `Recovered(ty)` -- the type was reached cleanly.
//! - `NotApplicable` -- the spec rule does not apply at this site
//!   (e.g. an aggregate-shaped `Const::Aggregate` operand carries no
//!   single derivable `Ty`, or a binary op whose output shape is a
//!   bridge-synthesised tuple that may not be interned at this stage).
//! - `Malformed` -- recovery failed because the body is structurally
//!   broken (head-local out of range, deref applied to a non-pointer,
//!   field projection over a non-aggregate predecessor, etc.). Callers
//!   must surface this as a typed `InvariantViolation`; they must NOT
//!   silently skip the invariant check.

use crate::body::Body;
use crate::const_value::Const;
use crate::ids::{ConstId, TyId};
use crate::layout::{EnumShape, LayoutKind};
use crate::model::RegseqModel;
use crate::operand::Operand;
use crate::place::{Place, PlaceElem};
use crate::rvalue::{AggregateKind, BinOp, Rvalue, UnOp};
use crate::ty::{Mutability, Ty, UintKind};

/// Outcome of a body-local type recovery.
///
/// See module docs for the per-variant semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeRecovery {
    /// The type was reached cleanly.
    Recovered(TyId),
    /// The spec rule does not apply at this site; the caller should
    /// skip the check without emitting an error.
    NotApplicable,
    /// Recovery failed because the body is malformed; the caller must
    /// emit a typed `InvariantViolation`.
    Malformed,
}

impl TypeRecovery {
    /// Returns the recovered `TyId`, panicking on the other variants.
    /// Test-only helper.
    #[cfg(test)]
    pub fn unwrap_recovered(self) -> TyId {
        match self {
            TypeRecovery::Recovered(ty) => ty,
            other => panic!("expected TypeRecovery::Recovered, got {:?}", other),
        }
    }
}

/// Returns the post-projection type for `place` against `body`.
///
/// Walks the section 19.4 narrowing table over the projection chain.
/// `Malformed` is returned when the head local is out of range or when
/// a projection step has no resolvable successor type per the table.
/// The structural pre-pass owned by `place_validator` should run
/// before this helper at body validation time; this helper assumes the
/// caller will surface `Malformed` rather than silently skipping a
/// dependent invariant.
pub fn place_ty(model: &RegseqModel, body: &Body, place: &Place) -> TypeRecovery {
    if place.local.index() >= body.local_decls.len() {
        return TypeRecovery::Malformed;
    }
    let mut current = body.local_decls[place.local].ty;
    let mut derived_array_elem: Option<TyId> = None;
    let mut active_variant: Option<u32> = None;

    for elem in place.projection.iter() {
        if current.index() >= model.types.len() {
            return TypeRecovery::Malformed;
        }
        match elem {
            PlaceElem::Deref => match model.types.resolve(current) {
                Ty::Ref(inner, _) | Ty::Ptr(inner, _) => {
                    current = *inner;
                    derived_array_elem = None;
                    active_variant = None;
                }
                _ => return TypeRecovery::Malformed,
            },
            PlaceElem::Field(_, field_ty) => {
                // Defensive: a Field projection's predecessor must
                // resolve to an aggregate (Struct/Union/Tuple/
                // Closure when no active variant, Enum when a prior
                // Downcast selected one). place_validator runs first
                // and rejects non-aggregate predecessors, but
                // body_ty is a publicly callable helper so the same
                // structural precondition is re-checked here per
                // spec section 19.4 narrowing rule for `Field`.
                let predecessor_ok = matches!(
                    (active_variant, model.types.resolve(current)),
                    (Some(_), Ty::Enum { .. })
                        | (
                            None,
                            Ty::Struct { .. }
                                | Ty::Union { .. }
                                | Ty::Tuple { .. }
                                | Ty::Closure(_, _),
                        )
                );
                if !predecessor_ok {
                    return TypeRecovery::Malformed;
                }
                if field_ty.index() >= model.types.len() {
                    return TypeRecovery::Malformed;
                }
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
                _ => return TypeRecovery::Malformed,
            },
            PlaceElem::ConstantIndex { from_end, .. } => {
                match (derived_array_elem, model.types.resolve(current)) {
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
                    _ => return TypeRecovery::Malformed,
                }
            }
            PlaceElem::Subslice { from, to, from_end } => {
                match (*from_end, model.types.resolve(current)) {
                    (false, Ty::Array(elem, _)) => {
                        let Some(len) = to.checked_sub(*from) else {
                            return TypeRecovery::Malformed;
                        };
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
                    _ => return TypeRecovery::Malformed,
                }
            }
            PlaceElem::Downcast(variant) => {
                if !matches!(model.types.resolve(current), Ty::Enum { .. }) {
                    return TypeRecovery::Malformed;
                }
                active_variant = Some(variant.as_u32());
            }
        }
    }
    let _ = active_variant;
    TypeRecovery::Recovered(current)
}

/// Returns the type of `operand` against `body`.
///
/// `Copy(p)` and `Move(p)` delegate to `place_ty`. `Constant(c)`
/// resolves through the const interner: scalar variants whose `Ty` is
/// derivable from the variant tag yield `Recovered(...)`; aggregate
/// and reloc variants yield `NotApplicable` (operand-context type
/// coherence for those is owned by upstream stages per spec section
/// 20.5). A `ConstId` outside the interner is `Malformed`.
pub fn operand_ty(model: &RegseqModel, body: &Body, operand: &Operand) -> TypeRecovery {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => place_ty(model, body, place),
        Operand::Constant(c) => const_ty(model, *c),
    }
}

fn const_ty(model: &RegseqModel, id: ConstId) -> TypeRecovery {
    if id.index() >= model.consts.len() {
        return TypeRecovery::Malformed;
    }
    match model.consts.resolve(id) {
        Const::Bool(_) => find_or_not_applicable(model, &Ty::Bool),
        Const::Char(_) => find_or_not_applicable(model, &Ty::Char),
        Const::Int(kind, _) => find_or_not_applicable(model, &Ty::Int(*kind)),
        Const::Uint(kind, _) => find_or_not_applicable(model, &Ty::Uint(*kind)),
        Const::Float(kind, _) => find_or_not_applicable(model, &Ty::Float(*kind)),
        Const::NullPtr(ty) => find_or_not_applicable(model, &Ty::Ptr(*ty, Mutability::Not)),
        // Aggregate, EnumValue, Reloc, Bytes, ZST: no single Ty is
        // derivable from inline data; per spec section 20.5, type
        // coherence here is upstream's responsibility.
        Const::ZST
        | Const::Bytes(_)
        | Const::Aggregate(_)
        | Const::EnumValue { .. }
        | Const::Reloc(_) => TypeRecovery::NotApplicable,
    }
}

fn find_or_not_applicable(model: &RegseqModel, ty: &Ty) -> TypeRecovery {
    match model.types.find(ty) {
        Some(found) => TypeRecovery::Recovered(found),
        None => TypeRecovery::NotApplicable,
    }
}

/// Returns the type produced by `rvalue` against `body`.
///
/// Per spec section 21 type recovery rules. Best-effort: when an
/// operand's type is `NotApplicable` and the output shape depends on
/// it, the rvalue itself returns `NotApplicable`. When recovery
/// requires resolving a nominal type that is not interned, the result
/// is `NotApplicable` (the bridge may not have a parallel interned
/// shape).
pub fn rvalue_ty(model: &RegseqModel, body: &Body, rvalue: &Rvalue) -> TypeRecovery {
    match rvalue {
        Rvalue::Use(op) => operand_ty(model, body, op),
        Rvalue::Repeat(op, count) => match operand_ty(model, body, op) {
            TypeRecovery::Recovered(elem) => {
                find_or_not_applicable(model, &Ty::Array(elem, *count))
            }
            other => other,
        },
        Rvalue::Ref(prov, place) => match place_ty(model, body, place) {
            TypeRecovery::Recovered(inner) => find_or_not_applicable(model, &Ty::Ref(inner, *prov)),
            other => other,
        },
        Rvalue::RawPtr(mutability, place) => match place_ty(model, body, place) {
            TypeRecovery::Recovered(inner) => {
                find_or_not_applicable(model, &Ty::Ptr(inner, *mutability))
            }
            other => other,
        },
        Rvalue::Cast(_, _, ty) => {
            if ty.index() >= model.types.len() {
                TypeRecovery::Malformed
            } else {
                TypeRecovery::Recovered(*ty)
            }
        }
        Rvalue::BinaryOp(op, left, _right) => match op {
            BinOp::Eq | BinOp::Lt | BinOp::Le | BinOp::Ne | BinOp::Ge | BinOp::Gt => {
                find_or_not_applicable(model, &Ty::Bool)
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
            // The overflow-checked and Cmp variants produce a tuple
            // / Ordering shape; recovering the matching interned Ty
            // requires nominal lookups not currently centralised in
            // this helper.
            BinOp::AddWithOverflow | BinOp::SubWithOverflow | BinOp::MulWithOverflow => {
                TypeRecovery::NotApplicable
            }
            BinOp::Cmp => TypeRecovery::NotApplicable,
        },
        Rvalue::UnaryOp(op, operand) => match op {
            UnOp::Not | UnOp::Neg => operand_ty(model, body, operand),
            // Per spec section 21: PtrMetadata extracts the metadata
            // pointee of a fat pointer.
            //   Ref/Ptr -> Dynamic { vtable_ty }: metadata is a
            //     pointer to the vtable struct; return
            //     Ty::Ptr(vtable_ty, Not).
            //   Ref/Ptr -> Slice/Str: metadata is a length;
            //     return Ty::Uint(Usize).
            //   Otherwise: the operand is not a fat pointer; the
            //     body is malformed for a PtrMetadata site.
            UnOp::PtrMetadata => match operand_ty(model, body, operand) {
                TypeRecovery::Recovered(op_ty) => ptr_metadata_ty(model, op_ty),
                other => other,
            },
        },
        // Discriminant produces an integer-typed scalar; the exact
        // tag scalar depends on the enum layout. Not directly
        // recoverable without a layout-side helper.
        Rvalue::Discriminant(_) => TypeRecovery::NotApplicable,
        Rvalue::Aggregate(kind, _) => match kind {
            AggregateKind::Array(ty)
            | AggregateKind::Tuple(ty)
            | AggregateKind::Struct(ty)
            | AggregateKind::Enum(ty, _)
            | AggregateKind::Union(ty, _)
            | AggregateKind::Closure(ty) => {
                if ty.index() >= model.types.len() {
                    TypeRecovery::Malformed
                } else {
                    TypeRecovery::Recovered(*ty)
                }
            }
            AggregateKind::RawPtr(pointee, mutability) => {
                find_or_not_applicable(model, &Ty::Ptr(*pointee, *mutability))
            }
        },
    }
}

fn ptr_metadata_ty(model: &RegseqModel, operand_ty: TyId) -> TypeRecovery {
    if operand_ty.index() >= model.types.len() {
        return TypeRecovery::Malformed;
    }
    let pointee = match model.types.resolve(operand_ty) {
        Ty::Ref(pointee, _) | Ty::Ptr(pointee, _) => *pointee,
        _ => return TypeRecovery::Malformed,
    };
    if pointee.index() >= model.types.len() {
        return TypeRecovery::Malformed;
    }
    match model.types.resolve(pointee) {
        Ty::Dynamic { vtable_ty, .. } => {
            find_or_not_applicable(model, &Ty::Ptr(*vtable_ty, Mutability::Not))
        }
        Ty::Slice(_) | Ty::Str => find_or_not_applicable(model, &Ty::Uint(UintKind::Usize)),
        _ => TypeRecovery::Malformed,
    }
}

/// Returns the variant count for `place_ty(p)` when `p` resolves to
/// `Ty::Enum`. Used by statement invariant B-2.
///
/// `NotApplicable` is returned for non-enum types (a separate invariant
/// rejects non-enum `SetDiscriminant.place`). `Malformed` is forwarded
/// from `place_ty`.
pub fn place_enum_variant_count(
    model: &RegseqModel,
    body: &Body,
    place: &Place,
) -> VariantRecovery {
    let ty = match place_ty(model, body, place) {
        TypeRecovery::Recovered(ty) => ty,
        TypeRecovery::NotApplicable => return VariantRecovery::NotApplicable,
        TypeRecovery::Malformed => return VariantRecovery::Malformed,
    };
    match model.types.resolve(ty) {
        Ty::Enum { layout, .. } => match &layout.kind {
            LayoutKind::Enum { shape } => VariantRecovery::Recovered(variant_count_of(shape)),
            _ => VariantRecovery::Malformed,
        },
        _ => VariantRecovery::NotEnum(ty),
    }
}

/// Outcome of a variant-count recovery against `place_ty`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariantRecovery {
    /// Variant count for the enum that `place_ty` resolved to.
    Recovered(u32),
    /// `place_ty` resolved cleanly but to a non-enum type; the
    /// embedded `TyId` is the resolved place type, used by callers to
    /// emit `SetDiscriminantNonEnumTarget`.
    NotEnum(TyId),
    /// `place_ty` does not apply at this site.
    NotApplicable,
    /// `place_ty` could not be recovered because the body is
    /// malformed.
    Malformed,
}

fn variant_count_of(shape: &EnumShape) -> u32 {
    match shape {
        EnumShape::Empty => 0,
        EnumShape::Single { .. } => 1,
        EnumShape::Multiple { variants, .. } => variants.len() as u32,
    }
}

/// Returns the resolved pointee type for `Ty::Ref` or `Ty::Ptr`.
///
/// `NotApplicable` is returned when `ty` is not a pointer-like type
/// (the spec rule does not apply); `Malformed` for an out-of-range
/// `TyId`.
pub fn pointee_ty(model: &RegseqModel, ty: TyId) -> TypeRecovery {
    if ty.index() >= model.types.len() {
        return TypeRecovery::Malformed;
    }
    match model.types.resolve(ty) {
        Ty::Ref(inner, _) | Ty::Ptr(inner, _) => TypeRecovery::Recovered(*inner),
        _ => TypeRecovery::NotApplicable,
    }
}
