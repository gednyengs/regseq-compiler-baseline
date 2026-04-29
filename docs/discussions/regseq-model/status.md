# Status

Last updated: 2026-04-28 (stage-1 fully closed: step-7 added the canonical `render_def_path` helper and the `PrettyPrinter` scaffold, closing the two responsibilities the post-step-6 audit surfaced; 229 integration tests passing).

Consolidated status tracker for phase-1 RegseqModel work. Update at
the end of every session that changes anything material -- a stale
tracker is worse than no tracker.

## Stage progress (phase 1)

- [x] **Stage 0: Architectural foundation.** Q1-Q7 closed; see
  `architecture.md`. Multi-crate three-tier dispatch infrastructure
  also rebuilt and verified end-to-end.
- [x] **Stage 1 spec.** ID conventions (with `CrateId`),
  StringInterner, SourceFile / Span (file-local), DefPath form
  (full 19-variant `DefPathData`), Instance / InstanceKind /
  monomorphization story, `impl` lifting principle, C-semantics
  framing, Rust -> C lowering reference, bridge-synthesized
  Items rule, top-level RegseqModel struct (with `crates`
  table, `primary_crate`, `sigs`, etc.), F4 + F4-2 determinism
  rules with explicit rank tables, custom serde for IDs and
  StringInterner, pretty-printer interface sketch. Research
  synthesis (`stage-1/research.md`), seven review passes (five
  Codex, two external) all addressed (`stage-1/review-log.md`),
  rustc excerpts cover Instance/InstanceKind/SymbolName/VtblEntry
  (`stage-1/rustc-excerpts.md`), spec (`stage-1/vocabulary.md`)
  implementation-ready.
- [x] **Stage 1 implementation.** Seven steps total in
  `crates/regseq-model/`:
  - Step 1: Idx + NonMaxU32 + `newtype_index!` + CrateId
    (`b8f1e67`).
  - Step 2: `IndexVec<I, T>` with flat-array serde (`fd0d3c8`).
  - Step 3: `StringInterner` + StringId (`82aaaed`).
  - Step 4: `SourceFile` + `Position` + `Span` + SourceFileId
    with resolution methods (`f34df65`); plus a follow-up
    (`f8e7b4e`) adding `SourceFileId::SYNTHESIZED` after the
    round-5 spec revision named the alias.
  - Round-5 spec revision committed in `8215cdc` (after step 4)
    redesigned the monomorphization story (Instance shape,
    InstanceKind, lifting) and the C-semantics framing.
  - Step 5 (`d78db53`): forward-declared `ItemId` / `BodyId` /
    `TyId` / `ConstId` / `FnSigId` / `InstanceId`;
    `StableCrateId` + `CrateInfo`; `DefPathData` (19 variants)
    + `DefPathSegment` + `DefPath` + `DefPathHash`;
    `def_path_data_sort_rank` (0..=18); `GenericArg` +
    `GenericArgs`; `Instance` + `InstanceKind` (six variants);
    `regseq_instance_kind_rank` (0..=5);
    `RustcInstanceKindTag` (Regseq-side mirror of rustc's
    15-variant `ty::InstanceKind`) +
    `rustc_instance_kind_sort_rank` (0..=14). Codex round 1
    returned MINOR (two test additions) -- both addressed.
  - Step 6 (this commit): top-level `RegseqModel` struct +
    `RegseqModel::with_primary` constructor +
    `validate(&self) -> Result<(), InvariantViolation>`
    enforcing the stage-1-checkable subset of the 10 spec
    invariants (1, 2, 4, 5, 8, 10 enforced; 3, 6, 9 deferred to
    later stages; 7 is structural). Empty placeholder structs
    `Item` / `Ty` / `Const` / `FnSig` / `Body` ship with the
    crate as forward-decls for stages 3, 4, 11, 12. Codex round
    1 returned APPROVE (one doc nit + optional dangling-ID
    coverage); both addressed.
  - Step 7 (this commit): canonical `render_def_path(path,
    crates, strings) -> String` helper in `def_path.rs`
    (pinned per-variant rendering against `nightly-2026-02-01`
    `rustc_hir/src/definitions.rs:185-204, :248-249, :505-520`);
    `PrettyPrinter<'a>` scaffold in new `pretty.rs` with
    stage-1 functional methods `render_span` and
    `render_def_path_value`, stage-N stubs for `render`,
    `render_ty`, `render_const`, `render_def_path(ItemId)`,
    `render_instance`; `RegseqModel::pretty()` accessor.
    Plan reviewed by Codex over three rounds (BLOCK -> BLOCK
    -> APPROVE) before implementation. Implementation reviewed
    by Codex once (MINOR -- one panic-prefix consistency note
    kept as a deliberate improvement, one missing negative
    test added).
  - 229 integration tests passing across ten test files
    (`idx_round_trip`, `index_vec`, `source`, `string_interner`,
    `crate_info`, `def_path`, `instance`, `model`,
    `render_def_path`, `pretty`); `cargo check --workspace --tests`
    clean.
- [partial] **Stage 2: Serialization format.** Format chosen
  (textual primary + serde JSON secondary, see Q5). Schema-level
  work tracks the rest of the stages -- per-stage variants gain
  Serialize derives and pretty-printer output as they land.
- [ ] **Stage 3: Type system.** Standard types, references, `!`,
  ZSTs (interaction with `RemoveZsts` to be experimentally pinned).
- [ ] **Stage 4: Constants and literals.** Scalars, byte arrays,
  aggregate constants, function pointers as constants.
- [ ] **Stage 5: MIR places and projections.**
- [ ] **Stage 6: MIR operands.**
- [ ] **Stage 7: MIR Rvalues.**
- [ ] **Stage 8: MIR statements.**
- [ ] **Stage 9: MIR terminators.**
- [ ] **Stage 10: MIR basic blocks.**
- [ ] **Stage 11: MIR bodies.**
- [ ] **Stage 12: Rust Items.** Includes lang-item / instance-kind
  classification per convergent pattern 5 in `prior-art/README.md`.
- [ ] **Stage 13: Domain-specific extensions.** Intrinsics, ordered
  regions, register access, ISR classification, error-cleanup
  analysis, callback context inference. Per Q3, lives inside the
  same single-level IR as additional variants or tags on existing
  variants.

## Phase 2 (pl011 device pressure)

Status: not yet started. Tooling in place (dump script, fixture,
cleanup-whitelist). First-body hand-lowering not yet attempted.

First-body candidates: `field_val` (~36 lines), `raw_reg_val`
(~22 lines), `mask_all_interrupts` (~22 lines, register access).

Phase 2 effectively waits on phase 1 to stabilize enough of
RegseqModel that authoring golden C against the device fixture is
tractable. Interleaving is expected: phase-2 attempts may surface
phase-1 spec gaps that get fixed in this folder, then phase 2
resumes.

See `../manual-lowering/README.md` for phase-2 specifics.

## Active focus

Stage 1 implementation is **closed**. Steps 1-6 are in
`crates/regseq-model/`; the foundation surface is complete and
ready for stage 3 (Type system) to start expanding the empty `Ty`
placeholder. The bridge (in `crates/regseq-driver/`) and stage 13
(domain extensions) can also start consuming the stage-1 surface.

State of the world:

- *Stage 1 spec is round-5-revised.* `stage-1/vocabulary.md`
  reflects seven review passes (R1-R6 + R5.7-R5.21 across an
  external follow-up). See `stage-1/review-log.md` for the audit
  trail.
- *All seven steps closed* in `crates/regseq-model/`. Step 5
  Codex round returned MINOR (two tests added); step 6
  returned APPROVE; step 7's plan went through three Codex
  rounds (anonymous-namespace string corrections against
  pinned rustc) and the implementation review returned MINOR
  (one missing negative test added).
- *229 integration tests* across the ten test files
  (`idx_round_trip`, `index_vec`, `source`, `string_interner`,
  `crate_info`, `def_path`, `instance`, `model`,
  `render_def_path`, `pretty`).
- *Workspace builds clean.* `cargo check --workspace --tests`
  passes with only the inherited dead-code warnings from
  `regseq-driver-old/src/extract/`.
- *Multi-crate dispatch (Q6) intact.* `crates/regseq/` facade,
  three-tier dispatch, env-var plumbing, cleanup-pass whitelist
  unchanged. `RegseqCallbacks::after_analysis` still empty until
  bridge work begins.

Open methodology guidance for stage 3+: the spec is still
mutable. If type-shape ambiguities or composition issues surface
during stage-N implementation, revise `stage-1/vocabulary.md` (or
the relevant stage spec) and trigger a Codex review of the
revision before continuing -- do not paper over with implementer
judgment.

## Open architectural decisions

None at present. Q1-Q7 are closed in `architecture.md`. Future
decisions land there as Q8, Q9, ... with references to evidence
and rationale.

## Blockers / waiting on

(none)

## Next action

Stage 1 is closed. The next concrete unit of work is one of:

1. **Stage 3 (Type system).** Begin expanding the `Ty`
   placeholder in `crates/regseq-model/src/placeholders.rs` into
   the full type vocabulary (standard scalar types, references,
   `!`, ZSTs, struct / enum / union layouts). Pin the canonical
   `Ty` representation rule that the F4-2 `ArgsKey` depends on.
   Ship behind one or more new modules (e.g., `ty.rs`) once the
   shape stabilizes; remove `Ty` from `placeholders.rs` when it
   moves out.
2. **Bridge (`crates/regseq-driver/src/`).** Begin implementing
   `RegseqCallbacks::after_analysis`: drive the stage-1
   construction pipeline (rustc tcx -> RegseqModel via
   `with_primary` + push) for the `field_val` first-body candidate
   in the pl011 fixture. This is also where the
   `rustc_ty::InstanceKind -> RustcInstanceKindTag` conversion
   lives. Surfacing requirements here may force stage-3 work
   (since you can't construct meaningful `Instance`s without
   `Ty`).
3. **Stage 13 (Domain extensions).** Pin the intrinsic vocabulary
   (per Q7); identify the Regseq-side enum / variant-on-existing
   surface that the bridge will populate.

Foundation-first order (all done):

1. ~~`Idx` trait + `newtype_index!` + CrateId.~~ Commit `b8f1e67`.
2. ~~`IndexVec<I, T>`.~~ Commit `fd0d3c8`.
3. ~~`StringInterner`.~~ Commit `82aaaed`.
4. ~~`SourceFile` + `Position` + `Span`.~~ Commit `f34df65`
   (plus `f8e7b4e` for `SourceFileId::SYNTHESIZED`).
5. ~~`DefPathData` family + Instance / InstanceKind / GenericArg
   + StableCrateId + CrateInfo + forward-declared IDs + rank
   tables.~~ Commit `d78db53`.
6. ~~Top-level `RegseqModel` + constructor + invariants.~~ This
   commit.

Stage-N implementation may now be delegated to a fresh
implementer agent; the foundation has stabilized enough that the
load-bearing context lives in the spec and the `regseq-model`
public surface rather than the spec-design conversation history.

Reference material in `crates/*-old/` is one `cargo build -p
regseq-model-old` (etc.) away if patterns from the prior design
need consulting; per "re-derive from evidence; do not import
baggage", consult deliberately, do not lift code wholesale.

## How to update this file

- Mark stages `[x]` (done), `[ ]` (not started), `[partial]`
  (started with brief note), or `[in progress]` (active focus
  pointer).
- Update `Last updated` at the top of the file when changes land.
- Keep entries short. Detailed status of in-progress work belongs
  in the per-stage doc when one exists.
- If a fork or architectural decision opens during stage work,
  add it to "Open architectural decisions" with a one-line scope
  pointer.
- When this file changes, also update the brief "Status" pointer
  in `README.md` only if the high-level summary there has gone
  stale.
