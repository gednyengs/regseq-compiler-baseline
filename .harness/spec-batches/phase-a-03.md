# Spec Batch: phase-a-03 - Phase B and C tracker realignment for the frontend-first plan

## Description

Author the third Phase A harness batch for the ASPLOS implementation stream.
This batch is tracker and brief maintenance work, not executable implementation
work.

The goal is to realign the downstream Phase B and Phase C task-spec tracker
entries with the revised frontend-first plan. The current tracker still carries
old batch boundaries and source references that were written before the plan
was rewritten around frontend closure first, then internal model and join.

This batch should leave the harness in a coherent pre-Phase-B state by:

- rewriting the Phase B and Phase C tracker items so their summaries,
  dependencies, source references, and authoring notes match the revised plan
- creating the missing downstream brief files needed for those corrected
  batches
- ensuring that only the true next downstream batch is left
  `ready_to_elaborate`

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`
- `.harness/spec-batches/BRIEF_TEMPLATE.md`
- `.harness/spec-batches/phase-a-01.md`
- `.harness/spec-batches/phase-a-02.md`
- `docs/plans/asplos-implementation-plan.md`
- `docs/pl011-compiler-collateral/compiler-baseline-overview.md`
- `docs/pl011-compiler-collateral/compiler-constraints.md`

## Output Specification

Create or modify:

- `.harness/task-spec-status.yaml`
- `.harness/spec-batches/phase-b-01.md`
- `.harness/spec-batches/phase-b-02.md`
- `.harness/spec-batches/phase-b-03.md`
- `.harness/spec-batches/phase-b-04.md`
- `.harness/spec-batches/phase-b-05.md`
- `.harness/spec-batches/phase-b-06.md`
- `.harness/spec-batches/phase-b-07.md`
- `.harness/spec-batches/phase-c-01.md`
- `.harness/spec-batches/phase-c-02.md`
- `.harness/spec-batches/phase-c-03.md`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be harness metadata and brief files only.
This batch must not create executable harness tasks and must not modify
`.harness/tasks.json`.

## Implementation Notes

This batch exists to make downstream task authoring coherent with the revised
plan before any new Phase B task-spec elaboration begins.

Follow these fixed decisions:

- Phase B is the frontend-closure phase. Its batches must center on:
  - accepted Layer 1 DSL surface work
  - helper macros
  - inert `#[rseq::...]` tool-attribute recognition in the compiler path
  - Layer 2 format/parser and validation
  - raw HIR/MIR extraction
  - raw extraction artifact emission
  - frontend-oriented validation and fixture coverage
- Phase C is the internal-model-and-join phase. Its batches must center on:
  - normalized declaration model
  - normalized binding model
  - Layer 1 plus Layer 2 join
  - analysis/bootstrap validation over normalized models
  - joined artifact stability for lowering
- Use the accepted PL011 compiler collateral package as the primary
  task-authoring input for PL011-facing batches.
- Preserve the stable harness groups created in Phase A-02. This batch changes
  tracker intent and brief context, not group layout.
- Keep extraction work in `regseq-driver`, not `regseq-model`.
- Keep PL011-first sequencing explicit. E1000-facing batches must remain
  downstream of PL011 frontend and model closure.
- Make the raw-artifact boundary explicit in Phase B briefs and the joined
  artifact boundary explicit in Phase C briefs.
- If the current tracker combines frontend parsing/extraction with normalized
  model or join work, split or rewrite those items so the batch boundary matches
  the revised plan.
- Use the current phase structure from the plan:
  - Phase B: Frontend Closure
  - Phase C: Internal Model and Join

## Non-Goals

Do not include:

- executable harness tasks under `.harness/tasks/`
- changes to `.harness/tasks.json`
- crate or source-code implementation work
- PL011 or E1000 code/spec implementation
- lowering or backend task authoring beyond tracker/brief rewriting
- reopening accepted compiler-collateral design decisions

## Acceptance Criteria

This batch is complete when:

- `phase-a-03` is represented by this brief and the brief is sufficient for a
  cold session to complete the tracker realignment
- all Phase B and Phase C tracker items in `.harness/task-spec-status.yaml`
  have summaries, dependencies, source references, and spec-authoring notes
  that match the revised frontend-first plan
- the missing brief files for Phase B and Phase C batches exist under
  `.harness/spec-batches/`
- no downstream Phase B or Phase C item is left `ready_to_elaborate` unless it
  is the first corrected next batch
- `phase-a-03` can be marked `tasked` with `generated_task_ids: []`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-a-03` to `tasked`
   - leave `generated_task_ids` as `[]`
2. Update the Phase B and Phase C tracker items to reflect the revised
   frontend-first sequencing and corrected source references.
3. Move only the first corrected dependency-satisfied downstream item to
   `ready_to_elaborate`.
4. Do not leave multiple downstream task-spec batches marked
   `ready_to_elaborate`.
5. Do not modify unrelated tracker items.
