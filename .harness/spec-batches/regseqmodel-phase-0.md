# Spec Batch: regseqmodel-phase-0 - Foundation types

## Description

This batch authors executable harness task specs for the foundation types
that all new RegseqModel code depends on. These are leaf types with no
dependencies on existing regseq-model modules.

The batch covers three type families: identity (Symbol interning, DeclId
with typed wrappers, source locations), type system (RegseqType, ScalarType,
Mutability), and operators (BinOp, UnOp). All types live in new submodules
(`identity/`, `ir/`) alongside the existing regseq-model modules.

The concrete outcome is a set of executable harness tasks that, when
completed, provide the foundational types for Phase 1 (IR types) and
Phase 2 (model structure).

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 0)
- `docs/discussions/models/regseq-model/spec.md` (Sections 7, 8)
- `docs/discussions/models/regseq-model/ir-reference.md` (Sections 1, 7)
- `crates/regseq-model/src/lib.rs`

## Output Specification

Create:

- `.harness/tasks/regseq-model/` task spec files for each task
- Updates to `.harness/tasks.json` via `task.py` commands

Update when done:

- `.harness/task-spec-status.yaml`

The output is executable harness tasks.

## Implementation Notes

- **Module layout**: Create `crates/regseq-model/src/identity/` and
  `crates/regseq-model/src/ir/` as new submodules. Add `pub mod identity;`
  and `pub mod ir;` to `lib.rs`. Do NOT modify existing modules.

- **Symbol interning** (identity/symbol.rs):
  - `Symbol(u32)`, `FileId(u32)`, `SymbolInterner`
  - Custom serde: Symbol serializes as its resolved string, not as raw u32
  - `SymbolInterner` has `intern(&mut self, s: &str) -> Symbol`,
    `intern_file(&mut self, path: &str) -> FileId`,
    `resolve(&self, sym: Symbol) -> &str`,
    `resolve_file(&self, file: FileId) -> &str`
  - Both Symbol and FileId share a single string pool with distinct newtypes

- **DeclId and typed wrappers** (identity/decl_id.rs):
  - `DeclId(u32)` base type
  - 9 typed wrappers: `ModuleId(DeclId)`, `ConstantId(DeclId)`,
    `FunctionId(DeclId)`, `TypeDefId(DeclId)`, `RegisterDescriptorId(DeclId)`,
    `RegisterBlockId(DeclId)`, `ResourceId(DeclId)`, `PropertyId(DeclId)`,
    `UserDataId(DeclId)`
  - Body-level IDs (not DeclId-based): `LocalId(u32)`, `BasicBlockId(u32)`
  - All: `Copy + Clone + Eq + Hash + Serialize + Deserialize`

- **Source locations** (identity/source.rs):
  - `SourceLocation { file: FileId, start_line: u32, start_column: u32, end_line: u32, end_column: u32 }`
  - `SourceOrigin` enum: `Source(SourceLocation)` | `CompilerGenerated`

- **RegseqType** (ir/types.rs):
  - 10 variants: `Scalar(ScalarType)`, `Bool`, `Unit`, `Never`,
    `Option(Box<RegseqType>)`, `Result(Box<RegseqType>, Box<RegseqType>)`,
    `Array(Box<RegseqType>, usize)`, `Slice(Box<RegseqType>)`,
    `Ref(Box<RegseqType>, Mutability)`, `Defined(TypeDefId)`
  - `ScalarType`: `U8`, `U16`, `U32`, `U64`, `I8`, `I16`, `I32`, `I64`, `Usize`, `Isize`
  - `Mutability`: `Shared`, `Mutable`

- **Operators** (ir/ops.rs):
  - `BinOp` with 26 variants (see ir-reference.md Section 3)
  - `UnOp` with 2 variants: `Not`, `Neg`

- **Serde convention**: All enums use adjacently-tagged format
  (`#[serde(tag = "kind", content = "data")]`). Optional fields use
  `#[serde(skip_serializing_if = "Option::is_none")]` with `#[serde(default)]`.

- **Task decomposition guidance**: Split by file/module boundary. Each task
  should be independently testable. Suggested split:
  1. identity/symbol.rs (SymbolInterner is the most complex piece)
  2. identity/decl_id.rs + identity/source.rs + identity/mod.rs (straightforward newtypes)
  3. ir/types.rs (RegseqType + ScalarType + Mutability)
  4. ir/ops.rs + ir/mod.rs (BinOp + UnOp)

## Non-Goals

- No IR composite types (Place, Operand, Rvalue, Statement, Terminator, Body) -- those are Phase 1.
- No model structure types (Module, Function, TypeDef, etc.) -- those are Phase 2.
- No changes to existing regseq-model modules.
- No extraction or normalization code.

## Acceptance Criteria

- All task spec files created under `.harness/tasks/regseq-model/`.
- Each task spec includes: implementation scope, non-goals, test plan (construction + serde round-trip), and quality gate.
- Tasks registered in `.harness/tasks.json` via `task.py`.
- `generated_task_ids` recorded in tracker.
- Tracker item updated.

## Update Instructions

1. Change `regseqmodel-phase-0` status from `ready_to_elaborate` to `tasked`.
2. Fill `generated_task_ids` with the new task IDs.
3. `regseqmodel-phase-1` remains `ready_to_elaborate` (it was already set that way and depends on phase-0).
4. `regseqmodel-phase-2` remains `not_ready` (depends on both phase-0 and phase-1).
