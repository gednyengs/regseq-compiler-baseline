# Status

Last updated: 2026-04-28 (stage-1 step 2 closed -- IndexVec<I, T> with flat-array serde).

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
- [in progress] **Stage 1 implementation.** Population of
  `crates/regseq-model/` with the stage-1 types. Foundation-first
  order in "Next action" below. Step 1 (Idx + NonMaxU32 +
  `newtype_index!` + CrateId) closed in `b8f1e67`. Step 2
  (`IndexVec<I, T>` with `#[serde(transparent)]` over inner
  `Vec<T>` and skipped phantom marker) closed; both Codex-reviewed
  (ACCEPT each). Step 3 (`StringInterner`) up next.
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

Stage-1 step 3 (`StringInterner` with hand-rolled serde, storage
only on the wire, lookup rebuilt on deserialize).

State of the world:

- *Step 1 closed* in commit `b8f1e67`. `Idx` trait with `'static`
  bound (R4.2), private `NonMaxU32` niche wrapper, declarative
  `newtype_index!` macro, `CrateId` with hand-rolled serde (route 1
  of R4.1). Codex ACCEPT.
- *Step 2 closed.* `IndexVec<I, T>` with `Vec<T>` inner and
  `PhantomData<fn(&I)>` marker (rustc-style variance and
  unconditional `Send + Sync`). Serde via `#[serde(transparent)]` +
  `#[serde(skip)]` produces a flat array of `T` on the wire. `I:
  Idx` bound only on `push`, `Index<I>`, `IndexMut<I>`. 11
  integration tests passing; Codex ACCEPT.
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

Implement stage-1 step 3 in `crates/regseq-model/`: `StringInterner`
with hand-rolled `Serialize`/`Deserialize` (storage only on the wire;
lookup rebuilt on deserialize; index-0-is-empty and no-duplicate
validation). See the `StringInterner` section of
`stage-1-vocabulary.md`. **Stage 1 is implemented directly in the
active session, not delegated to a separate implementer agent.**
Rationale: stage 1 is the foundation every later stage rides on,
and the conversation context built across the spec-design sessions
(including the methodology rules, decision history, and
spec-mutability posture) is hard to transfer cleanly to a fresh
agent. Once stage 1 is validated under compilation, stages 3-13
become more mechanical and can be delegated.

Foundation-first order (steps 1-2 done; remaining):

1. ~~`Idx` trait + `newtype_index!` analogue + custom serde for one
   ID.~~ *Done in commit `b8f1e67`.*
2. ~~`IndexVec<I, T>` with flat-array JSON shape.~~ *Done; closed in
   the present commit.*
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
