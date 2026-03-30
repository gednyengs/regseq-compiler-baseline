# Task: regseq-diagnostics-01 - Crate initialization

## Description

Activate the `regseq-diagnostics` crate as the shared diagnostics support
crate for the toolchain.

This task is intentionally limited to crate activation and crate-level
documentation. It does not yet define diagnostic IDs, span formats, or
emission APIs.

## Input Context

Read before starting:
- crates/regseq-diagnostics/README.md
- Cargo.toml
- docs/plans/asplos-implementation-plan.md

## Output Specification

Create:
- crates/regseq-diagnostics/Cargo.toml
- crates/regseq-diagnostics/src/lib.rs
- crates/regseq-diagnostics/tests/smoke.rs

Modify:
- Cargo.toml (add `"crates/regseq-diagnostics"` to workspace members)

## Implementation Notes

Cargo.toml:

```toml
[package]
name = "regseq-diagnostics"
version = "0.1.0"
edition = "2021"

[dependencies]
thiserror = "2"
```

lib.rs:
- Add a crate-level doc comment summarizing the diagnostics role from the README.
- Record that the crate owns rustc-independent diagnostic data structures and
  reporting support, not rustc callbacks.
- Do not define the real diagnostics engine in this task.

tests/smoke.rs:
- Smoke test that imports the crate successfully.

## Test Plan

Create:
- crates/regseq-diagnostics/tests/smoke.rs

Tests to implement:
1. Smoke test: verify the crate compiles and can be imported.

Test command: `cargo test -p regseq-diagnostics`

## Acceptance Criteria

- `regseq-diagnostics` is a live workspace member
- The crate remains rustc-independent
- `cargo test -p regseq-diagnostics` passes
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
