# Task: regseq-facade-09 - Lifecycle trait surface

## Description

Implement the accepted `Lifecycle` callback-family surface in the public
facade. This task owns only the trait declaration, export plumbing, and narrow
type-shape tests.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- crates/regseq/src/error.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/driver-lifecycle.md
- docs/discussions/regseq-error/regseq-error-spec.md

## Output Specification

Create:

- crates/regseq/src/lifecycle.rs
- crates/regseq/tests/lifecycle.rs

Modify:

- crates/regseq/src/lib.rs

## Implementation Notes

Add the accepted shared lifecycle callback contract:

```rust
pub trait Lifecycle {
    fn init(&mut self) -> Result<(), RegseqError>;
    fn deinit(&mut self) -> Result<(), RegseqError>;
}
```

Phase-B baseline:

- keep the accepted trait family name `Lifecycle`
- keep both methods on `&mut self`
- use `Result<(), RegseqError>` as the stable return shape
- keep this task limited to the facade declaration surface and export plumbing

Do not include:

- `#[rseq::driver]` or `#[rseq::device_class]` attribute handling
- `suspend` / `resume`
- backend behavior or registration glue
- extraction or validation logic

## Test Plan

Create:

- crates/regseq/tests/lifecycle.rs

Tests to implement:

1. Verify `Lifecycle` is publicly exported from `regseq`.
2. Verify a small type can implement `Lifecycle` with the accepted method
   signatures.
3. Verify both methods use `Result<(), RegseqError>`.

Test command: `cargo test -p regseq --test lifecycle`

## Acceptance Criteria

- `Lifecycle` exists as a public facade trait
- the accepted method names and signatures are preserved
- the task stays limited to facade trait shape
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
