# SESSION-B Review Log -- D7, D8, D15

Iterative Codex adversarial-review rounds for the SESSION-B addendums
at `tmp/SESSION-B/vocabulary-SESSION-B.md` and
`tmp/SESSION-B/discussions-SESSION-B.md`. Each entry records the
round's scope, verdict, issue list, and the corrections applied
before the next round.

Reviews are invoked via `scripts/python/codex_runner.py` per the
project's CLAUDE.md contract. Each round uses a fresh prompt (Codex
has no session continuation); re-reviews include a brief summary of
what was changed since the prior round.

The structured output contract for every round:
- First line: `VERDICT: <BLOCK | MINOR | APPROVE>`
- Numbered issues list, each tagged `[BLOCK | MINOR | NIT]` with a
  decision reference, file:line citation, 1-3-sentence description,
  and a 1-sentence suggested fix.
- Last line: `TOTAL: <N> issues (<count of BLOCK> BLOCK / <count of
  MINOR> MINOR / <count of NIT> NIT)`.

The Codex prompt includes both the shared stage-3 documents
(`vocabulary.md`, `discussions.md`, `review-log.md`,
`findings-rustc-draft.md`, `prior-art/*.md`) **and** the SESSION-B
addendums so Codex can verify that the addendums are internally
consistent with the locked spec context. Codex is instructed to
review only the SESSION-B addendums; cross-cluster sweep across all
parallel sessions is the merge agent's responsibility.

The SESSION-B-local cadence is independent from the shared
`stage-3/review-log.md` (which records Rounds 1-2 of the locked
foundation). Once SESSION-B closes here at APPROVE, the merge agent
folds these rounds into the shared log under a "Round 3, SESSION-B"
header.

---

## Round 3 (SESSION-B local) -- closed at APPROVE (2026-04-29)

**Trajectory.** 5 sub-rounds. Issue count 7 -> 1 -> 2 -> 1 -> 0.
Verdict trajectory: BLOCK -> BLOCK -> MINOR -> MINOR -> APPROVE.
BLOCK verdict cleared at sub-round 3.2; soundness re-check passed
in 3.4; final exhaustive sweep-completeness scan confirmed APPROVE
at 3.5.

**Scope.** D7, D8, D15 -- the SESSION-B addendums.
- `tmp/SESSION-B/vocabulary-SESSION-B.md`
- `tmp/SESSION-B/discussions-SESSION-B.md`

**Out of scope.** D5, D6, D9, D10, D13, D14, D16 (other parallel
sessions or already-locked decisions); cross-cluster consistency
(merge agent's job); markdown-style nits.

### Sub-round 3.1

**Date.** 2026-04-29.

**Codex run.** `--effort high`, `--timeout 1500`. Output captured
to `/tmp/codex_session_b_review_r3.1.txt`.

**Verdict.** `BLOCK` -- 2 BLOCK + 5 MINOR + 0 NIT (7 total).

**Issues raised.**

1. [BLOCK] D7/D8 (`vocabulary-SESSION-B.md` Decision block, region-
   erasure rule). Strict claim "every `TyKind::Ref` region is
   post-erasure `ReErased` and can be debug-asserted/discarded"
   contradicts evidence at `findings-rustc-draft.md:675, 695-697`:
   `Ref`'s region is `ReErased` for free regions but `ReBound`
   for late-bound regions inside `Binder<FnSig>` /
   `Binder<Predicate>` and `Dynamic` existential-predicate scopes.
2. [BLOCK] D15 (`vocabulary-SESSION-B.md` Reasoning block, consumer
   list; `discussions-SESSION-B.md` Reasoning). The addendum
   attributes symbol mangling to `FnAbi`, but locked stage-1
   context (`stage-1/vocabulary.md:77, 670-673`) places mangling
   on `Item.linkage` (stage 12) and `Instance.symbol_name`
   (default `tcx.symbol_name`). `extern "C" fn` without
   `#[no_mangle]` still receives a mangled symbol; ABI does not
   drive mangling.
3. [MINOR] D15 (`vocabulary-SESSION-B.md` bridge translation
   table). Wording "[any other 25 ExternAbi variants]" is
   off-by-one given evidence shows 28 total variants
   (`findings-rustc-draft.md:200`); accepting `Rust` and
   `C { unwind: false }` leaves 26 paths plus `C { unwind: true }`
   as a rejected value.
4. [MINOR] D7/D8 (`vocabulary-SESSION-B.md` rename-sweep table).
   Sweep misses locked Section 1.6 D1.6.a at `vocabulary.md:510-
   513`, which still uses `Ref(_, Str, _)` (3-position notation)
   and `RawPtr(Int(I8), _)` -- inconsistent with SESSION-B's new
   2-position `Ref(TyId, Provenance)` and renamed `Ty::Ptr`.
5. [MINOR] D7/D8 (`discussions-SESSION-B.md` cross-refs).
   References Theme A.5 for `TyKind::Ref` / `TyKind::RawPtr`; the
   actual section is Theme A.8 (`findings-rustc-draft.md:191-196`).
6. [MINOR] D7/D8 (`vocabulary-SESSION-B.md` LLVM prior-art).
   Citation says line 505 supports the typeless `ptr` claim, but
   line 505 is the no-mutability claim; opaque-pointer evidence
   is at `prior-art/llvm-ir.md:71` (PointerType address-space-only)
   and `:389-390` (textual `ptr`).
7. [MINOR] D7/D8 (`vocabulary-SESSION-B.md` rename-sweep table).
   `expectations.md:65-69` is omitted from the sweep; after
   SESSION-B locks D7/D8 with two variants, that `[open]` entry
   ("three-variant baseline") becomes stale.

**Corrections applied.**

1. Region rule (issue 1) -- relaxed Decision block and Worked-
   example invariant 2 in both `vocabulary-SESSION-B.md` and
   `discussions-SESSION-B.md`: bridge **discards** the `_region`
   slot regardless of variant; it may be `ReErased` (free, typical
   case for MIR-local types) or `ReBound` (late-bound inside
   binders). Removed the "Reject what shouldn't appear" methodology
   bullet that was tied to the strict assertion.
2. Mangling consumer (issue 2) -- removed the symbol-mangling
   bullet from the v1-consumer list in both addendums; added an
   explicit "Not a `FnAbi` consumer: symbol mangling" sub-block
   citing `stage-1/vocabulary.md:77` (Linkage on Item) and `:670-
   673` (Instance.symbol_name). Worked-example C-emission revised
   to show that `FnAbi::C` without `#[no_mangle]` linkage still
   produces a mangled symbol; only `#[no_mangle]` linkage drives
   the unmangled spelling.
3. Count phrasing (issue 3) -- changed "[any other 25 ExternAbi
   variants]" to "[all non-Rust / non-C ExternAbi variants]" in
   both bridge tables; updated the "26 rejected ABIs" methodology
   bullet to use the same phrasing.
4. D1.6 sweep (issue 4) -- expanded the rename-sweep table row
   for `vocabulary.md` to call out lines 510-514 and the specific
   `Ref(_, Str, _)` -> `Ref(Str, _)` and `RawPtr(Int(I8), _)` ->
   `Ptr(Int(I8), _)` rewrites required for D1.6.a.
5. Theme A.5 -> A.8 (issue 5) -- corrected the cross-ref in
   `discussions-SESSION-B.md` D7/D8 entry to Theme A.8 (lines
   191-196); also added Theme E.6 (lines 695-697) for the post-
   erasure `Region` evidence.
6. LLVM citation (issue 6) -- split the citation in the LLVM
   prior-art block: `:71` and `:389-390` for opaque-pointer
   evidence, `:505` for no-mutability claim.
7. expectations.md sweep (issue 7) -- added a row to the
   rename-sweep table directing the merge agent to mark the
   `[open]` D7/D8 expectation at `expectations.md:65-69` resolved
   to the locked two-variant alphabet.

**Next sub-round.** 3.2 -- re-run with corrected addendums; the
prompt names corrections 1-7 and asks Codex to verify the BLOCK
issues are resolved without introducing new soundness flaws.

### Sub-round 3.2

**Date.** 2026-04-29.

**Codex run.** `--effort high`, `--timeout 1500`. Output captured
to `/tmp/codex_session_b_review_r3.2.txt`.

**Verdict.** `BLOCK` -- 1 BLOCK + 0 MINOR + 0 NIT (1 total). Down
from 7 issues in 3.1.

**Issues raised.**

1. [BLOCK] D15 (`vocabulary-SESSION-B.md` Methodology rules block,
   lines 454 and 461-463). After 3.1's correction removed the
   "symbol-mangling rule" from the v1-consumer list, two stray
   methodology bullets still claimed the ABI marker "determines
   symbol mangling and pretty-printing" and listed mangling among
   active consumers. This contradicts the corrected sub-block at
   lines 409-419 ("Not a `FnAbi` consumer: symbol mangling"),
   which correctly cites `stage-1/vocabulary.md:77, 670-673`.

**Corrections applied.**

1. Methodology bullets (issue 1) -- updated the "C semantics with
   information preservation" bullet to cite pretty-printing and
   IR type identity (no longer mangling); updated the "Ask who
   consumes it" bullet to enumerate (pretty-printing, future
   FFI-validity linter, future calling-convention attribute
   emission) and added an explicit "Symbol mangling is *not* a
   consumer" parenthetical pointing back to the in-Reasoning
   sub-block. `discussions-SESSION-B.md` was clean (no stray
   mangling references); no changes there.

**Next sub-round.** 3.3 -- re-run; the prompt names the methodology-
bullet correction and asks Codex to verify it without introducing
new soundness flaws. Round 3 is converging quickly; 3.3 should
either APPROVE or surface a small residual.

### Sub-round 3.3

**Date.** 2026-04-29.

**Codex run.** `--effort high`, `--timeout 1500`. Output captured
to `/tmp/codex_session_b_review_r3.3.txt`.

**Verdict.** `MINOR` -- 0 BLOCK + 2 MINOR + 0 NIT (2 total). BLOCK
verdict cleared; only cosmetic / consistency fixes remain.

**Issues raised.**

1. [MINOR] D15 (consumer-list terminology). The corrected addendums
   use three different labels for the consumer enumeration:
   "Concrete v1 consumers" (`vocabulary-SESSION-B.md:394`),
   "Active v1 consumers" (`discussions-SESSION-B.md:217`), and the
   accurate "active and near-future consumers"
   (`vocabulary-SESSION-B.md:461`). The worked-example effects list
   at `vocabulary-SESSION-B.md:549-557` also omits the future
   FFI-validity linter from its enumeration of v1 effects.
2. [MINOR] D15 (rustc_codegen_clr prior-art bullet). The CLR bullet
   at `vocabulary-SESSION-B.md:498-500` claims CLR uses managed
   calling conventions / .NET `MethodAttributes`, but the cited
   evidence file does not support the `MethodAttributes` claim;
   the actual CLR evidence at `prior-art/rustc-codegen-clr.md:116-
   118` says `FnSig` carries no calling convention or varargs
   marker. The bullet should either be removed or rewritten as a
   non-comparable counterexample with the correct file:line.

**Corrections applied.**

1. Terminology standardized to "active and near-future consumers"
   in three places: `vocabulary-SESSION-B.md:394` (consumer-list
   header), `discussions-SESSION-B.md:217` (Reasoning block), and
   the worked-example effects list at
   `vocabulary-SESSION-B.md:549-557` (now enumerates four effects
   including the future FFI-validity linter as effect (c)).
2. CLR prior-art bullet rewritten to cite
   `prior-art/rustc-codegen-clr.md:116-118` directly: "CLR's
   `FnPtr(Interned<FnSig>)` carries an interned signature whose
   `FnSig` is `{ inputs: Box<[Type]>, output: Type }` -- no
   calling-convention or varargs marker stored." Framed as a
   non-comparable counterexample (CLR targets .NET CIL, not
   rustc-style ABI semantics).

**Next sub-round.** 3.4 -- re-run; the prompt names the two
terminology / citation corrections and asks Codex to verify them
plus do a final whole-addendum soundness re-check. With the BLOCK
verdict already cleared, 3.4 likely closes at APPROVE absent new
discoveries.

### Sub-round 3.4

**Date.** 2026-04-29.

**Codex run.** `--effort high`, `--timeout 1500`. Output captured
to `/tmp/codex_session_b_review_r3.4.txt`.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).

**Issues raised.**

1. [MINOR] D8 (sweep miss in Section 3.3 TyInterner walkthrough).
   The merge sweep covers Section 1.1, Section 1.6, and Section
   4.1 of the locked `vocabulary.md`, but misses the locked
   Section 3.3 TyInterner construction walkthrough at
   `vocabulary.md:1442, 1444, 1452, 1453, 1465`, which still
   builds and compares `Ty::Ref(TyId(0), Mutability::Mut,
   Provenance::UniqueBorrow)` and shorthand `Ty::Ref(0, Mut,
   Unique)` -- the pre-SESSION-B 3-tuple shape. If merged as
   written, Section 3.3 stays stale against the new 2-tuple
   `Ref(TyId, Provenance)` arity.

**Corrections applied.**

1. Section 3.3 sweep (issue 1) -- expanded the `vocabulary.md`
   row of the merge-sweep table in `vocabulary-SESSION-B.md`
   Section 0.1 to call out Section 3.3 explicitly with the five
   specific line numbers (1442, 1444, 1452, 1453, 1465) and
   prescribe the rewrite to the 2-tuple form
   `Ty::Ref(TyId(0), Provenance::UniqueBorrow)` /
   `Ty::Ref(0, Unique)`.

**Next sub-round.** 3.5 -- final verification pass; the prompt
names the Section 3.3 sweep correction and asks Codex to confirm
no further sweep misses or residual issues remain. Round 3
trajectory: 7 -> 1 -> 2 -> 1 issues; expecting APPROVE in 3.5.

### Sub-round 3.5

**Date.** 2026-04-29.

**Codex run.** `--effort high`, `--timeout 1500`. Output captured
to `/tmp/codex_session_b_review_r3.5.txt`.

**Verdict.** `APPROVE` -- 0 BLOCK + 0 MINOR + 0 NIT (0 total).

**Outcome.** SESSION-B's local Round 3 closes at APPROVE. The
addendums at `tmp/SESSION-B/vocabulary-SESSION-B.md` and
`tmp/SESSION-B/discussions-SESSION-B.md` are ready for the merge
agent to lift into the shared `vocabulary.md` and `discussions.md`
per the Section 0 merge instructions.

**Handoff to the merge agent.**

1. Apply the rename sweep `Ty::RawPtr` -> `Ty::Ptr` per the
   addendum's Section 0.1 table, including the explicitly-called-
   out sites in `vocabulary.md` Sections 1.6, 3.3, and 4.1, and
   `expectations.md:65-69`.
2. Lift the new D7/D8 audit-shape entry (Section 1.D7-D8) and the
   new D15 audit-shape entry (Section 1.D15) into `vocabulary.md`
   Section 1 with merge-agent-assigned final section numbers.
3. Replace the Section 4.1 worked example and append the new
   Section 4.1.1 edge-cases table.
4. Replace the D7, D8, D15 rows in the Section 5 open-decisions
   table with the LOCKED entries from the addendum.
5. Lift the distilled D7/D8 and D15 [LOCKED] entries from
   `discussions-SESSION-B.md` into `discussions.md` Section 1,
   replacing the existing [OPEN] entries at lines 472-494 (D7/D8)
   and 650-660 (D15).
6. Mark the D7/D8 expectation at `expectations.md:65-69` resolved
   to the locked two-variant alphabet.
7. Append a "Round 3, SESSION-B" header to the shared
   `stage-3/review-log.md` and inline the contents of this
   `review-log-SESSION-B.md` (sub-rounds 3.1 through 3.5 plus
   trajectory summary).
8. Sweep the bridge code (`crates/`) only AFTER the spec rename
   is complete -- the addendum notes the bridge code is out of
   spec sweep scope; treat it as a follow-on patch in the
   stage-3-or-later implementation work.
9. After the merge, run a comprehensive cross-cluster review
   (Round 4 of the shared cadence) that includes SESSION-B's
   contributions plus all other parallel sessions' contributions.
