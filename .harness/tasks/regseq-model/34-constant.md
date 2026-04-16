# regseq-model-34: Constant

## Description

Implement the constant model type for top-level and associated constant
items.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 2, step 2.4)
- `docs/discussions/models/regseq-model/spec.md` (Section 3.2: Constants and Statics, Section 6: Associated Constants)
- `crates/regseq-model/src/identity/` (ConstantId, Symbol)
- `crates/regseq-model/src/ir/types.rs` (RegseqType)
- `crates/regseq-model/src/ir/operand.rs` (LiteralValue)

## Implementation Notes

Create `crates/regseq-model/src/model/constant.rs`. Update `model/mod.rs`.

### Public API

```rust
// model/constant.rs

/// A named constant or static item.
///
/// Used for both top-level constants (Section 3.2) and associated
/// constants in impl blocks (Section 6).
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Constant {
    pub id: ConstantId,
    pub name: Symbol,
    pub ty: RegseqType,
    pub value: LiteralValue,
}
```

### Conventions

- `Constant` contains Symbol fields, so no standalone serde derives.
- `value` is always a `LiteralValue` -- non-literal constant
  initializers are not supported in the current spec.
- Same type is used for top-level constants (owned by Module) and
  associated constants (owned by ImplBlock).

## Non-Goals

- No module or impl block ownership (regseq-model-38, 39).

## Test Plan

- Construct a `Constant` with `LiteralValue::U32(0x07ff)`.
- Construct a `Constant` with `LiteralValue::Bool(true)`.
- Verify equality: two constants with same fields are equal.
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against spec.md
Sections 3.2 and 6 (Associated Constants). Verify the type covers
both use sites. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `model/constant.rs` created with `Constant` type.
- Re-exported from `model/mod.rs`.
- All tests pass. Codex review approved.
