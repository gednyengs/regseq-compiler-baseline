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

## Spec References

Required list of `<spec-file>#<anchor>` references for the batch as a
whole. Downstream tasks inherit a subset of these in their own
`spec_refs` arrays via `task.py add-task --spec-refs`. Use
`uv run python scripts/python/task.py read-spec-refs <spec-file>` to
emit the canonical anchor slugs for piping in.

Example:

- `regseq-model/spec.md#19`
- `regseq-model/spec.md#20`
- `regseq-model/pretty-printer.md#5`

## Phase

Required single phase tag for the batch (e.g. `1a`, `1b`, or a
phase-2 family tag such as `2-mmio`). Every executable task that
this batch elaborates inherits this phase tag via
`task.py add-task --phase`.

## Review Tiers

Required table naming which harness-side review tiers apply at batch
close. Task Review and Phase-Close Review are mandatory; Batch
Review is opt-in for coherent multi-task features. For phase-1b
batches whose closure depends on the cumulative
DSL -> IR -> C -> captured-stdout suite running green, add a fourth
row naming that suite gate.

| Tier | Scope | Trigger | Mandatory? |
| ---- | ----- | ------- | ---------- |
| Task Review | Per-task | After task tests pass | Yes |
| Batch Review | Per-batch cross-task | After all batch tasks done | Optional; set when batch is a coherent feature |
| Phase-Close Review | Per-phase | `phase-status <p>` returns PASS | Yes at phase boundary |

For Phase-Close Review semantics see `.harness/SESSION.md` (Review
section) and the disambiguation against README §5.6's spec-side
tiers.

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
