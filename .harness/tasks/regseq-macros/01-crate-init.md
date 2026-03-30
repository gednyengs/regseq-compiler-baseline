# Task: regseq-macros-01 - Crate initialization

## Description

Activate the `regseq-macros` crate as a live workspace member. This is a
proc-macro crate that provides helper macros for the Regseq DSL surface,
including `register_bitfields!`.

At activation time, the crate needs a Cargo.toml with `proc-macro = true`,
a lib.rs with the proc_macro crate attribute, and a smoke test. No actual
macro implementations are added in this task.

## Input Context

Read before starting:
- crates/regseq-macros/README.md
- Cargo.toml (workspace root)
- crates/regseq-driver/src/callbacks.rs

## Output Specification

Create:
- crates/regseq-macros/Cargo.toml
- crates/regseq-macros/src/lib.rs
- crates/regseq-macros/tests/smoke.rs

Modify:
- Cargo.toml (add `"crates/regseq-macros"` to workspace members)

## Implementation Notes

### Cargo.toml

```toml
[package]
name = "regseq-macros"
version = "0.1.0"
edition = "2021"

[lib]
proc-macro = true

[dependencies]
proc-macro2 = "1"
quote = "1"
syn = { version = "2", features = ["full", "extra-traits"] }
```

The `[lib] proc-macro = true` section marks this as a proc-macro crate.
The `syn`, `quote`, and `proc-macro2` dependencies are the standard
proc-macro development stack.

### src/lib.rs

Add a crate-level doc comment describing the crate as the procedural macro
crate for Regseq helper macros. It will provide `register_bitfields!` for
register layouts and any future helper macros needed by driver specs.

Document explicitly that `#[rseq::...]` is not provided by this proc-macro
crate. It is an inert custom tool-attribute namespace registered by the
compiler wrapper so the attributes remain available to HIR extraction.

No macro implementations are added at activation time. Subsequent tasks add
the actual macro definitions.

## Test Plan

Create:
- crates/regseq-macros/tests/smoke.rs

Tests to implement:
1. Smoke test: verify the crate compiles and can be imported. Since this is
   a proc-macro crate, the smoke test verifies that the crate appears in the
   dependency graph by checking that `cargo test -p regseq-macros` succeeds.

Test command: `cargo test -p regseq-macros`

## Acceptance Criteria

- Crate compiles as a proc-macro crate
- Smoke test passes
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
