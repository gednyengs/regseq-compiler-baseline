# Stage 11 -- MIR Bodies: Stage Memory

Stage-11-specific briefing for an LLM agent picking up the
body-level work. Read in addition to (not instead of) the
top-level `../README.md` (architectural foundation, global
methodology rules) and `../status.md` (top-level progress
tracker).

This file is referenced **only** from `../status.md`'s
"Active focus" block. The top-level `../README.md` does not
reference it -- by convention, to keep top-level context lean.

This file captures stage-11 *invariants* (mission, workflow,
principles, read order, Codex prompt template). Stage-11
*current state* (which decisions are locked, what's open,
active cluster, next concrete unit of work) lives in
`status.md`.

## 0. Mission

Stage 11 specifies the `Body` function-level container, the
`LocalDecl` per-local entry, the `LocalId` typed-index, the
body-level validation walks deferred from stages 5/8/9/10,
and the body-level transformation passes deferred from
stages 3/9 (vtable-indirect rewrite + Drop-as-Call collapse).
Unlike stages 7/8/9 (vocabulary enums) and stage 10 (per-block
container), stage 11 is the **body-level integration stage**:
it pins how all earlier stages' content composes inside one
function-level unit and how the bridge constructs that unit.

Stage 11 sits **seventh in the MIR pipeline** (stages 5-11
form a strict bottom-up chain: places -> operands -> rvalues
-> statements -> terminators -> basic blocks -> **bodies**).
It is the last MIR-shape stage; stage 12 introduces the
project-level `Item` table that bodies live inside.

Stage 11's content is referenced by:

- **Stage 12 Items.** Per D-11-5 candidate alt (c), bodies
  live as a structural field on Items: `Item.body:
  Option<Body>`. RegseqModel does not allocate a separate
  `BodyId`; the convention "one body per item" is honoured
  with shims (drop glue, vtable instances, fn-pointer
  shims) materialised as their own Items per stage-1 +
  stage-12 synthesis machinery. Per D-11-3 candidate alt
  (e), the function signature also lives on the Item
  (`Item.kind = Function { fn_sig, ... }` per stage-9 §9.4
  + stage-12 expansion), NOT on the body -- a 3-peer
  convergence (rustc-internal, rustc_public, Charon).
- **Stage 13 pretty-printing.** Per the categorical D-11-K
  decision, stage 13 owns ALL rendering for body-level
  content (`render_place`, `render_operand`, `render_rvalue`,
  `render_statement`, `render_terminator` + 4 sub-renderers,
  `render_basic_block`) plus the `_N` sigil for `LocalId`
  and the `bbN` sigil for `BlockId`, in addition to the
  `render_ty` already in stage-13 scope. Stage 11 hands
  stage 13 a frozen body-level vocabulary; stage 13
  decides on rendering style coherently.
- **Stage 14 backends.** Each `Body` lowers to a C
  function. `Body.local_decls` enumerates the C locals
  (return slot, parameters, temporaries; positional layout
  per stage-1 ID conventions extended to LocalId).
  `Body.basic_blocks` enumerates the labelled C basic
  blocks. The function signature drives the C function
  declaration via the parent Item.

Stage 11 receives content from earlier stages:

- **Stage 1** (foundation crate, ID conventions). `LocalId`
  forward-declared conceptually only; stage 11 introduces
  the typed-index itself via `newtype_index!` with
  `RETURN_PLACE = 0` const-body declaration (using
  stage-10's macro arm extension), a `bb`-paralleling `_N`
  sigil that stage 13 will own at render time, and the
  `Option<LocalId>` / `LocalId` 4-byte size assertions.
  Stage 11 also expands the empty `Body` and `FnSig`
  placeholders out of `crates/regseq-model/src/placeholders.rs`
  into their own modules.
- **Stage 3** (type system, D5 dyn-Trait dispatch). The
  bridge populates `Ty::Dynamic.vtable_ty: TyId` and
  registers vtable type Items (stage-12 synthesis).
  Stage 11 owns the **body-level rewrite pass** that
  detects `Cast::Unsize` rvalues coercing
  `Concrete -> dyn Trait` (triggering vtable-instance
  registration via stage 12) and rewrites
  `<dyn Trait>::method` Calls into vtable-indirect
  dispatch.
- **Stage 5** (Places, D-5-2). Place validation contract
  (5 rule categories pinned in §19.7). Stage 11 owns the
  walk implementation.
- **Stage 6** (Operands). Operand-site invariants
  (§20.5). Inherited via the joint body walk.
- **Stage 7** (Rvalues, D-7-3). Rvalue-site invariants
  (§21.5). Inherited via the joint body walk.
- **Stage 8** (Statements, D-8-3, D-8-4 Part C). Named-
  local preservation (`Body.var_debug_info` consumed at
  construction time to populate per-`LocalDecl.debug_name:
  Option<StringId>`). Statement walk implementation.
- **Stage 9** (Terminators, D-9-2, D-9-3 (b), D-9-10
  Part C). Drop-as-Call bridge collapse rules (with
  fresh-LocalIds for `_t_drop_arg` + `_t_drop_ret`).
  Vtable-rewrite Call shape: `CallCallee::Indirect(
  Operand)` carrying the loaded vtable function pointer.
  Terminator walk implementation. Construction invariant
  9 closure (every `Direct(InstanceId)` resolves in
  `model.instances`).
- **Stage 10** (BasicBlocks). `Body.basic_blocks:
  IndexVec<BlockId, BasicBlock>` storage shape locked at
  D-10-2; storage IS in reverse-postorder (B-RPO);
  `START_BLOCK = BlockId(0)` (B-Entry). Joint
  DFS-RPO walk procedure pinned in §24.4.3 with 6 steps
  (B-NonEmpty pre-check, B-Successors-Valid pre-check,
  DFS-RPO traversal, B-Reachable assertion, B-RPO
  assertion, B-Entry implicit). Stage 11 owns the walk
  implementation.

The output of stage 11 is `spec-snapshot.md` (the canonical
per-stage spec). This file briefs the agent on *how*
decisions about that spec are reached.

Stage 11's surface IS large -- it is the convergence point
of stages 5/8/9/10 deferred walks, stage-3 D5 deferred
rewrite, stage-9 D-9-2 deferred Drop-as-Call collapse,
stage-9 D-9-3 (b) Call shape commitment, stage-1 inbound
LocalId/Body/FnSig expansion, stage-8 D-8-3 named-local
preservation, and the categorical D-11-K rendering
deferral. Mirrors the situation of a "settling" stage that
binds many threads from earlier work.

## 1. Workflow (the rhythm we use)

Stage 11 inherits the two-phase rhythm proven in stages 3,
4, 5, 6, 7, 8, 9, and 10:

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
big-bang.** Stage 11's decision count (8 main + 1
sub-decision + D-11-K = 10 locks total) is comparable to
stage 9 (11 D-tags) and larger than stage 10 (6 + sub).
The default plan is a **single combined cluster** covering
all 8 main D-tags + sub-decisions + D-11-K (mirrors
stages 6-10). If Phase A surfaces a clean cluster split
(e.g., data-structure decisions D-11-1/2/3/4/5 separable
from walk/pass decisions D-11-6/7/8) the parallel-session
machinery in `../README.md` §5.3 is available; the
single-cluster default avoids the merge-tier overhead
unless evidence demands it.

`discussions.md` holds the distilled decision record;
`review-log.md` holds the round-by-round audit trail.

**Decision tag convention.** Stage 11 uses the prefixed
format `D-11-K` from day one (`D-11-1`, `D-11-2`, ...,
with sub-tags `D-11-6.1`, etc., per `../README.md` §5.2).
Stages 1 and 3 use legacy un-prefixed forms; stage 11
does not inherit those.

## 2. Stage-11-specific principles

Corollaries of `../README.md` "Methodology rules" applied
to the body-level surface:

- **Store rustc's resolved body form, not the surface
  expression.** The bridge consumes `mir::Body<'tcx>` from
  `optimized_mir` with `mir_opt_level=0` and the cleanup-
  pass whitelist applied. Stage-11's `Body` field set is
  the small subset of rustc's 17 fields that survives
  RegseqModel's binding constraints (post-mono, no DWARF,
  no borrowck, no coroutines, no instrumentation, no
  collector residue, C-shape codegen). The 17->~6 collapse
  derives mechanically from those constraints; stage 11
  formalises the collapse with rejection-table reasoning
  on the dropped fields.

- **`Body` field set: candidate ~6 fields.** The kickoff
  evidence pass and 4-peer prior-art converge on a small
  surviving set: `basic_blocks`, `local_decls`, `arg_count`,
  `span`, plus a Body-vs-Item linkage shape and possibly a
  `Locals` substructure collapsing `local_decls + arg_count`
  (Charon precedent at `gast.rs:31-41`). Stage 11 picks the
  exact shape under D-11-1.

- **`LocalDecl` field set: 4-field candidate.** Stage-8
  D-8-3's named-local preservation lifts rustc's body-level
  `var_debug_info` mapping into a per-`LocalDecl.debug_name:
  Option<StringId>` field. Combined with the type, the
  mutability marker, and (open question) per-local span,
  the candidate is `{ ty: TyId, mutability: Mutability,
  span: Span, debug_name: Option<StringId> }`. Charon
  precedent supports the lift (`Local { index, name, ty }`);
  rustc_public + mrustc keep the mapping body-level
  (silent / negative). The lift is a deliberate divergence
  justified by C-shape ergonomics: per-local lookup at
  render time + codegen identifier emission without a
  secondary scan over `var_debug_info`. Per-local span
  drop is an open pressure point (Charon drops it).

- **`FnSig` lives on the Item, NOT on the Body.** Per the
  D-11-3 candidate alt (e) (3-peer convergence: rustc-
  internal, rustc_public, Charon all keep signature on
  the function-level item, with rustc reaching it through
  `tcx.fn_sig(def_id)`, rustc_public through
  `FnDef::fn_sig()` at `ty.rs:713-717`, Charon through
  `FunDecl.signature` separate from `FunDecl.body`).
  Stage-1's inbound expectation framed FnSig as a
  structural field of Body; stage 11 reframes per evidence
  (FnSig is an Item-side concern, since Items are what
  carry call-site identity per stage-9 §9.4 +
  `InstanceKind::Function`). Stage 12 owns the Item-side
  FnSig integration; stage 11 simply does NOT carry a
  body-level `fn_sig` field. Argument types are still
  reachable through `Body.local_decls[1..=arg_count]`
  (positional convention from rustc + rustc_public);
  return type via `Body.local_decls[0]` (the return slot).

- **`LocalId` typed-index, not bare `usize`.** Per
  stage-1's ID-conventions framework (every domain ID is
  a u32-backed `newtype_index!` instance with a sigil and
  `Option<X>` niche optimisation), stage 11 introduces
  `LocalId` as a proper typed-index using stage-10's
  const-body macro arm:

  ```rust
  newtype_index! {
      pub struct LocalId {
          const RETURN_PLACE = 0;
      }
  }
  ```

  Mirrors stage-10's `BlockId::START_BLOCK = 0`
  declaration. The `_N` sigil rendering convention
  (`LocalId(N)` -> `_N`, e.g. `_0` for `RETURN_PLACE`,
  `_1`, `_2`, ...) is a rendering decision deferred to
  stage 13 per D-11-K.

- **Body-vs-Item linkage: `Item.body: Option<Body>`.**
  Per the D-11-5 candidate alt (c) (3-peer convergence:
  rustc has no BodyId and reaches bodies via
  `tcx.instance_mir(InstanceKind)`; rustc_public has
  `Instance::body() -> Option<Body>` at `mono.rs:53-55`;
  Charon has `FunDecl.body: Body` direct). Composes with
  Q3 (single-level IR) and the Item-as-anchor pattern.
  Stage 11 carries no `BodyId`; stage 12 absorbs Body as
  a structural field of `Item`.

- **Validation walk: visitor with override hooks.** Per
  D-11-6 candidate alt (b) (rustc_public's
  `make_mir_visitor!` template at `mir/visit.rs:42-517`
  + Charon's bi-modal `VisitAst`/`VisitBody` split at
  `src/ast/visitor.rs:130-191`). The 17-hook
  visit_X / super_X discipline (with exhaustive
  destructuring, no `..`, no catch-all `_`) is the right
  shape; mrustc's direct-loop walk in `MIR_Validate` at
  `check.cpp:445` is negative precedent (forfeits
  composability). The visitor lives in `regseq-model`
  per stage-10's principled model-vs-analysis line: the
  IR knows how to validate itself; CFG analyses
  (predecessors, dominators, RPO iteration) live in
  `regseq-analysis`.

- **Construction invariants 1 and 9 fold into the
  visitor.** Per D-11-6.1 candidate alt (a). No separate
  pre-pass: invariant 1 (TyId closure for body-side
  IDs: `LocalDecl.ty`, `Field`'s inline TyId,
  `AggregateKind`'s per-variant TyId, etc.) folds into
  the visitor's `visit_local_decl` + `visit_ty` hooks;
  invariant 9 (Direct(InstanceId) closure for callees)
  folds into `visit_terminator`'s `Call` arm.
  Compositional with the other walks. rustc's
  `non_ssa_locals` at `analyze.rs:17-56` is precedent for
  "lightweight visitor pre-walk populates a sidecar"
  but stage 11 doesn't need a sidecar; the closure
  invariants are simple membership tests.

- **Body-level transformation pass timing: hybrid (c).**
  Per D-11-7 candidate (Charon + mrustc convergence):
  - **Drop-as-Call collapse runs at bridge construction
    time.** The bridge translates each rustc
    `TerminatorKind::Drop` atomically into a
    `Statement::Assign(_t_drop_arg, Rvalue::RawPtr(Mut,
    place))` appended to the current block's statements
    list + a `TerminatorKind::Call` with destination
    `_t_drop_ret` and the resolved drop-glue Instance as
    callee. Per stage-9 §23.4 step 4. `_t_drop_arg` and
    `_t_drop_ret` allocate via the LocalDecls API
    (D-11-8).
  - **Vtable-indirect rewrite runs as a post-construction
    pass.** The bridge first produces a RegseqModel
    where dyn-Trait Calls have `Direct(InstanceId)`
    callees pointing to virtual instances (per stage-9's
    `flavor: FunctionInstanceFlavor::Virtual` shape +
    `virtual_idx: Some(idx)` in `InstanceKind::Function`
    per §9.4). A subsequent body-level pass walks every
    `BasicBlock.terminator`, detects `Call` terminators
    with virtual-flavor callees, and rewrites them in
    place: insert the projection-chain Statements
    (`_t_recv = Use(arg[0])`; for Ref receivers a
    source-normalization `_t_raw = RawPtr(<mut>,
    *_t_recv)`; `_t_meta = UnaryOp(PtrMetadata, _t_recv)`;
    `_t_fn = Use(*_t_meta.virtual_idx)`; `_t_data =
    Cast(PtrToPtr, _t_recv_or_raw, *unit)`), replace
    `Call.callee` with `Indirect(Move(_t_fn))`, replace
    `Call.args[0]` with `Move(_t_data)`. Per stage-9
    expectations.md "From stage 9: Body-level vtable-
    indirect rewrite pass" + Charon
    `transform_dyn_trait_calls` at
    `src/transform/normalize/transform_dyn_trait_calls.rs`
    + mrustc `MIR_Cleanup_Virtualize` at
    `src/mir/cleanup.cpp:548-684`.

  Rationale for the timing split: Drop-as-Call fits
  inside the per-rustc-terminator translation envelope
  (single rustc terminator -> 1 Statement + 1 Call);
  vtable-rewrite needs the full `RegseqModel` (resolved
  Instances, populated Item table for vtable types) to
  walk Calls and rewrite. Bridge-time rewrite would
  require interleaved Item synthesis with body
  construction, which contradicts Q2 (eager translation;
  fully-owned IR is the bridge output). rustc-internal's
  alternative (no rewrite; dispatch direct at codegen
  via `Instance::Virtual` + `tcx.vtable_entries`) is
  structurally inadmissible for C-emission under Q2.

- **Cross-stage friction with stage-5 D-5-1: explicit
  documentation.** The vtable-rewrite multi-statement
  shape stage-9 expectations.md specifies (5 fresh locals
  including source-normalization for `&dyn Trait`
  receivers) is FORCED, not a free design choice.
  Charon's in-place projection-chain rewrite at line 119
  (`call.func = FnOperand::Move(method_field_place)`,
  no fresh locals) requires a `PtrMetadata` PlaceElem
  variant **stage-5 D-5-1 does NOT have** (the locked
  6-variant set is Deref / Field / Index / ConstantIndex
  / Subslice / Downcast). Without `PtrMetadata` as a
  PlaceElem, the metadata extraction MUST be a separate
  Rvalue::UnaryOp(PtrMetadata, ...) in a Statement, and
  the chain expands accordingly. Stage 11 spec explicitly
  records this cross-stage friction: the rewrite shape
  is the only one stage-5's locked vocabulary admits;
  Charon's compactness is purchased by a different
  PlaceElem set that we don't have.

- **LocalDecls allocation API: lean toward method-on-
  Body.** Per D-11-8 candidate alt (a): a
  `Body::push_local(LocalDecl) -> LocalId` (or analogous
  method on a `Locals` substructure if D-11-1 picks
  Charon's collapse). Charon's `Locals::new_var` +
  `BodyTransformCtx` trait helpers (`fresh_var`,
  `borrow_to_new_var`, `raw_borrow_to_new_var` at
  `src/transform/ctx.rs`) are layered precedent;
  mrustc's split (`MirBuilder::new_temporary` for
  construction time + `MirMutator::new_temporary` for
  transformation passes with pending-statements
  buffering) is a finer-grained alternative. The bridge
  uses the method during Drop-as-Call collapse (allocates
  `_t_drop_arg` + `_t_drop_ret` per Drop site) and during
  vtable-rewrite (allocates the projection-chain locals).
  Note Charon's auto-`StorageLive`-on-fresh-var does NOT
  transfer (RegseqModel D-8-1 dropped `StorageLive`).

- **Categorical rendering deferral to stage 13 (D-11-K).**
  All renderer functions and sigils for body-level
  content move to stage 13:
  - `render_place(&Place, &Body)` (was D-5-K)
  - `render_operand(&Operand, &Body)` (was D-6-K)
  - `render_rvalue(&Rvalue, &Body)` (was D-7-K)
  - `render_statement(&Statement, &Body)` (was D-8-K)
  - `render_terminator(&Terminator, &Body)` (was D-9-K),
    plus 4 sub-renderers (`render_call_callee`,
    `render_switch_targets`, `render_assert_kind`, and
    the internal `render_terminator_kind` helper)
  - `render_basic_block(&BasicBlock, &Body)` (was
    D-10-K)
  - `_N` sigil for `LocalId` rendering (was stage-1
    inbound)
  - `bbN` sigil for `BlockId` rendering (was stage-10
    D-10-K.2)

  Rationale: stage 13 is responsible for pretty-printing
  decisions for the IR. Renderer style, sigil
  conventions, and per-method ownership form a coherent
  surface that benefits from a single stage owning it.
  Stage 11 hands stage 13 a frozen body-level vocabulary
  (Body / LocalDecl / FnSig / LocalId all pinned with
  no rendering claims); stage 13 picks the rendering
  shape with full input-readiness on top of `render_ty`
  (D14 deferred from stage 3) which itself is a stage-13
  concern.

  This decision **categorically retargets** the per-
  stage K-tag deferrals (D-5-K through D-10-K + the
  stage-1 `_N` sigil) from "deferred to stage 11" to
  "deferred to stage 13." The merger contract at
  stage-11 close flips `D-5-K`, `D-6-K`, `D-7-K`,
  `D-8-K`, `D-9-K`, `D-10-K` to `[REOPENED]` in
  top-level `decisions.md` with annotation
  `(retargeted to stage 13 per D-11-K)`. The
  bookkeeping operational consequence: rendering
  entries move out of `stage-11/expectations.md` into a
  new `stage-13/expectations.md` "From stage 11 (per
  D-11-K)" bundle preserving attribution to the
  predecessor stages. This migration is performed at
  stage-11 Phase B close per the merger contract.

  **Complementary scope retention at stage 11:** stage
  11 still owns the body-level *data structure spec*
  (Body / LocalDecl / FnSig location / LocalId
  definition), the *validation walks* (Place / Statement
  / Terminator / Body-level DFS-RPO inheritance), and
  the *transformation passes* (vtable-rewrite + Drop-as-
  Call). Only RENDERING moves; structural spec stays.

- **Source-side debuginfo at construction time only.**
  rustc's `Body.var_debug_info: Vec<VarDebugInfo<'tcx>>`
  carries `VarDebugInfo { name, source_info, composite,
  value, argument_index }` entries (rustc_middle/src/
  mir/mod.rs:264, 1235-1257). At bridge construction
  time, the bridge translates each entry whose
  `value = VarDebugInfoContents::Place(p)` with empty
  projection into a per-`LocalDecl.debug_name` for
  `p.local`. The body-level vector is NOT carried into
  RegseqModel. Confirmed analog: rustc itself partitions
  `var_debug_info` per-`Local` for codegen consumption
  via `FunctionCx.per_local_var_debug_info:
  Option<IndexVec<Local, Vec<...>>>` at
  `rustc_codegen_ssa/src/mir/mod.rs:46-47, 119` --
  structurally identical to D-8-3's design. Composite
  / non-Place / non-empty-projection entries are
  bridge-rejected (they encode debugger expressions
  RegseqModel doesn't use).

- **Compositional behavior beats special-case flags.**
  Same as stages 3-10. Examples for stage 11:
  - The `LocalDecl.debug_name = None` case (no source
    name; compiler-synthesised temporary or bridge-
    introduced rewrite-pass local) falls out of the
    `Option<StringId>` schema; no parallel
    "is_synthetic_local" flag.
  - The Body↔Item linkage falls out of the Item-as-
    anchor pattern (`Item.body: Option<Body>`); no
    parallel `BodyId` table or back-pointer.
  - The hybrid (c) timing for transformation passes
    falls out of the bridge envelope vs whole-Body
    requirements; no parallel "this Call needs rewrite"
    flag on Calls.
  - Drop-as-Call's Statement insertion falls out of
    the bridge's atomic per-terminator translation
    envelope; no parallel "synthetic statement"
    annotation.
  - Vtable-rewrite's invariant timing (B-3 / B-4 / B-6
    hold post-rewrite, not pre-rewrite, per stage-9
    §23.6.2) falls out of the rewrite running before
    the validation walk; no parallel "pre-rewrite"
    invariant tier.

- **C-semantics for the body surface.** Each `Body`
  lowers to a C function:
  ```c
  /* signature from Item.kind=Function.fn_sig */
  ret_ty fn_name(arg_ty1 arg1, ..., arg_tyN argN) {
      /* locals from local_decls[arg_count+1..] */
      local_ty1 _N1; ...

      /* basic blocks from body.basic_blocks */
      bb_0: { stmt_0; ...; terminator; }
      bb_1: { ... }
      ...
  }
  ```
  The label naming, identifier emission policy (named
  vs `_N`), and basic-block envelope are stage-13/14
  concerns. Stage 11 fixes the data the C function
  consumes.

- **F4 / F4-2 cross-build concerns are out of scope.**
  Body content is per-build. `LocalId` and `BlockId`
  numberings are body-local and need not canonicalise
  across builds. The Item-side `InstanceKey` is what
  canonicalises across builds (stage 1 + stage 12);
  stage 11's body-level content composes downstream.
  Same scope reduction as stages 5-10.

## 3. Read order for new agents picking up stage 11

In priority:

0. **If you are opening a specific D-tag**: read this
   file + `status.md` + `discussions.md` entries for the
   decisions in your cluster (once they exist) + the
   relevant spec sections in top-level `../spec.md`
   (Section 2 (`LocalId` ID inventory once stage 11
   formalises it), Section 9 (Instance / InstanceKind
   per stage-9 §9.4), Section 11 (Layout vocabulary
   for type/field/variant resolution via local types),
   Section 18.2 (per-method ownership table),
   Section 19 (Place + projection rules per stage 5),
   Sections 20-23 (Operand / Rvalue / Statement /
   Terminator vocabulary), Section 24 (BasicBlock +
   body-level table per stage 10).

1. This file (`stage-11/memory.md`) -- stage-11 invariants.

2. `status.md` -- current state (locked decisions, active
   cluster, next concrete unit of work).

3. `spec-snapshot.md` -- the working spec (initially a
   copy of `../spec.md`, 10463 lines; stage-11
   contributions accumulate as new sections describing
   `Body`, `LocalDecl`, `FnSig` location decision,
   `LocalId` introduction, joint validation walk,
   transformation passes, named-local preservation, and
   the per-stage invariants registry).

4. `expectations.md` -- inbound forward refs (sizable
   bundle covering stages 1, 3, 5, 6, 7, 8, 9, 10 plus
   bridge concerns; rendering items flagged as
   scope-deferred-to-stage-13 per D-11-K).

5. `discussions.md` -- distilled decision records
   (created when the first decision lands; lazy).

6. `review-log.md` -- round-by-round Codex audit trail
   (created when the first review round runs; lazy).

7. `prior-art/<project>.md` -- per-stage primary-source
   research (fresh pass per `../README.md` §2
   methodology rule -- do **not** reuse stage-0 or
   earlier-stage summaries; per-D-tag framing). Landed
   at stage open:
   - `prior-art/rustc.md` -- rustc-internal primary
     source under the peer-compiler-design lens. Body
     17-field set with rationale for each drop;
     LocalDecl 5-field + 9-variant LocalInfo with
     rationale; FnSig storage choice (no body-level
     cache; signature reached through `tcx.fn_sig`
     query); newtype_index! Local with `RETURN_PLACE = 0`
     const-body grammar at `rustc_index_macros/src/newtype.rs:87`;
     `instance_mir(InstanceKind)` as the only body-fetch
     path with no BodyId; `make_mir_visitor!` template
     at `mir/visit.rs:1-160`; codegen-SSA vs cranelift
     two-walk-shape comparison; rustc's deliberate
     non-rewrite for dyn-Trait dispatch (codegen-time
     direct dispatch via `Instance::Virtual` +
     `meth::VirtualIndex::from_index(idx).get_fn(...)` at
     `rustc_codegen_ssa/src/mir/block.rs:1165-1211`);
     `MirPass::run_pass(tcx, body: &mut Body)` interface
     at `mir_transform/elaborate_drops.rs:50-91` as the
     post-construction-pass implementation template.
   - `prior-art/rustc_public.md` -- rustc_public stable
     IR layer. 6-field Body `{ blocks, locals, arg_count,
     var_debug_info, spread_arg, span }` at
     `body.rs:13-38`; **no body-level fn_sig** (D-11-3
     alt (e) load-bearing precedent; signature reached
     via `FnDef::fn_sig() -> PolyFnSig` at
     `ty.rs:713-717`); 3-field LocalDecl `{ ty, span,
     mutability }` at `body.rs:126-131` (no debug_name
     -- D-8-3 divergence); body-level `var_debug_info`
     retained + `VarDebugInfo::local()` helper at
     `body.rs:732-739`; bare `pub type Local = usize`
     at `body.rs:850` with `RETURN_LOCAL = 0`;
     `Instance::body() -> Option<Body>` at
     `mono.rs:53-55` (D-11-5 alt (c) load-bearing
     precedent); `MirVisitor` / `MutMirVisitor` via
     `make_mir_visitor!` macro at `visit.rs:42-517`
     (D-11-6 LOAD-BEARING template; 17 visit/super hook
     pairs; exhaustive-destructuring discipline);
     `Body::new` consumes complete `Vec` collections
     with `pub(super)` privacy (no incremental push API;
     construct-once works for bridge-time, not for
     post-construction passes -- pressure on D-11-8).
   - `prior-art/charon.md` -- Charon ULLBC layer. 3-field
     Body `{ basic_blocks, locals: Locals, span }` with
     `Locals { arg_count, locals: Vector<LocalId, Local> }`
     substructure at `gast.rs:31-41` (D-11-1 collapse-
     candidate); `Local { index, name: Option<Symbol>,
     ty }` 3-field at `gast.rs:47-52` (no mutability,
     no span; debug_name lift POSITIVE precedent for
     D-8-3); `FunDecl.signature: FunSig` separate from
     `FunDecl.body: Body` (D-11-3 alt (e) precedent);
     `generate_index_type!` macro at `src/ids/mod.rs`
     with `MAX_INDEX = u32::MAX` niche; bi-modal
     `VisitAst` / `VisitBody` visitor at
     `src/ast/visitor.rs:130-191` (D-11-6 alt (b)
     supporting precedent); **`transform_dyn_trait_calls`
     at `src/transform/normalize/transform_dyn_trait_calls.rs:31-122`**
     (D-11-7 LOAD-BEARING precedent: post-construction
     `UllbcPass` registered in INITIAL_CLEANUP_PASSES
     at `src/transform/mod.rs:104, 111`; in-place
     projection-chain rewrite at line 119; **requires
     `PtrMetadata` PlaceElem stage-5 D-5-1 lacks**);
     `desugar_drops` at `desugar_drops.rs:23-72`
     (D-11-7 supporting precedent; opt-in
     `--desugar-drops` pass; rewrites `Drop` to `Call`
     with `&raw mut place` arg + fresh unit-typed
     return slot + copied target/on_unwind block ids);
     `Locals::new_var(name, ty) -> Place` +
     `BodyTransformCtx::fresh_var` /
     `borrow_to_new_var` / `raw_borrow_to_new_var` trait
     at `src/transform/ctx.rs` (D-11-8 alt (a)
     supporting precedent; auto-`StorageLive`-on-fresh-
     var does NOT transfer to RegseqModel D-8-1).
   - `prior-art/mrustc.md` -- mrustc Rust-to-C peer.
     3-field minimum `{ locals, drop_flags, blocks }` at
     `mir.hpp:1117-1130`; explicit rejection of named-
     local preservation (commented-out `local_names`;
     D-8-3 NEGATIVE precedent); typedef-asymmetry
     (`BasicBlockId` typed at `mir.hpp:20`, locals raw
     `unsigned int`; D-11-4 closes the asymmetry);
     direct-loop walk in `MIR_Validate` at
     `check.cpp:445` (D-11-6 NEGATIVE precedent;
     forfeits visitor composability); inline-fold via
     `TU_MATCH` (D-11-6.1 alt (a) supporting precedent);
     **`MIR_Cleanup_Virtualize` at `cleanup.cpp:548-684`**
     (D-11-7 LOAD-BEARING precedent: post-construction
     post-monomorph timing; single-field `Call.fcn`
     rewrite at line 685 matching `CallCallee::Indirect(
     Operand)` shape); `MirBuilder::new_temporary` at
     `mir_builder.cpp:232` for construction time +
     `MirMutator::new_temporary` at `cleanup.cpp:25-100`
     for transformation passes with pending-statements
     buffering (D-11-8 split-API alternative).

   Per-cluster top-up candidates (research only when the
   cluster's evidence demands it):
   - LLVM IR `Function` body shape and the parent
     `Function`'s `BasicBlock` list (relevant if D-11-1's
     Body field set decision needs an additional
     non-rustc peer perspective).
   - rustc-dev-guide MIR overview
     (https://rustc-dev-guide.rust-lang.org/mir/) for
     cross-reference of `Body` semantics.

8. `../README.md` -- top-level briefing (invariants only).

9. `../status.md` -- top-level progress tracker (cross-
   stage signals).

10. `../spec.md` -- merged top-level spec; consult
    Sections 2 (IDs and body-local ID constants), 9
    (Instance + InstanceKind per stage-9), 11 (Layout
    vocabulary), 18.2 (per-method ownership table for
    pretty-printer), 19-24 (Place / Operand / Rvalue /
    Statement / Terminator / BasicBlock vocabulary).

## 4. Codex review cadence and prompt template

The cadence and current round status live in `status.md`
"Codex review cadence (status)." This section captures the
*template* (reusable across rounds and clusters).

Prompt template skeleton (paste-and-edit per round; per the
project root `CLAUDE.md` "Prompt shape" guidance):

```
<task>
Adversarial review of stage-11 body-level spec, decisions <X>-<Y>, ROUND <N>.

[If N > 1: round context + summary of prior corrections]

Spec under review: docs/discussions/regseq-model/stage-11/spec-snapshot.md (focus: <sections>).
Out of scope: <e.g. unrelated D-tags, markdown style, rendering decisions deferred to stage 13 per D-11-K>.
Evidence files: <full list relevant to this cluster, including 4 prior-art/*.md files>.
Evaluate: <numbered criteria; 1. round-N-fix verification if N > 1; 2. new issues from corrections; 3. residual issues; 4-N. soundness/citation/consistency, especially the cross-stage friction with stage-5 D-5-1 PtrMetadata and the D-11-3 alt (e) reframe of stage-1 inbound>.
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
  stage-11 invariants change (rare).
- **Not where decisions live.** Decisions live in
  `spec-snapshot.md` (and `discussions.md` for audit shape).
