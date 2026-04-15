# Spec Batch: regseqmodel-phase-3d - PL011 fixture comprehensive update

## Description

This batch authors executable harness task specs for converting the
PL011 fixture to the new register architecture: register_block! macro,
Mmio<pl011::Regs>, all register access via Mmio methods, scoped module
imports, and pin_state attributes on marker types.

This is the fixture-facing proof that Phase 3a/3b/3c facade and macro
changes compose correctly in a realistic driver.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-status.yaml`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 3.7)
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `tests/fixtures/pl011-full-spec/src/registers.rs`
- `tests/fixtures/pl011-full-spec/src/lib.rs`
- `crates/regseq/src/mmio.rs` (post phase-3b)
- `crates/regseq/src/registers.rs` (post phase-3b)

## Output Specification

Create:

- `.harness/tasks/track-a-pl011/` task spec files

Update when done:

- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`

The output is executable harness tasks.

## Implementation Notes

- Convert `registers.rs` from `register_bitfields!` + `#[repr(C)]` struct
  to single `register_block!` call.
- Convert all `driver.rs` register access from wrapper methods to Mmio
  methods (see execution plan Phase 3.7 for the transformation patterns).
- Add `#[rseq::pin_state(id = "default")]` and `#[rseq::pin_state(id = "sleep")]`.
- Update imports to scoped module paths.
- Fixture must compile against updated facade and macros crates.

## Non-Goals

- No extraction pipeline changes.
- No golden file regeneration (that is Phase 4).
- No new fixture files.

## Acceptance Criteria

- Task specs created. PL011 fixture compiles with new register architecture.
- Tracker updated.

## Update Instructions

1. Change `regseqmodel-phase-3d` status to `tasked`.
2. Fill `generated_task_ids`.
3. Check if `regseqmodel-phase-4` can move to `ready_to_elaborate`
   (requires 3a, 3b, 3c, 3d all `tasked`).
