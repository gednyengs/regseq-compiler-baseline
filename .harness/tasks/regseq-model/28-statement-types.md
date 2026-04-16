# regseq-model-28: StatementKind and MirIntrinsic

## Description

Implement the statement types for the CFG-based IR. A statement is an
instruction within a basic block that does not transfer control flow.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 1, step 1.5)
- `docs/discussions/models/regseq-model/ir-reference.md` (Section 4: Statements)
- `crates/regseq-model/src/ir/place.rs` (Place)
- `crates/regseq-model/src/ir/operand.rs` (Operand)
- `crates/regseq-model/src/ir/rvalue.rs` (Rvalue)
- `crates/regseq-model/src/identity/` (LocalId)

## Implementation Notes

Create `crates/regseq-model/src/ir/statement.rs`. Update `ir/mod.rs`.

### Public API

```rust
// ir/statement.rs

/// A statement: semantic content only. Source location is in BodySourceMap.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct Statement {
    pub kind: StatementKind,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum StatementKind {
    /// Assignment: evaluate rvalue, store in place.
    Assign(Place, Rvalue),

    /// Write the discriminant tag for an enum variant.
    SetDiscriminant { place: Place, variant_index: u32 },

    /// Mark a local's stack storage as allocated.
    StorageLive(LocalId),

    /// Mark a local's stack storage as freed.
    StorageDead(LocalId),

    /// MIR-level non-diverging intrinsic (distinct from RegseqIntrinsic).
    MirIntrinsic(MirIntrinsic),
}

/// Non-diverging intrinsics carried from MIR.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum MirIntrinsic {
    /// Optimizer hint: assume this boolean is true.
    Assume(Operand),

    /// Copy elements from source to destination (non-overlapping).
    CopyNonOverlapping {
        src: Operand,
        dst: Operand,
        count: Operand,
    },
}
```

### Conventions

- 5 `StatementKind` variants, 2 `MirIntrinsic` variants.
- `MirIntrinsic` is named to distinguish from `RegseqIntrinsic` (domain
  operations). MirIntrinsic is a statement (always completes, no control
  flow); RegseqIntrinsic flows through IntrinsicCall terminators.
- Serde: adjacently-tagged for all enums.

## Non-Goals

- No Terminator types (regseq-model-29).
- No Body composite (regseq-model-30).

## Test Plan

- Construct each of the 5 `StatementKind` variants.
- Construct each of the 2 `MirIntrinsic` variants.
- `Statement { kind: StatementKind::Assign(place, rvalue) }` composes
  correctly with Place and Rvalue from earlier tasks.
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against ir-reference.md
Section 4. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `ir/statement.rs` created with all types.
- Re-exported from `ir/mod.rs`.
- All tests pass. Codex review approved.
