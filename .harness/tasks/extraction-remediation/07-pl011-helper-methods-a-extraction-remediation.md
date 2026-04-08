# Task: extraction-remediation-07 - PL011 helper-methods A extraction remediation

## Description

Implement the remediation defined in:

- `docs/validation/01-pl011/checks/remediation/07-helper-methods-extraction-a.md`

This task improves the compiler-side extraction boundary for the first helper
method batch so that the remaining failing helper bodies preserve exact
authored value identity and register-field locality rather than only coarse
operation presence.

This is an implementation task, not a proof-only task.

## Input Context

Read before starting:

- `docs/validation/01-pl011/README.md`
- `docs/validation/01-pl011/01-authored-source.md`
- `docs/validation/01-pl011/checks/07-helper-methods-extraction-a.md`
- `docs/validation/01-pl011/checks/results/07-helper-methods-extraction-a.md`
- `docs/validation/01-pl011/checks/remediation/07-helper-methods-extraction-a.md`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `tests/fixtures/pl011-full-spec/src/registers.rs`
- `crates/regseq-driver/src/`
- `crates/regseq-driver/tests/source_to_extraction_07_helper_methods_a.rs`

## Output Specification

Primary validation artifacts:

- `crates/regseq-driver/tests/source_to_extraction_07_helper_methods_a.rs`
- `docs/validation/01-pl011/checks/results/07-helper-methods-extraction-a.md`

Code changes may modify:

- `crates/regseq-driver/src/`

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
- Do not replace exact helper-body value extraction with source-only inference
  in the test or report.
- Preserve currently passing helper facts while adding the missing value and
  field-locality facts.

## Implementation Notes

This task should:

- preserve exact value identity and field-local update detail for:
  - `disable_fifo`
  - `configure_line_control`
  - `enable_uart`
- preserve enough operation detail to prove:
  - the authored register field being updated
  - the exact symbolic or scalar value being written/modified
  - that the proof is tied to the relevant register-local operation, not only
    a matching helper call elsewhere in the body
- keep the currently passing facts intact for the rest of the helper-methods A
  batch, including:
  - identity/signature/return-type facts
  - the already-passing helper body checks
- prefer strengthening extraction of operation/value facts at the body
  boundary rather than compensating with weaker test heuristics
- update the task-owned validation test only as needed to reflect the stronger
  extraction surface
- update the validation report after rerunning the task-specific test

This task should not:

- rewrite the validation check file
- broaden into unrelated helper-method or callback extraction work
- accept proxy call-presence facts where the contract requires exact value or
  field-local proof
- make unrelated refactors in `regseq-driver`

## Test Plan

Run at minimum:

- `CARGO_TARGET_DIR=target/extraction-remediation-07 cargo test -p regseq-driver --test source_to_extraction_07_helper_methods_a -- --nocapture`

Use additional targeted tests only if needed to support the implementation.

## Acceptance Criteria

- the extraction surface preserves enough exact value and field-local operation
  detail for the task-owned validation to prove:
  - `disable_fifo`
  - `configure_line_control`
  - `enable_uart`
- `source_to_extraction_07_helper_methods_a` passes
- `docs/validation/01-pl011/checks/results/07-helper-methods-extraction-a.md`
  is updated to reflect the new outcome accurately
