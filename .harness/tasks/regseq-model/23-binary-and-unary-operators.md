# regseq-model-23: Binary and unary operators

## Description

Implement the operator enums for the Regseq IR: `BinOp` (26 variants)
and `UnOp` (2 variants).

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 0, step 0.6)
- `docs/discussions/models/regseq-model/ir-reference.md` (Section 3: BinOp, UnOp)

## Implementation Notes

Create `crates/regseq-model/src/ir/ops.rs`. Update `ir/mod.rs` to
re-export.

### Public API

```rust
// ir/ops.rs

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub enum BinOp {
    // Arithmetic -- wrapping
    Add, Sub, Mul, Div, Rem,
    // Arithmetic -- UB on overflow (unchecked)
    AddUnchecked, SubUnchecked, MulUnchecked,
    // Arithmetic -- returns (result, overflow_bool)
    AddWithOverflow, SubWithOverflow, MulWithOverflow,
    // Bitwise
    BitXor, BitAnd, BitOr,
    // Shifts
    Shl, Shr,
    ShlUnchecked, ShrUnchecked,
    // Comparison
    Eq, Ne, Lt, Le, Gt, Ge,
    // Three-way comparison
    Cmp,
    // Pointer arithmetic
    Offset,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub enum UnOp {
    Not,
    Neg,
}
```

### Conventions

- All 26 `BinOp` variants from the IR reference are included for
  completeness, even though `*WithOverflow` variants do not appear
  in practice under release-profile MIR semantics.
- Serde: adjacently-tagged format
  (`#[serde(tag = "kind", content = "data")]`). Unit variants serialize
  as `{"kind": "Add"}` with no data field. Consistent with the
  project-wide enum serialization convention.
- No methods on `BinOp`/`UnOp` beyond derives. Helper methods
  (e.g., `is_comparison()`, `is_arithmetic()`) can be added later
  if needed but are not required for this task.

## Non-Goals

- No Rvalue or expression types (Phase 1).
- No semantic analysis of operators.

## Test Plan

- Construct each of the 26 `BinOp` variants and 2 `UnOp` variants.
- Equality: `BinOp::Add == BinOp::Add`, `BinOp::Add != BinOp::Sub`.
- Serde round-trip for every variant of both enums.
- Copy semantics: assign to a second variable, both are equal.

## Review

After tests pass, use `codex:rescue` to review. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `ir/ops.rs` created with `BinOp` (26 variants) and `UnOp` (2 variants).
- `ir/mod.rs` re-exports both types.
- All tests pass. Codex review approved.
