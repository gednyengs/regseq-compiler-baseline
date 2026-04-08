# Task: extraction-remediation-08 - PL011 helper-methods B extraction remediation

## Description

Implement the remediation defined in:

- `docs/validation/01-pl011/checks/remediation/08-helper-methods-extraction-b.md`

This task improves the compiler-side extraction boundary for the second helper
method batch so that the remaining failing helper bodies preserve exact
authored operand flow, path-sensitive control flow, and branch-local effects
rather than only coarse operation presence.

This is an implementation task, not a proof-only task.

## Input Context

Read before starting:

- `docs/validation/01-pl011/README.md`
- `docs/validation/01-pl011/01-authored-source.md`
- `docs/validation/01-pl011/checks/08-helper-methods-extraction-b.md`
- `docs/validation/01-pl011/checks/results/08-helper-methods-extraction-b.md`
- `docs/validation/01-pl011/checks/remediation/08-helper-methods-extraction-b.md`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `tests/fixtures/pl011-full-spec/src/registers.rs`
- `crates/regseq-driver/src/`
- `crates/regseq-model/src/`
- `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs`

## Output Specification

Primary validation artifacts:

- `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs`
- `docs/validation/01-pl011/checks/results/08-helper-methods-extraction-b.md`

Code changes may modify:

- `crates/regseq-driver/src/`
- `crates/regseq-model/src/` only if stronger helper-body facts require
  extending shared MIR/body structures without weakening existing consumers

The task should update the validation report to reflect the new extraction
behavior after the implementation lands and the test is rerun.

If the implementation is incomplete or still leaves failing checks:

- keep the report honest about what still fails
- record a short descriptive `fail_reason` in the harness
- summarize the remaining gap rather than weakening the check contract

## Constraints

- Do not weaken the extraction check contract.
- Do not change authored-source facts to make the task easier.
- Do not edit unrelated `source-to-extraction-*` test files or reports.
- Keep changes local to the helper-body extraction boundary and its supporting
  data structures.
- Only upgrade a failed check to `pass` if the new exercised evidence actually
  proves it at the required specificity.
- Do not replace exact helper-body flow extraction with source-only inference
  in the test or report.
- Preserve currently passing helper facts while adding the missing path and
  branch-local facts.
- Do not broaden into unrelated callback or backend work.
- Do not add PL011-specific branches, helper-name pattern matches, or
  fixture-path special cases in the compiler.
- Any extraction improvement must generalize to structurally similar authored
  code outside the PL011 fixture.
- Use the PL011 helpers as the validation target, not as the implementation
  dispatch key.

## Implementation Notes

This task should:

- strengthen extraction so the task-owned validation can prove the exact
  authored path predicates, not merely the presence of calls or literals
- prefer improving MIR/body extraction and branch-local dataflow visibility at
  the compiler boundary rather than compensating with weaker test heuristics
- keep the currently passing facts intact for the rest of the helper-methods B
  batch, including:
  - identity/signature/return-type facts
  - the already-passing helper body checks for:
    - `enable_clock_if_present`
    - `disable_clock_if_present`
    - `toggle_reset_if_present`
    - `apply_default_pins_if_present`
    - `apply_sleep_pins_if_present`
- update the task-owned validation test only as needed to reflect the stronger
  extraction surface
- update the validation report after rerunning the task-specific test

This task should preserve enough general extraction detail to support:

- higher-order call/combinator flow where a resource-derived receiver is
  transformed and then consumed by a fallback-producing operation
- exact helper-call sequencing together with assignment placement in the same
  control-flow region
- loop-condition, loop-body, and returned-local correspondence
- path-sensitive option/discriminant splitting with branch-local side effects
- conditional-write dataflow where a computed masked value both gates and
  supplies a write operation

This task should not:

- rewrite the validation check file
- accept proxy call-presence facts where the contract requires exact
  path-sensitive proof
- rely on alphabetical helper-call sets when the contract requires execution
  order
- make unrelated refactors in `regseq-driver` or `regseq-model`

## Test Plan

Run at minimum:

- `CARGO_TARGET_DIR=target/extraction-remediation-08 cargo test -p regseq-driver --test source_to_extraction_08_helper_methods_b -- --nocapture`

Use additional targeted tests only if needed to support the implementation.

The task-owned validation must still prove the PL011-specific authored facts
for the five currently failing helpers:

- `uart_clock_hz`
  - exact `self.clock.as_ref().map(Clock::get_rate).unwrap_or(Self::DEFAULT_UART_CLOCK_HZ)`
    style flow
  - the clock-rate read remains attached to the specific `self.clock` field
  - the fallback path uses `DEFAULT_UART_CLOCK_HZ`
- `shutdown_port`
  - exact helper order:
    - `mask_all_interrupts`
    - `clear_all_interrupts`
    - `disable_fifo`
    - `disable_uart`
  - `self.pending_tx = None` occurs after that authored helper sequence
- `service_rx_fifo`
  - loop condition corresponds to `self.rx_char().is_some()`
  - `handled` initializes to `false`
  - `handled = true` occurs on successful loop iterations
  - the returned bool is exactly the `handled` local
- `service_tx_fifo`
  - exact Some/None split on `pending_tx`
  - exact ready/not-ready split under the `Some` arm
  - DR write, `pending_tx = None`, and `disable_tx_interrupts` occur on the
    authored path only
  - the returned bool matches the authored branch behavior on each path
- `clear_explicit_interrupts`
  - `clear_mask = status & Self::EXPLICIT_ISR_CLEAR_MASK`
  - write path occurs only when `clear_mask != 0`
  - `clear_mask` is used as both raw write mask and raw write value

## Acceptance Criteria

- the extraction surface preserves enough exact operand flow, branch-local
  control-flow detail, and assignment placement for the task-owned validation
  to prove:
  - `uart_clock_hz`
  - `shutdown_port`
  - `service_rx_fifo`
  - `service_tx_fifo`
  - `clear_explicit_interrupts`
- currently passing helper-methods-B facts remain passing
- `source_to_extraction_08_helper_methods_b` passes
- `docs/validation/01-pl011/checks/results/08-helper-methods-extraction-b.md`
  is updated to reflect the new outcome accurately
