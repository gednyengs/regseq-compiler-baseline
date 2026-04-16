# regseq-model-26: Rvalue, BorrowKind, CastKind, AggregateKind

## Description

Implement the `Rvalue` enum and its supporting types. An rvalue is a
value-producing expression on the right side of an assignment. Rvalues
are purely standard -- all domain-specific operations flow through
IntrinsicCall terminators, not rvalues.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 1, step 1.3)
- `docs/discussions/models/regseq-model/ir-reference.md` (Section 3: Rvalues)
- `crates/regseq-model/src/ir/place.rs` (Place)
- `crates/regseq-model/src/ir/operand.rs` (Operand)
- `crates/regseq-model/src/ir/types.rs` (RegseqType)
- `crates/regseq-model/src/ir/ops.rs` (BinOp, UnOp)
- `crates/regseq-model/src/identity/` (TypeDefId)

## Implementation Notes

Create `crates/regseq-model/src/ir/rvalue.rs`. Update `ir/mod.rs`.

### Public API

```rust
// ir/rvalue.rs

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum Rvalue {
    Use(Operand),
    Repeat(Operand, usize),
    Ref(BorrowKind, Place),
    Cast(CastKind, Operand, RegseqType),
    BinaryOp(BinOp, Operand, Operand),
    UnaryOp(UnOp, Operand),
    Discriminant(Place),
    Aggregate(AggregateKind, Vec<Operand>),
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum BorrowKind {
    Shared,
    Mutable,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum CastKind {
    IntToInt,
    IntToFloat,
    FloatToInt,
    FloatToFloat,
    PtrToPtr,
    Transmute,
}

#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum AggregateKind {
    Array(RegseqType),
    Tuple,
    Adt { type_id: TypeDefId, variant_index: u32 },
}
```

### Conventions

- Serde: adjacently-tagged for all enums.
- `Rvalue::BinaryOp` takes the op inline (not boxed) since BinOp is Copy.
- `AggregateKind::Adt` carries `variant_index` which is 0 for structs.

## Non-Goals

- No Statement or Terminator types.
- No domain-specific operations (those are IntrinsicCall terminators).

## Test Plan

- Construct each of the 8 `Rvalue` variants.
- Construct each `BorrowKind`, `CastKind`, `AggregateKind` variant.
- `Rvalue::BinaryOp(BinOp::Add, ...)` composes correctly with Operand.
- `Rvalue::Aggregate(AggregateKind::Adt { type_id, variant_index: 0 }, fields)` for struct construction.
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against ir-reference.md
Section 3. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `ir/rvalue.rs` created with all 4 types.
- Re-exported from `ir/mod.rs`.
- All tests pass. Codex review approved.
