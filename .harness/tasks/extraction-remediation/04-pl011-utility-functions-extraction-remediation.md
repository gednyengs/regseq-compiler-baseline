# Task: extraction-remediation-04 - PL011 utility-functions extraction remediation

## Description

Implement the remediation defined in:

- `docs/validation/01-pl011/checks/remediation/04-utility-functions-extraction.md`

This task improves the compiler-side extraction boundary for the PL011 free
utility helpers so that exact signature facts and executable body facts are
preserved by extraction rather than reconstructed from source-only reasoning or
left absent from the extraction surface.

This is an implementation task, not a proof-only task.

## Input Context

Read before starting:

- `docs/validation/01-pl011/README.md`
- `docs/validation/01-pl011/01-authored-source.md`
- `docs/validation/01-pl011/checks/04-utility-functions-extraction.md`
- `docs/validation/01-pl011/checks/results/04-utility-functions-extraction.md`
- `docs/validation/01-pl011/checks/remediation/04-utility-functions-extraction.md`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `crates/regseq-driver/src/`
- `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs`

## Output Specification

Primary validation artifacts:

- `crates/regseq-driver/tests/source_to_extraction_04_utility_functions.rs`
- `docs/validation/01-pl011/checks/results/04-utility-functions-extraction.md`

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
- Keep changes local to the extraction boundary and its supporting data
  structures.
- Only upgrade a failed check to `pass` if the new exercised evidence actually
  proves it at the required specificity.
- Do not replace free-function extraction with source-only inference in the
  test or report.
- Preserve currently passing identity and canonical-path facts while adding the
  missing signature/body richness.

## Implementation Notes

This task should:

- preserve exact free-function signature facts for:
  - `field_val`
  - `raw_reg_val`
  - `field_from_raw`
- preserve enough free-function body structure to prove, from extraction
  output, the authored body-level facts for those three helpers:
  - callee identity
  - argument flow
  - arithmetic / masking / shifting structure
- ensure free functions are extracted through a surface strong enough to
  validate bodies, not only inherent methods and callbacks
- update the task-owned validation test only as needed to reflect the stronger
  extraction surface
- update the validation report after rerunning the task-specific test

This task should not:

- rewrite the validation check file
- broaden into unrelated extraction work
- accept proxy body facts where the contract requires exact structure
- make unrelated refactors in `regseq-driver`

## Test Plan

Run at minimum:

- `CARGO_TARGET_DIR=target/extraction-remediation-04 cargo test -p regseq-driver --test source_to_extraction_04_utility_functions -- --nocapture`

Use additional targeted tests only if needed to support the implementation.

## Acceptance Criteria

- the extraction surface preserves exact parameter lists and return types for:
  - `field_val`
  - `raw_reg_val`
  - `field_from_raw`
- the extraction surface preserves enough free-function body structure for the
  task-owned validation to prove the authored body predicates for all three
  helpers without source-only reasoning
- `source_to_extraction_04_utility_functions` passes
- `docs/validation/01-pl011/checks/results/04-utility-functions-extraction.md`
  is updated to reflect the new outcome accurately
