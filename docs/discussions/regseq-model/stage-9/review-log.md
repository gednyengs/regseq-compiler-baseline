# Stage 9 -- Review Log

Round-by-round Codex review trail for stage-9
`Terminator` vocabulary spec. Each round records the
prompt scope, verdict, issue list, and corrections
applied between rounds.

Per-round file naming: `/tmp/codex_d9_review_r<N>.txt`.

Tier discipline (per `../README.md` §5.6):
- **Tier 1 -- per-stage cluster review.** Stage-9 is a
  single combined cluster (no parallelism); all Phase B
  rounds in this file are tier 1. Iterate to
  `Lockability: yes`.
- **Tier 2 -- stage-internal cross-session.** N/A
  (single combined cluster; no sessions).
- **Tier 3 -- top-level post-merge review.** Runs after
  the merger contract integrates §23 into top-level
  `spec.md`. Recorded in top-level `review-log.md` under
  `## Stage 9` post-merger close.

---

## Round 1 -- 2026-05-05 (in flight)

**Scope.** Initial adversarial review of the freshly-
drafted §23 Terminator vocabulary + §18.2 ownership-
table updates + discussions.md decision records, for
all 11 D-tags + 5 sub-decisions LOCKED at Phase A
close.

**Prompt template.** Per `memory.md` §4 with
stage-9-specific evaluation criteria (10 numbered
items: soundness, citations, cross-stage consistency,
methodology rule application, prior-art accuracy,
internal cross-refs, inbound expectation closure,
bridge translation completeness, invariants registry
coherence, worked examples).

**Output capture.** `/tmp/codex_d9_review_r1.txt`.

**Verdict.** **BLOCK** (3 BLOCK + 6 MINOR + 1 NIT =
10 issues).

**Issues:**

1. [BLOCK] **VIRTUAL-BRIDGE-CONTRADICTION** -- §23.7
   bridge row for rustc `InstanceKind::Virtual(...)`
   said "passes through as Direct(<vtable shim
   InstanceId>) or rejects" -- both branches
   inconsistent with the stage-11 rewrite-pass
   contract.
2. [BLOCK] **DROP-PRESTATEMENT-ORDER** -- §23.4 said
   the bridge "prepends" the synthetic Assign;
   semantically wrong if earlier statements mutate
   the dropped place.
3. [BLOCK] **CLOSURE-CALLEE-GAP** -- B-5 claimed
   closures pre-monomorphized to fn-pointer types
   "per stage-7 D-7-1.6"; D-7-1.6 is the CastKind/
   PointerCoercion decision (not closure-call
   handling). Closures appear as Direct calls to
   synthesized call methods, not at Indirect.
4. [MINOR] **RENDERER-HANDOFF-MISMATCH** -- D-9-K
   listed 5 renderers including
   `render_terminator_kind`; §23.9 / §18.2 / stage-11
   expectations listed only 4.
5. [MINOR] **SPAN-STAGE1-MISMATCH** -- §23.1.1 said
   `Span` uses "SpanId interned indirection under the
   hood"; stage-1 spec defines Span as inline.
6. [MINOR] **UNWIND-CITATION-DRIFT** --
   `abort_unwinding_calls.rs:47` is `target_supports_unwinding`,
   not the rewrite site (lines 116-122).
7. [MINOR] **DEFPATHDATA-CLOSURE-OVERCLAIM** -- D-9-3
   closure of "Possibly extend DefPathData" was
   over-broad; stage-12's vtable-Item DefPathData
   encoding is separate.
8. [MINOR] **STAGE6-SPAN-EXPECTATION-NOT-CLOSED-AS-STATED**
   -- D-9-4 supersedes the stage-6 inbound rather than
   satisfying it as written.
9. [MINOR] **RUSTC-DROP-PRECEDENT-OVERSTATED** -- §23.4
   should acknowledge rustc's two extra branches (ZST
   elision; dyn-Trait dynamic-drop path).
10. [NIT] **RVALUE-RENDERING-WORDING** -- §23.9
    incorrectly listed Rvalue as embedded in Terminator.

**Corrections applied (R1 -> R2):**

- **BLOCK 1.** §23.7 Virtual row rewritten: bridge
  emits `Direct(<id>)` where the resolved Regseq
  Instance has `InstanceKind::Function { flavor:
  FunctionInstanceFlavor::Virtual, .. }` (per top-level
  `spec.md` §9.4; the `flavor` field is canonical in
  the current spec, post-stage-1 revision; cited at
  R2-time after R2 surfaced the stage-1-snapshot
  citation drift). Stage-11 walks every
  `Direct(InstanceId)`, checks `Instance.kind.flavor`,
  and rewrites to `Indirect(Operand)` for
  `flavor=Virtual`. No "rejects" alternative. Updated
  discussions.md D-9-3 bridge translation table
  similarly. Updated D-9-3 reasoning #2 to remove the
  now-incorrect "stage-1 doesn't expose the
  discriminator" claim and replace with the
  function-pointer-call-gap argument.
- **BLOCK 2.** §23.4 step 4 rewritten: bridge
  **appends** the synthetic Assign as the last
  statement immediately before the synthetic Call
  terminator (after all earlier statements that may
  have mutated `place`). Added "Why append, not
  prepend" rationale.
- **BLOCK 3.** §23.6.2 B-5 rewritten: closures are
  not direct callees in `Call.callee` position;
  closure invocation `c(args)` is desugared to a
  Direct call to the closure's `call_once` /
  `call_mut` / `call` method (synthesised Function
  Item per stage-12 §F). Closure value appears as
  the first argument (receiver), not as the callee.
  Removed the incorrect "stage-7 D-7-1.6" citation.
- **MINOR 4.** Demoted `render_terminator_kind` to
  a private dispatch helper; updated discussions.md
  D-9-K to list 4 public renderers + 1 internal
  helper. Updated stage-11 expectations to match.
- **MINOR 5.** §23.1.1 wrap text updated: "the field
  is an inline `Span` (per top-level spec.md §5.3,
  which carries Span as a plain inline struct, not
  via a SpanId interner -- spans are not
  deduplicated)."
- **MINOR 6.** All `abort_unwinding_calls.rs:47`
  citations updated to `:115-121` (the actual
  rewrite site under
  `!call_can_unwind || !target_supports_unwinding`)
  + `:137-138` (dead-block removal).
- **MINOR 7.** D-9-3 closure of "Possibly extend
  DefPathData" narrowed to "stage-9's call-callee
  shape only"; explicit note that stage-12's vtable-
  Item DefPathData encoding choice remains a stage-12
  decision. Updated stage-12 expectations cross-ref.
- **MINOR 8.** D-9-4 closure language changed from
  "closes" to "supersedes"; the stage-6 inbound
  listed (a) and (b); D-9-4 introduced (c) on
  consumer-landscape grounds. Updated discussions.md
  Section 3 and §23.7 bridge translation note.
- **MINOR 9.** §23.4 implementer's references
  expanded to acknowledge rustc's three branches
  (ZST elision at `block.rs:606-613`; dyn-Trait
  dynamic-drop path at `block.rs:624-657`; concrete
  dispatch at `:680-692`) and explicitly note
  RegseqModel's deliberate divergence on (i) ZST
  elision policy and (ii) `dyn Trait` drop handling.
- **NIT 10.** §23.9 wording corrected: removed
  `Rvalue` from the "embeds Place / Operand /
  Rvalue" list (Terminators do not embed Rvalue).

Also updated §23.6.2 B-3 / B-4 / B-6 to specify
**post-rewrite invariant timing** (the rewrite must
run before validate; pre-rewrite virtual calls have
designed receiver-type mismatch), and added
§23.8.6 worked example pre-rewrite shape (showing
the `Direct(<flavor=Virtual id>)` shape with
designed B-6 violation that the rewrite resolves).

Files touched in R1 -> R2:
- `stage-9/spec-snapshot.md` (§23.1.1, §23.4 step 4
  + implementer's refs, §23.6.2 B-3/B-4/B-5/B-6,
  §23.7 Call rule + args translation note, §23.8.6
  worked example, §23.9 wording, two
  abort_unwinding_calls citations)
- `stage-9/discussions.md` (D-9-3 reasoning #2 +
  bridge translation table + foreclosed-alternative
  + closures-of-inbound; D-9-4 supersession
  language; D-9-K renderer list; Section 3 inbound
  closures)
- `stage-11/expectations.md` (render-pass renderer
  list 5 -> 4; stub message list)
- `stage-12/expectations.md` (DefPathData closure
  scope clarification)

---

## Round 2 -- 2026-05-05

**Scope.** Verify R1 fixes; surface any new issues
from corrections; check residual issues.

**Output capture.** `/tmp/codex_d9_review_r2.txt`.

**Verdict.** **BLOCK** (3 BLOCK + 1 MINOR + 0 NIT =
4 issues). Lockability: no.

**R1 fix verification.** 8 of 10 RESOLVED; 2 PARTIALLY
RESOLVED:
- R1-1 NOT RESOLVED (citation provenance issue).
- R1-2 PARTIALLY RESOLVED (stage-11 expectations
  drift remained).
- R1-3 RESOLVED.
- R1-4 RESOLVED.
- R1-5 RESOLVED.
- R1-6 RESOLVED.
- R1-7 RESOLVED.
- R1-8 RESOLVED.
- R1-9 PARTIALLY RESOLVED (introduced new
  contradiction with stage-11 rewrite predicate).
- R1-10 RESOLVED.

**New issues:**

1. [BLOCK] **STAGE1-FLAVOR-PROVENANCE** -- citations to
   `stage-1 §9.4 line 1529` referenced the FROZEN
   stage-1 historical snapshot at `stage-1/spec-snapshot.md`,
   which predates the `flavor` field addition. The
   `flavor` field IS in the current authoritative spec
   (top-level `spec.md` §9.4 / stage-9/spec-snapshot.md
   §9.4 lines 1483-1539) but was added by a post-stage-1
   revision; citations should retarget to top-level
   `spec.md` §9.4.
2. [BLOCK] **VTABLE-REWRITE-ARG-MISMATCH** --
   §23.6.2 B-6 + §23.8.6 worked example show post-
   rewrite args[0] = data pointer extracted from fat
   pointer; but stage-11/expectations.md said "Args,
   destination, target unchanged" -- inconsistent.
   Stage-11 rewrite must rewrite args[0] to extract
   data pointer.
3. [BLOCK] **DYN-DROP-REWRITE-CONTRADICTION** -- §23.4
   said bridge emits `Direct(<concrete vtable
   drop-shim InstanceId>)` for dyn-Trait drops with
   stage-11 rewrite handling them. But stage-12 vtable
   drop shims have `flavor=Item` (per `spec.md` §9.4's
   bridge-synthesised-Function convention); stage-11
   rewrite predicate matches `flavor=Virtual`. Rustc
   emits `ty::InstanceKind::Virtual(drop_fn.def_id(),
   0)` for dynamic drops at `block.rs:624-657`. Spec
   needed to align: bridge emits `flavor=Virtual`
   (vtable index 0) for dyn-Trait drops; rewrite
   handles drop-virtual and method-virtual uniformly.
4. [MINOR] **DROP-PRESTATEMENT-EXPECTATION-DRIFT** --
   stage-11/expectations.md item 1 still said
   "prepended" (R1-2 partial fix); §23.4 was correct.

**Corrections applied (R2 -> R3):**

- **BLOCK 1.** All `stage-1 §9.4 line 1529` citations
  retargeted to "top-level `spec.md` §9.4" (the current
  authoritative reference; the `flavor` field was added
  by a post-stage-1 revision and is canonical in the
  current spec). Updated 4 sites in spec-snapshot.md
  + 2 sites in discussions.md + 1 site in
  stage-12/expectations.md.
- **BLOCK 2.** stage-11/expectations.md vtable rewrite
  rewritten: 4-step pass now (i) extracts metadata
  via PtrMetadata, (ii) loads function pointer via
  Deref + Field(method_idx), (iii) extracts data
  pointer via Field(0, *const ()), (iv) rewrites BOTH
  callee AND args[0] (was only callee). Added
  post-rewrite invariant timing note.
- **BLOCK 3.** §23.4 step 1 split into concrete vs
  dyn-Trait drop handling: concrete uses
  `flavor=DropGlue`; dyn-Trait uses `flavor=Virtual`
  (mirroring rustc's
  `InstanceKind::Virtual(drop_fn.def_id(), 0)`).
  Removed the "concrete vtable drop-shim InstanceId"
  claim. Updated implementer's references to clarify
  rustc-mirrored resolution. Stage-11 rewrite now
  uniformly handles `flavor=Virtual` for both
  index-0 drop-slot and index-N method-slot virtual
  calls.
- **MINOR 4.** stage-11/expectations.md "Pre-Call
  `&raw mut place` Statement" item 1 fixed:
  "prepended" -> "appended as the last statement"
  with cross-ref to §23.4 step 4 rationale.

Files touched in R2 -> R3:
- `stage-9/spec-snapshot.md` (§23.4 step 1 + §23.4
  implementer's refs item 2; §23.6.2 B-5 closure
  citation; §23.7 Virtual row citation)
- `stage-9/discussions.md` (D-9-3 reasoning #2 +
  bridge translation table)
- `stage-11/expectations.md` (vtable-indirect rewrite
  pass 4-step rewrite; Drop "prepended" -> "appended")
- `stage-12/expectations.md` (DefPathData closure
  flavor citation)

---

## Round 3 -- 2026-05-05

**Scope.** Verify R2 fixes; surface any new issues
from corrections; check residual issues.

**Output capture.** `/tmp/codex_d9_review_r3.txt`.

**Verdict.** **BLOCK** (3 BLOCK + 1 MINOR + 1 NIT =
5 issues). Lockability: no.

**R2 fix verification.** 1 RESOLVED; 3 PARTIALLY
RESOLVED:
- R2-1 PARTIALLY (operative spec retargeted; stale
  audit-log citation in review-log.md remained as
  NIT).
- R2-2 PARTIALLY (rewrite extracts data pointer; but
  `_t_data` typing not in vocabulary).
- R2-3 PARTIALLY (concrete-vs-dyn-Trait drop split
  done; vtable index recovery underspecified).
- R2-4 RESOLVED.

**New issues:**

1. [BLOCK] **VIRTUAL-INDEX-RECOVERY-GAP** -- the
   spec claims the dyn-drop vtable index is
   "preserved through the bridge translation" but
   the model shape stores no such field;
   `FunctionInstanceFlavor::Virtual` has no payload;
   `fn_shim_disambig` ranks all Virtuals to 5.
2. [BLOCK] **FAT-DATA-EXTRACTION-NOT-IN-VOCAB** --
   stage-11/expectations.md vtable rewrite uses
   `_t_data = (args[0]).0` with `Field(0, *const ())`
   projection; locked §19.4 PlaceElem typing only
   admits struct/union/tuple/closure/enum predecessors,
   not Ty::Ref/Ty::Ptr fat pointers or Ty::Dynamic.
   Need a legal data-extraction primitive.
3. [BLOCK] **BARE-DYNAMIC-RECEIVER-TYPE** -- stage-11
   said args[0] has type `Ty::Dynamic { ... }`, but
   model invariant 12 says Ty::Dynamic may only
   appear as pointee of Ty::Ref or Ty::Ptr.
4. [MINOR] **DROP-LOCALDECL-TY-RAWPTR** -- stage-11
   said `Ty::RawPtr(Mut, place_ty)`, but the variant
   is `Ty::Ptr(TyId, Mutability)`.
5. [NIT] **STALE-STAGE1-FLAVOR-CITATION** --
   review-log.md R1 corrections still said "per
   stage-1 §9.4 line 1529."

**Corrections applied (R3 -> R4):**

- **BLOCK 1.** Extended `InstanceKind::Function`
  variant with `virtual_idx: Option<u32>` field
  (`Some(idx)` exactly when `flavor=Virtual`; `None`
  otherwise). Bridge populates from rustc's
  `InstanceKind::Virtual(_, idx)`. Field added by
  stage-9 D-9-3 cross-stage revision (anticipated by
  the stage-1 inbound expectation procedure).
  Schema note added to §9.4 documenting the addition
  and the stage-9 trigger.
- **BLOCK 2.** Stage-11 vtable rewrite now uses
  `Rvalue::Cast(CastKind::PtrToPtr, Operand::Copy(args[0]),
  Ty::Ptr(Ty::Tuple([]), <mut>))` for data-pointer
  extraction (legal per stage-7 §21 CastKind
  vocabulary; preserves §19.4 / §19.7 invariants).
  Replaces the prior illegal `(args[0]).0`
  field-projection on a fat pointer.
- **BLOCK 3.** Stage-11 receiver-type wording
  corrected: receiver type is `Ty::Ref(dyn_ty, _)` or
  `Ty::Ptr(dyn_ty, _)` where
  `dyn_ty: Ty::Dynamic { ... }`. Bare Ty::Dynamic
  reference removed.
- **MINOR 4.** Stage-11 LocalDecls fixed:
  `Ty::RawPtr(Mut, place_ty)` -> `Ty::Ptr(place_ty,
  Mutability::Mut)`. Added cross-ref note that the Ty
  enum has `Ptr(TyId, Mutability)` not `RawPtr`.
- **NIT 5.** review-log.md R1 corrections updated to
  remove the stale `stage-1 §9.4 line 1529` reference
  with explanatory note that R2 surfaced the citation
  drift.

Files touched in R3 -> R4:
- `stage-9/spec-snapshot.md` (§9.4 Function variant
  schema extension; §23.4 step 1 dyn-drop reference
  to virtual_idx)
- `stage-11/expectations.md` (vtable-indirect rewrite
  step 1 receiver-type fix + virtual_idx recovery;
  step 2 Cast-based data extraction; step 4 LocalDecl
  type list updated; Drop LocalDecls fix RawPtr->Ptr)
- `stage-9/review-log.md` (R1 correction citation
  fix)

---

## Round 4 -- 2026-05-05

**Scope.** Verify R3 fixes; surface any new issues;
check lockability.

**Output capture.** `/tmp/codex_d9_review_r4.txt`.

**Verdict.** **BLOCK** (3 BLOCK + 1 MINOR + 0 NIT =
4 issues). Lockability: no.

**R3 fix verification.** 3 RESOLVED; 2 PARTIALLY:
- R3-1 PARTIALLY (added in snapshot; top-level spec.md
  not yet updated; D-9-3 record stale).
- R3-2 PARTIALLY (PtrToPtr in vocab; Ty::Ref source
  typing not yet supported).
- R3-3 RESOLVED.
- R3-4 RESOLVED.
- R3-5 RESOLVED.

**New issues:**

1. [BLOCK] **TOPLEVEL-SPEC-VIRTUAL-IDX-MISMATCH** --
   stage-11 references "per spec.md §9.4" for
   virtual_idx but top-level spec.md hasn't been
   updated by merger yet.
2. [BLOCK] **VIRTUAL-IDX-VALIDATION-GAP** -- §3.3
   validation doesn't check the `virtual_idx ↔
   flavor=Virtual` invariant.
3. [BLOCK] **PTRTOPTR-REF-SOURCE-TYPING-GAP** --
   `CastKind::PtrToPtr` requires raw-pointer source
   per stage-7 §21 (mirroring rustc's
   `validate.rs:1382-1392`); spec used it with
   `Ty::Ref` source.
4. [MINOR] **D9-3-DECISION-RECORD-STALE** --
   discussions.md D-9-3 still describes the
   pre-extension shape.

**Corrections applied (R4 -> R5):**

- **BLOCK 1.** All "per spec.md §9.4" references in
  stage-11/expectations.md retargeted to "per
  stage-9/spec-snapshot.md §9.4 (which post-merger
  lands in top-level spec.md §9.4)." Documents the
  pre-merger working state explicitly.
- **BLOCK 2.** Added §9.4 construction invariant text:
  `virtual_idx.is_some()` iff `flavor == Virtual`;
  validated by `RegseqModel::validate`. Mirror
  invariant added at §23.6.2 as B-8 (with walk-
  enforcement deferred to stage 11). Updated
  walk-pass deferral note at §23.6.3 to mention B-8
  in the B-1-B-8 list.
- **BLOCK 3.** Stage-11 vtable rewrite step 2 split
  into "source normalization" + "PtrToPtr cast":
  for `Ty::Ref(dyn_ty, _)` receivers (method-virtual
  case), insert intermediate `_t_raw =
  Rvalue::RawPtr(Mutability::Not / Mut, Place {
  local: <args[0]'s local>, projection: [Deref] })`
  -- legal per stage-7 §21.2; produces `Ty::Ptr(dyn_ty,
  _)`. For `Ty::Ptr(dyn_ty, _)` receivers (drop
  case), skip. Then `Cast(PtrToPtr, _t_raw_or_args0,
  Ty::Ptr(unit, mut))`. Both source and destination
  are `Ty::Ptr` per the locked stage-7 PtrToPtr typing
  rule.
- **MINOR 4.** discussions.md D-9-3 Decision text
  updated: notes the schema-extension requirement
  (`virtual_idx: Option<u32>` field on Function)
  surfaced post-locking via R3 review; describes the
  shape as `CallCallee::Indirect` plus the
  `Function.virtual_idx` field. Updated bridge
  translation table + reasoning #2 similarly.

Files touched in R4 -> R5:
- `stage-9/spec-snapshot.md` (§9.4 construction
  invariant; §23.6.2 B-8; §23.6.3 walk-pass list)
- `stage-9/discussions.md` (D-9-3 Decision shape +
  bridge translation table + reasoning #2)
- `stage-11/expectations.md` (vtable rewrite step 2
  source-normalization + PtrToPtr split; "per spec.md
  §9.4" -> "per stage-9/spec-snapshot.md §9.4")

---

## Round 5 -- 2026-05-05

**Scope.** Verify R4 fixes; surface any new issues;
check lockability.

**Output capture.** `/tmp/codex_d9_review_r5.txt`.

**Verdict.** **BLOCK** (2 BLOCK + 0 MINOR + 0 NIT =
2 issues). Lockability: no. Issue count converging
(R1: 10, R2: 4, R3: 5, R4: 4, R5: 2).

**R4 fix verification.** 2 RESOLVED; 2 PARTIALLY:
- R4-1 PARTIALLY (stage-11 + discussions retargeted;
  stage-9 spec-snapshot.md §23.7 row still cited
  top-level spec.md and omitted virtual_idx).
- R4-2 RESOLVED.
- R4-3 PARTIALLY (Deref-on-Ty::Ref typing OK;
  RawPtr+Cast+PtrToPtr all admissible; but rewrite
  text used ill-formed Operand/Place syntax).
- R4-4 RESOLVED.

**New issues:**

1. [BLOCK] **VTABLE-REWRITE-OPERAND-PLACE-MISMATCH**
   -- `Operand::Copy(args[0])` is wrong: args[0] is
   already an Operand, but Operand::Copy takes a
   Place. The rewrite must materialize args[0] to a
   fresh local first, then use that local consistently.
2. [BLOCK] **TOPLEVEL-SPEC-VIRTUAL-IDX-STALENESS** --
   stage-9 spec-snapshot.md §23.7 Virtual row still
   cited top-level spec.md §9.4 and omitted the
   `virtual_idx: Some(idx)` part of the Instance
   shape.

**Corrections applied (R5 -> R6):**

- **BLOCK 1.** Stage-11 vtable rewrite restructured
  with explicit step 2a: receiver materialization.
  The pass now: (a) matches args[0] = Copy(p) /
  Move(p) / Constant(c); (b) inserts `_t_recv =
  Rvalue::Use(args[0].clone())` to create a fresh
  receiver local; (c) uses `_t_recv` consistently in
  PtrMetadata (`Operand::Copy(Place::from(_t_recv))`)
  and RawPtr (`Place { local: _t_recv, projection:
  [Deref] }`). All Operand/Place types now match
  stage-9 §20.1 / §19.x typing. The fresh
  `_t_recv` LocalId is added to the LocalDecls list.
- **BLOCK 2.** §23.7 Virtual row updated: cite
  "**stage-9/spec-snapshot.md** §9.4 (post-merger
  lands in top-level spec.md §9.4)"; include
  `virtual_idx: Some(idx)` in the Instance shape;
  add cross-ref to §9.4 construction invariant +
  §23.6.2 B-8.

Files touched in R5 -> R6:
- `stage-9/spec-snapshot.md` (§23.7 Virtual row
  retargeted + virtual_idx field added)
- `stage-11/expectations.md` (vtable rewrite step 2
  receiver-materialization + Operand/Place syntax
  fixes; LocalDecls list updated)

---

## Round 6 -- 2026-05-05 -- **LOCKED**

**Scope.** Verify R5 fixes; surface any new issues;
check lockability.

**Output capture.** `/tmp/codex_d9_review_r6.txt`.

**Verdict.** **MINOR** (0 BLOCK + 0 MINOR + 1 NIT =
1 issue). **Lockability: yes.** Per stage-5 / stage-6 /
stage-7 / stage-8 precedent for MINOR-with-NIT-only-
and-Lockability:yes verdicts, the spec is locked at
this round.

Issue trajectory: R1=10, R2=4, R3=5, R4=4, R5=2,
R6=1. Six rounds; converged.

**R5 fix verification.** 2 RESOLVED (both):
- R5-1 RESOLVED. Stage-11 receiver materialization
  + Operand/Place syntax cleanup verified.
- R5-2 RESOLVED. §23.7 Virtual row + virtual_idx
  consistency verified.

**Issues found:**

1. [NIT] **RAW-DROP-RECEIVER-MUTABILITY-TYPO** --
   Post-rewrite invariant timing note in stage-11
   expectations said pre-rewrite raw dyn receivers
   are `*const dyn Trait`, but dyn-drop receivers are
   synthesized `&raw mut place` (i.e. `*mut dyn Trait`)
   per §23.4. The receiver mutability differs between
   method-virtual (`&dyn Trait` / `&mut dyn Trait`) and
   dyn-drop (`*mut dyn Trait`) cases.

**Corrections applied (R6 -- post-lock NIT cleanup):**

- **NIT 1.** Stage-11 post-rewrite-invariant-timing
  note reorganized to enumerate the two cases
  separately: method-virtual (args[0] is
  `Ty::Ref(dyn_ty, _)`) vs dyn-drop (args[0] is the
  synthetic `Ty::Ptr(dyn_ty, Mutability::Mut)` from
  §23.4 step 4). Each case lists its pre-rewrite
  args[0] type and the corresponding callee
  fn_sig.inputs()[0] mismatch. No spec-content
  change; clarification only.

Files touched in R6 NIT cleanup:
- `stage-11/expectations.md` (post-rewrite-invariant
  timing note method-virtual / dyn-drop split)

**Locked at R6** 2026-05-05.

---

## Lock summary

Stage-9 Phase B closed at R6 MINOR with Lockability:
yes 2026-05-05. Total Codex cost: 6 rounds; 26 distinct
findings closed (5 BLOCK from R1 + 3 BLOCK from R2 +
3 BLOCK from R3 + 3 BLOCK from R4 + 2 BLOCK from R5 +
the cumulative MINORs and the 1 NIT from R6 = 26).

Substantive trajectory:
- **R1 BLOCK** caught the
  VIRTUAL-BRIDGE-CONTRADICTION (the §23.7 row's
  "rejects pending rewrite" branch was incoherent with
  stage-11 ownership), the DROP-PRESTATEMENT-ORDER
  (prepends -> appends), and the CLOSURE-CALLEE-GAP
  (closures don't appear at Indirect; they're
  desugared to Direct call-method calls).
- **R2 BLOCK** surfaced the schema citation drift
  (stage-1/spec-snapshot.md frozen vs current top-
  level spec.md), the args[0] rewrite contradiction
  (stage-11 said "args unchanged" but B-6 required
  args[0] rewritten), and the dyn-drop rewrite
  contradiction (concrete shim vs Virtual flavor).
- **R3 BLOCK** surfaced the virtual_idx storage gap
  (FunctionInstanceFlavor::Virtual carried no
  payload; the index needed a place in the model),
  the fat-data-extraction-not-in-vocab issue
  (`Field(0, *const ())` violated PlaceElem typing;
  required `Cast(PtrToPtr)` instead), and the bare
  Ty::Dynamic receiver type (model invariant 12
  requires Ty::Dynamic only as Ty::Ref/Ptr pointee).
- **R4 BLOCK** surfaced the top-level-spec staleness
  (cite the stage-9 working copy until merger), the
  validation gap (need `virtual_idx ↔ flavor=Virtual`
  biconditional invariant), and the PtrToPtr-Ref-
  source-typing-gap (Cast requires Ty::Ptr source;
  introduce intermediate &raw const *args[0] step).
- **R5 BLOCK** surfaced the Operand/Place syntax
  mismatch (`Operand::Copy(args[0])` is wrong because
  args[0] is already an Operand; need a fresh
  `_t_recv` local) and the §23.7 row staleness.
- **R6 NIT** surfaced the dyn-drop receiver
  mutability typo.

Most load-bearing finding across the trajectory:
**R3's VIRTUAL-INDEX-RECOVERY-GAP** -- the
realization that `FunctionInstanceFlavor::Virtual`
without a payload couldn't preserve the vtable index,
forcing the `virtual_idx: Option<u32>` schema
extension on `InstanceKind::Function`. This was the
load-bearing schema decision triggered by D-9-3 (b);
without it, the bridge couldn't preserve enough info
for stage-11 to perform the rewrite correctly.

The stage-1 inbound expectation "Possibly extend
`DefPathData` for synthetic vtable scopes" was the
methodology pattern that anticipated this kind of
cross-stage schema addition; the `virtual_idx`
extension follows the same procedural pattern.
