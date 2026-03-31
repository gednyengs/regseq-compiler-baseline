# Spec Batch: phase-b-06 - declarative compiler-managed frontend surfaces

## Description

Author the sixth Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the declarative compiler-managed frontend surfaces used
by the accepted PL011 slice. The output should cover accepted property and
power-management metadata, along with any closely related compiler-facing
surfaces that carry intent without introducing new callback or wrapper APIs.

The output should make these declarations authorable and capturable without
drifting into Layer 2 parsing, normalized models, or final join logic.

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
- `docs/compiler-collateral/constructs/`
- `archive/guide/open-issues.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-facade/`
- `.harness/tasks/regseq-driver/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for the
declarative compiler-managed surfaces across the appropriate groups.

## Implementation Notes

Follow these fixed decisions:

- keep this batch focused on declarative metadata whose meaning is primarily
  compiler-managed
- include accepted property and power-management declaration forms only where
  they are part of the supported PL011 slice
- reuse earlier shared foundations and wrapper carriers where these declarations
  attach to items or fields
- do not introduce Layer 2 parser work, normalized models, or join logic here

Decompose into narrow tasks such as:

- property declaration surfaces
- power-management metadata surfaces
- driver-side capture tasks only where needed to make these declarations
  visible to later analysis/model work

## Non-Goals

Do not include:

- Layer 2 parser and property-binding work
- normalized model and join work
- raw-artifact closure work
- backend lowering
- E1000 expansion

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under the declarative-surface target groups
- property and power-management metadata surfaces are explicitly represented
- the task specs keep this work declarative and compiler-managed
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-b-06` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-06` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
