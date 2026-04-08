# Task: source-to-extraction-06 - PL011 driver constants authored->extraction boundary validation

## Description

Perform the boundary validation defined in:

- `docs/validation/01-pl011/checks/06-driver-constants-extraction.md`

This task validates whether the authored driver-constant facts for PL011
survive at the compiler-side boundary between the authored PL011 source and the
current Regseq extraction state.

This is a validation/proof task, not a feature-implementation task.

## Input Context

Read before starting:

- `docs/validation/01-pl011/01-authored-source.md`
- `docs/validation/01-pl011/checks/06-driver-constants-extraction.md`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `.harness/tasks/regseq-driver/19-raw-frontend-artifact-emission.md`
- `crates/regseq-driver/src/`
- `crates/regseq-driver/tests/`

## Output Specification

Output Path:

- `docs/validation/01-pl011/checks/results/06-driver-constants-extraction.md`

Create or modify:

- `crates/regseq-driver/tests/` only as narrowly needed for a dedicated
  compiler-side boundary validation test

Modify if needed:

- `crates/regseq-driver/src/` only if an existing test helper must be exposed
  for inspection and doing so does not change production compiler behavior

Also create or update:

- `docs/validation/01-pl011/checks/results/06-driver-constants-extraction.md`

The report file should contain the validation certificate for this task. The
harness task status and failure notes should summarize the result and point to
the report file rather than replacing it.

## Implementation Notes

This task should:

- execute the check specification exactly as written
- verify that the authored facts copied into the check file still match the
  actual fixture source before judging extraction outcomes
- if using a reduced or inline specimen, add an explicit precondition test that
  compares it against the real fixture source for every authored fact relevant
  to the checks in scope
- validate against the current compiler-side extraction state rather than
  treating `raw-frontend.json` as the primary boundary artifact
- inspect the extraction surfaces named in the check contract rather than a
  hand-constructed substitute or a later lowering artifact
- create a uniquely named integration test file owned by this task only
- do not reuse, edit, or delete another `source-to-extraction-*` task's test
  file
- write the detailed validation report to the declared output path
- keep the harness `fail_reason` short and descriptive
- ensure every `pass` in the report is backed by an automated assertion that
  directly exercises the required extraction surface
- include the actual assertion predicate(s) used for each check in the report
- include explicit specimen-fidelity assertion predicates and exact evidence in
  the report when a reduced or inline specimen is used
- run Cargo commands with a task-specific target dir, for example
  `CARGO_TARGET_DIR=target/source-to-extraction-06`
- use closed-world evidence for absence claims such as lost definitional
  relationships or owner membership

This task should not:

- rewrite the check specification
- widen the scope beyond the driver-constants boundary
- mark a check as `pass` based only on supporting prose or on a surface that
  the automated test does not actually inspect
- continue with extraction `pass` claims if the authored facts in the check
  file disagree with the fixture source
- continue with extraction `pass` claims when reduced-specimen fidelity has not
  been established by exercised evidence against the real fixture source
- use tautological or non-falsifiable assertions as exercised evidence

## Test Plan

Validation should include:

1. compiler-side inspection of the authored->extraction boundary for the PL011
   driver constants
2. assertions corresponding to the checks in:
   - `docs/validation/01-pl011/checks/06-driver-constants-extraction.md`
3. a precondition verification that the check file's authored facts match the
   fixture source
4. if a reduced or inline specimen is used, an exercised fidelity check that
   compares it against the fixture source for all relevant authored facts
5. a narrow automated test path that can be rerun deterministically
6. a report that records specimen fidelity, check-to-surface mapping, actual
   assertion predicates, and per-check exercised evidence

Suggested commands:

- `CARGO_TARGET_DIR=target/source-to-extraction-06 cargo test -p regseq-driver`
- `CARGO_TARGET_DIR=target/source-to-extraction-06 cargo test -p regseq-driver --test <task-specific-test-name>`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

## Acceptance Criteria

- a dedicated compiler-side validation test exists for this boundary check
- the validation uses the check specification as the source of truth
- the task verifies the copied authored facts against the fixture source before
  judging the extraction boundary
- when using a reduced or inline specimen, the task proves specimen fidelity
  against the real fixture source with exercised evidence before any affected
  check is allowed to pass
- the validation inspects compiler-side extraction state rather than relying
  only on the persisted raw artifact
- every `pass` in the report is backed by a direct automated assertion against
  the required extraction surface
- every reported assertion is non-tautological and included in the report
- the report explains specimen fidelity and check-to-surface mapping
- no `pass` relies on a weaker surrogate than the authored premise
- failures can be recorded precisely in harness notes
- the task remains scoped to the driver-constants boundary
- the task uses a uniquely named test file that does not conflict with sibling
  `source-to-extraction-*` tasks
- tests pass
