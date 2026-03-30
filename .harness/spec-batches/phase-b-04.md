# Spec Batch: phase-b-04 - PL011-core spec crate scaffold and core frontend evidence

## Description

Author the fourth Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the first real `track-a-pl011` tasks that prove the
accepted frontend surface is real and not only synthetic fixture coverage. The
tasks should cover a minimal PL011-facing spec crate with register definitions
and core driver declarations over the accepted PL011-core subset.

The output should establish the primary real-device frontend evidence used by
later extraction and validation work.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`
- `docs/plans/asplos-implementation-plan.md`
- `docs/pl011-compiler-collateral/compiler-baseline-overview.md`
- `docs/pl011-compiler-collateral/compiler-constraints.md`
- `docs/guide/pl011-spec-outline.md`
- `docs/guide/device-coverage.md`

## Output Specification

Create or modify:

- `.harness/tasks/track-a-pl011/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for the
`track-a-pl011` group.

## Implementation Notes

Follow these fixed decisions:

- this batch belongs to frontend closure, not internal model work
- keep the first PL011 slice minimal but real
- use the accepted facade, macro, and intrinsic surfaces already introduced by
  earlier Phase B batches
- keep optional or deferred constructs out unless the accepted PL011 baseline
  requires them to complete frontend closure
- treat `track-a-pl011` as primary evidence; add narrow synthetic fixtures only
  when they isolate a specific frontend edge

Task decomposition should separate:

- basic spec-crate scaffold and compile path
- register block / bitfield declarations
- core driver declarations over the accepted subset

## Non-Goals

Do not include:

- raw extraction tasks
- normalized model tasks
- Layer 2 parser or join work
- lowering or backend logic
- E1000-facing spec work

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under `.harness/tasks/track-a-pl011/`
- the tasks define a real PL011-facing frontend slice rather than validation-only fixtures
- the first PL011 slice remains minimal and frontend-oriented
- `tasks.json` references the new task IDs under `track-a-pl011`
- `phase-b-04` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-04` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
