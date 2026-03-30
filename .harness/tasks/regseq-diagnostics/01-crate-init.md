# Task: regseq-diagnostics-01 - Crate initialization

## Description

Activate the `regseq-diagnostics` crate as a live workspace member. This crate
provides the shared diagnostics engine used by analysis passes, backends, and
other compiler components.

The diagnostics crate has no rustc dependency and uses its own span
representation. At activation time, the crate needs a minimal Cargo.toml,
a lib.rs with crate-level documentation, and a smoke test.

## Input Context

Read before starting:
- crates/regseq-diagnostics/README.md
- Cargo.toml (workspace root)

## Output Specification

Create:
- crates/regseq-diagnostics/Cargo.toml
- crates/regseq-diagnostics/src/lib.rs
- crates/regseq-diagnostics/tests/smoke.rs

Modify:
- Cargo.toml (add `"crates/regseq-diagnostics"` to workspace members)

## Implementation Notes

### Cargo.toml

```toml
[package]
name = "regseq-diagnostics"
version = "0.1.0"
edition = "2021"

[dependencies]
thiserror = "2"
```

### src/lib.rs

Add a crate-level doc comment describing the crate as the shared diagnostics
engine for the Regseq compiler. It provides diagnostic severity levels, a
diagnostic ID system, structured messages with source spans, and emission
support.

No other content needed at activation time. Subsequent tasks add the actual
diagnostic types and infrastructure.

## Test Plan

Create:
- crates/regseq-diagnostics/tests/smoke.rs

Tests to implement:
1. Smoke test: verify the crate compiles and can be imported.

Test command: `cargo test -p regseq-diagnostics`

## Acceptance Criteria

- Crate compiles
- Smoke test passes
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
