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
