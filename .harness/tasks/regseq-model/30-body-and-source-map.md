# regseq-model-30: Body, BasicBlock, LocalDecl, OrderedRegion, BodySourceMap

## Description

Implement the body composite types: the top-level `Body` struct that
holds the CFG, local declarations, source map, and ordered regions.
This is the capstone of Phase 1 -- it assembles all IR types into the
function body representation.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 1, step 1.7)
- `docs/discussions/models/regseq-model/ir-reference.md` (Section 6: Body Structure)
- `crates/regseq-model/src/ir/statement.rs` (Statement)
- `crates/regseq-model/src/ir/terminator.rs` (Terminator)
- `crates/regseq-model/src/ir/types.rs` (RegseqType)
- `crates/regseq-model/src/identity/` (LocalId, BasicBlockId, Symbol, SourceOrigin)

## Implementation Notes

Create `crates/regseq-model/src/ir/body.rs`. Update `ir/mod.rs`.

### Public API

```rust
// ir/body.rs
use std::collections::BTreeMap;

/// A function body: local declarations + control flow graph.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Body {
    pub local_decls: Vec<LocalDecl>,
    pub arg_count: usize,
    pub basic_blocks: Vec<BasicBlock>,
    pub source_map: BodySourceMap,
    pub ordered_regions: Vec<OrderedRegion>,
}

/// A local variable, temporary, or the return place.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct LocalDecl {
    pub ty: RegseqType,
    pub mutable: bool,
    pub name: Option<Symbol>,
}

/// A basic block: statements followed by a terminator.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct BasicBlock {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}

/// An ordered region (SESE invariant on the CFG).
#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct OrderedRegion {
    pub entry: BasicBlockId,
    pub exit: BasicBlockId,
}

/// Source locations for all nodes in a function body.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct BodySourceMap {
    pub body_span: SourceOrigin,
    pub locals: Vec<SourceOrigin>,
    pub blocks: Vec<BasicBlockSourceMap>,
}

/// Source locations for one basic block's contents.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct BasicBlockSourceMap {
    pub statements: Vec<SourceOrigin>,
    pub terminator: SourceOrigin,
}

impl Body {
    /// Compute the predecessor map for this body's CFG.
    pub fn predecessors(&self) -> BTreeMap<BasicBlockId, Vec<BasicBlockId>> {
        // Walk all terminators, collect predecessor edges.
        todo!()
    }
}
```

### Conventions

- `local_decls[0]` is the return place (`_0`).
- `local_decls[1..=arg_count]` are function arguments.
- `basic_blocks[0]` is the entry block.
- `BasicBlock.terminator` is non-optional (unlike MIR).
- `BodySourceMap` is structurally parallel to `Body` -- index i in
  source map corresponds to index i in the body.
- `Body::predecessors()` is computed, not stored. At driver-spec scale
  (tens of blocks), recomputation is trivial.

## Non-Goals

- No model structure types (Module, Function) -- Phase 2.
- No IR construction logic -- Phase 5.
- No standalone serde on Body or types containing Symbol fields
  (LocalDecl.name is a Symbol -- model-level serialization concern).

## Test Plan

- Construct a minimal Body: 1 local (return place), 1 block with
  `Return` terminator, source map with 1 local entry and 1 block entry
  (structurally parallel), no ordered regions.
- Construct a 3-block Body: entry -> branch (SwitchInt) -> return.
  Verify `predecessors()` returns correct map.
- `BodySourceMap` structurally parallel: `locals.len() == local_decls.len()`,
  `blocks.len() == basic_blocks.len()`.
- `OrderedRegion` with entry/exit block IDs.
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against ir-reference.md
Section 6. Verify `predecessors()` handles all terminator edge types
(Goto, SwitchInt targets, Call target+error_target, IntrinsicCall
target+error_target, Drop target). Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `ir/body.rs` created with all types and `predecessors()` method.
- Re-exported from `ir/mod.rs`.
- All tests pass. Codex review approved.
