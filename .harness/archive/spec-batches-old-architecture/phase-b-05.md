# Spec Batch: phase-b-05 - core typed-resource wrappers and standalone facade intrinsics

## Description

Author the fifth Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the remaining shared typed-resource wrappers beyond
MMIO, such as `Clock`, `ResetLine`, and `PinCtrl`, along with any standalone
facade intrinsics and lightweight facade macro surfaces such as logging and
timed-delay APIs that the accepted full PL011 frontend slice needs before real
frontend assembly becomes realistic. For diagnostic logging specifically,
prefer a Regseq-owned logging intent shape over Rust formatting carriers.

The output should keep this work explicitly on the Layer 1 frontend side. These
wrappers should become available before real PL011 assembly and before Layer 2
binding work, but they should not reopen MMIO/register foundations.

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
- `docs/compiler-collateral/constructs/peripheral-clock-management.md`
- `docs/compiler-collateral/constructs/hardware-reset-sequencing-and-post-reset-polling.md`
- `docs/compiler-collateral/constructs/pin-controller-and-gpio-configuration.md`
- `docs/compiler-collateral/constructs/device-interrupt-setup-and-handling.md`
- `docs/compiler-collateral/constructs/synchronization.md`
- `docs/compiler-collateral/constructs/diagnostic-logging.md`
- `docs/compiler-collateral/constructs/timed-cpu-delay-and-sleep.md`
- `archive/guide/implementation-guide.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-facade/`
- `.harness/tasks/regseq-driver/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for the shared
typed-resource wrapper, standalone-intrinsic, and facade macro extension across
the appropriate groups.

## Implementation Notes

Follow these fixed decisions:

- keep this batch to typed resource carriers and standalone facade intrinsics
  that are not structurally tied to MMIO/registers, synchronization, or
  callback traits
- do not reopen MMIO/register foundations from `phase-b-02`
- add only the wrappers, lightweight facade macro surfaces, and standalone
  intrinsics that are
  broadly reused by the accepted full PL011 slice
- keep this work separate from the later Layer 2 parser/binding contract
- keep the supported subset bounded to the PL011 frontend slice

If compiler-facing capture tasks are needed, keep them focused on the immediate
frontend shape of these wrappers and intrinsics rather than on normalized
models.

## Non-Goals

Do not include:

- Layer 2 parser or binding work
- normalized model or join work
- backend or lowering work
- E1000-facing wrapper expansion

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under the typed-resource wrapper target groups
- shared wrappers beyond MMIO and any accepted standalone facade intrinsics are explicitly represented
- the task specs keep this work separate from MMIO/register foundations and from Layer 2 capture
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-b-05` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-05` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
