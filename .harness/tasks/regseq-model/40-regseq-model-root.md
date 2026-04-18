# regseq-model-40: RegseqModel Root Struct, mod.rs, and lib.rs Update

## Description

Implement the `RegseqModel` root struct that owns the symbol interner,
source locations, root module, and format version. This is the capstone
of Phase 2 -- it assembles all model types into the single unified
representation.

Also finalize `model/mod.rs` with all re-exports and update
`crates/regseq-model/src/lib.rs` to declare `pub mod model`.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 2, step 2.10-2.11)
- `docs/discussions/models/regseq-model/spec.md` (Section 1: Overview, pipeline role)
- `crates/regseq-model/src/identity/symbol.rs` (SymbolInterner)
- `crates/regseq-model/src/identity/source.rs` (SourceOrigin)
- `crates/regseq-model/src/model/module.rs` (Module)
- `crates/regseq-model/src/lib.rs` (existing module declarations)

## Implementation Notes

Create `crates/regseq-model/src/model/regseq_model.rs`. Update
`model/mod.rs` with all re-exports. Update `lib.rs`.

### Public API

```rust
// model/regseq_model.rs

use std::collections::BTreeMap;

/// The unified Regseq Model: the single progressively-filled IR that
/// the compiler owns.
///
/// Starts with normalized extraction data (Phase 1: declarations
/// populated, bindings empty), then absorbs platform binding data
/// (Phase 2: bindings populated).
#[derive(Clone, Debug)]
pub struct RegseqModel {
    /// Interned symbols for all names and identifiers.
    pub symbols: SymbolInterner,
    /// Source locations for all declarations, keyed by DeclId.
    pub source_locations: BTreeMap<DeclId, SourceOrigin>,
    /// The root module (crate module) of the module tree.
    pub root_module: Module,
    /// Format version for serialization compatibility.
    pub format_version: u32,
}
```

### model/mod.rs

Re-export all Phase 2 model types:

```rust
mod framework;
mod type_def;
mod register;
mod constant;
mod function;
mod resources;
mod driver;
mod impl_block;
mod module;
mod regseq_model;

pub use framework::*;
pub use type_def::*;
pub use register::*;
pub use constant::*;
pub use function::*;
pub use resources::*;
pub use driver::*;
pub use impl_block::*;
pub use module::*;
pub use regseq_model::*;
```

### lib.rs update

Add `pub mod model;` to the existing module declarations.

### Conventions

- `RegseqModel` derives `Clone` and `Debug` but NOT `Eq`/`PartialEq`
  because `SymbolInterner` may not implement them (interners are
  typically compared by identity, not by content).
- `format_version` starts at 1. Incremented when serialization format
  changes.
- `source_locations` uses a `BTreeMap<DeclId, SourceOrigin>` rather
  than a flat `Vec` indexed by DeclId value. This avoids requiring
  dense DeclId allocation and allows sparse ID assignment.
  `BTreeMap` gives deterministic iteration order for serialization.
- The root module's kind is `Normal` (the crate module itself is not
  the driver module; the driver module is typically a submodule).

## Non-Goals

- No normalization logic (Phase 6).
- No join logic (Phase 6).
- No model-level serialization (deferred until needed).
- No consumer migration (Phase 7).

## Test Plan

- Construct a minimal `RegseqModel`: empty SymbolInterner, empty
  source_locations, a root Normal module with one Driver submodule
  containing a DriverStruct, format_version = 1.
- Verify the root module is accessible and its kind is Normal.
- Verify the driver submodule is accessible and its kind is Driver.
- Verify format_version is 1.
- Verify `pub mod model` is declared in lib.rs.
- Verify all model types are accessible from `regseq_model::model::*`.
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against spec.md
Section 1 (pipeline role), and verify all model types from steps
2.1-2.9 are re-exported from model/mod.rs. Verify lib.rs declares
the module. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `model/regseq_model.rs` created with `RegseqModel`.
- `model/mod.rs` finalized with all re-exports.
- `lib.rs` updated with `pub mod model`.
- All tests pass. Codex review approved.
