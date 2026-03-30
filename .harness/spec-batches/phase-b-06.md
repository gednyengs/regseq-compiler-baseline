# Spec Batch: phase-b-06 - regseq-driver attribute recognition, raw extraction, and raw-artifact serialization

## Description

Author the sixth Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the `regseq-driver` extraction tasks needed to close
frontend capture for the accepted PL011 slice. The tasks should cover inert
attribute recognition, raw HIR/MIR extraction, operation-class preservation,
and serialization of the raw extraction artifact.

The output boundary for this batch is the raw artifact. It must not drift into
normalized model or join work.

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
- `docs/pl011-compiler-collateral/constructs/`
- `docs/guide/open-issues.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-driver/`
- `.harness/tasks/track-a-validation/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs, primarily for
`regseq-driver`, with validation tasks only where needed to prove raw-artifact
capture quality.

## Implementation Notes

Follow these fixed decisions:

- extraction stays in `regseq-driver`
- preserve operation classes in the raw artifact
- preserve type-driven choices and generic/resource shape where the collateral
  requires them
- preserve declaration order where later lifecycle behavior depends on it
- preserve intrinsic call-site capture and source traceability
- key the raw artifact by construct identity
- do not introduce normalized model or join work in this batch

Decompose into narrow extraction tasks such as:

- inert attribute recognition
- construct-family extraction
- raw-artifact serialization/debug output
- extraction-level validation and snapshots

## Non-Goals

Do not include:

- normalized declaration-model tasks
- normalized binding-model tasks
- Layer 1 + Layer 2 join logic
- backend lowering
- E1000 extraction work

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under `.harness/tasks/regseq-driver/`
- any supporting validation tasks are raw-artifact oriented
- the task specs make the raw-artifact boundary explicit
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-b-06` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-06` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
