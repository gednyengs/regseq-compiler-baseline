# regseq-linux-backend-04: Retarget `emit_semantic` to CFG `Body` via New C AST

**Depends on**: `regseq-backends-02`, `regseq-backends-03`,
`regseq-linux-backend-03`.

## Description

Phase-7C emitter rewrite. Retargets `linux::emit_semantic.rs` from
walking `SemanticModel` / `SemanticBody` / `Stmt` / `Expr` trees to
walking CFG `Body` + `BasicBlock` + `Statement` + `Terminator` from
`regseq_model::ir`. Output is produced via the new `backends::c`
AST (task 2) following the goto-based normal form from
`linux-emission-contract.md` (task 1).

Absorbs execution-plan Phase-8.1 ("Goto-based C emitter"). Emits
each `BasicBlock` as a labeled C block; control-flow terminators
(`Goto`, `SwitchInt`, `IntrinsicCall.error_target`, `Return`, loop
back-edges) become C `goto` statements.

**Writes the construct-level tests** that assert conformance to the
exemplar corpus from task 1. Per Codex round-3 review: those tests
live here, not in task 1, because this task creates the emitter
seam the tests exercise.

## Input Context

- `docs/discussions/models/regseq-model/linux-emission-contract.md`
  (landed by task 1) -- the normative specification every emission
  decision must cite.
- `tests/goldens/pl011/linux/constructs/*.c` (landed by task 1) --
  the exemplar corpus tests assert against.
- `crates/regseq-backends/src/linux/emission_tables.rs` (landed by
  task 1) -- consumed for intrinsic->API mapping and barrier
  placement.
- `crates/regseq-backends/src/c/` (landed by task 2) -- the AST
  this task uses.
- `docs/discussions/models/regseq-model/execution-plan.md` Phase
  8.1 (absorbed by Phase 7C).
- `docs/discussions/models/regseq-model/migration.md` Q5 Layers
  1-3 (construct tests contribute to Layer 3).
- `crates/regseq-model/src/ir/{body, statement, terminator,
  intrinsic}.rs` -- the CFG IR walked.
- `crates/regseq-backends/src/linux/emit_semantic.rs` lines 1-950
  -- the module being rewritten. The old code remains in the same
  file, replaced in-place (no separate "new" file + "old" file
  during transition; the emitter flips over in one commit).
- `crates/regseq-model/src/ir/body.rs` lines 65-154 -- the
  `OrderedRegion` + body-source-map shapes the emitter reads.

## Implementation Notes

### Function-level shape

For each `RegseqModel::Function` the emitter is asked to lower:

1. Emit a function signature per the emission contract's normal form.
2. Emit local declarations from `body.locals`, grouped by type, in
   `LocalId` order.
3. Emit entry-block label `bb<entry_id>:` (elided if `entry_id` is
   the first block).
4. Emit each `BasicBlock` in `BasicBlockId` order as a
   `CStmt::Labeled` wrapping a `CStmt::Compound` of
   `CBlockItem::Stmt`s for each `Statement` + a tail control-flow
   `CStmt` for the `Terminator`.

No reordering. No inlining. No flattening of straight-line chains
into one block (the contract requires one label per block so the
output is obvious from the CFG).

### Per-`Statement` lowering

| IR Statement | C lowering |
|--------------|------------|
| `StatementKind::Assign { place, rvalue }` | `CStmt::Expr(CExpr::Assign { lhs: lower_place(place), op: None, rhs: lower_rvalue(rvalue) })` |
| `StatementKind::StorageLive { .. }` | ignored (C declarations are all at function top) |
| `StatementKind::StorageDead { .. }` | ignored |
| `StatementKind::Nop` | `CStmt::Empty` |
| `StatementKind::SetDiscriminant { .. }` | `CStmt::Expr(...)` writing the tag field |

### Per-`Terminator` lowering

| IR Terminator | C lowering |
|---------------|------------|
| `Goto { target }` | `CStmt::Goto(format!("bb{}", target))` |
| `SwitchInt { discr, targets, otherwise }` | chain of `CStmt::If` with `CStmt::Goto` arms |
| `Return { value }` | `CStmt::Return(value.map(lower_operand))` |
| `Call { callee, args, destination, error_target }` | `CStmt::Expr(CExpr::Assign)` + `CStmt::Goto(target)`, plus `if (err) goto bb<error_target>` if present |
| `IntrinsicCall { intrinsic, args, metadata, destination, error_target }` | Per intrinsic: look up `INTRINSIC_LINUX_API` in `emission_tables`; synthesize the Linux call shape; handle `error_target` per `?`-propagation exemplar |
| `Unreachable` | `__builtin_unreachable();` |
| `Resume` | should not appear in emit paths (Phase-5 construction drops cleanup blocks ending in `resume`) |

### OrderedRegion barrier emission

For each `OrderedRegion { entry, exit }` on the body:

- At the start of block `entry`'s emission, emit the barrier C
  statement from `ORDERED_REGION_BARRIER` (compiler barrier: `asm
  volatile("" ::: "memory")`).
- At the end of block `exit`'s emission (before its terminator),
  emit the barrier.
- At every abnormal exit from the region (an `error_target` edge
  whose target is outside the region's block set), emit the
  barrier BEFORE the `goto` to the error block.

Reachability definition per `compiler-knowledge.md` "OrderedRegion:
SESE Invariant and CFG Modes": a block belongs to the region iff
it's reachable from `entry` without passing through `exit`.

### Error-path cleanup synthesis

This task implements the "handled cases" from
`compiler-knowledge.md` "Resource Cleanup on Error Paths":

- Case 1 (sequential `?` chain), Case 4 (match on result), Case 8
  (early return in branch), Case 10 (infallible ops in chain),
  Case 11 (inside ordered region), Case 12 (non-reversible
  failure).
- Dominator analysis finds reversible intrinsics (per
  `REVERSIBLE_CLEANUP_PAIRS` table) dominating each error-exit
  block. Emit reverse operations on a canonical cleanup label
  ladder (`err_<resource>`).

Cases 2, 3, 5 (conditional cleanup with optional resources), Case
6 (loop over array resources), Case 7 (interprocedural), Case 9
(re-acquisition): the emitter emits a diagnostic warning per
`compiler-knowledge.md` "Unhandled cases" and falls back to
best-effort cleanup (no reverse-ladder emitted for those sites).
Do NOT extend dominator analysis to handle these in this task --
the contract explicitly defers them.

### Construct-level tests (owned by this task)

Under `crates/regseq-backends/tests/cfg_construct_tests.rs`:

For each exemplar in `tests/goldens/pl011/linux/constructs/*.c`
(except callback-wrapper exemplars which belong to task 10):

```rust
#[test]
fn emit_straight_line_matches_exemplar() {
    let body = build_minimal_cfg_body(/* straight-line shape */);
    let actual = emit_function_body(&body, /* config */);
    let expected = std::fs::read_to_string(
        "tests/goldens/pl011/linux/constructs/straight_line.c"
    ).unwrap();
    assert_eq!(normalize(&actual), normalize(&expected));
}
```

`normalize` applies the normalization algorithm from task 1's
contract (whitespace, blank-line policy, comment banner). Tests
assert normalized equality, not raw byte equality, since a minor
trailing-newline difference should not false-positive.

Cover every exemplar this task's scope includes: straight_line,
branch_if, branch_if_else, loop_while, intrinsic_infallible,
intrinsic_fallible_qpropagate, ordered_region_straight,
ordered_region_with_error, cleanup_ladder_two_resources,
return_result, assignment_field, cast, mmio_read_field,
mmio_write_field, log_emit_format.

Test helpers (`build_minimal_cfg_body`, `emit_function_body`,
`normalize`) live alongside the tests.

### Deletion preserved for later

Do NOT delete `linux::emit_behavior.rs` in this task. That happens
in task 10. Do NOT delete `lowering::{mir_interpreter,
semantic_lowering, behavior_planner, body_interpreter}` -- task 14.

### Interaction with `emit_module.rs`

`emit_module.rs` still exists in its current (string-format) form
during this task. It calls the old `emit_semantic` helpers. The
rewrite of `emit_module.rs` is task 10; between tasks 9 and 10,
the old `emit_module.rs` compiles against the new `emit_semantic`
-- this requires either:

1. Keeping the old `emit_semantic` public API shape (the function
   names and signatures that `emit_module.rs` calls), with the
   internals rewritten. Preferred: the external API is stable;
   internals walk CFG.
2. If (1) is not feasible because the function boundaries don't
   align with CFG walking, introduce a temporary adapter:
   new CFG-based helpers + a thin legacy shim that converts old
   SemanticModel calls into `&Body` lookups (via
   `FunctionBodyIndex` from task 8) and invokes the new path.

Document the approach inline; task 10 removes the shim.

### Contract conformance review

Before closing this task, invoke Codex to verify every construct
test matches the corresponding contract rule + exemplar. This is
the second adversarial-review checkpoint for the contract (first
was in task 1).

## Non-Goals

- No `emit_module.rs` rewrite (task 10).
- No `emit_behavior.rs` deletion (task 10).
- No legacy-pipeline deletions (task 14).
- No callback-wrapper / probe / remove body emission (task 10
  owns module integration).
- No unsupported cleanup-case extensions (Cases 2, 3, 5, 6, 7, 9
  stay as diagnostic-only).
- **`crates/regseq-driver/src/construction*`** (strict non-goal).

## Test Plan

- **Construct tests**: every exemplar from task 1's corpus has a
  passing test under `cfg_construct_tests.rs`.
- **Normalization-algorithm stability**: two runs of
  `emit_function_body` on the same input produce byte-identical
  output.
- **PL011 end-to-end (smoke, not blessing)**: run full pipeline on
  PL011, check that `emit_semantic` produces non-empty output
  that parses via `cc -fsyntax-only` (wrapped in a minimal
  harness). Not a full blessing -- that's task 11.
- **Regression**: existing `pl011_linux_module_host_build_proof.rs`
  continues to pass (may be via the temporary adapter; document
  if so).
- `cargo test --workspace` green.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-linux-backend-04 against .harness/tasks/regseq-linux-backend/04-emit-semantic-cfg-rewrite.md.

Files:
- crates/regseq-backends/src/linux/emit_semantic.rs (rewritten)
- crates/regseq-backends/tests/cfg_construct_tests.rs (new)
- crates/regseq-backends/src/linux/emit_module.rs (expected: compiles
  unchanged via stable emit_semantic API or via documented shim)

Focus on:
- Every Statement / Terminator IR variant has a documented lowering
  in the module. Missing variants = MANDATORY.
- Every construct exemplar from tests/goldens/pl011/linux/constructs/
  has a matching test. Missing coverage = MANDATORY.
- OrderedRegion barriers placed per contract at entry + normal exit
  + every abnormal exit.
- `?`-propagation goto shape matches intrinsic_fallible_qpropagate.c.
- Cleanup ladder matches cleanup_ladder_two_resources.c for handled
  cases; unhandled cases emit diagnostic (not silent).
- Normal form applied: one label per BasicBlock, locals at top,
  no inlining.
- emit_module.rs continues to compile (adapter if needed).

Do NOT flag:
- emit_module.rs still being string-format (task 10).
- emit_behavior.rs still existing (task 10).
- lowering/* submodules still existing (task 14).
</task>
... (standard grounding + structured output) ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-backends --test cfg_construct_tests
cargo test -p regseq-backends
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-backends -- -D warnings
```

## Acceptance Criteria

- `linux::emit_semantic.rs` walks CFG `Body` via
  `backends::c::CStmt` / `CExpr`; no `SemanticBody` / `Stmt` /
  `Expr` imports remain in the file.
- Every IR `StatementKind` + `TerminatorKind` variant has a
  documented lowering.
- OrderedRegion barriers emitted per contract.
- Construct tests pass against the exemplar corpus.
- Host-build proof continues to pass (may be via documented
  transitional adapter; adapter retired in task 10).
- `cargo test --workspace` green.
- Codex review APPROVED.
