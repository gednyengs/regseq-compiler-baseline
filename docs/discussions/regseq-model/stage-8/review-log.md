# Stage 8 -- Codex Review Log

Round-by-round adversarial review trail for stage-8
spec-snapshot.md §22 (Statement vocabulary) cluster
(D-8-1 through D-8-K). Per `memory.md` §1 Phase B
workflow + §4 Codex prompt template.

Each round records: prompt scope, verdict, numbered
issues, corrections applied, and the next-round trigger.

---

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
