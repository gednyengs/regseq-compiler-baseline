# Task: regseq-driver-11 - PowerManagement trait capture

## Description

Implement the narrow compiler-facing extraction scaffolding for the optional
`PowerManagement` trait surface. This task should prove `regseq-driver` can
recover PM callback implementations and preserve the accepted optionality
boundary around the PM gate.

## Input Context

Read before starting:

- crates/regseq-driver/src/extract/callback_context.rs
- crates/regseq-driver/src/extract/driver_identity.rs
- crates/regseq-driver/src/extract/mod.rs
- crates/regseq-driver/src/extract/traits.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/device-power-state-management.md

## Output Specification

Modify:

- crates/regseq-driver/src/extract/traits.rs
- crates/regseq-driver/src/extract/mod.rs
- crates/regseq-driver/tests/traits.rs

## Implementation Notes

Extend the shared trait-capture machinery from `regseq-driver-10` to recognize:

- `impl PowerManagement for DriverType { ... }`

Required capture details:

- driver type name implementing the trait
- which accepted methods are present
- callback context classification:
  - `PowerManagement` -> `F`

Required structural validations for this task:

- `PowerManagement` impls must provide both `suspend` and `resume` with the
  accepted signatures
- reject wrong receiver types, wrong method names, and wrong return shapes
  deterministically

Optionality rule for this batch:

- preserve the accepted PM gate boundary by treating `PowerManagement` as
  optional in pure trait-capture mode
- this task may recognize the presence of `#[rseq::pm(...)]` if convenient, but
  it should not yet require `PowerManagement` based on the PM gate or vice
  versa; that cross-surface enforcement belongs to later join work

Keep this task bounded:

- do not implement PM-gate enforcement
- do not implement generated PM ordering
- do not analyze callback bodies
- do not lower PM callbacks

## Test Plan

Modify:

- crates/regseq-driver/tests/traits.rs

Tests to implement:

1. Verify `PowerManagement` impls with the accepted method set are recognized.
2. Verify wrong PM signatures are rejected deterministically.
3. Verify extracted PM callback context is `F`.
4. Verify the trait remains optional from the pure trait-capture perspective.

Test command: `cargo test -p regseq-driver --test traits`

## Acceptance Criteria

- `regseq-driver` can reach the accepted `PowerManagement` trait impl form
- the accepted PM callback context is preserved
- the task stays limited to trait-surface capture and structural validation
- the updated tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
