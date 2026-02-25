# Task: regseq-config-01 - Crate initialization

## Description

Initialize the `regseq-config` crate. This crate holds shared configuration
types used by cargo-regseq, regseq-driver, analysis passes, and backends.
It has no rustc dependency and is unit-testable in isolation.

## Input Context

Read before starting:
- crates/regseq-config/README.md
- Cargo.toml (workspace root)

## Output Specification

Create:
- crates/regseq-config/Cargo.toml
- crates/regseq-config/src/lib.rs

Modify:
- Cargo.toml (add `"crates/regseq-config"` to workspace members)

## Implementation Notes

Cargo.toml:
```toml
[package]
name = "regseq-config"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = "2"
```

lib.rs:
- Add a crate-level doc comment describing the crate's purpose.
- Define a placeholder error type using thiserror.
- No other content needed -- subsequent tasks add the actual types.

## Test Plan

Create:
- crates/regseq-config/tests/smoke.rs

Tests to implement:
1. Smoke test: verify the crate compiles and can be imported.

Test command: `cargo test -p regseq-config`

## Acceptance Criteria

- Crate compiles
- Smoke test passes
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
