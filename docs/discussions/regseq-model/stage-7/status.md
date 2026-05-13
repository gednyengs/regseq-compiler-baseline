# Stage 7 Status

Status: **STAGE 7 SPEC-SIDE CLOSED 2026-05-04.** All four
D-tags LOCKED at R5 APPROVE: D-7-1 (with sub-decisions
D-7-1.1-1.8 covering the 9-variant closed `Rvalue` surface
and per-variant payload shapes), D-7-2 (no Rvalue
interning), D-7-3 (Rvalue-site invariants + rejection
rules), D-7-K (`render_rvalue` deferred to stage 11).
Codex review trajectory: R1 BLOCK (7) -> R2 BLOCK (5) ->
R3 BLOCK (3) -> R4 MINOR (4) -> R5 APPROVE (0). Total
review cost: 5 rounds; 19 distinct findings closed across
the trajectory (5 BLOCK + 14 MINOR + 0 NIT). Spec content
in §21 of `spec-snapshot.md` plus §18.2 row update for
`render_rvalue`; audit shape in `discussions.md`; round-
by-round corrections in `review-log.md`.

**Ready for merger contract per README §5.5** (top-level
integration of `discussions.md`, `review-log.md`,
`decisions.md`, and `spec.md` §21 contribution + post-merge
Codex review pass).

Last updated: 2026-05-04 (Stage 7 spec-side closed at R5
APPROVE; ready for merger contract).

For top-level project status, see `../status.md`. For
project-wide invariants, see `../README.md`. For stage-7
invariants (workflow, principles, read order, Codex prompt
template), see `memory.md`. For the working spec, see
`spec-snapshot.md` (Phase B in flight: §21 added). For the
audit trail, see `discussions.md` (Phase B in flight:
audit-shape per locked D-tag). For the round-by-round
Codex audit trail, see `review-log.md` (lazy; created at
first round). For inbound forward refs (none filed; lazy),
see `expectations.md` (not yet created).

## Mission

Stage 7 specifies the `Rvalue` vocabulary that the bridge
fills from rustc's `mir::Rvalue<'tcx>` and that downstream
consumers (stages 8 statements, 9 terminators -- transitively
through `Statement`'s embedded `Rvalue`; codegen) read as the
RHS-expression surface of MIR. See `memory.md` Section 0 for
the full mission framing and the stage's role as third link
in the 5-11 MIR pipeline chain.

## Inbound expectations (from `expectations.md`)

**None filed at stage opening.** No earlier stage filed a
forward ref tagged "stage 7" / "stage-7" in its
`spec-snapshot.md` or `discussions.md`. Mentions of stage 7
in earlier stages' content are top-level pipeline notes
(stages 5-11 chain ordering), not forward refs filed against
stage 7. The `stage-7/` folder did not exist before this
opening; lazy creation happened with the stage's opening.

## Cluster split

**Four D-tags, single cluster.** Mirroring the stage-6
pattern, the four decisions are tightly coupled (variant
set, payload shapes, storage / interning, invariants all
sit on the same `Rvalue` enum) and benefit from cluster
review on the integrated content. Phase-A drafting did not
require subdivision — the eight D-7-1 sub-decisions
proceeded sequentially in conversation without unwieldy
discussion pressure.

**D-7-1 sub-decisions renumbered post-drafting.** The
original status.md proposal listed ten sub-decisions
(D-7-1.1-1.10); the cardinality decision (D-7-1.1)
collapsed two of them (the originally proposed D-7-1.5
ThreadLocalRef-payload and D-7-1.10 WrapUnsafeBinder-
payload) into rejection rules under D-7-3, since both
variants were dropped from the closed surface entirely.
Final D-7-1 sub-decision count: 8 (D-7-1.1 through
D-7-1.8).

The four D-tags (drafted state):

- **D-7-1: Rvalue variant set + per-variant payload shapes**
  (substantive). Closed `Rvalue` enum at 9 variants. Sub-
  decisions:
  - **D-7-1.1** [DRAFTED] -- 9-variant closed surface:
    `Use`, `Repeat`, `Ref`, `RawPtr`, `Cast`, `BinaryOp`,
    `UnaryOp`, `Discriminant`, `Aggregate`. Drop
    `CopyForDeref` and `ShallowInitBox` (cleanup-pass-
    discharged + codegen `bug!()`); drop `ThreadLocalRef`
    (DSL-surface claim: TLS unavailable to Linux/Zephyr
    drivers); drop `WrapUnsafeBinder` (nightly-experimental,
    zero corpus, no C-shape).
  - **D-7-1.2** [DRAFTED] -- `Use(Operand)` direct embed
    of stage-6 Operand. No curation.
  - **D-7-1.3** [DRAFTED] -- `Repeat(Operand, u64)` direct
    count, parallel to stage-3 `Ty::Array(elem_ty, n)`
    canonical form. Type-system-const not value-system; no
    `ConstId` interning.
  - **D-7-1.4** [DRAFTED] -- `Ref(BorrowKind, Place)`. Drop
    `Region` slot (compositional with stage-3 `Ty::Ref`
    region erasure). 2-variant `BorrowKind`
    (`SharedBorrow` / `UniqueBorrow`) reusing stage-3's
    discriminator. Drop rustc's `Fake` (cleanup-discharged
    -> D-7-3) and `MutBorrowKind` sub-flavours
    (codegen-collapsed via `bk.to_mutbl_lossy()`).
  - **D-7-1.5** [DRAFTED] -- `RawPtr(RawPtrKind, Place)`.
    2-variant `RawPtrKind` (`Const` / `Mut`). Bridge-side
    collapse: `FakeForPtrMetadata` -> `Mut` per rustc's
    "operationally equivalent to Mut" doc. Same bridge-
    lowering pattern as D-6-1.2's `RuntimeChecks` treatment.
  - **D-7-1.6** [DRAFTED] -- `Cast(CastKind, Operand, TyId)`.
    11-variant `CastKind` (`PointerExposeProvenance`,
    `PointerWithExposedProvenance`, `PointerCoercion(...)`,
    `IntToInt`, `FloatToInt`, `FloatToFloat`, `IntToFloat`,
    `PtrToPtr`, `FnPtrToPtr`, `Transmute`, `Subtype`).
    4-variant `PointerCoercion` sub-enum (`ReifyFnPointer`,
    `UnsafeFnPointer`, `ClosureFnPointer`, `Unsize`); drop
    `ArrayToPointer` and `MutToConstPointer` (cleanup-
    discharged -> D-7-3). Drop `CoercionSource` (diagnostic-
    only). Drop `hir::Safety` from
    `ReifyFnPointer`/`ClosureFnPointer` (structurally
    duplicated with destination `Ty::FnPtr(FnSigId)`'s
    signature).
  - **D-7-1.7** [DRAFTED] -- `BinaryOp(BinOp, Operand,
    Operand)` flat (Charon precedent; ~16-byte enum
    size penalty acceptable; rustc's Box was a size-budget
    optimization rustc needs and we don't). `UnaryOp(UnOp,
    Operand)`. Preserve all 26 BinOp variants and 3 UnOp
    variants verbatim.
  - **D-7-1.8** [DRAFTED] -- `Discriminant(Place)` direct;
    `Aggregate(AggregateKind, Box<[Operand]>)` with 7-
    variant `AggregateKind`: `Array(TyId)`, `Tuple(TyId)`,
    `Struct(TyId)`, `Enum(TyId, VariantIdx)`,
    `Union(TyId, FieldIdx)`, `Closure(TyId)` (identity view
    per §13.1), `RawPtr(TyId, Mutability)` (pointee TyId).
    Adt split three-way (Struct/Enum/Union) per stage-3
    NominalKind taxonomy. Drop `Coroutine`/`CoroutineClosure`
    (type-rejected at stage 3 §17 + StateTransform-
    discharged). Drop `UserTypeAnnotationIndex` slot
    (D-6-1.3 precedent + cleanup-pass clearing).

- **D-7-2** [DRAFTED] -- No Rvalue interning; Rvalue lives
  inline at use sites in stage-8 `Statement::Assign(Place,
  Rvalue)`. Compositional with D-5-2 (Place) and D-6-2
  (Operand).

- **D-7-3** [DRAFTED] -- Rvalue-site invariants. Two parts:
  rejection rules (7 rows: cleanup-discharged + DSL-surface
  + experimental + type-rejected) + positive invariants
  (operand wellformedness inheriting D-5-2/D-6-3, TyId
  interning, aggregate operand count matches layout per
  AggregateKind variant, Repeat count u64 trivially nonneg,
  VariantIdx/FieldIdx in-range). Walk implementation defers
  to stage 11. F4 / F4-2 cross-build identity OUT of scope
  (body-local content, parallel to D-5-2/D-6-3).

- **D-7-K** [DRAFTED] -- `pretty.rs::render_rvalue` deferred
  to stage 11 alongside `render_place` (D-5-K) and
  `render_operand` (D-6-K). Outbound expectation to file in
  `stage-11/expectations.md` "From stage 7" at lock time --
  `render_rvalue` needs `LocalDecl.ty` for AggregateKind
  name resolution (parallel to render_place's needs).

D-7-2, D-7-3, D-7-K are small standalone decisions; D-7-1
carries the substantive variant-set + payload-shape +
rejection-rule commitments.

## Locked decisions

**STAGE 7 CLUSTER LOCKED 2026-05-04 at R5 APPROVE** (zero
findings; clean APPROVE). D-7-1 (with sub-decisions
D-7-1.1-1.8) plus D-7-2, D-7-3, D-7-K all flipped
[DRAFT] -> [LOCKED] in `discussions.md`.

- **D-7-1 [LOCKED]** -- 9-variant closed `Rvalue` surface
  (Use, Repeat, Ref, RawPtr, Cast, BinaryOp, UnaryOp,
  Discriminant, Aggregate). Eight sub-decisions cover
  per-variant payload shapes:
  - D-7-1.1: Variant cardinality (drop CopyForDeref,
    ShallowInitBox, ThreadLocalRef, WrapUnsafeBinder).
  - D-7-1.2: `Use(Operand)` direct embed.
  - D-7-1.3: `Repeat(Operand, u64)` direct count.
  - D-7-1.4: `Ref(Provenance, Place)` (region-erased;
    reuses stage-3 `Provenance`).
  - D-7-1.5: `RawPtr(Mutability, Place)` with bridge
    collapse `FakeForPtrMetadata -> Mutability::Not`
    (matches rustc codegen's `to_mutbl_lossy()`).
  - D-7-1.6: `Cast(CastKind, Operand, TyId)` with
    11-variant CastKind + 4-variant PointerCoercion;
    drops CoercionSource and hir::Safety annotations.
  - D-7-1.7: `BinaryOp(BinOp, Operand, Operand)` flat
    storage (Charon precedent); `UnaryOp(UnOp, Operand)`;
    full 26-variant BinOp + 3-variant UnOp preserved.
  - D-7-1.8: `Discriminant(Place)` direct;
    `Aggregate(AggregateKind, Box<[Operand]>)` with
    7-variant AggregateKind (Array / Tuple / Struct /
    Enum / Union / Closure / RawPtr); three-way Adt split
    per stage-3 NominalKind taxonomy.

- **D-7-2 [LOCKED]** -- No Rvalue interning; inline at
  use sites in stage-8 `Statement::Assign(Place, Rvalue)`.
  Compositional with D-5-2 / D-6-2.

- **D-7-3 [LOCKED]** -- Rvalue-site invariants. Part A:
  7-row rejection table (cleanup-pass-discharged + DSL-
  surface + experimental + type-rejected). Part B: positive
  invariants (Operand-Wellformed, TyId-Interned,
  Aggregate-Operand-Count per AggregateKind variant,
  Repeat-Count-Wellformed, VariantIdx / FieldIdx in-range);
  rules specified at stage 7, walk-enforcement deferred to
  stage 11. Part C: walk-implementation deferred to stage
  11 (same split as D-5-2). Part D: F4 / F4-2 cross-build
  identity OUT of scope (body-local content).

- **D-7-K [LOCKED]** -- `pretty.rs::render_rvalue` deferred
  to stage 11 alongside `render_place` (D-5-K) and
  `render_operand` (D-6-K). Aggregate field/variant names
  resolve directly from AggregateKind's per-variant TyId
  via the §12 type interner; the operand-rendering subtree
  is what forces deferral. Outbound expectation to file in
  `stage-11/expectations.md` "From stage 7" at the merger
  contract step.

## Open decisions

(none -- cluster locked at R5 APPROVE)

## Codex review cadence (status)

**Cluster trajectory:**

| Round | Verdict | Issues | BLOCK / MINOR / NIT |
|-|-|-|-|
| R1 | BLOCK | 7 | 2 / 5 / 0 |
| R2 | BLOCK | 5 | 2 / 3 / 0 |
| R3 | BLOCK | 3 | 1 / 2 / 0 |
| R4 | MINOR | 4 | 0 / 4 / 0 |
| R5 | APPROVE | 0 | 0 / 0 / 0 |

Total Codex cost: 5 rounds. 19 distinct findings closed
(5 BLOCK + 14 MINOR + 0 NIT).

Round-by-round audit trail with verdicts, findings, and
corrections lives in `review-log.md`.

Prompt template: see `memory.md` §4.

## Stage-7-specific deferred items

(none expected; all four decisions are intra-stage with the
exception of D-7-K which defers `render_rvalue` to stage 11
-- a routine deferral pattern from D-5-K and D-6-K)

## Next concrete unit of work

**Stage 7 spec-side work is complete.** Hand off to the
merger contract per README §5.5:

1. Integrate `stage-7/discussions.md` into top-level
   `discussions.md` under a `## Stage 7` header
   (mechanical concat).
2. Integrate `stage-7/review-log.md` into top-level
   `review-log.md` under a `## Stage 7` header
   (mechanical concat).
3. Append decision entries to top-level `decisions.md`
   under `## Stage 7`: D-7-1 (parent), D-7-1.1 through
   D-7-1.8 (LOCKED), D-7-2 (LOCKED), D-7-3 (LOCKED),
   D-7-K (LOCKED).
4. (No earlier-stage decisions reopened by stage 7.)
5. Integrate `stage-7/spec-snapshot.md` §21 contribution
   into top-level `spec.md` per README §5.9 (topic-based,
   audit-shape-stripped, no-D-tag-inline-references rules);
   add a §21 Rvalue vocabulary section in topic-appropriate
   location plus the §18.2 row update for `render_rvalue`.
6. Run a Codex post-merge review pass on the integrated
   top-level `spec.md`. Iterate to APPROVE.
7. Update top-level `../status.md` to mark stage 7 closed
   and add a "Snapshot at close of stage 7 -- current spec
   lives in top-level `spec.md`" header to
   `stage-7/spec-snapshot.md`.

After the merger contract closes, file outbound expectation
in `stage-11/expectations.md` "From stage 7" for
`render_rvalue` deferral (per D-7-K) -- needs `LocalDecl.ty`
for embedded Place / Operand rendering (the operand-
rendering subtree is the deferral driver; AggregateKind
field/variant names resolve from the per-variant TyId
without LocalDecl.ty).

After all merger work closes, the next stage in the 5-11
dependency chain (stage 8: MIR statements) can open.
