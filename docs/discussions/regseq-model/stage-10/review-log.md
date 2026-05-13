# Stage 10 -- Codex Review Log

Round-by-round audit trail for stage-10 per-stage Codex
review of `spec-snapshot.md` (§24 Basic-block vocabulary
+ in-place updates to §22 / §23.5 / §18.2). Per
`memory.md` §4 template; cluster shape per
`status.md` Active cluster (single combined cluster
covering all 6 D-tags + 18 sub-decisions).

Reviewed entity: `stage-10/spec-snapshot.md` (10297
lines as of 2026-05-05; stage-10's contributions are §24
plus the in-place updates listed below).

Stage-10 in-place updates touched at Phase B step 4:
- §22 (Statement vocabulary intro paragraph at the
  start; §22.1.1 "Statements live inline" text;
  §23.4 step 4 Drop-as-Call collapse procedure):
  3 forward references to `BasicBlock.statements:
  Vec<Statement>` updated to `Box<[Statement]>` with
  §24.1.2 cross-references.
- §23.5 (Identity and storage): "Stage-10 vocabulary
  preview" replaced with forward-reference to §24 and
  locked `Box<[Statement]>` form per D-10-1.2.
- §18.2 (Renderer status): `render_basic_block` row
  added after `render_assert_kind`, before `render()`.

---

## Round 1 -- 2026-05-05

**Initiated.** Codex invocation in background via
`scripts/python/codex_runner.py` with `--effort high
--timeout 1500 --message-only`. Prompt covers all 6
D-tags + 18 sub-decisions across 9 evaluation
criteria. Output to `/tmp/codex_stage10_review_r1.txt`.

**Verdict.** BLOCK.

**Issues:**

1. **[BLOCK] D-10-4 / §24.4.3 (RPO walk vacuous).**
   §24.4.3's "every successor `j` is either back-edge
   (`j <= i`) or `j > i`" is vacuously true for every
   integer pair. Does not enforce B-RPO or rustc's
   actual postcondition. Need a real DFS edge
   classification or explicit RPO recompute + storage
   comparison.

2. **[BLOCK] D-10-4 / D-10-5 (RPO root vs unreachable
   blocks).** B-RPO defines RPO rooted at
   `START_BLOCK`, but §24.6.5 first bullet allows
   unreachable blocks; rustc's `Postorder::new` starts
   from one root, so unreachable blocks are not part
   of the rooted RPO. Either make reachability an
   invariant or define RPO over reachable subsequence.

3. **[MINOR] D-10-5 / §24.6.3 (walk ownership of Part
   A rows 1-2).** §24.6.3 says stage-11 body walk
   applies §24.6.1 rejection rules, but rows 1-2
   inspect rustc-side fields gone after bridge
   construction (`is_cleanup`, `Option<Terminator>`)
   and §24.7 already assigns those checks to bridge
   translation. discussions.md Part C only assigns
   B-RPO / B-Successors-Valid / B-NonEmpty to the
   walk. Fix: state Part A is bridge-enforced;
   stage 11 walk checks only Part B invariants.

4. **[MINOR] D-10-5 / §24.6.5 (block-ID density).**
   "Block-ID density is NOT enforced" claims IndexVec
   storage could have gaps, contradicting stage-1 §2.6
   canonical dense `IndexVec` semantics. IndexVec has
   no gaps by construction. Remove or restate.

5. **[MINOR] D-10-2.1 (typed-peer count off-by-one).**
   discussions.md D-10-2.1 reasoning chain step 3 says
   "3-of-4 peers use a typed-vec equivalent and only
   mrustc is weaker," but rustc_public uses
   `Vec<BasicBlock>` + `BasicBlockIdx = usize` (bare).
   Accurate count: 2-of-4 typed (rustc, Charon) vs
   2-of-4 bare (rustc_public, mrustc).

6. **[MINOR] D-10-3.4 / §24.3.4 (Charon `bb{}`
   citation stale).** §24.3.4 cites
   `fmt_with_ctx.rs:206-214` for the `bb{}` label, but
   those lines render block contents only; the actual
   label is at `fmt_with_ctx.rs:748` (within
   `:736-756`). Update the citation.

7. **[NIT] D-10-K (discussions.md cross-ref).**
   discussions.md D-10-K cross-references "§24.7" for
   pretty-printer deferral; actual section is §24.9.
   §24.7 is bridge translation rules.

**Per-D-tag Lockability:**
- D-10-1: yes (modulo issue 3 inherited enforcement
  wording).
- D-10-2: no with caveats (issue 5 typed-peer count).
- D-10-3: no with caveats (issue 6 Charon citation).
- D-10-4: no (issues 1-2 RPO enforcement +
  reachability).
- D-10-5: no (issues 2-4 walk ownership + density +
  reachability semantics).
- D-10-K: yes (with issue 7 cross-ref nit).

**TOTAL:** 7 issues (2 BLOCK / 4 MINOR / 1 NIT).

**Corrections applied (Round 1 close):**

Issue 1 (BLOCK) -- §24.4.3 walk-impl rewritten to
specify a concrete DFS-from-BlockId(0) +
post-order-reversal procedure that asserts storage
order matches the computed RPO; vacuous "either
back-edge or forward" wording removed.

Issue 2 (BLOCK) -- Reachability promoted from a
non-invariant to a positive invariant **B-Reachable**
in §24.6.2 ("All blocks in `Body.basic_blocks` are
reachable from `BlockId::START_BLOCK` via
`Terminator.successors()`"). The §24.6.5 reachability
bullet removed; B-RPO and B-Reachable are now
walk-checked together by a single DFS-RPO procedure
in §24.4.3 / §24.6.3. discussions.md D-10-5 Part B
updated to add B-Reachable (8 positive invariants
total: 2 inherited + 6 stage-10-introduced); status.md
locked-tag inventory updated.

Issue 3 (MINOR) -- §24.6.3 walk-impl text refined:
Part A rows 1-2 are bridge-enforced at construction
(per §24.7 step 2a-2b); the stage-11 body walk
enforces only Part B invariants (B-RPO + B-Reachable
+ B-Successors-Valid + B-NonEmpty; B-0a / B-0b
inherited from per-Statement / per-Terminator walks;
B-Entry / B-Terminator-Some structurally encoded).

Issue 4 (MINOR) -- §24.6.5 block-ID density bullet
removed (IndexVec is dense by stage-1 §2.6
construction; the bullet was intrinsically
contradictory). §24.6.5 now contains zero
non-invariants; renamed to a "Future considerations"
note pointing forward to potential per-instance
const-folding work in stage 11+.

Issue 5 (MINOR) -- discussions.md D-10-2.1 reasoning
chain step 3 corrected: peer typed-vec count is 2-of-4
(rustc, Charon), not 3-of-4. RegseqModel chooses
typed-IndexVec for stage-1 ID discipline alignment,
not for peer-majority alignment.

Issue 6 (MINOR) -- §24.3.4 Charon citation updated
from `src/pretty/fmt_with_ctx.rs:206-214` to
`src/pretty/fmt_with_ctx.rs:748` (within the
`:736-756` block; the line range covering the actual
`bb{}` label render).

Issue 7 (NIT) -- discussions.md D-10-K cross-ref
"§24.7" -> "§24.9" corrected.

Round 1 closed; ready for Round 2 verification of
fixes.

---

## Round 2 -- 2026-05-05

**Initiated.** Fresh Codex pass with the same prompt
template, framed as Round-2 verification of the 7
Round-1 fixes plus a search for new issues.

**Verdict.** BLOCK.

**Round 1 issue verification:**
- Issue 1 (RPO walk vacuity): RESOLVED.
- Issue 2 (RPO root vs unreachable / B-Reachable
  promotion): **INCOMPLETE** -- the proof citing
  `ReorderBasicBlocks::invert_bijective_mapping` as
  panicking on non-bijection is rustc-source-inaccurate
  (see new issue 1 below).
- Issue 3 (walk ownership of Part A): RESOLVED.
- Issue 4 (block-ID density): RESOLVED.
- Issue 5 (typed-peer count): RESOLVED.
- Issue 6 (Charon citation): RESOLVED.
- Issue 7 (D-10-K cross-ref): RESOLVED.

**New issues from Round 2:**

1. **[BLOCK] D-10-5 / §24.6.2 B-Reachable evidence
   inaccurate.** B-Reachable's evidence claim that
   `ReorderBasicBlocks::invert_bijective_mapping`
   panics on non-bijection (so unreachable blocks
   cannot exist when the pass runs) is wrong:
   `prettify.rs:28-30` may early-return if `rpo` is
   already sorted (bijection inversion never runs);
   `rustc_index::slice::invert_bijective_mapping`
   uses `debug_assert_eq!` over `self.len()`, not
   against `body.basic_blocks.len()`; and
   `reverse_postorder()` is reachable-rooted, so its
   length equals reachable count, not total count.
   Also "required pass" wording for
   `SimplifyCfg::PreOptimizations` is inaccurate
   (`is_required()` returns false; the pass is
   scheduled in `rustc_mir_transform/src/lib.rs:666-680`
   but not required).

2. **[MINOR] D-10-2 / discussions.md stale reasoning
   step 6.** discussions.md D-10-2 reasoning step 6
   still says "Reachability is NOT enforced" and that
   all peers carry unreachable blocks; this
   contradicts the Round-1 promotion of B-Reachable to
   Part B.

3. **[MINOR] D-10-4.3 + outbound expectations stale
   walk-list.** D-10-4.3 locked summary
   (`status.md:118`) and the outbound expectation #2
   in both `status.md:252-255` and
   `discussions.md:1445-1448` enumerate only B-RPO +
   B-Successors-Valid + B-NonEmpty (or only B-RPO +
   B-Entry), not the post-Round-1 joint walk
   (B-RPO + B-Reachable + B-Entry + B-Successors-Valid
   + B-NonEmpty).

4. **[MINOR] §24.4.3 / §24.6.3 walk ordering
   ambiguity.** The DFS-RPO procedure traverses
   `Terminator.successors()` before B-Successors-Valid
   is checked. An out-of-range successor BlockId
   would cause `IndexVec` indexing to panic during
   the DFS visit (rustc traversal pattern at
   `rustc_middle/src/mir/traversal.rs:124-134`),
   turning a clean B-Successors-Valid violation into
   an indexing failure.

5. **[NIT] §24.8 worked example wording.** The
   example's "3-block body" framing with a commented
   `BlockId(2)` is easy to misread as part of the
   example body, which would contradict B-Reachable.
   Suggested: "2-block pruned body" with the
   unreachable-block note outside the array.

**Per-D-tag Lockability (Round 2 state):**
- D-10-1: yes.
- D-10-2: no with caveats (issue 2 stale reasoning).
- D-10-3: yes.
- D-10-4: no with caveats (issue 3 outbound list +
  issue 4 walk ordering).
- D-10-5: no (issue 1 B-Reachable evidence; issue 4
  walk ordering).
- D-10-K: yes.

**TOTAL:** 5 issues (1 BLOCK / 3 MINOR / 1 NIT).

**Corrections applied (Round 2 close):**

Issue 1 (BLOCK) -- B-Reachable evidence in §24.6.2
(spec-snapshot.md) and D-10-5 reasoning chain step 6
(discussions.md) re-grounded: removed the inaccurate
`invert_bijective_mapping` panic claim; B-Reachable
is now justified as "verified by the stage-11
DFS-RPO walk procedure independently of upstream
pipeline assumptions; rustc's standard pipeline
schedules `SimplifyCfg::PreOptimizations` and
`remove_dead_blocks`
(`rustc_mir_transform/src/lib.rs:666-680`;
`rustc_mir_transform/src/simplify.rs:79-85,349-383`)
which prune unreachable blocks at our consumption
boundary, but the spec does not depend on this for
soundness". "Required pass" wording removed.

Issue 2 (MINOR) -- discussions.md D-10-2 reasoning
chain step 6 rewritten: reachability is not stored
as a cache/analysis field (the reason for D-10-2.3's
no-cache decision), but B-Reachable IS a body-level
positive invariant verified at the bridge boundary
by the stage-11 walk.

Issue 3 (MINOR) -- D-10-4.3 locked summary
(`status.md:118`) and outbound expectation #2 in
both `status.md` and `discussions.md` updated to list
the post-Round-1 joint walk: B-RPO + B-Reachable +
B-Entry (DFS-RPO joint procedure) + B-Successors-Valid
+ B-NonEmpty.

Issue 4 (MINOR) -- §24.4.3 walk-impl procedure
restructured to require B-NonEmpty + B-Successors-Valid
pre-checks BEFORE the DFS-RPO traversal, so an
invalid successor surfaces as a clean
B-Successors-Valid diagnostic rather than an
indexing panic. §24.6.3 walk-ownership text
updated to match.

Issue 5 (NIT) -- §24.8 worked example wording
updated: framed as a "2-block well-formed body"
with the unreachable-block note explicitly outside
the array (B-Reachable consequence).

Round 2 closed; ready for Round 3 verification.

---

## Round 3 -- 2026-05-05

**Initiated.** Fresh Codex pass with the same prompt
template, framed as Round-3 verification of the 5
Round-2 fixes plus a search for new issues.

**Verdict.** MINOR.

**Round 2 issue verification:**
- Issue 1 (B-Reachable evidence): NEW-ISSUE-INTRODUCED
  in the form of a stale step reference (Round-2 fix
  cited "step 3a-3b" in §24.6.2, but §24.4.3 has
  numbered steps 1-6; the correct reference is "step
  4 after step-3 DFS-RPO"). The substantive evidence
  fix from Round 2 is correct and accurate; only the
  citation back into §24.4.3 is off.
- Issue 2 (D-10-2 stale reasoning): RESOLVED.
- Issue 3 (outbound walk-list stale): RESOLVED.
- Issue 4 (walk ordering ambiguity): RESOLVED.
- Issue 5 (worked example wording): RESOLVED.

**New / residual issues from Round 3:**

1. **[MINOR] D-10-5 / §24.6.2 stale step reference.**
   Spec text says B-Reachable is verified by
   "§24.4.3 step 3a-3b", but §24.4.3 has numbered
   steps 1-6 (step 3 computes RPO; step 4 asserts
   B-Reachable). Update the cross-ref.

2. **[MINOR] D-10-4.3 reasoning chain doesn't
   mention pre-checks.** discussions.md D-10-4.3
   reasoning step 3 summarizes the DFS-RPO core but
   does not name §24.4.3 steps 1-2 (the B-NonEmpty
   and B-Successors-Valid pre-checks added in
   Round-2 closure). Update the reasoning to
   reflect the full step-1-through-6 procedure.

3. **[NIT] status.md review cadence stale.**
   `status.md` §3 still says "No rounds run yet",
   contradicting the Round-1 / Round-2 closure
   records in review-log.md and discussions.md.
   Update the cadence summary.

**Per-D-tag Lockability (Round 3 state):**
- D-10-1: yes.
- D-10-2: yes.
- D-10-3: yes.
- D-10-4: yes (with minor D-10-4.3 reasoning
  cleanup).
- D-10-5: yes (with minor §24.6.2 step-ref cleanup).
- D-10-K: yes.

**TOTAL:** 3 issues (0 BLOCK / 2 MINOR / 1 NIT).
**All 6 D-tags Lockability: yes.**

**Corrections applied (Round 3 close):**

Issue 1 (MINOR) -- §24.6.2 B-Reachable text updated:
"§24.4.3 step 3a-3b" -> "§24.4.3 step 4, after the
step-3 DFS-RPO computation".

Issue 2 (MINOR) -- discussions.md D-10-4.3 reasoning
chain step 3 rewritten to name §24.4.3 steps 1-2 as
the B-NonEmpty + B-Successors-Valid pre-checks and
steps 3-6 as the DFS-RPO + B-Reachable + B-RPO +
B-Entry joint check; reasoning step 5 added to
explicitly attribute the walk-ordering refinement to
Round-2 closure (Codex issue 4).

Issue 3 (NIT) -- status.md §3 "Codex review cadence"
updated with Round-1 / Round-2 / Round-3 outcomes
and Round-4 pending verification slot.

Round 3 closed; 3 cleanup fixes applied. Per-D-tag
Lockability:yes for all 6 D-tags signals close-out
proximity (matching stage-9 R6 close-out pattern of
MINOR-with-Lockability:yes). Round 4 will verify
Round-3 cleanup fixes; expected to close Phase B.

---

## Round 4 -- 2026-05-05

**Initiated.** Fresh Codex pass framed as a close-out
verification: confirm Round-3 cleanup fixes are
accurate and issue a final per-stage close-out
verdict.

**Verdict.** MINOR.

**Round 3 issue verification:**
- Issue 1 (stale step ref §24.6.2): RESOLVED.
- Issue 2 (D-10-4.3 reasoning incomplete):
  RESOLVED.
- Issue 3 (status.md cadence stale): RESOLVED.

**New / residual issues from Round 4:**

1. **[MINOR] D-10-5 / §24.6.1 SimplifyCfg "required"
   wording.** Rejection-table row 1 evidence still
   says "`SimplifyCfg::PreOptimizations` (required)
   prunes them"; rustc source shows `is_required()`
   returns `false` at `simplify.rs:101-103`. The
   pass is scheduled in runtime cleanup
   (`lib.rs:666-680`) but not "required". Non-load-
   bearing because the bridge still rejects survivors
   at construction; source-accuracy residual only.

2. **[NIT] status.md §0 inbound checklist
   unchecked.** Five inbound items in status.md §0
   are marked `[ ]` even though the same bullets
   say "Closed by D-10-X" and the file's update rule
   says closed inbound items should be marked `[x]`.

**Per-D-tag Lockability (Round 4 state):**
- D-10-1: yes.
- D-10-2: yes.
- D-10-3: yes.
- D-10-4: yes.
- D-10-5: yes (with minor source-wording cleanup).
- D-10-K: yes.

**Phase B close-out (Round 4):** NO with caveats --
2 wording cleanups pending.

**TOTAL:** 2 issues (0 BLOCK / 1 MINOR / 1 NIT).

**Corrections applied (Round 4 close):**

Issue 1 (MINOR) -- §24.6.1 rejection-table row 1
evidence rewritten. Old: "`SimplifyCfg::PreOptimizations`
(required) prunes them." New: "`SimplifyCfg::PreOptimizations`
is scheduled in rustc runtime cleanup
(`rustc_mir_transform/src/lib.rs:666-680`;
`is_required()` returns `false` at `simplify.rs:101-103`)
and prunes them in practice. The bridge still
rejects any survivor at construction."

Issue 2 (NIT) -- All 5 inbound items in status.md §0
marked `[x]` with attribution to the closing D-tag:
- BlockId via newtype_index! -> D-10-3.1
- newtype_index! const-body arm -> D-10-3.2
- bbN sigil rendering -> D-10-K.2 (deferred to
  stage 11; convention pinned in §24.3.4)
- Niche optimization for Option<BlockId> -> D-10-3.3
- Bridge concern RPO inheritance -> D-10-4.1

Round 4 closed; 2 cleanup fixes applied. Round 5 is
the final close-out verification.

---

## Round 5 -- 2026-05-05 (final close-out)

**Initiated.** Fresh Codex pass framed as the final
close-out verification: confirm Round-4 cleanup fixes
are accurate and issue Phase B close-out: YES.

**Verdict.** MINOR. **Phase B close-out: YES.**

**Round 4 issue verification:**
- Issue 1 (§24.6.1 SimplifyCfg "required" wording):
  RESOLVED. §24.6.1 row 1 now correctly says the
  pass is scheduled, not required; cites
  `rustc_mir_transform/src/lib.rs:666-680` for
  scheduling and `rustc_mir_transform/src/simplify.rs:101-103`
  for `is_required() == false`.
- Issue 2 (status.md §0 inbound checklist
  unchecked): RESOLVED. All 5 inbound items are now
  `[x]` with D-tag attributions: D-10-3.1, D-10-3.2,
  D-10-K.2, D-10-3.3, D-10-4.1.

**New / residual issues from Round 5
(post-close-out cleanup only):**

1. **[MINOR] D-10-3.4 stale Charon citation in
   discussions.md.** discussions.md D-10-3.4
   reasoning step 1 still cites Charon's `bb{}`
   label at `src/pretty/fmt_with_ctx.rs:206-214`
   (which renders block contents only); the
   corrected citation `:748` (within `:736-756`)
   was applied at Round-1 closure to spec-snapshot.md
   §24.3.4 and prior-art/charon.md but missed
   discussions.md.

2. **[NIT] status.md D-tag table missing D-10-K
   sub-decision rows.** discussions.md locks
   D-10-K.1 / K.2 / K.3 as separate sub-decisions;
   status.md D-tag table has only the parent D-10-K
   row. Sub-rows should be added; total locks count
   updated from 24 (6 + 18) to 27 (6 + 21).

**Per-D-tag Lockability (Round 5 final):**
- D-10-1: yes.
- D-10-2: yes.
- D-10-3: yes (with D-10-3.4 citation cleanup).
- D-10-4: yes.
- D-10-5: yes.
- D-10-K: yes (with status-table bookkeeping nit).

**Phase B close-out: YES.**

**TOTAL:** 2 issues (0 BLOCK / 1 MINOR / 1 NIT).

**Corrections applied (Round 5 close):**

Issue 1 (MINOR) -- discussions.md D-10-3.4 reasoning
step 1 Charon citation updated from
`src/pretty/fmt_with_ctx.rs:206-214` to
`src/pretty/fmt_with_ctx.rs:748` (within `:736-756`),
matching the spec-snapshot.md §24.3.4 and
prior-art/charon.md citations.

Issue 2 (NIT) -- status.md D-tag table extended with
D-10-K.1 / K.2 / K.3 sub-decision rows; total locks
count corrected from 24 (6 + 18) to 27 (6 + 21).

**Round 5 closed; per-stage Phase B CLOSED.**

---

## Per-stage review trajectory summary

5 rounds total; 19 distinct findings closed (3 BLOCK
+ 12 MINOR + 4 NIT) across the iteration:

- **Round 1** (BLOCK): 7 issues (2B+4M+1N) -- initial
  spec-snapshot review. Issues spanned RPO walk
  vacuity, RPO-vs-unreachable inconsistency, walk
  ownership of Part A, IndexVec density wording,
  typed-peer count off-by-one, Charon citation, D-10-K
  cross-ref.
- **Round 2** (BLOCK): 5 issues (1B+3M+1N) -- 6 of 7
  Round-1 fixes RESOLVED; 1 NEW-ISSUE (Round-1
  B-Reachable evidence used a flawed
  `invert_bijective_mapping` panic claim).
- **Round 3** (MINOR): 3 issues (0B+2M+1N) -- all 5
  Round-2 fixes RESOLVED; per-D-tag Lockability:yes
  for all 6 D-tags. 1 NEW-ISSUE (stale step ref in
  Round-2 §24.6.2 fix).
- **Round 4** (MINOR): 2 issues (0B+1M+1N) -- all 3
  Round-3 fixes RESOLVED; Phase B close-out: NO with
  caveats pending 2 wording cleanups.
- **Round 5** (MINOR + close-out: YES): 2 issues
  (0B+1M+1N) -- both Round-4 fixes RESOLVED; **Phase
  B close-out: YES.** 2 post-close-out bookkeeping
  cleanups applied (stale Charon citation in
  discussions.md, missing D-10-K sub-decision rows
  in status.md).

**Most load-bearing findings across the trajectory:**

1. **R1 issue 2 -> R2 issue 1 (B-Reachable
   promotion + evidence re-grounding).** The
   "reachability NOT enforced" non-invariant
   conflicted with B-RPO's rooted-CFG semantics;
   resolution promoted reachability to a positive
   invariant (B-Reachable) verified by the stage-11
   walk. Round-2 surfaced that the initial
   `invert_bijective_mapping` panic-on-non-bijection
   evidence was rustc-source-inaccurate; the
   corrected evidence grounds B-Reachable on
   walk verification + informational rustc-pipeline
   scheduling (`SimplifyCfg::PreOptimizations` +
   `remove_dead_blocks`).

2. **R1 issue 1 + R2 issue 4 (walk-impl rewrite).**
   The original §24.4.3 walk-impl said "every
   successor j is either back-edge or forward",
   which is vacuously true. Round 1 replaced it
   with a concrete DFS-RPO procedure; Round 2
   added B-NonEmpty + B-Successors-Valid pre-checks
   so the DFS cannot indexing-panic on an
   out-of-range successor.

3. **R1 issue 3 (walk ownership of Part A).**
   Originally the spec assigned Part-A enforcement
   to the stage-11 walk, but Part-A rows 1-2
   inspect rustc-side fields gone after bridge
   construction. Resolution: Part A is
   bridge-enforced; the walk handles Part B only.
   RegseqModel's `BasicBlock` shape makes Part-A
   conditions structurally inexpressible
   post-construction.

The trajectory is coherent: issue count strictly
decreasing (7 -> 5 -> 3 -> 2 -> 2), no BLOCK reopened
past Round 2, all Lockability `yes` since Round 3.
Comparable to stage-9's per-stage portion (6 rounds
+ 6 post-merge = 12 total; 51 findings; stage-10's
per-stage trajectory is tighter because the surface
is structural rather than vocabulary).
