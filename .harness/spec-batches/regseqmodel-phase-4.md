# Spec Batch: regseqmodel-phase-4 - Extraction pipeline updates

## Description

This batch authors executable harness task specs for updating the
extraction pipeline to recognize new `#[rseq::]` attributes and build
attribute-based registries. Includes cross-crate attribute validation
(gate test), new extraction modules (intrinsic/framework/abstraction
registries), pin-state extraction, source location extraction,
RawFrontendArtifact expansion, unit test specimen updates, and golden
file regeneration.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-status.yaml`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 4)
- `crates/regseq-driver/src/callbacks.rs`
- `crates/regseq-driver/src/raw_artifact.rs`
- `crates/regseq-driver/src/extract/mod.rs`
- `crates/regseq-driver/tests/` (33+ test files)
- `tests/fixtures/pl011-full-spec/expected/raw-frontend.json`

## Output Specification

Create:

- `.harness/tasks/regseq-driver/` task spec files
- `.harness/tasks/track-a-validation/` task spec files

Update when done:

- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`

The output is executable harness tasks.

## Implementation Notes

- Phase 4.0 (cross-crate attribute validation) MUST be the first task.
  It gates the rest of this phase.
- New extraction modules: `intrinsic_registry.rs`, `framework_types.rs`,
  `abstraction_types.rs`.
- Pin-state extraction extends `extract/pinctrl.rs`.
- Source location extraction adds spans for all DefId items.
- RawFrontendArtifact gets new surface fields.
- 33+ unit test specimens need attribute updates.
- Golden files regenerated after extraction output changes.

## Non-Goals

- No IR construction (Phase 5).
- No normalization or model changes.
- No backend changes.

## Acceptance Criteria

- Task specs created. Cross-crate gate test is first dependency.
- `cargo test` on regseq-driver passes. E2E tests pass with updated
  golden files.
- Tracker updated.

## Update Instructions

1. Change `regseqmodel-phase-4` status to `tasked`.
2. Fill `generated_task_ids`.
3. Check if `regseqmodel-phase-5` can advance.
