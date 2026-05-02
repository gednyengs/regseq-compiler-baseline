# Status

Last updated: 2026-05-01 (stage 3 closed and merged: top-level
`spec.md` and `decisions.md` created; D14 deferred to new
stage 13 [type-system pretty-printing]; old stage 13
[domain-specific extensions] renumbered to stage 14;
post-iteration cross-cluster Codex sweep R1-R10 closed at
APPROVE on 2026-04-30; post-merge Codex review R1-R4 on
top-level `spec.md` closed at APPROVE on 2026-05-01).

Top-level project tracker. Captures cross-stage signals only:
high-level stage progress, active focus, blockers, and deferred
items affecting multiple stages. Stage-specific progress lives in
`stage-N/status.md`.

For project entry point and invariants, read `README.md` first.

## Stage progress (phase 1)

Each entry: status box, stage name, brief signal-level summary,
pointer to detail.

- [x] **Stage 0: Architectural foundation.** Q1-Q7 closed: the
  cross-stage architectural decisions that govern the rest of
  the work (crate split, `'tcx` boundary, IR layering, integration
  point, tests/serialization, multi-crate strategy, intrinsic
  recognition). Multi-crate three-tier dispatch infrastructure
  rebuilt and verified end-to-end.
  - Stage status file: `architecture.md` (Q1-Q7 contain the full
    rationale; no separate stage-0-status.md).

- [x] **Stage 1: RegseqModel foundation crate.** ID conventions
  (with `CrateId`), StringInterner, SourceFile/Span (file-local),
  DefPath form (full 19-variant `DefPathData`),
  Instance/InstanceKind/monomorphization story, `impl` lifting
  principle, C-semantics framing, top-level `RegseqModel` with
  invariants, F4/F4-2 determinism rules, custom serde, pretty-
  printer scaffold. Spec is round-5-revised after seven review
  passes; implementation shipped across 7 steps; 229 integration
  tests passing.
  - Stage status file: `stage-1/status.md`.

- [partial] **Stage 2: Serialization format.** Format chosen per
  Q5: textual MIR-inspired pretty-printer (primary) + serde-derived
  JSON (secondary). Schema-level work tracks each subsequent stage
  as variants gain Serialize derives.
  - Stage status file: `architecture.md` Q5 (no separate stage-2
    status; this stage is convention-only with no implementation
    surface of its own).

- [x] **Stage 3: Type system.** Specifies the `Ty` enum the
  bridge fills from rustc's `TyKind` and codegen consumes for C
  type declarations. 15 decisions locked (D1, D2, D3, D4, D5,
  D6, D7, D8, D9, D10, D11, D12, D13, D15, D16) plus the S0
  framing principle. D14 (renderer style) deferred to the new
  stage 13 (type-system pretty-printing) -- the renderer's
  input depends on `Ty` content references that only become
  fully populated at stage 11 (`FnSig`) and stage 12 (`Item` /
  DefPath). Closed via SESSION-A/B/C parallel cluster +
  post-iteration cross-cluster Codex sweep (R1-R10, 2026-04-30,
  52 findings closed across the iteration).
  - Stage status file: `stage-3/status.md`.

- [ ] **Stage 4: Constants and literals.** Scalars, byte arrays,
  aggregate constants, function pointers as constants.
  - Stage status file: (lazily created when stage opens).

- [ ] **Stage 5: MIR places and projections.**
  - Stage status file: (lazily created).

- [ ] **Stage 6: MIR operands.**
  - Stage status file: (lazily created).

- [ ] **Stage 7: MIR Rvalues.**
  - Stage status file: (lazily created).

- [ ] **Stage 8: MIR statements.**
  - Stage status file: (lazily created).

- [ ] **Stage 9: MIR terminators.**
  - Stage status file: (lazily created).

- [ ] **Stage 10: MIR basic blocks.**
  - Stage status file: (lazily created; inbound forward refs in
    `stage-10/expectations.md`).

- [ ] **Stage 11: MIR bodies.**
  - Stage status file: (lazily created; inbound forward refs in
    `stage-11/expectations.md`).

- [ ] **Stage 12: Rust Items.** Includes lang-item / instance-kind
  classification per convergent pattern 5 in
  `stage-0/prior-art/README.md`.
  - Stage status file: (lazily created; inbound forward refs in
    `stage-12/expectations.md`).

- [ ] **Stage 13: Type-system pretty-printing.** Fills
  `pretty.rs::render_ty` and pins the renderer style for the
  closed 19-variant `Ty` enum. Inherits the deferred D14
  decision from stage 3 (Rust-source-style lean preserved as
  input). Depends on stage 11 (`FnSig` content) and stage 12
  (`Item` / DefPath rendering).
  - Stage status file: (lazily created; inbound forward refs in
    `stage-13/expectations.md`).

- [ ] **Stage 14: Domain-specific extensions.** Intrinsics, ordered
  regions, register access, ISR classification, error-cleanup
  analysis, callback context inference. Per Q3, lives inside the
  same single-level IR as additional variants or tags on existing
  variants.
  - Stage status file: (lazily created; inbound forward refs in
    `stage-14/expectations.md`).

## Phase 2 (pl011 device pressure)

Status: not yet started. Tooling in place (dump script, fixture,
cleanup-whitelist).

First-body candidates from the pl011 fixture: `field_val`
(~36 lines), `raw_reg_val` (~22 lines), `mask_all_interrupts`
(~22 lines, register access). Phase 2 effectively waits on phase 1
to stabilize enough of RegseqModel that authoring golden C against
the device fixture is tractable. Interleaving is expected:
phase-2 attempts may surface phase-1 spec gaps that get fixed in
this folder, then phase 2 resumes.

See `../manual-lowering/README.md` for phase-2 specifics.

## Active focus

**No stage in progress.** Stage 3 closed and merged 2026-05-01:
top-level `spec.md` and `decisions.md` reflect the integrated
stage-1 + stage-3 contributions, with the post-merge Codex
review iterating to APPROVE in 4 rounds.
The canonical RegseqModel spec now lives at top-level
`spec.md`; per-stage `stage-1/spec-snapshot.md` and
`stage-3/spec-snapshot.md` are frozen historical snapshots
(headers note "current spec lives in top-level `spec.md`").
Top-level `discussions.md` and `review-log.md` were not
created — only stage 3 has produced these artefacts under the
new convention, so per the append-if-exists rule (README
§5.5 step 1-2) the per-stage versions remain the project record
until a second stage closes.

Candidates for the next stage to open: stage 4 (constants),
stage 11 (MIR bodies + `FnSig`), stage 12 (Rust Items +
DefPath), stage 13 (type-system pretty-printing), stage 14
(domain extensions). Stage 13 and stage 14 wait on stage 11 +
stage 12 for input-readiness.

## Open architectural decisions

None at present. Q1-Q7 are closed in `architecture.md`. Future
decisions land there as Q8, Q9, ... with references to evidence
and rationale.

## Blockers / waiting on

(none)

## Deferred items (cross-stage)

Lightweight follow-ups that span multiple stages or have no clear
later-stage owner. Stage-specific deferrals live in the relevant
`stage-N/expectations.md` (forward refs) or
`stage-N/status.md` (per-stage open items).

- **`StringInterner` deserialize-path optimization.** Step 4 of
  stage-1 added the `serde` `rc` feature to derive serde for
  `Option<Arc<str>>` on `SourceFile.content`. The `StringInterner`
  deserialize path still goes through `Vec<String>` (one extra
  allocation per entry); could be refactored to `Vec<Arc<str>>`
  directly using the `rc` feature. Codex flagged as
  optional/non-blocking.

- **Construction invariants 3, 6, 9 (and `spec_crate_stable_id`
  half of invariant 4).** Deferred in `RegseqModel::validate`
  until stage 12 (`Item.def_path`), stage 9/12 (sites holding
  `InstanceId`), and the bridge's `BridgeInput`. Each deferred
  invariant has an in-source comment naming the stage that will
  close the gap.

- **`newtype_index!` macro const-body arm.** Currently the macro
  accepts only empty-body invocations. Filed in
  `stage-10/expectations.md` (`BlockId::START_BLOCK`) and
  `stage-11/expectations.md` (`LocalId::RETURN_PLACE`); whichever
  stage lands first introduces the const-body arm.

## Next action

Stage 3 closed and merged. **Open the next stage.** Candidates:
stage 4 (constants), stage 11 (MIR bodies + `FnSig`), stage 12
(Rust Items + DefPath), stage 13 (type-system pretty-printing),
stage 14 (domain extensions). User picks based on driver of
value:

- Phase-2 pl011 work needs stage 11/12 first.
- Stage 13 (pretty-printing) and stage 14 (domain extensions)
  need stage 11/12 input-readiness.
- Stage 4 (constants) feeds `Ty::Array` length canonicalisation
  and is independent of stages 11/12.

## How to update this file

- Mark stages `[x]` (closed), `[ ]` (not started), `[partial]`
  (started with brief note), or `[in progress]` (active focus).
- Update `Last updated` at the top when changes land.
- Stage-specific detail goes in `stage-N/status.md`,
  not here.
- Deferred items here are cross-stage only; stage-specific
  deferrals live in the relevant stage's status file.
- Stage status files are created lazily: a stage gets one when
  it becomes active, when it closes (preserving history), or
  when it accumulates detail worth tracking. Stages 0 and 2 do
  not have separate status files because their detail lives
  fully in `architecture.md`.
