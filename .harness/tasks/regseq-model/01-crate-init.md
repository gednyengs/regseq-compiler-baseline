# Task: regseq-model-01 - Crate initialization

## Description

Create and activate the `regseq-model` crate as a live workspace member. This
is a new crate that does not yet have a directory.

`regseq-model` is the rustc-independent crate that owns declaration models,
platform binding models, joined lowering inputs, serialization, and debug
artifacts. It has no rustc dependency and is the central data-structure crate
for the compiler pipeline.

The implementation plan fixes this topology decision: `regseq-model` owns
rustc-independent models, while `regseq-driver` owns extraction. This task
establishes the crate; later tasks add the actual model types.

## Input Context

Read before starting:
- Cargo.toml (workspace root)
- docs/plans/asplos-implementation-plan.md (section 3, topology decisions)
- crates/regseq-config/Cargo.toml (for dependency version consistency)

## Output Specification

Create:
- crates/regseq-model/Cargo.toml
- crates/regseq-model/src/lib.rs
- crates/regseq-model/tests/smoke.rs

Modify:
- Cargo.toml (add `"crates/regseq-model"` to workspace members)

## Implementation Notes

### Cargo.toml

```toml
[package]
name = "regseq-model"
version = "0.1.0"
edition = "2021"

[dependencies]
regseq-config = { path = "../regseq-config" }
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
serde_json = "1"
```

The `regseq-config` dependency provides shared configuration types. The `serde`
dependency supports model serialization, which is a core requirement (the
extracted-model artifact must be serializable and reviewable).

### src/lib.rs

Add a crate-level doc comment describing the crate as the rustc-independent
internal model for the Regseq compiler. It owns declaration models for
validated constructs, platform binding models for Layer 2 integration data,
joined lowering inputs, and serialization support for debug artifacts.

No other content needed at activation time. Subsequent tasks add the actual
model types.

## Test Plan

Create:
- crates/regseq-model/tests/smoke.rs

Tests to implement:
1. Smoke test: verify the crate compiles and can be imported.

Test command: `cargo test -p regseq-model`

## Acceptance Criteria

- Crate directory created at `crates/regseq-model/`
- Crate compiles
- Smoke test passes
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
