//! MIR terminator data shapes.
//!
//! `Terminator` is body-local content and is stored inline as the
//! control-flow exit of a basic block. There is no `TerminatorId` and
//! no top-level terminator interner; equality and hashing are
//! structural over `(kind, span)`.
//!
//! `BlockId` is an opaque body-local typed index for this module. Its
//! storage container lands in the basic-block vocabulary task.

use serde::{Deserialize, Serialize};

use crate::ids::{BlockId, InstanceId};
use crate::operand::Operand;
use crate::place::Place;
use crate::rvalue::BinOp;
use crate::source::Span;

/// A control-flow transfer ending a basic block, with its source
/// location.
///
/// Body-local content; lives inline inside basic-block storage, not
/// in any top-level interner table. Equality is structural over
/// `(kind, span)`. Payload shape is defined by spec.md section
/// 23.1.1.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Terminator {
    /// The typed basic-block exit.
    pub kind: TerminatorKind,
    /// The source location this terminator derives from.
    pub span: Span,
}

impl Terminator {
    /// Returns the successor basic blocks in the fixed order defined
    /// by spec.md section 23.1.4.
    pub fn successors(&self) -> impl Iterator<Item = BlockId> {
        let successors: Vec<BlockId> = match &self.kind {
            TerminatorKind::Goto(target) => vec![*target],
            TerminatorKind::SwitchInt { targets, .. } => targets
                .cases
                .iter()
                .map(|(_, target)| *target)
                .chain(std::iter::once(targets.otherwise))
                .collect(),
            TerminatorKind::Return | TerminatorKind::Unreachable => Vec::new(),
            TerminatorKind::Call { target, .. } => target.iter().copied().collect(),
            TerminatorKind::Assert { target, .. } => vec![*target],
        };
        successors.into_iter()
    }
}

/// Closed 6-variant set covering every basic-block exit in
/// well-formed driver MIR post-pipeline.
///
/// Variants outside this set are bridge-rejected, pipeline-discharged,
/// or collapsed at the bridge; in particular, rustc's `Drop` becomes
/// a synthetic `Call`, per spec.md sections 23.1.2 and 23.4.
/// Worst-case inline storage is the `Call` payload plus the enum
/// discriminator; the variable-length args list is held in a boxed
/// slice and no explicit representation is pinned.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TerminatorKind {
    /// Unconditional branch. Single successor.
    Goto(BlockId),

    /// Multi-target conditional branch on an integer-typed
    /// discriminant.
    SwitchInt {
        /// The value being tested.
        discr: Operand,
        /// Explicit case targets plus the catch-all target.
        targets: SwitchTargets,
    },

    /// Function return. The return value lives at the body return
    /// place; no payload is carried here.
    Return,

    /// Terminating undefined-behavior marker.
    Unreachable,

    /// Function call.
    Call {
        /// Compile-time or runtime-resolved callee.
        callee: CallCallee,
        /// Call arguments, with per-argument spans dropped.
        args: Box<[Operand]>,
        /// Place where the return value is written.
        destination: Place,
        /// Normal-return successor, or `None` for divergent calls.
        target: Option<BlockId>,
    },

    /// Guarded conditional branch.
    Assert {
        /// Condition operand.
        cond: Operand,
        /// Expected value of `cond` for normal continuation.
        expected: bool,
        /// Panic-message data used when the assertion fails.
        msg: AssertKind,
        /// Success-path successor block.
        target: BlockId,
    },
}

/// Closed 2-arm classification of `Call.callee`.
///
/// Payload shape is defined by spec.md section 23.3.1.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CallCallee {
    /// Direct call to a compile-time-resolved instance.
    Direct(InstanceId),

    /// Indirect call through a runtime function-pointer operand.
    Indirect(Operand),
}

/// Closed-form switch target table.
///
/// Explicit case-value targets are stored separately from the
/// catch-all destination; there is no positional "last entry is
/// otherwise" convention. Payload shape is defined by spec.md section
/// 23.3.2.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SwitchTargets {
    /// `(case value, branch target)` pairs in storage order.
    pub cases: Box<[(u128, BlockId)]>,
    /// Catch-all destination when no case value matches.
    pub otherwise: BlockId,
}

/// Closed 8-variant set of panic-message kinds for `Assert.msg`.
///
/// This is the RegseqModel subset of rustc's assertion taxonomy after
/// coroutine-only variants are removed by type rejection, per spec.md
/// section 23.3.3.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssertKind {
    /// Index-out-of-bounds: `index >= len`.
    BoundsCheck {
        /// Length operand.
        len: Operand,
        /// Index operand.
        index: Operand,
    },

    /// Arithmetic overflow on a checked binary operation.
    Overflow(BinOp, Operand, Operand),

    /// Negation overflow.
    OverflowNeg(Operand),

    /// Division by zero. Operand is the divisor.
    DivisionByZero(Operand),

    /// Remainder by zero. Operand is the divisor.
    RemainderByZero(Operand),

    /// Misaligned pointer dereference.
    MisalignedPointerDereference {
        /// Required alignment operand.
        required: Operand,
        /// Found pointer-as-usize operand.
        found: Operand,
    },

    /// Null pointer dereference.
    NullPointerDereference,

    /// Construction of an enum value with an invalid discriminant.
    InvalidEnumConstruction(Operand),
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    use super::{AssertKind, CallCallee, SwitchTargets, Terminator, TerminatorKind};
    use crate::{BinOp, BlockId, ConstId, InstanceId, LocalId, Operand, Place, SourceFileId, Span};

    fn block(index: u32) -> BlockId {
        BlockId::from_u32(index)
    }

    fn instance(index: u32) -> InstanceId {
        InstanceId::from_u32(index)
    }

    fn place(index: u32) -> Place {
        Place::from(LocalId::from_u32(index))
    }

    fn const_operand(index: u32) -> Operand {
        Operand::Constant(ConstId::from_u32(index))
    }

    fn copy_operand(local: u32) -> Operand {
        Operand::Copy(place(local))
    }

    fn span(lo: u32, hi: u32) -> Span {
        Span {
            file: SourceFileId::SYNTHESIZED,
            lo,
            hi,
        }
    }

    fn switch_targets() -> SwitchTargets {
        SwitchTargets {
            cases: Box::new([(0, block(1)), (1, block(2))]),
            otherwise: block(3),
        }
    }

    fn direct_call(target: Option<BlockId>) -> TerminatorKind {
        TerminatorKind::Call {
            callee: CallCallee::Direct(instance(7)),
            args: Box::new([const_operand(1)]),
            destination: place(0),
            target,
        }
    }

    fn representative_terminator_kinds() -> Vec<TerminatorKind> {
        vec![
            TerminatorKind::Goto(block(1)),
            TerminatorKind::SwitchInt {
                discr: const_operand(1),
                targets: switch_targets(),
            },
            TerminatorKind::Return,
            TerminatorKind::Unreachable,
            direct_call(Some(block(4))),
            TerminatorKind::Assert {
                cond: const_operand(2),
                expected: true,
                msg: AssertKind::NullPointerDereference,
                target: block(5),
            },
        ]
    }

    fn representative_assert_kinds() -> Vec<AssertKind> {
        vec![
            AssertKind::BoundsCheck {
                len: const_operand(1),
                index: const_operand(2),
            },
            AssertKind::Overflow(BinOp::Add, const_operand(3), const_operand(4)),
            AssertKind::OverflowNeg(const_operand(5)),
            AssertKind::DivisionByZero(const_operand(6)),
            AssertKind::RemainderByZero(const_operand(7)),
            AssertKind::MisalignedPointerDereference {
                required: const_operand(8),
                found: const_operand(9),
            },
            AssertKind::NullPointerDereference,
            AssertKind::InvalidEnumConstruction(const_operand(10)),
        ]
    }

    fn round_trip<T>(value: &T) -> T
    where
        T: serde::Serialize + serde::de::DeserializeOwned,
    {
        let json = serde_json::to_string(value).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    fn hash_value<T: Hash>(value: &T) -> u64 {
        let mut hasher = DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    fn exhaustive_terminator_kind_name(kind: TerminatorKind) -> &'static str {
        match kind {
            TerminatorKind::Goto(_) => "goto",
            TerminatorKind::SwitchInt { .. } => "switch_int",
            TerminatorKind::Return => "return",
            TerminatorKind::Unreachable => "unreachable",
            TerminatorKind::Call { .. } => "call",
            TerminatorKind::Assert { .. } => "assert",
        }
    }

    #[test]
    fn terminator_constructs_with_kind_and_span() {
        let terminator = Terminator {
            kind: TerminatorKind::Return,
            span: span(10, 11),
        };

        assert_eq!(terminator.kind, TerminatorKind::Return);
        assert_eq!(terminator.span, span(10, 11));
    }

    #[test]
    fn terminator_kind_goto_constructs() {
        let kind = TerminatorKind::Goto(block(3));

        assert!(matches!(kind, TerminatorKind::Goto(target) if target == block(3)));
    }

    #[test]
    fn terminator_kind_switch_int_constructs() {
        let discr = const_operand(1);
        let targets = switch_targets();
        let kind = TerminatorKind::SwitchInt {
            discr: discr.clone(),
            targets: targets.clone(),
        };

        assert!(
            matches!(kind, TerminatorKind::SwitchInt { discr: actual_discr, targets: actual_targets }
                if actual_discr == discr && actual_targets == targets)
        );
    }

    #[test]
    fn terminator_kind_return_and_unreachable_are_distinct() {
        assert_eq!(TerminatorKind::Return, TerminatorKind::Return);
        assert_eq!(TerminatorKind::Unreachable, TerminatorKind::Unreachable);
        assert_ne!(TerminatorKind::Return, TerminatorKind::Unreachable);
    }

    #[test]
    fn terminator_kind_call_constructs_with_direct_callee() {
        let kind = direct_call(Some(block(1)));

        assert!(
            matches!(kind, TerminatorKind::Call { callee: CallCallee::Direct(id), args, destination, target }
                if id == instance(7)
                    && args.as_ref() == [const_operand(1)]
                    && destination == place(0)
                    && target == Some(block(1)))
        );
    }

    #[test]
    fn terminator_kind_call_constructs_with_indirect_callee() {
        let callee = copy_operand(4);
        let kind = TerminatorKind::Call {
            callee: CallCallee::Indirect(callee.clone()),
            args: Box::new([const_operand(1)]),
            destination: place(0),
            target: Some(block(1)),
        };

        assert!(
            matches!(kind, TerminatorKind::Call { callee: CallCallee::Indirect(actual), .. }
                if actual == callee)
        );
    }

    #[test]
    fn terminator_kind_call_constructs_with_no_target_for_divergence() {
        let kind = direct_call(None);

        assert!(matches!(kind, TerminatorKind::Call { target: None, .. }));
    }

    #[test]
    fn terminator_kind_assert_constructs() {
        let kind = TerminatorKind::Assert {
            cond: const_operand(1),
            expected: true,
            msg: AssertKind::NullPointerDereference,
            target: block(2),
        };

        assert!(
            matches!(kind, TerminatorKind::Assert { cond, expected, msg, target }
                if cond == const_operand(1)
                    && expected
                    && msg == AssertKind::NullPointerDereference
                    && target == block(2))
        );
    }

    #[test]
    fn all_terminator_kind_variants_are_pairwise_distinct() {
        let variants = representative_terminator_kinds();

        for left in 0..variants.len() {
            for right in (left + 1)..variants.len() {
                assert_ne!(variants[left], variants[right]);
            }
        }
    }

    #[test]
    fn call_callee_variants_are_distinct_and_direct_equality_is_structural() {
        let direct = CallCallee::Direct(instance(7));
        let indirect = CallCallee::Indirect(copy_operand(1));

        assert_ne!(direct, indirect);
        assert_eq!(direct, CallCallee::Direct(instance(7)));
        assert_ne!(direct, CallCallee::Direct(instance(8)));
    }

    #[test]
    fn switch_targets_constructs_with_empty_cases() {
        let targets = SwitchTargets {
            cases: Box::new([]),
            otherwise: block(3),
        };

        assert!(targets.cases.is_empty());
        assert_eq!(targets.otherwise, block(3));
    }

    #[test]
    fn switch_targets_constructs_with_non_empty_cases() {
        let targets = switch_targets();
        let cases: Vec<_> = targets.cases.iter().copied().collect();

        assert_eq!(cases, vec![(0, block(1)), (1, block(2))]);
        assert_eq!(targets.cases.len(), 2);
        assert_eq!(targets.otherwise, block(3));
    }

    #[test]
    fn switch_targets_equality_and_hash_are_structural() {
        let left = switch_targets();
        let right = switch_targets();

        assert_eq!(left, right);
        assert_eq!(hash_value(&left), hash_value(&right));
    }

    #[test]
    fn assert_kind_bounds_check_constructs() {
        let kind = AssertKind::BoundsCheck {
            len: const_operand(1),
            index: const_operand(2),
        };

        assert!(matches!(kind, AssertKind::BoundsCheck { len, index }
                if len == const_operand(1) && index == const_operand(2)));
    }

    #[test]
    fn assert_kind_overflow_constructs() {
        let kind = AssertKind::Overflow(BinOp::Add, const_operand(1), const_operand(2));

        assert!(matches!(kind, AssertKind::Overflow(op, left, right)
                if op == BinOp::Add && left == const_operand(1) && right == const_operand(2)));
    }

    #[test]
    fn assert_kind_overflow_neg_constructs() {
        let kind = AssertKind::OverflowNeg(const_operand(1));

        assert!(matches!(kind, AssertKind::OverflowNeg(actual) if actual == const_operand(1)));
    }

    #[test]
    fn assert_kind_division_and_remainder_by_zero_are_distinct() {
        let division = AssertKind::DivisionByZero(const_operand(1));
        let remainder = AssertKind::RemainderByZero(const_operand(1));

        assert_ne!(division, remainder);
    }

    #[test]
    fn assert_kind_misaligned_pointer_dereference_constructs() {
        let kind = AssertKind::MisalignedPointerDereference {
            required: const_operand(1),
            found: const_operand(2),
        };

        assert!(
            matches!(kind, AssertKind::MisalignedPointerDereference { required, found }
                if required == const_operand(1) && found == const_operand(2))
        );
    }

    #[test]
    fn assert_kind_null_pointer_dereference_constructs() {
        assert_ne!(
            AssertKind::NullPointerDereference,
            AssertKind::InvalidEnumConstruction(const_operand(1))
        );
    }

    #[test]
    fn assert_kind_invalid_enum_construction_constructs() {
        let kind = AssertKind::InvalidEnumConstruction(const_operand(1));

        assert!(matches!(kind, AssertKind::InvalidEnumConstruction(actual)
                if actual == const_operand(1)));
    }

    #[test]
    fn all_assert_kind_variants_are_pairwise_distinct() {
        let variants = representative_assert_kinds();

        for left in 0..variants.len() {
            for right in (left + 1)..variants.len() {
                assert_ne!(variants[left], variants[right]);
            }
        }
    }

    #[test]
    fn successors_for_goto_yields_target() {
        let terminator = Terminator {
            kind: TerminatorKind::Goto(block(3)),
            span: span(0, 1),
        };

        assert_eq!(terminator.successors().collect::<Vec<_>>(), vec![block(3)]);
    }

    #[test]
    fn successors_for_switch_int_yields_cases_in_order_then_otherwise() {
        let non_empty = Terminator {
            kind: TerminatorKind::SwitchInt {
                discr: const_operand(1),
                targets: switch_targets(),
            },
            span: span(0, 1),
        };
        let empty = Terminator {
            kind: TerminatorKind::SwitchInt {
                discr: const_operand(1),
                targets: SwitchTargets {
                    cases: Box::new([]),
                    otherwise: block(9),
                },
            },
            span: span(0, 1),
        };

        assert_eq!(
            non_empty.successors().collect::<Vec<_>>(),
            vec![block(1), block(2), block(3)]
        );
        assert_eq!(empty.successors().collect::<Vec<_>>(), vec![block(9)]);
    }

    #[test]
    fn successors_for_return_and_unreachable_are_empty() {
        let return_ = Terminator {
            kind: TerminatorKind::Return,
            span: span(0, 1),
        };
        let unreachable = Terminator {
            kind: TerminatorKind::Unreachable,
            span: span(0, 1),
        };

        assert_eq!(return_.successors().collect::<Vec<_>>(), Vec::new());
        assert_eq!(unreachable.successors().collect::<Vec<_>>(), Vec::new());
    }

    #[test]
    fn successors_for_call_yields_normal_target_only_when_present() {
        let returning = Terminator {
            kind: direct_call(Some(block(4))),
            span: span(0, 1),
        };
        let divergent = Terminator {
            kind: direct_call(None),
            span: span(0, 1),
        };

        assert_eq!(returning.successors().collect::<Vec<_>>(), vec![block(4)]);
        assert_eq!(divergent.successors().collect::<Vec<_>>(), Vec::new());
    }

    #[test]
    fn successors_for_assert_yields_success_target() {
        let terminator = Terminator {
            kind: TerminatorKind::Assert {
                cond: const_operand(1),
                expected: true,
                msg: AssertKind::NullPointerDereference,
                target: block(5),
            },
            span: span(0, 1),
        };

        assert_eq!(terminator.successors().collect::<Vec<_>>(), vec![block(5)]);
    }

    #[test]
    fn exhaustive_match_confirms_only_six_terminator_kind_variants() {
        let names: Vec<_> = representative_terminator_kinds()
            .into_iter()
            .map(exhaustive_terminator_kind_name)
            .collect();

        assert_eq!(
            names,
            vec![
                "goto",
                "switch_int",
                "return",
                "unreachable",
                "call",
                "assert"
            ]
        );
    }

    #[test]
    fn serde_round_trips_terminator_for_each_kind() {
        for kind in representative_terminator_kinds() {
            let terminator = Terminator {
                kind,
                span: span(10, 11),
            };

            assert_eq!(round_trip(&terminator), terminator);
        }
    }

    #[test]
    fn serde_round_trips_each_call_callee_variant() {
        for callee in [
            CallCallee::Direct(instance(7)),
            CallCallee::Indirect(copy_operand(1)),
        ] {
            assert_eq!(round_trip(&callee), callee);
        }
    }

    #[test]
    fn serde_round_trips_switch_targets() {
        let targets = switch_targets();

        assert_eq!(round_trip(&targets), targets);
    }

    #[test]
    fn serde_round_trips_each_assert_kind_variant() {
        for kind in representative_assert_kinds() {
            assert_eq!(round_trip(&kind), kind);
        }
    }
}
