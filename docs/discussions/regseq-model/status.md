# Status

Last updated: 2026-05-03 (stage 4 closed and merged: all four
inbound expectations from stage 1 closed via D-4-1 / D-4-2 /
D-4-3 / D-4-K. D-4-1 LOCKED 2026-05-02 (Const enum, R1-R6,
20 findings); D-4-2 LOCKED 2026-05-03 (ConstIdentityKey
byte-form, R1-R7, 16 findings); D-4-3 LOCKED 2026-05-03
(reference-typed const handling via ConstMemoryStatic family,
R1-R4, 10 findings); D-4-K LOCKED 2026-05-03 (render_const
deferred to stage 14, R1-R4, 7 findings). Merger contract
completed per README §5.5: top-level `discussions.md` and
`review-log.md` created (stage 3 + stage 4 content under
per-stage headers); top-level `decisions.md` extended with
stage-4 entries; stage-4 spec contributions integrated into
top-level `spec.md`; post-merge Codex review pass R1-R4
closed at APPROVE on 2026-05-03 (12 findings closed). Stage 4
implementation-side work (filling `crates/regseq-model/src/`
with the locked design) not yet started -- separate workstream
parallel to stage 1's 7-step shipping pattern.).

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

- [partial] **Stage 4: Constants and literals.** Spec-side
  closed and merged across four locked decisions. D-4-1:
  closed 11-variant `Const` enum (Bool / Char / Int / Uint /
  Float / ZST / Bytes / Aggregate / EnumValue / NullPtr /
  Reloc) plus `Byte` three-state and `InstanceRef` 4-tuple
  cross-build identity. D-4-2: per-variant `ConstIdentityKey`
  byte-form schema + three-state-DFS acyclicity validation +
  value-level lookup injectivity ordering. D-4-3:
  reference-typed const handling (`&str` / `&[T]` literals)
  via the new `SyntheticItemKind::ConstMemoryStatic` family,
  with a content-derived `DefPathHash` and the existing
  variant set composing wide-pointer / thin-pointer references
  via `Aggregate([Reloc, Uint])` / `Reloc` (no new `Const`
  variant needed). D-4-K: deferred `render_const` to stage 14
  (latest of its dependencies). Spec contributions merged
  into top-level `spec.md` (post-merge Codex review pass
  R1-R4 closed at APPROVE on 2026-05-03; 12 findings closed).
  Implementation-side work (filling
  `crates/regseq-model/src/`) not yet started — separate
  workstream parallel to stage 1's 7-step shipping pattern.
  - Stage status file: `stage-4/status.md`.

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

**Stage 4 closed and merged 2026-05-03.** All four stage-1
inbound expectations closed under D-4-1 / D-4-2 / D-4-3 /
D-4-K (53 distinct Codex findings closed across 21 rounds
total over four clusters). Merger contract completed per
README §5.5: top-level `discussions.md` and `review-log.md`
created (containing stage 3 + stage 4 per-stage content under
respective headers); top-level `decisions.md` extended with
stage-4 entries; stage-4 spec contributions integrated into
top-level `spec.md` per the topic-based, audit-shape-stripped,
no-D-tag-inline-references rules in README §5.9; post-merge
Codex review pass R1-R4 closed at APPROVE on 2026-05-03 (12
findings closed across the iteration; mirrors stage 3's
4-round post-merge sweep).

The canonical RegseqModel spec lives at top-level `spec.md`;
per-stage `stage-1/spec-snapshot.md`, `stage-3/spec-snapshot.md`,
and `stage-4/spec-snapshot.md` are frozen historical
snapshots (each with a "Snapshot at close of stage N" header).

Stage 4 implementation-side work (filling
`crates/regseq-model/src/` with the locked design) is a
separate workstream — parallel to stage 1's 7-step shipping
pattern.

**No stage in progress.** Candidates for the next stage to
open: stage 11 (MIR bodies + `FnSig`), stage 12 (Rust Items +
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

Stage 4 closed and merged. **Two parallel workstreams
available:**

1. **Stage-4 implementation.** Fill `crates/regseq-model/src/`
   with the locked stage-4 design (`Const` enum, `Byte`,
   `InstanceRef`, `ContentHash`, `SyntheticItemKind::ConstMemoryStatic`,
   bridge dispatch arms, §3.3 Phase B validation rules).
   Parallel to stage-1's 7-step shipping pattern.

2. **Open the next stage.** Candidates: stage 11 (MIR bodies +
   `FnSig`), stage 12 (Rust Items + DefPath), stage 13
   (type-system pretty-printing), stage 14 (domain extensions).
   User picks based on driver of value:

   - Phase-2 pl011 work needs stage 11/12 first.
   - Stage 13 (pretty-printing) and stage 14 (domain extensions)
     need stage 11/12 input-readiness.
   - Stage 4 (constants) is closed; `Ty::Array` length
     canonicalisation is now grounded.

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
