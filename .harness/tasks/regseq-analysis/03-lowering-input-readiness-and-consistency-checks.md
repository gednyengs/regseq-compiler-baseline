# Task: regseq-analysis-03 - Lowering-input readiness and consistency checks

## Description

Implement the minimum analysis checks needed to confirm that the current PL011
declaration-plus-joined artifact is complete, consistent, and reviewable enough
to act as the lowering-input contract for Phase D.

This task should stay analysis-facing. It should not become backend lowering or
semantic diagnosis beyond the accepted Phase C boundary.

## Input Context

Read before starting:

- `docs/discussions/regseq-analysis/phase-c-03-analysis-bootstrap-plan.md`
- `.harness/tasks/regseq-analysis/02-analysis-input-and-lowering-bootstrap.md`
- `.harness/tasks/regseq-model/07-declaration-model-reviewability-and-traversal-baseline.md`
- `.harness/tasks/regseq-model/10-declaration-binding-join-and-joined-artifact-baseline.md`
- `.harness/tasks/track-a-validation/05-pl011-joined-artifact-proof.md`
- `docs/plans/asplos-implementation-plan.md`
- `docs/compiler-collateral/compiler-constraints.md`

## Output Specification

Modify:

- `crates/regseq-analysis/src/`
- `crates/regseq-analysis/tests/`

Add focused analysis passes or report builders that assess lowering-input
readiness over the current supported PL011 artifact.

## Implementation Notes

This task should NOT retest join correctness already covered by
`track-a-validation-05`. That task already proved that the PL011 join produces
structurally correct output.

This task adds analysis-layer checks that require the bootstrap contract from
`regseq-analysis-02` and that assess readiness properties spanning declaration
plus joined data together, such as caution-fact visibility and device-class
callback-family completeness.

The readiness checks should stay bounded to the accepted concerns for this
phase:

- operation/callback-family integrity for the current device-class surface
- property join visibility, including required-with-default lint carry-through
- resource join completeness and partial-join visibility
- cross-family ownership consistency, for example where declaration-side
  operation evidence refers to functions that should appear under one of the
  accepted callback families for the current device class
- explicit visibility of unmatched declaration-side and binding-side facts

The analysis should explicitly keep caution points visible for the current
supported PL011 frontend, including:

- required-with-default property lint state
- incomplete joins for supported resource families
- interrupt binding hints remaining diagnostic-only rather than authoritative
- optional-resource presence versus required-resource absence

The output may be a structured readiness report, check result set, or similar
deterministic analysis artifact, but it must not:

- perform backend-specific lowering
- convert partial join outcomes into silent success
- infer new declaration or binding facts
- turn unmatched facts into hard errors unless the accepted Phase C contract
  already requires that

## Test Plan

Add tests to verify:

1. the readiness checks succeed on the accepted PL011 lowering-input baseline
2. callback-family/operation integrity is reported deterministically
3. property lint state remains visible in the readiness output
4. partial and unmatched join outcomes remain explicit in the readiness output
5. interrupt binding-side authority is preserved in analysis output
6. representative negative cases fail or report instability in a reviewable way

Test commands:

- `cargo test -p regseq-analysis`
- `cargo clippy -p regseq-analysis -- -D warnings`

## Acceptance Criteria

- `regseq-analysis` provides deterministic lowering-input readiness checks over
  the current declaration-plus-joined PL011 artifact
- caution constructs present in the supported frontend remain visible
- no backend lowering logic is introduced
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
