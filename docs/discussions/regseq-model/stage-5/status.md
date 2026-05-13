# Stage 5 Status

Status: **STAGE 5 SPEC-SIDE CLOSED 2026-05-04.** All three
stage-5 clusters locked: D-5-1 (R10 MINOR; residuals
deferred to top-level merger), D-5-2 (R6 APPROVE), D-5-K
(R6 APPROVE; retargeted from stage 12 to stage 11 on R1
BLOCK finding). Total review cost: 16 Codex rounds (10 for
D-5-1; 6 for D-5-2 + D-5-K combined); 35 distinct findings
closed (7 BLOCK + 12 MINOR + 16 NIT). Next workstream:
merger contract per README §5.5.

Last updated: 2026-05-04 (stage 5 spec-side closure
complete; ready for merger contract).

For top-level project status, see `../status.md`. For project-wide
invariants, see `../README.md`. For stage-5 invariants (workflow,
principles, read order, Codex prompt template), see `memory.md`.
For the working spec, see `spec-snapshot.md`. For the audit
trail, see `discussions.md`. For inbound forward refs (none at
opening), see `expectations.md`.

## Mission

Stage 5 specifies the `Place` and `PlaceElem` vocabulary that
the bridge fills from rustc's `mir::Place<'tcx>` /
`mir::PlaceElem<'tcx>` and that downstream consumers (stages
6-11, codegen) read as the lvalue surface of MIR. See
`memory.md` Section 0 for the full mission framing and the
stage's role as head of the MIR pipeline.

## Inbound expectations (from `expectations.md`)

**None filed at stage opening.** No earlier stage filed a
forward ref tagged "stage 5" / "stage-5" in its
`spec-snapshot.md` or `discussions.md`. The `stage-5/` folder
did not exist before this opening; lazy creation happened with
the stage's opening, not an earlier stage's prefiling.

## Cluster split

**Three clusters** matching the natural grouping of the 8
Phase A questions:

- **D-5-1: `Place` and `PlaceElem` core vocabulary**
  (sub-decisions D-5-1.1 through D-5-1.6 covering Phase A
  questions 1, 2, 3, 4, 6, 7). The substantive cluster --
  variant set + per-variant payload shapes + storage shape +
  structural invariants. Tightly coupled (variant set drives
  payload shapes; storage shape pairs with variant cardinality;
  structural invariants ride on the variant set's actual
  reachable members). Solo-cluster pattern (no parallel
  sessions), mirroring stage 4's D-4-1 / D-4-2 / D-4-3
  precedent.
- **D-5-2: Place validation phase placement** (Phase A
  question 5). Standalone deferral decision: stage 5 owns the
  five validation rules; the walk implementation lands at
  stage 11 (when `LocalDecls` is materialised). Outbound
  expectation filed in `stage-11/expectations.md` "From stage 5".
- **D-5-K: `pretty.rs::render_place` deferral** (Phase A
  question 8). Standalone deferral decision: render_place
  defers to stage 11 (when `Body` / `LocalDecls` materialise
  and `LocalId` `_N` rendering lands; field/variant names
  read from §11 layout schema's `FieldLayout.name` /
  `VariantLayout.name`, accessed at render time via the head
  local's `LocalDecl.ty`). **Originally proposed for stage
  12** under the misimpression that `Item` carries
  `fields[].name` / `variants[].name` tables; **retargeted
  to stage 11** on the D-5-2 + D-5-K combined Codex review
  R1 BLOCK finding `d5k-name-source-and-target`. Outbound
  expectation filed in `stage-11/expectations.md` "From
  stage 5" alongside D-5-2's place-validation walk.

D-5-2 and D-5-K are small deferral decisions; D-5-1 carries
the substantive variant-set + invariant + storage commitments.

## Locked decisions

D-5-1 cluster LOCKED 2026-05-04 at R10 (MINOR, 2 NITs;
residual cross-reference cleanup deferred to top-level
merger Codex review per README §5.5 step 6 per the
methodology-deviation rationale recorded in `review-log.md`).
D-5-2 and D-5-K remain `[DRAFT]` pending their combined
Codex review trajectory.

- **D-5-1 [LOCKED]** -- `Place` and `PlaceElem` core
  vocabulary (parent). 6-variant closed `PlaceElem` set
  (Deref, Field, Index, ConstantIndex, Subslice, Downcast)
  reached as the parent decision; sub-decisions break out
  the per-question detail. Audit shape: `discussions.md` §1
  D-5-1 entry. Spec content: `spec-snapshot.md` §19 (Place
  and projections section).

  - **D-5-1.1 [LOCKED]** -- Closed vocabulary count: 6
    variants (reject `OpaqueCast` and `UnwrapUnsafeBinder`).
    Audit shape: `discussions.md` §1 D-5-1.1.
  - **D-5-1.2 [LOCKED]** -- `Field` payload: keep inline
    `TyId` (`Field(FieldIdx, TyId)`). Audit shape:
    `discussions.md` §1 D-5-1.2.
  - **D-5-1.3 [LOCKED]** -- `Downcast` payload: drop rustc's
    `Option<Symbol>` slot (`Downcast(VariantIdx)`). Audit
    shape: `discussions.md` §1 D-5-1.3.
  - **D-5-1.4 [LOCKED]** -- Projection storage shape:
    `Box<[PlaceElem]>`. Audit shape: `discussions.md` §1
    D-5-1.4.
  - **D-5-1.5 [LOCKED]** -- I-Deref-first invariant:
    codified strictly (`Deref` appears at most once and only
    at projection[0]). Audit shape: `discussions.md` §1
    D-5-1.5.
  - **D-5-1.6 [LOCKED]** -- I-Downcast-followed-by-Field
    invariant: codified strictly (`Downcast` is non-last AND
    immediately followed by `Field`). Audit shape:
    `discussions.md` §1 D-5-1.6.

- **D-5-2 [LOCKED]** -- Place validation phase placement:
  stage 5 owns the five validation rules; walk
  implementation lands at stage 11. Audit shape:
  `discussions.md` §1 D-5-2. Spec content:
  `spec-snapshot.md` §19.7 (rule statements) + §3.3 Phase B
  step 8 NOTE (deferral forward-declaration). Outbound:
  `stage-11/expectations.md` "From stage 5". Locked at R6
  APPROVE (alongside D-5-K).

- **D-5-K [LOCKED]** -- `pretty.rs::render_place` deferred to
  stage 11 (retargeted from stage 12 on R1 BLOCK finding;
  see "Cluster split" above). Audit shape: `discussions.md`
  §1 D-5-K. Spec content: `spec-snapshot.md` §19.8 + §18.2
  per-method ownership update. Outbound:
  `stage-11/expectations.md` "From stage 5" (alongside
  D-5-2). Locked at R6 APPROVE.

## Open decisions

(none -- Phase A closed)

## Codex review cadence (status)

Two review trajectories:

- **D-5-1 cluster** (substantive; sub-decisions 1.1-1.6).
  **CLOSED 2026-05-04 at R10 = MINOR** (2 NITs; residual
  cross-reference cleanup deferred to top-level merger
  Codex review per README §5.5 step 6). Trajectory:
  - R1 (2026-05-03): BLOCK -- 4 issues (2 BLOCK / 2 MINOR
    / 0 NIT). Vocab drift (Subtype/Coroutine/RawPtr);
    subslice arithmetic.
  - R2 (2026-05-03): BLOCK -- 5 issues (2 BLOCK / 3 MINOR
    / 0 NIT). Layout-vocabulary depth from R1's vocab fix;
    EnumShape access; renderer/storage hygiene.
  - R3 (2026-05-04): MINOR -- 3 issues (0 BLOCK / 2 MINOR
    / 1 NIT). Closure dispatch via parallel Struct;
    field-type summary; `layout.kind` access.
  - R4 (2026-05-04): MINOR -- 1 issue (0 BLOCK / 1 MINOR
    / 0 NIT). "Compositional behavior beats special-case
    flags" promoted to README §2.
  - R5 (2026-05-04): MINOR -- 1 issue (0 BLOCK / 1 MINOR
    / 0 NIT). PostCleanup attribution split per invariant.
  - R6 (2026-05-04): MINOR -- 2 issues (0 BLOCK / 1 MINOR
    / 1 NIT). Subslice over-admission tightened; Deref-first
    quote replaced with verbatim §6.1 evidence.
  - R7 (2026-05-04): MINOR -- 4 issues (0 BLOCK / 1 MINOR
    / 3 NIT). ConstantIndex parallel tightening; §15.1
    lead-in scope; H1 summary refresh; "Closed vocabulary"
    methodology rename to "No defensive fallbacks".
  - R8 (2026-05-04): MINOR -- 2 issues (0 BLOCK / 0 MINOR
    / 2 NIT). Methodology-rule alias residuals; prior-art
    §10.4 freshen.
  - R9 (2026-05-04): MINOR -- 1 issue (0 BLOCK / 0 MINOR
    / 1 NIT). Prior-art §3.7/§3.8/§6.3/§10.5 stale
    cross-check refs freshened.
  - R10 (2026-05-04): MINOR -- 2 issues (0 BLOCK / 0 MINOR
    / 2 NIT). D-5-1 Option A H2 framing; prior-art
    §6.2/§6.4 H1 split. **R10 CORRECTIONS APPLIED, NOT
    VERIFIED BY CODEX. CLUSTER LOCKED.**

  Total: 10 rounds; 25 distinct findings closed (4 BLOCK +
  10 MINOR + 11 NIT). Methodology-deviation rationale in
  `review-log.md` cluster-closure entry.

- **D-5-2 + D-5-K combined** (deferral pair). **CLOSED
  2026-05-04 at R6 = APPROVE.** Trajectory:
  - R1 (2026-05-04): BLOCK -- 4 issues (1 BLOCK / 1 MINOR
    / 2 NIT). Substantive: schema-driven retarget of
    D-5-K from stage 12 to stage 11 (Item carries no
    field/variant name tables; names live in §11
    `FieldLayout.name` / `VariantLayout.name`).
  - R2 (2026-05-04): BLOCK -- 2 issues (1 BLOCK / 1
    MINOR / 0 NIT). Retarget consistency residuals.
  - R3 (2026-05-04): BLOCK -- 2 issues (1 BLOCK / 0
    MINOR / 1 NIT). More retarget consistency residuals.
  - R4 (2026-05-04): MINOR -- 1 NIT. Stage-3 D-tag
    cross-reference fix.
  - R5 (2026-05-04): MINOR -- 1 NIT. Methodology bullet
    placement.
  - R6 (2026-05-04): APPROVE -- 0 issues.

  Total: 6 rounds; 10 distinct findings closed (3 BLOCK +
  2 MINOR + 5 NIT). Standard "Repeat until APPROVE" path
  per `memory.md` §1; no methodology deviation invoked.

Round-by-round audit trail with verdicts, issues, and
corrections lives in `review-log.md`.

Prompt template: see `memory.md` Section 4.

## Stage-5-specific deferred items

(none)

## Next concrete unit of work

**Stage 5 spec-side work is complete.** All three clusters
locked: D-5-1 (R10 MINOR; residuals deferred), D-5-2 (R6
APPROVE), D-5-K (R6 APPROVE). The next workstream is the
**merger contract per README §5.5**: top-level integration
of `spec.md`, `discussions.md`, `review-log.md`,
`decisions.md` plus a post-merge Codex review pass that will
sweep any residual cross-references deferred from D-5-1's
R10 closure.

The 7-step merger protocol (per README §5.5):
1. Integrate `stage-5/discussions.md` into top-level
   `discussions.md` under a `## Stage 5` header (mechanical
   concat).
2. Integrate `stage-5/review-log.md` into top-level
   `review-log.md` under a `## Stage 5` header (mechanical
   concat).
3. Append decision entries to `decisions.md` under
   `## Stage 5`: D-5-1, D-5-1.1 through D-5-1.6 (LOCKED),
   D-5-2 (LOCKED), D-5-K (LOCKED).
4. (No earlier-stage decisions reopened by stage 5.)
5. Integrate `stage-5/spec-snapshot.md` into top-level
   `spec.md` per README §5.9 (topic-based; audit-shape
   stripped; new §19 Place and projections section + §3.3
   Phase B step 8 NOTE addition + §15.1 rejection rows +
   §18.2 per-method ownership row update).
6. Run a Codex post-merge review pass on the integrated
   top-level `spec.md`. Iterate to APPROVE. **This pass
   sweeps residual cross-references deferred from D-5-1's
   R10 closure** (per the methodology-deviation rationale
   in `review-log.md`).
7. Update top-level `status.md` to mark stage 5 closed and
   add a "Snapshot at close of stage 5 -- current spec
   lives in top-level `spec.md`" header to
   `stage-5/spec-snapshot.md`.

After the merger contract completes, the next stage in the
5-11 dependency chain (stage 6: MIR operands) can open.

Concrete prompt-shape for the agent running the post-merge
Codex review (step 6):

> Adversarial review of stage-5-integrated top-level spec,
> ROUND 1.
>
> Spec under review:
> `docs/discussions/regseq-model/spec.md` (focus: newly
> integrated §19 Place and projections section + §3.3
> Phase B step 8 NOTE addition + §15.1 rejection rows for
> `OpaqueCast` / `UnwrapUnsafeBinder` + §18.2 per-method
> ownership row update for `render_place`; cross-reference
> consistency across all sections).
>
> Audit-shape ground truth (now top-level merged):
> `docs/discussions/regseq-model/discussions.md` §"Stage 5"
> + `docs/discussions/regseq-model/decisions.md` Stage 5
> entries.
>
> Out of scope: D-5-1 / D-5-2 / D-5-K decisions are LOCKED;
> only flag if the integration introduces an inconsistency.
>
> Special focus: any residual cross-references deferred
> from D-5-1's R10 closure (per
> `stage-5/review-log.md` "Cluster closure (2026-05-04):
> D-5-1 LOCKED at R10").

Concrete prompt-shape for the agent running R1:

> Adversarial review of stage-5 deferral decisions D-5-2
> (Place validation phase placement) and D-5-K
> (`pretty.rs::render_place` deferral to stage 11; retargeted
> from stage 12 on R1 BLOCK finding), ROUND N.
>
> Spec under review:
> `docs/discussions/regseq-model/stage-5/spec-snapshot.md`
> (focus: §19.7 Place validation rules + walk deferral to
> stage 11; §19.8 render_place deferral to stage 11; §3.3
> Phase B step 8 NOTE forward-declaration; §18.2 per-method
> ownership row for `render_place`).
>
> Audit-shape ground truth:
> `docs/discussions/regseq-model/stage-5/discussions.md` §1
> D-5-2 + D-5-K.
>
> Out of scope: D-5-1 (core vocabulary cluster, locked at
> R10 -- do not flag closed sub-decisions; only flag if a
> deferral decision surfaces an inconsistency with a
> closed D-5-1.x finding).
>
> Evidence files:
> - `docs/discussions/regseq-model/stage-5/prior-art/rustc.md`.
> - `docs/discussions/regseq-model/rustc-references.md`.
> - `docs/discussions/regseq-model/README.md`.
> - `docs/discussions/regseq-model/stage-1/spec-snapshot.md`.
> - `docs/discussions/regseq-model/stage-3/spec-snapshot.md`
>   (D14 -> stage-13 precedent).
> - `docs/discussions/regseq-model/stage-4/discussions.md`
>   (D-4-K -> stage-14 precedent).
> - `docs/discussions/regseq-model/stage-11/expectations.md`
>   "From stage 5" (outbound for both D-5-2 walk and D-5-K
>   render_place implementation).
> - `docs/discussions/regseq-model/stage-12/expectations.md`
>   "From stage 5" (retarget note; no stage-12 deliverables
>   incoming from stage 5).
>
> Evaluate per the §4 prompt template in
> `stage-5/memory.md` (verdict / numbered issues / hard cap
> 25). Specifically check:
> 1. D-5-2 validation rules (1-5 in §19.7) are coherent and
>    complete given the closed D-5-1 vocabulary?
> 2. D-5-2 deferral target (stage 11) is correctly justified
>    by `LocalDecls` materialisation?
> 3. D-5-K deferral target (stage 11) is correctly justified
>    by per-variant rendering dependency analysis (`Field`
>    needs predecessor layout's `FieldLayout.name`;
>    `Downcast` needs `VariantLayout.name`; `Index` needs
>    `LocalId` `_N` rendering -- all stage-11 territory
>    given Body materialisation)?
> 4. Outbound expectations in stage-11 mirror the D-5-2 /
>    D-5-K decisions correctly? (The previously-filed
>    stage-12 outbound for D-5-K has been moved to stage 11
>    and replaced with a retarget note in
>    stage-12/expectations.md.)
> 5. D-5-K's claim that `render_ty` (stage 13) is *not* a
>    dependency is sound (because `Field`'s inline `TyId` is
>    consumed by validation/codegen, not rendering)?

After this trajectory closes at APPROVE (or MINOR/NIT-only
state if a similar diminishing-returns pattern emerges),
stage 5's spec-side work is complete. Then the merger
contract per README §5.5 applies (top-level integration into
`../spec.md`, `../discussions.md`, `../review-log.md`,
`../decisions.md` plus post-merge Codex review). The
post-merge top-level Codex review will sweep any residual
cross-references deferred from D-5-1's R10 closure.

Per-cluster top-up research (per `../README.md` §2): if a
Codex review round surfaces a sub-question that needs
cross-project evidence, pull the relevant project's
research just-in-time and save findings under
`stage-5/prior-art/<project>.md`. Do not research
speculatively.
