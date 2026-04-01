# Task: regseq-facade-11 - PowerManagement trait surface

## Description

Implement the accepted optional `PowerManagement` callback-family surface in
the public facade. This task owns only the trait declaration and export
plumbing.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- crates/regseq/src/error.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/device-power-state-management.md
- docs/discussions/regseq-error/regseq-error-spec.md

## Output Specification

Create:

- crates/regseq/src/power.rs
- crates/regseq/tests/power.rs

Modify:

- crates/regseq/src/lib.rs

## Implementation Notes

Add the accepted optional power-management callback contract:

```rust
pub trait PowerManagement {
    fn suspend(&mut self) -> Result<(), RegseqError>;
    fn resume(&mut self) -> Result<(), RegseqError>;
}
```

Phase-B baseline:

- keep the accepted trait family name `PowerManagement`
- keep both methods on `&mut self`
- use `Result<(), RegseqError>` as the stable return shape
- keep the trait optional at the facade level; this task does not own the PM
  gate or extraction-time optionality enforcement

Do not include:

- `#[rseq::pm(...)]` attribute handling
- generated PM ordering
- backend PM registration behavior
- extraction or validation logic

## Test Plan

Create:

- crates/regseq/tests/power.rs

Tests to implement:

1. Verify `PowerManagement` is publicly exported from `regseq`.
2. Verify a small type can implement `PowerManagement` with the accepted
   method signatures.
3. Verify the trait remains optional from the facade perspective.

Test command: `cargo test -p regseq --test power`

## Acceptance Criteria

- `PowerManagement` exists as a public facade trait
- the accepted method names and signatures are preserved
- the task stays limited to facade trait shape
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
