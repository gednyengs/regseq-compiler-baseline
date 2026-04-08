# Task: extraction-remediation-06 - PL011 driver-constants extraction remediation

## Description

Implement the remediation defined in:

- `docs/validation/01-pl011/checks/remediation/06-driver-constants-extraction.md`

This task improves the compiler-side extraction boundary for PL011 driver
constants so that exact constant values and authored definition relationships
are preserved by extraction rather than inferred from weaker name/type facts.

This is an implementation task, not a proof-only task.

## Input Context

Read before starting:

- `docs/validation/01-pl011/README.md`
- `docs/validation/01-pl011/01-authored-source.md`
- `docs/validation/01-pl011/checks/06-driver-constants-extraction.md`
- `docs/validation/01-pl011/checks/results/06-driver-constants-extraction.md`
- `docs/validation/01-pl011/checks/remediation/06-driver-constants-extraction.md`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `crates/regseq-driver/src/`
- `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs`

## Output Specification

Primary validation artifacts:

- `crates/regseq-driver/tests/source_to_extraction_06_driver_constants.rs`
- `docs/validation/01-pl011/checks/results/06-driver-constants-extraction.md`

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
- Keep changes local to the constant extraction boundary and its supporting
  data structures.
- Only upgrade a failed check to `pass` if the new exercised evidence actually
  proves it at the required specificity.
- Do not replace exact constant-value extraction with source-only inference in
  the test or report.
- Preserve currently passing constant facts while adding the missing value and
  definition-shape facts.

## Implementation Notes

This task should:

- preserve exact evaluated values for all six in-scope driver constants
- preserve enough definition-expression structure to prove authored dependency
  relationships, especially:
  - `EXPLICIT_ISR_CLEAR_MASK = Self::ERROR_INTERRUPT_MASK`
- keep the currently passing constant facts intact:
  - names
  - types
  - canonical paths
  - owner paths
  - closed-world membership
  - MIR named references / distinct entry survival
- prefer extracting definition/initializer structure from the most local
  compiler-side boundary available rather than reconstructing it from source
  strings in the test
- update the task-owned validation test only as needed to reflect the stronger
  extraction surface
- update the validation report after rerunning the task-specific test

This task should not:

- rewrite the validation check file
- broaden into unrelated extraction work
- accept proxy facts where the contract requires exact literal values or exact
  authored dependency structure
- make unrelated refactors in `regseq-driver`

## Test Plan

Run at minimum:

- `CARGO_TARGET_DIR=target/extraction-remediation-06 cargo test -p regseq-driver --test source_to_extraction_06_driver_constants -- --nocapture`

Use additional targeted tests only if needed to support the implementation.

## Acceptance Criteria

- the extraction surface preserves exact evaluated values for all six driver
  constants in scope
- the extraction surface preserves enough definition-expression structure for
  the task-owned validation to prove the authored dependency relationship for
  `EXPLICIT_ISR_CLEAR_MASK`
- `source_to_extraction_06_driver_constants` passes
- `docs/validation/01-pl011/checks/results/06-driver-constants-extraction.md`
  is updated to reflect the new outcome accurately
