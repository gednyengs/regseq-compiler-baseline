//! Operand and LiteralValue types for the Regseq Model IR.
//!
//! An [`Operand`] is a value source for expressions -- the atomic unit
//! that flows into rvalues, statements, and terminators. A
//! [`LiteralValue`] is an inline scalar or boolean constant that carries
//! its value directly.

use serde::{Deserialize, Serialize};

use crate::identity::ConstantId;
use crate::ir::place::Place;

/// An operand in the Regseq IR -- a value source for expressions.
#[derive(Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum Operand {
    /// Load a value from a place (bitwise copy).
    Copy(Place),
    /// Load a value from a place, invalidating the source.
    Move(Place),
    /// Reference to a named constant declaration.
    Constant(ConstantId),
    /// Inline scalar or boolean value.
    Literal(LiteralValue),
}

/// Inline constant values that appear directly in expressions.
///
/// Each variant carries its value and implies a `RegseqType`:
/// `U32(v)` has type `Scalar(U32)`, `Bool(v)` has type `Bool`.
///
/// `Usize(u64)` and `Isize(i64)` use 64-bit backing storage because the
/// actual width is target-dependent. The backend narrows to the target's
/// pointer width.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum LiteralValue {
    /// Unsigned 8-bit integer.
    U8(u8),
    /// Unsigned 16-bit integer.
    U16(u16),
    /// Unsigned 32-bit integer.
    U32(u32),
    /// Unsigned 64-bit integer.
    U64(u64),
    /// Unsigned pointer-width integer (64-bit backing).
    Usize(u64),
    /// Signed 8-bit integer.
    I8(i8),
    /// Signed 16-bit integer.
    I16(i16),
    /// Signed 32-bit integer.
    I32(i32),
    /// Signed 64-bit integer.
    I64(i64),
    /// Signed pointer-width integer (64-bit backing).
    Isize(i64),
    /// Boolean value.
    Bool(bool),
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;
    use crate::identity::{DeclId, LocalId};

    // -- Construction: each of the 4 Operand variants --

    #[test]
    fn construct_operand_copy() {
        let place = Place {
            local: LocalId(0),
            projection: vec![],
        };
        let _ = Operand::Copy(place);
    }

    #[test]
    fn construct_operand_move() {
        let place = Place {
            local: LocalId(1),
            projection: vec![],
        };
        let _ = Operand::Move(place);
    }

    #[test]
    fn construct_operand_constant() {
        let _ = Operand::Constant(ConstantId::from(DeclId::new(5)));
    }

    #[test]
    fn construct_operand_literal() {
        let _ = Operand::Literal(LiteralValue::U32(42));
    }

    // -- Construction: each of the 11 LiteralValue variants --

    #[test]
    fn construct_literal_u8() {
        let _ = LiteralValue::U8(255);
    }

    #[test]
    fn construct_literal_u16() {
        let _ = LiteralValue::U16(0xFFFF);
    }

    #[test]
    fn construct_literal_u32() {
        let _ = LiteralValue::U32(0xDEAD_BEEF);
    }

    #[test]
    fn construct_literal_u64() {
        let _ = LiteralValue::U64(0xDEAD_BEEF_CAFE_BABE);
    }

    #[test]
    fn construct_literal_usize() {
        let _ = LiteralValue::Usize(1024);
    }

    #[test]
    fn construct_literal_i8() {
        let _ = LiteralValue::I8(-1);
    }

    #[test]
    fn construct_literal_i16() {
        let _ = LiteralValue::I16(-32768);
    }

    #[test]
    fn construct_literal_i32() {
        let _ = LiteralValue::I32(-100);
    }

    #[test]
    fn construct_literal_i64() {
        let _ = LiteralValue::I64(i64::MIN);
    }

    #[test]
    fn construct_literal_isize() {
        let _ = LiteralValue::Isize(-42);
    }

    #[test]
    fn construct_literal_bool() {
        let _ = LiteralValue::Bool(true);
    }

    // -- LiteralValue is Copy --

    #[test]
    fn literal_value_is_copy() {
        let a = LiteralValue::U32(42);
        let b = a; // copy, not move
        assert_eq!(a, b);
    }

    // -- Equality --

    #[test]
    fn same_literal_values_are_equal() {
        assert_eq!(LiteralValue::U32(42), LiteralValue::U32(42));
    }

    #[test]
    fn different_literal_values_same_variant_are_unequal() {
        assert_ne!(LiteralValue::U32(42), LiteralValue::U32(43));
    }

    #[test]
    fn same_numeric_value_different_variant_is_unequal() {
        assert_ne!(LiteralValue::U32(42), LiteralValue::I32(42));
    }
}
