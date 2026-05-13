# Stage 6 -- Review Log

Round-by-round Codex review trail for stage-6 spec content.
Each round captures the verdict, full issue list, and the
corrections applied before the next round (or before close
if the round is the last).

Cluster scope: D-6-1 (parent + sub-decisions D-6-1.1 through
D-6-1.4) + D-6-2 + D-6-3 + D-6-K (single combined cluster
per `status.md` "Cluster split").

Codex prompt template lives in `memory.md` §4. Round-specific
prompt-shape skeletons live in `status.md` "Next concrete
unit of work."

---

## Round 1 (2026-05-04)

**Verdict:** BLOCK. **Total:** 6 issues (2 BLOCK / 4 MINOR /
0 NIT).

**Codex output:** `/tmp/codex_d6_review_r1.txt` (also archived
inline below).

### Issues

1. **[BLOCK] `d6-runtimechecks-q6-grounding`** -- spec
   claims Q4/Q6 imply "single-crate-graph build" with one
   session per build, but Q6 explicitly specifies a
   multi-crate strategy (regseq facade + regseq-aware
   crates per `architecture.md:437-443, 458-465, 481-492,
   513-515`). The "single-crate-graph" framing is unfounded;
   the rustc multi-crate `RuntimeChecks` rationale collapse
   needs a different grounding.

2. **[BLOCK] `constoperand-span-loss`** -- spec claims
   operand-level `span` is recoverable from containing
   Statement / Terminator `SourceInfo`. Rustc has those at
   `statement.rs:14-19` and `terminator.rs:435-438`, but
   `TerminatorKind::Call` and `TailCall` carry their own
   `Box<[Spanned<Operand>]>` for arguments
   (`syntax.rs:811-820, 850-857`); the per-argument span is
   *outside* the Operand but not in the Terminator's
   `source_info`. The span-recovery claim is too coarse.

3. **[MINOR] `d6k-stub-message-drift`** -- §20.6 stub message
   says `unimplemented!("stage 11 fills render_operand
   (deferred)")`; §18.2 and stage-11/expectations.md "From
   stage 6" use `(deferred per D-6-K)`. D-5-K precedent
   includes the D-tag in the stub. Standardise on the
   D-tag-suffixed form.

4. **[MINOR] `const-c-lowering-crossref`** -- §20.4 says
   `Constant(id)` lowers "per the per-Const-variant lowering
   in §9.7," but §9.7 contains the Const enum, identity
   schema, and construction dispatch -- *not* a per-variant
   C-lowering table. Either add the table to §9.7 or reword
   §20.4 as a stage-6-internal sketch.

5. **[MINOR] `stale-charon-runtimechecks-mapping`** --
   `prior-art/charon.md:498-503` still says "RegseqModel
   rejects" RuntimeChecks; the actual decision (D-6-1.2)
   lowers to `Const::Bool`. Stale; contradicts the audit
   record.

6. **[MINOR] `audit-template-omissions`** -- discussions.md
   sub-decisions D-6-1.1, D-6-1.3, D-6-3, D-6-K omit "Key
   options" and/or "Worked example" sections. D-6-2 omits
   "Worked example." Add brief versions or mark as
   inherited from parent D-6-1.

### Corrections applied (pre-R2)

All 6 findings addressed.

1. **`d6-runtimechecks-q6-grounding` (BLOCK).** Re-grounded
   the `RuntimeChecks` lowering rationale. The wrong
   "single-crate-graph (Q4/Q6)" framing was replaced
   throughout (`spec-snapshot.md` §20.3.1 step 1;
   `discussions.md` D-6-1 reasoning, D-6-1.1 reasoning + new
   Worked example, D-6-1.2 reasoning; `memory.md` §2
   RuntimeChecks principle bullet; `prior-art/rustc.md`
   D-6-1 mapping; `status.md` D-6-1 [DRAFT] entry). The
   correct framing: per Q4 (`rustc_driver::Callbacks`
   integration), the bridge runs inside one rustc invocation
   per build with one consuming `Session` in scope; all
   `RuntimeChecks` operands -- including those from Q6's
   regseq-aware dependencies, consumed via
   `tcx.optimized_mir(def_id)` -- resolve against the
   bridge-host's session, mirroring rustc's standard codegen
   behavior (consumer's session determines the value).
   Different builds produce different lowered consts;
   F4 / F4-2 already excludes builds that disagree on
   sessions (build-equivalence quad in §16). Resolution call
   updated from `checks.value(sess)` to `checks.value(tcx.sess)`.

2. **`constoperand-span-loss` (BLOCK).** Refined the
   span-recovery claim to distinguish two cases:
   (a) Statement-level operand uses recover the use-site
   span via the containing Statement's `SourceInfo` at
   stage 8 (`rustc_middle/src/mir/statement.rs:14-19`);
   (b) Call / TailCall arguments carry a finer-grained
   per-argument span via `Spanned<Operand>` at the
   Terminator level (`syntax.rs:811-820, 850-857`),
   captured by stage 9 at the Terminator's argument-list
   slot, *not* in the Operand. The operand-internal `span`
   is dropped in both cases; the wrapper drop is justified.
   Updated `spec-snapshot.md` §20.1 docstring + §20.3
   paragraph; `discussions.md` D-6-1.3 Decision + Reasoning
   sections; `status.md` D-6-1 [DRAFT] entry.

3. **`d6k-stub-message-drift` (MINOR).** Updated
   `spec-snapshot.md` §20.6 stub message to
   `unimplemented!("stage 11 fills render_operand (deferred per D-6-K)")`,
   matching the §18.2 ownership row, the
   `stage-11/expectations.md` "From stage 6" stub
   reference, and the D-5-K precedent.

4. **`const-c-lowering-crossref` (MINOR).** Reworded
   `spec-snapshot.md` §20.4 `Constant(id)` row to
   acknowledge that §9.7 carries the `Const` vocabulary
   and construction dispatch but *not* a per-variant
   C-lowering table. Stage 6 commits only to the
   `Operand::Constant(ConstId)` -> Const-resolution
   boundary; concrete per-`Const`-variant C lowerings are
   the C-emission stage's territory.

5. **`stale-charon-runtimechecks-mapping` (MINOR).**
   Updated `prior-art/charon.md` D-6-1 mapping section
   from "Charon lowers, RegseqModel rejects" to
   "Charon convergence: both lower, with different
   downstream shapes" -- accurately reflecting D-6-1.2
   option-C decision. Charon lowers at the rvalue level
   (`Rvalue::NullaryOp` + `Move(local)`); RegseqModel
   lowers more aggressively at the operand level (directly
   to `Operand::Constant(ConstId)` referencing
   `Const::Bool(b)`). Each gradient is appropriate to its
   project's downstream consumers.

6. **`audit-template-omissions` (MINOR).** Added missing
   audit-template sections to `discussions.md`:
   - D-6-1.1: added Key options (referencing parent
     D-6-1's option set) + Worked example
     (`cfg!(overflow_checks)` translation chain).
   - D-6-1.3: added Key options (Box wrapper / inline
     Const / ConstId reference) + Worked example
     (`let x: u32 = 42;` translation with content-uniqueness
     deduplication).
   - D-6-2: added Worked example (two `Operand::Copy(Place::from(_1))`
     uses; interning would dedup but no consumer asks).
   - D-6-3: added Key options (enforce / weaken / inherit) +
     Worked example (`fn drop_string(s: String)`'s
     `Operand::Move(_1)` at drop point).
   - D-6-K: added Key options (stage 6 / stage 11 / stage 14) +
     Worked example (`Operand::Move(_3)` rendering chain;
     `Operand::Constant(id)` late-binding to stage 14
     `render_const`).

Ready for R2 review (verify R1 fixes; surface any new
issues from the corrections; sweep residual issues).

---

## Round 2 (2026-05-04)

**Verdict:** BLOCK. **Total:** 6 issues (2 BLOCK / 4
MINOR / 0 NIT). R1 verification: MINOR 3, MINOR 4, MINOR 6
fully verified. BLOCK 1, BLOCK 2, MINOR 5 partial-fix
residuals.

**Codex output:** `/tmp/codex_d6_review_r2.txt`.

### Issues

1. **[BLOCK] `runtimechecks-f4-quad-gap`** -- R1 BLOCK 1
   partially verified. The new "build-equivalence quad
   excludes builds that disagree on session settings"
   claim was unfounded: §12.4 / §16's quad lists equal
   source, pinned toolchain, target, and bridge options
   (limited to cleanup whitelist + `mir_opt_level=0`); it
   does NOT name `ub_checks` / `contract_checks` /
   `overflow_checks`. The cross-build alignment claim
   doesn't hold.

2. **[BLOCK] `stale-q6-single-crate-prior-art`** -- R1
   BLOCK 1 partially verified. `prior-art/rustc.md:201-209`
   still said RegseqModel "ingests a single regseq-aware
   crate per build" -- stale, contradicts Q6 (multi-crate
   strategy via `architecture.md:437-443, 487-492, 513-515`).

3. **[MINOR] `runtimechecks-call-drift`** -- R1 BLOCK 1
   partially verified. The translation table at §20.3 line
   5720 used `checks.value(sess)`; the audit-shape Option
   C / Decision text at `discussions.md:50-52, 236-243`
   also said `sess`. The corrected `tcx.sess` form was
   only at the resolution-chain narrative.

4. **[MINOR] `status-stale-single-crate-graph`** -- R1
   BLOCK 1 partially verified. `status.md:240-243` still
   contained the old "Q4/Q6 single-crate-graph" prompt
   text outside review-log quotation context.

5. **[MINOR] `stage9-span-expectation-missing`** -- R1
   BLOCK 2 partially verified. The span model itself was
   correct, but the refinement created a new outbound
   stage-9 obligation (per-argument span preservation via
   `Spanned<Operand>` at Call/TailCall Terminator sites)
   that wasn't filed in `stage-9/expectations.md`.

6. **[MINOR] `charon-stale-reject-text`** -- R1 MINOR 5
   partially verified. The D-6-1 mapping section was
   updated, but `prior-art/charon.md:215-218` still
   said RegseqModel's "current D-6-1 leaning" is "reject
   on encounter via `unsupported_construct!`" -- stale.

### Corrections applied (pre-R3)

All 6 findings addressed.

1. **`runtimechecks-f4-quad-gap` (BLOCK).** Replaced the
   F4/F4-2 cross-build-exclusion claim with an honest
   acknowledgement that §12.4 / §16's build-equivalence
   quad does NOT currently list `ub_checks` /
   `contract_checks` / `overflow_checks`; two builds
   differing only on those flags satisfy the quad's
   equivalence but produce different lowered consts. If
   cross-build alignment of `RuntimeChecks`-derived
   operands becomes a requirement, widening the quad in
   §12.4 is the appropriate fix (out of stage 6's scope).
   Updated `spec-snapshot.md` §20.3.1 step 1 + `discussions.md`
   D-6-1.2 reasoning.

2. **`stale-q6-single-crate-prior-art` (BLOCK).** Replaced
   the stale "single regseq-aware crate per build"
   paragraph in `prior-art/rustc.md:200-211` with the
   corrected bridge-host-session framing: per Q4 the
   bridge runs inside one rustc invocation per build with
   one consuming `Session`; Q6's multi-crate strategy
   (regseq facade + regseq-aware dependencies compiled
   separately under `RegseqAwareCallbacks`, with their MIR
   consumed via `tcx.optimized_mir(def_id)`) does not
   break this -- the bridge is a single consumer
   regardless of how many crates contributed MIR. The
   rustc multi-consumer rationale collapses to a single
   resolution site at the bridge boundary.

3. **`runtimechecks-call-drift` (MINOR).** Standardised
   `checks.value(tcx.sess)` everywhere in normative /
   example lowering text: `spec-snapshot.md` §20.3
   translation table; `discussions.md` D-6-1 Option C
   description, D-6-1.2 Option (c) description, D-6-1.2
   Decision step 1; `status.md` D-6-1 [DRAFT] entry
   chain.

4. **`status-stale-single-crate-graph` (MINOR).** Updated
   `status.md` R1 prompt-shape text to use the corrected
   Q4/Q6 wording (no "single-crate-graph" residual);
   evaluation criterion 2 now spells out "per Q4 (one
   consuming Session per build), `checks.value(tcx.sess)`
   resolves against the bridge-host's session even when
   MIR comes from regseq-aware dependencies via
   `tcx.optimized_mir(def_id)` (Q6)" + "cross-build
   determinism limits noted relative to §12.4 quad."
   Criterion 3 also refreshed to spell out the span-
   recovery split (Statement.source_info vs Spanned<Operand>
   at Terminator).

5. **`stage9-span-expectation-missing` (MINOR).** Filed
   outbound expectation in `stage-9/expectations.md`
   under a new "From stage 6" section. Stage 9 must
   preserve per-argument spans for Call / TailCall via
   `Spanned<Operand>` wrappers (or a parallel-slabs
   equivalent); shape choice is stage 9's, but the
   preservation obligation is stage-6-imposed. Cross-
   referenced from `discussions.md` D-6-1.3 Cross-refs.

6. **`charon-stale-reject-text` (MINOR).** Updated
   `prior-art/charon.md:215-218` -- the section now
   accurately reflects that RegseqModel's D-6-1.2
   decision *also* lowers (more aggressively, at the
   operand level vs Charon's rvalue level), not rejects.
   No other "RegseqModel rejects" residuals remain.

Ready for R3 review (verify R2 fixes; surface any new
issues; sweep residuals).

---

## Round 3 (2026-05-04)

**Verdict:** MINOR. **Total:** 1 issue (0 BLOCK / 1
MINOR / 0 NIT). All 6 R2 fixes verified.

**Codex output:** `/tmp/codex_d6_review_r3.txt`.

R2 fix verification: all six R2 fixes verified.
- R2 BLOCK 1 (F4/F4-2 quad gap re-framing): verified;
  spec, discussions, and §12.4/§16 grounding consistent.
- R2 BLOCK 2 (prior-art/rustc.md stale single-crate
  paragraph): verified; bridge-host session framing now
  consistent with Q6 architectural content.
- R2 MINOR 3 (sess -> tcx.sess): verified across all
  normative/example sites.
- R2 MINOR 4 (status.md prompt-template stale): verified.
- R2 MINOR 5 (stage-9 outbound expectation): verified;
  filed entry cites the right rustc source line refs and
  defines the wrapper / parallel-slabs deliverable shape.
- R2 MINOR 6 (charon.md stale "reject" mention):
  verified; both-lower framing now consistent throughout.

### Issues

1. **[MINOR] `status-review-cadence-stale`** -- `status.md`
   "Codex review cadence (status)" still said "No rounds
   run yet" when R1 and R2 had completed (both 2026-05-04).
   Bookkeeping drift only; not a substantive spec issue.

### Corrections applied (pre-R4)

1. **`status-review-cadence-stale` (MINOR).** Updated
   `status.md` "Codex review cadence (status)" section to
   reflect the actual R1 / R2 / R3 trajectory with
   verdicts, issue counts, and headline findings. Updated
   the file's top-level Status header from "PHASE A DRAFT"
   to "PHASE B IN PROGRESS -- R3 MINOR" with the trajectory
   summary; updated the "Last updated" line accordingly.

Ready for R4 review (verify the bookkeeping fix lands
cleanly; sweep for any final residuals; lock the cluster
if R4 = APPROVE or MINOR-NIT-only stable per the stage-5
diminishing-returns precedent).

---

## Round 4 (2026-05-04)

**Verdict:** MINOR. **Total:** 2 issues (0 BLOCK / 2
MINOR / 0 NIT). R3 fix partially verified
(cadence-section-only; other Phase-A residuals in
status.md uncovered). One additional citation finding.
Codex explicitly notes "Substantive §20 content otherwise
appears lock-ready after these fixes."

**Codex output:** `/tmp/codex_d6_review_r4.txt`.

### Issues

1. **[MINOR] `status-r3-fix-partial`** -- R3 corrections
   updated the "Codex review cadence (status)" section but
   left other Phase-A residuals in `status.md`: the
   "Locked decisions" intro still said "Phase A draft
   state... not yet committed... nor exercised by Codex
   review"; the "Next concrete unit of work" still said
   to commit §20 and file the stage-11 outbound
   expectation -- both already done before R1.

2. **[MINOR] `const-translation-crossref-wrong-section`**
   -- §20.3 says `translate_const` is "the §13 bridge
   translation"; §13 is "Synthesised Items" (closure /
   vtable / etc.). The `Const` vocabulary, `ConstInterner`,
   and rustc-to-Const construction dispatch are all in
   §9.7. Repeated in `discussions.md:326-329`.

### Corrections applied (pre-R5)

1. **`status-r3-fix-partial` (MINOR).** Refreshed the
   "Locked decisions" intro from "Phase A draft state...
   not yet committed... nor exercised by Codex review"
   to a Phase-B-in-progress framing (spec content
   committed, audit shape in discussions.md, outbound
   expectations filed in stage-9 and stage-11; D-tags
   remain `[DRAFT]` until lock). Replaced the "Next
   concrete unit of work" section's stale "commit drafted
   decisions" content with the actual current next step
   (R5 verification of R4 fixes; lock the cluster on
   APPROVE / MINOR-NIT-only stable). Replaced the stale
   R1 prompt-shape block with a brief R5 prompt-shape and
   note about historical R1-R4 prompts being recoverable
   from review-log.md / conversation history. Refreshed
   the file's top-level Status header with the R3 MINOR /
   R4 MINOR trajectory and adjusted the "Last updated"
   line.

2. **`const-translation-crossref-wrong-section` (MINOR).**
   Updated `spec-snapshot.md` §20.3 paragraph and
   `discussions.md` D-6-1.3 Decision section: the
   `translate_const` reference now cites "§9.7 bridge
   construction dispatch (`rustc -> Const`)" rather than
   "§13 bridge translation." Spot-checked all other §20
   cross-references; no other miscites.

Ready for R5 review (verify R4 fixes land cleanly;
sweep for any final residuals; lock the cluster if R5 =
APPROVE or MINOR-NIT-only stable per the stage-5
diminishing-returns precedent).

---

## Round 5 (2026-05-04)

**Verdict:** MINOR. **Total:** 2 issues (0 BLOCK / 2
MINOR / 0 NIT). R4 MINOR 2 fully verified; R4 MINOR 1
partial (status.md header / Last-updated forgot to
advance to R4). One new typo finding in stage-9 outbound.

**Codex output:** `/tmp/codex_d6_review_r5.txt`.

### Issues

1. **[MINOR] `status-r4-metadata-stale`** -- R4 MINOR 1
   partially verified. status.md "Locked decisions"
   intro, "Next concrete unit of work" section, and
   prompt-shape block were all refreshed to current
   state (Phase-B-in-progress; R5 verification + lock
   prep; R5-shaped prompt). However the file's *header
   metadata* (top-level Status block at line 3 + "Last
   updated" line) was not advanced -- still said "R3
   MINOR" / "R4 verification pending" instead of
   "R4 MINOR" / "R5 verification pending."

2. **[MINOR] `stage9-operand-interning-drift`** -- The
   stage-9 outbound expectation said per-argument span
   should sit alongside the "(interned) `Operand`."
   Contradicts D-6-2 / §20.2: operands have no
   `OperandId`, no top-level interning, are constructed
   inline at use sites. Typo introduced when filing the
   outbound.

### Corrections applied (pre-R6)

1. **`status-r4-metadata-stale` (MINOR).** Updated
   status.md header (line 3) to "R5 MINOR corrections
   applied; R6 verification pending"; expanded the
   trajectory summary to include R4 (2 findings) + R5
   (2 findings) with their headline issues; updated
   "Last updated" line accordingly. Refreshed the
   "Codex review cadence (status)" trajectory list to
   include R4 + R5 entries with verdicts, issue counts,
   and headline findings.

2. **`stage9-operand-interning-drift` (MINOR).**
   Replaced "(interned) `Operand`" with "inline
   `Operand`" in `stage-9/expectations.md` "From stage 6"
   section, plus a parenthetical citing D-6-2 explicitly
   so the inline-storage rule is visible at the outbound
   site.

Ready for R6 review (verify R5 fixes; final lockability
sweep; lock the cluster if R6 = APPROVE or MINOR-NIT-only
stable per the stage-5 diminishing-returns precedent).

---

## Round 6 (2026-05-04)

**Verdict:** MINOR. **Total:** 1 issue (0 BLOCK / 1 MINOR
/ 0 NIT). R5 MINOR 1 + 2 both verified. Codex explicit
verdict: **"Lockability: yes."**

**Codex output:** `/tmp/codex_d6_review_r6.txt`.

R5 fix verification: both verified.
- R5 MINOR 1 (status header / Last-updated / cadence
  refresh): verified across `status.md:3, 18, 148`.
- R5 MINOR 2 (stage-9 outbound "interned" -> "inline"):
  verified at `stage-9/expectations.md:159`,
  consistent with D-6-2's no-interning rule
  (`spec-snapshot.md:5697`, `discussions.md:496`).

Substantive §20 confirmed lock-ready by Codex at R4, R5,
and R6: 3 variants (`spec-snapshot.md:5646`), no operand
interning (`spec-snapshot.md:5697`), RuntimeChecks
lowering (`spec-snapshot.md:5743`), stage-9 span handoff
(`stage-9/expectations.md:144`), stage-11 render deferral
(`stage-11/expectations.md:260`).

### Issues

1. **[MINOR] `status-r5-lower-blocks-stale`** -- The
   status header / "Last updated" / cadence section were
   correctly advanced to R5/R6 framing, but lower blocks
   in the same file still described R5 as the next
   pending step: "Locked decisions" intro at line 91
   said "R5 verification pending"; "Next concrete unit
   of work" at line 195 said "kick off R5"; the trailing
   prompt block was R5-shaped. Codex explicitly noted
   the residual could be cleaned during lock or the
   top-level merger review.

### Corrections applied (cluster closure)

1. **`status-r5-lower-blocks-stale` (MINOR).** Refreshed
   all stale lower blocks in `status.md`:
   - Top-level Status header advanced to "STAGE 6 SPEC-
     SIDE CLOSED 2026-05-04" with the full R1-R6
     trajectory + total review cost (6 rounds, 17
     distinct findings: 4 BLOCK + 13 MINOR + 0 NIT).
   - "Locked decisions" intro replaced with the cluster-
     closure rationale; all four D-tags (D-6-1, D-6-2,
     D-6-3, D-6-K) marked `[LOCKED]`.
   - "Next concrete unit of work" rewritten as the
     7-step merger contract from README §5.5, mirroring
     stage-5's status.md cluster-closure pattern.
   - Stale R5-shaped prompt block replaced with the
     post-merge Codex review prompt-shape (step 6).

### Cluster closure rationale

**Stage 6 cluster locked at R6 MINOR (1 bookkeeping
finding).** Trajectory:
- R1 BLOCK -- 6 issues (substantive grounding errors).
- R2 BLOCK -- 6 issues (mostly R1 partial-fix residuals).
- R3 MINOR -- 1 issue (cadence drift).
- R4 MINOR -- 2 issues (status Phase-A residuals,
  §13->§9.7 cross-ref). **Codex notes substantive §20
  lock-ready.**
- R5 MINOR -- 2 issues (header drift, "interned"
  typo). **Codex confirms substantive §20 lock-ready.**
- R6 MINOR -- 1 issue (status lower-blocks drift).
  **Codex explicit verdict: "Lockability: yes."**

Total review cost: 6 Codex rounds; 17 distinct findings
closed (4 BLOCK + 13 MINOR + 0 NIT).

**Methodology-deviation rationale.** Per the stage-5
precedent ("Cluster closure (2026-05-04): D-5-1 LOCKED at
R10 MINOR (2 NITs; residual cross-reference cleanup
deferred to top-level merger Codex review per README §5.5
step 6)"), the R6 bookkeeping finding was swept inline
during R6 corrections; any further residuals discovered at
top-level merger review will be absorbed there rather than
spawning further per-stage rounds. Substantive §20 content
was confirmed lock-ready by Codex at three consecutive
rounds (R4, R5, R6), with R6 carrying an explicit
"Lockability: yes" verdict. The diminishing-returns
threshold has been reached.

Stage 6 spec-side work is complete. Next workstream:
merger contract per README §5.5 (top-level integration
into `../spec.md`, `../discussions.md`, `../review-log.md`,
`../decisions.md` plus post-merge Codex review pass).
