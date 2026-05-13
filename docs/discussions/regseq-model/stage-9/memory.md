# Stage 9 -- MIR Terminators: Stage Memory

Stage-9-specific briefing for an LLM agent picking up the
terminator work. Read in addition to (not instead of) the
top-level `../README.md` (architectural foundation, global
methodology rules) and `../status.md` (top-level progress
tracker).

This file is referenced **only** from `../status.md`'s
"Active focus" block. The top-level `../README.md` does not
reference it -- that's by convention, to keep top-level
context lean.

This file captures stage-9 *invariants* (mission, workflow,
principles, read order, Codex prompt template). Stage-9
*current state* (which decisions are locked, what's open,
active cluster, next concrete unit of work) lives in
`status.md`.

## 0. Mission

Stage 9 specifies the `Terminator` vocabulary that the
bridge fills from rustc's `mir::TerminatorKind<'tcx>` and
that downstream consumers (stage 10 basic blocks --
`BasicBlock.terminator: Terminator`; stage 11 bodies; stage
14 backends) read as the **control-flow-edge surface** of
MIR.

Stage 9 sits **fifth in the MIR pipeline** (stages 5-11
form a strict bottom-up chain: places -> operands ->
rvalues -> statements -> **terminators** -> basic blocks
-> bodies). Its content is referenced by:

- Stage 10 `BasicBlock`: `BasicBlock.terminator: Terminator`
  (or whatever the stage-10 storage decision dictates) --
  every basic block ends with exactly one stage-9
  Terminator.
- Stage 11 `Body`: bodies own basic blocks transitively;
  per-terminator walk visitors (D-9-10 walk-impl deferral)
  surface here. Body-level transformations (e.g., the
  vtable-indirect dispatch rewrite per D-9-3) live at
  stage 11 and consume the stage-9 surface.
- Stage 14 backends: each `Terminator` variant lowers to a
  C control-flow construct or runtime-helper call (e.g.,
  `Goto` -> `goto label;`; `Return` -> `return _0;`;
  `SwitchInt` -> `switch (discr) { case v: goto bb; ...
  default: goto bb_otherwise; }`; `Call` -> `lhs = fn(args);
  goto target;` for non-divergent calls or trailing
  `goto target;` after an abort helper for divergent;
  `Assert` -> `if (cond != expected) abort();
  goto target;`).

Stage 9 receives content from earlier stages:

- Stage 1 IDs (`LocalId`, `BlockId`, `ConstId`,
  `InstanceId`, `TyId`, `ItemId`) -- transitively through
  stages 3-8.
- Stage 1 `Span` material -- referenced by D-9-8
  (Terminator wrapper struct) and D-9-4 (per-arg span
  preservation candidate).
- Stage 1 `InstanceKind` (6-variant grandfathered closed
  set) -- consumed by `Call` callee at D-9-3 candidate (a)
  if the variant is reopened with `Virtual { idx }`.
- Stage 3 `Ty` (`VariantIdx` lookups; type-equality on
  Call destination/return-type matching; `Ty::Bool` /
  integer types for SwitchInt discr; `Ty::Bool` for Assert
  cond).
- Stage 3 layout content (`VariantLayout` / `FieldLayout`
  if D-9-3 candidate (b) consumes vtable layout names at
  render time -- though render is deferred to stage 11 by
  D-9-K).
- Stage 5 `Place` -- consumed by `Call.destination` and
  (under D-9-2) by the synthesised `&raw mut place` arg
  for the drop-glue `Call`.
- Stage 6 `Operand` -- consumed by `SwitchInt.discr`,
  `Call.func` (D-9-3 candidate (b) `Indirect` arm),
  `Call.args` per-position, and `Assert.cond`.
- Stage 7 `Rvalue` -- not directly consumed by stage 9
  (Rvalues live inside `Statement::Assign`, not
  Terminator), but the stage-7 `RawPtr` rule
  (D-7-1.4-flavored bridge collapse semantics) informs
  the synthesised `&raw mut place` operand the bridge
  emits at D-9-2 collapse.
- Stage 8 `Statement` -- not directly consumed by stage 9
  (Statements precede terminators within a basic block,
  not embedded in them), but stage 8's invariant chain
  (B-0a/b/c inherited from §19.7/§21.5/§20.5) feeds
  stage-9's invariant registry as additional layers.

Stage 9 introduces no new top-level interner. Its content
is fully expressible in terms of stage-1 + stages 3, 5, 6,
7, 8 vocabulary. **Subject to D-9-3 resolution**, stage 9
may either (a) reopen stage 1 to add `InstanceKind::Virtual
{ idx }` (a stage-1 spec revision per the spec-mutability
rule) or (b) introduce a stage-9-local `CallCallee` enum
that widens the Call callee position without touching stage
1.

Stage 9 closes **construction invariant 9**
(`InstanceId` references resolve in the model's
instance-closure) per the stage-1 expectation: once stage
9 introduces call-site references to `InstanceId`,
`validate`'s invariant 9 becomes enforceable
non-vacuously. Walk-implementation is deferred to stage 11
(matching D-8-4 Part C).

The output of stage 9 is `spec-snapshot.md` (the canonical
per-stage spec). This file briefs the agent on *how*
decisions about that spec are reached.

## 1. Workflow (the rhythm we use)

Stage 9 inherits the two-phase rhythm proven in stages 3,
4, 5, 6, 7, and 8:

**Phase A -- Discussion pass.** In conversation: lay out
the open decisions in a cluster with options + recommendation
+ question(s). The user weighs in. Sub-questions are
answered. Reasoning chains are validated. Decisions emerge.

**Phase B -- Spec commit + adversarial review.** Locked
decisions land in `spec-snapshot.md` with the full audit
shape (Decision / Reasoning / Methodology rules / Prior-art
evidence / Worked example). A Codex review-correction loop
runs against the cluster:
1. Initial review prompt (template in Section 4).
2. Codex returns verdict (BLOCK / MINOR / APPROVE) +
   numbered issues.
3. Address each issue; record corrections in
   `review-log.md`.
4. Re-run Codex with a fresh prompt naming the corrections.
5. Repeat until APPROVE.

**Reviews are per-cluster, not per-decision and not
big-bang.** Smaller scope sharpens the review (Codex's slot
count is finite); cluster decisions cascade so catching
errors early is cheaper.

`discussions.md` holds the distilled decision record;
`review-log.md` holds the round-by-round audit trail.

**Decision tag convention.** Stage 9 uses the prefixed
format `D-9-K` from day one (`D-9-1`, `D-9-2`, ..., with
sub-tags `D-9-1.1`, `D-9-1.2`, ..., per `../README.md`
§5.2). Stages 1 and 3 use legacy un-prefixed forms; stage 9
does not inherit those.

## 2. Stage-9-specific principles

Corollaries of `../README.md` "Methodology rules" applied
to the terminator vocabulary:

- **Store rustc's resolved terminator form, not the surface
  expression.** The bridge consumes `mir::TerminatorKind<'tcx>`
  from `optimized_mir` with `mir_opt_level=0` and the
  cleanup-pass whitelist applied. Terminators at this stage
  are post-monomorphization, post-region-erasure,
  post-`CleanupPostBorrowck` (FalseEdge/FalseUnwind ->
  Goto), post-`ElaborateDrops` (Drop is unconditional
  drop_in_place semantics), post-`AbortUnwindingCalls`
  (under panic=abort, unwind edges become
  `UnwindAction::Unreachable` and cleanup blocks become
  unreachable), post-`StateTransform` (Yield/CoroutineDrop
  discharged). Mirror what rustc hands us at this surface,
  not the pre-cleanup surface.

- **Closed `Terminator` vocabulary.** The locked variant set
  is rejected if exceeded. Any rustc `TerminatorKind<'tcx>`
  variant the spec does not name is a bridge-rejection event
  with a diagnostic.

- **Two rejection categories: pipeline-discharged vs
  DSL-only.** Per the rustc prior-art (§13 takeaways), the
  rejection table at D-9-9 distinguishes:
  - **Pipeline-discharged:** rustc's required passes
    (`CleanupPostBorrowck` for FalseEdge/FalseUnwind;
    `StateTransform` for Yield/CoroutineDrop;
    `AbortUnwindingCalls` under panic=abort for
    UnwindResume/UnwindTerminate via cleanup-block
    pruning) eliminate these variants before the bridge
    consumes the body. The rejection is a defensive
    safety check at the variant boundary; encountering one
    in well-formed `optimized_mir` indicates a
    pipeline-config bug, not a driver-source error.
  - **DSL-only:** rustc supports these variants in
    `optimized_mir`, but the Regseq DSL does not. `TailCall`
    (gated by `#![feature(explicit_tail_calls)]`) and
    `InlineAsm` (always available) reach the bridge under
    rustc's standard configuration; the bridge rejects them
    because the DSL surface does not include the constructs.
    The rejection is a wellformedness-of-driver-source check;
    encountering one indicates a driver-author error.

  Both categories surface the same bridge-rejection
  diagnostic shape; the table at D-9-9 names the discharge
  mechanism (or "DSL-only") for each entry.

- **Drop is collapsed at the bridge, not preserved as a
  variant.** Per stage-1's inbound expectation
  (`expectations.md` "From stage 1"), MIR's
  `TerminatorKind::Drop { place, target, unwind, replace,
  drop, async_fut }` is converted by the bridge into a
  synthetic `Call` terminator targeting the drop-glue
  `Instance` for the place's type. Per D-9-2. The
  `replace`, `drop`, `async_fut` extras are subsumed by
  this collapse (none survive into RegseqModel). rustc's
  own codegen-SSA performs the same collapse internally
  (`Instance::resolve_drop_in_place` + `helper.do_call` at
  `rustc_codegen_ssa/src/mir/block.rs:599-693`); the bridge
  performs it earlier, exposing the result to all
  consumers.

- **Cleanup-pass artefacts are reject-on-encounter, not
  variants.** Per the §17 spec rejection-strategy pattern,
  constructs that are discharged by required passes
  (`FalseEdge` / `FalseUnwind` via `CleanupPostBorrowck`;
  `Yield` / `CoroutineDrop` via `StateTransform`;
  `UnwindResume` / `UnwindTerminate` via
  `AbortUnwindingCalls` + cleanup-block pruning under
  panic=abort) are rejected if encountered in a well-formed
  `optimized_mir` body. The bridge does NOT carry
  corresponding variants in the RegseqModel `Terminator`
  enum. Same discipline as stages 5, 6, 7, and 8.

- **Out-of-pipeline constructs are reject-on-encounter,
  not variants.** Same discipline applied at the
  feature-flag / build-mode level: `TailCall` (only with
  `#![feature(explicit_tail_calls)]`) and `InlineAsm`
  (DSL-rejected; the DSL has explicit MMIO / synchronization /
  reset / clock intrinsics for low-level operations) are
  reject-on-encounter with a diagnostic naming the absent
  feature gate (TailCall) or the DSL-level alternative
  (InlineAsm), not a "feature might appear" carry.

- **Drop NLL/borrowck-only carriers.** Same discipline as
  stages 6, 7, 8. At the Terminator surface, the analog
  applies to:
  - `source_info.scope` (debuginfo-only; contributes nothing
    to C lowering) -- dropped at the bridge per D-9-8.
  - `Call.call_source` (`OverloadedOperator | MatchCmp |
    Misc | Use | Normal`; diagnostic-only) -- dropped at the
    bridge per D-9-8.
  - `Call.fn_span` (the function-position span without the
    receiver; diagnostic-only refinement subsumed by either
    the Terminator-level span or the per-arg span depending
    on D-9-4 resolution) -- dropped at the bridge per
    D-9-8.
  - `unwind: UnwindAction` on Call/Drop/Assert/InlineAsm
    (under panic=abort with `AbortUnwindingCalls` having
    run, the only surviving values are `Continue` and
    `Unreachable`; `Cleanup(_)` is gone via cleanup-block
    pruning; `Terminate(_)` is gone because cleanup blocks
    are gone). The `unwind` field carries no driver-relevant
    information at our consumption point and is dropped at
    the bridge per D-9-1.5/D-9-1.6 payload trimming.
  - rustc Drop's `replace` (diagnostic-only `bool`), `drop`
    (`Option<BasicBlock>` for async drop) and `async_fut`
    (`Option<Local>` for async drop) extras are subsumed
    by D-9-2 (Drop-as-Call); none survive into RegseqModel.

  Preserve only what carries C-relevant or
  diagnostic-relevant information that another stage
  consumes. The Terminator-level `span: Span` is the
  surviving span surface; per-arg spans (D-9-4) are a
  separate decision.

- **`Terminator` is body-local; no interning, no
  `TerminatorId`.** Terminator lives inside body content
  (basic-block payloads at stage 10, transitively at stage
  11), not in a top-level interned table. Equality and
  hashing are structural over the variant payload.
  Compositional with stage-5 / stage-6 / stage-7 / stage-8
  inline-storage choices. Rustc has no terminator-interner;
  rustc_public follows the same pattern; mrustc and Charon
  both store terminator inline at the basic-block boundary.
  Per D-9-7.

- **F4 / F4-2 cross-build concerns are out of scope.**
  `Terminator` is body-local content; it does not
  participate in cross-build identity (the F4-2 instance
  ordering canonicalises `(InstanceKey)`, not
  `(BodyContents)`). Terminator identity is per-build,
  structural, and need not be canonicalised across build
  environments. Same scope reduction as stages 5, 6, 7, 8.

- **Renderer style is decided deliberately.**
  `pretty.rs::render_terminator` defers to stage 11
  alongside `render_place` (D-5-K precedent),
  `render_operand` (D-6-K precedent), `render_rvalue`
  (D-7-K precedent), and `render_statement` (D-8-K
  precedent). What forces deferral: every variant's payload
  either embeds Place/Operand/Rvalue (D-5-K + D-6-K +
  D-7-K-deferred), references InstanceId for callee
  rendering (which depends on stage-12 `Item.def_path`
  rendering), or references a stage-3 type for Switch
  discr / Assert cond rendering (which is already
  available). The Place/Operand-embedded subtree is the
  primary deferral driver; the InstanceId-rendering subtree
  adds stage-12 dependency. Per D-9-K. Outbound expectation
  files in `stage-11/expectations.md` "From stage 9" at
  lock time.

- **Closes construction invariant 9.** Per the stage-1
  inbound expectation, stage 9 introduces call-site
  references to `InstanceId` (via the Call terminator's
  callee position, regardless of D-9-3 resolution). At the
  walk-implementation step (deferred to stage 11),
  `validate`'s invariant 9 (instance closure-completeness)
  becomes enforceable non-vacuously: walk every Call's
  `InstanceId` (the `Direct` arm under D-9-3 candidate (b),
  or the `InstanceId` under candidate (a) when
  `InstanceKind::Virtual` is **not** the variant -- virtual
  Calls have no InstanceId-pinned dispatch and are checked
  via vtable-shim instance closure separately at
  stage-12-synthesised vtable shims).

- **`Option<...>` niche optimizations: nothing new at
  stage 9.** Stage 9 introduces no new ID types. The
  4-byte `Option<X>` invariants stage 1 established for
  existing ID types are not affected. The Call
  `target: Option<BlockId>` field uses `Option<BlockId>`
  per stage-10 `BlockId` semantics; stage-1's niche
  optimization for `BlockId` carries through.

- **C-semantics for the terminator surface.** Each retained
  variant maps to a C control-flow form:
  - `Goto(target)`: `goto bb_<target>;`.
  - `SwitchInt { discr, targets }`: `switch (<discr>) {
    case <v_0>: goto bb_<target_0>; case <v_1>: goto
    bb_<target_1>; ... default: goto bb_<otherwise>; }`.
    Discr-type-driven; integer/bool/char `discr` values
    cast to C `unsigned long long` for the switch
    expression.
  - `Return`: `return <_0>;` (or void return for
    `Ty::Tuple([])` return type).
  - `Unreachable`: `__builtin_unreachable();` (clang/gcc)
    or `__assume(0);` (msvc) or fallback `for(;;);`.
  - `Call { callee, args, destination, target }`: `<dest>
    = <callee>(<args...>); goto <target>;` for
    non-divergent calls; `<callee>(<args...>);
    __builtin_unreachable();` for `target == None`
    (divergent). Callee rendering depends on D-9-3
    resolution (Direct: emit C function name from
    InstanceId's symbol_name; Indirect: dereference the
    function-pointer Operand).
  - `Assert { cond, expected, msg, target }`: `if (<cond>
    != <expected>) <abort_helper>(<msg>); goto <target>;`
    where `<abort_helper>` is the panic=abort helper for
    the runtime (e.g., `__rust_abort_with_msg(...)`,
    `panic_handler(...)`, or a DSL-level abort intrinsic
    chosen by stage 14 codegen).

  Dropped variants (TailCall, UnwindResume, UnwindTerminate,
  Yield, CoroutineDrop, FalseEdge, FalseUnwind, InlineAsm,
  rustc-Drop-as-separate-variant) have no C lowering -- the
  bridge rejects them on encounter (D-9-9); they do not
  reach stage-14 codegen.

  The C-shaping rule from `../README.md` "C semantics with
  information preservation" applies: store what rustc gave
  us, render in C shape on demand at stage 14.

- **Compositional behavior beats special-case flags.**
  Same as stages 3-8. Examples for stage 9:
  - `SwitchInt`'s discr-type constraint
    (`discr_ty(operand) ∈ {Ty::Bool, Ty::Char, Ty::Int(_),
    Ty::Uint(_)}`) falls out of stage-3 `Ty` interning +
    stage-6 operand-type lookup -- no parallel
    `is_typed_switch` flag.
  - `Assert.cond`'s `Ty::Bool` constraint falls out of
    operand-type lookup against stage-3 `Ty`, not a parallel
    `is_bool_assert` flag.
  - `Call`'s callee resolution (Direct vs Indirect under
    D-9-3 candidate (b), or Item vs Virtual under D-9-3
    candidate (a)) falls out of the variant choice itself,
    not a parallel `is_indirect_call` flag.
  - Drop's collapse to Call falls out of D-9-2 bridge
    rewriting (synthetic Call to drop-glue Instance), not a
    `is_drop_call` flag distinguishing the two at the
    Terminator surface.

- **Charon-style "Assert as Statement" alternative is
  noted, not adopted.** Charon classifies `Assert` as a
  `StatementKind` (not Terminator) and lifts rustc's
  `TerminatorKind::Assert` to a Statement push +
  replaces the terminator with `Goto`. This is a
  precedented design (`charon-lib/src/ast/ullbc_ast.rs:41`).
  Stage 9 keeps `Assert` at the Terminator level for
  information preservation: `target` (success path) and
  `unwind` (failure path; though `unwind` is dropped per
  panic=abort) are control-flow edges that lose their
  structural meaning when collapsed into a Statement.
  Phase-A discussion may revisit; the decision is to
  preserve the rustc surface unless evidence shows
  Charon's collapse outperforms it for our DSL.

## 3. Read order for new agents picking up stage 9

In priority:

0. **If you are opening a specific cluster**: read this
   file + `status.md` + `discussions.md` entries for the
   decisions in your cluster (once they exist) + the
   relevant spec sections (top-level `../spec.md` Section
   2 (`InstanceKind` for D-9-3 candidate (a)),
   Section 9 `Const`, Section 10 `Ty`, Section 13 layout +
   discriminant tables (relevant for SwitchInt discr-type
   match), Section 14 `FnSig` and `FnAbi` (relevant for
   Call args/destination type matching), Section 19
   `Place`, Section 20 `Operand`, Section 21 `Rvalue`,
   Section 22 `Statement` -- since stage-9 Terminator
   variants reference all of these).

1. This file (`stage-9/memory.md`) -- stage-9 invariants.

2. `status.md` -- current state (locked decisions, active
   cluster, next concrete unit of work).

3. `spec-snapshot.md` -- the working spec (initially a
   copy of `../spec.md`; stage-9 contributions accumulate
   as new sections describing `Terminator` and bridge
   translation rules for the terminator surface).

4. `expectations.md` -- inbound forward refs (from
   stage 1, stage 3, stage 6 -- 4 entries).

5. `discussions.md` -- distilled decision records (created
   when the first decision lands; lazy).

6. `review-log.md` -- round-by-round Codex audit trail
   (created when the first review round runs; lazy).

7. `prior-art/<project>.md` -- per-stage primary-source
   research (fresh pass per `../README.md` §2 methodology
   rule -- do **not** reuse stage-0 summaries). Landed at
   stage open:
   - `prior-art/rustc.md` -- rustc primary source:
     15-variant `TerminatorKind<'tcx>`, `Terminator<'tcx>`
     wrapper struct, `UnwindAction` / `UnwindTerminateReason`
     / `AssertKind<O>` / `CallSource` / `InlineAsmOperand` /
     `SwitchTargets` / `BackwardIncompatibleDropReason`
     supporting types, cleanup-pass discharge ordering
     (`CleanupPostBorrowck` `is_required: true`,
     `ElaborateDrops`, `AbortUnwindingCalls`,
     `coroutine::StateTransform`), validator + visitor
     references, codegen-SSA per-Terminator dispatch
     pattern, `InstanceKind::Virtual(DefId, usize)`
     vtable-index semantics, two-category rejection
     framing (pipeline-discharged vs DSL-only -- TailCall
     and InlineAsm reach `optimized_mir` under rustc's
     standard config; the others are pipeline-discharged
     unconditionally).
   - `prior-art/rustc_public.md` -- rustc_public primary
     source: 10-variant `TerminatorKind` collapse (drops 5,
     simplifies UnwindResume->Resume / UnwindTerminate->Abort,
     drops Drop's `replace`/`drop`/`async_fut`/`unwind`,
     drops Call's `Spanned<Operand>` -> `Vec<Operand>`,
     drops `call_source`/`fn_span`/`unwind`); the
     `Terminator { kind, span }` wrapper choice (drops
     `source_info.scope`); `InstanceKind { Item, Intrinsic,
     Virtual { idx }, Shim }` 4-variant exposed at
     `mono.rs:30` -- direct precedent for D-9-3 candidate
     (a); the bridge translator's 5 explicit drops
     (`TailCall` via `todo!()`, Yield / CoroutineDrop /
     FalseEdge / FalseUnwind via `unreachable!()`) at
     `unstable/convert/stable/mir.rs:704-790` -- direct
     precedent for D-9-9; per-arg span DROPPED entirely
     (not preserved via wrapper) -- `arg.node.stable(...)`
     at `unstable/convert/stable/mir.rs:752` -- adds a
     third option (c) "drop spans entirely" to D-9-4's
     two-candidate list.
   - `prior-art/charon.md` -- Charon primary source:
     ULLBC `RawTerminator` ~7 variants (Goto, Switch, Call,
     Abort, Return, Drop, Unreachable -- Assert is a
     **StatementKind**, not Terminator!); `transform_dyn_trait_calls`
     pass at `src/transform/normalize/transform_dyn_trait_calls.rs:31-122`
     synthesises `PtrMetadata -> Deref -> Field(method_field_id)`
     projection through the receiver's vtable and mutates
     `call.func = FnOperand::Move(method_field_place)` at
     line 119; `FnOperand::{Regular(FnPtr), Move(Place)}`
     callee split at `src/ast/gast.rs:386-391` -- direct
     precedent for D-9-3 candidate (b); Drop preserved as
     first-class Terminator with opt-in `--desugar-drops`
     pass at `src/transform/normalize/desugar_drops.rs:23-72`
     that rewrites Drop -> Call with `&raw mut place` arg;
     `AbortKind` 3-variant collapse (`Panic`,
     `UndefinedBehavior`, `UnwindTerminate`) shared between
     `Abort` Terminator and `Assert` Statement, drops
     `msg` field wholesale -- alternative for D-9-5
     (more aggressive than our proposed 8-variant subset);
     `Terminator { span, kind, comments_before }` wrapper
     adds `comments_before: Vec<String>` for Aeneas
     source-fidelity (novel; not adopted by RegseqModel).
   - `prior-art/mrustc.md` -- mrustc primary source:
     `MIR::Terminator` tagged union (~10 variants
     Incomplete, Return, Diverge, Goto, Panic, If, Switch,
     SwitchValue, Call, Drop) at `src/mir/mir.hpp`;
     **Drop is a Statement**, not a Terminator (`src/mir/mir.hpp:710-715`)
     -- a third drop-handling shape (in addition to
     "preserve-as-Terminator" and "collapse-to-Call");
     `CallTarget = Value(LValue) | Path(::HIR::Path) |
     Intrinsic{name: RcString, params}` at `src/mir/mir.hpp:595-602`
     -- 3-arm callee discriminator with NO Virtual arm;
     `MIR_Cleanup_Virtualize` pass at `src/mir/cleanup.cpp:548-684`
     synthesises vtable load and flips
     `Call.fcn = Path(<dyn Trait>::method)` ->
     `Value(<vtable_loaded_lv>)` -- direct precedent for
     cleanup-discharged callee rewriting (option (b)
     flavor); `Call.panic_block` field carries panic-edge
     directly on the Call shape -- precedent for the Call
     shape inheriting the panic edge that D-9-2 collapses
     Drop into; closed-form Terminator surface by
     construction discipline (cleanup mutates payloads,
     downgrades `Call -> Goto` in narrow cases, never
     introduces new variants).

   Per-cluster top-up candidates (research only when the
   cluster's evidence demands it):
   - kani / verus / prusti: control-flow invariants in
     Rust verification pipelines (relevant if D-9-10
     invariants surface verification-style preconditions).
   - LLVM IR `br` / `switch` / `call` / `invoke`
     semantics (relevant for D-9-1 codegen direction;
     informs whether driver targets' IR shape aligns with
     Rust's terminator surface).
   - C99 / C11 §6.8.4 (selection statements, `switch`
     semantics), §6.8.6 (jump statements) -- target-language
     control-flow semantics for D-9-K render_terminator
     curation at stage 11.
   - Itanium C++ ABI / DWARF EH for vtable layout details
     (relevant for D-9-3 candidate (a)/(b) vtable-load
     rendering at stage 14 codegen).

8. `../README.md` -- top-level briefing (invariants only).

9. `../status.md` -- top-level progress tracker (cross-stage
   signals).

10. `../spec.md` -- merged top-level spec; consult Section
    9 (`Const`), Section 10 (`Ty`), Section 13 (layout +
    discriminant tables), Section 14 (`FnSig` / `FnAbi`),
    Section 19 (`Place`), Section 20 (`Operand`), Section
    21 (`Rvalue`), Section 22 (`Statement`).

## 4. Codex review cadence and prompt template

The cadence and current round status live in `status.md`
"Codex review cadence (status)." This section captures the
*template* (reusable across rounds and clusters).

Prompt template skeleton (paste-and-edit per round; per the
project root `CLAUDE.md` "Prompt shape" guidance):

```
<task>
Adversarial review of stage-9 terminator spec, decisions <X>-<Y>, ROUND <N>.

[If N > 1: round context + summary of prior corrections]

Spec under review: docs/discussions/regseq-model/stage-9/spec-snapshot.md (focus: <sections>).
Out of scope: <e.g. unrelated D-tags, markdown style>.
Evidence files: <full list relevant to this cluster>.
Evaluate: <numbered criteria; 1. round-N-fix verification if N > 1; 2. new issues from corrections; 3. residual issues; 4-N. soundness/citation/consistency>.
</task>

<grounding_rules>
Cite file:line for every claim. Do not speculate. Mark hypotheses as hypotheses. Distinguish "what spec claims" from "what evidence file says."
</grounding_rules>

<structured_output_contract>
First line: VERDICT: <BLOCK | MINOR | APPROVE>
Then numbered issues: N. [BLOCK | MINOR | NIT] <tag> <line ref>
  <description> <suggested fix>
Hard cap 25; NITs only with budget.
Last line: TOTAL: <N> issues (<B> BLOCK / <M> MINOR / <T> NIT)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

Run via:
```bash
cat <<'EOF' | uv run python /<repo-root>/scripts/python/codex_runner.py --cwd /<repo-root> --effort high --timeout 1500 --message-only > /tmp/codex_d<cluster>_review_r<N>.txt 2>&1
<prompt>
EOF
```

with `run_in_background: true`. Capture output to `/tmp/...`.
Record each round in `review-log.md` with verdict, issue
list, and corrections applied.

## 5. What this file is NOT

- **Not the stage status tracker.** Use `status.md` for
  current state (locked decisions, active cluster, next
  concrete unit of work, blockers, deferred items).
- **Not a substitute for `spec-snapshot.md` (the spec) or
  `discussions.md` (the decision records).** Briefing only.
- **Not append-only.** Update sections in place when
  stage-9 invariants change (rare).
- **Not where decisions live.** Decisions live in
  `spec-snapshot.md` (and `discussions.md` for audit shape).
