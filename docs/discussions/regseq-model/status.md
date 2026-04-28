# Status

Last updated: 2026-04-28 (round-5 spec revision committed: Instance/InstanceKind/lifting/C-semantics; reviewed via Codex round 5 + external review + Codex round 6, all findings resolved; spec implementation-ready for step 5).

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
  synthesis (`stage-1-research.md`), seven review passes (five
  Codex, two external) all addressed (`stage-1-review-log.md`),
  rustc excerpts cover Instance/InstanceKind/SymbolName/VtblEntry
  (`rustc-excerpts.md`), spec (`stage-1-vocabulary.md`)
  implementation-ready.
- [in progress] **Stage 1 implementation.** Population of
  `crates/regseq-model/` with the stage-1 types. Foundation-first
  order in "Next action" below. Steps 1-4 closed:
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
    InstanceKind, lifting) and the C-semantics framing; no
    step-1-4 code change was required beyond the SYNTHESIZED
    alias.
  - 62 integration tests passing; `cargo check --workspace
    --tests` clean.
  - Step 5 (DefPath family + CrateInfo + Instance + InstanceKind
    + GenericArg + StableCrateId + FnSigId forward-decl) up
    next.
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

Step 5 (DefPathData + DefPathSegment + DefPath + DefPathHash +
CrateInfo + Instance + InstanceKind + GenericArg + StableCrateId
+ FnSigId forward-decl) implementation. Round-5 spec revision
is committed; spec is implementation-ready.

State of the world:

- *Stage 1 spec is round-5-revised.* `stage-1-vocabulary.md`
  reflects seven review passes (R1-R6 + R5.7-R5.21 across an
  external follow-up). Spec is implementation-ready. See
  `stage-1-review-log.md` for the audit trail.
- *Steps 1-4 closed* in `crates/regseq-model/` (commits
  `b8f1e67`, `fd0d3c8`, `82aaaed`, `f34df65`, plus follow-up
  `f8e7b4e` for `SourceFileId::SYNTHESIZED`). 62 integration
  tests passing.
- *Workspace builds clean.* `cargo check --workspace --tests`
  passes with only the inherited dead-code warnings from
  `regseq-driver-old/src/extract/`.
- *Multi-crate dispatch (Q6) intact.* `crates/regseq/` facade,
  three-tier dispatch, env-var plumbing, cleanup-pass whitelist
  unchanged. `RegseqCallbacks::after_analysis` still empty until
  bridge work begins (post-foundation).

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

Implement stage-1 step 5 in `crates/regseq-model/` against the
round-5-revised spec. Types in scope:

- `DefPathData` (19-variant rustc-mirror enum) + `DefPathSegment`
  + `DefPath` (with `crate_id`) + `DefPathHash`.
- `StableCrateId` (transparent `pub struct StableCrateId(pub u64);`
  wrapper).
- `CrateInfo { name: StringId, stable_crate_id: StableCrateId }`.
- `Instance { item, generic_args, symbol_name, span, kind }` +
  `InstanceKind` (six-variant enum: Function/Static/Const/Struct/
  Enum/Union; see "Instances: monomorphized items" in the spec).
- `GenericArg` enum (Type/Const) + `GenericArgs = Vec<GenericArg>`.
- Forward-declarations: `FnSigId` ID type (FnSig itself lands in
  stage 11; step 5 just declares the ID).
- The two pinned rank tables (`regseq_instance_kind_rank` for the
  six Regseq kinds; `rustc_instance_kind_sort_rank` for rustc's
  15-variant `InstanceKind`).

Read `stage-1-vocabulary.md` sections "DefPath form", "Lifting
`impl` items", "Instances: monomorphized items", and
"Bridge-synthesized Items" before starting. **Stage 1 is
implemented directly in the active session, not delegated to a
separate implementer agent.**
Rationale: stage 1 is the foundation every later stage rides on,
and the conversation context built across the spec-design sessions
(including the methodology rules, decision history, and
spec-mutability posture) is hard to transfer cleanly to a fresh
agent. Once stage 1 is validated under compilation, stages 3-13
become more mechanical and can be delegated.

Foundation-first order (steps 1-4 done; remaining):

1. ~~`Idx` trait + `newtype_index!` analogue + custom serde for one
   ID.~~ *Done in commit `b8f1e67`.*
2. ~~`IndexVec<I, T>` with flat-array JSON shape.~~ *Done in
   commit `fd0d3c8`.*
3. ~~`StringInterner` with custom `Serialize`/`Deserialize`.~~
   *Done in commit `82aaaed`.*
4. ~~`SourceFile` + `Position` + `Span` with the F1 file-local
   byte-offset rule.~~ *Done in commit `f34df65`.*
5. `DefPathData` (19 variants) + `DefPathSegment` + `DefPath`
   (with `crate_id`) + `DefPathHash` + `StableCrateId` +
   `CrateInfo` + `Instance` + `InstanceKind` (six variants) +
   `GenericArg` + `FnSigId` forward-decl. Round-5 spec is
   committed in `8215cdc` and implementation-ready. Implementer
   should also bake the `regseq_instance_kind_rank` and
   `rustc_instance_kind_sort_rank` tables (per the "Instance"
   section).
6. Top-level `RegseqModel` struct including `crates`,
   `primary_crate`, `sigs`, and the full construction-invariants
   assertions (1-10).

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
