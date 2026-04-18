# regseq-model-43: Pass 2 Straight-Line Body Construction

## Description

Implement the Pass 2 base body construction loop: obtain each
`FunctionId`'s monomorphized body via `tcx.optimized_mir(def_id)`
(or the exact equivalent returning
`mir_drops_elaborated_and_const_checked` output at
`mir_opt_level = 0`), translate locals, statements, and basic
terminators into Regseq IR, build the `BodySourceMap` in parallel,
drop unwind cleanup blocks, and attach the resulting `Body` to
its owning `Function`.

Covers straight-line code only -- SwitchInt, intrinsic
recognition, `?`-pattern folding, and other rules belong to
subsequent tasks.

## Input Context

- `docs/discussions/models/regseq-model/ir-reference.md` (Section 4
  Statements, Section 5 Terminators, Section 6 Body / LocalDecl /
  BasicBlock / BodySourceMap, Section 7 SourceOrigin, Section 8
  Pass 2 algorithm; preamble on MIR stage +
  release-profile semantics)
- `docs/discussions/models/regseq-model/spec.md` (Section 9 body
  ownership)
- `crates/regseq-model/src/ir/body.rs` (Body with `pub source_map:
  BodySourceMap`; BasicBlock; LocalDecl; BasicBlockSourceMap;
  OrderedRegion)
- `crates/regseq-model/src/ir/statement.rs` (StatementKind,
  MirIntrinsic)
- `crates/regseq-model/src/ir/terminator.rs` (TerminatorKind,
  SwitchTargets)
- `crates/regseq-driver/src/construction.rs` (from tasks 41-42)
- `tests/fixtures/pl011-full-spec/src/driver.rs` (line 69:
  `disable_uart()`)

## Implementation Notes

### Public entry point

```rust
pub fn build_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    function_id: FunctionId,
    type_def_index: &TypeDefIndex,
    constants: &ConstantIndex,
    functions: &FunctionIndex,
    interner: &mut SymbolInterner,
) -> Result<Body, ConstructionError>;
```

`TypeDefIndex`, `ConstantIndex`, and `FunctionIndex` resolve to
`cross_refs.type_defs`, `cross_refs.constants`, and
`cross_refs.functions` respectively on the `Pass1Output` returned
by `build_pass1_output` (task 41). `constants` requires task 42's
extension of `CrossRefMaps`; `functions` is already populated by
task 41.

Returns a fully-constructed `Body` whose `source_map` is owned by
the same struct (per `crates/regseq-model/src/ir/body.rs`). The
orchestrator attaches the returned `Body` to the owning
`Function` in the model by replacing the placeholder body
installed in Pass 1 (task 41).

Call `tcx.optimized_mir(def_id)` (or the precise query that
returns MIR at `mir_drops_elaborated_and_const_checked` with
`mir_opt_level = 0`, matching the compiler config pinned by
`regseq-driver-25`) to obtain the `mir::Body<'tcx>`.

### LocalDecl mapping

For each `mir::LocalDecl<'tcx>`:
- Map type via `map_ty` (task 42).
- Resolve user variable name by walking `mir_body.var_debug_info`
  and matching its target to each local. Intern the string via
  `SymbolInterner` and store as `LocalDecl.name: Option<Symbol>`.
  `None` for compiler-generated temps.
- Set `mutable` from `mir_local_decl.mutability`.

Set `Body.arg_count` from `mir_body.arg_count`. `LocalId(0)` is
the return place; `LocalId(1..=arg_count)` are arguments.

### Statement translation

Within each basic block, translate every `mir::Statement<'tcx>`
to Regseq IR `StatementKind`:
- `mir::StatementKind::Assign(box (place, rvalue))` →
  `Assign(Place, Rvalue)` (unbox; map both via task 42 helpers).
- `mir::StatementKind::SetDiscriminant { place, variant_index }`
  → direct.
- `mir::StatementKind::StorageLive(local)` →
  `StorageLive(LocalId)`.
- `mir::StatementKind::StorageDead(local)` →
  `StorageDead(LocalId)`.
- `mir::StatementKind::Intrinsic(box non_diverging)` →
  `MirIntrinsic(...)` (both sub-variants kept:
  `Assume(Operand)`, `CopyNonOverlapping { src, dst, count }`).
- Drop the rest per ir-reference.md Section 4 mapping table:
  `FakeRead`, `AscribeUserType`, `Retag`, `PlaceMention`,
  `Coverage`, `ConstEvalCounter`, `Nop`,
  `BackwardIncompatibleDropHint`.

### Terminator translation (this task: basic only)

- `mir::TerminatorKind::Goto { target }` →
  `Goto(BasicBlockId)`.
- `mir::TerminatorKind::Return` → `Return`.
- `mir::TerminatorKind::Unreachable` → `Unreachable`.
- `mir::TerminatorKind::Drop { place, target, .. }` →
  `Drop { place, target }` (drop `unwind`, `replace`, async
  fields).
- `mir::TerminatorKind::Call { func, args, destination, target,
  unwind, .. }` → `Call { callee, args, destination, target,
  error_target: None }`. Resolve `func` to `FunctionId` by looking
  up the callee's rustc `DefId` in `functions` (i.e.
  `cross_refs.functions` from `Pass1Output`). `unwind` dropped.
  `error_target` is `None` here -- task 46 (`?`-pattern folding)
  populates it later.
- `mir::TerminatorKind::SwitchInt`, `Assert`, `UnwindResume`,
  `UnwindTerminate`, `TailCall`, `Yield`, `CoroutineDrop`,
  `InlineAsm`, `FalseEdge`, `FalseUnwind` → not handled here
  (SwitchInt is task 44; the rest are dropped or hard-error per
  ir-reference.md Section 5).

### Cleanup-block dropping

Before translating, identify and drop every `mir::BasicBlockData`
with `is_cleanup == true` whose terminator is `UnwindResume`.
Driver code does not unwind. Maintain a translation map from
rustc `BasicBlock` index → Regseq IR `BasicBlockId` so dropped
blocks renumber successors correctly.

### BodySourceMap construction

Build a structurally-parallel `BodySourceMap` and store it on the
returned `Body.source_map`:
- `body_span` from `mir_body.span` resolved via
  `tcx.sess.source_map()`.
- `locals: Vec<SourceOrigin>` parallel to `local_decls`, populated
  from each `mir::LocalDecl::source_info`.
- `blocks: Vec<BasicBlockSourceMap>` parallel to `basic_blocks`,
  each carrying `statements: Vec<SourceOrigin>` from per-statement
  `source_info` and one `terminator: SourceOrigin` from the
  terminator's `source_info`.

### Source-vs-CompilerGenerated rule

- Use `Source(SourceLocation)` whenever a rustc `Span` is
  available and non-dummy -- this includes Drop terminators
  inserted by Rust's drop elaboration on `?` error paths (use
  the originating user span per ir-reference.md Section 7
  design rationale).
- Use `CompilerGenerated` only for genuinely synthesized nodes
  (none in this task -- compiler-generated bodies come later).

### Function attachment

After `build_body` returns, the orchestrator replaces the
placeholder body on the owning `Function` in the model. The
Function lives in its declared module per spec.md Section 9 (no
global body collection).

## Non-Goals

- No SwitchInt translation (task 44).
- No intrinsic recognition (task 45).
- No `?`-pattern folding (task 46).
- No cross-crate function extraction (task 47).
- No deref collapse (task 48).
- No OrderedRegion detection (task 49).
- No logging/register/pinctrl metadata extraction (tasks 50-52).
- No abstraction/intrinsic dependency-set derivation (task 53).
- No use of the legacy owned `Mir*` types.

## Test Plan

Integration test under `crates/regseq-driver/tests/` that
compiles the PL011 fixture and drives construction through
Pass 2's straight-line path:

- Translate `disable_uart()` from PL011 and assert:
  - `Body.arg_count == 1` (just `&mut self`).
  - `local_decls` length matches the MIR body's local count.
  - All `Call` terminators land as `Call` with `error_target:
    None`.
  - `body.source_map.body_span`, per-local, per-statement, and
    per-terminator entries all populated.
  - All cleanup blocks dropped; remaining block IDs renumber
    contiguously.
  - The `Body` is attached to the `disable_uart` Function
    inside the Driver module's impl block.
- Hand-built MIR fragment via a small fixture crate with a Drop
  terminator after a Call to verify Drop translation and
  `unwind` is dropped.
- Fixture exercising `Nop`/`FakeRead`/`Retag`/`Coverage`
  statements to verify they are dropped (may require a nightly
  attribute to surface them; if not reachable through normal
  code, skip and note in the test comment).
- Fixture exercising `NonDivergingIntrinsic::Assume` and
  `CopyNonOverlapping` to verify both `MirIntrinsic`
  sub-variants are produced.
- Negative test: a fixture containing `InlineAsm` or `TailCall`
  (if nightly permits) produces
  `ConstructionError::UnsupportedTerminator`.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to verify every retained
MIR construct in ir-reference.md Sections 4 and 5 (basic
terminators only) maps correctly from **live rustc types**,
every dropped MIR variant is dropped (not silently translated),
the source-origin rule for drop-elaboration is followed, and
`Body` is attached to the owning Function. Iterate until
approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `build_body()` translates `disable_uart()` to a Regseq IR
  `Body` whose structure matches a hand-verified golden
  serialization at the `RegseqModel` boundary.
- Every retained statement and basic terminator from
  ir-reference.md Sections 4-5 has at least one passing test.
- Cleanup-block dropping renumbers successors correctly.
- `BodySourceMap` is structurally parallel to `Body` for the
  PL011 test target.
- No import of `regseq_model::declaration::mir_body::*`.
- Codex review approved.
