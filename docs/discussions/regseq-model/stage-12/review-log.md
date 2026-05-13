# Stage 12 -- Review Log

Round-by-round Codex audit trail for stage-12 Phase B
per-stage review (tier 1 per README §5.6).

Each round entry records: prompt date, prompt summary,
verdict, issue count breakdown, list of substantive
findings, corrections applied + spec-snapshot.md sections
edited. Stage-12 prompt template is in `memory.md` §4.

---

## R1 -- 2026-05-06

**Prompt summary.** Adversarial review of stage-12
Item-level spec, decisions D-12-1 through D-12-K, full
cluster review. Focus: 7 main D-tags + 29 sub-decisions
locked at Phase A close. Spec under review:
`stage-12/spec-snapshot.md` (11661 lines after Phase B
edits). Out-of-scope: stage-11 spec-snapshot.md (frozen);
body-level rendering decisions per D-11-K + D-12-K;
synthesis machinery already pinned at stage 3 / 4.

12 evaluation criteria covering: internal consistency of
stage-12 contributions; Option B (D-12-1.4) soundness;
Static.mutability placement cross-stage coherence;
Item-level typed fields shape; construction-invariant
closure algorithms; bridge construction-pipeline order;
lang-item recognition; render_def_path stub-fill; D-12-K
rendering deferral; D-12-1.8 Instance-side helpers;
stage-11 reopening process flagging; spec text quality.

**Verdict.** BLOCK (R1 complete 2026-05-06).

**Findings.** 11 issues (6 BLOCK + 5 MINOR + 0 NIT).
All findings were stale Option-A residues from the
spec-content edits where some sections still framed
fn_sig + body / mutability as Item-side. Specifically:

1. [BLOCK] stale-item-fn-sig (line 269): §3 root struct
   `sigs` comment stale -- said FnSigInterner referenced
   from Item-side schema. Fix: rewrote to per-Instance
   schema with note about generic mono producing distinct
   FnSigIds per Instance.
2. [BLOCK] stale-body-placement (line 294, 403-405): §3
   "bodies live structurally inside Items" + invariant 1
   stale wording. Fix: removed Item-side wording; pointed
   to `InstanceKind::Function.body` per Option B.
3. [BLOCK] validation-function-shape (line 735): §3.3
   step 8 validated `Function { flavor, virtual_idx }`
   (old shape). Fix: updated to validate
   `Function { fn_sig, body, flavor, virtual_idx }` with
   per-field validation rules including the body=None
   foreign-decl correlation per D-12-2.1.
4. [BLOCK] static-mutability-stale-validation (line
   958-997, 836-846, 2666): VTableInstance and
   ConstMemoryStatic Static-shape invariants still
   checked `InstanceKind::Static { mutability:
   Mutability::Not, ... }`. Fix: split into Item-side
   mutability check (`ItemKind::Static { mutability:
   Mutability::Not }`) + Instance-side ty/init check
   (`InstanceKind::Static { ty, init: Some(_) }` per
   2-field schema).
5. [BLOCK] phase2-vtables-itemid (line 1049): §3.4
   Phase 2 cache claimed `RegseqModel.vtables`
   (PrincipalTraitKey -> ItemId) cache during translation,
   but final ItemIds aren't assigned until Phase 4.
   Fix: distinguished Phase-2 working cache (keyed by
   stable Phase-2-available handle: DefPathHash /
   placeholder) from final-form `RegseqModel.vtables`
   populated in Phase 5 after final ItemIds exist.
6. [MINOR] helper-count-mismatch (line 1167-1197): 13
   helpers listed but text said "12-helper inventory".
   Fix: updated count to 13 in spec-snapshot.md and
   status.md.
7. [BLOCK] stale-section25-item-side (line 10449-10517):
   §25 intro + §25.1 still said function signature
   "lives Item-side"; bodies "live as a structural field
   of Item"; "back-pointer". Fix: updated all three to
   Option B Instance-side framing.
8. [BLOCK] stale-section25-crossrefs (line 11142,
   11183-11240): §25.9.2 B-Body-2 cited
   `Item.kind = Function { fn_sig }`; §25.9.4 cited
   "Item-side InstanceKey"; §25.10 C-lowering example
   cited `Item.kind=Function.fn_sig`; §25.11 outbound
   cited "FnSig migration to Item.kind = Function";
   "Body materialization under Item". Fix: updated all
   to Instance-side under Option B; reframed §25.11
   outbound as "post-Option-B" with explicit list of
   stage-11 spec edits at merger.
9. [MINOR] stale-callee-example (line 9415-9419, 11103):
   Two callee_fn_sig example notes said "through the
   parent Item's `fn_sig` field". Fix: replaced with
   `inst.kind.as_function().fn_sig` per §23.6.2 helper
   under Option B.
10. [MINOR] d12k-stub-attribution (line 5768-5770):
    §18.2 said all stubs use "deferred per D-11-K";
    Item-level stubs should use "deferred per D-12-K".
    Fix: split stub-message convention into body-level
    (D-11-K) and Item-level (D-12-K) families; noted
    `render_def_path` is filled at stage 12 (D-12-6),
    not a stub.
11. [MINOR] four-peer-overclaim (line 10730-10738):
    §25.5 claimed "all four peers carry bodies
    per-Instance" but prior-art evidence didn't directly
    support that exact claim for all four. Fix: narrowed
    to "two peers (rustc-internal + rustc_public)
    directly establish the per-Instance body pattern;
    Charon's --monomorphize mode is supportive but
    doesn't carry the same Item/Instance split; mrustc
    is informational". Cited specific prior-art line
    references for each.

**Corrections applied.** All 11 fixes landed in
`stage-12/spec-snapshot.md` (now 11796 lines, +135 net
new lines after corrections). Helper-count fix also
applied to `stage-12/status.md`. No discussions.md edits
required (audit-shape already correct; corrections were
spec-text consistency issues, not decision changes).

**Spec sections edited:**
- §3 root struct (sigs comment + bodies comment).
- §3.1 invariant 1 (BodyId clarification reword).
- §3.3 Phase B step 8 (Function instance validation).
- §3.3 Phase B step 9 ConstMemoryStatic Static-shape check.
- §3.4 Phase 2 cache discipline (Phase2Handle -> ItemId
  distinction).
- §9.4 Phase B continued: VTableInstance + ConstMemoryStatic
  invariants split into Item-side + Instance-side checks.
- §13.3 ConstMemoryStatic comment (Item-side mutability).
- §13 ConstMemoryStatic synthesis description.
- §18.2 stub-message convention (split body-level vs
  Item-level attribution).
- §23.6 + §23.8 callee_fn_sig example notes.
- §25 intro + §25.1 (Option B framing).
- §25.9.2 B-Body-2 + §25.9.4 + §25.10 C-lowering example.
- §25.11 outbound expectations (post-Option-B reframe).

**Next round.** R2 to verify R1 corrections + check for
any new issues introduced. Continue until APPROVE.

---

## R2 -- 2026-05-06

**Prompt summary.** R1 fix verification + new-issue scan
+ cross-stage residual stale check. Same prompt template
+ same out-of-scope as R1, with explicit R1 fix summary
included for context.

**Verdict.** BLOCK (R2 complete 2026-05-06).

**Findings.** 5 issues (4 BLOCK + 1 MINOR + 0 NIT).
Residual stale wording I missed in R1.

1. [BLOCK] r1-2-stale-body-comment (line 298 -- §3 root
   struct comment): stale leading sentence "Bodies live
   structurally inside Items per stage-11 D-11-5" still
   present even though the rest of the comment described
   Option B. Fix: removed the stale sentence; cleaned up
   the comment to lead with "No separate BodyId. Per
   stage-12 D-12-1.4 (Option B; supersedes stage-11
   D-11-5's Item-side framing), bodies live as a
   structural field on InstanceKind::Function..."
2. [BLOCK] residual-item-side-body-wording (line 1797 --
   §9.2 Instance struct paragraph): said "parent Function
   Item's `body: None` per stage-11 D-11-5; bodies live
   Item-side structurally; no separate `bodies` table."
   Contradicted §9.4. Fix: rewrote to "Per stage-12
   D-12-1.4 (Option B; supersedes stage-11 D-11-5), any
   body lives on the Function Instance's
   InstanceKind::Function { body: Option<Body>, ... }, NOT
   on the Item; the metadata-only `core` lang-item case
   has body: None on the corresponding Instance."
3. [BLOCK] phase2-cache-final-side-table-drift (line
   2679 -- ConstMemoryStatic synthesis description): the
   lazy synthesis rule still did Phase-2 lookup in final
   `item_synthetic_kinds` and created the Instance during
   Phase 2, contradicting §3.4's pipeline-order contract.
   Fix: split the synthesis rule into Phase-2 (working
   cache `ContentHash -> Phase2Handle`; register synthetic
   Item shape with synthesised DefPathHash as Phase-2
   stable key), Phase-4 (post-F4-sort ItemId assignment +
   side-table population), Phase-5 (Instance construction
   with InstanceKind::Static { ty, init }).
4. [BLOCK] peer-evidence-citation-does-not-verify (line
   10827 -- §25.5 4-peer evidence): the corrected
   rustc_public claim cited `prior-art/rustc_public.md:14`
   but that line actually said bodies via CrateItem
   methods, not Instance::body. Also discussions.md still
   said "In all four peers" overclaim. Fix: (a) added
   "Per-Instance body access" paragraph to
   stage-12/prior-art/rustc_public.md citing
   `rustc_public/src/mir/mono.rs:53-55` as primary source
   for Instance::body() -> Option<Body>; (b) updated
   spec-snapshot.md citation to point at the new
   prior-art paragraph + primary source line range; (c)
   updated discussions.md D-12-1.4 (lines 414-428 + lines
   1390-1408) to narrow "all four peers" to "two
   directly-establishing peers (rustc-internal +
   rustc_public) + Charon supportive + mrustc
   informational only", matching the corrected §25.5
   claim.
5. [MINOR] r1-1-distinct-fnsigids-overclaim (line 277 --
   §3 root struct sigs comment): R1's corrected wording
   said "two monomorphizations of a generic fn produce
   two distinct Instances with distinct FnSigIds";
   FnSigInterner content-keys identical (inputs, output,
   abi, c_variadic, safety) tuples to share one FnSigId.
   Fix: rewrote to "Different substituted signatures get
   distinct FnSigIds; identical substituted signatures
   share one FnSigId via FnSigInterner's content-keyed
   identity rule per §14.4."

Plus one supplementary cleanup (not a Codex finding but
discovered during R2 fix application):
- §23.2.3 Return terminator description still said
  "via the parent Function Item's signature
  `model.sigs[item.kind.as_function().fn_sig].output()`";
  fixed to "`model.sigs[inst.kind.as_function().fn_sig].output()`"
  per Option B.

**Corrections applied.** All 5 fixes landed. Edits across
spec-snapshot.md (now 11820 lines, +24 net new lines vs
post-R1), prior-art/rustc_public.md (added per-Instance
body paragraph), discussions.md (lines 414-428 + lines
1390-1408 narrowed). No status.md edits this round.

**Spec sections edited:**
- §3 root struct (sigs comment + bodies comment;
  R2 #1, #5).
- §9.2 Instance struct paragraph (R2 #2).
- §13.3 ConstMemoryStatic synthesis description: 4-step
  rule expanded to 6 steps spanning Phase 2 / Phase 4 /
  Phase 5 (R2 #3).
- §23.2.3 Return terminator (supplementary cleanup).
- §25.5 Body-vs-Instance linkage 4-peer claim narrowed
  with primary-source citations (R2 #4).

**Discussions.md sections edited:**
- D-12-1.4 audit-shape "Reasoning" subsection (R2 #4).
- §2 Phase A discussion threads "Option B reframe"
  4-peer summary (R2 #4).

**Prior-art file edited:**
- stage-12/prior-art/rustc_public.md: new "Per-Instance
  body access" paragraph (R2 #4).

**Next round.** R3 to verify R2 corrections + check for
any further residual stale. Continue until APPROVE.

---

## R3 -- 2026-05-06

**Prompt summary.** R2 fix verification + new-issue scan
+ deeper cross-stage residual stale check. Scope explicit
on additional sweep targets: §13 closure / vtable /
ConstMemoryStatic synthesis descriptions; §9 sub-sections
beyond §9.2 / §9.4; §3.1 invariants beyond what was
already touched.

**Verdict.** BLOCK (R3 complete 2026-05-06).

**Findings.** 3 issues (2 BLOCK + 1 MINOR + 0 NIT). Fix
trajectory tightening: R1 had 11 issues, R2 had 5, R3
has 3. R2 fixes #1, #2, #4, #5 verified clean.

1. [BLOCK] phase2-cache-decision-record-drift
   (discussions.md:1138 -- D-12-5.2 audit-shape "Cache
   discipline" subsection): said Phase 2 cache is
   "e.g., RegseqModel.vtables for vtable types"
   (final-form cache). spec-snapshot.md §3.4 has the
   corrected Phase2Handle reframe, but discussions.md
   D-12-5.2 audit-shape didn't get the same treatment
   in R2. Fix: rewrote D-12-5.2 "Cache discipline"
   subsection to specify family-specific Phase-2 working
   caches keyed by stable Phase-2-available handles
   (PrincipalTraitKey -> Phase2Handle for vtables;
   closure_did -> Phase2Handle for closures; ContentHash
   -> Phase2Handle for ConstMemoryStatic);
   RegseqModel.vtables populated in Phase 5.
2. [BLOCK] residual-fnsig-on-function-item
   (spec-snapshot.md:3804 -- §13.1 closure family
   description): "Each Function Item's signature is
   substituted from the closure's
   closure_sig_as_fn_ptr_ty... carried in
   `model.sigs[id]`." Stale Option-A wording. Fix:
   reframed as "the corresponding Function Instance's
   `InstanceKind::Function { fn_sig, ... }` (per Option
   B; §9.4) carries the substituted signature... resolved
   through `model.sigs[fn_sig_id]`."
3. [MINOR] const-memory-step-numbering
   (spec-snapshot.md:2734 -- §13.3 ConstMemoryStatic
   synthesis 6-step list): R2 #3 introduced numbering
   drift -- after the new Phase 5 step `6.`, the
   dispatch-result step was numbered `5.`. Fix:
   renumbered to `7.` and clarified the dispatch result
   is post-Phase-5.

R2 fix-verification status (per Codex's own
verification):
- R2 #1 verified at spec-snapshot.md:299.
- R2 #2 verified at spec-snapshot.md:1799.
- R2 #4 verified across prior-art/rustc_public.md:16 +
  spec-snapshot.md:10831 + discussions.md:414.
- R2 #5 verified at spec-snapshot.md:276.
- R2 #3 was the only fix that had residual issues:
  the spec-snapshot.md edit was clean but discussions.md
  D-12-5.2 audit-shape didn't get the same treatment
  (R3 #1) and the 6-step list had a numbering drift
  (R3 #3).

**Corrections applied.** All 3 fixes landed.
spec-snapshot.md grew slightly (~12 lines net); D-12-5.2
audit-shape rewrote.

**Spec sections edited:**
- §13.1 closure call-method Function Item description
  (R3 #2).
- §13.3 ConstMemoryStatic synthesis step renumbering
  (R3 #3).

**Discussions.md sections edited:**
- D-12-5.2 audit-shape "Cache discipline" subsection
  expanded with family-specific Phase-2 working cache
  details (R3 #1).

**Next round.** R4 to verify R3 corrections + final
sweep. Stage-9 / stage-10 / stage-11 trajectory
benchmark suggests we're approaching APPROVE.

---

## R4 -- 2026-05-06

**Prompt summary.** R3 fix verification + new-issue scan
+ deeper sweep for R3-leftover stale Item-side wording.

**Verdict.** BLOCK (R4 complete 2026-05-06).

**Findings.** 6 issues (3 BLOCK + 3 MINOR + 0 NIT). All
R3 fixes verified by Codex.

1. [BLOCK] phase-a-body-on-item-residue
   (spec-snapshot.md:644 -- §3.3 Phase A step 2):
   "Bodies live structurally inside Items per stage-11
   D-11-5; no separate `bodies` table." Stale Option-A
   wording in deserialise Phase A description. Fix:
   reframed to "Bodies live structurally inside Function
   Instances on `InstanceKind::Function { body:
   Option<Body>, ... }` per stage-12 D-12-1.4 / Option B".
2. [BLOCK] basic-block-section-body-on-item-residue
   (spec-snapshot.md:9830 -- §24.2.1): "Bodies live
   Item-side structurally per stage-11 D-11-5; no
   separate `model.bodies` table." Stale Option-A
   wording in stage-10 IndexVec convention note. Fix:
   reframed to Instance-side Option B framing.
3. [BLOCK] drop-glue-function-shape-stale
   (spec-snapshot.md:11070 -- §25.7.1 Drop-as-Call
   collapse): drop-glue Instance shown as
   `InstanceKind::Function { flavor, virtual_idx }`
   (3-field, old shape) instead of 4-field with fn_sig
   + body. Fix: expanded to 4-field shape; documented
   that fn_sig is the substituted drop-glue signature
   and body is the synthesized drop-in-place body.
4. [MINOR] vtable-rewrite-phase-drift
   (spec-snapshot.md:11089 -- §25.7.2 vtable-indirect
   rewrite): said bridge-time rewrite would require
   Item table populated DURING body construction, but
   §3.4 has Items in Phase 4 and Bodies in Phase 5
   (separated). Fix: reframed to "intra-Phase-5 pass
   over Bodies after they are materialized but before
   the joint validation walk (§25.6 sub-step)";
   clarified that bridge-time (Phase 2) rewrite would
   couple Item synthesis with body translation,
   contradicting §3.4's Phase 2/4/5 separation.
5. [MINOR] stale-stage-header (spec-snapshot.md:13 --
   document header "Currently reflects" list): missing
   stage 12 in the reflected-stage list. Fix: appended
   "stage 12 (Rust Items; in-progress Phase B per
   stage-12/spec-snapshot.md -- working copy; merge into
   top-level spec.md happens at stage-12 merger contract
   close per README §5.5)".
6. [MINOR] non-ascii-text (spec-snapshot.md:3 + 898 +
   1032): Codex flagged em dashes ("--") and arrows
   ("->") as non-ASCII. **Disposition: not fixed.**
   Per CLAUDE.md global rule, the "NO UNICODE SYMBOLS"
   directive targets emojis and decorative symbols
   (the explicit examples cited are checkmark, X-mark,
   warning-sign, wrench). Em dashes and arrows are
   typographic/punctuation characters, not symbols, and
   are pervasive throughout the entire merged spec.md
   from all earlier stages (1, 3, 4, 5, 6, 7, 8, 9, 10,
   11). Converting only stage-12 contributions to
   strict ASCII would create cross-section inconsistency
   with the rest of the spec; the right disposition is
   either (a) project-wide convention cleanup at top-
   level merger time touching all stages, or (b) accept
   the existing project convention (em dashes / arrows
   in prose are fine; only emojis / decorative symbols
   are excluded). Stage-12 follows option (b). Recorded
   here as a deferred project-wide convention question.

**Corrections applied.** Fixes for R4 #1-5 landed.
spec-snapshot.md grew slightly (~25 lines net).
discussions.md unchanged this round.

**Spec sections edited:**
- Document header (R4 #5).
- §3.3 Phase A step 2 deserialise wording (R4 #1).
- §24.2.1 IndexVec convention note (R4 #2).
- §25.7.1 drop-glue Instance shape (R4 #3).
- §25.7.2 vtable-rewrite phase scheduling (R4 #4).

**R3 fix verification status (per Codex):**
- R3 #1 verified (D-12-5.2 family-specific Phase-2
  caches; RegseqModel.vtables in Phase 5).
- R3 #2 verified (§13.1 closure call signatures route
  through Function Instance).
- R3 #3 verified (ConstMemoryStatic step renumbering
  4-5-6 -> 4-5-6-7 with Phase 4 / Phase 5 / dispatch
  result).

**Next round.** R5. Trajectory: R1=11 issues, R2=5,
R3=3, R4=6 (3 BLOCK + 3 MINOR; 1 deferred). The R4
finding count reset upward because R4 explicitly
expanded the sweep scope ("deeper residual stale
sweep"); the BLOCK count is decreasing absolute
(R1=6, R2=4, R3=2, R4=3). Approaching APPROVE.

---

## R5 -- 2026-05-06

**Prompt summary.** R4 fix verification + final residual
stale sweep + R4 #6 (em-dash/arrow) explicit out-of-scope
disposition.

**Verdict.** BLOCK (R5 complete 2026-05-06).

**Findings.** 1 issue (1 BLOCK + 0 MINOR + 0 NIT). All
R4 fixes verified.

1. [BLOCK] drop-as-call-phase-drift
   (spec-snapshot.md:1064 + discussions.md:1108 -- §3.4
   Phase 2 synthesis trigger table): listed "Drop
   terminator -> Drop-as-Call collapse" as a Phase 2
   Item-synthesis trigger, but Drop-as-Call collapse
   rewrites Body content (insert Statement + Call inside
   a Body) -- a Phase 5 body-construction step, not a
   Phase 2 Item synthesis. Conflicts with R4 #4 fix
   (§25.7.2 intra-Phase-5 timing) and §3.4 Phase 5
   (Function Instances + bodies populated). Fix:
   reclassified the Drop-related Phase-2 trigger as
   "drop-glue Function Item synthesis" (the Item that
   the Phase-5 Drop-as-Call rewrite's Call terminator
   targets); added a "Per-phase body work in Phase 5"
   note to §3.4 Phase 5 explicitly sequencing
   (a) body materialisation, (b) Drop-as-Call collapse
   per §25.7.1, (c) vtable-indirect rewrite per §25.7.2,
   (d) joint validation walk per §25.6. Same reframe
   landed in discussions.md D-12-5.1 5-phase summary.

**R4 fix verification status (per Codex):**
- R4 #1 verified at spec-snapshot.md:645.
- R4 #2 verified at spec-snapshot.md:9830.
- R4 #3 verified at spec-snapshot.md:11077 (4-field
  drop-glue Function shape).
- R4 #4 partially verified at spec-snapshot.md:11103
  (intra-Phase-5 vtable rewrite) -- "partially" because
  the §3.4 Phase 2 table still had the conflicting
  Drop-as-Call entry (R5 #1).
- R4 #5 verified at spec-snapshot.md:13.
- Final residual sweep: no remaining 3-field
  `Function { flavor, virtual_idx }`; no remaining
  Item-side body / fn_sig wording; Static schema is
  2-field Instance-side with mutability on
  ItemKind::Static; no actual emojis / decorative
  symbols (R4 #6 disposition accepted).

**Corrections applied.** Single fix landed.
spec-snapshot.md grew slightly (~30 lines net for the
new Phase-5 body-work note); discussions.md D-12-5.1
5-phase summary updated for cross-stage consistency.

**Spec sections edited:**
- §3.4 Phase 2 synthesis-trigger table (R5 #1
  reclassification).
- §3.4 Phase 5 added "Per-phase body work in Phase 5"
  subsection sequencing the body-content rewrites.

**Discussions.md sections edited:**
- D-12-5.1 audit-shape 5-phase summary (R5 #1
  cross-stage consistency).

**Trajectory.** R1=11, R2=5, R3=3, R4=6 (3B+3M+1
deferred), R5=1 (1B). BLOCK count: R1=6, R2=4, R3=2,
R4=3, R5=1. Cluster is converging tightly. Expect R6 to
either APPROVE or to surface only MINOR / NIT issues.

**Next round.** R6.

---

## R6 -- 2026-05-06

**Prompt summary.** R5 fix verification + close-out
check. Asked Codex to indicate whether MINOR-or-NIT-only
suffices for close-out (per stage-9 / 10 / 11 precedent).

**Verdict.** **MINOR with explicit close-out: YES.**
R5 #1 verified. Codex stated: "Close-out acceptable:
yes. This is MINOR-only and lockable pending top-level
merger contract review."

**Findings.** 1 issue (0 BLOCK + 1 MINOR + 0 NIT).

1. [MINOR] residual-phase-wording (spec-snapshot.md:11088
   + 11092 + 11094 + 11275 + 11296 -- §25.7 + §25.7.1 +
   §25.7.3): said Drop-as-Call runs "at bridge
   construction time" without restating that this
   means Phase 5 body-construction (per §3.4), not
   Phase 2 Item synthesis. §25.7.1 title "Drop-as-Call
   collapse (bridge construction time)" was potentially
   ambiguous. Fix: (a) renamed §25.7.1 to "Drop-as-Call
   collapse (Phase 5 body construction)"; (b) reframed
   §25.7 intro to explicitly say both passes land in
   Phase 5; (c) updated §25.7.3 step 1 label from
   "Bridge construction" to "Phase 5 body materialisation
   / bridge body construction (after Phase 4 ItemId
   assignment)".

**Corrections applied.** Single MINOR fix landed.
spec-snapshot.md grew slightly (~15 lines net).

**Spec sections edited:**
- §25.7 intro (R6 #1; restated hybrid timing as "both
  in Phase 5", with sub-step distinction).
- §25.7.1 title + opening paragraph (R6 #1; explicit
  Phase 5 body-construction labeling + reference to
  Phase 4 ItemId pre-assignment).
- §25.7.3 step 1 + closing note (R6 #1; explicit
  "Phase 5 body materialisation" labeling +
  "All three steps run in Phase 5" closing note).

**R5 fix verification status (per Codex):**
- R5 #1 verified across spec-snapshot.md:1078,
  1080, 1166, 1172, 1183, 1190 + discussions.md:1108,
  1111, 1131. §3.4 Phase 2 trigger table reclassification
  + §3.4 Phase 5 body-work subsection + D-12-5.1 5-phase
  summary all aligned.

**Trajectory.** R1=11, R2=5, R3=3, R4=6, R5=1, R6=1
(MINOR-only). BLOCK count: R1=6, R2=4, R3=2, R4=3, R5=1,
R6=0. Closure verdict: MINOR with Lockability:yes.

**Next round.** R7 to verify the R6 MINOR fix and
formally close at APPROVE if possible. Per stage-9 /
stage-10 / stage-11 precedent, MINOR-with-close-out:yes
at R6 is sufficient to lock the cluster, but R7 verifies
the MINOR fix landed cleanly.

---

## R7 -- 2026-05-06 (FINAL CLOSE-OUT)

**Prompt summary.** R6 fix verification + final
close-out verdict.

**Verdict.** **APPROVE -- Lockability: yes -- 0 issues
(0 BLOCK / 0 MINOR / 0 NIT).**

**R6 fix verification status (per Codex):**
- R6 #1 verified across spec-snapshot.md:11088, 11091,
  11095 (§25.7 intro hybrid-timing reframe), :11104,
  11108 (§25.7.1 title + forced-timing paragraph
  Phase-5 labeling), :11290, 11294, 11317 (§25.7.3
  step 1 + closing note Phase-5 labeling). Cross-section
  consistency verified against §3.4 Phase 4/5
  boundaries.

**No new issues from R6 corrections.**

**Per-stage Codex review CLOSED at R7 APPROVE.**

---

## Final per-stage Codex review trajectory

| Round | Verdict | Issues | BLOCK / MINOR / NIT | Notes |
|-------|---------|--------|---------------------|-------|
| R1    | BLOCK   | 11     | 6 / 5 / 0           | Stale Option-A residues from spec-content edits |
| R2    | BLOCK   | 5      | 4 / 1 / 0           | Residual stale wording in §3 / §9.2 / §13.3 / §25.5 |
| R3    | BLOCK   | 3      | 2 / 1 / 0           | Audit-shape drift + §13.1 closure stale + numbering |
| R4    | BLOCK   | 6      | 3 / 3 / 0 (1 deferred) | Deeper sweep surfaced §3.3 / §24.2.1 / §25.7 stale; ASCII-only deferred |
| R5    | BLOCK   | 1      | 1 / 0 / 0           | §3.4 Phase 2 Drop-as-Call mis-classification |
| R6    | MINOR   | 1      | 0 / 1 / 0           | §25.7 phase-labeling inconsistency; explicit close-out: yes |
| R7    | APPROVE | 0      | 0 / 0 / 0           | All R6 fixes verified; Lockability: yes |

**Total per-stage Codex cost:** 7 rounds; 27 distinct
findings closed (16 BLOCK + 11 MINOR + 0 NIT;
1 deferred per project-wide convention disposition).
Stage-12 lands within the stage-9 / stage-10 / stage-11
trajectory band (5-12 rounds typical).

**Phase B closed 2026-05-06.** Stage-12 is now ready
for the merger contract per README §5.5 7-step protocol.

---

## Post-merge tier-3 R1 -- 2026-05-06

**Prompt summary.** First post-merge tier-3 review per
README §5.6 on the integrated top-level `spec.md`
(11917 lines after stage-12 integration). Scope: stage-12
integration coherence + cross-stage drift from Option B
+ foreign-item dispatch + bridge construction-pipeline +
D-12-1.3 typed fields + D-12-1.5 forward-pointer +
Items-pre-mono / Instances-per-mono invariant + stage-11
spec content coherence after Option B.

**Verdict.** BLOCK (post-merge R1 complete 2026-05-06).

**Findings.** 3 issues (3 BLOCK + 0 MINOR + 0 NIT). All
genuine cross-stage drift surfaced only at the
integrated-spec level (the per-stage R7 APPROVE was
within stage-12 contributions; the integration with
pre-stage-12 sections surfaced these issues).

1. [BLOCK] foreign-none-signal-drift (spec.md:1837 --
   §9.2 Instance struct paragraph): said "metadata-only
   `core` lang-item case has `body: None` on the
   corresponding Instance" but §3.1 invariant 10 +
   §25.5 + D-12-2.1 reserve `body: None` exclusively
   for foreign decls. Drop-glue and similar `core`
   lang-items have rustc-provided bodies via
   `tcx.instance_mir(InstanceKind::DropGlue(...))`,
   so the corresponding Instance has `body: Some(b)`
   at Phase 5 body construction. Fix: rewrote §9.2
   paragraph to clarify the metadata-only framing
   describes the Item's source-level metadata layer
   (no source body in spec crate), NOT the Instance-
   side body slot; Instance body is populated for
   `core` lang-items at Phase 5.
2. [BLOCK] validation-rustc-query-drift (spec.md:753 --
   §3.3 Phase B step 8): validated `body: None` by
   "the parent Item's source DefId satisfies
   `tcx.is_foreign_item(did)`". But validation runs
   model-side per §3.3 trust model + §3.5 BridgeInput
   (which only carries `spec_crate_stable_id`); rustc
   queries are not available at validation time. The
   model-side signal is `Item.linkage ==
   Linkage::Foreign` per D-12-1.3 Linkage enum +
   D-12-2.3 translation table. Fix: rewrote step 8 to
   check `body.is_none() iff item.linkage ==
   Linkage::Foreign` (model-side biconditional). Bridge
   construction uses `tcx.is_foreign_item`; validation
   reads stored `Item.linkage` only. Added analogous
   `init.is_none() iff Linkage::Foreign` check for
   Static Instances per D-12-2.2.
3. [BLOCK] vtable-erased-receiver-mutability (spec.md:9223
   + 9596 + 9611 + 11314 -- §23.6.2 B-6 + §23.8.6
   virtual-dispatch example + §25.7.2 invariant timing):
   said erased receiver type is `*const ()` or
   `*const () / *mut ()` (alternation). Per §13.2
   "Erased receiver type is uniformly `*mut ErasedPointee`,
   regardless of the source borrow flavor" + §17.2.1
   trait-object dispatch carve-out, the erased receiver
   is uniformly `*mut ()` (the `*const ()` variant would
   be unsound when the impl's Self contains UnsafeCell;
   the spec pins mutable-erased uniformly). Fix:
   normalized §23.6.2 B-6 + §23.8.6 example + §25.7.2
   to consistently say `*mut ()` for the erased
   receiver per §13.2 + §17.2.1's uniform rule; cited
   the cross-section reference inline.

**Corrections applied.** All 3 fixes landed in
top-level spec.md (~25 net new lines). discussions.md
unchanged this round (the audit-shape is already
correct; corrections were spec-text consistency
issues introduced at integration time).

**Spec sections edited (post-merge corrections):**
- §9.2 Instance struct paragraph (post-merge R1 #1).
- §3.3 Phase B step 8 Function + Static validation
  rules (post-merge R1 #2).
- §23.6.2 B-6 invariant text (post-merge R1 #3).
- §23.8.6 virtual-dispatch worked example (post-merge
  R1 #3).
- §25.7.2 invariant timing description (post-merge
  R1 #3).

**Next round.** Post-merge R2 to verify these
corrections + check for any further drift.


## Post-merge tier-3 R2 -- 2026-05-06

**Prompt summary.** Post-merge R1 fix verification +
new-issue scan + final cross-stage drift sweep at the
integrated-spec level.

**Verdict.** BLOCK (post-merge R2 complete 2026-05-06).

**Findings.** 1 issue (1 BLOCK + 0 MINOR + 0 NIT).
Post-merge R1 #1 + #2 verified complete; #3 was nearly
complete but had a residual `*const ()` reference in
§23.8.6 worked example's conclusion.

1. [BLOCK] vtable-erased-receiver-mutability-incomplete
   (spec.md:9628 -- §23.8.6 worked example post-rewrite
   B-6 conclusion): said "both `*const ()`" while §13.2
   + §23.6.2 B-6 + §23.8.6 example body all say `*mut ()`.
   Fix: changed "both `*const ()`" to "both `*mut ()` per
   §13.2 + §17.2.1's uniform-mutable-erased-receiver rule".

**Corrections applied.** Single fix landed (~3 lines).

**Spec sections edited:** §23.8.6 worked example
B-6 conclusion (post-merge R2 #1).

**Next round.** Post-merge R3 to verify the final fix
and close the merger contract per README §5.5 step 6.


## Post-merge tier-3 R3 -- 2026-05-06 (FINAL CLOSE-OUT)

**Prompt summary.** Post-merge R2 #1 fix verification +
final close-out check.

**Verdict.** **APPROVE -- Lockability: yes -- 0 issues
(0 BLOCK / 0 MINOR / 0 NIT).**

**Post-merge R2 #1 verification:**
Codex verified the fix is correct and complete at
spec.md:9626 (worked example post-rewrite conclusion now
says "both `*mut ()` per §13.2 + §17.2.1's
uniform-mutable-erased-receiver rule"). Cross-section
consistency confirmed across §13.2 (spec.md:3951), §17.2.1
(spec.md:5680), §23.6.2 B-6 (spec.md:9224), and §23.8.6
(spec.md:9626).

**Merger contract step 6 CLOSED at post-merge R3 APPROVE.**

---

## Final post-merge tier-3 review trajectory

| Round | Verdict | Issues | BLOCK / MINOR / NIT | Notes |
|-------|---------|--------|---------------------|-------|
| post-merge R1 | BLOCK | 3 | 3 / 0 / 0 | Cross-stage drift surfaced at integrated-spec level |
| post-merge R2 | BLOCK | 1 | 1 / 0 / 0 | Residual `*const ()` in §23.8.6 worked example |
| post-merge R3 | APPROVE | 0 | 0 / 0 / 0 | All R2 fixes verified; Lockability: yes |

**Total post-merge Codex cost:** 3 rounds; 4 distinct
findings closed (4 BLOCK + 0 MINOR + 0 NIT).

**Stage-12 total Codex cost:**
- Per-stage tier-1: R1-R7 = 7 rounds; 27 findings
  (16 BLOCK + 11 MINOR; 1 deferred per project-wide
  convention disposition).
- Post-merge tier-3: R1-R3 = 3 rounds; 4 findings
  (4 BLOCK + 0 MINOR).
- **Combined: 10 rounds; 31 findings closed.**

Stage-12 lands in the stage-9 / stage-10 / stage-11
trajectory band:
- Stage 9: 12 rounds / 51 findings.
- Stage 10: 9 rounds / 29 findings.
- Stage 11: 11 rounds / 26 findings.
- **Stage 12: 10 rounds / 31 findings.**

**Stage 12 Phase B + merger contract step 6 CLOSED
2026-05-06.**

---
