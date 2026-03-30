# Task: regseq-analysis-01 - Crate initialization

## Description

Activate the `regseq-analysis` crate as a live workspace member. This crate
provides the infrastructure for static analysis passes over rustc-independent
models. It depends on `regseq-diagnostics` for diagnostic emission and on
`regseq-config` for shared configuration types.

At activation time, the crate needs a minimal Cargo.toml, a lib.rs with
crate-level documentation, and a smoke test. No analysis passes are
implemented in this task.

## Input Context

Read before starting:
- crates/regseq-analysis/README.md
- Cargo.toml (workspace root)
- crates/regseq-diagnostics/Cargo.toml (must already exist from regseq-diagnostics-01)
- crates/regseq-model/Cargo.toml (must already exist from regseq-model-01)

## Output Specification

Create:
- crates/regseq-analysis/Cargo.toml
- crates/regseq-analysis/src/lib.rs
- crates/regseq-analysis/tests/smoke.rs

Modify:
- Cargo.toml (add `"crates/regseq-analysis"` to workspace members)

## Implementation Notes

### Cargo.toml

```toml
[package]
name = "regseq-analysis"
version = "0.1.0"
edition = "2021"

[dependencies]
regseq-config = { path = "../regseq-config" }
regseq-diagnostics = { path = "../regseq-diagnostics" }
regseq-model = { path = "../regseq-model" }
```

### src/lib.rs

Add a crate-level doc comment describing the crate as the static analysis
pass infrastructure for the Regseq compiler. It provides an analysis pass
trait, a pass registry, and validation passes over rustc-independent models
from `regseq-model`.

No analysis passes or pass infrastructure is added at activation time.
Subsequent tasks add the actual pass trait, registry, and analysis
implementations.

## Test Plan

Create:
- crates/regseq-analysis/tests/smoke.rs

Tests to implement:
1. Smoke test: verify the crate compiles and can be imported.

Test command: `cargo test -p regseq-analysis`

## Acceptance Criteria

- Crate compiles
- Smoke test passes
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
