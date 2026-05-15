//! MIR rvalue data shapes.
//!
//! `Rvalue` is body-local content and is stored inline at use sites.
//! There is no `RvalueId` and no top-level rvalue interner; equality
//! and hashing are structural over the variant payload.

use serde::{Deserialize, Serialize};

use crate::ids::{FieldIdx, TyId, VariantIdx};
use crate::operand::Operand;
use crate::place::Place;
use crate::ty::{Mutability, Provenance};

/// A typed RHS expression: a value-producing computation.
///
/// Body-local content; appears inside a body's statements, not in any
/// top-level interner table. Equality is structural.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rvalue {
    /// Identity: yields the operand's value unchanged.
    ///
    /// Payload shape is defined by spec.md section 21.2.1.
    Use(Operand),

    /// Array constructor: `[x; N]`.
    ///
    /// Payload shape is defined by spec.md section 21.2.2. The
    /// `u64` count is the post-monomorphisation array length.
    Repeat(Operand, u64),

    /// Borrow construction: `&p` or `&mut p`.
    ///
    /// Payload shape is defined by spec.md section 21.2.3. The
    /// `Provenance` discriminator reuses the type-system borrow
    /// flavor enum.
    Ref(Provenance, Place),

    /// Raw pointer construction: `&raw const p` or `&raw mut p`.
    ///
    /// Payload shape is defined by spec.md section 21.2.4. The
    /// `Mutability` discriminator reuses the type-system raw-pointer
    /// mutability enum.
    RawPtr(Mutability, Place),

    /// `as`-style cast or compiler-inserted coercion.
    ///
    /// Payload shape is defined by spec.md section 21.2.5. The
    /// destination `TyId` resolves through the type interner.
    Cast(CastKind, Operand, TyId),

    /// Binary arithmetic, comparison, or pointer offset.
    ///
    /// Payload shape is defined by spec.md section 21.2.6. Operands
    /// are stored inline rather than behind a boxed pair.
    BinaryOp(BinOp, Operand, Operand),

    /// Unary operation: `!x`, `-x`, or pointer-metadata extraction.
    ///
    /// Payload shape is defined by spec.md section 21.2.7.
    UnaryOp(UnOp, Operand),

    /// Read the discriminant of an enum at the place.
    ///
    /// Payload shape is defined by spec.md section 21.2.8. For
    /// non-enum predecessor types this yields zero.
    Discriminant(Place),

    /// Aggregate value construction.
    ///
    /// Payload shape is defined by spec.md section 21.2.9. Operand
    /// storage is a boxed slice.
    Aggregate(AggregateKind, Box<[Operand]>),
}

/// Closed cast vocabulary for `Rvalue::Cast`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CastKind {
    /// Expose pointer provenance as an integer.
    PointerExposeProvenance,
    /// Reattach previously exposed pointer provenance.
    PointerWithExposedProvenance,
    /// Pointer coercion with diagnostic-only source data dropped.
    PointerCoercion(PointerCoercion),
    /// Integer-to-integer cast.
    IntToInt,
    /// Floating-point-to-integer cast.
    FloatToInt,
    /// Floating-point-to-floating-point cast.
    FloatToFloat,
    /// Integer-to-floating-point cast.
    IntToFloat,
    /// Pointer-to-pointer cast.
    PtrToPtr,
    /// Function-pointer-to-pointer cast.
    FnPtrToPtr,
    /// Transmute cast.
    Transmute,
    /// Subtype coercion.
    Subtype,
}

/// Closed pointer-coercion vocabulary nested under `CastKind`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PointerCoercion {
    /// Reify a function item to a function pointer.
    ReifyFnPointer,
    /// Coerce a safe function pointer to an unsafe function pointer.
    UnsafeFnPointer,
    /// Coerce a closure to a function pointer.
    ClosureFnPointer,
    /// Unsizing pointer coercion.
    Unsize,
}

/// Closed binary-operation vocabulary for `Rvalue::BinaryOp`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BinOp {
    /// Wrapping addition.
    Add,
    /// Wrapping subtraction.
    Sub,
    /// Wrapping multiplication.
    Mul,
    /// Division.
    Div,
    /// Remainder.
    Rem,
    /// Unchecked addition.
    AddUnchecked,
    /// Unchecked subtraction.
    SubUnchecked,
    /// Unchecked multiplication.
    MulUnchecked,
    /// Overflow-detecting addition.
    AddWithOverflow,
    /// Overflow-detecting subtraction.
    SubWithOverflow,
    /// Overflow-detecting multiplication.
    MulWithOverflow,
    /// Bitwise exclusive-or.
    BitXor,
    /// Bitwise and.
    BitAnd,
    /// Bitwise or.
    BitOr,
    /// Wrapping shift left.
    Shl,
    /// Unchecked shift left.
    ShlUnchecked,
    /// Wrapping shift right.
    Shr,
    /// Unchecked shift right.
    ShrUnchecked,
    /// Equality comparison.
    Eq,
    /// Less-than comparison.
    Lt,
    /// Less-than-or-equal comparison.
    Le,
    /// Inequality comparison.
    Ne,
    /// Greater-than-or-equal comparison.
    Ge,
    /// Greater-than comparison.
    Gt,
    /// Three-way comparison returning `core::cmp::Ordering`.
    Cmp,
    /// Pointer offset with `<*const T>::offset` semantics.
    Offset,
}

/// Closed unary-operation vocabulary for `Rvalue::UnaryOp`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnOp {
    /// Logical inversion.
    Not,
    /// Arithmetic negation.
    Neg,
    /// Metadata extraction from a fat pointer.
    PtrMetadata,
}

/// Closed aggregate-constructor vocabulary for `Rvalue::Aggregate`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AggregateKind {
    /// Fixed-length array literal.
    ///
    /// The `TyId` resolves to `Ty::Array(elem_ty, n)`.
    Array(TyId),

    /// Tuple constructor.
    ///
    /// The `TyId` resolves to `Ty::Tuple { fields, layout }`.
    Tuple(TyId),

    /// Struct constructor.
    ///
    /// The `TyId` resolves to `Ty::Struct { item, args, layout }`.
    Struct(TyId),

    /// Enum variant constructor.
    ///
    /// The `TyId` resolves to `Ty::Enum { item, args, layout }`;
    /// `VariantIdx` selects the active variant.
    Enum(TyId, VariantIdx),

    /// Union constructor.
    ///
    /// The `TyId` resolves to `Ty::Union { item, args, layout }`;
    /// `FieldIdx` selects the active field.
    Union(TyId, FieldIdx),

    /// Closure environment construction.
    ///
    /// The `TyId` resolves to `Ty::Closure(item, args)`.
    Closure(TyId),

    /// Fat raw pointer construction from data pointer and metadata.
    ///
    /// The `TyId` is the pointee type and `Mutability` is the raw
    /// pointer mutability.
    RawPtr(TyId, Mutability),
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{AggregateKind, BinOp, CastKind, PointerCoercion, Rvalue, UnOp};
    use crate::{
        ConstId, FieldIdx, LocalId, Mutability, Operand, Place, Provenance, TyId, VariantIdx,
    };

    fn const_operand(index: u32) -> Operand {
        Operand::Constant(ConstId::from_u32(index))
    }

    fn bare_place(index: u32) -> Place {
        Place::from(LocalId::from_u32(index))
    }

    fn round_trip<T>(value: &T) -> T
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        let json = serde_json::to_string(value).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    fn all_cast_kinds() -> [CastKind; 11] {
        [
            CastKind::PointerExposeProvenance,
            CastKind::PointerWithExposedProvenance,
            CastKind::PointerCoercion(PointerCoercion::Unsize),
            CastKind::IntToInt,
            CastKind::FloatToInt,
            CastKind::FloatToFloat,
            CastKind::IntToFloat,
            CastKind::PtrToPtr,
            CastKind::FnPtrToPtr,
            CastKind::Transmute,
            CastKind::Subtype,
        ]
    }

    fn all_pointer_coercions() -> [PointerCoercion; 4] {
        [
            PointerCoercion::ReifyFnPointer,
            PointerCoercion::UnsafeFnPointer,
            PointerCoercion::ClosureFnPointer,
            PointerCoercion::Unsize,
        ]
    }

    fn all_bin_ops() -> [BinOp; 26] {
        [
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
            BinOp::ShlUnchecked,
            BinOp::Shr,
            BinOp::ShrUnchecked,
            BinOp::Eq,
            BinOp::Lt,
            BinOp::Le,
            BinOp::Ne,
            BinOp::Ge,
            BinOp::Gt,
            BinOp::Cmp,
            BinOp::Offset,
        ]
    }

    fn all_un_ops() -> [UnOp; 3] {
        [UnOp::Not, UnOp::Neg, UnOp::PtrMetadata]
    }

    fn all_aggregate_kinds() -> [AggregateKind; 7] {
        [
            AggregateKind::Array(TyId::from_u32(0)),
            AggregateKind::Tuple(TyId::from_u32(0)),
            AggregateKind::Struct(TyId::from_u32(0)),
            AggregateKind::Enum(TyId::from_u32(0), VariantIdx::from_u32(0)),
            AggregateKind::Union(TyId::from_u32(0), FieldIdx::from_u32(0)),
            AggregateKind::Closure(TyId::from_u32(0)),
            AggregateKind::RawPtr(TyId::from_u32(0), Mutability::Not),
        ]
    }

    fn representative_rvalues() -> [Rvalue; 9] {
        [
            Rvalue::Use(const_operand(0)),
            Rvalue::Repeat(const_operand(1), 4),
            Rvalue::Ref(Provenance::SharedBorrow, bare_place(0)),
            Rvalue::RawPtr(Mutability::Not, bare_place(1)),
            Rvalue::Cast(CastKind::IntToInt, const_operand(2), TyId::from_u32(0)),
            Rvalue::BinaryOp(BinOp::Add, const_operand(3), const_operand(4)),
            Rvalue::UnaryOp(UnOp::Not, const_operand(5)),
            Rvalue::Discriminant(bare_place(2)),
            Rvalue::Aggregate(AggregateKind::Tuple(TyId::from_u32(1)), Box::new([])),
        ]
    }

    #[test]
    fn use_rvalue_constructs() {
        let rvalue = Rvalue::Use(Operand::Constant(ConstId::from_u32(0)));

        assert!(matches!(
            rvalue,
            Rvalue::Use(Operand::Constant(id)) if id == ConstId::from_u32(0)
        ));
    }

    #[test]
    fn repeat_rvalue_constructs_with_count() {
        let operand = const_operand(0);
        let rvalue = Rvalue::Repeat(operand.clone(), 8);

        assert!(matches!(rvalue, Rvalue::Repeat(actual, 8) if actual == operand));
    }

    #[test]
    fn ref_rvalue_constructs_for_each_provenance() {
        let place = bare_place(0);
        let shared = Rvalue::Ref(Provenance::SharedBorrow, place.clone());
        let unique = Rvalue::Ref(Provenance::UniqueBorrow, place);

        assert_ne!(shared, unique);
    }

    #[test]
    fn raw_ptr_rvalue_constructs_for_each_mutability() {
        let place = bare_place(0);
        let not = Rvalue::RawPtr(Mutability::Not, place.clone());
        let mut_ = Rvalue::RawPtr(Mutability::Mut, place);

        assert_ne!(not, mut_);
    }

    #[test]
    fn cast_rvalue_constructs_for_each_cast_kind() {
        for kind in all_cast_kinds() {
            let rvalue = Rvalue::Cast(kind.clone(), const_operand(0), TyId::from_u32(1));

            assert!(matches!(
                rvalue,
                Rvalue::Cast(actual, Operand::Constant(_), ty)
                    if actual == kind && ty == TyId::from_u32(1)
            ));
        }
    }

    #[test]
    fn binary_op_rvalue_constructs_for_each_operator_family() {
        for op in [
            BinOp::Add,
            BinOp::AddUnchecked,
            BinOp::AddWithOverflow,
            BinOp::BitXor,
            BinOp::Shl,
            BinOp::ShlUnchecked,
            BinOp::Eq,
            BinOp::Cmp,
            BinOp::Offset,
        ] {
            let rvalue = Rvalue::BinaryOp(op.clone(), const_operand(0), const_operand(1));

            assert!(matches!(
                rvalue,
                Rvalue::BinaryOp(actual, Operand::Constant(left), Operand::Constant(right))
                    if actual == op
                        && left == ConstId::from_u32(0)
                        && right == ConstId::from_u32(1)
            ));
        }
    }

    #[test]
    fn binary_op_rvalue_reaches_all_bin_ops() {
        let mut values = HashSet::new();

        for op in all_bin_ops() {
            assert!(values.insert(Rvalue::BinaryOp(op, const_operand(0), const_operand(1))));
        }

        assert_eq!(values.len(), 26);
    }

    #[test]
    fn unary_op_rvalue_constructs_for_each_un_op() {
        let values = all_un_ops().map(|op| Rvalue::UnaryOp(op, const_operand(0)));

        assert_eq!(values[0], Rvalue::UnaryOp(UnOp::Not, const_operand(0)));
        assert_ne!(values[0], values[1]);
        assert_ne!(values[0], values[2]);
        assert_ne!(values[1], values[2]);
    }

    #[test]
    fn discriminant_rvalue_constructs() {
        let place = bare_place(0);
        let rvalue = Rvalue::Discriminant(place.clone());

        assert!(matches!(rvalue, Rvalue::Discriminant(actual) if actual == place));
    }

    #[test]
    fn aggregate_rvalue_constructs_for_each_aggregate_kind() {
        let mut values = HashSet::new();

        for kind in all_aggregate_kinds() {
            assert!(values.insert(Rvalue::Aggregate(kind, Box::new([const_operand(0)]))));
        }

        assert_eq!(values.len(), 7);
    }

    #[test]
    fn aggregate_rvalue_constructs_with_empty_operand_slab() {
        let rvalue = Rvalue::Aggregate(AggregateKind::Tuple(TyId::from_u32(0)), Box::new([]));

        assert!(matches!(rvalue, Rvalue::Aggregate(_, operands) if operands.is_empty()));
    }

    #[test]
    fn aggregate_rvalue_constructs_with_non_empty_operand_slab() {
        let c1 = ConstId::from_u32(1);
        let c2 = ConstId::from_u32(2);
        let rvalue = Rvalue::Aggregate(
            AggregateKind::Tuple(TyId::from_u32(0)),
            Box::new([Operand::Constant(c1), Operand::Constant(c2)]),
        );

        assert!(matches!(
            rvalue,
            Rvalue::Aggregate(_, operands)
                if operands.len() == 2
                    && operands.iter().eq([Operand::Constant(c1), Operand::Constant(c2)].iter())
        ));
    }

    #[test]
    fn rvalue_variants_are_pairwise_distinct() {
        let values = representative_rvalues();

        for (left_index, left) in values.iter().enumerate() {
            for right in values.iter().skip(left_index + 1) {
                assert_ne!(left, right);
            }
        }
    }

    #[test]
    fn cast_kind_reaches_all_top_level_variants() {
        let mut values = HashSet::new();

        for kind in all_cast_kinds() {
            assert!(values.insert(kind));
        }

        assert_eq!(values.len(), 11);
    }

    #[test]
    fn pointer_coercion_reaches_all_variants() {
        let mut values = HashSet::new();

        for coercion in all_pointer_coercions() {
            assert!(values.insert(coercion));
        }

        assert_eq!(values.len(), 4);
    }

    #[test]
    fn aggregate_kind_reaches_all_variants() {
        let mut values = HashSet::new();

        for kind in all_aggregate_kinds() {
            assert!(values.insert(kind));
        }

        assert_eq!(values.len(), 7);
    }

    #[test]
    fn enum_and_union_aggregate_kinds_are_distinct() {
        assert_ne!(
            AggregateKind::Enum(TyId::from_u32(0), VariantIdx::from_u32(0)),
            AggregateKind::Union(TyId::from_u32(0), FieldIdx::from_u32(0))
        );
    }

    #[test]
    fn exhaustive_bin_op_match_compiles() {
        for op in all_bin_ops() {
            let rank = match op {
                BinOp::Add => 0,
                BinOp::Sub => 1,
                BinOp::Mul => 2,
                BinOp::Div => 3,
                BinOp::Rem => 4,
                BinOp::AddUnchecked => 5,
                BinOp::SubUnchecked => 6,
                BinOp::MulUnchecked => 7,
                BinOp::AddWithOverflow => 8,
                BinOp::SubWithOverflow => 9,
                BinOp::MulWithOverflow => 10,
                BinOp::BitXor => 11,
                BinOp::BitAnd => 12,
                BinOp::BitOr => 13,
                BinOp::Shl => 14,
                BinOp::ShlUnchecked => 15,
                BinOp::Shr => 16,
                BinOp::ShrUnchecked => 17,
                BinOp::Eq => 18,
                BinOp::Lt => 19,
                BinOp::Le => 20,
                BinOp::Ne => 21,
                BinOp::Ge => 22,
                BinOp::Gt => 23,
                BinOp::Cmp => 24,
                BinOp::Offset => 25,
            };

            assert!(rank < 26);
        }
    }

    #[test]
    fn rvalue_hash_discriminates_variants() {
        let mut values = HashSet::new();

        for rvalue in representative_rvalues() {
            assert!(values.insert(rvalue));
        }

        assert_eq!(values.len(), 9);
    }

    #[test]
    fn serde_round_trips_each_rvalue_variant() {
        for rvalue in representative_rvalues() {
            assert_eq!(round_trip::<Rvalue>(&rvalue), rvalue);
        }
    }

    #[test]
    fn serde_round_trips_each_cast_kind_variant() {
        for kind in all_cast_kinds() {
            assert_eq!(round_trip::<CastKind>(&kind), kind);
        }
    }

    #[test]
    fn serde_round_trips_each_pointer_coercion_variant() {
        for coercion in all_pointer_coercions() {
            assert_eq!(round_trip::<PointerCoercion>(&coercion), coercion);
        }
    }

    #[test]
    fn serde_round_trips_each_bin_op_variant() {
        for op in all_bin_ops() {
            assert_eq!(round_trip::<BinOp>(&op), op);
        }
    }

    #[test]
    fn serde_round_trips_each_un_op_variant() {
        for op in all_un_ops() {
            assert_eq!(round_trip::<UnOp>(&op), op);
        }
    }

    #[test]
    fn serde_round_trips_each_aggregate_kind_variant() {
        for kind in all_aggregate_kinds() {
            assert_eq!(round_trip::<AggregateKind>(&kind), kind);
        }
    }

    #[test]
    fn rvalue_ref_reuses_type_module_provenance() {
        let provenance: crate::ty::Provenance = Provenance::SharedBorrow;
        let rvalue = Rvalue::Ref(provenance, bare_place(0));

        assert!(matches!(
            rvalue,
            Rvalue::Ref(actual, _) if actual == crate::ty::Provenance::SharedBorrow
        ));
    }

    #[test]
    fn rvalue_raw_ptr_reuses_type_module_mutability() {
        let mutability: crate::ty::Mutability = Mutability::Not;
        let rvalue = Rvalue::RawPtr(mutability, bare_place(0));

        assert!(matches!(
            rvalue,
            Rvalue::RawPtr(actual, _) if actual == crate::ty::Mutability::Not
        ));
    }
}
