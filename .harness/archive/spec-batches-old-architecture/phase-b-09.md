# Spec Batch: phase-b-09 - raw-artifact and frontend-closure validation

## Description

Author the ninth Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the validation and completion tasks needed to close
frontend capture for the accepted PL011 baseline. That includes raw extraction
artifact snapshots, caution-focused frontend QC, and the minimal Layer 1 /
Layer 2 boundary compatibility checks needed before Phase C model work begins.

The output should close Phase B without drifting into normalized models or
lowering.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`
- `docs/plans/asplos-implementation-plan.md`
- `docs/compiler-collateral/compiler-baseline-overview.md`
- `docs/compiler-collateral/compiler-constraints.md`
- `archive/guide/open-issues.md`
- `archive/guide/pl011-spec-outline.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-driver/`
- `.harness/tasks/track-a-validation/`
- `.harness/tasks/track-a-pl011/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for final
frontend-closure proof across the appropriate groups.

## Implementation Notes

Follow these fixed decisions:

- this batch closes Phase B
- validate against the raw artifact and frontend boundary, not the final joined
  model
- use the real PL011 slice as the main evidence source
- add targeted validation fixtures only where they isolate a frontend edge
- include stronger QC for caution constructs present in the accepted slice
- keep deferred frontend work in Phase B only if it is necessary to complete the
  accepted frontend baseline

Typical decomposition should separate:

- raw-artifact snapshot and review tasks
- caution-focused frontend capture QC
- minimal Layer 1 / Layer 2 boundary compatibility checks

## Non-Goals

Do not include:

- normalized declaration-model work
- normalized binding-model work
- Layer 1 + Layer 2 join logic
- backend lowering
- E1000 expansion

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under the frontend-closure target groups
- the task specs explicitly target raw-artifact proof and frontend-boundary
  validation
- any deferred construct work is kept in Phase B and justified by the accepted
  frontend baseline
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-b-09` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-09` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
