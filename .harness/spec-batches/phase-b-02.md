# Spec Batch: phase-b-02 - regseq-macros register_bitfields and helper macro surface

## Description

Author the second Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the `regseq-macros` tasks needed to support the
accepted PL011 frontend slice, centered on `register_bitfields!` and any other
accepted non-attribute helper macros required by that slice.

The output should keep macro ownership boundaries explicit: helper macros live
here, while inert `#[rseq::...]` tool attributes remain compiler-registered and
must not be assigned to `regseq-macros`.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`
- `docs/plans/asplos-implementation-plan.md`
- `docs/compiler-collateral/compiler-baseline-overview.md`
- `docs/compiler-collateral/constructs/register-access.md`
- `docs/compiler-collateral/constructs/serial-uart-management.md`
- `docs/compiler-collateral/constructs/device-interrupt-setup-and-handling.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-macros/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for the
`regseq-macros` group.

## Implementation Notes

Follow these fixed decisions:

- `register_bitfields!` must be explicit and visible in the task set
- keep helper macros separate from inert attribute ownership
- keep the macro surface bounded to the accepted PL011 frontend slice
- macro tasks should focus on authoring support, compile behavior, and tests
- do not mix extractor work or backend behavior into macro tasks

Decomposition guidance:

- one task for `register_bitfields!` itself
- separate helper-macro tasks only if they are accepted by the collateral and
  materially distinct

## Non-Goals

Do not include:

- ownership of `#[rseq::...]` inert tool attributes
- `regseq` facade wrapper or trait work
- intrinsic stub work
- extractor or driver tasks
- normalized model or join work
- E1000-specific macros

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under `.harness/tasks/regseq-macros/`
- `register_bitfields!` is explicitly represented in the authored tasks
- the tasks keep macro scope separate from inert attribute handling
- `tasks.json` references the new task IDs under `regseq-macros`
- `phase-b-02` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-02` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
