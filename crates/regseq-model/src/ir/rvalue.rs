//! Rvalue and supporting types for the Regseq Model IR.
//!
//! An [`Rvalue`] is a value-producing expression on the right side of
//! an assignment. Rvalues are purely standard -- arithmetic, logic,
//! casts, aggregates, references. All domain-specific operations flow
//! through `IntrinsicCall` terminators.

use serde::{Deserialize, Serialize};

use crate::identity::TypeDefId;
use crate::ir::operand::Operand;
use crate::ir::ops::{BinOp, UnOp};
use crate::ir::place::Place;
use crate::ir::types::RegseqType;

/// A value-producing expression on the right side of an assignment.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum Rvalue {
    /// Pass an operand through unchanged.
    Use(Operand),

    /// Array repetition: `[operand; count]`.
    Repeat(Operand, usize),

    /// Create a reference to a place.
    Ref(BorrowKind, Place),

    /// Type cast.
    Cast(CastKind, Operand, RegseqType),

    /// Binary operation.
    BinaryOp(BinOp, Operand, Operand),

    /// Unary operation.
    UnaryOp(UnOp, Operand),

    /// Read the discriminant (variant tag) of an enum-typed place.
    Discriminant(Place),

    /// Construct an aggregate value (struct, enum variant, tuple, array).
    Aggregate(AggregateKind, Vec<Operand>),
}

/// Borrow kind for the `Ref` rvalue.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum BorrowKind {
    /// Shared (immutable) borrow: `&T`.
    Shared,
    /// Mutable borrow: `&mut T`.
    Mutable,
}

/// Cast kind for the `Cast` rvalue.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum CastKind {
    /// Integer to integer (widening, narrowing, sign change).
    IntToInt,
    /// Integer to float.
    IntToFloat,
    /// Float to integer.
    FloatToInt,
    /// Float to float (widening or narrowing).
    FloatToFloat,
    /// Pointer to pointer (raw pointer casts).
    PtrToPtr,
    /// Transmute (bit reinterpretation).
    Transmute,
}

/// Aggregate kind for the `Aggregate` rvalue.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum AggregateKind {
    /// Array literal with element type.
    Array(RegseqType),
    /// Tuple construction.
    Tuple,
    /// Struct or enum variant construction.
    Adt {
        /// The type definition this aggregate constructs.
        type_id: TypeDefId,
        /// Variant index (0 for structs).
        variant_index: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{DeclId, LocalId};
    use crate::ir::operand::LiteralValue;
    use crate::ir::types::ScalarType;

    // -- Construction: each of the 8 Rvalue variants --

    #[test]
    fn construct_rvalue_use() {
        let _ = Rvalue::Use(Operand::Literal(LiteralValue::U32(1)));
    }

    #[test]
    fn construct_rvalue_repeat() {
        let _ = Rvalue::Repeat(Operand::Literal(LiteralValue::U8(0)), 16);
    }

    #[test]
    fn construct_rvalue_ref() {
        let place = Place {
            local: LocalId(0),
            projection: vec![],
        };
        let _ = Rvalue::Ref(BorrowKind::Shared, place);
    }

    #[test]
    fn construct_rvalue_cast() {
        let _ = Rvalue::Cast(
            CastKind::IntToInt,
            Operand::Literal(LiteralValue::U8(255)),
            RegseqType::Scalar(ScalarType::U32),
        );
    }

    #[test]
    fn construct_rvalue_binary_op() {
        let _ = Rvalue::BinaryOp(
            BinOp::Add,
            Operand::Literal(LiteralValue::U32(1)),
            Operand::Literal(LiteralValue::U32(2)),
        );
    }

    #[test]
    fn construct_rvalue_unary_op() {
        let _ = Rvalue::UnaryOp(UnOp::Not, Operand::Literal(LiteralValue::Bool(true)));
    }

    #[test]
    fn construct_rvalue_discriminant() {
        let place = Place {
            local: LocalId(3),
            projection: vec![],
        };
        let _ = Rvalue::Discriminant(place);
    }

    #[test]
    fn construct_rvalue_aggregate() {
        let _ = Rvalue::Aggregate(
            AggregateKind::Tuple,
            vec![
                Operand::Literal(LiteralValue::U32(1)),
                Operand::Literal(LiteralValue::Bool(true)),
            ],
        );
    }

    // -- Construction: each BorrowKind variant --

    #[test]
    fn construct_borrow_kind_shared() {
        let _ = BorrowKind::Shared;
    }

    #[test]
    fn construct_borrow_kind_mutable() {
        let _ = BorrowKind::Mutable;
    }

    // -- Construction: each CastKind variant --

    #[test]
    fn construct_all_cast_kind_variants() {
        let _ = CastKind::IntToInt;
        let _ = CastKind::IntToFloat;
        let _ = CastKind::FloatToInt;
        let _ = CastKind::FloatToFloat;
        let _ = CastKind::PtrToPtr;
        let _ = CastKind::Transmute;
    }

    // -- Construction: each AggregateKind variant --

    #[test]
    fn construct_aggregate_kind_array() {
        let _ = AggregateKind::Array(RegseqType::Scalar(ScalarType::U32));
    }

    #[test]
    fn construct_aggregate_kind_tuple() {
        let _ = AggregateKind::Tuple;
    }

    #[test]
    fn construct_aggregate_kind_adt() {
        let _ = AggregateKind::Adt {
            type_id: TypeDefId::from(DeclId::new(10)),
            variant_index: 0,
        };
    }

    // -- Composition: BinaryOp with Operand --

    #[test]
    fn binary_op_add_composes_with_operands() {
        let lhs = Operand::Literal(LiteralValue::U32(10));
        let rhs = Operand::Literal(LiteralValue::U32(20));
        let rv = Rvalue::BinaryOp(BinOp::Add, lhs.clone(), rhs.clone());
        assert_eq!(rv, Rvalue::BinaryOp(BinOp::Add, lhs, rhs));
    }

    // -- Composition: Aggregate Adt for struct construction --

    #[test]
    fn aggregate_adt_struct_construction() {
        let type_id = TypeDefId::from(DeclId::new(42));
        let fields = vec![
            Operand::Literal(LiteralValue::U32(0x1000)),
            Operand::Literal(LiteralValue::Bool(false)),
        ];
        let rv = Rvalue::Aggregate(
            AggregateKind::Adt {
                type_id,
                variant_index: 0,
            },
            fields,
        );
        match &rv {
            Rvalue::Aggregate(AggregateKind::Adt { variant_index, .. }, ops) => {
                assert_eq!(*variant_index, 0);
                assert_eq!(ops.len(), 2);
            }
            _ => panic!("expected Aggregate(Adt, ...)"),
        }
    }
}
