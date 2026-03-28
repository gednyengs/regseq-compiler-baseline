# Spec Batch: phase-a-02 - Harness group expansion for implementation work

## Description

Author the second harness task-spec batch for the ASPLOS implementation stream.
This batch should extend the harness structure itself so later implementation
tasks have stable group homes.

This is task-spec authoring work, not code implementation work.

The goal is to define the harness grouping layout needed for the new
implementation stream after the initial workspace-activation tasks exist.

## Input Context

Read before drafting task specs:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`
- `.harness/spec-batches/phase-a-01.md`
- `docs/plans/asplos-implementation-plan.md`
- `docs/guide/implementation-guide.md`

## Output Specification

Create or modify:

- `.harness/tasks.json`
- any new group directories under `.harness/tasks/` needed to host later work

Update when done:

- `.harness/task-spec-status.yaml`

The result of this batch should be harness structure that can cleanly host
later implementation task specs.

## Implementation Notes

This batch should define the grouping layout, not the substantive
implementation tasks inside those groups.

Expected target groups:

- `regseq-facade`
- `regseq-macros`
- `regseq-model`
- `regseq-lowering`
- `regseq-linux-backend`
- `regseq-zephyr-backend`
- `track-a-pl011`
- `track-a-e1000`
- `track-a-validation`
- `track-a-eval`

Keep naming aligned with the implementation plan unless there is a strong
reason to do otherwise.

## Non-Goals

Do not include:

- real implementation tasks inside the new groups unless they are required
  to make the harness metadata valid
- DSL surface work
- extraction work
- PL011 or E1000 spec work
- lowering or backend logic

## Acceptance Criteria

This spec batch is complete when:

- the harness group layout for the implementation stream is represented in
  `.harness/tasks.json`
- any required group directories exist under `.harness/tasks/`
- the tracker entry `phase-a-02` is updated to `tasked`
- `generated_task_ids` for `phase-a-02` is updated if this batch creates
  executable harness tasks, or left empty if it only changes group structure

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-a-02` to `tasked`
   - populate `generated_task_ids` if applicable
2. Move the next dependency-satisfied item to `ready_to_elaborate` if
   appropriate.
3. Do not modify unrelated tracker items.
