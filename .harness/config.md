# Regseq Compiler Harness -- Global Configuration

## Project Overview

You are working on the Regseq compiler: a custom Rust compiler toolchain that
reads Regseq driver specifications (Rust files with `#[rseq::*]` annotations)
and generates OS-specific C device drivers for Linux and Zephyr.

The workspace contains 8 crates. See each crate's `README.md` for its purpose:
- `cargo-regseq` -- cargo subcommand (CLI, toolchain, orchestration)
- `regseq` -- public facade for driver authors (types, traits, macro re-exports)
- `regseq-macros` -- procedural macros (`register_bitfields!`, etc.)
- `regseq-driver` -- rustc wrapper (HIR/MIR extraction, pipeline orchestration)
- `regseq-analysis` -- static analysis passes
- `regseq-backends` -- C code generation backends (Linux, Zephyr)
- `regseq-config` -- shared configuration types (no rustc dependency)
- `regseq-diagnostics` -- shared diagnostics engine (no rustc dependency)

## Coding Conventions

- Keep individual source files under 500 lines. Split into submodules when
  approaching this limit.
- Only modify files relevant to your task.
- Write doc comments on all public types and functions.
- Use `thiserror` for error types. Use `anyhow` only in binary crates.
- No unicode symbols in documentation or comments.
- No author attributions in commit messages.

## Testing Requirements

- Every new public function or type must have at least one test.
- Write tests BEFORE implementation when the task spec provides a test plan.
- Run `cargo test -p <crate>` after changes to verify your work.
- Run `cargo test --workspace` before marking a task done to check for regressions.
- Run `cargo clippy --workspace` before marking a task done.

## Task Workflow

1. Run `python task.py next` to find your assigned task.
2. Run `python task.py start <id>` to claim it.
3. Read the task spec markdown file for full instructions.
4. Read all files listed in the task's Input Context section.
5. Implement according to the Output Specification.
6. Write tests from the Test Plan.
7. Iterate until tests pass.
8. Run `python task.py done <id>` to complete (runs verification automatically).
