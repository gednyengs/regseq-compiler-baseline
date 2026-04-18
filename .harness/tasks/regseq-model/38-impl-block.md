# regseq-model-38: ImplBlock

## Description

Implement the impl block model type for inherent and trait impl blocks
on non-driver types. The same type also represents Regseq trait impls
and driver impl blocks (their distinction comes from structural position
in the module, not from the ImplBlock type itself).

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 2, step 2.8)
- `docs/discussions/models/regseq-model/spec.md` (Section 3.7: Impl Blocks, Section 5: Regseq Trait Impls, Section 6: Driver Impl Blocks)
- `crates/regseq-model/src/identity/` (TypeDefId, Symbol)
- `crates/regseq-model/src/model/function.rs` (Function)
- `crates/regseq-model/src/model/constant.rs` (Constant)

## Implementation Notes

Create `crates/regseq-model/src/model/impl_block.rs`. Update `model/mod.rs`.

### Public API

```rust
// model/impl_block.rs

/// An impl block: inherent or trait implementation.
///
/// Used for:
/// - Inherent impl blocks on non-driver types (Section 3.7)
/// - Trait impl blocks on non-driver types (Section 3.7)
/// - Regseq trait impls for the driver struct (Section 5)
/// - Driver impl blocks with helpers/ISRs (Section 6)
///
/// The distinction between these use sites comes from structural
/// position in the Module, not from this type.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ImplBlock {
    /// The TypeDefId of the type being implemented.
    pub target_type: TypeDefId,
    /// The trait being implemented, if this is a trait impl.
    /// None for inherent impls.
    pub trait_name: Option<Symbol>,
    /// Methods defined in this impl block.
    pub methods: Vec<Function>,
    /// Associated constants defined in this impl block.
    pub associated_constants: Vec<Constant>,
}
```

### Conventions

- `ImplBlock` contains Symbol and Function (which contains Body) fields,
  so no standalone serde derives.
- `trait_name` is `None` for inherent impls, `Some` for trait impls.
- For Regseq trait impls (Lifecycle, Serial, etc.), `trait_name` is
  the trait's name.
- For driver impl blocks (Section 6), `trait_name` is `None` and
  `target_type` is the driver struct's TypeDefId.
- `associated_constants` may be populated for any impl block kind.
  In practice, Regseq trait impls (Lifecycle, Serial, etc.) do not
  define associated constants, but the type does not enforce this
  restriction. The field is always present; it is simply empty when
  no associated constants exist.

## Non-Goals

- No callback context classification (future attrs concern).
- No ISR binding metadata (future attrs concern).
- No module ownership (regseq-model-39).

## Test Plan

- Construct an inherent ImplBlock (trait_name = None) with 2 methods
  and 1 associated constant.
- Construct a trait ImplBlock (trait_name = Some("Lifecycle")) with
  init/deinit methods.
- Construct a driver ImplBlock with helper methods and an ISR handler.
- Verify trait_name is None vs Some as expected.
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against spec.md
Sections 3.7, 5, and 6. Verify the ImplBlock type covers all four
use sites. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `model/impl_block.rs` created with ImplBlock type.
- Re-exported from `model/mod.rs`.
- All tests pass. Codex review approved.
