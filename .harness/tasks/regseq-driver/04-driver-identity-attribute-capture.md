# Task: regseq-driver-04 - Driver identity attribute capture

## Description

Implement the first real frontend-capture task in `regseq-driver`: recognize and
extract the structural driver item attributes that establish driver identity for
the accepted PL011 baseline:

- `#[rseq::driver(name = "...", compatible = [...])]`
- `#[rseq::device_class(...)]`

This task must prove the compiler can reach these inert tool attributes in HIR
and extract their values into a narrow internal representation suitable for
later analysis/model work. The output boundary is attribute capture and
validation only, not raw-artifact serialization or normalized models.

## Input Context

Read before starting:

- crates/regseq-driver/src/callbacks.rs
- crates/regseq-driver/src/lib.rs
- crates/regseq-driver/tests/config.rs
- docs/compiler-collateral/constructs/driver-lifecycle.md
- docs/compiler-collateral/compiler-baseline-overview.md

## Output Specification

Create:

- crates/regseq-driver/src/extract/mod.rs
- crates/regseq-driver/src/extract/driver_identity.rs
- crates/regseq-driver/tests/driver_identity.rs

Modify:

- crates/regseq-driver/src/lib.rs
- crates/regseq-driver/src/callbacks.rs

## Implementation Notes

### Capture scope

Recognize and extract only the structural driver item attributes:

- `driver_name` from `#[rseq::driver(name = "...")]`
- `compatible` from `#[rseq::driver(compatible = [...])]`
- `device_class` from `#[rseq::device_class(...)]`

Keep the extracted representation internal to `regseq-driver`. A compact record
such as `DriverIdentity` or `DriverIdentityAttrs` is appropriate here, but it is
not yet the final rustc-independent model.

### Required validations

Enforce the current accepted baseline:

- every driver in the current frontend slice must declare `driver_name`
- every driver in the current frontend slice must declare `compatible`
- every driver in the current frontend slice must declare `device_class`
- duplicate or malformed identity attributes should produce a deterministic
  extraction error

### Pipeline boundary

This task may invoke the extraction helper from `after_analysis()` or a small
test-only harness, but it must not:

- serialize the result as a raw artifact
- introduce field-level attribute capture
- capture trait implementations
- capture handler-binding forms
- introduce normalized model code

## Test Plan

Create:

- crates/regseq-driver/tests/driver_identity.rs

Tests to implement:

1. Compile a tiny fixture crate with `#[rseq::driver(...)]` and
   `#[rseq::device_class(...)]`, then assert the extraction helper returns the
   expected `driver_name`, `compatible`, and `device_class`.
2. Verify a missing `driver_name` is rejected.
3. Verify a missing `compatible` is rejected.
4. Verify a missing `device_class` is rejected.
5. Verify malformed or duplicate identity attributes are rejected.

Test command: `cargo test -p regseq-driver --test driver_identity`

## Acceptance Criteria

- `regseq-driver` can reach and extract the accepted structural driver item
  attributes from HIR
- required-presence and malformed-attribute validations are enforced
- the task stays limited to driver identity capture and validation
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes

