# Spec Batch: phase-b-03 - synchronization and interrupt shared extension

## Description

Author the third Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the shared interrupt and synchronization substrate used
across later frontend slices. The output should cover interrupt wrappers and
metadata, bound-handler capture forms, the synchronization primitives needed
before later callback and real-PL011 slices can be authored cleanly, and the
deferred `regseq::ordered!` register-ordering macro surface.

The result should be a bounded set of facade and driver tasks that establish the
shared concurrency and handler-binding substrate without drifting into full
construct completion for every interrupt-using feature.

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
- `docs/compiler-collateral/constructs/device-interrupt-setup-and-handling.md`
- `docs/compiler-collateral/constructs/synchronization.md`
- `docs/compiler-collateral/constructs/register-access.md`
- `docs/discussions/regseq-intrinsics/regseq-intrinsics-discussion.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-facade/`
- `.harness/tasks/regseq-macros/`
- `.harness/tasks/regseq-driver/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for the shared
interrupt and synchronization extension across the appropriate facade, macro,
and driver groups.

## Implementation Notes

Follow these fixed decisions:

- keep this batch focused on shared interrupt and synchronization foundations
- use the accepted handler-binding model from the compiler collateral
- keep synchronization type-driven
- assign the deferred `regseq::ordered!` user-facing ordering surface to this batch
- include only the shared interrupt and synchronization surfaces required by the
  accepted PL011 slice
- do not commit lowering behavior in this batch

Typical decomposition should separate:

- canonical intrinsic-home normalization needed by this shared substrate
- interrupt wrappers and handler-facing types
- bound-handler metadata and method-form capture
- synchronization wrappers and guard surfaces
- register-ordering macro surface and its bounded compiler-facing recognition

## Non-Goals

Do not include:

- MMIO/register work already covered by `phase-b-02`
- callback-family trait work
- unrelated declarative property work
- normalized model or join work
- lowering contracts or backend behavior

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under the shared interrupt/synchronization target groups
- interrupt wrappers, bound-handler forms, synchronization primitives, and the deferred `regseq::ordered!` surface are explicitly represented
- the task specs keep this work bounded to shared substrate concerns
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-b-03` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-03` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
