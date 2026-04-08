# Task: source-to-extraction-01 - PL011 register-block authored->extraction boundary validation

## Description

Perform the boundary validation defined in:

- `docs/validation/01-pl011/checks/01-register-block-extraction.md`

This task validates whether the authored register-block facts for
`Pl011Registers` survive at the compiler-side boundary between the authored
PL011 source and the current Regseq extraction state.

This is a validation/proof task, not a feature-implementation task.

## Input Context

Read before starting:

- `docs/validation/01-pl011/01-authored-source.md`
- `docs/validation/01-pl011/checks/01-register-block-extraction.md`
- `tests/fixtures/pl011-full-spec/src/registers.rs`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `.harness/tasks/regseq-driver/06-register-mmio-frontend-capture-scaffolding.md`
- `.harness/tasks/regseq-driver/19-raw-frontend-artifact-emission.md`
- `crates/regseq-driver/src/`
- `crates/regseq-driver/tests/`

## Output Specification

Output Path:

- `docs/validation/01-pl011/checks/results/01-register-block-extraction.md`

Create or modify:

- `crates/regseq-driver/tests/` only as narrowly needed for a dedicated
  compiler-side boundary validation test

Modify if needed:

- `crates/regseq-driver/src/` only if an existing test helper must be exposed
  for inspection and doing so does not change production compiler behavior

Also create or update:

- `docs/validation/01-pl011/checks/results/01-register-block-extraction.md`

The report file should contain the validation certificate for this task. The
harness task status and failure notes should summarize the result and point to
the report file rather than replacing it.

If the task fails:

- record a short descriptive `fail_reason` in the harness
- put the detailed reasoning, evidence, per-check outcomes, and counterexamples
  in the report file at the declared output path
- include `Suggested Fix Direction / Implementation Hints` for each failed
  check in the report file
- include a short `Specimen Fidelity` section in the report that states whether
  the validation used the real fixture files or a reduced inline mirror
- if a reduced inline mirror is used, justify why it is equivalent for every
  check in scope
- include a short `Check-to-Surface Mapping` section in the report that maps
  each check to the extraction surface(s) and test assertion(s) used to prove it

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
- use programmatic inspection where possible
- inspect the extraction surfaces named in the check contract rather than a
  hand-constructed substitute or a later lowering artifact
- create a uniquely named integration test file owned by this task only
- do not reuse, edit, or delete another `source-to-extraction-*` task's test
  file
- cite exact code locations in failure notes when a check fails
- write the detailed validation report to the declared output path
- keep the harness `fail_reason` short and descriptive
- include concise remediation hints for failed checks in the report
- ensure every `pass` in the report is backed by an automated assertion that
  directly exercises the required extraction surface
- prefer using the real PL011 fixture files directly
- if using an inline or reduced specimen, preserve every authored structural
  fact required by the checks and explain that equivalence in the report
- require exact-premise proof: a `pass` must be proven at the same specificity
  as the authored premise, not by a weaker surrogate fact
- include the actual assertion predicate(s) used for each check in the report
- include explicit specimen-fidelity assertion predicates and exact evidence in
  the report when a reduced or inline specimen is used
- run Cargo commands with a task-specific target dir, for example
  `CARGO_TARGET_DIR=target/source-to-extraction-01`
- use closed-world evidence for absence claims such as "no registers are
  inserted, removed, renamed, or reordered"
- ensure multi-property premises are proven by assertions that cover every
  named property, not just a subset
- keep the validation narrow and reviewer-readable

This task should not:

- rewrite the check specification
- widen the scope beyond the register-block boundary
- turn into a broad frontend-QC sweep
- rely on naming heuristics when the compiler-side boundary does not preserve a
  fact explicitly
- mark a check as `pass` based only on supporting prose or on a surface that
  the automated test does not actually inspect
- mark a check as `pass` based on a suffix match, partial shape match, or
  “equivalent mechanism” argument when the authored premise is stronger
- continue with extraction `pass` claims if the authored facts in the check
  file disagree with the fixture source
- continue with extraction `pass` claims when reduced-specimen fidelity has not
  been established by exercised evidence against the real fixture source
- use tautological or non-falsifiable assertions as exercised evidence

If the validation finds that authored facts are dropped or weakened at this
boundary, let the task fail and record targeted harness failure notes rather
than compensating with narrative-only explanations.

## Test Plan

Validation should include:

1. compiler-side inspection of the authored->extraction boundary for the PL011
   fixture
2. assertions corresponding to the checks in:
   - `docs/validation/01-pl011/checks/01-register-block-extraction.md`
3. a precondition verification that the check file's authored facts match the
   fixture source
4. if a reduced or inline specimen is used, an exercised fidelity check that
   compares it against the fixture source for all relevant authored facts
5. a narrow automated test path that can be rerun deterministically
6. a report that records specimen fidelity, check-to-surface mapping, actual
   assertion predicates, and
   per-check exercised evidence

Suggested commands:

- `CARGO_TARGET_DIR=target/source-to-extraction-01 cargo test -p regseq-driver`
- `CARGO_TARGET_DIR=target/source-to-extraction-01 cargo test -p regseq-driver --test <task-specific-test-name>`
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
- the task remains scoped to the register-block boundary
- the task uses a uniquely named test file that does not conflict with sibling
  `source-to-extraction-*` tasks
- tests pass
