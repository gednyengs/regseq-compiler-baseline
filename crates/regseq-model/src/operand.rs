//! MIR operand data shapes.
//!
//! `Operand` is body-local content and is stored inline at use
//! sites. There is no `OperandId` and no top-level operand interner;
//! equality and hashing are structural over the variant payload.
//! Operand-site validation is deferred to the body validation stages.

use serde::{Deserialize, Serialize};

use crate::ids::ConstId;
use crate::place::Place;

/// Literal-value vocabulary used by operand-context consumers.
///
/// The literal-value surface is the existing `Const` enum. This alias
/// provides a focused import path for consumers that render or inspect
/// constants reached through `Operand::Constant`, without introducing
/// a separate literal-value type.
pub use crate::const_value::Const as LiteralValue;

/// A typed RHS atom: either a load from a place, or a constant value.
///
/// `Operand` is body-local content; it appears inside a body's
/// rvalues, statements, or terminators rather than in any top-level
/// interner table. Equality is structural. Worst-case inline storage
/// is a `Place` payload plus the enum discriminator; this type does
/// not use rustc's `Box<ConstOperand>` wrapper and does not pin an
/// explicit representation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Operand {
    /// Load a value from the place.
    ///
    /// Marks the place as participating in a copy-style read.
    /// Semantically equivalent to `Move` after drop-elaboration in
    /// `optimized_mir`, but distinguishable for backend lowering such
    /// as debug-info annotation or copy-vs-memcpy dispatch.
    Copy(Place),

    /// Load a value from the place.
    ///
    /// Marks the place as participating in a move-style consumption.
    /// At rustc's `Call` argument site this enables in-place passing
    /// semantics; elsewhere in `optimized_mir` this is operationally
    /// equivalent to `Copy`.
    Move(Place),

    /// A constant value referenced by its `ConstId`.
    ///
    /// The `ConstId` indexes into the model's `ConstInterner`. This
    /// flat shape drops rustc's `Box<ConstOperand>` wrapper, `user_ty`,
    /// and operand-level span; use-site spans belong to the containing
    /// statement or terminator data.
    Constant(ConstId),
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::Operand;
    use crate::operand::LiteralValue;
    use crate::{Const, ConstId, FieldIdx, LocalId, Place, PlaceElem, TyId, VariantIdx};

    fn bare_place(local: u32) -> Place {
        Place::from(LocalId::from_u32(local))
    }

    fn projected_place() -> Place {
        Place {
            local: LocalId::from_u32(2),
            projection: Box::new([
                PlaceElem::Deref,
                PlaceElem::Field(FieldIdx::from_u32(1), TyId::from_u32(9)),
                PlaceElem::Downcast(VariantIdx::from_u32(3)),
                PlaceElem::Field(FieldIdx::from_u32(0), TyId::from_u32(10)),
            ]),
        }
    }

    fn round_trip(value: &Operand) -> Operand {
        let json = serde_json::to_string(value).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn copy_operand_constructs_with_bare_local_place() {
        let place = Place::from(LocalId::ZERO);
        let operand = Operand::Copy(place.clone());

        assert!(matches!(operand, Operand::Copy(actual) if actual == place));
    }

    #[test]
    fn move_operand_constructs_with_non_trivial_projection() {
        let place = projected_place();
        let operand = Operand::Move(place.clone());

        assert!(matches!(operand, Operand::Move(actual) if actual == place));
    }

    #[test]
    fn constant_operand_constructs_with_const_id() {
        let operand = Operand::Constant(ConstId::from_u32(7));

        assert!(matches!(
            operand,
            Operand::Constant(id) if id == ConstId::from_u32(7)
        ));
    }

    #[test]
    fn operand_variants_are_pairwise_distinct() {
        let copy = Operand::Copy(bare_place(0));
        let move_ = Operand::Move(bare_place(1));
        let constant = Operand::Constant(ConstId::from_u32(7));

        assert_ne!(copy, move_);
        assert_ne!(copy, constant);
        assert_ne!(move_, constant);
    }

    #[test]
    fn copy_and_move_are_distinct_over_same_place() {
        let place = projected_place();

        assert_ne!(Operand::Copy(place.clone()), Operand::Move(place));
    }

    #[test]
    fn copy_operands_with_identical_place_are_equal() {
        let place = projected_place();

        assert_eq!(Operand::Copy(place.clone()), Operand::Copy(place));
    }

    #[test]
    fn copy_operands_with_different_places_are_not_equal() {
        let left = Operand::Copy(bare_place(0));
        let right = Operand::Copy(bare_place(1));

        assert_ne!(left, right);
    }

    #[test]
    fn constant_operands_with_identical_const_id_are_equal() {
        let left = Operand::Constant(ConstId::from_u32(7));
        let right = Operand::Constant(ConstId::from_u32(7));

        assert_eq!(left, right);
    }

    #[test]
    fn constant_operands_with_different_const_ids_are_not_equal() {
        let left = Operand::Constant(ConstId::from_u32(7));
        let right = Operand::Constant(ConstId::from_u32(8));

        assert_ne!(left, right);
    }

    #[test]
    fn operand_hash_discriminates_variants_and_payloads() {
        let values = [
            Operand::Copy(projected_place()),
            Operand::Move(projected_place()),
            Operand::Constant(ConstId::from_u32(7)),
        ];
        let mut set = HashSet::new();

        for value in values.iter().cloned() {
            assert!(set.insert(value));
        }
        assert_eq!(set.len(), 3);

        for value in values {
            assert!(!set.insert(value));
        }
        assert_eq!(set.len(), 3);
    }

    #[test]
    fn operand_clone_preserves_equality_for_each_variant() {
        for value in [
            Operand::Copy(bare_place(0)),
            Operand::Move(projected_place()),
            Operand::Constant(ConstId::from_u32(7)),
        ] {
            assert_eq!(value.clone(), value);
        }
    }

    #[test]
    fn serde_round_trips_copy_operand() {
        let operand = Operand::Copy(bare_place(0));

        assert_eq!(round_trip(&operand), operand);
    }

    #[test]
    fn serde_round_trips_move_operand() {
        let operand = Operand::Move(projected_place());

        assert_eq!(round_trip(&operand), operand);
    }

    #[test]
    fn serde_round_trips_constant_operand() {
        let operand = Operand::Constant(ConstId::from_u32(7));

        assert_eq!(round_trip(&operand), operand);
    }

    #[test]
    fn serde_round_trips_copy_operand_with_non_trivial_projection() {
        let operand = Operand::Copy(projected_place());

        assert_eq!(round_trip(&operand), operand);
    }

    #[test]
    fn literal_value_re_export_resolves_to_const() {
        let value: LiteralValue = LiteralValue::Bool(true);
        let expected: Const = Const::Bool(true);

        assert_eq!(value, expected);
    }

    const _: () = assert!(core::mem::size_of::<ConstId>() == 4);
}
