# Spec Batch: regseqmodel-phase-8 - Backend C emission from CFG-based IR

## Description

This batch authors executable harness task specs for C code emission
from the new CFG-based Regseq IR. Three deliverables:

1. **Goto-based C emitter.** Each `BasicBlock` becomes a labeled C
   block; control flow is direct `goto`/`if`-`goto`. Correct first;
   structured-control-flow recovery is a follow-up phase.
2. **Dominator-based resource cleanup** for lifecycle callbacks.
   For each error exit on a fallible callback (e.g., `init()`),
   identify reversible intrinsics dominating that exit and emit
   their reverse operations in reverse order, producing the
   standard C goto-chain cleanup pattern.
3. **Removal of deprecated backend code** that is now superseded:
   `lowering/mir_interpreter/`, `lowering/semantic_lowering/`,
   `lowering/behavior.rs`, `behavior_planner.rs`,
   `body_interpreter.rs`.

This phase is the first to rewrite Linux emission semantics. Phase 7
left `linux/emitter.rs` and `emit_*.rs` untouched; this phase
replaces them with CFG-based emitters that consume the Regseq IR
bodies built in Phase 5.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-status.yaml`
- `.harness/spec-batches/BRIEF_TEMPLATE.md`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 8,
  including the PL011 init() rollback risk row and the goto-based
  C readability tradeoff)
- `docs/discussions/models/regseq-model/spec.md` (Section 5 trait
  callbacks, Section 6 ISR handlers, Section 10 intrinsic catalog,
  Section 13 backend legality verification)
- `docs/discussions/models/regseq-model/ir-reference.md` (Sections 4,
  5, 6 -- statements, terminators, bodies; the emitter consumes
  these)
- `docs/discussions/models/regseq-model/compiler-knowledge.md`
  (Section 1 framework type lowering, Section 2 abstraction type
  intrinsic methods + reverse pairs + cleanup CFG modes, Section 3
  backend lowering details)
- `crates/regseq-model/src/ir/body.rs` (Body, BasicBlock,
  predecessors, OrderedRegion)
- `crates/regseq-model/src/model/regseq_model.rs`
- `crates/regseq-backends/src/linux/emitter.rs` (current emitter --
  to be replaced)
- `crates/regseq-backends/src/linux/emit_behavior.rs`
- `crates/regseq-backends/src/linux/emit_module.rs`
- `crates/regseq-backends/src/linux/emit_regs.rs`
- `crates/regseq-backends/src/linux/emit_semantic.rs` (current
  semantic emitter -- to be replaced)
- `crates/regseq-backends/src/linux/c_ast.rs`
- `crates/regseq-backends/src/lowering/mir_interpreter/` (to be
  deleted)
- `crates/regseq-backends/src/lowering/semantic_lowering/` (to be
  deleted)
- `crates/regseq-backends/src/lowering/behavior.rs` (to be deleted)
- `crates/regseq-backends/src/lowering/behavior_planner.rs` (to be
  deleted)
- `crates/regseq-backends/src/lowering/body_interpreter.rs` (to be
  deleted)
- `tests/fixtures/pl011-full-spec/` (the canonical comparison
  target)

## Output Specification

Create:

- `.harness/tasks/regseq-linux-backend/` task specs (emitter
  rewrite, per-terminator emission, ordered-region barriers)
- `.harness/tasks/regseq-lowering/` task specs (dominator-based
  cleanup analysis, deprecated-code removal)

Update when done:

- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`

## Implementation Notes

- **Goto-based emission strategy.** For each `Body`:
  - Emit each `BasicBlock` as a labeled C block
    (`bb<index>:`).
  - `Goto(target)` emits `goto bb<target>;`.
  - `SwitchInt { discriminant, targets }` emits a `switch` (or
    `if`/`else if` chain for binary cases like `Bool`); the default
    branch handles `otherwise`.
  - `Call { callee, args, destination, target, error_target }` emits
    the call, assigns `destination`, then `goto bb<target>`. If
    `error_target` is `Some`, emit a check on the returned
    `Result`/`Option` discriminant and `goto bb<error_target>` on
    Break.
  - `IntrinsicCall { intrinsic, args, metadata, destination, target,
    error_target }` lowers per intrinsic, using the
    `IntrinsicMetadata` to drive register access (offset, mask,
    shift, access width), logging (template + placeholders), or
    pin-state application (`state_id`).
  - `Drop { place, target }` emits the type's drop glue (or
    nothing for non-Drop types post drop-elaboration), then
    `goto bb<target>`.
  - `Return` emits `return _0;`.
  - `Unreachable` emits `__builtin_unreachable();` (or platform
    equivalent).
- **Variable names** come from `LocalDecl.name` when present
  (interned `Symbol` resolves through the model's `SymbolInterner`).
  For unnamed temporaries, emit `_<localid>`.
- **Type lowering** follows the per-variant table in spec.md
  Section 8 and the framework/abstraction tables in
  compiler-knowledge.md Sections 1-2.
- **OrderedRegion barrier placement** (ir-reference.md Section 6,
  spec.md Section 9): for every `OrderedRegion { entry, exit }`
  on a `Body`, emit a compiler barrier
  (`asm volatile("" ::: "memory");`) at:
  - The top of `entry` (before the first operation).
  - The top of `exit` (after the last operation).
  - The top of every block reached only via an `error_target` edge
    that originates inside the region (each abnormal exit gets its
    own barrier).
  Hardware memory barriers are NOT implied -- MMIO volatile
  semantics handle hardware ordering separately.
- **Dominator-based cleanup analysis** (spec.md Section 10,
  compiler-knowledge.md Section 2):
  - Compute a dominator tree on the callback's `Body` using
    `Body::predecessors()`.
  - For each error exit edge (`error_target` from any
    Call/IntrinsicCall, plus explicit `Err`-returning blocks),
    walk the dominator chain back to entry and collect every
    reversible `IntrinsicCall` that dominates the exit.
  - Reverse-intrinsic pairs (the table the emitter consults):
    `ClockEnable` ↔ `ClockDisable`, `ResetAssert` ↔ `ResetDeassert`,
    `PinCtrlApply(default)` ↔ `PinCtrlApply(<teardown_state>)` per
    the driver's pin-state attrs (use `default` if no explicit
    teardown state). Add additional pairs as the intrinsic catalog
    grows -- the table lives in the cleanup module so it's the
    single source of truth.
  - Emit reverse operations in **reverse acquisition order** before
    the error return.
  - The framework does NOT call the teardown callback
    (`deinit()`) when `init()` fails -- generated rollback is the
    sole cleanup mechanism (spec.md Section 10).
- **Handled cleanup CFG modes** (compiler-knowledge.md Section 2):
  sequential `?` chain (Case 1), match on result (Case 4),
  early return in branch (Case 8), infallible ops in chain
  (Case 8 variant), reversible op inside ordered region (barrier
  before reverse), non-reversible failure (no rollback for that
  step, continue rollback chain).
- **Unhandled cleanup CFG modes** (emit a diagnostic and skip
  rollback for the affected acquisition; do NOT generate
  incorrect cleanup): conditional acquisitions on optional
  resources (Cases 2, 3, 5), array acquisitions requiring
  loop induction tracking (Case 6), interprocedural acquisitions
  inside helpers (Case 7), and any future case the table doesn't
  cover (Case 9 placeholder).
- **PL011 init() limitation** (execution-plan Gaps+Risks): PL011's
  `init()` uses helper-contained optional acquisitions
  (Case 7 + Case 2). The compiler emits a diagnostic warning;
  each helper handles its own internal failure, so the direct `?`
  chain in `init()` itself is safe for dominator analysis. Do not
  attempt interprocedural rollback synthesis in this phase.
- **Goto-based C readability** is an accepted tradeoff
  (execution-plan Gaps+Risks). Generated code reads like a state
  machine, not idiomatic C. Structured control-flow recovery is
  out of scope for this phase and is a separate follow-up.
- **Deprecated code removal** is its own task and runs LAST.
  Removing files before consumers detach causes build failures.
  Verify with `cargo check --workspace` after each removal.
- **Phase boundary.** Phase 8 ends with the Linux backend emitting
  C from `RegseqModel` for PL011 with a structural match against
  the existing reference output (modulo expected goto/structured
  differences), all deprecated lowering directories deleted, and
  diagnostics surfaced for unsupported cleanup cases. **Do not**
  delete old model modules (Phase 9) and **do not** rewrite
  generated-output goldens to match new emission until Phase 9.

## Non-Goals

- No structured control-flow recovery (separate follow-up phase).
- No Zephyr backend updates.
- No conditional cleanup flag synthesis for optional resources
  (Cases 2, 3, 5) -- emit diagnostic instead.
- No loop induction variable tracking for array cleanup (Case 6)
  -- emit diagnostic.
- No interprocedural resource summaries (Case 7) -- emit
  diagnostic.
- No new intrinsics, no closure support, no synchronization
  intrinsics.
- No old model module deletion (Phase 9).
- No golden file regeneration to lock the new output (Phase 9).

## Acceptance Criteria

- Goto-based emitter produces functionally-equivalent C for every
  PL011 callback and helper covered by the existing reference
  output. Functional equivalence is verified by:
  - Compiling the generated module with the existing PL011 host
    build (the same proof used today by
    `pl011-linux-validation-11`).
  - Per-case structural assertions on the emitted C: every
    `?` desugars to a goto-chain to its `error_target` block,
    every `OrderedRegion` is bracketed by compiler barriers at
    entry/exit/abnormal exits, every reversible `IntrinsicCall`
    that dominates an error exit has a matching reverse call in
    reverse order on that exit's path.
- Dominator-based cleanup is verified per handled CFG mode with
  one targeted IR specimen each: sequential `?` chain, match on
  result, early return in branch, infallible op in chain, op
  inside ordered region, non-reversible op in chain.
- For each unsupported case (Cases 2, 3, 5, 6, 7, 9), the
  compiler emits a named diagnostic identifying the case number
  and the affected resource, and does NOT emit incorrect
  cleanup.
- PL011 `init()`: direct `?` chain rolls back correctly;
  helper-contained optional acquisitions emit the documented
  diagnostic.
- Deprecated directories `lowering/mir_interpreter/`,
  `lowering/semantic_lowering/`, and files `lowering/behavior.rs`,
  `lowering/behavior_planner.rs`, `lowering/body_interpreter.rs`
  are deleted; `cargo check --workspace` and
  `cargo test --workspace` pass.
- No reference to the deleted modules remains in the workspace
  (verified by grep over `crates/`).
- Tracker updated; `regseqmodel-phase-9` advances to
  `ready_to_elaborate`.

## Update Instructions

1. Change `regseqmodel-phase-8` status to `tasked`.
2. Fill `generated_task_ids`.
3. Move `regseqmodel-phase-9` to `ready_to_elaborate`.
