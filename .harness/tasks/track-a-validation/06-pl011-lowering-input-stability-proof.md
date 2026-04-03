# Task: track-a-validation-06 - PL011 lowering-input stability proof

## Description

Add the final reviewer-facing proof that the current PL011 declaration model,
joined artifact, and analysis bootstrap together form a stable lowering-input
contract before Phase D begins.

This task is validation-only. It should not redesign the models or the
analysis-layer contract.

## Input Context

Read before starting:

- `docs/discussions/regseq-analysis/phase-c-03-analysis-bootstrap-plan.md`
- `.harness/tasks/regseq-analysis/02-analysis-input-and-lowering-bootstrap.md`
- `.harness/tasks/regseq-analysis/03-lowering-input-readiness-and-consistency-checks.md`
- `.harness/tasks/track-a-validation/05-pl011-joined-artifact-proof.md`
- `tests/fixtures/pl011-spec-slice/`
- `tests/fixtures/pl011-full-spec/`

## Output Specification

Modify:

- `tests/e2e/tests/`
- `crates/regseq-analysis/tests/` only if small shared snapshots or helpers are
  needed

Add reviewer-facing validation that demonstrates the PL011 lowering-input
contract is stable, deterministic, and honest about both complete and partial
facts.

## Implementation Notes

This proof should cover:

- the accepted PL011 declaration-side input
- the accepted joined artifact
- the analysis/readiness output built over them

The proof should explicitly demonstrate:

- deterministic output suitable for review/snapshot comparison
- callback-family visibility for the current serial/lifecycle surface
- property-lint visibility
- resource join completeness and incompleteness where relevant
- unmatched fact visibility rather than silent drop
- declaration-only construct accessibility for registers, synchronization, and
  operation evidence through the analysis bootstrap layer

This task should produce or update at least one checked-in snapshot that Phase
D tasks can reference as the stable lowering-input baseline. The snapshot
should cover the analysis/readiness output for the PL011 artifact, not only the
joined artifact already covered by `track-a-validation-05`.

The proof should include stronger QC for caution-side facts already present in
the current supported PL011 frontend, rather than assuming the joined artifact
alone is enough.

Do not turn this task into:

- model redesign
- binding-schema redesign
- analysis-policy redesign beyond the accepted readiness scope
- backend lowering validation
- DMA or optional richer serial expansion

## Test Plan

Add tests to verify:

1. the PL011 lowering-input proof remains deterministic
2. the analysis/readiness layer sees the expected callback families
3. property-lint state is visible in reviewer-facing output
4. partial join outcomes remain visible in reviewer-facing output
5. unmatched facts remain visible in reviewer-facing output
6. declaration-only constructs (register blocks/descriptors, synchronization
   fields, operation call sites) remain accessible through the bootstrap layer
7. the resulting proof is stable enough to act as the last Phase C gate before
   Phase D authoring

Test commands:

- `cargo test -p regseq-analysis`
- `cargo test -p regseq-e2e`
- `cargo clippy --workspace -- -D warnings`

## Acceptance Criteria

- there is a reviewer-facing PL011 lowering-input stability proof
- the proof covers joined-artifact stability plus analysis/readiness output
- caution-focused visibility is preserved for the supported PL011 frontend
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
