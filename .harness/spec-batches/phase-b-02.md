# Spec Batch: phase-b-02 - register and MMIO shared extension

## Description

Author the second Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the shared Register Access foundation needed by many
later frontend slices. The output should cover `Mmio<T>`, register-declaration
support types, the backing model for register definitions, helper macros such as
`register_bitfields!`, and the register-specific intrinsic surface required by
that foundation.

The output should keep ownership boundaries explicit: helper macros live in
`regseq-macros`, wrapper and support types live in `regseq`, and inert
`#[rseq::...]` tool attributes remain compiler-registered and must not be
assigned to `regseq-macros`.

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
- `.harness/tasks/regseq-facade/`
- `.harness/tasks/regseq-driver/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for the shared
register/MMIO extension across the appropriate groups.

## Implementation Notes

Follow these fixed decisions:

- treat register and MMIO infrastructure as a shared extension, not as a purely
  local construct slice
- `Mmio<T>` and the register-declaration model must exist before later wrapper
  and spec-slice work can be realistic
- `register_bitfields!` must be explicit and visible in the task set
- keep helper macros separate from inert attribute ownership
- keep the surface bounded to the accepted PL011 frontend slice
- include only register-specific intrinsic support that is structurally tied to
  this foundation
- do not mix normalized model or backend behavior into these tasks

Decomposition guidance:

- facade tasks for `Mmio<T>` and the register support types
- a macro task for `register_bitfields!`
- separate helper-macro tasks only if they are accepted and materially distinct
- driver-side capture tasks only where the register/MMIO foundation requires
  compiler-facing shape checks

## Non-Goals

Do not include:

- ownership of unrelated `#[rseq::...]` inert tool attributes
- callback-family trait work
- unrelated synchronization or interrupt work
- normalized model or join work
- E1000-specific macros

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under the shared register/MMIO target groups
- `Mmio<T>` and the register-declaration backing model are explicitly represented
- `register_bitfields!` is explicitly represented in the authored tasks
- the tasks keep macro scope separate from inert attribute handling
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-b-02` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-02` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
