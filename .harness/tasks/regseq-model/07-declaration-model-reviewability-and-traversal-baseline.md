# Task: regseq-model-07 - Declaration-model reviewability and traversal baseline

## Description

Make the declaration model reviewable and traversal-friendly enough for later
Phase C work.

This task should prove that the normalized declaration model is not only
constructible, but also inspectable and usable as the stable input to later
binding, join, and analysis work.

## Input Context

Read before starting:

- `docs/discussions/regseq-model/phase-c-01-declaration-model-plan.md`
- `crates/regseq-model/src/`
- `.harness/tasks/regseq-model/05-declaration-model-schema-and-boundaries.md`
- `.harness/tasks/regseq-model/06-raw-frontend-artifact-to-declaration-model-normalization.md`
- `tests/fixtures/pl011-spec-slice/expected/raw-frontend.json`
- `tests/fixtures/pl011-full-spec/`

## Output Specification

Modify:

- `crates/regseq-model/src/`
- `crates/regseq-model/tests/`

Add only the reviewability and traversal support needed for the declaration
model to serve as a stable reviewer-facing and later-pass-facing artifact.

## Implementation Notes

This task should provide:

- serialization or snapshot-friendly representation for the declaration model
- stable debug/review output for representative normalized PL011 frontend data
- traversal-friendly access patterns that do not destroy construct ownership

Good targets include:

- round-trip serialization tests
- stable field-ordering or deterministic output tests
- fixture-backed model snapshots based on the supported PL011 frontend
- helper iteration/query surfaces that still respect construct-local ownership

The spec-slice raw artifact is acceptable as one deterministic fixture input,
but it is not sufficient by itself because several raw families are empty there.
This task should therefore prove reviewability with:

- the checked-in spec-slice raw artifact, and
- at least one richer normalized declaration artifact that exercises families
  absent from the spec-slice fixture

Keep this task bounded:

- do not introduce binding-model or join traversal
- do not add backend-facing lowering helpers
- do not replace construct-local ownership with one global operation graph

## Test Plan

Add tests to verify:

1. declaration-model output is deterministic
2. a representative normalized PL011 declaration artifact is reviewable in a
   stable serialized/debug form
3. traversal helpers expose construct-local declarations and operation evidence
   without flattening ownership
4. serialization round-trips where appropriate
5. richer declaration families beyond the narrow spec-slice path are also
   reviewable in deterministic form

Test commands:

- `cargo test -p regseq-model`
- `cargo clippy -p regseq-model -- -D warnings`

## Acceptance Criteria

- the declaration model has a stable reviewer-facing representation
- the normalized PL011 declaration artifact can be inspected deterministically
- traversal support exists without collapsing construct ownership
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
