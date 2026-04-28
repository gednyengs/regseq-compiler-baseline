# Status

Last updated: 2026-04-28 (post Option-B refactor; stage-1 spec done across four review rounds; implementation queued).

Consolidated status tracker for phase-1 RegseqModel work. Update at
the end of every session that changes anything material -- a stale
tracker is worse than no tracker.

## Stage progress (phase 1)

- [x] **Stage 0: Architectural foundation.** Q1-Q7 closed; see
  `architecture.md`. Multi-crate three-tier dispatch infrastructure
  also rebuilt and verified end-to-end.
- [x] **Stage 1 spec.** ID conventions (with `CrateId`),
  StringInterner, SourceFile / Span (file-local), DefPath form
  (full 19-variant `DefPathData`), Instance, top-level RegseqModel
  struct (with `crates` table and `primary_crate`), F4 determinism
  rules, custom serde for IDs and StringInterner, pretty-printer
  interface sketch. Research synthesis (`stage-1-research.md`),
  four review rounds (three Codex, one external) all addressed
  (`stage-1-review-log.md`), spec (`stage-1-vocabulary.md`)
  implementation-ready.
- [ ] **Stage 1 implementation.** Population of
  `crates/regseq-model/` with the stage-1 types. Workspace is
  prepped (old stack moved to `*-old` siblings, un-suffixed crates
  wiped to stubs); not yet started. Foundation-first order in
  "Next action" below.
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

Between sessions; ready to start stage-1 implementation.

State of the world (post commit `9352ea0`):

- *Stage 1 spec complete.* `stage-1-vocabulary.md` reflects three
  review rounds (16 issues, all addressed). Implementation-ready.
- *Workspace prepped.* Old IR stack (regseq-model, regseq-analysis,
  regseq-backends, the `construction/`+`extract/` portions of
  regseq-driver, tests/e2e content) moved to `*-old` siblings for
  direct reference. Un-suffixed crates wiped to stubs. Tag
  `pre-stage-1-impl-archive` points at the parent of `9352ea0`.
- *Workspace builds clean.* `cargo check --workspace --tests`
  passes with only the inherited dead-code warnings from
  `regseq-driver-old/src/extract/`. Stage-0 regression in
  `regseq-driver/tests/args.rs` fixed in the refactor commit.
- *Multi-crate rebuild (Q6) complete.* `crates/regseq/` empty
  facade with `[package.metadata.regseq]` marker. Three-tier
  dispatch verified end-to-end. The aware-crates env-var plumbing
  and cleanup-pass whitelist are settled. New regseq-driver does
  not yet contain bridge code; `RegseqCallbacks::after_analysis`
  is empty until stage-1 implementation lands.

Open methodology guidance for the implementer: the spec is
mutable. If type-shape ambiguities or composition issues surface
during implementation, revise `stage-1-vocabulary.md` (and trigger
a Codex review of the revision) before continuing -- do not paper
over with implementer judgment.

## Open architectural decisions

None at present. Q1-Q7 are closed in `architecture.md`. Future
decisions land there as Q8, Q9, ... with references to evidence
and rationale.

## Blockers / waiting on

(none)

## Next action

Implement stage 1 in `crates/regseq-model/` against
`stage-1-vocabulary.md`. **Stage 1 is implemented directly in the
active session, not delegated to a separate implementer agent.**
Rationale: stage 1 is the foundation every later stage rides on,
and the conversation context built across the spec-design sessions
(including the methodology rules, decision history, and
spec-mutability posture) is hard to transfer cleanly to a fresh
agent. Once stage 1 is validated under compilation, stages 3-13
become more mechanical and can be delegated.

Foundation-first order:

1. `Idx` trait + `newtype_index!` analogue (with inherent
   `pub const fn from_u32`) + custom serde for one ID; validate
   that round-trip preserves the logical index, niche encoding
   stays internal, and `Option<Id>` is 4 bytes.
2. `IndexVec<I, T>` with flat-array JSON shape (likely
   `#[serde(transparent)]` over inner `Vec<T>` with `PhantomData<I>`
   skipped). Validate round-trip against a small integration test.
3. `StringInterner` with custom `Serialize`/`Deserialize` (storage
   only on the wire; lookup rebuilt on deserialize; index-0-is-empty
   and no-duplicate validation).
4. `SourceFile` + `Position` + `Span` with the F1 file-local
   byte-offset rule. (No bridge code yet; resolution methods only.)
5. `DefPathData` (19 variants) + `DefPathSegment` + `DefPath`
   (with `crate_id`) + `DefPathHash` + `CrateInfo` + `Instance` +
   `GenericArg` + `StableCrateId` mirror.
6. Top-level `RegseqModel` struct including `crates`,
   `primary_crate`, and the construction-invariants assertions.

Each step lands as its own commit. Spec issues surfaced during
implementation trigger spec revision (the spec is mutable per
methodology) before continuing.

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
