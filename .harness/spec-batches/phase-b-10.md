# Spec Batch: phase-b-10 - Serial device-class v1 alignment

## Description

Author a focused reopened Phase B batch that aligns the compiler-facing serial
device-class surface with the authoritative v1 Serial device-class spec.

This is task-spec authoring work, not executable implementation work.

The purpose of this batch is to prevent Phase C from normalizing an outdated
serial contract. The earlier Phase B serial tasks established the first
accepted baseline, but the newer Serial device-class refinement now supersedes
that baseline where they differ. This batch should update the facade,
extraction, validation, and PL011 fixture-conformance work so the compiler can
resume from the v1 Serial authority rather than the older six-method surface.

The output should be executable harness task specs for a small serial-alignment
batch. Keep the batch narrow: it should align the stable v1 serial contract,
not reopen serial taxonomy, add RS-485, or widen the current PL011 scope.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`
- `docs/discussions/device-class-serial/serial-device-class-spec.md`
- `docs/discussions/device-class-serial/serial-os-support-matrix.md`
- `.harness/tasks/regseq-facade/10-serial-trait-surfaces.md`
- `.harness/tasks/regseq-driver/10-lifecycle-and-serial-trait-capture.md`
- `crates/regseq/src/serial.rs`
- `crates/regseq-driver/src/extract/traits.rs`
- `tests/fixtures/pl011-full-spec/src/driver.rs`

## Output Specification

Create or modify:

- `.harness/spec-batches/phase-b-10.md`
- `.harness/tasks/regseq-facade/`
- `.harness/tasks/regseq-driver/`
- `.harness/tasks/track-a-validation/`
- `.harness/tasks/track-a-pl011/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs plus the
required harness metadata updates.

## Implementation Notes

Follow these fixed decisions:

- `docs/discussions/device-class-serial/serial-device-class-spec.md` is the
  current authority for compiler-facing serial work.
- The earlier Phase B serial task specs remain useful historical baseline, but
  this batch supersedes them where the v1 Serial device-class spec differs.
- The stable v1 core includes:
  - `tx_empty`
  - `flush_rx_fifo`
  - `flush_tx_fifo`
  - `send_break`
- `SerialConsole` remains a separate optional companion trait and its stable v1
  signature is `console_write(&mut self, bytes: &[u8])`.
- Flow control, modem/control, RS-485, and DMA do not become new stable v1
  serial traits in this batch.
- The compiler owns TX/RX loop generation around byte primitives; do not invent
  a buffered companion trait.
- Keep the alignment bounded to the serial device-class contract and its direct
  compiler consequences.

Typical decomposition should separate:

- public facade alignment in `regseq`
- trait extraction and validation alignment in `regseq-driver`
- regression/contract validation updates
- PL011 fixture conformance to the revised trait surface

## Non-Goals

Do not include:

- normalized declaration-model work
- Layer 2 parser or join work
- RS-485 design or implementation
- a new `SerialFlowControl` trait
- a new `SerialModemControl` trait
- serial-specific DMA surface design
- broader PL011 feature expansion beyond conformance to the revised serial
  contract

## Acceptance Criteria

This batch is complete when:

- executable harness task specs exist for the serial v1 alignment work
- the specs clearly treat the device-class serial spec as authoritative
- the task split covers facade, extraction, validation, and PL011 conformance
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-b-10` is marked `tasked`
- `phase-c-01` no longer appears ready before the serial-alignment batch is
  completed

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - add `phase-b-10` as `tasked`
   - populate `generated_task_ids` with the new task IDs
   - change `phase-c-01` so it depends on `phase-b-10`
2. Update `.harness/tasks.json` with the new tasks.
3. Do not modify unrelated tracker items.
