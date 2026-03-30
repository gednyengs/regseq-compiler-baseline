# Spec Batch: phase-b-07 - frontend closure validation, boundary compatibility, and deferred Phase B expansion

## Description

Author the seventh Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the validation and completion tasks needed to finish
frontend closure for the accepted PL011 baseline. That includes raw-artifact
snapshot coverage, Layer 1/Layer 2 boundary compatibility checks,
caution-focused extraction QC, and any deferred-but-still-Phase-B PL011
construct work needed to finish the accepted frontend scope.

The output should close Phase B without moving deferred frontend constructs into
Phase C.

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
- `archive/guide/pl011-spec-outline.md`
- `archive/guide/open-issues.md`

## Output Specification

Create or modify:

- `.harness/tasks/track-a-pl011/`
- `.harness/tasks/track-a-validation/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for frontend
closure completion.

## Implementation Notes

Follow these fixed decisions:

- keep this batch in Phase B
- validate against the raw artifact and frontend boundary, not the final joined model
- use `track-a-pl011` as the main real-device evidence
- add narrow validation fixtures only where they isolate frontend edge cases
- include stronger QC for caution constructs present in the supported slice
- if deferred Phase B constructs are required, keep them bounded to frontend
  closure and do not let them spill into normalized model work

## Non-Goals

Do not include:

- normalized declaration-model work
- normalized binding-model work
- join logic
- backend lowering
- E1000 expansion

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under `.harness/tasks/track-a-pl011/`
  and/or `.harness/tasks/track-a-validation/`
- the task specs explicitly target frontend-closure completion
- any deferred construct work is kept in Phase B and justified by the accepted frontend baseline
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-b-07` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-07` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
