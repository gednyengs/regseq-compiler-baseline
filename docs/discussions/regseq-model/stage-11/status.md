# Stage 11 Status

Status: **PHASE B COMPLETE 2026-05-05.** Per-stage Codex
review CLOSED at R6 APPROVE with Lockability: yes for all
10 D-tags and Close-out: YES. Stage-11 ready for top-level
merger contract per README §5.5 7-step protocol.

Phase B trajectory: 6 productive Codex rounds (1 timed-out
attempt) + 19 distinct substantive findings closed
(9 BLOCK + 10 MINOR). Stage-11 lands in the
stage-9 / stage-10 convergence band (stage-9: 6 rounds /
21 findings; stage-10: 5 rounds / 19 findings).

Phase A baseline: all 10 D-tags
locked in conversation across a single combined cluster
(D-11-1 Body 4-field shape; D-11-2 LocalDecl 4-field
shape; D-11-3 FnSig on Item with stage-1 inbound reframe;
D-11-4 LocalId via newtype_index! + RETURN_PLACE = 0;
D-11-5 no BodyId; bodies as Item structural field; D-11-6
joint validation visitor with 10 hook pairs; D-11-6.1
construction invariants 1+9 fold-in; D-11-7 hybrid (c)
timing; D-11-8 single push API + synthetic-local helper;
D-11-K categorical rendering deferral to stage 13).

Phase A discussion order: D-11-1 -> D-11-2 -> D-11-3 ->
D-11-4 -> D-11-5 -> D-11-6 + D-11-6.1 -> D-11-7 -> D-11-8
-> D-11-K. Notable substantive question raised mid-Phase-A:
whether stage-9 D-9-3 should be reopened to alt (a) (keep
Direct(InstanceId) for virtual; codegen-side dispatch)
to remove the stage-11 vtable-rewrite pass; concluded with
"don't reopen, proceed with hybrid (c)" -- the cross-stage
friction with stage-5 D-5-1 (no PtrMetadata PlaceElem)
forces multi-statement rewrite shape regardless of which
stage owns the rewrite, and stage-9's IR-uniformity
preference is reasonable.

Phase 0 baseline: stage-11 folder bootstrapped per
`../README.md` §5.1; `spec-snapshot.md` copied from
top-level `../spec.md` (10463 lines); inbound
`expectations.md` read as entry checklist; rustc kickoff
evidence pass completed; per-stage prior-art research
landed in 4 primary-source files (rustc.md ~5800 words,
rustc_public.md ~3000 words, charon.md ~5400 words,
mrustc.md ~3500 words); `memory.md` and this `status.md`
drafted from grounded knowledge.

**STAGE 11 CLOSED 2026-05-05.** Merger contract per README
§5.5 7-step protocol completed: top-level `discussions.md`,
`review-log.md`, `decisions.md` extended with stage-11
content; D-5-K through D-10-K + D-10-K.1/.2/.3 flipped to
`[REOPENED]` per D-11-K's categorical retargeting;
`spec-snapshot.md` integrated into top-level `spec.md`;
post-merge tier-3 Codex review R1-R5 closed at R5 APPROVE
on 2026-05-05; outbound bundles filed in
`stage-12/expectations.md` "From stage 11" and
`stage-13/expectations.md` "From stage 11 (per D-11-K)";
this `spec-snapshot.md` frozen with the snapshot header.

The current authoritative spec lives in top-level
`../spec.md`; this `spec-snapshot.md` is now a frozen
historical artifact.

Total stage-11 Codex cost: **11 rounds** (6 per-stage +
5 post-merge); **26 distinct findings closed** (9 BLOCK +
17 MINOR + 0 NIT). Stage-11 lands between stage-9
(12 rounds / 51 findings) and stage-10 (9 rounds / 29
findings) in the convergence band.

Last updated: 2026-05-05 (stage 11 closed and merged).

For top-level project status, see `../status.md`. For
project-wide invariants, see `../README.md`. For stage-11
invariants (workflow, principles, read order, Codex prompt
template), see `memory.md`. For the working spec, see
`spec-snapshot.md`. For inbound forward refs, see
`expectations.md`. For the audit trail, see
`discussions.md` (lazy; created when the first decision
lands). For the round-by-round Codex audit trail, see
`review-log.md` (lazy; created at first round).

## 0. Inbound expectations checklist

From `expectations.md`. Status flags:
- `[ ]` -- in scope; will close as a stage-11 D-tag locks
- `[OUT-13]` -- out of stage-11 scope; deferred to stage 13
  per the categorical D-11-K decision
- `[x]` -- closed by a locked stage-11 D-tag

### From stage 1

- [ ] **Define `LocalId` via `newtype_index!`.** Mirrors
  stage-10's `BlockId` introduction. Will close under
  D-11-4 (typed-index + `RETURN_PLACE = 0` const-body
  declaration using stage-10's macro arm).
- [ ] **Expand `Body` placeholder.** Move out of
  `placeholders.rs` into its own module. Will close
  under D-11-1 (Body field set).
- [ ] **Expand `FnSig` placeholder.** Stage-1 inbound
  framed FnSig as a structural field of Body; the
  evidence-driven D-11-3 candidate alt (e) reframes
  this as **FnSig on Item**, not on Body
  (3-peer convergence: rustc-internal,
  rustc_public, Charon). Will close under D-11-3 with
  the reframe; the operational consequence is that
  stage 12 (Items) absorbs the FnSig field via
  `Item.kind = Function { fn_sig, ... }` per stage-9
  §9.4 + stage-12 expansion. Outbound expectation will
  be filed in `stage-12/expectations.md` "From stage 11
  (per D-11-3 alt e)".
- [ ] **Resolve the `BodyId == ItemId` open question.**
  Will close under D-11-5 with candidate alt (c):
  bodies live in `Item.body: Option<Body>`, no separate
  BodyId. 3-peer convergence (rustc, rustc_public,
  Charon).
- [OUT-13] **`_N` sigil rendering.** Stage-1 inbound
  flagged this as deferred to "stage 10/11"; per D-11-K
  categorical deferral, the `_N` sigil for `LocalId`
  rendering moves to stage 13 alongside all other
  body-level renderers and sigils. Stage 11 owns
  `LocalId` typed-index introduction (D-11-4); stage
  13 owns the `_N` rendering convention.
- [ ] **Niche optimization for `Option<LocalId>`.**
  Will close under D-11-4 (4-byte size assertions
  appended to `crates/regseq-model/src/lib.rs`
  alongside `BlockId` from stage 10). Spec-side
  pin only at stage 11; impl lands at end-of-phase-1
  sweep.
- [ ] **Construction invariant 1 extends.** When `Body`
  and `FnSig` carry IDs, stage 11 extends
  `RegseqModel::validate`'s invariant-1 walk to those
  fields. Will close under D-11-6.1 (fold into
  visitor's `visit_local_decl` + `visit_ty` hooks +
  Item-side fields when stage 12 adds them).

### Bridge concerns (from stage 1)

- [ ] **`tcx.optimized_mir(def_id)` consumption.** The
  bridge fetches optimized MIR per the cleanup-pass
  whitelist applied by `regseq-driver/src/callbacks.rs`.
  Spec-side: the whitelist is documented in stage-1
  §3.3 + stage-10 D-10-4. Implicitly addressed; no
  D-tag needed (bridge mechanics, not spec content).
- [ ] **Function-signature instances vs items.** Per
  D-11-3 alt (e), every `InstanceKind::Function` has a
  signature on the parent Item, not on the Body. Stage
  11 records the reframe; stage 12 owns Item-side
  population. Will close jointly with D-11-3 + outbound
  expectation to stage 12.

### From stage 3

- [ ] **Implement body-level dyn-Trait dispatch
  rewriting pass** (D5). Will close under D-11-7
  (post-construction pass per hybrid (c) timing;
  multi-statement rewrite shape forced by stage-5
  D-5-1 lacking `PtrMetadata` PlaceElem; Charon
  `transform_dyn_trait_calls` and mrustc
  `MIR_Cleanup_Virtualize` peer precedents).

### From stage 5

- [ ] **Implement Place validation walk** (D-5-2). Will
  close under D-11-6 (fold into joint visitor's
  `visit_place` hook applying §19.7's 5 rule
  categories: type-coherence walk, head-local
  existence, FieldIdx range, VariantIdx range,
  structural invariants).
- [OUT-13] **Fill `pretty.rs::render_place(&Place,
  &Body)` stub** (D-5-K). Per D-11-K categorical
  deferral, this renderer moves to stage 13 alongside
  all other body-level renderers. Stage-5 D-5-K's
  rationale (per-variant dependencies on Body
  materialisation + LocalId rendering) still holds;
  the dependencies are simply both stage-13-resident
  rather than mixed stage-11/stage-13. Outbound
  expectation will be filed in `stage-13/expectations.md`
  "From stage 11 (per D-11-K)" with attribution to
  stage 5.

### From stage 6

- [OUT-13] **Fill `pretty.rs::render_operand(&Operand,
  &Body)` stub** (D-6-K). Per D-11-K categorical
  deferral. Outbound to `stage-13/expectations.md`
  with attribution to stage 6.

### From stage 7

- [OUT-13] **Fill `pretty.rs::render_rvalue(&Rvalue,
  &Body)` stub** (D-7-K). Per D-11-K categorical
  deferral. Outbound to `stage-13/expectations.md`
  with attribution to stage 7.

### From stage 8

- [OUT-13] **`render_statement` deferral** (D-8-K). Per
  D-11-K categorical deferral. Outbound to
  `stage-13/expectations.md` with attribution to stage
  8.
- [ ] **Statement walk implementation** (D-8-4 Part C).
  Will close under D-11-6 (fold into joint visitor's
  `visit_statement` hook applying §22.5's 5 positive
  invariants B-1 through B-5 plus inherited B-0a/b/c
  from §19.7 / §21.5 / §20.5).
- [ ] **Named-local preservation** (D-8-3). Will close
  under D-11-2 (4-field LocalDecl candidate including
  `debug_name: Option<StringId>` populated from
  rustc's body-level `var_debug_info` at construction
  time). Charon precedent positive (`Local.name:
  Option<Symbol>`); rustc_public + mrustc keep the
  mapping body-level (deliberate divergence justified
  by C-shape ergonomics: per-local lookup at render
  time + codegen identifier emission without
  secondary scan).

### From stage 9

- [OUT-13] **`render_terminator` deferral + 4
  sub-renderers** (D-9-K: `render_terminator`,
  `render_call_callee`, `render_switch_targets`,
  `render_assert_kind`). Per D-11-K categorical
  deferral. Outbound to `stage-13/expectations.md`
  with attribution to stage 9.
- [ ] **Terminator walk implementation** (D-9-10 Part
  C). Will close under D-11-6 (fold into joint
  visitor's `visit_terminator` hook applying §23.6's
  9 positive invariants B-0a/c inherited + B-1
  through B-7).
- [ ] **Body-level vtable-indirect rewrite pass**
  (D-9-3 (b) shape commitment + stage-3 D5 closure).
  Will close under D-11-7 (post-construction pass;
  hybrid (c) timing). The 5-fresh-locals
  multi-statement rewrite shape stage-9 expectations
  specifies is FORCED by stage-5 D-5-1 lacking
  `PtrMetadata` PlaceElem; spec records this
  cross-stage friction explicitly.
- [ ] **Drop-as-Call bridge concerns: fresh LocalIds**
  (D-9-2). Will close under D-11-7 (Drop-as-Call at
  bridge construction time; allocates `_t_drop_arg`
  + `_t_drop_ret` per Drop site) plus D-11-8
  (LocalDecls allocation API).
- [ ] **Closes construction invariant 9** (D-9-10
  B-7). Will close under D-11-6.1 (fold into
  visitor's `visit_terminator` hook -- every
  `Direct(InstanceId)` resolves in `model.instances`).

### From stage 10

- [OUT-13] **`render_basic_block` deferral** (D-10-K).
  Per D-11-K categorical deferral. Outbound to
  `stage-13/expectations.md` with attribution to
  stage 10.
- [ ] **Body walk implementation per §24.6.3 / §24.4.3**
  (D-10-5 Part C). Will close under D-11-6 (joint
  visitor's body-level traversal applying the 6-step
  DFS-RPO procedure: B-NonEmpty pre-check,
  B-Successors-Valid pre-check, DFS-RPO traversal,
  B-Reachable assertion, B-RPO assertion, B-Entry
  implicit).
- [OUT-13] **`BlockId` sigil-rendering helper**
  (D-10-K.2; `bbN` convention pinned at §24.3.4).
  Per D-11-K categorical deferral. Outbound to
  `stage-13/expectations.md` with attribution to
  stage 10.
- [ ] **Body-level basic-block table integration**
  (D-10-2). Will close under D-11-1 (Body's
  `basic_blocks: IndexVec<BlockId, BasicBlock>`
  field).
- [ ] **`LocalId::RETURN_PLACE` migration** (D-10-3.2
  deferral). Will close under D-11-4 (uses stage-10's
  const-body macro arm extension; symmetric with
  `BlockId::START_BLOCK = 0`).

## 1. D-tag allocation

D-tags allocated upfront from grounded rustc + prior-art
evidence (per `../README.md` §5.1 step 5). Sub-decisions
allocated during Phase A as evidence demands.

| Tag       | Status     | Candidate content (one-line summary)                                                                                                                                                                       |
|-----------|------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| D-11-1    | [LOCKED]  | Body field set (~6 fields candidate: `basic_blocks`, `local_decls`, `arg_count`, `span` + open: `Locals` substructure (Charon precedent) + Body-vs-Item linkage shape).                                    |
| D-11-2    | [LOCKED]  | LocalDecl 4-field candidate `{ty: TyId, mutability: Mutability, span: Span, debug_name: Option<StringId>}`. Open: per-local span (Charon drops it). D-8-3 lift positive precedent (Charon); 2 silent.    |
| D-11-3    | [LOCKED]  | FnSig on Item per alt (e); 3-peer convergence (rustc-internal, rustc_public, Charon). Reframes stage-1 inbound. Outbound to stage 12 for Item-side absorption.                                               |
| D-11-4    | [LOCKED]  | LocalId via `newtype_index!` + `const RETURN_PLACE = 0`; size/niche assertions; symmetric with stage-10 BlockId; absorbs stage-10 D-10-3.2 deferred migration.                                              |
| D-11-5    | [LOCKED]  | Body↔Item linkage: alt (c) -- bodies in `Item.body: Option<Body>`, no BodyId. 3-peer convergence (rustc, rustc_public, Charon).                                                                              |
| D-11-6    | [LOCKED]  | Joint validation walk: visitor with override hooks per alt (b); `rustc_public::MirVisitor` template + Charon bi-modal `VisitAst`/`VisitBody` precedent; mrustc direct-loop negative precedent.            |
| D-11-6.1  | [LOCKED]  | Construction invariants 1 + 9 fold into visitor hooks per alt (a); no separate pre-pass; `visit_local_decl` + `visit_ty` for inv 1; `visit_terminator::Call` for inv 9.                                  |
| D-11-7    | [LOCKED]  | Hybrid (c) timing: Drop-as-Call at bridge construction time (stage-9 §23.4 step 4) + vtable-rewrite as post-construction pass (Charon `transform_dyn_trait_calls` + mrustc `MIR_Cleanup_Virtualize`).         |
| D-11-8    | [LOCKED]  | LocalDecls allocation API: lean alt (a) method-on-Body / Locals; Charon `Locals::new_var` + `BodyTransformCtx` trait precedent; mrustc Builder/Mutator split as alternative.                              |
| D-11-K    | [LOCKED]  | Categorical rendering deferral to stage 13 -- retargets D-5-K through D-10-K + stage-1 `_N` sigil + stage-10 `bbN` sigil. Stage 13 owns body-level rendering as a coherent surface.                       |

Locked-tag count: 8 main + 1 sub-decision + 1 K-tag = 10
locks-in-conversation. `[LOCKED]` flag denotes "locked
during Phase A; awaiting Phase B audit-shape commit + per-
stage Codex review APPROVE before flipping to final
`[LOCKED]`." Status flips to `[LOCKED]` at Phase B close;
flips to `[REOPENED]` only if a Phase B Codex review
round surfaces a substantive issue requiring decision
revision (rare; stage-3 R5 had 1 such case across the
project history).

Phase B work to do:
- Audit-shape entries (Decision / Reasoning / Methodology
  rules / Prior-art evidence / Worked example) for all 10
  locks land in `stage-11/discussions.md`.
- Spec content lands as new sections in
  `stage-11/spec-snapshot.md` (with in-place updates to
  existing sections per `../README.md` §5.9 -- e.g., §22
  / §23 / §24 forward-references to body-content tags get
  refined; §18.2 per-method ownership table gets the
  body-level renderer rows REMOVED per D-11-K).
- Codex per-stage review loop runs against the cluster
  (template in `memory.md` §4); rounds R1, R2, ... append
  to `stage-11/review-log.md` with verdict + issue counts
  + corrections.

## 2. Active cluster

**Phase A complete.** Single combined cluster covered all
10 D-tags in conversation (mirrors stages 6-10 single-
cluster closures). Final Phase A discussion order: D-11-1
-> D-11-2 -> D-11-3 -> D-11-4 -> D-11-5 -> D-11-6 +
D-11-6.1 -> D-11-7 -> D-11-8 -> D-11-K.

Two substantive Phase A discussion threads worth
recording for Phase B audit-shape capture:

- **D-11-3 reframe.** Stage-1 inbound framed FnSig as a
  structural field of Body. 4-peer evidence (rustc-internal,
  rustc_public, Charon, mrustc) converged on FnSig living
  on the Item, not on the Body. Phase A locked alt (e):
  Body has no fn_sig field; signature lives on
  `Item.kind = Function { fn_sig, ... }`; argument types
  reachable through `Body.local_decls[1..=arg_count]`,
  return type via `Body.local_decls[RETURN_PLACE]`.
  Outbound expectation filed for stage 12 to absorb the
  schema. Stage-1 inbound's Body-side framing is
  superseded.

- **D-9-3 reopening question.** Mid-Phase-A discussion
  raised whether stage-9 D-9-3 should be reopened to
  alt (a) (keep Direct(InstanceId) for virtual calls;
  let stage-14 codegen dispatch on `flavor: Virtual`
  via existing `virtual_idx` payload) to remove the
  stage-11 vtable-rewrite pass entirely. Concluded with
  "don't reopen": (i) the cross-stage friction with
  stage-5 D-5-1 (no `PtrMetadata` PlaceElem) forces
  multi-statement rewrite shape regardless of which
  stage owns the rewrite -- not a stage-9-specific
  cost; (ii) stage-9's IR-uniformity preference (one
  Call code path post-rewrite) is a reasonable design
  choice with 12-rounds-of-Codex-review pedigree; (iii)
  reopening cost (Phase A re-do, merger contract
  re-open, ~10-15 stage-9 findings re-confirm) is high
  for a stylistic preference shift. Stage-11 absorbs
  the rewrite-pass cost; D-11-7 hybrid (c) timing locks.

**Phase B active (next).** Spec commit + Codex review.

## 3. Codex review cadence (status)

**Phase B not yet active.** Per-stage review trajectory
TBD; rounds R1, R2, ... will be appended here as they
run, recording verdict + issue counts + close-out
status. Codex prompt template is in `memory.md` §4.

## 4. Open phase set-up recap

For audit / methodology trace:

- [x] Stage-11 folder bootstrapped (already existed with
  `expectations.md` from earlier-stage forward refs;
  no creation needed).
- [x] `spec-snapshot.md` copied from top-level
  `../spec.md` (10463 lines) as Phase A working
  starting point.
- [x] `expectations.md` read as entry checklist
  (sizable bundle covering stages 1, 3, 5, 6, 7, 8, 9,
  10 + bridge concerns; rendering items flagged
  scope-deferred-to-stage-13 per categorical D-11-K).
- [x] Rustc kickoff evidence pass completed:
  - `rustc_middle/src/mir/mod.rs:208-335`
    (`Body<'tcx>` 17-field enumeration with per-field
    rationale; 11 fields drop under RegseqModel
    binding constraints).
  - `rustc_middle/src/mir/mod.rs:951-1054`
    (`LocalDecl<'tcx>` 5-field + `LocalInfo` 9-variant
    enumeration).
  - `rustc_middle/src/mir/mod.rs:264, 1235-1257`
    (`var_debug_info` body-level mapping; named-local
    preservation source).
  - `rustc_middle/src/mir/mod.rs:842-850`
    (`newtype_index!` Local with `RETURN_PLACE = 0`
    const-body grammar).
  - `rustc_type_ir/src/ty_kind.rs:728-744` (FnSig
    structure: `inputs_and_output`, `c_variadic`,
    `safety`, `abi`).
  - `rustc_codegen_ssa/src/mir/mod.rs:1-310` (canonical
    body walk: `codegen_mir` + `FunctionCx` setup +
    `traversal_order` + `arg_local_refs` +
    `vars_and_temps_iter` + per-block dispatch loop +
    `per_local_var_debug_info` partition precedent).
  - `rustc_codegen_cranelift/src/base.rs:33-321`
    (cleaner body walk: storage-order
    `iter_enumerated()` loop with inline per-statement
    / per-terminator dispatch).
  - `rustc_codegen_ssa/src/mir/locals.rs:15-38`
    (`Locals<V>` per-local sidecar pattern).
  - `rustc_codegen_ssa/src/mir/analyze.rs:17-56`
    (pre-walk classification precedent: `non_ssa_locals`).
  - `rustc_public/src/mir/visit.rs:42-160`
    (`MirVisitor` template).
- [x] D-tags allocated (10 total: 8 main + 1
  sub-decision + D-11-K).
- [x] Per-stage prior-art research landed in 4
  primary-source files (full per-D-tag synthesis +
  per-project takeaways):
  - `prior-art/rustc.md` (~5800 words; peer-compiler-
    design lens).
  - `prior-art/rustc_public.md` (~3000 words; D-11-3
    alt (e) + D-11-5 alt (c) + D-11-6 visitor template
    load-bearing).
  - `prior-art/charon.md` (~5400 words; D-11-7 +
    D-11-8 + Locals substructure + cross-stage
    friction load-bearing).
  - `prior-art/mrustc.md` (~3500 words; D-11-7
    `MIR_Cleanup_Virtualize` + Builder/Mutator split
    + D-11-2 negative precedent load-bearing).
- [x] `memory.md` drafted (workflow, principles, read
  order, Codex template) from grounded knowledge.
- [x] `status.md` drafted (this file) from grounded
  knowledge.

## 5. Next concrete unit of work

**Phase B: spec commit + Codex review.**

Three sub-units of Phase B work, sequenced:

1. **Audit-shape entries in `stage-11/discussions.md`.**
   For each of the 10 locked D-tags, write a per-decision
   entry with the audit shape (Decision / Reasoning /
   Methodology rules invoked / Prior-art evidence /
   Worked example). Estimated 500-1500 lines total
   (50-150 lines per D-tag depending on substantive
   content; D-11-3 + D-11-7 are the largest; D-11-4 +
   D-11-K are the smallest).

2. **Spec content in `stage-11/spec-snapshot.md`.** Add
   new sections covering the locked vocabulary:
   - `Body` 4-field struct + layout convention + rejection
     table for ~14 dropped rustc fields.
   - `LocalDecl` 4-field struct + rejection table for 3
     dropped rustc fields.
   - FnSig location reframe (Body has no fn_sig;
     signature on Item).
   - `LocalId` typed-index + `RETURN_PLACE = 0` const +
     niche/size assertions.
   - Body-vs-Item linkage (no BodyId; bodies in Item).
   - Joint validation walk (visitor trait shape with 10
     hook pairs; D-11-6.1 invariant fold-in; §24.4.3
     procedure absorption).
   - Body-level transformation passes (Drop-as-Call
     bridge collapse rules referencing stage-9 §23.4 +
     vtable-indirect rewrite procedure referencing
     stage-9 §23.3 + cross-stage friction note with
     stage-5 D-5-1 + pass scheduling).
   - LocalDecls allocation API (`push_local` +
     `push_synthetic_local`).
   - In-place updates: §22 / §23 / §24 forward references
     refined; §18.2 per-method ownership table -- body-
     level renderer rows REMOVED per D-11-K.

3. **Codex per-stage review loop** with template from
   `memory.md` §4. Rounds R1, R2, ... iterate until
   APPROVE. Each round records in
   `stage-11/review-log.md` with verdict + issue counts
   + corrections. Stage-9 / stage-10 trajectory gives a
   benchmark: 5-12 rounds typical for this size of
   cluster.

After Phase B APPROVES: stage-11 merger contract per
README §5.5 7-step protocol (filed in §8 outbound bundles
+ task #5 cross-stage rendering migration).

## 6. Blockers

None. All upstream dependencies are pinned:

- Stage 1 (foundation crate; ID conventions; cleanup-pass
  whitelist) -- closed.
- Stage 3 (type system; `Ty::Dynamic.vtable_ty`) -- closed.
- Stage 5 (Place + projection; D-5-1 6-variant `PlaceElem`
  vocabulary; D-5-2 validation rule statements) -- closed.
- Stage 6 (Operand) -- closed.
- Stage 7 (Rvalue) -- closed.
- Stage 8 (Statement; D-8-3 named-local preservation
  outbound; D-8-4 invariants) -- closed.
- Stage 9 (Terminator; D-9-2 Drop-as-Call rules;
  D-9-3 (b) Call shape; D-9-10 invariants;
  `InstanceKind::Function.flavor` + `virtual_idx` schema
  extension at §9.4) -- closed.
- Stage 10 (BasicBlock + body-level table; D-10-2
  storage; D-10-4 RPO ordering; §24.4.3 DFS-RPO
  procedure) -- closed.

## 7. Deferred items (stage-11-specific)

To be populated as Phase A surfaces them. Stage-specific
deferrals live here; cross-stage deferrals live in
`../status.md` "Deferred items (cross-stage)".

(none yet)

## 8. Outbound expectations to file at Phase B close

To file at Phase B close (after per-stage Codex APPROVES,
before merger contract). Targets:

### To `stage-12/expectations.md` "From stage 11"

Predicted entries based on D-tag candidates:

1. **FnSig migration to Item.kind=Function** (per D-11-3
   alt (e)). Stage 12 absorbs `fn_sig: FnSig` as a
   structural field of `Item.kind = Function { fn_sig,
   ... }`. The `InstanceKind::Function` schema at
   stage-9 §9.4 already carries `flavor` + `virtual_idx`;
   stage 12 extends with `fn_sig` from the post-stage-11
   reframe.
2. **Body materialisation under Item** (per D-11-5
   alt (c)). Stage 12's `Item` struct gets `body:
   Option<Body>` field. The `Option` accommodates
   foreign declarations (no body) + items the
   monomorphization collector references but does not
   emit (e.g., uninstantiated generics; not actually
   reachable post-mono but spec-side wording).
3. **Construction invariant 9 closure** (D-9-10 B-7
   close-out). Stage 11's visitor checks
   `Direct(InstanceId)` membership in
   `model.instances`; stage 12 owns `model.instances`
   integrity (one-to-one with Items; populated by the
   collector; exhausted by the validator).

### To `stage-13/expectations.md` "From stage 11 (per D-11-K)"

Categorical retargeting bundle (operational consequence
of D-11-K). Lifts the rendering items out of
`stage-11/expectations.md` into a new stage-13 inbound,
preserving attribution to predecessor stages:

1. **`render_place(&Place, &Body)`** (originally D-5-K).
2. **`render_operand(&Operand, &Body)`** (originally
   D-6-K).
3. **`render_rvalue(&Rvalue, &Body)`** (originally
   D-7-K).
4. **`render_statement(&Statement, &Body)`** (originally
   D-8-K).
5. **`render_terminator(&Terminator, &Body)` + 3
   public sub-renderers + 1 internal helper**
   (originally D-9-K).
6. **`render_basic_block(&BasicBlock, &Body)`**
   (originally D-10-K).
7. **`_N` sigil for `LocalId`** (originally stage-1
   inbound).
8. **`bbN` sigil for `BlockId`** (originally
   stage-10 D-10-K.2; convention pinned at §24.3.4).
9. **§18.2 per-method ownership table consolidation**:
   stage 13 absorbs all body-level renderer rows that
   currently target stage 11.

Plus the existing stage-13 D14 deferral (Rust-source-style
lean for `render_ty` per stage-3 D14) -- no change.

### Other potentially affected stages

- **Stage 14 backends**: no new outbound from stage 11.
  The `Body` -> C function lowering is well-defined
  by stage-11's data-structure spec + stage-12's Item
  + stage-9's Call shape; stage 14 already has
  sufficient input.

Filing happens at Phase B close (after per-stage Codex
APPROVES); not yet filed.

## 9. How to update this file

- **Status line at top:** updated at each phase
  transition (Phase 0 complete -> Phase A active ->
  Phase B active cluster N -> stage closed).
- **D-tag allocation table:** updated when a tag
  transitions [LOCKED] -> [LOCKED] / [DEFERRED] /
  [REOPENED]. Sub-tags appended as they are allocated
  during Work.
- **Active cluster:** narrowed as Phase A progresses.
- **Codex review cadence:** updated per round with
  verdict + issue counts.
- **Inbound expectations checklist:** marked `[x]` as
  D-tags close inbound items; `[OUT-13]` items retain
  their flag (closed by D-11-K categorical defer + the
  Phase B close outbound migration).
- **Deferred items:** appended as Phase A surfaces them.
- **Outbound expectations:** moved to "filed" once
  written to the relevant later-stage's
  `expectations.md`.
