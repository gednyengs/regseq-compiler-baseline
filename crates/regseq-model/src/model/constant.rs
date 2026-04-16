//! Constant model type.
//!
//! [`Constant`] models both top-level constant items (see spec.md
//! Section 3.2) and associated constants defined inside impl blocks
//! (see spec.md Section 6). A single type covers both use sites;
//! ownership is attached by the containing [`Module`] or [`ImplBlock`]
//! in later tasks.

use crate::identity::{ConstantId, Symbol};
use crate::ir::{LiteralValue, RegseqType};

/// A named constant or static item.
///
/// Used for both top-level constants (Section 3.2) and associated
/// constants in impl blocks (Section 6). Non-literal initializers are
/// not supported; `value` is always a [`LiteralValue`].
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Constant {
    /// The constant's declaration identifier.
    pub id: ConstantId,
    /// The constant's identifier as an interned symbol.
    pub name: Symbol,
    /// The constant's type.
    pub ty: RegseqType,
    /// The constant's value.
    pub value: LiteralValue,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{DeclId, SymbolInterner};
    use crate::ir::ScalarType;

    fn sym(interner: &mut SymbolInterner, s: &str) -> Symbol {
        interner.intern(s)
    }

    #[test]
    fn constructs_constant_with_u32_literal() {
        let mut interner = SymbolInterner::new();
        let name = sym(&mut interner, "BAUD_RATE");

        let c = Constant {
            id: ConstantId::from(DeclId::new(1)),
            name,
            ty: RegseqType::Scalar(ScalarType::U32),
            value: LiteralValue::U32(0x07ff),
        };

        assert_eq!(c.value, LiteralValue::U32(0x07ff));
        assert_eq!(c.ty, RegseqType::Scalar(ScalarType::U32));
    }

    #[test]
    fn constructs_constant_with_bool_literal() {
        let mut interner = SymbolInterner::new();
        let name = sym(&mut interner, "FEATURE_ENABLED");

        let c = Constant {
            id: ConstantId::from(DeclId::new(2)),
            name,
            ty: RegseqType::Bool,
            value: LiteralValue::Bool(true),
        };

        assert_eq!(c.value, LiteralValue::Bool(true));
        assert_eq!(c.ty, RegseqType::Bool);
    }

    #[test]
    fn two_constants_with_same_fields_are_equal() {
        let mut interner = SymbolInterner::new();
        let name = sym(&mut interner, "X");
        let id = ConstantId::from(DeclId::new(3));

        let a = Constant {
            id,
            name,
            ty: RegseqType::Scalar(ScalarType::U32),
            value: LiteralValue::U32(42),
        };
        let b = Constant {
            id,
            name,
            ty: RegseqType::Scalar(ScalarType::U32),
            value: LiteralValue::U32(42),
        };

        assert_eq!(a, b);
    }

    #[test]
    fn constants_with_different_values_are_unequal() {
        let mut interner = SymbolInterner::new();
        let name = sym(&mut interner, "X");
        let id = ConstantId::from(DeclId::new(4));

        let a = Constant {
            id,
            name,
            ty: RegseqType::Scalar(ScalarType::U32),
            value: LiteralValue::U32(1),
        };
        let b = Constant {
            id,
            name,
            ty: RegseqType::Scalar(ScalarType::U32),
            value: LiteralValue::U32(2),
        };

        assert_ne!(a, b);
    }
}
