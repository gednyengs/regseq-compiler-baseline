# Spec Batch: regseqmodel-phase-3c - ordered! macro migration and compiler config

## Description

This batch authors executable harness task specs for two small but
prerequisite changes: migrating the ordered! macro from memory_barrier()
intrinsic expansion to ordered_region_begin()/ordered_region_end() marker
calls, and updating the compiler config to set release-profile MIR
semantics.

Both changes are prerequisites for Phase 5 (IR construction): OrderedRegion
detection expects the new markers, and the IR spec requires release-profile
MIR without overflow checks or debug assertions.

The concrete outcome is a set of executable harness tasks (likely 2-3 small
tasks).

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 3.5, 3.8)
- `docs/discussions/models/regseq-model/spec.md` (Section 9, Ordered Flat Regions)
- `docs/discussions/models/regseq-model/compiler-knowledge.md` (Section 2, OrderedRegion)
- `docs/discussions/models/regseq-model/ir-reference.md` (Section 6, OrderedRegion)
- `crates/regseq-macros/src/ordered.rs`
- `crates/regseq/src/intrinsics/sync.rs`
- `crates/regseq-driver/src/callbacks.rs`

## Output Specification

Create:

- `.harness/tasks/regseq-macros/` task spec for ordered! migration
- `.harness/tasks/regseq-facade/` task spec for internal marker functions
- `.harness/tasks/regseq-driver/` task spec for compiler config update

Update when done:

- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`

The output is executable harness tasks.

## Implementation Notes

- **ordered! macro migration** (crates/regseq-macros/src/ordered.rs):
  - Current: expands to `memory_barrier(BarrierKind::Full)` calls bracketing the block
  - Target: expands to `regseq::internal::ordered_region_begin()` / `ordered_region_end()` calls
  - The marker functions are compiler markers, not real intrinsics -- they
    exist only so the IR constructor can detect ordered region boundaries

- **Internal marker functions** (new: crates/regseq/src/internal.rs or similar):
  - `pub fn ordered_region_begin()` -- stub, recognized by compiler by DefId
  - `pub fn ordered_region_end()` -- stub, recognized by compiler by DefId
  - These are NOT #[rseq::intrinsic] -- they are internal compiler hints
    in the `regseq::internal` namespace

- **Compiler config** (crates/regseq-driver/src/callbacks.rs):
  - Add `overflow_checks = Some(false)` and `debug_assertions = Some(false)`
  - Alongside existing `mir_opt_level = Some(0)` and `always_encode_mir = true`
  - This ensures MIR uses `Add`/`Sub`/`Mul` (not `*WithOverflow`) and no
    `Assert` terminators appear

- **Task decomposition guidance**:
  1. Internal marker functions + ordered! macro update (facade + macros, tightly coupled)
  2. Compiler config for release-profile MIR (driver, independent)

## Non-Goals

- No OrderedRegion detection logic (Phase 5).
- No changes to PL011 fixture (the fixture already uses ordered! -- the macro
  expansion changes transparently).
- No removal of memory_barrier() function (keep it for possible future use).

## Acceptance Criteria

- All task spec files created.
- Each task spec includes: implementation scope, non-goals, test plan, quality gate.
- Tasks registered in `.harness/tasks.json`.
- `generated_task_ids` recorded in tracker.
- Tracker item updated.

## Update Instructions

1. Change `regseqmodel-phase-3c` status from `ready_to_elaborate` to `tasked`.
2. Fill `generated_task_ids` with the new task IDs.
3. No downstream items to advance directly (phase-4 depends on all of 3a-3d).
