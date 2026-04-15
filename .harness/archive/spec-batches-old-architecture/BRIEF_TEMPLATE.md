# Spec Batch: <tracker-id> - <title>

## Description

Describe the batch in 2-4 short paragraphs.

State:

- what this batch is for
- whether it is task-spec authoring work or executable implementation work
- what concrete outcome it should produce

Keep the batch bounded to one tracker item.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `<other exact files required for this batch>`

List only the files required for a cold session to complete this batch well.

## Output Specification

Create or modify:

- `<exact file or file pattern>`
- `<exact file or file pattern>`

Update when done:

- `.harness/task-spec-status.yaml`

State explicitly whether the output should be:

- executable harness tasks
- harness metadata only
- a new brief
- or some combination of the above

## Implementation Notes

Record the fixed decisions that the worker must follow.

Include:

- any decomposition guidance
- any naming constraints
- any important references from the implementation plan
- any sequencing or dependency constraints

If the batch exists to author task specs, say so clearly.

## Non-Goals

List the work that is explicitly out of scope for this batch.

This section should prevent a cold session from expanding the batch into
adjacent phases.

## Acceptance Criteria

Define what must be true for this batch to count as complete.

Use concrete completion conditions such as:

- specific files created or modified
- tracker item status updated
- generated task IDs recorded
- a required review artifact produced

## Update Instructions

State exactly how `.harness/task-spec-status.yaml` should be updated after
the batch is finished.

Include:

1. which item changes status
2. what status it changes to
3. whether `generated_task_ids` must be filled
4. whether the next dependency-satisfied item should move to `ready_to_elaborate`
