# Spec Batch: regseqmodel-phase-3a - Facade attribute annotations and ErrorKind removal

## Description

This batch authors executable harness task specs for adding `#[rseq::]`
attribute annotations to all facade types and intrinsic methods, and
removing the deprecated `ErrorKind` alias.

This is additive work -- `#[rseq::]` attributes are inert tool attributes
that do not change compilation behavior. The annotations prepare the
facade crate for attribute-based extraction in Phase 4.

The concrete outcome is a set of executable harness tasks that annotate
framework types, abstraction types, and intrinsic methods with their
semantic classification attributes.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 3.1, 3.2, 3.3, 3.4)
- `docs/discussions/models/regseq-model/spec.md` (Sections 3.4, 10.1, 10.2)
- `docs/discussions/models/regseq-model/compiler-knowledge.md` (Sections 1, 2)
- `crates/regseq/src/lib.rs`
- `crates/regseq/src/error.rs`
- `crates/regseq/src/serial.rs`
- `crates/regseq/src/clock.rs`
- `crates/regseq/src/reset.rs`
- `crates/regseq/src/pinctrl.rs`
- `crates/regseq/src/mmio.rs`
- `crates/regseq/src/irq.rs`
- `crates/regseq/src/sync.rs`
- `crates/regseq/src/time.rs`
- `crates/regseq/src/registers.rs`
- `crates/regseq/src/intrinsics/timed_delay.rs`
- `crates/regseq/src/intrinsics/diagnostic_logging.rs`
- `crates/regseq/src/intrinsics/register_access.rs`

## Output Specification

Create:

- `.harness/tasks/regseq-facade/` task spec files for each task

Update when done:

- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`

The output is executable harness tasks.

## Implementation Notes

- **Framework type annotations** (`#[rseq::framework_type("...")]`):
  10 types across error.rs, serial.rs, time.rs, irq.rs. See execution
  plan Phase 3.1 for the full table.

- **Abstraction type annotations** (`#[rseq::abstraction("...")]`):
  9 types across clock.rs, reset.rs, pinctrl.rs, irq.rs, mmio.rs, sync.rs.
  See execution plan Phase 3.2 for the full table.

- **Intrinsic annotations** (`#[rseq::intrinsic("...")]`):
  Register access intrinsics go on `Mmio<T>` methods (NOT on
  ReadOnly/WriteOnly/ReadWrite). Clock, ResetLine, PinCtrl carry their
  own intrinsic methods. Free-function intrinsics (busy_wait, sleep,
  logging, poll_until) get named annotations. See execution plan
  Phase 3.3 for the full table.

- **ErrorKind removal**: Remove `pub type ErrorKind = SerialErrors;`
  from serial.rs and its re-export in lib.rs.

- **NOTE**: This batch does NOT restructure the register access surface
  (that is phase-3b). It only adds attributes to existing methods.
  However, intrinsic annotations for register access should be added
  to Mmio<T> methods, which means phase-3b must create those methods
  first. The attribute-only annotations (framework_type, abstraction,
  non-register intrinsics, ErrorKind removal) can proceed independently.

- **Task decomposition guidance**: Split by concern:
  1. Framework type + abstraction annotations (all files, additive only)
  2. Non-register intrinsic annotations (Clock, ResetLine, PinCtrl, timed_delay, logging, poll_until)
  3. ErrorKind removal
  Note: Register intrinsic annotations on Mmio<T> depend on phase-3b
  and should be part of that batch, not this one.

## Non-Goals

- No register access surface restructure (phase-3b).
- No register_block! macro (phase-3b).
- No ordered! macro changes (phase-3c).
- No PL011 fixture changes (phase-3d).
- No extraction pipeline changes (Phase 4).

## Acceptance Criteria

- All task spec files created under `.harness/tasks/regseq-facade/`.
- Each task spec includes: implementation scope, non-goals, test plan, quality gate.
- Tasks registered in `.harness/tasks.json`.
- `generated_task_ids` recorded in tracker.
- Tracker item updated.

## Update Instructions

1. Change `regseqmodel-phase-3a` status from `ready_to_elaborate` to `tasked`.
2. Fill `generated_task_ids` with the new task IDs.
3. No downstream items to advance (phase-3d depends on phase-3b, not phase-3a).
