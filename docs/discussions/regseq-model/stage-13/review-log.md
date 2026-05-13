# Stage 13 -- Review Log

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

---

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
