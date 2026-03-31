# Spec Batch: phase-b-01 - shared frontend foundation for PL011 compiler capture

## Description

Author the first Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the smallest shared frontend foundation needed before
construct-local PL011 capture can proceed. The output should cover common
support types, structural driver attributes, callback-context baselines, and
shared result or entry types that multiple later slices would otherwise invent
independently.

The resulting task specs should make the first real frontend slices non-arbitrary
without pulling MMIO/register foundations, normalized model work, or lowering
concerns into the same batch.

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
- `docs/compiler-collateral/constructs/register-access.md`
- `docs/compiler-collateral/constructs/serial-uart-management.md`
- `docs/compiler-collateral/constructs/device-interrupt-setup-and-handling.md`
- `docs/compiler-collateral/constructs/peripheral-clock-management.md`
- `docs/compiler-collateral/constructs/synchronization.md`
- `docs/compiler-collateral/constructs/firmware-property-access.md`
- `docs/compiler-collateral/constructs/device-power-state-management.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-facade/`
- `.harness/tasks/regseq-driver/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for the shared
foundation work under `regseq-facade` and `regseq-driver`.

## Implementation Notes

Follow these fixed decisions:

- cover the accepted PL011-core subset only
- keep the batch minimal and shared
- include structural driver attributes such as `#[rseq::driver(...)]` and
  `#[rseq::device_class(...)]`
- include common support types and entry/result types such as `RegseqError`,
  `Duration`, and `IrqResult` where the accepted baseline requires them
- define callback-context baselines such as `F`, `S`, and `H` only to the extent
  needed to avoid conflicting assumptions later
- do not pull MMIO/register foundations into this batch
- do not pull synchronization bundles, normalized model, join, or lowering work
  into this batch

Decompose into narrow tasks. Typical splits should separate:

- shared support and result types
- structural driver-attribute capture
- callback-context baseline capture

## Non-Goals

Do not include:

- `regseq-macros` helper macro work
- MMIO/register surfaces
- synchronization and interrupt shared extensions
- normalized model or join work
- Layer 2 parser or binding work
- lowering or backend logic
- E1000-facing surface work

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under the shared-foundation target groups
- each task is narrow, testable, and limited to shared frontend prerequisites
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-b-01` can be marked `tasked`
- `generated_task_ids` for `phase-b-01` lists the new shared-foundation task IDs

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-01` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
