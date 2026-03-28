# Regseq Compiler Harness -- Global Configuration

## Project Overview

You are working on the Regseq compiler: a custom Rust compiler toolchain that
reads Regseq driver specifications (Rust files with `#[rseq::*]` annotations)
and generates OS-specific C device drivers for Linux and Zephyr.

Current active workspace members:
- `cargo-regseq` -- cargo subcommand (CLI, toolchain, orchestration)
- `regseq-config` -- shared configuration types (no rustc dependency)
- `regseq-driver` -- rustc wrapper (HIR/MIR extraction, pipeline orchestration)
- `tests/e2e` -- end-to-end test workspace member

Planned implementation crates for the ASPLOS track:
- `regseq` -- public facade for driver authors
- `regseq-macros` -- procedural macros (`register_bitfields!`, etc.)
- `regseq-model` -- rustc-independent declaration and binding models
- `regseq-analysis` -- static analysis passes
- `regseq-backends` -- C code generation backends (Linux, Zephyr)
- `regseq-diagnostics` -- shared diagnostics engine

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
- Verification on `scripts/python/task.py done` runs three checks in order:
  1. `cargo test --workspace` -- all tests must pass
  2. `cargo fmt --all -- --check` -- code must be formatted
  3. `cargo clippy --workspace -- -D warnings` -- no clippy warnings
- Run `cargo fmt` before completing a task to avoid fmt check failures.

## Workflow

Use `uv run python scripts/python/task.py next-step` to determine the next action. See `.harness/SESSION.md` for the worker bootstrap and `.harness/task-spec-workflow.md` for the fallback task-spec authoring workflow used when no executable harness tasks exist.
