# Task: regseq-driver-18 - PM gate capture and structural validation

## Description

Implement the narrow compiler-facing extraction scaffolding for the accepted
 PM gate metadata. This task should prove `regseq-driver` can recover
 `#[rseq::pm(...)]` from the driver type, preserve the gate properties, and
 enforce the immediate structural relationship to the previously established
 `PowerManagement` trait surface.

## Input Context

Read before starting:

- crates/regseq-driver/src/extract/mod.rs
- crates/regseq-driver/src/extract/traits.rs
- crates/regseq-driver/src/callbacks.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/device-power-state-management.md

## Output Specification

Create:

- crates/regseq-driver/src/extract/pm.rs
- crates/regseq-driver/tests/pm.rs

Modify:

- crates/regseq-driver/src/extract/mod.rs
- crates/regseq-driver/src/extract/traits.rs only if needed to query
  `PowerManagement` impl presence cleanly

## Implementation Notes

Recognize and extract only the accepted PM gate form:

- `#[rseq::pm(idle = ..., wakeup_capable = ...)]` on the driver type

The extraction output should remain internal to `regseq-driver`. Narrow records
 such as `PmGateSurface` and `PmGateProperties` are appropriate.

Required capture details:

- driver type name
- `idle` boolean
- `wakeup_capable` boolean
- whether an accepted `PowerManagement` impl is present for the same driver
  type

Required structural validations for this task:

- reject malformed `#[rseq::pm(...)]` attributes deterministically
- reject duplicate keys within one PM attribute
- reject duplicate PM gate attributes on the same driver type
- if the PM gate is present, require an accepted `impl PowerManagement for
  DriverType`
- if the PM gate is absent, do not require the trait

Important boundary:

- this task owns only the declarative PM gate and the immediate structural
  link to the existing trait surface
- it must not implement PM ordering
- it must not implement PM registration glue
- it must not lower any PM behavior
- it must not perform cross-construct PM sequencing checks

Important optionality rule:

- a `PowerManagement` impl may exist without the PM gate and should not be
  rejected by this task
- the PM gate is the condition that makes the trait required, not the reverse

## Test Plan

Create:

- crates/regseq-driver/tests/pm.rs

Tests to implement:

1. Verify `#[rseq::pm(idle = ..., wakeup_capable = ...)]` is recognized and
   preserves both booleans.
2. Verify malformed PM gate attributes are rejected deterministically.
3. Verify duplicate keys and duplicate PM gate attributes are rejected
   deterministically.
4. Verify a PM gate without an accepted `PowerManagement` impl is rejected.
5. Verify a valid PM gate paired with a valid `PowerManagement` impl is
   accepted.
6. Verify a `PowerManagement` impl without a PM gate remains allowed.

Test command: `cargo test -p regseq-driver --test pm`

## Acceptance Criteria

- `regseq-driver` can recover the accepted PM gate declaration form
- the immediate gate-to-trait structural rule is enforced
- the task stays limited to declarative metadata capture and structural
  validation
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
