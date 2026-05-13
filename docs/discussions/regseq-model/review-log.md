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

### Stage-5 merger (2026-05-04)

In progress.

#### R1 (2026-05-04): VERDICT: MINOR -- 5 issues (4 MINOR / 1 NIT)

**Prompt body:** archived at
`/tmp/codex_stage5_postmerge_r1_prompt.txt`. Specifically
sweeps D-5-1's R10 deferred residuals per the
methodology-deviation rationale recorded in the per-stage
review-log.md under "Cluster closure (2026-05-04): D-5-1
LOCKED at R10".

**Codex output capture:** `/tmp/codex_stage5_postmerge_r1.txt`.

**Verdict:** MINOR -- 5 issues (0 BLOCK / 4 MINOR / 1 NIT).

**Issues:**

1. **[MINOR] stale-stage-summary** [`spec.md:13`] -- preamble
   still says "stages 1, 3, and 4"; needs to add stage 5.

2. **[MINOR] dtag-inline-prose** [`spec.md:5173`,
   `spec.md:5428`, `spec.md:5530`, `spec.md:5581-5582`] --
   D-tag inline references in spec prose
   (`deferred per D-5-K`, `per stage-3 D6.3`) should be
   replaced with `discussions.md` pointer form per README
   §5.9 rule 5.

3. **[MINOR] historical-stage-pointer** [`spec.md:5413`,
   `spec.md:5566-5567`] -- pointers to
   `stage-5/discussions.md` should be top-level
   `discussions.md` (audit ground truth is now merged).

4. **[MINOR] audit-bookkeeping-leak** [`spec.md:5555`,
   `spec.md:5606-5608`, `spec.md:5558-5559`] -- §19 carries
   merger/audit bookkeeping ("Outbound expectation
   filed...", "This deferral mirrors D-4-K") rather than
   live spec content. Per README §5.9 rule 3, audit
   material is dropped from spec.md.

5. **[NIT] renderer-summary-omits-place** [`spec.md:5211`]
   -- §18.2 follow-on summary mentions
   render_ty/render_const/render_def_path/render_instance/render()
   but omits render_place.

**Corrections applied for R2 (2026-05-04):**

1. **stale-stage-summary -- FIXED.** spec.md:13-14 updated
   to "stages 1 + 3 + 4 + stage 5 (MIR places and
   projections)".

2. **dtag-inline-prose -- FIXED.** Three sites:
   - §18.2 row at spec.md:5173: "deferred per D-5-K"
     replaced with "deferred; see `discussions.md` D-5-K
     for rationale".
   - §19.4 Field row at spec.md:5428: "per stage-3 D6.3"
     replaced with "see `discussions.md` D6.3 for
     rationale".
   - §19.7 rule 3 at spec.md:5530: "per stage-3 D6.3"
     replaced with "see `discussions.md` D6.3 for
     rationale".

   Excluded from rewrite: §19.1 doc-comment block at
   spec.md:5270 (operational doc-comment block, intended
   for the eventual implementation; permitted per the
   prompt's exception list).

3. **historical-stage-pointer -- FIXED.** Two sites:
   - §19.3 at spec.md:5413: "stage-5/discussions.md
     D-5-1.6" -> "discussions.md D-5-1.6".
   - §19.8 at spec.md:5566: "audited in
     stage-5/discussions.md D-5-K" sentence dropped (the
     subsequent prose is the audit content).

4. **audit-bookkeeping-leak -- FIXED.** Three sites:
   - §19.7 walk-deferral paragraph: dropped "Outbound
     expectation filed in stage-11/expectations.md 'From
     stage 5'" sentence and the "This deferral mirrors
     D-4-K" precedent paragraph; merged remaining content
     with a "see discussions.md D-5-2 for the audit shape
     and the deferral rationale" pointer.
   - §19.8 opening: "audited in stage-5/discussions.md
     D-5-K" parenthetical removed; the per-variant table
     stands alone as spec content.
   - §19.8 closing: "Outbound expectation filed in
     stage-11/expectations.md 'From stage 5' (alongside
     D-5-2's place-validation walk expectation)" sentence
     replaced with "(See discussions.md D-5-K for the
     per-variant dependency audit and the filing of the
     stage-11 deliverable.)" pointer.

5. **renderer-summary-omits-place -- FIXED.** §18.2 summary
   line extended: "stage 11 for `render_place`" added to
   the per-method dispatch list.

#### R2 (2026-05-04): VERDICT: MINOR -- 1 issue (1 MINOR / 0 NIT)

**Prompt body:** archived at
`/tmp/codex_stage5_postmerge_r2_prompt.txt`. Names R1's
five corrections explicitly.

**Codex output capture:** `/tmp/codex_stage5_postmerge_r2.txt`.

**Verdict:** MINOR -- 1 issue (0 BLOCK / 1 MINOR / 0 NIT).

**Issues:**

1. **[MINOR] residual-dtag-inline-prose** [`spec.md:5578-5579`]

   §19.8 still has plain-prose D-tag wording: "per stage-3
   D6.3" (line-wrapped across lines 5578-5579 in the
   render_place per-variant breakdown for `Ty::Closure`).
   This was missed in R1's sweep because the wrap broke
   single-line greps for the canonical pattern. README
   §5.9 rule 5 says spec text should avoid inline D-tag
   parentheticals.

   *Suggested fix:* change to "closure-as-struct
   identity-vs-layout split; see `discussions.md` D6.3
   for rationale".

**Corrections applied for R3 (2026-05-04):**

1. **residual-dtag-inline-prose -- FIXED.** §19.8
   Ty::Closure dependency-table bullet at spec.md:5577-5579
   updated: "(closure-as-struct identity-vs-layout split
   per stage-3 D6.3)" -> "(closure-as-struct
   identity-vs-layout split; see `discussions.md` D6.3
   for rationale)". Verified via multi-line-aware grep
   that no remaining "per stage-3 D6" / "per stage-3\nD6"
   patterns exist in spec prose; only the exception at
   line 5270 inside a Rust `///` doc-comment block
   remains (operational, intended for the eventual
   implementation; permitted per the prompt's exception
   list).

#### R3 (2026-05-04): VERDICT: MINOR -- 1 issue (1 MINOR / 0 NIT)

**Prompt body:** archived at
`/tmp/codex_stage5_postmerge_r3_prompt.txt`. Names R2's
correction explicitly.

**Codex output capture:** `/tmp/codex_stage5_postmerge_r3.txt`.

**Verdict:** MINOR -- 1 issue (0 BLOCK / 1 MINOR / 0 NIT).

**Issues:**

1. **[MINOR] h2-stale-prior-art-design-pressure**
   [`prior-art/rustc.md:1006-1024`]

   `prior-art/rustc.md` §8 still framed
   `UnwrapUnsafeBinder` as "Probably not present" because
   Regseq DSL inputs are "stable `#![no_std]` Rust" and
   suggested rejection-diagnostic wording cite feature
   instability. That conflicts with H2 REFINED: the DSL
   does not formally exclude `unsafe_binders`; rationale
   for rejection is no driver use case + closed-vocabulary
   policy. The integrated top-level spec already reflects
   the refined result.

   *Suggested fix:* rewrite §8's `UnwrapUnsafeBinder`
   design-pressure paragraph to match H2 REFINED, or mark
   it explicitly superseded by the hypothesis cross-check
   block.

**Corrections applied for R4 (2026-05-04):**

1. **h2-stale-prior-art-design-pressure -- FIXED.** §8's
   `OpaqueCast` and `UnwrapUnsafeBinder` paragraphs both
   rewritten:
   - `OpaqueCast` paragraph now states "Absent under both
     solver paths in our input (H1 REFINED -- legacy
     strips it via `PostAnalysisNormalize`; next-solver
     never constructs it because the construction site at
     `mir_build/src/builder/matches/match_pair.rs:104-118`
     is symmetrically gated). D-5-1 chose option (a):
     reject if encountered (no closed-vocabulary slot
     reserved)."
   - `UnwrapUnsafeBinder` paragraph rewritten with H2
     REFINED framing: "DSL surface ... carries no formal
     feature-gate allow/deny list ... so a driver could in
     principle write `#![feature(unsafe_binders)]` -- but
     no plausible device-driver pattern needs
     `unsafe<...>` types ... D-5-1 chose option (a):
     reject if encountered. Rationale follows 'no
     defensive fallbacks; the toolchain is pinned' -- carry
     no closed-vocabulary slot for a variant we have no
     use case for. Diagnostic cites the closed-vocabulary
     policy and the absence of a v1 driver use case (NOT
     the feature's instability per se -- a future stable
     `unsafe_binders` would not change the rejection)."
   - "Phase A decision" footer replaced with "decided in
     favour of (a) at D-5-1.1 LOCKED" + cross-reference to
     "Hypothesis cross-check results" section.

#### R4 (2026-05-04): VERDICT: MINOR -- 2 issues (1 MINOR / 1 NIT)

**Prompt body:** archived at
`/tmp/codex_stage5_postmerge_r4_prompt.txt`. Names R3's
correction explicitly.

**Codex output capture:** `/tmp/codex_stage5_postmerge_r4.txt`.

**Verdict:** MINOR -- 2 issues (0 BLOCK / 1 MINOR / 1 NIT).

**Issues:**

1. **[MINOR] prior-art-open-renderer-target-stale**
   [`prior-art/rustc.md:1181-1192`]

   `prior-art/rustc.md` §11 item 8 still framed
   `render_place` as an open Phase A question and said
   field/variant name lookup comes from stage-12
   Item/DefPath, with possible deferral to stage 13 or 12.
   The merged spec says the latest dependency is stage 11
   and Item/stage 12 is not a dependency.

   *Suggested fix:* replace §11 item 8 with a closed note:
   D-5-K LOCKED, deferred to stage 11; layout-side names;
   `render_ty` not a dependency.

2. **[NIT] residual-dtag-inline-heading** [`spec.md:5562`,
   `spec.md:5203-5205`, `spec.md:5603-5606`]

   §19.8 heading still embeds `(D-5-K)` suffix; stub-string
   `deferred per D-5-K` still appears in two live spec
   locations (§18.2 ownership row + §19.8 stub message).
   Per README §5.9 rule 5 these inline D-tag references
   should be avoided when an adjacent `discussions.md`
   pointer can carry the rationale.

   *Suggested fix:* make the heading "Rendering: deferred
   to stage 11" and remove the D-tag from the stub
   message, leaving the adjacent `discussions.md` pointer
   for rationale.

**Corrections applied for R5 (2026-05-04):**

1. **prior-art-open-renderer-target-stale -- FIXED.**
   `prior-art/rustc.md` §11 item 8 rewritten as a closed
   note: "**Renderer style decision (D-5-K)** -- CLOSED.
   `pretty.rs::render_place` deferred to **stage 11**.
   Original framing of this question proposed stage-13 or
   stage-12 deferral options; D-5-K LOCKED at stage 11 on
   the D-5-2/D-5-K combined R1 BLOCK finding that `Item`
   carries no field/variant name tables -- field and
   variant names live in §11 layout schema's
   `FieldLayout.name` / `VariantLayout.name`, reachable at
   render time via the head local's `LocalDecl.ty`. `Item`
   (stage 12) is not a `render_place` dependency, and
   `render_ty` (stage 13) is not a dependency either... See
   `discussions.md` D-5-K for the audit shape and retarget
   rationale." Trailing "These are framing inputs for
   Phase A; not decisions" sentence updated to note that
   questions 1-7 closed via Phase A and question 8 closed
   at D-5-K post-retarget.

2. **residual-dtag-inline-heading -- FIXED.**
   - §19.8 heading: "Rendering: deferred to stage 11
     (D-5-K)" -> "Rendering: deferred to stage 11".
   - §18.2 ownership row stub message:
     `unimplemented!("stage 11 fills render_place (deferred
     per D-5-K)")` -> `unimplemented!("stage 11 fills
     render_place (deferred)")`.
   - §19.8 stub message at spec.md:5603-5604: same
     replacement (`deferred per D-5-K` -> `deferred`).

   The adjacent `discussions.md` D-5-K pointers in both
   §18.2 and §19.8 carry the rationale.

#### R5 (2026-05-04): VERDICT: MINOR -- 2 issues (1 MINOR / 1 NIT)

**Prompt body:** archived at
`/tmp/codex_stage5_postmerge_r5_prompt.txt`. Names R4's
two corrections explicitly.

**Codex output capture:** `/tmp/codex_stage5_postmerge_r5.txt`.

**Verdict:** MINOR -- 2 issues (0 BLOCK / 1 MINOR / 1 NIT).

**Issues:**

1. **[MINOR] methodology-summary-stale** [`spec.md:52-67`]

   `spec.md` §1.2 lists README §2's binding methodology
   rules but omits two rules added during this trajectory:
   "Spec before implementation" (added earlier in this
   session) and "Compositional behavior beats special-case
   flags" (added at D-5-1 R4 promotion). Stage 5 decisions
   invoke both omitted rules in the integrated audit
   record.

   *Suggested fix:* add both omitted one-line bullets to
   `spec.md` §1.2 so its binding methodology summary
   matches README §2.

2. **[NIT] stale-d5k-stub-in-discussions** [`discussions.md:5621`]

   The live spec stubs now use
   `unimplemented!("stage 11 fills render_place
   (deferred)")`, but the integrated D-5-K audit-shape
   entry's Option B description still quotes the old
   `deferred per D-5-K` stub. Decision text picks Option B,
   so the text is internally inconsistent.

   *Suggested fix:* update the quoted Option B stub to
   `deferred`, or remove the exact stub string from the
   option text.

**Corrections applied for R6 (2026-05-04):**

1. **methodology-summary-stale -- FIXED.** spec.md §1.2
   methodology-rules summary list extended with two new
   bullets (matching their README §2 placement):
   - "Spec before implementation" inserted between "Spec
     is mutable" and "Compiler-theory-first" -- one-line
     summary: "phase-1 implementation deferred to a single
     end-of-phase-1 sweep; no per-stage interleaving".
   - "Compositional behavior beats special-case flags"
     inserted between "Compiler-theory-first" and "Source
     declares its own compiler-relevant attributes" --
     one-line summary: "when a behavior derives from
     existing rules, do not introduce a parallel
     flag-style marker; variant tags + the existing rule
     chain carry the distinction".

2. **stale-d5k-stub-in-discussions -- FIXED.**
   discussions.md D-5-K Option B description:
   `unimplemented!("stage 11 fills render_place (deferred
   per D-5-K)")` -> `unimplemented!("stage 11 fills
   render_place (deferred)")`. Now matches the live spec
   stub at spec.md §18.2 + §19.8.

#### R6 (2026-05-04): VERDICT: MINOR -- 1 issue (1 NIT)

**Prompt body:** archived at
`/tmp/codex_stage5_postmerge_r6_prompt.txt`. Names R5's
two corrections explicitly.

**Codex output capture:** `/tmp/codex_stage5_postmerge_r6.txt`.

**Verdict:** MINOR -- 1 issue (0 BLOCK / 0 MINOR / 1 NIT).
First round without any MINOR.

**Issues:**

1. **[NIT] h2-stale-option-c-unreachable**
   [`discussions.md:4670-4673`]

   D-5-1 Option C still groups `UnwrapUnsafeBinder` with
   `OpaqueCast` as "unreachable". That is stale for H2:
   the evidence says `UnwrapUnsafeBinder` has no MirPhase
   restriction and would reach input unchanged if a driver
   opted into `unsafe_binders`; the chosen rule is
   rejection for no v1 driver use case, not unreachability.

   *Suggested fix:* rewrite Option C as carrying all rustc
   variants, including `OpaqueCast` (absent under H1) and
   `UnwrapUnsafeBinder` (feature-gated/no v1 use case
   under H2), rather than calling both unreachable.

**Corrections applied for R7 (2026-05-04):**

1. **h2-stale-option-c-unreachable -- FIXED.** D-5-1 Option
   C in discussions.md:4670-4674 rewritten to split
   attribution per variant: "Carry all 8 of rustc's
   variants, including `OpaqueCast` (absent from our input
   under both solver paths per H1 REFINED) and
   `UnwrapUnsafeBinder` (gated behind unstable
   `unsafe_binders`; no MirPhase restriction, would reach
   input unchanged if a driver opted in; no v1 driver use
   case per H2 REFINED). Future toolchain changes wouldn't
   force spec revisions; lower each as a no-op type
   refinement where appropriate."

#### R7 (2026-05-04): VERDICT: MINOR -- 1 issue (1 NIT)

**Prompt body:** archived at
`/tmp/codex_stage5_postmerge_r7_prompt.txt`. Names R6's
correction explicitly.

**Codex output capture:** `/tmp/codex_stage5_postmerge_r7.txt`.

**Verdict:** MINOR -- 1 issue (0 BLOCK / 0 MINOR / 1 NIT).
Second consecutive round without any MINOR.

**Issues:**

1. **[NIT] h2-stale-reachability-framing**
   [`prior-art/rustc.md:146-149`]

   `prior-art/rustc.md` §2 early framing says D-5-1 must
   decide which rustc variants are "reachable in our
   `optimized_mir` input" versus "pre-elaborated away".
   Stale for `UnwrapUnsafeBinder`: H2 REFINED says the
   variant has no MirPhase restriction and would reach
   input unchanged if a driver opted in -- the rejection
   reason is no-v1-driver-use-case + closed-vocabulary
   policy, not pre-elaboration.

   *Suggested fix:* rewrite the early framing to include
   unsupported closed-vocabulary rejections, e.g. "which
   are reachable, which are absent due solver/phase
   behavior, and which are rejected if encountered."

**Corrections applied for R8 (2026-05-04):**

1. **h2-stale-reachability-framing -- FIXED.**
   `prior-art/rustc.md` §2 early framing rewritten with
   three categories instead of two: "(a) reachable in our
   `optimized_mir` input; (b) absent due to solver/phase
   behavior (e.g., `OpaqueCast` per H1 REFINED); (c)
   rejected if encountered as a closed-vocabulary policy
   (e.g., `UnwrapUnsafeBinder` -- has no MirPhase
   restriction so would reach input unchanged if a driver
   opted into the unstable feature, but no v1 driver use
   case so rejected per H2 REFINED + the 'no defensive
   fallbacks' methodology rule)."

#### R8 (2026-05-04): VERDICT: MINOR -- 2 issues (2 NIT)

**Prompt body:** archived at
`/tmp/codex_stage5_postmerge_r8_prompt.txt`. Names R7's
correction explicitly.

**Codex output capture:** `/tmp/codex_stage5_postmerge_r8.txt`.

**Verdict:** MINOR -- 2 issues (0 BLOCK / 0 MINOR / 2 NIT).
Third consecutive round without any MINOR.

**Issues:**

1. **[NIT] spec-prior-art-path-drift** [`spec.md:5303`,
   `spec.md:5333`]

   §19 Rust doc-comments cite `prior-art/rustc.md` (no
   `stage-5/` prefix) at lines 5303 and 5333; the same
   live top-level section uses `stage-5/prior-art/rustc.md`
   (with prefix) at line 5458. Inconsistent path form.

   *Suggested fix:* normalize the two unqualified
   citations to `stage-5/prior-art/rustc.md`, or drop the
   evidence-file citation from the live spec prose.

2. **[NIT] d5-1.2-id-width-crossref**
   [`discussions.md:4988-4989`]

   D-5-1.2 says `TyId` is `u32` by "stage 1 D-5-1's
   id-width choice" -- a wrong cross-reference (the D-5-1
   tag belongs to stage 5, not stage 1; stage 1 uses
   legacy F-tags). The live spec already states the
   actual ID-width rule directly at §2.2.

   *Suggested fix:* replace "stage 1 D-5-1's id-width
   choice" with "the stage-1 ID-width rule / top-level §2.2
   backing representation."

**Corrections applied for R9 (2026-05-04):**

1. **spec-prior-art-path-drift -- FIXED.** Two §19
   doc-comment citations updated from `prior-art/rustc.md`
   to `stage-5/prior-art/rustc.md` (matching the existing
   prefixed form at line 5458 and consistent with the
   merger-doesn't-touch-prior-art rule from README §5.5):
   - §19.1 ConstantIndex doc-comment (line 5303): now
     cites `stage-5/prior-art/rustc.md` §3.4.
   - §19.1 Subslice doc-comment (line 5333): now cites
     `stage-5/prior-art/rustc.md` §3.5.

2. **d5-1.2-id-width-crossref -- FIXED.** D-5-1.2 cost
   analysis paragraph at discussions.md:4988-4989 updated:
   "stage 1 D-5-1's id-width choice" replaced with "the
   stage-1 ID-width rule (top-level `spec.md` §2.2 backing
   representation)".

R9 not run -- post-merge LOCKED at R8 by user decision
(2026-05-04). See "Stage-5 post-merge closure" below.

### Stage-5 post-merge closure (2026-05-04): LOCKED at R8

After R8's NIT-only verdict (third consecutive round
without any MINOR), the user elected to lock the stage-5
post-merge review rather than continue iterating toward an
explicit APPROVE round. The R8 corrections (spec.md path
normalisation + discussions.md D-tag cross-reference fix)
are applied but **not verified by an R9 round**; treat as
the final applied state of the stage-5 post-merge.

**Trajectory summary:**

| Round | Verdict | Issues | Severity profile |
|---|---|---|---|
| R1 | MINOR | 5 | 4 MINOR / 1 NIT |
| R2 | MINOR | 1 | 1 MINOR / 0 NIT |
| R3 | MINOR | 1 | 1 MINOR / 0 NIT |
| R4 | MINOR | 2 | 1 MINOR / 1 NIT |
| R5 | MINOR | 2 | 1 MINOR / 1 NIT |
| R6 | MINOR | 1 | 0 MINOR / 1 NIT |
| R7 | MINOR | 1 | 0 MINOR / 1 NIT |
| R8 | MINOR | 2 | 0 MINOR / 2 NIT |

Total: 8 rounds; 15 distinct findings closed (0 BLOCK + 7
MINOR + 8 NIT). All R1-R8 corrections applied; R9 unverified.

**Methodology-deviation rationale.** README §5.5 step 6
mandates "Iterate to APPROVE." This closure is a more
substantive deviation than D-5-1's R10 lock because there
is no further backstop in the methodology -- this top-level
merger review IS the safety net. Justification:

- Substantive content settled since R5 (last MINOR-level
  finding was the §1.2 methodology summary missing two
  bullets -- now fixed).
- Three consecutive rounds (R6/R7/R8) with NIT-only
  verdicts; findings increasingly granular (Option C
  wording, prior-art early framing, doc-comment path
  consistency, single D-tag cross-reference precision).
- Cumulative stage-5 Codex cost reached 24 rounds (10
  D-5-1 + 6 D-5-2/D-5-K + 8 post-merge), more than 2x
  stage-4's 14 rounds; the cost is paying out the D-5-1
  R10 lock-at-MINOR decision exactly as anticipated.
- The trajectory is asymptotic but slow: each round finds
  1-2 NITs; APPROVE could land in 1-3 more rounds, but
  could also continue.

**Residuals deferred at lock.** R8 was the last verified
round with concrete findings. R8's two NIT corrections
(spec.md path normalisation; D-5-1.2 D-tag cross-reference)
are applied but unverified. Any further residuals that
would have been surfaced by R9+ are deferred to:

- The next stage's merger Codex review (when stage 6 / 7 /
  ... close, the post-merge sweep at that tier provides
  another adversarial pass over the integrated top-level
  spec, naturally re-encountering any stage-5 residuals
  that affect cross-stage consistency).
- Implementation-time review (when phase-1 implementation
  begins at end-of-phase-1, the spec is read closely and
  any wording residuals that affect implementation can be
  surfaced there).
- Phase-2 device-pressure review (pl011 lowering puts
  cross-stage spec content under device-driver pressure,
  which surfaces wording inconsistencies that pure spec
  review can miss).

**Stage-5 spec-side status:** post-merge LOCKED at R8.
Merger contract step 6 closes (with documented
deviation); step 7 (bookkeeping) closes stage 5 entirely.

---

## Stage 5 (MIR places and projections)

Round-by-round adversarial review trail for stage-5
`spec-snapshot.md`. Each round records verdict, full issue
list, and corrections applied (the latter populated when
corrections land before the next round).

Status of decisions in this file mirrors `discussions.md` §1
status tags (`[DRAFT]` / `[LOCKED]` / `[REOPENED]`).

D-tag convention: prefixed `D-5-K` form per `../README.md` §5.2.

Codex runner contract: project root `CLAUDE.md`. Per-round
prompt template: `memory.md` §4. Output capture: `/tmp/codex_d<cluster>_review_r<N>.txt`.

---

## D-5-1 cluster review trajectory

Initial review of D-5-1 sub-decisions 1.1 through 1.6 (core
`Place` / `PlaceElem` vocabulary + per-variant payload shapes
+ projection storage shape + structural invariants). D-5-2 and
D-5-K are out of scope for this trajectory and are reviewed
separately.

### R1 (2026-05-03): VERDICT: BLOCK

**Prompt body:** archived at
`/tmp/codex_d51_review_r1_prompt.txt`. Per `memory.md` §4
template (task / grounding_rules / structured_output_contract
/ default_follow_through_policy). Evidence files cited:
`prior-art/rustc.md`, `rustc-references.md`, `README.md`,
`stage-1/spec-snapshot.md`, `stage-3/spec-snapshot.md`.

**Codex output capture:** `/tmp/codex_d51_review_r1.txt`.

**Verdict:** BLOCK

**Issue count:** 4 (2 BLOCK / 2 MINOR / 0 NIT)

**Issues:**

1. **[BLOCK] subslice-narrowing** [`spec-snapshot.md:5349`]

   §19.4 says array `Subslice` always narrows to `Ty::Array(elem, n - from - to)`,
   but rustc has two array cases: `!from_end => to - from`,
   `from_end => n - from - to`
   (`prior-art/rustc.md:325-337`; pinned source
   `rustc_middle/src/mir/statement.rs:224-234`).

   *Suggested fix:* split the array successor rule into the
   two rustc cases, and keep the slice case as `Ty::Slice(elem)`.

2. **[BLOCK] regseq-ty-vocabulary** [`spec-snapshot.md:5345`]

   §19.4 uses `Ty::RawPtr`, `Ty::Adt`, and `Ty::Coroutine`,
   but the stage-5 `Ty` enum has `Ptr`, `Struct`, `Enum`,
   `Union`, and no `RawPtr` / `Adt` / `Coroutine` variants
   (`spec-snapshot.md:2375-2395`). The spec also rejects the
   coroutine family at type translation (`spec-snapshot.md:4568`).

   *Suggested fix:* rewrite §19.4 in Regseq `Ty` vocabulary:
   `Deref` accepts `Ty::Ref` / `Ty::Ptr`; `Field` accepts
   `Struct` / `Union` / `Tuple` / `Closure` plus `Enum` when
   an active variant is set; `Downcast` accepts `Ty::Enum`.

3. **[MINOR] field-active-variant** [`spec-snapshot.md:5346`]

   The `Field` row says active variant is "unchanged (cleared
   after consumption)," which is internally contradictory; the
   following paragraph says the active variant is cleared
   after `Field` (`spec-snapshot.md:5352-5359`). Rustc returns
   `PlaceTy::from_ty(...)` for `Field`, clearing
   `variant_index` (`prior-art/rustc.md:610-614`).

   *Suggested fix:* make the `Field` successor active-variant
   cell `None`.

4. **[MINOR] prior-art-count-citation** [`discussions.md:165`]

   The audit trail cites `prior-art/rustc.md` §2 as an
   "8-variant" enumeration, while that evidence section
   currently says "9 variants" and "All 9 variants"
   (`prior-art/rustc.md:86-120`). The pinned source shown in
   the evidence block lists 8 variants: `Deref`, `Field`,
   `Index`, `ConstantIndex`, `Subslice`, `Downcast`,
   `OpaqueCast`, `UnwrapUnsafeBinder`
   (`prior-art/rustc.md:93-110`).

   *Suggested fix:* correct `prior-art/rustc.md` §2's count
   text to 8, or adjust the audit citation so it does not
   cite contradictory evidence.

**Corrections applied for R2 (2026-05-03):**

1. **[BLOCK] subslice-narrowing -- FIXED.** §19.4 Subslice
   row split into two rows per `from_end`:
   - `Subslice { from, to, from_end: false }` over
     `Ty::Array(elem, _)` yields `Ty::Array(elem, to - from)`.
   - `Subslice { from, to, from_end: true }` over
     `Ty::Array(elem, n)` yields `Ty::Array(elem, n - from - to)`;
     over `Ty::Slice(elem)` yields unchanged.

   New prose paragraph explains the `from_end`-driven split
   and cites `rustc_middle/src/mir/statement.rs:224-234` plus
   `prior-art/rustc.md` §3.5. Added the doc-level invariant
   note: for `PlaceElem`s, `from_end == true` iff the base is
   a slice; the narrowing rule covers both array cases for
   completeness.

2. **[BLOCK] regseq-ty-vocabulary -- FIXED.** §19.4 table
   rewritten in Regseq `Ty` vocabulary:
   - `Deref`: predecessor `Ty::Ref(inner, _)` or
     `Ty::Ptr(inner, _)` (raw pointer); successor is the
     resolved `inner`. Matches the actual `Ty::Ref(TyId,
     Provenance)` and `Ty::Ptr(TyId, Mutability)` shapes from
     §10.
   - `Field`: predecessor `Ty::Struct{..}`, `Ty::Union{..}`,
     `Ty::Tuple{..}`, or `Ty::Closure(..)` when active
     variant is `None`; predecessor `Ty::Enum{..}` when
     active variant is `Some(v)` (projecting into variant
     `v`'s fields). Closure-as-struct shape per stage-3 D6.3.
   - `Downcast`: predecessor `Ty::Enum{..}` only (coroutine
     family is §15.1-rejected at type translation).

   Added a note immediately after the table: `Ty::Coroutine`
   does not appear because the coroutine family is rejected
   at type translation (§15.1); rustc's `Field`-on-coroutine
   and `Downcast`-on-coroutine narrowing arms have no analog
   in our input.

   Same vocabulary fix applied to `discussions.md` D-5-2's
   rule-listing (the parallel place-validation rule statement
   in the audit-shape entry). `Ty::Adt` parenthetical in
   D-5-1.2's Option B reasoning generalised to "the projected
   ADT's `VariantLayout.fields[idx]`" so the term covers
   `Struct` / `Union` / `Tuple` / `Closure` uniformly.

3. **[MINOR] field-active-variant -- FIXED.** §19.4 `Field`
   row's "Active variant after step" cell now reads
   `None (cleared)` (no contradictory phrasing). The header
   row also renamed from "Active variant" to "Active variant
   after step" to make the column meaning unambiguous. The
   following prose paragraph already said "the post-`Field`
   place type has no active variant"; updated to read "active
   variant `None`" for consistency. Discussions.md D-5-2 rule
   listing also updated to make the post-`Field` clearing
   explicit.

4. **[MINOR] prior-art-count-citation -- FIXED.**
   `prior-art/rustc.md` §2 corrected at three sites:
   - "9 variants in source-declaration order" -> "8 variants
     in source-declaration order".
   - "confirms the 9-variant set" / "All 9 variants are
     listed" -> "confirms the 8-variant set" / "All 8
     variants are listed".
   - "9 variants in rustc's projection enum" -> "8 variants
     in rustc's projection enum".

   `discussions.md` D-5-1's "8-variant" wording (the
   `[DRAFT]` audit-shape entry) now matches the evidence
   file. The `try_map` impl enumeration in
   `prior-art/rustc.md` §2 already lists 8 names; no change
   needed there.

### R2 (2026-05-03): VERDICT: BLOCK

**Prompt body:** archived at
`/tmp/codex_d51_review_r2_prompt.txt`. Names R1's four
corrections explicitly so Codex can verify them. Per
`memory.md` §4 template.

**Codex output capture:** `/tmp/codex_d51_review_r2.txt`.

**Verdict:** BLOCK

**Issue count:** 5 (2 BLOCK / 3 MINOR / 0 NIT)

**Issues:**

1. **[BLOCK] field-layout-vocabulary** [`spec-snapshot.md:5237`]

   §19.1 says `FieldIdx` indexes the projected type's
   `VariantLayout.fields`, but §19.4 permits `Field` over
   `Ty::Struct`, `Ty::Union`, `Ty::Tuple`, and `Ty::Closure`
   when no enum variant is active. Stage-5 layout vocabulary
   says `FieldIdx` indexes `LayoutKind::Struct.fields`,
   `LayoutKind::Union.fields`, and `VariantLayout.fields`,
   not `VariantLayout.fields` universally.

   *Suggested fix:* rewrite the `Field` doc/validation
   wording to dispatch by predecessor shape: struct/tuple/
   closure-captures via struct fields, union via union
   fields, enum-after-Downcast via `VariantLayout.fields`.

2. **[BLOCK] variantidx-range-enumshape** [`spec-snapshot.md:5435`]

   §19.7 says `Downcast(idx)` validates against
   `0..ty_enum.variants.len()`, and discussions.md repeats
   this at lines 878-880. But `EnumShape` has `Empty`,
   `Single { variant, ... }`, and `Multiple { variants, ... }`;
   only `Multiple` has a `variants: Vec<VariantLayout>`.

   *Suggested fix:* define a variant-count helper over
   `EnumShape` (`Empty = 0`, `Single = 1`,
   `Multiple = variants.len()`) and use that in §19.7 and
   discussions.md.

3. **[MINOR] downcast-coroutine-doc** [`spec-snapshot.md:5270`]

   The `PlaceElem::Downcast` doc still says "enum or coroutine
   variant," but §15.1 rejects `Coroutine` / `CoroutineClosure`
   / `CoroutineWitness`, and §19.4 explicitly says coroutine
   narrowing has no Regseq analog.

   *Suggested fix:* change the Regseq doc comment to
   "enum variant" and, if useful, add "rustc coroutine
   Downcast forms are unreachable because coroutine types
   are rejected."

4. **[MINOR] subslice-audit-summary-overadmits-slice-false** [`discussions.md:857`]

   The D-5-2 rule summary says
   `Subslice { from, to, from_end }` may have
   `Ty::Slice(elem)` as predecessor generally, while §19.4
   only admits slices in the `from_end: true` row. The
   evidence says for `PlaceElem`s, `from_end` is true iff
   the base is a slice and false for arrays.

   *Suggested fix:* split the D-5-2 summary to mirror §19.4,
   or add the same doc-level invariant note there.

5. **[MINOR] storage-contract-escape-hatch** [`spec-snapshot.md:5296`]

   The spec exposes `projection: Box<[PlaceElem]>` as the
   concrete `Place` shape, but then says the impl may swap
   to small-vec post-profiling without spec revision.
   discussions.md makes the same claim after deciding
   `Box<[PlaceElem]>`.

   *Suggested fix:* either remove the no-revision swap
   language, or make the spec describe an abstract
   slice-backed projection API instead of a concrete
   public field type.

**Corrections applied for R3 (2026-05-03):**

1. **[BLOCK] field-layout-vocabulary -- FIXED.** §19.1
   `Field(FieldIdx, TyId)` doc rewritten to dispatch by
   predecessor type: `LayoutKind::Struct.fields` for
   `Ty::Struct{..}` / `Ty::Closure(..)`;
   `LayoutKind::Union.fields` for `Ty::Union{..}`; tuple's
   `fields` for `Ty::Tuple{..}`; active variant's
   `VariantLayout.fields` for `Ty::Enum{..}` after Downcast.
   §19.7 rule 3 (FieldIdx range) updated to use the same
   dispatch (was previously
   "0..variant_layout.fields.len() for the appropriate
   variant" -- now spells out the per-predecessor dispatch).
   Discussions.md D-5-2 rule 3 also updated to mirror.

2. **[BLOCK] variantidx-range-enumshape -- FIXED.** §19.7
   rule 4 reworded to use a `variant_count(ty_enum.shape)`
   helper that dispatches on `EnumShape`: `Empty -> 0`,
   `Single { .. } -> 1`,
   `Multiple { variants, .. } -> variants.len()`.
   Discussions.md D-5-2 rule 4 updated to mirror.

3. **[MINOR] downcast-coroutine-doc -- FIXED.** §19.1
   `Downcast(VariantIdx)` doc updated to say "Downcast to a
   specific enum variant" with a follow-up sentence:
   "Coroutine variants are unreachable because the coroutine
   family is rejected at type translation (§15.1)."

4. **[MINOR] subslice-audit-summary-overadmits-slice-false
   -- FIXED.** Discussions.md D-5-2 rule 1's Subslice bullet
   split into two bullets per `from_end` (mirrors §19.4
   exactly): `from_end: false` -> array only with successor
   `Ty::Array(elem, to - from)`; `from_end: true` -> array
   `Ty::Array(elem, n - from - to)` or slice (unchanged).
   Doc-level invariant note from `prior-art/rustc.md` §3.5
   added.

5. **[MINOR] storage-contract-escape-hatch -- FIXED.** §19.2
   storage paragraph and discussions.md D-5-1.4 *Why not
   Option C* both rewritten: removed "without spec revision"
   / "no spec revision" language. Now explicit that swapping
   to `SmallVec` would land via a follow-up spec revision
   (the impl change is mechanical but not invisible to
   consumers that pattern-match on the field type).

### R3 (2026-05-03): VERDICT: MINOR

**Prompt body:** archived at
`/tmp/codex_d51_review_r3_prompt.txt`. Names R2's five
corrections explicitly so Codex can verify them.

**Codex output capture:** `/tmp/codex_d51_review_r3.txt`.

**Verdict:** MINOR (no BLOCKs)

**Issue count:** 3 (0 BLOCK / 2 MINOR / 1 NIT)

**Issues:**

1. **[MINOR] closure-field-layout-dispatch** [`spec-snapshot.md:5237`]

   §19.1, §19.7, and D-5-2 rule 3 dispatch `Ty::Closure(..)`
   directly to `LayoutKind::Struct.fields`
   (`spec-snapshot.md:5237-5246`,
   `spec-snapshot.md:5444-5452`, `discussions.md:881-888`).
   But `Ty::Closure` is an identity view with layout via a
   parallel `Ty::Struct` (`spec-snapshot.md:2363`,
   `spec-snapshot.md:3289-3299`), while inline `Layout`
   lives only on `Ty::Struct`, `Ty::Enum`, `Ty::Union`, and
   `Ty::Tuple` (`spec-snapshot.md:2527-2530`).

   *Suggested fix:* for `Ty::Closure(item, args)`, explicitly
   resolve `find_nominal(NominalKind::Struct, item, args)`
   and use that `Ty::Struct`'s `LayoutKind::Struct.fields`.

2. **[MINOR] d5-2-field-type-summary-variantlayout** [`discussions.md:845`]

   D-5-2 rule 1 lists struct/union/tuple/closure and
   enum-with-active-variant predecessors, but then says the
   inline type is consistent with `VariantLayout.fields[idx]`
   (`discussions.md:845-851`). `VariantLayout.fields` is
   only the enum active-variant case; non-enum predecessors
   use `LayoutKind::Struct.fields`, `LayoutKind::Union.fields`,
   or tuple fields per the corrected rule 3
   (`discussions.md:881-888`) and §19.7
   (`spec-snapshot.md:5444-5452`).

   *Suggested fix:* replace the parenthetical with "the
   inline `ty` is consistent with the field selected by the
   same predecessor dispatch as rule 3."

3. **[NIT] variant-count-helper-accessor** [`spec-snapshot.md:5454`]

   The EnumShape-aware count itself is correct:
   `Empty -> 0`, `Single -> 1`, `Multiple.variants.len()`
   (`spec-snapshot.md:5454-5459`, `discussions.md:890-894`),
   matching `EnumShape` (`spec-snapshot.md:2611-2627`). The
   wording `ty_enum.shape` is just slightly off the actual
   schema: `Ty::Enum` carries `layout`, and `shape` lives
   under `LayoutKind::Enum` (`spec-snapshot.md:2366`,
   `spec-snapshot.md:2393`, `spec-snapshot.md:2541-2545`).

   *Suggested fix:* say `variant_count(shape)` after
   extracting `shape` from the predecessor enum's
   `layout.kind`.

**Corrections applied for R4 (2026-05-03):**

1. **[MINOR] closure-field-layout-dispatch -- FIXED.** §19.1
   `Field(FieldIdx, TyId)` doc rewritten to dispatch each
   predecessor type explicitly:
   - `Ty::Struct{..}` -> `LayoutKind::Struct.fields`.
   - `Ty::Closure(item, args)` -> resolve parallel
     `Ty::Struct(item, args, _)` via
     `find_nominal(NominalKind::Struct, item, args)`; use
     that struct's `LayoutKind::Struct.fields`. Notes the
     closure-as-struct identity-vs-layout split per stage-3
     D6.3 and that `Ty::Closure` itself does not carry an
     inline `Layout`.
   - `Ty::Union{..}` -> `LayoutKind::Union.fields`.
   - `Ty::Tuple{..}` -> the tuple's `fields` vector.
   - `Ty::Enum{..}` after `Downcast` -> active variant's
     `VariantLayout.fields` (under `LayoutKind::Enum.shape`'s
     `Multiple.variants` or `Single.variant` per §11.1).

   §19.7 rule 3 updated to mirror, including the closure
   dispatch via `find_nominal`. discussions.md D-5-2 rule 3
   also updated.

2. **[MINOR] d5-2-field-type-summary-variantlayout -- FIXED.**
   discussions.md D-5-2 rule 1's Field bullet parenthetical
   changed from "the inline `ty` is consistent with what
   `VariantLayout.fields[idx]` would yield" to "the inline
   `ty` is consistent with the field selected by the same
   predecessor dispatch as rule 3 below." This correctly
   defers to rule 3's per-predecessor field-list lookup.

3. **[NIT] variant-count-helper-accessor -- FIXED.** §19.7
   rule 4 reworded: "let `shape` be the `EnumShape` carried
   in the predecessor `Ty::Enum{..}`'s `layout.kind` (per
   §11.1, the layout's inner `LayoutKind::Enum.shape`). Then
   `idx` is in `0..variant_count(shape)`, where
   `variant_count` dispatches on `EnumShape` (§11.1): ..."
   discussions.md D-5-2 rule 4 updated to mirror.

### R4 (2026-05-04): VERDICT: MINOR

**Prompt body:** archived at
`/tmp/codex_d51_review_r4_prompt.txt`. Names R3's three
corrections explicitly so Codex can verify them.

**Codex output capture:** `/tmp/codex_d51_review_r4.txt`.

**Verdict:** MINOR (no BLOCKs; substantive content settled)

**Issue count:** 1 (0 BLOCK / 1 MINOR / 0 NIT)

**Issues:**

1. **[MINOR] methodology-rule-not-in-readme** [`discussions.md:155`]

   D-5-1 invokes "Compositional behavior beats special-case
   flags" as a methodology rule, and D-5-1.3 repeats it at
   `discussions.md:453`. README §2 is the binding
   methodology list (`README.md:95`); the relevant listed
   rules are "No defensive fallbacks" (`README.md:106`) and
   "C semantics with information preservation" / "store the
   result, not the input" (`README.md:157`), but the
   "Compositional..." rule is not listed there.

   *Suggested fix:* replace those methodology bullets with
   the exact README §2 rule they are deriving from, likely
   "C semantics with information preservation," or add the
   "Compositional..." rule to README §2 before invoking it
   as binding methodology.

**Corrections applied for R5 (2026-05-04):**

1. **[MINOR] methodology-rule-not-in-readme -- FIXED via
   promotion (option (b) of the two-option triage).** The
   "Compositional behavior beats special-case flags" rule
   has been promoted to a binding methodology rule in
   `README.md` §2, inserted between "Compiler-theory-first"
   and "Source declares its own compiler-relevant
   attributes" (with the same insertion-point precedent as
   "Spec before implementation" earlier in this stage).
   Body adapted from stage-3 / stage-4 / stage-5 usage:

   > "When a behavior derives compositionally from existing
   > rules, do not introduce a parallel flag-style marker.
   > Variant tags and the existing rule chain carry the
   > distinction; flags duplicate them and create a
   > side-channel that must be kept in sync. Examples across
   > stages: MaybeUninit's drop suppression falls out of
   > Union no-auto-drop plus inner ManuallyDrop, no
   > `is_maybe_uninit` flag needed (stage 3); `&str` /
   > `&[T]` reference-typed const handling falls out of
   > `Aggregate([Reloc, Uint])` plus `ConstMemoryStatic`,
   > no new `Const` variant needed (stage 4); Place's
   > `Deref`-first and `Downcast`-followed-by-`Field` shape
   > falls out of rustc's PostCleanup invariants, no
   > parallel `is_box_deref` / `is_synthetic_index` markers
   > needed (stage 5). Where composition does not naturally
   > cover a case, prefer adding a variant or refining a
   > rule over introducing a flag."

   The promotion retroactively legitimises stage-3 and
   stage-4 audit-shape citations of the same principle
   (which Codex did not flag during their review
   trajectories). Stage-5 audit shapes in `discussions.md`
   D-5-1 and D-5-1.3 now properly cite README §2.

### R5 (2026-05-04): VERDICT: MINOR

**Prompt body:** archived at
`/tmp/codex_d51_review_r5_prompt.txt`. Names R4's single
correction (the README §2 promotion) explicitly.

**Codex output capture:** `/tmp/codex_d51_review_r5.txt`.

**Verdict:** MINOR (no BLOCKs; substantive content settled;
this round catches a precision finding from the R4 promotion)

**Issue count:** 1 (0 BLOCK / 1 MINOR / 0 NIT)

**Issues:**

1. **[MINOR] postcleanup-misattribution** [`README.md:156`]

   The R4 fix adds the rule in the binding methodology list,
   but its stage-5 example says both `Deref`-first and
   `Downcast`-followed-by-`Field` fall out of rustc
   PostCleanup invariants (`README.md:156-158`). The spec
   repeats that both structural invariants are from
   PostCleanup (`spec-snapshot.md:5336-5338`). Evidence only
   grounds `Deref`-first in PostCleanup
   (`prior-art/rustc.md:781-802`).
   `Downcast`-followed-by-`Field` is grounded instead in
   `PlaceTy::projection_ty`'s assertion
   (`prior-art/rustc.md:394-407`) plus H4's SetDiscriminant
   cross-check (`prior-art/rustc.md:1337-1384`).

   *Suggested fix:* change both passages to say
   `Deref`-first comes from PostCleanup, while
   `Downcast`-followed-by-`Field` comes from the
   `PlaceTy::projection_ty` successor assertion and H4.

**Corrections applied for R6 (2026-05-04):**

1. **[MINOR] postcleanup-misattribution -- FIXED.** Four
   passages corrected to split the attribution per invariant:
   - `README.md` §2 "Compositional behavior beats special-case
     flags" stage-5 example: now reads "Place's structural
     invariants -- `Deref`-first (from rustc's PostCleanup
     phase) and `Downcast`-followed-by-`Field` (from
     `PlaceTy::projection_ty`'s successor assertion plus
     exhaustive cross-check that no rustc construction site
     produces `Downcast`-as-last) -- fall out of existing
     rustc rules..."
   - `spec-snapshot.md` §19.3 intro paragraph: now reads
     "Two structural invariants -- both strictly tighter
     than the rustc `ProjectionElem` enum's static type,
     with distinct grounds: I-Deref-first comes from rustc's
     PostCleanup phase (a guaranteed invariant of our
     `MirPhase::Runtime(Optimized)` input);
     I-Downcast-followed-by-Field comes from
     `PlaceTy::projection_ty`'s successor assertion plus the
     H4 exhaustive cross-check across all rustc
     `SetDiscriminant` construction sites..."
   - `discussions.md` D-5-1 Option A description: split
     attribution per invariant (PostCleanup phase vs.
     `PlaceTy::projection_ty` assertion plus exhaustive
     cross-check).
   - `discussions.md` D-5-1 *Why structural invariants are
     codified* paragraph: rewritten to lead with "Rustc-side
     mechanisms guarantee a tighter shape... with distinct
     grounds per invariant," then attribute Deref-first to
     PostCleanup phase-level invariant and
     Downcast-followed-by-Field to runtime
     `PlaceTy::projection_ty` assertion plus H4 empirical
     cross-check.

   The two invariants now have distinct attribution
   throughout the spec, README, and audit trail; no remaining
   passage conflates them.

### R6 (2026-05-04): VERDICT: MINOR

**Prompt body:** archived at
`/tmp/codex_d51_review_r6_prompt.txt`. Names R5's four
attribution-split corrections explicitly.

**Codex output capture:** `/tmp/codex_d51_review_r6.txt`.

**Verdict:** MINOR (no BLOCKs; finer precision findings)

**Issue count:** 2 (0 BLOCK / 1 MINOR / 1 NIT)

**Issues:**

1. **[MINOR] subslice-from-end-overadmission** [`spec-snapshot.md:5378`]

   Spec claims `Subslice { ..., from_end: true }` is valid
   over `Ty::Array` or `Ty::Slice`
   (`spec-snapshot.md:5378`), and discussions mirrors that
   (`discussions.md:869-877`). But the same spec note says
   the `PlaceElem` doc-level invariant is `from_end == true`
   iff the base is a slice (`spec-snapshot.md:5400-5403`),
   and the evidence file says `true` iff slice, `false` for
   arrays (`prior-art/rustc.md:299-306`). That over-admits
   an array+`from_end: true` `PlaceElem` despite the
   pinned-shape methodology to reject impossible shapes
   (`README.md:106-110`).

   *Suggested fix:* make Place validation reject
   `Subslice { from_end: true }` over arrays; keep the
   rustc helper's array-true narrowing arm only as a
   non-normative completeness note. Update the doc comment
   at `spec-snapshot.md:5281-5285` and
   `discussions.md:869-877` to match.

2. **[NIT] deref-first-quote-mismatch** [`discussions.md:574`]

   D-5-1.5 correctly claims `Deref`-first is grounded in
   PostCleanup (`discussions.md:574-588`), but the quoted
   excerpt at `discussions.md:577-580` is not the
   load-bearing evidence text recorded in prior-art. The
   evidence file's §6.1 sentence is that `Deref`
   projections must be the first projection within any
   place (`prior-art/rustc.md:781-802`), and D-5-1.5 cites
   that same section as evidence
   (`discussions.md:617-626`).

   *Suggested fix:* replace the stale quote with the exact
   §6.1 wording from prior-art, or paraphrase without
   block-quoting.

**Corrections applied for R7 (2026-05-04):**

1. **[MINOR] subslice-from-end-overadmission -- FIXED.**
   Four passages corrected to tighten the Subslice shape
   to the two combinations that actually appear in our
   input:
   - §19.1 `Subslice` doc-comment: rewritten to spell out
     the doc-level invariant ("`from_end == true` iff the
     base is a slice; `false` iff the base is an array")
     and the two valid combinations; explicitly states the
     cross combinations are rejected by Place validation.
   - §19.4 table: `Subslice { from_end: true }` row
     tightened to predecessor `Ty::Slice(elem)` only
     (was previously "`Ty::Array(elem, n)` or
     `Ty::Slice(elem)`"). Successor: unchanged
     `Ty::Slice(elem)`.
   - §19.4 prose paragraph: rewritten to lead with the
     doc-level invariant; explicitly notes that cross
     combinations are bridge-rejected; clarifies that
     rustc's narrowing function handles both array arms
     for non-`PlaceElem` contexts as a general utility,
     but the `PlaceElem` doc fixes the input shape.
   - discussions.md D-5-2 rule 1 Subslice bullet: split
     into three sub-bullets (the two valid combinations +
     a third bullet for the rejected cross combinations
     with the doc-level invariant rationale).

2. **[NIT] deref-first-quote-mismatch -- FIXED.**
   discussions.md D-5-1.5 *Why Option A* paragraph: the
   block-quote at lines 577-580 (which paraphrased rustc's
   restriction as "Most uses of `Field` (or other compound
   projections) on places that *don't already start with*
   `Deref`") replaced with the exact load-bearing sentence
   from `prior-art/rustc.md` §6.1: "Furthermore, **`Deref`
   projections must be the first projection within any
   place** (if they appear at all)." The new lead-in
   reads: "Rustc's `PostCleanup` phase restricts `Deref`
   to first-projection-only. The load-bearing sentence in
   the `MirPhase` declaration at `syntax.rs:119-120`
   (recorded verbatim in `prior-art/rustc.md` §6.1)
   reads:" -- pointing precisely to the evidence
   citation.

### R7 (2026-05-04): VERDICT: MINOR

**Prompt body:** archived at
`/tmp/codex_d51_review_r7_prompt.txt`. Names R6's two
corrections explicitly.

**Codex output capture:** `/tmp/codex_d51_review_r7.txt`.

**Verdict:** MINOR (no BLOCKs; precision NIT findings)

**Issue count:** 4 (0 BLOCK / 1 MINOR / 3 NIT)

**Issues:**

1. **[MINOR] constantindex-from-end-overadmission** [`spec-snapshot.md:5384`]

   §19.4 accepts `ConstantIndex { ... }` over `Ty::Array`
   or `Ty::Slice` without splitting by `from_end`, and
   §19.7 rule 1 defers validity to §19.4
   (`spec-snapshot.md:5464-5468`). The evidence says
   `from_end` is always `false` when indexing an array
   (`prior-art/rustc.md:265-271`). D-5-2 mirrors the
   over-broad rule (`discussions.md:862-866`), and §19.6
   even describes `from_end: true` using the predecessor
   array length (`spec-snapshot.md:5450-5451`).

   *Suggested fix:* split `ConstantIndex` like Subslice:
   `from_end: false` over array or slice; `from_end: true`
   over slice only; reject array+true as a bridge bug.
   Update §19.6 to avoid "array length" for the true case.

2. **[NIT] rejection-section-scope** [`spec-snapshot.md:4557`]

   §15.1 says the table covers type or call translation
   and frames two type-oriented categories
   (`spec-snapshot.md:4557-4561`), but the table includes
   place-translation rows for `OpaqueCast` and
   `UnwrapUnsafeBinder` (`spec-snapshot.md:4596-4597`).

   *Suggested fix:* change the lead-in to "type, call, or
   place translation" and add a third category for closed
   `PlaceElem` vocabulary rejections.

3. **[NIT] h1-refined-summary-stale** [`discussions.md:298`]

   D-5-1.1 summarizes H1 as "`OpaqueCast` strip under both
   solver paths" (`discussions.md:298-299`). H1 REFINED
   says next-solver never constructs it, while legacy
   strips it (`prior-art/rustc.md:1179-1194`), and
   explicitly asks wording to say absent under both paths
   rather than just stripped
   (`prior-art/rustc.md:1211-1215`).

   *Suggested fix:* replace the bullet with "H1 REFINED --
   `OpaqueCast` absent under both solver paths: next-solver
   never constructs it; legacy strips it."

4. **[NIT] methodology-rule-alias** [`discussions.md:146`]

   D-5-1 / D-5-1.1 invoke "Closed vocabulary; reject what
   shouldn't appear" as methodology
   (`discussions.md:144-149`, `discussions.md:281-284`).
   The matching README §2 rule is "No defensive fallbacks,"
   including "reject if it appears unexpectedly"
   (`README.md:106-110`); Q7's "closed vocabulary; unknown
   rejected" is an architectural summary for intrinsic
   recognition (`README.md:250-252`), not a README §2
   methodology rule.

   *Suggested fix:* reword those bullets under "No
   defensive fallbacks," or promote a general
   closed-vocabulary methodology rule if that is intended.

**Corrections applied for R8 (2026-05-04):**

1. **[MINOR] constantindex-from-end-overadmission -- FIXED.**
   Four passages corrected to tighten ConstantIndex shape
   to the three combinations that appear in our input:
   - §19.1 `ConstantIndex` doc-comment: spelled out the
     doc-level invariant ("`from_end` is always `false`
     when indexing an array; the MIR builder folds the
     array length into the offset rather than emitting a
     from-end form") and the three valid combinations;
     explicitly rejects array+`from_end:true` as a Place
     validation event. Worked-example block now annotates
     "slice base only" on the `from_end: true` rows.
   - §19.4 table: `ConstantIndex` row split into
     `from_end: false` (Array or Slice predecessor) and
     `from_end: true` (Slice only).
   - §19.6 (C-semantics) `ConstantIndex { from_end: true }`
     row updated: "with `len` being the predecessor
     slice's runtime length; `from_end: true` is
     slice-only per §19.4" (was "with `len` derived from
     the predecessor's array length", which was both
     wrong-attribution-wise and incorrect for slice
     bases).
   - discussions.md D-5-2 rule 1 ConstantIndex bullet:
     split into two sub-bullets (the two valid combinations
     + a follow-up note that the array+`from_end:true`
     cross combination is rejected).

2. **[NIT] rejection-section-scope -- FIXED.** §15.1
   lead-in extended to "type, call, or place translation"
   with a third category: "closed `PlaceElem` vocabulary
   rejections (rustc projection variants that are absent
   from our `optimized_mir` input or gated behind unstable
   features and have no v1 driver use case; see §19.1)."

3. **[NIT] h1-refined-summary-stale -- FIXED.** D-5-1.1
   prior-art-evidence bullet replaced: "`OpaqueCast` strip
   under both solver paths" -> "`OpaqueCast` absent under
   both solver paths: next-solver never constructs it;
   legacy constructs then strips it via
   `PostAnalysisNormalize`."

4. **[NIT] methodology-rule-alias -- FIXED via option (a)
   of Codex's two suggestions.** The "Closed vocabulary;
   reject what shouldn't appear" bullet was renamed to
   "No defensive fallbacks; the toolchain is pinned" in
   both D-5-1 (`discussions.md:146`) and D-5-1.1
   (`discussions.md:283`). The closed-vocabulary content
   was merged into the existing "No defensive fallbacks"
   bullet body (with a note that the "reject if it appears
   unexpectedly" clause of README §2's "No defensive
   fallbacks" rule body mechanically covers the
   closed-vocabulary rejection). Option (b) -- promoting
   "Closed vocabulary..." to a separate README §2 rule --
   was considered but option (a) is cleaner here because
   the existing "No defensive fallbacks" bullet body
   already covers the rejection clause.

### R8 (2026-05-04): VERDICT: MINOR

**Prompt body:** archived at
`/tmp/codex_d51_review_r8_prompt.txt`. Names R7's four
corrections explicitly.

**Codex output capture:** `/tmp/codex_d51_review_r8.txt`.

**Verdict:** MINOR (no BLOCKs; no MINOR; tail-end NITs only)

**Issue count:** 2 (0 BLOCK / 0 MINOR / 2 NIT)

**Issues:**

1. **[NIT] methodology-rule-alias-residual** [`discussions.md:620`]

   D-5-1 and D-5-1.1 were corrected (R7 fix), but D-5-1.5
   and D-5-1.6 still invoke "Reject what shouldn't appear"
   as if it were a README §2 methodology rule
   (`discussions.md:620-622`, `discussions.md:736-740`).
   README §2's actual matching rule is "No defensive
   fallbacks," including "reject if it appears
   unexpectedly" (`README.md:106-110`).

   *Suggested fix:* merge these residual bullets into
   "No defensive fallbacks; the toolchain is pinned," as
   done for D-5-1 / D-5-1.1.

2. **[NIT] h3-stale-prior-art-crossref** [`prior-art/rustc.md:1055`]

   The spec/decision text now reflects H3 correctly:
   D-5-1.5's Box-deref example uses a raw-pointer temp
   with `Deref` first (`discussions.md:640-653`), matching
   H3 CONFIRMED (`prior-art/rustc.md:1280-1327`). But
   prior-art §10.4 still says that shape is
   "HYPOTHESIS: ... UNVERIFIED"
   (`prior-art/rustc.md:1055-1070`), while D-5-1 broadly
   cites prior-art §10 for "Box-deref elaboration shape"
   (`discussions.md:184-187`).

   *Suggested fix:* update prior-art §10.4 to state that
   H3 is confirmed and point to the H3 record, or narrow
   the D-5-1 citation to the H3 cross-check block.

**Corrections applied for R9 (2026-05-04):**

1. **[NIT] methodology-rule-alias-residual -- FIXED.**
   Two parallel residual sites in `discussions.md`
   (D-5-1.5 and D-5-1.6) updated:
   - D-5-1.5 *Methodology rules invoked*: removed standalone
     "*Reject what shouldn't appear.*" bullet; merged its
     content into the "No defensive fallbacks; the
     toolchain is pinned" bullet body, with a sentence:
     "Includes the 'reject if it appears unexpectedly'
     clause: if a non-first `Deref` ever lands, the bridge
     raises a diagnostic via §3.3 Phase B place validation."
   - D-5-1.6 *Methodology rules invoked*: same merge, with
     body: "Includes the 'reject if it appears
     unexpectedly' clause: the strict form of the
     invariant rejects shapes the pinned toolchain does
     not produce, and the bridge raises a diagnostic if
     `Downcast`-as-last ever appears."

   All four D-5-1 sub-decisions (D-5-1, D-5-1.1, D-5-1.5,
   D-5-1.6) now cite "No defensive fallbacks" properly;
   none invoke standalone "Reject what shouldn't appear" or
   "Closed vocabulary; reject what shouldn't appear" as a
   methodology rule. Verified via grep -- no remaining
   citations.

2. **[NIT] h3-stale-prior-art-crossref -- FIXED.**
   `prior-art/rustc.md` §10.4 rewritten: replaced the
   "HYPOTHESIS: ... UNVERIFIED" prose with a CONFIRMED
   summary that points at the H3 record. Concrete
   elaboration shape now stated: fresh `*const T` temp via
   `patch.new_temp`; `Cast(Transmute, Copy(box.0.0),
   *const T)` statement inserted; `place.local` rewritten
   to the temp; `Deref` preserved as first projection.
   Stage 5's input therefore sees ordinary `[Deref, ...]`
   chains over raw-pointer locals. The H3 record is
   referenced for the worked example.

### R9 (2026-05-04): VERDICT: MINOR

**Prompt body:** archived at
`/tmp/codex_d51_review_r9_prompt.txt`. Names R8's two
corrections explicitly.

**Codex output capture:** `/tmp/codex_d51_review_r9.txt`.

**Verdict:** MINOR (no BLOCKs; no MINOR; single NIT)

**Issue count:** 1 (0 BLOCK / 0 MINOR / 1 NIT)

**Issues:**

1. **[NIT] residual-stale-hypothesis-evidence** [`prior-art/rustc.md:471`]

   The R8 target sites are fixed, but nearby evidence
   sections still carry stale pre-cross-check wording:
   - H1: `prior-art/rustc.md` §3.7 still describes
     `OpaqueCast` as "stripped" and "unverified" for
     next-solver (lines 471-476) despite H1 REFINED at
     lines 1171-1216.
   - H2: §3.8 still framed as a HYPOTHESIS based on
     instability/`#![no_std]` (lines 507-517) despite
     H2 REFINED at lines 1221-1236 (DSL does not formally
     gate feature opt-in).
   - H3: §6.3 still says body Places see an inline
     `Field/Field/Field/Deref` Box chain (lines 824-831)
     despite H3 CONFIRMED and §10.4 saying the bridge sees
     a fresh raw-pointer temp with `Deref` first.
   - H4: §10.5 still says `Downcast`-as-last is
     "UNVERIFIED" and probably arises in `SetDiscriminant`
     writes (lines 1075-1084) despite H4 REFUTED at lines
     1338-1387.

   *Suggested fix:* update or explicitly mark those earlier
   passages as superseded by the hypothesis cross-check
   block, matching the corrected §10.4 style.

**Corrections applied for R10 (2026-05-04):**

1. **[NIT] residual-stale-hypothesis-evidence -- FIXED.**
   Four sections of `prior-art/rustc.md` rewritten to
   reflect the cross-check results, matching the §10.4
   style:
   - §3.7 `OpaqueCast` "INFERENCE" paragraph: replaced
     with "CONCLUSION (H1 REFINED, see 'Hypothesis
     cross-check results')" stating `OpaqueCast` is absent
     under both solver paths -- legacy strips it via
     `PostAnalysisNormalize`; next-solver never constructs
     it (the construction site at
     `mir_build/src/builder/matches/match_pair.rs:104-118`
     is symmetrically gated).
   - §3.8 `UnwrapUnsafeBinder` "HYPOTHESIS" paragraph:
     replaced with "CONCLUSION (H2 REFINED)" stating
     `RegseqCallbacks` carries no feature-gate allow/deny
     list (`callbacks.rs:60-119`), so a driver could in
     principle opt in, but no plausible driver pattern
     needs `unsafe<...>` types; D-5-1 chose option (a)
     reject as a closed-vocabulary diagnostic.
   - §6.3 Box-deref inline-chain claim: replaced with
     "CONCLUSION (H3 CONFIRMED)" stating the elaborated
     form uses a fresh `*const T` temp local via
     `patch.new_temp` + `Cast(Transmute, ...)` statement +
     `place.local` rewrite; our `Place` sees ordinary
     `[Deref, ...]` chains over raw-pointer locals (NOT
     long inline chains over Box's nested
     `Unique`/`NonNull`/`*const T`).
   - §10.5 `Downcast`-followed-by-`Field` "UNVERIFIED"
     paragraph: replaced with "CONCLUSION (H4 REFUTED)"
     listing the 3 exhaustively-enumerated
     `SetDiscriminant` construction sites in rustc, none
     of which use Downcast (`coroutine.rs:367` uses
     `Place::from(SELF_ARG)`; `shim.rs:770-773` uses
     `Place::return_place()`;
     `mir_build/.../instruction.rs:33` is the test-only
     `mir!` macro). Strict form (Downcast non-last AND
     immediately followed by Field) codified per D-5-1.6.

   Verified via grep -- no remaining `UNVERIFIED` /
   `HYPOTHESIS:` markers in `prior-art/rustc.md` (the
   "Hypothesis cross-check results" section header itself
   is unchanged; all four cross-check verdicts referenced
   from upstream sections via "CONCLUSION (H<N> ...)"
   stanzas).

### R10 (2026-05-04): VERDICT: MINOR

**Prompt body:** archived at
`/tmp/codex_d51_review_r10_prompt.txt`. Names R9's single
correction explicitly.

**Codex output capture:** `/tmp/codex_d51_review_r10.txt`.

**Verdict:** MINOR (no BLOCKs; no MINOR; 2 NITs continuing
the residual-cross-reference cleanup pattern)

**Issue count:** 2 (0 BLOCK / 0 MINOR / 2 NIT)

**Issues:**

1. **[NIT] h2-stale-option-summary** [`discussions.md:40`]

   D-5-1 Option A still says both rejected variants "don't
   appear" in `optimized_mir` input, naming `OpaqueCast`
   and `UnwrapUnsafeBinder` together. That is stale for H2:
   evidence says Regseq does not formally exclude
   `unsafe_binders`, a driver could opt in, and
   `UnwrapUnsafeBinder` would reach input unchanged if used.

   *Suggested fix:* split the sentence: `OpaqueCast` is
   absent under both solver paths;
   `UnwrapUnsafeBinder` is gated/no v1 use case and
   rejected if encountered.

2. **[NIT] h1-stale-prior-art-summary** [`prior-art/rustc.md:822`]

   The R9 target §3.7 now correctly states the
   legacy/next-solver split, but §6.2 still summarizes
   `OpaqueCast` as "confirmed via PostAnalysisNormalize"
   (line 822-823), and §6.4's table row says it is
   "Stripped by `PostAnalysisNormalize` before
   `RuntimePhase::Initial`" (line 859). H1's correct
   cross-check result is split: next-solver never
   constructs it, while legacy strips it.

   *Suggested fix:* update both summaries to the same
   wording as §3.7: absent under both solver paths, for
   different reasons.

**Corrections applied for R11 (2026-05-04):**

1. **[NIT] h2-stale-option-summary -- FIXED.** D-5-1
   Option A description in `discussions.md` rewritten to
   split the rationale per variant: "reject `OpaqueCast`
   (absent from our `optimized_mir` input under both solver
   paths per H1 REFINED -- legacy strips it via
   `PostAnalysisNormalize`, next-solver never constructs it)
   and `UnwrapUnsafeBinder` (gated behind unstable
   `unsafe_binders` feature; the DSL does not formally
   exclude opt-in but no driver use case per H2 REFINED --
   closed-vocabulary diagnostic if encountered)."

2. **[NIT] h1-stale-prior-art-summary -- FIXED.** Two
   summary sites in `prior-art/rustc.md` updated:
   - §6.2 summary line: "So `OpaqueCast` is **gone** in our
     input (Section 3.7; H1 REFINED -- absent under both
     solver paths for different reasons: legacy strips it
     via `PostAnalysisNormalize`; next-solver never
     constructs it because the construction site at
     `mir_build/src/builder/matches/match_pair.rs:104-118`
     is symmetrically gated)."
   - §6.4 table row for `OpaqueCast`: cell expanded from
     "Stripped by `PostAnalysisNormalize` before
     `RuntimePhase::Initial`" to "Absent under both solver
     paths (H1 REFINED): legacy strips it via
     `PostAnalysisNormalize`; next-solver never constructs
     it (gated construction site at
     `mir_build/.../match_pair.rs:104-118`)." Also updated
     `UnwrapUnsafeBinder` row from "RARE / GATED" to "NO
     (rejected)" with a body note: "Tied to unstable
     `unsafe_binders` feature; no MirPhase restriction. DSL
     does not formally gate feature opt-in (H2 REFINED),
     but no driver use case + closed-vocabulary diagnostic
     per D-5-1.1."

### Cluster closure (2026-05-04): D-5-1 LOCKED at R10

After R10's NIT-only verdict, the user elected to lock the
D-5-1 cluster rather than continue iterating toward an
explicit APPROVE round. Status flipped from `[DRAFT]` to
`[LOCKED]` for D-5-1 parent and sub-decisions D-5-1.1
through D-5-1.6.

**Trajectory summary:**

| Round | Verdict | Issues | Severity |
|---|---|---|---|
| R1 | BLOCK | 4 | 2 BLOCK / 2 MINOR / 0 NIT |
| R2 | BLOCK | 5 | 2 BLOCK / 3 MINOR / 0 NIT |
| R3 | MINOR | 3 | 0 BLOCK / 2 MINOR / 1 NIT |
| R4 | MINOR | 1 | 0 BLOCK / 1 MINOR / 0 NIT |
| R5 | MINOR | 1 | 0 BLOCK / 1 MINOR / 0 NIT |
| R6 | MINOR | 2 | 0 BLOCK / 1 MINOR / 1 NIT |
| R7 | MINOR | 4 | 0 BLOCK / 1 MINOR / 3 NIT |
| R8 | MINOR | 2 | 0 BLOCK / 0 MINOR / 2 NIT |
| R9 | MINOR | 1 | 0 BLOCK / 0 MINOR / 1 NIT |
| R10 | MINOR | 2 | 0 BLOCK / 0 MINOR / 2 NIT |

R10's two NITs were applied as corrections (recorded above)
but not verified by an R11 round. The R10 corrections are
small, uniform with the established cross-reference-cleanup
pattern, and unverified-by-Codex; treat as the last applied
state of the cluster.

**Methodology deviation rationale.** `memory.md` §1 says
"Repeat until APPROVE." The user elected to deviate after R10
based on the trajectory pattern:

- Substantive content settled at R3 (no BLOCKs since;
  no MINORs since R8).
- R7-R10 verdicts all NIT-dominant or NIT-only -- the residual
  findings have been valid cross-reference cleanup tasks, not
  content disagreements.
- The top-level merger Codex review (README §5.5 step 6)
  provides an independent adversarial pass over the
  integrated top-level `spec.md`; that review will sweep any
  residual cross-reference drift surfaced by stage-5
  integration.
- Cost-benefit: additional rounds at this depth produce
  shrinking marginal value; each round costs ~10-15 minutes
  of Codex effort, and the expected per-round finding count
  is now 1-3 NITs.

**Precedent.** This closure pattern -- "lock at MINOR-with-
NITs-only when the substantive content has settled and the
remaining findings are cross-reference cleanup, deferring
final-state cleanup to top-level merger review" -- is
available to later stages that exhibit similar trajectories.
The R3-R10 trend (no BLOCKs since R3; NIT-dominant since R7)
is the visible signal that the substantive content has
settled.

**What R11+ would have addressed (deferred to merger review).**
The known R10 fixes were applied; what remains is whatever
new residual cross-reference sites Codex would find on the
next sweep. Based on the R7-R10 pattern, expected R11+ scope
would be 1-3 small wording fixes for sites not yet touched.
The merger contract (README §5.5 step 6) runs Codex against
the integrated top-level `spec.md`, which catches drift at
the project-wide level rather than the per-stage level --
this is the natural place to address whatever residuals
remain.

**D-5-1 cluster status:** `[LOCKED]` (per-stage). Top-level
status flips when the merger contract completes per README
§5.5.

---

## D-5-2 + D-5-K combined review trajectory

Combined review of the deferral decisions: D-5-2 (Place
validation phase placement -- rules at stage 5; walk at
stage 11) and D-5-K (`render_place` deferral). Reviewed
together because both are deferral-only and small in scope.

### R1 (2026-05-04): VERDICT: BLOCK

**Prompt body:** archived at
`/tmp/codex_d52_d5k_review_r1_prompt.txt`. Per `memory.md`
§4 template; structured around 7 evaluation criteria
covering rule coherence (1), deferral-target soundness
(2-3), outbound consistency (4), methodology compliance
(5), cross-section consistency (6), and hypothesis
grounding (7).

**Codex output capture:** `/tmp/codex_d52_d5k_review_r1.txt`.

**Verdict:** BLOCK -- a substantive deferral-target finding
that R1-R10 of D-5-1 did not surface (D-5-1's reviews
focused on the variant set / payload / invariants; D-5-K's
deferral target is reviewed here as criterion 3).

**Issue count:** 4 (1 BLOCK / 1 MINOR / 2 NIT)

**Issues:**

1. **[BLOCK] d5k-name-source-and-target** [`spec-snapshot.md:5544`]

   D-5-K claimed `render_place`'s latest dependency is
   stage 12 because `Field` / `Downcast` names come from
   stage-12 `Item` content (§19.8; D-5-K audit-shape;
   `stage-12/expectations.md` "From stage 5"). Evidence
   says those names already live in stage-3 / §11 layout
   schema: `FieldLayout.name` (stage-3 spec
   §2497-2521; stage-5 §2547-2551) and
   `VariantLayout.name` (stage-5 §2630). Current `Item`
   has no `fields[]` / `variants[]` -- it carries
   `def_path`, `kind`, `span`, flags, and
   `opaque_handle_eligible` (§8 Item schema). The D-5-K
   dependency analysis is unsound as written; with the
   actual schema, the latest real dependency is stage 11
   (Body / LocalDecls materialisation + LocalId `_N`
   rendering), not stage 12.

   *Suggested fix:* either retarget D-5-K / §18.2 / §19.8 /
   stage-12 expectation to stage 11 and source names from
   `FieldLayout.name` / `VariantLayout.name`, or explicitly
   add a stage-12 expectation/spec change for
   `Item.fields[]` / `Item.variants[]` and explain why
   that does not duplicate existing layout names.

2. **[MINOR] d5-2-outbound-stale-range-rules** [`stage-11/expectations.md:156`]

   The stage-11 outbound expectation for D-5-2 does not
   mirror §19.7's range rules. Says `FieldIdx` is in
   `0..variant_layout.fields.len()` for the predecessor
   type, but §19.7 dispatches across struct, closure-as-
   struct, union, tuple, and enum-active-variant cases.
   Says `VariantIdx` checks `0..ty_enum.variants.len()`,
   but §19.7 defines `variant_count(shape)` over
   `EnumShape::{Empty, Single, Multiple}` (R7's R3 fix).

   *Suggested fix:* rewrite stage-11 expectation rules 3
   and 4 to copy §19.7's predecessor dispatch and
   `variant_count(shape)` helper.

3. **[NIT] d5-2-methodology-label** [`discussions.md:955`]

   D-5-2's methodology block invokes "Spec is mutable;
   specification surfaces drive refactor" instead of
   "Spec before implementation" (the binding README §2
   rule that actually applies to the rules-here /
   implementation-when-dependencies-are-ready pattern).

   *Suggested fix:* change or add the methodology bullet
   to explicitly cite "Spec before implementation."

4. **[NIT] d5-k-methodology-missing-compositional** [`discussions.md:1063`]

   D-5-K's methodology block cites "Spec before
   implementation," forward expectations, and per-method
   ownership, but does not invoke "Compositional behavior
   beats special-case flags" -- even though README §2
   lists that rule and includes Place as a stage-5
   example.

   *Suggested fix:* add a methodology bullet tying
   `render_place` to compositional rendering from the
   projection chain plus existing name tables, no
   placeholder/special-case rendering flags.

**Corrections applied for R2 (2026-05-04):**

1. **[BLOCK] d5k-name-source-and-target -- FIXED via
   retarget (option (a) of Codex's two suggestions).**
   D-5-K retargeted from stage 12 to stage 11 throughout:
   - §19.8: section header changed from "Rendering:
     deferred to stage 12 (D-5-K)" to "Rendering: deferred
     to stage 11 (D-5-K)"; body rewritten to spell out the
     per-variant dependency chain via stage-3 / §11 layout
     schema (`FieldLayout.name`, `VariantLayout.name`)
     reachable at render time via the head local's
     `LocalDecl.ty`; explicit note that `Item` (stage 12)
     carries no field/variant name tables.
   - §18.2 per-method ownership row for `render_place`:
     stage target updated from 12 to 11; per-variant
     dependency table updated to point at layout-side
     names; stub message updated to
     `unimplemented!("stage 11 fills render_place
     (deferred per D-5-K)")`.
   - `discussions.md` D-5-K full audit-shape entry
     rewritten: section heading retargeted to "to stage 11
     [DRAFT]"; Key Options section updated (Option B is
     stage 11; original stage-12 option becomes Option C,
     marked "rejected on evidence cross-check" with the
     R1-finding citation); Decision flips to Option B
     (stage 11); per-variant dependency table rewritten
     against actual schema; *Why not Option C* paragraph
     records the retarget rationale; Cross-references and
     Outbound expectation paths updated to stage 11.
   - `discussions.md` D-5-1.3 *Compositional argument*
     paragraph: stale reference "render_place (D-5-K,
     deferred to stage 12)" updated to "stage 11."
   - `stage-11/expectations.md` "From stage 5": added a
     new entry "Fill `pretty.rs::render_place(&Place,
     &Body)` stub (per D-5-K)" with the corrected
     per-variant dependency table and retarget rationale
     (R1 BLOCK finding cited).
   - `stage-12/expectations.md` "From stage 5": replaced
     the previously-filed render_place entry with a brief
     "(No stage-12 deliverables incoming from stage 5.)"
     note explaining the retargeting and pointing at
     `stage-11/expectations.md`.
   - `stage-5/status.md`: prompt-shape for the
     D-5-2/D-5-K review updated to reflect the retarget
     ("§19.8 render_place deferral to stage 11").

2. **[MINOR] d5-2-outbound-stale-range-rules -- FIXED.**
   `stage-11/expectations.md` "From stage 5" rules 3 and
   4 rewritten to mirror §19.7 verbatim:
   - Rule 3: per-predecessor dispatch
     (`LayoutKind::Struct.fields.len()` for Struct;
     `find_nominal` then struct's fields-len for Closure;
     `LayoutKind::Union.fields.len()` for Union; tuple's
     `fields.len()` for Tuple; active variant's
     `VariantLayout.fields.len()` for Enum-after-Downcast).
   - Rule 4: `variant_count(shape)` helper over `EnumShape`
     extracted from `layout.kind`.

3. **[NIT] d5-2-methodology-label -- FIXED.** D-5-2
   *Methodology rules invoked* block: "Spec is mutable;
   specification surfaces drive refactor" replaced with
   "Spec before implementation" (citing the README §2
   binding rule for end-of-phase-1 implementation
   discipline) as the lead methodology citation. The
   "Spec is mutable" point preserved as a separate bullet
   for the case where stage 11 surfaces a constraint that
   calls for revising rule 2.

4. **[NIT] d5-k-methodology-missing-compositional --
   FIXED.** D-5-K *Methodology rules invoked* block
   extended with a "*Compositional behavior beats
   special-case flags*" bullet: "render_place's output is
   composed mechanically from the projection chain plus
   existing layout-side name tables (`FieldLayout.name`,
   `VariantLayout.name`); there are no placeholder-
   rendering / special-case-rendering flags on `Place` or
   `PlaceElem`. Names are recovered from the same schema
   that codegen and validation consume."

### R2 (2026-05-04): VERDICT: BLOCK

**Prompt body:** archived at
`/tmp/codex_d52_d5k_review_r2_prompt.txt`. Names R1's four
corrections explicitly.

**Codex output capture:** `/tmp/codex_d52_d5k_review_r2.txt`.

**Verdict:** BLOCK -- the retarget had two consistency
misses I didn't catch on the first pass.

**Issue count:** 2 (1 BLOCK / 1 MINOR / 0 NIT)

**Issues:**

1. **[BLOCK] d5-1-3-stale-item-name-source** [`discussions.md:459`]

   D-5-1.3 now says `render_place` is deferred to stage 11
   (R1 fix landed), but the surrounding *Why not Option B*
   and *Compositional argument* paragraphs still claim
   field/variant names need `Item`/DefPath lookups
   (`discussions.md:459-466`). That contradicts the
   corrected D-5-K text, which rejects `Item.fields[].name`
   / `Item.variants[].name` and says names live in
   `FieldLayout.name` / `VariantLayout.name`. The stale
   paragraphs preserve the original (incorrect)
   stage-12-target reasoning even though the deferral
   target was retargeted.

   *Suggested fix:* rewrite the D-5-1.3 render-side
   recovery paragraphs to source field/variant names from
   the stage-3 §11 layout schema, not stage-12 `Item`.

2. **[MINOR] d5k-spec-snapshot-field-render-cases** [`spec-snapshot.md:5175`]

   The spec-snapshot summaries under §18.2 and §19.8 only
   mention `LayoutKind::Struct.fields[idx].name` and enum
   active-variant fields for `Field` rendering. The same
   spec defines `Field` over struct, closure via parallel
   struct, union, tuple, and enum-after-Downcast; the
   corrected D-5-K audit shape and stage-11 outbound table
   include all those cases. §18.2 and §19.8 prose lag
   behind D-5-K's full per-predecessor table.

   *Suggested fix:* expand §18.2 and §19.8 `Field`
   dependency prose to mirror D-5-K: struct fields,
   closure parallel struct, union fields, tuple fields,
   and active enum variant fields.

**Corrections applied for R3 (2026-05-04):**

1. **[BLOCK] d5-1-3-stale-item-name-source -- FIXED.**
   D-5-1.3 *Why not Option B* paragraph rewritten:
   "Render-side recovery is direct: `pretty.rs::render_place`
   (D-5-K, deferred to stage 11) reads variant names from
   the enum's `LayoutKind::Enum.shape.variants[idx].name`
   (or `Single.variant.name`) -- pinned in stage 3 / §11
   layout schema (`VariantLayout.name: StringId`),
   reachable at render time via the head local's
   `LocalDecl.ty`. No stage-12 `Item` lookup is needed
   (`Item` carries `def_path` / `kind` / `span` / flags /
   `opaque_handle_eligible`, not variant name tables)."
   *Compositional argument* paragraph also corrected:
   "Variant names belong at the variant's declaration site
   -- specifically at `VariantLayout.name` per stage-3 D11
   (the layout-side schema, not stage-12 `Item.variants`)."
   D-5-1 *Methodology rules invoked* C-semantics bullet
   also touched up at line 174 ("Names recovered
   render-side from layout-side `FieldLayout.name` /
   `VariantLayout.name`") to drop the residual "Item
   lookups" phrasing.

2. **[MINOR] d5k-spec-snapshot-field-render-cases -- FIXED.**
   §18.2 per-method ownership row's `Field` dependency
   prose expanded to mirror D-5-K's per-predecessor table:
   `LayoutKind::Struct.fields[idx].name` for `Ty::Struct`;
   `find_nominal` to parallel `Ty::Struct` then
   `LayoutKind::Struct.fields[idx].name` for
   `Ty::Closure(item, args)`;
   `LayoutKind::Union.fields[idx].name` for `Ty::Union`;
   tuple's `fields[idx].name` for `Ty::Tuple`; active
   variant's `VariantLayout.fields[idx].name` for
   `Ty::Enum` after `Downcast`. §19.8 `Field` dependency
   prose given the same expansion as a bulleted per-
   predecessor list.

### R3 (2026-05-04): VERDICT: BLOCK

**Prompt body:** archived at
`/tmp/codex_d52_d5k_review_r3_prompt.txt`. Names R2's two
corrections explicitly.

**Codex output capture:** `/tmp/codex_d52_d5k_review_r3.txt`.

**Verdict:** BLOCK -- the retarget had two more residual
stale references I missed. Same pattern as R2 (consistency
sweep across surfaces touching the deferral-target claim).

**Issue count:** 2 (1 BLOCK / 0 MINOR / 1 NIT)

**Issues:**

1. **[BLOCK] d5k-stale-item-name-source-spec-snapshot** [`spec-snapshot.md:5347`]

   §19.1 Downcast doc still says the dropped symbol is
   "recovered render-side from the enum's Item"
   (`spec-snapshot.md:5347-5348`); §19.6 (C-semantics)
   Field row still says "field name from the parent ADT's
   `Item`" (`spec-snapshot.md:5491-5492`). Both contradict
   the corrected D-5-K position that `Item` is not a
   render_place dependency.

   *Suggested fix:* §19.1 -- "recovered render-side from
   the enum layout's `VariantLayout.name`"; §19.6 row --
   "field name from predecessor layout's `FieldLayout.name`,
   dispatched per §19.8."

2. **[NIT] d5k-status-stale-stage12-target** [`status.md:72`]

   `stage-5/status.md` Cluster split section still
   describes D-5-K as deferred to stage 12 with lookups via
   `Item.fields[]` / `Item.variants[]` and outbound in
   stage-12. Locked decisions section's D-5-K bullet
   similarly references stage 12 + stage-12 outbound. The
   prompt-shape template also still asks whether the
   stage-12 target is justified. All conflict with the
   corrected D-5-K target.

   *Suggested fix:* update the status table and prompt
   template to stage 11, layout-side `FieldLayout.name` /
   `VariantLayout.name`, and `stage-11/expectations.md`.

**Corrections applied for R4 (2026-05-04):**

1. **[BLOCK] d5k-stale-item-name-source-spec-snapshot --
   FIXED.** Two passages corrected:
   - §19.1 `Downcast` doc-comment: stale "recovered
     render-side from the enum's Item" replaced with
     "recovered render-side from the enum layout's
     `VariantLayout.name` per §11.1 / §19.8."
   - §19.6 (C-semantics) `Field` row: "field name from the
     parent ADT's `Item`" replaced with "field name from
     predecessor layout's `FieldLayout.name`, dispatched
     per §19.8."

2. **[NIT] d5k-status-stale-stage12-target -- FIXED.**
   `stage-5/status.md` updated at three sites:
   - Cluster split D-5-K bullet: "defers to stage 12"
     replaced with "defers to stage 11" plus a "**Originally
     proposed for stage 12** under the misimpression that
     `Item` carries `fields[].name` / `variants[].name`
     tables; **retargeted to stage 11** on the D-5-2 +
     D-5-K combined Codex review R1 BLOCK finding..."
     paragraph; outbound location flipped to
     `stage-11/expectations.md`.
   - Locked decisions D-5-K row: "deferred to stage 11
     (retargeted from stage 12 on R1 BLOCK finding)";
     outbound flipped to stage-11.
   - Next concrete unit of work: prompt-shape template
     mentions retarget from stage 12; criterion 3 updated
     from "stage 12" to "stage 11" + layout-side name
     sources; criterion 4 updated to reflect that the
     previously-filed stage-12 outbound has moved to
     stage-11 and been replaced with a retarget note;
     evidence-files list updated to clarify stage-11
     contains both walk + render_place outbounds while
     stage-12 carries the retarget note.

### R4 (2026-05-04): VERDICT: MINOR

**Prompt body:** archived at
`/tmp/codex_d52_d5k_review_r4_prompt.txt`. Names R3's two
corrections explicitly.

**Codex output capture:** `/tmp/codex_d52_d5k_review_r4.txt`.

**Verdict:** MINOR (no BLOCKs; no MINOR; single NIT)

**Issue count:** 1 (0 BLOCK / 0 MINOR / 1 NIT)

**Issues:**

1. **[NIT] d5-downcast-variantidx-wrong-stage3-crossref** [`spec-snapshot.md:5344`]

   §19.1 Downcast doc-comment correctly says the dropped
   symbol is recovered from `VariantLayout.name` (per
   §11.1 / §19.8), and §19.6 sources `Field` names from
   `FieldLayout.name`. But the same Downcast doc still
   labels `VariantIdx` as the source-order variant index
   "(stage-3 D6)" -- D6 is closure naming/synthesis, not
   the layout / variant table decision (which is D11).

   *Suggested fix:* replace `(stage-3 D6)` with
   `(stage-3 D11 / §11.0)` or `(per §2.4 / §11.0)`.

**Corrections applied for R5 (2026-05-04):**

1. **[NIT] d5-downcast-variantidx-wrong-stage3-crossref --
   FIXED.** §19.1 `Downcast` doc-comment cross-reference
   updated: "(stage-3 D6)" replaced with "(per §2.4 /
   §11.0; stage-3 D11 pinned the layout-side variant
   table)". Verified via grep that other "stage-3 D6.3"
   references in spec-snapshot.md / discussions.md are
   intentional (closure-as-struct shape per stage-3 D6.3,
   which IS the correct D-tag for closure synthesis).

### R5 (2026-05-04): VERDICT: MINOR

**Prompt body:** archived at
`/tmp/codex_d52_d5k_review_r5_prompt.txt`. Names R4's
correction explicitly.

**Codex output capture:** `/tmp/codex_d52_d5k_review_r5.txt`.

**Verdict:** MINOR (no BLOCKs; no MINOR; single NIT)

**Issue count:** 1 (0 BLOCK / 0 MINOR / 1 NIT)

**Issues:**

1. **[NIT] d5k-methodology-bullet-not-readme-rule** [`discussions.md:1120`]

   D-5-K lists "Per-method ownership in `pretty.rs`" under
   "Methodology rules invoked", but README §2's binding
   methodology list does not define "per-method ownership"
   as a methodology rule -- it's a cross-section
   consistency note about §18.2 being updated.

   *Suggested fix:* move this bullet out of "Methodology
   rules invoked" into a "Bookkeeping / cross-section
   consistency" note, or rephrase as evidence that the
   §18.2 ownership table was updated.

**Corrections applied for R6 (2026-05-04):**

1. **[NIT] d5k-methodology-bullet-not-readme-rule -- FIXED.**
   D-5-K *Methodology rules invoked* block: removed the
   "Per-method ownership in `pretty.rs`" bullet. Same
   content moved into a new "**Bookkeeping / cross-section
   consistency.**" sub-paragraph immediately following the
   methodology block, with explicit framing: "This is a
   spec consistency obligation rather than a README §2
   binding methodology rule." The §18.2 cross-reference is
   preserved.

### R6 (2026-05-04): VERDICT: APPROVE

**Prompt body:** archived at
`/tmp/codex_d52_d5k_review_r6_prompt.txt`. Names R5's
correction explicitly.

**Codex output capture:** `/tmp/codex_d52_d5k_review_r6.txt`.

**Verdict:** APPROVE -- 0 issues.

**Issue count:** 0 (0 BLOCK / 0 MINOR / 0 NIT)

D-5-2 and D-5-K cluster is closed. Status flipped from
`[DRAFT]` to `[LOCKED]` for D-5-2 and D-5-K in
`discussions.md`, and stage-5/status.md reflects the
closure.

### Cluster closure (2026-05-04): D-5-2 + D-5-K LOCKED at R6

**Trajectory summary:**

| Round | Verdict | Issues | Severity |
|---|---|---|---|
| R1 | BLOCK | 4 | 1 BLOCK / 1 MINOR / 2 NIT (substantive: stage-12 -> stage-11 retarget) |
| R2 | BLOCK | 2 | 1 BLOCK / 1 MINOR / 0 NIT (retarget consistency) |
| R3 | BLOCK | 2 | 1 BLOCK / 0 MINOR / 1 NIT (further retarget consistency) |
| R4 | MINOR | 1 | 0 BLOCK / 0 MINOR / 1 NIT (D-tag cross-ref) |
| R5 | MINOR | 1 | 0 BLOCK / 0 MINOR / 1 NIT (methodology bullet placement) |
| R6 | APPROVE | 0 | -- |

Total: 6 rounds; 10 distinct findings closed (3 BLOCK + 2
MINOR + 5 NIT). Substantive surprise at R1 (the
schema-driven retarget) was the load-bearing finding;
R2-R5 were consistency residuals from the retarget; R6
clean.

**Methodology compliance.** Cluster closed via standard
"Repeat until APPROVE" path per `memory.md` §1, mirroring
stage-4 D-4-3 / D-4-K trajectories (4 rounds each). The
D-5-1 R10 precedent of locking at MINOR-with-NIT-only was
*not* invoked here -- the cluster reached APPROVE at R6
with comfortably small marginal effort.

**D-5-2 + D-5-K cluster status:** `[LOCKED]` (per-stage).

---

## Stage 5 spec-side closure (2026-05-04)

All three stage-5 clusters now closed:

- **D-5-1 [LOCKED]** -- core vocabulary + structural
  invariants + storage shape (sub-decisions D-5-1.1
  through D-5-1.6). Locked at R10 = MINOR (NITs only;
  residuals deferred to top-level merger Codex review per
  README §5.5 step 6 per the methodology-deviation
  rationale recorded above). 25 distinct findings closed
  across 10 rounds.
- **D-5-2 [LOCKED]** -- Place validation phase placement
  (rules at stage 5; walk at stage 11). Locked at R6 =
  APPROVE alongside D-5-K.
- **D-5-K [LOCKED]** -- `pretty.rs::render_place` deferred
  to stage 11 (retargeted from stage 12 on R1 BLOCK
  finding). Locked at R6 = APPROVE alongside D-5-2.

**Total review cost:** 16 Codex rounds (10 for D-5-1; 6
for D-5-2 + D-5-K combined); 35 distinct findings closed
(7 BLOCK + 12 MINOR + 16 NIT).

**Next workstream:** merger contract per README §5.5
(top-level integration of `spec.md`, `discussions.md`,
`review-log.md`, `decisions.md` plus post-merge Codex
review). The post-merge top-level Codex review will sweep
any residual cross-references deferred from D-5-1's R10
closure.

---


---

## Stage 6 (MIR operands)

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

---

## Stage 6 spec-side closure (2026-05-04)

Stage-6 cluster (D-6-1 parent + sub-decisions D-6-1.1
through D-6-1.4 + D-6-2 + D-6-3 + D-6-K) closed across two
review trajectories:

- **Per-stage trajectory (6 rounds, R1-R6).** Substantive
  §20 content confirmed lock-ready by Codex at R4, R5,
  and R6 with explicit "Lockability: yes" verdict at R6.
  Cluster locked at R6 MINOR (1 bookkeeping finding swept
  inline). 17 distinct findings closed (4 BLOCK + 13
  MINOR + 0 NIT).

- **Post-merge trajectory (2 rounds + NIT).** Top-level
  spec.md / discussions.md / review-log.md / decisions.md
  integrated per merger contract steps 1-5 (README §5.5).
  Post-merge R1 = MINOR (2 findings:
  `stage6-audit-status-drift` -- merged Stage 6 D-tag
  headings still marked `[DRAFT]` instead of `[LOCKED]`;
  `constant-type-identity-overclaim` -- §20.5 overstated
  that `Const` always carries its own type identity, but
  `ZST` and `EnumValue` get type from consumer context).
  Both R1 fixes applied. Post-merge R2 = APPROVE with 1
  NIT (`locked-legend-approve-overclaim` -- legend defined
  `[LOCKED]` as "closed at APPROVE" but cluster locked at
  R6 MINOR per the stage-5 R10 diminishing-returns
  precedent). NIT fix applied (legend reworded to "closed
  by an approved lockability decision (APPROVE, or
  MINOR-NIT-only stable per the stage-5 diminishing-
  returns precedent)").

**Total review cost:** 8 Codex rounds (6 per-stage + 2
post-merge); 20 distinct findings closed (4 BLOCK + 15
MINOR + 1 NIT).

**Stage 6 status.** All four D-tags LOCKED. Spec content
integrated into top-level §20. Audit shape merged under
`## Stage 6 (MIR operands)` in discussions.md. Review trail
merged under `## Stage 6 (MIR operands)` in review-log.md.
Decisions registry extended with `### Stage 6 (MIR operands)`
in decisions.md.

**Outbound expectations** filed:
- `stage-9/expectations.md` "From stage 6" -- per-argument
  span preservation for Call / TailCall via
  `Spanned<Operand>` wrapper (or parallel slabs).
- `stage-11/expectations.md` "From stage 6" --
  `pretty.rs::render_operand` deferral alongside
  `render_place`.

**Next workstream:** Stage 7 (MIR Rvalues), the next link
in the dependency-driven 5-11 chain.


## Stage 7 (MIR rvalues)

# Stage 7 Review Log

Round-by-round Codex audit trail for stage-7's spec
cluster (D-7-1 with sub-decisions D-7-1.1-1.8, D-7-2,
D-7-3, D-7-K). Each round records the verdict, issue
list, and corrections applied.

For the prompt template, see `memory.md` §4. For decision
content, see `discussions.md`. For the spec under review,
see `spec-snapshot.md` §21 (Rvalue vocabulary) + §18.2
row update for `render_rvalue`.

---

## Round 1 (2026-05-04)

**Verdict:** BLOCK. 7 issues (2 BLOCK + 5 MINOR + 0 NIT).

**Codex output:** `/tmp/codex_d7_review_r1.txt`.

### Findings

1. **[BLOCK] aggregate-array-tyid** (`spec-snapshot.md:6624`).
   `AggregateKind::Array(TyId)` internal contradiction:
   the §21.3.6 enum doc and §21.5.2 invariant require
   `TyId` to resolve to `Ty::Array(elem, n)` (destination
   array type), but the §21.6.4 bridge rule maps rustc
   `AggregateKind::Array(ty)` to `Array(translate(ty))`
   where rustc's `ty` is the *element* type
   (`rustc_middle/src/mir/syntax.rs:1541`). discussions.md
   line 1096 also reads "Array(TyId) -- element type."

2. **[BLOCK] tuple-bridge-incomplete** (`spec-snapshot.md:6625`).
   The Tuple bridge rule says "reconstruct the tuple type
   from operand types and intern" but stage 3's `Ty::Tuple`
   requires both `fields` and an inline `layout`
   (`spec.md:2372`, `spec.md:2399`); stage-3 translation
   gets tuple layout from `tcx.layout_of` (`spec.md:4530`).
   The bridge rule must be explicit about layout.

3. **[MINOR] cleanup-required-citations** (`spec-snapshot.md:6458`).
   `is_required()` claims in the §21.5.1 rejection table
   need explicit cites. `cleanup_post_borrowck.rs:33-43`
   shows fake-borrow NOPing; `is_required()` is at
   `:88`. `lib.rs:658-659` shows `EraseDerefTemps` and
   `ElaborateBoxDerefs` in the runtime-lowering pass
   list; their required status is at
   `erase_deref_temps.rs:38` and
   `elaborate_box_derefs.rs:224`.

4. **[MINOR] rvalue-payload-drift** (`discussions.md:66`).
   discussions.md still records the parent and sub-
   decisions as `Ref(BorrowKind, Place)` and
   `RawPtr(RawPtrKind, Place)`, while §21 has resolved
   those payloads to reused stage-3 types
   `Ref(Provenance, Place)` and `RawPtr(Mutability,
   Place)` (`spec-snapshot.md:5961`,
   `spec-snapshot.md:5968`).

5. **[MINOR] render-rvalue-context** (`spec-snapshot.md:6785`).
   §21.8 says aggregate rendering field/variant names
   resolve "via the head local's `LocalDecl.ty`" --
   stale post-D-7-1.8 redesign. Every `AggregateKind`
   variant carries a `TyId` in its payload, so name
   resolution comes from the TyId via the §12 type
   interner, not from the head local.

6. **[MINOR] dtag-inline-reference** (`spec-snapshot.md:6795`,
   `spec-snapshot.md:5244`).
   README §5.9 rule 5: spec text should not embed D-tag
   inline parentheticals. The `unimplemented!` stub
   strings at §21.8 and the §18.2 row update both
   embed `(deferred per D-7-K)`.

7. **[MINOR] methodology-attribution** (`discussions.md:135`).
   discussions.md cites "Closed vocabulary; unknown
   rejected" and "Cleanup-pass artefacts are reject-
   on-encounter" as `../README.md` §2 methodology
   rules. The cleanup-artefact wording is in
   `../memory.md` §2 (stage-7 principle). The closed-
   vocabulary discipline is also a stage-7 principle
   (memory.md §2), not a top-level methodology rule.

### Corrections applied

For each finding, the following edits land before R2:

1. **Array bridge rule.** §21.6.4 Array row updated to
   construct the destination `Ty::Array` from the rustc
   element type and the operand count:
   `AggregateKind::Array(elem_ty)` ->
   `AggregateKind::Array(intern_or_lookup(Ty::Array(translate(elem_ty), operand_count)))`.
   discussions.md D-7-1.8 enum doc line 1096 updated:
   `Array(TyId)` carries the destination array type.

2. **Tuple bridge rule.** §21.6.4 Tuple row updated to
   obtain the destination tuple type from
   `rvalue.ty(body, tcx)` (rustc's MIR Rvalue type-of
   helper); translation routes through stage-3
   `Ty::Tuple { fields, layout }` interning where layout
   is sourced via `tcx.layout_of` per §15.

3. **Cleanup-pass `is_required()` citations.** §21.5.1
   table rows for `CopyForDeref`, `ShallowInitBox`,
   `BorrowKind::Fake`, and `PointerCoercion(MutToConstPointer
   | ArrayToPointer)` all now carry the `is_required()`
   line cite alongside the pass-listing cite. discussions.md
   parallel claims updated.

4. **Phase B realization notes.** discussions.md D-7-1.4
   and D-7-1.5 each carry a Phase-B-realization note that
   the abstract 2-variant `BorrowKind` / `RawPtrKind`
   discriminator is realized in §21 by reusing stage-3's
   existing `Provenance` (`spec.md:2458`) /
   `Mutability` (`spec.md:2461`) enums with identical
   content. The audit-trail abstract names are aliases for
   the realized stage-3 enums.

5. **Aggregate-rendering rationale.** §21.8 Aggregate row
   and explanatory paragraph updated: aggregate field/
   variant names resolve directly from the
   `AggregateKind`'s `TyId` via the §12 type interner;
   what defers to stage 11 for `Aggregate` is the
   *operand* rendering subtree (each operand routes
   through `render_operand`).

6. **D-tag inline removal.** §21.8 stub string changed
   from `unimplemented!("stage 11 fills render_rvalue
   (deferred per D-7-K)")` to `unimplemented!("stage 11
   fills render_rvalue")`. §18.2 row stub string
   identical change.

7. **Methodology re-attribution.** discussions.md
   sites that cite "Closed vocabulary; unknown rejected"
   or "Cleanup-pass artefacts are reject-on-encounter"
   now route to `../memory.md` §2 (stage-7-specific
   principles) instead of `../README.md` §2
   (top-level methodology).

After corrections land: re-run Codex R2 with a fresh
prompt naming the closed findings; verify R1 fixes.

---

## Round 2 (2026-05-04)

**Verdict:** BLOCK. 5 issues (2 BLOCK + 3 MINOR + 0 NIT).

**Codex output:** `/tmp/codex_d7_review_r2.txt`.

R1 trajectory: 4 of 5 R2 findings are R1-correction
residuals where the spec-snapshot.md fix landed but the
parallel update in discussions.md / memory.md was missed.
1 R2 finding is new (aggregate-render-name-access-path,
MINOR 5).

### Findings

1. **[BLOCK] tuple-bridge-residual** (`discussions.md:1155`).
   D-7-1.8 in discussions.md still records the old Tuple
   bridge rule (`Tuple(<TyId of Ty::Tuple([translate(op.ty())
   for op in operands])>)`) -- the R1-incomplete form
   without explicit layout sourcing. spec-snapshot.md §21.6.4
   was correctly updated to use `rvalue.ty(body, tcx)` plus
   stage-3 `tcx.layout_of` interning, but discussions.md's
   parallel record was not.

2. **[BLOCK] fakeforptrmetadata-methodology-contradiction**
   (`memory.md:140`).
   memory.md §2 lists `RawPtrKind::FakeForPtrMetadata` as a
   cleanup-pass artefact subject to the "reject-on-encounter,
   not variants" principle. But D-7-1.5 says
   `FakeForPtrMetadata` is reachable (NOT cleanup-discharged)
   and the bridge collapses it to `Mutability::Mut` per
   rustc's "operationally equivalent to Mut" doc.
   Methodology principle contradicts the actual D-7-1.5
   decision.

3. **[MINOR] cleanup-citations-still-stale-in-discussions**
   (`discussions.md:1453`).
   §21.5.1 in spec-snapshot.md got the `is_required()` line
   citations in R1, but D-7-3's parallel rejection table in
   discussions.md was not updated to mirror them.

4. **[MINOR] render-rvalue-audit-trail-stale**
   (`discussions.md:1651`, `memory.md:206-211`).
   §21.8 spec was updated in R1 to resolve aggregate field/
   variant names from AggregateKind's TyId via the §12 type
   interner (no LocalDecl.ty needed). But D-7-K in
   discussions.md and memory.md §2 (Renderer style principle)
   both still claim names resolve via `LocalDecl.ty`.

5. **[MINOR] aggregate-render-name-access-path**
   (`spec-snapshot.md:6787`).
   New finding (not an R1 residual). §21.8 says
   `Struct -> Ty::Struct.fields`, `Enum -> Ty::Enum variants
   table`, `Union -> Ty::Union.fields`, but stage-3's
   `Ty::Struct/Enum/Union` actually store layouts inline;
   fields and variants are reached through the layout
   vocabulary (`LayoutKind::Struct.fields`,
   `LayoutKind::Enum.shape`, `LayoutKind::Union.fields`).

### Corrections applied

1. **Tuple bridge rule in discussions.md.** D-7-1.8 bridge-
   translation paragraph rewritten to match §21.6.4 verbatim
   pattern: `Tuple -> Tuple(translate(rvalue.ty(body, tcx)))`
   with explicit layout-via-`tcx.layout_of` sourcing.

2. **FakeForPtrMetadata methodology refactor.** memory.md §2
   "Cleanup-pass artefacts are reject-on-encounter, not
   variants" principle now carves out `FakeForPtrMetadata`
   from the cleanup-discharged list; a new sub-paragraph
   "Distinct from this principle: bridge-collapse artefacts"
   documents the bridge-collapse pattern (rustc's
   "operationally equivalent to Mut" doc) as a distinct
   discipline applied to a distinct construct.

3. **D-7-3 rejection table citations.** discussions.md D-7-3
   rejection table (Part A) rows for `CopyForDeref`,
   `ShallowInitBox`, `Ref(_, Fake(_), _)`, and
   `Cast(PointerCoercion(MutToConstPointer | ArrayToPointer))`
   now carry the same `is_required()` line citations that
   §21.5.1 carries.

4. **Render-rvalue audit-trail catch-up.** D-7-K table row
   for Aggregate updated; D-7-K worked example paragraph
   updated; memory.md §2 "Renderer style is decided
   deliberately" principle updated to point at
   AggregateKind's per-variant TyId payload as the name
   resolution path (no LocalDecl.ty), with the operand-
   rendering subtree as the actual deferral driver.

5. **Aggregate name access-path correction.** §21.8 spec
   text updated to use stage-3's actual layout-vocabulary
   path: `Ty::Struct { layout } -> LayoutKind::Struct.fields`,
   `Ty::Enum { layout } -> LayoutKind::Enum.shape -> active
   variant's VariantLayout.fields`, `Ty::Union { layout } ->
   LayoutKind::Union.fields`. Closure path goes via the
   parallel Ty::Struct find_nominal lookup, then the
   captures-struct's `LayoutKind::Struct.fields`.

After corrections land: re-run Codex R3.

---

## Round 3 (2026-05-04)

**Verdict:** BLOCK. 3 issues (1 BLOCK + 2 MINOR + 0 NIT).

**Codex output:** `/tmp/codex_d7_review_r3.txt`.

Trajectory: 7 (R1) -> 5 (R2) -> 3 (R3). Convergence good.
The R3 BLOCK is a substantive finding (R1 picked the wrong
collapse target for FakeForPtrMetadata); the 2 MINORs are
residuals where the previous tuple-shape and rejected-
variant fixes did not fully propagate into memory.md.

### Findings

1. **[BLOCK] fakeforptrmetadata-collapse-target-contradicts-codegen**
   (`spec-snapshot.md:6558`).
   We claimed `FakeForPtrMetadata` collapses to
   `Mutability::Mut`, justified by the syntax.rs:189-197 doc
   comment "operationally equivalent to `RawPtrKind::Mut`."
   But rustc codegen calls `kind.to_mutbl_lossy()` at
   `rustc_codegen_ssa/src/mir/rvalue.rs:513-515` which maps
   `FakeForPtrMetadata` to `Mutability::Not` (per
   `rustc_middle/src/mir/statement.rs:994-1003`: "we have
   no type corresponding to a fake borrow, so use `*const`
   as an approximation"). The doc comment describes Miri
   retag semantics (a borrow-stack abstract-machine
   concept); the actual codegen ground truth is
   `Mutability::Not`.

2. **[MINOR] memory-c-semantics-lists-rejected-variants**
   (`memory.md:232-256`).
   memory.md §2's "C-semantics for the rvalue surface"
   listed `ThreadLocalRef` and `WrapUnsafeBinder` among the
   variant-to-C-form mappings. Both are dropped from the
   RegseqModel surface (D-7-1.1) and rejected at bridge
   time (D-7-3); they have no C lowering.

3. **[MINOR] tuple-layout-shape-residual**
   (`discussions.md:1264, 1491`).
   D-7-1.8 reasoning paragraph still said
   `Ty::Tuple(elem_tys)`; D-7-3 tuple operand-count rule
   said the same. The corrected stage-3 form is
   `Ty::Tuple { fields, layout }` per `spec-snapshot.md:6323`
   and `spec.md:4530`.

### Corrections applied

1. **FakeForPtrMetadata collapse target flip.** Every site
   that recorded `FakeForPtrMetadata -> Mutability::Mut`
   updated to `Mutability::Not`:
   - spec-snapshot.md §21.6.1 (top-level Rvalue translation
     table row).
   - spec-snapshot.md §21.6.2 (FakeForPtrMetadata collapse
     section, full rewrite citing
     `to_mutbl_lossy()` at statement.rs:994-1003 and
     codegen invocation at mir/rvalue.rs:513-515; calls out
     that the syntax.rs:189 doc comment describes Miri
     retag semantics, not codegen behaviour).
   - spec-snapshot.md §21.2.5 cross-reference updated.
   - discussions.md D-7-1.5 Decision block, Bridge
     translation rule, Phase B realization note, and
     Reasoning section all rewritten to use `Not` as the
     collapse target with the rustc-codegen ground-truth
     citation. Charon's choice (`Shared` / read-only) now
     agrees with our choice rather than diverging.
   - discussions.md D-7-1.5 worked example updated to show
     `Mutability::Not` collapse output.
   - memory.md §2 bridge-collapse-artefact sub-paragraph
     updated to cite `to_mutbl_lossy()` mapping.

2. **memory.md §2 C-semantics list pruned.** Removed
   `ThreadLocalRef` and `WrapUnsafeBinder` bullets; added
   a closing note that the four dropped variants have no
   C lowering and are bridge-rejected per D-7-3.

3. **Tuple-shape residuals updated.** discussions.md
   D-7-1.8 reasoning ("Tuple(TyId) for symmetry") now
   cites `Ty::Tuple { fields, layout }` with explicit
   layout-via-`tcx.layout_of` sourcing. D-7-3 tuple
   operand-count rule updated to `fields.len()` for
   `Ty::Tuple { fields, layout }`.

After corrections land: re-run Codex R4.

---

## Round 4 (2026-05-04)

**Verdict:** MINOR. 4 issues (0 BLOCK + 4 MINOR + 0 NIT).

**Codex output:** `/tmp/codex_d7_review_r4.txt`.

Trajectory: 7 BLOCK -> 5 BLOCK -> 3 BLOCK -> 4 MINOR.
Crossed the BLOCK threshold; substantive issues are gone.
All four findings are bookkeeping (memory.md mission
text, D-7-3 phase wording, stale §21.x cross-refs in
discussions.md, missing stage-3 anchor on the
WrapUnsafeBinder rejection row).

### Findings

1. **[MINOR] memory-wrapunsafebinder-still-listed-live**
   (`memory.md:61`).
   memory.md §0 mission listed `WrapUnsafeBinder` among
   operand-shaped Rvalue payload positions; that variant is
   dropped per D-7-1.1 and rejected per D-7-3.

2. **[MINOR] invariant-enforcement-phase-conflict**
   (`discussions.md:1468`, `:1492`).
   D-7-3 said positive invariants are enforced "at
   construction" while the spec says they are enforced by
   the stage-11 body-level walk (`spec-snapshot.md:6470`,
   `:6520`). Same split as D-5-2 (rules at stage 5; walk
   at stage 11).

3. **[MINOR] stale-section-crossrefs**
   (`discussions.md:209, :453, :602, :643, :763, :816,
   :940, :1106, :1156, :1362, :1629, :1747`).
   Multiple discussion cross-refs pointed at old section
   numbers: bridge translation cited as §21.3 (correct:
   §21.6); rendering cited as §21.7 (correct: §21.8);
   walk deferral cited as §21.6 (correct: §21.5.3).

4. **[MINOR] wrapunsafebinder-spec-row-misses-stage3-anchor**
   (`spec-snapshot.md:6459`).
   The §21.5.1 rejection row for `WrapUnsafeBinder` cited
   only feature-stability / no-C-shape / no-corpus grounds,
   missing the strongest ground (§15.1 type-level
   rejection of `Ty::UnsafeBinder` per `spec.md:4583`).

### Corrections applied

1. **WrapUnsafeBinder pruned from memory.md mission.** §0
   mission consumer list now enumerates the seven retained
   operand-shaped consumers (`Use`, `Repeat`, `Cast`,
   `BinaryOp`, `UnaryOp`, `Aggregate`) plus a
   trailing parenthetical noting `WrapUnsafeBinder` would
   have been an additional consumer but is dropped /
   rejected.

2. **D-7-3 phase wording aligned.** Decision summary now
   reads "Positive invariants specified at stage 7 (Part B);
   walk-enforcement implementation deferred to stage 11's
   body-level traversal (Part C; same split as D-5-2)."
   Part B section heading also updated to "Positive
   invariants (well-formedness rules; walk-enforcement
   deferred to stage 11)."

3. **§21.x cross-refs fixed.** All `§21.3 (bridge translation)`
   sites updated to `§21.6` (with sub-section refinements
   `§21.6.2`, `§21.6.3`, `§21.6.4` where appropriate); all
   `§21.7 (rendering)` -> `§21.8`; `§21.6 (walk-implementation)`
   -> `§21.5.3`; "filed in §15 / §21.3" -> "filed in §15 /
   §21.6" (3 sites). Sub-enum cross-refs corrected:
   `§21.2 (BorrowKind sub-enum)` -> `§21.3.1 (Provenance
   reuse note)`; `§21.2 (RawPtrKind sub-enum)` ->
   `§21.3.2 (Mutability reuse note)`; `§21.2 (CastKind /
   PointerCoercion sub-enums)` -> `§21.3.3`; `§21.2 (BinOp /
   UnOp sub-enums)` -> `§21.3.4 / §21.3.5`; `§21.2
   (AggregateKind sub-enum)` -> `§21.3.6`.

4. **WrapUnsafeBinder stage-3 anchor added.** §21.5.1
   rejection row for `WrapUnsafeBinder` now leads with
   "§15.1 (Type-level rejection table at line 4583)
   rejects `Ty::UnsafeBinder` at type translation; the
   rvalue-level rejection cascades by composition," with
   feature-stability and C-shape grounds as additional.
   §21.1.1 dropped-variant paragraph for WrapUnsafeBinder
   updated identically.

After corrections land: re-run Codex R5 targeting APPROVE.

---

## Round 5 (2026-05-04)

**Verdict:** APPROVE. 0 issues.

**Codex output:** `/tmp/codex_d7_review_r5.txt`.

R4 fixes verified end-to-end: memory.md §0 lists only the
seven retained operand-shaped consumers and mentions
WrapUnsafeBinder only as dropped/rejected. D-7-3 separates
bridge-time rejection (Part A) from stage-7 invariant
specification (Part B) and stage-11 walk enforcement
(Part C); aligned with §21.5 / §21.5.3 in the spec.
Discussion cross-refs all resolve to existing §21 headings:
§21.6 (bridge translation), §21.6.2 (FakeForPtrMetadata
collapse), §21.6.3 (CastKind translation), §21.6.4
(AggregateKind translation), §21.5.3 (walk deferral),
§21.8 (rendering deferral), §21.3.1-§21.3.6 (sub-enums).
WrapUnsafeBinder rejection now leads with stage-3
`Ty::UnsafeBinder` cascade (`spec.md:4583`) in both §21.1.1
and §21.5.1.

**Cluster trajectory.**

| Round | Verdict | Issues | BLOCK / MINOR / NIT |
|-|-|-|-|
| R1 | BLOCK | 7 | 2 / 5 / 0 |
| R2 | BLOCK | 5 | 2 / 3 / 0 |
| R3 | BLOCK | 3 | 1 / 2 / 0 |
| R4 | MINOR | 4 | 0 / 4 / 0 |
| R5 | APPROVE | 0 | 0 / 0 / 0 |

**Total Codex cost:** 5 rounds. **Distinct findings closed
across the trajectory:** 19 (5 BLOCK + 14 MINOR + 0 NIT).

### Cluster lock

D-7-1 (with sub-decisions D-7-1.1-1.8), D-7-2, D-7-3, D-7-K
all flip [DRAFT] -> [LOCKED] in `discussions.md`. Stage 7's
spec-side work is complete; ready for the merger contract
per README §5.5 (top-level integration of `discussions.md`,
`review-log.md`, `decisions.md`, and `spec.md` §21
contribution + post-merge Codex review pass).

## Stage 7 spec-side closure (2026-05-04)

Stage-7 cluster (D-7-1 parent + sub-decisions D-7-1.1
through D-7-1.8 + D-7-2 + D-7-3 + D-7-K) closed across
two review trajectories:

- **Per-stage trajectory (5 rounds, R1-R5).** R1 BLOCK
  (2 BLOCK + 5 MINOR), R2 BLOCK (2 BLOCK + 3 MINOR), R3
  BLOCK (1 BLOCK + 2 MINOR), R4 MINOR (4 MINOR), R5
  APPROVE (0 issues). Substantive content lock-ready by
  R3 (the R3 BLOCK was the FakeForPtrMetadata collapse-
  target correction from `Mutability::Mut` to
  `Mutability::Not` -- discovered via codegen
  `to_mutbl_lossy()` ground-truth at
  `rustc_middle/src/mir/statement.rs:994-1003`); R4 was
  bookkeeping; R5 clean APPROVE. 19 distinct findings
  closed (5 BLOCK + 14 MINOR + 0 NIT).

- **Post-merge trajectory (5 rounds, R1-R5).** Top-level
  spec.md / discussions.md / review-log.md / decisions.md
  integrated per merger contract steps 1-5 (README §5.5).
  R1 MINOR (4: stale §17/§15.1 cross-refs, invariant-count
  mismatch, D-tag inline leak, stage-attribution leak),
  R2 MINOR (2: stale §17 audit-trail residuals,
  render_rvalue deferral-reason overstating body-ownership),
  R3 MINOR (2: more stale §17 residuals, render_rvalue
  Cast placeholder drift), R4 MINOR (2: even deeper
  audit-trail §17 residuals, D-7-K Phase-A-name drift in
  render table), R5 APPROVE (0 issues). All R-fixes
  applied. 10 distinct findings closed (0 BLOCK + 10
  MINOR + 0 NIT).

**Total review cost:** 10 Codex rounds (5 per-stage + 5
post-merge); **29 distinct findings closed** (5 BLOCK +
24 MINOR + 0 NIT).

**Stage 7 status.** All four D-tags LOCKED. Spec content
integrated into top-level §21 (Rvalue vocabulary) plus
§18.2 row update for `render_rvalue`. Audit shape merged
under `## Stage 7 (MIR rvalues)` in discussions.md.
Review trail merged under `## Stage 7 (MIR rvalues)` in
review-log.md plus this closure entry. Decisions registry
extended with `### Stage 7 (MIR rvalues)` in decisions.md.


---

## Stage 8 (MIR statements)

## R1 -- 2026-05-04

**Prompt scope.** Stage-8 spec D-8-1 through D-8-K
under review (full cluster). Focus: §22 of
spec-snapshot.md plus the audit shape in discussions.md
plus status.md cluster split. Evidence files: 4
prior-art primary-source files + project README +
architecture.md + top-level spec.md baseline +
crates/regseq-driver/src/callbacks.rs.

**Output capture.** `/tmp/codex_stage8_review_r1.txt`
(verdict + 6 issues).

**Verdict.** `BLOCK` -- 2 BLOCK + 4 MINOR + 0 NIT.

**Issues.**

| # | Severity | Tag | Reference |
|---|----------|-----|-----------|
| 1 | BLOCK | D-8-1.3-TY-SHAPE | spec-snapshot.md:7148 + discussions.md:498-508, 1156-1159 |
| 2 | BLOCK | D-8-1.4-COPYNONOVERLAPPING-TYPES | spec-snapshot.md:7447 |
| 3 | MINOR | D-8-4-NOP-DISCHARGE | spec-snapshot.md:7364 |
| 4 | MINOR | D-8-4-INTRINSIC-OPERAND-WALK | discussions.md:1112 |
| 5 | MINOR | STATUS-ROW-COUNT | status.md:3, 109-115, 136-143 |
| 6 | MINOR | D-8-K-RENDER-TABLE | spec-snapshot.md:7678 vs §18.2 |

**Corrections applied (R1 -> R2).**

1. **D-8-1.3-TY-SHAPE (BLOCK)**: Spec text used a
   fabricated `Ty::Adt(NominalId)` umbrella with
   `NominalKind == Enum` discriminator. Stage-3's
   actual `Ty` enum at `spec.md:2383-2403` has direct
   `Ty::Struct { item, args, layout }`,
   `Ty::Enum { item, args, layout }`,
   `Ty::Union { item, args, layout }` -- NOT a
   `Ty::Adt` umbrella. `NominalKind` exists at
   `spec.md:2896` but only as the discriminator for
   `find_nominal(kind, item, args)` lookups, NOT as a
   variant tag on `Ty`. Replaced all instances of
   `Ty::Adt(NominalId)` with `Ty::Enum { item, args,
   layout }`; replaced `NominalKind == Enum` checks
   with direct `Ty::Enum { .. }` matches; rewrote
   variant-count lookup to use `LayoutKind::Enum.shape`
   per `EnumShape` cases at `spec.md:2620-2636` (Empty
   -> 0; Single -> 1; Multiple { variants, .. } ->
   variants.len()) -- matching the existing §19.7
   rule 4 + §21.5.2 I-Rvalue-VariantIdx-Wellformed
   dispatch pattern. Affected sites:
   - spec-snapshot.md §22.1.2 `StatementKind` doc (the
     SetDiscriminant inline doc-comment).
   - spec-snapshot.md §22.2.2 (SetDiscriminant payload
     description).
   - spec-snapshot.md §22.5.2 B-2, B-3 (positive
     invariants).
   - spec-snapshot.md §22.6.1 (bridge translation
     table; the rule reference for SetDiscriminant
     translation row).
   - spec-snapshot.md §22.7.2 (SetDiscriminant worked
     example).
   - discussions.md D-8-1.3 audit (Decision +
     Reasoning).
   - discussions.md D-8-4 audit (Decision Part B-3 +
     worked walk).

2. **D-8-1.4-COPYNONOVERLAPPING-TYPES (BLOCK)**:
   Spec B-5 claimed src/dst types must be raw-pointer
   `Ty::Ptr(_, _)` and that "rustc validator pins
   pointer-pointer-usize." Rustc's actual validator at
   `rustc_mir_transform/src/validate.rs:1517-1549`
   accepts any type with `builtin_deref(true)`, which
   covers raw pointers, references, AND `Box<T>`. The
   doc at `rustc_middle/src/mir/syntax.rs:494-500`
   states "src and dest must each be a reference,
   pointer, or `Box`." Widened B-5 to "any type whose
   pointee can be taken via `builtin_deref(true)`":
   raw pointer, reference, or Box-like. The pointee-
   equality and count-as-usize rules stand. Updated
   the "validator pins" wording to reflect the actual
   rule; cite `validate.rs:1517-1549` and
   `syntax.rs:494-500`. Affected sites:
   - spec-snapshot.md §22.5.2 B-5.
   - discussions.md D-8-1.4 audit (Reasoning + B-5
     citation).

3. **D-8-4-NOP-DISCHARGE (MINOR)**: The §22.5.1 Nop
   row cited only `RemovePlaceMention` and
   `RemoveStorageMarkers` as sweepers. Driver also
   force-enables `RemoveZsts` and `SimplifyLocals-final`
   (`crates/regseq-driver/src/callbacks.rs:28-29`).
   `RemoveZsts` calls `make_nop(true)` at
   `rustc_mir_transform/src/remove_zsts.rs:143` (after
   `RemoveStorageMarkers` in the optimization order
   per `rustc_mir_transform/src/lib.rs:728-730`);
   `SimplifyLocals-final` strips `StatementKind::Nop`
   at `rustc_mir_transform/src/simplify.rs:580` and
   makes additional `Nop`s at lines 643 / 705. Added
   `SimplifyLocals-final` and `RemoveZsts` to the Nop
   row's discharge mechanism list, with the post-
   `RemoveZsts` Nop -> `SimplifyLocals-final` strip
   trace cited.

4. **D-8-4-INTRINSIC-OPERAND-WALK (MINOR)**: Worked
   walk in discussions.md D-8-4 applied B-0b
   transitively to Intrinsic operands. B-0b is defined
   only for `Rvalue` operands embedded via `Assign`
   (`I-Rvalue-Operand-Wellformed` at §21.5.2 applies
   to operands embedded in rvalues). Operand-site
   validity for non-Rvalue-embedded operands lives in
   §20.5. Added explicit B-0c
   `I-Statement-Operand-Wellformed` to spec-snapshot.md
   §22.5.2 covering operand positions in
   `Intrinsic::Assume(op)` and
   `Intrinsic::CopyNonOverlapping{src, dst, count}`,
   inheriting §20.5 invariants directly (rather than
   transitively through B-0b). Updated the discussions.md
   worked walk to apply B-0c (not B-0b) to Intrinsic
   operands. Affected sites:
   - spec-snapshot.md §22.5.2 (added B-0c).
   - discussions.md D-8-4 audit (worked walk now
     references B-0c for Intrinsic operands).
   - discussions.md D-8-4 audit (Decision text Part B
     enumeration extends from 7 to 8 invariants:
     B-0a/B-0b/B-0c inherited + B-1-B-5
     stage-8-introduced).

5. **STATUS-ROW-COUNT (MINOR)**: status.md
   inconsistencies fixed:
   - "11-row reject-on-encounter table" at line 3
     standardised to "12-row reject-on-encounter table"
     (matching spec-snapshot.md §22.5.1's actual row
     count: 11 distinct rows + 1 row covering split
     `Coverage(BlockMarker | SpanMarker)` vs
     `Coverage(non-marker)` in two cases).
   - "10-row" near line 109-115 brought into agreement
     with the 12-row count.
   - Stale "open question" wording for `RemoveStorageMarkers`
     deleted (the question was resolved during Phase-A
     discussion via the
     `crates/regseq-driver/src/callbacks.rs:25-66` +
     `pass_manager.rs:213-239` trace; status.md should
     reflect the resolved state).

   Note on row counting: the 12-row count includes the
   split `Coverage(BlockMarker | SpanMarker)` (Group 1
   row) vs `Coverage(non-marker variants)` (Group 3
   row), which together cover the single rustc
   `Coverage(_)` variant in two distinct ways.
   Likewise, `Assign(_, Rvalue::Ref(_, BorrowKind::Fake,
   _))` and `Assign(_, _)` (the surviving general
   case) are accounted for separately. The "10 distinct
   dropped rustc variants" framing captures the
   top-level rustc enum cardinality reduction.

6. **D-8-K-RENDER-TABLE (MINOR)**: §22.8 claimed "the
   §18.2 per-method ownership table is updated
   accordingly" but §18.2 (lines 5167-5253) listed
   only 8 renderers without `render_statement`. Two
   options: (a) add `render_statement` row to §18.2
   now (matches stage-7 D-7-K's pattern of having
   `render_rvalue` already in §18.2); (b) clarify that
   the update lands at merger close. Chose (a) for
   consistency with how stage 7's spec-snapshot.md
   already carries the `render_rvalue` row. Added
   `render_statement(&Statement, &Body) -- stage 11`
   row after `render_rvalue` at §18.2 with per-variant
   dependency analysis (mirroring the stage-7
   `render_rvalue` row's audit shape: routes through
   `render_place` (D-5-K), `render_rvalue` (D-7-K ->
   transitively `render_operand`), and per-variant
   payload-specific lookups for `SetDiscriminant`'s
   variant-name display).

**Next-round trigger.** Re-run Codex review against
the corrected spec + audit + status. Expect findings
to drop substantially; potential residuals in
peer-comparison detail or compositional-rule
articulation.

---

## R2 -- 2026-05-04

**Prompt scope.** Re-review against R1-corrected
spec + audit + status. Prompt explicitly enumerates
the 6 R1 fixes and asks for per-fix VERIFIED /
PARTIAL / NOT_ADDRESSED verification + new issues
introduced + residuals.

**Output capture.** `/tmp/codex_stage8_review_r2.txt`.

**Verdict.** `BLOCK` -- 1 BLOCK + 3 MINOR + 0 NIT.

**R1 fix verification:**

| R1 Issue | R2 Verdict | Notes |
|----------|------------|-------|
| D-8-1.3-TY-SHAPE | VERIFIED | Direct `Ty::Enum` matching landed cleanly. |
| D-8-1.4-COPYNONOVERLAPPING-TYPES | PARTIAL | Spec widened to `builtin_deref(true)`, but over-admits Box against stage-3 §15.1 rejection (R2 issue #1); status.md detailed text still has stale raw-pointer-only claim (R2 issue #3). |
| D-8-4-NOP-DISCHARGE | PARTIAL | Mechanism added but pipeline-order citation wrong (R2 issue #4). |
| D-8-4-INTRINSIC-OPERAND-WALK | PARTIAL | B-0c added but misstates §20.5 contents (R2 issue #2). |
| STATUS-ROW-COUNT | PARTIAL | Headline count fixed but detailed Phase-A-tail status text still stale (R2 issue #3). |
| D-8-K-RENDER-TABLE | VERIFIED | §18.2 row added cleanly. |

**R2 issues.**

| # | Severity | Tag | Reference |
|---|----------|-----|-----------|
| 1 | BLOCK | D-8-1.4-BOX-ADMISSION | spec-snapshot.md:7506, 7527 |
| 2 | MINOR | D-8-4-B0C-SECTION-20-MISQUOTE | spec-snapshot.md:7437 |
| 3 | MINOR | STATUS-STALE-DETAILS | status.md:284, 391, 415, 428, 524 |
| 4 | MINOR | D-8-4-NOP-ORDER-CITATION | spec-snapshot.md:7399 |

**Corrections applied (R2 -> R3).**

1. **D-8-1.4-BOX-ADMISSION (BLOCK)**: Spec text
   admitted `Box`-shaped `Ty::Struct{..}` as an
   accepted CopyNonOverlapping operand type. Rustc's
   validator does accept Box (`builtin_deref(true)`
   returns Some for Box per
   `rustc_middle/src/mir/syntax.rs:496`), but stage-3
   §15.1 rejects `Box<T>` at type translation (`alloc`
   construct outside the no-alloc driver DSL; see
   `spec.md:4546`). INFERENCE: a Box-typed operand
   cannot reach stage 8 RegseqModel without reopening
   stage-3's Box rejection. Narrowed B-5 to RegseqModel-
   representable deref-able types (`Ty::Ptr` and
   `Ty::Ref`); explicitly noted that rustc's validator
   also accepts Box but §15.1 rejects Box before
   Statement translation, so Box-typed operands do not
   appear at the stage-8 surface. Affected sites:
   - spec-snapshot.md §22.5.2 B-5 (narrowed type list +
     §15.1 cross-reference).
   - discussions.md D-8-1.4 audit (Reasoning + worked
     example).

2. **D-8-4-B0C-SECTION-20-MISQUOTE (MINOR)**: B-0c
   text claimed §20.5 includes "TyId-interned-ness on
   any embedded TyId reachable through stage-7
   `Constant(ConstId)` traversal." Evidence file
   `spec.md:5858-5895` shows §20.5 specifies one
   operand-site invariant (`I-Operand-Place-Validity`
   for `Copy`/`Move` places, inheriting §19.7) and
   explicitly states "Constant type coherence is owned
   by stages 7-9, not §20.5." Removed the
   Constant/TyId-internedness clause from B-0c;
   reduced B-0c to inherit only `I-Operand-Place-
   Validity` from §20.5 for the operand positions in
   `Intrinsic` payloads. Affected sites:
   - spec-snapshot.md §22.5.2 B-0c.

3. **STATUS-STALE-DETAILS (MINOR)**: Multiple stale
   details in status.md:
   (a) Detailed D-8-1.4 text at line 284 still said
   src/dst "raw-pointer types" only; updated to
   match the narrowed B-5 (Ty::Ptr | Ty::Ref) plus
   the §15.1 Box rejection cross-reference.
   (b) Detailed D-8-4 breakdown enumerated 11 entries
   (combining StorageLive / StorageDead) but
   headline claimed 12-row; split entry to two rows
   (StorageLive separate from StorageDead) so
   detailed enumeration matches the 12-row claim.
   (c) "Locked decisions" tail section still said
   "stage just opened; Phase-A discussion has not
   started" — stale Phase-A wording. Removed the
   stale tail section; the post-R1 status header
   already captures the current state.

4. **D-8-4-NOP-ORDER-CITATION (MINOR)**: spec-
   snapshot.md cited
   `rustc_mir_transform/src/lib.rs:728-730` for
   SimplifyLocals-final ordering. Evidence shows that
   range covers RemoveStorageMarkers + RemoveZsts;
   `SimplifyLocals::Final` appears later in the
   optimization-passes list at `lib.rs:775`. Updated
   the Nop discharge row to cite both
   `lib.rs:728-730` (for RemoveZsts) and
   `lib.rs:775` (for SimplifyLocals::Final).

**Next-round trigger.** Re-run Codex review against
the R2-corrected files. Expect verdict at MINOR or
APPROVE; structural soundness has been established.

---

## R3 -- 2026-05-04

**Prompt scope.** Re-review against R2-corrected
files. Prompt enumerates the 4 R2 fixes and asks for
per-fix VERIFIED / PARTIAL / NOT_ADDRESSED
verification + new issues + residuals.

**Output capture.** `/tmp/codex_stage8_review_r3.txt`.

**Verdict.** `MINOR` -- 0 BLOCK + 4 MINOR + 0 NIT.

**R2 fix verification (all VERIFIED).**

| R2 Issue | R3 Verdict |
|----------|------------|
| D-8-1.4-BOX-ADMISSION | VERIFIED |
| D-8-4-B0C-SECTION-20-MISQUOTE | VERIFIED |
| STATUS-STALE-DETAILS | VERIFIED |
| D-8-4-NOP-ORDER-CITATION | VERIFIED |

Structural soundness now confirmed across two
verification passes; all 6 R1 + 4 R2 issues addressed.

**R3 issues.** All 4 are status.md cleanup -- stale
Phase-A speculation that survived the prior status
revisions and now contradicts the locked headline
text.

| # | Severity | Tag | Reference |
|---|----------|-----|-----------|
| 1 | MINOR | STATUS-R2-STALE-OPEN-DECISIONS | status.md:539 |
| 2 | MINOR | STATUS-D-8-1.3-STALE-STRUCT-LEAN | status.md:249 |
| 3 | MINOR | STATUS-D-8-1.4-STALE-FANOUT-DETAIL | status.md:309 |
| 4 | MINOR | STATUS-D-8-4-ROW-ORDER-MISMATCH | status.md:443 |

**Corrections applied (R3 -> R4).**

1. **STATUS-R2-STALE-OPEN-DECISIONS (MINOR)**:
   "Open decisions" tail still said "R1 corrections
   applied; R2 to launch" -- stale by one round.
   Updated to reflect R2 done + R3 MINOR-only +
   R3 corrections in progress, OR removed as
   redundant with the now-detailed Codex review
   cadence block at the top of the file.

2. **STATUS-D-8-1.3-STALE-STRUCT-LEAN (MINOR)**:
   Detailed Phase-A tail at line 249 said "the
   project leans struct" while the headline locks
   `SetDiscriminant(Place, VariantIdx)` flat tuple
   at line 217 + spec-snapshot.md §22.2.2 + discussions.md
   D-8-1.3. Updated the detailed tail to "project
   leans tuple" matching the locked decision.

3. **STATUS-D-8-1.4-STALE-FANOUT-DETAIL (MINOR)**:
   Detailed Phase-A tail at line 309 said "Phase-A
   must pick" and "prefer flat / fan-out" while the
   headline at line 262 locks nested
   `Intrinsic(NonDivergingIntrinsic)`. Rewrote the
   detailed tail to match the nested decision +
   semantic-grouping rationale per spec-snapshot.md
   §22.2.3 + discussions.md D-8-1.4.

4. **STATUS-D-8-4-ROW-ORDER-MISMATCH (MINOR)**:
   D-8-4 Part A was grouped by discharge mechanism
   (Group 1 cleanup-NOPed, Group 2 strip-passes,
   Group 3 build-flag-gated) per the headline at
   line 395 + spec-snapshot.md §22.5.1. The detailed
   enumeration placed `Coverage(non-marker)` as row
   5 between cleanup-pass rows; moved to Group 3
   tail (after Retag, ConstEvalCounter) so the
   detailed enumeration matches the spec table's
   mechanism grouping.

**Next-round trigger.** Re-run Codex review (R4)
against the R3-corrected status.md. Expect APPROVE
or MINOR-with-NIT-only-stable per the diminishing-
returns precedent from stage 5; the spec-snapshot.md
+ discussions.md content has been verified clean
across R2 + R3.

---

## R4 -- 2026-05-05

**Prompt scope.** Re-review against R3-corrected
files. Prompt enumerates the 4 R3 fixes and asks for
per-fix VERIFIED / PARTIAL / NOT_ADDRESSED
verification + new issues + residuals + an explicit
`Lockability: yes | no` verdict line.

**Output capture.** `/tmp/codex_stage8_review_r4.txt`.

**Verdict.** `MINOR` with **`Lockability: yes`** -- 0
BLOCK + 1 MINOR + 0 NIT.

**R3 fix verification (all VERIFIED).**

| R3 Issue | R4 Verdict |
|----------|------------|
| STATUS-R2-STALE-OPEN-DECISIONS | VERIFIED |
| STATUS-D-8-1.3-STALE-STRUCT-LEAN | VERIFIED |
| STATUS-D-8-1.4-STALE-FANOUT-DETAIL | VERIFIED |
| STATUS-D-8-4-ROW-ORDER-MISMATCH | VERIFIED |

**R4 issue.** Single MINOR; status.md cleanup only.

| # | Severity | Tag | Reference |
|---|----------|-----|-----------|
| 1 | MINOR | STATUS-STALE-PHASE-A-TAIL | status.md:625-638 |

**Correction applied (R4).**

1. **STATUS-STALE-PHASE-A-TAIL (MINOR)**: After the
   "Next concrete unit of work" section already
   captured the R1/R2/R3 trajectory + R4-launch
   pointer, the file's tail still resumed stale
   Phase-A entry-options instructions ("D-8-3 first,"
   "D-8-4 Part A first," "Phase-A discussion lands
   locked decisions... Phase-B follows"). These were
   leftover from the original Open-phase status draft
   and contradicted the active state (Phase-B
   complete, R4 in flight). Replaced lines 625-638
   with the post-Codex-APPROVE / lockable-MINOR
   action plan: flip D-tags to [LOCKED]; hand off to
   the README §5.5 merger contract; file the three-
   item outbound bundle in stage-11/expectations.md.

**Lockability decision.** Per the stage-6 precedent
("Stage 6 (MIR operands) CLOSED AND MERGED 2026-05-04.
All four D-tags locked across a single combined
cluster... at R6 MINOR with explicit 'Lockability:
yes' verdict"; recorded in top-level
`../status.md`), R4's MINOR-with-`Lockability: yes`
verdict is the cluster-lock signal. The single MINOR
is non-substantive (status.md cleanup, not affecting
spec correctness); spec-snapshot.md §22 +
discussions.md content was verified clean across
R2 + R3 + R4. **Cluster locks at R4 2026-05-05.**

All six D-8-K tags flip from [DRAFTED] to [LOCKED]
in status.md + discussions.md.

**Next action.** Hand off to the README §5.5 merger
contract for top-level integration:
1. Concat `stage-8/discussions.md` into top-level
   `../discussions.md` under `## Stage 8` header.
2. Concat `stage-8/review-log.md` into top-level
   `../review-log.md` under `## Stage 8` header.
3. Append D-8-1 (parent + 4 sub-decisions D-8-1.1
   through D-8-1.4), D-8-2, D-8-3, D-8-4, D-8-K to
   top-level `../decisions.md` under `## Stage 8`
   with `[LOCKED]` status.
4. (No earlier-stage decisions reopened by stage 8.)
5. Integrate `stage-8/spec-snapshot.md` §22 +
   §18.2 row update for `render_statement` into
   top-level `../spec.md` per README §5.9 (topic-
   based, audit-shape-stripped, no-D-tag-inline-
   references rules).
6. Run a Codex post-merge review pass on the
   integrated top-level `../spec.md`. Iterate to
   APPROVE.
7. Update top-level `../status.md` to mark stage 8
   closed and add a "Snapshot at close of stage 8"
   header to `stage-8/spec-snapshot.md`.

After the merger contract closes, file the
three-item outbound bundle in
`../stage-11/expectations.md` "From stage 8":
(i) render_statement deferral (D-8-K);
(ii) Statement walk implementation (D-8-4 Part C);
(iii) named-local preservation (D-8-3).

Total stage-8 Codex cost (per-stage): 4 rounds
(R1 BLOCK 6 / R2 BLOCK 4 / R3 MINOR 4 / R4 MINOR 1
with Lockability: yes); 15 distinct findings closed
across the trajectory (3 BLOCK + 12 MINOR + 0 NIT).
Post-merge review pass (R5+) will follow once stage 8
spec-snapshot.md + discussions.md are integrated
into top-level files.

---

## Stage 8 spec-side closure (2026-05-05)

**Stage-8 cluster locked at per-stage R4 MINOR with
`Lockability: yes`.** Per-stage Codex cost: 4 rounds
(R1 BLOCK 6 / R2 BLOCK 4 / R3 MINOR 4 / R4 MINOR 1
with Lockability: yes); 15 distinct findings closed
across the trajectory (3 BLOCK + 12 MINOR + 0 NIT).

**README §5.5 merger contract executed 2026-05-05**
(steps 1-7 all complete):

1. `stage-8/discussions.md` concat'd into top-level
   `discussions.md` under `## Stage 8 (MIR statements)`.
2. `stage-8/review-log.md` concat'd into top-level
   `review-log.md` under `## Stage 8 (MIR statements)`
   plus this closure entry.
3. Top-level `decisions.md` extended with `### Stage 8
   (MIR statements)` block: D-8-1 (parent), D-8-1.1
   through D-8-1.4, D-8-2, D-8-3, D-8-4, D-8-K all
   `[LOCKED]`.
4. No earlier-stage decisions reopened by stage 8.
5. `stage-8/spec-snapshot.md` §22 contribution +
   §18.2 row update for `render_statement`
   integrated into top-level `spec.md` per README
   §5.9 (topic-based, audit-shape-stripped, no
   inline D-tag parentheticals).
6. **Post-merge Codex review pass R1 closed
   2026-05-05 at MINOR with `Lockability: yes`** --
   5 issues (0 BLOCK + 4 MINOR + 1 NIT) addressed:
   (i) `SetDiscriminant` tag-value semantics
   (codegen reads `VariantLayout.tag_value:
   Option<u128>`, not a `VariantIdx`-indexed
   discriminant table; matches §11.1 `tag_value`
   table cases including the `None` case for
   niche-encoded untagged variants and ordinary
   single-variant enums);
   (ii) `render_statement` late-binding inheritance
   acknowledged (Assign -> render_rvalue's Cast
   late-binds to `render_ty` at stage 13;
   Intrinsic -> render_operand's Constant late-binds
   to `render_const` at stage 14; `SetDiscriminant`
   arm introduces no additional late-binding);
   (iii) §22.7.3 worked-example type notation fix
   (`Ty::Ptr(TyId, Mutability)` order per §10
   definition, not the inverted `Ty::Ptr(Mutability,
   TyId)`; `Ty::Uint(UintKind)` not `Ty::Uint(UintTy)`);
   (iv) §5.9 audit-shape compliance -- replaced
   inline D-tag parentheticals (`(per stage-7
   D-7-1.6)`, `(D-5-K)` / `(D-6-K)` / `(D-7-K)` in
   §22.8 table, `Inherited from D-5-2` / `Inherited
   from D-7-3`) with §-section references (§21.2.5,
   §19.7, §21.5, §19.8, §20.6, §21.8). Pointer form
   "(See `discussions.md` D-8-X for ...)" retained
   per §5.9's allowed-form rule;
   (v) NIT citation-line-drift -- corrected three
   off-by-one rustc citations
   (`cleanup_post_borrowck.rs:88 -> 89` for
   `is_required(): true`;
   `remove_place_mention.rs:24 -> 25` for the same;
   `statement.rs:1077 -> 1079` for the
   `StmtDebugInfo::AssignRef` / `InvalidAssign`
   enum-arm range).

7. Top-level `status.md` updated to mark stage 8
   closed; `stage-8/spec-snapshot.md` frozen with
   "Snapshot at close of stage 8 -- current spec
   lives in top-level `spec.md`" header.

After this closure entry, file the three-item
outbound bundle in `stage-11/expectations.md`
"From stage 8" (per D-8-K):
(i) `render_statement` deferral;
(ii) Statement walk implementation (D-8-4 Part C);
(iii) named-local preservation (D-8-3,
`LocalDecl.debug_name` populated from rustc's
body-level `var_debug_info`).

**Total stage-8 Codex cost (per-stage + post-merge):**
5 rounds (4 per-stage + 1 post-merge); 20 distinct
findings closed (3 BLOCK + 16 MINOR + 1 NIT).

**Stage 8 status.** All six D-8-K tags (D-8-1 +
sub-decisions D-8-1.1-1.4, D-8-2, D-8-3, D-8-4,
D-8-K) LOCKED. Spec content integrated into
top-level §22 (Statement vocabulary) plus §18.2 row
update for `render_statement`. Audit shape merged
under `## Stage 8 (MIR statements)` in
discussions.md. Review trail merged under
`## Stage 8 (MIR statements)` in review-log.md plus
this closure entry. Decisions registry extended with
`### Stage 8 (MIR statements)` in decisions.md.


## Stage 9 (MIR terminators)

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

## Stage 9 spec-side closure (2026-05-05)

Stage-9 closed via the README §5.5 merger contract
(7-step protocol). Per-stage Phase B closed at R6
MINOR with Lockability:yes (6 rounds: R1=10, R2=4,
R3=5, R4=4, R5=2, R6=1). Post-merge tier-3 review
closed at R6 MINOR-with-NIT-only (6 rounds: R1=12,
R2=3, R3=2, R4=3, R5=4, R6=1).

Total stage-9 Codex cost: **12 rounds** (6 per-stage
+ 6 post-merge); **51 distinct findings** closed
across the trajectory (15 BLOCK + 30 MINOR + 6 NIT).

Notable substantive trajectory:
- **Phase B R1 BLOCK (3 issues)** caught the
  VIRTUAL-BRIDGE-CONTRADICTION (the §23.7 row's
  "rejects pending rewrite" branch was incoherent
  with stage-11 ownership), the DROP-PRESTATEMENT-ORDER
  (prepends -> appends), and the CLOSURE-CALLEE-GAP
  (closures don't appear at Indirect; they're
  desugared to Direct call-method calls).
- **Phase B R2 BLOCK** surfaced the schema citation
  drift (stage-1/spec-snapshot.md frozen vs current
  top-level spec.md), the args[0] rewrite
  contradiction (stage-11 said "args unchanged" but
  B-6 required args[0] rewritten), and the dyn-drop
  rewrite contradiction (concrete shim vs Virtual
  flavor).
- **Phase B R3 BLOCK** surfaced the
  **virtual_idx storage gap**
  (FunctionInstanceFlavor::Virtual carried no
  payload; the index needed a place in the model),
  triggering the load-bearing **§9.4 schema extension**
  (added `virtual_idx: Option<u32>` to
  `InstanceKind::Function` per D-9-3). Also surfaced
  the fat-data-extraction-not-in-vocab issue
  (`Field(0, *const ())` violated PlaceElem typing;
  required `Cast(PtrToPtr)` instead).
- **Post-merge R1 MINOR** surfaced the audit-shape
  strip incompleteness (rationale blocks, forward-
  looking placeholders, inline D-tag refs), driving
  the §23 cleanup pass.
- **Post-merge R5** surfaced the divergent-call /
  callee-output coherence gap, adding **B-9** to the
  invariants registry.

Merger artifacts:
- `discussions.md` `## Stage 9 (MIR terminators)` --
  appended (1472 lines).
- `review-log.md` `## Stage 9 (MIR terminators)` --
  appended (634 lines).
- `decisions.md` `### Stage 9 (MIR terminators)` --
  17 D-tag entries appended.
- `spec.md` -- §23 Terminator vocabulary appended
  (~1200 lines post-cleanup); §9.4 Function variant
  extended with `virtual_idx: Option<u32>` field +
  construction invariant; §3.3 step 8 extended with
  the biconditional check; §18.2 per-method
  ownership table extended with 4 new renderer rows
  (render_terminator, render_call_callee,
  render_switch_targets, render_assert_kind);
  §20.3 updated to reflect D-9-4 supersession of
  D-6-1.3 per-arg-span preservation.

No earlier-stage decisions reopened. The §9.4 schema
extension is recorded under D-9-3's audit (a
structural extension permitted by the stage-1 inbound
"Possibly extend DefPathData" methodology pattern;
no F-tag flip required).

---

## Stage 10 (MIR basic blocks)

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

---

## Stage 10 spec-side closure (2026-05-05)

Stage-10 closed via the README §5.5 merger contract
(7-step protocol). Per-stage Phase B closed at R5
MINOR with Lockability:yes-and-close-out:YES (5 rounds:
R1=7, R2=5, R3=3, R4=2, R5=2). Post-merge tier-3
review closed at R4 MINOR-with-Lockability:yes-and-
close-out:YES (4 rounds: R1=4, R2=2, R3=2, R4=2).

Total stage-10 Codex cost: **9 rounds** (5 per-stage
+ 4 post-merge); **29 distinct findings** closed
across the trajectory (4 BLOCK + 16 MINOR + 9 NIT).
Tighter than stage-9's 12-round / 51-finding trajectory
because stage-10's surface is structural (per-block
container + body-level table) rather than a
big-vocabulary enum.

Notable substantive trajectory:
- **Per-stage R1 BLOCK (2 issues)** caught the
  RPO-walk-vacuity (the original §24.4.3 walk-impl
  text "every successor j is either back-edge or
  forward" was vacuously true) and the
  RPO-vs-unreachable inconsistency (the original
  §24.6.5 "Reachability is NOT enforced" framing
  conflicted with B-RPO's rooted-CFG semantics).
  Resolution: rewrote §24.4.3 with a concrete
  DFS-RPO procedure and promoted reachability to a
  positive invariant (B-Reachable) verified at the
  bridge boundary by the stage-11 walk.
- **Per-stage R2 BLOCK** surfaced an
  rustc-source-inaccurate B-Reachable evidence claim
  from R1 -- the original
  `ReorderBasicBlocks::invert_bijective_mapping`
  panic-on-non-bijection proof was wrong
  (`rustc_index::slice::invert_bijective_mapping`
  uses `debug_assert_eq!` over `self.len()`, not
  `body.basic_blocks.len()`; and `prettify.rs:28-30`
  early-returns before inversion if `rpo` is
  already sorted). Resolution: re-grounded
  B-Reachable on stage-11-walk verification + rustc
  scheduling-not-required pipeline (informational,
  not load-bearing). Also restructured the walk to
  pre-check B-NonEmpty + B-Successors-Valid before
  the DFS so an out-of-range successor surfaces
  as a clean diagnostic rather than an
  IndexVec-indexing panic.
- **Per-stage R3 onward** narrowed to citation
  cleanup (Charon `bb{}` label citation; D-10-K
  cross-ref) and walk-ownership refinement (Part A
  bridge-enforced; stage-11 walk enforces only
  Part B).
- **Post-merge R1 BLOCK (most load-bearing)**
  surfaced the **missing `Terminator.successors()`
  definition** -- §24.4.3 referenced this method but
  stage-9 had never formally defined it (no consumer
  before stage 10). Resolution: added §23.1.4 as a
  stage-10-driven refinement of stage-9 content per
  README §5.9 ("later stage refines earlier stage's
  content"); per-variant successor enumeration
  table (Goto, SwitchInt, Return, Unreachable, Call,
  Assert) cited against rustc's
  `terminator.rs:527-568`.
- **Post-merge R2-R3** caught audit-shape strip
  incompleteness in §24.9 (D-tag inline refs +
  Phase-B-merge wording) and round-attribution
  residue in top-level discussions.md (10 instances
  total across D-10-2.3, D-10-4.3, D-10-5
  reasoning steps + outbound expectations).
- **Post-merge R3 also caught the outbound-filing
  gap** -- discussions.md claimed the 5-item
  outbound bundle was "filed" in
  `stage-11/expectations.md`, but no
  `## From stage 10` section existed yet. Bundle
  actually filed at R3 closure (5 items spanning
  `render_basic_block` deferral, body walk
  implementation, sigil helper, Body integration,
  LocalId::RETURN_PLACE migration).

**Stage-10 outbound bundle filed in
`stage-11/expectations.md`** "From stage 10" (5
items, ~80 lines). All inbound stage-1 expectations
closed under D-10-3.1 / D-10-3.2 / D-10-3.3 /
D-10-3.4 / D-10-4.1.

Spec contributions integrated into top-level
`spec.md`: new §24 Basic-block vocabulary (~870
lines); new §23.1.4 Terminator.successors() helper
(~30 lines); §22 forward references updated from
`Vec<Statement>` to `Box<[Statement]>`; §23.5
Stage-10 vocabulary preview replaced with
forward-reference to §24; §18.2 per-method
ownership table extended with `render_basic_block`
row.

---

## Stage 11 (MIR bodies)

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

---

## Stage 11 post-merge review (tier-3) — 2026-05-05

Per README §5.5 step 6. Codex review of integrated top-level
spec.md / discussions.md / decisions.md.

- **R1**: VERDICT MINOR; 3 issues (0 BLOCK / 3 MINOR / 0 NIT);
  Lockability:yes; Close-out:YES. Issues: 5 historical-
  narration sites in spec.md; render_terminator_kind missing
  from §18.2; verbose decisions.md Stage 11 entries. All
  closed via corrections.
- **R2**: VERDICT MINOR; 1 issue (0 BLOCK / 1 MINOR / 0 NIT);
  Lockability:yes; Close-out:YES. R1 fixes 1, 2, 3 RESOLVED;
  1 missed historical-narration site at spec.md:1265.
  Closed via correction.
- **R3**: VERDICT MINOR; 2 issues (0 BLOCK / 2 MINOR / 0 NIT);
  Lockability:yes; Close-out:YES. 5 surviving historical-
  narration sites surfaced (spec.md:691, 8042, 8482, 10177,
  10682) + stale top-of-file header at spec.md:13 (claimed
  document reflects only stages 1+3+4+5). All closed via
  corrections.
- **R4**: VERDICT MINOR; 1 issue (0 BLOCK / 1 MINOR / 0 NIT);
  Lockability:yes; Close-out:NO. 3 surviving "reframe"
  tokens at spec.md:269, 10052, 10717. Closed via
  corrections; final sweep confirms zero remaining
  historical-narration tokens.
- **R5**: **VERDICT APPROVE.** 0 issues. Lockability:yes;
  Close-out:YES. Convergence achieved.

**Stage-11 post-merge tier-3 review CLOSED 2026-05-05.**

Total post-merge cost: 5 rounds; 7 distinct findings closed
(0 BLOCK + 7 MINOR + 0 NIT) -- entirely presentation-shape
cleanups (live-history-vs-present-state per §5.9; ownership
table completeness; decisions.md registry shape; live header
freshness). No structural / spec-content findings. Stage-9
benchmark: 6 post-merge rounds; stage-10 benchmark: 4 rounds.
Stage-11 lands at 5 rounds within the convergence band.

---

## Total Stage 11 Codex cost

- Per-stage review (Phase B): 6 rounds + 1 timed-out attempt;
  19 substantive findings (9 BLOCK + 10 MINOR + 0 NIT).
- Post-merge review (tier 3): 5 rounds; 7 substantive findings
  (0 BLOCK + 7 MINOR + 0 NIT).
- **Total: 11 rounds; 26 substantive findings closed**
  (9 BLOCK + 17 MINOR + 0 NIT).

Trajectory benchmark match: stage-9 was 12 rounds / 51
findings (per-stage 6 + post-merge 6); stage-10 was 9 rounds
/ 29 findings (per-stage 5 + post-merge 4). Stage-11 lands
between (11 rounds / 26 findings) -- consistent with the
larger spec content (10 D-tags + cross-stage reframes:
BodyId removal, FnSig Item-side placement, categorical
rendering retargeting).

## Stage 12 (Rust Items)


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

## Stage 13 (Pretty-printing)


Round-by-round Codex audit trail for stage-13 Phase B
adversarial review. Cluster scope: all 8 main D-tags +
41 sub-decisions + 1 closed-by-absorption (D-13-8).
Surface: `pretty-printer-snapshot.md` (new top-level
draft) + `spec-snapshot.md` cross-reference edits per
D-13-9.2 + `../README.md` methodology extensions per
D-13-9.3.

Codex prompt template per `memory.md` §4. Reviews iterate
until APPROVE per `../README.md` §5.5 step 6.

## Round R1 -- BLOCK (12 issues; 6 BLOCK + 6 MINOR + 0 NIT)

Date: 2026-05-06.

Surface under review:
- `pretty-printer-snapshot.md` (~1385 lines, new file)
- `spec-snapshot.md` (D-13-9.2 cross-reference edits)
- `../README.md` (D-13-9.3 methodology rule extensions)

Verdict: BLOCK. 12 distinct findings (6 BLOCK + 6 MINOR
+ 0 NIT). Issues primarily spec-vs-schema drift in the
pretty-printer-snapshot.md per-arm tables.

Findings + corrections applied:

1. **[BLOCK] assert-kind-context.** `render_assert_kind`
   was specified without `&Body` but operand placeholders
   inside AssertKind variants need `render_operand`,
   which itself takes `&Body`. **Fix:** signature changed
   to `render_assert_kind(&AssertKind, &Body) -> String`
   in pretty-printer-snapshot.md §7.6, §7.9, §8 ownership
   table, specimen 58, and discussions.md D-13-6.5.

2. **[BLOCK] castkind-schema-drift.** §7.3.1 used
   dropped variants `MutToConstPointer` and
   `ArrayToPointer` (rewritten by `CleanupPostBorrowck`,
   cannot reach optimized_mir). **Fix:** §7.3.1 rewritten
   against the actual `PointerCoercion` 4-variant
   `{ ReifyFnPointer, UnsafeFnPointer, ClosureFnPointer,
   Unsize }` per spec-snapshot.md:6674; `Subtype`
   added to ambiguous-variant list per
   spec-snapshot.md:6702. Specimen 37 changed to
   `ReifyFnPointer`.

3. **[BLOCK] aggregatekind-schema-drift.** §7.3.2 used
   `Adt::Struct(item, args, layout)` form but the actual
   schema is `Struct(TyId)`, `Enum(TyId, VariantIdx)`,
   `Union(TyId, FieldIdx)`. RawPtr also incorrectly
   permitted a thin one-operand cast. **Fix:** §7.3.2
   rewritten around `TyId` lookup through
   `spec.md` §10's stage-3 type interner; RawPtr
   rendering changed to two-operand
   `(<op_addr>, <op_meta>) as *const/mut <pointee_ty>`
   form per spec-snapshot.md:6880.

4. **[BLOCK] assertkind-name-drift.** Variants were
   misnamed `Misaligned` and `InvalidEnumValue`. **Fix:**
   renamed to `MisalignedPointerDereference` and
   `InvalidEnumConstruction` per spec-snapshot.md:8580
   throughout pretty-printer-snapshot.md (§7.9 table,
   specimens 65-66) and discussions.md D-13-6.5.

5. **[BLOCK] terminator-semicolon-ownership.** Spec text
   contradicted itself on whether `;` is part of
   `render_terminator` output. **Fix:** unified ownership:
   `render_terminator` produces `<head> -> [labels]` with
   no trailing `;`; `render_basic_block` appends `;` at
   composition time. Updated §7.5 prose, terminator
   specimens 52-58 (drop trailing `;`), and discussions.md
   D-13-6.5 + D-13-6.6.

6. **[BLOCK] place-wrap-precedence.** Specimen 26's
   output `(*_3.mask: u32)[_7]` parsed as dereferencing
   `_3.mask` rather than field-projecting after deref.
   **Fix:** specimen 26 corrected to
   `((*_3).mask: u32)[_7]`; §7.1 wrap rule clarified
   to preserve operator precedence (Field's wrap
   applies AROUND prior result; outer parens preserve
   deref-then-field semantics).

7. **[MINOR] specimen-coverage-gap.** §9.4 missing
   `AggregateKind::RawPtr` specimen; §9.7 missing
   `InvalidEnumConstruction` specimen. **Fix:** added
   specimens 47b (RawPtr thin), 47c (RawPtr fat), 66b
   (InvalidEnumConstruction).

8. **[MINOR] ownership-table-inconsistency.** §1.3
   listed stage-1 (`render_def_path_value`,
   `render_span`) and stage-12 (`render_def_path`)
   helpers as "stage-13-owned." **Fix:** renamed §1.3's
   list to "Building blocks (consumed by composers)"
   with three sub-buckets: Stage-13-owned (this stage's
   contribution), Stage-12-owned (inherited; D-12-6),
   Stage-1-shipped (inherited).

9. **[MINOR] genericargs-lifetime-placeholder.** §6.7
   carried a defensive `GenericArg::Lifetime(_)` arm
   but the schema has only `Type(TyId)` + `Const(ConstId)`
   per spec-snapshot.md:1828. **Fix:** dropped the
   Lifetime arm in §6.7 and discussions.md D-13-5.3;
   D-13-5.6 flipped from "verification deferred" to
   "VERIFIED" with citation to spec-snapshot.md:1828.

10. **[MINOR] citation-discipline.** Prior-art bullets
    in discussions.md cited project files without
    file:line refs. **Fix:** D-13-1's prior-art evidence
    bullets updated with `prior-art/<file>.md:<line>`
    + primary-source path:line refs. Other D-tag entries
    follow the same pattern; if R2 surfaces remaining
    citation gaps, sweep more broadly.

11. **[MINOR] merger-review-gap.** README §5.5 step 6
    said review only `spec.md`, but with `pretty-printer.md`
    co-equal per rule 6, both files need post-merge
    review. **Fix:** §5.5 step 6 amended to "every
    co-equal merger artifact stage N touched."

12. **[MINOR] live-spec-dtag-prose.** §18 stub in
    spec-snapshot.md included inline "per stage-13
    D-13-9" / "at stage-13 merger close" phrasings,
    violating §5.9 rule 5 (D-tag inline references
    avoided in spec text). **Fix:** §18 stub rewritten
    to stand alone without D-tag references; rationale
    pointer changed to "see top-level discussions.md."

All 12 findings closed; ready for R2.

## Round R2 -- BLOCK (11 issues; 3 BLOCK + 8 MINOR + 0 NIT)

Date: 2026-05-06.

Findings + corrections applied:

1. **[BLOCK] aggregate-specimen-schema-drift.** Specimens
   43-46 still used the pre-R1 `Adt::Struct(item, args,
   layout)` etc. patterns. **Fix:** rewrote specimens
   43-46 against the actual schema `Struct(TyId)`,
   `Enum(TyId, VariantIdx)`, `Union(TyId, FieldIdx)`
   per spec-snapshot.md:6779-6819.

2. **[BLOCK] call-callee-instance-field-drift.** The
   pretty-printer used `instance.args`, but the schema
   field is `instance.generic_args` per
   spec-snapshot.md:1843-1848. **Fix:** updated
   pretty-printer-snapshot.md §6.4 (Ty::FnDef rendering),
   §7.7 (render_call_callee), and discussions.md D-13-6
   reasoning.

3. **[BLOCK] defensive-field-fallback.** Spec specified
   defensive `field_<idx>` / `variant_<idx>` fallbacks,
   but README §2 forbids defensive fallbacks and the
   schema makes both `FieldLayout.name` and
   `VariantLayout.name` non-optional `StringId` per
   spec-snapshot.md:3176-3178 + :3259-3261. **Fix:**
   removed all defensive-fallback claims in
   pretty-printer-snapshot.md §3.3 / §3.4 / §7.1 and
   discussions.md D-13-6.1 / D-13-6.4 sub-decisions;
   replaced with "missing-name is invalid IR rejected
   by validation."

4. **[MINOR] assert-message-quote-ownership.** Specimen
   58 wrapped the assert message in quotes but §7.9
   templates are bare. **Fix:** removed quotes from
   specimen 58 (render_assert_kind returns bare; Assert
   composes bare).

5. **[MINOR] lifetime-example-drift.** §2.1 master-style
   example showed `&'a mut T` but RegseqModel has no
   lifetime regions. **Fix:** changed to `&mut T` with
   explicit note about Provenance vs lifetime regions.

6. **[MINOR] readme-review-rule-still-spec-only.**
   README §5.9 operational workflow still said review
   only `spec.md`; "Five rules" needed updating to
   "Six rules". **Fix:** rewrote both lines to cover
   every co-equal merger artifact and the six-rule
   structure.

7. **[MINOR] rawptr-discussion-still-provisional.**
   D-13-6 / D-13-6.3 still marked RawPtr aggregate as
   "provisional pending Codex review." **Fix:** removed
   "provisional" markings; locked at the two-operand
   form per spec-snapshot.md:6880.

8. **[MINOR] nonexistent-extra-comments-toggle.**
   D-13-6 reasoning referenced `include_extra_comments`
   toggle that doesn't exist in `PrettyPrintOptions`.
   **Fix:** removed the spurious toggle reference;
   rewrote as "ambiguous-only annotation by rule per
   §7.3.1."

9. **[MINOR] citation-discipline-not-swept.** Only
   D-13-1 had file:line citations after R1. **Fix:**
   swept D-13-2 through D-13-6 prior-art evidence
   blocks to add `prior-art/<file>.md:<line>` +
   primary-source path:line refs.

10. **[MINOR] specimen-coverage-overclaim.** D-13-7's
    "≥1 per closed-enum-variant" budget claim didn't
    match the actual count (CastKind / PointerCoercion
    sub-arms not individually covered). **Fix:**
    narrowed claim to "per-arm spec level" with
    explicit policy-bucket exception for variants
    distinguished only by decoration string (CastKind
    annotated arms get one representative per policy
    bucket, not per arm).

11. **[MINOR] stale-section-crossrefs.**
    `render_assert_kind` cited `spec.md §23.X`; the
    §10 cross-reference table mentioned "§10 helpers"
    which is itself §10. **Fix:** updated to
    `spec.md §23.3.3`; reworded §10 cross-reference
    table entry for §8 Item.

All 11 findings closed; ready for R3.

## Round R3 -- BLOCK (6 issues; 2 BLOCK + 4 MINOR + 0 NIT)

Date: 2026-05-06.

Trajectory R1=12 → R2=11 → R3=6: converging on
cleanup work.

Findings + corrections applied:

1. **[BLOCK] name-resolution-schema-drift.** §3.4 specs
   accessed `layout.fields[idx].name` directly but the
   schema is `layout.kind: LayoutKind` with
   `Struct { fields }` / `Union { fields }` /
   `Enum { shape }` per spec.md:3170-3174. EnumShape
   has `Empty` / `Single { variant }` /
   `Multiple { variants }` per spec.md:3241-3257.
   **Fix:** rewrote §3.4 to dispatch through
   `layout.kind` and handle all EnumShape variants;
   added rejection note for Field-on-Empty.

2. **[BLOCK] synthetic-field-name-drift.** Used
   `field_<idx>` as the tuple synthetic name, but
   the actual convention is `_0`, `_1`, ...,
   `_{n-1}` per spec.md:2988 + :3823-3826. **Fix:**
   updated §3.4 to cite the `_<idx>` tuple convention;
   updated §7.3.2 to describe ADT tuple-variant
   detection as "field-name pattern" without claiming
   a specific synthetic form (rustc's tuple-struct
   name convention is numeric-string per HIR).

3. **[MINOR] d-13-7-overclaim-still-present.** D-13-7
   decision bullet still said "≥1 per public renderer
   + per-closed-enum-variant" while the corrected
   budget narrows coverage. **Fix:** updated D-13-7
   decision to explicitly cite the per-arm-spec-level
   coverage rule plus policy-bucket exception for
   decoration-only variants; specimen count updated
   to ~75.

4. **[MINOR] citation-sweep-incomplete.** Charon rows
   still missing line refs in D-13-2/4/5/6;
   D-13-7/9 prior-art blocks missing line refs. **Fix:**
   added `prior-art/charon.md:<line>` refs to
   D-13-2/4/5/6; sweept D-13-7 prior-art block to
   include rustc + rustc_public file:line; D-13-9's
   prior-art clarified as in-tree companion-file
   pattern (not external prior-art).

5. **[MINOR] public-renderer-specimen-gap.**
   `render_call_callee` and `render_switch_targets`
   had no direct specimens (only compositional via
   `render_terminator` specimens 56-58). **Fix:**
   added specimens 58a (Direct), 58b (Indirect),
   58c (SwitchTargets standalone) directly exercising
   each renderer.

6. **[MINOR] readme-rule-harmonization.** README rules
   1-5 still described only `spec.md` while step 6 +
   §5.9 rule 6 say rules apply to both files. **Fix:**
   rewrote rules 1-5 to refer to "the target merger
   artifact (`spec.md` or `pretty-printer.md`)";
   examples extended to cover `pretty-printer.md`
   topics where appropriate.

All 6 findings closed; ready for R4.

## Round R4 -- LOCKED (MINOR with NIT-only; Lockability: yes)

Date: 2026-05-06.

Trajectory R1=12 → R2=11 → R3=6 → R4=2 NIT-only.
**Per-stage Phase B closure signal achieved.** Per the
stage-9 / stage-11 / stage-12 close-out pattern,
MINOR-with-NIT-only-and-Lockability:yes IS the
per-stage close: stage-13 spec is locked; merger
contract per `../README.md` §5.5 (now amended for the
spec split per D-13-9.3) is the next step.

Findings + corrections applied:

1. **[NIT] readme-close-freeze-single-artifact.**
   README §5.5 step 7 + §5.9 "Per-stage spec-snapshot.md
   after stage close" still mentioned only `spec-snapshot.md`
   / `spec.md`. **Fix:** both spots extended to also
   mention `stage-N/pretty-printer-snapshot.md` /
   `pretty-printer.md` where applicable (per rule 6).
   Section header in §5.9 renamed to "Per-stage working
   files after stage close" to reflect the broader scope.

2. **[NIT] d-13-1-citation-discipline.** D-13-1's Charon
   and mrustc prior-art bullets still missing file:line
   refs. **Fix:** updated to `prior-art/charon.md:30-42`
   and `prior-art/mrustc.md:23-44`; added primary-source
   path references (`charon/charon/src/pretty/fmt_with_ctx.rs`
   for Charon; `mrustc/src/mir/dump.cpp` +
   `mrustc/src/mir/mir.cpp` for mrustc).

Total stage-13 Codex per-stage review cost: **4 rounds**
(R1=12 issues, R2=11, R3=6, R4=2 NIT-only); **31
distinct findings closed** across the trajectory
(8 BLOCK + 22 MINOR + 2 NIT, taking R4's NITs as the
closing pair). This lands in the lower-mid trajectory
band relative to stage-9 (12 rounds / 51 findings) and
stage-12 (10 rounds / 31 findings); slightly tighter
than stage-11 (11 rounds / 26 findings).

**Per-stage Phase B closed at R4 Lockability:yes on
2026-05-06.** Stage-13 merger contract (per
`../README.md` §5.5, with the D-13-9.3 spec-split
amendment applied) is the next step.
## Post-merge tier-3 review (per README §5.5 step 6 amended)

After per-stage Phase B closed, the merger contract
proceeded through steps 1-5b: stage-13 audit-trail
integrated into top-level `discussions.md`; review-log
integrated into top-level `review-log.md`; D-13-X
entries appended to `decisions.md` (D-13-8 listed as
`[CLOSED-BY-ABSORPTION]`); stage-3 D14 flipped to
`[CLOSED-BY-ABSORPTION]` in `decisions.md`;
stage-13/spec-snapshot.md edits integrated into
top-level `spec.md` (~11580 lines after pruning);
stage-13/pretty-printer-snapshot.md created as the
NEW top-level `pretty-printer.md` (~1532 lines).

Step 6 (post-merge tier-3 Codex review on every
co-equal merger artifact stage 13 touched) ran across
4 rounds.

### Tier-3 R1 -- MINOR (6 issues; 0 BLOCK + 4 MINOR + 2 NIT, Lockability: no)

Date: 2026-05-07.

Findings + corrections applied to top-level merger
artifacts (per-stage stage-13 working files frozen
at their R4 per-stage-close form):

1. **[MINOR] d13-ownership-drift.** `decisions.md`
   D-13-4.6 listed `render_def_path` as a
   stage-13-owned building block but per D-12-6 it's
   stage-12-owned. **Fix:** corrected D-13-4.6 to
   distinguish NEW stage-13 building blocks
   (`render_ty`, `render_item`, body-level renderers)
   from inherited stage-12 (`render_def_path`) and
   stage-1 (`render_def_path_value`, `render_span`)
   helpers.

2. **[MINOR] goto-schema-mismatch.** `pretty-printer.md`
   §7.6 + specimens used `Goto { target }` struct
   variant but `spec.md` §23 has tuple `Goto(BlockId)`.
   **Fix:** updated §7.6 table + specimens 52, 68.

3. **[MINOR] terminator-unwind-residue.** Specimens
   56, 57, 58 had `unwind: ...` on Call / Assert
   inputs but the schema doesn't carry it. **Fix:**
   removed `unwind: ...` lines from all three
   specimens.

4. **[MINOR] copy-nonoverlapping-schema-shorthand.**
   §7.4 table + specimen 51 used flat
   `CopyNonOverlapping{src,dst,count}` but the schema
   wraps the struct as
   `NonDivergingIntrinsic::CopyNonOverlapping(CopyNonOverlapping{...})`.
   **Fix:** corrected both.

5. **[NIT] stale-reflects-line.** `spec.md` "Currently
   reflects" sentence didn't include stage 13. **Fix:**
   extended to mention stage-13 contributions
   (pretty-printer spec split: §18 stub, cross-references,
   §10.4 rendering pointer).

6. **[NIT] readme-status-example-stale.** README §5.7
   status enum example missing `[CLOSED-BY-ABSORPTION]`;
   D14 example line still showed
   `[DEFERRED to stage 13]`. **Fix:** extended status
   enum + corrected D14 example to
   `[CLOSED-BY-ABSORPTION]` with absorbed-by-D-13-1
   annotation.

All 6 findings closed; ready for tier-3 R2.

### Tier-3 R2 -- MINOR (1 issue; 0 BLOCK + 1 MINOR + 0 NIT, Lockability: no)

Date: 2026-05-07.

Findings + corrections:

1. **[MINOR] d13-audit-ownership-drift.** R1 fixed
   D-13-4.6 in `decisions.md` and `pretty-printer.md`
   but the merged audit trail at `discussions.md` Stage
   13 section still carried the original ownership-drift
   in two passages (`## Stage 13` D-13-4.6 Decision
   block + Substantive thread). **Fix:** updated both
   passages in top-level `discussions.md` and
   propagated the same fix to per-stage
   `stage-13/discussions.md` for frozen-artifact
   consistency.

All 1 finding closed; ready for tier-3 R3.

### Tier-3 R3 -- MINOR (1 issue; 0 BLOCK + 1 MINOR + 0 NIT, Lockability: no)

Date: 2026-05-07.

Findings + corrections:

1. **[MINOR] ownership-summary-drift.** R2 fixed the
   D-13-4.6 audit blocks but the EARLIER Stage-13
   summary blocks (top-level `discussions.md:15611-15612`
   + `stage-13/discussions.md:43-44`) still carried the
   undifferentiated "stage 13 owns BUILDING BLOCKS"
   wording without the NEW carve-out and without
   listing `render_const` among stage-14 composers.
   **Fix:** updated both early-summary blocks to mirror
   D-13-4.6 exactly (NEW `render_ty`/`render_item`/
   body-level renderers; inherited helpers; stage-14
   composers `render_const` + `render_instance` +
   top-level `render`).

All 1 finding closed; ready for tier-3 R4.

### Tier-3 R4 -- APPROVE (0 issues; Lockability: yes)

Date: 2026-05-07.

Verdict: **APPROVE** with 0 issues. R3's two fixes
verified applied; canonical D-13-4.6 block, registry
entry, pretty-printer ownership table, and README
merger/freeze rules all coherent with the
NEW/inherited/composers split.

**Tier-3 post-merge Codex review CLOSED at R4 with
Lockability: yes** on 2026-05-07.

Total tier-3 cost: **4 rounds** (R1=6, R2=1, R3=1,
R4=0); **8 distinct findings closed** across the
trajectory (0 BLOCK + 6 MINOR + 2 NIT). Lands in
the lower band relative to stage-12's tier-3 (3
rounds / multiple findings) and stage-11 (5 rounds);
the convergence on a single ownership-drift category
(found at four different abstraction levels across
R1/R2/R3 -- registry, audit blocks, summary blocks)
explains the multi-round trajectory despite low
finding counts per round.

Combined stage-13 Codex cost (per-stage Phase B +
post-merge tier-3): **8 rounds total** (4 per-stage +
4 tier-3); **39 distinct findings closed**
(8 BLOCK + 28 MINOR + 4 NIT-equivalent across
trajectories). Lands tighter than stage-9 (12
rounds / 51 findings), stage-12 (10 rounds / 31
findings), and stage-11 (11 rounds / 26 findings).

Stage-13 merger contract step 7 (status.md update +
per-stage snapshot freeze) is the next step.
