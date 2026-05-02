# SESSION-A review log

Round-by-round Codex adversarial-review trail for SESSION-A's
Phase-B spec commit (D5 `Dynamic` lowering + D6 closure naming).
Each round records scope, verdict, issue list, and the
corrections applied before the next round.

Reviews are invoked via `scripts/python/codex_runner.py` per the
project's CLAUDE.md contract. Each round uses a fresh prompt
(Codex has no session continuation); re-reviews include a brief
summary of what was changed since the prior round.

The structured output contract for every round:

- First line: `VERDICT: <BLOCK | MINOR | APPROVE>`.
- Numbered issues list, each tagged `[BLOCK | MINOR | NIT]` with
  a decision reference, file:line citation, 1-3-sentence
  description, and a 1-sentence suggested fix.
- Last line: `TOTAL: <N> issues (<count of BLOCK> BLOCK / <count
  of MINOR> MINOR / <count of NIT> NIT)`.

The merge session, after closing all parallel SESSIONs at
APPROVE, conducts a separate cross-cluster sweep (Round 4 in the
top-level cadence per `stage-3-status.md`).

Reviewer is given:
- Canonical stage-3 documents as locked context: `vocabulary.md`,
  `discussions.md`, `review-log.md`, `findings-rustc-draft.md`,
  `expectations.md`, `prior-art/<project>.md`, `stage-3-status.md`,
  `memory.md`.
- Stage-1 documents as locked context where SESSION-A relies on
  stage-1 commitments: `stage-1/vocabulary.md`.
- SESSION-A addendums as the live source for D5 / D6:
  `tmp/SESSION-A/vocabulary-SESSION-A.md`,
  `tmp/SESSION-A/discussions-SESSION-A.md`.

The reviewer is instructed to treat SESSION-A addendums as the
live spec for D5 and D6; canonical files are reference context
(except for the wording correction at `vocabulary.md:1294-1297`,
which the reviewer should evaluate as already-superseded by
`vocabulary-SESSION-A.md` Section 5).

---

## Round 1 -- D5 + D6 initial review

**Date.** 2026-04-29.

**Scope.** SESSION-A's Phase-B spec commit for D5 (Dynamic
lowering) and D6 (closure naming). Live spec:
`tmp/SESSION-A/vocabulary-SESSION-A.md` and
`tmp/SESSION-A/discussions-SESSION-A.md`. Locked context: stage-3
canonical files, stage-1 vocabulary, prior-art notes, methodology
context.

**Codex run.** `--effort high`, `--timeout 1500`, output captured
at `/tmp/codex_session_a_d5d6_review_r1.txt`.

**Verdict.** `BLOCK` -- 7 BLOCK + 6 MINOR + 0 NIT (13 total
issues).

**Issues (verbatim from Codex output).**

1. *[BLOCK] D5.2 (vocabulary-SESSION-A.md:158).* Stage-3 SESSION-A
   assigns dispatch rewriting and `Cast::Unsize` vtable-instance
   registration to stage 11, but stage-1:68 / stage-1:893-895
   reserve trait-object lowering for stages 9/12/13.
2. *[BLOCK] D5.2 (discussions-SESSION-A.md:403).* Forward
   expectation claims the rewritten indirect call is a normal
   `Call` with `FnPtr` callee, but stage-1:873-876 says calls
   carry `InstanceId`. A loaded vtable fn pointer is not an
   `InstanceId` under that shape.
3. *[BLOCK] D5.3-A (vocabulary-SESSION-A.md:220).* SESSION-A
   introduces a per-(Trait, projection_bindings) vtable type
   `Item`, while stage-1:905-918 / 1059-1063 reserve a
   per-(Trait, SelfTy) vtable Static Item with hash including
   `self_ty_canonical_hash`. SESSION-A does not distinguish the
   new vtable-type item from the already-reserved vtable-static
   item.
4. *[BLOCK] D5.3-A (vocabulary-SESSION-A.md:223).* The spec
   defers the concrete vtable DefPath naming convention to
   `discussions-SESSION-A.md`, but the discussion only states the
   vtable type is a nominal `Ty::Struct` and gives no naming
   convention. The worked example uses a `DefPathData::Closure`-style
   anon segment, while stage-1 defines `Closure` as a
   closure-expression segment and says vtable scopes may require
   a stage-1 revision.
5. *[BLOCK] D5.3-A (vocabulary-SESSION-A.md:230).* SESSION-A says
   the synthetic vtable `Ty::Struct` is interned via the existing
   `translate_adt` flow, with layout from
   `tcx.layout_of(<vtable struct ty>)`. But `translate_adt`
   computes layout from a rustc `rustc_ty`, which the synthesized
   vtable type does not have; stage-1:893-895 deferred the
   precise vtable struct `Ty` to stage 9/12.
6. *[BLOCK] D5 worked example (vocabulary-SESSION-A.md:729).*
   Vtable field has erased type `uint32_t (*read)(const void
   *self)` and is initialized with `PL011_read`, but stage-1:597-603
   says trait impl methods are lifted with concrete receivers
   like `&Foo`. The C example assigns a concrete-receiver fn to
   an erased-receiver fn-pointer slot.
7. *[BLOCK] D5.3-B (vocabulary-SESSION-A.md:316).* D5.3-B says
   trait-object `data` mutability follows the enclosing
   `Ref`/`RawPtr`, and the example uses `SharedBorrow`, but the
   emitted C struct has `void *data` (not `const void *data`);
   `vocabulary.md:1537-1550` reserves `const T*` for shared
   borrows.
8. *[MINOR] D5.3-A (vocabulary-SESSION-A.md:289).* CLR cited as
   supporting nominal vtable structs, but `prior-art/rustc-codegen-clr.md:227-228`
   says CLR's `TyKind::Dynamic` returns a `ClassRef` named
   `"Dyn"` with no fields. The cited evidence supports
   fat-pointer synthesis, not nominal vtable structs.
9. *[MINOR] D5.3-A (vocabulary-SESSION-A.md:237).*
   `RegseqModel.vtables: HashMap<...>` lacks a serde behavior
   spec; stage-1:1274-1299 requires deterministic serde shapes.
10. *[MINOR] D6 worked example (vocabulary-SESSION-A.md:805).*
    Example concludes "FnOnce-typed only" from `impl FnOnce`
    return bound, but `findings-rustc-draft.md:125-127` only says
    `ClosureKind` is encoded in closure args, not that a return
    bound determines it.
11. *[MINOR] D6.2 (vocabulary-SESSION-A.md:550).* Cited
    stage-1:487-491 for verbose anon-namespace rendering, but
    those lines only state unnamed variants carry no `StringId`;
    the verbose `{namespace#N}` rule is at stage-1:550-552.
12. *[MINOR] D5.1 (vocabulary-SESSION-A.md:82).* Cited
    stage-1:608-611 for both six-variant `ItemKind` and
    trait/assoc-type erasure; trait erasure is at :608-611,
    assoc-type erasure at :605-607, six-variant `ItemKind` at
    :625-627.
13. *[MINOR] Region-erasure primer (vocabulary-SESSION-A.md:73).*
    Region-erasure primer is in `discussions-SESSION-A.md`
    Section 3, not Section 2 as cited. Primer also claims stage-1
    `FnSig` has no region slots without citation; stage-1 only
    has `sigs` placeholder, expanded shape is stage 11.

**Corrections applied before Round 2.**

The corrections re-architect SESSION-A's scope per user
direction: SESSION-A commits **stage-3-only** content; the
synthesis details, Call-shape design, and pass implementation
that previously bled into stage-12/9/11 territory are forwarded
to those stages' `expectations.md` files so the receiving stages
start with SESSION-A's design analysis instead of from scratch.

*Substantive (BLOCK-driven) changes.*

1. *Issue 1 (D5.2 stage 11 vs stage-1's 9/12/13).* Reframed D5.2
   from "stage 11 owns dispatch rewriting" to "work distributes
   across stage 3 (this SESSION), stage 9 (Call shape), stage 11
   (body-level pass), stage 12 (Item synthesis)." The stage-3
   commitment is just `Ty::Dynamic.vtable_ty: TyId` and bridge
   eager population; downstream layers each get a focused
   forward-expectation respecting stage-1's reservations.

2. *Issue 2 (post-rewrite Call shape vs `InstanceId`).* Forwarded
   to `stage-9/expectations.md` as a design question. SESSION-A
   surfaces two candidate solutions (Virtual InstanceKind variant
   per rustc precedent, or widened `Call.callee` enum) and
   evaluation criteria; stage-9 picks.

3. *Issues 3 + 4 (vtable Item granularity, DefPath naming).*
   D5.3-A reframed from "introduce a per-(Trait, projections)
   nominal `Ty::Struct` Item in stage 3" to "stage-3 commits to
   the contract that `vtable_ty: TyId` resolves to a
   `Ty::Struct`; the Item details (DefPath, hash, fields, layout,
   surrounding instance/shim Items, dedup table) are pinned in
   `stage-12/expectations.md`." The two-Item structure (vtable
   type Item + vtable instance Item) and the vtable shim
   Function Items (issue 6) are documented in the stage-12
   forward-expectation. Stage-1's per-(Trait, SelfTy) instance
   reservation is preserved unchanged; the per-(Trait,
   projections) type Item is a new SESSION-A-recognised
   addition forwarded to stage-12.

4. *Issue 5 (translate_adt flow doesn't fit synthesised vtable
   layout).* Resolved by deferring to stage-12: the
   forward-expectation specifies that the vtable type's layout
   is synthesised from rustc's `tcx.vtable_entries(trait_ref)`
   sequence (drop_glue + size + align + per-method fn-ptrs +
   supertrait ptrs), not via `tcx.layout_of` round-trip. Stage-3
   commits only to the surface contract that the bridge can
   construct and intern the resulting `Ty::Struct`.

5. *Issue 6 (erased-receiver shim).* Forwarded to stage-12 as
   "Vtable shim Function Items per (Trait, ConcreteImpl,
   method)" in section C of the stage-12 forward-expectation,
   citing rustc's `InstanceKind::VTableShim` precedent. The D5
   worked example in `vocabulary-SESSION-A.md` Section 3.1 now
   shows the shim explicitly in the C output
   (`Driver_read__shim_PL011`).

6. *Issue 7 (const qualifier on shared trait-object data).*
   D5.3-B now spells out three fat-pointer typedef variants
   (shared / mutable / raw) with appropriate qualifications
   matching `vocabulary.md:1537-1550`. The Section 3.1 worked
   example updated to use `const void *data` for `&dyn Driver`.

*Citation / presentation (MINOR-driven) changes.*

7. *Issue 8 (CLR citation recast).* The CLR prior-art entry in
   D5.3-A is now framed as a divergence/fat-pointer-synthesis
   precedent rather than vtable-struct precedent; the CLR-only
   "Dyn ClassRef with no fields" line is cited explicitly to
   show the divergence point.

8. *Issue 9 (`RegseqModel.vtables` serde behavior).* The
   forward-expectation to stage-12 now specifies the
   `#[serde(skip)]` + rebuild-on-deserialize policy mirroring
   `TyInterner.lookup`.

9. *Issue 10 (closure example ClosureKind clarity).* Replaced
   the original `move |x| x + n` example (which is actually
   `Fn`-typed) with an unambiguously `FnOnce`-typed example
   (`move || s.into_bytes()`, consuming the captured `String`).

10. *Issue 11 (stage-1 verbose-rendering citation).* Replaced
    the stage-1:487-491 citation with stage-1:545-554 in D6.2.

11. *Issue 12 (D5.1 multi-citation for stage-1 erasure facts).*
    Split the stage-1 reasoning citation into separate references:
    :608-611 (trait declaration erasure), :605-607 (associated
    type erasure), :625-627 (six-variant `ItemKind`).

12. *Issue 13 (region-erasure primer references).* Updated
    in-text references from "Section 2" to "Section 3" of
    `discussions-SESSION-A.md`. Removed the unsupported "Stage-1
    `FnSig` has no region slots" claim from the primer's
    "already implicit" evidence list, replacing it with a
    forward-looking note that stage-11 (which expands the
    `FnSig` shape) inherits the invariant.

*Files updated.*

- `vocabulary-SESSION-A.md`: rewritten with reduced D5.3-A scope,
  expanded D5.3-B fat-pointer treatment, fixed citations,
  fixed worked example C output (const qualifier, vtable shim).
- `discussions-SESSION-A.md`: rewritten with updated D5/D6
  discussion entries, region-erasure primer fixes, expanded
  Section 4 with three forward-expectation entries (stage 9, 11,
  12).
- `review-log-SESSION-A.md`: this Round 1 entry plus the present
  corrections-applied summary.

Round 2 will re-evaluate the corrected addendums plus verify
that the forward-expectations are well-formed and that no new
issues arise from the corrections.

---

## Round 2 -- D5 + D6 re-review after re-architecting

**Date.** 2026-04-29.

**Scope.** Same as Round 1, with Round-1 corrections applied
(scope-shrunk SESSION-A; forward-expectations to stages 9, 11,
12; citation/presentation fixes).

**Codex run.** `--effort high`, `--timeout 1500`, output captured
at `/tmp/codex_session_a_d5d6_review_r2.txt`.

**Verdict.** `BLOCK` -- 3 BLOCK + 4 MINOR + 0 NIT (7 total
issues). Down from 13 in Round 1.

**Issues (verbatim from Codex output).**

1. *[BLOCK] D5.3-B (vocabulary-SESSION-A.md:337).* Round-1 issue
   7 only partially fixed: shared C example uses `const void
   *data` at lines 782-785, but the live spec still uses
   pre-D7/D8-lock IR shapes `Ref(_, Dynamic, _)` /
   `RawPtr(Dynamic, _)` at vocabulary-SESSION-A.md:337-343 and
   discussions-SESSION-A.md:86-98. Locked D7/D8 defines
   `Ty::Ref(TyId, Provenance)` and `Ty::Ptr(TyId, Mutability)`
   at `vocabulary.md:574-580`, with bridge rules at
   `vocabulary.md:599-601`.

2. *[BLOCK] D5.3-B (vocabulary-SESSION-A.md:361).* "Three typedef
   variants" omits `*const dyn Trait`: D5.3-B specifies shared
   borrow / mutable borrow / raw `*mut dyn Trait` only, while
   locked D7/D8 keeps raw-pointer mutability as
   `Ptr(_, Mutability)` and maps `*const T` to `const T *`
   separately from `*mut T` at `vocabulary.md:1936-1937`. Leaves
   codegen for `Ty::Ptr(t_dyn, Mutability::Not)` unspecified.

3. *[BLOCK] D5.3-B (vocabulary-SESSION-A.md:350).* Typedef
   naming convention is under-keyed: D5.3-B names fat-pointer
   typedefs as `trait_obj_<Trait>`, but the same section says
   vtable/fat-pointer emission is per `(Trait,
   projection_bindings)`. Two trait objects with the same
   principal trait but different associated-type bindings would
   collide.

4. *[MINOR] D5.2 (discussions-SESSION-A.md:438).* Stage-9
   forward expectation's option (a) conflates rustc's callable
   `InstanceKind::Virtual` with Regseq's public `InstanceKind`:
   stage-1 keeps Regseq `InstanceKind` as six emission variants,
   and uses rustc shim kinds as a hidden `fn_shim_disambig` for
   Function instances at `stage-1/vocabulary.md:735-754`.
   Presenting a new public Virtual `InstanceKind` is misleading.

5. *[MINOR] D5.3-A (discussions-SESSION-A.md:575).* Stage-12
   vtable layout expectation says to include rustc Vacant slots
   in the vtable `Ty::Struct` field sequence but does not say
   how Vacant entries should be represented (omitted fields,
   padding/reserved fields, placeholder pointer fields).
   Stage-11 later depends on method-index field lookup, so
   preserving or remapping indexes is a real design requirement.

6. *[MINOR] D5.3-B (vocabulary-SESSION-A.md:346).* Provenance
   citation is wrong: cites `vocabulary.md:1537-1550`, but those
   lines are D3 prior-art, not pointer qualification policy.
   The relevant locked text is D7/D8 at `vocabulary.md:570-601`
   and the C qualification worked example at
   `vocabulary.md:1923-1951`.

7. *[MINOR] D5.3-A (discussions-SESSION-A.md:642).* Round-1
   issue 9's `#[serde(skip)]` rebuild policy citation is wrong:
   says rebuild rule is at `vocabulary.md:1324-1325` (D3
   walkthrough text); actual rule is at
   `vocabulary.md:1760-1804`.

**Corrections applied before Round 3.**

The Round-2 issues fall into two categories:

*A. Out-of-date IR shapes (issues 1, 2, 6).* The canonical
`vocabulary.md` was updated since SESSION-A started: D7/D8 is
now LOCKED at Section 1.8 (lines 570-737), introducing the
`Ref(TyId, Provenance)` and `Ty::Ptr(TyId, Mutability)` shapes
(Mutability dropped from Ref; RawPtr renamed to Ptr; raw
pointers carry Mutability per stage-1 semantics, with no
`Provenance::Raw` variant). All SESSION-A references to the
pre-D7/D8 3-field `Ref` and `RawPtr` were updated to the new
shapes. The fat-pointer typedef table was extended from three
variants to four (`&dyn Trait`, `&mut dyn Trait`, `*const dyn
Trait`, `*mut dyn Trait`) with the appropriate `data`-field
qualifications per `vocabulary.md:1923-1951`. Citation
`vocabulary.md:1537-1550` replaced with `:570-601` (D7/D8
decision) plus `:1923-1951` (C qualification worked example).

*B. Forward-expectation refinements (issues 3, 4, 5, 7).*
- Issue 3: Added a "Typedef-name uniqueness" sub-section to
  D5.3-B that specifies the per-(Trait, projection_bindings)
  uniqueness obligation and delegates concrete name mangling to
  codegen (which derives the C name from stage-12's
  discriminating DefPath plus the pointer-kind qualifier).
- Issue 4: Stage-9 forward-expectation option (a) recast from
  "add Virtual variant to public InstanceKind" to "use
  fn_shim_disambig discriminator on existing Function
  InstanceKind variant per stage-1's framework
  (stage-1/vocabulary.md:735-754, :786-805)." The public
  six-variant InstanceKind closure stays intact.
- Issue 5: Vacant slot representation pinned in stage-12
  forward-expectation: explicit placeholder fn-pointer fields
  with NULL initialiser (not omitted, not padding), so that
  method-index lookup in stage-11 aligns with rustc's
  vtable_entries position.
- Issue 7: Citation corrected from
  `vocabulary.md:1324-1325` to `vocabulary.md:1760-1804` (the
  TyInterner serde / rebuild section).

The wording-correction line numbers in vocabulary-SESSION-A.md
Section 5 also updated (1294-1297 -> 1689-1697 to reflect the
D7/D8-merge line shift). The corrected "before" text in the
wording-correction quote uses `Ty::Ptr` (which is what the
current canonical text says post-D7/D8 merge), not the
pre-merge `Ty::RawPtr`.

Round 3 will re-evaluate after these corrections.

---

## Round 3 -- D5 + D6 third review

**Date.** 2026-04-29.

**Scope.** Same as Round 2, after Round-2 corrections (locked
D7/D8 shape adoption, fat-pointer typedef table extension to
four pointer-kind variants, stage-9 forward-expectation option
(a) recast, Vacant slot policy, citation fixes).

**Codex run.** `--effort high`, `--timeout 1500`, output captured
at `/tmp/codex_session_a_d5d6_review_r3.txt`.

**Verdict.** `BLOCK` -- 3 BLOCK + 2 MINOR + 0 NIT (5 total
issues). Down from 7 in Round 2.

Trajectory: 13 -> 7 -> 5.

**Round-2 verification.** Codex confirmed 5/7 Round-2 fixes are
fully applied (R2-1, R2-2, R2-5, R2-6, R2-7); 2/7 are partially
fixed and regress in Round-3 issues 1-3 (R2-3 typedef-name
uniqueness; R2-4 Virtual-InstanceKind recast).

**Issues (verbatim from Codex output).**

1. *[BLOCK] D5.3-B (vocabulary-SESSION-A.md:833).* The corrected
   typedef policy says four pointer-kind variants with different
   `data` qualification, but the worked example still says the
   trait-object typedef is emitted "once per (Trait,
   projection_bindings)" -- cannot represent both `const void
   *data` and `void *data` without losing constness or colliding
   typedef names. Fix: key fat-pointer typedef emission by
   `(Trait, projection_bindings, pointer-kind)`.

2. *[BLOCK] D5.2 (vocabulary-SESSION-A.md:177).* Round-2 issue 4
   not fully fixed: D5.2 distribution table still says "New
   `Virtual` `InstanceKind` variant", and stage-11 expectation
   in discussions-SESSION-A.md:524-526 still says "Virtual
   InstanceKind variant or widened Call.callee shape." Stage 1
   keeps public InstanceKind to six variants
   (stage-1/vocabulary.md:820-849, :869-871).

3. *[BLOCK] D5.2 (discussions-SESSION-A.md:444).* The recast
   stage-9 option (a) says codegen recognises a Virtual
   `fn_shim_disambig`, but stage 1 explicitly says the shim
   discriminator is **not** exposed as a public Instance field
   (stage-1/vocabulary.md:747-754). Fix: state that option (a)
   requires an explicit public-discriminator stage-1 revision,
   or drop in favour of widened `Call.callee`.

4. *[MINOR] D5.3-A (discussions-SESSION-A.md:664).* Stage-12
   schema names `TraitDefPathHash` and calls it a stage-1
   concept, but stage 1 defines `DefPathHash` only and uses
   `trait_def_path_hash` as a component name only. Fix: use
   `DefPathHash` for the principal trait key.

5. *[MINOR] Region-erasure primer (discussions-SESSION-A.md:407).*
   Citation gap: primer cites `vocabulary.md:67` as the `Ty::Ref`
   shape, but :67 is now `Ptr` (post-D7/D8 reordering) and
   `Ref(TyId, Provenance)` is at :68. Fix: cite :68 or :67-68.

**Corrections applied before Round 4.**

- *Issue 1.* D5 worked example at vocabulary-SESSION-A.md:833
  rewritten: vtable struct typedef remains per `(Trait,
  projection_bindings)`; trait-object (fat-pointer) typedef
  emission now explicitly keyed by `(Trait, projection_bindings,
  pointer-kind)`. Rationale spelled out (the four pointer-kind
  variants carry different data-field qualifications and cannot
  share a typedef).

- *Issue 2.* Both residual mentions of "Virtual InstanceKind
  variant" updated:
  - vocabulary-SESSION-A.md:177 (D5.2 distribution table) now
    reads "Stage-9 picks between two design candidates -- a
    public `Instance` discriminator addition (parallel to rustc's
    `Virtual` `InstanceKind`, requiring a stage-1 revision per
    `stage-1/vocabulary.md:747-754`) or a widened `Call.callee`
    shape."
  - discussions-SESSION-A.md:524-526 (stage-11 expectation)
    similarly recast to "a public Instance discriminator
    addition ... or a widened Call.callee shape."

- *Issue 3.* Stage-9 forward-expectation option (a) substantively
  rewritten. The old recast that said "use existing
  `fn_shim_disambig` discriminator" is replaced. The new (a)
  acknowledges that *both* sub-paths -- (a-i) adding a public
  `Virtual` variant to InstanceKind, or (a-ii) promoting
  `fn_shim_disambig` to public -- require stage-1 revisions per
  the spec-mutability rule at stage-1/vocabulary.md:919-927.
  Option (b) (widened `Call.callee`) is recast as the
  stage-9-only path. Evaluation criteria refined.

- *Issue 4.* All capitalised `TraitDefPathHash` type names
  replaced with `DefPathHash`. Lowercase `trait_def_path_hash`
  (component name in hash compositions) preserved -- those match
  stage-1's usage at :928-930.

- *Issue 5.* Region-erasure primer citation updated from
  `vocabulary.md:67` to `vocabulary.md:68` (with the D7/D8
  decision still cited at :570-601).

Round 4 will re-evaluate after these corrections.

---

## Round 4 -- D5 + D6 fourth review

**Date.** 2026-04-29.

**Scope.** Same as Round 3, after Round-3 corrections (worked-
example typedef-key extension, residual Virtual InstanceKind
references reframed, stage-9 option (a) substantively recast,
TraitDefPathHash -> DefPathHash, region-erasure primer citation
:67 -> :68).

**Codex run.** `--effort high`, `--timeout 1500`, output captured
at `/tmp/codex_session_a_d5d6_review_r4.txt`.

**Verdict.** `MINOR` -- 1 MINOR + 0 BLOCK + 0 NIT (1 total
issue). Down from 5 in Round 3.

Trajectory: 13 -> 7 -> 5 -> 1.

**Round-3 verification.** Codex confirmed Round-3 issues 1-4
are fully addressed (citing
vocabulary-SESSION-A.md:833-840 for issue 1,
vocabulary-SESSION-A.md:177 for issue 2 part one,
discussions-SESSION-A.md:532-537 for issue 2 part two,
discussions-SESSION-A.md:445-463 for issue 3,
discussions-SESSION-A.md:675-679 for issue 4). Round-3 issue 5
(region-erasure primer) is partially fixed -- the main primer
citation was updated correctly, but a second cross-ref citation
within the same primer's "Cross-refs" sub-section was missed.

**Issue (verbatim from Codex output).**

1. *[MINOR] Region-erasure primer
   (discussions-SESSION-A.md:408).* Round-3 issue 5 is only
   partially fixed: the main primer citation now correctly
   points to vocabulary.md:68 and vocabulary.md:570-601 at
   discussions-SESSION-A.md:345-346, but the cross-ref still
   cites vocabulary.md:67 as Ty::Ref. Locked vocabulary.md:67-68
   shows Ptr at 67 and Ref at 68. Fix: change to :68 (or :67-68
   if both pointer rows are intended).

**Corrections applied before Round 5.**

- Issue 1: changed
  `discussions-SESSION-A.md:408` from `vocabulary.md:67` to
  `vocabulary.md:68`. The primary primer citation at :345-346
  was already correct from Round 3's fix; the cross-ref is now
  consistent with it.

Round 5 expected to APPROVE.

---

## Round 5 -- D5 + D6 fifth review

**Date.** 2026-04-29.

**Scope.** Same as Round 4, after Round-4 correction (single
cross-ref citation `:67` -> `:68` at
discussions-SESSION-A.md:408).

**Codex run.** `--effort high`, `--timeout 1500`, output captured
at `/tmp/codex_session_a_d5d6_review_r5.txt`.

**Verdict.** `MINOR` -- 1 MINOR + 0 BLOCK + 0 NIT (1 total
issue).

Trajectory: 13 -> 7 -> 5 -> 1 -> 1.

**Round-4 verification.** Codex confirmed the cross-ref citation
fix (Round-4 issue 1) is now consistent with the primary primer
citation.

**Issue (verbatim from Codex output).**

1. *[MINOR] D5.3-B (vocabulary-SESSION-A.md:353).* D5.3-B
   says `dyn Trait` data-field qualifications follow locked
   D7/D8 and the C-qualification policy, but the table reduces
   variants to only `const void *data` vs `void *data`. Locked
   D7/D8 also includes `UnsafeCell` const-drop and `restrict`
   handling for `&mut` where uniqueness is preserved
   (vocabulary.md:1941-1950); the trait-object table
   underspecifies how those qualifiers apply or are
   intentionally suppressed.

**Corrections applied before Round 6.**

- Issue 1: Added a "Trait-object override on D7/D8's
  UnsafeCell + restrict policy" sub-section in D5.3-B
  (vocabulary-SESSION-A.md), explaining why neither adjustment
  applies to the type-erased trait-object `data` field. The
  rationale: codegen cannot inspect the erased pointee for
  UnsafeCell content (so no type-aware const-drop), and
  cannot establish non-aliasing on the erased pointee at the
  trait-object level (so no restrict). The four-variant table
  remains as a deliberate simplification at the type-erased
  level; per-(Trait, projection_bindings) codegen overrides or
  post-devirtualisation upgrades remain available downstream.
  The vtable field is unaffected (always `static const`).

Round 6 expected to APPROVE.

---

## Round 6 -- D5 + D6 final review

**Date.** 2026-04-29.

**Scope.** Same as Round 5, after Round-5 correction (D5.3-B
trait-object override on D7/D8's UnsafeCell + restrict policy).

**Codex run.** `--effort high`, `--timeout 1500`, output captured
at `/tmp/codex_session_a_d5d6_review_r6.txt`.

**Verdict.** `APPROVE` -- 0 BLOCK + 0 MINOR + 0 NIT (0 total
issues).

**Trajectory.** 13 (BLOCK) -> 7 (BLOCK) -> 5 (BLOCK) ->
1 (MINOR) -> 1 (MINOR) -> 0 (APPROVE). Six rounds.

**Round-5 verification.** Codex confirmed the D5.3-B trait-object
override sub-section is in place at vocabulary-SESSION-A.md:358-390,
explicitly suppressing UnsafeCell const-drop and restrict
adjustments for trait-object data fields per the rationale that
the underlying type is erased at the trait-object level.

**SESSION-A closed at APPROVE.** Phase-B spec commit is final;
the merge session can splice the D5/D6 blocks into canonical
stage-3 documents and file the forward-expectations into stage 9,
11, and 12 expectations.md files per the splice instructions in
vocabulary-SESSION-A.md Section 0 and discussions-SESSION-A.md
Section 0.

No further review rounds are scheduled for SESSION-A. The
post-SESSION cross-cluster sweep (Round 4 in the top-level
stage-3 cadence per stage-3-status.md) runs after all parallel
SESSIONs complete and is the responsibility of the merge session.


