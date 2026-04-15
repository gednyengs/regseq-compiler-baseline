# Spec Batch: phase-b-07 - Layer 2 binding format and property capture

## Description

Author the seventh Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the Layer 2 binding contract for the accepted PL011
frontend slice. The output should cover the file shape, parser, validation path,
and the PL011-relevant property capture needed before real join logic begins in
Phase C.

The output should treat firmware-property capture as a mixed Layer 1 / Layer 2
frontier while keeping the final normalized binding model and join work out of
scope.

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
- `archive/guide/pl011-spec-outline.md`
- `archive/guide/open-issues.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-model/`
- `.harness/tasks/track-a-validation/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for the Layer 2
binding contract across the appropriate groups.

## Implementation Notes

Follow these fixed decisions:

- keep Layer 2 capture separate from the later normalized binding model and
  Layer 1 + Layer 2 join logic in Phase C
- keep the supported subset bounded to the accepted PL011 frontend slice
- include the limited boundary compatibility checks required to prove the input
  contract, but do not define the final join here
- treat firmware-property capture as a mixed Layer 1 / Layer 2 frontier
- use validation tasks only where they prove parser behavior or the accepted
  input contract

## Non-Goals

Do not include:

- raw-artifact closure work
- real PL011 slice assembly
- normalized model or join work
- backend lowering
- E1000 expansion

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under the Layer 2 target groups
- the Layer 2 file shape, parser, validation path, and property capture are explicitly represented
- the task specs keep this work separate from normalized binding-model and join work
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-b-07` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-07` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
