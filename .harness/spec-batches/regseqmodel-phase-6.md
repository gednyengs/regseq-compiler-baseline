# Spec Batch: regseqmodel-phase-6 - Normalization, join, and verification

## Description

This batch authors executable harness task specs for wiring the IR
construction pass into the pipeline: the normalize() function that
produces a RegseqModel from RawFrontendArtifact, the populate_bindings()
function for in-place platform join, and phase legality verification.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-status.yaml`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 6)
- `docs/discussions/models/regseq-model/spec.md` (Sections 11, 12, 13)
- `crates/regseq-model/src/ir/construction.rs` (Phase 5)
- `crates/regseq-model/src/platform_parser.rs`
- `crates/regseq-model/src/declaration/normalize.rs` (old, for reference)
- `crates/regseq-model/src/joined/join.rs` (old, for reference)

## Output Specification

Create:

- `.harness/tasks/regseq-model/` task spec files

Update when done:

- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`

## Implementation Notes

- `normalize()` wraps Pass 1 + Pass 2 from Phase 5.
- `populate_bindings()` is in-place mutation on `&mut RegseqModel`.
  Matches by resource `id` against platform file sections.
- Phase legality verification: post-Phase-1 (all declarations, no
  bindings) and post-Phase-2 (required bindings populated).
- Prove with full PL011 pipeline.

## Non-Goals

- No consumer migration (Phase 7).
- No backend changes.

## Acceptance Criteria

- Full pipeline for PL011: extraction -> normalize -> join -> verify.
- Tracker updated.

## Update Instructions

1. Change `regseqmodel-phase-6` to `tasked`.
2. Fill `generated_task_ids`.
3. Move `regseqmodel-phase-7` to `ready_to_elaborate`.
