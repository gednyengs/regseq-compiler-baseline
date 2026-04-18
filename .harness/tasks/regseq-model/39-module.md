# regseq-model-39: Module

## Description

Implement the module model type. A module is the structural container
for all model items, with a kind (Driver or Normal), nested submodules,
and dependency sets for abstractions and intrinsics.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 2, step 2.9)
- `docs/discussions/models/regseq-model/spec.md` (Section 2: Module Tree, Section 3: Module Items)
- `crates/regseq-model/src/identity/` (ModuleId, Symbol)
- `crates/regseq-model/src/ir/intrinsic.rs` (RegseqAbstraction, RegseqIntrinsic)
- `crates/regseq-model/src/model/` (all preceding model types)

## Implementation Notes

Create `crates/regseq-model/src/model/module.rs`. Update `model/mod.rs`.

### Public API

```rust
// model/module.rs

use std::collections::BTreeSet;

/// The kind of a module.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum ModuleKind {
    /// Contains the driver struct, Regseq trait impls, and driver impl blocks.
    Driver,
    /// Contains only general-purpose items.
    Normal,
}

/// A module in the Regseq model tree.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Module {
    pub id: ModuleId,
    pub name: Symbol,
    pub kind: ModuleKind,

    // --- Items (Section 3) ---

    /// Nested submodules (Section 3.1).
    pub submodules: Vec<Module>,
    /// Top-level constants and statics (Section 3.2).
    pub constants: Vec<Constant>,
    /// Free functions (Section 3.3).
    pub free_functions: Vec<Function>,
    /// Type definitions: structs and enums (Section 3.4).
    pub type_defs: Vec<TypeDef>,
    /// Register descriptors (Section 3.5).
    pub register_descriptors: Vec<RegisterDescriptor>,
    /// Register blocks (Section 3.6).
    pub register_blocks: Vec<RegisterBlock>,
    /// Impl blocks for non-driver types (Section 3.7).
    pub impl_blocks: Vec<ImplBlock>,

    // --- Driver-only items (present when kind == Driver) ---

    /// The driver struct declaration (Section 4). None for Normal modules.
    pub driver: Option<DriverStruct>,
    /// Regseq trait impls for the driver (Section 5). Empty for Normal modules.
    pub trait_impls: Vec<ImplBlock>,
    /// Driver impl blocks with helpers and ISRs (Section 6). Empty for Normal modules.
    pub driver_impl_blocks: Vec<ImplBlock>,

    // --- Dependency sets (Section 10) ---

    /// Regseq abstractions referenced within this module.
    pub abstractions_used: BTreeSet<RegseqAbstraction>,
    /// Regseq intrinsic functions invoked within this module.
    pub intrinsics_used: BTreeSet<RegseqIntrinsic>,
}
```

### Conventions

- `Module` contains Symbol fields and nested types, so no standalone
  serde. `ModuleKind` can derive serde.
- Exactly one module in the tree has kind `Driver`.
- Driver-only fields (`driver`, `trait_impls`, `driver_impl_blocks`)
  are `Option`/empty `Vec` for Normal modules.
- `submodules` allows arbitrary nesting.
- `abstractions_used` and `intrinsics_used` are `BTreeSet` for
  deterministic ordering.

## Non-Goals

- No RegseqModel root struct (regseq-model-40).
- No module tree construction logic (Phase 5/6).

## Test Plan

- Construct a Normal module with 1 type_def, 1 free_function, 1 constant.
- Construct a Driver module with a DriverStruct, 1 trait_impl
  (Lifecycle), 1 driver_impl_block, and non-empty abstractions_used.
- Construct a module tree: root Normal module containing a Driver
  submodule and a Normal submodule.
- Verify exactly one module has kind == Driver in the tree.
- Serde round-trip for `ModuleKind` both variants.
- Verify abstractions_used and intrinsics_used are BTreeSet
  (deterministic iteration order).
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against spec.md
Sections 2 and 3. Verify all item categories are present, Driver/Normal
distinction is correct, and dependency sets match Section 10. Iterate
until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `model/module.rs` created with Module and ModuleKind types.
- Re-exported from `model/mod.rs`.
- All tests pass. Codex review approved.
