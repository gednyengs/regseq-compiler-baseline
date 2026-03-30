# Task: regseq-backends-01 - Crate initialization

## Description

Activate the `regseq-backends` crate as a live workspace member. This crate
provides the backend registry and C code generation for Linux and Zephyr
targets.

The implementation plan fixes this decision: Linux and Zephyr backends live
as modules inside `regseq-backends` initially. They remain separate harness
task groups even though they share one crate. A split into separate crates
is only warranted if a later phase shows the single-crate layout blocks task
isolation or dependency hygiene.

At activation time, the crate encodes the unified-crate/separate-module
topology decision concretely by creating `src/linux.rs` and `src/zephyr.rs`
stub modules alongside the main `src/lib.rs`.

## Input Context

Read before starting:
- crates/regseq-backends/README.md
- Cargo.toml (workspace root)
- crates/regseq-diagnostics/Cargo.toml (must already exist from regseq-diagnostics-01)
- crates/regseq-model/Cargo.toml (must already exist from regseq-model-01)
- docs/plans/asplos-implementation-plan.md (section 3, backend topology)

## Output Specification

Create:
- crates/regseq-backends/Cargo.toml
- crates/regseq-backends/src/lib.rs
- crates/regseq-backends/src/linux.rs
- crates/regseq-backends/src/zephyr.rs
- crates/regseq-backends/tests/smoke.rs

Modify:
- Cargo.toml (add `"crates/regseq-backends"` to workspace members)

## Implementation Notes

### Cargo.toml

```toml
[package]
name = "regseq-backends"
version = "0.1.0"
edition = "2021"

[dependencies]
regseq-config = { path = "../regseq-config" }
regseq-diagnostics = { path = "../regseq-diagnostics" }
regseq-model = { path = "../regseq-model" }
```

### src/lib.rs

Add a crate-level doc comment describing the crate as the C code generation
backend for the Regseq compiler. It provides a backend trait, a backend
registry, and per-OS modules for Linux and Zephyr target emission. Linux
and Zephyr are separate modules within this crate.

Declare the two backend modules:

```rust
pub mod linux;
pub mod zephyr;
```

No backend trait, registry, or emission logic is added at activation time.
Subsequent tasks add the actual backend infrastructure.

### src/linux.rs

Add a module-level doc comment describing this module as the Linux kernel
C code generation backend. The module is empty at activation time.

### src/zephyr.rs

Add a module-level doc comment describing this module as the Zephyr
C code generation backend. The module is empty at activation time.

## Test Plan

Create:
- crates/regseq-backends/tests/smoke.rs

Tests to implement:
1. Smoke test: verify the crate compiles and can be imported.

Test command: `cargo test -p regseq-backends`

## Acceptance Criteria

- Crate compiles
- `src/linux.rs` and `src/zephyr.rs` exist as declared modules
- Smoke test passes
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
