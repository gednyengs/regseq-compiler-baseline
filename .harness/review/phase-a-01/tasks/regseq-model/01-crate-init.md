# Task: regseq-model-01 - Crate initialization

## Description

Add and activate `regseq-model` as the rustc-independent internal model crate.

This task records the topology decision from the implementation plan: models
for declarations, platform bindings, joined lowering inputs, serialization,
and review artifacts live in `regseq-model`, while extraction remains in
`regseq-driver`.

The task is limited to crate activation and boundary documentation. It does
not add declaration-model types, binding formats, or serializer behavior.

## Input Context

Read before starting:
- Cargo.toml
- docs/plans/asplos-implementation-plan.md
- docs/guide/implementation-guide.md

## Output Specification

Create:
- crates/regseq-model/README.md
- crates/regseq-model/Cargo.toml
- crates/regseq-model/src/lib.rs
- crates/regseq-model/tests/smoke.rs

Modify:
- Cargo.toml (add `"crates/regseq-model"` to workspace members)

## Implementation Notes

README.md:
- Document the crate as the rustc-independent home for declaration models,
  Layer 2 platform-binding models, joined lowering inputs, serialization, and
  debug artifacts.
- State explicitly that HIR and MIR extraction stays in `regseq-driver`.

Cargo.toml:

```toml
[package]
name = "regseq-model"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = "2"
```

lib.rs:
- Add a crate-level doc comment that matches the implementation plan.
- Add a small placeholder module layout that names future responsibility areas
  without adding real model types yet. Acceptable stubs are private modules
  such as `declarations`, `bindings`, and `joined`.
- Do not depend on rustc crates.
- Do not place extraction hooks or rustc span types here.

tests/smoke.rs:
- Smoke test that imports the crate successfully.

## Test Plan

Create:
- crates/regseq-model/tests/smoke.rs

Tests to implement:
1. Smoke test: verify the crate compiles and can be imported.

Test command: `cargo test -p regseq-model`

## Acceptance Criteria

- `regseq-model` exists and is a live workspace member
- The crate has no rustc dependency
- The crate documentation states the Layer 1 plus Layer 2 model boundary and
  the extraction split with `regseq-driver`
- `cargo test -p regseq-model` passes
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
