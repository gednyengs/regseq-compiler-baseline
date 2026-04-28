# Status

Last updated: 2026-04-27 (stage-1 research synthesis, F1-F4 locked).

Consolidated status tracker for phase-1 RegseqModel work. Update at
the end of every session that changes anything material -- a stale
tracker is worse than no tracker.

## Stage progress (phase 1)

- [x] **Stage 0: Architectural foundation.** Q1-Q7 closed; see
  `architecture.md`. Multi-crate three-tier dispatch infrastructure
  also rebuilt and verified end-to-end.
- [x] **Stage 1: Vocabulary.** ID types, StringInterner, SourceFile
  table, Span representation, DefPath form, Instance form, top-level
  RegseqModel struct, determinism rules, serialization conventions.
  Research synthesis (`stage-1-research.md`) complete and Codex-
  reviewed; F1-F4 locked. Spec (`stage-1-vocabulary.md`) drafted.
  Implementation (population of the `regseq-model` crate with the
  stage-1 types) not yet started; that is the next concrete work
  unit.
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

Stage 1 (vocabulary) spec drafted and reviewed across three rounds
(two Codex, one external). Research synthesis (`stage-1-
research.md`) complete; spec (`stage-1-vocabulary.md`) covers all
five components plus determinism, serialization, and pretty-printer
interface. Sixteen issues surfaced across the three review rounds,
all addressed; see `stage-1-review-log.md`.

Round-3 fixes (latest): added `CrateId` and `CrateInfo` for
multi-crate-graph identity (replacing the single-crate root
fields), pinned custom serde for IDs and `StringInterner` (derive
would expose internal encoding), pinned always-verbose
`DefPath.rendered`, made `line_text` 1-based to match `Position`,
tightened the bridge span invariant to keep both endpoints in the
same source file. Spec is implementation-ready.

Multi-crate rebuild (Q6) is complete:

- `crates/regseq/` exists as an empty facade crate with the
  `[package.metadata.regseq]` marker and the symmetric
  `register_tool` declarations in source.
- `RegseqAwareCallbacks` restored in `regseq-driver`, sharing the
  full cleanup whitelist with `RegseqCallbacks` via
  `regseq_mir_passes()`.
- `regseq_mir_passes()` updated to the canonical "clean MIR"
  whitelist matching `scripts/dump-fixture-mir.sh`.
- `RegseqCallbacks::after_analysis` is empty until the bridge
  lands; the prior extraction code in `extract/` and
  `construction/` stays as dormant reference (per
  disable-and-keep).
- `cargo-regseq` recognizes regseq-aware crates via
  `cargo metadata` and injects `REGSEQ_AWARE_CRATES`.
- Tests verify the dispatch end-to-end (default smoke + ignored
  full cargo build under wrapper).

Items will be promoted from each spec's `crate::regseq` submodule
to the shared `regseq` crate incrementally as redesign work
touches them. Intrinsic recognition follows Q7 (attribute-name
based, closed vocabulary).

## Open architectural decisions

None at present. Q1-Q7 are closed in `architecture.md`. Future
decisions land there as Q8, Q9, ... with references to evidence
and rationale.

## Blockers / waiting on

(none)

## Next action

Implement stage 1 in the `regseq-model` crate against
`stage-1-vocabulary.md` (post-three-review-rounds). Foundation-first
order recommended: (1) `Idx` + `newtype_index!` analogue + custom
serde for one ID, validate round-trip; (2) `IndexVec<I, T>` with
flat-array on-wire shape; (3) `StringInterner` with custom serde
and validation rules; (4) `SourceFile` + `Position` + `Span` with
file-local F1 rule; (5) `DefPathData` + `DefPathSegment` + `DefPath`
(now with `crate_id`) + `DefPathHash` + `CrateInfo` + `Instance`;
(6) top-level `RegseqModel` struct including `crates` and
`primary_crate`. Each step lands as its own commit; spec issues
surfaced during implementation trigger spec revision (the spec is
mutable per methodology) before continuing.

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
