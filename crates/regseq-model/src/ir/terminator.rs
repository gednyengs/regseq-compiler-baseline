//! Terminator types for the Regseq Model IR.
//!
//! A [`Terminator`] is the final instruction of a basic block that
//! transfers control flow to one or more successor blocks.

use serde::{Deserialize, Serialize};

use crate::identity::{BasicBlockId, FunctionId};
use crate::ir::intrinsic::{IntrinsicMetadata, RegseqIntrinsic};
use crate::ir::operand::Operand;
use crate::ir::place::Place;

/// A terminator: semantic content only. Source location is in BodySourceMap.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Terminator {
    /// The kind of terminator.
    pub kind: TerminatorKind,
}

/// The kind of a terminator that ends a basic block.
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum TerminatorKind {
    /// Unconditional jump to a block.
    Goto(BasicBlockId),

    /// Multi-way branch on an integer discriminant.
    SwitchInt {
        /// The value to switch on.
        discriminant: Operand,
        /// Branch targets (values + default).
        targets: SwitchTargets,
    },

    /// Call a user-defined function.
    Call {
        /// The function to call.
        callee: FunctionId,
        /// Arguments to pass.
        args: Vec<Operand>,
        /// Place to store the return value.
        destination: Place,
        /// Block to jump to after return (`None` if the call diverges).
        target: Option<BasicBlockId>,
        /// Block to jump to on error (`None` for infallible calls).
        error_target: Option<BasicBlockId>,
    },

    /// Call a domain-specific intrinsic operation.
    IntrinsicCall {
        /// Which intrinsic to call.
        intrinsic: RegseqIntrinsic,
        /// Arguments to pass.
        args: Vec<Operand>,
        /// Structured metadata for backends.
        metadata: Option<IntrinsicMetadata>,
        /// Place to store the return value.
        destination: Place,
        /// Block to jump to after return (intrinsics always return).
        target: BasicBlockId,
        /// Block to jump to on error (`None` for infallible intrinsics).
        error_target: Option<BasicBlockId>,
    },

    /// Drop a value at a place.
    Drop {
        /// The place whose value to drop.
        place: Place,
        /// Block to jump to after the drop completes.
        target: BasicBlockId,
    },

    /// Return from the current function.
    Return,

    /// Indicates unreachable code.
    Unreachable,
}

/// Branch targets for `SwitchInt`.
///
/// `targets` has one more entry than `values` -- the last target is the
/// default branch taken when no value matches.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct SwitchTargets {
    /// Discriminant values to match against.
    pub values: Vec<u128>,
    /// Target basic blocks. Length is `values.len() + 1` -- last is default.
    pub targets: Vec<BasicBlockId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{DeclId, LocalId, RegisterBlockId};
    use crate::ir::intrinsic::RegisterFieldMeta;
    use crate::ir::operand::LiteralValue;
    use crate::ir::place::Projection;
    use crate::ir::types::{RegseqType, ScalarType};

    // -- Construction: each of the 7 TerminatorKind variants --

    #[test]
    fn construct_goto() {
        let _ = Terminator {
            kind: TerminatorKind::Goto(BasicBlockId(1)),
        };
    }

    #[test]
    fn construct_switch_int() {
        let _ = Terminator {
            kind: TerminatorKind::SwitchInt {
                discriminant: Operand::Literal(LiteralValue::U32(0)),
                targets: SwitchTargets {
                    values: vec![0, 1],
                    targets: vec![BasicBlockId(1), BasicBlockId(2), BasicBlockId(3)],
                },
            },
        };
    }

    #[test]
    fn construct_call() {
        let _ = Terminator {
            kind: TerminatorKind::Call {
                callee: FunctionId::from(DeclId::new(0)),
                args: vec![Operand::Literal(LiteralValue::U32(42))],
                destination: Place {
                    local: LocalId(0),
                    projection: vec![],
                },
                target: Some(BasicBlockId(1)),
                error_target: None,
            },
        };
    }

    #[test]
    fn construct_intrinsic_call() {
        let _ = Terminator {
            kind: TerminatorKind::IntrinsicCall {
                intrinsic: RegseqIntrinsic::RegisterRead,
                args: vec![],
                metadata: None,
                destination: Place {
                    local: LocalId(0),
                    projection: vec![],
                },
                target: BasicBlockId(1),
                error_target: None,
            },
        };
    }

    #[test]
    fn construct_drop() {
        let _ = Terminator {
            kind: TerminatorKind::Drop {
                place: Place {
                    local: LocalId(3),
                    projection: vec![],
                },
                target: BasicBlockId(4),
            },
        };
    }

    #[test]
    fn construct_return() {
        let _ = Terminator {
            kind: TerminatorKind::Return,
        };
    }

    #[test]
    fn construct_unreachable() {
        let _ = Terminator {
            kind: TerminatorKind::Unreachable,
        };
    }

    // -- SwitchTargets with 2 values and 3 targets --

    #[test]
    fn switch_targets_two_values_three_targets() {
        let st = SwitchTargets {
            values: vec![0, 1],
            targets: vec![BasicBlockId(10), BasicBlockId(20), BasicBlockId(30)],
        };
        assert_eq!(st.values.len() + 1, st.targets.len());
    }

    // -- IntrinsicCall with metadata --

    #[test]
    fn intrinsic_call_with_register_metadata() {
        use crate::identity::SymbolInterner;

        let mut interner = SymbolInterner::new();
        let field_name = interner.intern("baud_divint");

        let _ = Terminator {
            kind: TerminatorKind::IntrinsicCall {
                intrinsic: RegseqIntrinsic::RegisterReadField,
                args: vec![Operand::Copy(Place {
                    local: LocalId(1),
                    projection: vec![
                        Projection::Deref,
                        Projection::Field {
                            index: 0,
                            name: None,
                            ty: RegseqType::Scalar(ScalarType::U32),
                        },
                    ],
                })],
                metadata: Some(IntrinsicMetadata::Register {
                    register_block: RegisterBlockId::from(DeclId::new(0)),
                    register_offset: 0x24,
                    access_width: 32,
                    field: Some(RegisterFieldMeta {
                        mask: 0xFFFF,
                        shift: 0,
                        name: field_name,
                    }),
                }),
                destination: Place {
                    local: LocalId(5),
                    projection: vec![],
                },
                target: BasicBlockId(2),
                error_target: None,
            },
        };
    }

    // -- IntrinsicCall with error_target --

    #[test]
    fn intrinsic_call_with_error_target() {
        let _ = Terminator {
            kind: TerminatorKind::IntrinsicCall {
                intrinsic: RegseqIntrinsic::PollUntil,
                args: vec![],
                metadata: None,
                destination: Place {
                    local: LocalId(0),
                    projection: vec![],
                },
                target: BasicBlockId(3),
                error_target: Some(BasicBlockId(4)),
            },
        };
    }

    // -- Call with error_target: None (infallible) --

    #[test]
    fn call_with_no_error_target() {
        let term = Terminator {
            kind: TerminatorKind::Call {
                callee: FunctionId::from(DeclId::new(10)),
                args: vec![],
                destination: Place {
                    local: LocalId(0),
                    projection: vec![],
                },
                target: Some(BasicBlockId(1)),
                error_target: None,
            },
        };
        match &term.kind {
            TerminatorKind::Call { error_target, .. } => {
                assert!(error_target.is_none());
            }
            _ => panic!("expected Call"),
        }
    }
}
