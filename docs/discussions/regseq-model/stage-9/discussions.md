# Stage 9 -- Discussion Record

Distilled record of the design dialogue producing the
stage-9 `Terminator` vocabulary spec. Each entry captures
the question raised, key options considered, the decision,
the reasoning chain, methodology rules invoked, prior-art
evidence, and cross-references to `spec-snapshot.md` (the
spec) and `review-log.md` (the round-by-round audit
trail; created on first review round).

Status of decisions in this file mirrors `spec-snapshot.md`'s
"Terminator vocabulary" section (§23) plus adjacent
additions to §18.2, plus `status.md` "D-tag allocation".

D-tag convention: prefixed `D-9-K` form per `../README.md`
§5.2.

Status tags:
- `[LOCKED]` -- audit-shape recorded; per-stage Phase A
  closed; ready for Phase B Codex review.
- `[REOPENED]` -- a later sub-decision or sibling D-tag
  forced a revision; superseded entry kept for history.
- `[DEFERRED to stage N]` -- moved to a later stage's scope.

Phase A closed 2026-05-05 with all 11 D-tags + 5
sub-decisions LOCKED across a single combined cluster.

---

## 1. Decision discussions

### D-9-1 -- `Terminator` core vocabulary [LOCKED]

**Question.** What is the closed `TerminatorKind` value
shape that the bridge fills from rustc's
`mir::TerminatorKind<'tcx>` and that downstream consumers
(stage 10 basic blocks -- `BasicBlock.terminator:
Terminator`; stage 11 bodies; stage 14 backends) read as
the per-block control-flow-edge surface?

**Key options.**
1. **Mirror rustc verbatim (15 variants).** Carry every
   `mir::TerminatorKind<'tcx>` arm including borrowck
   artifacts (FalseEdge, FalseUnwind), coroutine ops
   (Yield, CoroutineDrop), unwinding terminators
   (UnwindResume, UnwindTerminate), specialized variants
   (TailCall, InlineAsm), and the raw Drop variant.
2. **Mirror rustc_public (10 variants).** Drop the 5
   borrowck/coroutine artifacts; preserve Drop and
   InlineAsm.
3. **Charon-style ~7 variants** with Assert as a Statement
   (not Terminator).
4. **mrustc-style ~10 variants** with Drop as a Statement.
5. **RegseqModel 6 variants.** Smallest closed surface
   that covers well-formed driver MIR post-pipeline:
   Goto, SwitchInt, Return, Unreachable, Call, Assert.

**Decision.** Option 5 -- **6 variants** (D-9-1.1
cardinality), with Drop collapsed at the bridge (D-9-2),
Assert kept at Terminator level (D-9-1.6), and 8 rustc
variants reject-on-encounter (D-9-9).

**Reasoning chain.**
1. The `optimized_mir` pipeline plus our cleanup-pass
   whitelist eliminates 6 rustc variants
   unconditionally (FalseEdge / FalseUnwind via
   `CleanupPostBorrowck`; Yield / CoroutineDrop via
   `coroutine::StateTransform`; UnwindResume /
   UnwindTerminate via `AbortUnwindingCalls` +
   `SimplifyCfg::PreOptimizations`).
2. The DSL surface excludes 2 rustc variants
   structurally: TailCall (requires unstable
   `explicit_tail_calls`) and InlineAsm (DSL provides
   domain-specific intrinsics for low-level operations).
3. Drop is post-`ElaborateDrops` semantically equivalent
   to a call to `drop_in_place::<T>(p)`; collapsing to
   Call at the bridge is the C-shape lowering per
   README §2 "C-semantics with information preservation."
4. Assert is structurally a guarded conditional branch,
   not a statement-level side-effect; its `target` and
   (post-AbortUnwindingCalls implicit) panic-edge make
   it a control-flow construct.

**Methodology rules invoked.**
- README §2 "C-semantics with information preservation"
  (drives Drop-as-Call collapse).
- README §2 "No defensive fallbacks" (rejected variants
  do not get carried defensively).
- README §2 "Closed `Ty` enum" pattern (closed-form
  surface; reject-on-encounter for non-conforming).
- README §2 "Compositional behavior beats special-case
  flags" (per-variant dispatch via the closed enum, not
  flag-shaped discriminators).

**Prior-art evidence.**
- rustc 15-variant `TerminatorKind<'tcx>` at
  `rustc_middle/src/mir/syntax.rs:703-987`.
- rustc_public 10-variant `TerminatorKind` at
  `rustc_public/src/mir/body.rs:154-192`.
- Charon's ~7-variant `RawTerminator` (Assert as
  Statement) at `charon-lib/src/ast/ullbc_ast.rs:41`.
- mrustc's ~10-variant tagged union (Drop as Statement)
  at `src/mir/mir.hpp:710-715`.

**Cross-refs.**
- `spec-snapshot.md` §23.1.2 (TerminatorKind enum +
  comparison table).
- Sub-decisions: D-9-1.1 (cardinality), D-9-1.2 (Goto
  payload), D-9-1.3 (SwitchInt payload), D-9-1.4
  (Return / Unreachable payloads), D-9-1.5 (Call
  payload), D-9-1.6 (Assert payload + Terminator-level
  classification).

---

### D-9-1.1 -- TerminatorKind cardinality [LOCKED]

**Question.** Specifically how many variants in the
closed `TerminatorKind` set, and which?

**Key options.**
- **5 variants:** Goto, SwitchInt, Return, Unreachable,
  Call (Assert demoted to Statement Charon-style; D-9-2
  collapses Drop).
- **6 variants** (recommended): + Assert.
- **7 variants:** + Drop preserved.
- **8+ variants:** + Drop + InlineAsm.

**Decision.** **6 variants** -- Goto, SwitchInt, Return,
Unreachable, Call, Assert.

**Reasoning.**
1. The 6-variant cardinality follows from three
   sub-claims locked elsewhere: (a) D-9-2 collapses Drop
   at the bridge (no Drop variant); (b) D-9-1.6 keeps
   Assert at Terminator level (not Statement); (c)
   D-9-9 rejects InlineAsm + TailCall (DSL-only) and
   the 6 pipeline-discharged variants.
2. Each rejected variant is provably unreachable
   (pipeline-discharged) or out-of-DSL (DSL-only); no
   variant is added defensively.
3. The 6-variant set is the **smallest closed surface**
   that fully expresses well-formed driver MIR
   post-pipeline.

**Methodology rules invoked.**
- README §2 "Closed `Ty` enum" (closed-form pattern at
  the variant level).
- README §2 "No defensive fallbacks."

**Prior-art evidence.** Comparative cardinality:
- rustc 15 / rustc_public 10 / Charon ~7 (Assert
  demoted) / mrustc ~10 (Drop demoted) / **RegseqModel
  6**. The narrowest precedented closed surface;
  intermediate-narrow forms (Charon/mrustc) trade
  Terminator slots for Statement slots, which our
  D-9-1.6 / D-9-2 decisions both reject.

**Cross-refs.**
- `spec-snapshot.md` §23.1.2 (closed 6-variant enum) +
  §23.1.3 (what's dropped).
- D-9-2 (Drop collapse), D-9-1.6 (Assert classification),
  D-9-9 (rejection table).

---

### D-9-1.2 -- `Goto(BlockId)` payload [LOCKED]

**Question.** What is the payload shape of `Goto`?

**Decision.** Single-field flat tuple
`Goto(BlockId)` -- the target block.

**Reasoning.** Universal across rustc / rustc_public /
Charon / mrustc; no alternatives worth surveying. The
`BlockId` resolves to a basic block in the enclosing
body's `basic_blocks` table (stage-10 vocabulary). Bridge
translation: `mir::TerminatorKind::Goto { target } ->
Goto(<bridge_block_id>)`.

**Cross-refs.** `spec-snapshot.md` §23.2.1.

---

### D-9-1.3 -- `SwitchInt { discr, targets }` payload [LOCKED]

**Question.** What is the payload shape of `SwitchInt`?

**Decision.** Two named fields:
```rust
SwitchInt {
    discr:   Operand,
    targets: SwitchTargets,
}
```
where `SwitchTargets` storage is per D-9-6.

**Reasoning.**
1. `discr`'s type ∈ {`Ty::Bool`, `Ty::Char`, `Ty::Int(_)`,
   `Ty::Uint(_)`} (per §23.6.2 B-1; stage-3 type-taxonomy
   match). Enum-discriminant switches reach SwitchInt
   only via a preceding `Rvalue::Discriminant` (stage 7)
   projection that produces an integer-typed Operand;
   SwitchInt itself never sees an enum-typed discr.
2. `targets` carries the (case_value -> block) mapping
   plus the otherwise destination per the explicit-split
   form at D-9-6.

**Methodology rules invoked.** README §2 "Compositional
behavior beats special-case flags" -- discr-type
constraint falls out of stage-3 `Ty` interning + stage-6
operand-type lookup; no parallel `is_typed_switch` flag.

**Prior-art evidence.** Universal payload shape
across rustc (`syntax.rs:715-719`), rustc_public
(`body.rs:158-161`), Charon (similar), mrustc.

**Cross-refs.** `spec-snapshot.md` §23.2.2; D-9-6 for
SwitchTargets storage.

---

### D-9-1.4 -- `Return` and `Unreachable` payloads [LOCKED]

**Question.** What are the payloads of `Return` and
`Unreachable`?

**Decision.** Both unit variants -- no payload.

**Reasoning.**
1. `Return`'s return value lives implicitly at
   `LocalId::RETURN_PLACE` (`_0` per stage-1's
   body-local ID convention); type comes from
   `FnSig.output()` (resolved at stage 11). The
   Terminator carries no operand because the return
   value is at `_0` post-`optimized_mir`.
2. `Unreachable` is the terminating UB marker; no
   structural information is associated with it. Emitted
   naturally from `loop {}` patterns and as the output
   of cleanup-pass simplifications under panic=abort.

**Prior-art evidence.** Universal as unit variants
across rustc / rustc_public / Charon / mrustc.

**Cross-refs.** `spec-snapshot.md` §23.2.3, §23.2.4.

---

### D-9-1.5 -- `Call { callee, args, destination, target }` payload [LOCKED]

**Question.** What is the payload shape of `Call`?

**Decision.** Four named fields:
```rust
Call {
    callee:      CallCallee,            // D-9-3
    args:        Box<[Operand]>,        // D-9-4 (c)
    destination: Place,
    target:      Option<BlockId>,
}
```

**Reasoning** (per-field):
- `callee: CallCallee` per D-9-3 -- `Direct(InstanceId)`
  / `Indirect(Operand)` two-arm enum.
- `args: Box<[Operand]>` per D-9-4 (c) -- bare Operand
  sequence; per-arg spans dropped. `Box<[T]>` matches
  stage-7's `Aggregate` storage-shape rule (slim-pointer
  + length, no `Vec` overhead).
- `destination: Place` always present in
  `optimized_mir` (rustc pre-allocates the destination
  Place post-cleanup; the source-MIR `Option<Place>`
  shape is gone).
- `target: Option<BlockId>` -- `Some(bb)` indicates
  successor block on normal return; `None` indicates a
  divergent call (`panic!()`, `-> !` callee,
  `core::intrinsics::unreachable()`). rustc passthrough
  on semantics.

**Dropped from rustc Call payload** (per D-9-8):
`unwind`, `call_source`, `fn_span`. Each subsumed or
diagnostic-only with no driver-relevant consumer.

**Methodology rules invoked.** README §2 "C-semantics
with information preservation" (drop fields with no
consumer); "No defensive fallbacks" (don't preserve
fn_span / call_source against hypothetical future
codegen-warning use).

**Prior-art evidence.** Field-by-field comparison:
- rustc has `func`, `args: Box<[Spanned<Operand>]>`,
  `destination: Place`, `target: Option<BasicBlock>`,
  `unwind`, `call_source`, `fn_span`
  (`syntax.rs:811-832`).
- rustc_public has `func: Operand`, `args: Vec<Operand>`,
  `destination: Place`, `target: Option<BasicBlockIdx>`,
  `unwind` (`body.rs:171-177`). Drops Spanned wrapper
  on args (`mir.rs:752` does `arg.node.stable(...)`).

**Cross-refs.** `spec-snapshot.md` §23.2.5; D-9-3
(CallCallee), D-9-4 (per-arg spans), D-9-8 (wrapper +
dropped fields).

---

### D-9-1.6 -- Assert as Terminator (not Statement) [LOCKED]

**Question.** Is `Assert` classified as a Terminator (a
control-flow-edge instruction) or as a Statement (a
trapping side-effect)?

**Key options.**
- **(A) Terminator** -- rustc / rustc_public preserve
  Assert at the Terminator level. The `target` field
  carries the success-edge; the panic-edge is
  structurally implicit under panic=abort post-
  `AbortUnwindingCalls`.
- **(B) Statement** -- Charon classifies Assert as a
  StatementKind (`charon-lib/src/ast/ullbc_ast.rs:41`).
  Translator demotes rustc's terminator-level Assert to
  a Statement push + replaces the terminator with a
  plain `Goto`.

**Decision.** **(A) Terminator.**

**Reasoning.** Three load-bearing reasons:

1. **Bridge structural transparency.** Strategy A
   (Terminator) is a one-to-one variant rename + payload
   trim. Strategy B (Statement) requires either
   Goto-synthesis noise (preserve block boundaries) or
   a real CFG flattening pass (Charon's actual choice;
   merges Assert chains across single-predecessor
   blocks). The flattening violates our
   `mir_opt_level=0` discipline of pushing CFG opts
   upstream of the bridge.

2. **CFG-shape stability.** Stage-11 body-level passes
   (e.g., D-9-3 candidate (b)'s vtable-indirect rewrite
   pass) iterate terminators and need "this block has a
   control branch" as a terminator-variant property,
   not a statement-walk derivation. Compiler-theory-
   wise, exit semantics belong on the terminator.

3. **No information advantage to demotion.** Codegen
   output is identical in both strategies (both lower
   to `if (!cond) abort_helper(...); goto target;`).
   Strategy B's Statement-level uniformity buys nothing
   under our straight-through C codegen.

**Methodology rules invoked.**
- README §2 "Compositional behavior beats special-case
  flags" (Strategy B's flattening introduces
  CFG-shape-special-casing that doesn't compose
  cleanly).
- README §2 "C-semantics with information
  preservation" (preserve the structural
  guarded-conditional-branch semantics).

**Prior-art evidence.**
- rustc preserves Assert as Terminator
  (`syntax.rs:875-881`). Compiler-theory aligned with
  guarded-branch classification.
- rustc_public preserves Assert as Terminator
  (`body.rs:178-184`).
- Charon demotes to Statement (`ullbc_ast.rs:41`).
  Motivated by Aeneas verification-semantics target,
  not C codegen.
- mrustc has separate `Panic` Terminator (different
  framing -- explicit divergent terminator with
  unconditional abort, no boolean-guard structure).

**Charon's precedent does not transfer.** Their target
is verification semantics, not C codegen. RegseqModel's
codegen consumes terminator-shaped Assert directly; the
demotion would shift cost without payoff.

**Decision payload** (locked together with the
Terminator-level classification):
```rust
Assert {
    cond:     Operand,
    expected: bool,
    msg:      AssertKind,    // 8-variant per D-9-5
    target:   BlockId,
}
```
Drop `unwind` per D-9-8.

**Cross-refs.** `spec-snapshot.md` §23.2.6; D-9-5
(AssertKind variant subset).

---

### D-9-2 -- Drop-as-Call collapse at the bridge [LOCKED]

**Question.** How does the bridge handle rustc's
`TerminatorKind::Drop`? (a) Preserve as a separate
Terminator variant; (b) collapse at the bridge to a
synthetic Call to drop-glue; (c) collapse at codegen
(preserve as Statement-level).

**Decision.** **(b) Bridge-side collapse** to a synthetic
`TerminatorKind::Call` targeting the drop-glue Instance.

**Reasoning.** Three load-bearing reasons:

1. **C-semantics with information preservation.**
   README §2: "RegseqModel uses C-shaped lowerings
   rather than mirroring Rust-internal compiler shapes.
   The bridge does the desugaring (drop-glue calls
   become regular Calls...)." Drop-as-Call IS the C
   shape; preserving Drop as a variant means every
   downstream consumer must duplicate the collapse
   logic.

2. **Resolves construction invariant 9 closure
   correctness uniformly.** Stage 1's `validate`
   invariant 9 says every `InstanceId` in the model
   must resolve in the instance-closure. Drop-glue
   instances are part of that closure (synthesised by
   the bridge as part of mono-item discovery). With a
   separate `Drop` variant, the closure-walking
   visitor would need a special case: "if you see
   Drop, look up `tcx.resolve_drop_in_place(place_ty)`
   to get the synthetic instance and check its
   closure-membership." With the collapse, the walker
   simply visits every Call's `InstanceId`. Pattern
   uniformity -> simpler invariant.

3. **No information loss.** Post-`ElaborateDrops`,
   rustc's Drop is "an unconditional call to
   drop_in_place with a pointer to the place." The
   variant's extras (`replace: bool`, `drop:
   Option<BasicBlock>`, `async_fut: Option<Local>`)
   are diagnostic-only or async-coroutine-only -- both
   subsumed (per D-9-8) or unreachable under our DSL
   (coroutines type-rejected at stage 3 §15.1).

**Methodology rules invoked.**
- README §2 "C-semantics with information preservation."
- README §2 "Compositional behavior beats special-case
  flags" (uniform Call surface vs Drop-special-case
  Visitor logic).

**Prior-art evidence.**
- **rustc-internal precedent (load-bearing).** rustc's
  codegen-SSA performs the same collapse internally
  inside the backend dispatch loop at
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
  is structurally identical: rewrites `Drop { place,
  target, ... }` into a `Call` with `&raw mut place`
  arg, fresh unit-typed return slot, copied `target`.
  RegseqModel applies the same shape unconditionally
  at the bridge instead of as an opt-in normalize
  pass.
- **Charon's belt-and-braces ZST elision.** Charon's
  `filter_trivial_drops` pass collapses no-op-destruct
  Drops directly to `Goto`. RegseqModel defers this to
  stage-14 codegen peephole (per policy b.1; see
  `spec-snapshot.md` §23.4 step 2).
- **Stage-1 inbound expectation.** Per
  `expectations.md` "From stage 1": "Stage 9 lowers
  `Drop` terminators to regular `Call` terminators
  targeting the drop-glue `Instance` for the type."

**Bridge translation rule (5 steps):** see
`spec-snapshot.md` §23.4. Closes stage-1 inbound
"Drop lowered to Call" expectation; closes stage-1
inbound bridge concern about drop-glue Instance
interning.

**Outbound to stage 11.** Fresh `LocalId` allocation
for the `&raw mut place` temp + fresh unit-typed
return slot are stage-11 LocalDecls concerns. Filed in
`stage-11/expectations.md` "From stage 9".

**Cross-refs.** `spec-snapshot.md` §23.4 (5-step
bridge rule with full citations); §23.6.2 B-7 (closes
construction invariant 9 uniformly via `Direct`
walks).

---

### D-9-3 -- Call callee shape supporting vtable-indirect dispatch [LOCKED]

**Question.** What does `Call.callee` look like in the
closed 6-variant `TerminatorKind`? The shape must serve
three call shapes that arise in driver MIR: (1) direct
calls to monomorphized functions, (2) function-pointer
calls (`let f: fn(...) = ...; f(x)`), (3) virtual
dispatch through `dyn Trait`.

**Key options.**

1. **(a) Reopen stage-1 InstanceKind to add `Virtual { idx:
   usize }`.** Public Regseq InstanceKind grows from 6
   emission variants to 7. `Call.callee: InstanceId`
   stays uniform; the InstanceKind discriminator
   distinguishes Direct vs Virtual.

2. **(b) Widen Call.callee to `CallCallee::{Direct(InstanceId),
   Indirect(Operand)}`.** Closed 2-arm enum at the
   callee position. `Direct` carries a static
   `InstanceId` (handles case 1). `Indirect(Operand)`
   carries a runtime function-pointer value (handles
   cases 2 and 3 uniformly).

**Decision.** **(b)** -- `CallCallee::{Direct(InstanceId),
Indirect(Operand)}`. Stage-1 InstanceKind variant set
stays intact at 6 variants (no new variant added).
**However**, post-locking review (R3) surfaced a
schema-extension requirement: the `InstanceKind::Function`
variant gains a `virtual_idx: Option<u32>` field to
preserve rustc's vtable-slot index for Virtual
instances (per §9.4 construction invariant: `virtual_idx.is_some()`
iff `flavor == FunctionInstanceFlavor::Virtual`).
Stage-11's vtable-indirect rewrite reads this field to
determine which vtable slot to load. The schema
addition is a **stage-9 D-9-3 cross-stage revision**
(the schema-extension procedure anticipated by the
stage-1 inbound expectation "Possibly extend
`DefPathData` for synthetic vtable scopes" and the
broader "stage-1 revision triggered by stage 9"
methodology pattern). The mirror invariant lives at
§23.6.2 B-8.

**Reasoning.** Five load-bearing reasons:

1. **Required for completeness regardless of dyn
   Trait.** Function-pointer calls (case 2:
   `let f: fn(...) = ...; f(args)`) need a
   runtime-resolved callee position. Stage-1's
   emission-only InstanceKind closure cannot model
   "the callee is determined at runtime from a runtime
   value." Option (a) has no clean home for case 2.

2. **Closes the function-pointer-call gap that option
   (a) would leave open.** The current spec
   (`spec.md` §9.4) already exposes the rustc-internal
   Virtual discriminator via `InstanceKind::Function.flavor:
   FunctionInstanceFlavor` with the `Virtual` variant
   (the `flavor` field was added by a post-stage-1
   spec revision and is now canonical in top-level
   `spec.md`; the frozen `stage-1/spec-snapshot.md`
   historical artifact predates this revision and
   shows only `Function { sig, body }`). Stage-9 D-9-3
   further extends `Function` with `virtual_idx:
   Option<u32>` to preserve rustc's vtable-slot index
   (also a cross-stage revision; see §9.4 construction
   invariant + §23.6.2 B-8). With these schema
   pieces, the bridge can emit `Direct(<id>)` with
   `flavor=Virtual` + `virtual_idx=Some(idx)` and
   stage 11 can detect + rewrite it.
   However, function-pointer calls (`let f: fn(...) =
   ...; f(args)`) need a runtime-resolved callee
   position, and `InstanceKind::Function.flavor` cannot
   model "the callee is determined at runtime from a
   runtime value" -- the InstanceId is statically
   resolved by definition. Option (a) handles only
   case 3 (vtable dispatch); case 2 (function-pointer
   calls) requires a callee-shape distinction
   regardless. Option (b) handles cases 2-3 uniformly
   through the `Indirect(Operand)` arm.

3. **Proper IR variant per compiler theory.** Direct/
   Indirect call distinction is universal across mature
   IRs:
   - LLVM IR: `call` direct vs indirect on SSA value.
   - Cranelift IR: `call` vs `call_indirect`.
   - mrustc: `CallTarget = Value(LValue) |
     Path(::HIR::Path) | Intrinsic` at
     `src/mir/mir.hpp:595-602`.
   - Charon: `FnOperand::{Regular(FnPtr), Move(Place)}`
     at `charon-lib/src/ast/gast.rs:386-391`.
   Making it explicit at the IR level surfaces a
   structural distinction into the type system, where
   the type checker enforces it. Aligns with the
   project's compiler-theory-first methodology rule.

4. **C-codegen shape match.** C distinguishes
   `name(args)` (direct) from `(*ptr)(args)` (indirect).
   The IR mirrors this directly under (b).

5. **Cleans up invariant 9 closure-walk.** Direct's
   InstanceId is closure-checked; Indirect's Operand
   has no InstanceId to check (function pointer's value
   is a runtime concern, not a closure concern). Option
   (a) would require a Virtual special case in the
   closure-walking visitor (skip the closure check
   because the InstanceId points to a vtable shim, not
   a single Instance).

**Methodology rules invoked.**
- README §2 "Compiler-theory-first" -- prefer proper
  IR variants over narrow workarounds. The Direct/
  Indirect split is the proper IR variant; option (a)
  is a narrow workaround that handles only case 3
  partially.
- README §2 "Compositional behavior beats special-case
  flags" -- option (a) introduces a Virtual variant
  that's structurally different from the emission-only
  InstanceKind variants; option (b) factors the
  distinction at the call site where it belongs.
- README §2 "C semantics with information preservation"
  -- preserve the structural Direct/Indirect distinction
  the C language exposes.

**Prior-art evidence.**
- **rustc-internal favors (a)-shape at the resolution
  layer** but uses (b)-shape at the IR level: `Call.func:
  Operand` is uniformly an Operand (the discriminator is
  recovered from the Operand's type/Constant resolution).
  Source: `rustc_middle/src/mir/syntax.rs:811-832`.
- **rustc_public favors (a)-shape:** exposes
  `InstanceKind { Item, Intrinsic, Virtual { idx },
  Shim }` at `rustc_public/src/mir/mono.rs:30-37`;
  bridge translator lifts `ty::InstanceKind::Virtual(_,
  idx)` directly into the public InstanceKind at
  `rustc_public_bridge/src/unstable/convert/stable/ty.rs:949-951`.
- **Charon favors (b)-shape:** `FnOperand::{Regular(FnPtr),
  Move(Place)}` callee split at
  `charon-lib/src/ast/gast.rs:386-391`;
  `transform_dyn_trait_calls` pass at
  `charon-lib/src/transform/normalize/transform_dyn_trait_calls.rs:31-122`
  synthesizes vtable function-pointer load via
  `PtrMetadata -> Deref -> Field(method_field_id)`
  projection chain and mutates `call.func =
  FnOperand::Move(method_field_place)` at line 119.
- **mrustc favors (b)-flavored shape:**
  `CallTarget::{Path, Value, Intrinsic}` callee union
  at `src/mir/mir.hpp:595-602`; `MIR_Cleanup_Virtualize`
  pass at `src/mir/cleanup.cpp:548-684` flips
  `Call.fcn = Path(<dyn Trait>::method)` to
  `Value(<vtable_loaded_lv>)`.

**Foreclosed alternative.** Candidate (a) -- relying
on the stage-1 `FunctionInstanceFlavor::Virtual`
discriminator alone -- handles only case 3 (vtable
dispatch); case 2 (function-pointer calls) requires a
callee-shape distinction regardless. Option (b)'s
`Indirect(Operand)` arm subsumes case 2 cleanly. Once
(b) is in the IR for case 2, using it uniformly for
case 3 (post-rewrite) is structurally cleaner than
maintaining a parallel discriminator-based path. (a)
is foreclosed not because the discriminator is missing
-- it isn't -- but because (b) handles strictly more
of the call-shape design space.

**Bridge translation rule (4 cases):**

| Rustc `Call.func` shape | RegseqModel `callee` |
|-------------------------|----------------------|
| `Operand::Constant(c)` where `c.const_.ty() = Ty::FnDef(def_id, args)` resolves to a non-Virtual Instance | `Direct(<bridge-interned InstanceId>)` -- resolved Regseq Instance has `kind = InstanceKind::Function { flavor: <non-Virtual variant>, .. }` |
| `Operand::Constant(c)` where the resolved rustc Instance has `InstanceKind::Virtual(_, idx)` | `Direct(<bridge-interned InstanceId>)` -- resolved Regseq Instance has `kind = InstanceKind::Function { flavor: FunctionInstanceFlavor::Virtual, virtual_idx: Some(idx), .. }` (per stage-9/spec-snapshot.md §9.4 with the D-9-3 schema extension; post-merger lands in top-level spec.md §9.4). This is the **pre-rewrite shape**; stage-11's vtable-indirect rewrite (per §23.6.2 B-3/B-4/B-5/B-6 post-rewrite invariant timing + B-8 virtual_idx-flavor biconditional) walks every `Direct(InstanceId)`, reads `Instance.kind.flavor` + `virtual_idx`, and rewrites to `Indirect(Operand)` synthesizing the vtable load through index `virtual_idx.unwrap()` |
| `Operand::Move(p)` / `Copy(p)` where `p.ty() = Ty::FnPtr(...)` | `Indirect(Operand::Move(p))` / `Indirect(Operand::Copy(p))` |
| Anything else | bridge-rejection (mistyped callee shape) |

**Outbound to stage 11.** Body-level rewrite pass
recognising rustc `InstanceKind::Virtual`-resolved Calls
and rewriting them to `Indirect(Operand)` form via
synthesised `PtrMetadata -> Deref -> Field(method_idx)`
projection statements + fresh LocalIds. Charon's
`transform_dyn_trait_calls.rs:31-122` is the direct
template. Filed in `stage-11/expectations.md`.

**Outbound to stage 12.** Vtable shim Item synthesis
(already filed by stage 1; cross-ref). Under D-9-3
(b), stage 9 introduces no new InstanceKind variant
and therefore no Virtual-shim-scope DefPathData need.
**Stage 12's separate DefPathData encoding choice for
vtable type Items and vtable instance Items** (per
`stage-12/expectations.md` "From stage 3" Sections A
and B -- "overload an existing variant, or add a new
`DefPathData::VTableType` variant per stage-1's
anticipated revision procedure") **remains a stage-12
decision**; D-9-3 does not constrain it.

**Closes inbound expectations:**
- Stage-1: "Vtable-indirect dispatch (Call terminator
  shape)" -- closed.
- Stage-1: "Possibly extend `DefPathData` for synthetic
  vtable scopes" -- closed only as it relates to
  stage-9's call-callee shape (no Virtual-shim-scope
  variant needed). Stage-12's separate vtable Item
  DefPathData encoding choice is **not** closed by
  D-9-3 and remains a stage-12 decision.
- Stage-3 D5: "Define Call terminator shape supporting
  vtable-indirect dispatch" -- closed.

**Cross-refs.** `spec-snapshot.md` §23.3.1 (CallCallee
enum) + §23.7 (bridge translation rules) + §23.8.6
(virtual dispatch worked example).

---

### D-9-4 -- Per-arg span preservation for Call args [LOCKED]

**Question.** What does the `args` field of
`TerminatorKind::Call` look like? Stage 6 D-6-1.3 dropped
the operand-internal span and listed two candidates;
rustc_public surfaced a third.

**Key options.**
- **(a) Inline wrapper:** `args: Box<[ArgWithSpan]>`
  where `pub struct ArgWithSpan { operand: Operand,
  span: Span }`. Mirrors rustc's `Spanned<Operand>`.
- **(b) Parallel slabs:** `args: Box<[Operand]>` plus
  `arg_spans: Box<[Span]>`. Decouples span from operand
  storage.
- **(c) Drop spans entirely:** `args: Box<[Operand]>`.
  No per-arg span preserved. Direct precedent:
  rustc_public at
  `rustc_public_bridge/src/unstable/convert/stable/mir.rs:752`
  does `args.iter().map(|arg|
  arg.node.stable(...)).collect()` -- strips the
  `Spanned` wrapper unconditionally.

**Decision.** **(c) Drop spans entirely.** `args:
Box<[Operand]>` -- bare Operand sequence; no per-arg
span carried. Per-arg diagnostic granularity falls back
to the Terminator-level `span` (the call site as a
whole).

**Reasoning.** Three load-bearing reasons:

1. **No load-bearing consumer.** Stage-11
   `validate(...)` walk uses structural type-equality /
   arity-match invariants (not span-keyed); stage-11
   body-level rewrite passes operate on Operand
   structure (not source position); stage-14 codegen
   does not emit spans. Compiler-internal driver-source
   diagnostics run pre-bridge at the rustc/HIR layer
   (per-arg spans there, not at RegseqModel level).
   Speculative future consumers (phase-2 device-pressure
   debugging, codegen-time arg warnings) have workable
   Terminator-level fallbacks; for driver code where
   calls typically take 0-4 args, granularity loss is
   bounded.

2. **No defensive preservation.** Stage 6 D-6-1.3
   deferred the question to stage 9 specifically so the
   consumer landscape could be examined with full IR
   visibility. Examined; no consumer warrants the
   storage overhead.

3. **rustc_public peer alignment + consistency with
   D-9-8.** rustc_public (closest existing owned-IR
   analog) drops per-arg spans; D-9-8 drops
   `source_info.scope`, `Call.call_source`,
   `Call.fn_span`, `Call.unwind` with the same "no
   driver-relevant consumer" rationale. Per-arg spans
   fall in the same category.

**Methodology rules invoked.**
- README §2 "No defensive fallbacks" -- per-arg spans
  preservation against hypothetical future
  codegen-warning use is exactly the pattern the rule
  rejects.
- README §2 "C semantics with information preservation"
  -- preserve information rustc gives us *for free*,
  *interrogated per construct*: per-arg spans fail
  the "who consumes them" interrogation.

**Prior-art evidence.**
- **rustc preserves `Spanned<Operand>`** at
  `rustc_middle/src/mir/syntax.rs:820`. Per-arg span
  granularity is a rustc design feature.
- **rustc_public drops per-arg spans** at
  `rustc_public_bridge/src/unstable/convert/stable/mir.rs:752`
  (`arg.node.stable(...)` strips the Spanned wrapper).
  Direct precedent for option (c).
- Charon and mrustc both store args as
  `Vec<Operand>` -- no per-arg spans (consistent with
  option (c)).

**Bridge translation:** for each rustc
`Call.args: Box<[Spanned<Operand<'tcx>>]>`,
`args.iter().map(|sp| translate_operand(&sp.node)).collect()`
-- strips the `Spanned` wrapper, drops `sp.span`.

**Supersedes inbound expectation:** stage-6 D-6-1.3
"`Spanned<Operand>` per-arg span preservation". The
inbound listed (a) inline-wrapper and (b)
parallel-slabs as the two preservation candidates and
expected stage 9 to pick one; D-9-4 instead **revises
the expectation** by introducing a third option (c)
drop-entirely, motivated by the consumer-landscape
analysis. Stage-6 D-6-1.3 is closed by supersession,
not by satisfaction-as-written.

**Cross-refs.** `spec-snapshot.md` §23.2.5 (Call
payload args field) + §23.7 (bridge translation rules).

---

### D-9-5 -- AssertKind variant subset [LOCKED]

**Question.** `Assert.msg: AssertKind` carries the
panic-message data. Rustc's `AssertKind<O>` has 11
variants. Which subset does RegseqModel keep?

**Key options.**
- **(α) All 11 variants** (rustc_public-aligned).
- **(β) 8-variant subset** -- drop 3 coroutine-only
  variants.
- **(γ) Charon-style 3-variant `AbortKind` collapse**
  with `msg` field discarded wholesale.

**Decision.** **(β) 8-variant subset.** Drop
`ResumedAfterReturn`, `ResumedAfterPanic`,
`ResumedAfterDrop`; keep BoundsCheck, Overflow,
OverflowNeg, DivisionByZero, RemainderByZero,
MisalignedPointerDereference, NullPointerDereference,
InvalidEnumConstruction.

**Reasoning.** Three load-bearing reasons:

1. **Type-rejection cascade consistency.** Stages 5-8
   each reject variants whose precondition is
   type-rejected at stage 3 (coroutines, Box, etc.).
   The 3 coroutine-only Assert variants fit this
   discipline cleanly: their types cannot reach
   RegseqModel (stage-3 §15.1 rejects `Coroutine`
   types), so the variants are structurally
   unreachable. Carrying unreachable variants is the
   same anti-pattern as carrying unreachable types.

2. **Diagnostic-quality preservation for
   driver-relevant cases.** Driver code does encounter
   the 8 retained variants:
   - **BoundsCheck** -- array indexing bounds checks.
   - **Overflow / OverflowNeg** -- arithmetic overflow
     in MMIO field manipulation, saturating
     arithmetic.
   - **DivisionByZero / RemainderByZero** -- rare but
     possible in driver clock-rate computation.
   - **MisalignedPointerDereference / NullPointerDereference**
     -- hot path for raw-pointer driver code.
   - **InvalidEnumConstruction** -- enum-construction
     validation.
   Each lowers to a specific
   `__rust_abort_with_msg(format_str, operands...)`
   call providing useful runtime diagnostics. (γ)'s
   collapse to bare `panic` forfeits this.

3. **rustc_public-aligned with a single deliberate
   divergence.** rustc_public preserves all 11; we drop
   3 on the type-rejection-cascade rule. The divergence
   is principled, documentable as "AssertKind variants
   whose precondition is type-rejected at stage 3 are
   pruned," and aligned with the stage-7/-8 discipline.

**Methodology rules invoked.**
- README §2 "No defensive fallbacks" (don't carry
  unreachable variants).
- README §2 "C-semantics with information preservation"
  (preserve diagnostic content where it serves
  driver-relevant cases).

**Prior-art evidence.**
- rustc 11-variant `AssertKind<O>` at
  `rustc_middle/src/mir/syntax.rs:1066-1078`.
- rustc_public preserves all 11 at
  `rustc_public/src/mir/body.rs:260-273`.
- Charon collapses to 3-variant `AbortKind` at
  `charon-lib/src/ast/...` -- forfeits diagnostic
  content; aligned with verification-IR target, not
  C codegen.
- Stage-7 D-7-1 dropped variants whose preconditions
  type-rejected at stage 3 (Coroutine constructions);
  stage-8 D-8-1.3 narrowed similarly. RegseqModel-
  consistent pattern.

**Bridge translation.** 8 variants translate
one-to-one (operands routed through stage-6 Operand
machinery, BinOp through stage-7 BinOp machinery); 3
coroutine variants emit a bridge-rejection diagnostic
(double-discharged with `coroutine::StateTransform`).

**Cross-refs.** `spec-snapshot.md` §23.3.3 (AssertKind
enum); D-9-1.6 (Assert payload); §23.6.1 row 3/4
(coroutine variant rejection).

---

### D-9-6 -- SwitchTargets storage shape [LOCKED]

**Question.** What is the storage shape of
`SwitchTargets`? Two candidates:

- **(a) Mirror rustc** -- "values + targets, last
  targets entry is otherwise" with `targets.len() ==
  values.len() + 1` invariant. Schema:
  `values: Box<[u128]>` + `targets: Box<[BlockId]>`
  with end-of-vector otherwise convention.
- **(b) Explicit split** -- `cases: Box<[(u128,
  BlockId)]>` plus `otherwise: BlockId`. Decouples the
  invariant from the storage shape.

**Decision.** **(b) Explicit split.**

```rust
pub struct SwitchTargets {
    pub cases:     Box<[(u128, BlockId)]>,
    pub otherwise: BlockId,
}
```

**Reasoning.** Three load-bearing reasons:

1. **Explicit > implicit.** The otherwise destination
   is intrinsically different from a case block
   (catch-all, not value-mapped). (a)'s positional
   encoding hides that distinction; (b) makes it
   structurally explicit.

2. **No runtime invariant to enforce.** (a) requires
   `targets.len() == values.len() + 1` walk-pass-check;
   (b) lets the type system enforce the otherwise
   field's presence as a struct field.

3. **Compositional with the zero-cases edge case.**
   Under (b), `cases: []` + `otherwise: bb` is natural;
   under (a), `values: []` + `targets: [otherwise]` is
   a len-1 special case.

**Methodology rules invoked.**
- README §2 "Compositional behavior beats special-case
  flags" -- "the last entry is special-case otherwise"
  is exactly the kind of positional-special-case the
  rule pushes against.

**Prior-art evidence.**
- rustc uses (a) at `rustc_middle/src/mir/syntax.rs:1005-1031`.
  Rustc's rationale (in source comments at 1014-1029):
  keeping targets in a single sliceable type is
  important because `TerminatorKind::successors`
  returns `impl Iterator` -- multiple concrete iterator
  types would require boxed trait objects. **This
  rationale does not apply to RegseqModel**: we don't
  share an iterator across variants the way rustc does.
- rustc_public uses an (b)-shape variant:
  `SwitchTargets { branches: Vec<(u128, BasicBlockIdx)>,
  otherwise: BasicBlockIdx }`. Closer peer for our
  case.
- Charon splits by type: `SwitchKind` variants
  (`If(BlockId, BlockId)` for bool, `SwitchInt(IntegerTy,
  Vec<(ScalarValue, BlockId)>, BlockId)` for integer).
  Explicit otherwise field across all variants.

**Bridge translation:** see `spec-snapshot.md` §23.7
(translate one-pass over rustc's parallel arrays).
The case-value width is `u128`; rustc's `Pu128`
(packed wrapper for HashStable) translates to bare
`u128`.

**Cross-refs.** `spec-snapshot.md` §23.3.2; D-9-1.3
(SwitchInt payload using SwitchTargets); §23.6.2 B-1
(discr type invariant).

---

### D-9-7 -- No Terminator interning [LOCKED]

**Question.** Is `Terminator` interned (top-level table
with `TerminatorId` indices) or stored inline at the
basic-block boundary?

**Decision.** **Inline storage.** No top-level
`terminators` interner table; no `TerminatorId` index
type. `BasicBlock.terminator: Terminator` is the
storage form (stage-10 vocabulary preview).

**Reasoning.** Three load-bearing reasons:

1. **Universal precedent.** All four prior-art
   projects store terminators inline at the basic-block
   boundary:
   - rustc: `BasicBlockData<'tcx>.terminator:
     Option<Terminator<'tcx>>`.
   - rustc_public: `BasicBlock.terminator: Terminator`
     at `rustc_public/src/mir/body.rs:135-139`.
   - Charon: `BlockData.terminator: Terminator` at
     `charon-lib/src/ast/ullbc_ast.rs`.
   - mrustc: `BasicBlock.terminator: Terminator` at
     `src/mir/mir.hpp`.
   No project interns.

2. **No deduplication win.** Inter-body terminator
   sharing is theoretically possible (every `Return`
   is identical; `Unreachable` is identical) but
   vanishingly thin. `Goto(bb)`, `SwitchInt`, `Call`,
   `Assert` all carry body-local data (`BlockId`,
   embedded `Operand`/`Place`, target lists, callee
   `InstanceId`) -- practically zero cross-body
   byte-equality. The two unit variants are 1 byte
   each; even with thousands of bodies, savings are
   negligible against the indirection cost.

3. **Pattern-match consistency.** Stages 5-8 lock
   inline storage at every level (Place projections
   D-5-x, Operand D-6-2, Rvalue D-7-2, Statement
   D-8-2). The "body-local content lives inside the
   body" rule is established. Interning Terminator
   alone breaks the rule for no payoff.

**Methodology rules invoked.**
- README §2 "Closed `Ty` enum" pattern (closed-form
  vocabulary; structural identity).
- Pattern consistency with stages 5-8 inline storage.

**F4 / F4-2 cross-build identity is OUT of scope.**
Terminator content is per-build structural;
cross-build canonicalisation operates on
`(InstanceKey)` per stage-1 F4-2, not
`(BodyContents)`. Same scope reduction as stages 5-8.

**Cross-refs.** `spec-snapshot.md` §23.5; stage-10
preview shape `BasicBlock { statements, terminator }`.

---

### D-9-8 -- Terminator wrapper struct shape [LOCKED]

**Question.** What does the `Terminator` wrapper struct
(holding `TerminatorKind` plus per-terminator metadata)
look like, and which rustc-side fields survive into
RegseqModel?

**Decision.**
```rust
pub struct Terminator {
    pub kind: TerminatorKind,
    pub span: Span,
}
```

Direct precedent: rustc_public at
`rustc_public/src/mir/body.rs:140`. Mirrors stage-8
D-8-3 `Statement { kind, span: Span }`.

**Dropped fields:**

| Field | Source | Drop reason |
|-------|--------|-------------|
| `source_info.scope: SourceScope` | wrapper | Debuginfo-only DWARF-emission index. RegseqModel does not emit DWARF (per stage-1 spec rejection). |
| `Call.call_source: CallSource` | variant payload | 5-variant diagnostic enum (`OverloadedOperator`/`MatchCmp`/`Misc`/`Use`/`Normal`); rustc itself uses it only for "better error messages." |
| `Call.fn_span: Span` | variant payload | Function-position span without the receiver -- per-call diagnostic refinement subsumed by either the wrapper-level `span` or the per-arg span at D-9-4. |
| `Drop.replace: bool` | variant payload | Diagnostic-only flag. Subsumed by D-9-2 collapse to Call. |
| `Drop.drop: Option<BasicBlock>` | variant payload | Async-drop coroutine yield-point; coroutines type-rejected at stage 3 §15.1; subsumed by D-9-2. |
| `Drop.async_fut: Option<Local>` | variant payload | Async-drop future local; coroutines type-rejected; subsumed by D-9-2. |
| `unwind: UnwindAction` (on Call/Drop/Assert/InlineAsm) | variant payload | Under panic=abort with `AbortUnwindingCalls` having run, only `Continue` and `Unreachable` survive at our consumption point and collapse semantically. The field carries no driver-relevant information. |

**Sub-decision: `unwind: UnwindAction` -- drop entirely.**

`UnwindAction` has 4 variants: `Continue`, `Unreachable`,
`Terminate(reason)`, `Cleanup(BasicBlock)`. Under our
pinned config:
- `panic = "abort"` is required for driver `no_std`
  builds.
- `AbortUnwindingCalls` runs at
  `rustc_mir_transform/src/lib.rs:651` (in
  `run_runtime_lowering_passes`, before
  `optimized_mir`); under panic=abort it rewrites
  unwind edges to `UnwindAction::Unreachable` at
  `rustc_mir_transform/src/abort_unwinding_calls.rs:47`.
- Cleanup blocks become unreachable;
  `SimplifyCfg::PreOptimizations` (in our cleanup-pass
  whitelist) prunes them.

Surviving values at our consumption point: `Continue`
and `Unreachable` -- both semantically equivalent
because there are no cleanup edges to traverse.
`Cleanup(_)` and `Terminate(_)` are gone. **Dropping is
sound.**

**Edge case** (acknowledged but not addressed): the
rustc prior-art flagged `extern "C-unwind"` as a
potential way for unwinding to leak into drop glue even
under panic=abort. Under our DSL surface, FFI is `extern
"C"` only (Regseq does not surface `extern "C-unwind"`).
If a future DSL extension introduces unwinding-FFI,
that's a new spec event with its own decision; the
methodology rule "no defensive fallbacks" applies -- we
don't preserve `unwind` defensively against a
hypothetical that isn't on the roadmap.

**Methodology rules invoked.**
- README §2 "C-semantics with information preservation"
  (preserve fields with consumers; drop those without).
- README §2 "No defensive fallbacks" (don't preserve
  `unwind` against a hypothetical DSL extension).

**Prior-art evidence.**
- rustc `Terminator { source_info, kind }` at
  `rustc_middle/src/mir/terminator.rs` -- preserves
  scope.
- rustc_public `Terminator { kind, span }` at
  `rustc_public/src/mir/body.rs:140` -- drops scope.
  Direct precedent.
- mrustc no source-info on terminator (spans are
  diagnostic-only and discarded).

**Cross-refs.** `spec-snapshot.md` §23.1.1 (wrapper
struct definition); D-9-4 (per-arg spans).

---

### D-9-9 -- Cleanup-pass-discharged rejection table [LOCKED]

**Question.** Which rustc `TerminatorKind<'tcx>` variants
does the bridge reject, what diagnostic does each
produce, and what is the discharge mechanism (the ground
for asserting the variant won't appear under our pinned
config)?

**Decision.** 9-row rejection table with two-category
framing:
- **Pipeline-discharged (rows 1-6):** rustc's required
  passes eliminate these before `optimized_mir`.
  Encountering one in well-formed `optimized_mir`
  indicates a pipeline-config bug.
- **DSL-only (rows 7-8):** rustc supports the variant
  in `optimized_mir`; the Regseq DSL surface does not
  include the source-level construct. Encountering one
  indicates a driver-source error.
- **Bridge-collapsed (row 9):** Drop is converted to
  Call per D-9-2; row 9 is in the table for
  completeness so the implementer doesn't list Drop as
  "supported as-is."

The full table appears at `spec-snapshot.md` §23.6.1.

**Reasoning.** The two-category framing reflects a real
structural distinction surfaced by the rustc prior-art:

- **Rows 1-6 are pipeline-discharged unconditionally.**
  `CleanupPostBorrowck` (rows 1-2; `is_required: true`)
  rewrites FalseEdge/FalseUnwind to Goto;
  `coroutine::StateTransform` (rows 3-4) discharges
  Yield/CoroutineDrop; `AbortUnwindingCalls` + cleanup-
  block pruning (rows 5-6) eliminates UnwindResume/
  UnwindTerminate under panic=abort.

- **Rows 7-8 are NOT pipeline-discharged.** TailCall
  reaches `optimized_mir` whenever
  `#![feature(explicit_tail_calls)]` is set; InlineAsm
  reaches it unconditionally. The bridge cannot rely on
  rustc to remove them; rejection must be DSL-level
  (a wellformedness-of-driver-source property), not
  pipeline-level.

This distinction matters for the diagnostic message:
rows 1-6 carry an "internal error: pipeline
misconfiguration" framing distinct from rows 7-8's
"driver source error" framing.

**Methodology rules invoked.**
- README §2 "No defensive fallbacks" -- variants whose
  precondition is satisfied by the pinned toolchain
  are not carried defensively.
- README §2 "Closed `Ty` enum" pattern (closed-form
  surface; reject-on-encounter for non-conforming).

**Prior-art evidence.**
- **rustc-internal rejection points** (the "Rejection
  points" table in `rustc-references.md`):
  `rustc_codegen_ssa/src/mir/block.rs:1550-1555` --
  `bug!("coroutine ops in codegen")` for
  Yield/CoroutineDrop; `bug!("borrowck false edges in
  codegen")` for FalseEdge/FalseUnwind. Same placement
  pattern as our rejection table; we use
  `unsupported_construct!` instead of `bug!` so the
  driver author sees a useful message.
- **rustc_public's bridge translator** at
  `rustc_public_bridge/src/unstable/convert/stable/mir.rs:704-790`
  -- 5 explicit drops (`TailCall` via `todo!()`;
  Yield / CoroutineDrop / FalseEdge / FalseUnwind via
  `unreachable!()`). The `unreachable!()` vs
  `todo!()` discipline distinction (correctness
  precondition violated vs incomplete implementation)
  surfaces here too -- we don't `panic`, but we
  surface the framing in our diagnostic message.

**Walk-pass enforcement deferred to stage 11** per
D-9-10 Part C. The 9-row table specifies rejection at
the bridge translation site; the walk-pass is
mostly tautological under the closed-enum surface but
covers the case where some future construction path
might violate the surface.

**Cross-refs.** `spec-snapshot.md` §23.6.1 (full
9-row table).

---

### D-9-10 -- Terminator-site invariants registry [LOCKED]

**Question.** What invariants do well-formed Terminator
content satisfy, beyond those carried over from earlier
stages? How are they enforced, and at what stage?

**Decision.** 5-part registry mirroring stage-8 D-8-4.

**Part A: rejection table.** Cross-references D-9-9
(9-row rejection table; 8 rejected rustc variants + 1
collapsed via D-9-2).

**Part B: positive invariants.** 2 inherited + 7
stage-9-introduced.

*Inherited (2):* stage-9 Terminator embeds Place
(Call.destination) and Operand (multiple sites);
Statement-level and Rvalue-level invariants do not
propagate (those constructs do not appear in Terminator
content).
- **B-0a** -- All embedded Place content satisfies
  Place invariants per stage-5 §19.7.
- **B-0c** -- All embedded Operand content satisfies
  Operand invariants per stage-6 §20.5.

*Stage-9-introduced (7):*
- **B-1** -- `SwitchInt.discr` Operand has type ∈
  {`Ty::Bool`, `Ty::Char`, `Ty::Int(_)`, `Ty::Uint(_)`}.
- **B-2** -- `Assert.cond` Operand has type `Ty::Bool`.
- **B-3** -- Call destination type matches callee
  return type (Direct: via `lookup_instance(...).fn_sig.output()`;
  Indirect: via Operand type's `Ty::FnPtr.output()`).
- **B-4** -- Call args length matches callee signature
  arity.
- **B-5** -- Call callee is callable (Direct: InstanceId
  always callable by stage-1 instance-closure
  construction; Indirect: Operand type ∈ {Ty::FnPtr(_)}).
- **B-6** -- Per-arg type match. Catches stage-11
  rewrite-pass bugs (e.g., the vtable-indirect rewrite
  passing the wrong receiver type).
- **B-7** -- Closes construction invariant 9: every
  `Direct(InstanceId)` referenced from any
  Terminator::Call resolves in `model.instances`.

**Part C: walk-implementation deferred to stage 11.**
Same precedent as stage-8 D-8-4 Part C. Stage 11 owns
`Body::validate(...)` walk + visitor-trait extensions
+ actual `assert!` checks + bridge-rejection diagnostic
emission for §23.6.1 row encounters.

**Part D: F4 / F4-2 cross-build identity OUT of
scope.** Per D-9-7 (no Terminator interning), Terminator
is body-local content. Cross-build identity operates on
`(InstanceKey)` per stage-1 F4-2.

**Part E: peer comparison (informational).** rustc
uses `validate.rs` per-Terminator rules with `bug!()`;
rustc_public uses light validation; Charon uses
walk-based well-formedness checks; mrustc uses
two-layer validator (`MIR_Validate` + `MIR_Validate_Full`)
with fatal `MIR_BUG`. RegseqModel takes the
walk-pass-deferred-to-stage-11 discipline matching
stages 5-8.

**Reasoning.** The 5-part structure provides a complete
contract for stage 11 to consume:
- Part A says what to reject (boundary check at the
  bridge).
- Part B says what to positively check (in-body
  walks).
- Part C names the consumer (stage 11).
- Part D scopes out F4/F4-2 (no cross-build identity).
- Part E benchmarks against peers.

**Methodology rules invoked.**
- README §2 "Compositional behavior beats special-case
  flags" -- B-0a/c inherited; no parallel
  re-checking of Place/Operand invariants.
- Pattern consistency with stage-8 D-8-4 5-part
  registry.

**Prior-art evidence.** Listed in Part E above.

**Outbound to stage 11.** Walk implementation per
Part C; visitor-trait extensions covering Terminator,
CallCallee, SwitchTargets, AssertKind. Filed in
`stage-11/expectations.md` "From stage 9".

**Closes inbound expectation:** stage-1's deferred
construction invariant 9 (per `expectations.md`
"From stage 1") -- closed by B-7 (rules) + outbound to
stage 11 (walk).

**Cross-refs.** `spec-snapshot.md` §23.6 (full 5-part
registry); §23.6.1 (rejection table); §23.6.2
(positive invariants); §23.6.3 (walk-implementation
deferral).

---

### D-9-K -- `pretty.rs::render_terminator` deferral [LOCKED]

**Question.** When does `pretty.rs::render_terminator`
get implemented?

**Decision.** **Deferred to stage 11**, alongside
`render_place` (D-5-K), `render_operand` (D-6-K),
`render_rvalue` (D-7-K), and `render_statement` (D-8-K).
Stage 11 owns all five `render_*` methods together;
the deferral cascade D-5-K -> D-6-K -> D-7-K -> D-8-K
-> D-9-K closes here.

Four public renderers to implement at stage 11 (plus
one internal dispatch helper not in the §18.2 ownership
table):
- `render_terminator(&Terminator, &Body)` -- top-level
  public entry point. Internally dispatches per-variant
  on `TerminatorKind` (the dispatch helper is a private
  detail of stage-11's implementation, not a separate
  public renderer).
- `render_call_callee(&CallCallee, &Body)` -- the
  D-9-3 (b) two-arm dispatch. Public.
- `render_switch_targets(&SwitchTargets, &Body)` --
  the D-9-6 explicit-split form. Public.
- `render_assert_kind(&AssertKind, &Body)` -- the
  D-9-5 8-variant dispatch. Public.

**Reasoning.** Stage-9 Terminator references content
that needs render-time resolution from sources only
fully populated at stage 11 (and partially at stage 12).

Per-variant dependency:

| Variant | Rendering input | Stage |
|---------|-----------------|-------|
| `Goto(BlockId)` | block-label rendering convention | 11 (Body owns BlockId space) |
| `SwitchInt { discr, targets }` | render_operand + render_switch_targets | 11 |
| `Return` / `Unreachable` | self-contained keywords | -- |
| `Call { callee, args, destination, target }` | render_call_callee + render_operand + render_place + block-label | 11 + 12 (CallCallee::Direct) |
| `Assert { cond, expected, msg, target }` | render_operand + bool literal + render_assert_kind + block-label | 11 |

`CallCallee::Direct(InstanceId)` rendering late-binds to
**stage 12**: the InstanceId resolves to an `Instance`,
whose `Item.def_path` is rendered via
`render_def_path(ItemId)` per stage-1's helper (stub at
`crates/regseq-model/src/pretty.rs`; stage-12 fills the
body per `stage-12/expectations.md`). The
`render_call_callee` `Direct` arm calls
`render_def_path` and the result is the C function
symbol name.

`CallCallee::Indirect(Operand)` delegates entirely to
`render_operand` -- no stage-12 dependency.

The InstanceId-rendering subtree (Call.callee::Direct)
adds a stage-12 dependency; render_terminator lands at
stage 11 *with a forward-reference to stage-12 def-path
rendering* -- matching how stage 8's render_statement
lands at stage 11 with hooks to render_operand /
render_place / render_rvalue.

**Methodology rules invoked.**
- "Land at the latest first-batch dependency"
  pattern from D-5-K, D-6-K, D-7-K, D-8-K.
- README §2 "Spec before implementation" --
  rendering is implementation; defer until the spec
  is fully closed.

**Prior-art evidence.** Direct cascade from stages
5-8: every body-content renderer defers to stage 11.
No project-specific precedent needed.

**§18.2 per-method ownership table update.** Adds
4 public renderer rows for stage-9 deferred methods
(not the private `render_terminator_kind` dispatch
helper):
- `render_terminator(&Terminator, &Body)` -- stage 11.
- `render_call_callee(&CallCallee, &Body)` -- stage 11.
- `render_switch_targets(&SwitchTargets, &Body)` --
  stage 11.
- `render_assert_kind(&AssertKind, &Body)` -- stage 11.

**Outbound to stage 11.** Three-item bundle (mirroring
D-8-K's three-item outbound):
1. `render_terminator` deferral with the four
   sub-renderers.
2. Terminator walk implementation (D-9-10 Part C).
3. Body-level vtable-indirect rewrite pass per D-9-3
   (b).

Plus the bridge concerns from D-9-2 (Drop-as-Call):
fresh LocalIds for `&raw mut place` temp + fresh
unit-typed return slot. Filed in
`stage-11/expectations.md` "From stage 9".

**Cross-refs.** `spec-snapshot.md` §23.9 (rendering
deferral note + per-variant dependency table); §18.2
(per-method ownership table update).

---

## 2. Outbound expectations summary

To file in `stage-11/expectations.md` "From stage 9":

1. `render_terminator` deferral with 4 sub-renderers
   (D-9-K).
2. Terminator walk implementation (D-9-10 Part C).
3. Body-level vtable-indirect rewrite pass (D-9-3 (b)
   outbound; closes stage-3 D5 inbound shape question).
4. Fresh LocalId allocation for `&raw mut place` temp
   (D-9-2).
5. Fresh unit-typed return slot for drop-glue Call
   (D-9-2).

To cross-ref in `stage-12/expectations.md` "From
stage 9":

- "Possibly extend `DefPathData`" question CLOSED
  with "no extension required" disposition under
  D-9-3 (b).
- Vtable shim Item synthesis cross-ref to existing
  "From stage 3" Section A/C entries (informational).

Top-level `spec.md` §18.2 per-method ownership table
update for `render_terminator`, `render_call_callee`,
`render_switch_targets`, `render_assert_kind` (all
stage-11 owner) lands at merger close.

## 3. Earlier-stage decisions touched

**Stage-1 inbound expectations closed:**
- "Drop lowered to Call" (closed by D-9-2).
- "Vtable-indirect dispatch (Call terminator shape)"
  (closed by D-9-3 (b)).
- "Possibly extend `DefPathData` for synthetic vtable
  scopes" (closed only as it relates to stage-9's
  call-callee shape; under D-9-3 (b), no Virtual-shim-
  scope DefPathData variant is needed at stage 9.
  Stage-12's separate vtable Item DefPathData encoding
  choice remains open per
  `stage-12/expectations.md` "From stage 3" Sections A
  and B; D-9-3 does not constrain it.)
- Construction invariant 9 (closed by D-9-10 B-7
  rules + outbound walk to stage 11).

**Stage-3 D5 inbound closed:**
- "Define Call terminator shape supporting
  vtable-indirect dispatch" (closed by D-9-3 (b)).

**Stage-6 D-6-1.3 inbound superseded:**
- "`Spanned<Operand>` per-arg span preservation"
  (superseded by D-9-4: the inbound listed (a) and
  (b) preservation shapes; D-9-4 introduced (c)
  drop-entirely on consumer-landscape grounds).
  Closure is by revision, not by satisfaction.

No earlier-stage decisions reopened by stage 9. No
cross-stage friction surfaced during Phase A discussion.

---
