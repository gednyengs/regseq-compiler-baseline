//! Statement types for the Regseq Model IR.
//!
//! A [`Statement`] is an instruction within a basic block that does not
//! transfer control flow. Statements execute sequentially within a block
//! until the block's terminator is reached.

use serde::{Deserialize, Serialize};

use crate::identity::LocalId;
use crate::ir::operand::Operand;
use crate::ir::place::Place;
use crate::ir::rvalue::Rvalue;

/// A statement: semantic content only. Source location is in BodySourceMap.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Statement {
    /// The kind of statement.
    pub kind: StatementKind,
}

/// The kind of a statement within a basic block.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum StatementKind {
    /// Assignment: evaluate rvalue, store in place.
    Assign(Place, Rvalue),

    /// Write the discriminant tag for an enum variant.
    SetDiscriminant {
        /// The enum-typed place to modify.
        place: Place,
        /// The variant index to write.
        variant_index: u32,
    },

    /// Mark a local's stack storage as allocated.
    StorageLive(LocalId),

    /// Mark a local's stack storage as freed.
    StorageDead(LocalId),

    /// MIR-level non-diverging intrinsic (distinct from RegseqIntrinsic).
    MirIntrinsic(MirIntrinsic),
}

/// Non-diverging intrinsics carried from MIR.
///
/// These are distinct from [`RegseqIntrinsic`](super::intrinsic::RegseqIntrinsic)
/// which represents domain-specific operations and flows through
/// `IntrinsicCall` terminators. `MirIntrinsic` is a statement -- it
/// always completes and does not transfer control flow.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum MirIntrinsic {
    /// Optimizer hint: assume this boolean is true.
    Assume(Operand),

    /// Copy elements from source to destination (non-overlapping).
    CopyNonOverlapping {
        /// Source pointer.
        src: Operand,
        /// Destination pointer.
        dst: Operand,
        /// Number of elements to copy.
        count: Operand,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::operand::LiteralValue;
    use crate::ir::types::ScalarType;

    // -- Construction: each of the 5 StatementKind variants --

    #[test]
    fn construct_statement_assign() {
        let place = Place {
            local: LocalId(0),
            projection: vec![],
        };
        let rvalue = Rvalue::Use(Operand::Literal(LiteralValue::U32(42)));
        let stmt = Statement {
            kind: StatementKind::Assign(place, rvalue),
        };
        matches!(stmt.kind, StatementKind::Assign(_, _));
    }

    #[test]
    fn construct_statement_set_discriminant() {
        let place = Place {
            local: LocalId(1),
            projection: vec![],
        };
        let _ = Statement {
            kind: StatementKind::SetDiscriminant {
                place,
                variant_index: 0,
            },
        };
    }

    #[test]
    fn construct_statement_storage_live() {
        let _ = Statement {
            kind: StatementKind::StorageLive(LocalId(2)),
        };
    }

    #[test]
    fn construct_statement_storage_dead() {
        let _ = Statement {
            kind: StatementKind::StorageDead(LocalId(2)),
        };
    }

    #[test]
    fn construct_statement_mir_intrinsic() {
        let _ = Statement {
            kind: StatementKind::MirIntrinsic(MirIntrinsic::Assume(Operand::Literal(
                LiteralValue::Bool(true),
            ))),
        };
    }

    // -- Construction: each of the 2 MirIntrinsic variants --

    #[test]
    fn construct_mir_intrinsic_assume() {
        let _ = MirIntrinsic::Assume(Operand::Literal(LiteralValue::Bool(true)));
    }

    #[test]
    fn construct_mir_intrinsic_copy_non_overlapping() {
        let _ = MirIntrinsic::CopyNonOverlapping {
            src: Operand::Copy(Place {
                local: LocalId(0),
                projection: vec![],
            }),
            dst: Operand::Copy(Place {
                local: LocalId(1),
                projection: vec![],
            }),
            count: Operand::Literal(LiteralValue::Usize(16)),
        };
    }

    // -- Composition: Statement with Place and Rvalue --

    #[test]
    fn statement_assign_composes_with_place_and_rvalue() {
        let place = Place {
            local: LocalId(5),
            projection: vec![],
        };
        let rvalue = Rvalue::Cast(
            crate::ir::rvalue::CastKind::IntToInt,
            Operand::Literal(LiteralValue::U8(255)),
            crate::ir::types::RegseqType::Scalar(ScalarType::U32),
        );
        let stmt = Statement {
            kind: StatementKind::Assign(place.clone(), rvalue.clone()),
        };
        assert_eq!(
            stmt,
            Statement {
                kind: StatementKind::Assign(place, rvalue),
            }
        );
    }
}
