# Stage 3 SESSION-C -- review log (addendum)

This file is a session-local addendum to `../../review-log.md`. It
records the Codex adversarial-review rounds run for SESSION-C's
decisions (D9, D10, D13, D16) against the session-local addendum
documents (`vocabulary-SESSION-C.md`, `discussions-SESSION-C.md`)
read together with the shared stage-3 documents for context.

Reviews are invoked via `scripts/python/codex_runner.py` per the
project's CLAUDE.md contract. Each round uses a fresh prompt; per
the stage-3 workflow (`../memory.md` Section 4), each Codex prompt
provides the reviewer with:

- The shared stage-3 spec (`../../vocabulary.md`) for context on
  locked decisions.
- The shared discussions record (`../../discussions.md`) for
  decision history.
- The shared review log (`../../review-log.md`) for prior-round
  rationale.
- This session's addendums (`vocabulary-SESSION-C.md`,
  `discussions-SESSION-C.md`) -- the actual material under review.
- This file (`review-log-SESSION-C.md`) for prior SESSION-C rounds.

The structured output contract is the same as the shared review log:
- First line: `VERDICT: <BLOCK | MINOR | APPROVE>`
- Numbered issues list, each tagged `[BLOCK | MINOR | NIT]` with a
  decision reference, file:line citation, 1-3-sentence description,
  and a 1-sentence suggested fix.
- Last line: `TOTAL: <N> issues (<count of BLOCK> BLOCK / <count of
  MINOR> MINOR / <count of NIT> NIT)`.

Rounds in this file correspond to **Round 3** of the stage-3 review
cadence (per `../stage-3-status.md` "Codex review cadence (status)"),
the cluster covering D5-D10 + D13-D16 details, restricted to
SESSION-C's slice (D9 + D10 + D13 + D16). Sub-rounds are numbered
3.<n>.

After all parallel sessions reach APPROVE on their own slices, the
merge session integrates the addendums into the shared docs and
runs Round 4 (the comprehensive cross-cluster sweep) against the
merged spec.

---

## Round 3.1 -- D9 + D10 + S0 framing initial review

**Date.** 2026-04-29.

**Scope.** SESSION-C operation/memory-effects sub-cluster:
- S0 (no-QualType-layer framing principle) in `vocabulary-SESSION-C.md`.
- S1 + S1.1 (D9 volatile -- decision body + worked example).
- S2 + S2.1 (D10 atomics -- decision body + worked example).
- D9 + D10 distilled records in `discussions-SESSION-C.md`.

D13 and D16 were explicitly out of scope (still placeholder in this
session). D5/D6/D7/D8/D14/D15 (other parallel sessions' work) were
out of scope.

**Inputs to Codex.**
- Under review: `tmp/SESSION-C/vocabulary-SESSION-C.md`,
  `tmp/SESSION-C/discussions-SESSION-C.md`.
- Required context (read-only): shared `vocabulary.md`,
  `discussions.md`, `review-log.md`, `findings-rustc-draft.md`,
  `expectations.md`, all 7 `prior-art/*.md` Theme D entries,
  `regseq-background.md`, `architecture.md` Q7, `context.md`,
  `stage-1/vocabulary.md` lines 869-899,
  `tests/fixtures/pl011-full-spec/src/regseq/mmio.rs:35-71`,
  `stage-13/expectations.md`.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r1.txt`. Background-launched.

**Verdict.** `BLOCK` -- 3 BLOCK + 6 MINOR + 1 NIT (10 total).
(Codex's "TOTAL" line miscounted as 4 BLOCK / 5 MINOR / 1 NIT;
the explicit issue list is authoritative.)

**Issues.**

1. *[BLOCK] S0-locked-overclaim
   (`vocabulary-SESSION-C.md:40`).* S0's qualifier-to-IR-site
   table column header read "Locked by", but the `const` and
   `restrict` rows depend on D7/D8 -- still open in the shared
   `discussions.md:472,479`.

2. *[BLOCK] stage13-citation-overclaim
   (`vocabulary-SESSION-C.md:627`).* The addendum said
   `stage-13/expectations.md` owns atomic intrinsic vocabulary,
   ordering encoding, and D9/D10 rejection enforcement; the
   file currently records only the closed-vocabulary mechanism
   plus bridge-side recognition surface (lines 12-17, 50-58).

3. *[BLOCK] unresolved-relative-paths
   (`discussions-SESSION-C.md:117`,
   `discussions-SESSION-C.md:250`,
   `vocabulary-SESSION-C.md:876`).* `../../stage-1/...`,
   `../../architecture.md`, and `../../stage-13/expectations.md`
   resolve from `tmp/SESSION-C/` to `stage-3/<file>` (wrong);
   correct path is `../../../<file>`.

4. *[MINOR] methodology-path
   (`vocabulary-SESSION-C.md:11,865`).* `../../context.md`
   misresolves; correct is `../../../context.md`. (Same class
   as issue 3.)

5. *[MINOR] ordering-resolution-overclaim
   (`vocabulary-SESSION-C.md:515`).* D10 reasoning sub-question 3
   said atomic ordering is "fully resolved" before the bridge and
   "const-evaluates pre-bridge", but the facade sketch shows
   ordering as an ordinary value parameter; the IR keeps
   `Ordering::Acquire` as an operand. Internally inconsistent.

6. *[MINOR] background-citation-overclaim
   (`vocabulary-SESSION-C.md:277`).* Cited
   `regseq-background.md:14-27` as naming `mmio_read`/`mmio_write`,
   but those lines establish the three-mechanism convention with
   `busy_wait`, `sleep`, `poll_until` as illustrative examples;
   the `mmio_*` names come from the pl011 fixture only.

7. *[MINOR] charon-volatile-overclaim
   (`vocabulary-SESSION-C.md:235`).* Charon Theme D supports the
   no-type-level-marker pattern for atomic / `UnsafeCell` /
   `MaybeUninit` / `PhantomData` markers (lines 609, 664), but
   does not specifically address volatile.

8. *[MINOR] compcert-atomic-lines
   (`vocabulary-SESSION-C.md:601`).* D10 cited
   `prior-art/compcert.md` lines 415-434 for atomics, but those
   lines are about volatile; the `_Atomic` evidence is at lines
   436-451.

9. *[MINOR] mlir-atomic-citation
   (`vocabulary-SESSION-C.md:597`).* D10 claimed MLIR uses
   dedicated dialect ops (`arith.atomic_*`, `memref.atomic_rmw`)
   citing `prior-art/mlir.md:365-405`, but those lines describe
   type parameters/interfaces/traits generally; MLIR's prior-art
   file has no atomic-specific evidence.

10. *[NIT] pl011-bit-comment
    (`vocabulary-SESSION-C.md:324`).* Comment "bit 5 = TXFE / TXFF
    in pl011" is internally ambiguous (TXFE and TXFF are different
    flags).

**Corrections applied.**

- *Issue 1.* Changed S0 table column header from "Locked by" to
  "Source / status"; tagged each row with explicit
  LOCKED / OPEN status; added a paragraph after the table
  explaining the principle is robust to D7/D8's locked outcome.
- *Issue 2.* Narrowed both D9 and D10 stage-13 citations to
  what `stage-13/expectations.md` (lines 12-17, 50-58) currently
  records; explicitly noted the atomic-vocabulary, ordering-
  encoding, and Call-translation enforcement entries as outbound
  forward refs that the merge session should land into
  `stage-13/expectations.md`. Added a top-level merge note
  (item 2) recording these tasks.
- *Issues 3 + 4.* Replaced all `../../context.md`,
  `../../architecture.md`, `../../stage-1/vocabulary.md`,
  `../../stage-13/expectations.md` with `../../../<file>` across
  both addendum files.
- *Issue 5.* Rewrote D10 reasoning sub-question 3 to say "memory
  ordering is operation-level metadata; stage 13 picks the
  encoding shape (per-(op × ordering) InstanceIds vs
  const-generic ordering on a single InstanceId per op)";
  removed the "fully resolved pre-bridge / const-evaluates
  pre-bridge" phrasing.
- *Issue 6.* Split D9 + D10 DSL evidence: cite
  `regseq-background.md:14-27` only for the
  three-mechanism / operational-verbs *category*; cite
  `tests/fixtures/pl011-full-spec/src/regseq/mmio.rs:35-71` for
  the specific MMIO intrinsic names.
- *Issue 7.* Rewrote D9's Charon prior-art bullet to scope
  Charon's contribution to atomic/marker types (not volatile);
  qualified that the no-volatile-marker-in-TyKind absence is
  suggestive but not load-bearing. Replaced D9's "6/7 surveyed
  projects" convergence count with explicit project enumeration.
- *Issue 8.* Updated D10 CompCert citation line range from
  415-434 to 436-451; clarified that volatile-specific lines
  are 415-434 (separate from atomics).
- *Issue 9.* Weakened D10 MLIR bullet to the general "special
  semantics off type" pattern (lines 365-405) and explicitly
  noted that the file has no atomic-specific evidence; the
  inference about `arith.atomic_*` / `memref.atomic_rmw` is
  marked as plausible but not load-bearing for D10. Replaced
  D10's "7/7" convergence claim with a softer enumeration that
  acknowledges CompCert's atomics-N/A status and MLIR's
  general-pattern-only contribution.
- *Issue 10.* Replaced ambiguous `// bit 5 = TXFE / TXFF in
  pl011` with `// status-bit check (illustrative)`; the
  type-system point doesn't depend on the specific flag name.

The substantive D9 + D10 + S0 locks are unchanged. All
corrections are citation-precision and scoping fixes; none
disturbed the core decisions.

**Status.** Round 3.2 prepared; pending Codex re-review.

---

## Round 3.2 -- D9 + D10 + S0 framing re-review after Round-3.1 fixes

**Date.** 2026-04-29.

**Scope.** Same as Round 3.1. Round 3.2 prompt asked Codex to
(a) verify each Round-3.1 fix via the cited evidence, (b) check
for new issues introduced by the corrections, (c) sweep for
remaining issues Round 3.1 may have missed, (d) coverage gaps.

**Inputs to Codex.** Same as Round 3.1, plus a Round-3.1
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r2.txt`. Background-launched.

**Verdict.** `BLOCK` -- 1 BLOCK + 4 MINOR + 0 NIT (5 total).
Down from Round 3.1's 10 issues. Verdict trajectory: BLOCK
(R3.1, 10) -> BLOCK (R3.2, 5).

**Issues.**

1. *[BLOCK] stage13-citation-overclaim residual
   (`discussions-SESSION-C.md:123`,
   `discussions-SESSION-C.md:256`).* Round 3.1 issue 2 was fixed
   in `vocabulary-SESSION-C.md` but the same narrowing did not
   propagate to the cross-refs in `discussions-SESSION-C.md`. D9
   and D10 cross-refs there still describe
   `stage-13/expectations.md` as owning Call-translation
   rejection enforcement and the atomic intrinsic vocabulary /
   ordering encoding; the file actually records only the
   Q7-flavoured closed-vocabulary mechanism (`stage-13/expectations.md`
   lines 12-17, 50-58).

2. *[MINOR] ordering-resolution-overclaim residual
   (`vocabulary-SESSION-C.md:614`,
   `discussions-SESSION-C.md:226 area`).* Round 3.1 issue 5 was
   fixed in D10 sub-question 3, but the same overclaim survived
   in two other places: D10's methodology-rule bullet "Don't redo
   work the rustc compiler has already done" still asserted
   "Memory ordering is resolved pre-bridge", and the
   discussions-side D10 reasoning still said ordering is "fully
   resolved in rustc's MIR before the bridge sees it". Both now
   conflict with the corrected operation-level-metadata framing.

3. *[MINOR] compcert-atomic-lines residual
   (`discussions-SESSION-C.md:266`).* Round 3.1 issue 8 fixed
   the line range in `vocabulary-SESSION-C.md` but the same
   citation in `discussions-SESSION-C.md` still pointed at
   415-434 (volatile lines) for atomics; the `_Atomic` evidence
   is at 436-451.

4. *[MINOR] atomic-facade-forward-ref-overclaim
   (`vocabulary-SESSION-C.md:698`).* The D10 DSL-evidence bullet
   said in present tense that `regseq::Atomic<T>::load`,
   `::store`, `::fetch_add` "carry the same attribute" as the
   pl011 fixture's `mmio.rs` methods, but the same addendum says
   the exact `Atomic<T>` API is illustrative-and-stage-13-
   territory. New issue introduced when the round-3.1 stage-13
   narrowing exposed the planned-vs-existing tension.

5. *[MINOR] linux-atomic-worked-example-storage
   (`vocabulary-SESSION-C.md:845-872`).* Linux C lowering wrote
   `return atomic_read_acquire((const atomic_t *)&counter->value);`
   while the C-side `Atomic<u32>` storage is declared as plain
   `uint32_t value`. Type-punning a `const uint32_t *` to
   `const atomic_t *` is not valid in general (`atomic_t` is an
   opaque struct wrapping `int`).

**Corrections applied.**

- *Issue 1.* Updated both `discussions-SESSION-C.md` cross-ref
  blocks (D9 at line 117-125, D10 at line 252-265) to mirror the
  narrowed stage-13 citation pattern from the vocabulary file:
  cite `stage-13/expectations.md` lines 12-17 and 50-58 for what
  it currently records, and explicitly mark the MMIO/atomic
  intrinsic-vocabulary entries, ordering encoding, and
  Call-translation rejection enforcement as outbound forward
  refs the merge session should land into stage-13 expectations.

- *Issue 2.* Rewrote both occurrences. Vocabulary methodology
  bullet now says "Memory ordering is operation-level metadata
  that survives at the operation/InstanceId level; stage 13
  chooses the encoding shape." Discussions D10 reasoning
  rewrote the "Memory ordering (Q3)" paragraph the same way.

- *Issue 3.* Updated `discussions-SESSION-C.md` D10 cross-ref
  CompCert line range from 415-434 to 436-451; clarified that
  volatile-specific lines are 415-434 (separate).

- *Issue 4.* Rephrased the D10 DSL-evidence Q7 bullet from
  present tense ("carry the same attribute") to future tense
  ("will carry the `#[rseq::intrinsic("atomic_*")]` attribute
  pattern"); explicitly noted that the precise method set,
  generic bounds, and ordering encoding are stage-13 territory;
  added a sentence that D10's stage-3 lock commits to the
  *posture* (DSL-defined facade with intrinsic-attribute
  methods) but not to the facade's exact API shape.

- *Issue 5.* Replaced the Linux lowering with the canonical
  Linux non-RMW Acquire idiom for plain memory locations:
  `return smp_load_acquire(&counter->value);`. Added a prose
  note explaining why `smp_load_acquire` is preferred over the
  invalid `atomic_t`-cast form, and noting that if a future
  codegen choice prefers `atomic_t`-typed storage (to enable
  RMW ops like `atomic_add_acquire`), the storage shape would
  change to `atomic_t value` -- a stage-13 per-target
  decision, not a stage-3 lock.

The substantive D9 + D10 + S0 locks remain unchanged after
Round 3.2 corrections.

**Status.** Round 3.3 prepared; pending Codex re-review.

---

## Round 3.3 -- D9 + D10 + S0 framing re-review after Round-3.2 fixes

**Date.** 2026-04-29.

**Scope.** Same as Round 3.1 / Round 3.2. Re-review prompt asked
Codex to verify each Round-3.2 fix, check for new issues from
the corrections, sweep for residuals, and check coverage gaps.

**Inputs to Codex.** Same as prior rounds, plus a Round-3.2
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r3.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 3 MINOR + 0 NIT (3 total).
Down from Round 3.2's 5 issues. Verdict trajectory: BLOCK
(R3.1, 10) -> BLOCK (R3.2, 5) -> MINOR (R3.3, 3). All
substantive Round-3.2 fixes verified by Codex.

**Issues.**

1. *[MINOR] d10-rejection-scope-overbroad
   (`vocabulary-SESSION-C.md:718, 957`;
   `discussions-SESSION-C.md:268`).* D10's main policy text
   says the bridge rejects `core::sync::atomic::*` *types and
   their methods* plus `core::intrinsics::atomic_*` (correct,
   with the type/method split made explicit at S2 D10
   discussion sub-question 2 and explicitly excluding
   `core::sync::atomic::Ordering`). But the stage-13
   forward-ref bullet, the S5 status row, and the
   discussions-SESSION-C cross-ref still phrased the rejection
   as broad "rejection of `core::sync::atomic::*`" and framed
   enforcement uniformly as "Call-translation enforcement",
   without the type-vs-method split or the `Ordering` exclusion.
   Inconsistent with the more precise main-policy text.

2. *[MINOR] zephyr-atomic-get-type-mismatch
   (`vocabulary-SESSION-C.md:897`).* The Zephyr worked-example
   comment said `atomic_get(&counter->value)` "would also work,
   but pessimises Acquire to SeqCst". This had the same
   storage-shape problem the Linux fix removed: `atomic_get`
   takes `atomic_t *` (long, word-sized per
   `include/zephyr/sys/atomic_types.h`), so it is not
   type-compatible with the plain `uint32_t value` storage
   declared in the worked example.

3. *[MINOR] linux-ordering-exactness-overclaim
   (`vocabulary-SESSION-C.md:761`;
   `discussions-SESSION-C.md:225`).* D10 claimed Linux
   "preserves Rust's ordering exactly" via the suffix family,
   and the discussions side said "full ordering lattice on
   Linux". The cited Linux suffix set is
   `_relaxed`/`_acquire`/`_release`/unsuffixed (= fully
   ordered) -- four levels, while the regseq DSL `Ordering`
   set has five (Relaxed/Acquire/Release/AcqRel/SeqCst).
   `AcqRel` does not map to a Linux suffix and must be
   conservatively strengthened to fully-ordered. The
   "preserves exactly" claim was therefore over-strong.

**Corrections applied.**

- *Issue 1.* Rewrote the D10 stage-13 forward-ref bullet
  (`vocabulary-SESSION-C.md:715-733`) to enumerate the
  rejection's structural decomposition: (a) type-translation
  rejection of the `core::sync::atomic::Atomic*` type family
  (`AtomicU32`, `AtomicBool`, `AtomicI64`, `AtomicPtr<T>`,
  ...), (b) Call-translation rejection of methods on those
  types and of the rustc-internal `core::intrinsics::atomic_*`
  family, with (c) `core::sync::atomic::Ordering` (value-level
  enum) explicitly *not* in the rejection scope. Updated the
  S5 D10 row to mirror the type-vs-method split and the
  Ordering exclusion. Updated the top-level merge note (item
  2) outbound forward-ref text to use the same structural
  decomposition. Updated the discussions-SESSION-C.md D10
  cross-ref to mirror the same phrasing.

- *Issue 2.* Rewrote the Zephyr worked-example comment to
  say: for the plain `uint32_t` storage shown, only the GCC
  `__atomic_*` builtin works directly; Zephyr's `atomic_get`
  is not type-compatible with `uint32_t value` storage (it
  takes `atomic_t *`, long, word-sized); a different storage
  shape (e.g., `atomic_t value;` declared per stage-13's
  per-target choice) would be required to use it; even given
  compatible storage, Zephyr's public `atomic_get` is
  SeqCst-only and would pessimise Acquire to SeqCst.

- *Issue 3.* Rewrote the "implication for stage 13" prose in
  S2 D10 (`vocabulary-SESSION-C.md:776-790`) to drop the
  "preserves Rust's ordering exactly" claim. The new text
  states: stage-13 codegen for each target must define a
  per-op mapping from the regseq `Ordering` set
  (`Relaxed`/`Acquire`/`Release`/`AcqRel`/`SeqCst`) into the
  target's C-side surface, preserving Rust ordering where
  the target supports it directly (Linux suffixes for
  Relaxed/Acquire/Release; unsuffixed for SeqCst) and
  conservatively strengthening where it does not (AcqRel ->
  unsuffixed fully-ordered, since Linux has no AcqRel
  suffix). Zephyr handled the same way: GCC `__atomic_*`
  preserves exactly; public API pessimises to SeqCst.
  Mirrored the same correction in the discussions-side D10
  reasoning's "Per-target codegen latitude" bullet.

The substantive D9 + D10 + S0 locks remain unchanged after
Round 3.3 corrections. All three Round-3.3 issues were
precision/scoping fixes; none disturbed the core decisions.

**Status.** Round 3.4 prepared; pending Codex re-review.

---

## Round 3.4 -- D9 + D10 + S0 framing re-review after Round-3.3 fixes

**Date.** 2026-04-29.

**Scope.** Same as prior rounds. Same evaluation criteria.

**Inputs to Codex.** Same as prior rounds, plus a Round-3.3
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r4.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 2 MINOR + 0 NIT (2 total).
Down from Round 3.3's 3 issues. Verdict trajectory: BLOCK
(R3.1, 10) -> BLOCK (R3.2, 5) -> MINOR (R3.3, 3) ->
MINOR (R3.4, 2). Both R3.4 issues are residuals where R3.3
fixes did not propagate to every site of the same claim.

**Issues.**

1. *[MINOR] d10-rejection-scope-overbroad-residual
   (`vocabulary-SESSION-C.md:64, 954`).* The R3.3 structural
   decomposition (type-translation rejection of
   `core::sync::atomic::Atomic*` types; Call-translation
   rejection of methods + `core::intrinsics::atomic_*`;
   Ordering excluded) was correctly added in the merge-note
   bullet at line 45 and 52, in S5 at line 992, and in the
   D10 stage-13 forward-ref block. But the same merge note's
   tail at line 64 still asked stage 13 to record
   "Call-translation rejection enforcement for both D9 and
   D10", dropping D10's type-translation half. The worked
   example at line 954 ("Rejected counter-example") prose
   said rejection applies to "the entire `core::sync::atomic::*`
   namespace and its methods", conflicting with the
   `Ordering` exclusion at line 513.

2. *[MINOR] linux-ordering-exactness-residual
   (`vocabulary-SESSION-C.md:588`).* The R3.3 corrected prose
   at line 776/783 stated the AcqRel strengthening explicitly
   and mirrored it on the discussions side (line 224). But
   the earlier D10 reasoning bullet at line 588 ("Per-target
   codegen latitude is unaffected") still said "full ordering
   lattice on Linux via the `_relaxed`/`_acquire`/`_release`
   suffix family", which is the same Round-3.3 overclaim --
   that suffix set does not cover AcqRel.

**Corrections applied.**

- *Issue 1.* Replaced the merge-note item-2 tail ("the
  Call-translation rejection enforcement for both D9 and D10")
  with an explicit per-decision decomposition: D9's
  Call-translation rejection of
  `core::ptr::{read,write}_volatile`, plus D10's two-phase
  rejection (type-translation rejection of the
  `core::sync::atomic::Atomic*` type family; Call-translation
  rejection of methods on those types and of
  `core::intrinsics::atomic_*`; with `Ordering` explicitly
  *not* in scope). Replaced the worked-example "applied to the
  entire `core::sync::atomic::*` namespace" prose with the
  same structural decomposition: type-translation rejection of
  the `core::sync::atomic::Atomic*` type family, plus
  Call-translation rejection of methods and of
  `core::intrinsics::atomic_*`; explicitly noted that the
  diagnostic on the counter-example arises from the
  `&AtomicU32` parameter type (not from the `Ordering::Acquire`
  argument).

- *Issue 2.* Rewrote the D10 reasoning sub-question 2 bullet
  "Per-target codegen latitude is unaffected" at line 588 to
  match the R3.3-corrected discussions-side wording: Linux's
  `atomic_*` family covers Relaxed/Acquire/Release directly
  via the `_relaxed`/`_acquire`/`_release` suffixes plus an
  unsuffixed (fully ordered) form for SeqCst, with AcqRel
  conservatively strengthened to unsuffixed (Linux has no
  dedicated AcqRel suffix); Zephyr's public API is SeqCst-only,
  with `__atomic_*` builtin bypass to preserve non-SeqCst
  orderings.

The substantive D9 + D10 + S0 locks remain unchanged after
Round 3.4 corrections.

**Status.** Round 3.5 prepared; pending Codex re-review.

---

## Round 3.5 -- D9 + D10 + S0 framing re-review after Round-3.4 fixes

**Date.** 2026-04-29.

**Scope.** Same as prior rounds. Round-3.5 prompt explicitly
asked Codex to do a cross-site consistency sweep this time
(after R3.4 was bitten by partial propagation).

**Inputs to Codex.** Same as prior rounds, plus a Round-3.4
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r5.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 2 MINOR + 0 NIT (2 total).
Verdict trajectory: BLOCK (R3.1, 10) -> BLOCK (R3.2, 5) ->
MINOR (R3.3, 3) -> MINOR (R3.4, 2) -> MINOR (R3.5, 2). Same
two issue classes as R3.4, but caught at additional sites
the R3.4 fixes had not reached.

**Issues.**

1. *[MINOR] d10-rejection-scope-overbroad-residual
   (`discussions-SESSION-C.md:171`,
   `discussions-SESSION-C.md:161-168`,
   `vocabulary-SESSION-C.md:551-552`,
   `vocabulary-SESSION-C.md:631-633`,
   `vocabulary-SESSION-C.md:959-961`).* The structural
   decomposition (type-translation rejection of
   `core::sync::atomic::Atomic*` types; Call-translation
   rejection of methods + `core::intrinsics::atomic_*`;
   Ordering excluded) was correctly added in the merge note,
   the S5 row, the D10 stage-13 forward-ref block, and the
   D10 Decision row's body. But the same broad-shorthand
   `core::sync::atomic::*`-rejection language survived in
   the discussions-side D10 Decision sentence, the option
   framing (Options 1/2/3), the vocabulary D10 ingest-policy
   sub-heading, the methodology bullet for "no defensive
   fallbacks", and the worked-example "Rejected
   counter-example" code comment.

2. *[MINOR] linux-ordering-exactness-residual
   (`vocabulary-SESSION-C.md:763-769`,
   `discussions-SESSION-C.md:247-251`).* The corrected prose
   stating the four Linux ordering surfaces and the AcqRel
   strengthening landed at vocabulary-SESSION-C.md:776/783
   (and was mirrored on the discussions side at line 224).
   But the Linux external-evidence bullet at vocabulary-
   SESSION-C.md:763-769 still said "Operations carry the full
   ordering lattice via suffix:", and the discussions-side
   Linux/Zephyr summary at line 247-251 still said "Linux
   uses ... with a suffix-driven ordering lattice."

**Corrections applied.**

- *Issue 1.* Six sites updated:
  - vocabulary-SESSION-C.md D10 Decision body (the "bridge
    rejects" paragraph): rewrote the rejection to a two-phase
    bullet list (type-translation phase enumerating every
    `core::sync::atomic::Atomic*` member; Call-translation
    phase enumerating the method families and the rustc-
    internal intrinsic family) plus an explicit "not in the
    rejection scope: `core::sync::atomic::Ordering`" line.
  - vocabulary-SESSION-C.md sub-heading (Q2 ingest policy):
    rewrote to "the `core::sync::atomic::Atomic*` type family
    + their methods + `core::intrinsics::atomic_*` are
    rejected (Ordering excluded; see decomposition above)";
    inside the bullet, replaced "Admitting `core::sync::atomic::*`"
    with "Admitting the `core::sync::atomic` operational API".
  - vocabulary-SESSION-C.md methodology bullet "no defensive
    fallbacks": replaced "rejects raw `core::sync::atomic::*`"
    with "rejects the `core::sync::atomic::Atomic*` type
    family and methods plus the rustc-internal
    `core::intrinsics::atomic_*` family (Ordering excluded)".
  - vocabulary-SESSION-C.md worked-example rejected
    counter-example comment: replaced
    "core::sync::atomic::* rejection scope" with "type-
    translation rejection: AtomicU32 is in the
    core::sync::atomic::Atomic* type-family rejection list".
  - discussions-SESSION-C.md D10 Decision: rewrote to enumerate
    the structural decomposition (type-translation rejection
    of the full `Atomic*` type-family list, Call-translation
    rejection of methods and `core::intrinsics::atomic_*`,
    Ordering excluded) before stating the regseq DSL surface.
  - discussions-SESSION-C.md option framing: prefixed the
    block with a parenthetical noting that each option below
    is the broad alternative considered and that the locked
    decision narrows it structurally; replaced
    "core::sync::atomic::*" with "the core::sync::atomic
    operational API" inside Options 1/2/3.
  - discussions-SESSION-C.md Q2 question framing
    (`discussions-SESSION-C.md:151-154`): replaced
    `core::sync::atomic::*'s public API` with a more precise
    description that names the Atomic* types, their methods,
    and the rustc-internal `core::intrinsics::atomic_*`
    family.

- *Issue 2.* Two sites updated:
  - vocabulary-SESSION-C.md Linux external-evidence bullet:
    "Operations carry the full ordering lattice via suffix"
    rewritten to "Operations expose four ordering surfaces
    via suffix -- `_relaxed` (Relaxed), `_acquire` (Acquire),
    `_release` (Release), and unsuffixed (fully ordered, used
    for SeqCst and -- conservatively, since no Linux suffix
    maps to it directly -- for AcqRel)".
  - discussions-SESSION-C.md Linux/Zephyr summary: replaced
    "with a suffix-driven ordering lattice" with the four-
    surface mapping plus the AcqRel-strengthens-to-unsuffixed
    note.

Final grep sweep across both files confirms no remaining
`core::sync::atomic::*` broad-shorthand sites and no
`full ordering lattice` / `suffix-driven ordering lattice`
phrasings.

The substantive D9 + D10 + S0 locks remain unchanged after
Round 3.5 corrections.

**Status.** Round 3.6 prepared; pending Codex re-review.

---

## Round 3.6 -- D9 + D10 + S0 framing re-review after Round-3.5 fixes

**Date.** 2026-04-29.

**Scope.** Same as prior rounds. Round-3.6 prompt explicitly
asked Codex to verify Round-3.5 fixes at every flagged site
plus an independent cross-site consistency sweep.

**Inputs to Codex.** Same as prior rounds, plus a Round-3.5
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r6.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
Verdict trajectory: BLOCK (R3.1, 10) -> BLOCK (R3.2, 5) ->
MINOR (R3.3, 3) -> MINOR (R3.4, 2) -> MINOR (R3.5, 2) ->
MINOR (R3.6, 1). Round-3.5 fixes verified at every flagged
site; the new issue is a real factual omission in the atomic
family enumeration.

**Issues.**

1. *[MINOR] d10-atomic-family-enumeration-incomplete
   (`vocabulary-SESSION-C.md:516-518`,
   `discussions-SESSION-C.md:181-184`).* Both the vocabulary
   D10 Decision body's "Members:" list and the discussions-
   side D10 Decision body enumerate the
   `core::sync::atomic::Atomic*` family as `AtomicU32`,
   `AtomicI32`, `AtomicU64`, `AtomicI64`, `AtomicUsize`,
   `AtomicIsize`, `AtomicU8`, `AtomicI8`, `AtomicU16`,
   `AtomicI16`, `AtomicBool`, `AtomicPtr<T>` -- but omit
   `AtomicI128` and `AtomicU128`, which exist in the pinned
   toolchain at `library/core/src/sync/atomic.rs:3884-3920`
   under `cfg(target_has_atomic_load_store = "128")`.
   Authoritative rejection-scope claim was therefore
   incomplete.

**Corrections applied.**

- *Issue 1.* Added `AtomicU128` and `AtomicI128` to both
  enumerations, with a parenthetical noting the 128-bit
  variants are cfg-gated on
  `target_has_atomic_load_store = "128"` per the cited
  toolchain source lines, and that the rejection scope is
  the family (not gated by the cfg).

The substantive D9 + D10 + S0 locks remain unchanged after
Round 3.6 corrections.

**Status.** Round 3.7 prepared; pending Codex re-review.

---

## Round 3.7 -- D9 + D10 + S0 framing re-review after Round-3.6 fixes

**Date.** 2026-04-29.

**Scope.** Same as prior rounds.

**Inputs to Codex.** Same as prior rounds, plus a Round-3.6
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r7.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
Verdict trajectory: BLOCK (R3.1, 10) -> BLOCK (R3.2, 5) ->
MINOR (R3.3, 3) -> MINOR (R3.4, 2) -> MINOR (R3.5, 2) ->
MINOR (R3.6, 1) -> MINOR (R3.7, 1). Round-3.6 fix verified
correct; new issue found is a separate enumeration omission.

**Issues.**

1. *[MINOR] d10-generic-atomic-alias-omitted
   (`vocabulary-SESSION-C.md:512`,
   `discussions-SESSION-C.md:180-189`).* The Round-3.6
   AtomicU128 / AtomicI128 fix verified correct. But the
   pinned toolchain source also defines an unstable generic
   alias `core::sync::atomic::Atomic<T>` (gated on
   `feature(generic_atomic)`, tracking issue
   rust-lang/rust#130539, at
   `library/core/src/sync/atomic.rs:340-345`) which the
   Decision body's "Members:" enumeration omits. The
   omission is also semantically ambiguous because D10
   introduces a regseq-DSL `regseq::Atomic<T>` facade with
   the same short name; readers might confuse the two.

**Corrections applied.**

- *Issue 1.* Added `core::sync::atomic::Atomic<T>` to both
  enumerations with parentheticals noting:
  (a) it is the unstable generic atomic alias gated on
  `feature(generic_atomic)`, tracking issue
  rust-lang/rust#130539, at toolchain source line 340-345;
  (b) the rejection scope is the family, not gated by the
  unstable cfg.
  Added a "Naming clarification" paragraph immediately after
  the Members list in the vocabulary D10 Decision body
  explicitly distinguishing `core::sync::atomic::Atomic<T>`
  (the unstable core alias, in rejection scope) from the
  regseq-DSL `regseq::Atomic<T>` facade (the canonical
  driver-side surface, the accept path).
  Discussions side received the equivalent inline note
  inside the Decision sentence (no separate paragraph
  needed there since the discussions style is denser).

The substantive D9 + D10 + S0 locks remain unchanged after
Round 3.7 corrections.

**Status.** Round 3.8 prepared; pending Codex re-review.

---

## Round 3.8 -- D9 + D10 + S0 framing re-review after Round-3.7 fixes

**Date.** 2026-04-29.

**Scope.** Same as prior rounds.

**Inputs to Codex.** Same as prior rounds, plus a Round-3.7
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r8.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
Verdict trajectory: BLOCK (R3.1, 10) -> BLOCK (R3.2, 5) ->
MINOR (R3.3, 3) -> MINOR (R3.4, 2) -> MINOR (R3.5, 2) ->
MINOR (R3.6, 1) -> MINOR (R3.7, 1) -> MINOR (R3.8, 1).
Round-3.7 fix verified correct; new issue is a terminology
precision slip.

**Issues.**

1. *[MINOR] d10-generic-atomic-feature-gate-mislabeled
   (`vocabulary-SESSION-C.md:528-529`).* The Round-3.7
   `core::sync::atomic::Atomic<T>` addition cited the correct
   evidence (`feature(generic_atomic)`, tracking issue
   rust-lang/rust#130539, toolchain source line 340-345),
   but the follow-on sentence said "rejection scope is the
   family, not gated by the unstable cfg". `#[unstable(feature
   = "generic_atomic", issue = "130539")]` is an *unstable
   feature gate*, not a `#[cfg(...)]` attribute -- they are
   related but distinct rustc mechanisms. The 128-bit
   `target_has_atomic_load_store` gate is correctly labelled
   as cfg in the line above, so the imprecision is local to
   the generic-atomic clause.

**Corrections applied.**

- *Issue 1.* Replaced "rejection scope is the family, not
  gated by the unstable cfg" with "Rejection scope is the
  family, not conditional on the unstable feature gate (the
  alias is in scope even though source-side use of it
  requires `#![feature(generic_atomic)]`)." Also tightened
  the upstream phrasing: "the unstable generic atomic type
  alias declared with the `#[unstable(feature =
  "generic_atomic", issue = "130539")]` attribute" -- naming
  the actual rustc attribute rather than paraphrasing.

The substantive D9 + D10 + S0 locks remain unchanged after
Round 3.8 corrections.

**Status.** Round 3.9 prepared; pending Codex re-review.

---

## Round 3.9 -- D9 + D10 + S0 framing re-review after Round-3.8 fixes

**Date.** 2026-04-29.

**Scope.** Same as prior rounds.

**Inputs to Codex.** Same as prior rounds, plus a Round-3.8
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r9.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
Verdict trajectory: BLOCK (R3.1, 10) -> BLOCK (R3.2, 5) ->
MINOR (R3.3, 3) -> MINOR (R3.4, 2) -> MINOR (R3.5, 2) ->
MINOR (R3.6, 1) -> MINOR (R3.7, 1) -> MINOR (R3.8, 1) ->
MINOR (R3.9, 1). Round-3.8 fix verified correct in the
vocabulary file but did not propagate to the discussions
file -- partial-propagation pattern again.

**Issues.**

1. *[MINOR] d10-generic-atomic-feature-gate-still-paraphrased-in-discussions
   (`discussions-SESSION-C.md:189-193`).* The R3.8 terminology
   precision (use the actual rustc attribute
   `#[unstable(feature = "generic_atomic", issue = "130539")]`
   and "feature gate" rather than paraphrased "gated on
   `feature(generic_atomic)`" or "unstable cfg") landed in the
   vocabulary D10 Decision body but did not propagate to the
   discussions D10 Decision sentence. The discussions still
   said "gated on `feature(generic_atomic)`".

**Corrections applied.**

- *Issue 1.* Mirrored the R3.8 vocabulary phrasing into the
  discussions D10 Decision sentence: replaced "gated on
  `feature(generic_atomic)`" with "declared with
  `#[unstable(feature = "generic_atomic", issue = "130539")]`
  at `library/core/src/sync/atomic.rs:340-345` in the pinned
  toolchain"; replaced the implicit gating note with
  "rejection scope is the family, not conditional on the
  unstable feature gate (the alias is in scope even though
  source-side use of it requires `#![feature(generic_atomic)]`)".

The substantive D9 + D10 + S0 locks remain unchanged after
Round 3.9 corrections.

**Status.** Round 3.10 prepared; pending Codex re-review.

---

## Round 3.10 -- D9 + D10 + S0 framing re-review after Round-3.9 fixes

**Date.** 2026-04-29.

**Scope.** Same as prior rounds.

**Inputs to Codex.** Same as prior rounds, plus a Round-3.9
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r10.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
Verdict trajectory: BLOCK (R3.1, 10) -> BLOCK (R3.2, 5) ->
MINOR (R3.3, 3) -> MINOR (R3.4, 2) -> MINOR (R3.5, 2) ->
MINOR (R3.6, 1) -> MINOR (R3.7, 1) -> MINOR (R3.8, 1) ->
MINOR (R3.9, 1) -> MINOR (R3.10, 1). Round-3.9 fix verified
correct; new issue is a substantive correctness issue in
the worked example (not a propagation residual).

**Issues.**

1. *[MINOR] d10-worked-example-keeps-const-after-unsafecell-drop
   (`vocabulary-SESSION-C.md:951, 980`).* The S2.1 worked
   example's Linux and Zephyr C lowerings both used the C
   signature `static inline uint32_t read_count(const
   Atomic_u32_t *counter)`, but per locked D1.5
   (`stage-3/vocabulary.md:1545, :1670`), references whose
   referent transitively contains `UnsafeCell` MUST drop the
   C `const` qualifier. `Atomic_u32_t` contains
   `UnsafeCell<u32>` (per the structural composition the
   worked example itself relies on -- the Rust source uses
   `&Atomic<u32>` and the D10 reasoning rests on the inner
   `is_unsafe_cell` flag handling const-dropping
   automatically). The worked example was therefore
   inconsistent with the very D1.5 mechanism it cites.

**Corrections applied.**

- *Issue 1.* Removed `const` from both the Linux and Zephyr
  C signatures (`Atomic_u32_t *counter` instead of `const
  Atomic_u32_t *counter`). Added an inline comment block
  before the Linux signature explaining the `const` drop and
  cross-referencing the locked D1.5 + Section 4.5 worked
  example in the shared spec; added a shorter back-reference
  comment before the Zephyr signature noting the same drop
  applies. The C body is unchanged -- the load expression
  reads through the pointer, which is valid regardless of
  the `const` qualifier; removing `const` correctly reflects
  the D1.5 contract that interior mutation is permitted
  through such references.

This is a substantive correctness fix, but it does NOT
disturb the D9 + D10 + S0 substantive locks; it brings the
worked example into alignment with the already-locked D1.5
contract that D10 explicitly leans on.

**Status.** Round 3.11 prepared; pending Codex re-review.

---

## Round 3.11 -- D9 + D10 + S0 framing re-review after Round-3.10 fixes

**Date.** 2026-04-29.

**Scope.** Same as prior rounds.

**Inputs to Codex.** Same as prior rounds, plus a Round-3.10
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r11.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
Verdict trajectory: BLOCK (R3.1, 10) -> BLOCK (R3.2, 5) ->
MINOR (R3.3, 3) -> MINOR (R3.4, 2) -> MINOR (R3.5, 2) ->
MINOR (R3.6, 1) -> MINOR (R3.7, 1) -> MINOR (R3.8, 1) ->
MINOR (R3.9, 1) -> MINOR (R3.10, 1) -> MINOR (R3.11, 1).
Round-3.10 fix verified correct; new issue is a precision
issue in S0's qualifier-to-IR-site table.

**Issues.**

1. *[MINOR] d1.5-const-source-row-over-scopes-unsafecell
   (`vocabulary-SESSION-C.md:104`,
   `discussions-SESSION-C.md:65`).* The S0 framing principle
   table claimed `const` is derived from `(Mutability,
   Provenance)` on `Ty::Ref` *or* `Ty::RawPtr` plus the
   transitive `is_unsafe_cell` flag. But locked D1.5 scopes
   the `is_unsafe_cell` flag specifically to dropping `const`
   on enclosing *references*, not raw pointers
   (`stage-3/vocabulary.md:382, :418`); Section 4.1 confirms:
   "drop when the *reference* type transitively contains
   UnsafeCell" (`stage-3/vocabulary.md:1545`). For raw
   pointers, `*const T` vs `*mut T` is determined entirely
   by the explicit `Mutability` on `Ty::RawPtr`; the
   `is_unsafe_cell` flag does not apply.

**Corrections applied.**

- *Issue 1.* Split the `const` row in S0's qualifier-to-IR-
  site table into two rows:
  - `const` on `Ty::Ref`: derived from `(Mutability,
    Provenance)` on `Ty::Ref` plus the transitive
    `is_unsafe_cell` flag on `Item` (the flag drops `const`
    when the referent transitively contains `UnsafeCell`);
    sources D1.1 + D1.5 LOCKED, D7/D8 OPEN.
  - `const` on `Ty::RawPtr`: derived from `Mutability` on
    `Ty::RawPtr` only; the `is_unsafe_cell` flag does *not*
    apply to raw pointers per locked D1.5; source D1.1
    LOCKED.
  Mirrored the same split into the discussions D9 reasoning
  qualifier-to-IR-site bullet list.

The substantive D9 + D10 + S0 locks remain unchanged after
Round 3.11 corrections. The S0 split is a precision
improvement, not a substantive change to the framing
principle.

**Status.** Round 3.12 prepared; pending Codex re-review.

---

## Round 3.12 -- D9 + D10 + S0 framing re-review after Round-3.11 fixes

**Date.** 2026-04-29.

**Scope.** Same as prior rounds.

**Inputs to Codex.** Same as prior rounds, plus a Round-3.11
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r12.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
Verdict trajectory: BLOCK (R3.1, 10) -> BLOCK (R3.2, 5) ->
MINOR (R3.3, 3) -> MINOR (R3.4, 2) -> MINOR (R3.5, 2) ->
MINOR (R3.6, 1) -> MINOR (R3.7, 1) -> MINOR (R3.8, 1) ->
MINOR (R3.9, 1) -> MINOR (R3.10, 1) -> MINOR (R3.11, 1) ->
MINOR (R3.12, 1). Round-3.11 fix verified correct; new
issue is a stale path + status-wording residual.

**Issues.**

1. *[MINOR] session-status-crossrefs-stale-or-wrong
   (`discussions-SESSION-C.md:18-20`,
   `vocabulary-SESSION-C.md:1083-1084`).* Two related
   problems:
   - The discussions-SESSION-C.md header said "Status of each
     decision in this file mirrors the live status recorded in
     `../stage-3-status.md`" -- but the actual file lives at
     `stage-3/stage-3-status.md`, so from `tmp/SESSION-C/` the
     correct relative path is `../../stage-3-status.md`. Same
     2-vs-3-level path bug recurred in vocabulary-SESSION-C.md
     S6 references for `memory.md` and `stage-3-status.md`.
   - The discussions header also implied SESSION-C's status
     mirrors the shared status file, but that's the wrong
     framing: SESSION-C marks D9/D10 LOCKED locally, while
     the shared status file correctly still shows them open
     (per parallel-session protocol, the shared docs only
     update when the merge session lands SESSION-C's
     content).

**Corrections applied.**

- *Issue 1.* Rewrote the discussions-SESSION-C.md header to
  describe SESSION-C's status as session-local (D9/D10
  marked LOCKED in this addendum pending merge-session
  integration); explained that the shared status file at
  `../../stage-3-status.md` and the Section 5 table of
  `../../vocabulary.md` will show D9/D10 as open until the
  merge lands. Fixed the vocabulary-SESSION-C.md S6 paths
  for `memory.md` and `stage-3-status.md` from `../*` to
  `../../*`.

The substantive D9 + D10 + S0 locks remain unchanged after
Round 3.12 corrections.

**Status.** Round 3.13 prepared; pending Codex re-review.

---

## Round 3.13 -- D9 + D10 + S0 framing re-review after Round-3.12 fixes

**Date.** 2026-04-29.

**Scope.** Same as prior rounds; R3.13 prompt explicitly
raised the bar to a comprehensive readiness check ("are the
addendums ready to be lifted into the shared spec at merge
time without further correction?").

**Inputs to Codex.** Same as prior rounds, plus a Round-3.12
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r13.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
Verdict trajectory: BLOCK (R3.1, 10) -> BLOCK (R3.2, 5) ->
MINOR (R3.3, 3) -> MINOR (R3.4, 2) -> MINOR (R3.5, 2) ->
8 consecutive rounds at MINOR-1 (R3.6 through R3.13).
Round-3.12 fix verified; new issue is a coverage gap
(non-`Ordering` free functions in `core::sync::atomic`).

**Issues.**

1. *[MINOR] d10-core-atomic-free-functions-unclassified
   (`vocabulary-SESSION-C.md:511, 544, 551`,
   `discussions-SESSION-C.md:156`).* D10's rejection
   decomposition addresses `core::sync::atomic::Atomic*`
   types, methods on those types, and the rustc-internal
   `core::intrinsics::atomic_*` family, and explicitly
   excludes `core::sync::atomic::Ordering` (the value-level
   enum). But it never classifies the *free functions* in
   `core::sync::atomic` -- notably
   `core::sync::atomic::fence(Ordering)` and
   `core::sync::atomic::compiler_fence(Ordering)`. Per the
   compiler-collateral construct doc
   (`docs/compiler-collateral/constructs/atomic-memory-access-and-ordering.md`
   lines 113, 175), the regseq Atomic construct surface
   explicitly excludes barrier/fence elements from Atomic.
   D10 should explicitly classify these.

**Corrections applied.**

- *Issue 1.* Added an "Out of D10's scope (free functions for
  memory barriers / fences)" paragraph to the D10 Decision
  body in vocabulary-SESSION-C.md (immediately after the
  Ordering-exclusion paragraph), stating that
  `core::sync::atomic::fence` / `compiler_fence` and related
  free functions are *not* part of the regseq Atomic
  construct (citing the compiler-collateral construct doc),
  that their positive classification (reject /
  admit-via-barrier-intrinsic / something else) is deferred
  to a future construct-level decision, and that Q7's
  unknown-name-rejection posture implicitly rejects them
  until that decision lands. Mirrored a shorter version into
  the discussions-SESSION-C.md D10 Decision sentence and
  appended a clause to the S5 row.

The substantive D9 + D10 + S0 locks remain unchanged after
Round 3.13 corrections. The barrier/fence carve-out is a
*scope clarification*, not a substantive change to D10's
locked decision.

**Status.** Round 3.14 prepared; pending Codex re-review.

---

## Round 3.14 -- D9 + D10 + S0 framing re-review after Round-3.13 fixes

**Date.** 2026-04-29.

**Scope.** Same as prior rounds.

**Inputs to Codex.** Same as prior rounds, plus a Round-3.13
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r14.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
Verdict trajectory: BLOCK (R3.1, 10) -> BLOCK (R3.2, 5) ->
MINOR (R3.3, 3) -> MINOR (R3.4, 2) -> MINOR (R3.5, 2) ->
9 consecutive rounds at MINOR-1 (R3.6 through R3.14).
Round-3.13 carve-out verified at the three primary sites
(D10 Decision body, S5 row, discussions Decision sentence)
but did not propagate to the three forward-ref / cross-ref
sites.

**Issues.**

1. *[MINOR] d10-barrier-fence-forward-refs-stale
   (`vocabulary-SESSION-C.md:61-72, 803-824`,
   `discussions-SESSION-C.md:323-333`).* The R3.13 barrier/
   fence carve-out (classifying `fence`, `compiler_fence` as
   out-of-D10-scope, deferred to a future construct-level
   decision) landed in the D10 Decision body, the S5 row,
   and the discussions D10 Decision sentence -- but did not
   propagate to: the top-level merge note item 2 outbound
   forward-ref list; the D10 stage-13 forward-ref paragraph
   in vocabulary-SESSION-C.md; the discussions cross-ref
   stage-13 entry. Each of these still listed only the
   atomic intrinsic vocabulary, ordering encoding, type-
   family rejection, method/intrinsic rejection, and Ordering
   exclusion -- omitting the barrier/fence carve-out.

**Corrections applied.**

- *Issue 1.* Three sites updated, all to add a (d) clause
  noting that `core::sync::atomic::{fence, compiler_fence}`
  and related barrier/fence free functions are explicitly
  out of D10's scope, not atomic intrinsic vocabulary
  entries, with positive classification deferred to a
  future construct-level decision per the compiler-
  collateral construct doc. Sites:
  - vocabulary-SESSION-C.md merge note item 2 (top-level).
  - vocabulary-SESSION-C.md D10 stage-13 forward-ref
    paragraph (DSL-evidence section).
  - discussions-SESSION-C.md D10 cross-ref paragraph
    (stage-13 entry).

The substantive D9 + D10 + S0 locks remain unchanged after
Round 3.14 corrections.

**Status.** Round 3.15 prepared; pending Codex re-review.

---

## Round 3.15 -- D9 + D10 + S0 framing re-review after Round-3.14 fixes

**Date.** 2026-04-29.

**Scope.** Same as prior rounds.

**Inputs to Codex.** Same as prior rounds, plus a Round-3.14
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r15.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
Verdict trajectory: BLOCK (R3.1, 10) -> BLOCK (R3.2, 5) ->
MINOR (R3.3, 3) -> MINOR (R3.4, 2) -> MINOR (R3.5, 2) ->
10 consecutive rounds at MINOR-1 (R3.6 through R3.15).
Round-3.14 propagation verified at all six sites; new
issue is a substantive over-claim about Q7's scope.

**Issues.**

1. *[MINOR] d10-fence-q7-implicit-rejection-overclaim
   (`vocabulary-SESSION-C.md:575-583`,
   `discussions-SESSION-C.md:218-220`).* The R3.13 / R3.14
   barrier/fence carve-out paragraph claimed that "Q7's
   unknown-name-rejection posture implicitly rejects" the
   fence/compiler_fence free functions until a future
   construct-level decision lands. But Q7 is specifically
   the `#[rseq::intrinsic("NAME")]`-attribute recognition
   mechanism: it rejects unknown attribute *names*, not
   arbitrary `core::*` free-function calls. Whether the
   bridge rejects raw `core::sync::atomic::fence(...)` is
   governed by a different mechanism (e.g., a stage-13
   unsupported-call policy or the eventual construct-level
   decision), not by Q7. The Q7-implicit-rejection claim
   was therefore an over-claim about Q7's scope.

**Corrections applied.**

- *Issue 1.* Rewrote the relevant paragraphs in both files:
  - vocabulary-SESSION-C.md D10 Decision body: replaced the
    Q7-implicit-rejection claim with a narrower statement
    that D10 adds no accepted intrinsic-vocabulary entry
    for `core::sync::atomic::{fence, compiler_fence}`; any
    source-level use of them remains outside D10's accept
    path; the bridge-side handling is *not* governed by
    Q7's attribute mechanism (citing
    `architecture.md` Q7 and `stage-13/expectations.md:12-17, 50-58`
    for the scope of Q7); the actual handling (reject /
    error-on-unsupported / eventually accept) is owed to
    the future construct-level decision or a separate
    stage-13 unsupported-call policy.
  - discussions-SESSION-C.md D10 Decision sentence: same
    substance in shorter form; explicitly states that Q7's
    recognition mechanism applies to `#[rseq::intrinsic]`-
    attributed callees, not to raw `core::*` free-function
    calls.

The substantive D9 + D10 + S0 locks remain unchanged after
Round 3.15 corrections. The Q7 scoping correction is a
precision fix to D10's prose, not a change to D10's locked
substantive claims.

**Status.** Round 3.16 prepared; pending Codex re-review.

---

## Round 3.16 -- D9 + D10 + S0 framing re-review after Round-3.15 fixes

**Date.** 2026-04-29.

**Scope.** Same as prior rounds.

**Inputs to Codex.** Same as prior rounds, plus a Round-3.15
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r16.txt`. Background-launched.

**Verdict.** `APPROVE` -- 0 BLOCK + 0 MINOR + 0 NIT (0 total).

Verdict trajectory: BLOCK (R3.1, 10) -> BLOCK (R3.2, 5) ->
MINOR (R3.3, 3) -> MINOR (R3.4, 2) -> MINOR (R3.5, 2) ->
MINOR (R3.6, 1) -> MINOR (R3.7, 1) -> MINOR (R3.8, 1) ->
MINOR (R3.9, 1) -> MINOR (R3.10, 1) -> MINOR (R3.11, 1) ->
MINOR (R3.12, 1) -> MINOR (R3.13, 1) -> MINOR (R3.14, 1) ->
MINOR (R3.15, 1) -> **APPROVE (R3.16, 0)**.

Codex's confirmation cited the corrected Q7 scoping at
`vocabulary-SESSION-C.md:575-588` and
`discussions-SESSION-C.md:218-225`, plus the supporting
evidence at `architecture.md:584-589` (Q7 is attribute-on-
callee recognition only), `stage-13/expectations.md:12-17,
56-58` (recognition surface is attribute lookup), and
`docs/compiler-collateral/constructs/atomic-memory-access-and-ordering.md:113-117, 177-181`
(barrier/fence elements explicitly outside the Atomic
construct).

**Issues.** None.

**Corrections applied.** None.

The substantive D9 + D10 + S0 locks reach Codex APPROVE
after 16 rounds. The substantive content locked at Round
3.1 has been stable throughout; rounds 3.2-3.15 refined
citation precision, terminology, enumeration completeness,
worked-example correctness (notably the const-drop
violation caught at R3.10 and the fence/compiler_fence
coverage gap caught at R3.13), and cross-site propagation
of every correction.

**Status.** D9 + D10 + S0 cluster CLOSED. Ready for merge-
session integration into the shared
`docs/discussions/regseq-model/stage-3/vocabulary.md` and
`docs/discussions/regseq-model/stage-3/discussions.md`.
Next SESSION-C work: Phase A discussion on D13 (discriminant
level), then D16 (`c_void` recognition); then a final
SESSION-C cross-cluster sweep before merge.

---

## Round 4.1 -- D13 + D16 initial review

**Date.** 2026-04-29.

**Scope.** SESSION-C second sub-cluster: D13 (discriminant
level) + D16 (`c_void` recognition). The first sub-cluster
(D9 + D10 + S0) reached APPROVE on Round 3.16; this round
introduces the new D13 + D16 substantive content for
adversarial review. The R4.1 prompt explicitly scoped D9 /
D10 / S0 as out-of-scope (only re-flag if the new content
contradicts them).

**Inputs to Codex.** Same context files as R3.x rounds, plus
the now-locked-in-shared-spec D7/D8 entry
(`vocabulary.md` Section 1.8; `discussions.md` Section 1
D7/D8 entry at line 472+) and D15 (locked at line 749+).

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r41.txt`. Background-launched.

**Verdict.** `BLOCK` -- 3 BLOCK + 3 MINOR + 0 NIT (6 total).

**Issues.**

1. *[BLOCK] D13-semantic-vs-tag-value
   (`vocabulary-SESSION-C.md:1127, 1146, 1465`).* D13's
   single field `discriminant: Option<i128>` conflated two
   distinct concepts: the Rust-level *semantic discriminant*
   (what `Discriminant(place)` returns; per
   `tcx.discriminants(def_id)` -- always present, never
   absent) and the layout-level *tag bytes*
   (what `SetDiscriminant` writes; absent for niche-untagged
   variants where the inner data IS the tag, and absent for
   `EnumShape::Single` variants where no tag scalar exists).
   The two senses *diverge* for niche-encoded enums.

2. *[BLOCK] D13-i128-and-source-citation
   (`vocabulary-SESSION-C.md:1154`).* The `i128` width
   justification cited `rustc_middle/src/ty/adt.rs:159`
   for `VariantDef.discr`, but the actual location is
   `rustc_middle/src/ty/mod.rs:1116-1128` (and
   `VariantDiscr` itself is `Explicit(DefId)` /
   `Relative(u32)`, not `i128`-typed; defined at
   `rustc_middle/src/ty/mod.rs:1313-1323`). The resolved
   discriminant is `Discr { val: u128, ty: Ty }` at
   `rustc_middle/src/ty/util.rs:36-40` -- `u128`, not
   `i128`. Signed interpretation is supplied by the `ty`
   field, paralleling our `TagScalar.scalar_ty`.

3. *[MINOR] D13-single-empty-coverage
   (`vocabulary-SESSION-C.md:1135`).* D13 only specified
   field semantics for `EnumShape::Multiple`; D11's
   `VariantLayout` is also used for `EnumShape::Single`,
   and `EnumShape::Empty` carries no `VariantLayout` at
   all. Coverage gap.

4. *[BLOCK] D16-c_void-primary-source-mismatch
   (`vocabulary-SESSION-C.md:1532`).* D16 repeatedly
   described `core::ffi::c_void` as a zero-variant /
   `EnumShape::Empty` enum, but the pinned-toolchain source
   (`library/core/src/ffi/mod.rs:49-76`) declares c_void
   with two hidden `__variant1` / `__variant2` variants
   specifically to make the type *inhabited*, not
   uninhabited. The "zero-variant uninhabited enum" framing
   was factually wrong. The rejection rationale needs to
   rest on lang-item FFI identity and the missing
   FFI-canonical `void *` lowering decision, not on the
   structural variant count.

5. *[MINOR] D16-merge-note-omission
   (`vocabulary-SESSION-C.md:70`).* The top-level merge note
   item 2's "(d) bridge-side rejection enforcement" sub-list
   only enumerated D9 and D10 rejections; D16's
   type-translation rejection of `core::ffi::c_void` was
   missing from the merge-session task list.

6. *[MINOR] D13-stale-type-shape-names
   (`discussions-SESSION-C.md:401`,
   `vocabulary-SESSION-C.md:1442, 1463`).* D13 used the old
   type-shape names `Ty::RawPtr(_, _)` and
   `Ty::Ref(_, _, _)` in prose and worked examples, but the
   shared spec now (post-D7/D8 lock during SESSION-C's
   R3.x iterations) uses `Ty::Ptr(TyId, Mutability)` (renamed)
   and `Ty::Ref(TyId, Provenance)` (2-arg form, Provenance
   subsumes Mutability for refs).

**Corrections applied (broader scope than the 6 issues alone).**

This round triggered a broader SESSION-C-wide sync. While
fixing the issues, I discovered that:
- D7/D8 was locked in the shared spec
  (`vocabulary.md:570-694`; `discussions.md:472+`) during
  SESSION-C's R3.x iterations on D9 + D10. The new locked
  type-shape names (`Ty::Ptr` renamed from `Ty::RawPtr`;
  `Ty::Ref` is now 2-arg with `Provenance` subsuming
  `Mutability`) need to propagate to *all* SESSION-C
  sections, not just D13.
- D15 was also locked in the shared spec
  (`discussions.md:749`); no direct SESSION-C interaction
  but recorded for context.

Full correction list:

- *Issue 1 + 3.* Replaced the single-field
  `discriminant: Option<i128>` with two fields:
  `discriminant: u128` (always present; semantic) and
  `tag_value: Option<u128>` (layout-level; `None` for
  niche-untagged variants and for `EnumShape::Single`
  variants). Updated D13 Decision body, Reasoning prose,
  Methodology section, both worked examples (S3.1 Direct
  with user-assigned tags; S3.2 niche-encoded
  `Option<&u32>`), and the discussions D13 entry to use
  the two-field shape consistently. Added explicit
  coverage for `EnumShape::Single` (`discriminant: Some(v)`,
  `tag_value: None`) and `EnumShape::Empty` (no
  `VariantLayout`, fields don't apply).

- *Issue 2.* Changed `i128` → `u128` width;
  citations updated to `rustc_middle/src/ty/util.rs:36-40`
  (resolved `Discr.val: u128`),
  `rustc_middle/src/ty/mod.rs:1116-1128`
  (`VariantDef.discr: VariantDiscr`),
  `rustc_middle/src/ty/mod.rs:1313-1323` (`VariantDiscr`
  enum: `Explicit(DefId)` / `Relative(u32)` -- the
  source-level form). Noted that signed interpretation
  comes from `TagScalar.scalar_ty: TyId`, paralleling
  rustc's `Discr.ty: Ty`.

- *Issue 4.* Reframed D16's rejection rationale.
  Removed the "zero-variant uninhabited enum" framing
  throughout (in S4 Decision body, Reasoning, prior-art
  evidence, and discussions D16 entry). Added explicit
  citation to `library/core/src/ffi/mod.rs:49-76` for the
  two hidden `__variant1` / `__variant2` variants. Updated
  the rejection rationale to rest on (a) lang-item FFI
  identity (recognised via `tcx.is_lang_item(did,
  LangItem::CVoid)`) and (b) the missing FFI-canonical
  `void *` C-lowering decision (without recognition,
  c_void translates as a 2-variant `repr(u8)` enum
  structurally indistinguishable from any user-defined
  `repr(u8) enum Foo { X, Y }`, producing a 1-byte tagged
  struct in C -- not the FFI-canonical `void *` that
  c_void's lang-item identity entails). Added a
  parenthetical noting that D1.6.c's "zero-variant enum"
  description reflects an older toolchain shape; D16 does
  not need to revisit D1.6.c on this point because the
  lock rests on lang-item identity plus FFI-lowering
  policy, not on the structural variant count.

- *Issue 5.* Extended merge note item 2's "(d)" sub-list
  to include D16's type-translation rejection of
  `core::ffi::c_void` (recognised via
  `tcx.is_lang_item(did, LangItem::CVoid)` per D1.5;
  diagnostic recommends opaque-struct pattern per D1.4
  precedent).

- *Issue 6 + broader sync.* Renamed all `Ty::RawPtr` -> `Ty::Ptr`
  via replace_all in both addendum files. Updated `Ty::Ref`
  3-arg references to the new 2-arg form (Provenance
  subsumes Mutability for refs). Updated S0 framing-
  principle table to reflect locked D7/D8 status (no
  longer "OPEN -- current lean is three-variant"; now
  "LOCKED" with citation to `vocabulary.md` Section 1.8
  and `discussions.md` Section 1 D7/D8 entry). Updated
  merge note item 3 ("S0 robustness to D7/D8") from
  "the merge session should sweep the S0 table for
  consistency with D7/D8's locked outcome" to "the table
  is current; no re-wording pending." Updated worked
  examples in D9 (Ty: RawPtr → Ty: Ptr; Mutability::Mut
  spelled explicitly) and D10 (Ty: Ref(... , Not,
  SharedBorrow) → Ty: Ref(..., Provenance::SharedBorrow)
  2-arg form). Updated D13 worked-example IR schema
  similarly.

The substantive D13 + D16 + S0 + D9 + D10 locks remain
unchanged in their core commitments after R4.1
corrections; the changes are precision fixes (citation
correctness, naming sync, schema split) and one factual
correction (c_void's variant count).

**Status.** Round 4.2 prepared; pending Codex re-review.

---

## Round 4.2 -- D13 + D16 re-review after Round-4.1 fixes

**Date.** 2026-04-29.

**Scope.** Same as R4.1.

**Inputs to Codex.** Same as R4.1, plus a Round-4.1
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r42.txt`. Background-launched.

**Verdict.** `BLOCK` -- 1 BLOCK + 4 MINOR + 0 NIT (5 total).
Down from R4.1's 6 issues. The blocker plus 1 minor are R4.1
reframing residuals (D16 stale "empty enum" claims at
follow-on sites the R4.1 reframing didn't reach); the other
3 minors are new substantive catches.

**Issues.**

1. *[BLOCK] D16-stale-empty-cvoid
   (`discussions-SESSION-C.md:579, 621`).* The R4.1 reframing
   landed at the top-level Decision body in both files but
   missed two follow-on places in the discussions:
   - Option γ description still said "empty-shape c_void
     lowering is incomplete in v1" -- stale relative to the
     corrected 2-variant `repr(u8)` shape.
   - Reasoning sub-strand 3 ("non-rejection alternative does
     not earn its weight") still said translating c_void as
     `EnumShape::Empty` admits "an uninhabited Rust type" --
     same stale claim.

2. *[MINOR] D16-stale-charon-empty
   (`vocabulary-SESSION-C.md:1845`).* The Charon prior-art
   paragraph still said Charon admits c_void because
   "abstract reasoning over an empty enum is well-defined" --
   stale relative to the corrected pinned-toolchain shape.
   The Charon evidence supports only the "ordinary ADT / no
   special marker" claim.

3. *[MINOR] D13-discriminant-option-spelling
   (`vocabulary-SESSION-C.md:1350-1366`).* The coverage-
   bullets section used `discriminant = Some(...)` syntax
   for direct, niche, untagged, and Single cases, but the
   schema declares `pub discriminant: u128` (not `Option`).
   The worked examples correctly use bare values
   (`discriminant: 0`, etc.); the bullets were inconsistent
   with the schema.

4. *[MINOR] D13-rustc-api-spelling
   (`vocabulary-SESSION-C.md:1147, 1221-1226`).* Repeatedly
   spelled the rustc API as `tcx.discriminants(def_id)` or
   "the `tcx.discriminants` query"; the actual API is
   `AdtDef::discriminants(self, tcx)` returning
   `(VariantIdx, Discr)`
   (`compiler/rustc_middle/src/ty/adt.rs:583-600`); accessed
   via `adt_def.discriminants(tcx)` after looking up the
   `AdtDef` from the type. `Discr` itself is at
   `compiler/rustc_middle/src/ty/util.rs:36-40`.

5. *[MINOR] D16-const-void-future-lowering
   (`vocabulary-SESSION-C.md:1735-1736`).* The D16 future-
   reopen note collapsed `*mut c_void` and `*const c_void`
   to a single C `void *` lowering. Per locked D7/D8,
   `Ty::Ptr(TyId, Mutability)` carries `Mutability`, and S0
   says raw-pointer `const` derives from that field. So
   `*mut c_void` → `void *` and `*const c_void` →
   `const void *` are distinct C-spellings the reopened D16
   would need to choose between (or accept both).

**Corrections applied.**

- *Issue 1.* Updated both follow-on sites in
  discussions-SESSION-C.md:
  - Option γ: replaced "empty-shape c_void lowering" with
    "the structurally-faithful 2-variant `repr(u8)` c_void
    lowering does not match the FFI-canonical `void *`
    shape and is incomplete in v1."
  - Reasoning sub-strand 3: rewrote to use the
    pinned-toolchain shape (2-variant `repr(u8)` enum,
    `__variant1` / `__variant2`,
    `library/core/src/ffi/mod.rs:49-76`) and the
    "structurally-faithful but FFI-wrong 1-byte tagged
    struct" framing instead of the prior "uninhabited
    type / `struct c_void;` opaque forward declaration"
    framing.

- *Issue 2.* Rewrote the Charon prior-art paragraph in
  vocabulary-SESSION-C.md S4: removed the
  "abstract reasoning over an empty enum" claim;
  retained only what the prior-art file actually
  supports (Charon treats c_void as ordinary ADT with no
  type-level marker, lines 609-611 and 664-678) and
  contrasted with our codegen-target posture's stricter
  emission requirements (FFI-canonical `void *`).

- *Issue 3.* Rewrote the D13 coverage bullets to use bare
  `discriminant = v` / `discriminant = rustc_resolved_value`
  (matching the schema `pub discriminant: u128`); kept
  `tag_value = Some(...)` / `tag_value = None` since
  `tag_value` is genuinely `Option<u128>`.

- *Issue 4.* Replaced all `tcx.discriminants(def_id)` and
  `tcx.discriminants(StatusDefId)` and `tcx.discriminants`
  variants with the correct `adt_def.discriminants(tcx)` /
  `status_adt_def.discriminants(tcx)` spelling. Updated the
  citation from "`adt_def.discriminants(tcx)` query" to
  "`AdtDef::discriminants(self, tcx)` iterator
  (`compiler/rustc_middle/src/ty/adt.rs:583-600`)".

- *Issue 5.* Rewrote the D16 future-reopen C-lowering bullet
  to distinguish `*mut c_void` -> `void *` from
  `*const c_void` -> `const void *`, noting that the exact
  const-qualified spelling is part of the reopened D16's
  decision (so D16 reopens with both spellings on the table
  rather than defaulting to `void *` for both).

The substantive D9 + D10 + S0 + D13 + D16 locks remain
unchanged after R4.2 corrections.

**Status.** Round 4.3 prepared; pending Codex re-review.

---

## Round 4.3 -- D13 + D16 re-review after Round-4.2 fixes

**Date.** 2026-04-29.

**Scope.** Same as R4.2.

**Inputs to Codex.** Same as R4.2, plus a Round-4.2
correction summary in the prompt header.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r43.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 2 MINOR + 0 NIT (2 total).
Trajectory in this sub-cluster: R4.1 BLOCK (6) -> R4.2
BLOCK (5) -> R4.3 MINOR (2). All R4.2 BLOCK + 4 MINOR
fixes verified.

**Issues.**

1. *[MINOR] D16-charon-cvoid-evidence-overclaim
   (`vocabulary-SESSION-C.md:1852`).* The D16 Charon
   prior-art paragraph said "Charon treats `c_void` as an
   ordinary Adt with no type-level marker" citing
   `prior-art/charon.md` Theme D, but the cited evidence
   (lines 609 and 664-678) lists specific marker types Charon
   handles as ordinary ADTs (`UnsafeCell`, `MaybeUninit`,
   `PhantomData`, atomic types, `NonNull`, `Cell`, `RefCell`,
   `Pin`, `ManuallyDrop`); `c_void` is not in that list. The
   claim is an inference from Charon's general
   `TyKind::Adt(TypeDeclRef)` model with limited builtins,
   not direct evidence.

2. *[MINOR] D16-summary-reopen-void-const-collapse
   (`discussions-SESSION-C.md:592, 595`;
   `vocabulary-SESSION-C.md:1954`).* The D16 detailed
   future-reopen note (R4.2 fix) correctly distinguishes
   `*mut c_void` -> `void *` from `*const c_void` ->
   `const void *`, but two summary sites still collapsed
   the two pointer forms to "FFI-canonical `void *`
   lowering" only: the discussions D16 Decision-summary
   sentence about reopening triggers, and the S5 D16 row's
   reopen condition.

**Corrections applied.**

- *Issue 1.* Rewrote the D16 Charon prior-art paragraph in
  vocabulary-SESSION-C.md to mark the c_void claim as an
  inference, not direct evidence: "*Charon* (inference, not
  direct evidence). The `prior-art/charon.md` Theme D entry
  catalogues which Rust marker types Charon treats as
  ordinary ADTs without type-level markers: `UnsafeCell`,
  `MaybeUninit`, `PhantomData`, atomic types, `NonNull`,
  `Cell`, `RefCell`, `Pin`, `ManuallyDrop` (lines 609 and
  664-678). The evidence file does not specifically name
  `c_void`. By Charon's general posture
  (`TyKind::Adt(TypeDeclRef)` for any nominal ADT, with
  limited builtins), c_void would *inferentially* fall into
  the same ordinary-ADT bucket; this is not directly cited
  evidence but an extrapolation."

- *Issue 2.* Updated both summary sites:
  - discussions-SESSION-C.md D16 Decision-summary trigger
    sentence: replaced "FFI-canonical `void *` lowering"
    with "FFI-canonical pointer-to-c_void lowering --
    specifically `*mut c_void` -> C `void *` and/or
    `*const c_void` -> C `const void *` (the two pointer
    forms have distinct C spellings per locked D7/D8's
    `Ty::Ptr(TyId, Mutability)` and S0's raw-pointer
    `const`-derivation rule)".
  - vocabulary-SESSION-C.md S5 D16 row: replaced "stable
    need for FFI-canonical `void *` lowering" with
    "stable need for FFI-canonical pointer-to-c_void
    lowering (`*mut c_void` -> `void *`; `*const c_void`
    -> `const void *`)".

The substantive D9 + D10 + S0 + D13 + D16 locks remain
unchanged after R4.3 corrections.

**Status.** Round 4.4 prepared; pending Codex re-review.

---

## Round 4.4 -- D13 + D16 re-review after Round-4.3 fixes

**Date.** 2026-04-29.

**Scope.** Same as R4.3.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r44.txt`. Background-launched.

**Verdict.** `MINOR` -- 0 BLOCK + 2 MINOR + 0 NIT (2 total).
Trajectory in this sub-cluster: R4.1 BLOCK (6) -> R4.2
BLOCK (5) -> R4.3 MINOR (2) -> R4.4 MINOR (2). Both R4.4
issues are partial-propagation residuals from R4.3 fixes.

**Issues.**

1. *[MINOR] D16-discussions-charon-evidence-overclaim
   (`discussions-SESSION-C.md:675`).* The R4.3 fix to mark
   the Charon c_void claim as inference (not direct
   evidence) landed in the vocabulary-side prior-art
   paragraph but did not propagate to the discussions-side
   cross-ref, which still said "Charon treats c_void as
   ordinary Adt with no type-level marker" without the
   inference qualifier.

2. *[MINOR] D16-residual-void-star-summary-collapse
   (`vocabulary-SESSION-C.md:1800-1801`).* The R4.3 fix to
   distinguish `*mut c_void` -> `void *` from
   `*const c_void` -> `const void *` landed in the S5 row,
   the discussions Decision-summary trigger, and the
   detailed S4 future-reopen note, but missed one
   summary-prose site in the D16 Reasoning sub-strand 3
   ("non-rejection alternative does not earn its weight"),
   which still summarized the future reopen as "an FFI-
   canonical `void *` lowering decision" (singular `void *`).

**Corrections applied.**

- *Issue 1.* Mirrored the vocabulary-side inference
  framing into the discussions-side D16 cross-ref:
  "`../../prior-art/charon.md` Theme D lines 609 and
  664-678 (Charon catalogues which marker types --
  `UnsafeCell`, `MaybeUninit`, `PhantomData`, atomic types,
  `NonNull`, `Cell`, `RefCell`, `Pin`, `ManuallyDrop` --
  are treated as ordinary ADTs without type-level markers;
  `c_void` is not specifically named, so the claim that
  Charon admits c_void the same way is *inference* from
  Charon's general `TyKind::Adt(TypeDeclRef)` model with
  limited builtins, not direct evidence -- the alpha-style
  admittance posture, which D16 declines for the v1 codegen
  reasons above)."

- *Issue 2.* Updated the D16 Reasoning sub-strand 3
  closing sentence to spell out both pointer-form C
  spellings: "either path before D16 can reopen with
  proper recognition plus an FFI-canonical
  pointer-to-c_void lowering decision (covering both
  `*mut c_void` -> C `void *` and `*const c_void` -> C
  `const void *`, per locked D7/D8's `Ty::Ptr(TyId,
  Mutability)` shape and S0's raw-pointer `const`-
  derivation rule)."

The substantive D9 + D10 + S0 + D13 + D16 locks remain
unchanged after R4.4 corrections.

**Status.** Round 4.5 prepared; pending Codex re-review.

---

## Round 4.5 -- D13 + D16 re-review after Round-4.4 fixes

**Date.** 2026-04-29.

**Scope.** Same as R4.4. R4.5 prompt explicitly asked Codex
for an exhaustive cross-spec sweep this round, given the
R4.x history of partial-propagation residuals.

**Codex run.** `--effort high`, `--timeout 1500`,
`--message-only`. Output captured to
`/tmp/codex_dC_review_r45.txt`. Background-launched.

**Verdict.** `APPROVE` -- 0 BLOCK + 0 MINOR + 0 NIT (0 total).

Verdict trajectory in the D13 + D16 sub-cluster:
R4.1 BLOCK (6) -> R4.2 BLOCK (5) -> R4.3 MINOR (2) ->
R4.4 MINOR (2) -> **R4.5 APPROVE (0)**.

Cross-cluster verdict trajectory (full SESSION-C):
- D9 + D10 + S0 sub-cluster:
  R3.1 BLOCK (10) -> R3.2 BLOCK (5) -> R3.3 MINOR (3) ->
  R3.4 MINOR (2) -> R3.5 MINOR (2) -> R3.6 MINOR (1) ->
  R3.7 MINOR (1) -> R3.8 MINOR (1) -> R3.9 MINOR (1) ->
  R3.10 MINOR (1) -> R3.11 MINOR (1) -> R3.12 MINOR (1) ->
  R3.13 MINOR (1) -> R3.14 MINOR (1) -> R3.15 MINOR (1) ->
  **R3.16 APPROVE (0)** (16 rounds; closed 2026-04-29).
- D13 + D16 sub-cluster: 5 rounds (R4.1-R4.5); closed
  2026-04-29.
- Total: 21 Codex rounds across both sub-clusters.

**Issues.** None.

**Corrections applied.** None.

The substantive D9 + D10 + S0 + D13 + D16 locks are now
formally adversarial-review-approved. The substantive
content has been stable since each decision's initial
Phase B commit; rounds since refined citation precision,
terminology, enumeration completeness, worked-example
correctness, cross-site propagation, and (in two cases:
R3.10 const-drop, R3.13 fence/compiler_fence; R4.1
discriminant-vs-tag split, R4.1 c_void-not-zero-variant)
caught real correctness or coverage issues that improved
the substantive claims without overturning the locks.

**Status.** SESSION-C work CLOSED. All four SESSION-C
decisions plus the S0 framing principle ready for merge-
session integration into the shared
`docs/discussions/regseq-model/stage-3/vocabulary.md` and
`docs/discussions/regseq-model/stage-3/discussions.md`.

**Merge-session checklist (consolidated from per-decision
merge notes in `vocabulary-SESSION-C.md`).**

The merge session that integrates SESSION-C should:

1. *Land S0 framing principle* into the shared spec --
   either as a standalone framing subsection at the top of
   `vocabulary.md` Section 1 or folded into D9's Reasoning
   (the merge session decides; both placements are correct
   per the merge note).

2. *Append outbound forward-ref entries to
   `stage-13/expectations.md`* recording:
   - The MMIO `mmio_*` intrinsic family (D9).
   - The atomic-operation intrinsic family plus the
     ordering encoding choice (D10).
   - D9's Call-translation rejection of
     `core::ptr::{read,write}_volatile`.
   - D10's two-phase rejection: type-translation rejection
     of `core::sync::atomic::Atomic*` family;
     Call-translation rejection of methods on those types
     and of `core::intrinsics::atomic_*`; with
     `core::sync::atomic::Ordering` explicitly excluded
     and `core::sync::atomic::{fence, compiler_fence}`
     explicitly out-of-D10-scope (deferred to a future
     construct decision per
     `docs/compiler-collateral/constructs/atomic-memory-access-and-ordering.md`).
   - D16's type-translation rejection of `core::ffi::c_void`
     (recognised via `tcx.is_lang_item(did, LangItem::CVoid)`
     per D1.5; diagnostic recommends opaque-struct pattern
     per D1.4 precedent).

3. *Refine D11's locked Layout schema* in the shared
   `vocabulary.md` Section 2.2 by adding two fields to
   `VariantLayout`:
   - `discriminant: u128` (Rust-level semantic
     discriminant; always present per
     `adt_def.discriminants(tcx)`).
   - `tag_value: Option<u128>` (layout-level tag bytes;
     `None` for niche-encoded untagged variants and for
     `EnumShape::Single` variants).

4. *Update Section 5 open-decisions table* in
   `vocabulary.md`: replace the D9 / D10 / D13 / D16 rows
   with the LOCKED versions from
   `vocabulary-SESSION-C.md` S5.

5. *Cross-cluster sweep (Round 4 in stage-3 cadence)*:
   after merging SESSION-C with the other parallel
   sessions, run a final comprehensive Codex sweep on the
   integrated shared spec. (D7/D8 and D15 were locked in
   the shared spec during SESSION-C's iteration; the merge
   session should verify SESSION-C's references to those
   already-locked decisions remain consistent.)
