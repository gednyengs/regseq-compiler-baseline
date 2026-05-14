# Spec Batch: regseqmodel-phase-1 - CFG-based IR types

## Description

This batch authors executable harness task specs for the CFG-based IR
type hierarchy that replaces the old expression-tree semantic IR. These
types build on Phase 0 foundation types and provide the representation
for function bodies in the RegseqModel.

The batch covers Place/Projection, Operand/LiteralValue, Rvalue with
supporting types, RegseqIntrinsic and IntrinsicMetadata, Statement and
Terminator kinds, and the Body/BasicBlock/LocalDecl composite.

The concrete outcome is a set of executable harness tasks that deliver
all IR types needed for Phase 5 (IR construction pass).

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 1)
- `docs/discussions/models/regseq-model/ir-reference.md` (Sections 1-7)
- `docs/discussions/models/regseq-model/spec.md` (Sections 9, 10)
- `crates/regseq-model/src/ir/` (Phase 0 types already in place)

## Output Specification

Create:

- `.harness/tasks/regseq-model/` task spec files for each task

Update when done:

- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`

The output is executable harness tasks.

## Implementation Notes

- All types live under `crates/regseq-model/src/ir/`.
- Types depend on Phase 0 types: `Symbol`, `LocalId`, `BasicBlockId`,
  `ConstantId`, `FunctionId`, `TypeDefId`, `RegisterBlockId`, `RegseqType`.
- Existing `declaration/mir_body.rs` types remain as the extraction
  serialization format. New `ir/` types are the processed domain-enriched
  output. Both coexist.
- Serde: adjacently-tagged enums, Symbol as string, Option fields omitted
  when None.
- `IntrinsicCall` terminator carries `metadata: Option<IntrinsicMetadata>`
  and `error_target: Option<BasicBlockId>`.
- `Body` owns `BodySourceMap` (structurally parallel to basic_blocks and
  local_decls).
- `Body::predecessors()` is a computed method, not stored state.

- **Task decomposition guidance**: Split by file. Each file is independently
  testable. Suggested:
  1. Place + Projection (ir/place.rs)
  2. Operand + LiteralValue (ir/operand.rs)
  3. Rvalue + BorrowKind + CastKind + AggregateKind (ir/rvalue.rs)
  4. RegseqIntrinsic + RegseqAbstraction + IntrinsicMetadata (ir/intrinsic.rs)
  5. StatementKind + MirIntrinsic (ir/statement.rs)
  6. TerminatorKind + SwitchTargets (ir/terminator.rs)
  7. Body + BasicBlock + LocalDecl + OrderedRegion + BodySourceMap (ir/body.rs)

## Non-Goals

- No model structure types (Module, Function, TypeDef) -- Phase 2.
- No IR construction logic -- Phase 5.
- No changes to existing regseq-model modules.

## Acceptance Criteria

- All task spec files created under `.harness/tasks/regseq-model/`.
- Each task spec includes: scope, non-goals, test plan (construction +
  serde round-trip), review section (Codex review), quality gate.
- Tasks registered in `.harness/tasks.json`.
- `generated_task_ids` recorded in tracker.

## Update Instructions

1. Change `regseqmodel-phase-1` status to `tasked`.
2. Fill `generated_task_ids`.
3. Check if `regseqmodel-phase-2` can move to `ready_to_elaborate`
   (requires both phase-0 and phase-1 to be `tasked`).
