# Spec Batch: regseqmodel-phase-3b - Register access surface restructure and register_block! macro

## Description

This batch authors executable harness task specs for the register
architecture overhaul: moving register access methods from
ReadOnly/WriteOnly/ReadWrite wrapper types to Mmio<T>, adding the
RegisterOf trait system for access control, and implementing the
register_block! proc macro that replaces register_bitfields! + manual
#[repr(C)] struct with a unified scoped definition.

This is the most impactful Phase 3 batch because it changes the authored
driver surface. Every register access in every fixture changes form.

The concrete outcome is a set of executable harness tasks that deliver
the new register access architecture and the register_block! macro.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 3.3, 3.6)
- `docs/discussions/models/regseq-model/spec.md` (Section 10.2)
- `docs/discussions/models/regseq-model/compiler-knowledge.md` (Section 2, Mmio)
- `crates/regseq/src/mmio.rs`
- `crates/regseq/src/registers.rs`
- `crates/regseq-macros/src/lib.rs`
- `crates/regseq-macros/src/register_bitfields.rs`
- `tests/fixtures/pl011-full-spec/src/registers.rs`

## Output Specification

Create:

- `.harness/tasks/regseq-facade/` task spec files for register access restructure
- `.harness/tasks/regseq-macros/` task spec files for register_block! macro

Update when done:

- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`

The output is executable harness tasks.

## Implementation Notes

- **Register access on Mmio<T>** (crates/regseq/src/mmio.rs):
  - Add methods: `read`, `read_field`, `write`, `write_field`, `modify`
  - `read`/`read_field`/`write`/`write_field` carry `#[rseq::intrinsic("mmio_...")]`
  - `modify` is NOT an intrinsic (calls `read` then `write` internally)
  - All constrained by `R: RegisterOf<Block>` + `R::Access: ReadAccess/WriteAccess`

- **Supporting types** (crates/regseq/src/registers.rs):
  - `RegisterOf<Block>` trait with `type Access`
  - `ReadAccess`, `WriteAccess` marker traits
  - `ReadOnlyAccess`, `WriteOnlyAccess`, `ReadWriteAccess` types with impls
  - `RegisterId<W, R>` type for full-register read/write
  - `ReadOnly<T, R>`, `WriteOnly<T, R>`, `ReadWrite<T, R>` become layout-only:
    remove read/write/modify methods, keep #[repr(transparent)]

- **register_block! macro** (crates/regseq-macros/src/register_block.rs):
  - Syntax: `regseq::register_block! { #[endian, access_width] pub name { reg: AccessKind { fields... }, ... } }`
  - Generates scoped module: `name::Regs` (layout struct), `name::reg` (per-register module
    with Field, FieldValue, RegisterId, Register marker, OFFSET constant),
    `RegisterOf<name::Regs>` trait impls
  - The generated HIR should be compatible with what the current extractor
    recognizes (register blocks with #[rseq::register_block], descriptors
    with field/offset/width metadata)

- **Task decomposition guidance**:
  1. RegisterOf trait + access control types + RegisterId (registers.rs additions)
  2. Mmio<T> register access methods with intrinsic annotations (mmio.rs)
  3. Strip ReadOnly/WriteOnly/ReadWrite of their access methods (registers.rs removals)
  4. register_block! proc macro (regseq-macros, largest task)
  
  Tasks 1-3 are in regseq-facade. Task 4 is in regseq-macros.
  Task 2 depends on task 1. Task 3 depends on task 2.
  Task 4 depends on task 1 (needs RegisterOf, RegisterId types).

## Non-Goals

- No PL011 fixture conversion (phase-3d -- separate batch).
- No extraction pipeline changes (Phase 4).
- No changes to existing register_bitfields! macro (kept alongside register_block! during transition).
- No removal of register_bitfields! (deferred to Phase 9 cleanup).

## Acceptance Criteria

- All task spec files created.
- Each task spec includes: implementation scope, non-goals, test plan, quality gate.
- Tasks registered in `.harness/tasks.json`.
- `generated_task_ids` recorded in tracker.
- Tracker item updated.

## Update Instructions

1. Change `regseqmodel-phase-3b` status from `ready_to_elaborate` to `tasked`.
2. Fill `generated_task_ids` with the new task IDs.
3. Move `regseqmodel-phase-3d` to `ready_to_elaborate` (its dependency on phase-3b is now satisfied).
