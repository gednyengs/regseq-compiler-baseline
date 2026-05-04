# Review log

Merged top-level Codex adversarial-review trail for RegseqModel
stage closures. Per README §5.5 step 2, this file is assembled
by the merger at end-of-stage and contains the per-stage
`review-log.md` content verbatim under per-stage headers
(mechanical concat). Per-stage review logs live in
`stage-N/review-log.md` and are the authoritative artefact
during stage work; this file is the merged record across
closed stages.

Reviews are invoked via `scripts/python/codex_runner.py` per
the project's CLAUDE.md contract. Each round uses a fresh
prompt; re-reviews include a brief summary of what was changed
since the prior round.

The structured output contract for every round:
- First line: `VERDICT: <BLOCK | MINOR | APPROVE>`
- Numbered issues list, each tagged `[BLOCK | MINOR | NIT]`
  with a decision reference, file:line citation, 1-3-sentence
  description, and a 1-sentence suggested fix.
- Last line: `TOTAL: <N> issues (<count of BLOCK> BLOCK /
  <count of MINOR> MINOR / <count of NIT> NIT)`.

Stage 1 used legacy F-tag form and did not produce a
`review-log.md` under the new convention. Stages 3 and 4 are
the first two stages to close under the new convention per
README §5.5 (the append-if-exists rule); this file was created
on stage 4's merger (the second stage to close).

---

## Stage 3 (type system)

# Stage 3 -- review log

Iterative Codex adversarial-review rounds for the stage-3 type-system
spec at `spec-snapshot.md`. Each entry records the round's scope, verdict,
issue list, and the corrections applied before the next round.

Reviews are invoked via `scripts/python/codex_runner.py` per the project's
CLAUDE.md contract. Each round uses a fresh prompt (Codex has no session
continuation); re-reviews include a brief summary of what was changed
since the prior round.

The structured output contract for every round:
- First line: `VERDICT: <BLOCK | MINOR | APPROVE>`
- Numbered issues list, each tagged `[BLOCK | MINOR | NIT]` with a
  decision reference, file:line citation, 1-3-sentence description,
  and a 1-sentence suggested fix.
- Last line: `TOTAL: <N> issues (<count of BLOCK> BLOCK / <count of
  MINOR> MINOR / <count of NIT> NIT)`.

---

## Round 1 -- D1.1-D1.6 initial review

**Date.** 2026-04-29.

**Scope.** Locked decisions D1.1 through D1.6 (variant scope, ADT
kind separation, Option/Result handling, rejection list, lang-item
flags, auxiliary variant decisions). Section 3 worked examples were
in scope as supporting evidence. D2 (locked but content open) and
D3-D16 (open) were out of scope.

**Codex run.** `--effort high`, `--timeout 1500`, output captured to
`/tmp/codex_d1_review_r1.txt`. Total runtime: a few minutes.

**Verdict.** `BLOCK` -- 1 BLOCK + 8 MINOR + 0 NIT (9 total issues).

**Issues.**

1. *[BLOCK] D1.5 / Section 3.1.* Lowering example showed `restrict`
   on `&u32` (shared borrow). Methodology rule (`README.md`,
   "C semantics with information preservation") ties `restrict` to
   `&mut T` / `UniqueBorrow` only; shared `&T` does not get
   `restrict`.
2. *[MINOR] D1.5.* `Pin` listed as v1 flag with "no codegen impact"
   and "forward compatibility" rationale -- conflicts with
   "no defensive fallbacks" and "ask who consumes it" methodology
   rules.
3. *[MINOR] D1.5.* Prior-art evidence claimed `AdtFlags` covers all
   five v1 flags, but `MaybeUninit` is not cached in `AdtFlags`
   (queried via `tcx.is_lang_item` per
   `findings-rustc-draft.md` Theme D.1).
4. *[MINOR] D1.5.* Charon citation referred to `BuiltinImplData` for
   lang-item recognition; `BuiltinImplData` actually covers
   built-in/auto traits (Sized, Send, etc.), not ADTs like
   UnsafeCell (`prior-art/charon.md` Theme D).
5. *[MINOR] D1.5 / Section 3.6.* Drop-suppression claim for
   `MaybeUninit` not supported by cited prior art -- Charon and
   mrustc treat `MaybeUninit` as ordinary ADT.
6. *[MINOR] D1.1 / D1.4.* Rejection lists conflated non-rigid
   variants (cannot survive monomorphization) with rigid-but-DSL-
   rejected variants (Pat, UnsafeBinder, CoroutineWitness). Per
   `findings-rustc-draft.md` Theme A.1, `is_known_rigid()` returns
   true for all three.
7. *[MINOR] D1.1.* "MLIR 14 builtin" cited in convergent vocabulary
   size, but MLIR's type system is open / dialect-extensible, not a
   closed enum (`prior-art/mlir.md` Theme A).
8. *[MINOR] D1.4.* Foreign rejection rationale included claims
   about instability, no-stabilization-path, Linux conventions, and
   opaque-struct-as-canonical-alternative that were not grounded in
   any cited evidence file.
9. *[MINOR] D1.6.c.* `Never` lowering text said C `void` is
   "equivalent in semantics: function never returns" *and*
   required `__attribute__((noreturn))` -- internally inconsistent
   (`void` is a return-type spelling; `noreturn` is the never-
   returns attribute; they are complementary, not equivalent).

**Corrections applied to `spec-snapshot.md`.**

- *Issue 1.* Rewrote Section 3.1 lowering table: removed `restrict`
  from `&u32`; clarified that `restrict` is added only for `&mut`
  (UniqueBorrow) "where uniqueness is preserved" per the methodology
  rule. Added prose explaining `const` and `restrict` are governed
  by separate axes (UnsafeCell affects `const`; borrow flavor affects
  `restrict`). Section 3.5 (UnsafeCell) similarly reworded -- the
  flag drops `const`, not "both".
- *Issue 2.* Removed `Pin<P>` from the D1.5 flag table. Added a
  "Lang items not flagged in v1" subsection explaining: no codegen
  consumer for synchronous driver code; recognizing without a
  consumer violates the methodology rule. Pin is treated as ordinary
  `Ty::Struct` for v1 and reintroduced when async support arrives.
- *Issues 3 + 5.* Removed `MaybeUninit<T>` from the D1.5 flag table.
  The drop-suppression behavior is automatic via union-no-auto-drop
  + the inner `is_manually_drop` flag (composition of two existing
  rules). Added explanation in D1.5 "Lang items not flagged in v1"
  and rewrote Section 3.6 to walk through the structural derivation.
- *Issue 4.* Removed the `BuiltinImplData` citation from D1.5
  prior-art. Replaced with the simpler accurate claim: Charon treats
  the relevant lang-items as ordinary ADTs.
- *Issue 6.* Restructured D1.4 into two explicit categories:
  *Category A -- Rigid but DSL-rejected* (Foreign, Box, Coroutine
  family, Pat, UnsafeBinder) and *Category B -- Non-rigid (cannot
  survive monomorphization)* (Param, Bound, Placeholder, Infer,
  Error, Alias::*). Each Category-B entry cites the rustc-draft
  theme that establishes its non-survival. D1.1 "rejection
  reasoning" subsection rewritten to list only the non-rigid /
  cannot-survive group; it now defers to D1.4 for the formal
  complete list and explicitly notes that some rigid variants are
  also rejected for DSL scope.
- *Issue 7.* MLIR removed from the numeric convergence claim;
  separated as an open-vocabulary data point with explicit citation
  (`prior-art/mlir.md` Theme A).
- *Issue 8.* Foreign rationale narrowed to grounded claims: no
  surveyed driver / Rust-to-target project carries a `Foreign`
  variant; the opaque-struct pattern is universally supported.
  Removed unsupported claims about instability timeline and Linux
  kernel conventions.
- *Issue 9.* Never lowering rewritten to make `void` (return-type
  spelling) and `__attribute__((noreturn))` (non-return semantics)
  *complementary, not equivalent*. The full lowering of
  `fn diverge() -> !` is shown as
  `__attribute__((noreturn)) void diverge(void);`.

D1.5's flag count moved from five (PhantomData, UnsafeCell,
MaybeUninit, ManuallyDrop, Pin) to three (PhantomData, UnsafeCell,
ManuallyDrop). Variant count in `Ty` is unchanged at 19 (flags are
on `Item`, not on `Ty`).

**Status.** Round 2 prepared; pending Codex re-review.

---

## Round 2 -- D1.1-D1.6 re-review after round-1 fixes

**Date.** 2026-04-29.

**Scope.** Same as round 1 (D1.1-D1.6 + Section 3 worked examples).
Round 2 prompt explicitly asked Codex to (a) verify each round-1 fix
via the cited evidence, (b) check for new issues introduced by the
corrections, and (c) sweep for remaining issues round 1 may have
missed.

**Codex run.** `--effort high`, `--timeout 1500`, output captured to
`/tmp/codex_d1_review_r2.txt`. Total runtime: a few minutes.

**Verdict.** `MINOR` -- 0 BLOCK + 6 MINOR + 1 NIT (7 total issues).
No round-1 fix flagged as incorrect.

**Issues.**

1. *[MINOR] D1.2 / cross-decision.* D1.2 still listed all five
   pre-correction lang-item flags (Box, PhantomData, UnsafeCell,
   MaybeUninit, ManuallyDrop, Pin) as "recognized via flags," but
   D1.5 had been reduced to three. Stale cross-reference from
   round-1 fixes that did not propagate.
2. *[MINOR] D1.4.* Category A intro asserted that all entries
   "could survive monomorphization," but `UnsafeBinder` is unstable
   / typeck-leaning per `findings-rustc-draft.md` (Theme A.1 watch-
   out). The category framing was over-strong.
3. *[MINOR] D1.1.* Convergent vocabulary claim ("all surveyed
   closed vocabularies sit between 14 and 22") did not reckon with
   *rustc-codegen-c* (6 variants -- but project incomplete) or
   *CompCert* (8 front-end constructors -- but C is the source).
4. *[MINOR] D1.5.* Internal inconsistency: D1.5 prose first said
   the bridge calls `tcx.is_lang_item(...)` for v1 flags, then
   later said the bridge uses `AdtFlags` predicates. Both claims
   were present.
5. *[MINOR] D1.6.c.* `c_void` description still said "ordinary Adt
   with a lang-item flag" and pointed to a nonexistent D1.5
   "Notable adjacent rejections" heading (renamed to "Lang items
   not flagged in v1" in round 1).
6. *[MINOR] D1.5 prior art.* Charon citation claimed alignment
   with "flag-on-Item, not separate Ty variant," but the evidence
   only supports the "not separate Ty variant" half -- Charon does
   not flag these on its decl side either.
7. *[NIT] Section 3.6.* The `MaybeUninit<T>` structural form
   (`union { uninit: (), value: ManuallyDrop<T> }`) was correctly
   stated but lacked a primary-source citation to `core`.

**Corrections applied to `spec-snapshot.md`.**

- *Issue 1.* D1.2 paragraph rewritten to name only the three v1
  flags (PhantomData, UnsafeCell, ManuallyDrop) and to explicitly
  cross-link D1.5 for the unflagged-in-v1 set (Pin, MaybeUninit,
  CVoid).
- *Issue 2.* D1.4 Category A intro reworded: "rigid per
  `is_known_rigid()`" replaces "could survive monomorphization,"
  with a parenthetical noting that some entries (`UnsafeBinder`)
  are unstable and unlikely to surface in optimized MIR.
- *Issue 3.* D1.1 convergent-vocabulary paragraph qualified to
  "Rust- or LLVM-rich closed-vocabulary projects." rustc-codegen-c
  (incomplete) and CompCert (C source language) added as
  non-comparable lower-level data points with citations.
- *Issue 4.* D1.5 recognition prose rewritten: bridge uses
  `AdtFlags` predicates for the three v1 flags; `tcx.is_lang_item`
  reserved for unflagged / deferred lang items. The duplicate
  "Recognition mechanism (per flag)" subsection collapsed to a
  short cross-reference table to the `AdtFlags` bits.
- *Issue 5.* D1.6.c `c_void` paragraph rewritten to describe
  `c_void` as the `CVoid` lang-item ADT (zero-variant enum, per
  `findings-rustc-draft.md` Theme D.2), to cross-reference D1.5
  "Lang items not flagged in v1," and to defer the flag-or-not
  question to D16.
- *Issue 6.* D1.5 Charon citation reworded: Charon supports the
  "*not* a separate `Ty` variant" half; the flag-on-`Item`
  storage is our own choice (Charon does not flag these on its
  decl side either).
- *Issue 7.* Section 3.6 MaybeUninit structural form citation
  added: `library/core/src/mem/maybe_uninit.rs:345-348` (verbatim
  from the pinned toolchain).

**Status.** Round 3 prepared; pending Codex re-review. Goal: APPROVE.

---

## Round 3 -- D1.1-D1.6 re-review after round-2 fixes

**Date.** 2026-04-29.

**Scope.** Same as round 1 + 2.

**Codex run.** `--effort high`, `--timeout 1500`, output captured to
`/tmp/codex_d1_review_r3.txt`. Total runtime: a few minutes.

**Verdict.** `MINOR` -- 0 BLOCK + 4 MINOR + 0 NIT (4 total issues).
Trend is monotone-improving across rounds (9 -> 7 -> 4). No round-2
fix was flagged as incorrect; remaining issues are residual nuance
the prior rounds either did not surface or under-corrected.

**Issues.**

1. *[MINOR] D1.4 / round-2-fix.* Round-2 Category A intro still
   asserted entries "are not eliminated by typeck or
   monomorphization," which is too strong for `UnsafeBinder` (rigid
   per `is_known_rigid()` but typeck-only/unstable per
   `findings-rustc-draft.md` Theme A.1 W.7).
2. *[MINOR] D1.1 / round-2-fix.* CompCert variant count given as 8
   in the round-2 prose, but the evidence enumerates 9 `Ctypes.type`
   constructors (`Tvoid, Tint, Tlong, Tfloat, Tpointer, Tarray,
   Tfunction, Tstruct, Tunion`) per `prior-art/compcert.md` Theme A.
3. *[MINOR] D1.5 / round-2-fix.* The "tcx.is_lang_item reserved
   for unflagged or deferred" prose listed `Pin` and `OwnedBox`
   alongside the truly-unflagged set (`MaybeUninit`, `CVoid`,
   `VaList`), but `Pin` is cached as `IS_PIN` and `OwnedBox` as
   `IS_BOX` in `AdtFlags` per `findings-rustc-draft.md` Theme D.1.
4. *[MINOR] D1.4.* The Coroutine-family rejection rationale relied
   on uncited domain claims (Linux work queues, Zephyr message
   queues, Embassy domain). Local evidence supports a narrower
   `#![no_std]` / no-runtime-facilities scope rule
   (`regseq-background.md:34-38`) and the prior `async fn`
   rejection at `stage-1/spec-snapshot.md:81`.

**Corrections applied to `spec-snapshot.md`.**

- *Issue 1.* D1.4 Category A intro reworded to drop the
  "not eliminated by..." survivability claim entirely. New framing:
  "rigid per `is_known_rigid()`, placing them on the same side of
  the partition as the variants we accept; rejected here for v1
  driver-DSL scope reasons." `UnsafeBinder` footnote retained
  (rigid by partition; unstable; rejected anyway).
- *Issue 2.* CompCert count corrected from 8 to 9 with the
  enumerated constructor list inline.
- *Issue 3.* D1.5 recognition-mechanism prose split into two
  paths: (a) `AdtFlags` predicates -- used uniformly for
  PhantomData / UnsafeCell / ManuallyDrop (v1 flags) AND for
  rejection diagnostics (`OwnedBox` -> `IS_BOX`) AND for future
  `Pin` recognition (`IS_PIN`); (b) `tcx.is_lang_item(...)` --
  reserved strictly for lang items NOT cached in `AdtFlags`
  (`MaybeUninit`, `CVoid`, `VaList`). The reservation is now
  evidence-accurate.
- *Issue 4.* Coroutine-family reasoning rewritten to cite
  `regseq-background.md:34-38` (no_std, limited core, no runtime
  facilities) and `stage-1/spec-snapshot.md:81` (the established
  `async fn` rejection). Removed the uncited Linux/Zephyr/Embassy
  domain examples.

**Status.** Round 4 prepared; pending Codex re-review. Goal: APPROVE.

---

## Round 4 -- D1.1-D1.6 re-review after round-3 fixes

**Date.** 2026-04-29.

**Scope.** Same as round 1-3.

**Codex run.** `--effort high`, `--timeout 1500`, output captured to
`/tmp/codex_d1_review_r4.txt`. Total runtime: a few minutes.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total issue).
Trend continues monotone-improving (9 -> 7 -> 4 -> 1).

**Issues.**

1. *[MINOR] D1.1.* Variant counts in the convergent-vocabulary
   evidence paragraph were stale: rustc `TyKind` cited as 28 (table
   in `findings-rustc-draft.md` lines 31-59 enumerates 29 rows
   including `Error`); mrustc cited as 16 (`TypeData` enumerated at
   `prior-art/mrustc.md:47-83` lists 15 named variants).

**Corrections applied to `spec-snapshot.md`.**

- *Issue 1.* Counts updated: rustc `TyKind` `28` -> `29` (with
  table-row line range cited inline); mrustc `16` -> `15` (with
  the named variant list inlined to make the count auditable in
  place). The "between 14 and 22 closed variants" band claim is
  unaffected by either correction.

**Status.** Round 5 prepared; pending Codex re-review. Goal: APPROVE.

---

## Round 5 -- D1.1-D1.6 re-review after round-4 fix

**Date.** 2026-04-29.

**Scope.** Same as rounds 1-4.

**Codex run.** `--effort high`, `--timeout 1500`, output captured to
`/tmp/codex_d1_review_r5.txt`.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total issue).
The round-4 fix to D1.1 was confirmed correct, but a stale
"16-variant TypeData" line in Section 5.2 (References) had not been
updated -- the round-4 sweep only updated the D1.1 paragraph.

**Issues.**

1. *[MINOR] D1.1 / round-4-fix.* Section 5.2 mrustc summary still
   said "16-variant TypeData" while D1.1 was already corrected to
   15.

**Corrections applied to `spec-snapshot.md`.**

- *Issue 1.* Section 5.2 mrustc summary updated: "16-variant
  TypeData" -> "15-variant TypeData."

**Status.** Round 6 prepared; pending Codex re-review. Goal: APPROVE.

---

## Round 6 -- D1.1-D1.6 final re-review

**Date.** 2026-04-29.

**Scope.** Same as rounds 1-5.

**Codex run.** `--effort high`, `--timeout 1500`, output captured to
`/tmp/codex_d1_review_r6.txt`.

**Verdict.** `APPROVE` -- 0 BLOCK + 0 MINOR + 0 NIT (0 issues).

The D1 review-correction loop has converged. D1.1 through D1.6 are
locked. No further D1 review rounds are required before the spec
moves on to D3/D11.

**Trend across all six rounds.**

| Round | Verdict | BLOCK | MINOR | NIT | Total |
| --- | --- | --- | --- | --- | --- |
| 1 | BLOCK | 1 | 8 | 0 | 9 |
| 2 | MINOR | 0 | 6 | 1 | 7 |
| 3 | MINOR | 0 | 4 | 0 | 4 |
| 4 | MINOR | 0 | 1 | 0 | 1 |
| 5 | MINOR | 0 | 1 | 0 | 1 |
| 6 | APPROVE | 0 | 0 | 0 | 0 |

Monotone-improving issue counts; verdict shifted from BLOCK to
MINOR at round 2 and stayed MINOR through round 5 as residual
small accuracy issues were swept; APPROVE at round 6.

**Status.** D1 LOCKED. Loop closed.

---

## Round 2.1 -- D2 + D3 + D4 + D11 + D12 initial review

**Date.** 2026-04-29.

**Scope.** Structural-foundation cluster: D2 (already locked but
included for cross-cluster consistency), D3 (interning strategy),
D4 (interning location), D11 (layout content subset), D12 (niche
optimization preservation -- formal corollary of D11). Spec under
review: `spec-snapshot.md` Sections 2 (Layout), 3 (Identity and
interning), 4.4 (niche example, pre-existing), 4.8 (packed-struct
example, new), 5 (Open-decisions table updates). Synthesis:
`discussions.md` Section 1 (D3, D4, D11, D12 entries) + Section
2.4 (repr enumeration) + Section 2.5 (TyInterner walkthrough).

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_d3d4d11d12_review_r2.txt`.

**Verdict.** `BLOCK` -- 2 BLOCK + 6 MINOR + 0 NIT (8 total issues).

**Issues.**

1. *[BLOCK] D3 spec-snapshot.md:904.* Determinism circularity. Spec
   claimed "TyId is the canonical F4-2 fingerprint" while also
   claiming "TyIds are deterministic because the bridge walks
   instances in F4-2 order." Stage-1 (`stage-1/spec-snapshot.md:1247,
   :1254`) actually specifies that `ArgsKey` compares type args
   by structural `Ty` representation, with the canonical form
   pinned in stage 3 -- not by `TyId`. The "TyId IS the
   fingerprint" claim was an overclaim that produced the
   circularity.
2. *[BLOCK] D3 spec-snapshot.md:910.* "Children-before-parents"
   interning conflicts with recursive ADTs. `FieldLayout.ty: TyId`
   plus the deserialization "lower indices already present" rule
   cannot terminate on `struct Node { next: *mut Node }`. Rustc
   evidence (`findings-rustc-draft.md:174` citing
   `rustc_middle/src/ty/adt.rs:74-98`) explicitly shows rustc
   handles this by *not* embedding field types in `AdtDef`,
   keying ADT identity on `DefId` only.
3. *[MINOR] D4 discussions.md:850.* `lookup.get(...)` exposed to
   consumers but `TyInterner.lookup` is private and the public
   API has no lookup/find method.
4. *[MINOR] D3 spec-snapshot.md:844.* "TyId is locked stage 1 as
   `NonMaxU32`" overstates stage 1, which leaves the niche
   mechanism (NonZeroU32 / NonMaxU32 / rustc-style top-256) to
   implementation choice (`stage-1/spec-snapshot.md:117-128`).
5. *[MINOR] D11 spec-snapshot.md:659.* `TagScalar` always carries
   `valid_range`, but rustc's `Scalar::Union` has no `valid_range`
   (`findings-rustc-draft.md:337`). Need an invariant clarifying
   that enum tags are always sourced from `Scalar::Initialized`.
6. *[MINOR] D11 spec-snapshot.md:746.* `largest_niche` drop rationale
   ("parent-only consumer") needed primary citation or weakening
   to v1-consumer-analysis framing.
7. *[MINOR] D11 spec-snapshot.md:713.* `IS_C` drop rationale "observable
   from offsets matching source order" was unsound; non-`repr(C)`
   types can also have monotonic offsets. Fix: rationale should
   say "resolved offsets are authoritative; flag carries no
   information codegen needs once bytes are known."
8. *[MINOR] Cross-ref discussions.md:103, :180, :192-194, :423.*
   Several post-renumber `Section 3.x` references stale (Section
   3 was renumbered to Section 4 for worked examples; new Section
   3 is Identity and interning). Plus `Section 4 D-tag` references
   needed update to `Section 5 D-tag` (Open decisions table moved
   from Section 4 to Section 5).

**Corrections applied to `spec-snapshot.md`.**

- *Issue 1.* D3 reasoning rewritten. New constraint #1 grounds
  ArgsKey in stage-1's existing structural-Ty rule
  (`:1254-1257`). Cross-build claim weakened: "InstanceId
  ordering is reproducible across builds; integer TyId values are
  content-addressed within a build but not guaranteed identical
  across builds; snapshot tests rely on pretty-printer's
  content-based rendering, not TyId integer stability."
- *Issue 2.* D3 identity rule rewritten as **nominal/structural
  split**: ADT-shaped variants (Struct/Enum/Union/FnDef/Closure)
  hash + compare by `(item, args)` only, with Layout excluded
  from `Hash + Eq`. All other variants use structural derive.
  Construction invariant 3 added: ADT forward references via
  TyId reservation -- bridge reserves `TyId(K)` for the ADT,
  then computes layout (which may circle back via the
  nominal-identity hash), then patches the layout into
  `storage[K]`. Lookup key remains valid because identity hash
  excludes layout. Worked self-referential trace added at the
  end of Section 3.3 walkthrough (`struct Node { next: *mut
  Node }`).
- *Issue 3.* `find(&self, ty: &Ty) -> Option<TyId>` added to
  `TyInterner`'s public API as a non-mutating structural-identity
  lookup. Prose at Section 3.2 + Section 2.5 walkthrough
  (discussions.md) updated to use `find` instead of
  `lookup.get`.
- *Issue 4.* D3 wording reworded: "u32-backed newtype with
  niche-optimised `Option<TyId>` (stage-1 lock at `:117-128`;
  the *mechanism* is implementation choice). Stage 3 chooses
  `NonMaxU32`."
- *Issue 5.* `TagScalar` got an invariant comment specifying
  it always sources from `Scalar::Initialized`; `Scalar::Union`
  at a tag position is rejected (per "no defensive fallbacks").
- *Issue 6.* `largest_niche` rationale rewritten as a v1
  consumer analysis: "no analysis or codegen pass currently
  consumes a child type's `largest_niche`; enum codegen reads
  the parent's `tag_encoding`, which is a resolved decision
  rustc has already made; Charon similarly omits."
- *Issue 7.* IS_C / IS_TRANSPARENT mapping-table cell rewritten:
  "the resolved `(size, align, offsets)` are authoritative for C
  emission; the flags themselves carry no information codegen
  needs once the bytes are known."

**Corrections applied to `discussions.md`.**

- *Issue 8.* Stage-3 `Section 3.x` references at lines 103-104,
  180, 192-194, 422-423 updated to the post-renumber targets
  (worked examples now at Section 4.x; D7/D8 worked-C lowering
  at Section 4.1; provenance refs at Section 5 D7/D8). Plus
  `Section 4 D` → `Section 5 D` at lines 376, 398, 437, 451,
  559, 576, 589, 601 via batch sed (Open decisions table moved
  from Section 4 to Section 5).
- *Issue 1 + 2 follow-on.* D3 entry rewritten with
  nominal/structural identity rule and the F4-2-is-structural
  framing. Section 2.5 (TyInterner walkthrough) Determinism
  mechanism rewritten to ground in stage-1's structural ArgsKey
  rule; cross-build TyId stability claim weakened.

**Status.** Round 2.2 prepared; pending Codex re-review. Goal:
APPROVE.

---

## Round 2.2 -- D2 + D3 + D4 + D11 + D12 re-review after Round-2.1 fixes

**Date.** 2026-04-29.

**Scope.** Same as Round 2.1.

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_d3d4d11d12_review_r2_2.txt`.

**Verdict.** `BLOCK` -- 2 BLOCK + 5 MINOR + 0 NIT (7 total).
Issue count went from 8 → 7; Round-2.1 fixes for issues 4, 5, 7,
8 confirmed resolved. The two remaining BLOCKs are subtler
refinements of the originals.

**Issues.**

1. *[BLOCK] D3 spec-snapshot.md:920.* The F4-2 fix still conflates
   two notions of equality. Spec said "ArgsKey canonical form is
   exactly the identity rule above" but the identity rule for
   non-ADT variants uses TyId with shallow `Hash + Eq` (compares
   integer values). Combined with "TyId values not stable across
   builds," this makes ArgsKey not stable across builds either.
   Codex's suggested fix: define a *separate* deep canonicalizer
   that recursively resolves TyIds to type content via
   `model.types.resolve(...)`, using nominal `(item, args)` for
   ADTs to break cycles -- distinct from in-model `HashMap<Ty,
   TyId>` equality.
2. *[BLOCK] D4 spec-snapshot.md:1198.* Deserialize validation
   ("every referenced inner TyId must already be present at lower
   index") contradicts the recursive-ADT trace which produces
   forward references (`Node` at TyId(K) referencing `*mut Node`
   at TyId(K+1)). Codex's suggested fix: validate `id.index() <
   storage.len()` *after* the full array is loaded; permit
   forward references; rebuild `lookup` after deserialize.
3. *[MINOR] D3/D2 spec-snapshot.md:882.* On a `lookup` hit for an
   ADT, no explicit check that the excluded metadata (Layout)
   agrees. Rustc's `AdtDefData::eq` runs a debug-only deep-equality
   assertion for the same case
   (`rustc_middle/src/ty/adt.rs:127-132`).
4. *[MINOR] D3 discussions.md:804.* Section 2.5 still claimed
   `Ty` "derives `Hash + Eq + Clone`" with "structural derive is
   sufficient" -- contradicts the locked nominal/structural
   split.
5. *[MINOR] D4 discussions.md:344.* D4 entry's API listing still
   said "Public methods mirror StringInterner: new, intern,
   resolve, len, iter" -- missing `find` (added in Round 2.1).
6. *[MINOR] D11 spec-snapshot.md:664.* TagScalar invariant said
   `Scalar::Union` is "emitted only for union-shape fields, never
   for an enum's tag" but cited rustc evidence only establishes
   that `Variants::Multiple.tag` is a Scalar and Scalar::Union
   has no `valid_range` -- not the "never enum tag" claim. Fix
   by phrasing as a bridge invariant: reject if non-Initialized.
7. *[MINOR] D11 spec-snapshot.md:755.* `largest_niche` rationale
   said "the documented purpose of the field is to enable parent
   layouts" but cited evidence only lists the field, not its
   purpose. Drop "documented purpose"; rely on v1 consumer
   analysis only.

**Corrections applied to `spec-snapshot.md`.**

- *Issue 1.* D3 reasoning constraint #1 fully rewritten. New
  framing: "F4-2 ArgsKey uses a deep structural canonicalizer,
  distinct from the BiMap's in-build `Hash + Eq`." The
  canonicalizer walks Ty content, resolving every nested TyId
  via `model.types.resolve(...)`; for ADT-shaped variants it
  uses `(DefPathHash, args canonical)` to break self-reference
  cycles. The two notions of equality coexist because they
  answer different questions ("already interned?" vs "content
  equivalent across builds?"). Cross-build properties section
  rewritten to clarify ItemId/InstanceId stability does not
  depend on TyId integer stability.
- *Issue 2.* Section 3.2 deserialize-validation rewritten to
  permit forward references. Read full storage array first;
  validate `id.index() < storage.len()` *after* load; rebuild
  `lookup` after.
- *Issue 3.* Added bridge-side metadata-equality discipline note
  to the nominal-identity bullet: bridge does not recompute
  layout on hit (rustc layout query is functionally
  determined); in `cfg!(debug_assertions)` the bridge may run a
  deep-equality assertion mirroring rustc's `AdtDefData::eq`.
- *Issue 6.* TagScalar invariant rephrased as bridge invariant:
  "if `Scalar::Union` appears at the tag position, the bridge
  rejects the layout shape per 'no defensive fallbacks'." No
  uncited claims about rustc's emission behaviour.
- *Issue 7.* `largest_niche` rationale rewritten: drop
  "documented purpose" framing; rustc evidence only enumerates
  the field's presence; rely on v1 consumer analysis only;
  Charon citation reframed as independent corroboration.

**Corrections applied to `discussions.md`.**

- *Issue 1.* D3 entry "F4-2 ArgsKey is structural" prose
  rewritten to match vocabulary's deep-canonicalizer framing.
  Constraint #1 paraphrased to match.
- *Issue 4.* Section 2.5 derive language replaced with explicit
  custom `Hash + Eq` rule matching the nominal/structural split.
- *Issue 5.* D4 entry's public methods list updated to include
  `find(&self, &Ty) -> Option<TyId>` with a one-line note about
  it being an intentional asymmetry from `StringInterner`.

**Status.** Round 2.3 prepared; pending Codex re-review. Goal:
APPROVE.

---

## Round 2.3 -- D2 + D3 + D4 + D11 + D12 re-review after Round-2.2 fixes

**Date.** 2026-04-29.

**Scope.** Same as Rounds 2.1, 2.2.

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_d3d4d11d12_review_r2_3.txt`.

**Verdict.** `BLOCK` -- 2 BLOCK + 2 MINOR + 0 NIT (4 total).
Trajectory across Round 2 sub-rounds: 8 → 7 → 4. Round-2.2 fixes
for issues 3, 4, 5, 6 confirmed resolved. The two remaining
BLOCKs are completeness gaps in the deep-canonicalizer concept.

**Issues.**

1. *[BLOCK] D3 spec-snapshot.md:943.* The deep canonicalizer was
   incomplete. It only specified resolving `TyId`-typed fields,
   but `Ty::FnPtr(FnSigId)` carries a `FnSigId` (not `TyId`)
   pointing into `model.sigs`. `GenericArg::Type(TyId)` per
   stage-1 `:652-661` can target an `FnPtr`. F4-2 ArgsKey
   stability would break for any GenericArgs containing `FnPtr`
   or other ID-bearing variants the canonicalizer didn't cover.
2. *[BLOCK] D4 spec-snapshot.md:1251.* Deserialize step 4 said
   "Rebuild lookup by re-interning each Ty in storage order (or
   any order)." Re-interning would allocate fresh TyIds, breaking
   the index-based references in already-loaded Tys (e.g., the
   recursive-Node trace where TyId(K) references TyId(K+1)).
3. *[MINOR] D3 discussions.md:876.* Section 2.5 still said
   "ArgsKey canonical form is the nominal/structural identity
   rule pinned in D3" -- contradicts the Round-2.2 D3 fix that
   made the canonicalizer separate from BiMap equality.
4. *[MINOR] D11 spec-snapshot.md:768.* `largest_niche` rationale
   said Charon's omission "independently corroborates that the
   field is not load-bearing for byte-exact codegen." Charon's
   evidence (`charon.md:418-435`) supports "simplified verifier
   IR can omit it" but not "not load-bearing for byte-exact
   codegen" -- the byte-exact justification rests on the v1
   consumer analysis + retained parent `tag_encoding`, not on
   Charon's posture.

**Corrections applied to `spec-snapshot.md`.**

- *Issue 1.* D3 reasoning constraint #1 expanded to enumerate
  per-IR-identifier termination rules for the deep canonicalizer:
  `TyId` (resolve via `model.types`), `ItemId` (terminate at
  `DefPathHash`), `FnSigId` (expand `FnSig` contents and recurse
  on inner Tys), `StringId` (resolve to string content),
  `ConstId` (canonical const-eval result -- pinned in stage 4).
  Every ID-bearing field reachable from `Ty` is now covered.
- *Issue 2.* Section 3.2 deserialize step 4 rewritten: rebuild
  `lookup` by direct insertion -- `for each (id, ty) in
  storage.iter_enumerated(): lookup.insert(ty.clone(), id)` --
  not via the public `intern`. Preserves original TyId values.
- *Issue 4.* `largest_niche` Charon citation trimmed to
  "independent corroboration that an analysis/verifier IR can
  omit this field." Byte-exact-codegen claim now rests
  explicitly on v1 consumer analysis + retained parent
  `tag_encoding` (D11 + D12).

**Corrections applied to `discussions.md`.**

- *Issue 3.* Section 2.5 "Determinism mechanism" rewritten to
  describe the deep canonicalizer (with the per-IR-identifier
  termination rules) rather than referring to the BiMap identity
  rule as the canonical form. Adds the within-build short-circuit
  note explicitly.

**Status.** Round 2.4 prepared; pending Codex re-review. Goal:
APPROVE.

---

## Round 2.4 -- D2 + D3 + D4 + D11 + D12 re-review after Round-2.3 fixes

**Date.** 2026-04-29.

**Scope.** Same as Rounds 2.1-2.3.

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_d3d4d11d12_review_r2_4.txt`.

**Verdict.** `BLOCK` -- 2 BLOCK + 1 MINOR + 0 NIT (3 total).
Trajectory: 8 → 7 → 4 → 3. Round-2.3 fixes for issues 4 (Charon
overcite) and partial issue 2 (deserialize step 4 direct
insertion) confirmed resolved.

**Issues.**

1. *[BLOCK] D3 spec-snapshot.md:940.* The canonicalizer's "walks Ty
   content" framing combined with the `FieldLayout.ty` /
   `TagScalar.scalar_ty` traversal claim contradicted the
   "DefPathHash terminates ADT recursion" claim. ADT variants
   carry inline `Layout` whose `FieldLayout.ty` can point back
   at the parent (per the recursive-Node trace at spec-snapshot.md
   Section 3.3). If the canonicalizer literally walked into
   Layout, ADT recursion would still cycle. Need to make ADT
   non-traversal explicit: canonical encoding for ADT-shaped
   variants is exactly `(variant_kind_tag, DefPathHash,
   canonical_args)`; Layout is *not* visited.
2. *[BLOCK] D4 spec-snapshot.md:1256.* The serialization paragraph
   still said `lookup` is rebuilt "by re-interning each Ty in
   order", contradicting the corrected step 4 (direct insertion)
   which Round 2.3 had introduced. Inconsistency between adjacent
   paragraphs.
3. *[MINOR] D3 discussions.md:289.* The D3 entry summary in
   discussions.md still mentioned only `TyId` resolution + ADT
   `DefPathHash`, missing the `FnSigId` / `StringId` / `ConstId`
   termination rules added in Round 2.3 (which Section 2.5
   already mirrored). Stale summary.

**Corrections applied to `spec-snapshot.md`.**

- *Issue 1.* D3 reasoning constraint #1 restructured. Replaced
  the "walks Ty content" framing with **per-Ty-variant-kind
  rules**: ADT-shaped variants encode exactly as
  `(variant_kind_tag, DefPathHash, canonical_args)` with Layout
  explicitly *not traversed*; non-ADT structural variants
  recurse over their fields with the per-IR-identifier rules
  applied. Added a parenthetical to the per-IR-identifier rules
  block: "apply inside non-ADT structural recursion and inside
  `canonical_args` of ADT-shaped variants; *not* inside an ADT's
  inline `Layout`, which is skipped per the ADT rule above."
  Updated the `TyId` rule to clarify `FieldLayout.ty` and
  `TagScalar.scalar_ty` are not visited.
- *Issue 2.* Serialization paragraph rewritten: removed the
  "re-interning each Ty in order" language; now says the
  on-wire form is `storage` only and `lookup` is reconstructed
  at deserialize "by direct insertion -- *not* via the public
  `intern` API." Added a closing line explaining step 3's
  uniqueness validation prevents duplicate keys on the
  direct-insertion path.

**Corrections applied to `discussions.md`.**

- *Issue 3.* D3 entry's "F4-2 ArgsKey" prose rewritten to
  mirror spec-snapshot.md Section 3.1's per-Ty-variant-kind +
  per-IR-identifier rule structure. Added the full ID
  termination list (`TyId`/`ItemId`/`FnSigId`/`StringId`/
  `ConstId`) with one-line summaries; cross-references
  `spec-snapshot.md` Section 3.1 for full text.

**Status.** Round 2.5 prepared; pending Codex re-review. Goal:
APPROVE.

---

## Round 2.5 -- D2 + D3 + D4 + D11 + D12 re-review after Round-2.4 fixes

**Date.** 2026-04-29.

**Scope.** Same as Rounds 2.1-2.4.

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_d3d4d11d12_review_r2_5.txt`.

**Verdict.** `MINOR` -- 0 BLOCK + 2 MINOR + 0 NIT (2 total).
**Verdict shifted from BLOCK to MINOR.** Trajectory: 8 → 7 → 4
→ 3 → 2. Round 2.4's fixes (issues 1, 2, 3) confirmed resolved;
the two remaining MINORs are presentation issues in already-
correct logic.

**Issues.**

1. *[MINOR] D4 spec-snapshot.md:1202 / discussions.md:878-884.* The
   generic `translate_ty` pseudocode shows
   `build_ty_from_rustc(...)` recursing before
   `model.types.intern(our_ty)`, which is correct for non-ADT
   variants but doesn't show the ADT reservation path required
   by D3 invariant 3. Same pattern in discussions.md Section 2.5.
2. *[MINOR] D3 spec-snapshot.md:921.* The early recursive-Node
   explanation in Section 3.1 says "the layout's `FieldLayout.ty`
   for `next` then references the just-reserved `TyId(K)`," but
   `next: *mut Node` desugars to `Ty::Ptr(TyId, Mut)` whose
   inner is the pointee. So `FieldLayout.ty` carries `TyId(K+1)`
   (the `Ptr` type), and the `Ptr`'s inner is `TyId(K)` (Node).
   The Section 3.3 trace gets this right; Section 3.1's early
   explanation has the indirection wrong.

**Corrections applied to `spec-snapshot.md`.**

- *Issue 1.* Replaced the single-flow `translate_ty` pseudocode
  with two paths -- a generic non-ADT path (current `intern`-
  after-recursion logic) and an explicit ADT-shaped reservation
  path (`translate_adt`) that intern-reserves first, computes
  layout (recursing through the BiMap nominal hash), then patches
  via a `set_layout(id, layout)` helper. The ADT branch in
  `translate_ty` dispatches via `is_adt_shaped(rustc_ty)`. Added
  an explanatory comment that bridge-synthesised non-rustc types
  (closures, fat pointers, drop-glue) call into the same
  `intern`/`translate_adt` paths uniformly.
- *Issue 2.* Section 3.1 early Node explanation rewritten to
  show the correct two-step intern: first the pointer type
  `Ty::Ptr(TyId(K), Mut)` is interned as `TyId(K+1)` (its
  inner pointee is `TyId(K)`, terminating the recursion); then
  the `next` field's `FieldLayout.ty` carries `TyId(K+1)` (the
  pointer), not `TyId(K)` directly. Added cross-reference to
  Section 3.3 for the full intern sequence with explicit TyId
  assignments.

**Corrections applied to `discussions.md`.**

- *Issue 1 follow-on.* Section 2.5's pseudocode block updated to
  match: added comment header indicating it's the non-ADT path,
  added `if is_adt_shaped(rustc_ty) { return self.translate_adt(rustc_ty); }`
  dispatch, and a cross-reference to spec-snapshot.md Section 3.2
  for the ADT reservation flow.

**Status.** Round 2.6 prepared; pending Codex re-review. Goal:
APPROVE.

---

## Round 2.6 -- D2 + D3 + D4 + D11 + D12 re-review after Round-2.5 fixes

**Date.** 2026-04-29.

**Scope.** Same as Rounds 2.1-2.5.

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_d3d4d11d12_review_r2_6.txt`.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 issue).
Trajectory: 8 → 7 → 4 → 3 → 2 → 1.

**Issues.**

1. *[MINOR] D4 spec-snapshot.md:1234.* `translate_adt` always
   computed and patched layout after `intern(placeholder)`,
   but D3 invariant 3 plus the bridge-side metadata-equality
   discipline at Section 3.1 say nominal-identity hits return
   the existing TyId *without recomputing* layout. The public
   `intern` returns only `TyId` and cannot distinguish "fresh
   reservation" from "existing hit." Plus a duplicate
   synthesized-type paragraph with mutually inconsistent text
   about whether ADT-shaped synthesised types use `intern` or
   `translate_adt`.

**Corrections applied to `spec-snapshot.md`.**

- *Issue 1.* `translate_adt` pseudocode rewritten to use `find`
  before reserve. Sequence: (1) check `ty_cache`; (2) probe via
  `find(&probe_ty)` -- if hit, return existing TyId without
  recomputing or re-patching; (3) on miss, `intern(placeholder)`
  to reserve a fresh TyId, compute layout (may recurse and
  circle back via the find probe), `set_layout` once. The flow
  now matches D3 invariant 3 exactly.
- *Issue 1 cleanup.* Removed the duplicate synthesized-type
  paragraph. Single canonical statement: non-ADT synthesised
  types call `model.types.intern(...)` directly; ADT-shaped
  synthesised types (closures-as-structs per D6) go through
  `translate_adt`'s reservation flow with a synthesised
  `ItemId`. Clarified that fat-pointer structs (D5) and
  drop-glue (function decls) are non-ADT in our IR.

**Status.** Round 2.7 prepared; pending Codex re-review. Goal:
APPROVE.

---

## Round 2.7 -- D2 + D3 + D4 + D11 + D12 re-review after Round-2.6 fixes

**Date.** 2026-04-29.

**Scope.** Same as Rounds 2.1-2.6.

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_d3d4d11d12_review_r2_7.txt`.

**Verdict.** `MINOR` -- 0 BLOCK + 2 MINOR + 0 NIT (2 issues).
Slight regression in count (1 → 2) due to two distinct cleanup
gaps surfacing now that the substantive flow is correct.
Trajectory: 8 → 7 → 4 → 3 → 2 → 1 → 2.

**Issues.**

1. *[MINOR] D4 discussions.md:901-904.* Round-2.6 cleanup of the
   duplicate synthesized-types paragraph in spec-snapshot.md left
   the parallel paragraph in discussions.md Section 2.5
   untouched. discussions.md still said "closures, fat-pointer
   structs, drop-glue all skip ty_cache and call intern
   directly," contradicting spec-snapshot.md's canonical statement.
2. *[MINOR] D3/D4 spec-snapshot.md:1239.* `translate_adt`
   pseudocode unconditionally built the probe as `Ty::Struct {
   item, args, layout: pending() }`, but D3 declares ADT-shaped
   variants are *five* siblings (Struct/Enum/Union/FnDef/Closure
   per D1.2). For an Enum input, building a Struct probe would
   miss in `find`.

**Corrections applied to `discussions.md`.**

- *Issue 1.* discussions.md Section 2.5's synthesized-types
  bullet rewritten to match spec-snapshot.md: non-ADT synthesised
  types call `intern` directly (fat-pointer structs lower to
  `Ty::Ptr`/`Ty::Tuple` shapes -- non-ADT in our IR);
  ADT-shaped synthesised types (closures-as-structs per D6,
  becoming `Ty::Closure(synthesised_ItemId, GenericArgs)`) go
  through `translate_adt` with a synthesised `ItemId`. Drop-glue
  clarified as a function decl (not a Ty) that does not flow
  through TyInterner.

**Corrections applied to `spec-snapshot.md`.**

- *Issue 2.* `translate_adt` pseudocode rewritten to dispatch
  on ADT kind. Introduced `enum AdtKind { Struct, Enum, Union,
  FnDef, Closure }` and `adt_components` returning
  `(kind, item, args)`. Probe construction now uses `match
  kind` to build the matching sibling variant (`Ty::Struct`,
  `Ty::Enum`, `Ty::Union`, `Ty::FnDef`, `Ty::Closure`).
  Layout-bearing kinds (Struct/Enum/Union/Closure) compute and
  patch; FnDef is a ZST without Layout and skips the patch step.

**Status.** Round 2.8 prepared; pending Codex re-review. Goal:
APPROVE.

---

## Round 2.8 -- D2 + D3 + D4 + D11 + D12 re-review after Round-2.7 fixes

**Date.** 2026-04-29.

**Scope.** Same as Rounds 2.1-2.7.

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_d3d4d11d12_review_r2_8.txt`.

**Verdict.** `BLOCK` -- 1 BLOCK + 2 MINOR + 0 NIT (3 issues).
Trajectory: 8 → 7 → 4 → 3 → 2 → 1 → 2 → 3. Slight regression as
the Round-2.7 AdtKind dispatch surfaced inconsistencies elsewhere
in the spec.

**Issues.**

1. *[BLOCK] D2/D4 spec-snapshot.md:1263.* `translate_adt` patched
   layout for `Closure` as well, but `Ty::Closure(ItemId,
   GenericArgs)` has no layout field per D1.1 (spec-snapshot.md:71)
   and D2 (spec-snapshot.md:574-575) which restrict inline `Layout`
   to `Ty::Struct/Enum/Union`. Closure layout is D6 territory
   (still open); the captures struct will be a separately-
   synthesised `Ty::Struct`.
2. *[MINOR] D3 spec-snapshot.md:1072.* Construction invariant 3
   was stale relative to the AdtKind dispatch -- it hardcoded
   `Ty::Struct{...}` placeholder/probe text instead of
   describing the per-AdtKind sibling probe.
3. *[MINOR] D4 discussions.md:397.* The D4 summary parenthetical
   listed "drop-glue decls" as a synthesised Ty that flows
   through TyInterner. Inconsistent with the corrected Section
   2.5 statement that drop-glue is a function decl, not a Ty.

**Corrections applied to `spec-snapshot.md`.**

- *Issue 1.* `translate_adt` layout-patching match changed from
  `Struct | Enum | Union | Closure` to `Struct | Enum | Union`
  only. Added an in-pseudocode comment explaining: FnDef is ZST
  identity carrier with no layout slot; Closure carries no
  inline layout in v1 -- it references a separately-synthesised
  captures struct (per D6) which is its own `Ty::Struct` with
  its own layout, interned independently.
- *Issue 2.* Construction invariant 3 rewritten with
  per-AdtKind probe construction sequence. Probe is built per
  `kind`: layout-bearing variants (Struct/Enum/Union) use
  PENDING placeholders; non-layout-bearing variants
  (FnDef/Closure) use their final forms directly. Layout
  patching is restricted to the layout-bearing branch. Section
  3.3 cross-reference unchanged.

**Corrections applied to `discussions.md`.**

- *Issue 3.* D4 summary parenthetical rewritten to match the
  Section 2.5 / spec-snapshot.md split: ADT-shaped synthesised
  `Ty::Closure(synthesised_ItemId, GenericArgs)` per D6;
  non-ADT synthesised fat-pointer-shape Ty values per D5
  (lowering to `Ty::Ptr` / `Ty::Tuple`). Drop-glue
  explicitly excluded -- it's a function decl, not a Ty.

**Status.** Round 2.9 prepared; pending Codex re-review. Goal:
APPROVE.

---

## Round 2.9 -- D2 + D3 + D4 + D11 + D12 final re-review

**Date.** 2026-04-29.

**Scope.** Same as Rounds 2.1-2.8.

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_d3d4d11d12_review_r2_9.txt`.

**Verdict.** `APPROVE` -- 0 BLOCK + 0 MINOR + 0 NIT (0 issues).

The Round 2 review-correction loop has converged. D2 (already
locked at Round 1) is re-confirmed in the structural-foundation
context; D3, D4, D11, D12 are locked.

Codex's verification statement: "`translate_adt` now patches
layout only for `Struct | Enum | Union` (`spec-snapshot.md:1276-1288`),
invariant 3 matches that layout-bearing/non-layout-bearing split
(`spec-snapshot.md:1072-1102`), and the D4 synthesised-type split
excludes drop-glue from `TyInterner` (`discussions.md:397-402`,
`discussions.md:905-915`, `spec-snapshot.md:1293-1304`)."

**Trend across all nine Round 2 sub-rounds.**

| Round | Verdict | BLOCK | MINOR | NIT | Total |
| --- | --- | --- | --- | --- | --- |
| 2.1 | BLOCK | 2 | 6 | 0 | 8 |
| 2.2 | BLOCK | 2 | 5 | 0 | 7 |
| 2.3 | BLOCK | 2 | 2 | 0 | 4 |
| 2.4 | BLOCK | 2 | 1 | 0 | 3 |
| 2.5 | MINOR | 0 | 2 | 0 | 2 |
| 2.6 | MINOR | 0 | 1 | 0 | 1 |
| 2.7 | MINOR | 0 | 2 | 0 | 2 |
| 2.8 | BLOCK | 1 | 2 | 0 | 3 |
| 2.9 | APPROVE | 0 | 0 | 0 | 0 |

Verdict shifted from BLOCK to MINOR at round 2.5 once the deep-
canonicalizer concept and the deserialize forward-reference rule
landed. Round 2.7→2.8 saw a regression to BLOCK (1→2→3 issues)
when the AdtKind dispatch fix in 2.7 surfaced previously-hidden
inconsistencies (Closure layout-bearing claim, stale construction
invariant 3, drop-glue parenthetical). Those landed in 2.8;
Round 2.9 closes at APPROVE.

**Substantive design refinements that emerged from this loop.**

- *Deep canonicalizer separate from BiMap equality.* F4-2
  ArgsKey uses a recursive walk over `Ty` content that resolves
  every nested IR identifier (TyId via `model.types.resolve`;
  ItemId terminating at DefPathHash; FnSigId expanding to FnSig
  contents; StringId to string content; ConstId to canonical
  const-eval result -- the last pinned in stage 4) and stops at
  `(DefPathHash, canonical_args)` for ADT-shaped variants.
  Distinct from the BiMap's in-build `Hash + Eq`, which compares
  TyId integers shallowly.
- *Nominal/structural identity split.* ADT-shaped variants
  (Struct/Enum/Union/FnDef/Closure) hash and compare by
  `(item, args)` only -- Layout excluded from `Hash + Eq`. Non-
  ADT variants use structural derive. Mirrors rustc's `AdtDefData`.
- *ADT forward references via TyId reservation.* The bridge
  reserves a TyId for ADT-shaped variants before computing
  layout. `translate_adt` uses `find` before reserve; layout-
  patching applies only to layout-bearing variants
  (Struct/Enum/Union). Recursive ADTs terminate via the BiMap's
  nominal-identity hash.
- *Deserialize permits forward references.* Storage array is
  read fully first, then references validated by index range
  alone (no order requirement). `lookup` rebuilt by direct
  insertion preserving original TyIds.
- *`pack: Option<u32>` is the only repr escape.* All other repr
  effects derive from resolved `(size, align, offsets)`; `pack`
  is the only one C compilers won't reproduce without an
  explicit `__attribute__((packed))`.
- *Tag-encoding fidelity.* `TagEncoding::Niche` retains
  `(untagged_variant, niche_variants, niche_start)` -- diverges
  from Charon (which keeps untagged_variant only) because we
  emit byte-exact C codegen; Charon's verifier-IR posture allows
  the simplification but ours doesn't.

**Status.** Round 2 LOCKED. Cluster D2 + D3 + D4 + D11 + D12
locked. Loop closed.

---

## Round 3, SESSION-B -- D7, D8, D15

Iterative Codex adversarial-review rounds for the SESSION-B
addendums (D7 + D8 + D15) prior to merge. Each entry records the
round's scope, verdict, issue list, and the corrections applied
before the next round. SESSION-B's local cadence ran independently
from the shared cadence; once it closed at APPROVE, the merge agent
folded the rounds into this shared log under this header.

The structured output contract for every round:
- First line: `VERDICT: <BLOCK | MINOR | APPROVE>`
- Numbered issues list, each tagged `[BLOCK | MINOR | NIT]` with a
  decision reference, file:line citation, 1-3-sentence description,
  and a 1-sentence suggested fix.
- Last line: `TOTAL: <N> issues (<count of BLOCK> BLOCK / <count of
  MINOR> MINOR / <count of NIT> NIT)`.

The Codex prompt included both the shared stage-3 documents
(`spec-snapshot.md`, `discussions.md`, `review-log.md`,
`findings-rustc-draft.md`, `prior-art/*.md`) **and** the SESSION-B
addendums so Codex could verify that the addendums were internally
consistent with the locked spec context. Codex was instructed to
review only the SESSION-B addendums; cross-cluster sweep across all
parallel sessions is the merge agent's responsibility.

### Round 3 (SESSION-B local) -- closed at APPROVE (2026-04-29)

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

#### Sub-round 3.1

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
   context (`stage-1/spec-snapshot.md:77, 670-673`) places mangling
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
   Sweep misses locked Section 1.6 D1.6.a at `spec-snapshot.md:510-
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
   citing `stage-1/spec-snapshot.md:77` (Linkage on Item) and `:670-
   673` (Instance.symbol_name). Worked-example C-emission revised
   to show that `FnAbi::C` without `#[no_mangle]` linkage still
   produces a mangled symbol; only `#[no_mangle]` linkage drives
   the unmangled spelling.
3. Count phrasing (issue 3) -- changed "[any other 25 ExternAbi
   variants]" to "[all non-Rust / non-C ExternAbi variants]" in
   both bridge tables; updated the "26 rejected ABIs" methodology
   bullet to use the same phrasing.
4. D1.6 sweep (issue 4) -- expanded the rename-sweep table row
   for `spec-snapshot.md` to call out lines 510-514 and the specific
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

#### Sub-round 3.2

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
   which correctly cites `stage-1/spec-snapshot.md:77, 670-673`.

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

#### Sub-round 3.3

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

#### Sub-round 3.4

**Date.** 2026-04-29.

**Codex run.** `--effort high`, `--timeout 1500`. Output captured
to `/tmp/codex_session_b_review_r3.4.txt`.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).

**Issues raised.**

1. [MINOR] D8 (sweep miss in Section 3.3 TyInterner walkthrough).
   The merge sweep covers Section 1.1, Section 1.6, and Section
   4.1 of the locked `spec-snapshot.md`, but misses the locked
   Section 3.3 TyInterner construction walkthrough at
   `spec-snapshot.md:1442, 1444, 1452, 1453, 1465`, which still
   builds and compares `Ty::Ref(TyId(0), Mutability::Mut,
   Provenance::UniqueBorrow)` and shorthand `Ty::Ref(0, Mut,
   Unique)` -- the pre-SESSION-B 3-tuple shape. If merged as
   written, Section 3.3 stays stale against the new 2-tuple
   `Ref(TyId, Provenance)` arity.

**Corrections applied.**

1. Section 3.3 sweep (issue 1) -- expanded the `spec-snapshot.md`
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

#### Sub-round 3.5

**Date.** 2026-04-29.

**Codex run.** `--effort high`, `--timeout 1500`. Output captured
to `/tmp/codex_session_b_review_r3.5.txt`.

**Verdict.** `APPROVE` -- 0 BLOCK + 0 MINOR + 0 NIT (0 total).

**Outcome.** SESSION-B's local Round 3 closed at APPROVE. The
addendums at `tmp/SESSION-B/vocabulary-SESSION-B.md` and
`tmp/SESSION-B/discussions-SESSION-B.md` were merged into the
shared `spec-snapshot.md` and `discussions.md` per the addendum's
Section 0 merge instructions: rename sweep `Ty::RawPtr -> Ty::Ptr`
applied across `spec-snapshot.md` (Sections 1.1, 1.6, 3.1, 3.2, 3.3,
4.1), `discussions.md`, and `review-log.md`; new audit-shape
entries lifted into `spec-snapshot.md` Section 1.8 (D7/D8) and
Section 1.9 (D15); Section 4.1 worked example replaced and
Section 4.1.1 edge-cases table appended; Section 5 D7/D8/D15 rows
flipped to LOCKED; `discussions.md` D7/D8 and D15 [OPEN] entries
replaced with [LOCKED] equivalents; `expectations.md:65-69`
marked resolved.

**Trend across all five SESSION-B sub-rounds.**

| Round | Verdict | BLOCK | MINOR | NIT | Total |
| --- | --- | --- | --- | --- | --- |
| 3.1 | BLOCK   | 2 | 5 | 0 | 7 |
| 3.2 | BLOCK   | 1 | 0 | 0 | 1 |
| 3.3 | MINOR   | 0 | 2 | 0 | 2 |
| 3.4 | MINOR   | 0 | 1 | 0 | 1 |
| 3.5 | APPROVE | 0 | 0 | 0 | 0 |

**Status.** SESSION-B (D7, D8, D15) LOCKED. Cluster Round 3 partial:
SESSION-A (D5/D6) and SESSION-C (D9/D10/D13/D16) still pending;
D14 will run after all parallel sessions are merged. After all
parallel sessions land, Round 4 (comprehensive cross-cluster
sweep) follows.

---

## Round 3, SESSION-A -- D5, D6

Iterative Codex adversarial-review rounds for the SESSION-A
addendums (D5 + D6) prior to merge. Each entry records the
round's scope, verdict, issue list, and the corrections applied
before the next round. SESSION-A's local cadence ran independently
from the shared cadence; once it closed at APPROVE, the merge
agent folded the rounds into this shared log under this header.

The structured output contract for every round:
- First line: `VERDICT: <BLOCK | MINOR | APPROVE>`
- Numbered issues list, each tagged `[BLOCK | MINOR | NIT]` with a
  decision reference, file:line citation, 1-3-sentence description,
  and a 1-sentence suggested fix.
- Last line: `TOTAL: <N> issues (<count of BLOCK> BLOCK / <count of
  MINOR> MINOR / <count of NIT> NIT)`.

The Codex prompt included both the shared stage-3 documents
(`spec-snapshot.md`, `discussions.md`, `review-log.md`,
`findings-rustc-draft.md`, `expectations.md`,
`status.md`, `memory.md`, `prior-art/<project>.md`),
locked stage-1 context (`stage-1/spec-snapshot.md`), and the
SESSION-A addendums as the live source for D5 and D6.

### Round 3 (SESSION-A local) -- closed at APPROVE (2026-04-29)

**Trajectory.** 6 sub-rounds. Issue count 13 -> 7 -> 5 -> 1 -> 1
-> 0. Verdict trajectory: BLOCK -> BLOCK -> BLOCK -> MINOR ->
MINOR -> APPROVE. Verdict shifted from BLOCK to MINOR at
sub-round 3.4 once the D7/D8-shape adoption, the
typedef-uniqueness key extension, and the stage-9 option (a)
recast all landed; APPROVE at 3.6.

**Scope.** D5 (Dynamic lowering, sub-decisions D5.1-D5.3) + D6
(closure naming, sub-decisions D6.1-D6.3) -- the SESSION-A
addendums.
- `tmp/SESSION-A/vocabulary-SESSION-A.md`
- `tmp/SESSION-A/discussions-SESSION-A.md`

**Out of scope.** D7-D16 not in this SESSION's slice
(SESSION-B / SESSION-C / D14); cross-cluster consistency (merge
agent's job); markdown-style nits.

#### Sub-round 3.1

**Verdict.** `BLOCK` -- 7 BLOCK + 6 MINOR + 0 NIT (13 total).

**Headline issues.**

1. *[BLOCK] D5.2.* SESSION-A initially assigned dispatch
   rewriting and `Cast::Unsize` vtable-instance registration to
   stage 11, but stage-1 reserves trait-object lowering for
   stages 9/12/13.
2. *[BLOCK] D5.2.* The post-rewrite indirect call shape did not
   align with stage-1's `InstanceId`-pinned `Call` shape.
3. *[BLOCK] D5.3-A.* SESSION-A introduced a per-(Trait,
   projection_bindings) vtable type Item without distinguishing
   it from stage-1's reserved per-(Trait, SelfTy) vtable Static
   Item.
4. *[BLOCK] D5.3-A.* No DefPath naming convention given for the
   new vtable type Item.
5. *[BLOCK] D5.3-A.* The synthesised vtable `Ty::Struct` was
   said to be interned via `translate_adt`, but
   `translate_adt` requires a rustc `Ty` for layout, which the
   synthesised type does not have.
6. *[BLOCK] D5 worked example.* Vtable field used erased
   `*const void` receiver but was initialised with the concrete
   `PL011_read` (which has `&PL011` receiver per
   `stage-1/spec-snapshot.md:597-603`).
7. *[BLOCK] D5.3-B.* Worked-example C output used `void *data`
   for `&dyn Driver` (SharedBorrow), conflicting with the
   `const T *` policy for shared borrows.

Plus 6 MINOR issues (CLR citation, missing serde policy,
ClosureKind inference, citation accuracy, multi-citation
splits, region-erasure primer cross-refs).

**Corrections applied (high-level).** The session's scope was
re-architected: SESSION-A commits stage-3-only content; synthesis
mechanics, post-rewrite Call shape, and pass implementation are
forwarded to `stage-9/expectations.md`,
`stage-11/expectations.md`, `stage-12/expectations.md` so the
receiving stages start from SESSION-A's analysis rather than
scratch. The vtable shim Function Items (per `(Trait,
ConcreteImpl, method)`) were introduced explicitly to bridge the
erased-receiver / concrete-receiver gap. The C worked example
was corrected to use `const void *data` for SharedBorrow and to
show the vtable shim explicitly. The five other MINOR issues
were addressed inline.

#### Sub-round 3.2

**Verdict.** `BLOCK` -- 3 BLOCK + 4 MINOR + 0 NIT (7 total).
Down from 13 in 3.1.

**Headline issues.**

1. *[BLOCK] D5.3-B.* Spec still used pre-D7/D8-lock IR shapes
   `Ref(_, Dynamic, _)` / `RawPtr(Dynamic, _)`; locked D7/D8
   defines `Ty::Ref(TyId, Provenance)` and `Ty::Ptr(TyId,
   Mutability)`.
2. *[BLOCK] D5.3-B.* Three typedef variants given (shared / mut
   / raw `*mut`) -- omits `*const dyn Trait` -- since locked
   D7/D8 keeps raw-pointer mutability as `Ptr(_, Mutability)`
   with `*const T` mapped separately.
3. *[BLOCK] D5.3-B.* Typedef naming convention under-keyed: said
   `trait_obj_<Trait>`, but emission is per-(Trait,
   projection_bindings); two trait objects with the same
   principal but different associated-type bindings would
   collide.

Plus 4 MINOR (Virtual InstanceKind framing; stage-12 Vacant slot
representation undefined; provenance citation wrong;
`#[serde(skip)]` rebuild policy citation wrong).

**Corrections applied.** All references to pre-D7/D8 IR shapes
updated to the locked `Ref(TyId, Provenance)` / `Ty::Ptr(TyId,
Mutability)` shapes. Fat-pointer typedef table extended from
three variants to four. Typedef-name uniqueness sub-section
added to D5.3-B specifying per-(Trait, projection_bindings,
pointer-kind) keying. Stage-9 option (a) recast to use
`fn_shim_disambig` discriminator. Vacant slot policy pinned in
stage-12 forward-expectation as explicit placeholder fields with
NULL initialisers. Citation corrections applied.

#### Sub-round 3.3

**Verdict.** `BLOCK` -- 3 BLOCK + 2 MINOR + 0 NIT (5 total).
Down from 7. Trajectory: 13 -> 7 -> 5.

**Headline issues.**

1. *[BLOCK] D5.3-B worked example.* Trait-object typedef
   emission still said "once per (Trait, projection_bindings)"
   -- cannot represent both `const void *data` and `void *data`
   without losing constness or colliding typedef names.
2. *[BLOCK] D5.2.* "New `Virtual` `InstanceKind` variant" still
   appeared in two places; stage-1 keeps public InstanceKind to
   six variants.
3. *[BLOCK] D5.2.* The recast stage-9 option (a) said codegen
   recognises a Virtual `fn_shim_disambig`, but stage 1
   explicitly says the shim discriminator is **not** exposed as
   a public Instance field.

Plus 2 MINOR (`TraitDefPathHash` -> `DefPathHash`; region-erasure
primer citation off-by-one).

**Corrections applied.** Trait-object typedef emission key
extended to `(Trait, projection_bindings, pointer-kind)`.
Residual "Virtual InstanceKind variant" framings recast to
"public Instance discriminator addition (parallel to rustc's
Virtual InstanceKind, requiring a stage-1 revision)" or
"widened Call.callee shape" wherever they appeared. Stage-9
option (a) substantively rewritten: both sub-paths (a-i adding a
public Virtual variant, a-ii promoting `fn_shim_disambig`)
require stage-1 revisions per the spec-mutability rule. Option
(b) recast as the stage-9-only path. `TraitDefPathHash` ->
`DefPathHash` throughout. Region-erasure primer citation
corrected.

#### Sub-round 3.4

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
**Verdict shifted from BLOCK to MINOR.** Trajectory: 13 -> 7 ->
5 -> 1.

**Issue.** Region-erasure primer cross-ref still cited a stale
`spec-snapshot.md:67` for `Ty::Ref` (should be `:68` post-D7/D8
reordering).

**Corrections applied.** Single citation corrected.

#### Sub-round 3.5

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
Trajectory: 13 -> 7 -> 5 -> 1 -> 1.

**Issue.** D5.3-B fat-pointer table reduced to `const void *data`
vs `void *data` and didn't address how locked D7/D8's
`UnsafeCell` const-drop and `restrict` adjustments apply (or are
intentionally suppressed) for the trait-object `data` field.

**Corrections applied.** Added a "Trait-object override on
D7/D8's UnsafeCell + restrict policy" sub-section to D5.3-B
explaining why neither adjustment applies at the type-erased
trait-object level (codegen cannot inspect the erased pointee
for `UnsafeCell`; cannot establish non-aliasing on the erased
pointee). The four-variant table is a deliberate simplification;
per-(Trait, projection_bindings) codegen overrides or
post-devirtualisation `restrict` upgrades remain available
downstream.

#### Sub-round 3.6

**Verdict.** `APPROVE` -- 0 BLOCK + 0 MINOR + 0 NIT (0 total).

**Outcome.** SESSION-A's local Round 3 closed at APPROVE.

**Trend across all six SESSION-A sub-rounds.**

| Round | Verdict | BLOCK | MINOR | NIT | Total |
| --- | --- | --- | --- | --- | --- |
| 3.1 | BLOCK   | 7 | 6 | 0 | 13 |
| 3.2 | BLOCK   | 3 | 4 | 0 |  7 |
| 3.3 | BLOCK   | 3 | 2 | 0 |  5 |
| 3.4 | MINOR   | 0 | 1 | 0 |  1 |
| 3.5 | MINOR   | 0 | 1 | 0 |  1 |
| 3.6 | APPROVE | 0 | 0 | 0 |  0 |

**Status.** SESSION-A (D5, D6) LOCKED. Cluster Round 3 status:
SESSION-B + SESSION-A merged; SESSION-C (D9/D10/D13/D16) still
pending; D14 will run after all parallel sessions are merged.

---

## Round 3, SESSION-C -- D9, D10, D13, D16

Iterative Codex adversarial-review rounds for the SESSION-C
addendums (D9 + D10 + S0 framing principle + D13 + D16) prior
to merge. SESSION-C ran two sub-clusters (D9 + D10 + S0; then
D13 + D16) and totalled 21 Codex rounds. Once the session closed
at APPROVE, the merge agent folded the rounds into this shared
log under this header.

The structured output contract for every round:
- First line: `VERDICT: <BLOCK | MINOR | APPROVE>`.
- Numbered issues list, each tagged `[BLOCK | MINOR | NIT]` with
  a decision reference, file:line citation, 1-3-sentence
  description, and a 1-sentence suggested fix.
- Last line: `TOTAL: <N> issues (<count of BLOCK> BLOCK / <count
  of MINOR> MINOR / <count of NIT> NIT)`.

The Codex prompt included shared stage-3 documents
(`spec-snapshot.md`, `discussions.md`, `review-log.md`,
`findings-rustc-draft.md`, `expectations.md`,
`status.md`, `memory.md`,
`prior-art/<project>.md`), locked stage-1 context
(`stage-1/spec-snapshot.md`), the methodology context
(`README.md`), the DSL background
(`regseq-background.md`), and the SESSION-C addendums as the
live source for D9 / D10 / D13 / D16. SESSION-C was iterated
through the two-sub-cluster split (substantive locks first;
citation/terminology/cross-site propagation refined over the
remaining rounds).

### Round 3 (SESSION-C local) -- closed at APPROVE (2026-04-29)

**Trajectory.** 21 sub-rounds total across two sub-clusters.
- D9 + D10 + S0 sub-cluster: 16 sub-rounds. Issue trajectory:
  10 -> 5 -> 3 -> 2 -> 2 -> 1 -> 1 -> 1 -> 1 -> 1 -> 1 -> 1 ->
  1 -> 1 -> 1 -> 0. Verdict trajectory: BLOCK -> BLOCK ->
  MINOR -> ... -> MINOR -> APPROVE. Verdict shifted from BLOCK
  to MINOR at sub-round 3.3; APPROVE at 3.16.
- D13 + D16 sub-cluster: 5 sub-rounds. Issue trajectory: 6 ->
  5 -> 2 -> 2 -> 0. Verdict trajectory: BLOCK -> BLOCK ->
  MINOR -> MINOR -> APPROVE. Verdict shifted from BLOCK to
  MINOR at sub-round 4.3; APPROVE at 4.5.

**Scope.** D9 (volatile semantics) + S0 framing principle +
D10 (atomics) + D13 (discriminant level) + D16 (`c_void`
recognition) -- the SESSION-C addendums.
- `tmp/SESSION-C/vocabulary-SESSION-C.md`
- `tmp/SESSION-C/discussions-SESSION-C.md`

**Out of scope.** D5 / D6 / D7 / D8 / D14 / D15 not in this
SESSION's slice (other parallel sessions and the post-merge
D14 slot); cross-cluster consistency (merge agent's job);
markdown-style nits.

**Substantive locks (carried unchanged from each decision's
initial Phase B commit).**

- *D9.* Volatility is encoded on the intrinsic `InstanceId`;
  no `Ty` bit, no Statement flag. Bridge rejects raw
  `core::ptr::{read,write}_volatile`. MMIO via
  `#[rseq::intrinsic("mmio_*")]` exclusively.
- *S0.* Lifted to a named methodology principle ("no QualType
  layer"): each C qualifier maps to a distinct, semantically-
  natural IR site (`const` -> `Provenance` / `Mutability`;
  `static` -> `Item` decl; `restrict` ->
  `Provenance::UniqueBorrow`; `volatile` -> intrinsic
  `InstanceId`).
- *D10.* No `Ty` bit; atomic types are ordinary `Ty::Struct`
  with the inner `is_unsafe_cell` flag (D1.5); atomic
  operations are stage-14 intrinsics on the regseq-DSL
  `Atomic<T>` facade. Bridge rejects
  `core::sync::atomic::Atomic*` types (type-translation) and
  their methods + `core::intrinsics::atomic_*` family
  (Call-translation); `core::sync::atomic::Ordering`
  explicitly *not* rejected; barrier/fence free functions
  (`fence`, `compiler_fence`) are out of D10's scope (deferred
  to future construct decision).
- *D13.* Discriminant *type* settled by D11
  (`TagScalar.scalar_ty`); per-variant discriminant *values*
  added via two new fields on `VariantLayout`:
  `discriminant: u128` (Rust-level semantic discriminant,
  always present per `adt_def.discriminants(tcx)`) and
  `tag_value: Option<u128>` (layout-level tag bytes for
  `SetDiscriminant`; `None` for niche-encoded untagged
  variants and for `EnumShape::Single` variants).
- *D16.* Bridge rejects `core::ffi::c_void` at type
  translation in v1 (recognition via `tcx.is_lang_item(did,
  LangItem::CVoid)` per D1.5); diagnostic recommends
  opaque-struct pattern (per D1.4 precedent). D1.5's "no flag"
  lock unchanged. Reopens when a v1 driver fixture or stage-14
  intrinsic surfaces a stable need for FFI-canonical pointer-
  to-c_void lowering.

**Refinements that emerged from the loop.** The 21 Codex rounds
refined citation precision, terminology, enumeration
completeness, worked-example correctness, and cross-site
propagation. Two rounds caught real correctness or coverage
issues that improved the substantive claims without overturning
the locks:
- *R3.10 const-drop / `restrict`.* The D5.3-B trait-object
  override on D7/D8's UnsafeCell + `restrict` policy added a
  sub-section explaining why neither adjustment applies to the
  type-erased trait-object `data` field. (Note: this was
  surfaced during SESSION-C's iteration; D5.3-B itself is
  SESSION-A territory. SESSION-C's fix landed in shared
  vocabulary at the time the cross-cluster issue surfaced.)
- *R3.13 fence/compiler_fence scope.* The D10 entry was
  refined to explicitly call out
  `core::sync::atomic::{fence, compiler_fence}` as out of
  D10's scope (deferred to a future construct decision per
  `docs/compiler-collateral/constructs/atomic-memory-access-and-ordering.md`),
  rather than implicitly bundling them with the operational
  API rejection.
- *R4.1 discriminant-vs-tag split.* The original D13 framing
  conflated the discriminant *type* and the per-variant
  discriminant *values* questions. R4.1 surfaced the
  conflation; the locked answer cleanly separates the two
  senses (discriminant *type* settled by D11; per-variant
  *values* split into `discriminant: u128` /
  `tag_value: Option<u128>`).
- *R4.1 c_void-not-zero-variant.* The original D16 framing
  described `c_void` as a "zero-variant enum" inheriting from
  D1.6.c. The pinned-toolchain `core::ffi::c_void`
  (`library/core/src/ffi/mod.rs:49-76`) actually carries two
  hidden `__variant1` / `__variant2` variants, making the
  type *inhabited*. The substantive claim that `c_void` would
  translate to a structurally-faithful but FFI-wrong 1-byte
  tagged struct rests on the corrected 2-variant shape, not
  on the older zero-variant claim.

**Trend across all 21 SESSION-C sub-rounds.**

| Round | Verdict | BLOCK | MINOR | NIT | Total |
| --- | --- | --- | --- | --- | --- |
| 3.1  | BLOCK   | 4 | 5 | 1 | 10 |
| 3.2  | BLOCK   | 1 | 4 | 0 |  5 |
| 3.3  | MINOR   | 0 | 3 | 0 |  3 |
| 3.4  | MINOR   | 0 | 2 | 0 |  2 |
| 3.5  | MINOR   | 0 | 2 | 0 |  2 |
| 3.6  | MINOR   | 0 | 1 | 0 |  1 |
| 3.7  | MINOR   | 0 | 1 | 0 |  1 |
| 3.8  | MINOR   | 0 | 1 | 0 |  1 |
| 3.9  | MINOR   | 0 | 1 | 0 |  1 |
| 3.10 | MINOR   | 0 | 1 | 0 |  1 |
| 3.11 | MINOR   | 0 | 1 | 0 |  1 |
| 3.12 | MINOR   | 0 | 1 | 0 |  1 |
| 3.13 | MINOR   | 0 | 1 | 0 |  1 |
| 3.14 | MINOR   | 0 | 1 | 0 |  1 |
| 3.15 | MINOR   | 0 | 1 | 0 |  1 |
| 3.16 | APPROVE | 0 | 0 | 0 |  0 |
| 4.1  | BLOCK   | 4 | 2 | 0 |  6 |
| 4.2  | BLOCK   | 2 | 3 | 0 |  5 |
| 4.3  | MINOR   | 0 | 2 | 0 |  2 |
| 4.4  | MINOR   | 0 | 2 | 0 |  2 |
| 4.5  | APPROVE | 0 | 0 | 0 |  0 |

**Status.** SESSION-C (D9, D10, D13, D16) plus the S0 framing
principle LOCKED. Cluster Round 3 status: SESSION-A + SESSION-B
+ SESSION-C all merged. Remaining: D14 (renderer style,
sequenced after parallel sessions land), then Round 4
(comprehensive cross-cluster sweep).

---

## Round 5 -- post-external-review fix loop on top-level `spec.md`

**Date.** 2026-05-01.

**Trigger.** External review document at
`/Users/nyengele/Downloads/regseq-type-system-review.md`
identified 26 findings against the merged top-level `spec.md`.
Two internal Codex adjudication passes ran before opening the
fix loop:

- *Pass 1 (independent adjudication).* Spec + stage-3 audit
  trail + stage-3 review-log + external review fed in; no
  agent-side analysis included to avoid anchoring. Verdict:
  21 VALID, 4 FRAMING-ONLY, 1 WRONG, 3 ALREADY-CLOSED-IN-R2.9.
  Plus 4 MISSED-BY-REVIEWER findings.
- *Pass 2 (focused HRTB factual check).* Verified that the
  pre-existing §17.1 predicate ("`Ty::FnPtr` over a
  `Binder<FnSig>` containing `ReBound`") rejects ordinary
  fn-pointer types like `fn(&u32)`. Grounded in 7 rustc source
  citations: `rustc_ast_lowering::lower_lifetime_binder`
  (`:903-917`), `LifetimeRes::Fresh` from
  `rustc_resolve::late` (`:1820-1844`),
  `Region::new_bound` from
  `rustc_hir_analysis::hir_ty_lowering` (`:2864-2879`), and
  `erase_and_anonymize_regions` preservation of `ReBound`
  (`rustc_middle::ty::erase_regions:19-33`). Predicate over-
  rejection confirmed: driver callback signatures dead on
  arrival under prior wording.

Pass 2 supersedes Pass 1's "FRAMING-ONLY" verdict on the HRTB
finding. Pass 1's read accepted the spec text at face value
("HRTB-bearing rejected"); Pass 2 traced what the predicate
evaluates to at the bridge boundary.

**Scope.** `spec.md` only. `stage-3/spec-snapshot.md` is the
frozen close-of-stage artefact (per §5.5 step 7) and is not
modified. Stage-3 D-tag namespace is not extended (no
`D-3-17+` tags); the Fix→Review loop is a maintenance
iteration on the live snapshot, not a stage reopen.

**Excluded from this loop (with rationale):**

- `TyIdentityKey` extraction. Already-closed at R2.9; the
  team explicitly chose custom `Hash + Eq` on `Ty` with the
  nominal/structural identity split. Issues 2 (`FnSigId`
  content-uniqueness) and 3 (`ConstId` content-uniqueness)
  are addressed instead by interning the *leaf* IDs through
  content-keyed interners (`FnSigInterner`, `ConstInterner`),
  preserving the in-build BiMap shape.
- "Closed `Ty` enum is v1 contract" softening. Pass 1 marked
  WRONG; the closed 19-variant rule is binding under Q3
  single-level IR architecture, not editorial wording.
  Softening would invite "v2 will extend it" assumptions
  that violate Q3.
- Lifting full fat-pointer C-typedef shapes from Appendix A
  to a normative `spec.md` subsection. The spec keeps the
  *qualification rules* that constrain those shapes (`const`
  derivation via `Provenance` + `contains_unsafe_cell`,
  raw-pointer derivation via `Mutability`); the literal C
  typedef shapes are codegen-contract material owned by the
  codegen stage.
- `Provenance` -> `BorrowKind` rename. Pass 1 marked
  FRAMING-ONLY. High-churn for no soundness consequence; a
  one-paragraph clarifying note in §10.1 covers the
  "narrower than rustc's pointer provenance" confusion
  vector.

### Round 5.1 fix manifest

Edits landed in `spec.md`. Each entry: source finding ->
spec section(s) touched -> one-line summary.

**Tier 1 (blocking before further build-out):**

1. *HRTB predicate sharpness* (Pass 2). §17.1 + §15
   translation table + §15.1 rejection table. Predicate
   sharpened from "any `Binder<FnSig>` containing `ReBound`"
   to "substantive HRTB" -- bound region in projection,
   nested binder, or trait-predicate position other than
   immediate `&T` / `&mut T` ref slot. Accepts `fn(&T)`,
   `fn(&A, &B)`, `fn(&mut T) -> &U`. Rejects
   `for<'a> fn(<T as Trait<'a>>::Assoc)` and
   `dyn for<'a> Trait<&'a T>`.
2. *`FnSigId` and `ConstId` content-uniqueness* (Pass 1
   #2, #3). New §14.4 `FnSigInterner` (mirror `TyInterner`
   shape; identity over the §14.3 canonical schema; same
   custom-serde pattern). New §9.7 `ConstInterner` (same
   pattern; full `Const` schema deferred to stage 4 with
   stage-3 commitment to content-uniqueness via canonical
   const-eval result). §3 `RegseqModel` field types changed
   from `IndexVec` to interner. §3.1 invariant 11 added
   (content-keyed interner uniqueness).
3. *`TargetInfo`* (Pass 1 #4). §3 -- new `TargetInfo` and
   `Endian` types; new `target: TargetInfo` field on
   `RegseqModel`. §3.1 invariant 15 added (TargetInfo
   well-formedness). Determinism (§16) implicitly
   parameterised over `target` equality.
4. *`FieldLayout` / `VariantLayout` ordering* (Pass 1 #6).
   New §11.0 -- field index = source/MIR field index;
   `FieldLayout.offset` = byte offset from containing
   aggregate (whole-enum-object for variant payloads);
   `EnumShape::Multiple.variants` indexed by `VariantIdx`;
   ZST handling.

**Tier 2 (high-value, paired with Tier 1):**

5. *C ABI admissibility* (Pass 1 #5). New §14.5 --
   `is_c_abi_admissible` predicate; explicit accept/reject
   sets; bridge enforcement at `FnAbi::C` `FnSig`
   translation; `BridgeError::UnsupportedCAbiType`. New
   §15.1 rejection row.
6. *Recursive `contains_unsafe_cell`* (Pass 1 #9). New
   §17.2.1 -- predicate signature, recursion table per `Ty`
   variant, termination via `TyInterner` acyclicity, and
   the C-qualification rules using the predicate.

**Codex Pass 1 missed-by-reviewer findings (all 4 landed):**

7. *Signed-discriminant interpretation* (Pass 1 missed #1).
   §11.1 -- `discriminant` / `tag_value` `u128` carry the
   bit pattern (truncated/zero-extended); codegen reads
   `TagScalar.scalar_ty` to decide sign-extension on emit.
8. *Closure `find_nominal` API* (Pass 1 missed #2). §12
   `TyInterner` method added; new `NominalKind` enum;
   §13.1 wording fixed to use
   `find_nominal(NominalKind::Struct, item, args)` instead
   of the non-executable
   `find(&Ty::Struct { item, args, layout: _ })`.
9. *Unsized marker placement invariant* (Pass 1 missed #3).
   §3.1 invariant 12 added; §15 translation table notes;
   §15.1 rejection row.
10. *Vtable rebuild marker* (Pass 1 missed #4). §13.2 --
    vtable type Item DefPath rendered as
    `<vtable_ty for Trait[<projection_bindings>]>` (reserved
    canonical form distinct from `<vtable for Trait for SelfTy>`
    used by vtable instances). Vtable dedup table rebuild
    scans for the `<vtable_ty for ` prefix marker. §9.6
    updated to mention the new canonical form.

**Tier 3 cheap-and-worth (bundled in this loop):**

11. *`DynKind::DynStar` rejection* (Pass 1 #7 partial).
    §15 translation table -- `Dynamic(_, _, DynKind::Dyn)`
    accepts; `Dynamic(_, _, DynKind::DynStar)` rejects.
    §15.1 rejection row.
12. *Special-ADT dispatch order* (Pass 1 #10). §15 --
    explicit ordering note: lang-item flags first
    (`PhantomData` / `UnsafeCell` / `ManuallyDrop`),
    DSL-rejected next (`Box` / `c_void` / `Atomic*` / etc.),
    generic ADT translation only if no special rule applies.
13. *`Ty::Dynamic` well-formedness invariant* (Pass 1 #14
    framing-only -> elevated to invariant). §3.1 invariant
    13 added.
14. *`InstanceKind` layout consistency invariant* (Pass 1
    #15). §3.1 invariant 14 added.
15. *Wider deserialise validation* (Pass 1 #19). §12.5 --
    validation extended to `ItemId`, `FnSigId`, `ConstId`,
    `StringId`, `SourceFileId`, `VariantIdx` reachability;
    layout sanity checks (offset within size, FieldLayout.ty
    resolves, FieldLayout.name resolves, nominal layout
    matches Ty variant kind).
16. *I128 / U128 / F16 / F128 backend rejection* (Pass 1 #12).
    New §10.3 -- bridge admits unconditionally; codegen
    rejects per-target via `BackendError::UnsupportedScalar`.
17. *`Instance.symbol_name` -> `emission_name`* (Pass 1 #16).
    §9.2 renamed; semantics split by `InstanceKind`: rustc's
    `tcx.symbol_name(MonoItem::*)` for callable/static
    instances, bridge-constructed `(DefPath, ArgsKey)`-derived
    name for type-definition instances (Struct/Enum/Union),
    construction rule pinned in stage 13. §9.3 reference
    updated.
18. *`Provenance` clarifying note* (Pass 1 #8 FRAMING-ONLY).
    §10.1 -- one-paragraph note that `Provenance` here means
    borrow flavor, not rustc-style pointer provenance / alias
    history. No rename.

### Round 5.1 review

**Codex run.** `--effort high`, `--timeout 1800`, output captured
to `/tmp/codex_round_5_1.txt`.

**Verdict.** `BLOCK` — 2 BLOCK + 1 MINOR + 1 NIT (4 issues total).

All 18 manifest items landed. 15 landed cleanly; 3 landed with
identified gaps (HRTB still collapses, C ABI predicate not
executable from the model, target qualification only local to §3).

**Findings.**

- *BLOCK 1: HRTB exception still collapses rustc-distinct
  fn-pointer types.* The §17.1 predicate accepts any binder
  whose bound regions appear only in immediate ref positions.
  This admits both `for<'a> fn(&'a A, &'a B)` (relational —
  single bound region used twice) and
  `for<'a, 'b> fn(&'a A, &'b B)` (independent — two distinct
  bound regions used once each); both lower to
  `(inputs: [Ref(A), Ref(B)])` post-erasure and collapse into
  one `FnSigId`. This is the exact identity-collapse the fix
  was meant to prevent (D15 type-identity argument).
- *BLOCK 2: C ABI admissibility predicate is not executable
  from the model + admits arrays.* §14.5 reads `repr(C)` /
  `repr(transparent)` flags that §11.3 explicitly drops. Also
  admits `Array(elem, _)` by-value, but C arrays are not
  by-value parameters or returns (decay to pointer in
  parameter position; cannot return).
- *MINOR 3: Target-qualified determinism only local to §3.*
  §12.4 cross-build-property table and §16 determinism contract
  still claim "identical / stable for same source under pinned
  toolchain" without restating target equality.
- *NIT 4: Stale `see §11.x` comment in `FieldLayout.offset`*
  after the new subsection landed as §11.0.

### Round 5.2 fix manifest

**B1 fix (HRTB predicate sharper still).** §17.1 — added
**condition 4** to the rejection predicate: a binder is
rejected when **a bound region appears in more than one
position in the binder body** (relational binders). Predicate
is now: reject if (1) projection, (2) nested binder, (3)
non-ref-slot trait predicate, OR (4) any bound region used
more than once. Accepts: `fn(&T)`, `fn(&A, &B)`,
`extern "C" fn(*const T)`, `fn(u32, &T) -> u32`. Rejects:
`for<'a> fn(&'a A) -> &'a A` (input-output relational by
elision), `for<'a> fn(&'a A, &'a B)` (explicit relational).
§15.1 rejection rows reworded to cite the predicate by
reference. Driver-impact note added: relational shapes are
expressible as Function Items (`Ty::FnDef`) just not as
`Ty::FnPtr` value types; future relaxation path documented
(adding a `binder_shape` carrier on `FnSig` if a real driver
need surfaces).

**B2 fix (C ABI predicate is bridge-side, no Array by value).**
§14.5 — explicit "Where the check runs" subsection: the
predicate is **bridge-side at FnSig translation**, consulting
rustc directly for repr classification (`adt_def.repr()`).
The IR carries the *outcome* (presence of an `FnAbi::C`
`FnSig` in `model.sigs` is the bridge's certificate that the
predicate held), not the input flags. New construction
invariant 16 in §3.1 pins the outcome rule. Removed
`Array(elem, _)` from the admissible list; explicit row in
the rejected list noting C arrays are not by-value params /
returns and that driver code passes them as `Ptr(Array(...))`
or wraps in `repr(C)` struct.

**M3 fix (target-qualified determinism contract-wide).** §12.4
— preface added introducing the **build-equivalence quad**
(equal source, equal pinned toolchain, equal
`RegseqModel.target`, equal bridge options); table entries
updated to reference it; "Each `Layout`" entry calls out
target dependence explicitly. §16 opening line rewritten to
require the build-equivalence quad and note `DefPathHash` as
the cross-quad identity carrier.

**N4 fix.** `FieldLayout.offset` comment updated from
`see §11.x` to `see §11.0`.

### Round 5.2 review

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_round_5_2.txt`.

**Verdict.** `MINOR` — 0 BLOCK + 1 MINOR + 0 NIT (1 issue total).

All four Round 5.1 findings landed. Three (B1 HRTB, B2 C ABI,
N4 stale ref) RESOLVED; one (M3 target qualification)
PARTIALLY-RESOLVED — the §12.4 quad and §16 contract were
qualified, but §3.1 invariant 3 retained un-qualified "two
builds of the same source" language. No new regressions.

### Round 5.3 fix manifest

**M3 residual fix.** §3.1 invariant 3 — phrasing updated to
reference the §12.4 build-equivalence quad explicitly: "Two
builds satisfying the build-equivalence quad (§12.4) produce
identical `ItemId` assignments. Builds outside the quad are
aligned by `DefPathHash` (§7.3), not by sequential `ItemId`."
This brings the invariant into line with the §16 contract and
the §12.4 cross-build property table.

### Round 5.3 review

**Codex run.** `--effort high`, `--timeout 1200`, output captured
to `/tmp/codex_round_5_3.txt`.

**Verdict.** `APPROVE` — 0 BLOCK + 0 MINOR + 0 NIT.

The M3 residual was RESOLVED: §3.1 invariant 3, §12.4 quad
definition, and §16 contract now agree on the build-equivalence
quad as the determinism precondition. Final-sweep across the
cumulative Round 5 edits returned no remaining findings.

### Round 5 closing summary

**Trajectory.**

| Round | Verdict | BLOCK | MINOR | NIT | Total |
| ---   | ---     | ---   | ---   | --- | ---   |
| 5.1   | BLOCK   | 2     | 1     | 1   | 4     |
| 5.2   | MINOR   | 0     | 1     | 0   | 1     |
| 5.3   | APPROVE | 0     | 0     | 0   | 0     |

**Substantive design refinements that emerged from this loop.**

- *Sharper HRTB predicate.* The original §17.1 phrasing ("any
  `Binder<FnSig>` containing `ReBound`") would have rejected
  every fn-pointer type with a reference parameter (rustc
  represents `fn(&u32)` as `for<'r> fn(&'r u32)` with anonymised
  `ReBound` after `erase_and_anonymize_regions`). Pass 2's HRTB
  factual check (7 rustc source citations) confirmed the
  over-rejection. Round 5.1 introduced a "substantive HRTB"
  predicate; Round 5.2 added the relational-binder rejection
  (condition 4) after Codex caught that the looser predicate
  collapsed `for<'a> fn(&'a A, &'a B)` and
  `for<'a, 'b> fn(&'a A, &'b B)` into one `FnSigId` post-erasure.
  Final predicate accepts `fn(&T)`, `fn(&A, &B)` (independent
  elision), and `extern "C" fn(*const T)`; rejects relational
  binders, projection HRTB, nested HRTB, and trait-predicate HRTB
  outside immediate ref slots.
- *C ABI admissibility is bridge-side, not model-walked.* Round
  5.1 introduced an `is_c_abi_admissible` predicate that read
  `repr(C)` / `repr(transparent)` flags. Round 5.2 caught that
  §11.3 (D11 LOCKED) explicitly drops these flags from `Layout`,
  making the predicate non-executable from the model alone. The
  resolution: the bridge runs the predicate at `FnSig` translation
  using rustc-side queries (`adt_def.repr().c()`,
  `adt_def.repr().transparent()`); the IR carries the *outcome*
  via construction invariant 16. This matches "store the result,
  not the input" methodology. `Array(_, _)` was removed from the
  admissible list (C arrays decay to pointers in parameter
  position; cannot be returned by value).
- *Build-equivalence quad as the determinism precondition.* The
  original §16 / §12.4 wording said "two builds of the same
  source produce identical assignments" — silently parameterised
  over target. Round 5.1 introduced `TargetInfo`; Round 5.2
  generalised to the **build-equivalence quad** (source ×
  pinned toolchain × target × bridge options) and propagated
  the qualification through §12.4 cross-build property table,
  §16 contract, and (after Round 5.3 residual fix) §3.1
  invariant 3. `DefPathHash` (§7.3) is the cross-quad identity
  carrier when builds disagree on any quad component.
- *Vtable rebuild marker.* The vtable type Item DefPath now
  uses a reserved canonical form `<vtable_ty for Trait[<projection_bindings>]>`
  distinct from the vtable instance Item form
  `<vtable for Trait for SelfTy>`. The serde-skipped
  `RegseqModel.vtables` dedup table is rebuilt on deserialise
  by scanning items for the `<vtable_ty for ` prefix.
- *`emission_name` rename.* `Instance.symbol_name` was overloaded:
  rustc's mangled name for callable/static instances vs.
  undefined for type-definition instances (Struct/Enum/Union).
  Renamed to `emission_name` with kind-aware construction:
  `tcx.symbol_name(MonoItem::*)` for callables/statics;
  bridge-constructed `(DefPath, ArgsKey)`-derived C-identifier
  for type instances (rule pinned in stage 13 codegen).
- *Recursive `contains_unsafe_cell` predicate.* The Atomic
  worked example (Appendix A.9) and the "drop `const` on shared
  references to `UnsafeCell`-bearing wrappers" rule depend on
  recursion through `Struct`/`Enum`/`Union`/`Tuple`/`Array`/
  `Closure`-captures, stopping at `Ref`/`Ptr` boundaries. §17.2.1
  pins the predicate signature, recursion table, and termination
  via `TyInterner` acyclicity.

**Status.** Round 5 LOCKED. Top-level `spec.md` reflects the
post-external-review fix loop in full. Stage-3 `discussions.md`
is unchanged; stage-3 D-tag namespace is not extended. The
external review at `/Users/nyengele/Downloads/regseq-type-system-review.md`
is fully adjudicated: 21 VALID findings landed (with 4 substantive
edits during the loop iteration), 4 missed-by-reviewer findings
landed, 4 FRAMING-ONLY findings noted with the rationale recorded
in §5.1 manifest exclusions, 1 WRONG finding (the "v1 contract"
Ty softening) explicitly declined per Q3, 3 ALREADY-CLOSED
findings (R2.9 closed-decision relitigation) declined.

---

## Round 6 -- second-external-review fix loop

**Date.** 2026-05-01.

**Trigger.** Second external review document at
`/Users/nyengele/Downloads/regseq-updated-spec-review.md`,
delivered after Round 5 closed at APPROVE. The reviewer
acknowledges the Round 5 improvements and identifies a new set
of P0/P1/P2 findings concentrated in: identity mechanics, C
ABI fidelity (substitution rules for transparent wrappers and
fieldless enums), wide-pointer/trait-object lowering,
synthetic-metadata recovery, and validation order.

**Adjudication.** No separate Codex pass was launched for this
review (the findings are sufficiently grounded that an
independent verdict was not load-bearing). Issue-by-issue
analysis recorded in the consolidated take preceding the
manifest below.

**Scope.** `spec.md` only. `stage-3/spec-snapshot.md` remains
the frozen close-of-stage artefact. Stage-3 D-tag namespace
remains unextended.

**Excluded from this loop (with rationale):**

- *P0.1 `TyIdentityKey` extraction* — the same finding as
  Round 5 Issue 1, and ALREADY-CLOSED-IN-R2.9. Two independent
  reviewers raising the same surface concern is signal that the
  spec wording invites it; Round 6 addresses by adding a
  visibility note in §12 explaining why the custom `Hash + Eq`
  design was chosen and pointing readers at R2.9. No structural
  refactor.
- *P2.1 Provenance -> BorrowKind rename* — same as previous,
  declined as FRAMING-ONLY. Keep the §10.1 clarifying note.
- *P2.5 PhantomData ZST exemption in transparent recursion* —
  rendered moot by P0.2.a: transparent wrappers are unwrapped
  bridge-side at C boundaries (substitution to inner non-ZST
  field type, ZST markers skipped during inspection), so the
  recursion-with-PhantomData edge case does not arise.
- *P2.6 Closure synthetic Function Item DefPaths* — covered by
  T1.5 (SyntheticItemKind side table); the structured tag is
  the load-bearing recovery mechanism, DefPath rendered forms
  remain as display convention.
- *P2.8 Pretty-printer stubs* — stage-13 implementation work,
  not a spec gap. The §18.2 status note already documents the
  stub state.

### Round 6.1 fix manifest

Edits landed in `spec.md`. Each entry: source finding -> spec
section(s) touched -> one-line summary.

**Tier 1 (substantive, blocking):**

1. *T1.1 / P0.2.a Transparent ABI unwrapping at C boundaries.*
   §14.5 -- `repr(transparent)` aggregates are bridge-substituted
   to the inner non-ZST field's type at `FnAbi::C` parameter /
   return positions (recursive to fixed point; ZST marker
   fields like `PhantomData<U>` skipped). Wrapper still exists
   for value-context use. Construction invariant 16 updated.
2. *T1.2 / P0.2.b Fieldless `repr(int)` enum tag-scalar
   substitution.* §14.5 -- fieldless `#[repr(uN)]` /
   `#[repr(iN)]` enums (every variant unit) are bridge-
   substituted to their tag scalar at the C boundary. Enums
   with payload variants are *not* fieldless and remain in the
   admissible-with-recursive-field-check rule (typically
   rejected for absence of canonical C ABI for tagged unions).
3. *T1.3 / P0.2.c Nested FnPtr ABI recursion.* §14.5 +
   §15.1 -- `FnPtr(sig)` admissible at C boundary iff
   `model.sigs[sig].abi == FnAbi::C` AND nested sig's
   inputs/output are themselves admissible. New rejection row
   for `FnPtr(sig)` with `sig.abi == FnAbi::Rust` at C boundary.
4. *T1.4 / P0.4 Trait-object erased receiver always `*mut ()`.*
   §13.2 -- vtable receiver type uniform `*mut ()` regardless
   of source borrow flavor; rationale (UnsafeCell soundness on
   shared `&self` impls) documented inline. New §17.2.1
   "Trait-object dispatch carve-out" subsection. Appendix A.6
   updated (`const void *data` -> `void *data` in
   `trait_obj_Driver`; vtable receiver fields updated).
5. *T1.5 / P0.6 Structured `SyntheticItemKind` side table.*
   §3 -- new `item_synthetic_kinds: IndexVec<ItemId,
   SyntheticItemKind>` field on `RegseqModel`. New §13.3
   subsection pinning the closed enum (None,
   ClosureCaptures, ClosureCall { call_kind }, ClosureDropGlue,
   VTableType, VTableInstance, VTableMethodShim, VTableDropShim).
   §13.2 vtable rebuild now walks `item_synthetic_kinds` for
   `VTableType` entries instead of parsing `def_path.rendered`.
   §3.1 invariant 2 extended to require `item_synthetic_kinds`
   parallel to `items` and `None` for source-level items.
6. *T1.6 / P0.3 Fat-pointer carve-out + wide-pointer
   qualification.* §17.2.1 -- "Wide-pointee carve-out"
   subsection added. Sized-pointee rule now explicit; unsized-
   pointee rule (Str/Slice/Dynamic) routes through fat-pointer
   aggregate with per-row qualification of the `data` field.
   Cross-reference to Appendix A.2 for literal C struct shapes
   (codegen contract; not lifted into normative spec body).

**Tier 2 (high-value):**

7. *T2.1 / P0.5 Slimmed `BuildInfo`.* §3 -- new `BuildInfo
   { toolchain: StringId, bridge_options_hash: u128 }`
   struct + `build: BuildInfo` field on `RegseqModel`. §3.1
   new invariant 17 (well-formedness). The reviewer's full
   `BuildInfo` (rustc version, channel, bridge crate version,
   mir_opt_level, cleanup_pass_whitelist_hash) was slimmed to
   the two essentials for build-equivalence-quad verification.
8. *T2.2 / P1.1 Validation order + c_variadic invariants.*
   New §3.3 "Model validation order" subsection -- normative
   ordering with explicit dependency arrows (consts <- sigs <-
   types <- layout/instances). §14.3 `c_variadic` invariants:
   `true` implies FnAbi::C, `inputs.len() >= 1`, output may be
   Never but inputs may not.
9. *T2.3 / P1.2 Const blocking-dependency note.* §9.7 -- new
   "Stage-3 minimum vs stage-4 requirement" subsection
   enumerating the constructs that reference `ConstId` for
   richer values (statics, vtable instances, string literals,
   aggregates, function-pointer / vtable-pointer / static-
   pointer relocations). Pins the minimum surface stage 4 must
   cover for canonicalisation.
10. *T2.4 / P1.3 `size_align_of` contract.* §11 -- new §11.5
    subsection. `SizeAlign { size, align }` struct + method on
    `RegseqModel`. Per-`Ty`-variant rules: ADT/Tuple read
    inline Layout; Array stride*len; thin pointer 1-slot;
    fat pointer 2-slot via target.pointer_width; FnPtr 1-slot;
    FnDef ZST; primitives target-derived; Str/Slice/Dynamic
    panic if queried directly.
11. *T2.5 / P1.5 HRTB position-traversal definition + 8-case
    test list + softer wording.* §17.1 -- new "Position
    traversal definition" subsection (precise rules for `&&'a T`
    nesting, generic-arg references, output-slot use).
    8-case test table with accept/reject expectations.
    "no two rustc-distinct binder shapes collapse" softened to
    "intended to avoid known binder-shape collapses; stage-13
    will add a formal test suite."
12. *T2.6 / P1.6 Reject Char at C boundary.* §14.5 + §15.1 --
    Char rejected at FnAbi::C parameter/return; rationale
    (Unicode-scalar-value validity invariant not preserved by
    C uint32_t).
13. *T2.7 / P1.8 Reject auto-only trait objects.* §15.1 --
    new rejection row for `Ty::Dynamic` with no principal trait.
    §3.1 invariant 13 updated to require principal trait.
14. *T2.8 / P0.2.d Narrow §10 Array C-side intent.* Row reads
    "C array `T[N]` in field/local context only; rejected at
    FnAbi::C parameter/return per §14.5."

**Tier 3 (cheap-and-worth):**

15. *T3.1 / P0.1 §12 visibility note for custom Hash+Eq design.*
    Short paragraph at the top of §12 explaining the R2.9-locked
    design choice and pointing future maintainers at
    discussions.md §2.5 before considering refactor.
16. *T3.2 / P1.4 Injectivity verification note.* §9.3 -- bridge
    MUST verify `(rank, item, args, fn_shim_disambig)` is
    injective for the admitted InstanceKind set; debug-build
    assertion at instance interning; toolchain-bump non-injective
    case extends rank table with payload-key encoding.
17. *T3.3 / P1.7 Opaque-handle C convention.* §14.5 -- inline
    note that `repr(C)` ZST struct used only behind Ref/Ptr
    may be lowered as C incomplete struct declaration (`struct
    H;`); rejected for by-value or sized-field use.
18. *T3.4 / P2.2 Revised closed-Ty wording.* §17.4 -- explicit
    separation: domain-specific extensions forbidden by Q3
    (single-level IR rule); source-language expansion is the
    revision path. Codex Pass 1 had marked the previous review's
    looser wording WRONG; this revised wording preserves Q3.
19. *T3.5 / P2.3 Crate ordering.* §3.1 invariant 5 extended:
    `crates` sorted by `(crate_name, stable_crate_id)` before
    `CrateId` assignment; out-of-quad alignment via
    `StableCrateId`.
20. *T3.6 / P2.4 Pointer/ref TagScalar target-width
    interpretation.* §11.2 -- `valid_range`, `niche_start`,
    `tag_value` for pointer/ref tags interpreted as truncated
    to `target.pointer_width`; null-pointer constants emit as
    integer-zero cast.
21. *T3.7 / P2.7 FnDef explicit C-boundary rejection.* §14.5 --
    removed from admissible list, added to rejected list with
    "bridge bug if encountered at FnAbi::C boundary" note.
    §15.1 row added.

### Round 6.1 review

**Codex run.** `--effort high`, `--timeout 1800`, output captured
to `/tmp/codex_round_6_1.txt`.

**Verdict.** `BLOCK` — 1 BLOCK + 1 MINOR + 2 NIT (4 issues total).

All 21 manifest items landed: 17 cleanly, 4 with identified gaps
(items 1, 2, 6, 10 LANDED-WITH-ISSUE per the new-issues list
below). Codex did not disagree with any manifest exclusion.

**Findings.**

- *BLOCK 1: C-boundary substitution collapses `FnSig` type
  identity.* The fixes for items 1 (transparent unwrap) and 2
  (fieldless enum substitution) stored the substituted TyId
  *directly* in `FnSig.inputs/output`. This collapses
  rustc-distinct types: `extern "C" fn(Handle) -> Handle` (with
  `Handle(u32)` `repr(transparent)`) and
  `extern "C" fn(u32) -> u32` would share one `FnSigId`,
  violating D15's type-identity argument. Same problem for
  fieldless `repr(int)` enums: `extern "C" fn(Status) -> Status`
  would collapse with `extern "C" fn(u32) -> u32`. The fix needs
  to preserve original Rust TyIds in FnSig and carry the
  C-side lowering shape in a separate side table.
- *MINOR 2: Wide-pointer table omits raw `*const str` /
  `*mut str`.* Item 6's qualification table covers
  `Ref(Str, _)` and various `Slice`/`Dynamic` rows but missed
  `Ptr(Str, Mutability::{Not, Mut})`. Construction invariant
  12 admits these placements, so the carve-out must specify
  their qualification.
- *NIT 3: HRTB traversal assigns raw pointers a lifetime
  slot.* Item 11's traversal text says "TyKind::Ref /
  TyKind::RawPtr nodes whose lifetime slot binds a region" —
  but raw pointers have no lifetime slot in rustc.
- *NIT 4: Array stride formula reversed.* Item 10 wrote
  `round_up_to(align(elem), size(elem))`; conventional shape
  is "round size up to alignment."

### Round 6.2 fix manifest

**B1 fix (preserve FnSig type identity; record C ABI lowering
in a side table).**

§3 — new `sig_c_abi_lowerings: IndexVec<FnSigId,
Option<CAbiLowering>>` field on `RegseqModel`, parallel to
`sigs`. Comment block on the field cross-references §14.5.

§3.1 — invariant 16 rewritten: `FnSig.inputs/output` preserve
**original Rust TyIds**; the C-side per-position lowering shape
is recorded in `sig_c_abi_lowerings[id]: Some(CAbiLowering)`
when `sigs[id].abi == FnAbi::C`, and `None` otherwise. Length
parity with `sigs`.

§3.3 — validation order step 5 extended: validate
`sig_c_abi_lowerings.len() == sigs.len()` and per-entry
presence rule.

§14.5 — preamble rewritten to introduce `CAbiShape` and
`CAbiLowering` types. Three lowering shapes:
- `Direct` — type's natural C representation.
- `TransparentInner { inner_ty: TyId }` — repr(transparent)
  unwrap (recursively to fixed point; ZST marker fields
  skipped).
- `TagScalar { tag_scalar_ty: TyId }` — fieldless repr(int)
  enum lowered as its tag scalar.
Worked example block showing
`extern "C" fn(Handle) -> Handle` and
`extern "C" fn(u32) -> u32` getting distinct `FnSigId`s with
matching C declarations via the side table. Per-position
admissibility predicate rewritten to assign a `CAbiShape` per
position (Direct for scalars/pointers/repr(C) ADTs;
TransparentInner for repr(transparent); TagScalar for
fieldless repr(int)). Closing paragraph rewritten: codegen
reads `sig_c_abi_lowerings` to know what to emit; D15-style
type identity preserved via original TyIds in FnSig. Note that
`CAbiLowering` is sibling-only metadata and does NOT affect
FnSigInterner identity.

**M2 fix (raw `Ptr(Str, _)` rows in wide-pointer table).**

§17.2.1 wide-pointer table — added two new rows:
- `Ptr(Str, Mutability::Not)` -> `data` is `const uint8_t *`,
  `len` target-width unsigned.
- `Ptr(Str, Mutability::Mut)` -> `data` is `uint8_t *`, same
  `len`.

**N3 fix (HRTB traversal raw-pointer wording).**

§17.1 position-traversal definition — opening clause rewritten
to reference only `TyKind::Ref` (not `TyKind::RawPtr`). Added
explanatory sentence: raw pointers have no lifetime slot in
rustc; bound regions inside a raw pointer's pointee fall under
condition 3 (non-ref-slot structural position) and cause
rejection.

**N4 fix (array stride formula).**

§11.5 size_align_of table — array row formula corrected to
`stride = round_up_to_multiple(size: size(elem), boundary:
align(elem))` with explicit "rounds element size up to a
multiple of element alignment" wording. Argument names labelled
to remove ambiguity.

### Round 6.2 review

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_round_6_2.txt`.

**Verdict.** `BLOCK` — 1 BLOCK + 2 MINOR + 0 NIT (3 issues total).

All four Round 6.1 findings RESOLVED (B1 FnSig identity, M2 raw
str rows, N3 RawPtr wording, N4 array stride). Three new issues
introduced by the CAbiLowering refactor.

**Findings.**

- *BLOCK 1: `TransparentInner` is not compositional across
  non-transparent C lowerings.* My CAbiShape design recorded
  `TransparentInner { inner_ty }` for repr(transparent)
  unwrapping. For `repr(transparent) struct W(Status)` where
  `Status` is fieldless `repr(u32)`, the predicate admits W by
  recursively admitting Status, but stores only
  `TransparentInner { inner_ty: Status }`. Codegen reading
  this cannot know to apply TagScalar substitution because
  repr facts are intentionally not stored in the IR (D11). The
  composition collapses.
- *MINOR 2: §14.5 misses unsized `Ref`/`Ptr` fat-pointer
  boundary cases.* §14.5's admissible list says "`Ref`/`Ptr`
  to any sized pointee" — silently leaves unsized pointees
  (`Ref(Str, _)`, `Ref(Slice(T), _)`, `Ref(Dynamic{..}, _)`)
  unmentioned. §17.2.1 defines fat-pointer lowering for these
  but §14.5 only explicitly rejects the bare unsized markers,
  not the `Ref`/`Ptr` to them.
- *MINOR 3: Side-table validation omits arity and shape-
  target checks.* §3.3 step 5 only checks
  `sig_c_abi_lowerings.len() == sigs.len()` and presence rule;
  doesn't enforce per-`CAbiLowering` arity
  (`inputs.len() == FnSig.inputs.len()`) or that
  `Substituted.emit_as` resolves to a valid TyId, and doesn't
  enforce that `TagScalar`-reason `emit_as` is a primitive int.

### Round 6.3 fix manifest

**B1 fix (compositional CAbiShape with final emit_as).**

§14.5 — `CAbiShape` redesigned to flat-with-final-emit-as:
- `Direct` — use position's own TyId.
- `Substituted { emit_as: TyId, reason: SubstReason }` — use
  the named TyId at the C boundary; `emit_as` is the *final
  composed* terminal TyId after applying transparent
  unwrapping and / or fieldless-enum substitution recursively
  to a fixed point.
- `SubstReason { TransparentChain | TagScalar |
  TransparentChainAndTagScalar }` — records why the
  substitution occurred, for diagnostics.

The composition is captured in `emit_as`, not as a recursive
shape ladder, because the shape is a *result* (codegen reads
`emit_as` directly) and storing the ladder would require
re-deriving repr facts that are intentionally not in the IR
(D11). For `extern "C" fn(W(Status))` where W is transparent
over fieldless Status: shape is `Substituted { emit_as:
TyId(u32), reason: TransparentChainAndTagScalar }`.

§14.5 worked example, §3.1 invariant 16, and the per-position
admissibility predicate updated to use the new shape names.
Trailing closing paragraph rewritten to cite `Substituted.emit_as`.

**M2 fix (explicit reject for unsized-pointee Ref/Ptr at C
boundary).**

§14.5 admissibility list — new explicit rejection bullet:
`Ptr`/`Ref` where the pointee is `Str`, `Slice(_)`, or
`Dynamic{..}` is rejected at C boundaries. Driver code crossing
extern "C" passes data pointer + length/vtable as separate
parameters. Cross-reference to §17.2.1 (Rust-internal fat-
pointer lowering still applies).

§15.1 rejection table — new row for unsized-pointee Ref/Ptr at
C boundary.

§17.2.1 wide-pointer carve-out — appended "C boundary
rejection" note clarifying the wide-pointer rules apply to
Rust-internal uses; extern "C" boundary rejects per §14.5.

**M3 fix (extended side-table validation).**

§3.3 step 5 extended with three sub-checks for each present
`CAbiLowering` entry:
- Arity: `lowering.inputs.len() == sigs[id].inputs.len()`.
- Shape-target reachability: every `Substituted.emit_as` TyId
  resolves.
- Tag-scalar primitiveness: `emit_as` for reasons `TagScalar`
  and `TransparentChainAndTagScalar` resolves to `Ty::Int(_)`
  or `Ty::Uint(_)`.

### Round 6.3 review

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_round_6_3.txt`.

**Verdict.** `APPROVE` — 0 BLOCK + 0 MINOR + 0 NIT.

All three Round 6.2 findings RESOLVED (B1 CAbiShape composition,
M2 unsized-pointee rejection, M3 side-table validation). Final-
sweep returned no remaining findings. The Round 6.3 edits
preserve the previously accepted FnSig identity fix: original
TyIds remain in `FnSig.inputs/output`, while C emission
consults `sig_c_abi_lowerings` and `Substituted.emit_as`
without requiring codegen to re-derive rustc repr facts absent
from the IR (D11).

### Round 6 closing summary

**Trajectory.**

| Round | Verdict | BLOCK | MINOR | NIT | Total |
| ---   | ---     | ---   | ---   | --- | ---   |
| 6.1   | BLOCK   | 1     | 1     | 2   | 4     |
| 6.2   | BLOCK   | 1     | 2     | 0   | 3     |
| 6.3   | APPROVE | 0     | 0     | 0   | 0     |

**Substantive design refinements that emerged from this loop.**

- *FnSig type-identity preservation under C ABI lowering.* The
  initial Round 6.1 fix for `repr(transparent)` and fieldless
  `repr(int)` ABI substitution stored the substituted type
  *into* `FnSig.inputs/output`, which would have collapsed
  rustc-distinct types like `extern "C" fn(Handle) -> Handle`
  and `extern "C" fn(u32) -> u32` into one `FnSigId` —
  violating the same D15 type-identity argument that locked
  `FnAbi`. Codex Round 6.1 caught this. The Round 6.2 fix
  introduced `sig_c_abi_lowerings: IndexVec<FnSigId,
  Option<CAbiLowering>>` as a parallel side table, with
  `CAbiShape { Direct | Substituted { emit_as, reason } }`
  recording the C-side per-position lowering. `FnSig.inputs/
  output` preserve the original Rust TyIds; codegen reads the
  side table for emission. New construction invariant 16.
- *Compositional `CAbiShape` with final-composed `emit_as`.*
  The first iteration of `CAbiShape` recorded
  `TransparentInner { inner_ty: TyId }` for transparent
  unwrapping. Codex Round 6.2 caught that `repr(transparent)
  struct W(Status)` where `Status` is fieldless `repr(u32)`
  collapses: the predicate admits W by recursively admitting
  Status as `TagScalar`, but storing only `TransparentInner
  { inner_ty: Status }` would force codegen to re-derive
  Status's repr — a fact intentionally not in the IR per D11.
  The Round 6.3 fix flattened `CAbiShape` to `Direct |
  Substituted { emit_as, reason }` with `emit_as` carrying the
  *final composed* terminal TyId after recursive unwrapping
  to a fixed point; `reason` discriminates
  `TransparentChain | TagScalar | TransparentChainAndTagScalar`
  for diagnostics.
- *Trait-object erased receivers uniformly `*mut ()`.* The
  external reviewer caught (P0.4) that the prior vtable
  shape used `*const ()` for shared `&self` method receivers.
  For impls whose `Self` contains `UnsafeCell`, mutating
  through `&self` is legitimate Rust — emitting `*const ()`
  for those receivers would be C undefined behaviour on the
  actual write. Round 6 §13.2 + §17.2.1 trait-object-dispatch
  carve-out: receivers and trait-object `data` field uniformly
  `*mut ()` regardless of source borrow flavor; Rust borrow
  flavor still preserved at the IR type level.
- *Structured `SyntheticItemKind` side table replacing rendered-
  string parsing.* Round 5 had introduced `<vtable_ty for ...>`
  rendered-prefix parsing for vtable dedup table rebuild on
  deserialise. The external reviewer caught (P0.6) that
  `def_path.rendered` is a display string, not a semantic
  recovery surface. Round 6 §13.3 `SyntheticItemKind` enum
  on a parallel `item_synthetic_kinds: IndexVec<ItemId,
  SyntheticItemKind>` side table is the structured anchor;
  rendered prefixes remain as display convention.
- *Wide-pointer carve-out: Rust-internal fat-pointer lowering
  + C boundary rejection.* The external reviewer caught
  (P0.3) that §17.2.1's `Ref`/`Ptr` C-qualification rule was
  internally contradictory with the appendix-only fat-pointer
  promise. Round 6 §17.2.1 added an explicit "Wide-pointee
  carve-out" subsection pinning the qualification + slot
  count of fat-pointer aggregates for Str/Slice/Dynamic
  pointees in Rust-internal contexts; §14.5 + §15.1 reject
  `Ref`/`Ptr` to unsized pointees at C boundaries (driver
  code passes data + length/vtable as separate parameters
  at extern "C").
- *`BuildInfo` provenance for build-equivalence quad
  verification.* The external reviewer caught (P0.5) that
  determinism claims referenced "pinned toolchain" and "equal
  bridge options" but the model carried only `TargetInfo` —
  a deserialised consumer couldn't verify the quad. Round 6
  added slimmed `BuildInfo { toolchain: StringId,
  bridge_options_hash: u128 }` (skipping the reviewer's
  fuller version of rustc-version / channel / crate-version
  / mir-opt-level on grounds of noise vs signal). Construction
  invariant 17.
- *`size_align_of` query contract for non-inline-Layout types.*
  External reviewer's P1.3 caught that validation and codegen
  both need sizes/aligns for primitives, pointers, arrays,
  fn-pointers, fat pointers — none of which carry inline
  `Layout`. Round 6 §11.5 pinned the `SizeAlign` struct +
  per-`Ty`-variant rules table, with target-derived primitives
  via `target.data_layout`.
- *Validation order parametrised by inter-table dependencies.*
  External reviewer P1.1 caught that `TyInterner` identity
  uniqueness depends on `FnSigId` and `ConstId` content-
  uniqueness — validation order matters. Round 6 §3.3 pins
  the order: strings → source_files → crates/items → consts
  → sigs → types → layout → instances → vtables rebuild. The
  Round 6.3 extension added per-`CAbiLowering` arity and
  shape-target validation.
- *HRTB position-traversal definition + 8-case test list.*
  External reviewer's P1.5 caught that the position-traversal
  rule needed mechanical precision. Round 6 §17.1 added the
  position-traversal subsection (precise rules for `&&'a T`
  nesting, generic-arg references, output-slot use; raw
  pointers fall under condition 3 not the immediate-ref-slot
  rule per Round 6.2 nit fix) plus the 8-case worked-example
  table.
- *Closed `Ty` wording sharpened.* The reviewer's revised
  P2.2 wording explicitly separates *domain extensions*
  (forbidden by Q3) from *source-language expansion*
  (revision path). Round 5's Codex Pass 1 had marked the
  earlier looser wording WRONG; this revised wording
  preserves Q3 while honestly acknowledging future toolchain
  evolution.

**Status.** Round 6 LOCKED. Top-level `spec.md` reflects the
second-external-review fix loop in full. Stage-3
`discussions.md` is unchanged; stage-3 D-tag namespace remains
unextended. Two iterative Codex catches inside the loop (Round
6.1 BLOCK on FnSig identity collapse, Round 6.2 BLOCK on
CAbiShape composition) confirm the value of the in-loop
adversarial review pattern — both gaps would have shipped as
real correctness issues without the iteration.

Spec line count: 2354 (post-Round-5) → 2891 (post-Round-6.1) →
3041 (post-Round-6.3). Net +687 lines across Round 6 fix loop;
~20 net new spec sections / subsections / invariants.

---

## Round 7 -- third-external-review fix loop

**Date.** 2026-05-01.

**Trigger.** Third external review document at
`/Users/nyengele/Downloads/regseq-spec2-deep-review.md`,
delivered after Round 6 closed at APPROVE. The reviewer
acknowledges Round 6's improvements and identifies a new set
of P0/P1/P2 findings, several of which are immediate
correctness regressions from Round 6's structural changes
(notably P0.1 unit return rejected, P0.2 array fields rejected
inside repr(C) structs).

**Adjudication.** No separate Codex pass was launched for this
review (the findings are sufficiently sharp and most are
new-since-Round-6, so an independent verdict was not
load-bearing). Issue-by-issue analysis recorded in the
consolidated take preceding the manifest below.

**Scope.** `spec.md` only. `stage-3/spec-snapshot.md` remains
the frozen close-of-stage artefact. Stage-3 D-tag namespace
remains unextended.

**Excluded from this loop (with rationale):**

- *Reviewer's option A for P0.3 (full structural CanonicalTyKey
  recursive enums).* Chose option C instead — `ArgsKey` as
  `pub struct ArgsKey(pub u128)` content hash with
  normatively-pinned canonical byte-encoding. Same cross-build
  stability, much smaller surface, no recursive enum / Box
  allocation chain. Rationale documented inline in §12.3.
- *Reviewer's reject-explicit-`'static`-binders option for
  P0.5.* Chose accept-with-documented-collapse instead.
  Rationale: identical C output, identical machine behaviour,
  rustc treats them as subtype-related (not type-equal), and
  D15's lock guards types with *distinct* C outputs. Documented
  in §17.1.
- *No test-suite landed in spec body.* The reviewer's 8-case
  HRTB test list and other test matrices are stage-13
  implementation work, not spec-body content. The §17.1
  position-traversal test table covers the HRTB cases at
  spec-body level.

### Round 7.1 fix manifest

Edits landed in `spec.md`. Each entry: source finding -> spec
section(s) touched -> one-line summary.

**Tier 1 (urgent / structural):**

1. *P0.1 Unit-return as void at FnAbi::C output.* §14.5 -- new
   "Unit return" rule in CAbiPositionAdmissible: empty tuple
   admitted at output position with shape `Direct`; codegen
   emits C `void`. Empty tuple at input position rejected in
   v1 (no ZST omission policy). §15.1 rejection row for unit
   at input. §14.3 FnSig invariant explicitly admits empty
   tuple as output.
2. *P0.2 Split admissibility into position vs field
   predicates.* §14.5 -- restructured into
   `CAbiPositionAdmissible(ty, position)` and
   `CFieldAdmissible(ty)`. Different acceptance sets:
   `Array` rejected at positions but admitted as fields with
   element recursion; ZST unit admitted as field; tuple
   non-unit rejected in both. The transparent-wrapper rule
   recurses through `CAbiPositionAdmissible` (boundary
   semantics); `repr(C)` aggregates recurse through
   `CFieldAdmissible` (field semantics). This fixes the Round
   6 collapse where `repr(C) struct { bytes: [u8; 16] }` was
   rejected because Array failed the single-recursive predicate.
3. *P0.3 ArgsKey concrete schema.* §12.3 -- pinned `ArgsKey`
   as `pub struct ArgsKey(pub u128);` (128-bit content hash via
   SipHash-2-4 with fixed all-zeros key). Normative byte-
   encoding pinned per `Ty` variant tag (append-only u8
   discriminants 0-18) with per-IR-identifier termination
   rules. Cross-build determinism preserved via stable
   canonicalisation; no allocation-order IDs in encoded bytes.
4. *P0.4 PrincipalTraitKey for vtable identity.* §13.3 -- new
   `PrincipalTraitKey { trait_path, trait_args,
   projection_bindings }` struct. All four vtable-family
   `SyntheticItemKind` variants and `RegseqModel.vtables`
   updated to key on `PrincipalTraitKey` instead of bare
   `(trait_path, projection_args)`. `dyn Register<u32>` and
   `dyn Register<u64>` now have distinct vtable types as they
   should.
5. *P0.5 HRTB output-only rejection + 'static collapse policy.*
   §17.1 -- added condition 5 (bound region in output position
   rejected) to predicate. Test table updated with output-
   position cases (Tests D, F). New "'static and free-region
   collapse policy" subsection: accept both `fn(&u32)` and
   `fn(&'static u32)`; document the collapse explicitly with
   D15 reasoning (subtype-related types with identical C
   output do not violate D15).
6. *P0.6 Vtable synthetic layout algorithm.* §13.2 -- added
   "Synthetic vtable layout algorithm" subsection. Closed
   8-step algorithm pinning representation (`repr(C)`-like),
   field order (`tcx.vtable_entries` order), per-field
   sizes/aligns from `target.scalar_layouts`, offsets via
   standard C aggregate layout, aggregate alignment and size
   computation. Provenance is location-pinned (no explicit
   tag): §11 governs source-derived; §13.2 governs vtable.
7. *P0.7 Canonical vacant vtable slot type.* §13.2 -- vacant
   slots typed as `Ty::FnPtr(canonical_vacant_sig_id)` where
   the canonical vacant sig is `extern "C" fn(*mut ()) -> ()`
   (single shared FnSig entry across all vtables). Pinned slot
   size = pointer width; null initializer. Erased pointee Ty
   pinned as canonical `Ty::Tuple { fields: [], layout:
   {size:0, align:1} }` (Rust unit), with codegen carve-out
   emitting `void *` per the trait-object dispatch rule.

**Tier 2 (high-value):**

8. *P1.1 Item / ItemKind schema.* §8.0 (new) -- pinned
   `pub struct Item { def_path, kind, span,
   is_phantom_data, is_unsafe_cell, is_manually_drop,
   opaque_handle_eligible }` and `pub enum ItemKind { Function,
   Static, Const, Struct, Enum, Union }`. Attributes
   intentionally not stored (§8.0 rationale).
9. *P1.2 Synthetic DefPath segments + DefPathHash construction.*
   §13.4 (new) -- pinned segment encoding per family
   (closure/vtable) using reserved `<regseq_synth>` first-
   segment domain separator. Pinned synthetic DefPathHash
   construction: `SipHash-2-4` over domain string +
   variant discriminant + triggering crate id + canonical
   payload bytes. Reserved `StableCrateId(0)` for synthetic
   items domain-separates from real-crate `DefPathHash` values.
   New construction invariant 18 (item_def_paths global
   uniqueness across source-derived and synthetic spaces).
10. *P1.3 ScalarLayouts table on TargetInfo.* §3 -- added
    `ScalarLayouts { bool_, char_, int[], uint[], float[],
    fn_ptr, data_ptr }` field to `TargetInfo`. Bridge populates
    from rustc; consumers query directly. §11.5 size_align_of
    primitive cases now read `target.scalar_layouts.*` instead
    of "target-derived from data_layout". Construction invariant
    15 extended.
11. *P1.4 Validation order parsing/validation separation.*
    §3.3 rewritten with explicit Phase A (parse all storage)
    + Phase B (validate ordered by inter-table dependency)
    + Phase C (rebuild transient maps). Resolves the Round 6
    chicken-and-egg of validating sigs against types.len()
    before types loaded.
12. *P1.5 FnPtr nested declarator emission.* §14.5 -- new
    "Nested FnPtr declarator emission" paragraph: when emitting
    `Ty::FnPtr(sig)` as a C declarator, codegen reads
    `sig_c_abi_lowerings[sig]` for the nested signature and
    applies per-position shapes inside the declarator.
    Validation invariant: any FnPtr admitted at a C boundary
    or as a C-aggregate field has an entry in
    `sig_c_abi_lowerings`.
13. *P1.6 TagScalar Bool/Char policy.* §11.2 -- pinned
    "logical scalar type" policy: `scalar_ty` may be Bool,
    Char, Int, Uint, Ptr, Ref. Float not admitted. Added
    width-fit invariant: stored `valid_range` / `niche_start` /
    `tag_value` u128 values must fit the bit width of
    scalar_ty (Bool = 8, Char = 32, etc.).
14. *P1.7 ConstIdentityKey relocation forward expectation.*
    §9.7 stage-4 list extended: cross-build canonical encoding
    of InstanceId references must use stable instance identity
    `(regseq_instance_kind_rank, item_def_path_hash,
    canonical_generic_args, fn_shim_disambig)`, not local
    integer.
15. *P1.8 FunctionInstanceFlavor on Instance.* §9.4 --
    `InstanceKind::Function` extended with `flavor:
    FunctionInstanceFlavor`. Closed enum with 15 variants
    mirroring rustc's `ty::InstanceKind` rank table; append-
    only on toolchain bumps. Avoids parsing `emission_name`
    for stable-instance-identity reconstruction.
16. *P1.9 TyInterner stronger guardrails.* §12 visibility
    note extended with three explicit rules: no outside code
    uses HashMap<Ty,_>; full-value comparison via
    `Ty::full_eq`; validation walks layout fields structurally.
17. *P1.10 Opaque-handle global-use predicate as Item
    metadata.* §8.0 added `opaque_handle_eligible: bool` field
    on Item. §14.5 "Opaque-handle convention" paragraph
    rewritten to describe the global reachability analysis
    the bridge runs at construction. New construction
    invariant 19 (consistency conditions: ItemKind::Struct,
    repr(C), ZST, every use behind Ref/Ptr).

**Tier 3 (cheap clarifications):**

18. *P2.1 Empty tuple = unit footnote.* §10 Tuple row updated.
19. *P2.2 Never-input rejection generalised.* §14.3 FnSig
    construction invariants extended: Never inputs rejected
    for all FnSigs (not just c_variadic).
20. *P2.3 Closure call receiver shapes.* §13.1 -- per-
    ClosureCallKind receiver pinned: Call → &Captures,
    CallMut → &mut Captures, CallOnce → Captures by value.
21. *P2.4 Vtable erased receiver canonical Ty.* §13.2 --
    pinned `Ty::Tuple { fields: [] }` as the canonical
    erased pointee; codegen carve-out emits `void *`.
22. *P2.5 Mechanical LayoutKind ↔ Ty matching.* §12.5 step 4
    expanded with five explicit mechanical checks (variant ↔
    kind, tuple field-name convention, field reachability,
    field-fits-in-layout with checked arithmetic, alignment
    respected modulo packed override).
23. *P2.6 Array stride overflow validation.* §11.5 Array row
    extended with `checked_mul` + target usize-max bound.
24. *P2.7 Char rejection extends to repr(C) fields.* §14.5
    `CFieldAdmissible` rejects Char with same validity-
    invariant rationale. §15.1 rejection row added.
25. *P2.8 Wide-scalar backend rejection through nested FnPtr.*
    §10.3 extended: I128/U128/F16/F128 rejection applies at
    any backend-emitted position including nested FnPtr
    declarators.
26. *P2.9 BuildInfo.toolchain wording tightened.* §3 doc
    comment expanded: must carry channel + date (for
    nightlies) or version string; commit hash when available;
    host triple. Pinned-toolchain example (project's nightly)
    documented.
27. *P2.10 Restrict emission constraint.* §17.2 -- restrict
    rule rewritten: emitted only on direct fn parameters
    whose MIR use does not copy the pointer; never for
    erased trait-object data or raw pointers; absence is
    safe default; type system carries no restrict flag and
    UniqueBorrow does not mechanically imply restrict.

### Round 7.1 review

**Codex run.** `--effort high`, `--timeout 1800`, output captured
to `/tmp/codex_round_7_1.txt`.

**Verdict.** `BLOCK` — 1 BLOCK + 4 MINOR + 0 NIT (5 issues total).

All 27 manifest items landed: 22 cleanly, 5 with stale-text
or contradiction issues. Codex did not disagree with any
manifest exclusion. The findings are coordination failures from
the Round 7.1 edits — sections updated locally without all the
corresponding stale text in other sections being swept.

**Findings.**

- *BLOCK 1: ArgsKey hash construction is not constructively
  specified.* §12.3 said `ArgsKey(pub u128)` is computed by
  `SipHash-2-4` with a fixed all-zeros key, but standard
  SipHash-2-4 yields 64-bit output, not 128-bit. Two conforming
  implementations cannot derive the same 16 bytes. Same problem
  in §13.4 synthetic DefPathHash.
- *MINOR 2: PrincipalTraitKey sweep left old vtable identity
  text.* §3.1 invariant 13 still said `(principal_trait,
  projection_bindings)` and named `VTableType { trait_path,
  projection_args }`. §13.2 still introduced vtable type Items
  per `(Trait, projection_bindings)`. Risks reintroducing the
  P0.4 collapse.
- *MINOR 3: ScalarLayouts remains optional in §11.5.* §3 says
  `scalar_layouts` is authoritative; §11.5 still said primitive
  sizes are "target-derived from `target.data_layout`" with the
  bridge "MAY pre-populate" a side scalar table. Drift.
- *MINOR 4: HRTB output-position rule contradicts traversal
  definition.* Condition 5 rejects all output-position bound
  regions, but the traversal bullet still said an output-only
  single-use binding is acceptable. Direct contradiction.
- *MINOR 5: FunctionInstanceFlavor storage contradicts §9.3
  prose.* §9.4 added `flavor: FunctionInstanceFlavor`, but §9.3
  still said "shim discriminator is NOT exposed as a public
  field on Instance."

### Round 7.2 fix manifest

**B1 fix (ArgsKey hash construction).** §12.3 ArgsKey rewritten
to use **SHA-256 truncated to leading 16 bytes** (big-endian),
loaded as `u128`. Portability rationale (every conforming
implementation produces identical digests), cryptographic-grade
collision resistance documented inline. §13.4 synthetic
DefPathHash construction also rewritten to use **SHA-256
truncated to leading 8 bytes** (big-endian) for the `local_hash:
u64` per §7.3 schema. Same hash family (SHA-256) used uniformly
for both content-addressed identifiers; differs only in
truncation width. Resolves the BLOCK 1 implementability gap.

**M2 fix (PrincipalTraitKey sweep).**

§3.1 invariant 13 rewritten: vtable_ty now described as
resolving to the synthesised vtable Ty::Struct for the trait
object's full PrincipalTraitKey (`trait_path`, `trait_args`,
`projection_bindings`); SyntheticItemKind::VTableType payload
correctly named as `{ principal: PrincipalTraitKey }`.

§13.2 vtable type Item bullet rewritten: keying described as
"per PrincipalTraitKey" with the worked example
`dyn Register<u32>` vs `dyn Register<u64>` (differ only in
trait_args). Rendered DefPath form updated to include trait_args
in the rendered shape.

**M3 fix (ScalarLayouts authoritative in §11.5).**

§11.5 size_align_of table primitive rows rewritten:
- Bool/Char/Int(k)/Uint(k)/Float(k) now read explicit
  `target.scalar_layouts.{bool_, char_, int[k], uint[k],
  float[k]}` entries.
- Ref/Ptr to sized pointee reads `target.scalar_layouts.data_ptr`.
- Ref/Ptr to unsized pointee uses `(2 * data_ptr.size,
  data_ptr.align)`.
- FnPtr reads `target.scalar_layouts.fn_ptr`.

Explanatory paragraph below the table replaces the old "MAY
pre-populate" text with a normative "scalar_layouts is the
authoritative source; consumers do not parse data_layout."

**M4 fix (HRTB traversal aligned with condition 5).**

§17.1 traversal bullet for output-position rewritten: "Output-
position bound regions are rejected by condition 5 in v1,
regardless of whether the same region appears elsewhere — even
output-only single-use bindings collapse with `'static` /
free-region forms post-erasure and are rejected to preserve
identity."

**M5 fix (§9.3 reflects FunctionInstanceFlavor on Instance).**

§9.3 paragraph about the shim discriminator rewritten: "The
shim discriminator IS exposed as a public field on Instance via
`InstanceKind::Function.flavor`" with cross-reference to §9.4
and the rank-table mapping
`FunctionInstanceFlavor` → `fn_shim_disambig: u8`.

### Round 7.2 review

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_round_7_2.txt`.

**Verdict.** `MINOR` — 0 BLOCK + 2 MINOR + 0 NIT (2 issues total).

All five Round 7.1 findings RESOLVED. Two new minor issues —
both byte-order/sweep nits introduced by the Round 7.2 hash-
unification + PrincipalTraitKey sweep:

- *MINOR 1: `ArgsKey bytes` byte order not pinned for synthetic
  payloads.* §12.3 defines `ArgsKey` as `u128` loaded big-
  endian from the leading 16 SHA-256 digest bytes; §13.4
  references "`ArgsKey bytes (16 B)`" inside synthetic
  `DefPathHash` payloads without specifying the byte order.
  Two implementations could produce byte-different payloads.
- *MINOR 2: §9.6 stale vtable type rendered form.* §13.2 and
  §13.4 now use
  `<vtable_ty for Trait[<trait_args>][<projection_bindings>]>`,
  but §9.6's example block still showed the old
  `<vtable_ty for Trait[<projection_bindings>]>` (no
  `trait_args`).

### Round 7.3 fix manifest

**M1 fix (ArgsKey canonical byte form pinned).** §12.3 — added
"Canonical byte form of `ArgsKey` (for embedding in other hash
inputs)" subsection. `ArgsKey` byte form = 16 leading SHA-256
digest bytes in their original digest order, equivalently
`ArgsKey.0.to_be_bytes()`. `ArgsKey hex` (used in §13.4
DefPath segments) = lower-case hex of the same 16 bytes. Pins
encoding so all implementations produce byte-identical synthetic
payloads.

**M2 fix (§9.6 sweep).** §9.6 vtable rendered-form example
updated to include `trait_args`:
`<vtable_ty for Trait[<trait_args>][<projection_bindings>]>`.
Cross-references to §13.2 (rendered-form contract) and §13.4
(structured `DefPath.segments` encoding) added.

### Round 7.3 review

**Codex run.** `--effort high`, `--timeout 1200`, output captured
to `/tmp/codex_round_7_3.txt`.

**Verdict.** `MINOR` — 0 BLOCK + 1 MINOR + 0 NIT (1 issue total).

Both Round 7.2 findings RESOLVED (M1 ArgsKey byte order, M2
§9.6 sweep). Final-sweep flagged one cross-document staleness:
`stage-3/discussions.md` line 523 still uses the pre-Round-7
`(Trait, projection_bindings)` keying language for the SESSION-A
vtable type Item parallel.

**Adjudication.** `stage-3/discussions.md` is the **frozen
historical audit trail** of stage-3 decisions; the project
methodology preserves it as a record of decisions at the time
they were made (per the merger contract and §5.5 close-of-stage
discipline). Round 7's `PrincipalTraitKey` refinement does not
rewrite history — the SESSION-A *conceptual* decision (per-
trait-object-identity vtable type Items) is unchanged; only
the per-key field set expanded to address the generic-trait
collapse. The methodologically clean response is **not** to
rewrite the historical text but to add a single editorial
forward-pointer note that preserves the original sentence and
points readers at the live spec for the post-Round-7 keying.

### Round 7.4 fix manifest

**M1 fix (forward-pointer note in discussions.md).**

`stage-3/discussions.md` line 523 — added a footnote-style
`<sup>†</sup>` marker after the SESSION-A
`(Trait, projection_bindings)` sentence and a quoted
explanatory note immediately below: documents the Round 7
refinement (per `review-log.md` Round 7.1 manifest item 4 /
P0.4); states that the conceptual decision is unchanged but
the per-key fields expanded; cross-references `spec.md` §13.3
as the live shape. Original sentence preserved verbatim.

This is the single editorial touch on stage-3/discussions.md
across all of Round 7 — the audit trail otherwise remains
unchanged. Rationale documented above (R7.3 adjudication).

### Round 7.4 review

**Codex run.** `--effort high`, `--timeout 1200`, output captured
to `/tmp/codex_round_7_4.txt`.

**Verdict.** `APPROVE` — 0 BLOCK + 0 MINOR + 0 NIT.

The R7.3 finding is RESOLVED: discussions.md preserves the
original SESSION-A historical sentence verbatim; the footnote-
style note clearly marks the Round 7 refinement and forwards
to `PrincipalTraitKey` in spec.md §13.3. Cross-references
between spec.md, discussions.md, and review-log.md verified
consistent. Final-sweep returned no remaining findings.

### Round 7 closing summary

**Trajectory.**

| Round | Verdict | BLOCK | MINOR | NIT | Total |
| ---   | ---     | ---   | ---   | --- | ---   |
| 7.1   | BLOCK   | 1     | 4     | 0   | 5     |
| 7.2   | MINOR   | 0     | 2     | 0   | 2     |
| 7.3   | MINOR   | 0     | 1     | 0   | 1     |
| 7.4   | APPROVE | 0     | 0     | 0   | 0     |

**Substantive design refinements that emerged from this loop.**

- *Two regressions from Round 6 caught immediately at R7.1.*
  Round 6's structural changes introduced two real correctness
  bugs that the third external reviewer caught (P0.1 unit
  return rejected; P0.2 `repr(C) struct { bytes: [u8; 16] }`
  rejected). Round 7.1 landed both fixes plus 25 other items
  in one batch. These weren't pure regressions of older
  Round-5/4 spec — they were artifacts of Round 6's new
  recursive C-ABI predicate (P0.2) and Round 6's `Tuple { .. }`
  rejection (P0.1) being too broad. Caught at the first
  external-review-after-Round-6 cycle.
- *Position vs field admissibility split.* The single recursive
  C-ABI predicate from Round 6 conflated two different
  questions: "can `ty` be passed by-value at a C boundary?" vs
  "can `ty` appear inside a C-representable aggregate field?".
  These have different acceptance sets (Array admitted as
  field, rejected at boundary; ZST unit admitted as field,
  rejected as parameter). Round 7 split into
  `CAbiPositionAdmissible(ty, position)` and
  `CFieldAdmissible(ty)`. The mutual recursion is explicit:
  aggregates recurse on `CFieldAdmissible`; transparent-wrapper
  unwrapping recurses on `CAbiPositionAdmissible` (boundary
  semantics).
- *`PrincipalTraitKey` for trait-object identity.* The
  `(trait_path, projection_bindings)` keying from Round 6
  collapsed generic trait objects: `dyn Register<u32>` and
  `dyn Register<u64>` would share a vtable type Item. Round 7
  introduced `PrincipalTraitKey { trait_path, trait_args,
  projection_bindings }` and threaded it through all four
  vtable-family `SyntheticItemKind` variants and the
  `RegseqModel.vtables` dedup table. R7.1's MINOR 2 caught a
  partial sweep (invariant 13 + §13.2 still had stale
  `(Trait, projection_bindings)` text); R7.2 completed the
  sweep. R7.3 caught an additional staleness in
  discussions.md, resolved with a forward-pointer footnote
  rather than rewriting historical text.
- *`ArgsKey` and synthetic `DefPathHash` unified under SHA-256
  truncation.* R7.1's first iteration pinned `ArgsKey` to
  `SipHash-2-4` with all-zeros key — but SipHash-2-4 produces
  64-bit output, not 128-bit, so two implementations couldn't
  derive identical 16 bytes. R7.1 BLOCK 1 caught this. R7.2
  switched to **SHA-256 truncated to leading 16 bytes (big-
  endian) loaded as `u128`** for ArgsKey, and **SHA-256
  truncated to leading 8 bytes for the synthetic
  `DefPathHash.local_hash: u64`**. Same hash family for both,
  differing only in truncation width. R7.2 caught a residual
  byte-order ambiguity in how `ArgsKey` bytes embed in
  synthetic payloads; R7.3 pinned the canonical byte form
  (`ArgsKey.0.to_be_bytes()`).
- *`FunctionInstanceFlavor` exposes the shim discriminator on
  `Instance`.* The `RegseqInstanceKey.fn_shim_disambig: u8`
  used during interning didn't appear on the deserialised
  `Instance`. R7.1 manifest item 15 added `flavor:
  FunctionInstanceFlavor` to `InstanceKind::Function` (closed
  enum mirroring rustc's 15-variant `ty::InstanceKind` rank
  table; append-only on toolchain bumps). Resolves the
  brittleness of reconstructing stable instance identity by
  parsing `emission_name`.
- *`ScalarLayouts` table makes the model self-describing.*
  R7.1 added `target.scalar_layouts: ScalarLayouts` storing
  the bridge-resolved `SizeAlign` for every primitive scalar
  type. R7.1 MINOR 3 caught that §11.5 still said primitive
  sizes are "target-derived from `data_layout`" with the side
  table "MAY be pre-populated"; R7.2 made `scalar_layouts`
  the authoritative source ("consumers do not parse
  `data_layout`"). The model is now genuinely rustc-free for
  primitive size/align queries.
- *Synthetic `DefPath.segments` and `DefPathHash` construction
  pinned.* R7.1 §13.4 added a complete specification of
  synthetic-item DefPath segment encoding (per family,
  reserved `<regseq_synth>` first-segment domain separator)
  and synthetic `DefPathHash` construction (SHA-256 over
  domain string + variant discriminant + triggering crate id
  + canonical payload bytes; reserved `StableCrateId(0)`
  domain-separates synthetic from real-crate hashes). New
  construction invariant 18 pins global `item_def_paths`
  uniqueness across both spaces.
- *HRTB predicate gained condition 5 (output-position
  rejection) + documented `'static` collapse policy.* Round
  6's predicate accepted output-only single-use bound
  regions; R7 added condition 5 (reject output-position
  bound regions outright) to prevent collapse with
  `fn() -> &'static T` shapes. The `'static` vs elided
  collapse for input-position single-use cases is documented
  explicitly as accepted with rationale (identical C output,
  identical machine behaviour, rustc subtype-related not
  type-equal).
- *`Item` schema and `opaque_handle_eligible` global predicate.*
  R7.1 §8.0 finally pinned the `Item` struct (referenced
  throughout but never previously defined) and added
  `opaque_handle_eligible: bool` as a bridge-computed item
  metadata field carrying the global-reachability outcome
  for the C incomplete-struct lowering convention. New
  construction invariant 19 (consistency conditions).

**Status.** Round 7 LOCKED. Top-level `spec.md` reflects the
third-external-review fix loop in full. `stage-3/discussions.md`
received a single footnote-style editorial note (R7.4) marking
the Round 7 refinement of SESSION-A's vtable keying; the
historical SESSION-A text is preserved verbatim. Stage-3
D-tag namespace remains unextended. Three iterative Codex
catches inside the loop (R7.1 BLOCK on FnSig identity hash
construction, R7.2 byte-order ambiguity, R7.3 cross-document
staleness) confirm the value of the in-loop adversarial
review pattern — all three would have shipped as real
correctness or interop issues without iteration.

Spec line count: 3041 (post-Round-6) → 3733 (post-Round-7.1)
→ 3760 (post-Round-7.3). Net +719 lines across Round 7 fix
loop; ~10 net new spec sections / subsections / invariants
(notable: §3 ScalarLayouts + BuildInfo, §3.3 phase-split
validation order, §8.0 Item schema, §11.5 size_align_of,
§12 TyInterner guardrails, §12.3 ArgsKey hash schema,
§13.3 PrincipalTraitKey, §13.4 synthetic DefPath
construction, §14.5 split admissibility predicates).

---

## Round 8 -- fourth-external-review fix loop

**Date.** 2026-05-02.

**Trigger.** Fourth external review document at
`/Users/nyengele/Downloads/regseq-spec3-deep-review.md`,
delivered after Round 7 closed at APPROVE. The reviewer
acknowledges Round 7's structural improvements and identifies
23 P0/P1/P2 findings, several of which are immediate
correctness regressions from Round 7's structural additions
(P0.1 closure synthetic identity collision, P0.5 Item-level
opaque flag for generic structs, P0.7 SizeAlign-vs-Layout
schema mismatch).

**Adjudication.** No separate Codex pass was launched (the
findings are sharp and most are new-since-Round-7 schema
issues; an independent verdict would not add load-bearing
information). Issue-by-issue analysis recorded in the
consolidated take preceding the manifest below.

**Scope.** `spec.md` only. `stage-3/spec-snapshot.md` remains
the frozen close-of-stage artefact. Stage-3 D-tag namespace
remains unextended.

**Excluded from this loop (with rationale):**

- *Reviewer's per-monomorphization side table for opaque-handle
  eligibility (P0.5 alternative).* Chose v1 non-generic
  restriction instead. Driver opaque handles are typically
  `#[repr(C)] struct OpaqueCtx { _private: [u8; 0] }` — non-
  generic. Restricting to non-generic in v1 is methodology-
  aligned (reject what's out-of-scope rather than over-engineer
  for hypothetical needs). Future revisions can lift to a
  per-monomorphization side table if generic opaque handles
  surface as a real driver use case.
- *Reviewer's option to reuse `ArgsKey` with pinned grammar for
  projection bindings (P0.4 alternative).* Chose distinct
  `ProjectionBindingsKey` type for clarity at use sites: the
  encoding rules are different (sorted by `assoc_item
  DefPathHash` instead of by position), and a separate type
  prevents accidental misuse.
- *Reviewer's stage-3 ConstInterner split (P1.1 alternative).*
  Chose stage-4 forward expectation instead. Stage 3's
  ConstInterner currently carries only scalar primitives +
  array lengths (no value-level constants), so the dependency
  cycle is theoretical at stage 3. Stage 4 is the right place
  to introduce the two-phase validation.
- *Reviewer's proposed alternative B for P0.3 (`RegseqInternalC`
  variant on `FnAbi`).* Chose Option A (vtable shims use
  `FnAbi::C` uniformly; trait objects with non-C-ABI methods
  rejected). Option A is simpler and matches the OS-integration
  framing of Regseq traits (driver callbacks are C-shaped
  surfaces).
- *Test cases in spec body.* P2.2 / P2.4 / P2.5 test
  recommendations are stage-13 implementation work, recorded
  as forward expectations.

### Round 8.1 fix manifest

Edits landed in `spec.md`. Each entry: source finding -> spec
section(s) touched -> one-line summary.

**Tier 1 (urgent / structural):**

1. *P0.1 Closure synthetic identity.* §13.1 + §13.4 + §3.1
   inv 2: closure captures Item is **source-derived** (uses
   rustc closure DefPath verbatim, rustc DefPathHash);
   `SyntheticItemKind::ClosureCaptures` is purely a
   discriminator. Only `ClosureCall` and `ClosureDropGlue` use
   synthetic DefPaths under `<regseq_synth>`. §13.4 closure
   family payload now uses **closure expression's**
   `DefPathHash` (not the enclosing function's), so two sibling
   closures `let a = || 1; let b = || 2;` get distinct identity.
   Inv 2 carve-out documents the ClosureCaptures source-derived
   exception.
2. *P0.2 Drop "triggering crate" from synthetic identity.*
   §13.4 synthetic-DefPathHash construction stripped of
   `triggering_crate.stable_crate_id` payload. Identity is
   purely (synthetic_kind, family-specific payload). New
   "Per-family ownership" section in §13.4 specifies
   deterministic `crate_id` for cross-references / display
   (closure: closure expression's crate; vtable: principal
   trait's crate, fallback `primary_crate`). §9.6 updated to
   reflect identity-vs-display split.
3. *P0.3 Vtable shim FnAbi pinned as FnAbi::C.* §13.2 added
   "Method shim and drop shim ABI" subsection: all
   bridge-synthesised vtable shim Function Items use
   `FnAbi::C` (matching vacant slot canonical sig). Their
   erased signatures checked with `CAbiPositionAdmissible`
   after receiver erasure. New §15.1 row:
   `BridgeError::UnsupportedTraitObjectMethod` for trait
   objects with non-C-ABI-admissible dispatchable methods.
4. *P0.4 ProjectionBindingsKey distinct from ArgsKey.* §13.3
   introduced `pub struct ProjectionBindingsKey(pub u128)`
   with normative byte encoding: sorted entries by `assoc_item
   DefPathHash` containing `(path, binding_kind, canonical
   value bytes)`. `PrincipalTraitKey.projection_bindings`
   field type changed from `ArgsKey` to
   `ProjectionBindingsKey`. §13.4 vtable family payload
   updated to use `ProjectionBindingsKey` bytes.
5. *P0.5 Opaque-handle eligibility restricted to non-generic
   Items.* §3.1 invariant 19 rewritten with five conditions:
   `ItemKind::Struct`, `repr(C)`, **non-generic**, monomorphic
   layout ZST, every reachable use behind Ref/Ptr. Generic
   opaque-handle candidates rejected at bridge with
   `BridgeError::GenericOpaqueHandle` (new §15.1 row). §8.0
   `Item.opaque_handle_eligible` doc rewritten to match.
   Removes the inconsistent "Item's layout" wording (Items
   don't have layouts).
6. *P0.6 Primitive enum schemas + actual `pub enum Ty`.* §10.0
   added with concrete `pub enum Ty` Rust block. §10.0 also
   pins `IntKind`, `UintKind`, `FloatKind` (`#[repr(u8)]`,
   append-only discriminants 0..N) plus `INT_KIND_COUNT` /
   `UINT_KIND_COUNT` / `FLOAT_KIND_COUNT` constants and
   `Safety { Safe = 0, Unsafe = 1 }`. §10.1 added concrete
   Rust blocks for `Provenance`, `Mutability`, `AutoTrait`,
   `FnAbi` with `#[repr(u8)]` discriminants.
7. *P0.7 UNIT_LAYOUT constant.* §11 added `pub const
   UNIT_LAYOUT: Layout` constant. §13.2 erased-pointee Ty
   rewritten to use `UNIT_LAYOUT` (was incorrectly
   `SizeAlign { size: 0, align: 1 }`, a schema mismatch).
   New §3.3 Phase B validation: `Ty::Tuple { fields: [] }`
   carries exactly `UNIT_LAYOUT`.
8. *P0.8 Validation enforcement gaps.* §3.3 Phase B extended
   with explicit per-invariant validation steps: unsized
   marker placement (inv 12), Ty::Dynamic well-formedness
   (inv 13), SyntheticItemKind ↔ ItemKind consistency,
   item_def_paths uniqueness (inv 18), opaque-handle
   consistency (inv 19), FnSig invariants, CAbiLowering shape,
   layout invariants (non-overlap, packed alignment formula,
   tag scalar width-fit), F4-2 sorted instance order, unit
   layout. §3.3 Phase C extended with mandatory duplicate-key
   rejection during vtable rebuild.

**Tier 2 (high-value):**

9. *P1.1 Stage-4 const validation phase split (forward
   expectation).* §9.7 added "Stage-4 const-validation phase
   split" subsection: type-level const phase (early, before
   TyInterner) for scalar const generics; value-level const
   phase (late, after TyInterner + instances) for richer
   constants. Implementation may use one table with two-pass
   validation or two tables.
10. *P1.2 PhantomData ZST CFieldAdmissible.* §14.5 added rule
    admitting `Struct { item, .. }` with
    `model.items[item].is_phantom_data == true && layout.size
    == 0 && layout.align == 1` as a C-aggregate field, even
    though PhantomData is default `repr(Rust)`. Codegen omits
    such fields from emitted C aggregates.
11. *P1.3 Packed alignment formula.* §3.3 Phase B layout
    invariants: `required_field_align = match layout.pack {
    None => align(field), Some(n) => min(align(field), n) }`;
    `field.offset % required_field_align == 0`. Replaces the
    Round 7 too-weak rule that allowed offset 1 for u32 in
    `repr(packed(2))`.
12. *P1.4 Non-overlapping non-ZST field validation.* §3.3
    Phase B layout invariants: no two non-ZST FieldLayouts in
    the same struct/tuple/VariantLayout payload may have
    overlapping byte intervals. ZST fields may share offsets.
13. *P1.5 Never-input as DSL rejection.* §14.3 wording changed
    from "bridge bug" to explicit
    `BridgeError::UnsupportedNeverInput { fn_sig_id_or_callee_def_id,
    input_idx }`. New §15.1 rejection row.
14. *P1.6 Static mutability + foreign statics.*
    `InstanceKind::Static` extended: `Static { ty: TyId, init:
    Option<ConstId>, mutability: Mutability }`. `None` for
    foreign statics; `mutability` distinguishes
    `static`/`static mut` (affects emitted C `const` storage
    and aliasing).
15. *P1.7 StableCrateId(0) bridge-side validation.* §3.1 new
    invariant 20: bridge validates at extraction that no real
    crate has `stable_crate_id == 0`; if encountered, hard-
    rejects with `BridgeError::StableCrateIdZeroCollision`.
    Synthetic-real domain separation is rejection-enforced,
    not hash-probabilistic.
16. *P1.8 ArgsKey collision wording (honesty).* §12.3 collision
    paragraph rewritten: "stable content fingerprint, not
    proof of equality"; serialised model stores `u128` only;
    cryptographic-grade collision resistance below `2^-50`
    for realistic driver volumes; debug-build retention of
    canonical bytes is optional, not part of serialised
    contract.
17. *P1.9 'static collapse exception text.* §17.5 (new) "Type-
    identity preservation contract (with explicit exception)":
    end-to-end identity preservation for nominal/ABI/safety/
    const-generic/non-region structural distinctions; single
    exception is the §17.1-admitted region-only collapse for
    fn-pointer types. Existing §17.5 "Spec is mutable"
    renumbered to §17.6.
18. *P1.10 bridge_options_hash canonical algorithm.* §3
    BuildInfo doc extended with normative algorithm: SHA-256
    truncated to leading 16 bytes (big-endian) over a sorted
    UTF-8 `name=value` byte encoding. Pinned option set:
    `mir_opt_level`, `cleanup_pass_whitelist`,
    `ub_check_force_offs`, plus future flags.

**Tier 3 (cleanup):**

19. *P2.1 Dedup duplicate schema definitions.* §3
    `SyntheticItemKind` enum body removed; replaced with
    `pub enum SyntheticItemKind { /* ... see §13.3 ... */ }`
    placeholder pointing at the normative §13.3 location.
    Avoids drift between §3 and §13.3.
20. *P2.3 trait_path doc clarification.* §13.3
    `PrincipalTraitKey.trait_path` doc comment expanded:
    "need **not** correspond to an `ItemId` in `model.items`
    — trait declarations are erased per §8.1; implementers
    must not validate `trait_path` through `item_def_paths`."

**Stage-13 forward expectations (P2.2 / P2.4 / P2.5):** test
notes for `Ty::full_eq` vs identity equality, pretty-printer
CAbiLowering rendering, and wide-scalar rejection through
nested FnPtr — recorded as stage-13 implementation work, not
spec-body content.

### Round 8.1 review

**Codex run.** `--effort high`, `--timeout 1800`, output captured
to `/tmp/codex_round_8_1.txt`.

**Verdict.** `MINOR` — 0 BLOCK + 3 MINOR + 2 NIT (5 issues total).

All 20 manifest items landed: 16 cleanly, 4 with sweep-coverage
gaps (P0.4 ProjectionBindingsKey, P0.5 opaque-handle non-generic
propagation, P1.3 packed alignment, P2.1 schema dedup). All five
new findings are coordination failures from incomplete sweeps —
no new design issues. Codex agreed with all manifest exclusions
(non-generic v1 opaque-handle restriction, distinct
ProjectionBindingsKey type, stage-4 const-validation phase
split, Option A FnAbi::C vtable shims, test-suite deferral).

**Findings.**

- *MINOR 1: Packed alignment validation contradiction.* §3.3
  Phase B has the correct formula
  (`min(align_of(field.ty), pack)`); §12.5 still has the old
  too-weak rule (`pack <= align_of(field.ty)`) that allowed
  offset 1 for `repr(packed(2))` `u32`. Two normative
  validation sections disagree.
- *MINOR 2: ProjectionBindingsKey stale text.* §13.3 schema
  introduces `ProjectionBindingsKey`, but the "Identity
  carriers" paragraph still attributes projection-binding
  canonicalisation to `ArgsKey`. Plus `DefPathHash` byte-form
  wording said "in digest order" — but `DefPathHash` byte form
  is `StableCrateId u64 LE + local_hash u64 LE` per §7.3.
- *MINOR 3: Opaque-handle non-generic propagation incomplete.*
  §3.1 invariant 19 + §8.0 say non-generic; §14.5 "Opaque-
  handle convention" still has the unrestricted "true iff every
  reachable use behind Ref/Ptr AND repr(C) ZST" without
  non-genericity. Plus §3.1 used "no generic parameters in
  DefPath segments" as the criterion — Codex correctly noted
  this isn't a reliable carrier (DefPath segments don't
  expose generic-parameter lists).
- *NIT 4: Duplicate schema cleanup partial.* SyntheticItemKind
  dedup landed (§3 → §13.3 reference), but `CrateInfo` is
  still defined twice (§3 + §6) and `FnAbi` has a discriminant-
  pinned definition in §10.1 plus a shorter schema in §14.1.
- *NIT 5: §13.4 synthetic-kind count text stale.* Says "four
  other synthetic kinds" but lists six (ClosureCall,
  ClosureDropGlue, VTableType, VTableInstance,
  VTableMethodShim, VTableDropShim).

### Round 8.2 fix manifest

**M1 fix (packed alignment in §12.5).** §12.5 step 4
"Alignment respected" bullet rewritten to use the §3.3 Phase B
formula: `required_field_align = match layout.pack { None =>
align_of(field.ty), Some(n) => min(align_of(field.ty), n) }`;
`field.offset % required_field_align == 0`. Inline note
explicitly calls out the previously-too-weak rule.

**M2 fix (ProjectionBindingsKey + DefPathHash byte form
sweep).** §13.3 "Identity carriers" paragraph rewritten:
`ArgsKey` for trait_args + self_ty; `ProjectionBindingsKey`
for projection_bindings; rationale (named pairs vs positional
args) explicit. §13.3 ProjectionBindings byte encoding row
for assoc_item DefPathHash bytes updated to "StableCrateId
u64 LE + local_hash u64 LE per §7.3 byte form" (matching the
canonical DefPathHash byte form). §13.4 ClosureCall /
ClosureDropGlue payload row updated identically.

**M3 fix (opaque-handle non-generic propagation +
model-checkable criterion).** §14.5 "Opaque-handle convention"
paragraph rewritten with all five conditions explicit
(`ItemKind::Struct`, `repr(C)`, **non-generic**, ZST monomorphic
layout, every reachable use behind Ref/Ptr) — matching §3.1
invariant 19. New "Model-checkable non-generic criterion"
paragraph: a non-generic Item is one for which `model.types`
contains exactly one `Ty::Struct { item: this, args, layout }`
entry with `args.is_empty()`. Validation reads this directly
from `model.types`, not from rustc-side generic-parameter
lists or DefPath segments. §3.1 invariant 19 updated to the
same model-checkable criterion (replacing the earlier
"DefPath segments" criterion).

**N4 fix (CrateInfo + FnAbi dedup).** §6 CrateInfo body
collapsed to "normative definition lives in §3" reference
(explanatory text retained). §14.1 FnAbi body collapsed to
"normative definition lives in §10.1" reference (the FnAbi
constraint paragraphs about identity preservation retained).

**N5 fix (count correction).** §13.4 "four other synthetic
kinds" → "remaining six synthetic kinds" matching the actual
list count.

### Round 8.2 review

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_round_8_2.txt`.

**Verdict.** `APPROVE` — 0 BLOCK + 0 MINOR + 0 NIT.

All five Round 8.1 findings RESOLVED (M1 packed alignment, M2
ProjectionBindingsKey sweep, M3 opaque-handle non-generic
propagation, N4 CrateInfo + FnAbi dedup, N5 synthetic-kind
count). No new regressions.

### Round 8 closing summary

**Trajectory.**

| Round | Verdict | BLOCK | MINOR | NIT | Total |
| ---   | ---     | ---   | ---   | --- | ---   |
| 8.1   | MINOR   | 0     | 3     | 2   | 5     |
| 8.2   | APPROVE | 0     | 0     | 0   | 0     |

Notably the shortest closing iteration of any round so far
(2 sub-rounds, no BLOCK at any tier). Round 8.1's manifest
landed all 20 items cleanly enough that only sweep cleanups
remained.

**Substantive design refinements that emerged from this loop.**

- *Closure synthetic identity uses closure expression's
  DefPathHash.* The Round 7 §13.4 used the *enclosing function's*
  `DefPathHash`, which collided for two sibling closures
  `let a = || 1; let b = || 2;`. R8.1 manifest item 1 fixed
  the payload to the closure expression's DefPathHash and
  introduced the §3.1 invariant 2 carve-out: `ClosureCaptures`
  Items are source-derived (use rustc closure DefPath verbatim
  with rustc DefPathHash); the discriminator marks shape only.
  Only `ClosureCall` and `ClosureDropGlue` use synthetic
  DefPaths.
- *Synthetic-item identity is discovery-order independent.*
  Round 7 §13.4 included `triggering_crate.stable_crate_id`
  in the synthetic-DefPathHash payload — but the same vtable
  type can be required by multiple crates, and the "first
  crate that triggered synthesis" depended on discovery order.
  R8.1 manifest item 2 stripped triggering_crate from
  identity entirely. Identity is purely (synthetic_kind,
  family-specific payload). Per-family ownership for
  cross-references and display computed deterministically
  (closure: closure-expression's crate; vtable: principal
  trait's crate, fallback `primary_crate`).
- *Vtable shim ABI pinned uniformly as `FnAbi::C`.* R8.1
  manifest item 3 added §13.2 "Method shim and drop shim ABI"
  subsection: all bridge-synthesised vtable shim Function
  Items use `FnAbi::C`, matching the vacant-slot canonical
  signature and OS-integration framing of Regseq traits.
  Trait objects with non-C-ABI-admissible dispatchable methods
  rejected at trait-object translation with a new
  `BridgeError::UnsupportedTraitObjectMethod` diagnostic. Pins
  the v1 contract that `dyn Trait` surfaces are C-shaped.
- *`ProjectionBindingsKey` distinct from `ArgsKey`.* Round 7
  used `ArgsKey` for projection_bindings, but projection
  bindings are *named* `(assoc_item, value)` pairs, not
  positional args. R8.1 manifest item 4 introduced
  `ProjectionBindingsKey(pub u128)` as a distinct type with
  its own normative byte encoding (sorted by `assoc_item
  DefPathHash`, includes `binding_kind` discriminator). Two
  `dyn Trait<AssocA = u32>` and `dyn Trait<AssocB = u32>` now
  produce distinct `ProjectionBindingsKey`s; `dyn Two<A = u32,
  B = u64>` and `dyn Two<B = u64, A = u32>` produce identical
  ones (sort by assoc_item path).
- *Opaque-handle eligibility restricted to non-generic Items
  in v1 with model-checkable criterion.* Round 7 stored
  `Item.opaque_handle_eligible: bool` and referred to "Item's
  layout" — but Items have no layout (only Tys/Instances do).
  For generic structs (`Handle<T>`), opaque-handle eligibility
  is per-monomorphisation. R8.1 manifest item 5 restricted
  eligibility to non-generic Items in v1 (rejects generic
  candidates with `BridgeError::GenericOpaqueHandle`); R8.2
  refined the non-generic criterion to **model-checkable**:
  exactly one `Ty::Struct { item: this, args, layout }` entry
  in `model.types` with `args.is_empty()`. No need to inspect
  rustc-side generic-parameter lists.
- *Concrete `pub enum Ty` and primitive scalar enum schemas
  pinned with #[repr(u8)] discriminants.* R8.1 manifest item 6
  added §10.0 with the full `pub enum Ty` Rust block and
  pinned `IntKind`/`UintKind`/`FloatKind`/`Safety` with
  append-only `#[repr(u8)]` discriminants. Plus
  `INT_KIND_COUNT`/`UINT_KIND_COUNT`/`FLOAT_KIND_COUNT`
  constants. §10.1 added concrete Rust blocks for
  `Provenance`/`Mutability`/`AutoTrait`/`FnAbi` with
  discriminants. Resolves the implementability gap where these
  enums were referenced throughout but never given concrete
  shapes.
- *`UNIT_LAYOUT` constant unifies the canonical erased-unit
  type.* Round 7 §13.2 wrote `Ty::Tuple { fields: [], layout:
  SizeAlign { ... } }` — schema mismatch (`Ty::Tuple.layout`
  is `Layout`, not `SizeAlign`). R8.1 manifest item 7 added
  `pub const UNIT_LAYOUT: Layout` constant in §11 and updated
  §13.2 to use it. New §3.3 Phase B validation: empty tuple
  carries exactly UNIT_LAYOUT.
- *Validation enforcement substantially extended.* R8.1
  manifest item 8 + R8.2 M1 expanded §3.3 Phase B with 10
  invariant-enforcement rules: unsized marker placement,
  Ty::Dynamic well-formedness, SyntheticItemKind ↔ ItemKind
  consistency, item_def_paths uniqueness, opaque-handle
  consistency, FnSig invariants, CAbiLowering shape, layout
  invariants (non-overlap + packed alignment formula + tag
  scalar width-fit), F4-2 sorted instance order, unit layout.
  Phase C extended with mandatory duplicate-key rejection
  during vtable rebuild.
- *`bridge_options_hash` canonical algorithm pinned.* R8.1
  manifest item 18 specified SHA-256 truncated to leading
  16 bytes (big-endian) over a sorted UTF-8 `name=value` byte
  encoding, with the option set enumerated (mir_opt_level,
  cleanup_pass_whitelist, ub_check_force_offs). Same hash
  family as `ArgsKey` for portability.
- *Static mutability and foreign statics representable.* R8.1
  manifest item 14 changed `InstanceKind::Static { ty, init }`
  to `Static { ty, init: Option<ConstId>, mutability:
  Mutability }`. `None` for foreign statics; `mutability`
  affects emitted C `const` storage.
- *Type-identity preservation contract with explicit collapse
  exception.* R8.1 manifest item 17 added §17.5 making the
  D15 type-identity contract explicit — Rust-distinct types
  remain distinct in the IR, with one **single exception**
  (the §17.1-admitted region-only collapse for fn-pointer
  types). The exception explicitly does NOT apply to nominal,
  ABI, safety, const-generic, or non-region structural
  distinctions.

**Status.** Round 8 LOCKED. Top-level `spec.md` reflects the
fourth-external-review fix loop in full. `stage-3/discussions.md`
is unchanged this round (no historical-text refinements
needed). Stage-3 D-tag namespace remains unextended. The Round
8 trajectory was the cleanest of any round (no BLOCK; only
2 sub-rounds to APPROVE) — Round 7's structural foundations
were strong enough that the fourth review found mostly
schema-precision gaps rather than design errors.

Spec line count: 3776 (post-Round-7) → 4282 (post-Round-8.1)
→ ~4290 (post-Round-8.2). Net +514 lines across Round 8 fix
loop; ~10 net new schema definitions / invariants / sections
(notable: §10.0 Ty + primitive enum schemas, §10.1 auxiliary
enum discriminants, §11 UNIT_LAYOUT, §13.3
ProjectionBindingsKey, §13.4 closure-expression DefPathHash
identity + per-family ownership, §14.5 PhantomData ZST rule
+ opaque-handle non-generic restriction, §17.5 type-identity
preservation contract, §3.1 invariants 19-20, §3.3 Phase B
extended validation).

---

## Round 9 -- fifth-external-review fix loop (Regseq-purpose-weighted)

**Date.** 2026-05-02.

**Trigger.** Fifth external review document at
`/Users/nyengele/Downloads/regseq-spec4-deep-review.md`,
delivered after Round 8 closed at APPROVE. The reviewer
acknowledges Round 8's improvements and identifies 18
P0/P1/P2 findings (5 freeze blockers, 8 major, 4 medium,
6 minor sub-items).

**Adjudication: Regseq-purpose-weighted.** Unlike prior
rounds (which accepted reviewer findings broadly), this loop
explicitly weighed each finding against Regseq's actual
purpose (no-std OS-agnostic device-driver DSL targeting
Linux + Zephyr, with `extern "C"` boundaries to kernel APIs
that use simple types) rather than against general-purpose
Rust-to-C compiler rigor. Five findings demoted or declined
on Regseq-purpose grounds; the rest landed.

**Excluded from this loop (with rationale):**

- *F1 reviewer-proposed `adt_c_abi_lowerings` side table.*
  Demoted to MEDIUM (truthfulness). The reviewer's HFA/HFV
  ABI-classification edge cases require float-array
  aggregates and similar — patterns that don't appear in
  driver kernel APIs. Linux/Zephyr SystemV/AAPCS classify
  layout-equivalent C aggregates of scalars/pointers
  identically. Adopted instead: document the v1 ABI assumption
  in §14.5 (no schema added). Methodology-aligned (no
  defensive fallbacks for hypotheticals).
- *F2 reviewer-proposed `AdtReprFacts` schema (Option A).*
  Demoted to MEDIUM (truthfulness). The reviewer frames
  validation as a tampering/security concern, but Regseq's
  pipeline has no adversarial-input trust boundary (bridge
  produces, codegen consumes, JSON is secondary per Q5).
  Adopted instead: weakened §3.3 wording (Option B); added
  "Validation authority and trust model" subsection clarifying
  that bridge certificates are trusted post-deserialise.
- *F4c reviewer-proposed `TagScalar` split into
  `logical_ty`/`storage_ty`.* Adopted lighter alternative:
  codegen rule in §11.2 stating that for Bool/Char tag scalars
  with invalid bit patterns (Option<bool>-style niches),
  storage emits as unsigned integer of scalar width. No
  schema split — same logical Ty in IR, storage emission rule
  at codegen.
- *F3 reviewer-proposed `FunctionInstanceOrigin` enum.*
  Adopted lighter alternative: pin sentinel —
  bridge-synthesised Function Items use `flavor =
  FunctionInstanceFlavor::Item` by convention; identity
  disambiguation already provided by unique synthetic
  ItemIds (§13.4). Documented in §9.4. No new enum.
- *F13b generic typed handles forward planning.* Declined.
  v1 non-generic restriction is methodology-aligned (don't
  pre-design for hypothetical needs); lift to per-
  monomorphisation later if a real driver use case surfaces.

### Round 9.1 fix manifest

Edits landed in `spec.md`. Each entry: source finding -> spec
section(s) touched -> one-line summary.

**Tier 1 (real correctness for driver code):**

1. *F5 Recursion-cycle handling.* §14.5 — added "Recursion-
   cycle handling (memoised checking)" subsection. Bridge
   maintains `visiting_position`/`visiting_field`/`visiting_sig`
   sets; FnPtr edges break cycles via provisional acceptance;
   by-value field cycles rejected as model corruption
   (`BridgeError::CyclicAggregateField`). Critical for driver
   callback structs (`struct file_operations`, etc.) which
   contain function pointers referencing the same struct.
2. *F4a Single-variant `repr(int)` enum scalar.* §11.1 —
   `EnumShape::Single { variant, scalar: Option<TagScalar> }`
   extension. `scalar: Some(_)` for fieldless `repr(int)`
   enums (e.g., `#[repr(u8)] enum Only { A = 5 }` allocates
   u8 storage). `tag_value` semantics table updated.
3. *F4b Normative niche tag-value formula.* §11.2 — pinned
   formula `tag_value(v) = (niche_start + (v.idx -
   niche_variants.start.idx)) mod 2^scalar_width` with
   explicit width-fit and untagged-variant rule.
4. *F4c Codegen storage rule for invalid Bool/Char niche
   patterns.* §11.2 — codegen emits storage lvalue as
   unsigned integer of scalar width (`uint8_t` for Bool,
   `uint32_t` for Char) at niche read/write sites; logical
   Ty stays as Bool/Char in IR. No TagScalar split.
5. *F3 Synthetic FunctionInstanceFlavor sentinel.* §9.4 —
   bridge-synthesised Function Items use `flavor =
   FunctionInstanceFlavor::Item` by convention; identity
   disambiguation via unique synthetic ItemIds. Lighter than
   the reviewer's FunctionInstanceOrigin enum.
6. *F9 Never at C boundaries.* §14.5 — `Ref(Never, _)` and
   `Ptr(Never, _)` rejected at FnAbi::C parameter/return;
   `FnPtr(sig)` with `sig.output == Never` rejected at C
   boundary in v1 (top-level `extern "C" fn() -> !` already
   admitted via §14.5 Never-output rule). Two new §15.1 rows.
7. *F6 vtable_entry_index + method_def_path on
   VTableMethodShim.* §13.3 — `VTableMethodShim { ...,
   vtable_entry_index: u32, method_def_path: DefPathHash }`
   replaces `method_rank: u32`. Vtable_entry_index matches
   `tcx.vtable_entries` sequence (includes drop/size/align/
   supertrait/vacant slots, not just dispatchable methods).
   §13.2 method-shim description updated; Safety pinned to
   mirror source method. §13.4 synthetic payload + DefPath
   segments updated.

**Tier 2 (API safety / consistency):**

8. *F8 Private TyIdentityKey for interner; public Ty derived
   Eq/Hash.* §12 — interner uses private `TyIdentityKey`
   (internal to regseq-model, not in spec public surface);
   public `Ty` has derived `PartialEq`/`Eq`/`Hash` (full-value
   equality including layout). Self-referential ADT
   reservation still works via pre-layout TyIdentityKey
   construction. Round 7 guardrails section replaced with
   "Identity model" rationale. Consistent with R2.9 (no
   *spec-level* TyIdentityKey type; private internal one is
   implementation detail).
9. *F7 Self-contained closure synthetic metadata.* §13.3 —
   `ClosureCaptures { closure: DefPathHash }`,
   `ClosureCall { closure: DefPathHash, call_kind }`,
   `ClosureDropGlue { closure: DefPathHash }`. Closure
   identity carried inline matching the vtable family pattern.
   §13.4 payload reads from the structured tag, not from
   DefPath segments hex.
10. *F13a Formal opaque-handle reachability traversal.* §14.5
    — added "Reachable-use traversal (formal definition)"
    subsection with explicit traversal-domain table:
    `FnSig.inputs/output`, `FieldLayout.ty`, Array elem,
    Tuple field, GenericArg::Type, nested FnPtr (recurse),
    plus stage-4 / stage-9-11 forward expectations for
    consts and body locals. Two bridges satisfying the
    quad produce identical eligibility outcomes.

**Tier 3 (truthfulness / wording):**

11. *F1 v1 C ABI assumption documented.* §14.5 — appended
    "v1 nested-field C ABI assumption" paragraph: layout-
    equivalent C aggregate emission relies on C compiler
    classifying identically; holds for v1 target set
    (Linux + Zephyr SystemV/AAPCS) with the type set bridge
    admits at field positions; methodology rationale (no
    defensive fallbacks for non-driver ABI cases).
12. *F2 Validation authority weakened.* §3.3 — added
    "Validation authority and trust model" subsection
    clarifying validation enforces internal well-formedness
    + bridge-certificate shape; does not re-prove rustc-side
    facts; bridge-produced models are trusted; external
    adversarial-input deployments must add their own
    verification.
13. *F10 §8.0 attribute claim scoped to stage 3.* §8.0 —
    rewritten to scope the "no general attribute table" claim
    to stage 3; explicit forward expectation that stage 14
    (domain-specific extensions) introduces property metadata
    side table keyed by `ItemId`/`InstanceId`/`FieldPath`/`Span`.
14. *F11 Fence/barrier forward expectation.* §15.1
    `core::sync::atomic::{fence, compiler_fence}` row updated
    with explicit stage-14 forward expectation for
    `#[rseq::intrinsic("compiler_fence" | "device_fence" | ...)]`
    intrinsic vocabulary.
15. *F12 Regseq facade trait C-shape design rule.* §13.2 —
    added "Regseq facade trait-surface design rule" paragraph
    explicit about the v1 constraint: facade traits intended
    for `dyn Trait` use must use C-ABI-admissible erased
    method surfaces (fieldless `repr(int)` status, primitive
    types, `repr(C)` aggregates). Default `Result<T, E>` /
    `Option<T>` rejected at trait-object boundary; static
    dispatch unrestricted. Constraint applies only at the
    trait-object boundary.
16. *F14 Collision honesty for ProjectionBindingsKey +
    synthetic DefPathHash.* §13.3 ProjectionBindingsKey
    section + §13.4 synthetic-DefPathHash section: added
    parallel collision-honesty paragraphs matching the
    `ArgsKey` (§12.3) framing. Synthetic DefPathHash is
    64-bit; cross-model collision outside v1 threat model;
    cross-build identity comparisons should use the
    structured `SyntheticItemKind` payload (collision-proof).
17. *F15 Stage-4 const deferral status note.* Top-level
    "Snapshot scope" paragraph added after the "Currently
    reflects" line. States full stage-1 + stage-3
    specification; value-level constants (vtable instances,
    string literals, statics with relocations, etc.) await
    stage 4.
18. *F17 Pretty-printer §18.2 truthfulness.* Reworded to
    state full-model rendering is not yet available;
    snapshot tests for the complete stage-3 model are
    blocked on filling remaining renderers; "primary
    serialisation" remains accurate as contract direction,
    not current capability; secondary JSON used for tests
    where readability is acceptable until stage 13 fills
    the renderers.

**Tier 4 (minor):**

19. *F18a Provenance vs BorrowKind typo.* §12.3 ArgsKey
    encoding: `u8 BorrowKind` → `u8 Provenance discriminant
    (§10.1)`.
20. *F18b ProjectionBindingsKey sort order pinned.* §13.3
    encoding: "sorted ascending by assoc_item DefPathHash"
    → "sorted lexicographically (ascending) over the 16
    canonical bytes (StableCrateId u64 LE + local_hash u64
    LE per §7.3 byte form)."
21. *F18c DefPath.rendered validation.* §3.3 Phase B: new
    invariant — re-derive canonical rendering from `crate_id`
    + `segments` and check equality with stored `rendered`.
22. *F18d Vtable type field naming convention.* §13.2
    synthetic vtable layout algorithm extended with step 9:
    pinned per-slot field naming (`__drop_in_place`,
    `__size`, `__align`, source method names with `#N`
    disambiguator on collision, `__super_TraitName`,
    `__vacant_N`).
23. *F18e VTableInstance Mutability::Not + init.is_some()
    validation.* §3.3 Phase B: new invariant for
    VTableInstance Items.
24. *F18f Source span content: None validation.* §3.3 Phase
    B: explicit rule for content-None source files —
    line_starts monotonicity checked, lo/hi not bounded by
    content length, ranges accepted but resolve to None via
    Span::slice.

### Round 9.1 review

**Codex run.** `--effort high`, `--timeout 1800`, output captured
to `/tmp/codex_round_9_1.txt`.

**Verdict.** `MINOR` — 0 BLOCK + 2 MINOR + 1 NIT (3 issues total).

All 24 manifest items landed: 22 cleanly, 2 with stale-text
contradictions from incomplete sweeps. **All five Regseq-
purpose-weighted demotions accepted by Codex** with explicit
`regseq-background.md` citations grounding each rationale —
F1 (no-std driver targeting Linux/Zephyr; layout-equivalent
emission OK for kernel APIs), F2 (no adversarial JSON
boundary; bridge-produced models consumed by compiler stages),
F4c (codegen rule sufficient for v1 driver scope), F3 (sentinel
sufficient — synthetic ItemIds already disambiguate), F13b
(typed handles not in v1 background requirements). No new
design issues introduced; only sweep-coverage gaps from the
Round 9 schema changes.

**Findings.**

- *MINOR 1: §12.1 + §12.3 stale `Hash + Eq over Ty` wording
  contradicts F8 private TyIdentityKey model.* The Round 9
  refinement moved the lookup key to a private
  `TyIdentityKey`, but §12.1 identity-rules table still
  labels columns as `Hash + Eq` rules; §12.3 "Distinct from
  BiMap Hash + Eq" subsection still describes lookup as
  shallow `Hash + Eq` over `Ty`. Both should describe the
  private `TyIdentityKey` shape instead.
- *MINOR 2: §3.3 Phase B opaque-handle validation contradicts
  F2 trust model.* New "Validation authority" subsection says
  validation does not re-prove rustc-only facts; Phase B
  still says it validates all five opaque-handle conditions
  including source-side `repr(C)` (a rustc-only fact not
  stored in the model per §11.3). Inconsistent.
- *NIT 3: §11.1 tag-value table niche cross-reference points
  to nonexistent §11.1.x.* The niche formula lives in §11.2.
  One-line fix.

### Round 9.2 fix manifest

**M1 fix (TyInterner identity-rules text aligned with private
TyIdentityKey).** §12 `find_nominal` paragraph rewritten:
construction goes through private `TyIdentityKey` from
`(kind, item, args)`; callers don't need a concrete `Ty` /
`Layout`. §12.1 retitled "Identity rules (private
`TyIdentityKey` shape)"; preamble explicit that the private
key is the lookup key and public Ty has derived full-value
Eq/Hash; identity table column header renamed
`TyIdentityKey shape` (not `Hash + Eq` rule). §12.3 "Distinct
from BiMap Hash + Eq" subsection retitled "Distinct from
TyInterner identity (per-build) lookup"; body rewritten to
describe `TyIdentityKey` as the per-build shallow key,
`ArgsKey` as the cross-build deep key.

**M2 fix (split bridge-certified facts from model-checkable
ones in §3.3 Phase B opaque-handle validation).** §3.3 Phase
B "Opaque-handle consistency" entry rewritten: validation
enforces only the **model-checkable subset** of invariant 19
(struct kind, non-generic via `Ty::Struct` count, ZST
monomorphic layout, formal reachability traversal). The
**source-side `repr(C)` fact** is explicitly marked as a
**bridge certificate** not re-proved by validation —
matches the §3.3 trust-model wording. §3.1 invariant 19
updated correspondingly: `repr(C)` condition annotated
`(bridge certificate)`, other conditions `(model-checkable)`.

**N3 fix (niche cross-reference).** §11.1 tag-value table:
"`Some(v)` per niche-encoding formula (§11.1.x)" → "`Some(v)`
per the niche tag-value formula (§11.2)".

### Round 9.2 review

**Codex run.** `--effort high`, `--timeout 1500`, output captured
to `/tmp/codex_round_9_2.txt`.

**Verdict.** `MINOR` — 0 BLOCK + 1 MINOR + 0 NIT (1 issue total).

M1 RESOLVED, N3 RESOLVED, M2 PARTIALLY-RESOLVED. The remaining
issue is internal contradiction from incomplete sweep:

- *MINOR 1: §3.3 Phase B preamble still classifies opaque-
  handle global-use predicate as bridge-certified-only.* The
  preamble's example (`opaque_handle_eligible`) is stale —
  Round 9 made the reachability traversal model-checkable,
  but the preamble still cites the global-use predicate as
  rustc-only. Plus §3.1 invariant 19's ZST-layout and
  reachable-use bullets weren't annotated `(model-checkable)`
  matching the Phase B split.

### Round 9.3 fix manifest

**M1 fix (preamble + invariant 19 sweep).**

§3.3 Phase B preamble rewritten: replaces the stale
`opaque_handle_eligible` example with a precise statement —
the only remaining bridge-certified-only conditions are
narrowly the rustc repr classifications used by §14.5
admissibility predicates and the source-side `repr(C)`
certificate in invariant 19. Where an invariant condition IS
model-computable (formal reachable-use traversal in §14.5,
non-generic criterion via `Ty::Struct` count, ZST monomorphic
layout via inline `Layout`), validation re-runs the condition
mechanically — not bridge-certified-only.

§3.1 invariant 19: ZST monomorphic layout bullet annotated
`(model-checkable from the inline Layout)`; reachable-use
bullet annotated `(model-checkable via §14.5 reachable-use
traversal)` with explicit cross-reference to the §3.3 Phase B
validation split (bridge-certified `repr(C)` vs other four
model-checkable conditions).

### Round 9.3 review

**Codex run.** `--effort high`, `--timeout 1200`, output captured
to `/tmp/codex_round_9_3.txt`.

**Verdict.** `APPROVE` — 0 BLOCK + 0 MINOR + 0 NIT.

The R9.2 finding is RESOLVED: §3.3 Phase B preamble narrowly
limits bridge-certified-only facts to rustc repr classifications
+ source-side `repr(C)` certificate; model-computable
conditions are mechanically re-run. §3.1 invariant 19 matches
the split (source-side `repr(C)` bridge-certified; non-generic,
ZST layout, reachable-use all model-checkable). Final-sweep
across cumulative Round 9 edits returned no remaining
inconsistencies.

### Round 9 closing summary

**Trajectory.**

| Round | Verdict | BLOCK | MINOR | NIT | Total |
| ---   | ---     | ---   | ---   | --- | ---   |
| 9.1   | MINOR   | 0     | 2     | 1   | 3     |
| 9.2   | MINOR   | 0     | 1     | 0   | 1     |
| 9.3   | APPROVE | 0     | 0     | 0   | 0     |

**The Regseq-purpose-weighted approach worked.** All five
demotions/declines (F1 adt_c_abi_lowerings, F2 AdtReprFacts,
F4c TagScalar split, F3 FunctionInstanceOrigin enum, F13b
generic typed handles) were accepted by Codex with explicit
`regseq-background.md` citations grounding each rationale.
None of the demoted findings reappeared as new issues; the
remaining iterations were all sweep-coverage cleanups from
the Round 9.1 schema changes.

**Substantive design refinements that emerged from this loop.**

- *Recursion-cycle handling for C ABI predicates (F5).* Driver
  callback structs (Linux's `struct file_operations`,
  Zephyr's device callback tables) are exactly the recursive
  callback shape — a `repr(C)` struct containing function
  pointers whose signatures reference the same struct.
  Without cycle handling, the bridge would loop on real
  driver fixtures. R9.1 §14.5 added the memoised /
  coinductive checking discipline (visiting sets per
  predicate; FnPtr edges break cycles via provisional
  acceptance; by-value field cycles rejected as model
  corruption).
- *Enum scalar/niche representation completed (F4a/b/c).*
  R9.1 added the `EnumShape::Single { variant, scalar:
  Option<TagScalar> }` extension for fieldless `repr(int)`
  enums (single-variant status codes), pinned the normative
  niche tag-value formula (`Option<&T>` and
  `Option<bool>`-style niches), and added the codegen
  storage-emission rule for invalid Bool/Char bit patterns
  (use unsigned integer of scalar width — no TagScalar split
  needed). All three sub-issues hit driver code: status enums,
  niche-encoded `Option<&T>` callbacks, occasional
  `Option<bool>` niches.
- *Synthetic FunctionInstanceFlavor sentinel (F3).* Round 7
  introduced `FunctionInstanceFlavor` as a structured form
  of the rustc shim discriminator, but bridge-synthesised
  Function Items (vtable shims, closure call methods) have
  no rustc `ty::InstanceKind` variant. Rather than introduce
  a `FunctionInstanceOrigin { Rustc(...), RegseqSynthetic }`
  enum (the reviewer's option), R9.1 pinned the simpler
  sentinel: synthetic functions use
  `flavor = FunctionInstanceFlavor::Item`; their unique
  synthetic ItemIds already disambiguate. Methodology-aligned
  (lighter schema, sufficient for v1).
- *Never at C boundaries explicitly rejected (F9).* `Ref(Never,
  _)` and `Ptr(Never, _)` slipped through Round 8's
  `CAbiPositionAdmissible.Direct` rule for "any sized
  pointee" because Never is sized (size 0, align 1). R9.1
  added explicit rejection rows for these plus `FnPtr(sig)`
  with `sig.output == Never` at C boundaries. Top-level
  `extern "C" fn() -> !` already lowers to noreturn void.
- *Vtable method identity uses `vtable_entry_index` not
  `method_rank` (F6).* Round 8 used `method_rank` (rank in
  dispatchable methods); R9.1 corrected to `vtable_entry_index`
  (position in `tcx.vtable_entries` — includes drop/size/
  align/supertrait/vacant slots). Body-level dispatch
  indexes by slot, so the rank-vs-index distinction is
  load-bearing for correctness. R9.1 also added
  `method_def_path: DefPathHash` for semantic identity that
  survives slot-reordering edge cases, and pinned vtable
  shim Safety to mirror the source method.
- *Private TyIdentityKey for interner; public Ty derived
  Eq/Hash (F8).* Three external reviewers raised the
  enforceability hazard of public `Ty` Eq/Hash being
  identity-only (R5, R6, R7 doc guardrails were
  unenforceable across multi-crate consumers). R9.1 moved
  the lookup key to a private `TyIdentityKey` (internal to
  regseq-model crate, not in spec public surface — consistent
  with R2.9 which declined a *spec-level* TyIdentityKey
  type), making public `Ty` Eq/Hash derived full-value.
  Self-referential ADT reservation still works via
  pre-layout `TyIdentityKey` construction.
- *Closure synthetic metadata self-contained (F7).* Round 7
  put closure identity in DefPath segments hex; R9.1
  promoted it to the `SyntheticItemKind` payload
  (`ClosureCaptures { closure: DefPathHash }`,
  `ClosureCall { closure: DefPathHash, call_kind }`,
  `ClosureDropGlue { closure: DefPathHash }`) matching the
  vtable family pattern. §13.4 synthetic payload reads from
  the structured tag, not from segments hex.
- *Formal opaque-handle reachability traversal (F13a).*
  Round 8's "every reachable use behind Ref/Ptr" was
  informal. R9.1 added the formal traversal-domain table
  in §14.5 (FnSig.inputs/output, FieldLayout.ty, Array
  elem, Tuple field, GenericArg::Type, nested FnPtr) plus
  stage-4/9-11 forward expectations. Two bridges satisfying
  the build-equivalence quad now produce identical
  eligibility outcomes.
- *Validation authority and trust model (F2 truthfulness).*
  R9.1 + R9.2 + R9.3 added explicit "Validation authority
  and trust model" to §3.3 clarifying that bridge
  certificates are trusted post-deserialise; validation
  enforces internal well-formedness + certificate shape;
  does not re-prove rustc-only facts. R9.3 narrowed the
  bridge-certified-only carve-out to rustc repr
  classifications + source-side `repr(C)` certificate; all
  other invariants explicitly model-computable. Honest about
  what validation actually does without adding repr-fact
  storage (the methodology-aligned answer for Regseq's
  no-adversarial-input pipeline).
- *v1 C ABI assumption documented for nested aggregate fields
  (F1 truthfulness).* R9.1 §14.5 pinned the v1 assumption:
  layout-equivalent C aggregate emission relies on the C
  compiler classifying identically; holds for v1 target set
  (Linux + Zephyr SystemV/AAPCS) with the type set the bridge
  admits at field positions. Future targets requiring
  per-field ABI distinctions extend the contract; v1 takes
  the simpler path. Methodology-aligned (no defensive
  fallbacks for non-driver ABI cases).
- *Regseq facade trait C-shape design rule (F12).* R9.1
  §13.2 explicit: `dyn Trait` use requires C-ABI-admissible
  erased method surfaces (fieldless `repr(int)` status,
  primitives, `repr(C)` aggregates); `Result<T, E>` /
  `Option<T>` rejected at trait-object boundary. Static
  dispatch unrestricted. Constraint applies only at the
  trait-object boundary.
- *Property metadata + fence/barrier vocabulary forward
  expectations (F10, F11).* R9.1 scoped §8.0's "no general
  attribute table" claim to stage 3; explicit forward
  expectation that stage 14 introduces property metadata
  side table. R9.1 §15.1 fence/barrier rejection row
  extended with stage-14 forward expectation for
  `#[rseq::intrinsic("compiler_fence" | "device_fence")]`
  vocabulary.

**Status.** Round 9 LOCKED. Top-level `spec.md` reflects the
fifth-external-review fix loop in full, with the
Regseq-purpose-weighted demotions documented and accepted by
the adversarial review. `stage-3/discussions.md` is unchanged
this round. Stage-3 D-tag namespace remains unextended.

**The Round 9 trajectory** — three sub-rounds (MINOR → MINOR →
APPROVE) with no BLOCK at any tier — was the cleanest among
the four most-recent rounds, despite Round 9 being the
largest manifest (24 items, including 5 explicit demotions
adjudicated by Codex). Holding findings to Regseq's actual
purpose rather than general-purpose Rust-to-C compiler rigor
both reduced the surface (5 demotions = ~28% of reviewer
findings) and produced a cleaner closing iteration.

Spec line count: 4310 (post-Round-8) → 4687 (post-Round-9.1)
→ ~4710 (post-Round-9.3). Net +400 lines across Round 9 fix
loop; ~12 net new schema definitions / invariants / sections
(notable: §11 EnumShape::Single scalar option + niche formula
+ Bool/Char storage rule, §12 private TyIdentityKey + public
derived Eq/Hash, §13 vtable_entry_index + method_def_path +
self-contained closure metadata + Regseq facade trait rule,
§14.5 cycle handling + Never-at-C rejection + opaque-handle
reachability traversal + v1 C ABI assumption, §3.3 validation
authority + trust model + extended Phase B invariants).


















---

## Stage 4 (constants and literals)

# Stage 4 -- review log

Iterative Codex adversarial-review rounds for the stage-4 const-
vocabulary spec at `spec-snapshot.md` (primary, §9.7) plus the
audit-shape decision record at `discussions.md` (D-4-1 entries).
Each entry records the round's scope, verdict, issue list, and
the corrections applied before the next round.

Reviews are invoked via `scripts/python/codex_runner.py` per the
project's CLAUDE.md contract. Each round uses a fresh prompt
(Codex has no session continuation); re-reviews include a brief
summary of what was changed since the prior round.

The structured output contract for every round:
- First line: `VERDICT: <BLOCK | MINOR | APPROVE>`
- Numbered issues list, each tagged `[BLOCK | MINOR | NIT]` with
  a decision reference, file:line citation, 1-3-sentence
  description, and a 1-sentence suggested fix.
- Last line: `TOTAL: <N> issues (<count of BLOCK> BLOCK /
  <count of MINOR> MINOR / <count of NIT> NIT)`.

Prompt template skeleton: `memory.md` §4.

---

## Round 1 -- D-4-1 initial review

**Date.** 2026-05-02.

**Scope.** D-4-1 cluster (parent decision + sub-decisions D-4-1.1
through D-4-1.9) covering the 11-variant `Const` enum, the `Byte`
three-state, the `InstanceRef` 4-tuple, the type-level vs value-
level partition, the bridge construction dispatch, and the
const-validation phase split. Spec under review:
`stage-4/spec-snapshot.md` §9.7. Audit shape under review:
`stage-4/discussions.md` D-4-1 entries.

Out of scope: the per-variant `ConstIdentityKey` byte-level
serialisation order (D-4-2 territory); `pretty.rs::render_const`
renderer style (post-D-4-1); stage-1/3 sections of
`spec-snapshot.md` outside §9.7; the existing top-level
`spec.md` §9.7 stub (which stage 4 supersedes via the merger).

**Codex run.** `--effort high`, `--timeout 1500`, output to
`/tmp/codex_d4_1_review_r1.txt`.

**Verdict.** `BLOCK` -- 7 BLOCK + 3 MINOR + 0 NIT (10 total
issues).

**Issues.**

1. *[BLOCK] D-4-1.5 / §9.7 (`spec-snapshot.md`).* `GlobalAlloc::Function`
   projection dropped the rustc-shim-flavor disambiguator
   (`shim_disambig: None`); §9.3's `RegseqInstanceKey` requires
   `fn_shim_disambig` to disambiguate shims sharing
   `(DefId, args)` (e.g., `Item(foo)` vs `ReifyShim(foo)`).
   D-4-1.5's claim that `kind_rank` separates `Function` from
   `DropGlue` was wrong — `DropGlue` is a
   `FunctionInstanceFlavor`, not a separate Regseq InstanceKind.
2. *[BLOCK] D-4-1.5 / §9.7.* `GlobalAlloc::VTable` projection
   used `kind_rank: VTableInstance-rank`, but §9.3's pinned
   table has only six ranks (Function/Static/Const/Struct/Enum/
   Union); vtable instances are `Static` Items synthesised per
   §13.2.
3. *[BLOCK] §9.7 bridge dispatch.* The slice and anonymous-
   memory paths synthesised data Static Items, but §13.3's
   `SyntheticItemKind` universe lists six families and none for
   memory/string backings.
4. *[BLOCK] §9.7 Byte invariants.* "For every byte at offset
   `o` with `Byte::Reloc(r)`: bytes at `[o..o+ptr_size]` ..."
   reads literally as requiring the second byte of any pointer
   slot to start its own aligned run.
5. *[BLOCK] §9.7 validation split.* Wording "aggregate children
   are not type-level" contradicts the partition table that
   admits scalars at value-level; the slice dispatch produces
   `Aggregate([Reloc(...), Uint(Usize, ...)])` with a type-
   level scalar child.
6. *[BLOCK] §9.7 Slice / NullPtr dispatch.* `NullPtr(TyId)`
   description claimed to cover the empty-string fallback `&str`
   data pointer, but the Slice dispatch always emits an
   `Aggregate`, never a `NullPtr` at the data field.
7. *[BLOCK] §9.7 validation order.* §9.7 appended a new value-
   level const validation step after Phase B step 8, but §3.3
   Phase B / Phase C numbering and the dependency-arrow summary
   were not updated.
8. *[MINOR] D-4-1.9 (`discussions.md`).* Bridge dispatch's
   `ConstValue::Scalar(Scalar::Int(_))` arm listed `ZST` as a
   possible target; the spec dispatch table correctly gives
   `ZeroSized → ZST` as a separate arm.
9. *[MINOR] D-4-1.3 (`discussions.md`).* Wording said the
   aggregate "key" is `Vec<ConstId>`, conflating storage shape
   with cross-build canonical key (which recursively uses each
   child's canonical content per §12.3).
10. *[MINOR] D-4-1.5 / §9.7.* `shim_disambig: Option<ShimDisambig>`
    pinned partly as a v2 "structural hedge" even though §13.4
    already encodes vtable method/drop distinctions in
    synthesised `DefPathHash` — violates README §5.9 "no
    forward-looking placeholders."

Full Codex output: `/tmp/codex_d4_1_review_r1.txt`.

**Corrections applied to `spec-snapshot.md` / `discussions.md`.**

- *Issues 1, 2, 10.* Replaced `Option<ShimDisambig>` enum with
  `fn_shim_disambig: u8` (matching §9.3's existing field). The
  4-tuple is now `(regseq_kind_rank, def_path_hash, args_key,
  fn_shim_disambig)` — same shape as `RegseqInstanceKey` minus
  the local `ItemId`. `Function` projection populates
  `fn_shim_disambig` from `rustc_instance_kind_sort_rank(flavor)`;
  `VTable` projection uses `regseq_kind_rank: 1` (Static),
  marked in `item_synthetic_kinds` as `VTableInstance` per §13.2.
  `discussions.md` D-4-1.5 prose updated to match: dropped the
  v2-hedge rationale; added "why no separate ShimDisambig enum"
  explicitly grounded in §13.4 disambiguation.
- *Issue 3.* Dropped the slice / anonymous-memory dispatch
  arms from §9.7. Added "Stage-4 deferred: reference-typed
  const handling" subsection; filed deferral in
  `stage-4/status.md` "Stage-4-specific deferred items"; filed
  outbound forward expectation in
  `stage-12/expectations.md` for the
  `SyntheticItemKind::ConstMemoryStatic` family extension.
- *Issue 4.* Rephrased Byte well-formedness invariant in terms
  of *maximal relocation runs* (length-`ptr_size` runs of
  same-`InstanceRef` bytes starting on `ptr_align`-aligned
  offsets), replacing the per-byte rule.
- *Issue 5.* Refactored value-level validation prose: type-
  level scalar children ARE admissible inside value-level
  aggregates (the type-level / value-level boundary is variant-
  intrinsic and non-transitive — a value-level aggregate may
  carry type-level scalar children without becoming type-level
  itself).
- *Issue 6.* Dropped the empty-string-fallback claim from
  `NullPtr` description; updated D-4-1.6 reasoning to use
  in-scope examples (vacant vtable slots vs source-level
  `core::ptr::null::<T>()`).
- *Issue 7.* Edited §3.3 Phase B to insert the value-level
  const validation step as new step 9 (between step 8 and
  Phase C); renumbered Phase C step 9 → step 10; updated the
  dependency-arrow summary to `consts (type-level) ← sigs ←
  types ← (layout, instances) ← consts (value-level)`. Updated
  step 4 wording to acknowledge the type-level phase split.
  §9.7's "Const validation phase split" subsection now
  references the concrete §3.3 step numbers (step 4 / step 9).
- *Issue 8.* Removed `ZST` from D-4-1.9's `Scalar::Int` arm in
  `discussions.md`.
- *Issue 9.* Refactored D-4-1.3 reasoning to clarify
  `Vec<ConstId>` is the storage shape; the cross-build canonical
  identity key recursively uses each child's canonical content
  per §12.3.

---

## Round 2 -- D-4-1 re-review after round-1 fixes

**Date.** 2026-05-02.

**Scope.** Same as round 1 (D-4-1 cluster: parent + sub-decisions
D-4-1.1 through D-4-1.9; spec under review §9.7 +
discussions.md). Round-1 corrections include: `InstanceRef`
shape realignment with §9.3; vtable rank fix; slice/memory
dispatch deferral; Byte invariant rephrasing; aggregate-child
type-level admissibility clarification; §3.3 step 9 insertion;
trivial fixes in `discussions.md`.

**Codex run.** `--effort high`, `--timeout 1500`, output to
`/tmp/codex_d4_1_review_r2.txt`.

**Verdict.** `BLOCK` -- 2 BLOCK + 3 MINOR + 0 NIT (5 total
issues). All round-1 issues closed; round-2 findings are
residual / corrections-of-corrections.

**Issues.**

1. *[BLOCK] §9.7 / D-4-1.5.* `Function` projection set
   `fn_shim_disambig: rustc_instance_kind_sort_rank(instance.def)`,
   but §9.3 / §9.4 say the public discriminator is
   `InstanceKind::Function.flavor` (`FunctionInstanceFlavor`)
   and the rank table maps that flavor to the `u8`. Wording
   conflated rustc-side `instance.def` with the Regseq-side
   flavor wrapper.
2. *[BLOCK] §9.7 / §12.3.* `ArgsKey::EMPTY` referenced by
   `Static` and `VTable` projections, but §12.3 defines only
   `pub struct ArgsKey(pub u128)` — no `EMPTY` constant. The
   projection depended on an undefined sentinel.
3. *[MINOR] §9.7 deferral consistency.* `Bytes` variant
   doc-comment listed `&str` / `&[u8]` backing as a use case
   while D-4-1.9 explicitly defers `&str` / `&[T]` reference-
   typed const handling.
4. *[MINOR] D-4-1 parent methodology (`discussions.md`).*
   Parent-decision compositional-behavior bullet still claimed
   "`&str` backing falls out of `Aggregate` over a `Reloc` and
   a `Uint`" without acknowledging the `ConstMemoryStatic`
   deferral.
5. *[MINOR] `status.md` stale D-4-1.5 shape.* "Open decisions"
   catalogue still listed D-4-1.5 as
   `(kind_rank, def_path_hash, args_key, shim_disambig:
   Option<ShimDisambig>)` — pre-correction shape. Could mislead
   the next pass.

Full Codex output: `/tmp/codex_d4_1_review_r2.txt`.

**Corrections applied.**

- *Issue 1.* Rewrote the `Function` GlobalAlloc projection row
  to extract `def_path_hash` from `instance.def_id()` and
  `fn_shim_disambig` from `rustc_instance_kind_sort_rank(flavor)`
  where `flavor: FunctionInstanceFlavor` is the §9.4 wrapper.
  Added explicit examples (`Item(foo)` rank 0, `ReifyShim(foo)`
  rank 3, `DropGlue(foo)` rank 10).
- *Issue 2.* Added `ArgsKey::EMPTY` constant definition to
  §12.3 ("ArgsKey value type" subsection) — the canonical
  `ArgsKey` for an empty `GenericArgs`, computed as the
  leading 16 bytes of `SHA-256(0x00 0x00 0x00 0x00)` (the
  4-byte LE u32 length prefix per §12.3's canonical encoding)
  loaded as a big-endian u128. Referenced from `Static` /
  `VTable` projections in §9.7 unchanged.
- *Issue 3.* Updated `Bytes` variant doc-comment in §9.7 to
  drop `&str` / `&[u8]` from listed use cases and added a
  pointer to the deferral subsection.
- *Issue 4.* Reworded D-4-1 parent decision's compositional-
  behavior bullet: explicitly noted that `&str` / `&[T]`
  reference-typed const handling is stage-4-deferred until
  `ConstMemoryStatic` lands; the `Aggregate([Reloc, Uint])`
  shape is the *expected post-deferral* compositional form,
  not an in-scope D-4-1 path.
- *Issue 5.* Updated `status.md` "Open decisions" D-4-1.5
  entry to the corrected 4-tuple
  `(regseq_kind_rank, def_path_hash, args_key,
  fn_shim_disambig)`.

---

## Round 3 -- D-4-1 re-review after round-2 fixes

**Date.** 2026-05-02.

**Scope.** Same as round 2. Round-2 corrections include:
`Function` projection rephrased via `FunctionInstanceFlavor`;
`ArgsKey::EMPTY` defined in §12.3; `Bytes` doc-comment scope
narrowed; D-4-1 parent decision &str example reframed as
post-deferral; `status.md` D-4-1.5 catalogue entry refreshed.

**Codex run.** `--effort high`, `--timeout 1500`, output to
`/tmp/codex_d4_1_review_r3.txt`.

**Verdict.** `BLOCK` -- 1 BLOCK + 1 MINOR + 0 NIT (2 total
issues). All round-2 issues closed; round-3 findings are
deeper-residual.

**Issues.**

1. *[BLOCK] §12.3 / `ArgsKey::EMPTY`.* The new `ArgsKey::EMPTY`
   definition relies on a "GenericArgs encodes as 4-byte
   length-zero" claim, but §12.3 never defines the
   `GenericArgs` container grammar (length prefix + N entries
   with `GenericArg::{Type, Const}` per-entry tags). The
   per-`Ty` encodings call `canonical_args(GenericArgs)`
   without the container grammar pinned. This leaves
   non-empty `argskey_of(instance.args)` under-specified.
2. *[MINOR] §9.3 / §9.7 function shim rank.* `rustc_instance_kind_sort_rank`
   in §9.3 is typed as taking `&rustc_ty::InstanceKind<'_>`,
   but the §9.7 corrected projection passes
   `flavor: FunctionInstanceFlavor`. Cosmetic typed-function-
   call mismatch.

Full Codex output: `/tmp/codex_d4_1_review_r3.txt`.

**Corrections applied.**

- *Issue 1.* Added `GenericArgs container encoding` subsection
  to §12.3 between the per-`Ty` tag table and the cross-build
  determinism subsection. Pins the grammar:
  `u32 LE arg_count` followed by N `GenericArg` entries; per
  entry `u8 disc` (0=Type, 1=Const) + canonical payload bytes.
  Empty case (N=0) yields exactly the 4 bytes
  `0x00 0x00 0x00 0x00`. `ArgsKey::EMPTY`'s
  prior-paragraph reference to "encoding rules below" now
  resolves to a concrete spec.
- *Issue 2.* Rephrased the §9.7 `Function` projection's
  `fn_shim_disambig` clause from a typed function call
  (`rustc_instance_kind_sort_rank(flavor)`) to a table-lookup
  reference: "the `u8` rank looked up in §9.3's
  `rustc_instance_kind_sort_rank` table for the
  `FunctionInstanceFlavor` corresponding to `instance.def`."
  Avoids the type-signature mismatch without redefining the
  §9.3 helper.

---

## Round 4 -- D-4-1 re-review after round-3 fixes

**Date.** 2026-05-02.

**Scope.** Same as round 3. Round-3 corrections include: `GenericArgs`
container grammar pinned in §12.3; `ArgsKey::EMPTY`'s
encoding-byte composition now resolved; §9.7 `Function`
projection rephrased to table-lookup form.

**Codex run.** `--effort high`, `--timeout 1500`, output to
`/tmp/codex_d4_1_review_r4.txt`.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
All BLOCKs cleared; sole remaining issue is a cross-reference
correctness fix.

**Issues.**

1. *[MINOR] D-4-1 / §12.3 (`spec-snapshot.md`).* The new
   "GenericArgs container encoding" subsection's introductory
   parenthetical claimed `canonical_args(GenericArgs)` appears
   in the `Tuple` per-field rule, but the per-`Ty` table
   entry for `Tuple` actually uses `canonical_args(TyId)`
   (per-field, not the `GenericArgs` container). Cross-
   reference error in the parenthetical, not a grammar
   failure.

Full Codex output: `/tmp/codex_d4_1_review_r4.txt`.

**Corrections applied.**

- *Issue 1.* Removed "and the `Tuple` per-field rule" from
  the introductory parenthetical of the new `GenericArgs
  container encoding` subsection. The remaining list (`FnDef`,
  `Closure`, `Struct`, `Enum`, `Union`) matches the per-`Ty`
  table entries that actually invoke `canonical_args(GenericArgs)`.

---

## Round 5 -- D-4-1 final re-review after round-4 fix

**Date.** 2026-05-02.

**Scope.** Same as round 4. Round-4 correction is the
single-line removal of "and the `Tuple` per-field rule" from
§12.3's `GenericArgs container encoding` subsection
introductory parenthetical.

**Codex run.** `--effort high`, `--timeout 1500`, output to
`/tmp/codex_d4_1_review_r5.txt`.

**Verdict.** `MINOR` -- 0 BLOCK + 1 MINOR + 0 NIT (1 total).
Round-4 fix verified correct; one **new** MINOR found
(longstanding inconsistency that prior rounds did not flag).

**Issues.**

1. *[MINOR] D-4-1.1 / §9.7 (`spec-snapshot.md`).* Float-padding
   wording inconsistency between `discussions.md` and
   `spec-snapshot.md`. `discussions.md` D-4-1.1 says smaller
   floats "zero-pad into the high bits" with worked example
   `Float(F32, 0x3f800000)` (which has the upper 96 bits of
   the `u128` zero). §9.7's `Float` doc-comment said
   "zero-pad into the low-order bits" and the bridge dispatch
   table said "low-order zero-pad" — both read as "low-order
   bits are zero," contradicting the example.

Full Codex output: `/tmp/codex_d4_1_review_r5.txt`.

**Corrections applied.**

- *Issue 1.* Rewrote both §9.7 references to use unambiguous
  phrasing: "the bit-pattern occupies the low-order bits of
  `u128`; the high-order bits are zero for narrow widths,"
  with the same `1.0f32 → 0x3f800000` example as
  `discussions.md`. Both files now agree on the bit-pattern
  layout.

---

## Round 6 -- D-4-1 final close-out re-review after round-5 fix

**Date.** 2026-05-02.

**Scope.** Same as round 5. Round-5 correction is the float-
padding wording fix in §9.7's `Float` doc-comment and bridge
dispatch table.

**Codex run.** `--effort high`, `--timeout 1500`, output to
`/tmp/codex_d4_1_review_r6.txt`.

**Verdict.** `APPROVE` -- 0 BLOCK + 0 MINOR + 0 NIT (0 total).
D-4-1 cluster closed. Codex confirms cumulative scoped
consistency holds across §3.3 (content-keyed uniqueness), §9.7
(validation phase split), §12.3 (canonical const recursion).
Audit-shape coverage retained for D-4-1 parent and all 9
sub-decisions.

**Issues.** None.

**Corrections applied.** None required.

---

## Round-trajectory summary -- D-4-1

```
R1: 10 issues (7 BLOCK / 3 MINOR / 0 NIT)   [BLOCK]
R2:  5 issues (2 BLOCK / 3 MINOR / 0 NIT)   [BLOCK] -- all R1 closed
R3:  2 issues (1 BLOCK / 1 MINOR / 0 NIT)   [BLOCK] -- all R2 closed
R4:  1 issue  (0 BLOCK / 1 MINOR / 0 NIT)   [MINOR] -- all R3 closed
R5:  1 issue  (0 BLOCK / 1 MINOR / 0 NIT)   [MINOR] -- R4 closed; new pre-existing inconsistency surfaced
R6:  0 issues                                [APPROVE] -- R5 closed
```

20 distinct findings closed across the iteration. D-4-1 cluster
LOCKED in `discussions.md` and `status.md`. The cluster is the
first stage-4 spec contribution ready for the merger contract
(per `../README.md` §5.5); the merger will run after subsequent
stage-4 sub-decisions (D-4-2 onward) close, since stage-4 is
solo and the merger is end-of-stage, not end-of-cluster.

---

## Round 1 -- D-4-2 initial review

**Date.** 2026-05-03.

**Scope.** D-4-2 cluster (parent decision + sub-decisions D-4-2.1
through D-4-2.9) covering the per-`Const`-variant byte-encoding
tags subsection of §9.7 (the `ConstIdentityKey` byte form),
§12.3's concrete `ConstId` walk-rule row replacing the
stage-4-schema stub, and §3.3 Phase B step 9's added const-graph
acyclicity sub-bullet.

**Verdict.** BLOCK -- 6 issues (2 BLOCK / 4 MINOR / 0 NIT).

**Issues raised.**

1. **[BLOCK] args-key-byte-source.** §9.7's `args_key` byte
   form said "underlying 16 bytes from `ArgsKey.0`" without
   pinning a `u128`-to-bytes operation; native-endian /
   transmute interpretation possible.
2. **[BLOCK] acyclicity-algorithm.** §3.3 step 9 + D-4-2.6
   said "DFS visited-set" without distinguishing
   "currently-on-stack" from "already-validated"; could miss
   cycles or falsely reject shared DAG children.
3. **[MINOR] scalar-kind-discriminant-crossref.** §9.7
   pointed at "§12.3 assignments" for `IntKind` /
   `UintKind` / `FloatKind` discs, but the normative
   source is §10's `#[repr(u8)]` enum definitions.
4. **[MINOR] append-only-audit-wording.** D-4-2.1 said
   "245 reserved tag values" right after invoking the
   no-forward-looking-placeholders rule -- contradiction.
5. **[MINOR] stale-line-crossrefs.** D-4-2 audit text used
   line refs (`spec-snapshot.md:2680-2706`,
   `spec-snapshot.md:2716`, `spec-snapshot.md:2684-2686`,
   `spec-snapshot.md:2658-2660`, `spec-snapshot.md:2649-2650`)
   that drifted after §9.7's byte-encoding subsection
   shifted line numbers.
6. **[MINOR] prior-art-overclaim.** D-4-2 parent cited
   `prior-art/README.md` §3.1 as "cross-cluster framing of
   hash-based identity keys for content-addressed dedup,"
   but §3.1 actually documents the *negative* result
   (5-of-5 non-rustc projects fail) with only rustc as
   adjacent precedent.

**Corrections applied.**

- Issue 1: Replaced "underlying 16 bytes" wording in §9.7
  and D-4-2.8 with explicit `ArgsKey.0.to_be_bytes()`;
  forbid native memory layout / transmute; spelled out the
  documented BE exception to §12.3's general LE rule
  (anchored on §12.3's BE-load convention for
  `ArgsKey::EMPTY`).
- Issue 2: Replaced "DFS visited-set" wording in §3.3
  step 9 and D-4-2.6 with explicit three-state DFS
  (`Unseen` / `Visiting` / `Done`); back-edge to
  `Visiting` is corruption, edge to `Done` is skipped
  (legitimate DAG sharing).
- Issue 3: Updated §9.7 byte-encoding subsection and
  D-4-2.2 to cite §10's `#[repr(u8)]` enum definitions
  as the normative source for `IntKind` / `UintKind` /
  `FloatKind` discs; noted §12.3 reuses the same values.
- Issue 4: Reworded D-4-2.1 from "245 reserved tag values"
  to "245 currently-unassigned tag values available for
  append-only future variants"; clarified
  no-forward-looking-placeholders means values are not
  pre-reserved for any specific extension.
- Issue 5: Replaced specific line refs in D-4-2 audit text
  (D-4-2 parent reasoning, D-4-2.2's discriminant cite,
  D-4-2.3's `GenericArgs` cite, D-4-2.8's BE-load
  convention cite) with section-only refs that don't drift
  with line numbers.
- Issue 6: Reframed D-4-2's `README.md` §3.1 cite to
  match D-4-1.5's accurate framing (5-of-5 negative;
  rustc adjacent); reframed D-4-2.1 / D-4-2.3's cites
  to use per-project files (LLVM, MLIR, charon, mrustc,
  regseq-model-old) instead of overclaiming via §3.1;
  reframed D-4-2.8's cite to inherit from D-4-1.5's
  framing.

Round-2 prompt focus: verify each correction; surface any
new issues introduced by the corrections; check residual
soundness across the cluster.

---

## Round 2 -- D-4-2 re-review after round-1 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 1. Round-1 corrections summarised
above.

**Verdict.** MINOR -- 4 issues (0 BLOCK / 4 MINOR / 0 NIT).
All round-1 issues verified closed. Findings are residual
cleanup of the round-1 fixes (incomplete propagation across
parallel mentions of the same concept).

**Issues raised.**

1. **[MINOR] args-key-underlying-bytes-residual.** D-4-2.8's
   normative text correctly pinned `to_be_bytes()`, but a
   downstream paragraph still said the stream "emits the
   underlying bytes directly, making endianness a
   non-question" -- partial regression on the R1 fix.
2. **[MINOR] acyclicity-visited-set-residual.** §3.3 step 9
   and D-4-2.6's main text correctly say "three-state DFS
   marks," but §9.7's recursion paragraph and D-4-2.6's
   first option text still summarised as "DFS visited-set."
3. **[MINOR] scalar-kind-crossref-residual.** D-4-2.2's
   normative text correctly cites §10's enum definitions,
   but the option text still said "reusing the §12.3
   assignments" and the cross-ref still located the disc
   assignments in §12.3.
4. **[MINOR] stale-line-crossrefs-residual.** R1 cleanup
   missed several stale `spec-snapshot.md:NNNN` refs:
   D-4-2 parent's "lines 2680-2706," D-4-2.4's
   `spec-snapshot.md:162`, D-4-2.4's
   `spec-snapshot.md:1689-1692`, D-4-2.5's
   `spec-snapshot.md:1741-1758`, D-4-2.7's
   `spec-snapshot.md:2669`, D-4-2.8's
   `spec-snapshot.md:1767-1803`.

**Corrections applied.**

- Issue 1: Replaced D-4-2.8's lingering "underlying bytes
  directly, making endianness a non-question" paragraph
  with explicit `ArgsKey.0.to_be_bytes()` wording, anchored
  on §12.3's BE-load convention.
- Issue 2: Updated §9.7's recursion paragraph from
  "DFS visited-set" to "three-state DFS marks --
  Unseen / Visiting / Done -- ... back-edge to Visiting
  is corruption, edge to Done is skipped as a legitimate
  shared DAG child." Updated D-4-2.6's first option text
  from "visited-set" to "three-state DFS marks
  (Unseen / Visiting / Done)" with back-edge-to-Visiting
  rejection.
- Issue 3: Updated D-4-2.2's option 1 text and Cross-refs
  to cite §10's `#[repr(u8)]` enum definitions as the
  normative source; §12.3 noted only as downstream reuse.
- Issue 4: Replaced all six remaining stale line refs in
  discussions.md with section-only cross-references
  (`§12.3's per-Ty byte-encoding-tags table`,
  `§3 VariantIdx type-alias declaration`,
  `§9.7's Const::EnumValue declaration`,
  `§9.7's Byte enum "Byte-sequence well-formedness
  invariants" subsection`,
  `§12.3's per-IR-identifier termination rule for TyId`,
  `§9.7's InstanceRef struct-declaration order`).

Round-3 prompt focus: verify R2 corrections; confirm no
further residual ambiguity; check whether the cluster is
ready for APPROVE.

---

## Round 3 -- D-4-2 re-review after round-2 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 2. Round-2 corrections summarised
above.

**Verdict.** BLOCK -- 1 issue (1 BLOCK / 0 MINOR / 0 NIT).
All round-2 issues verified closed (R2.1 / R2.2 / R2.3 / R2.4
all confirmed). The BLOCK is a deeper architectural finding
about validation ordering that the cumulative-consistency
pass surfaced after R2's residual cleanup completed.

**Issue raised.**

1. **[BLOCK] validation-order-before-acyclicity.** §3.3 step 4
   said it validates `ConstInterner.lookup` injectivity over
   canonical-content keys "for all variants" before step 9's
   acyclicity sub-bullet. But computing canonical keys for
   `Aggregate` / `EnumValue` requires recursive
   `canonical_const(ConstId)` calls, and step 9's three-state
   DFS is what makes such recursion safe (prevents infinite
   loops on tampered cyclic models). The validator was ordered
   to compute recursive value-level keys BEFORE the cycle
   check that makes those computations safe.

**Correction applied.**

- Restructured the §3.3 phase split so step 4 is type-level
  only: (a) recompute canonical-content keys for type-level
  scalar entries (no recursion; bounded), (b) verify they
  match stored `ConstInterner.lookup` keys, (c) injectivity
  over the type-level subset of `ConstInterner.lookup`,
  (d) every `GenericArg::Const(_)` reference resolves to a
  type-level variant. Value-level entries are NOT recomputed
  at step 4.
- Added a new value-level sub-bullet to §3.3 step 9 ordered
  AFTER the const-graph acyclicity sub-bullet:
  "Value-level lookup injectivity and canonical-form
  well-formedness." Recomputes canonical keys for value-level
  entries (now safe to recurse — DAG verified), checks they
  match stored keys, verifies injectivity over the value-level
  subset. Combined with step 4's type-level pass, this
  completes injectivity over the full `ConstInterner.lookup`
  key set.
- Updated §9.7's "Const validation phase split" subsection to
  reflect the refined split and the step-9 ordering.
- Updated D-4-2.6 audit-shape entry with a "Validation-order
  refinement" paragraph spelling out the four sub-steps of
  step 9 in order: resolve → content checks → acyclicity →
  value-level key recomputation + injectivity.

Round-4 prompt focus: verify R3 correction; confirm no
further validation-order issues; check whether the cluster is
ready for APPROVE.

---

## Round 4 -- D-4-2 re-review after round-3 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 3. Round-3 correction summarised
above.

**Verdict.** BLOCK -- 2 issues (1 BLOCK / 1 MINOR / 0 NIT).
Both are residual cleanup of the round-3 phase-split
restructuring that did not propagate to two parallel
mentions.

**Issues raised.**

1. **[BLOCK] phase-split-summary-still-claims-full-step4-uniqueness.**
   §3.3's "Phase B (continued) — late value-level const
   validation" preface still said "item 4 validates
   `ConstInterner` content uniqueness (no duplicate canonical-
   content lookup-map keys)" without the type-level qualifier
   — partial regression on the R3 fix. As written, the summary
   could be read as reintroducing the round-3 defect: full
   lookup-key uniqueness before value-level recursive keys are
   safe to compute.
2. **[MINOR] phase-split-order-wording-ambiguous.** §9.7's
   "Const validation phase split" subsection said
   "step-9 sub-bullets are ordered so const-graph acyclicity
   verifies first," but §3.3 step 9 actually orders sub-
   reference / content checks BEFORE acyclicity, then value-
   level key recomputation AFTER acyclicity. The §9.7 wording
   contradicted the §3.3 step ordering and D-4-2.6's audit-
   shape sub-step description.

**Corrections applied.**

- Issue 1: Updated §3.3's late-value-level preface to qualify
  step 4 as the type-level subset only ("type-level scalar
  entries: canonical-content lookup-map injectivity and
  canonical-form well-formedness for `Bool` / `Char` / `Int`
  / `Uint` / `Float`"); explicitly noted that "full
  `ConstInterner.lookup` injectivity over the entire key set
  is completed by the value-level pass at step 9 after
  acyclicity verifies." Updated step 9's preface to say
  "sub-reference resolution and value-level lookup injectivity
  are validated late."
- Issue 2: Replaced §9.7's "acyclicity verifies first" wording
  with the explicit four-sub-step ordering: (1) sub-reference
  resolution; (2) content checks; (3) acyclicity via three-
  state DFS; (4) value-level key recomputation + lookup
  injectivity. Noted that steps (1) and (2) are recursion-
  free (resolution, not key recomputation) so they can run
  before acyclicity; step (4) runs after acyclicity so
  recursive `canonical_const(ConstId)` calls terminate.

Round-5 prompt focus: verify R4 corrections; confirm no
further parallel mentions of validation order need updating;
check whether the cluster is ready for APPROVE.

---

## Round 5 -- D-4-2 re-review after round-4 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 4. Round-4 corrections summarised
above.

**Verdict.** MINOR -- 1 issue (0 BLOCK / 1 MINOR / 0 NIT).
All round-4 issues verified closed. The one finding is a
naming inconsistency surfaced by the cumulative-consistency
lens.

**Issue raised.**

1. **[MINOR] stale-canonical-const-repr-name.** §9.7's
   identity contract still said `ArgsKey` traversal queries
   `canonical_const_repr(ConstId)`, but the D-4-2 byte-form
   subsection defined the recursion as
   `canonical_const(ConstId)`, leaving one stale, undefined
   helper name in the cluster. Pre-existing wording from
   D-4-1's draft phase; surfaced now because R4's cleanup
   pass made naming consistency tractable to verify.

**Correction applied.**

- Replaced §9.7's `canonical_const_repr(ConstId)` reference
  with `canonical_const(ConstId)` and added "per the per-
  `Const`-variant byte-encoding tags subsection below" so
  readers can locate the helper's definition.
- Replaced one further `canonical_const_repr` reference in
  D-4-1.3's locked audit-shape entry (at `discussions.md:331`)
  with `canonical_const` plus a note that this is D-4-2's
  per-`Const`-variant byte-encoding helper. D-4-1.3 is
  LOCKED but the corrected naming is normative now, and the
  prior inconsistent name was a placeholder.

Round-6 prompt focus: verify R5 correction; check whether the
cluster is ready for APPROVE.

---

## Round 6 -- D-4-2 re-review after round-5 fix

**Date.** 2026-05-03.

**Scope.** Same as round 5. Round-5 correction summarised
above.

**Verdict.** MINOR -- 2 issues (0 BLOCK / 2 MINOR / 0 NIT).
R5 fix verified closed. Both findings are residual cross-
reference inaccuracies surfaced by the cumulative-consistency
lens.

**Issues raised.**

1. **[MINOR] stale-genericarg-const-phase-label.** §12.3's
   `GenericArg::Const` encoding said "the value-level
   identity-key rules pinned in D-4-2," but
   `GenericArg::Const(_)` positions resolve to type-level
   scalar variants only per §3.3 step 4 and §9.7's partition.
   Pre-existing wording from before D-4-2 closed; surfaced
   now because R5's naming-consistency pass made phase-label
   verification tractable.
2. **[MINOR] d-4-2-array-constid-crossref.** D-4-2 parent's
   reasoning said §12.3's `ConstId` walk-rule calls
   `canonical_const(ConstId)` from the per-`Ty` table for
   "(`Array`, `GenericArg::Const`)," but `Ty::Array` carries
   `(TyId, u64)` (a literal `u64` length, not a `ConstId`),
   so `Array` is not actually a `ConstId` call site. The
   only `ConstId` call site in the per-`Ty` walk is the
   `GenericArgs` container's `GenericArg::Const(ConstId)`
   arm.

**Corrections applied.**

- Issue 1: Replaced §12.3's `disc = 1` payload note from
  "the value-level identity-key rules pinned in D-4-2" to
  "the per-`Const`-variant byte-encoding rules pinned in
  D-4-2 / §9.7" plus "GenericArg::Const(_) positions resolve
  to type-level scalar variants only per §3.3 step 4 and
  §9.7's type-level / value-level partition."
- Issue 2: Updated D-4-2 parent's reasoning (iii) to drop
  `Array` from the call-site list and clarify that the only
  `ConstId` call site is the `GenericArgs` container's
  `GenericArg::Const(ConstId)` arm; noted that
  `Ty::Array(elem, n: u64)` carries a literal `u64`, not a
  `ConstId`.

Round-7 prompt focus: verify R6 corrections; final
cumulative-consistency check; expected to APPROVE.

---

## Round 7 -- D-4-2 final close-out re-review after round-6 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 6. Round-6 corrections summarised
above.

**Verdict.** APPROVE -- 0 issues (0 BLOCK / 0 MINOR / 0 NIT).
All round-6 issues verified closed. Cumulative-consistency
pass found no further issues. D-4-2 cluster closed.

**Corrections applied.** None required.

---

## Round-trajectory summary -- D-4-2

```
R1: 6 issues (2 BLOCK / 4 MINOR / 0 NIT)   [BLOCK]
R2: 4 issues (0 BLOCK / 4 MINOR / 0 NIT)   [MINOR] -- all R1 closed; residual cleanup
R3: 1 issue  (1 BLOCK / 0 MINOR / 0 NIT)   [BLOCK] -- all R2 closed; deeper validation-order finding surfaced
R4: 2 issues (1 BLOCK / 1 MINOR / 0 NIT)   [BLOCK] -- R3 closed; phase-split parallel mentions not propagated
R5: 1 issue  (0 BLOCK / 1 MINOR / 0 NIT)   [MINOR] -- all R4 closed; naming inconsistency surfaced
R6: 2 issues (0 BLOCK / 2 MINOR / 0 NIT)   [MINOR] -- R5 closed; cross-reference inaccuracies surfaced
R7: 0 issues                                [APPROVE] -- all R6 closed
```

16 distinct findings closed across the iteration. D-4-2
cluster LOCKED in `discussions.md` and `status.md`. The
trajectory pattern (alternating BLOCK / MINOR rounds with
deeper findings surfacing as cumulative-consistency tightened)
mirrors D-4-1's R1=10 → R2=5 → R3=2 → R4=1 → R5=1 → R6=0
trajectory; both clusters required ~6-7 rounds to converge,
which is the established stage-4 spec-review depth.

---

## Round 1 -- D-4-3 initial review

**Date.** 2026-05-03.

**Scope.** D-4-3 cluster (parent decision + sub-decisions
D-4-3.1 through D-4-3.10) covering reference-typed const
handling via the new `SyntheticItemKind::ConstMemoryStatic`
family. Whole-family scope: stage-4 owns bridge dispatch +
const composition + Phase B validation; stage-12 territory
(§13.3 enum extension + §13.4 segments / DefPathHash) closed
in stage 4 since the changes mirror the vtable family
precedent. Spec under review: `stage-4/spec-snapshot.md` §9.7
(new "Reference-typed const handling (D-4-3)" subsection
replacing the prior deferred subsection; new dispatch table
rows for `Slice`, `Indirect`-over-Memory, and `Memory(memory)`
GlobalAlloc projection); §13.3 (`ContentHash` type;
`ConstMemoryStatic` variant + canonical encoding subsection);
§13.4 (const-memory family segments + DefPathHash + ownership
rule); §3.3 Phase B step 9 (`ConstMemoryStatic dedup` sub-
bullet). Audit shape: `stage-4/discussions.md` D-4-3 parent +
D-4-3.1 through D-4-3.10. Outbound: `stage-12/expectations.md`
"From stage 4" flipped to `[CLOSED by D-4-3]`.

**Verdict.** BLOCK -- 6 issues (2 BLOCK / 3 MINOR / 1 NIT).

**Issues raised.**

1. **[BLOCK] static-init-shape.** `InstanceKind::Static`
   stores `init: Option<ConstId>`, but the synthesis rule
   said `init: ConstId` (without the `Option` wrapper).
   Schema mismatch.
2. **[BLOCK] phase-b-family-validation.** §3.3 Phase B's
   `SyntheticItemKind ↔ ItemKind` consistency list omitted
   `ConstMemoryStatic → ItemKind::Static`. Step 9 had a
   dedup check but no immutable-statics validation parallel
   to the existing `VTableInstance` immutable-statics rule.
3. **[MINOR] stale-deferred-const-text.** §9.7 still had
   inline doc-comments on `Const::Bytes` and `Const::NullPtr`
   saying reference-typed handling "is deferred" even though
   D-4-3 closes it. The append-only `Const`-tag note also
   mentioned a future `ConstMemoryStatic`-backed family
   appending at tag `11+`, contradicting D-4-3's "no new
   `Const` variant" decision.
4. **[MINOR] thin-pointer-type-side.** The synthesised Static
   is type-erased to `Ty::Array(U8, N)`. Wide-pointer (Slice)
   dispatch documented how the typed-pointer side is carried
   by the wrapping `Aggregate`, but thin-pointer (Indirect)
   dispatch had no equivalent statement about how the
   pointee type is recovered at the use site.
5. **[MINOR] forward-placeholder-d4k.** Non-zero offset
   rejection text named a "future D-4-K extension" — a
   forward-looking placeholder that contradicts D-4-3.8's
   audit-shape claim of "no slot reserved."
6. **[NIT] methodology-mislabel.** D-4-3.10 (audit-trail
   bookkeeping) cited "Cross-build canonicalisation via
   stable identity" as its methodology rule, which is a
   substantive methodology rule that doesn't apply to
   bookkeeping.

**Corrections applied.**

- Issue 1: Updated §9.7 synthesis rule and D-4-3.6 audit-shape
  to write `init: Some(c)` with the full `Static { ty, init,
  mutability }` shape; explicitly noted that `init: None` is
  invalid for `ConstMemoryStatic` Items. Updated §3.3 step 9
  dedup sub-bullet to read `init: Some(c)` and reference the
  immutable-statics rule for `init: None` rejection.
- Issue 2: Added `ConstMemoryStatic { .. } → ItemKind::Static`
  to the `SyntheticItemKind ↔ ItemKind` consistency list in
  §3.3 Phase B. Added a new `ConstMemoryStatic` immutable-
  statics validation bullet parallel to the
  `VTableInstance` rule, requiring `mutability: Not`,
  `init: Some(c)`, byte-array `Ty` shape, and `Const::Bytes`
  init shape.
- Issue 3: Updated §9.7's `Const::Bytes` doc-comment to
  reference `ConstMemoryStatic` synthesised statics directly
  (no longer "deferred"). Updated `Const::NullPtr` doc-comment
  to clarify that reference-typed consts are handled by D-4-3,
  not as `NullPtr`. Updated the append-only `Const`-tag note
  to remove the `ConstMemoryStatic`-backed-family example
  (since D-4-3 confirmed no new `Const` variant) and replaced
  with a generic future-variant statement.
- Issue 4: Added a "Pointee type at the use site" paragraph
  after the bridge dispatch arms, explicitly stating that the
  consumer's context supplies the pointee type for both Slice
  (via the wrapping `Aggregate`'s position type) and
  Indirect-thin (via the `Reloc`'s position type). Noted
  C codegen casts from the byte static at the use site.
- Issue 5: Removed "future D-4-K extension" wording from §9.7
  and from the D-4-3 parent + D-4-3.8 audit entries. Replaced
  with "rejected unless a future spec revision admits them"
  (no specific decision name reserved).
- Issue 6: D-4-3.10 methodology rules updated to "None
  applicable — audit-trail bookkeeping," referencing the
  project-wide outbound-expectations convention rather than
  a stage-4 methodology rule.

Round-2 prompt focus: verify each correction; surface any
new issues introduced by the corrections; check residual
soundness across the cluster.

---

## Round 2 -- D-4-3 re-review after round-1 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 1. Round-1 corrections summarised
above.

**Verdict.** BLOCK -- 3 issues (1 BLOCK / 2 MINOR / 0 NIT).
All round-1 issues verified closed. The findings are
schema-shape and provenance / consistency cleanups surfaced
by the cumulative-consistency lens.

**Issues raised.**

1. **[BLOCK] item-ty-schema.** D-4-3 synthesis said
   `item.ty = TyId for Ty::Array(...)`, but the stage-1 `Item`
   schema does not have a `ty` field -- per-kind rich data
   (including the Static's `ty`) lives on the `Instance`, not
   the `Item`. `Item` carries only `def_path`, `kind`, `span`,
   flags, and `opaque_handle_eligible`.
2. **[MINOR] stale-stage4-status.** `stage-12/expectations.md`
   was already flipped to `[CLOSED by D-4-3]`, but
   `stage-4/status.md` still said D-4-3 was "next / open" with
   the deferred-items section flagging `&str` / `&[T]` as
   open. Inconsistent in-flight state.
3. **[MINOR] wrong-lock-provenance.** D-4-3's non-zero-offset
   rationale said adding an offset would change a "stage-3-
   locked structure" twice, but `InstanceRef` is the
   stage-4-locked const-relocation shape from D-4-1.5 / §9.7.

**Corrections applied.**

- Issue 1: Removed `item.ty = TyId for Ty::Array(...)` from
  the §9.7 synthesis rule and from D-4-3.6's audit-shape
  decision text. Updated the synthesised `Instance` shape to
  carry the `ty` (`InstanceKind::Static { ty: array_ty, init:
  Some(c), mutability: Not }`) and added an explanatory note
  in both places that per-kind rich data lives on the
  `Instance`, not the `Item`, per the stage-1 schema. The
  immutable-statics validation rule was already correct
  (reads `array_ty` from the Instance, not the Item) -- no
  change needed there.
- Issue 2: Updated `stage-4/status.md`'s top-of-file Status
  line to "D-4-3 IN CODEX REVIEW"; reworked the inbound-
  expectations bullet to mark `&str` / `&[T]` closed by D-4-3
  (in active review); added an "In-flight decisions" section
  for D-4-3; renamed "Open decisions" to "(post-D-4-3)";
  rewrote the "Stage-4-specific deferred items" section to
  state no items are currently active; rewrote "Next concrete
  unit of work" to focus on completing the D-4-3 review loop;
  updated the cluster-split discussion to reference D-4-K
  (not D-4-3+) as the only remaining decision after D-4-3.
- Issue 3: Replaced "stage-3-locked structure" with
  "stage-4-locked `InstanceRef` shape (D-4-1.5 / §9.7)" in
  both occurrences in `discussions.md` (D-4-3 parent reasoning
  + D-4-3.8 reasoning).

Round-3 prompt focus: verify R2 corrections; check whether
the cluster is ready for APPROVE.

---

## Round 3 -- D-4-3 re-review after round-2 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 2. Round-2 corrections summarised
above.

**Verdict.** MINOR -- 1 issue (0 BLOCK / 1 MINOR / 0 NIT).
All round-2 issues verified closed. The single finding is a
cross-reference inaccuracy in the D-4-3 parent's audit-shape
entry that did not propagate when status.md was restructured.

**Issue raised.**

1. **[MINOR] stale-d4-3-crossrefs.** D-4-3 parent's "Cross-refs"
   section pointed at `status.md` "Open decisions / Locked
   decisions" but D-4-3 now lives under "In-flight decisions"
   (with "Open decisions (post-D-4-3)" containing only D-4-K).
   Same cross-ref said `review-log.md` rounds were "none yet,"
   but D-4-3 had Round 1 and Round 2 entries.

**Correction applied.**

- Updated D-4-3 parent's cross-ref to point at `status.md`
  "In-flight decisions" / "Open decisions (post-D-4-3)" and
  `review-log.md` "D-4-3 rounds" (replacing "rounds (none
  yet)").

Round-4 prompt focus: verify R3 correction; final cumulative-
consistency check; expected to APPROVE.

---

## Round 4 -- D-4-3 final close-out re-review after round-3 fix

**Date.** 2026-05-03.

**Scope.** Same as round 3.

**Verdict.** APPROVE -- 0 issues (0 BLOCK / 0 MINOR / 0 NIT).
All round-3 issues verified closed. Cumulative-consistency
pass found no further issues. D-4-3 cluster closed.

**Corrections applied.** None required.

---

## Round-trajectory summary -- D-4-3

```
R1: 6 issues (2 BLOCK / 3 MINOR / 1 NIT)   [BLOCK]
R2: 3 issues (1 BLOCK / 2 MINOR / 0 NIT)   [BLOCK] -- all R1 closed; schema + provenance findings
R3: 1 issue  (0 BLOCK / 1 MINOR / 0 NIT)   [MINOR] -- all R2 closed; cross-ref propagation
R4: 0 issues                                [APPROVE] -- R3 closed
```

10 distinct findings closed across the iteration. D-4-3
cluster LOCKED in `discussions.md` and `status.md`. The
trajectory was tighter than D-4-1 (R1=10, 6 rounds) and
D-4-2 (R1=6, 7 rounds) -- 4 rounds total -- because most
D-4-3 design decisions were mechanical mirror of the
vtable-family precedent (§13.2 / §13.3 / §13.4) rather than
new design space. The deepest finding (R2 item-ty-schema)
was a real schema-shape error caught by the cumulative-
consistency lens after the surface-level wording errors
had been cleared.

## Round 1 -- D-4-K initial review

**Date.** 2026-05-03.

**Scope.** D-4-K cluster (single decision; no sub-decisions)
covering the `pretty.rs::render_const` renderer-style decision.
Phase B edits applied across `crates/regseq-model/src/pretty.rs`
(stub message + doc-comment), `stage-4/spec-snapshot.md` §18.2
(Renderer status per-method ownership table),
`stage-13/expectations.md` (initial deferral target -- later
moved per R1.2), `stage-4/discussions.md` D-4-K parent entry,
`stage-4/status.md` (Inbound expectations + In-flight decisions).

**Verdict.** BLOCK -- 4 issues (2 BLOCK / 2 MINOR / 0 NIT).

**Issues raised.**

1. **[BLOCK] dependency-count.** "7 of 11 variants depend on
   `render_ty`" was wrong in five locations. Actual count:
   5 self-contained scalars (`Bool` / `Char` / `Int` / `Uint`
   / `Float`) + 5 depending on `render_ty` (`ZST` / `Bytes` /
   `Aggregate` / `EnumValue` / `NullPtr`) + `Reloc` depending
   additionally on `render_def_path` + `render_instance` =
   11 total. Of these, 5 of 11 depend on `render_ty`; 6 of 11
   depend on at least one not-yet-filled renderer. The
   "four self-contained scalars" wording at one location also
   miscounted (it's 5, not 4).
2. **[BLOCK] deferral-target.** D-4-K deferred `render_const`
   to stage 13, but the stage-13 fill would still have a
   transitive-stub gap for `Reloc(InstanceRef)` -- which
   depends on `render_instance` (stage 14). That's the same
   "transitive panic" failure mode D-4-K rejected for
   Option C, applied to one variant. Codex suggested either
   defer to stage 14 (where all dependencies are filled) or
   split "stage 13 owns non-Reloc; stage 14 closes Reloc arm."
3. **[MINOR] stale-line-refs.** D-4-K reasoning cited
   `pretty.rs:121` and `pretty.rs:130` for `render_def_path` /
   `render_instance` stub lines, but the actual line numbers
   were 132 and 140 after the doc-comment expansion.
4. **[MINOR] status-staleness.** Two `stage-4/status.md`
   mentions of D-4-3 still said "in active Codex review"
   even though D-4-3 was LOCKED in the previous decision
   commit.

**Corrections applied.**

- Issue 1: Replaced "7 of 11" with the accurate count
  (5 / 5 / 1 = 11) in all five locations:
  `pretty.rs::render_const` doc-comment, `spec-snapshot.md`
  §18.2 ownership table, `stage-14/expectations.md`
  "From stage 4" (formerly stage-13's), `discussions.md` D-4-K
  parent reasoning, `status.md` top-of-file Status. Also fixed
  "four self-contained scalars" → "5 self-contained scalars"
  in D-4-K reasoning.
- Issue 2: Retargeted D-4-K's deferral from stage 13 to
  stage 14 (where all dependencies -- `render_def_path` from
  stage 12, `render_ty` from stage 13, `render_instance` from
  stage 14 -- are filled). Updated the stub message to
  "stage 14 fills render_const (deferred per D-4-K)"; moved
  the "From stage 4" outbound from `stage-13/expectations.md`
  to `stage-14/expectations.md` (alongside the existing
  `render_instance` outbound); added a top-of-file note in
  `stage-13/expectations.md` pointing at stage 14 for
  `render_const`. D-4-K audit-shape decision text rewrote
  Option A as "defer to stage 14" + added Option D
  (defer-to-stage-13 with the rejection reasoning). Updated
  reasoning to note D-4-K extends the D14 → stage-13
  precedent one stage further.
- Issue 3: Removed brittle line-number references from
  D-4-K reasoning (pointing at stub method names instead).
- Issue 4: Removed "(in active Codex review)" parentheticals
  from the two stale D-4-3 mentions in `status.md`.

Round-2 prompt focus: verify each correction; surface any
new issues introduced by the retarget; check residual
soundness across the cluster.

---

## Round 2 -- D-4-K re-review after round-1 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 1. Round-1 corrections summarised
above.

**Verdict.** BLOCK -- 2 issues (1 BLOCK / 1 MINOR / 0 NIT).
All round-1 issues verified closed. The findings are missed
files in the round-1 retarget pass.

**Issues raised.**

1. **[BLOCK] stale-inbound-expectation.** R1's retarget from
   stage-13 to stage-14 didn't propagate to
   `stage-4/expectations.md`'s render_const inbound entry,
   which still said "stage 4 replaces the body with a renderer"
   and quoted the old `unimplemented!("stage 4 fills
   render_const")` message. The inbound contradicted D-4-K's
   selected Option A deferral and the actual stub message.
2. **[MINOR] review-scope-bookkeeping.** `stage-4/status.md`'s
   "Next concrete unit of work" review-target list omitted
   `stage-14/expectations.md` after the retarget (it still
   listed `stage-13/expectations.md` as the outbound location).

**Corrections applied.**

- Issue 1: Updated `stage-4/expectations.md`'s render_const
  inbound entry to mark it `[CLOSED by D-4-K via deferral to
  stage 14]`; replaced the stale stub-replacement promise with
  a description of D-4-K's deferral rationale + the variant-
  by-variant dependency breakdown + cross-refs to the
  stage-14 outbound and the audit-shape entry.
- Issue 2: Updated `stage-4/status.md`'s review-target list
  to include `stage-14/expectations.md` (the actual outbound
  location), `stage-13/expectations.md` (top-of-file pointer
  only), and `stage-4/expectations.md` (the inbound closure).

Round-3 prompt focus: verify R2 corrections; final
cumulative-consistency check; expected to APPROVE.

---

## Round 3 -- D-4-K re-review after round-2 fixes

**Date.** 2026-05-03.

**Scope.** Same as round 2.

**Verdict.** MINOR -- 1 issue (0 MINOR / 1 MINOR / 0 NIT).
All round-2 issues verified closed. The single finding is a
follow-on §18.2 prose inconsistency.

**Issue raised.**

1. **[MINOR] stale-snapshot-test-deferral.** §18.2's per-method
   ownership table now correctly assigns `render_const` to
   stage 14, but the follow-on snapshot-test prose note still
   said tests with human-readable output are "deferred to
   stage 13" (a holdover from before the per-method table
   existed).

**Correction applied.**

- Updated §18.2's snapshot-test deferral prose to reference
  the per-method ownership table directly: "deferred to the
  owning renderer stages per the table above (stage 13 for
  render_ty; stage 14 for render_const and render_instance;
  stage 12 for render_def_path(ItemId); stage 12+ for
  top-level render())."

Round-4 prompt focus: verify R3 correction; expected to APPROVE.

---

## Round 4 -- D-4-K final close-out re-review after round-3 fix

**Date.** 2026-05-03.

**Scope.** Same as round 3.

**Verdict.** APPROVE -- 0 issues (0 BLOCK / 0 MINOR / 0 NIT).
All round-3 issues verified closed. Cumulative-consistency
pass found no further issues. D-4-K cluster closed.

**Corrections applied.** None required.

---

## Round-trajectory summary -- D-4-K

```
R1: 4 issues (2 BLOCK / 2 MINOR / 0 NIT)   [BLOCK]
R2: 2 issues (1 BLOCK / 1 MINOR / 0 NIT)   [BLOCK] -- all R1 closed; missed-files retarget findings
R3: 1 issue  (0 BLOCK / 1 MINOR / 0 NIT)   [MINOR] -- R2 closed; §18.2 prose
R4: 0 issues                                [APPROVE] -- R3 closed
```

7 distinct findings closed across the iteration. D-4-K
cluster LOCKED in `discussions.md` and `status.md`. The
trajectory was the tightest of stage 4's four clusters
(R1=10/6/6/4; ending in 6/7/4/4 rounds), reflecting D-4-K's
lighter design surface (deferral decision; no spec-shape
work) -- the only "real" finding was R1's deferral-target
retarget from stage 13 to stage 14, surfaced by Codex's
careful dependency analysis. The other findings were
mechanical propagation issues across the multi-file edit
surface.

Stage 4's spec-side work is now complete: D-4-1 (variant set),
D-4-2 (byte-form schema), D-4-3 (reference-typed handling),
D-4-K (renderer deferral). All four stage-1 inbound
expectations are closed. Implementation-side work follows
separately.

---

## Post-merge cross-stage reviews

Per README §5.5 step 6, each stage closure runs a Codex post-merge
review pass on the integrated top-level `spec.md`. This catches
cross-stage drift between the merging stage's contributions and
existing content. Each merger's rounds are recorded below.

### Stage-3 merger (2026-05-01)

Closed at R4=APPROVE on 2026-05-01 (per top-level `status.md`'s
last-updated note before stage-4 merger). Detail not preserved
under the new convention — status.md is the historical record
for stage 3's merger.

### Stage-4 merger (2026-05-03)

Closed at R4=APPROVE on 2026-05-03.

Trajectory:
- R1: 10 issues (1 BLOCK / 9 MINOR / 0 NIT). Largest:
  preamble retarget (stage 1 + stage 3 → stage 1 + stage 3 +
  stage 4); pre-existing pre-stage-4 issues surfaced (D-tag
  inline references; round-history prose; forward-looking
  placeholders in §8 / §14.5 / §15.1 rejection table; §18.2
  historical deferral language). All closed by R2.
- R2: 1 MINOR (`Ty::Array(Ty::Uint(U8), n)` syntax wrong;
  `Ty::Array` is `Array(TyId, u64)` so the elem must be a
  `TyId` resolved separately to `Ty::Uint(UintKind::U8)`).
  Three locations fixed (§3.3 inlined ConstMemoryStatic
  shape check, §9.7 bridge synthesis rule, §9.7 pointee-type
  prose). Closed by R3.
- R3: 1 MINOR (residual `ty = Ty::Array(u8, N)` shorthand in
  §13.3's `ConstMemoryStatic` doc-comment). Closed by R4.
- R4: APPROVE 0.

12 distinct findings closed across the iteration (10 + 1 + 1).
Mirrors stage-3's post-merge R1-R4 trajectory; both closed in
4 rounds.
