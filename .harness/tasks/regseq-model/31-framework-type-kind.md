# regseq-model-31: FrameworkTypeKind and TypeDefClassification

## Description

Implement the framework type kind enum and type definition classification
types. These are used by type definitions (regseq-model-32) to classify
types as user-defined, abstraction, or framework.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 2, step 2.1)
- `docs/discussions/models/regseq-model/spec.md` (Section 3.4: Type Definitions)
- `crates/regseq-model/src/ir/intrinsic.rs` (RegseqAbstraction)

## Implementation Notes

Create `crates/regseq-model/src/model/framework.rs`. Create
`crates/regseq-model/src/model/mod.rs` with re-exports.

### Public API

```rust
// model/framework.rs

use serde::{Serialize, Deserialize};
use crate::ir::RegseqAbstraction;

/// Identifies a specific Regseq framework type.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum FrameworkTypeKind {
    RegseqError,
    SerialErrors,
    RxChar,
    SerialConfig,
    DataBits,
    Parity,
    StopBits,
    FlowControl,
    Duration,
    IrqResult,
}

/// Classification of a type definition.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum TypeDefClassification {
    /// Spec author's types with no special Regseq semantics.
    UserDefined,
    /// Regseq abstraction type discovered via #[rseq::abstraction("...")].
    Abstraction(RegseqAbstraction),
    /// Regseq framework type discovered via #[rseq::framework_type("...")].
    Framework(FrameworkTypeKind),
}
```

### Conventions

- 10 `FrameworkTypeKind` variants matching spec.md Section 3.4.
- 3 `TypeDefClassification` variants.
- Adjacently-tagged serde for all enums.
- `FrameworkTypeKind` is `Copy`; `TypeDefClassification` is `Clone`
  (because `RegseqAbstraction` is `Copy`, but keeping `Clone` for
  uniformity with other model types).

## Non-Goals

- No type definition struct (regseq-model-32).
- No attribute parsing or recognition logic.

## Test Plan

- Construct each of the 10 `FrameworkTypeKind` variants.
- Construct each of the 3 `TypeDefClassification` variants.
- Serde round-trip for all variants (serialize to JSON, deserialize back,
  compare for equality).
- Verify `FrameworkTypeKind` is `Copy`.
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against spec.md
Section 3.4. Verify all 10 framework type variants are present and
match the attribute string mappings. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `model/framework.rs` created with both types.
- `model/mod.rs` created with re-exports.
- All tests pass. Codex review approved.
