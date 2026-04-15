# Spec Batch: regseqmodel-phase-7 - Consumer migration to RegseqModel

## Description

This batch authors executable harness task specs for migrating all
consumers from the old model types (DeclarationModel, BindingModel,
JoinedArtifact, SemanticModel) to the new unified RegseqModel. Uses a
temporary adapter layer for incremental migration.

Migration order: analysis -> lowering planner -> Linux backend ->
pipeline glue (emit_driver.rs) -> E2E tests.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-status.yaml`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 7)
- `crates/regseq-analysis/src/input.rs`
- `crates/regseq-backends/src/lowering/planner.rs`
- `crates/regseq-backends/src/linux/planner.rs`
- `crates/regseq-backends/src/linux/emitter.rs`
- `crates/regseq-driver/src/emit_driver.rs`
- `tests/e2e/tests/`

## Output Specification

Create:

- `.harness/tasks/regseq-analysis/` task specs
- `.harness/tasks/regseq-lowering/` task specs
- `.harness/tasks/regseq-driver/` task specs

Update when done:

- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`

## Implementation Notes

- Temporary `compat.rs` adapter bridges old consumer APIs to new model.
- Old modules stay alive during migration.
- Each consumer migrated as a separate task.
- All tests must pass after each migration step.

## Non-Goals

- No old module deletion (Phase 9).
- No new C emission logic (Phase 8).

## Acceptance Criteria

- All consumers migrated. Tests pass. Tracker updated.

## Update Instructions

1. Change `regseqmodel-phase-7` to `tasked`.
2. Fill `generated_task_ids`.
3. Move `regseqmodel-phase-8` to `ready_to_elaborate`.
