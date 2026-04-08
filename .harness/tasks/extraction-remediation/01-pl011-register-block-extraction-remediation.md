# Task: extraction-remediation-01 - PL011 register-block extraction remediation

## Description

Implement the remediation defined in:

- `docs/validation/01-pl011/checks/remediation/01-register-block-extraction.md`

This task improves the compiler-side extraction boundary for the PL011
register block so that the authored facts required by the validation contract
are preserved by extraction rather than merely detected as missing.

This is an implementation task, not a proof-only task.

## Input Context

Read before starting:

- `docs/validation/01-pl011/README.md`
- `docs/validation/01-pl011/01-authored-source.md`
- `docs/validation/01-pl011/checks/01-register-block-extraction.md`
- `docs/validation/01-pl011/checks/results/01-register-block-extraction.md`
- `docs/validation/01-pl011/checks/remediation/01-register-block-extraction.md`
- `tests/fixtures/pl011-full-spec/src/registers.rs`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `crates/regseq-driver/src/`
- `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs`

## Output Specification

Primary validation artifacts:

- `crates/regseq-driver/tests/source_to_extraction_01_register_block.rs`
- `docs/validation/01-pl011/checks/results/01-register-block-extraction.md`

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
- Do not replace exact property extraction with ad hoc inference in the test.
- Preserve currently passing facts while adding the missing ones.

## Implementation Notes

This task should:

- implement extraction support for the full authored register-block property
  set, not just the currently failing PL011 properties
- preserve reserved-region extent/layout facts strongly enough for the
  validation check to pass
- preserve per-register metadata at the register-block boundary:
  - field name
  - register type name
  - byte offset
  - access kind
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

- `CARGO_TARGET_DIR=target/extraction-remediation-01 cargo test -p regseq-driver --test source_to_extraction_01_register_block -- --nocapture`

Use additional targeted tests only if needed to support the implementation.

## Acceptance Criteria

- the extraction surface preserves the full authored register-block property
  set as explicit extracted facts
- reserved-region extents are reconstructable from extracted metadata
- per-register field name, type name, byte offset, and access kind are
  preserved explicitly at the block boundary
- `source_to_extraction_01_register_block` passes
- `docs/validation/01-pl011/checks/results/01-register-block-extraction.md`
  is updated to reflect the new outcome accurately
