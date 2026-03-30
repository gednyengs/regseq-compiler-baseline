# Task: regseq-facade-01 - Crate initialization

## Description

Activate the `regseq` facade crate as the public Layer 1 authoring surface for
driver specifications.

This task creates a compiling library crate with crate-level documentation and
the dependency edge to `regseq-macros`. It does not define resource wrappers,
traits, intrinsics, or final macro re-exports yet.

## Input Context

Read before starting:
- crates/regseq/README.md
- Cargo.toml
- docs/plans/asplos-implementation-plan.md
- docs/guide/implementation-guide.md

## Output Specification

Create:
- crates/regseq/Cargo.toml
- crates/regseq/src/lib.rs
- crates/regseq/tests/smoke.rs

Modify:
- Cargo.toml (add `"crates/regseq"` to workspace members)

## Implementation Notes

Cargo.toml:

```toml
[package]
name = "regseq"
version = "0.1.0"
edition = "2021"

[dependencies]
regseq-macros = { path = "../regseq-macros" }
```

lib.rs:
- Add a crate-level doc comment describing `regseq` as the Layer 1 facade.
- Record that this crate hosts driver-author-facing types, traits, intrinsics,
  and macro re-exports in later phases.
- Add a narrow placeholder module structure only if needed to keep the file
  under the project size limit.
- Do not define real DSL types or intrinsic functions in this task.
- Do not pull rustc-specific logic into this crate.

Because `regseq-macros-01` leaves the proc-macro crate intentionally empty,
this task should not force a real macro re-export yet. The dependency edge is
the topology decision that matters in Phase A.

tests/smoke.rs:
- Smoke test that imports the crate successfully.

## Test Plan

Create:
- crates/regseq/tests/smoke.rs

Tests to implement:
1. Smoke test: verify the crate compiles and can be imported.

Test command: `cargo test -p regseq`

## Acceptance Criteria

- `regseq` is a live workspace member
- The crate depends on `regseq-macros`
- The crate documentation records the facade role without defining real DSL
  behavior prematurely
- `cargo test -p regseq` passes
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
