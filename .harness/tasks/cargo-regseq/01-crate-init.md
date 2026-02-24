# Task: cargo-regseq-01 - Crate initialization

## Description

Initialize the `cargo-regseq` crate as a Rust binary crate with a lib/bin split.
The binary (`main.rs`) is a thin entry point that delegates to `lib.rs`. All
testable logic lives in the library. Add the crate to the workspace `Cargo.toml`.

This crate is a cargo subcommand. When installed, cargo discovers it automatically
as `cargo regseq` because the binary name starts with `cargo-`.

## Input Context

Read before starting:
- crates/cargo-regseq/README.md
- Cargo.toml (workspace root)
- .harness/config.md

## Output Specification

Create:
- crates/cargo-regseq/Cargo.toml
- crates/cargo-regseq/src/main.rs
- crates/cargo-regseq/src/lib.rs

Modify:
- Cargo.toml (add "crates/cargo-regseq" to workspace members)

## Implementation Notes

- `Cargo.toml`: name = "cargo-regseq", edition = "2021". Add dependencies:
  `clap` (with `derive` feature) for argument parsing.
- `main.rs`: call a `run()` function from `lib.rs`, handle errors with
  `process::exit(1)` on failure. Print errors to stderr.
- `lib.rs`: define `pub fn run() -> Result<(), Error>` that currently just
  returns `Ok(())`. Define a placeholder `Error` type (can use `anyhow::Error`
  or a simple custom enum). Add a brief module doc comment.

## Test Plan

Create:
- crates/cargo-regseq/tests/smoke.rs

Tests to implement:
1. Import `cargo_regseq::run` and call it; verify it returns Ok(())

Test command: `cargo test -p cargo-regseq`

## Acceptance Criteria

- `cargo build -p cargo-regseq` succeeds
- `cargo test -p cargo-regseq` passes
- `cargo test --workspace` passes
- `cargo clippy --workspace` clean
