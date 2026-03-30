# Spec Batch: phase-c-01 - regseq-model normalized declaration model for the supported PL011 slice

## Description

Author the first Phase C batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the `regseq-model` tasks that normalize the raw
extraction artifact into stable rustc-independent declaration-model types for
the supported PL011 slice.

The output should make the beginning of internal-model work explicit and should
be driven by what raw extraction actually produces, not by a precomputed model
shape that outruns the frontend artifact.

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
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for the
`regseq-model` group.

## Implementation Notes

Follow these fixed decisions:

- start from the raw extraction artifact produced in Phase B
- preserve construct ownership, operation classes, type-driven properties, and
  source traceability
- keep normalized declaration-model work separate from the normalized binding
  model and from join logic
- keep the supported slice bounded to the accepted PL011 frontend closure output

Task decomposition should reflect clear declaration-model responsibilities
rather than frontend extraction or backend concerns.

## Non-Goals

Do not include:

- raw extraction tasks
- Layer 2 parser tasks
- normalized binding-model tasks
- join logic
- lowering or backend work

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under `.harness/tasks/regseq-model/`
- the task specs clearly start from raw extraction rather than speculative normalized shapes
- declaration-model work is kept separate from binding-model and join work
- `tasks.json` references the new task IDs under `regseq-model`
- `phase-c-01` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-c-01` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
