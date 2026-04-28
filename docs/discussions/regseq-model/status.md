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
  order in "Next action" below. Steps 1-4 closed: Idx + NonMaxU32 +
  `newtype_index!` + CrateId (`b8f1e67`); `IndexVec<I, T>` with
  flat-array serde (`fd0d3c8`); `StringInterner` + StringId
  (`82aaaed`); `SourceFile` + `Position` + `Span` + SourceFileId
  with resolution methods (`position`, `line_text`, `Span::start`/
  `end`/`slice`). All four Codex-reviewed; step 4 had one MEDIUM
  correctness finding (position fallback when `line_starts[0] >
  byte`), addressed with a fix and a regression test before
  commit. Step 5 (DefPath family + CrateInfo + Instance +
  GenericArg + StableCrateId) up next.
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

- *Step 1 closed* in `b8f1e67`. `Idx` trait with `'static` bound
  (R4.2), private `NonMaxU32` niche wrapper, declarative
  `newtype_index!` macro, `CrateId` with hand-rolled serde (route
  1 of R4.1). Codex ACCEPT.
- *Step 2 closed* in `fd0d3c8`. `IndexVec<I, T>` with `Vec<T>`
  inner and `PhantomData<fn(&I)>` marker. Serde via
  `#[serde(transparent)]` + `#[serde(skip)]` produces a flat array
  of `T` on the wire. Codex ACCEPT.
- *Step 3 closed* in `82aaaed`. `StringInterner` with hand-rolled
  serde and deserialize-time validation; `StringId` added as the
  second concrete ID. Codex ACCEPT.
- *Step 4 closed.* `SourceFile`, `Position`, `Span`, and
  `SourceFileId` (third concrete ID). Resolution methods:
  `position` (binary search via `partition_point`), `line_text`
  (1-based, strips `\n`/`\r\n`), `Span::start`/`end`/`slice`
  (file-local). Serde derived for `SourceFile`/`Position`/`Span`;
  `serde` `rc` feature added so `Option<Arc<str>>` content can be
  derived. 60 integration tests passing total (12 idx + 11
  index_vec + 22 source + 15 string_interner). Codex REVISE
  (MEDIUM correctness finding on `position` fallback when
  `line_starts[0] > byte`); fix applied and regression test added
  before commit.
- *Stage 1 spec round-4-revised.* `stage-1-vocabulary.md` reflects
  four review rounds (R4.1 serde mechanism, R4.2 `Idx: 'static`).
  Implementation-ready.
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

Implement stage-1 step 5 in `crates/regseq-model/`: `DefPathData`
(19 variants per the spec), `DefPathSegment`, `DefPath` (with
`crate_id`), `DefPathHash`, `CrateInfo`, `Instance`, `GenericArg`,
and `StableCrateId`. See the "DefPath form" and adjacent sections
of `stage-1-vocabulary.md`. This is the largest single step --
forward-declared by stages 11-12 but populated at stage 1.
**Stage 1 is implemented directly in the active session, not
delegated to a separate implementer agent.**
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
   (with `crate_id`) + `DefPathHash` + `CrateInfo` + `Instance`
   (with `InstanceKind` per round-5 revision) + `GenericArg` +
   `StableCrateId` mirror. Pending round-5 spec acceptance
   (Codex pass + external review) before implementation begins.
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
