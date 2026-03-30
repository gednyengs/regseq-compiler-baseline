# Spec Batch: phase-b-05 - regseq-model Layer 2 binding format, parser, and validation

## Description

Author the fifth Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the frontend-side Layer 2 tasks needed for the
accepted PL011 slice: file format, parser, and validation support for the
supported binding inputs.

The output should keep this work explicitly pre-normalization. The batch is
about the accepted binding input contract and its immediate validation path, not
the final normalized binding model used later in Phase C.

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
- `docs/guide/implementation-guide.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-model/`
- `.harness/tasks/track-a-validation/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs, primarily for
`regseq-model`, with validation tasks only where they are necessary to prove the
accepted binding contract.

## Implementation Notes

Follow these fixed decisions:

- keep Layer 2 format/parser work separate from extraction logic
- keep Layer 2 format/parser work separate from the later normalized binding
  model
- include the minimal boundary-compatibility checks required by the plan
- do not define the final join in this batch
- keep the supported subset bounded to the PL011 frontend slice

If validation tasks are needed, keep them focused on parser behavior and the
accepted Layer 2 boundary contract.

## Non-Goals

Do not include:

- raw extraction tasks
- normalized declaration-model work
- normalized binding-model work
- real join logic
- lowering or backend work
- E1000 binding expansion

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under `.harness/tasks/regseq-model/`
- any supporting validation tasks are narrowly scoped to the Layer 2 boundary
- the task specs keep parser/validation work separate from normalized model/join work
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-b-05` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-05` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
