# regseq-model-22: RegseqType, ScalarType, and Mutability

## Description

Implement the RegseqModel type system: `RegseqType` (10 variants),
`ScalarType` (10 variants), and `Mutability` (2 variants).

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 0, step 0.5)
- `docs/discussions/models/regseq-model/spec.md` (Section 8: Type System)
- `crates/regseq-model/src/identity/decl_id.rs` (needs `TypeDefId`)

## Implementation Notes

Create `crates/regseq-model/src/ir/types.rs` and
`crates/regseq-model/src/ir/mod.rs`. Add `pub mod ir;` to
`crates/regseq-model/src/lib.rs`.

### Public API

```rust
// ir/types.rs

#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum RegseqType {
    Scalar(ScalarType),
    Bool,
    Unit,
    Never,
    Option(Box<RegseqType>),
    Result(Box<RegseqType>, Box<RegseqType>),
    Array(Box<RegseqType>, usize),
    Slice(Box<RegseqType>),
    Ref(Box<RegseqType>, Mutability),
    Defined(TypeDefId),
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum ScalarType {
    U8, U16, U32, U64,
    I8, I16, I32, I64,
    Usize, Isize,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Mutability {
    Shared,
    Mutable,
}
```

### Conventions

- Recursive variants (`Option`, `Result`, `Array`, `Slice`, `Ref`) use
  `Box` to keep `RegseqType` a finite-size enum.
- `Defined(TypeDefId)` is the uniform representation for all struct/enum
  types. The classification (UserDefined/Abstraction/Framework) lives on
  the type definition, not in the type reference.
- Serde: ALL enums use adjacently-tagged format
  (`#[serde(tag = "kind", content = "data")]`). This applies to
  `RegseqType`, `ScalarType`, and `Mutability` uniformly. Unit variants
  (e.g., `ScalarType::U32`) serialize as `{"kind": "U32"}` with no data
  field.
- `Bool` is separate from `ScalarType` because it has no bit-width and
  maps to C's `_Bool`.
- `Debug` impls should be readable (e.g., `Scalar(U32)`, `Ref(Bool, Mutable)`).

## Non-Goals

- No `FrameworkTypeKind` or `TypeDefClassification` (Phase 2).
- No type construction from rustc `ty::Ty` (Phase 5).
- No LiteralValue (regseq-model-23 handles this indirectly, but
  LiteralValue lives in ir/operand.rs which is Phase 1).

## Test Plan

- Construct each of the 10 `RegseqType` variants.
- Nested types: `Option(Scalar(U32))`, `Result(Unit, Defined(id))`,
  `Ref(Array(Scalar(U8), 4), Mutable)`.
- Equality: `Scalar(U32) == Scalar(U32)`, `Scalar(U32) != Scalar(U16)`.
- Serde round-trip for each variant, including nested types.
- `ScalarType` and `Mutability` round-trip independently.

## Review

After tests pass, use `codex:rescue` to review. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `ir/types.rs` created with all types.
- `ir/mod.rs` re-exports `RegseqType`, `ScalarType`, `Mutability`.
- `lib.rs` updated with `pub mod ir;`.
- All tests pass. Codex review approved.
