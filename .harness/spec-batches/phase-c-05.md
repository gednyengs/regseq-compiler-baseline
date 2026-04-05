# Spec Batch: phase-c-05 - authored body capture prerequisite for behavior lowering

## Description

Author the prerequisite task batch needed to make authored callback/helper body
capture a real compiler fact before full PL011 behavioral lowering continues.

This batch exists because the current lowering path still synthesizes PL011
behavior in compiler source instead of consuming captured authored bodies from
the frontend pipeline.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `crates/regseq-driver/src/extract/`
- `crates/regseq-driver/src/raw_artifact.rs`
- `crates/regseq-model/src/declaration/raw_input.rs`
- `crates/regseq-model/src/declaration/`
- `crates/regseq-analysis/src/`
- `crates/regseq-backends/src/lowering/behavior_planner.rs`
- `docs/discussions/pl011-full-spec/pl011-unified-spec-boundary.md`
- `docs/discussions/linux-codegen/lowering-plan-interface-boundary.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-driver/`
- `.harness/tasks/regseq-model/`
- `.harness/tasks/regseq-analysis/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs plus the
required harness metadata updates.

## Implementation Notes

Follow these fixed decisions:

- Authored callback/helper bodies must flow through the real extraction
  pipeline:
  - `regseq-driver`
  - raw frontend artifact
  - `regseq-model`
  - `regseq-analysis`
- No analysis-local or lowering-local manual reconstruction of PL011 behavior.
- Captured body representations must remain OS-neutral and device-neutral.
- This batch is upstream of behavioral lowering. It should not emit Linux code
  or choose Linux-specific mechanisms.

## Non-Goals

Do not include:

- Linux backend emission
- guest/QEMU smoke validation
- device-specific behavioral templates in analysis or model code
- reopening unrelated Phase B/C work outside authored body capture

## Acceptance Criteria

This batch is complete when:

- executable harness task specs exist for:
  - frontend authored-body extraction
  - authored-body model schema/normalization
  - analysis exposure and supported-subset validation
- `tasks.json` references the new task IDs under the appropriate groups with
  correct dependency order
- the new task batch is recorded in `.harness/task-spec-status.yaml`
- Phase D behavioral lowering can depend on the new analysis task instead of
  synthesizing PL011 callback logic
