# Status

Last updated: 2026-04-27.

Consolidated status tracker for phase-1 RegseqModel work. Update at
the end of every session that changes anything material -- a stale
tracker is worse than no tracker.

## Stage progress (phase 1)

- [x] **Stage 0: Architectural foundation.** Q1-Q5 closed; see
  `architecture.md`.
- [ ] **Stage 1: Vocabulary.** ID types, StringInterner, SourceFile
  table, DefPath form, top-level RegseqModel struct. Pre-loaded
  with content from Q2 and Q5; not started.
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

Multi-crate strategy revert + rebuild (Q6 in `architecture.md`).
The facade collapse is being reverted as a single commit; the
three-tier wrapper dispatch and `regseq` facade will be rebuilt in
subsequent sessions with an updated `regseq_mir_passes()` whitelist
and Q1-Q5 alignment. The historical implementation in commit
`f0b2db1` is the structural reference.

## Open architectural decisions

None at present. Q1-Q6 are closed in `architecture.md`. Future
decisions land there as Q7, Q8, ... with references to evidence
and rationale.

## Blockers / waiting on

(none)

## Next action

Start stage 1 (vocabulary). Q2 and Q5 pre-loaded the content; see
`architecture.md` "Implications for stages 1-13" for the concrete
to-spec list.

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
