# Task: regseq-driver-12 - Clock resource and wrapper capture

## Description

Implement the narrow compiler-facing extraction scaffolding for the accepted
clock resource field shapes and wrapper-method calls. This task should prove
`regseq-driver` can recover the immediate frontend shape of the `Clock`
construct without introducing Layer 2 join work or lowering behavior.

## Input Context

Read before starting:

- crates/regseq-driver/src/extract/mod.rs
- crates/regseq-driver/src/extract/register_access.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/peripheral-clock-management.md

## Output Specification

Create:

- crates/regseq-driver/src/extract/clock.rs
- crates/regseq-driver/tests/clock.rs

Modify:

- crates/regseq-driver/src/extract/mod.rs

## Implementation Notes

Recognize and extract only the immediate shared clock frontend forms:

- `#[rseq::clock(id = "...")]` on `Clock` and `Option<Clock>` fields
- field type `Clock` vs `Option<Clock>`
- wrapper-method call sites:
  - `enable()`
  - `disable()`
  - `get_rate()`

The extraction output should remain internal to `regseq-driver`. A narrow
record such as `ClockSurface`, `ClockField`, and `ClockCallSite` is
appropriate.

Required capture details:

- field name
- declared clock id from `#[rseq::clock(id = "...")]`
- whether the field is required (`Clock`) or optional (`Option<Clock>`)
- wrapper method identity and containing function/method name

Required structural validations for this task:

- every recognized clock field must have a `clock_id`
- reject malformed `#[rseq::clock(...)]` attributes deterministically
- only recognize wrapper-method calls whose receiver resolves to a known clock
  field or borrowed clock field

Keep this task bounded:

- do not join against Layer 2 `default_frequency`
- do not implement compiler-managed `acquire_clock`
- do not lower wrapper methods
- do not validate init ordering or PM ordering

## Test Plan

Create:

- crates/regseq-driver/tests/clock.rs

Tests to implement:

1. Verify `Clock` and `Option<Clock>` fields with `#[rseq::clock(id = "...")]`
   are recognized.
2. Verify required vs optional field shape is preserved.
3. Verify wrapper-method call sites for `enable`, `disable`, and `get_rate`
   are recognized on known clock fields.
4. Verify malformed or missing `clock_id` attributes are rejected
   deterministically.
5. Verify unrelated methods with the same names are not misclassified as clock
   wrapper calls.

Test command: `cargo test -p regseq-driver --test clock`

## Acceptance Criteria

- `regseq-driver` can reach the accepted clock resource and wrapper-call forms
- the task stays limited to immediate frontend capture and structural
  validation
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
