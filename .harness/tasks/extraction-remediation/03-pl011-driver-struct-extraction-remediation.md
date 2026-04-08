# Task: extraction-remediation-03 - PL011 driver-struct extraction remediation

## Description

Implement the remediation defined in:

- `docs/validation/01-pl011/checks/remediation/03-driver-struct-extraction.md`

This task improves the compiler-side extraction boundary for the PL011 driver
struct so that exact Rust type fidelity for driver resources is preserved by
extraction rather than approximated by shape, requiredness, or classification.

This is an implementation task, not a proof-only task.

## Input Context

Read before starting:

- `docs/validation/01-pl011/README.md`
- `docs/validation/01-pl011/01-authored-source.md`
- `docs/validation/01-pl011/checks/03-driver-struct-extraction.md`
- `docs/validation/01-pl011/checks/results/03-driver-struct-extraction.md`
- `docs/validation/01-pl011/checks/remediation/03-driver-struct-extraction.md`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `tests/fixtures/pl011-full-spec/src/registers.rs`
- `crates/regseq-driver/src/`
- `crates/regseq-driver/tests/source_to_extraction_03_driver_struct.rs`

## Output Specification

Primary validation artifacts:

- `crates/regseq-driver/tests/source_to_extraction_03_driver_struct.rs`
- `docs/validation/01-pl011/checks/results/03-driver-struct-extraction.md`

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
- Do not replace exact Rust type extraction with proxy reasoning from
  requiredness, shape, ids, or classification.
- Preserve currently passing facts while adding the missing ones.

## Implementation Notes

This task should:

- add exact type fidelity for the four currently failing driver resources:
  - `irq: Interrupt`
  - `clock: Option<Clock>`
  - `reset: Option<ResetLine>`
  - `pinctrl: Option<PinCtrl>`
- preserve the currently passing facts for those resources:
  - field name
  - id
  - requiredness
  - classification
  - canonical path reconstruction
- prefer reusing any existing type-rendering helper already used by property
  extraction or associated constant extraction
- update the task-owned validation test only as needed to reflect the stronger
  extraction surface
- update the validation report after rerunning the task-specific test

This task should not:

- rewrite the validation check file
- broaden into unrelated extraction work
- treat proxy facts as proof of exact authored metadata
- make unrelated refactors in `regseq-driver`

## Test Plan

Run at minimum:

- `CARGO_TARGET_DIR=target/extraction-remediation-03 cargo test -p regseq-driver --test source_to_extraction_03_driver_struct -- --nocapture`

Use additional targeted tests only if needed to support the implementation.

## Acceptance Criteria

- the extraction surface preserves exact resource Rust types for:
  - `irq`
  - `clock`
  - `reset`
  - `pinctrl`
- `source_to_extraction_03_driver_struct` passes
- `docs/validation/01-pl011/checks/results/03-driver-struct-extraction.md`
  is updated to reflect the new outcome accurately
