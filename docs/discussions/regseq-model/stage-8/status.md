# Stage 8 Status

Status: **STAGE 8 SPEC-SIDE CLOSED 2026-05-05.** All
six D-8-K tags LOCKED at R4 MINOR with explicit
`Lockability: yes` verdict per the stage-6 precedent.
D-8-1 (parent + 4 sub-decisions D-8-1.1 through
D-8-1.4), D-8-2, D-8-3, D-8-4, D-8-K all flipped
[DRAFTED] -> [LOCKED] across status.md + discussions.md
2026-05-05. Total per-stage Codex cost: 4 rounds
(R1 BLOCK 6 issues / R2 BLOCK 4 / R3 MINOR 4 / R4
MINOR 1 with Lockability: yes); 15 distinct findings
closed across the trajectory (3 BLOCK + 12 MINOR + 0
NIT).

**Ready for the README §5.5 merger contract** for
top-level integration (`../spec.md`,
`../discussions.md`, `../review-log.md`,
`../decisions.md`) plus post-merge Codex review
pass and outbound-expectation filing in
`../stage-11/expectations.md` "From stage 8".

(Original drafting state, retained for context.) All
six D-8-K tags drafted in conversation: D-8-1 (parent)
+ sub-decisions D-8-1.1 (3-variant closed surface
`Assign`, `SetDiscriminant`, `Intrinsic`; 12-row
reject-on-encounter table covering 10 distinct dropped
rustc variants, with split rows for fake-borrow
`Assign` vs the surviving `Assign` general case and
for `Coverage(BlockMarker | SpanMarker)` vs
`Coverage(non-marker)`; `RemoveStorageMarkers` was
flagged as an enable-gate open question during Phase-A
discussion and was resolved via
`crates/regseq-driver/src/callbacks.rs:25-66` +
`pass_manager.rs:213-239` -- `mir_enable_passes`
overrides `is_enabled`, so the pass DOES run under
our `mir_opt_level=0` pipeline), D-8-1.2 (`Assign(Place,
Rvalue)` flat tuple), D-8-1.3 (`SetDiscriminant(Place,
VariantIdx)` flat tuple, target type
`Ty::Enum { item, args, layout }` per §10 -- direct
match on the §10 `Ty` taxonomy, not via a
`Ty::Adt(NominalId)` umbrella that doesn't exist),
D-8-1.4 (nested `Intrinsic(NonDivergingIntrinsic)`,
`CopyNonOverlapping{src, dst, count}` named-fields
struct, flat throughout); D-8-2 (no Statement
interning); D-8-3 (`Statement { kind, span: Span }`
wrapper, drop scope and debuginfos, named-local
preservation outbound to stage 11); D-8-4 (5-part
invariants + rejection registry; Part B carries 8
positive invariants -- B-0a/B-0b/B-0c inherited +
B-1-B-5 stage-8-introduced); D-8-K
(`render_statement` deferred to stage 11 with three-
item outbound bundle).

**Phase-B drafting complete**; spec-snapshot.md §22
(~860 lines), discussions.md (~1330 lines), §18.2
per-method ownership table updated for
`render_statement`. **Codex review trajectory**:
R1 BLOCK (2/4/0; 6 issues addressed) ->
R2 BLOCK (1/3/0; 4 issues addressed) ->
R3 MINOR (0/4/0; status.md-only stale Phase-A details;
4 issues addressed) -> R4 MINOR (0/1/0; status.md-
only; 1 issue addressed) with `Lockability: yes`.
**LOCKED 2026-05-05.**

Last updated: 2026-05-05 (Stage 8 spec-side LOCKED
at R4 MINOR with Lockability: yes; ready for merger
contract).

For top-level project status, see `../status.md`. For
project-wide invariants, see `../README.md`. For stage-8
invariants (workflow, principles, read order, Codex prompt
template), see `memory.md`. For the working spec, see
`spec-snapshot.md` (Phase-A drafting starts on first
locked decision). For the audit trail, see
`discussions.md` (lazy; created when the first decision
lands). For the round-by-round Codex audit trail, see
`review-log.md` (lazy; created at first round). For
inbound forward refs (none filed; greenfield),
`expectations.md` is not created.

## Mission

Stage 8 specifies the `Statement` vocabulary that the
bridge fills from rustc's `mir::StatementKind<'tcx>` and
that downstream consumers (stage 10 basic blocks --
`BasicBlock.statements: Vec<Statement>`; stage 11 bodies;
stage 14 backends) read as the sub-terminator instruction
surface of MIR. Stage 8 is the **fourth link** in the
5-11 bottom-up dependency chain (places -> operands ->
rvalues -> **statements** -> terminators -> basic blocks
-> bodies). Per the spec-before-implementation discipline
(`../README.md` Methodology rules), stage-8 deliverables
are spec-only; implementation lands in the
end-of-phase-1 sweep.

See `memory.md` Section 0 for the full mission framing.

## Inbound expectations (from `expectations.md`)

**None filed at stage opening.** Cross-references in
earlier stages' content
(`../stage-6/spec-snapshot.md:5681`,
`../stage-6/spec-snapshot.md:5745`,
`../stage-7/spec-snapshot.md:5699`,
`../stage-7/spec-snapshot.md:5763`,
`../stage-7/spec-snapshot.md:6126`) mention that
operand-level spans fall back to "the containing
Statement's `SourceInfo` at stage 8" -- pipeline notes,
not formal forward refs filed against stage 8. The
`stage-8/` folder did not exist before this opening; lazy
creation happened with the stage's opening.
`expectations.md` is therefore not created (no content to
track).

The pipeline notes constrain D-8-3 (Statement wrapper
struct: must preserve span material such that stage-9
terminator-side and stage-11 body-side rendering can lift
spans to embedded operands and Places).

## Cluster split

**Single cluster, six D-tags** (mirrors the stage-6 /
stage-7 single-cluster pattern; the six decisions are
tightly coupled around the `Statement` enum and its
wrapper struct, benefit from cluster-level review on
integrated content, and Phase-A drafting does not require
subdivision). Sub-decisions of D-8-1 sequence
through Phase-A discussion as four sub-questions.

The six D-tags (proposed state, post-evidence-pass +
post-prior-art-pass; Phase-A discussion will refine the
variant cardinality, payload shapes, wrapper struct,
rejection rules):

- **D-8-1** [LOCKED 2026-05-05] -- `Statement` variant
  set + per-variant payload shapes (substantive). All
  four sub-decisions D-8-1.1-1.4 drafted in Phase-A
  conversation 2026-05-04. Rustc's 13-variant
  `StatementKind<'tcx>` is reduced to a closed surface for
  RegseqModel after applying our pipeline's required +
  whitelisted cleanup-pass discharge. Sub-decisions:
  - **D-8-1.1** [LOCKED 2026-05-05] -- Variant cardinality.
    **Closed 3-variant surface: `Assign`, `SetDiscriminant`,
    `Intrinsic`.** The 10 distinct dropped rustc
    `StatementKind` variants reject-on-encounter at the
    bridge with diagnostics citing the responsible pass /
    absent flag, captured in D-8-4 Part A's 12-row
    rejection table (12 rows because `Coverage` splits
    into Group 1 marker arms and Group 3 non-marker arms,
    and `Assign(_, Rvalue::Ref(_, BorrowKind::Fake, _))`
    accounts separately from the surviving general
    `Assign`). The `RemoveStorageMarkers` enable-gate
    question raised during the kickoff evidence pass was
    resolved during Phase-A discussion:
    `crates/regseq-driver/src/callbacks.rs:25-66` shows
    `mir_enable_passes = regseq_mir_passes()` registers
    `("RemoveStorageMarkers", true)`, and rustc's pass
    manager `pass_manager.rs:213-239` consults
    `mir_enable_passes` as an override that bypasses the
    `is_enabled` gate. Therefore `RemoveStorageMarkers`
    DOES run under our `mir_opt_level=0` pipeline,
    `StorageLive` / `StorageDead` are stripped before
    optimized_mir, and the corresponding rows in D-8-4
    Part A stand.
    Detailed breakdown follows below for completeness:
    Drop
    `FakeRead` / `AscribeUserType` /
    `BackwardIncompatibleDropHint`
    (`CleanupPostBorrowck` NOPs them; `RemovePlaceMention`
    sweeps the resulting Nops). Drop `PlaceMention`
    (`RemovePlaceMention` required, default-on; gated
    only by `!mir_preserve_ub` which is off by default).
    Drop `Nop` (swept by `RemovePlaceMention`,
    `RemoveStorageMarkers`, and `SimplifyLocals-final`).
    Drop `StorageLive` / `StorageDead` (stripped by
    `RemoveStorageMarkers`, force-on via the
    `mir_enable_passes` whitelist as resolved above).
    Drop `Retag` (only with `-Z mir-emit-retag`; out of
    pipeline). Drop `ConstEvalCounter` (only inserted by
    `CtfeLimit` on const-eval bodies; driver bodies don't
    receive it). Drop `Coverage` (only with
    `-C instrument-coverage`; driver builds don't enable
    coverage). **Closed surface (proposal): `Assign`,
    `SetDiscriminant`, `Intrinsic` -- 3 variants.** Each
    dropped variant reject-on-encounter at the bridge
    with a diagnostic citing the required / whitelisted
    pass that should have discharged it (or the absent
    build flag). Peer comparison range: rustc 13;
    rustc_public 12 (drops only `BackwardIncompatibleDropHint`);
    Charon 9; mrustc 8 (peer-narrowest after
    RegseqModel's candidate). Phase-A consideration:
    mrustc's 8-variant surface drops `SetDiscriminant`
    (folded into `RValue::EnumVariant`) and routes
    intrinsics through `Terminator::Call(...)`; Charon's
    9-variant surface drops the generic `Intrinsic(...)`
    variant (Assume fans out to `Assert{...}`).
    RegseqModel's 3-variant candidate sides with rustc /
    rustc_public on keeping `SetDiscriminant` and
    `Intrinsic`-as-Statement; Phase-A must justify these
    over mrustc's and Charon's tighter alternatives
    (or accept a tighter alternative).
  - **D-8-1.2** [LOCKED 2026-05-05] -- `Assign` payload
    shape. **`Assign(Place, Rvalue)` flat tuple** -- no
    Box (rustc's Box is solely for the
    `static_assert_size!(StatementKind, 16)` budget at
    `[rustc_middle/src/mir/syntax.rs:1753]` which
    RegseqModel does not inherit; same flat-`BinaryOp`
    precedent as stage-7 D-7-1.7). **Type-equality
    invariant `place_ty(lhs) == rvalue_ty(rhs)` lands as
    D-8-4 Part B-1** with walk-enforcement deferred to
    stage 11 (parallel to D-5-2 / D-7-3 walk-impl
    deferrals); no `assign_ty` slot on the variant (type
    is recoverable from existing stage-3 / stage-5 /
    stage-7 vocabulary). **No drop-suppression carrier**:
    `Assign` does NOT call drop on the previous LHS value
    per `[rustc_middle/src/mir/syntax.rs:312-317]`; drop
    is the explicit Drop terminator's job at stage 9
    (compositional with stage-9 drop, not a parallel flag).
    Detailed breakdown follows below for completeness:
    Three sub-questions: (a) flat `(Place, Rvalue)` vs
    `Box<(Place, Rvalue)>` -- rustc Boxes for the 16-byte
    `StatementKind` size assert
    [`rustc_middle/src/mir/syntax.rs:1753`]; rustc_public
    flattens; mrustc flattens; Charon flattens. Per
    stage-7's flat-`BinaryOp` precedent (rustc's Box was
    a size-budget optimization rustc needs and we don't),
    prefer flat. (b) type-equality invariant --
    `Assign(p, rv)` requires
    `place_ty(p) == rvalue_ty(rv)` (rustc validator pin
    [`rustc_mir_transform/src/validate.rs:1467-1582`];
    "Assignments in which the types of the place and
    rvalue differ are not well-formed" --
    `rustc_middle/src/mir/syntax.rs:324`); inherit from
    D-7-3 with stage-8-specific statement-site invariant.
    (c) post-drop-elaboration drop-suppression
    interaction (no special case needed: `Assign` does
    NOT call drop on the previous LHS value
    [`syntax.rs:312-322`] -- drop is the explicit drop
    terminator's job at stage 9).
  - **D-8-1.3** [LOCKED 2026-05-05] -- `SetDiscriminant`
    payload shape. **`SetDiscriminant(Place, VariantIdx)`
    flat tuple** -- no Box, no struct-with-named-fields
    (matches stage-7 precedent of tuple for small 2-3-field
    payloads: D-7-1.4 `Ref(Provenance, Place)`, D-7-1.5
    `RawPtr(Mutability, Place)`, D-7-1.8 `Aggregate(AggregateKind,
    Box<[Operand]>)`; rustc's Box on `place` is the 16-byte
    size-budget artefact, not inherited). **Invariants land
    in D-8-4 Part B**: B-2 `variant_index <
    variant_count(place_ty(p))` reading
    `Ty::Enum.layout`'s `LayoutKind::Enum.shape` cases per
    §11.1; B-3 target-type must be `Ty::Enum { item, args,
    layout }` per §10 (compositional with the §10 `Ty`
    taxonomy: coroutines already rejected at stage 3 §15.1;
    `ty::Alias(ty::Opaque, ..)` normalized away by required
    `PostAnalysisNormalize` pass per
    `[rustc_mir_transform/src/lib.rs:642]`; `Ty::Struct{..}`
    and `Ty::Union{..}` have no discriminant to write so
    they reject at the bridge). Walk-enforcement deferred
    to stage 11. **No redundant Item / Layout slot on the
    variant**: place type is fully recoverable from
    `place_ty(p)` via stage-3 / stage-5 / §10 vocabulary at
    render/codegen time (matches stage-7 Aggregate's
    TyId-lookup-via-§12-type-interner pattern).
    Detailed breakdown follows below for completeness:
    Three sub-questions: (a) struct vs flat tuple
    -- rustc uses `{ place: Box<Place>, variant_index:
    VariantIdx }`; rustc_public flattens to `{ place:
    Place, variant_index: VariantIdx }`; Charon uses
    `SetDiscriminant(Place, VariantId)` flat tuple.
    Prefer flat (Place is already a 16-byte-ish struct;
    Box was rustc's size budget per the 16-byte assert).
    Phase-A resolution: the project chose flat tuple
    (cf. stage-7 `Ref(Provenance, Place)` /
    `RawPtr(Mutability, Place)` / `Aggregate(AggregateKind,
    Box<[Operand]>)` precedent for 2-3-field payloads
    with distinct types). (b) variant_index in-range invariant
    (parallel to D-7-3 Aggregate-Operand-Count). (c)
    target type constraint -- rustc allows on `ty::Adt |
    ty::Coroutine | ty::Alias(Opaque, _)` per validator
    [`validate.rs`]; we type-rejected coroutines at
    stage 3 §17, so it is enum-only at the bridge.
    Cleanup-discharged for non-enum target via stage-3
    type rejection (compositional with stage-3, not a
    parallel `target_kind` flag).
  - **D-8-1.4** [LOCKED 2026-05-05] -- `Intrinsic`
    payload shape. (Sub-question (a) "lift to Call vs
    keep as Statement variant" was already settled by
    D-8-1.1's confirmation that the closed 3-variant
    surface includes `Intrinsic` -- placement-as-Statement
    is locked.) **Nested `Intrinsic(NonDivergingIntrinsic)`**
    with sub-enum `Assume(Operand) | CopyNonOverlapping(CopyNonOverlapping)`
    and sub-struct `CopyNonOverlapping { src: Operand, dst:
    Operand, count: Operand }` (named struct fields are
    correctness-essential here -- src/dst are both Operand
    and order-confusable; named fields prevent silent
    wrong-direction copy bugs at the bridge translation
    site and stage-14 codegen site). **Flat throughout the
    sub-tree** (no Box on `Intrinsic` arm; no Box on
    `CopyNonOverlapping` payload) -- same flat-vs-Box
    reasoning as D-8-1.2 (a) and D-8-1.3 (a). The nested-
    vs-fanout choice preserves rustc's "non-diverging
    rustc-internal hint primitives" semantic grouping and
    future-proofs against new sub-variants (rustc may add
    more `NonDivergingIntrinsic` arms later -- they land
    as sub-enum extensions, not Statement-enum changes).
    **Invariants land in D-8-4 Part B**: B-4
    `Intrinsic::Assume(op)` requires `operand_ty(op) ==
    Ty::Bool`; B-5 `Intrinsic::CopyNonOverlapping{src, dst,
    count}` requires src/dst to be deref-able pointer-like
    types -- `Ty::Ptr` or `Ty::Ref` (the RegseqModel-
    representable subset of rustc's `builtin_deref(true)`
    set; `Box<T>` is also accepted by rustc's validator
    but stage-3 §15.1 rejects `Box<T>` at type translation
    so Box-typed operands cannot reach stage 8) -- with
    identical pointee types and count `Ty::Uint(Usize)`
    (rustc validator pins per
    `[rustc_mir_transform/src/validate.rs:1517-1549]`).
    Walk-enforcement deferred to stage 11.
    Detailed breakdown follows below for completeness:
    Four sub-questions: (a) lift to Call vs keep as
    Statement variant -- mrustc lifts ALL intrinsics
    through `Terminator::Call(CallTarget::Intrinsic{...})`;
    rustc keeps non-diverging intrinsics as Statement
    `Intrinsic` to "avoid adding a new block and
    terminator for simple intrinsics"
    [`syntax.rs:448-451`]; rustc_public mirrors rustc;
    Charon fans out (Assume to Assert; CopyNonOverlapping
    to dedicated boxed variant) at Statement-level.
    Phase-A resolution: keep as Statement (per D-8-1.1's
    confirmation that the closed 3-variant surface
    includes `Intrinsic`); the "lift to Call" alternative
    inflates basic-block count for straight-line code AND
    collides with Q7's regseq DSL intrinsic-recognition
    namespace at Calls. (b) flatten `NonDivergingIntrinsic`
    into top-level `Statement` variants vs keep nested.
    Charon fans out (per (a)); rustc / rustc_public keep
    nested. Phase-A resolution: keep nested -- preserves
    rustc's "non-diverging hint primitive" semantic
    grouping; keeps the closed Statement surface at 3
    variants (fanout would expand to 4 without semantic
    gain); future-proofs against new
    `NonDivergingIntrinsic` arms (rustc may add more --
    they land as sub-enum extensions, not Statement-enum
    changes). (c) `CopyNonOverlapping` payload -- struct
    `{ src: Operand, dst: Operand, count: Operand }`
    (rustc_public matches rustc; flat 3-Operand) -- chosen
    for src/dst order-disambiguation correctness (all
    three fields are Operand; tuple shape would be
    silently order-confusable). (d) Box-or-flat -- rustc
    Boxes the `NonDivergingIntrinsic`; rustc_public
    flattens. Phase-A resolution: flat throughout, per
    the same Box-as-rustc-size-budget reasoning as
    D-8-1.2.

- **D-8-2** [LOCKED 2026-05-05] -- No `Statement` interning.
  Statements live inline in stage-10 `BasicBlock.statements:
  Vec<Statement>` (or `Box<[Statement]>` per stage-10's
  storage decision). Compositional with D-5-2 / D-6-2 /
  D-7-2. F4 / F4-2 cross-build identity OUT of scope
  (body-local content). Peer evidence: rustc has no
  statement-interner; rustc_public uses `Vec<Statement>`
  with `Tables` interning only DefIds / AllocIds / Spans /
  Types / Instances / TyConsts / MirConsts / Layouts (not
  Statements) [`rustc_public_bridge/src/lib.rs:45-54`];
  mrustc uses `std::vector<Statement>`; Charon uses
  `Vec<Statement>` with positional `(BlockId, usize)`
  identity (LLBC adds `StatementId` only after CFG
  reconstruction).

- **D-8-3** [LOCKED 2026-05-05] -- `Statement` wrapper
  struct shape. **`Statement { kind: StatementKind, span:
  Span }`** -- single inline `Span` field (12 bytes per
  stage-1 §5.3, no `SpanId` interning since stage 1
  rejected span-deduplication); drop `source_info.scope`
  (debuginfo-only, no C-shape meaning); drop
  `debuginfos: StmtDebugInfos` entirely (per-statement
  DWARF instrumentation hints like `AssignRef(Local,
  Place)` / `InvalidAssign(Local)`; carries no name
  material; rejected on stage-1 C-shape preservation
  grounds). Field order `kind, span` matches rustc_public
  (the closest existing analog). **Reject Charon's
  `comments_before: Vec<String>`** (Aeneas
  source-fidelity for formal-verification artefacts; out
  of scope for our C-emission pipeline). **Outbound
  stage-11 expectation: named-local preservation.** Stage
  11's `Body` / `LocalDecls` design MUST preserve
  user-source variable names (rustc's body-level
  `var_debug_info: Vec<VarDebugInfo>` per
  `[rustc_middle/src/mir/mod.rs:264, 1235-1257]`) for
  (a) pretty-printer readability per Q5;
  (b) generated-C readability so reviewers see
  `temp = ...;` not `_3 = ...;`; (c) downstream debugger /
  linter usability on the emitted C. Stage 1 spec rejected
  DWARF-emission instrumentation (the `StmtDebugInfos`
  per-statement hints) but did NOT preempt name-mapping
  at the Body level, which is distinct semantic content.
  Recommended provision: `LocalDecl.debug_name:
  Option<StringId>` populated from rustc's
  `var_debug_info`, scoped within the body, used by
  `render_local` / `render_place` and by the C-backend
  identifier emission policy. To be filed in
  `stage-11/expectations.md` "From stage 8" at stage-8
  close (merger contract step) alongside D-8-K's
  `render_statement` deferral.
  Detailed breakdown follows below for completeness:
  Rustc carries `{ source_info: SourceInfo, kind,
  debuginfos: StmtDebugInfos }`; rustc_public flattens
  `source_info` to a single `span: Span` and drops
  `debuginfos`; Charon adds `comments_before:
  Vec<String>` for Aeneas source-fidelity; mrustc carries
  no source-info / span material on Statement at all.
  Three sub-questions: (a) the span surface -- single
  `span: SpanId` (D-1 span vocabulary) vs full
  `SourceInfo { span, scope }`. Closing the stage-6
  forward ref ("operand-level span comes from containing
  Statement.source_info") needs only `span`; `scope` is
  debuginfo-only. Prefer single `span`. (b) `debuginfos`
  -- drop entirely (RegseqModel doesn't emit Rust-level
  debuginfo; stage-1 spec rejected debuginfo storage on
  C-shape preservation grounds). (c) `Spanned<Operand>`
  per-arg span deferral from stage-6's outbound
  expectation to stage-9 (Call args) -- that is stage-9's
  concern, not stage-8's; stage 8 only fixes the
  Statement-level span surface.

- **D-8-4** [LOCKED 2026-05-05] -- Statement-site
  invariants + rejection rules. Five-part registry:
  Part A 12-row rejection table covering 10 distinct
  dropped rustc variants plus split rows for fake-borrow
  `Assign` (Group 1) and `Coverage(BlockMarker |
  SpanMarker)` vs `Coverage(non-marker)` (Group 1 vs
  Group 3), grouped by discharge mechanism (Group 1
  CleanupPostBorrowck-NOPed -- FakeRead, AscribeUserType,
  BackwardIncompatibleDropHint, Coverage(BlockMarker |
  SpanMarker), Fake-borrow Assigns; Group 2 always-running
  cleanup-pass strip -- PlaceMention, Nop (also swept by
  SimplifyLocals-final), StorageLive, StorageDead; Group 3
  build-flag-gated -- Retag, ConstEvalCounter,
  Coverage(non-marker)); Part B 8 positive invariants
  (B-0a Place wellformedness inherited from D-5-2 explicit;
  B-0b Rvalue wellformedness inherited from D-7-3
  explicit; B-0c Operand wellformedness inherited from
  §20.5 explicit, applied to operand positions in
  `Intrinsic` payloads not enclosed by an Rvalue; B-1
  Assign type-equality; B-2 SetDiscriminant
  variant_index in-range via `Ty::Enum.layout`'s
  `EnumShape` cases; B-3 SetDiscriminant target-type
  `Ty::Enum { item, args, layout }` per §10; B-4
  Intrinsic::Assume operand_ty=Ty::Bool; B-5
  Intrinsic::CopyNonOverlapping deref-able-pointer +
  pointee-equality + count-as-usize per rustc validator
  `validate.rs:1517-1549`); Part C walk-implementation
  deferred to stage 11 (compositional with B-0a/B-0b/B-0c
  inheritance -- stage-11 visitor handles Place / Rvalue
  / Operand / Statement invariants in a single pass; same
  pattern as D-5-2 / D-7-3 walk-impl deferral); Part D
  F4/F4-2 cross-build identity OUT of scope (body-local
  content per D-8-2); Part E peer comparison
  informational (rustc 3 invariant clusters;
  rustc_public none; Charon none; mrustc fatal MIR_BUG
  on violation).
  Detailed breakdown follows below for completeness:
  Two parts:
  - **Part A (rejection table)**: rows for cleanup-
    discharged + out-of-pipeline variants, citing the
    discharging pass per row:
    Group 1 (CleanupPostBorrowck-NOPed; resulting Nops
    swept by Group 2 passes):
    1. `FakeRead` -- `CleanupPostBorrowck` NOPs;
       `RemovePlaceMention` sweeps Nops (always-running
       required pass).
    2. `AscribeUserType` -- same as FakeRead.
    3. `BackwardIncompatibleDropHint` -- same as FakeRead.
    4. `Coverage(BlockMarker | SpanMarker)` --
       `CleanupPostBorrowck` NOPs.
    5. `Assign(_, Rvalue::Ref(_, BorrowKind::Fake, _))`
       -- `CleanupPostBorrowck` NOPs the whole Assign.
    Group 2 (always-running cleanup-pass strip):
    6. `PlaceMention` -- `RemovePlaceMention` strips
       (required, default-on).
    7. `Nop` -- `RemovePlaceMention` and
       `RemoveStorageMarkers` and `SimplifyLocals-final`
       strip; `RemoveZsts` also produces transient Nops
       (via `make_nop`) that `SimplifyLocals-final`
       sweeps.
    8. `StorageLive` -- `RemoveStorageMarkers` strips
       (force-on via the `mir_enable_passes` whitelist;
       `is_enabled` gate bypassed by the pass-manager
       override per D-8-1.1's resolution).
    9. `StorageDead` -- same as `StorageLive`.
    Group 3 (build-flag-gated; never injected for our
    compilation mode):
    10. `Retag` -- only with `-Z mir-emit-retag`; out of
        pipeline.
    11. `ConstEvalCounter` -- only injected by `CtfeLimit`
        for const-eval bodies; driver bodies don't receive.
    12. `Coverage(VirtualCounter | ConditionMarker | ...)`
        -- only emitted by `InstrumentCoverage` under
        `-C instrument-coverage`; driver builds don't
        enable coverage instrumentation.
    Diagnostic message format: cite the required /
    whitelisted pass that should have discharged the
    variant, or the absent build flag.
  - **Part B (positive invariants)**: Place wellformedness
    inheriting D-5-2; Rvalue wellformedness inheriting
    D-7-3; Assign type-equality (`place_ty == rvalue_ty`,
    D-8-1.2); SetDiscriminant target-enum-type +
    variant_index in-range (D-8-1.3); Intrinsic per-variant
    operand types (Assume: `Operand: Ty::Bool`;
    CopyNonOverlapping: src/dst same-type-pointer, count
    `Ty::Uint(Usize)`). Walk-enforcement deferred to
    stage 11 (parallel to D-5-2, D-7-3 walk-impl
    deferral).
  - **Part C (peer comparison)**: rustc validator pins
    type-equality, target-type, operand-type invariants
    [`validate.rs:1467-1582`]; rustc_public enforces no
    construction-site invariants on statements; Charon
    enforces no Statement-level rejection (all 7
    borrowck/lint/scaffolding variants are absorb-or-drop
    returning `None`); mrustc has two-layer validator
    (`MIR_Validate` shape + `MIR_Validate_Full`
    move-state) with fatal `MIR_BUG` on violation.
    RegseqModel sides with rustc + mrustc on validator
    discipline.

- **D-8-K** [LOCKED 2026-05-05] --
  `pretty.rs::render_statement` deferred to stage 11
  alongside `render_place` (D-5-K), `render_operand`
  (D-6-K), `render_rvalue` (D-7-K). Deferral driver:
  every variant's payload either embeds a Place
  (D-5-K-deferred), an Rvalue carrying Operands
  (D-7-K + D-6-K-deferred), or Operands directly
  (D-6-K-deferred); the embedded-rendering subtree
  forces deferral. **Three-item outbound bundle to file
  in `stage-11/expectations.md` "From stage 8" at merger
  close**: (i) render_statement deferral; (ii) Statement
  walk implementation (from D-8-4 Part C, applies Part A
  rejection table + Part B positive invariants in one
  body-level pass, compositional with D-5-2 Place walk
  + D-7-3 Rvalue walk inheritance); (iii) named-local
  preservation (from D-8-3, `LocalDecl.debug_name:
  Option<StringId>` populated from rustc's body-level
  `var_debug_info` per `[rustc_middle/src/mir/mod.rs:264,
  1235-1257]`). Top-level `spec.md` §18.2 per-method
  ownership row update for `render_statement` lands at
  merger close.
  Detailed breakdown follows below for completeness:
  `pretty.rs::render_statement`
  deferred to stage 11 alongside `render_place` (D-5-K),
  `render_operand` (D-6-K), `render_rvalue` (D-7-K).
  Outbound expectation to file in
  `stage-11/expectations.md` "From stage 8" at lock time.
  Deferral driver: every variant's payload either is a
  Place (D-5-K-deferred), embeds an Rvalue carrying
  Operands (D-7-K + D-6-K-deferred), or carries Operands
  directly (D-6-K-deferred). The embedded-rendering
  subtree forces deferral. Peer evidence: rustc_public
  has partial `pretty_statement` with FIX-ME comments for
  AscribeUserType / Coverage / Intrinsic; Charon ships
  full `FmtWithCtx`; mrustc ships per-variant
  `Statement::operator<<`. RegseqModel inherits the
  defer-to-stage-11 pattern from prior stages.

D-8-2, D-8-3, D-8-4, D-8-K are small standalone
decisions; D-8-1 carries the substantive variant-set +
payload-shape commitments.

## Locked decisions

**STAGE 8 CLUSTER LOCKED 2026-05-05 at R4 MINOR with
explicit `Lockability: yes` verdict** (per the stage-6
precedent for MINOR-with-Lockability:yes lock; the
single residual MINOR is non-substantive status.md
cleanup that does not affect spec correctness).

- **D-8-1 [LOCKED]** -- 3-variant closed `Statement`
  surface (`Assign(Place, Rvalue)`,
  `SetDiscriminant(Place, VariantIdx)`,
  `Intrinsic(NonDivergingIntrinsic)`). Four sub-
  decisions D-8-1.1 through D-8-1.4 cover variant
  cardinality and per-variant payload shapes.
- **D-8-1.1 [LOCKED]** -- variant cardinality (3-
  variant closed surface; 12-row reject-on-encounter
  table covering 10 distinct dropped rustc variants).
- **D-8-1.2 [LOCKED]** -- `Assign(Place, Rvalue)`
  flat tuple; type-equality invariant in D-8-4
  Part B-1.
- **D-8-1.3 [LOCKED]** -- `SetDiscriminant(Place,
  VariantIdx)` flat tuple; target-type rule in D-8-4
  Part B-3 requires `Ty::Enum { item, args, layout }`
  per §10; variant_index in-range rule in D-8-4
  Part B-2.
- **D-8-1.4 [LOCKED]** -- `Intrinsic(NonDivergingIntrinsic)`
  nested with sub-enum `Assume(Operand) |
  CopyNonOverlapping(CopyNonOverlapping)` and named-
  fields sub-struct `CopyNonOverlapping { src, dst,
  count }`; flat throughout (no Box); per-variant
  operand-type rules in D-8-4 Part B-4 (Assume
  bool) + B-5 (CopyNonOverlapping deref-able-
  pointer + pointee-equality + count-as-usize).
- **D-8-2 [LOCKED]** -- no Statement interning;
  inline storage in stage-10's basic-block
  storage; F4 / F4-2 cross-build identity OUT of
  scope.
- **D-8-3 [LOCKED]** -- wrapper struct `Statement {
  kind, span: Span }`; drop scope and debuginfos;
  named-local preservation outbound to stage 11.
- **D-8-4 [LOCKED]** -- 5-part registry: Part A
  12-row rejection table; Part B 8 positive
  invariants (B-0a Place / B-0b Rvalue / B-0c
  Operand inherited; B-1 through B-5 stage-8-
  introduced); Part C walk-impl deferred to stage
  11; Part D F4 / F4-2 OUT of scope; Part E peer
  comparison informational.
- **D-8-K [LOCKED]** -- `render_statement` deferred
  to stage 11; three-item outbound bundle to file
  in `../stage-11/expectations.md` "From stage 8"
  at merger close.

## Open decisions

(none -- cluster locked at R4 MINOR with
Lockability: yes 2026-05-05)

The `RemoveStorageMarkers` enable-gate question raised
during the kickoff evidence pass was resolved during
Phase-A discussion via the
`crates/regseq-driver/src/callbacks.rs:25-66` +
`pass_manager.rs:213-239` trace; the pass DOES run.

## Codex review cadence (status)

| Round | Verdict | Issues | BLOCK / MINOR / NIT |
|-|-|-|-|
| R1 | BLOCK | 6 | 2 / 4 / 0 |
| R2 | BLOCK | 4 | 1 / 3 / 0 |
| R3 | MINOR | 4 | 0 / 4 / 0 |
| R4 | MINOR | 1 | 0 / 1 / 0 (Lockability: yes) |

Per-stage Codex cost: 4 rounds. 15 distinct findings
closed across the trajectory (3 BLOCK + 12 MINOR + 0
NIT). Cluster LOCKED 2026-05-05 at R4 MINOR with
explicit `Lockability: yes` verdict per the stage-6
precedent.

Round-by-round audit trail with verdicts, findings, and
corrections lives in `review-log.md`.

Prompt template: see `memory.md` §4.

## Stage-8-specific deferred items

(none expected at this point; D-8-K is a routine deferral
pattern from D-5-K, D-6-K, D-7-K -- `render_statement` to
stage 11 alongside the rest of the body-rendering subtree)

## Next concrete unit of work

**Stage 8 spec-side work is complete (LOCKED at R4
2026-05-05 with `Lockability: yes`).** Hand off to
the merger contract per README §5.5:

1. Integrate `stage-8/discussions.md` into top-level
   `../discussions.md` under a `## Stage 8` header
   (mechanical concat).
2. Integrate `stage-8/review-log.md` into top-level
   `../review-log.md` under a `## Stage 8` header
   (mechanical concat).
3. Append decision entries to top-level
   `../decisions.md` under `## Stage 8`: D-8-1
   (parent), D-8-1.1 through D-8-1.4 (LOCKED),
   D-8-2 (LOCKED), D-8-3 (LOCKED), D-8-4 (LOCKED),
   D-8-K (LOCKED).
4. (No earlier-stage decisions reopened by stage 8.)
5. Integrate `stage-8/spec-snapshot.md` §22
   contribution + §18.2 row update for
   `render_statement` into top-level `../spec.md`
   per README §5.9 (topic-based, audit-shape-
   stripped, no-D-tag-inline-references rules).
6. Run a Codex post-merge review pass on the
   integrated top-level `../spec.md`. Iterate to
   APPROVE.
7. Update top-level `../status.md` to mark stage 8
   closed and add a "Snapshot at close of stage 8 --
   current spec lives in top-level `spec.md`" header
   to `stage-8/spec-snapshot.md`.

After the merger contract closes, file the
three-item outbound bundle in
`../stage-11/expectations.md` "From stage 8" (per
D-8-K):
(i) `render_statement` deferral;
(ii) Statement walk implementation (D-8-4 Part C);
(iii) named-local preservation (D-8-3,
`LocalDecl.debug_name` populated from rustc's
body-level `var_debug_info`).
