# Stage 5 -- Codex Review Log

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
