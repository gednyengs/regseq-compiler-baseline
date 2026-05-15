//! MIR basic-block data shape.
//!
//! `BasicBlock` is body-local content stored inline in
//! `Body.basic_blocks: IndexVec<BlockId, BasicBlock>`.
//! Body-level storage is specified to be in reverse-postorder from
//! `BlockId::START_BLOCK` (B-RPO), and `BlockId::START_BLOCK` is the
//! entry block of every body (B-Entry). The body-walk task
//! regseq-model-82 / P1a-VAL-02 owns enforcement of those invariants,
//! together with reachability, non-empty-body, and successor-validity
//! checks.

use serde::{Deserialize, Serialize};

use crate::statement::Statement;
use crate::terminator::Terminator;

/// A basic block: zero or more straight-line statements followed by
/// exactly one control-flow terminator.
///
/// Body-local content; lives inline inside the body's basic-block
/// table at `Body.basic_blocks` (spec.md section 24.2). Equality is
/// structural. The statement payload is finalized `Box<[Statement]>`
/// storage per spec.md section 24.1.2. The terminator is
/// non-optional per spec.md section 24.1.3: rustc's construction-only
/// `Option<Terminator>` state is collapsed at the bridge boundary.
///
/// RegseqModel drops rustc's `is_cleanup` field per spec.md section
/// 24.1.4 and `after_last_stmt_debuginfos` per spec.md section
/// 24.1.5. It also carries no block-level span or scope metadata per
/// spec.md section 24.1.6.
///
/// The custom pretty-printer renders the surrounding `BlockId(N)` as
/// `bbN`; that sigil convention is implemented by P1a-PP-03, while
/// `Debug` for the typed ID remains `BlockId(N)`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BasicBlock {
    /// The straight-line instruction sequence preceding the
    /// terminator. Finalized single-fill payload per spec.md section
    /// 24.1.2; consumers index by `usize` or iterate via
    /// `&[Statement]` deref.
    pub statements: Box<[Statement]>,

    /// The control-flow transfer ending this block. Exactly one
    /// terminator per block; no `Option`, per spec.md section 24.1.3.
    pub terminator: Terminator,
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::BasicBlock;
    use crate::{
        CallCallee, ConstId, InstanceId, LocalId, Operand, Place, Rvalue, SourceFileId, Span,
        Statement, StatementKind, Terminator, TerminatorKind,
    };

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
            kind: StatementKind::Assign(place(local), Rvalue::Use(const_operand(const_index))),
            span: span(span_lo, span_lo + 1),
        }
    }

    fn return_terminator() -> Terminator {
        Terminator {
            kind: TerminatorKind::Return,
            span: span(90, 91),
        }
    }

    fn unreachable_terminator() -> Terminator {
        Terminator {
            kind: TerminatorKind::Unreachable,
            span: span(90, 91),
        }
    }

    fn call_terminator() -> Terminator {
        Terminator {
            kind: TerminatorKind::Call {
                callee: CallCallee::Direct(InstanceId::from_u32(7)),
                args: Box::new([const_operand(1), const_operand(2)]),
                destination: place(0),
                target: None,
            },
            span: span(92, 93),
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
    fn basic_block_constructs_with_empty_statements() {
        let terminator = return_terminator();
        let block = BasicBlock {
            statements: Box::new([]),
            terminator: terminator.clone(),
        };

        assert!(block.statements.is_empty());
        assert_eq!(block.terminator, terminator);
    }

    #[test]
    fn basic_block_preserves_non_empty_statement_order() {
        let first = assign_statement(0, 1, 10);
        let second = assign_statement(1, 2, 20);
        let block = BasicBlock {
            statements: Box::new([first.clone(), second.clone()]),
            terminator: return_terminator(),
        };

        assert_eq!(block.statements.len(), 2);
        assert_eq!(
            block.statements.iter().collect::<Vec<_>>(),
            vec![&first, &second]
        );
    }

    #[test]
    fn basic_block_equality_is_structural_for_identical_blocks() {
        let left = BasicBlock {
            statements: Box::new([assign_statement(0, 1, 10)]),
            terminator: return_terminator(),
        };
        let right = left.clone();

        assert_eq!(left, right);
    }

    #[test]
    fn basic_block_equality_includes_statements() {
        let left = BasicBlock {
            statements: Box::new([assign_statement(0, 1, 10)]),
            terminator: return_terminator(),
        };
        let right = BasicBlock {
            statements: Box::new([assign_statement(1, 2, 20)]),
            terminator: return_terminator(),
        };

        assert_ne!(left, right);
    }

    #[test]
    fn basic_block_equality_includes_terminator() {
        let left = BasicBlock {
            statements: Box::new([assign_statement(0, 1, 10)]),
            terminator: return_terminator(),
        };
        let right = BasicBlock {
            statements: Box::new([assign_statement(0, 1, 10)]),
            terminator: unreachable_terminator(),
        };

        assert_ne!(left, right);
    }

    #[test]
    fn basic_block_hash_is_structural() {
        let first = BasicBlock {
            statements: Box::new([assign_statement(0, 1, 10)]),
            terminator: return_terminator(),
        };
        let second = BasicBlock {
            statements: Box::new([assign_statement(1, 2, 20)]),
            terminator: return_terminator(),
        };
        let mut blocks = HashSet::new();

        blocks.insert(first.clone());
        blocks.insert(second.clone());
        blocks.insert(first);
        blocks.insert(second);

        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn basic_block_serde_round_trips_empty_statements() {
        let block = BasicBlock {
            statements: Box::new([]),
            terminator: return_terminator(),
        };

        assert_eq!(round_trip::<BasicBlock>(&block), block);
    }

    #[test]
    fn basic_block_serde_round_trips_non_empty_statements() {
        let block = BasicBlock {
            statements: Box::new([assign_statement(0, 1, 10), assign_statement(1, 2, 20)]),
            terminator: call_terminator(),
        };

        assert_eq!(round_trip::<BasicBlock>(&block), block);
    }

    #[test]
    fn basic_block_has_no_cleanup_field() {
        // Struct initialization with only `statements` and
        // `terminator` is the compile-time check. Adding
        // `is_cleanup: false` here would fail to compile.
        let block = BasicBlock {
            statements: Box::new([]),
            terminator: return_terminator(),
        };

        assert!(block.statements.is_empty());
    }

    #[test]
    fn basic_block_requires_non_optional_terminator() {
        // Struct initialization requires a `Terminator`. Replacing the
        // value below with `None` would fail to compile.
        let block = BasicBlock {
            statements: Box::new([]),
            terminator: return_terminator(),
        };

        assert_eq!(block.terminator.kind, TerminatorKind::Return);
    }
}
