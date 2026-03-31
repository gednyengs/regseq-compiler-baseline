# Task: regseq-driver-07 - Interrupt resource and ISR capture

## Description

Implement the narrow compiler-facing extraction scaffolding for the accepted
interrupt resource surface and `#[rseq::isr]` bound-handler forms. This task
should prove `regseq-driver` can recover the shared interrupt frontend forms
without introducing lowering behavior or wider callback-family work.

## Input Context

Read before starting:

- crates/regseq-driver/src/callbacks.rs
- crates/regseq-driver/src/extract/callback_context.rs
- crates/regseq-driver/src/extract/mod.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/device-interrupt-setup-and-handling.md

## Output Specification

Create:

- crates/regseq-driver/src/extract/interrupts.rs
- crates/regseq-driver/tests/interrupts.rs

Modify:

- crates/regseq-driver/src/extract/mod.rs
- crates/regseq-driver/src/callbacks.rs

## Implementation Notes

Recognize and extract only the shared interrupt frontend forms:

- `#[rseq::interrupt(id = \"...\")]` on interrupt resource fields
- `Interrupt`, `Option<Interrupt>`, and `[Interrupt; N]` typed field shapes
- Layer 2 interrupt binding properties:
  - `trigger_type`
  - `trigger_polarity`
  - `priority`
  - `shared`
- `#[rseq::isr]` bound-handler forms:
  - bare unique-resource auto-bind
  - explicit field bind
  - array bind with `index: usize`

The extraction output should remain internal to `regseq-driver`. A narrow
record such as `InterruptSurface` / `IsrBinding` is appropriate.

Required structural validations for this task:

- bare `#[rseq::isr]` only when exactly one interrupt resource exists
- explicit field binding is required for ambiguous multi-resource cases
- array-bound handlers require `index: usize`
- ISR handler methods must return `IrqResult`
- one handler per non-optional interrupt resource
- bound handlers are classified as callback context `H`

Keep this task bounded:

- preserve `shared` metadata for later checks, but do not attempt control-flow
  proofs that a `NotHandled` path exists
- do not add line enable/disable, registration glue, vector allocation, or
  lowering behavior
- do not widen this task into trait-callback extraction

## Test Plan

Create:

- crates/regseq-driver/tests/interrupts.rs

Tests to implement:

1. Verify interrupt resource fields with `Interrupt`, `Option<Interrupt>`, and
   `[Interrupt; N]` are recognized.
2. Verify all accepted `#[rseq::isr]` binding forms are recognized.
3. Verify structural validation rejects ambiguous bare auto-bind, invalid
   array-handler signatures, and wrong ISR return types deterministically.
4. Verify Layer 2 interrupt binding properties are reachable from extraction.
5. Verify extracted handler context is `H`.

Test command: `cargo test -p regseq-driver --test interrupts`

## Acceptance Criteria

- `regseq-driver` can reach the accepted interrupt resource and bound-handler
  forms
- the task stays limited to capture scaffolding and structural validation
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
