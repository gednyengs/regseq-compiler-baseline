# Spec Batch: regseqmodel-phase-8 - Backend C emission from CFG-based IR

## Description

This batch authors executable harness task specs for C code emission
from the new CFG-based IR: goto-based C emitter, dominator-based
resource cleanup for lifecycle callback error paths, and removal of
deprecated backend code (mir_interpreter/, semantic_lowering/).

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-status.yaml`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 8)
- `docs/discussions/models/regseq-model/compiler-knowledge.md` (Section 2, cleanup)
- `crates/regseq-backends/src/linux/emit_semantic.rs`
- `crates/regseq-backends/src/linux/emitter.rs`
- `crates/regseq-backends/src/linux/c_ast.rs`
- `crates/regseq-backends/src/lowering/mir_interpreter/`
- `crates/regseq-backends/src/lowering/semantic_lowering/`

## Output Specification

Create:

- `.harness/tasks/regseq-linux-backend/` task specs
- `.harness/tasks/regseq-lowering/` task specs

Update when done:

- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`

## Implementation Notes

- Goto-based emission first: each BasicBlock as labeled C block.
  Correct before readable.
- Dominator-based cleanup: sequential ? chain, match on result, early
  return, infallible ops in chain, inside ordered region, non-reversible.
- PL011 init() limitations acknowledged: helper-contained optional
  acquisitions emit diagnostic, not incorrect cleanup.
- Remove mir_interpreter/ (6 files), semantic_lowering/ (10 files),
  behavior.rs, behavior_planner.rs, body_interpreter.rs.

## Non-Goals

- No structured control flow recovery (follow-up).
- No Zephyr backend.
- No conditional cleanup flags (Cases 2, 3, 5).

## Acceptance Criteria

- End-to-end: PL011 -> C emission -> compare generated output.
- Deprecated code removed. Tracker updated.

## Update Instructions

1. Change `regseqmodel-phase-8` to `tasked`.
2. Fill `generated_task_ids`.
3. Move `regseqmodel-phase-9` to `ready_to_elaborate`.
