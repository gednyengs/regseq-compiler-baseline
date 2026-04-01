# Task: regseq-driver-14 - PinCtrl resource and wrapper capture

## Description

Implement the narrow compiler-facing extraction scaffolding for the accepted
`PinCtrl` resource field shapes and `apply::<State>()` wrapper calls. This task
should prove `regseq-driver` can recover the immediate frontend shape of the
pinctrl construct without introducing Layer 2 join work or lowering behavior.

## Input Context

Read before starting:

- crates/regseq-driver/src/extract/mod.rs
- crates/regseq-driver/src/extract/register_access.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/pin-controller-and-gpio-configuration.md

## Output Specification

Create:

- crates/regseq-driver/src/extract/pinctrl.rs
- crates/regseq-driver/tests/pinctrl.rs

Modify:

- crates/regseq-driver/src/extract/mod.rs

## Implementation Notes

Recognize and extract only the immediate shared pinctrl frontend forms:

- `PinCtrl` and `Option<PinCtrl>` typed fields
- optional `#[rseq::pinctrl(id = "...")]` override on pinctrl fields
- wrapper-method call sites:
  - `apply::<State>()`

The extraction output should remain internal to `regseq-driver`. A narrow
record such as `PinCtrlSurface`, `PinCtrlField`, and `PinCtrlCallSite` is
appropriate.

Required capture details:

- field name
- whether the field is required (`PinCtrl`) or optional (`Option<PinCtrl>`)
- optional explicit `pinctrl_id` override when present
- state marker type name used at `apply::<State>()` call sites
- containing function/method name for each recognized call site

Required structural validations for this task:

- reject malformed `#[rseq::pinctrl(...)]` attributes deterministically
- `apply` must be called with a concrete generic state argument
- extraction must recover that concrete turbofish state argument from
  `apply::<State>()` rather than only matching the method name
- only recognize wrapper-method calls whose receiver resolves to a known
  pinctrl field or borrowed pinctrl field

Keep this task bounded:

- do not validate state markers against Layer 2 `pin_states`
- do not implement compiler-managed `acquire_pinctrl`
- do not implement PM-generated state transitions
- do not lower pinctrl state selection

## Test Plan

Create:

- crates/regseq-driver/tests/pinctrl.rs

Tests to implement:

1. Verify `PinCtrl` and `Option<PinCtrl>` fields are recognized.
2. Verify optional `#[rseq::pinctrl(id = "...")]` overrides are captured.
3. Verify `apply::<State>()` call sites are recognized and preserve the state
   marker type name.
4. Verify malformed pinctrl attributes are rejected deterministically.
5. Verify non-generic or unrelated `apply` methods are not misclassified as
   pinctrl wrapper calls.

Test command: `cargo test -p regseq-driver --test pinctrl`

## Acceptance Criteria

- `regseq-driver` can reach the accepted pinctrl resource and wrapper-call
  forms
- the task stays limited to immediate frontend capture and structural
  validation
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
