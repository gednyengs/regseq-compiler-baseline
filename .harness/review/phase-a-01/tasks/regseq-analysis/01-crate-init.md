# Task: regseq-analysis-01 - Crate initialization

## Description

Activate the `regseq-analysis` crate as the rustc-independent analysis layer
over internal Regseq models.

This task records the dependency boundary that analysis consumes shared model
and diagnostics crates rather than owning extraction. It does not implement
analysis pass traits or concrete validation logic yet.

## Input Context

Read before starting:
- crates/regseq-analysis/README.md
- Cargo.toml
- docs/plans/asplos-implementation-plan.md
- docs/guide/implementation-guide.md

## Output Specification

Create:
- crates/regseq-analysis/Cargo.toml
- crates/regseq-analysis/src/lib.rs
- crates/regseq-analysis/tests/smoke.rs

Modify:
- Cargo.toml (add `"crates/regseq-analysis"` to workspace members)

## Implementation Notes

Cargo.toml:

```toml
[package]
name = "regseq-analysis"
version = "0.1.0"
edition = "2021"

[dependencies]
regseq-config = { path = "../regseq-config" }
regseq-diagnostics = { path = "../regseq-diagnostics" }
regseq-model = { path = "../regseq-model" }
thiserror = "2"
```

lib.rs:
- Add a crate-level doc comment describing the crate as the home for analysis
  and validation passes over rustc-independent models.
- Document explicitly that rustc extraction remains in `regseq-driver`.
- Do not define real pass traits or implement SYN, SHAD, DMA, or propagation
  rules in this task.
- Keep any placeholder module layout private unless a public item is required
  to make tests meaningful.

tests/smoke.rs:
- Smoke test that imports the crate successfully.

## Test Plan

Create:
- crates/regseq-analysis/tests/smoke.rs

Tests to implement:
1. Smoke test: verify the crate compiles and can be imported.

Test command: `cargo test -p regseq-analysis`

## Acceptance Criteria

- `regseq-analysis` is a live workspace member
- The crate depends on `regseq-model`, `regseq-diagnostics`, and `regseq-config`
- The crate documentation preserves the extraction-vs-analysis split
- `cargo test -p regseq-analysis` passes
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
