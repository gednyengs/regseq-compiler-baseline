# Regseq Compiler Harness -- Global Configuration

## Project Overview

You are working on the Regseq compiler: a custom Rust compiler toolchain that
reads Regseq driver specifications (Rust files with `#[rseq::*]` annotations)
and generates OS-specific C device drivers for Linux and Zephyr.

Current active workspace members:
- `cargo-regseq` -- cargo subcommand (CLI, toolchain, orchestration)
- `regseq` -- public Layer 1 facade for driver authors
- `regseq-analysis` -- static analysis and validation passes over rustc-independent models
- `regseq-backends` -- shared lowering/backend crate with Linux and Zephyr modules
- `regseq-config` -- shared configuration types (no rustc dependency)
- `regseq-diagnostics` -- shared diagnostics engine
- `regseq-driver` -- rustc wrapper (HIR/MIR extraction, pipeline orchestration)
- `regseq-macros` -- helper macro crate, including `register_bitfields!`
- `regseq-model` -- rustc-independent declaration, binding, and joined-artifact models
- `tests/e2e` -- end-to-end test workspace member

Active implementation stream notes:
- inert `#[rseq::...]` tool attributes are registered by `regseq-driver`, not provided by `regseq-macros`
- `regseq-backends` remains a single crate for now, even though Linux and Zephyr are separate harness task groups

## Coding Conventions

- Keep individual source files under 500 lines. Split into submodules when
  approaching this limit.
- Only modify files relevant to your task.
- Write doc comments on all public types and functions.
- Use `thiserror` for error types in library crates. Use `anyhow` only in
  binary crate top-level orchestration (e.g., `main.rs`, `run()`).
- No unicode symbols in documentation or comments.
- No author attributions in commit messages.

## Testing Requirements

- Every new public function or type must have at least one test.
- Define public API signatures (with `todo!()` bodies) first, then write tests
  against them, then implement. This ensures tests compile before implementation.
- Run `cargo test -p <crate>` after changes to verify your work.
- `scripts/python/task.py done` first runs cleanup steps that may modify files:
  1. `cargo fmt --all` -- applies formatting in place
  2. `cargo fix --workspace --allow-dirty --allow-staged` -- applies compiler-suggested fixes
  3. `cargo clippy --fix --workspace --allow-dirty --allow-staged` -- applies machine-applicable clippy fixes
  4. `cargo fmt --all` -- re-formats after automated fixes
- After cleanup, `scripts/python/task.py done` runs three verification checks in order:
  1. `cargo test --workspace` -- all tests must pass
  2. `cargo fmt --all -- --check` -- code must already be formatted
  3. `cargo clippy --workspace -- -D warnings` -- no clippy warnings remain

## Workflow

Use `uv run python scripts/python/task.py next-step` to determine the next action. See `.harness/SESSION.md` for the worker bootstrap and `.harness/task-spec-workflow.md` for the fallback task-spec authoring workflow used when no executable harness tasks exist.
