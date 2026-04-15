# Spec Batch: regseqmodel-phase-5 - MIR-to-Regseq-IR construction pass

## Description

This batch authors executable harness task specs for the two-pass IR
construction that transforms extracted MIR bodies + attribute registries
into populated RegseqModel with CFG-based Regseq IR function bodies.

This is the hardest phase. It includes the declaration skeleton (Pass 1),
body construction (Pass 2), and 7 recognition rules implemented
incrementally against PL011 fixture functions of increasing complexity.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-status.yaml`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 5)
- `docs/discussions/models/regseq-model/ir-reference.md` (Section 8)
- `docs/discussions/models/regseq-model/compiler-knowledge.md`
- `crates/regseq-model/src/ir/` (Phase 0 + 1 types)
- `crates/regseq-model/src/model/` (Phase 2 types)
- `crates/regseq-model/src/declaration/mir_body.rs` (extraction input)
- `tests/fixtures/pl011-full-spec/src/driver.rs`

## Output Specification

Create:

- `.harness/tasks/regseq-model/` task spec files

Update when done:

- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`

The output is executable harness tasks.

## Implementation Notes

- New file: `crates/regseq-model/src/ir/construction.rs`.
- Direct construction from MirBody JSON + registries. No intermediate
  AnnotatedMirBody.
- Pass 1: declaration skeleton (module tree, DeclId assignment,
  identity_enrichment mapping).
- Pass 2: body construction with 7 recognition rules.
- Incremental implementation order (9 test targets):
  1. Straight-line code -- `disable_uart()`
  2. SwitchInt -- `tx_ready()`, `send_break()`
  3. ?-pattern folding -- `enable_clock_if_present()`
  4. Loop detection -- `flush_rx_fifo()`
  5. OrderedRegion -- `program_polling_port()`
  6. Logging metadata -- (add fixture call if needed)
  7. Register metadata -- all register access functions
  8. modify() body -- `disable_fifo()`, `configure_line_control()`
  9. Volatile dead-temp -- `flush_rx_fifo()` read-and-discard

- Recommend splitting into multiple tasks per recognition rule or
  complexity tier.

## Non-Goals

- No normalization pipeline wiring (Phase 6).
- No backend changes.
- No consumer migration.

## Acceptance Criteria

- Task specs created with incremental test targets.
- IR for each PL011 function matches structural expectations.
- Tracker updated.

## Update Instructions

1. Change `regseqmodel-phase-5` status to `tasked`.
2. Fill `generated_task_ids`.
3. Move `regseqmodel-phase-6` to `ready_to_elaborate`.
