# Task: source-to-extraction-08 - PL011 helper methods B authored->extraction boundary validation

## Description

Perform the boundary validation defined in:

- `docs/validation/01-pl011/checks/08-helper-methods-extraction-b.md`

This task validates whether the second half of the authored helper-method facts
for PL011 survive at the compiler-side boundary between the authored PL011
source and the current Regseq extraction state.

This is a validation/proof task, not a feature-implementation task.

## Input Context

Read before starting:

- `docs/validation/01-pl011/01-authored-source.md`
- `docs/validation/01-pl011/checks/08-helper-methods-extraction-b.md`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `.harness/tasks/regseq-driver/19-raw-frontend-artifact-emission.md`
- `crates/regseq-driver/src/`
- `crates/regseq-driver/tests/`

## Output Specification

Output Path:

- `docs/validation/01-pl011/checks/results/08-helper-methods-extraction-b.md`

Test Path:

- `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs`

Create or modify:

- `crates/regseq-driver/tests/source_to_extraction_08_helper_methods_b.rs`

Modify if needed:

- `crates/regseq-driver/src/` only if an existing test helper must be exposed
  for inspection and doing so does not change production compiler behavior

Also create or update:

- `docs/validation/01-pl011/checks/results/08-helper-methods-extraction-b.md`

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
- create or update exactly the declared `Test Path`
- do not reuse, edit, or delete another `source-to-extraction-*` task's test file
- write the detailed validation report to the declared output path
- keep the harness `fail_reason` short and descriptive
- ensure every `pass` in the report is backed by an automated assertion that
  directly exercises the required extraction surface
- include the actual assertion predicate(s) used for each check in the report
- include explicit specimen-fidelity assertion predicates and exact evidence in
  the report when a reduced or inline specimen is used
- run Cargo commands with a task-specific target dir, for example
  `CARGO_TARGET_DIR=target/source-to-extraction-08`
- ensure multi-property premises are proven by assertions that cover every
  named property, not just a subset
- use path-sensitive assertions for the body-heavy helpers:
  - `uart_clock_hz`: prove higher-order `Clock::get_rate` combinator usage and
    exact fallback constant flow
  - `shutdown_port`: prove exact helper order and `pending_tx = None` placement
  - `service_rx_fifo`: prove exact loop condition, handled flag behavior, and
    returned-bool correspondence
  - `service_tx_fifo`: prove exact Some/None and ready/not-ready path behavior
  - `clear_explicit_interrupts`: prove conditional write only when clear_mask != 0

This task should not:

- rewrite the check specification
- widen the scope beyond the helper-methods-B boundary
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
   helper methods in check group B
2. assertions corresponding to the checks in:
   - `docs/validation/01-pl011/checks/08-helper-methods-extraction-b.md`
3. a precondition verification that the check file's authored facts match the
   fixture source
4. if a reduced or inline specimen is used, an exercised fidelity check that
   compares it against the fixture source for all relevant authored facts
5. a narrow automated test path that can be rerun deterministically
6. a report that records specimen fidelity, check-to-surface mapping, actual
   assertion predicates, and per-check exercised evidence

Suggested commands:

- `CARGO_TARGET_DIR=target/source-to-extraction-08 cargo test -p regseq-driver`
- `CARGO_TARGET_DIR=target/source-to-extraction-08 cargo test -p regseq-driver --test <task-specific-test-name>`
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
- path-sensitive helper checks only pass if each authored branch/path predicate
  is exercised directly
- failures can be recorded precisely in harness notes
- the task remains scoped to the helper-methods-B boundary
- the task uses a uniquely named test file that does not conflict with sibling
  `source-to-extraction-*` tasks
- the task creates or updates exactly the declared `Test Path`
- tests pass
