# Stage 11 -- Review Log

Round-by-round Codex audit trail for stage-11's
Phase B per-stage adversarial review.

Each round's entry captures: verdict, issue counts,
corrections applied (per issue), and any cross-references
to spec-snapshot.md / discussions.md sections that were
revised.

Codex prompt template lives in `memory.md` §4. Stage-11
prompt scope: spec-snapshot.md §25 + adjacent in-place
updates (§14, §18.2, §19.8, §20.6, §21.8, §22.8, §23.9,
§24.9); discussions.md (10 D-tag audit-shape entries);
prior-art evidence files (rustc, rustc_public, charon,
mrustc).

Out-of-scope per the prompt: rendering content
(retargeted to stage 13 per D-11-K); reopening earlier
D-tags; phase-1 implementation details; markdown
formatting NITs.

Phase B opens 2026-05-05 with R1 against the cluster of
all 10 locked decisions.

---

## Round 1 (2026-05-05)

**VERDICT: BLOCK.** 8 issues (4 BLOCK / 4 MINOR / 0 NIT).
Lockability: no; D-11-3, D-11-5, and D-11-K need fixes
before final `[LOCKED]`.

Output captured at `/tmp/codex_stage11_review_r1.txt`.

**Issues + corrections applied:**

1. **[BLOCK] D-11-5 BodyId references** at
   `spec-snapshot.md:151, 290, 382, 432, 5191`. §25.5 said
   "no BodyId" but earlier normative schema still listed
   `BodyId` in the ID inventory, `RegseqModel.bodies`,
   invariant 1, invariant 10, and the §18.1 sigil table.
   **Resolution:** removed `BodyId` from the ID inventory
   (line 151), reframed `RegseqModel.bodies` as a
   stage-11-superseded baseline annotation (line 290),
   removed `BodyId` from invariant 1's resolve list (line
   382), rewrote invariant 10 to reference Item-side body
   per stage-11 D-11-3 + D-11-5 (line 432), removed
   `BodyId` row from the sigil table (line 5191).

2. **[BLOCK] D-11-3/D-11-5 §9.4 InstanceKind::Function
   contradiction** at `spec-snapshot.md:1504`. The variant
   still carried `sig: FnSigId` and `body: Option<BodyId>`,
   contradicting D-11-3 (Item-side FnSig) and D-11-5
   (no BodyId). **Resolution:** removed `sig` and `body`
   fields from `InstanceKind::Function`; added a stage-11
   reframe note explaining that signature + body live
   on the parent Item (`Instance.item: ItemId` ->
   `Item.kind = Function { fn_sig, body, ... }` per
   stage-12 schema expansion). The variant now carries
   only `flavor: FunctionInstanceFlavor` and `virtual_idx:
   Option<u32>` (post-mono dispatch metadata).

3. **[BLOCK] D-11-3 outbound-vs-§14 contradiction** at
   `discussions.md:357` (D-11-3.5) + `discussions.md:1187`
   (§4 outbound bundle). D-11-3.5 said the outbound to
   stage 12 "drops `safety`," but §14.3 keeps `safety`
   in the canonical 5-field FnSig schema. **Resolution:**
   updated D-11-3.5 + the §4 outbound to specify the
   pinned 5-field §14.3 schema (preserve `safety`). D-11-3
   reframes placement only, not schema; §14.3 stands.

4. **[BLOCK] D-11-K.4 vs §18.2 contradiction** at
   `discussions.md:1059`. D-11-K.4 said body-level renderer
   rows are REMOVED at merger contract; §18.2 in the
   stage-11 spec-snapshot.md already shows them as
   stage-13-owned. **Resolution:** rewrote D-11-K.4 to
   match the actual §18.2 state: rows are RETARGETED in
   place from "stage 11 (deferred)" to "stage 13 (per
   D-11-K)" at the stage-11 merger contract; rows remain
   in §18.2 with the new ownership; stage 13 may refine
   further when it lands.

5. **[MINOR] D-11-K.2 item count** at `discussions.md:1044`.
   D-11-K.2 said "8 items moving" but the actual list is
   9 public renderers + 1 internal helper + 2 sigil
   wirings = 12. **Resolution:** updated D-11-K.2's
   enumeration to specify "9 public renderers + 1 internal
   helper + 2 sigil rendering wirings = **12 items
   total**." Also fixed the §4 outbound bundle reference
   from "8-item rendering bundle" to "12-item rendering
   bundle."

6. **[MINOR] Stage-12/13 expectations.md cross-stage
   references** at `stage-12/expectations.md:361` and
   `stage-13/expectations.md:54`. Existing entries still
   said `render_place` was retargeted to stage 11 (the
   pre-D-11-K state). **Resolution:** updated
   `stage-12/expectations.md` to record the second
   retargeting (stage 11 -> stage 13 per D-11-K) with a
   cross-reference to `../stage-11/discussions.md`.
   Updated `stage-13/expectations.md`'s render_local
   coordination note to reflect D-11-K's categorical
   deferral (body-level renderers + sigils all stage-13-
   owned).

7. **[MINOR] §25.7.3 step-count mismatch** at
   `spec-snapshot.md:10502`. §25.7.3 said vtable rewrite
   is "5-step" but §25.7.2 has 6 procedure steps including
   the Call rewrite. **Resolution:** updated §25.7.3 to
   say "6-step rewrite (5 synthesis steps + Call rewrite
   at step 6)."

8. **[MINOR] Prior-art file:line citations** at
   `discussions.md:211, 311, 598` (D-11-2, D-11-3, D-11-6
   prior-art bullets). Several bullets cited only files
   /sections, not file:line. **Resolution:** added line-
   number references to all prior-art file citations in
   D-11-2, D-11-3, D-11-6 + corrected lingering
   alphabetic-section refs (§A/§D/§E/§G/§H) to match the
   actual numbered sections in the prior-art files
   (`prior-art/rustc.md` uses §1-§10; not §A-§K). Lines
   95, 425, 514, 804, 919 corrected.

All 4 BLOCK + 4 MINOR closed via corrections. Round 2
prompt to verify the corrections + scan for any
NEW-ISSUE-INTRODUCED.

## Round 2 (2026-05-05)

**VERDICT: BLOCK.** 4 issues (2 BLOCK / 2 MINOR / 0 NIT).
Lockability: no.

R1 RESOLUTIONS verified:
- R1-1 (BodyId references) -- RESOLVED.
- R1-3 (D-11-3 outbound vs §14) -- RESOLVED.
- R1-6 (stage-12/13 expectations.md) -- RESOLVED.
- R1-7 (§25.7.3 step count) -- RESOLVED.

R1 RESIDUAL issues (NOT-RESOLVED in R1's corrections):

R2-1 [BLOCK] R1-2 NOT-RESOLVED at `spec-snapshot.md:267, 685,
1261`. The §9.4 variant itself is correctly fixed (lines
~1516-1525), but stale current-spec text still references
`Function { sig, body, flavor, virtual_idx }` and "function
signatures live via FnSigId on InstanceKind::Function" at:
- Line 267 (`pub sigs:` field comment in §3 RegseqModel
  struct).
- Lines 685-694 (§3.X validation rule walk for `instances`
  and `bodies`).
- Lines 1261-1265 (§8 ItemKind text describing per-ItemKind
  rich data on Instance).
**Resolution:** updated `pub sigs:` comment to reference
the Item-side schema (Function-flavored Item carries
`fn_sig: FnSigId` + `body: Option<Body>` per stage-12
expansion). Updated validation rule to drop "and bodies"
+ list new Function payload (`flavor`, `virtual_idx` only).
Updated §8 ItemKind paragraph to reframe per D-11-3 +
D-11-5 (signatures + bodies live Item-side; only post-mono
dispatch metadata on InstanceKind::Function).

R2-2 [BLOCK] R1-4 NOT-RESOLVED at `spec-snapshot.md:10684`
(§25.11 outbound bundle). I fixed D-11-K.4 in
discussions.md but missed §25.11 in spec-snapshot.md;
the §25.11 outbound still said "§18.2 row removal at
merger" while the corrected D-11-K.4 says rows are
RETARGETED in place. **Resolution:** updated §25.11 to
"§18.2 row retargeting in place at merger" + expanded
the bundle enumeration to the 12-item form for
consistency with D-11-K.2.

R2-3 [MINOR] R1-5 NOT-RESOLVED at `discussions.md:1231`
(§4 outbound bundle second bullet). I fixed the first
bullet's "8-item rendering bundle" -> "12-item" but
missed the second bullet's "8 new body-content renderer
rows + 2 sigil helpers" subordinate count. **Resolution:**
updated the second bullet to reference D-11-K.2's
12-item enumeration (9 public + 1 internal + 2 sigils).

R2-4 [MINOR] R1-8 NOT-RESOLVED at `discussions.md:1042`
(D-11-K's prior-art bullet). I corrected §A/§B/§C/§D/§E/
§F/§G/§H but missed §K. The actual D-11-K-related section
in `prior-art/rustc.md` is §9 at line 537. **Resolution:**
changed `§K` to `prior-art/rustc.md:537 §9`; also added
line refs for the rustc_public + Charon peer citations
(`prior-art/rustc_public.md:681 §8`,
`prior-art/charon.md:719 §9`).

All 2 BLOCK + 2 MINOR closed via corrections. Round 3
prompt to verify the corrections + scan for any
NEW-ISSUE-INTRODUCED.

## Round 3 (2026-05-05)

**VERDICT: BLOCK.** 7 issues (2 BLOCK / 2 MINOR / 3 NIT).
Lockability: no.

R2 RESOLUTIONS verified:
- R2-2 (§25.11 outbound bundle wording) -- RESOLVED.
- R2-3 (discussions.md outbound bundle count) -- RESOLVED.
- R2-4 (D-11-K prior-art citation §K -> §9) -- RESOLVED.

R3 NEW issues:

R3-1 [BLOCK] §25.4 FnSig type-vs-ID mismatch at
`spec-snapshot.md:10166`. §25.4 said `item.kind =
Function { fn_sig: FnSig, ... }` but per §14.4 FnSigInterner
identity, FnSig is interned and references should use
`FnSigId`. **Resolution:** updated §25.4 to
`fn_sig: FnSigId, ... }` resolved through
`model.sigs[fn_sig]` per §14.4 identity rule.

R3-2 [BLOCK] BodyId reframe incomplete -- 4 stale
`bodies` references at `spec-snapshot.md:624, 1393,
1590, 9232`. **Resolution:**
- Line 624 (Phase A parsing): dropped `bodies` from
  raw-storage parsing list with a stage-11 reframe note
  (bodies live structurally inside Items per D-11-5).
- Line 1393 (§9.2 lang-item Item entries): replaced
  "no body in `bodies`" with "the parent Function Item's
  `body: None` per stage-11 D-11-5".
- Line 1590 (§9.4 rich-data table list): dropped
  `bodies` from the rich-data tables list; added stage-11
  reframe note (bodies Item-side; no separate `bodies`
  table).
- Line 9232 (§24.2.1 IndexVec convention list): dropped
  `model.bodies` from the crate-level interner list with
  stage-11 reframe note.

R3-3 [MINOR] §9.4 doc comment stale `sig` reference at
`spec-snapshot.md:1519`. Comment said "sig is always
present" referencing a now-non-existent
`InstanceKind::Function.sig` field. **Resolution:**
updated to "the parent Function Item's `fn_sig` is
always present" matching the post-reframe shape.

R3-4 [MINOR] §25.7.1 stale virtual-flavor payload shape
at `spec-snapshot.md:10384`. Said `flavor:
FunctionInstanceFlavor::Virtual { virtual_idx: Some(0) }`
treating Virtual as a tuple variant, but `Virtual` is
unit-variant + `virtual_idx` is a separate
`InstanceKind::Function` field. **Resolution:** rewrote
to `InstanceKind::Function { flavor:
FunctionInstanceFlavor::Virtual, virtual_idx: Some(0) }`
matching §25.7.2's correct shape + the §9.4 schema.

All 2 BLOCK + 2 MINOR closed via corrections. Round 4
prompt to verify the corrections + scan for any
NEW-ISSUE-INTRODUCED.

## Round 4 (2026-05-05)

**VERDICT: BLOCK.** 5 issues (1 BLOCK / 0 MINOR / 4 NIT).
Lockability: no; D-11-3 not lockable until §23 call-
invariant signature lookup retargeted to Item-side.

R3 RESOLUTIONS verified:
- R3-1 (§25.4 FnSigId / model.sigs[fn_sig]) -- RESOLVED.
- R3-2 (4 stale `bodies` references) -- RESOLVED.
- R3-3 (§9.4 doc comment) -- RESOLVED.
- R3-4 (§25.7.1 virtual-flavor payload shape) -- RESOLVED.

R4 NEW issue:

R4-1 [BLOCK] §23.6 + §23.8 + §25.7.2 + discussions.md
D-11-7.5 stale `lookup_instance(callee_id).fn_sig` /
"resolved Instance's fn_sig" / "callee's fn_sig.inputs()[0]"
shorthands at:
- spec-snapshot.md:8513, 8515, 8530, 8608 (§23.6.2 B-3,
  B-4, B-9 invariant statements).
- spec-snapshot.md:8932 (§23.8 example prose).
- spec-snapshot.md:10506 (§25.7.2 invariant-timing prose).
- spec-snapshot.md:8589, 8955, 8982 (additional prose
  references).
- discussions.md:780 (D-11-7.5 reasoning).

These all assumed `InstanceKind::Function` carried a
`sig: FnSigId` field, which the stage-11 D-11-3 + D-11-5
reframe removed. **Resolution:** added a `callee_fn_sig`
helper definition at the head of §23.6.2 (post-§23.6.2
preamble) defining the 3-level indirection
(Instance -> Item -> FnSigId -> FnSig), then replaced
all stale shorthands. `lookup_instance(callee_id).fn_sig`
-> `callee_fn_sig(callee_id)` (8 sites; 2 multi-line
references reflowed). Prose references (§23.8 example,
§25.7.2 timing, discussions.md D-11-7.5) updated to
either use the helper or explicitly describe the
Item-side resolution chain.

All 1 BLOCK closed via corrections; 4 NITs are RESOLVED
verifications. Round 5 prompt to verify R4 corrections
+ scan for NEW-ISSUE-INTRODUCED + check spec-wide
coherence under the now-fully-applied stage-11 reframe.

## Round 5 (2026-05-05)

**VERDICT: MINOR.** 2 issues (0 BLOCK / 2 MINOR / 0 NIT).
Lockability: 7-of-10 yes (D-11-1, D-11-2, D-11-4, D-11-5,
D-11-8, D-11-K -- correct count is 6 fully-yes per Codex
output); 3 with-conditions (D-11-3, D-11-6, D-11-7) per
the 2 remaining MINORs. R4-1 verification PASSED:
no remaining live `lookup_instance(callee_id).fn_sig`
shorthand; the helper definition is acceptable as a
stage-11 forward reference to stage-12's `Item.kind =
Function { fn_sig }` schema.

R5-first-attempt timed out at 1500s; R5-retry with a
focused prompt completed.

R5 NEW issues:

R5-1 [MINOR] D-11-3 §23.2.3 Return-variant prose at
`spec-snapshot.md:8046`. Said `FnSig.output()` is resolved
"via the body's signature," but post stage-11 D-11-3
reframe Body has no `fn_sig` and the signature is
Item-side. **Resolution:** updated §23.2.3 prose to give
both reachability paths: (a) directly via
`Body.local_decls[RETURN_PLACE].ty` (positional
convention); (b) via the parent Function Item's signature
`model.sigs[item.kind.as_function().fn_sig].output()` per
the §23.6.2 `callee_fn_sig` helper. Also softened §25
overview phrase at line 9988 ("function-signature reference"
-> reframe noting Body has no fn_sig field, signature
Item-side per D-11-3, types reachable through
`local_decls`).

R5-2 [MINOR] D-11-6/D-11-7 Terminator-invariant range typo
at `spec-snapshot.md:10375` (§25.6.4) and
`spec-snapshot.md:10645` (§25.9 B-Body-10 row). Both said
"§23.6 B-1..B-7" but §23.6 has B-1..B-9 (B-8 + B-9 are
post-rewrite + closure-completeness invariants, both
stage-11-relevant). **Resolution:** updated both summary
rows from `B-1..B-7` to `B-1..B-9`.

All 2 MINORs closed via corrections. Round 6 prompt to
verify R5 corrections + final lockability assessment.
Stage-9 / stage-10 trajectory benchmark: typically
APPROVE or MINOR-with-Lockability:yes-for-all reached
within 5-7 rounds; stage 11's pattern is on track for
R6 convergence.

## Round 6 (2026-05-05)

**VERDICT: APPROVE.** 0 issues. **Lockability: yes for
all 10 D-tags. Close-out: YES.**

R5 RESOLUTIONS verified (per-D-tag with-conditions cleared):
- R5-1 (§23.2.3 + §25 overview) -- RESOLVED. D-11-3 now yes.
- R5-2 (§25.6.4 + §25.9 B-Body-10) -- RESOLVED. D-11-6 +
  D-11-7 now yes.

Per-D-tag lockability (all yes):
- D-11-1 yes (Body 4-field shape).
- D-11-2 yes (LocalDecl 4-field shape).
- D-11-3 yes (FnSig Item-side reframe; rewrite-before-
  validation pinned).
- D-11-4 yes (LocalId via newtype_index!).
- D-11-5 yes (no BodyId; bodies as Item structural field).
- D-11-6 yes (joint validation visitor; 10 hook pairs).
- D-11-6.1 yes (construction invariants 1+9 fold-in).
- D-11-7 yes (hybrid (c) timing; cross-stage friction
  documented; rewrite-before-validation pinned).
- D-11-8 yes (single push API + synthetic-local helper).
- D-11-K yes (categorical rendering deferral to stage 13).

**Phase B per-stage Codex review CLOSED 2026-05-05.**

Total stage-11 Phase B Codex cost: **6 productive rounds**
(R1, R2, R3, R4, R5-retry, R6) + 1 timed-out attempt
(R5-first-attempt). 19 distinct findings closed:
- R1: 4 BLOCK + 4 MINOR + 0 NIT (8 total)
- R2: 2 BLOCK + 2 MINOR + 0 NIT (4 total; 4 R1 RESOLVED NITs)
- R3: 2 BLOCK + 2 MINOR + 3 NIT (4 substantive; 3 R2 RESOLVED NITs)
- R4: 1 BLOCK + 0 MINOR + 4 NIT (1 substantive; 4 R3 RESOLVED NITs)
- R5: 0 BLOCK + 2 MINOR + 0 NIT (2 substantive)
- R6: 0 issues (4 R5 RESOLVED NITs)

Total substantive issues closed: **19** (9 BLOCK + 10
MINOR + 0 NIT [excluding RESOLVED-verification NITs]).

Trajectory benchmark match: stage-9's per-stage portion
was 6 rounds + 21 findings; stage-10's was 5 rounds + 19
findings. Stage 11 lands at 6 rounds + 19 findings --
right in the convergence band given the larger spec
content (10 D-tags + cross-stage reframes).

Next step: stage-11 merger contract per README §5.5 7-step
protocol.
