# Task: regseq-macros-01 - Crate initialization

## Description

Activate the `regseq-macros` crate as a real workspace member.

This task establishes the proc-macro crate required by the implementation
plan, but it does not implement any Regseq macro behavior yet. The output is
a compiling proc-macro crate with crate-level documentation and the standard
macro-authoring dependency set.

## Input Context

Read before starting:
- crates/regseq-macros/README.md
- Cargo.toml
- docs/plans/asplos-implementation-plan.md
- docs/guide/implementation-guide.md

## Output Specification

Create:
- crates/regseq-macros/Cargo.toml
- crates/regseq-macros/src/lib.rs

Modify:
- Cargo.toml (add `"crates/regseq-macros"` to workspace members)

## Implementation Notes

Cargo.toml:

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
syn = { version = "2", features = ["full"] }
```

lib.rs:
- Add a crate-level doc comment summarizing the crate role from the plan and README.
- Do not define real attribute macros or `register_bitfields!` in this task.
- Leave a short note that later tasks add the actual proc-macro entry points.
- Keep the crate compiling as an intentionally empty proc-macro crate.

The point of this task is workspace activation and topology, not macro surface
design. Do not invent placeholder Regseq syntax just to make the crate feel
less empty.

## Test Plan

Tests to implement:
1. Unit smoke test in `src/lib.rs`: verify the crate builds and the test
   target runs.

Test command: `cargo test -p regseq-macros`

## Acceptance Criteria

- `regseq-macros` is a live workspace member
- The crate compiles as a proc-macro crate
- `cargo test -p regseq-macros` passes
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
