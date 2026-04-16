# regseq-model-25: Operand and LiteralValue

## Description

Implement the `Operand` and `LiteralValue` types. An operand is a value
source for expressions -- the atomic unit that flows into rvalues,
statements, and terminators.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 1, step 1.2)
- `docs/discussions/models/regseq-model/ir-reference.md` (Section 1: Operands)
- `crates/regseq-model/src/identity/` (ConstantId from Phase 0)
- `crates/regseq-model/src/ir/place.rs` (Place from regseq-model-24)

## Implementation Notes

Create `crates/regseq-model/src/ir/operand.rs`. Update `ir/mod.rs`.

### Public API

```rust
// ir/operand.rs

/// An operand in the Regseq IR -- a value source for expressions.
#[derive(Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum Operand {
    /// Load a value from a place (bitwise copy).
    Copy(Place),
    /// Load a value from a place, invalidating the source.
    Move(Place),
    /// Reference to a named constant declaration.
    Constant(ConstantId),
    /// Inline scalar or boolean value.
    Literal(LiteralValue),
}

/// Inline constant values that appear directly in expressions.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum LiteralValue {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    Usize(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    Isize(i64),
    Bool(bool),
}
```

### Conventions

- `LiteralValue` is `Copy` -- all variants are small scalars.
- `Usize(u64)` and `Isize(i64)` use 64-bit backing because actual width
  is target-dependent. Backend narrows based on platform `arch` field.
- `Unit` and `Never` have no literal representations.
- Serde: adjacently-tagged for both enums.

## Non-Goals

- No Rvalue types (regseq-model-26).
- No type inference or validation.

## Test Plan

- Construct each of the 4 `Operand` variants.
- Construct each of the 11 `LiteralValue` variants.
- `LiteralValue` is `Copy`: assign to second variable, both equal.
- Equality: `LiteralValue::U32(42) == LiteralValue::U32(42)`,
  `LiteralValue::U32(42) != LiteralValue::U32(43)`,
  `LiteralValue::U32(42) != LiteralValue::I32(42)`.
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against this spec and
ir-reference.md Section 1. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `ir/operand.rs` created with `Operand` (4 variants) and `LiteralValue` (11 variants).
- Re-exported from `ir/mod.rs`.
- All tests pass. Codex review approved.
