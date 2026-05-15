//! MIR statement data shapes.
//!
//! `Statement` is body-local content and is stored inline in basic
//! block storage. There is no `StatementId` and no top-level statement
//! interner; equality and hashing are structural over `(kind, span)`.

use serde::{Deserialize, Serialize};

use crate::ids::VariantIdx;
use crate::operand::Operand;
use crate::place::Place;
use crate::rvalue::Rvalue;
use crate::source::Span;

/// A sub-terminator instruction in a basic block, with its source
/// location.
///
/// Body-local content; lives inline inside basic-block storage, not in
/// any top-level interner table. Equality is structural over `(kind,
/// span)`. Payload shape is defined by spec.md section 22.1.1.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Statement {
    /// The typed sub-terminator instruction.
    pub kind: StatementKind,
    /// The source location this statement derives from.
    pub span: Span,
}

/// A typed sub-terminator instruction.
///
/// Body-local content; appears inside `Statement.kind`. The closed
/// three-variant surface is defined by spec.md section 22.1.2.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatementKind {
    /// In-place value assignment: `place := rvalue`.
    ///
    /// Does not call drop on the previous left-hand-side value; drop
    /// is the explicit `Drop` terminator's job in the parent body
    /// vocabulary. Payload shape is defined by spec.md section 22.2.1.
    Assign(Place, Rvalue),

    /// In-place enum tag-write.
    ///
    /// Writes the discriminant for the named variant to the place,
    /// without touching variant payload fields. Payload shape is
    /// defined by spec.md section 22.2.2.
    SetDiscriminant(Place, VariantIdx),

    /// Non-diverging rustc-internal hint primitive.
    ///
    /// Carries only rustc's non-diverging statement-level primitives;
    /// regseq DSL semantic intrinsics flow through call terminators.
    /// Payload shape is defined by spec.md section 22.2.3.
    Intrinsic(NonDivergingIntrinsic),
}

/// Non-diverging rustc-internal hint primitives that survive into
/// RegseqModel as statement-level content.
///
/// Closed two-variant set; the bridge rejects rustc intrinsic arms
/// outside this set. Payload shape is defined by spec.md section
/// 22.3.1.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NonDivergingIntrinsic {
    /// Boolean hint: the operand is true.
    ///
    /// If the operand is false, semantics matches
    /// `TerminatorKind::Unreachable`.
    Assume(Operand),

    /// Inline non-overlapping byte copy.
    ///
    /// The `src` and `dst` operands are pointers or references, and
    /// `count` is a `usize` operand. The src and dst regions must not
    /// overlap. See spec.md section 22.3.1.
    CopyNonOverlapping(CopyNonOverlapping),
}

/// Payload for `NonDivergingIntrinsic::CopyNonOverlapping`.
///
/// Named-fields struct, not a tuple: `src`, `dst`, and `count` are all
/// `Operand` and order-confusable in a tuple shape. Payload shape is
/// defined by spec.md section 22.3.2.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CopyNonOverlapping {
    /// Source pointer or reference operand.
    pub src: Operand,
    /// Destination pointer or reference operand.
    pub dst: Operand,
    /// Element count operand, typed as `usize` by statement-site
    /// invariants.
    pub count: Operand,
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{CopyNonOverlapping, NonDivergingIntrinsic, Statement, StatementKind};
    use crate::{ConstId, LocalId, Operand, Place, Rvalue, SourceFileId, Span, VariantIdx};

    fn span(lo: u32, hi: u32) -> Span {
        Span {
            file: SourceFileId::SYNTHESIZED,
            lo,
            hi,
        }
    }

    fn place(local: u32) -> Place {
        Place::from(LocalId::from_u32(local))
    }

    fn const_operand(index: u32) -> Operand {
        Operand::Constant(ConstId::from_u32(index))
    }

    fn assign_statement(local: u32, const_index: u32, span_lo: u32) -> Statement {
        Statement {
            kind: StatementKind::Assign(
                place(local),
                Rvalue::Use(Operand::Constant(ConstId::from_u32(const_index))),
            ),
            span: span(span_lo, span_lo + 1),
        }
    }

    fn copy_non_overlapping() -> CopyNonOverlapping {
        CopyNonOverlapping {
            src: const_operand(1),
            dst: const_operand(2),
            count: const_operand(3),
        }
    }

    fn round_trip<T>(value: &T) -> T
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        let json = serde_json::to_string(value).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn statement_constructs_with_assign_kind_and_span() {
        let kind = StatementKind::Assign(place(0), Rvalue::Use(const_operand(1)));
        let statement = Statement {
            kind: kind.clone(),
            span: span(10, 15),
        };

        assert_eq!(statement.kind, kind);
        assert_eq!(statement.span, span(10, 15));
    }

    #[test]
    fn statement_struct_init_uses_kind_then_span_order() {
        let statement = Statement {
            kind: StatementKind::Assign(place(0), Rvalue::Use(const_operand(1))),
            span: span(10, 15),
        };

        assert_eq!(statement.span.lo, 10);
    }

    #[test]
    fn statement_kind_assign_constructs() {
        let destination = place(0);
        let source = Rvalue::Use(const_operand(1));
        let kind = StatementKind::Assign(destination.clone(), source.clone());

        assert!(
            matches!(kind, StatementKind::Assign(actual_place, actual_rvalue)
                if actual_place == destination && actual_rvalue == source)
        );
    }

    #[test]
    fn statement_kind_set_discriminant_constructs() {
        let target = place(0);
        let variant = VariantIdx::from_u32(2);
        let kind = StatementKind::SetDiscriminant(target.clone(), variant);

        assert!(
            matches!(kind, StatementKind::SetDiscriminant(actual_place, actual_variant)
                if actual_place == target && actual_variant == variant)
        );
    }

    #[test]
    fn statement_kind_intrinsic_assume_constructs() {
        let operand = const_operand(1);
        let kind = StatementKind::Intrinsic(NonDivergingIntrinsic::Assume(operand.clone()));

        assert!(
            matches!(kind, StatementKind::Intrinsic(NonDivergingIntrinsic::Assume(actual))
                if actual == operand)
        );
    }

    #[test]
    fn statement_kind_intrinsic_copy_non_overlapping_constructs() {
        let payload = copy_non_overlapping();
        let kind =
            StatementKind::Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(payload.clone()));

        assert!(matches!(
            kind,
            StatementKind::Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(actual))
                if actual.src == payload.src
                    && actual.dst == payload.dst
                    && actual.count == payload.count
        ));
    }

    #[test]
    fn statement_kind_variants_are_pairwise_distinct() {
        let assign = StatementKind::Assign(place(0), Rvalue::Use(const_operand(1)));
        let discriminant = StatementKind::SetDiscriminant(place(0), VariantIdx::from_u32(0));
        let intrinsic = StatementKind::Intrinsic(NonDivergingIntrinsic::Assume(const_operand(1)));

        assert_ne!(assign, discriminant);
        assert_ne!(assign, intrinsic);
        assert_ne!(discriminant, intrinsic);
    }

    #[test]
    fn statement_kind_assign_equality_identical() {
        let left = StatementKind::Assign(place(0), Rvalue::Use(const_operand(1)));
        let right = StatementKind::Assign(place(0), Rvalue::Use(const_operand(1)));

        assert_eq!(left, right);
    }

    #[test]
    fn statement_kind_assign_equality_differs_by_place() {
        let left = StatementKind::Assign(place(0), Rvalue::Use(const_operand(1)));
        let right = StatementKind::Assign(place(1), Rvalue::Use(const_operand(1)));

        assert_ne!(left, right);
    }

    #[test]
    fn statement_kind_assign_equality_differs_by_rvalue() {
        let left = StatementKind::Assign(place(0), Rvalue::Use(const_operand(1)));
        let right = StatementKind::Assign(place(0), Rvalue::Use(const_operand(2)));

        assert_ne!(left, right);
    }

    #[test]
    fn statement_kind_set_discriminant_equality_differs_by_variant() {
        let left = StatementKind::SetDiscriminant(place(0), VariantIdx::from_u32(0));
        let right = StatementKind::SetDiscriminant(place(0), VariantIdx::from_u32(1));

        assert_ne!(left, right);
    }

    #[test]
    fn statement_equality_includes_span() {
        let left = assign_statement(0, 1, 10);
        let right = assign_statement(0, 1, 11);

        assert_ne!(left, right);
    }

    #[test]
    fn non_diverging_intrinsic_assume_constructs() {
        let operand = const_operand(1);
        let intrinsic = NonDivergingIntrinsic::Assume(operand.clone());

        assert!(matches!(intrinsic, NonDivergingIntrinsic::Assume(actual) if actual == operand));
    }

    #[test]
    fn non_diverging_intrinsic_copy_non_overlapping_constructs() {
        let payload = copy_non_overlapping();
        let intrinsic = NonDivergingIntrinsic::CopyNonOverlapping(payload.clone());

        assert!(
            matches!(intrinsic, NonDivergingIntrinsic::CopyNonOverlapping(actual) if actual == payload)
        );
    }

    #[test]
    fn non_diverging_intrinsic_variants_are_distinct() {
        let assume = NonDivergingIntrinsic::Assume(const_operand(1));
        let copy = NonDivergingIntrinsic::CopyNonOverlapping(copy_non_overlapping());

        assert_ne!(assume, copy);
    }

    #[test]
    fn copy_non_overlapping_field_names_discriminate_src_and_dst() {
        let payload = CopyNonOverlapping {
            src: const_operand(1),
            dst: const_operand(2),
            count: const_operand(3),
        };
        let swapped = CopyNonOverlapping {
            src: const_operand(2),
            dst: const_operand(1),
            count: const_operand(3),
        };

        assert_ne!(payload, swapped);
    }

    #[test]
    fn copy_non_overlapping_count_reads_back_as_operand() {
        let count = const_operand(3);
        let payload = CopyNonOverlapping {
            src: const_operand(1),
            dst: const_operand(2),
            count: count.clone(),
        };

        assert_eq!(payload.count, count);
    }

    #[test]
    fn statement_hash_discriminates_structurally_distinct_values() {
        let values = [
            assign_statement(0, 1, 10),
            Statement {
                kind: StatementKind::SetDiscriminant(place(0), VariantIdx::from_u32(0)),
                span: span(10, 11),
            },
            Statement {
                kind: StatementKind::Intrinsic(NonDivergingIntrinsic::Assume(const_operand(1))),
                span: span(10, 11),
            },
            assign_statement(0, 1, 11),
        ];
        let mut set = HashSet::new();

        for value in values.iter().cloned() {
            assert!(set.insert(value));
        }
        assert_eq!(set.len(), values.len());

        for value in values {
            assert!(!set.insert(value));
        }
        assert_eq!(set.len(), 4);
    }

    #[test]
    fn serde_round_trips_statement_assign() {
        let statement = assign_statement(0, 1, 10);

        assert_eq!(round_trip(&statement), statement);
    }

    #[test]
    fn serde_round_trips_statement_set_discriminant() {
        let statement = Statement {
            kind: StatementKind::SetDiscriminant(place(0), VariantIdx::from_u32(2)),
            span: span(10, 11),
        };

        assert_eq!(round_trip(&statement), statement);
    }

    #[test]
    fn serde_round_trips_statement_intrinsic_assume() {
        let statement = Statement {
            kind: StatementKind::Intrinsic(NonDivergingIntrinsic::Assume(const_operand(1))),
            span: span(10, 11),
        };

        assert_eq!(round_trip(&statement), statement);
    }

    #[test]
    fn serde_round_trips_statement_intrinsic_copy_non_overlapping() {
        let statement = Statement {
            kind: StatementKind::Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(
                copy_non_overlapping(),
            )),
            span: span(10, 11),
        };

        assert_eq!(round_trip(&statement), statement);
    }

    #[test]
    fn serde_round_trips_non_diverging_intrinsic_variants() {
        let assume = NonDivergingIntrinsic::Assume(const_operand(1));
        let copy = NonDivergingIntrinsic::CopyNonOverlapping(copy_non_overlapping());

        assert_eq!(round_trip(&assume), assume);
        assert_eq!(round_trip(&copy), copy);
    }

    #[test]
    fn serde_round_trips_copy_non_overlapping() {
        let payload = copy_non_overlapping();

        assert_eq!(round_trip(&payload), payload);
    }
}
