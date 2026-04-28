//! Binary and unary operator enums for the Regseq IR.

use serde::{Deserialize, Serialize};

/// Binary operator.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum BinOp {
    // Arithmetic -- wrapping
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    // Arithmetic -- UB on overflow (unchecked)
    AddUnchecked,
    SubUnchecked,
    MulUnchecked,
    // Arithmetic -- returns (result, overflow_bool)
    AddWithOverflow,
    SubWithOverflow,
    MulWithOverflow,
    // Bitwise
    BitXor,
    BitAnd,
    BitOr,
    // Shifts
    Shl,
    Shr,
    ShlUnchecked,
    ShrUnchecked,
    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // Three-way comparison
    Cmp,
    // Pointer arithmetic
    Offset,
}

/// Unary operator.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum UnOp {
    Not,
    Neg,
}

// All tests in this module exercise post-HIR/MIR output (constructed model,
// JSON serialization, snapshots). Voided pending MonoRust + RegseqModel
// redesign. See docs/discussions/compiler-refactoring/test-audit.md.
#[cfg(all(test, feature = "post-monorust-tests"))]
mod tests {
    use super::*;

    // -- Construction: all 26 BinOp variants --

    #[test]
    fn construct_all_binop_variants() {
        let variants = [
            BinOp::Add,
            BinOp::Sub,
            BinOp::Mul,
            BinOp::Div,
            BinOp::Rem,
            BinOp::AddUnchecked,
            BinOp::SubUnchecked,
            BinOp::MulUnchecked,
            BinOp::AddWithOverflow,
            BinOp::SubWithOverflow,
            BinOp::MulWithOverflow,
            BinOp::BitXor,
            BinOp::BitAnd,
            BinOp::BitOr,
            BinOp::Shl,
            BinOp::Shr,
            BinOp::ShlUnchecked,
            BinOp::ShrUnchecked,
            BinOp::Eq,
            BinOp::Ne,
            BinOp::Lt,
            BinOp::Le,
            BinOp::Gt,
            BinOp::Ge,
            BinOp::Cmp,
            BinOp::Offset,
        ];
        assert_eq!(variants.len(), 26);
    }

    // -- Construction: both UnOp variants --

    #[test]
    fn construct_all_unop_variants() {
        let variants = [UnOp::Not, UnOp::Neg];
        assert_eq!(variants.len(), 2);
    }

    // -- Equality --

    #[test]
    fn binop_equality() {
        assert_eq!(BinOp::Add, BinOp::Add);
        assert_ne!(BinOp::Add, BinOp::Sub);
    }

    #[test]
    fn unop_equality() {
        assert_eq!(UnOp::Not, UnOp::Not);
        assert_ne!(UnOp::Not, UnOp::Neg);
    }

    // -- Copy semantics --

    #[test]
    fn binop_copy_semantics() {
        let a = BinOp::BitAnd;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn unop_copy_semantics() {
        let a = UnOp::Neg;
        let b = a;
        assert_eq!(a, b);
    }

    // -- Serde round-trip: every BinOp variant --

    #[test]
    fn serde_round_trip_all_binop_variants() {
        let variants = [
            BinOp::Add,
            BinOp::Sub,
            BinOp::Mul,
            BinOp::Div,
            BinOp::Rem,
            BinOp::AddUnchecked,
            BinOp::SubUnchecked,
            BinOp::MulUnchecked,
            BinOp::AddWithOverflow,
            BinOp::SubWithOverflow,
            BinOp::MulWithOverflow,
            BinOp::BitXor,
            BinOp::BitAnd,
            BinOp::BitOr,
            BinOp::Shl,
            BinOp::Shr,
            BinOp::ShlUnchecked,
            BinOp::ShrUnchecked,
            BinOp::Eq,
            BinOp::Ne,
            BinOp::Lt,
            BinOp::Le,
            BinOp::Gt,
            BinOp::Ge,
            BinOp::Cmp,
            BinOp::Offset,
        ];
        for v in &variants {
            let json = serde_json::to_string(v).unwrap();
            assert_eq!(*v, serde_json::from_str::<BinOp>(&json).unwrap());
        }
    }

    // -- Serde round-trip: every UnOp variant --

    #[test]
    fn serde_round_trip_all_unop_variants() {
        for v in &[UnOp::Not, UnOp::Neg] {
            let json = serde_json::to_string(v).unwrap();
            assert_eq!(*v, serde_json::from_str::<UnOp>(&json).unwrap());
        }
    }

    // -- Serde format: adjacently-tagged --

    #[test]
    fn serde_adjacently_tagged_binop() {
        let json = serde_json::to_string(&BinOp::Add).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["kind"], "Add");
        assert!(v.get("data").is_none());
    }

    #[test]
    fn serde_adjacently_tagged_unop() {
        let json = serde_json::to_string(&UnOp::Not).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["kind"], "Not");
        assert!(v.get("data").is_none());
    }
}
