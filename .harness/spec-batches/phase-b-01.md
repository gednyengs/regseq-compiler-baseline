# Spec Batch: phase-b-01 - regseq facade PL011-core resource wrappers and callback traits

## Description

Author the first Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the first `regseq` facade tasks needed for frontend
closure of the accepted PL011 slice. The output should cover typed resource
wrappers, marker types, and callback-family traits for the PL011-core subset,
using the accepted PL011 compiler collateral as the primary operational input.

The resulting task specs should make the Layer 1 facade authorable without
pulling extraction, normalized model, or lowering concerns into the same batch.

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
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for the
`regseq-facade` group.

## Implementation Notes

This batch should stay facade-only.

Follow these fixed decisions:

- cover the accepted PL011-core subset only
- focus on typed resource wrappers, marker types, and accepted callback-family
  traits
- use accepted callback-family names from the compiler collateral
- keep wrapper APIs aligned with accepted wrapper-family conventions
- keep synchronization type-driven
- keep Driver Lifecycle as the declaration site for `driver_name` and
  `compatible`
- do not pull inert `#[rseq::...]` extraction into this batch
- do not pull normalized model, join, or lowering work into this batch

Decompose into narrow `regseq-facade` tasks. Typical splits should separate:

- resource wrapper and marker-type surfaces
- callback-family traits
- property-carrying wrapper surfaces where needed

## Non-Goals

Do not include:

- `regseq-macros` helper macro work
- intrinsic stub work
- extractor or compiler-driver work
- normalized model or join work
- Layer 2 parser or binding work
- lowering or backend logic
- E1000-facing surface work

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under `.harness/tasks/regseq-facade/`
- each task is narrow, testable, and facade-only
- `tasks.json` references the new task IDs under `regseq-facade`
- `phase-b-01` can be marked `tasked`
- `generated_task_ids` for `phase-b-01` lists the new facade task IDs

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-01` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
