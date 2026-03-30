# Spec Batch: phase-b-03 - regseq facade intrinsic stubs for PL011-core frontend support

## Description

Author the third Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the minimal intrinsic-surface task set needed to make
the accepted PL011 frontend slice authorable. The main expected candidates are
the accepted logging and timed-delay surfaces, and only if the frontend
evidence actually requires them.

The result should be a bounded set of facade tasks that provide compile-time
authoring support without making premature lowering commitments.

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
- `docs/pl011-compiler-collateral/constructs/diagnostic-logging.md`
- `docs/pl011-compiler-collateral/constructs/timed-cpu-delay-and-sleep.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-facade/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for the
`regseq-facade` group.

## Implementation Notes

Follow these fixed decisions:

- treat these as authoring-support intrinsics first
- keep the intrinsic surface minimal
- use only accepted intrinsic semantics from the compiler collateral
- do not commit lowering behavior in this batch
- do not pull E1000-only intrinsic surfaces into this batch

If the accepted PL011 frontend slice can compile without one of the candidate
intrinsic families, omit it from this batch rather than speculating.

## Non-Goals

Do not include:

- facade wrapper or callback-family trait work already covered by `phase-b-01`
- macro work
- extractor or raw-artifact work
- normalized model or join work
- lowering contracts or backend behavior

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under `.harness/tasks/regseq-facade/`
- the intrinsic surface is explicitly bounded to accepted frontend needs
- the task specs clearly avoid making lowering commitments
- `tasks.json` references the new task IDs under `regseq-facade`
- `phase-b-03` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-03` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
