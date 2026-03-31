# Spec Batch: phase-b-04 - core trait callback surfaces

## Description

Author the fourth Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the accepted callback-family contracts used by the
early PL011 frontend slice. The output should cover `Lifecycle`, `Serial`,
optional `SerialConsole`, and `PowerManagement` only where the accepted PL011
baseline requires it.

The output should make callback contracts authorable and analyzable on top of
the earlier shared foundations, without yet moving into the full real-PL011
assembly batch.

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
- `docs/compiler-collateral/constructs/driver-lifecycle.md`
- `docs/compiler-collateral/constructs/serial-uart-management.md`
- `docs/compiler-collateral/constructs/device-power-state-management.md`
- `archive/guide/pl011-spec-outline.md`
- `archive/guide/device-coverage.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-facade/`
- `.harness/tasks/regseq-driver/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for the core
trait callback surfaces across the appropriate groups.

## Implementation Notes

Follow these fixed decisions:

- treat callback traits as shared frontend contracts that build on the earlier
  shared foundations
- use only accepted callback-family names and signatures from the compiler
  collateral
- keep structural driver attributes in their earlier batch
- keep method bodies and tests frontend-oriented, with enough reachability to
  support later extraction tasks
- include `PowerManagement` only if it is part of the accepted early PL011 slice

Task decomposition should separate:

- lifecycle surfaces
- serial callback surfaces
- optional console or power-management surfaces only where accepted

## Non-Goals

Do not include:

- MMIO/register foundations already covered by `phase-b-02`
- synchronization and interrupt shared extensions already covered by `phase-b-03`
- real PL011 slice assembly
- normalized model, join, lowering, or E1000 work

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under the callback-surface target groups
- accepted callback-family traits are explicitly represented with stable signatures
- the task specs keep callback contracts separate from later real-PL011 assembly work
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-b-04` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-04` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
