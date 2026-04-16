# regseq-model-29: TerminatorKind and SwitchTargets

## Description

Implement the terminator types for the CFG-based IR. A terminator is the
final instruction of a basic block that transfers control flow to one or
more successor blocks.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 1, step 1.6)
- `docs/discussions/models/regseq-model/ir-reference.md` (Section 5: Terminators)
- `crates/regseq-model/src/ir/place.rs` (Place)
- `crates/regseq-model/src/ir/operand.rs` (Operand)
- `crates/regseq-model/src/ir/intrinsic.rs` (RegseqIntrinsic, IntrinsicMetadata)
- `crates/regseq-model/src/identity/` (BasicBlockId, FunctionId)

## Implementation Notes

Create `crates/regseq-model/src/ir/terminator.rs`. Update `ir/mod.rs`.

### Public API

```rust
// ir/terminator.rs

/// A terminator: semantic content only. Source location in BodySourceMap.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Terminator {
    pub kind: TerminatorKind,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum TerminatorKind {
    Goto(BasicBlockId),

    SwitchInt {
        discriminant: Operand,
        targets: SwitchTargets,
    },

    Call {
        callee: FunctionId,
        args: Vec<Operand>,
        destination: Place,
        target: Option<BasicBlockId>,
        error_target: Option<BasicBlockId>,
    },

    IntrinsicCall {
        intrinsic: RegseqIntrinsic,
        args: Vec<Operand>,
        metadata: Option<IntrinsicMetadata>,
        destination: Place,
        target: BasicBlockId,
        error_target: Option<BasicBlockId>,
    },

    Drop {
        place: Place,
        target: BasicBlockId,
    },

    Return,

    Unreachable,
}

/// Branch targets for SwitchInt.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct SwitchTargets {
    /// Discriminant values to match against.
    pub values: Vec<u128>,
    /// Target basic blocks. Length is values.len() + 1 -- last is default.
    pub targets: Vec<BasicBlockId>,
}
```

### Conventions

- 7 `TerminatorKind` variants.
- `IntrinsicCall.target` is NOT `Option` -- intrinsics always return.
  `Call.target` IS `Option` because user functions may diverge.
- `error_target` on Call and IntrinsicCall is populated by `?`-pattern
  folding during IR construction (Phase 5). `None` for infallible ops.
- `Drop` has no `error_target` -- `Drop::drop` returns `()`.
- `SwitchTargets.targets` has one more entry than `values` (last = default).
- **Option field serialization**: All `Option` fields (`Call.target`,
  `Call.error_target`, `IntrinsicCall.metadata`, `IntrinsicCall.error_target`)
  use `#[serde(skip_serializing_if = "Option::is_none")]` with
  `#[serde(default)]` so they are omitted from JSON when `None`, not
  serialized as `null`.
- **Serde**: TerminatorKind contains Place and Operand which have Symbol
  fields. Serde derives cannot be added until model-level serialization
  is built. The Option-field convention above will apply when serde is added.

## Non-Goals

- No Body composite (regseq-model-30).
- No `?`-pattern folding logic (Phase 5).

## Test Plan

- Construct each of the 7 `TerminatorKind` variants.
- `SwitchTargets` with 2 values and 3 targets (last is default).
- `IntrinsicCall` with `metadata: Some(IntrinsicMetadata::Register { ... })`.
- `IntrinsicCall` with `error_target: Some(bb)` for fallible intrinsic.
- `Call` with `error_target: None` for infallible call.
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against ir-reference.md
Section 5. Pay attention to the distinction between Call.target (Option)
and IntrinsicCall.target (not Option). Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `ir/terminator.rs` created with all types.
- Re-exported from `ir/mod.rs`.
- All tests pass. Codex review approved.
