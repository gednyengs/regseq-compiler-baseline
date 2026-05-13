# Stage 9 Status

Status: **STAGE 9 CLOSED AND MERGED 2026-05-05.**
All 11 D-tags + 5 sub-decisions LOCKED. Phase B
closed at R6 MINOR-with-Lockability:yes (6 rounds).
Merger contract per README §5.5 completed: top-level
`spec.md` extended with §23 Terminator vocabulary +
§9.4 schema extension (`virtual_idx: Option<u32>`
field) + §3.3 step 8 biconditional check + §18.2
per-method ownership rows + §20.3 supersession
note; `discussions.md`, `review-log.md`,
`decisions.md` extended with stage-9 entries;
post-merge Codex review pass R1-R6 closed at R6
MINOR-with-NIT-only-Lockability:yes (6 rounds; 51
total findings closed across both Phase B and
post-merge tiers).

This spec-snapshot.md is now a frozen historical
artifact (see header). The current authoritative spec
lives in top-level `spec.md`.

Locked tag inventory:
- D-9-1.1 cardinality 6 (Goto, SwitchInt, Return,
  Unreachable, Call, Assert)
- D-9-1.2 `Goto(BlockId)`
- D-9-1.3 `SwitchInt { discr, targets }`
- D-9-1.4 `Return` / `Unreachable` unit variants
- D-9-1.5 `Call { callee, args, destination, target }`
- D-9-1.6 Assert kept at Terminator level (not Statement
  Charon-style); payload `Assert { cond, expected, msg,
  target }`
- D-9-2 Drop-as-Call collapse at the bridge
- D-9-3 `CallCallee::{Direct(InstanceId),
  Indirect(Operand)}` -- the load-bearing decision; (b)
  candidate; no stage-1 revision
- D-9-4 drop per-arg spans entirely
- D-9-5 8-variant AssertKind (drop 3 coroutine variants)
- D-9-6 SwitchTargets explicit split
  (`cases: Box<[(u128, BlockId)]>` + `otherwise: BlockId`)
- D-9-7 inline storage (no Terminator interning)
- D-9-8 wrapper `Terminator { kind, span }` (drop
  source_info.scope / Call.call_source / Call.fn_span /
  unwind)
- D-9-9 9-row rejection table (6 pipeline-discharged + 2
  DSL-only + 1 collapsed via D-9-2)
- D-9-10 invariants registry (5-part: Part A rejection
  cross-ref, Part B 2 inherited + 7 introduced positive
  invariants B-0a/c + B-1-B-7, Part C walk-impl deferred
  to stage 11, Part D F4/F4-2 OUT, Part E peer
  comparison)
- D-9-K `render_terminator` + 4 sub-renderers deferred
  to stage 11 (closes the D-5-K through D-9-K
  deferral cascade)

Open phase set-up recap:
spec-snapshot.md copied from top-level `../spec.md`
(8197 lines); inbound expectations.md read (4 entries
from stages 1, 3, 6); rustc kickoff evidence pass
completed (15-variant `TerminatorKind<'tcx>`,
cleanup-pass discharge ordering verified --
`CleanupPostBorrowck` rewrites FalseEdge/FalseUnwind
to Goto unconditionally with `is_required: true`;
`ElaborateDrops`, `AbortUnwindingCalls`,
`coroutine::StateTransform` discharge sequence pinned);
11 D-tags allocated [DRAFTED]; per-stage prior-art
research landed in 4 primary-source files (rustc.md
2016 lines, rustc_public.md 1342 lines, charon.md 1634
lines, mrustc.md 1128 lines = 6120 lines total);
memory.md and this status.md drafted from grounded
knowledge.

Last updated: 2026-05-05 (Phase A closed; all 11
D-tags + 5 sub-decisions LOCKED; ready for Phase B
spec-snapshot drafting + Codex review).

For top-level project status, see `../status.md`. For
project-wide invariants, see `../README.md`. For stage-9
invariants (workflow, principles, read order, Codex
prompt template), see `memory.md`. For the working spec,
see `spec-snapshot.md`. For the audit trail, see
`discussions.md` (lazy; created when the first decision
lands). For the round-by-round Codex audit trail, see
`review-log.md` (lazy; created at first round). For
inbound forward refs, see `expectations.md` (4 entries
from stages 1, 3, 6).

## 0. Inbound expectations checklist

From `expectations.md`:

- [ ] **Stage 1 -- `Drop` lowered to `Call`.** Bridge
  converts `TerminatorKind::Drop` into a regular `Call`
  targeting the drop-glue Instance. Stage 9 has no
  separate `Drop` variant. Closed by D-9-2.
- [ ] **Stage 1 -- Vtable-indirect dispatch (Call
  terminator shape).** Stage 9 owns the Call terminator
  shape that expresses dynamic-dispatch dispatch through
  a vtable-loaded function pointer. Stage 12 owns vtable
  type and vtable instance Item synthesis; stage 11 owns
  the body-level rewrite that recognises `Cast::Unsize`
  coercions and rewrites `<dyn Trait as Trait>::method`
  calls into the indirect form. Closed by D-9-3 (callee
  shape) + outbound-to-stage-11 expectation (rewrite-pass
  ownership) + outbound-to-stage-12 expectation (vtable
  shim Item synthesis -- already filed at
  `../stage-12/expectations.md`).
- [ ] **Stage 1 -- Possibly extend `DefPathData` for
  synthetic vtable scopes.** If D-9-3 resolution requires
  a *new* `DefPathData` variant for synthetic vtable
  scopes (rather than overloading an existing variant),
  appending it is a stage-1 revision per the toolchain-bump
  methodology event. Trigger Codex review of the addition.
  TBD by D-9-3 resolution; outbound-to-stage-12
  expectation captures the vtable-DefPath piece.
- [ ] **Stage 1 -- Closes construction invariant 9.**
  Once stage 9 introduces call-site references to
  `InstanceId`, `validate`'s invariant 9 (instance
  closure-completeness) becomes enforceable
  non-vacuously. Walk-implementation deferred to stage
  11 per D-9-10 + D-8-4 Part C precedent. Closed by
  D-9-10 (rules) + outbound-to-stage-11 expectation
  (walk implementation).
- [ ] **Stage 3 D5 -- Define Call terminator shape
  supporting vtable-indirect dispatch.** Two design
  candidates: (a) public `InstanceKind::Virtual { idx }`
  variant addition to stage-1 InstanceKind (a stage-1
  revision) -- direct precedent in rustc_public at
  `mono.rs:30`; (b) widen `Call.callee` to
  `CallCallee::{Direct(InstanceId), Indirect(Operand)}`
  (stage-9-only widening; no stage-1 revision) -- direct
  precedent in Charon at `gast.rs:386-391` +
  `transform_dyn_trait_calls.rs:31-122`. Resolution
  pending Phase A. Closed by D-9-3.
- [ ] **Stage 6 D-6-1.3 -- Capture Call / TailCall
  per-argument spans via `Spanned<Operand>` at the
  Terminator level.** Stage 6 dropped the operand-internal
  span; stage 9 picks the per-arg span shape. Two
  candidates from stage 6 inbound: (a) inline
  `ArgWithSpan { operand: Operand, span: Span }` wrapper;
  (b) parallel slabs (`args: Box<[Operand]>` plus
  `arg_spans: Box<[Span]>`). New third option from
  rustc_public prior-art: (c) drop per-arg spans
  entirely (rustc_public's `arg.node.stable(...)` at
  `mir.rs:752` strips the Spanned wrapper). Resolution
  pending Phase A. Closed by D-9-4. (TailCall is
  rejected per D-9-9, so D-9-4 only governs Call.)

Stage 9 also files outbound expectations to:
- **stage-11/expectations.md** -- `render_terminator`
  deferral (D-9-K); body-level vtable-indirect rewrite
  pass (D-9-3 candidate (b) ownership); Terminator walk
  implementation per D-9-10 Part C; per-arg span
  preservation if D-9-4 resolves to (a) or (b).
- **stage-12/expectations.md** -- vtable shim Item
  synthesis (already filed by stage 1; cross-ref).
  Possible new `DefPathData` variant if D-9-3 resolution
  requires (a stage-1 revision).
- **stage-1/expectations.md** -- if D-9-3 resolution
  selects candidate (a), this triggers a stage-1 revision
  (reopen InstanceKind closure to add `Virtual { idx }`)
  + Codex review per the toolchain-bump methodology
  event.

## 1. D-tag allocation (DRAFTED; pending Phase A)

D-9-1 through D-9-K inclusive, 11 parent tags + 6
sub-decisions. Mirrors stage-8's tag density (5 + 4
sub = 9). Sub-decisions enumerate per-aspect choices that
emerge from variant-by-variant payload review.

- **D-9-1 [DRAFTED]** -- Closed `TerminatorKind` surface
  (variant cardinality + per-variant payload shapes).
  Proposed 6 variants: `Goto`, `SwitchInt`, `Return`,
  `Unreachable`, `Call`, `Assert`. Narrower than rustc's
  15, narrower than rustc_public's 10, comparable to
  Charon's ~7 (Charon classifies Assert as Statement)
  and mrustc's ~10 (mrustc classifies Drop as Statement).
  - **D-9-1.1 [LOCKED 2026-05-05]** -- Cardinality
    fixed at 6 variants: `Goto`, `SwitchInt`, `Return`,
    `Unreachable`, `Call`, `Assert`. Smallest closed
    surface that covers well-formed driver MIR
    post-pipeline. All other rustc terminator variants
    are pipeline-discharged (FalseEdge / FalseUnwind /
    Yield / CoroutineDrop / UnwindResume /
    UnwindTerminate; rustc-`Drop`-as-separate-variant
    collapsed at bridge per D-9-2) or DSL-only
    (TailCall / InlineAsm). Locks the scope; per-variant
    payload shapes follow at D-9-1.2-1.6.
  - **D-9-1.2 [LOCKED 2026-05-05]** --
    `Goto(BlockId)` payload: single field, flat tuple.
    Universal across rustc / rustc_public / Charon /
    mrustc; no alternatives worth surveying. Bridge:
    `mir::TerminatorKind::Goto { target }` ->
    `Goto(<bridge_block_id>)`.
  - **D-9-1.3 [LOCKED 2026-05-05]** --
    `SwitchInt { discr: Operand, targets: SwitchTargets }`
    payload. Two fields: `discr` is the value being
    tested; `targets` carries the (value -> block)
    mapping plus the otherwise destination per D-9-6.
    Bridge translation: rustc `discr` translates via
    stage-6 Operand machinery; `targets` translates per
    D-9-6 explicit-split rule.
  - **D-9-1.4 [LOCKED 2026-05-05]** -- `Return` and
    `Unreachable` are unit variants (no payload). Return's
    return-place is implicitly `_0` (per stage-1
    `LocalId::RETURN_PLACE` + stage-11 LocalDecls
    outbound expectation about `LocalId::RETURN_PLACE`);
    type comes from the body's `FnSig.output()` (stage-11
    concern). Unreachable is the terminating UB marker;
    emitted naturally from `loop {}` patterns and as the
    output of cleanup-pass simplifications. Universal
    across all four prior-art projects as unit variants.
  - **D-9-1.5 [LOCKED 2026-05-05]** -- `Call` payload
    fully determined by D-9-3 + D-9-4 + D-9-8:
    ```
    Call {
      callee:      CallCallee,        // D-9-3
      args:        Box<[Operand]>,    // D-9-4 (c)
      destination: Place,
      target:      Option<BlockId>,
    }
    ```
    Per-field rationale: (i) `callee` per D-9-3
    `Direct(InstanceId)` / `Indirect(Operand)` four-case
    bridge translation rule; (ii) `args` per D-9-4 (c)
    `arg.node` strip -- bare Operand sequence;
    `Box<[T]>` matches stage-7 `Aggregate`'s
    storage-shape rule (slim-pointer + length); (iii)
    `destination: Place` always present in
    `optimized_mir` (rustc pre-allocates destination
    post-cleanup; `Option<Place>` source-MIR shape is
    gone); (iv) `target: Option<BlockId>` -- `Some(bb)`
    means normal-return successor; `None` means
    divergent call (`panic!()`, `-> !` callee, etc.);
    rustc passthrough on semantics. Drop
    `unwind`/`call_source`/`fn_span` per D-9-8.
  - **D-9-1.6 [LOCKED 2026-05-05]** -- `Assert` is
    classified as a Terminator (not demoted to
    Statement Charon-style). Three load-bearing
    reasons: (i) bridge structural transparency --
    Strategy A (keep as Terminator) is a one-to-one
    variant rename + payload trim; Strategy B requires
    either Goto-synthesis noise (B.1) or a real CFG
    flattening pass (B.2) which violates our
    `mir_opt_level=0` discipline of pushing CFG opts
    upstream of the bridge; (ii) CFG-shape stability --
    stage-11 body-level passes (e.g., D-9-3 candidate
    (b)'s vtable-indirect rewrite) iterate terminators
    and need "this block has a control branch" as a
    terminator-variant property, not a statement-walk
    derivation; (iii) no information advantage --
    codegen output is identical in both strategies, so
    Strategy B's Statement-level uniformity buys
    nothing under our straight-through C codegen.
    Charon's precedent doesn't transfer (Aeneas
    verification-semantics target); rustc / rustc_public
    / mrustc all classify Assert (or its Panic analog)
    as Terminator. Payload shape: `Assert { cond:
    Operand, expected: bool, msg: AssertKind, target:
    BlockId }` with AssertKind subset per D-9-5; drop
    `unwind` per D-9-8 (under panic=abort the
    surviving values carry no driver-relevant
    information).
- **D-9-2 [LOCKED 2026-05-05]** -- Drop-as-Call
  lowering at the bridge.
  `TerminatorKind::Drop { place, target, unwind, replace,
  drop, async_fut }` is converted unconditionally by the
  bridge into a synthetic `TerminatorKind::Call` targeting
  the drop-glue `Instance` for `place_ty`. Stage-9 surface
  has **no separate `Drop` variant**.

  **Bridge translation rule (5 steps; implementer's
  reference).**
  1. **Resolve drop-glue Instance.** Compute
     `place_ty = place.ty(local_decls, tcx)`; call
     `Instance::resolve_drop_in_place(tcx, place_ty)` to
     obtain the drop-glue `Instance<'tcx>`; bridge-intern
     as a RegseqModel `InstanceId` via the
     `RustcInstanceKindTag::DropGlue` discriminator on the
     F4-2 RegseqInstanceKey (per stage-1 instance-closure
     machinery; ensures drop-glue InstanceIds stay
     distinct from `Item`-kind InstanceIds of the same
     DefId).
  2. **No ZST elision at the bridge** (policy b.1).
     Always emit the Call to the no-op drop-glue when
     `!place_ty.needs_drop(...)`; let stage-14 codegen
     peephole `Call(no_op_drop_glue)` -> `<no emission>`.
     (Phase-B fixture should verify whether
     whitelisted-`RemoveZsts` already elides ZST drops
     upstream; bridge spec stays unconditional.)
  3. **Synthesize the Call.**
     ```
     Terminator::Call {
       callee:      <D-9-3 shape>(drop_glue_instance_id),
       args:        Box::new([Operand::Move(<&raw mut
                    place> temp)]),
       destination: <fresh unit-typed temp>,
       target:      Some(target),  // copied from Drop
     }
     ```
  4. **Synthesize the `&raw mut place` operand**
     (policy b.i). Bridge prepends
     `Statement::Assign(<fresh_local>, Rvalue::RawPtr(
     Mutability::Mut, place))` to the current basic
     block; the Call argument is `Operand::Move(<fresh_local>)`.
     Stage-6 `Operand` does NOT admit an `&raw mut place`
     form directly (per D-6-1's 3-variant surface), so
     pre-statement insertion is the only path. The fresh
     `LocalId` allocation is outbound to stage 11 (filed
     in `stage-11/expectations.md`).
  5. **Synthesize the unit return slot.** Drop-glue
     returns `()`. Allocate a fresh `LocalId` of type
     `Ty::Tuple([])` for the destination. Outbound to
     stage 11.

  **Implementer's reference points.**
  - **rustc-internal precedent.** rustc's codegen-SSA
    performs the same collapse internally inside the
    backend dispatch loop. See
    `rustc_codegen_ssa/src/mir/block.rs:599-693` --
    `codegen_drop_terminator` resolves drop-glue via
    `Instance::resolve_drop_in_place(tcx, ty)` and
    dispatches through `helper.do_call(...)` exactly as
    if the source was an explicit `drop_in_place::<T>(p)`
    call. The bridge performs the collapse one stage
    earlier so all RegseqModel consumers see uniform
    Call.
  - **Charon precedent.** Charon's optional
    `--desugar-drops` pass at
    `charon-lib/src/transform/normalize/desugar_drops.rs:23-72`
    is structurally identical: rewrites
    `Drop { place, target, ... }` into a `Call` with
    `&raw mut place` arg, fresh unit-typed return slot,
    copied `target`/`on_unwind` block ids. RegseqModel
    applies the same shape unconditionally at the
    bridge instead of as an opt-in normalize pass.
  - **Charon's belt-and-braces ZST elision.** Charon's
    `filter_trivial_drops` pass collapses no-op-destruct
    Drops directly to `Goto`. RegseqModel defers this to
    stage-14 codegen peephole instead (per policy b.1
    above), but the implementer can mirror the
    `filter_trivial_drops` shape if a future phase-B
    finding shows ZST-Call emission is wasteful.

  **Three load-bearing reasons** (per Phase-A
  discussion): (i) C-semantics with information
  preservation per README §2 -- Drop-as-Call IS the C
  shape; (ii) resolves construction invariant 9 closure
  correctness -- walker simply visits every Call's
  `InstanceId` rather than special-casing Drop;
  (iii) no information loss -- post-`ElaborateDrops`,
  Drop is "an unconditional call to drop_in_place";
  the variant's extras (`replace` / `drop` /
  `async_fut`) are diagnostic-only or async-coroutine-
  only (latter type-rejected at stage 3 §15.1).

  Closes stage-1 inbound expectations: "Drop lowered to
  Call" + drop-glue Instance interning bridge concern
  (`expectations.md` "From stage 1"). Outbound to
  `stage-11/expectations.md` "From stage 9":
  bridge-synthesized `&raw mut place` pre-statement
  Assign + fresh `LocalId` allocation + fresh
  unit-typed return slot.
- **D-9-3 [LOCKED 2026-05-05; LOAD-BEARING]** --
  Call callee shape: **`CallCallee::{Direct(InstanceId),
  Indirect(Operand)}`** (candidate (b) per the stage-3
  D5 inbound). Stage-1 InstanceKind closure stays
  intact at 6 variants; no stage-1 revision needed.

  **Concrete shape:**
  ```
  pub enum TerminatorKind {
    ... ,
    Call {
      callee:      CallCallee,
      args:        <D-9-4 shape>,
      destination: Place,
      target:      Option<BlockId>,
    },
  }

  pub enum CallCallee {
    Direct(InstanceId),
    Indirect(Operand),
  }
  ```

  No interning (matches D-9-7); structural derive of
  `PartialEq` / `Hash` for body-level identity.

  **Five load-bearing reasons** (per Phase-A
  discussion):
  1. **Required for completeness regardless of dyn
     Trait.** Function-pointer calls (case 2:
     `let f: fn(...) = ...; f(args)`) need a
     runtime-resolved callee position. Stage-1's
     emission-only InstanceKind closure cannot model
     "the callee is determined at runtime"; (a) has no
     clean home for case 2.
  2. **Preserves stage-1's emission-only InstanceKind
     design rationale.** No stage-1 revision; no Codex
     toolchain-bump round; the closed 6-variant set
     stays closed.
  3. **Proper IR variant per compiler theory.** Direct/
     Indirect call distinction is universal across
     mature IRs (LLVM `call` direct vs indirect on SSA
     value; Cranelift `call` vs `call_indirect`;
     mrustc's `CallTarget = Value(LValue) |
     Path(::HIR::Path) | Intrinsic` at
     `src/mir/mir.hpp:595-602`; Charon's
     `FnOperand::{Regular(FnPtr), Move(Place)}` at
     `charon-lib/src/ast/gast.rs:386-391`). Making it
     explicit at the IR level surfaces a structural
     distinction into the type system.
  4. **C-codegen shape match.** C distinguishes
     `name(args)` (direct) from `(*ptr)(args)`
     (indirect). The IR mirrors this directly.
  5. **Cleans up invariant 9 closure-walk.** Direct's
     InstanceId is closure-checked; Indirect's Operand
     has no InstanceId to check (function pointer's
     value is a runtime concern, not a closure
     concern).

  **Bridge translation rule (4 cases):**
  - rustc `Call.func = Operand::Constant(c)` where
    `c.const_.ty() = Ty::FnDef(def_id, args)` resolves
    to a non-Virtual Instance -> `Direct(<bridge-
    interned InstanceId>)`.
  - rustc `Call.func = Operand::Constant(c)` where the
    resolved Instance has rustc's
    `InstanceKind::Virtual(...)` -> bridge passes
    through as `Direct(<vtable shim InstanceId>)` if
    stage-12 has synthesised the shim, or rejects
    pending the rewrite pass; the body-level rewrite
    pass at stage 11 handles the dyn-Trait recognition.
  - rustc `Call.func = Operand::Move(p)` /
    `Operand::Copy(p)` where `p.ty() = Ty::FnPtr(...)`
    -> `Indirect(Operand::Move(p))` /
    `Indirect(Operand::Copy(p))`.
  - Anything else -> bridge-rejection (mistyped callee
    shape).

  **Stage-11 outbound expectation:** body-level rewrite
  pass that recognises `<dyn Trait as Trait>::method`
  Call sites (whose resolved Instance has rustc's
  `InstanceKind::Virtual`) and rewrites them to a
  chain of `Statement::Assign` extracting the vtable
  function pointer through `PtrMetadata` -> `Deref` ->
  `Field(method_idx)` projections + an
  `Indirect(Operand::Move(<final temp>))` callee.
  Charon's `transform_dyn_trait_calls.rs:31-122` is the
  direct template; mrustc's `MIR_Cleanup_Virtualize` at
  `cleanup.cpp:548-684` is a structurally similar peer.
  Filed in `stage-11/expectations.md` "From stage 9".

  **Stage-12 outbound expectation:** vtable shim Item
  synthesis (already filed by stage 1; cross-ref). No
  new `DefPathData` variant needed under (b) since
  vtable shims appear as Items, not as a new InstanceKind
  variant (closes the stage-1 inbound "Possibly extend
  DefPathData for synthetic vtable scopes" question
  conditionally on (b) -- under (b), no extension is
  required).

  **Foreclosed alternatives.** Candidate (a) -- adding
  `InstanceKind::Virtual { idx }` to stage-1
  InstanceKind -- would have required a stage-1 spec
  revision and blurred the emission-only design rule;
  more importantly, it would handle only case 3
  (vtable dispatch) and leave case 2 (function-pointer
  calls) without a clean home. (b) handles cases 2-3
  uniformly through the `Indirect(Operand)` arm.

  Closes stage-1 inbound expectation "Vtable-indirect
  dispatch (Call terminator shape)" and stage-3 D5
  inbound expectation; closes stage-1 inbound "Possibly
  extend `DefPathData` for synthetic vtable scopes"
  (under (b), no extension is required).
- **D-9-4 [LOCKED 2026-05-05]** -- Per-arg span
  preservation: **(c) drop entirely**. `args:
  Box<[Operand]>` -- bare Operand sequence; no per-arg
  span carried. Per-arg diagnostic granularity falls
  back to the Terminator-level `span` (the call site as
  a whole).

  **Bridge translation:** for each rustc
  `Call.args: Box<[Spanned<Operand<'tcx>>]>`,
  `args.iter().map(|sp| translate_operand(&sp.node)).collect()`
  -- strips the `Spanned` wrapper, drops `sp.span`.
  Mirrors rustc_public's bridge translator at
  `rustc_public_bridge/src/unstable/convert/stable/mir.rs:752`.

  **Three load-bearing reasons** (per Phase-A
  discussion):
  1. **No load-bearing consumer.** Stage-11
     `validate(...)` walk uses structural type-equality
     / arity-match invariants (not span-keyed); stage-11
     body-level rewrite passes operate on Operand
     structure (not source position); stage-14 codegen
     does not emit spans. Compiler-internal driver-
     source diagnostics run pre-bridge at the rustc/HIR
     layer (per-arg spans there, not at RegseqModel
     level). Speculative future consumers (phase-2
     device-pressure debugging, codegen-time arg
     warnings) have workable Terminator-level
     fallbacks; for driver code where calls typically
     take 0-4 args, granularity loss is bounded.
  2. **No defensive preservation.** Stage 6 D-6-1.3
     deferred the question to stage 9 specifically so
     the consumer landscape could be examined with full
     IR visibility. Examined; no consumer warrants the
     storage overhead. Per the README §2
     "no defensive fallbacks" methodology rule.
  3. **rustc_public peer alignment + consistency with
     D-9-8.** rustc_public (closest existing owned-IR
     analog) drops per-arg spans; D-9-8 drops
     `source_info.scope`, `Call.call_source`,
     `Call.fn_span`, `Call.unwind` with the same
     "no driver-relevant consumer" rationale. Per-arg
     spans fall in the same category.

  **Foreclosed alternatives.** Candidate (a) inline
  wrapper struct (`Box<[ArgWithSpan]>`) and candidate
  (b) parallel slabs (`Box<[Operand]>` + `Box<[Span]>`)
  both preserve per-arg span granularity at storage cost
  per arg. Both forecloseed under the "no load-bearing
  consumer" finding. Stage 6's D-6-1.3 inbound listed
  (a) and (b); (c) was surfaced by rustc_public
  prior-art and adopted.

  Closes stage-6 D-6-1.3 inbound expectation
  ("`Spanned<Operand>` per-arg span preservation") with
  the (c) drop disposition.
- **D-9-5 [LOCKED 2026-05-05]** -- `AssertKind`
  subset: **(β) 8-variant subset**.
  ```
  pub enum AssertKind {
    BoundsCheck { len: Operand, index: Operand },
    Overflow(BinOp, Operand, Operand),
    OverflowNeg(Operand),
    DivisionByZero(Operand),
    RemainderByZero(Operand),
    MisalignedPointerDereference {
      required: Operand,
      found:    Operand,
    },
    NullPointerDereference,
    InvalidEnumConstruction(Operand),
  }
  ```
  Drop the 3 coroutine-only variants
  (`ResumedAfterReturn`, `ResumedAfterPanic`,
  `ResumedAfterDrop` per
  `rustc_middle/src/mir/syntax.rs:1072-1074`); keep the
  8 driver-relevant variants.

  **Three load-bearing reasons** (per Phase-A
  discussion):
  1. **Type-rejection cascade consistency.** Stages 5-8
     each reject variants whose precondition is
     type-rejected at stage 3 (coroutines, Box, etc.).
     The 3 coroutine-only Assert variants fit this
     discipline cleanly: their types cannot reach
     RegseqModel (stage-3 §15.1 rejects `Coroutine`
     types), so the variants are structurally
     unreachable. Carrying unreachable variants is the
     same anti-pattern as carrying unreachable types --
     README §2's "no defensive fallbacks" rule applies.
  2. **Diagnostic-quality preservation for driver-
     relevant cases.** Driver code does encounter the
     8 retained variants (bounds checks, arithmetic
     overflow in MMIO field manipulation, division
     by zero in clock-rate computation, null/misaligned
     pointer derefs in raw-pointer driver code,
     enum-construction validation). Each lowers to a
     specific `__rust_abort_with_msg(format_str,
     operands...)` call providing useful runtime
     diagnostics. Charon's (γ) 3-variant collapse to
     bare `AbortKind` would forfeit this.
  3. **rustc_public-aligned with a single deliberate
     divergence.** rustc_public preserves all 11; we
     drop 3 on the type-rejection-cascade rule. The
     divergence is principled, documentable as
     "AssertKind variants whose precondition is
     type-rejected at stage 3 are pruned," and aligned
     with the stage-7/-8 discipline.

  **Bridge translation rule.** For each rustc
  `AssertKind<Operand<'tcx>>`: 8 variants translate
  one-to-one (with operands routed through stage-6
  Operand machinery and BinOp through stage-7 BinOp
  machinery); 3 coroutine variants
  (`ResumedAfterReturn` / `ResumedAfterPanic` /
  `ResumedAfterDrop`) emit a bridge-rejection
  diagnostic naming stage-3 §15.1 type-rejection as
  the discharge mechanism (double-discharged with
  `coroutine::StateTransform`).

  **Foreclosed alternatives.** (α) preservation of all
  11 variants -- structurally unreachable variants
  carry no driver-relevant content; (γ) Charon-style
  3-variant `AbortKind` collapse -- forfeits
  diagnostic information codegen could surface as
  message-bearing panic helper calls.

  **D-9-1.6 payload now fully resolved:**
  ```
  Assert {
    cond:     Operand,
    expected: bool,
    msg:      AssertKind,    // 8-variant per D-9-5
    target:   BlockId,
  }
  ```
- **D-9-6 [LOCKED 2026-05-05]** -- `SwitchTargets`
  storage: **(b) explicit split**.
  ```
  pub struct SwitchTargets {
    pub cases:     Box<[(u128, BlockId)]>,
    pub otherwise: BlockId,
  }
  ```
  Three reasons: (i) explicit > implicit -- the
  otherwise destination is intrinsically different from
  a case block (catch-all, not value-mapped), and
  positional encoding (rustc-mirror candidate (a))
  hides that distinction; (ii) no runtime invariant to
  enforce -- type system enforces the otherwise field
  is present, vs (a)'s `targets.len() == values.len() +
  1` runtime check; (iii) compositional with the
  zero-cases edge case -- under (b), `cases: []` +
  `otherwise: bb` is natural; under (a), `values: []` +
  `targets: [otherwise]` is a len-1 special case.
  Aligned with the README §2 methodology rule:
  "Compositional behavior beats special-case flags."
  Closest peer is rustc_public's
  `SwitchTargets { branches: Vec<(u128, BasicBlockIdx)>,
  otherwise: BasicBlockIdx }`. Pair-locality on the
  case-search walk; bridge translation is one-pass over
  rustc's parallel arrays. Rustc's rationale for
  candidate (a) (in `rustc_middle/src/mir/syntax.rs:1014-1029`
  -- "sliceable type for `successors` impl Iterator")
  does not apply to RegseqModel.

  **Bridge translation:**
  ```
  let n = mir_targets.targets.len();
  let cases = (0..n-1).map(|i| (
    mir_targets.values[i].into(),    // Pu128 -> u128
    bridge_block_id(mir_targets.targets[i]),
  )).collect::<Box<[_]>>();
  let otherwise = bridge_block_id(mir_targets.targets[n-1]);
  SwitchTargets { cases, otherwise }
  ```

  Invariant on `discr` type ({`Ty::Bool`, `Ty::Char`,
  `Ty::Int(_)`, `Ty::Uint(_)`}) carried into D-9-10 as
  B-1.
- **D-9-7 [LOCKED 2026-05-05]** -- No Terminator
  interning. Inline storage in stage-10
  `BasicBlock.terminator: Terminator`. Three reasons:
  (i) universal precedent across rustc / rustc_public /
  Charon / mrustc -- no project interns terminators;
  (ii) no deduplication win -- only the unit variants
  `Return` / `Unreachable` are byte-shareable;
  `Goto(bb)` / `SwitchInt` / `Call` / `Assert` all carry
  body-local data (`BlockId`, embedded `Operand` /
  `Place`, target lists, callee `InstanceId`); (iii)
  pattern-match consistency with the stages 5-8
  inline-storage rule (Place projections D-5-x, Operand
  D-6-2, Rvalue D-7-2, Statement D-8-2). F4 / F4-2
  cross-build identity OUT of scope (per-build
  structural identity is sufficient).
- **D-9-8 [LOCKED 2026-05-05]** -- Wrapper struct
  shape: `Terminator { kind: TerminatorKind, span:
  Span }`. Direct precedent: rustc_public at
  `rustc_public/src/mir/body.rs:140`. Mirrors stage-8
  D-8-3. **Dropped fields:** (i) `source_info.scope:
  SourceScope` -- debuginfo-only DWARF-emission index;
  RegseqModel does not emit DWARF per stage-1 spec
  rejection of debuginfo storage; (ii) `Call.call_source:
  CallSource` -- 5-variant diagnostic enum carrying no
  C-codegen-relevant info; (iii) `Call.fn_span: Span`
  -- function-position diagnostic refinement subsumed
  by wrapper-level `span` or per-arg span per D-9-4;
  (iv) `Drop.replace`/`drop`/`async_fut` -- subsumed by
  D-9-2 collapse to Call (no Drop variant survives
  anyway); (v) `unwind: UnwindAction` on Call / Drop /
  Assert / InlineAsm -- under panic=abort with
  `AbortUnwindingCalls` having run at
  `rustc_mir_transform/src/lib.rs:651` per
  `abort_unwinding_calls.rs:47`, only `Continue` and
  `Unreachable` survive at our consumption point and
  collapse semantically (no cleanup edges); preserving
  the field would store a 1-bit flag with no consumer.
  No defensive fallback against hypothetical future
  `extern "C-unwind"` exposure -- if a future DSL
  extension surfaces unwinding-FFI, that's a new spec
  event with its own decision per the no-defensive-
  fallbacks methodology rule. **Span field type:**
  `span: Span` (matching stage-8 D-8-3; no change to
  stage-1 Span machinery). **Per-arg spans** are out of
  scope for D-9-8; lives at D-9-4.
- **D-9-9 [LOCKED 2026-05-05]** -- 9-row rejection
  table; two-category framing (pipeline-discharged vs
  DSL-only) per the rustc prior-art's load-bearing
  finding that `TailCall` and `InlineAsm` are NOT
  pipeline-discharged but reach `optimized_mir` under
  rustc's standard config when their feature gates are
  set.

  **Category 1: pipeline-discharged (6 rows).**
  Encountering one in well-formed `optimized_mir`
  indicates a pipeline-config bug; bridge emits an
  "internal error" diagnostic.
  - **Row 1**: `FalseEdge { real_target,
    imaginary_target }` -- `CleanupPostBorrowck`
    rewrites to `Goto(real_target)` at
    `rustc_mir_transform/src/cleanup_post_borrowck.rs:67-74`;
    pass is `is_required: true` per
    `cleanup_post_borrowck.rs:88-90`; runs in
    `run_analysis_cleanup_passes` at
    `rustc_mir_transform/src/lib.rs:618-634`.
  - **Row 2**: `FalseUnwind { real_target, unwind }` --
    same pass, same site at
    `cleanup_post_borrowck.rs:69`.
  - **Row 3**: `Yield { value, resume, resume_arg, drop }`
    -- `coroutine::StateTransform` discharges at
    `rustc_mir_transform/src/lib.rs:660`; stage-3 §15.1
    also type-rejects coroutines (double-discharged).
  - **Row 4**: `CoroutineDrop` -- same pass + same
    type-rejection.
  - **Row 5**: `UnwindResume` -- under panic=abort,
    `AbortUnwindingCalls` rewrites unwind edges to
    `UnwindAction::Unreachable` at
    `rustc_mir_transform/src/abort_unwinding_calls.rs:47`;
    cleanup blocks become unreachable; Resume
    terminators only live in cleanup blocks per
    `rustc_middle/src/mir/syntax.rs:725-727`;
    `SimplifyCfg::PreOptimizations` (in our
    cleanup-pass whitelist) prunes them.
  - **Row 6**: `UnwindTerminate(reason)` -- same
    panic=abort pruning path.

  **Category 2: DSL-only (2 rows).**
  rustc supports the variant in `optimized_mir`; the
  Regseq DSL surface does not include the source-level
  construct. Bridge emits a "driver source error"
  diagnostic naming a useful next step.
  - **Row 7**: `TailCall { func, args, fn_span }` --
    rustc supports under
    `#![feature(explicit_tail_calls)]` (unstable).
    Diagnostic: "tail calls are not supported in
    Regseq driver source; the `become` expression and
    `#![feature(explicit_tail_calls)]` are not part of
    the DSL surface".
  - **Row 8**: `InlineAsm { ... }` -- rustc supports
    unconditionally; no pipeline pass discharges it.
    Diagnostic: "inline assembly is not supported in
    Regseq driver source; use the domain-specific
    intrinsics (MMIO, synchronization, reset, clock,
    ...) for low-level operations".

  **Row 9 (collapse, not rejection): `Drop { place,
  target, ... }`** -- bridge-collapsed to synthetic
  Call per D-9-2; never *rejected*, never *passed
  through* as a stage-9 variant. Recorded in the table
  for completeness so the implementer doesn't list
  Drop as "supported as-is."

  Total: 8 rejected variants (rows 1-8) + 1 collapsed
  (row 9, D-9-2) = 9-row table covering all 9 rustc
  TerminatorKind variants outside the closed 6-variant
  surface from D-9-1.1.

  **Diagnostic discipline.** Rustc_public's bridge
  translator (`rustc_public_bridge/src/unstable/convert/stable/mir.rs:704-790`)
  surfaces the discipline distinction between
  `unreachable!()` (correctness precondition violated:
  rows 1-6 here) and `todo!()` (incomplete
  implementation: not applicable to rows 7-8 here, which
  are deliberate DSL-source rejections). RegseqModel
  bridge follows the same `unsupported_construct!`-style
  pattern stages 1, 3, 5, 6, 7, 8 use, with the message
  text distinguishing "internal error: pipeline
  misconfiguration" (rows 1-6) from "driver source
  error: ..." (rows 7-8).

  **Walk-pass enforcement deferred to stage 11.**
  `validate(...)` walk on the model body checks that no
  Terminator variant outside the closed 6-variant set
  per D-9-1.1 appears. Mostly tautological under the
  closed-enum surface, but covers the case where some
  future construction path (e.g., a body-level rewrite
  pass at stage 11) might violate the surface. Filed in
  `stage-11/expectations.md` "From stage 9" with the
  Part C walk implementation.
- **D-9-10 [LOCKED 2026-05-05]** -- 5-part invariants
  registry mirroring stage-8 D-8-4.

  **Part A: rejection table.** Cross-references D-9-9
  (8 rejected rustc variants + 1 collapsed via D-9-2).
  No new content; recorded for registry completeness so
  the implementer treats walk-checks and rejection-checks
  as a unified discipline.

  **Part B: positive invariants.** 2 inherited + 7
  stage-9-introduced.

  *Inherited (2):* stage-9 Terminator embeds Place
  (Call.destination) and Operand (multiple sites);
  Statement-level and Rvalue-level invariants do not
  propagate (those constructs do not appear in
  Terminator content).
  - **B-0a** -- All embedded Place content satisfies
    Place invariants per stage-5 §19.7.
  - **B-0c** -- All embedded Operand content satisfies
    Operand invariants per stage-6 §20.5.

  *Stage-9-introduced (7):*
  - **B-1** -- `SwitchInt.discr` Operand has type
    `∈ {Ty::Bool, Ty::Char, Ty::Int(_), Ty::Uint(_)}`.
    Stage-3 type-taxonomy match. Enum-discriminant
    SwitchInts go through a preceding
    `Rvalue::Discriminant` (stage-7) projection that
    produces an integer-typed Operand; SwitchInt itself
    never sees an enum-typed discr.
  - **B-2** -- `Assert.cond` Operand has type `Ty::Bool`.
    Stage-3 type-taxonomy match.
  - **B-3** -- Call destination type matches callee
    return type:
    - (Direct) `destination_ty == lookup_instance(
      callee_id).fn_sig.output()`.
    - (Indirect) `destination_ty == operand_ty(
      callee_operand).output()` where
      `operand_ty(callee_operand) = Ty::FnPtr(...)` per
      B-5.
  - **B-4** -- Call args length matches callee signature
    arity:
    - (Direct) `args.len() == lookup_instance(callee_id
      ).fn_sig.inputs().len()`.
    - (Indirect) `args.len() == operand_ty(
      callee_operand).inputs().len()`.
  - **B-5** -- Call callee is callable:
    - (Direct) `InstanceId` always callable by
      stage-1 instance-closure construction (closure
      verification per B-7).
    - (Indirect) `operand_ty(callee_operand) ∈
      {Ty::FnPtr(_)}`. NOT `Ty::FnDef` (that resolves
      to Direct), NOT closures (closures are
      pre-monomorphized to fn-pointer types per
      stage-7 D-7-1.6), NOT any other type.
  - **B-6** -- Per-arg type match. For each
    `i ∈ 0..args.len()`, `operand_ty(args[i]) ==
    callee_inputs[i]` where `callee_inputs` comes from
    the same source as B-4. Catches stage-11
    rewrite-pass bugs (e.g., the vtable-indirect
    rewrite passing the wrong receiver type).
  - **B-7** -- **Closes construction invariant 9.**
    Every `Direct(InstanceId)` referenced from any
    Terminator::Call resolves in `model.instances`.
    `Indirect(Operand)` callees do not pin an
    InstanceId and do not contribute to the closure
    check. Load-bearing closure-correctness invariant
    the stage-1 inbound expectation pinned for stage 9.

  **Part C: walk-implementation deferred to stage 11.**
  Same precedent as stage-8 D-8-4 Part C. Stage 11
  owns:
  - `Body::validate(...)` walk over all Terminators in
    all basic blocks.
  - Visitor-trait extensions surfacing Terminator,
    CallCallee, SwitchTargets at body-level walks.
  - Actual `assert!` / `assert_eq!` checks for B-0a/c
    and B-1-B-7.
  - Bridge-rejection diagnostic emission for D-9-9 row
    encounters (though most rejection happens at the
    bridge translation site itself, not at body-level
    walk).

  Filed in `stage-11/expectations.md` "From stage 9":
  Terminator walk-pass implementation + visitor-trait
  extension covering Terminator / CallCallee /
  SwitchTargets.

  **Part D: F4/F4-2 cross-build identity OUT of scope.**
  Per D-9-7 (no Terminator interning), Terminator is
  body-local content. Cross-build identity operates on
  `(InstanceKey)` per stage-1 F4-2; Terminator content
  is per-build structural and need not be canonicalized
  across build environments. Same scope reduction as
  stages 5, 6, 7, 8.

  **Part E: peer comparison (informational).**
  | Project | Style | Reference |
  |---------|-------|-----------|
  | rustc | `validate.rs` per-Terminator rules; `bug!()` on violation | `rustc_mir_transform/src/validate.rs` |
  | rustc_public | Light validation; consumers do their own checks | `rustc_public/src/mir/...` |
  | Charon | Walk-based well-formedness in `transform/check_*` | `charon-lib/src/transform/check_generics.rs` etc. |
  | mrustc | Two-layer validator; fatal `MIR_BUG` on violation | `src/mir/check.cpp`, `check_full.cpp` |
  RegseqModel takes the **walk-pass-deferred-to-stage-11**
  discipline matching stages 5-8.
- **D-9-K [LOCKED 2026-05-05]** -- `render_terminator`
  deferred to stage 11. Closes the deferral cascade
  D-5-K (render_place) -> D-6-K (render_operand) ->
  D-7-K (render_rvalue) -> D-8-K (render_statement) ->
  D-9-K (render_terminator). Stage 11 owns all five
  `render_*` methods together.

  **Stage-11 ownership at render time:** stage 11
  implements `render_terminator`,
  `render_terminator_kind`, `render_call_callee`,
  `render_switch_targets`, `render_assert_kind`
  alongside the four earlier render_* methods.

  **Render-time dependencies:**
  - Embedded Operand (SwitchInt.discr, Call args,
    Call.callee::Indirect, Assert.cond, AssertKind
    operands) -> render_operand at stage 11.
  - Embedded Place (Call.destination) -> render_place
    at stage 11.
  - CallCallee::Direct(InstanceId) -> symbol-name
    materialisation from `Item.def_path` at stage 12
    (Items + DefPath); render_terminator at stage 11
    forward-references stage-12 def-path rendering,
    matching how stage-8's render_statement lands at
    stage 11 with hooks to render_operand /
    render_place / render_rvalue.
  - BlockId -> stage-11 Body owns BlockId space.
  - LocalId (transitively via Place.head_local +
    LocalDecls.debug_name) -> stage 11 LocalDecls +
    var_debug_info.
  - Span -> stage 1 (already available).

  **§18.2 pretty.rs per-method ownership table update:**
  add rows for `render_terminator`, `render_call_callee`,
  `render_switch_targets`, `render_assert_kind` -- all
  marked "stage 11" in the implementation column.

  **Three-item outbound bundle to file in
  `stage-11/expectations.md` "From stage 9":**
  1. `render_terminator` deferral with the four
     associated sub-renderers (kind / callee / switch
     targets / assert kind).
  2. Terminator walk implementation per D-9-10 Part C.
  3. Body-level vtable-indirect rewrite pass per D-9-3
     (b) -- recognises rustc `InstanceKind::Virtual`-
     resolved Calls and rewrites them to
     `Indirect(Operand)` form via synthesised
     `PtrMetadata -> Deref -> Field(method_idx)`
     projection statements + fresh `LocalId`s. Charon's
     `transform_dyn_trait_calls.rs:31-122` is the
     direct template.

  Plus the bridge concerns from D-9-2 (Drop-as-Call):
  fresh LocalId allocation for the `&raw mut place`
  temp + fresh unit-typed return slot are stage-11
  LocalDecls concerns; filed in the same
  expectations bundle.

## 2. Cluster organization

Following stage-7 and stage-8 precedent: **single
combined cluster** covering all D-9-K parents +
sub-decisions. Stage-9 has neither parallelism need (the
decisions cascade across each other -- D-9-3 informs
D-9-1.5; D-9-4 informs D-9-1.5; D-9-5 informs D-9-1.6;
D-9-2 informs D-9-9 invariant scope) nor
horizontally-decomposable subspaces (no decision is
fully independent of the others). Solo cluster, no
`sessions/` subfolder.

This matches the stage-8 pattern: 4 per-stage rounds + 1
post-merge round; total 5-10 rounds expected.

## 3. Codex review cadence (status)

Stage 9 is at the **kickoff complete** state. Phase A
discussion has not yet started. Phase B review is
contingent on Phase A producing locked decisions.

Round budget: aim for 4-6 per-stage rounds + 2-4
post-merge rounds, mirroring stage-7 / stage-8
trajectories. Complexity drivers:
- D-9-3 conflicting prior-art may extend Phase A
  discussion.
- D-9-4 three-candidate split similarly.
- D-9-9 two-category framing is novel.
- D-9-10 invariant chain inherits 4 layers (B-0a-d) and
  introduces 6 new ones (B-1-6); expect Codex pushback
  on type-equality / arity-match wording.

Codex prompt template lives in `memory.md` §4.

## 4. Active focus

**Phase A discussion: combined cluster opened
2026-05-05.** Decision discussion order (decisions
marked LOCKED have closed):
1. ~~D-9-1.1 cardinality~~ **LOCKED** (6 variants).
2. ~~D-9-7 no interning~~ **LOCKED**.
3. ~~D-9-8 wrapper struct~~ **LOCKED**.
4. ~~D-9-2 Drop-as-Call collapse~~ **LOCKED**.
5. ~~D-9-9 rejection table~~ **LOCKED**.
6. ~~D-9-3 callee shape~~ **LOCKED** (b: Direct/Indirect
   enum).
7. ~~D-9-4 per-arg span~~ **LOCKED** (c: drop entirely).
8. ~~D-9-1.2 (Goto) / D-9-1.4 (Return, Unreachable) /
   D-9-1.5 (Call)~~ **LOCKED batched** 2026-05-05.
9. ~~D-9-1.3 (SwitchInt) / D-9-6 (SwitchTargets explicit
   split)~~ **LOCKED** 2026-05-05.
10. ~~D-9-5 AssertKind subset~~ **LOCKED** (β: 8-variant
    subset).
11. ~~D-9-10 invariants registry~~ **LOCKED** 2026-05-05.
12. ~~D-9-K render deferral~~ **LOCKED** 2026-05-05.

**Phase A closed 2026-05-05.** All 11 D-tags + 5
sub-decisions LOCKED. Cluster is fully drafted in
conversation.

**Phase B next (sequential):**
- File outbound expectations to
  `../stage-11/expectations.md` "From stage 9" (5-item
  bundle: render_terminator deferral, Terminator walk
  implementation, vtable-indirect rewrite pass, fresh
  LocalIds for Drop-as-Call temps, fresh unit-typed
  return slot).
- Cross-ref outbound to `../stage-12/expectations.md`
  (vtable shim Item synthesis already filed by stage 1;
  closes "Possibly extend DefPathData" question with
  "no extension required" disposition under D-9-3 (b)).
- Draft `spec-snapshot.md` new §23 Terminator
  vocabulary section with audit shape per decision (D-9-1
  through D-9-10 + D-9-K).
- Update `spec-snapshot.md` §18.2 per-method ownership
  table for the four new render_* methods.
- Create `discussions.md` with audit-shape decision
  records.
- Run Codex review rounds (4-6 per-stage rounds
  expected, mirroring stage-7/-8 trajectories).
- Iterate to APPROVE.
- Hand off to merger contract per README §5.5.

## 5. What this file is NOT

- **Not the briefing.** Use `memory.md` for invariants
  (mission, workflow, principles, read order, Codex
  prompt template).
- **Not the spec or audit trail.** Use `spec-snapshot.md`
  and `discussions.md`.
- **Not append-only.** Update sections in place when
  state changes (locked decisions flip [DRAFTED] ->
  [LOCKED]; active cluster shifts; round count
  advances).
