# Spec Batch: phase-c-02 - regseq-model normalized binding model and Layer 1 plus Layer 2 join

## Description

Author the second Phase C batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the normalized binding-model tasks and the real
Layer 1 + Layer 2 join tasks needed to turn the supported PL011 slice into a
reviewable joined artifact suitable for lowering preparation.

The output should establish the join boundary explicitly and keep it distinct
from both frontend parsing and backend lowering.

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
- `docs/guide/implementation-guide.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-model/`
- `.harness/tasks/track-a-validation/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs, primarily for
`regseq-model`, with validation tasks only where needed to prove the joined
artifact boundary.

## Implementation Notes

Follow these fixed decisions:

- build the normalized binding model from the accepted Layer 2 contract already
  established in Phase B
- make the join boundary explicit and reviewable
- use real PL011 binding instances to prove the join
- do not widen the Layer 2 contract during normalization
- do not pull lowering or backend concerns into this batch

## Non-Goals

Do not include:

- raw extraction tasks
- frontend parser work
- declaration-model redesign beyond what Phase C requires
- backend lowering
- E1000 expansion

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under `.harness/tasks/regseq-model/`
  and/or `.harness/tasks/track-a-validation/`
- the task specs explicitly cover normalized binding model and real join work
- the joined artifact is treated as the output boundary for this batch
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-c-02` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-c-02` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
