# Spec Batch: phase-c-03 - analysis bootstrap, joined-artifact validation, and lowering-input stability

## Description

Author the third Phase C batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the minimum analysis/bootstrap and validation tasks
needed to confirm that the normalized PL011 slice is complete, reviewable, and
stable enough to act as the lowering-input contract for Phase D.

The output should make this the last model-side gate before lowering-oriented
task authoring begins.

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
- `docs/guide/open-issues.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-analysis/`
- `.harness/tasks/track-a-validation/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for
`regseq-analysis` and any required joined-artifact validation tasks.

## Implementation Notes

Follow these fixed decisions:

- this is the last Phase C gate before lowering-oriented task authoring
- focus on joined-artifact completeness, consistency, and stability
- include stronger QC for caution constructs present in the supported PL011 slice
- keep the output boundary as a stable lowering-input contract
- do not drift into backend realization

## Non-Goals

Do not include:

- frontend facade or extraction work
- normalized model construction already covered by earlier Phase C batches
- backend lowering or backend validation
- E1000 work

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under `.harness/tasks/regseq-analysis/`
  and/or `.harness/tasks/track-a-validation/`
- the task specs explicitly target joined-artifact validation and lowering-input stability
- caution-focused checks are carried where the supported slice requires them
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-c-03` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-c-03` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
