//! Body composite types for the Regseq Model IR.
//!
//! A [`Body`] is the executable content of a function: local variable
//! declarations and a control flow graph of basic blocks. Every
//! `Function` in the model owns exactly one `Body`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::identity::SourceOrigin;
use crate::identity::{BasicBlockId, Symbol};
use crate::ir::statement::Statement;
use crate::ir::terminator::{Terminator, TerminatorKind};
use crate::ir::types::RegseqType;

/// A function body: local declarations + control flow graph.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Body {
    /// Local variable declarations.
    ///
    /// Index 0 is the return place (`_0`). Indices `1..=arg_count` are
    /// function arguments. The rest are user variables and compiler
    /// temporaries.
    pub local_decls: Vec<LocalDecl>,

    /// Number of function arguments.
    pub arg_count: usize,

    /// Basic blocks forming the control flow graph.
    ///
    /// Block 0 is the entry block. Blocks are indexed by `BasicBlockId`.
    pub basic_blocks: Vec<BasicBlock>,

    /// Source locations for body-level nodes.
    pub source_map: BodySourceMap,

    /// Ordered regions within this body.
    pub ordered_regions: Vec<OrderedRegion>,
}

/// A local variable, temporary, or the return place.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct LocalDecl {
    /// The local's type.
    pub ty: RegseqType,
    /// Whether the binding is mutable (`let mut` vs `let`).
    pub mutable: bool,
    /// The user's variable name, if this local corresponds to a
    /// user-declared variable or function argument. `None` for
    /// compiler-generated temporaries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<Symbol>,
}

/// A basic block: a sequence of statements followed by a terminator.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct BasicBlock {
    /// Ordered statements executed sequentially.
    pub statements: Vec<Statement>,
    /// The block's terminator. Every block has exactly one.
    pub terminator: Terminator,
}

/// An ordered region defined by a single-entry single-exit (SESE)
/// invariant on the CFG.
#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct OrderedRegion {
    /// The single entry point of the region.
    pub entry: BasicBlockId,
    /// The single normal exit point of the region.
    pub exit: BasicBlockId,
}

/// Source locations for all nodes in a function body.
///
/// Structurally parallel to the body: one entry per local, one per
/// statement, one per terminator.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct BodySourceMap {
    /// Source location for the function body as a whole.
    pub body_span: SourceOrigin,
    /// Source locations for each local declaration (parallel to `local_decls`).
    pub locals: Vec<SourceOrigin>,
    /// Source locations for each basic block's contents (parallel to `basic_blocks`).
    pub blocks: Vec<BasicBlockSourceMap>,
}

/// Source locations for one basic block's contents.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct BasicBlockSourceMap {
    /// Source locations for each statement (parallel to `BasicBlock.statements`).
    pub statements: Vec<SourceOrigin>,
    /// Source location for the block's terminator.
    pub terminator: SourceOrigin,
}

impl Body {
    /// Compute the predecessor map for this body's CFG.
    ///
    /// Returns a map from each basic block to the list of blocks that
    /// have it as a successor (via any terminator edge: Goto, SwitchInt
    /// targets, Call/IntrinsicCall target and error_target, Drop target).
    pub fn predecessors(&self) -> BTreeMap<BasicBlockId, Vec<BasicBlockId>> {
        let mut preds: BTreeMap<BasicBlockId, Vec<BasicBlockId>> = BTreeMap::new();

        // Ensure every block has an entry (even if no predecessors).
        for i in 0..self.basic_blocks.len() {
            preds.entry(BasicBlockId(i as u32)).or_default();
        }

        for (i, block) in self.basic_blocks.iter().enumerate() {
            let src = BasicBlockId(i as u32);

            match &block.terminator.kind {
                TerminatorKind::Goto(target) => {
                    preds.entry(*target).or_default().push(src);
                }
                TerminatorKind::SwitchInt { targets, .. } => {
                    for target in &targets.targets {
                        preds.entry(*target).or_default().push(src);
                    }
                }
                TerminatorKind::Call {
                    target,
                    error_target,
                    ..
                } => {
                    if let Some(t) = target {
                        preds.entry(*t).or_default().push(src);
                    }
                    if let Some(e) = error_target {
                        preds.entry(*e).or_default().push(src);
                    }
                }
                TerminatorKind::IntrinsicCall {
                    target,
                    error_target,
                    ..
                } => {
                    preds.entry(*target).or_default().push(src);
                    if let Some(e) = error_target {
                        preds.entry(*e).or_default().push(src);
                    }
                }
                TerminatorKind::Drop { target, .. } => {
                    preds.entry(*target).or_default().push(src);
                }
                TerminatorKind::Return | TerminatorKind::Unreachable => {}
            }
        }

        preds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{LocalId, SourceLocation, SymbolInterner};
    use crate::ir::operand::{LiteralValue, Operand};
    use crate::ir::statement::StatementKind;
    use crate::ir::terminator::SwitchTargets;
    use crate::ir::types::ScalarType;

    fn compiler_generated() -> SourceOrigin {
        SourceOrigin::CompilerGenerated
    }

    fn dummy_source() -> SourceOrigin {
        let mut interner = SymbolInterner::new();
        let file = interner.intern_file("test.rs");
        SourceOrigin::Source(SourceLocation {
            file,
            start_line: 1,
            start_column: 1,
            end_line: 1,
            end_column: 10,
        })
    }

    // -- Minimal Body: 1 local, 1 block, Return --

    #[test]
    fn construct_minimal_body() {
        let body = Body {
            local_decls: vec![LocalDecl {
                ty: RegseqType::Unit,
                mutable: true,
                name: None,
            }],
            arg_count: 0,
            basic_blocks: vec![BasicBlock {
                statements: vec![],
                terminator: Terminator {
                    kind: TerminatorKind::Return,
                },
            }],
            source_map: BodySourceMap {
                body_span: dummy_source(),
                locals: vec![compiler_generated()],
                blocks: vec![BasicBlockSourceMap {
                    statements: vec![],
                    terminator: compiler_generated(),
                }],
            },
            ordered_regions: vec![],
        };

        // Source map is structurally parallel.
        assert_eq!(body.source_map.locals.len(), body.local_decls.len());
        assert_eq!(body.source_map.blocks.len(), body.basic_blocks.len());
    }

    // -- 3-block Body: entry -> SwitchInt -> return --

    #[test]
    fn three_block_body_predecessors() {
        // bb0: SwitchInt { discriminant: _1, targets: [0 -> bb1, default -> bb2] }
        // bb1: Return
        // bb2: Return
        let body = Body {
            local_decls: vec![
                LocalDecl {
                    ty: RegseqType::Unit,
                    mutable: true,
                    name: None,
                },
                LocalDecl {
                    ty: RegseqType::Scalar(ScalarType::U32),
                    mutable: false,
                    name: None,
                },
            ],
            arg_count: 1,
            basic_blocks: vec![
                BasicBlock {
                    statements: vec![],
                    terminator: Terminator {
                        kind: TerminatorKind::SwitchInt {
                            discriminant: Operand::Copy(crate::ir::place::Place {
                                local: LocalId(1),
                                projection: vec![],
                            }),
                            targets: SwitchTargets {
                                values: vec![0],
                                targets: vec![BasicBlockId(1), BasicBlockId(2)],
                            },
                        },
                    },
                },
                BasicBlock {
                    statements: vec![],
                    terminator: Terminator {
                        kind: TerminatorKind::Return,
                    },
                },
                BasicBlock {
                    statements: vec![],
                    terminator: Terminator {
                        kind: TerminatorKind::Return,
                    },
                },
            ],
            source_map: BodySourceMap {
                body_span: dummy_source(),
                locals: vec![compiler_generated(), compiler_generated()],
                blocks: vec![
                    BasicBlockSourceMap {
                        statements: vec![],
                        terminator: compiler_generated(),
                    },
                    BasicBlockSourceMap {
                        statements: vec![],
                        terminator: compiler_generated(),
                    },
                    BasicBlockSourceMap {
                        statements: vec![],
                        terminator: compiler_generated(),
                    },
                ],
            },
            ordered_regions: vec![],
        };

        let preds = body.predecessors();

        // bb0 has no predecessors (entry block).
        assert!(preds[&BasicBlockId(0)].is_empty());
        // bb1 has bb0 as predecessor.
        assert_eq!(preds[&BasicBlockId(1)], vec![BasicBlockId(0)]);
        // bb2 has bb0 as predecessor.
        assert_eq!(preds[&BasicBlockId(2)], vec![BasicBlockId(0)]);
    }

    // -- BodySourceMap structurally parallel --

    #[test]
    fn source_map_structurally_parallel() {
        let body = Body {
            local_decls: vec![
                LocalDecl {
                    ty: RegseqType::Unit,
                    mutable: true,
                    name: None,
                },
                LocalDecl {
                    ty: RegseqType::Scalar(ScalarType::U32),
                    mutable: false,
                    name: None,
                },
            ],
            arg_count: 1,
            basic_blocks: vec![
                BasicBlock {
                    statements: vec![Statement {
                        kind: StatementKind::StorageLive(LocalId(1)),
                    }],
                    terminator: Terminator {
                        kind: TerminatorKind::Goto(BasicBlockId(1)),
                    },
                },
                BasicBlock {
                    statements: vec![],
                    terminator: Terminator {
                        kind: TerminatorKind::Return,
                    },
                },
            ],
            source_map: BodySourceMap {
                body_span: dummy_source(),
                locals: vec![compiler_generated(), compiler_generated()],
                blocks: vec![
                    BasicBlockSourceMap {
                        statements: vec![compiler_generated()],
                        terminator: compiler_generated(),
                    },
                    BasicBlockSourceMap {
                        statements: vec![],
                        terminator: compiler_generated(),
                    },
                ],
            },
            ordered_regions: vec![],
        };

        assert_eq!(body.source_map.locals.len(), body.local_decls.len());
        assert_eq!(body.source_map.blocks.len(), body.basic_blocks.len());
        assert_eq!(
            body.source_map.blocks[0].statements.len(),
            body.basic_blocks[0].statements.len()
        );
    }

    // -- OrderedRegion --

    #[test]
    fn construct_ordered_region() {
        let region = OrderedRegion {
            entry: BasicBlockId(2),
            exit: BasicBlockId(5),
        };
        assert_eq!(region.entry, BasicBlockId(2));
        assert_eq!(region.exit, BasicBlockId(5));
    }
}
