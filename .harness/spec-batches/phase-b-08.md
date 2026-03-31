# Spec Batch: phase-b-08 - real PL011 frontend slice assembly

## Description

Author the eighth Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the first real `track-a-pl011` tasks that prove the
accepted Phase B surfaces compose in a realistic PL011-facing driver setting,
not only in isolated snippets. The tasks should cover a narrow but real PL011
frontend slice over the accepted baseline.

The output should establish the primary real-device frontend evidence used by
the final Phase B closure checks.

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
- `archive/guide/device-coverage.md`

## Output Specification

Create or modify:

- `.harness/tasks/track-a-pl011/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for the real
PL011 frontend slice under `track-a-pl011`.

## Implementation Notes

Follow these fixed decisions:

- keep this batch frontend-only and centered on a narrow but real PL011 slice
- use the accepted shared foundations and shared extensions from earlier Phase B
  batches
- use `track-a-pl011` as the primary evidence stream
- add narrow synthetic fixtures only where they isolate a specific frontend edge
- do not move raw-artifact closure, normalized model work, or lowering work into
  this batch

Task decomposition should separate:

- PL011 spec-crate scaffold and compile path
- register and field declarations needed by the accepted slice
- driver declarations and callback implementations needed by the accepted slice

## Non-Goals

Do not include:

- raw-artifact closure work
- normalized model or join work
- backend lowering
- E1000-facing spec work

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under `.harness/tasks/track-a-pl011/`
- the tasks define a real PL011-facing frontend slice rather than snippet-only
  fixtures
- the first PL011 slice remains narrow and frontend-oriented
- `tasks.json` references the new task IDs under `track-a-pl011`
- `phase-b-08` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-08` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
