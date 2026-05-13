# Stage 9 -- Prior Art: rustc primary source

Primary-source rustc evidence for `mir::Terminator<'tcx>` /
`mir::TerminatorKind<'tcx>` and the surrounding pipeline (cleanup
passes, runtime-lowering passes, codegen consumer, MIR validator,
visitor, pretty-printer), scoped to what stage 9's candidate decision
namespace needs:

- `D-9-1` Closed `TerminatorKind` surface (proposed 6 variants:
  `Goto`, `SwitchInt`, `Return`, `Unreachable`, `Call`, `Assert`).
  Sub-tags `D-9-1.1` cardinality, `D-9-1.2`-`D-9-1.6` per-variant
  payloads.
- `D-9-2` Drop-as-Call lowering at the bridge (synthetic `Call` to
  drop-glue Instance).
- `D-9-3` Call callee shape supporting vtable-indirect dispatch.
- `D-9-4` Per-arg span preservation (`Spanned<Operand>`).
- `D-9-5` `AssertKind` subset (drop the 3 coroutine-related
  variants, keep 8).
- `D-9-6` `SwitchTargets` storage shape.
- `D-9-7` No Terminator interning.
- `D-9-8` Wrapper struct `Terminator { kind, span }`.
- `D-9-9` Cleanup-pass-discharged rejection table (`TailCall`,
  `UnwindResume`, `UnwindTerminate`, `Yield`, `CoroutineDrop`,
  `FalseEdge`, `FalseUnwind`, `InlineAsm`, plus Drop's `replace` /
  `drop` / `async_fut`).
- `D-9-10` Invariants registry.
- `D-9-K` `render_terminator` deferral to stage 11.

Out of scope here: the **rustc_public** flattening of TerminatorKind
(that surface is a separate lens), the stage-1 `InstanceKind`
closure decision (covered in `../../stage-1/prior-art/rustc.md`,
referenced only where load-bearing for D-9-3), and any RegseqModel
interning / intern-arena policy beyond what the rustc evidence
suggests.

All paths below are relative to the pinned-toolchain root:

```
~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/
```

(See `../../rustc-references.md` for the canonical statement of
this root path.)

Every factual claim cites a `[file:line]` in that pinned source.
Hypotheses are flagged HYPOTHESIS:. Inferences combining two
sources into a third are flagged INFERENCE:. The Regseq compiler
consumes `optimized_mir` outputs, so "what reaches the bridge"
means: the body returned by `optimized_mir` after
`run_analysis_to_runtime_passes` and `run_optimization_passes` have
completed.

A note on pass-manager semantics: stage 8 already established
[`stage-8/prior-art/rustc.md` §3] the distinction between
`is_required(): bool` (a `Optimizations::Suppressed` override hint)
and `is_enabled(&Session): bool` (the actual gate). That analysis
is not duplicated here. We use those terms in the same sense.

---

## 1. `TerminatorKind<'tcx>` full variant table

`TerminatorKind<'tcx>` is a 15-variant closed enum
[`rustc_middle/src/mir/syntax.rs:702-987`]:

```rust
#[derive(Clone, TyEncodable, TyDecodable, Hash, HashStable, PartialEq, TypeFoldable, TypeVisitable)]
pub enum TerminatorKind<'tcx> {
    Goto { target: BasicBlock },
    SwitchInt { discr: Operand<'tcx>, targets: SwitchTargets },
    UnwindResume,
    UnwindTerminate(UnwindTerminateReason),
    Return,
    Unreachable,
    Drop { place, target, unwind, replace, drop, async_fut },
    Call { func, args, destination, target, unwind, call_source, fn_span },
    TailCall { func, args, fn_span },
    Assert { cond, expected, msg, target, unwind },
    Yield { value, resume, resume_arg, drop },
    CoroutineDrop,
    FalseEdge { real_target, imaginary_target },
    FalseUnwind { real_target, unwind },
    InlineAsm { asm_macro, template, operands, options, line_spans, targets, unwind },
}
```

Cardinality is closed (no `Other` / `Opaque` arm); confirmed by
the cardinality of `name()`'s match
[`rustc_middle/src/mir/terminator.rs:474-492`].

Size budget: rustc pins
[`rustc_middle/src/mir/syntax.rs:1754`]:

```rust
static_assert_size!(TerminatorKind<'_>, 80);
```

80 bytes. Notably, this is far larger than `StatementKind`'s 16
[`syntax.rs:1753`]; rustc accepts a wider per-kind budget for
terminators because there is exactly one terminator per basic
block (vs. a vector of statements), so the per-block allocation
cost is amortized differently. INFERENCE: this is the signal that
boxing payloads inside terminator variants is rare in rustc's own
choices -- only `Assert.msg` is `Box<AssertMessage>`, and only
`Call.args`/`InlineAsm.operands` are `Box<[T]>` (slice indirection
for array-typed payloads, not size-discrimination indirection).

The 15-variant inventory below summarizes per-variant payload
shape and the most salient semantic notes; deep dives on the
variants RegseqModel keeps live in §4-§9, and the discharge
mechanics for the variants RegseqModel rejects live in §3.

| # | Variant | Source line | Payload | Notes |
|---|---------|-------------|---------|-------|
| 1 | `Goto` | `syntax.rs:705` | `target: BasicBlock` | Single successor, no unwind. |
| 2 | `SwitchInt` | `syntax.rs:715-719` | `discr: Operand`, `targets: SwitchTargets` | Switch over integer/char/bool. Target dedup invariant; "otherwise" is the last entry (§7). |
| 3 | `UnwindResume` | `syntax.rs:727` | (unit) | Continue unwind out of cleanup block. Cleanup-blocks-only invariant. Not permitted with `-C unwind=abort` after deaggregation [`syntax.rs:725-726`]. |
| 4 | `UnwindTerminate` | `syntax.rs:733` | `UnwindTerminateReason` | Terminate process during unwind. Cleanup-blocks-only. |
| 5 | `Return` | `syntax.rs:746` | (unit) | Return from function (assigns `_0`). Forbidden in cleanup blocks [`validate.rs:518-523`]. |
| 6 | `Unreachable` | `syntax.rs:751` | (unit) | UB if executed. |
| 7 | `Drop` | `syntax.rs:790-799` | `place, target, unwind, replace, drop, async_fut` | After drop elaboration: unconditional call to `drop_in_place` [`syntax.rs:755-757`]. See §4. |
| 8 | `Call` | `syntax.rs:811-832` | `func, args, destination, target, unwind, call_source, fn_span` | The general callable-target terminator. See §5. |
| 9 | `TailCall` | `syntax.rs:850-862` | `func, args, fn_span` | `become` lowering; chimera of Call + Return. No `target`/`destination`/`unwind` -- inherited from caller. |
| 10 | `Assert` | `syntax.rs:875-881` | `cond, expected, msg, target, unwind` | Run-time invariant check; failure paths to lang-item panic call. See §6. |
| 11 | `Yield` | `syntax.rs:900-909` | `value, resume, resume_arg, drop` | Coroutine suspension. Not permitted outside coroutine bodies, or after coroutine lowering [`syntax.rs:897`]. |
| 12 | `CoroutineDrop` | `syntax.rs:921` | (unit) | End of coroutine drop glue. Same gate as `Yield`. |
| 13 | `FalseEdge` | `syntax.rs:929-935` | `real_target, imaginary_target` | Borrowck conservative goto; runtime semantics is goto. Disallowed after drop elaboration [`syntax.rs:928`]. |
| 14 | `FalseUnwind` | `syntax.rs:944-953` | `real_target, unwind` | Borrowck conservative loop unwind; runtime semantics is goto. Disallowed after drop elaboration [`syntax.rs:943`]. |
| 15 | `InlineAsm` | `syntax.rs:957-986` | `asm_macro, template, operands, options, line_spans, targets, unwind` | `asm!` / `naked_asm!` block. See §9. |

Maps to `D-9-1.1` (cardinality of the rustc surface is 15;
candidate RegseqModel surface is 6 = `Goto`, `SwitchInt`,
`Return`, `Unreachable`, `Call`, `Assert`. Drop is collapsed
into Call by D-9-2; the remaining 8 are rejected per the §3
discharge table).

---

## 2. `Terminator<'tcx>` wrapper struct

The wrapper [`rustc_middle/src/mir/terminator.rs:435-439`]:

```rust
#[derive(Clone, Debug, TyEncodable, TyDecodable, HashStable, TypeFoldable, TypeVisitable)]
pub struct Terminator<'tcx> {
    pub source_info: SourceInfo,
    pub kind: TerminatorKind<'tcx>,
}
```

Two fields. `source_info` is `SourceInfo { span: Span, scope:
SourceScope }` [`rustc_middle/src/mir/mod.rs:822-830`] -- the same
pair `Statement<'tcx>` carries (see stage-8 prior-art §4). So the
wrapper itself is "kind plus debug bookkeeping," and there is no
indirection (`kind` is stored inline; the wrapper's size is
80 + 16 = 96 bytes ignoring alignment padding -- no
`static_assert_size!` is pinned for the wrapper).

Notable accessors and helpers on the wrapper / kind:

- `successors()` -> `Successors<'_>`
  [`terminator.rs:442-445, 527-574`]. Returns an
  `impl DoubleEndedIterator<Item = BasicBlock>` -- this is the
  per-variant CFG-edge enumeration (used by stage-10 BasicBlock
  closure and stage-13 traversal).
- `identical_successor()` [`terminator.rs:448-453`].
- `successors_mut(...)` [`terminator.rs:455-458, 576-637`].
- `unwind()` -> `Option<&UnwindAction>`
  [`terminator.rs:460-463, 640-660`]. Only 5 of 15 variants
  carry an `unwind` slot: `Call`, `Assert`, `Drop`, `FalseUnwind`,
  `InlineAsm`. The other 10 return `None`.
- `as_switch()` [`terminator.rs:683-689`], `as_goto()`
  [`terminator.rs:691-697`] -- pattern-match shortcuts.
- `can_write_to_memory()` [`terminator.rs:699-719`] -- a
  metadata accessor stage 14 backends may want; `Call`, `Drop`,
  `TailCall`, `Yield`, `InlineAsm` write, the rest do not.
- `name()` [`terminator.rs:474-492`] -- returns the kind name
  as a `&'static str`. RegseqModel `render_terminator` deferral
  (D-9-K) means we will eventually want the analogue of `name()`
  + `fmt_head` + `fmt_successor_labels` (§12).
- `edges()` -> `TerminatorEdges<'_, 'tcx>`
  [`terminator.rs:769-841`]. A 5-variant `TerminatorEdges` enum
  [`terminator.rs:722-741`] (`None`, `Single`, `Double`,
  `AssignOnReturn`, `SwitchInt`) that summarizes the CFG
  topology for dataflow consumers. Stage-10 / stage-13 may want
  an analogous "shape-of-edges" view.

What rustc carries that RegseqModel may not need:

- `successors_mut` is a CFG-mutation hook used by transform
  passes [`terminator.rs:576-637`]. RegseqModel is a downstream
  consumer; we read but do not rewrite the CFG. INFERENCE: the
  mutating accessors are not load-bearing for RegseqModel.
- `edges()`'s `AssignOnReturn` variant
  [`terminator.rs:733-738`] bundles `CallReturnPlaces`
  (`Call(Place)`, `Yield(Place)`, `InlineAsm(&[InlineAsmOperand])`)
  -- this is the dataflow-pass interface for "places written on
  the success edge." HYPOTHESIS: stage-13 (analyses) may want
  the equivalent, but stage 9 doesn't need to define
  `TerminatorEdges` -- per-variant successor enumeration suffices.

D-9-8 framing: rustc's `Terminator { source_info, kind }` is the
minimal shape. RegseqModel's parallel decision is whether to
flatten `SourceInfo { span, scope }` to just `span` (since
RegseqModel does not consume the borrowck-era `scope` for any
known stage 14 backend purpose -- HYPOTHESIS pending stage-11
debug-info closure). The proposal `Terminator { kind, span }` in
the D-9 framing matches a "drop the scope" simplification.

D-9-7 framing: `TerminatorKind<'tcx>` does not derive `Eq` or
`Hash` directly on the kind (it does derive `PartialEq` /
`Hash` for use in `HashStable`). rustc does not intern terminators
-- they are stored inline in `BasicBlockData`. INFERENCE:
RegseqModel inheriting that "no interning, owned per-block" choice
is a low-friction decision; the rustc precedent is unambiguous.

---

## 3. Cleanup-pass discharge table

This is the spine of the D-9-9 rejection table. The pipeline order
is set in [`rustc_mir_transform/src/lib.rs:583-687`]:

```
optimized_mir
  -> run_analysis_to_runtime_passes  [lib.rs:583]
       -> run_analysis_cleanup_passes [lib.rs:618-634]
           1. ImpossiblePredicates
           2. CleanupPostBorrowck       <-- discharges FalseEdge, FalseUnwind
           3. RemoveNoopLandingPads
           4. SimplifyCfg::PostAnalysis
           5. Derefer
       -> run_runtime_lowering_passes  [lib.rs:637-664]
           1. CriticalCallEdges
           2. PostAnalysisNormalize
           3. Subtyper
           4. ElaborateDrops            <-- discharges drop conditionality
           5. CheckDropRecursion (lint)
           6. AbortUnwindingCalls       <-- discharges UnwindResume / UnwindTerminate under panic=abort
           7. AddMovesForPackedDrops
           8. AddRetag
           9. EraseDerefTemps
          10. ElaborateBoxDerefs
          11. StateTransform            <-- discharges Yield / CoroutineDrop
          12. KnownPanicsLint (lint)
       -> run_runtime_cleanup_passes   [lib.rs:667-687]
           1. LowerIntrinsics
           2. RemovePlaceMention
           3. SimplifyCfg::PreOptimizations
  -> run_optimization_passes            [lib.rs:689-...]
       (the Regseq cleanup-pass whitelist applies here)
```

For each rustc terminator variant the Regseq pipeline plans to
reject, this table states what discharges it and where, in pipeline
order:

### 3.1 `FalseEdge` / `FalseUnwind`

[`syntax.rs:929-953`]. Both are runtime-semantically gotos; both
exist purely to make borrowck more conservative.

**Discharge.** `CleanupPostBorrowck::run_pass` rewrites both into
plain `Goto` [`rustc_mir_transform/src/cleanup_post_borrowck.rs:67-74`]:

```rust
match terminator.kind {
    TerminatorKind::FalseEdge { real_target, .. }
    | TerminatorKind::FalseUnwind { real_target, .. } => {
        invalidate_cfg = true;
        terminator.kind = TerminatorKind::Goto { target: real_target };
    }
    _ => {}
}
```

`is_required: true` [`cleanup_post_borrowck.rs:88-90`]. The pass
runs in `run_analysis_cleanup_passes`
[`lib.rs:621`] which runs unconditionally as the first cleanup
phase before any of the runtime-lowering passes. Validator
confirms the gate: at `MirPhase::Runtime(RuntimePhase::Initial)`
or later, both variants are hard errors
[`validate.rs:466-485`]:

```rust
TerminatorKind::FalseEdge { .. } => {
    if self.body.phase >= MirPhase::Runtime(RuntimePhase::Initial) {
        self.fail(location, "`FalseEdge` should have been removed after drop elaboration");
    }
    ...
}
TerminatorKind::FalseUnwind { .. } => {
    if self.body.phase >= MirPhase::Runtime(RuntimePhase::Initial) {
        self.fail(location, "`FalseUnwind` should have been removed after drop elaboration");
    }
    ...
}
```

`optimized_mir` is `Runtime(PostCleanup)` (set at
[`lib.rs:612`]) which is greater than `Runtime(Initial)`. So
both variants are categorically prohibited in `optimized_mir`
bodies. The codegen consumers `bug!` on them
[`rustc_codegen_ssa/src/mir/block.rs:1553-1555`] and
[`rustc_codegen_cranelift/src/base.rs:579-583`].

D-9-9 entry: `FalseEdge` / `FalseUnwind` -> "rewritten to
`Goto { real_target }` by `CleanupPostBorrowck` (`is_required: true`,
unconditional cleanup phase); validator-rejected at
`Runtime(Initial)+`; bridge rejects with diagnostic if seen
(defence-in-depth)."

### 3.2 `Yield` / `CoroutineDrop`

[`syntax.rs:900-921`]. Both exist only in coroutine bodies.

**Discharge.** `coroutine::StateTransform::run_pass`
[`rustc_mir_transform/src/coroutine.rs:1465-1681`] rewrites the
entire coroutine body into a state-machine ADT, with `Yield`
sites replaced by suspension-point assignments (the
`TransformVisitor` walk at
[`coroutine.rs:1590`]) and `CoroutineDrop` consumed by
`elaborate_coroutine_drops`. `is_required: true`
[`coroutine.rs:1683-1685`]. Runs in `run_runtime_lowering_passes`
[`lib.rs:660`].

Validator confirms the gate: at `Runtime(Initial)+`, both are
hard errors [`validate.rs:454-465, 492-501`]:

```rust
TerminatorKind::Yield { resume, drop, .. } => {
    if self.body.coroutine.is_none() {
        self.fail(location, "`Yield` cannot appear outside coroutine bodies");
    }
    if self.body.phase >= MirPhase::Runtime(RuntimePhase::Initial) {
        self.fail(location, "`Yield` should have been replaced by coroutine lowering");
    }
    ...
}
TerminatorKind::CoroutineDrop => {
    if self.body.coroutine.is_none() {
        self.fail(location, "`CoroutineDrop` cannot appear outside coroutine bodies");
    }
    if self.body.phase >= MirPhase::Runtime(RuntimePhase::Initial) {
        self.fail(location, "`CoroutineDrop` should have been replaced by coroutine lowering");
    }
}
```

Codegen-SSA `bug!`s on both
[`rustc_codegen_ssa/src/mir/block.rs:1550-1552`]:

```rust
mir::TerminatorKind::CoroutineDrop | mir::TerminatorKind::Yield { .. } => {
    bug!("coroutine ops in codegen")
}
```

cranelift mirror at
[`rustc_codegen_cranelift/src/base.rs:579-583`]:

```rust
TerminatorKind::Yield { .. }
| TerminatorKind::FalseEdge { .. }
| TerminatorKind::FalseUnwind { .. }
| TerminatorKind::CoroutineDrop => {
    bug!("shouldn't exist at codegen {:?}", bb_data.terminator());
}
```

There is also a deeper layer of unreachability: stage 3 already
rejected `ty::Coroutine` / `ty::CoroutineWitness` from the
RegseqModel `Ty` surface. INFERENCE: even if the validator gate
were lifted, the bridge would refuse the function at the type
layer first. `Yield` / `CoroutineDrop` are doubly-discharged.

D-9-9 entry: `Yield` / `CoroutineDrop` -> "discharged by
`StateTransform` (`is_required: true`); validator-rejected at
`Runtime(Initial)+`; secondary discharge: stage-3 `Ty` surface
already rejects coroutine types so non-coroutine bodies cannot
contain these terminators."

### 3.3 `UnwindResume` / `UnwindTerminate`

[`syntax.rs:727-733`]. Cleanup-block terminators that interact
with the unwinder.

**Discharge.** Two cooperating mechanisms under `panic=abort`:

(a) `AbortUnwindingCalls::run_pass`
[`rustc_mir_transform/src/abort_unwinding_calls.rs:67-83`]
rewrites `UnwindResume` in non-unwinding bodies:

```rust
if let TerminatorKind::UnwindResume = &terminator.kind {
    if !target_supports_unwinding {
        terminator.kind = TerminatorKind::Unreachable;
    } else if !body_can_unwind {
        terminator.kind = TerminatorKind::UnwindTerminate(UnwindTerminateReason::Abi);
    }
}
```

`is_required: true`
[`abort_unwinding_calls.rs:141-143`]. Runs in
`run_runtime_lowering_passes` [`lib.rs:651`].

(b) The same pass also strips `UnwindAction::Cleanup(_)` /
`UnwindAction::Continue` from non-cleanup-block terminators when
the call cannot unwind or the target does not support unwinding
[`abort_unwinding_calls.rs:116-134`], replacing them with
`UnwindAction::Unreachable` or `Terminate`. After this pass
under `panic=abort`, all `UnwindAction` instances in non-cleanup
blocks are `Unreachable` or `Terminate` (no live cleanup edges
to cleanup blocks).

(c) Under `-C panic=abort` plus the absence of any function in
the body that can unwind, the `target_supports_unwinding /
body_can_unwind` gate triggers: every `UnwindResume` becomes
either `Unreachable` (for wasm-style targets without
exception-handling) or `UnwindTerminate(Abi)`.

Validator: both variants are cleanup-blocks-only
[`validate.rs:503-517`]:

```rust
TerminatorKind::UnwindResume => {
    let bb = location.block;
    if !self.body.basic_blocks[bb].is_cleanup {
        self.fail(location, "Cannot `UnwindResume` from non-cleanup basic block")
    }
    if !self.can_unwind {
        self.fail(location, "Cannot `UnwindResume` in a function that cannot unwind")
    }
}
TerminatorKind::UnwindTerminate(_) => {
    let bb = location.block;
    if !self.body.basic_blocks[bb].is_cleanup {
        self.fail(location, "Cannot `UnwindTerminate` from non-cleanup basic block")
    }
}
```

The Regseq pipeline pins `panic=abort` per the project README's
`#![no_std]` driver constraint. Combined with the Rust-only-body
case (the bridge processes `#[regseq]` items, which are
Rust-defined; `body_can_unwind` is determined by whether they
contain unwind-able terminators -- `Drop`, `Call` to unwinding
ABI, `Assert`, `InlineAsm` with `MAY_UNWIND`. Under
`PanicStrategy::Abort` even Rust-defined functions are treated as
non-unwinding via `layout::fn_can_unwind`
[`abort_unwinding_calls.rs:60-61`], so `body_can_unwind` is
typically false). HYPOTHESIS: under the pinned panic=abort
pipeline, every `UnwindResume` becomes `UnwindTerminate(Abi)` or
`Unreachable`. The latter is in the kept-6 set; the former is
*not*.

INFERENCE: `UnwindTerminate(Abi)` could still survive in
`optimized_mir` cleanup blocks. But cleanup blocks themselves
become unreachable after `AbortUnwindingCalls` strips the cleanup
edges in the body, and `super::simplify::remove_dead_blocks`
runs at the end of the pass [`abort_unwinding_calls.rs:138`].

`PanicStrategy::unwinds()` is defined at
[`rustc_target/src/spec/mod.rs:860-862`]:

```rust
pub fn unwinds(self) -> bool {
    matches!(self, PanicStrategy::Unwind)
}
```

So `panic=abort` -> `unwinds() = false` -> `body_can_unwind` is
false (modulo extern "C-unwind" cases) and cleanup blocks are
removed.

D-9-9 entry: `UnwindResume` / `UnwindTerminate` -> "discharged by
`AbortUnwindingCalls` (`is_required: true`) under `panic=abort`;
non-cleanup-block invariant means the cleanup blocks they live
in are dead-block-removed at the end of the pass; bridge rejects
with diagnostic if seen (defence-in-depth)."

### 3.4 `TailCall`

[`syntax.rs:850-862`]. The `become` lowering. Has `func`, `args`,
`fn_span` only -- no `target`, `destination`, `unwind` (those are
inherited from the caller's stack frame).

**Discharge.** No pass rewrites `TailCall` in `optimized_mir`.
`TailCall` is a normally-supported terminator in rustc:
codegen-SSA dispatches it
[`rustc_codegen_ssa/src/mir/block.rs:1536-1549`] by re-using
`codegen_call_terminator` with `CallKind::Tail`. The cranelift
backend `span_bug!`s on it
[`rustc_codegen_cranelift/src/base.rs:519-522`]:

```rust
TerminatorKind::TailCall { fn_span, .. } => span_bug!(
    *fn_span,
    "tail calls are not yet supported in `rustc_codegen_cranelift` backend"
),
```

So `TailCall` is reachable in `optimized_mir`. The bridge cannot
rely on a cleanup pass.

`TailCall` is gated at the language level by the unstable
`explicit_tail_calls` feature [`syntax.rs:858`'s FIXME comment
mentions `become` keyword]. Driver code under `#![no_std]` and
without `#![feature(explicit_tail_calls)]` cannot construct a
`TailCall`. INFERENCE: `TailCall` is reachable only via the
unstable feature, and Regseq driver specs do not opt into it.

D-9-9 entry: `TailCall` -> "no MIR pass discharges; reachable in
`optimized_mir` whenever `#![feature(explicit_tail_calls)]` is
set; bridge rejects with diagnostic, citing 'tail calls require
unstable feature gate not enabled in driver builds'."

### 3.5 `InlineAsm`

[`syntax.rs:957-986`]. The `asm!` / `naked_asm!` block.

**Discharge.** No pass discharges `InlineAsm`. It is a
fully-supported terminator in `optimized_mir` -- both
codegen-SSA and codegen-cranelift dispatch it
[`rustc_codegen_ssa/src/mir/block.rs:1557-1578`] and
[`rustc_codegen_cranelift/src/base.rs:523-559`].

Driver-domain rationale for rejection: the OS-portable C output
target cannot emit Rust inline assembly verbatim; mapping `asm!`
operand templates and register classes into Linux-kernel /
Zephyr-toolchain inline-asm syntax is a non-trivial backend
concern that the project explicitly defers. Rejection is at the
DSL level (specs declare register sequences, not asm blocks).

D-9-9 entry: `InlineAsm` -> "not pipeline-discharged; reachable
in `optimized_mir`; rejected at the DSL level (driver specs do
not contain `asm!`); bridge rejects with diagnostic."

### 3.6 `Drop`'s `replace` / `drop` / `async_fut` fields

[`syntax.rs:790-799`]. After elaboration the `Drop` terminator
is a *complete* runtime construct, but three of its six fields
are post-codegen-irrelevant under our pipeline:

- `replace: bool` -- doc text [`syntax.rs:769-771`]: "indicates
  whether this terminator was created as part of an assignment.
  This should only be used for diagnostic purposes, and does not
  have any operational meaning."
- `drop: Option<BasicBlock>` -- async-drop coroutine-drop
  yield-point target [`syntax.rs:783-787`]. Only consumed by
  `coroutine::StateTransform` `expand_async_drops`.
- `async_fut: Option<Local>` -- async drop future local
  [`syntax.rs:782`]. Only used in
  `coroutine::StateTransform`.

Both codegen consumers assert these are `None` post-State-Transform
[`rustc_codegen_ssa/src/mir/block.rs:1486-1490`]:

```rust
mir::TerminatorKind::Drop { place, target, unwind, replace: _, drop, async_fut } => {
    assert!(
        async_fut.is_none() && drop.is_none(),
        "Async Drop must be expanded or reset to sync before codegen"
    );
    ...
}
```

cranelift mirror identical
[`rustc_codegen_cranelift/src/base.rs:585-591`].

INFERENCE: in `optimized_mir`, `drop = None` and `async_fut =
None` post-StateTransform; `replace` is purely diagnostic. The
bridge can ignore all three. See §4.

D-9-9 entry: `Drop.replace` -> "diagnostic-only flag, no
operational meaning"; `Drop.drop` / `Drop.async_fut` -> "always
`None` post-StateTransform under `panic=abort` pipeline; codegen
consumers assert this."

---

## 4. `Drop` terminator post-elaboration semantics

### 4.1 Pre- vs. post-elaboration semantics

Doc text [`syntax.rs:753-757`]:

> The behavior of this statement differs significantly before and
> after drop elaboration.
>
> After drop elaboration: `Drop` terminators are a complete nop
> for types that have no drop glue. For other types, `Drop`
> terminators behave exactly like a call to
> `core::mem::drop_in_place` with a pointer to the given place.

Pre-elaboration `Drop` is *conditional* on dataflow-derived
"is-this-place-still-initialized" flags. `ElaborateDrops` is the
pass that resolves the conditionality
[`rustc_mir_transform/src/elaborate_drops.rs:50-94`]:

```rust
impl<'tcx> crate::MirPass<'tcx> for ElaborateDrops {
    #[instrument(level = "trace", skip(self, tcx, body))]
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        ...
    }
    fn is_required(&self) -> bool { true }
}
```

Runs in `run_runtime_lowering_passes` [`lib.rs:645`]. The
phase-transition note in `MirPhase::Runtime` doc
[`syntax.rs:70-75`] confirms: "In runtime MIR, the drops are
unconditional; when a `Drop` terminator is reached, if the type
has drop glue that drop glue is always executed."

`optimized_mir` is `Runtime(PostCleanup)` and therefore
post-elaboration. INFERENCE: every `Drop` the bridge sees is
unconditional drop-glue invocation.

### 4.2 Post-elaboration `Drop` is essentially a `Call` to drop glue

The semantics doc [`syntax.rs:756-757`] is explicit: "for other
types, `Drop` terminators behave exactly like a call to
`core::mem::drop_in_place` with a pointer to the given place."

Codegen-SSA confirms this by structure:
`codegen_drop_terminator` [`block.rs:599-693`] resolves
`Instance::resolve_drop_in_place(tcx, ty)` [`block.rs:608`] and
then calls `helper.do_call` [`block.rs:680-692`] -- the same
helper used by `Call`. The dynamic-dispatch case for `dyn Trait`
manufactures an `Instance { def: InstanceKind::Virtual(...) }`
and threads it through `do_call` [`block.rs:627-657`]:

```rust
ty::Dynamic(_, _) => {
    let virtual_drop = Instance {
        def: ty::InstanceKind::Virtual(drop_fn.def_id(), 0),
        args: drop_fn.args,
    };
    ...
    (true, ..., virtual_drop)
}
_ => (false, bx.get_fn_addr(drop_fn), ..., drop_fn),
```

Two cases:

1. **Concrete type with drop glue**: resolves to
   `InstanceKind::DropGlue(_, Some(ty))` (or the concrete
   `InstanceKind::Item` for user-`impl Drop`); codegen emits a
   direct call.
2. **`dyn Trait`**: resolves to `InstanceKind::Virtual(drop_fn,
   0)` (vtable index 0 is the drop-in-place entry), and codegen
   emits an indirect call via vtable lookup.

There's also a third sub-case: `DropGlue(_, None)` -- the empty
drop glue for types without destructors -- which codegen
short-circuits to a `Goto` to `target` [`block.rs:610-613`]:

```rust
if let ty::InstanceKind::DropGlue(_, None) = drop_fn.def {
    return helper.funclet_br(self, bx, target, mergeable_succ);
}
```

This is the "complete nop for types that have no drop glue" path
from the doc.

### 4.3 D-9-2: the bridge's Drop-as-Call collapse

D-9-2 framing: the bridge synthesizes a `Call` that targets the
resolved drop-glue Instance for `place.ty()`. Three sub-decisions
fall out:

1. **Resolve at bridge time.** The bridge runs
   `Instance::resolve_drop_in_place(tcx, place.ty())` (the same
   call codegen-SSA uses) at `optimized_mir` -> RegseqModel
   translation time. INFERENCE: this gives a typed Instance
   identifier the stage-1 closure already accepts (modulo
   `Virtual`, see §5.4 below).
2. **Empty-glue elision.** When `Instance.def =
   DropGlue(_, None)`, emit no Call -- just a `Goto` to `target`.
   Mirrors `block.rs:610-613`.
3. **Concrete vs. virtual dispatch.** For `place.ty() = ty::Adt(_)`
   etc., resolve to a concrete drop glue and emit a direct Call.
   For `place.ty() = ty::Dynamic(_, _)`, emit a vtable-indirect
   Call. The latter requires either `InstanceKind::Virtual` in
   stage 1 (D-9-3 candidate (a)) or `CallCallee::Indirect(Operand)`
   at the Call surface (candidate (b)).

INFERENCE: D-9-2 is enabled by D-9-3. Without a way to express
vtable dispatch in the resolved-callee position, the Drop-as-Call
collapse is incomplete because it cannot represent
`drop_in_place::<dyn Trait>`.

### 4.4 What about `unwind` on `Drop`?

`Drop.unwind: UnwindAction` [`syntax.rs:793`] is the cleanup
edge. Under `panic=abort`, `AbortUnwindingCalls`
[`abort_unwinding_calls.rs:100-103, 116-122`] strips it:

```rust
TerminatorKind::Drop { .. } => {
    tcx.sess.opts.unstable_opts.panic_in_drop == PanicStrategy::Unwind
        && layout::fn_can_unwind(tcx, None, ExternAbi::Rust)
}
...
if !call_can_unwind || !target_supports_unwinding {
    let cleanup = block.terminator_mut().unwind_mut().unwrap();
    *cleanup = UnwindAction::Unreachable;
}
```

`panic_in_drop` defaults to `PanicStrategy::Abort` (per session
defaults). Under `panic=abort`, the `Drop`'s unwind becomes
`UnwindAction::Unreachable` or is removed when the cleanup block
becomes unreachable. INFERENCE: the bridge can either drop the
`unwind` field entirely or reflect "Unreachable" as an
attribute on the synthesized Call -- since the kept-6 set does
not include `UnwindResume`/`UnwindTerminate`, no actual cleanup
edge is followed at runtime.

---

## 5. `Call` terminator anatomy

[`syntax.rs:811-832`]. The seven-field structure
(decomposed in the table below):

| Field | Type | Source | Purpose |
|-------|------|--------|---------|
| `func` | `Operand<'tcx>` | `syntax.rs:813` | The callable. Must be `ty::FnDef(..)` or `ty::FnPtr(..)` per validator [`validate.rs:1612-1623`]. |
| `args` | `Box<[Spanned<Operand<'tcx>>]>` | `syntax.rs:820` | Argument list with per-arg span. |
| `destination` | `Place<'tcx>` | `syntax.rs:822` | Where the return value is written. |
| `target` | `Option<BasicBlock>` | `syntax.rs:824` | Success-edge successor; `None` means the call diverges. |
| `unwind` | `UnwindAction` | `syntax.rs:826` | Unwind-edge action. |
| `call_source` | `CallSource` | `syntax.rs:828` | Diagnostic-only HIR origin. |
| `fn_span` | `Span` | `syntax.rs:831` | Span of the function (no receiver / dot). |

### 5.1 `func: Operand<'tcx>` and callee resolution

The callee is *not* a pre-resolved `Instance` at the MIR level --
it is an arbitrary `Operand` (Constant, Copy, or Move) whose type
must be `FnDef` or `FnPtr`. The validator type-check
[`validate.rs:1612-1623`]:

```rust
TerminatorKind::Call { func, .. } | TerminatorKind::TailCall { func, .. } => {
    let func_ty = func.ty(&self.body.local_decls, self.tcx);
    match func_ty.kind() {
        ty::FnPtr(..) | ty::FnDef(..) => {}
        _ => self.fail(
            location,
            format!(
                "encountered non-callable type {func_ty} in `{}` terminator",
                terminator.kind.name()
            ),
        ),
    }
    ...
}
```

Codegen-SSA performs the resolution via `Instance::expect_resolve`
when `func_ty.kind() == ty::FnDef(...)`
[`block.rs:913-921`]:

```rust
let (instance, mut llfn) = match *callee.layout.ty.kind() {
    ty::FnDef(def_id, generic_args) => {
        let instance = ty::Instance::expect_resolve(
            bx.tcx(),
            bx.typing_env(),
            def_id,
            generic_args,
            fn_span,
        );

        match instance.def {
            ty::InstanceKind::DropGlue(_, None) => { /* nop */ }
            ty::InstanceKind::Intrinsic(def_id) => { /* intrinsic dispatch */ }
            _ => { ... }
        }
    }
    _ => { /* FnPtr: indirect call via the operand value */ }
};
```

Two structural cases that drive D-9-3:

1. **`func.ty() == ty::FnDef(def_id, args)`.** Resolves to a
   concrete `Instance { def: InstanceKind::Item(_) | ... , args }`
   at codegen time. Direct call.
2. **`func.ty() == ty::FnPtr(_)`.** No def_id; the call is
   indirect via the loaded function pointer. The codegen path
   bypasses the `Instance::expect_resolve` branch entirely and
   uses the operand's value as the call target.

There is a third logical case relevant for the Drop-as-Call
collapse: vtable-virtual dispatch. In MIR Call, this manifests
when `func.ty() == ty::FnDef(def_id, args)` and `InstanceKind`
resolves to `Virtual(def_id, idx)` (typically because `def_id`
is a trait method and the receiver is `dyn Trait`). The codegen
path for this is at [`block.rs:1165-1211`]:

```rust
if let (0, Some(ty::InstanceKind::Virtual(_, idx))) = (i, instance.map(|i| i.def)) {
    match op.val {
        Pair(data_ptr, meta) => {
            ...
            llfn = Some(meth::VirtualIndex::from_index(idx).get_fn(...));
            llargs.push(data_ptr);
            continue 'make_args;
        }
        Ref(PlaceValue { llval: data_ptr, llextra: Some(meta), .. }) => {
            // by-value dynamic dispatch
            llfn = Some(meth::VirtualIndex::from_index(idx).get_fn(...));
            llargs.push(data_ptr);
            continue;
        }
        _ => { span_bug!(fn_span, "can't codegen a virtual call on {:#?}", op); }
    }
}
```

The first argument's value is unwrapped to `(data_ptr, meta)`,
the meta is used as a vtable to look up the function pointer at
the indexed slot, and the call becomes effectively indirect.

### 5.2 D-9-3: closure of the callee shape

Two candidate shapes:

**(a) Extend stage-1 `InstanceKind` with a `Virtual` variant.**
This mirrors rustc's own representation: `InstanceKind::Virtual(DefId,
usize)` [`rustc_middle/src/ty/instance.rs:111-119`]:

```rust
/// Dynamic dispatch to `<dyn Trait as Trait>::fn`.
///
/// This `InstanceKind` may have a callable MIR as the default implementation.
/// Calls to `Virtual` instances must be codegen'd as virtual calls through the vtable.
/// *This means we might not know exactly what is being called.*
Virtual(DefId, usize),
```

Stage-1 already closed `InstanceKind`; adding `Virtual` is a
single-variant extension with the same shape rustc uses. The
backend lowering for the C target then needs a vtable
representation -- which is a separate stage 14 concern but is
necessary for `dyn Trait` support generally.

**(b) Widen `Call.callee` to `CallCallee::{Direct(InstanceId),
Indirect(Operand)}`.** The Indirect arm carries the raw operand
(matching rustc's "FnPtr" path). Vtable dispatch is then a
*kind* of Indirect call: the bridge would synthesize the operand
that loads the function pointer from the vtable.

Trade-off:

- (a) is faithful to rustc's instance-resolution model: the
  bridge runs `Instance::expect_resolve` and stores the result.
  Fewer layers, more semantic content per Call. The Drop-as-Call
  collapse (D-9-2) becomes natural because drop glue resolution
  also produces an `InstanceKind::Virtual` for `dyn Trait` (see
  §4.2).
- (b) defers vtable concerns to stage-14 backends but loses
  the symmetric Drop-as-Call story: indirect Drop would carry an
  Operand that stage-14 backends would have to manufacture from
  the place's vtable, duplicating the resolution rustc does
  internally.

INFERENCE: (a) is the cleaner closure for the bridge's
"information-preservation" mandate. The bridge runs the same
resolution rustc's codegen runs, stores the result.

### 5.3 `args: Box<[Spanned<Operand<'tcx>>]>` and D-9-4

`Spanned<T>` is defined in
[`rustc_span/src/source_map.rs:77-81`]:

```rust
#[derive(Clone, Encodable, Decodable, Debug, Copy, PartialEq, Hash, HashStable_Generic)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}
```

Two-field struct: `node` is the actual operand, `span` is the
HIR-derived span (e.g., `a` and `b` in `x.foo(a, b)` per the
doc at [`syntax.rs:818-819`]).

The visitor walk
[`rustc_middle/src/mir/visit.rs:586-605`] reads
`arg.node` and ignores `arg.span`:

```rust
TerminatorKind::Call { func, args, destination, ... } => {
    self.visit_span($(& $mutability)? *fn_span);
    self.visit_operand(func, location);
    for arg in args {
        self.visit_operand(&$($mutability)? arg.node, location);
    }
    self.visit_place(...);
}
```

The validator never reads `arg.span` -- it uses `fn_span` or
`SourceInfo::span` for diagnostics
[`validate.rs:384-441`].

INFERENCE: `Spanned<Operand>` is structurally a 2-tuple of
`(Operand, Span)`. The two D-9-4 candidates:

**(a) Inline `ArgWithSpan { node: Operand, span: Span }` per arg.**
Mirrors rustc directly. Each Call carries `Vec<ArgWithSpan>` (or
the boxed-slice equivalent).

**(b) Parallel slabs.** `args: Vec<Operand>` plus `arg_spans:
Vec<Span>`, length-matched. Saves one struct layer.

Trade-off:

- (a) is semantically tighter -- the per-arg pair is the natural
  unit, and it cannot get out of sync with itself.
- (b) is more memory-efficient if Spans are 8-byte and Operands
  are larger (`Operand<'tcx>` is not size-asserted in rustc but
  it carries a `Place` + `Constant` enum -- non-trivial). Two
  separate vectors avoid struct padding per element.

HYPOTHESIS: rustc chose (a) because the boxed-slice
`Box<[Spanned<Operand>]>` is the natural payload for
`type_foldable` derivation -- folding a `Spanned<T>` is mechanical,
folding two parallel slices isn't. RegseqModel doesn't need
TypeFoldable semantics, so the trade is purely about API
ergonomics. INFERENCE: (a) parallels rustc, but neither choice
is load-bearing for downstream stages.

### 5.4 `destination: Place<'tcx>`

The place to which the return value is written. Always present
(unlike `target` which is `Option`). Validator checks
`destination` is not behind a packed-projection
[`validate.rs:412-423`]:

```rust
if most_packed_projection(self.tcx, &self.body.local_decls, destination).is_some() {
    self.fail(
        location,
        format!(
            "encountered packed place in `Call` terminator destination: {:?}",
            terminator.kind,
        ),
    );
}
```

D-9-1.5 framing: RegseqModel's Call.destination is just stage-5
`Place`. No structural choice.

### 5.5 `target: Option<BasicBlock>`

`None` means the callee diverges (the call necessarily does not
return). Common case: calls to `panic`, `core::intrinsics::abort`,
etc.

Validator: when `target` is `Some` and the body phase is
`Runtime(Optimized)+`, the edge must not be a critical edge
[`validate.rs:398-408`]. RegseqModel doesn't run optimization
passes so the critical-edge invariant is not enforced -- but the
input MIR has had `CriticalCallEdges` run on it
[`lib.rs:640`] so the invariant holds anyway.

### 5.6 `unwind: UnwindAction` (under panic=abort)

See §3.3 + §8. Under panic=abort, after `AbortUnwindingCalls`,
all live Call terminators have `unwind = Unreachable` (for
non-unwinding callees, the common case) or `unwind = Continue`
(for `extern "C-unwind"` callees in unwinding bodies, but
non-Rust-defined-bodies unwinding is suppressed too unless
`body_can_unwind`). INFERENCE: the bridge can store `unwind`
verbatim or fold it away under the panic=abort assumption; the
former is "preserve information" and matches the project's
"C-shape with information preservation" mandate.

### 5.7 `call_source: CallSource`

[`syntax.rs:642-660`]. 5 variants:

```rust
pub enum CallSource {
    OverloadedOperator,  // a > b, a + b
    MatchCmp,            // comparison generated by match
    Misc,                // other non-HIR desugaring
    Use,                 // use of value, generating a clone call
    Normal,              // normal function call
}
```

Doc: "Used only for diagnostics" [`syntax.rs:641`]. The accessor
`from_hir_call` [`terminator.rs:844-848`]:

```rust
pub fn from_hir_call(self) -> bool {
    matches!(self, CallSource::Normal)
}
```

INFERENCE: not load-bearing for codegen. The bridge can drop
`call_source` without affecting C-output behavior.

### 5.8 `fn_span: Span`

Per the doc [`syntax.rs:829-831`], the span of the function name
itself (excluding the receiver / dot). Distinct from
`SourceInfo::span` which is the call expression's span. Codegen
uses `fn_span` to thread location for caller-location intrinsics
[`block.rs:1245-1258`].

INFERENCE: RegseqModel can preserve `fn_span` for stage-14
diagnostic purposes or fold it into a single span on the
synthesized Call statement; not load-bearing for behavior.

---

## 6. `Assert` terminator anatomy

[`syntax.rs:875-881`]. Five fields:

```rust
Assert {
    cond: Operand<'tcx>,
    expected: bool,
    msg: Box<AssertMessage<'tcx>>,
    target: BasicBlock,
    unwind: UnwindAction,
}
```

Semantics [`syntax.rs:864-874`]:

> Evaluates the operand, which must have type `bool`. If it is
> not equal to `expected`, initiates a panic. Initiating a panic
> corresponds to a `Call` terminator with some unspecified
> constant as the function to call, all the operands stored in
> the `AssertMessage` as parameters, and `None` for the
> destination. Keep in mind that the `cleanup` path is not
> necessarily executed even in the case of a panic, for example
> in `-C panic=abort`.

The doc explicitly notes [`syntax.rs:870-874`] that under
runtime MIR with overflow checks disabled, certain Assert
variants degenerate to `goto target`. This is implemented at
codegen time, not in a MIR pass:
[`rustc_codegen_cranelift/src/base.rs:368-372`]:

```rust
TerminatorKind::Assert { cond, expected, msg, target, unwind } => {
    if !fx.tcx.sess.overflow_checks() && msg.is_optional_overflow_check() {
        let target = fx.get_block(*target);
        fx.bcx.ins().jump(target, &[]);
        continue;
    }
    ...
}
```

`is_optional_overflow_check`
[`rustc_middle/src/mir/terminator.rs:163-168`]:

```rust
pub fn is_optional_overflow_check(&self) -> bool {
    use AssertKind::*;
    use BinOp::*;
    matches!(self, OverflowNeg(..) | Overflow(Add | Sub | Mul | Shl | Shr, ..))
}
```

So `Overflow(Div, ..)`, `Overflow(Rem, ..)`, `DivisionByZero(..)`,
`RemainderByZero(..)` are *non-optional* checks: they cannot be
suppressed by `-C overflow-checks=no` because their failure is
UB.

### 6.1 `cond: Operand<'tcx>` and `expected: bool`

Validator type-check [`validate.rs:1630-1639`]:

```rust
TerminatorKind::Assert { cond, .. } => {
    let cond_ty = cond.ty(&self.body.local_decls, self.tcx);
    if cond_ty != self.tcx.types.bool {
        self.fail(
            location,
            format!(
                "encountered non-boolean condition of type {cond_ty} in `Assert` terminator"
            ),
        );
    }
}
```

`expected` is the value `cond` must equal for the success edge
to be taken. Typically `true` for "assert that X holds" and
`false` for "assert that X is not the failure case." The
combination `(cond, expected)` lets MIR builders avoid emitting
explicit `Not` rvalues when the natural test polarity is
inverted -- e.g., `assert(is_aligned == true)` vs.
`assert(is_misaligned == false)`.

### 6.2 `AssertKind<O>` (`AssertMessage<'tcx> = AssertKind<Operand<'tcx>>`)

[`syntax.rs:1066-1078, 1113-1114`]:

```rust
pub enum AssertKind<O> {
    BoundsCheck { len: O, index: O },
    Overflow(BinOp, O, O),
    OverflowNeg(O),
    DivisionByZero(O),
    RemainderByZero(O),
    ResumedAfterReturn(CoroutineKind),
    ResumedAfterPanic(CoroutineKind),
    ResumedAfterDrop(CoroutineKind),
    MisalignedPointerDereference { required: O, found: O },
    NullPointerDereference,
    InvalidEnumConstruction(O),
}

pub type AssertMessage<'tcx> = AssertKind<Operand<'tcx>>;
```

11 variants. Three are coroutine-related and the others are
driver-relevant.

### 6.3 D-9-5: the keep-8 / drop-3 split

**Coroutine-related (drop):**

- `ResumedAfterReturn(CoroutineKind)`
- `ResumedAfterPanic(CoroutineKind)`
- `ResumedAfterDrop(CoroutineKind)`

These exist for the runtime check that fires when a coroutine is
resumed in an invalid state. They are emitted only by coroutine
lowering code (the lang-item table at
[`terminator.rs:190-220`] dispatches each to a coroutine-state
panic lang item).

INFERENCE: stage 3 already rejects `ty::Coroutine` /
`ty::CoroutineWitness`, so non-coroutine bodies cannot contain
these AssertKinds. They are unreachable in `optimized_mir` for
RegseqModel input.

**Driver-relevant (keep):**

| AssertKind | Source | Emitter |
|------------|--------|---------|
| `BoundsCheck { len, index }` | `syntax.rs:1067` | `rustc_mir_build/src/builder/expr/as_place.rs:9` (array/slice indexing) |
| `Overflow(BinOp, l, r)` | `syntax.rs:1068` | `rustc_mir_build/src/builder/expr/as_rvalue.rs:529, 579, 592` (checked arithmetic) |
| `OverflowNeg(o)` | `syntax.rs:1069` | `rustc_mir_build/src/builder/expr/as_rvalue.rs:121` (checked negation) |
| `DivisionByZero(o)` | `syntax.rs:1070` | `rustc_mir_build/src/builder/expr/as_rvalue.rs:588` (checked div) |
| `RemainderByZero(o)` | `syntax.rs:1071` | `rustc_mir_build/src/builder/expr/as_rvalue.rs:590` (checked rem) |
| `MisalignedPointerDereference { required, found }` | `syntax.rs:1075` | `rustc_mir_transform/src/check_alignment.rs:143-146` |
| `NullPointerDereference` | `syntax.rs:1076` | `rustc_mir_transform/src/check_null.rs:125` |
| `InvalidEnumConstruction(o)` | `syntax.rs:1077` | `rustc_mir_transform/src/check_enums.rs:411, 447, 530` |

Three of these are emitted by passes in the Regseq cleanup-pass
whitelist (`CheckAlignment`, `CheckNull`, `CheckEnums`), and the
others come from MIR construction in `rustc_mir_build`.

### 6.4 The CheckAlignment / CheckNull / CheckEnums passes

`CheckAlignment::is_enabled`
[`rustc_mir_transform/src/check_alignment.rs:15-17`]:

```rust
fn is_enabled(&self, sess: &Session) -> bool {
    sess.ub_checks()
}
```

`CheckNull::is_enabled` identical
[`check_null.rs:13-15`]. `CheckEnums::is_enabled`
[`check_enums.rs:18-20`] also same.

`is_required: true` for all three
[`check_alignment.rs:35-37`, `check_null.rs:27-29`,
`check_enums.rs:111-113`]. These passes run in
`run_optimization_passes` [`lib.rs:709-711`]:

```rust
pm::run_passes(
    tcx,
    body,
    &[
        // Add some UB checks before any UB gets optimized away.
        &check_alignment::CheckAlignment,
        &check_null::CheckNull,
        &check_enums::CheckEnums,
        ...
    ],
    ...
);
```

So under `mir_opt_level >= 1` and `ub_checks` enabled (the
defaults under `-O0`), all three Asserts can appear in
`optimized_mir`. The Regseq pipeline whitelists these passes,
matching the rustc default.

### 6.5 `target` / `unwind`

Same shape as Call.target, Call.unwind. Validator
[`validate.rs:450-453`] checks both edges. Under panic=abort,
`unwind` is reduced to `Unreachable` or removed by
`AbortUnwindingCalls` [`abort_unwinding_calls.rs:104-106`]:

```rust
TerminatorKind::Assert { .. } | TerminatorKind::FalseUnwind { .. } => {
    layout::fn_can_unwind(tcx, None, ExternAbi::Rust)
}
```

So `Assert.unwind` under panic=abort is similarly degenerate.

---

## 7. `SwitchInt` and `SwitchTargets`

### 7.1 The terminator [`syntax.rs:715-719`]

```rust
SwitchInt {
    discr: Operand<'tcx>,
    targets: SwitchTargets,
}
```

Doc [`syntax.rs:709-714`]: "First, evaluates the `discr` operand.
The type of the operand must be a signed or unsigned integer,
char, or bool, and must match the given type. Then, if the list
of switch targets contains the computed value, continues
execution at the associated basic block. Otherwise, continues
execution at the 'otherwise' basic block. Target values may not
appear more than once."

Validator type-check [`validate.rs:1590-1610`]: `discr` must be
`Uint`, `Int`, `Char`, or `Bool`, and the values must fit in the
discriminant's bit width:

```rust
TerminatorKind::SwitchInt { targets, discr } => {
    let switch_ty = discr.ty(&self.body.local_decls, self.tcx);
    let target_width = self.tcx.sess.target.pointer_width;
    let size = Size::from_bits(match switch_ty.kind() {
        ty::Uint(uint) => uint.normalize(target_width).bit_width().unwrap(),
        ty::Int(int) => int.normalize(target_width).bit_width().unwrap(),
        ty::Char => 32,
        ty::Bool => 1,
        other => bug!("unhandled type: {:?}", other),
    });
    for (value, _) in targets.iter() {
        if ScalarInt::try_from_uint(value, size).is_none() {
            self.fail(
                location,
                format!("the value {value:#x} is not a proper {switch_ty}"),
            )
        }
    }
}
```

Validator dedup [`validate.rs:358-376`]:

```rust
self.value_cache.clear();
self.value_cache.extend(targets.iter().map(|(value, _)| value));
let has_duplicates = targets.iter().len() != self.value_cache.len();
if has_duplicates {
    self.fail(location, format!("duplicated values in `SwitchInt` terminator: {:?}", terminator.kind));
}
```

D-9-1.3 framing: SwitchInt payload is `(Operand, SwitchTargets)`.

### 7.2 `SwitchTargets` [`syntax.rs:1004-1031`]

```rust
pub struct SwitchTargets {
    pub(super) values: SmallVec<[Pu128; 1]>,
    pub(super) targets: SmallVec<[BasicBlock; 2]>,
}
```

Critical invariant from the source comment
[`syntax.rs:1010-1012`]:

> Possible branch targets. The last element of this vector is
> used for the "otherwise" branch, so `targets.len() ==
> values.len() + 1` always holds.

The comment also explains why this asymmetric layout was chosen
[`syntax.rs:1014-1031`]:

> Note: This invariant is non-obvious and easy to violate. This
> would be a more rigorous representation:
>
> ```
>   normal: SmallVec<[(Pu128, BasicBlock); 1]>,
>   otherwise: BasicBlock,
> ```
>
> But it's important to have the targets in a sliceable type,
> because target slices show up elsewhere. E.g.
> `TerminatorKind::InlineAsm` has a boxed slice, and
> `TerminatorKind::FalseEdge` has a single target that can be
> converted to a slice with `slice::from_ref`.
>
> Why does this matter? In functions like
> `TerminatorKind::successors` we return `impl Iterator` and a
> non-slice-of-targets representation here causes problems
> because multiple different concrete iterator types would be
> involved and we would need a boxed trait object, which
> requires an allocation, which is expensive if done frequently.

INFERENCE: rustc's choice is driven by `successors()` ergonomics
(unifying the iterator type across terminator variants).
RegseqModel's parallel question -- "store as `Vec<(u128,
BasicBlock)>` plus an `otherwise: BasicBlock`, or as the
asymmetric flat slabs?" -- has a different tradeoff because
RegseqModel has no equivalent of `Successors<'_>` in its
public surface (the bridge consumes, doesn't iterate).

`Pu128` [`rustc_data_structures/src/packed.rs:8-12`]:

```rust
/// A packed 128-bit integer. Useful for reducing the size of structures in
/// some cases.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(packed(8))]
pub struct Pu128(pub u128);
```

8-byte-packed `u128` to avoid 16-byte alignment-induced padding
in containing structs. INFERENCE: this is purely a size
optimization; RegseqModel can use `u128` directly without the
packing concern.

`SwitchTargets` accessors:

- `new(targets, otherwise)` -- constructor
  [`terminator.rs:18-23`].
- `static_if(value, then, else_)` -- 2-target builder
  [`terminator.rs:27-29`].
- `as_static_if()` -- inverse of `static_if`
  [`terminator.rs:33-41`]. Returns `Some((value, then, else_))`
  iff the SwitchTargets is exactly a 2-target if-then-else.
- `otherwise()` -- the fallback target [`terminator.rs:45-47`].
- `iter()` -> `SwitchTargetsIter` [`terminator.rs:56-58`] --
  yields `(u128, BasicBlock)` pairs *excluding* otherwise.
- `all_targets()` / `all_values()` -- raw slabs
  [`terminator.rs:62-80`].
- `target_for_value(value)` -- exhaustive lookup with otherwise
  fallback [`terminator.rs:86-88`].
- `add_target(value, bb)` -- panics on duplicate value
  [`terminator.rs:92-99`].
- `is_distinct()` -- predicates that all targets (including
  fallback) are distinct [`terminator.rs:103-105`].

### 7.3 D-9-6 framing

Three viable RegseqModel storage shapes:

**(a) Mirror rustc:** `values: Vec<u128>`, `targets:
Vec<BasicBlockId>`, with the "last targets entry is otherwise"
invariant.

**(b) Symmetric pairs + otherwise:** `cases: Vec<(u128,
BasicBlockId)>`, `otherwise: BasicBlockId`.

**(c) Single map:** `cases: BTreeMap<u128, BasicBlockId>` plus
`otherwise: BasicBlockId`. Loses iteration order, gains O(log n)
lookup.

(b) is the shape rustc explicitly considered and rejected for
ergonomic reasons that don't apply to RegseqModel (no unified
`Successors` iterator). INFERENCE: (b) is the cleanest
RegseqModel choice; the asymmetric (a) pays a complexity tax for
no gain. (c) is a stage-13/14 lookup-side concern, not a
storage-side concern.

---

## 8. `UnwindAction` and panic=abort interaction

[`syntax.rs:1033-1050`]. Four variants:

```rust
pub enum UnwindAction {
    /// No action is to be taken. Continue unwinding.
    Continue,
    /// Triggers undefined behavior if unwind happens.
    Unreachable,
    /// Terminates the execution if unwind happens.
    Terminate(UnwindTerminateReason),
    /// Cleanups to be done.
    Cleanup(BasicBlock),
}
```

Carried by 5 terminators: `Call`, `Assert`, `Drop`,
`FalseUnwind`, `InlineAsm`
[`terminator.rs:642-660`].

`UnwindTerminateReason` [`syntax.rs:1052-1061`]: `Abi` |
`InCleanup`. `as_str` / `lang_item` mappings at
[`terminator.rs:137-159`].

### 8.1 What survives under panic=abort

`PanicStrategy::unwinds()`
[`rustc_target/src/spec/mod.rs:860-862`] returns false for
`PanicStrategy::Abort`. Combined with
`AbortUnwindingCalls`'s logic [§3.3]:

1. **`UnwindAction::Continue`** in non-cleanup blocks
   becomes `UnwindAction::Terminate(Abi)` if the body cannot
   unwind [`abort_unwinding_calls.rs:123-134`]:

   ```rust
   } else if !body_can_unwind
       && matches!(terminator.unwind(), Some(UnwindAction::Continue))
   {
       let cleanup = block.terminator_mut().unwind_mut().unwrap();
       *cleanup = UnwindAction::Terminate(UnwindTerminateReason::Abi);
   }
   ```

2. **`UnwindAction::Cleanup(bb)`** is replaced with
   `Unreachable` if the call cannot unwind or the target does not
   support unwinding [`abort_unwinding_calls.rs:116-122`]:

   ```rust
   if !call_can_unwind || !target_supports_unwinding {
       let cleanup = block.terminator_mut().unwind_mut().unwrap();
       *cleanup = UnwindAction::Unreachable;
   }
   ```

3. **`UnwindAction::Unreachable`** stays.
4. **`UnwindAction::Terminate(_)`** stays.

`super::simplify::remove_dead_blocks` runs at the end
[`abort_unwinding_calls.rs:138`]; cleanup blocks reachable only
from the now-stripped `Cleanup(_)` arrows become unreachable and
are removed.

INFERENCE: under panic=abort, what survives in `optimized_mir`
is predominantly `UnwindAction::Unreachable` on each unwind-bearing
terminator, with `Terminate(Abi)` as a less-common case. The
bridge can either:

- Drop `unwind` field entirely (it's never followed at runtime
  under panic=abort).
- Preserve verbatim per the "information preservation" mandate.

D-9-9 connection: rejecting `UnwindResume` / `UnwindTerminate`
*as terminators* is consistent with this picture -- the UnwindAction
*field* values (specifically `Terminate(_)`) can survive on Call
/ Drop / Assert, but the *terminator* `UnwindTerminate` lives
only in cleanup blocks which are dead-block-eliminated.

---

## 9. `InlineAsm` terminator

[`syntax.rs:957-986`]. Seven fields:

```rust
InlineAsm {
    asm_macro: InlineAsmMacro,
    template: &'tcx [InlineAsmTemplatePiece],
    operands: Box<[InlineAsmOperand<'tcx>]>,
    options: InlineAsmOptions,
    line_spans: &'tcx [Span],
    targets: Box<[BasicBlock]>,
    unwind: UnwindAction,
}
```

### 9.1 `asm_macro` and the `naked_asm` distinction

[`syntax.rs:662-670`]:

```rust
pub enum InlineAsmMacro {
    Asm,
    NakedAsm,
}
```

`InlineAsmMacro::diverges` [`terminator.rs:850-857`]:

```rust
pub const fn diverges(self, options: InlineAsmOptions) -> bool {
    match self {
        InlineAsmMacro::Asm => options.contains(InlineAsmOptions::NORETURN),
        InlineAsmMacro::NakedAsm => true,
    }
}
```

`naked_asm!` always diverges. `asm!` diverges only when
`NORETURN` is set.

### 9.2 `targets: Box<[BasicBlock]>` semantics

Per the doc [`syntax.rs:978-981`]:

> Valid targets for the inline assembly. The first element is
> the fallthrough destination, unless `asm_macro ==
> InlineAsmMacro::NakedAsm` or `InlineAsmOptions::NORETURN` is
> set.

So `targets[0]` is the fallthrough; `targets[1..]` are labeled
exits (jumped to via `InlineAsmOperand::Label { target_index }`,
which is an index into `targets`
[`syntax.rs:1107-1110`]).

### 9.3 `MAY_UNWIND` option

[`rustc_ast/src/ast.rs:2703`]:

```rust
const MAY_UNWIND      = 1 << 8;
```

The `unwind: UnwindAction` field is "present if and only if
`InlineAsmOptions::MAY_UNWIND` is set" per the doc
[`syntax.rs:983-985`]. Cranelift fatal-errors on this combination
[`rustc_codegen_cranelift/src/base.rs:532-537`]:

```rust
if options.contains(InlineAsmOptions::MAY_UNWIND) {
    fx.tcx.dcx().span_fatal(
        source_info.span,
        "cranelift doesn't support unwinding from inline assembly.",
    );
}
```

### 9.4 D-9-9 rejection at the DSL level

InlineAsm is reachable in `optimized_mir` (no MIR pass
discharges it). Stage-9 D-9-9 framing: the DSL itself does not
emit `asm!` -- driver specs declare register-sequence operations
through Regseq DSL primitives, not raw inline assembly. The
bridge rejects `InlineAsm` at the `optimized_mir` ingestion
level with a "DSL does not permit inline assembly" diagnostic.

INFERENCE: this is *not* a cleanup-pass-discharge story (no
pipeline guarantee), it is a *DSL invariant* story. The bridge
rejects `InlineAsm` because the input language guarantees its
absence, not because the pipeline removed it.

---

## 10. Validator and walk-pass references

### 10.1 Validator (D-9-10 invariants registry)

`rustc_mir_transform/src/validate.rs` validates MIR at every
phase transition. Two relevant entry points:

(a) **`visit_terminator` (general)** at
[`validate.rs:353-528`] -- per-variant edge / cleanup / phase
checks. Already cited extensively above. Notable entries:

- `Goto`: edge check [`validate.rs:355-357`].
- `SwitchInt`: edge checks for all targets + dedup
  [`validate.rs:358-376`].
- `Drop`: edge check + unwind-edge check + drop-edge check (if
  `drop` is `Some`) [`validate.rs:377-383`].
- `Call` / `TailCall`: edge check, unwind-edge check,
  critical-call-edge check (Optimized+), packed-place check
  for destination and Move args, force-inline-not-inlined check
  [`validate.rs:384-449`].
- `Assert`: edge check, unwind-edge check
  [`validate.rs:450-453`].
- `Yield`: coroutine-only + Runtime(Initial)+ rejected
  [`validate.rs:454-465`].
- `FalseEdge`: Runtime(Initial)+ rejected
  [`validate.rs:466-475`].
- `FalseUnwind`: Runtime(Initial)+ rejected
  [`validate.rs:476-485`].
- `InlineAsm`: edge checks for all targets + unwind-edge
  [`validate.rs:486-491`].
- `CoroutineDrop`: coroutine-only + Runtime(Initial)+ rejected
  [`validate.rs:492-501`].
- `UnwindResume`: cleanup-blocks-only + can_unwind-only
  [`validate.rs:503-511`].
- `UnwindTerminate`: cleanup-blocks-only
  [`validate.rs:512-517`].
- `Return`: not-in-cleanup-blocks [`validate.rs:518-523`].
- `Unreachable`: no checks [`validate.rs:524`].

(b) **`visit_terminator` (type-aware second pass)** at
[`validate.rs:1588-1655`] -- type-checks operand types:

- `SwitchInt`: discriminant size match for all values
  [`validate.rs:1590-1610`].
- `Call`/`TailCall`: callee type must be `FnDef` or `FnPtr`;
  TailCall-specific FIXME [`validate.rs:1612-1628`].
- `Assert`: cond type must be `bool`
  [`validate.rs:1630-1639`].
- All others: no type-shape check [`validate.rs:1641-1651`].

D-9-10 framing: the RegseqModel invariants registry mirrors a
subset of these, scoped to the kept-6 set:

- `Goto.target` resolves to a valid `BasicBlockId` in the body.
- `SwitchInt.discr.ty()` is one of (signed int, unsigned int,
  char, bool); values fit in the discriminant width; values
  unique; targets are valid BasicBlockIds; "otherwise" target
  is a valid BasicBlockId.
- `Return` is not in a cleanup block (under our pipeline,
  cleanup blocks are dead-block-eliminated, so this is
  defence-in-depth).
- `Unreachable` has no payload.
- `Call.func.ty()` is `FnDef` or `FnPtr`; `Call.target` (when
  Some) is a valid BasicBlockId; `Call.destination` is not
  packed-projected; `Call.args[i].node` Move places are not
  packed-projected.
- `Assert.cond.ty() == bool`; `Assert.target` is a valid
  BasicBlockId.

### 10.2 Visitor walk pattern

`rustc_middle/src/mir/visit.rs` defines `super_terminator`
[`visit.rs:526-671`] -- the canonical "walk every operand /
place / span" visitor. The per-variant pattern is the
load-bearing reference for stage-10 (basic block traversal) and
stage-13 (analyses):

- `Goto`, `UnwindResume`, `UnwindTerminate`, `CoroutineDrop`,
  `Unreachable`, `FalseEdge`, `FalseUnwind`: no operand walk
  [`visit.rs:535-541`].
- `Return`: walks `RETURN_PLACE` as a Move use
  [`visit.rs:543-558`].
- `SwitchInt`: walks `discr` only [`visit.rs:560-562`].
- `Drop`: walks `place` (as a Drop use) and `async_fut` if
  present [`visit.rs:564-584`].
- `Call`: walks `fn_span`, `func`, `args[i].node`,
  `destination` [`visit.rs:586-605`].
- `TailCall`: walks `fn_span`, `func`, `args[i].node`
  [`visit.rs:607-613`].
- `Assert`: walks `cond` and `msg` (via
  `visit_assert_message`) [`visit.rs:615-618`].
- `Yield`: walks `value` and `resume_arg` [`visit.rs:620-627`].
- `InlineAsm`: walks all `operands`
  [`visit.rs:629-669`].

INFERENCE: stage-10 / stage-13 walks over RegseqModel
terminators can mirror this dispatch. D-9-K (`render_terminator`
deferral to stage 11) means we are not yet specifying the
visitor / printer surface, but the rustc pattern is the
reference shape.

---

## 11. Codegen consumer pattern

The per-Terminator dispatch in `rustc_codegen_ssa/src/mir/block.rs`
is the canonical "what does each variant become?" reference. The
top-level dispatch [`block.rs:1455-1579`]:

```rust
self.set_debug_loc(bx, terminator.source_info);
match terminator.kind {
    mir::TerminatorKind::UnwindResume => { ... bug! variant absent ... }
    mir::TerminatorKind::UnwindTerminate(reason) => { ... }
    mir::TerminatorKind::Goto { target } => { ... helper.funclet_br ... }
    mir::TerminatorKind::SwitchInt { ref discr, ref targets } => { ... }
    mir::TerminatorKind::Return => { ... codegen_return_terminator ... }
    mir::TerminatorKind::Unreachable => { bx.unreachable(); ... }
    mir::TerminatorKind::Drop { place, target, unwind, replace: _, drop, async_fut } => {
        assert!(async_fut.is_none() && drop.is_none(), ...);
        self.codegen_drop_terminator(...)
    }
    mir::TerminatorKind::Assert { ref cond, expected, ref msg, target, unwind } => { ... }
    mir::TerminatorKind::Call { ref func, ref args, destination, target, unwind, call_source: _, fn_span } => {
        self.codegen_call_terminator(..., CallKind::Normal, ...)
    }
    mir::TerminatorKind::TailCall { ref func, ref args, fn_span } => {
        self.codegen_call_terminator(
            ...
            mir::Place::from(mir::RETURN_PLACE),     // synthetic destination
            None,                                     // no target
            mir::UnwindAction::Unreachable,           // no unwind
            ...
            CallKind::Tail,
            ...
        )
    }
    mir::TerminatorKind::CoroutineDrop | mir::TerminatorKind::Yield { .. } => {
        bug!("coroutine ops in codegen")
    }
    mir::TerminatorKind::FalseEdge { .. } | mir::TerminatorKind::FalseUnwind { .. } => {
        bug!("borrowck false edges in codegen")
    }
    mir::TerminatorKind::InlineAsm { ... } => self.codegen_asm_terminator(...),
}
```

Two structural takeaways:

1. **Rejections via `bug!`** [`block.rs:1550-1555`]: the
   downstream consumer treats `CoroutineDrop`, `Yield`,
   `FalseEdge`, `FalseUnwind` as ICE conditions, on the
   assumption that the cleanup/lowering pipeline discharged
   them. RegseqModel takes the same posture: bridge rejects with
   diagnostic if these reach the bridge (defence-in-depth).
2. **Drop and TailCall both reuse the Call codegen path.**
   Drop becomes a direct or virtual Call to drop glue
   (`codegen_drop_terminator` -> `helper.do_call`). TailCall
   reuses `codegen_call_terminator` with synthetic
   destination / no target / hardcoded `Unreachable` unwind
   [`block.rs:1536-1549`]. INFERENCE: the codegen-SSA layer is
   the precedent for D-9-2 (Drop-as-Call) -- rustc itself
   collapses Drop into a Call structurally.

The cranelift mirror at
[`rustc_codegen_cranelift/src/base.rs:342-593`] follows the same
pattern with two differences:

- `TailCall` is a span_bug [`base.rs:519-522`]
  ("not yet supported in `rustc_codegen_cranelift` backend").
- `InlineAsm` with `MAY_UNWIND` is a span_fatal
  [`base.rs:532-537`].

D-9-9 connection: the codegen-side `bug!` placements at
[`block.rs:1550-1555`] are the precedent for stage 9's
rejection table. The bridge's per-variant dispatch should
"reject with diagnostic" for the 8 dropped variants, "translate"
for the 6 kept variants, and "synthesize a Call" for the 1
collapsed variant (Drop).

---

## 12. Pretty-printing references

`rustc_middle/src/mir/pretty.rs` carries the canonical Terminator
rendering. The `Debug` impl
[`pretty.rs:872-917`]:

```rust
impl<'tcx> Debug for TerminatorKind<'tcx> {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        self.fmt_head(fmt)?;
        let successor_count = self.successors().count();
        let labels = self.fmt_successor_labels();
        ...
        match (successor_count, show_unwind) {
            (0, false) => Ok(()),
            (0, true) => { write!(fmt, " -> ")?; fmt_unwind(fmt) }
            (1, false) => write!(fmt, " -> {:?}", self.successors().next().unwrap()),
            _ => {
                write!(fmt, " -> [")?;
                for (i, target) in self.successors().enumerate() {
                    if i > 0 { write!(fmt, ", ")?; }
                    write!(fmt, "{}: {:?}", labels[i], target)?;
                }
                if show_unwind { write!(fmt, ", ")?; fmt_unwind(fmt)?; }
                write!(fmt, "]")
            }
        }
    }
}
```

The render is composed of three pieces:

- `fmt_head` [`pretty.rs:919-1022`] -- the per-variant prefix
  (e.g., `goto`, `switchInt(_1)`, `assert(_2, "msg", ..)`).
- `successors()` [`terminator.rs:527-574`] -- iterator of
  successor BasicBlocks.
- `fmt_successor_labels` [`pretty.rs:1024-...`] -- per-edge
  string labels (`return`, `unwind`, `drop`, `success`,
  `real`, `imaginary`, `otherwise`, etc.).

The rendering is then "head -> [label1: bb1, label2: bb2, ...]"
when there are multiple successors.

D-9-K framing: stage-9 explicitly defers `render_terminator`. The
rustc precedent above shows that a complete renderer needs:
per-variant prefix syntax, edge enumeration, edge labels, and
unwind-action inline string. Stage 11 will close this surface for
RegseqModel; for now, stage-9 just records the rustc reference.

`fmt_head` per-variant text:

- `Goto { .. }` -> `"goto"` [`pretty.rs:926`].
- `SwitchInt { discr, .. }` -> `"switchInt({discr:?})"` [927].
- `Return` -> `"return"` [928].
- `CoroutineDrop` -> `"coroutine_drop"` [929].
- `UnwindResume` -> `"resume"` [930].
- `UnwindTerminate(reason)` -> `"terminate({reason.as_short_str()})"` [931-933].
- `Yield { value, resume_arg, .. }` -> `"{resume_arg:?} = yield({value:?})"` [934].
- `Unreachable` -> `"unreachable"` [935].
- `Drop { place, async_fut: None, .. }` -> `"drop({place:?})"` [936].
- `Drop { place, async_fut: Some(fut), .. }` -> `"async drop({place:?}; poll={fut:?})"` [937-939].
- `Call { func, args, destination, .. }` -> `"{destination:?} = {func:?}(arg0, arg1, ...)"` [940-950].
- `TailCall { func, args, .. }` -> `"tailcall {func:?}(arg0, ...)"` [951-960].
- `Assert { cond, expected, msg, .. }` -> `"assert({"" if expected else "!"}{cond:?}, ...)"` (calls `msg.fmt_assert_args`) [961-969].
- `FalseEdge` -> `"falseEdge"` [970].
- `FalseUnwind` -> `"falseUnwind"` [971].
- `InlineAsm { template, operands, options, .. }` -> `"asm!(\"{template}\", op0, op1, ..., options(...))"` [972-1020].

The successor-labels per-variant
[`pretty.rs:1025-...`]:

- `Return | TailCall { .. } | UnwindResume | UnwindTerminate(_) | Unreachable | CoroutineDrop` -> `vec![]`.
- `Goto { .. }` -> `vec![""]`.
- `SwitchInt { targets, .. }` -> `vec!["{value}", ..., "otherwise"]`.
- `Call { target: Some, unwind: Cleanup, .. }` -> `vec!["return", "unwind"]`.
- `Call { target: Some, .. }` -> `vec!["return"]`.
- `Call { target: None, unwind: Cleanup, .. }` -> `vec!["unwind"]`.
- `Call { target: None, .. }` -> `vec![]`.
- `Yield { drop: Some, .. }` -> `vec!["resume", "drop"]`.
- `Yield { drop: None, .. }` -> `vec!["resume"]`.
- `Drop { unwind: Cleanup, drop: Some, .. }` -> `vec!["return", "unwind", "drop"]`.
- `Drop { unwind: Cleanup, drop: None, .. }` -> `vec!["return", "unwind"]`.
- `Drop { drop: Some, .. }` -> `vec!["return", "drop"]`.
- `Drop { .. }` -> `vec!["return"]`.
- `Assert { unwind: Cleanup, .. }` -> `vec!["success", "unwind"]`.
- `Assert { .. }` -> `vec!["success"]`.
- `FalseEdge` -> `vec!["real", "imaginary"]`.
- `FalseUnwind { unwind: Cleanup, .. }` -> `vec!["real", "unwind"]`.
- `FalseUnwind { .. }` -> `vec!["real"]`.
- `InlineAsm` -> per-target labels, "return" / "unwind" / "label N".

The graphviz / DOT renderer at
`rustc_middle/src/mir/generic_graph.rs` reuses the same pieces
[`generic_graph.rs:24, 54`]:

```rust
let labels = terminator.kind.fmt_successor_labels();
...
data.terminator().kind.fmt_head(&mut terminator_head).unwrap();
```

INFERENCE: stage 11's `render_terminator` for the kept-6
RegseqModel surface will need at most 6 prefix strings, 4 edge
labels (`success`, `unwind`, `otherwise`, ""), and an
unwind-action inline format. The 9 dropped variants do not need
rendering (they are bridge-rejected before any printer sees
them).

---

## 13. Stage-9 takeaways table

Per-D-tag mapping of the rustc evidence that grounds the stage-9
decisions.

**D-9-1.1 (cardinality of the kept set):**

- rustc has 15 variants [`syntax.rs:702-987`].
- 8 are pipeline-discharged or DSL-rejected (§3): `TailCall`,
  `UnwindResume`, `UnwindTerminate`, `Yield`, `CoroutineDrop`,
  `FalseEdge`, `FalseUnwind`, `InlineAsm`.
- 1 is collapsed at the bridge (D-9-2): `Drop`.
- 6 are kept: `Goto`, `SwitchInt`, `Return`, `Unreachable`,
  `Call`, `Assert`.

**D-9-1.2 (Goto payload):** Single `BasicBlock` field
[`syntax.rs:705`]; trivial.

**D-9-1.3 (SwitchInt payload):** `(Operand, SwitchTargets)`
[`syntax.rs:715-719`]; SwitchTargets shape per §7.

**D-9-1.4 (Return / Unreachable payloads):** Both unit
[`syntax.rs:746, 751`].

**D-9-1.5 (Call payload):** 7 fields
[`syntax.rs:811-832`]; per-field analysis in §5. Three structural
sub-decisions:

- D-9-3 (callee shape) -- Direct/Virtual via stage-1
  `InstanceKind::Virtual` vs. `CallCallee::{Direct,Indirect}`
  surface. Evidence: rustc's `InstanceKind::Virtual`
  [`instance.rs:111-119`] and the codegen-SSA virtual-call
  unwrapping [`block.rs:1165-1211`].
- D-9-4 (per-arg span) -- inline `ArgWithSpan` vs. parallel
  slabs. Evidence: rustc uses inline `Spanned<Operand>`
  [`syntax.rs:820`, `source_map.rs:77-81`].
- Whether to preserve `call_source` (diagnostic-only) and
  `fn_span` (caller-location). Evidence: `call_source` is
  doc-flagged "Used only for diagnostics" [`syntax.rs:641`].

**D-9-1.6 (Assert payload):** 5 fields
[`syntax.rs:875-881`]; AssertKind subset per §6.3 (D-9-5).

**D-9-2 (Drop-as-Call):** rustc's own codegen layer reduces Drop
to a Call via `Instance::resolve_drop_in_place` and
`helper.do_call` [`block.rs:599-693`]. The bridge does the same
collapse at the MIR level instead of at codegen time.
Empty-glue elision precedent at [`block.rs:610-613`].

**D-9-3 (callee shape supporting vtable):** see D-9-1.5 and §5.2.
Rustc's structural answer is `InstanceKind::Virtual(DefId, usize)`
[`instance.rs:111-119`]; the dispatch happens in
`codegen_call_terminator`'s arg-loop
[`block.rs:1165-1211`].

**D-9-4 (per-arg span):** see D-9-1.5 and §5.3. Rustc uses
inline `Spanned<Operand>` [`syntax.rs:820`].

**D-9-5 (AssertKind subset):** drop 3 coroutine-related
[`syntax.rs:1072-1074`], keep 8 driver-relevant. Coroutine
variants are stage-3 unreachable (no `ty::Coroutine` in our
`Ty` surface). See §6.3.

**D-9-6 (SwitchTargets storage):** rustc uses asymmetric flat
slabs with "last targets entry is otherwise" invariant
[`syntax.rs:1004-1031`]; the source comment explicitly
considered and rejected the symmetric `(Vec<(u128, BB)>,
otherwise: BB)` shape for ergonomic reasons that don't apply to
RegseqModel. See §7.3.

**D-9-7 (no Terminator interning):** rustc's `TerminatorKind`
does not derive `Eq` (only `PartialEq`); `Terminator` is stored
inline in `BasicBlockData`; no interning arena; size budget is
80 bytes per kind [`syntax.rs:1754`]. RegseqModel inheriting
"owned-per-block, no interning" is the path of least resistance.

**D-9-8 (wrapper struct shape):** rustc uses `Terminator {
source_info: SourceInfo, kind: TerminatorKind }`
[`terminator.rs:435-439`]; `SourceInfo = (Span, SourceScope)`
[`mod.rs:822-830`]. RegseqModel proposed `{ kind, span }` is a
flatten (drops the borrowck-era SourceScope which has no known
stage-14 consumer; HYPOTHESIS pending stage-11 closure).

**D-9-9 (cleanup-pass discharge table):** see §3 in full.
Per-variant discharge mechanism with `is_required` /
`is_enabled` annotations and validator-rejection citations.

**D-9-10 (invariants registry):** see §10.1. The validator's
two-pass check at [`validate.rs:353-528, 1588-1655`] is the
reference. RegseqModel's registry is a subset scoped to the
kept-6 set.

**D-9-K (`render_terminator` deferral):** see §12. Stage 11
will close the renderer surface; stage 9 records the rustc
precedent (`fmt_head` + `successors` + `fmt_successor_labels`)
without specifying the RegseqModel printer.

---

## Appendix: Quick file-by-file index

For navigation across the rustc primary sources cited above:

- `rustc_middle/src/mir/syntax.rs:702-987` -- TerminatorKind
  enum, 15 variants.
- `rustc_middle/src/mir/syntax.rs:1004-1031` -- SwitchTargets.
- `rustc_middle/src/mir/syntax.rs:1033-1050` -- UnwindAction.
- `rustc_middle/src/mir/syntax.rs:1052-1061` -- UnwindTerminateReason.
- `rustc_middle/src/mir/syntax.rs:1066-1078` -- AssertKind (11 variants).
- `rustc_middle/src/mir/syntax.rs:1080-1111` -- InlineAsmOperand (7 variants).
- `rustc_middle/src/mir/syntax.rs:1113-1114` -- AssertMessage type alias.
- `rustc_middle/src/mir/syntax.rs:642-660` -- CallSource.
- `rustc_middle/src/mir/syntax.rs:662-670` -- InlineAsmMacro.
- `rustc_middle/src/mir/syntax.rs:1754` -- size assert (80 bytes).
- `rustc_middle/src/mir/terminator.rs:13-126` -- SwitchTargets impls.
- `rustc_middle/src/mir/terminator.rs:128-160` -- UnwindAction / UnwindTerminateReason impls.
- `rustc_middle/src/mir/terminator.rs:162-433` -- AssertKind impls (lang_item map, fmt_assert_args).
- `rustc_middle/src/mir/terminator.rs:435-469` -- Terminator wrapper struct + accessors.
- `rustc_middle/src/mir/terminator.rs:471-498` -- TerminatorKind name() / if_().
- `rustc_middle/src/mir/terminator.rs:500-637` -- Successors iterator + successors_mut.
- `rustc_middle/src/mir/terminator.rs:640-720` -- unwind() / unwind_mut() / as_switch / as_goto / can_write_to_memory.
- `rustc_middle/src/mir/terminator.rs:722-750` -- TerminatorEdges / CallReturnPlaces.
- `rustc_middle/src/mir/terminator.rs:769-841` -- edges() per variant.
- `rustc_middle/src/mir/visit.rs:526-671` -- super_terminator visitor walk.
- `rustc_middle/src/mir/pretty.rs:872-1080` -- Terminator Debug + fmt_head + fmt_successor_labels.
- `rustc_middle/src/ty/instance.rs:62-185` -- InstanceKind enum (16 variants, including Virtual).
- `rustc_data_structures/src/packed.rs:8-12` -- Pu128.
- `rustc_span/src/source_map.rs:77-81` -- Spanned<T>.
- `rustc_target/src/spec/mod.rs:854-862` -- PanicStrategy::unwinds().
- `rustc_ast/src/ast.rs:2697-2706` -- InlineAsmOptions bit flags (NORETURN, MAY_UNWIND, ...).
- `rustc_mir_transform/src/lib.rs:583-687` -- pipeline phase orchestrator.
- `rustc_mir_transform/src/lib.rs:618-633` -- run_analysis_cleanup_passes.
- `rustc_mir_transform/src/lib.rs:637-664` -- run_runtime_lowering_passes.
- `rustc_mir_transform/src/lib.rs:667-687` -- run_runtime_cleanup_passes.
- `rustc_mir_transform/src/lib.rs:689-...` -- run_optimization_passes.
- `rustc_mir_transform/src/cleanup_post_borrowck.rs:67-90` -- FalseEdge/FalseUnwind discharge.
- `rustc_mir_transform/src/elaborate_drops.rs:50-94` -- ElaborateDrops pass.
- `rustc_mir_transform/src/abort_unwinding_calls.rs:67-144` -- UnwindResume/UnwindTerminate/cleanup-edge discharge.
- `rustc_mir_transform/src/coroutine.rs:1465-1685` -- StateTransform pass.
- `rustc_mir_transform/src/check_alignment.rs:14-148` -- MisalignedPointerDereference Assert emission.
- `rustc_mir_transform/src/check_null.rs:12-127` -- NullPointerDereference Assert emission.
- `rustc_mir_transform/src/check_enums.rs:17-114` -- InvalidEnumConstruction Assert emission.
- `rustc_mir_transform/src/known_panics_lint.rs` -- a lint, not an emitter.
- `rustc_mir_transform/src/validate.rs:353-528` -- visit_terminator (general).
- `rustc_mir_transform/src/validate.rs:1588-1655` -- visit_terminator (type-aware).
- `rustc_codegen_ssa/src/mir/block.rs:1455-1579` -- per-Terminator codegen dispatch.
- `rustc_codegen_ssa/src/mir/block.rs:599-693` -- codegen_drop_terminator.
- `rustc_codegen_ssa/src/mir/block.rs:894-...` -- codegen_call_terminator.
- `rustc_codegen_ssa/src/mir/block.rs:1165-1211` -- Virtual-call vtable unwrap.
- `rustc_codegen_cranelift/src/base.rs:284-595` -- cranelift mirror of per-Terminator dispatch.
- `rustc_mir_build/src/builder/expr/as_place.rs:9` -- BoundsCheck Assert origin.
- `rustc_mir_build/src/builder/expr/as_rvalue.rs:121, 529, 579, 588, 590, 592` -- arithmetic Assert origins.
