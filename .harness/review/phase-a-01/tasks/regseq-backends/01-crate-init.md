# Task: regseq-backends-01 - Crate initialization

## Description

Activate the `regseq-backends` crate as the unified backend crate for target
OS code generation.

This task captures the current topology decision from the implementation plan:
Linux and Zephyr remain separate backend workstreams, but they share one crate
initially. The activation work must therefore encode distinct module
boundaries inside a single backend crate.

This task does not implement lowering logic, MIR structuring, or C emission.

## Input Context

Read before starting:
- crates/regseq-backends/README.md
- Cargo.toml
- docs/plans/asplos-implementation-plan.md
- docs/guide/implementation-guide.md

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

Cargo.toml:

```toml
[package]
name = "regseq-backends"
version = "0.1.0"
edition = "2021"

[dependencies]
regseq-config = { path = "../regseq-config" }
regseq-diagnostics = { path = "../regseq-diagnostics" }
regseq-model = { path = "../regseq-model" }
thiserror = "2"
```

lib.rs:
- Add a crate-level doc comment describing `regseq-backends` as the shared
  backend crate.
- Declare `mod linux;` and `mod zephyr;` so the single-crate, two-workstream
  topology is explicit from the start.
- Keep those modules internal for now unless a public item is required.
- Do not define backend traits or codegen APIs yet.

linux.rs and zephyr.rs:
- Add short module docs indicating these files are the future homes of Linux
  and Zephyr lowering logic.
- Keep both modules as compiling stubs only.

tests/smoke.rs:
- Smoke test that imports the crate successfully.

## Test Plan

Create:
- crates/regseq-backends/tests/smoke.rs

Tests to implement:
1. Smoke test: verify the crate compiles and can be imported.

Test command: `cargo test -p regseq-backends`

## Acceptance Criteria

- `regseq-backends` is a live workspace member
- The crate depends on `regseq-model`, `regseq-diagnostics`, and `regseq-config`
- Separate `linux` and `zephyr` module stubs exist inside the unified backend crate
- `cargo test -p regseq-backends` passes
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
