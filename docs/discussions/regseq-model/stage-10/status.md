# Stage 10 Status

Status: **STAGE 10 CLOSED AND MERGED 2026-05-05.** All
6 top-level D-tags + 21 sub-decisions LOCKED across a
single combined cluster (D-10-1 BasicBlock struct shape,
D-10-2 body-level container, D-10-3 BlockId
introduction, D-10-4 ordering invariants, D-10-5
invariants registry, D-10-K renderer deferral). Merger
contract per README §5.5 completed: top-level
`spec.md`, `discussions.md`, `review-log.md`,
`decisions.md` all extended with stage-10 content;
post-merge tier-3 Codex review closed 2026-05-05 at R4
MINOR-with-Lockability:yes-and-close-out:YES (4 rounds;
10 distinct findings closed: 1 BLOCK + 5 MINOR + 4 NIT).

This `spec-snapshot.md` is now a frozen historical
artifact (see header). The current authoritative spec
lives in top-level `../spec.md`, which includes the
post-merge §23.1.4 `Terminator.successors()` helper
addition (per the missing-helper-definition BLOCK
surfaced at post-merge R1).

Total stage-10 Codex cost: **9 rounds** (5 per-stage +
4 post-merge); **29 distinct findings** closed (4 BLOCK
+ 16 MINOR + 9 NIT). Phase A closed 2026-05-05 with
all 6 D-tags locked in conversation; Phase B closed
2026-05-05 at per-stage R5; merger close-out 2026-05-05
at post-merge R4.

Open phase recap (2026-05-05): stage-10 folder
bootstrapped per `../README.md` §5.1; spec-snapshot.md
copied from top-level `../spec.md` (9502 lines); inbound
`expectations.md` read (4 entries from stage 1 + bridge
concerns); rustc kickoff evidence pass completed
(`BasicBlock` newtype_index at
`rustc_middle/src/mir/mod.rs:1262-1291` with `START_BLOCK = 0`
const-body grammar; `BasicBlockData<'tcx>` 4-field shape at
`mod.rs:1308-1331`; `BasicBlocks<'tcx>` wrapper with lazy
`Arc<Cache>` at `basic_blocks.rs:14-19`;
`ReorderBasicBlocks` `is_required: false` but force-on via
the cleanup whitelist at `crates/regseq-driver/src/callbacks.rs:30`,
preserves `START_BLOCK` per `prettify.rs:33`); 6 D-tags
allocated; per-stage prior-art research landed in 4
primary-source files (rustc.md 1207 lines, rustc_public.md
565 lines, charon.md 533 lines, mrustc.md 913 lines = 3218
lines total); memory.md and this status.md drafted from
grounded knowledge.

**Ready for Phase B spec commit.** All 6 D-tags locked in
conversation; full audit shape lands in
`stage-10/discussions.md` and spec content lands as §24
in `stage-10/spec-snapshot.md`.

Last updated: 2026-05-05 (Phase A complete; Phase B
begins).

For top-level project status, see `../status.md`. For
project-wide invariants, see `../README.md`. For stage-10
invariants (workflow, principles, read order, Codex prompt
template), see `memory.md`. For the working spec, see
`spec-snapshot.md`. For inbound forward refs, see
`expectations.md` (4 entries from stage 1 + bridge
concerns). For the audit trail, see `discussions.md`
(lazy; created when the first decision lands). For the
round-by-round Codex audit trail, see `review-log.md`
(lazy; created at first round).

## 0. Inbound expectations checklist

From `expectations.md`:

- [x] **Stage 1 -- Define `BlockId` via `newtype_index!`.**
  Stage 1 forward-declared `LocalId` and `BlockId` only
  conceptually; the IDs themselves are stage-10/11
  concerns. Stage 10 introduces `BlockId` using the
  existing macro at `crates/regseq-model/src/idx.rs`.
  **Closed by D-10-3.1** (spec-side; impl lands at
  end-of-phase-1 sweep).
- [x] **Stage 1 -- `newtype_index!` const-body arm.** The
  current macro accepts only empty-body invocations
  (`pub struct $Name {}`). Stage 10 needs a const-body
  arm so `BlockId::START_BLOCK = BlockId::from_u32(0)`
  declares inline. The empty-body arm must keep working
  unchanged. **Closed by D-10-3.2** (macro grammar
  pinned in §24.3.2; impl lands at end-of-phase-1
  sweep; `LocalId::RETURN_PLACE` migration deferred to
  stage 11).
- [x] **Stage 1 -- `bbN` sigil rendering.** Add
  `BlockId`-rendering to `pretty.rs`. The spec sigil is
  `bbN` (e.g., `bb0`, `bb1`). Textual convention pinned
  in §24.3.4; **rendering wiring closed by D-10-K.2**
  (deferred to stage 11 alongside `render_basic_block`,
  introduced at first-consumer site).
- [x] **Stage 1 -- Niche optimization for
  `Option<BlockId>`.** When stage 10 adds `BlockId`,
  append the corresponding `Option<BlockId>` and
  `BlockId` 4-byte size assertions to
  `crates/regseq-model/src/lib.rs`. **Closed by
  D-10-3.3** (assertions land at the same spec-section
  as the `newtype_index!` application; spec-side
  pinned in §24.3.3; impl lands at end-of-phase-1
  sweep).
- [x] **Bridge concern -- Block ID assignment in
  reverse-postorder.** The cleanup-pass whitelist at
  `crates/regseq-driver/src/callbacks.rs` runs
  `ReorderBasicBlocks` (reverse-postorder block IDs)
  before MIR reaches the bridge. Stage 10 inherits this
  ordering when assigning `BlockId`s. **Closed by
  D-10-4.1** (B-RPO positive invariant; walk-checked
  at stage 11 per §24.4.3).

## 1. D-tag allocation

D-tags allocated upfront from grounded rustc evidence
(per `../README.md` §5.1 step 5). Sub-decisions
(`D-10-1.1`, `D-10-2.1`, ...) allocated during Phase A
as evidence demanded.

| Tag      | Status     | Locked content (one-line summary)                                                                                                                                          |
|----------|------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| D-10-1   | [LOCKED]   | 2-field `BasicBlock { statements: Box<[Statement]>, terminator: Terminator }`; drop `is_cleanup` + `after_last_stmt_debuginfos`; non-Optional terminator; no metadata.     |
| D-10-1.1 | [LOCKED]   | Cardinality: 2 fields (`statements`, `terminator`).                                                                                                                        |
| D-10-1.2 | [LOCKED]   | `statements: Box<[Statement]>` (RegseqModel internal pattern; finalized payload; stage-7/9 `Box<[T]>` precedent).                                                           |
| D-10-1.3 | [LOCKED]   | `terminator: Terminator` non-Optional (type-level encoding of post-construction invariant; rustc_public/Charon/mrustc all do this).                                         |
| D-10-1.4 | [LOCKED]   | Drop `is_cleanup` (panic=abort discharge from stage-9 D-9-9 rows 5-6).                                                                                                     |
| D-10-1.5 | [LOCKED]   | Drop `after_last_stmt_debuginfos` (debuginfo discharge from stage-8 D-8-3).                                                                                                |
| D-10-1.6 | [LOCKED]   | No `BasicBlock`-level metadata (no span, no scope; spans live on Statement and Terminator).                                                                                |
| D-10-2   | [LOCKED]   | Bare `IndexVec<BlockId, BasicBlock>` storage; no wrapper / no cache / no alias; CFG analyses live in `regseq-analysis` per Q1 + principled rule (model-vs-analysis line).   |
| D-10-2.1 | [LOCKED]   | Storage: `IndexVec<BlockId, BasicBlock>` (typed-IndexVec; stage-1 §2.6 canonical).                                                                                          |
| D-10-2.2 | [LOCKED]   | No wrapper struct; bare type at field-declaration sites.                                                                                                                    |
| D-10-2.3 | [LOCKED]   | No CFG cache (predecessors / dominators / etc.); analyses live in `regseq-analysis`. Principled rule documented in §spec.                                                    |
| D-10-2.4 | [LOCKED]   | No type alias.                                                                                                                                                              |
| D-10-3   | [LOCKED]   | `BlockId` via `newtype_index!`; macro extended with const-body arm; size/niche assertions; sigil `bbN`; `LocalId::RETURN_PLACE` migration deferred to stage 11.            |
| D-10-3.1 | [LOCKED]   | `BlockId` introduced via `newtype_index!` in `crates/regseq-model/src/idx.rs` (mechanical inheritance from stage-1 ID conventions).                                          |
| D-10-3.2 | [LOCKED]   | `newtype_index!` macro extended with const-body arm `pub struct $Name { const NAME = INT_LITERAL; }`; empty-body arm preserved; `LocalId::RETURN_PLACE` migration -> stage 11. |
| D-10-3.3 | [LOCKED]   | `size_of::<BlockId>() == 4` and `size_of::<Option<BlockId>>() == 4` static assertions appended to `crates/regseq-model/src/lib.rs`.                                        |
| D-10-3.4 | [LOCKED]   | Sigil textual convention `BlockId(N)` -> `bbN`; macro `Debug` impl unchanged; rendering site deferred to stage 11 under D-10-K.                                            |
| D-10-4   | [LOCKED]   | B-RPO storage order + B-Entry `START_BLOCK = BlockId(0)`; walk deferred to stage 11 (joint DFS-RPO procedure with B-Reachable per Round-1 promotion); no defensive fallback for non-RPO inputs. |
| D-10-4.1 | [LOCKED]   | B-RPO: `Body.basic_blocks` storage is in reverse-postorder of the CFG rooted at `START_BLOCK`. Inherited from `ReorderBasicBlocks` in cleanup whitelist.                    |
| D-10-4.2 | [LOCKED]   | B-Entry: `BlockId::START_BLOCK == BlockId(0)` is the entry block. Inherited from rustc's `START_BLOCK` const + `prettify.rs:33` fixed-point.                                 |
| D-10-4.3 | [LOCKED]   | Stage-11 owns walk-implementation that jointly enforces B-RPO + B-Reachable + B-Entry via the §24.4.3 DFS-RPO procedure (steps 3-6), preceded by B-NonEmpty + B-Successors-Valid pre-checks (steps 1-2). Outbound expectation filed at lock time. |
| D-10-4.4 | [LOCKED]   | No defensive fallback for non-RPO inputs (per "no defensive fallbacks" methodology rule + pinned-toolchain contract).                                                       |
| D-10-5   | [LOCKED]   | 5-part invariants registry: 3-row rejection table (bridge-enforced) + 8 positive invariants (2 inherited + 6 stage-10-introduced incl. B-Reachable) + walk-impl deferred to stage 11 (Part B only) + F4/F4-2 OUT + Part E forward-looking (no stage-10 non-invariants). |
| D-10-K   | [LOCKED]   | `render_basic_block` deferred to stage 11 alongside D-5-K through D-9-K cascade; sigil helper introduced at stage 11 alongside first consumer; §18.2 row added.            |
| D-10-K.1 | [LOCKED]   | `render_basic_block(&BasicBlock, &Body)` materializes at stage 11; closes the D-5-K through D-9-K renderer-deferral cascade.                                                |
| D-10-K.2 | [LOCKED]   | `BlockId` sigil-rendering helper introduced at stage 11 alongside first consumer; textual convention `bbN` pinned in §24.3.4 (stage-10) but renderer wiring deferred.        |
| D-10-K.3 | [LOCKED]   | §18.2 per-method ownership table extended with `render_basic_block` row (deferred to stage 11); sigil helper not public surface, no §18.2 row.                              |

Locked tag count: 6 top-level + 21 sub-decisions = 27
locks total. All locked-in-conversation 2026-05-05;
Phase B per-stage Codex review CLOSED 2026-05-05 at
R5 MINOR-with-Lockability:yes-and-close-out:YES (5
rounds; 19 distinct findings closed: 3 BLOCK + 12
MINOR + 4 NIT across the iteration). Ready for
top-level merger contract per README §5.5.

## 2. Active cluster

**Phase A complete.** Single combined cluster covered all
6 D-tags in conversation (mirrors stages 6, 7, 8
single-cluster closures). Final Phase A discussion order:
D-10-3 -> D-10-1 -> D-10-2 -> D-10-4 -> D-10-5 -> D-10-K.

D-10-2 surfaced one substantive Phase A discussion thread
(should derived CFG analyses such as predecessors /
dominators be stored in `regseq-model`?) that resolved
into a principled rule documented in §spec: *"the model
stores facts the bridge owns or rustc has resolved; the
model does not store analyses derivable from those facts
via standard CFG/SSA algorithms. The single exception --
RPO ordering of `basic_blocks` storage -- is justified
by being the layout itself, not a separate field."*

**Phase B active.** Spec commit + Codex review in
progress.

## 3. Codex review cadence (status)

Per-stage review trajectory (single combined cluster;
template in `memory.md` §4):

- **Round 1** (2026-05-05): VERDICT BLOCK; 7 issues
  (2 BLOCK / 4 MINOR / 1 NIT). All 7 closed via
  corrections. See `review-log.md` Round 1.
- **Round 2** (2026-05-05): VERDICT BLOCK; 5 issues
  (1 BLOCK / 3 MINOR / 1 NIT) -- 6 of 7 Round-1 fixes
  RESOLVED, 1 NEW-ISSUE-INTRODUCED in B-Reachable
  evidence (rustc-source-inaccurate
  `invert_bijective_mapping` claim from Round 1 fix).
  All 5 closed via corrections. See `review-log.md`
  Round 2.
- **Round 3** (2026-05-05): VERDICT MINOR; 3 issues
  (0 BLOCK / 2 MINOR / 1 NIT) -- all 5 Round-2 fixes
  RESOLVED. Per-D-tag Lockability: yes for all 6
  D-tags (with minor cleanup pending). 3 issues
  closed via corrections. See `review-log.md`
  Round 3.
- **Round 4** (2026-05-05): VERDICT MINOR; 2 issues
  (0 BLOCK / 1 MINOR / 1 NIT) -- all 3 Round-3 fixes
  RESOLVED. Per-D-tag Lockability: yes for all 6
  D-tags. Phase B close-out: NO with caveats pending
  the 2 wording cleanups. Both closed via
  corrections. See `review-log.md` Round 4.
- **Round 5** (2026-05-05): VERDICT MINOR; 2 issues
  (0 BLOCK / 1 MINOR / 1 NIT) -- both Round-4 fixes
  RESOLVED. **Phase B close-out: YES.** Per-D-tag
  Lockability: yes for all 6 D-tags (with two
  bookkeeping items closed via post-verdict
  corrections: stale Charon citation in discussions.md
  D-10-3.4 reasoning step 1, and missing D-10-K.1 /
  K.2 / K.3 sub-decision rows in the status.md
  D-tag table). Both closed; total locks updated to
  27 (6 + 21). See `review-log.md` Round 5.

**Per-stage Codex review CLOSED.** Total findings
across 5 rounds: **19 distinct issues closed (3 BLOCK
+ 12 MINOR + 4 NIT)** -- comparable to stage-9's
trajectory (12 rounds total including post-merge,
51 findings; stage-10's per-stage portion is tighter
because the surface is structural rather than a
big-vocabulary enum).

Hand-off to top-level merger contract per README
§5.5 (7-step protocol) is the next concrete unit of
work (see §5).

## 4. Open phase set-up recap

For audit / methodology trace:

- [x] Stage-10 folder bootstrapped (already existed with
  `expectations.md` from stage-1 forward refs).
- [x] `spec-snapshot.md` copied from top-level
  `../spec.md` (9502 lines) as Phase A working starting
  point.
- [x] `expectations.md` read as entry checklist (5
  entries: 4 from stage 1 + 1 bridge concern).
- [x] Rustc kickoff evidence pass completed:
  - `rustc_middle/src/mir/mod.rs:1262-1291`
    (`BasicBlock` newtype_index with `START_BLOCK = 0`
    const-body grammar; `bb{}` debug_format).
  - `rustc_middle/src/mir/mod.rs:1308-1413`
    (`BasicBlockData<'tcx>` 4-field struct + methods;
    `terminator: Option<...>` with construction-only
    docstring at lines 1318-1324).
  - `rustc_middle/src/mir/basic_blocks.rs:14-178`
    (`BasicBlocks<'tcx>` wrapper with lazy
    `Arc<Cache>` for predecessors / RPO / dominators).
  - `rustc_middle/src/mir/traversal.rs` (Postorder /
    reverse_postorder iterators).
  - `rustc_mir_transform/src/prettify.rs:14-42`
    (`ReorderBasicBlocks` pass: `is_required: false`,
    force-on via our cleanup whitelist; `START_BLOCK`
    fixed-point at line 33).
  - `crates/regseq-driver/src/callbacks.rs:25-36`
    (`regseq_mir_passes()` sets `ReorderBasicBlocks`
    to `true`, putting it in the force-on cleanup
    whitelist).
  - `rustc_public/src/mir/body.rs:14-200` (rustc_public
    `Body { blocks: Vec<BasicBlock> }`; `BasicBlock {
    statements, terminator }`; `BasicBlockIdx = usize`
    bare alias).
- [x] D-tags allocated (6 total: D-10-1 through D-10-5 +
  D-10-K).
- [x] Per-stage prior-art research landed:
  `prior-art/rustc.md` (1207 lines),
  `prior-art/rustc_public.md` (565 lines),
  `prior-art/charon.md` (533 lines),
  `prior-art/mrustc.md` (913 lines) -- 3218 lines total.
- [x] `memory.md` drafted (workflow, principles, read
  order, Codex template) from grounded knowledge.
- [x] `status.md` drafted (this file) from grounded
  knowledge.

## 5. Next concrete unit of work

**Phase B closed 2026-05-05.** All steps complete:

1. [done] Updated this file (Phase A close inventory
   + Phase B per-round status).
2. [done] Drafted `discussions.md` with audit shape
   (Decision / Reasoning / Methodology rules /
   Prior-art evidence / Worked example) per locked
   D-tag (6 top-level + 21 sub-decisions).
3. [done] Appended §24 Basic-block vocabulary to
   `spec-snapshot.md` (formal pin for all 6 D-tags).
4. [done] Updated `spec-snapshot.md` in place:
   §23.5 -> forward-reference to §24 + locked-form
   `Box<[Statement]>`; §22 forward references
   updated; §18.2 per-method ownership row for
   `render_basic_block`.
5. [done] Ran Codex per-stage review (5 rounds);
   closed at R5 MINOR-with-Lockability:yes-and-
   close-out:YES.

**Hand-off to top-level merger** (next concrete unit
of work):

Per README §5.5 7-step merger protocol:
1. Integrate `stage-10/discussions.md` into top-level
   `discussions.md` under `## Stage 10` header
   (mechanical concat).
2. Integrate `stage-10/review-log.md` into top-level
   `review-log.md` under `## Stage 10` header
   (mechanical concat).
3. Append `D-10-{1..5,K}` + 21 sub-decision entries
   to top-level `decisions.md` under `## Stage 10`
   section ([LOCKED] status).
4. Flip status of any earlier-stage decisions
   reopened by stage 10 (none expected; stage-10's
   spec contributions are additive rather than
   refining).
5. Integrate `stage-10/spec-snapshot.md` §24 (and
   in-place updates to §22 / §23.5 / §18.2) into
   top-level `spec.md` per README §5.9 update
   guidelines (strip audit shape; topic-organized).
6. Run Codex post-merge review pass on integrated
   `spec.md`. Iterate to APPROVE.
7. Update top-level `status.md` to mark stage 10
   closed; freeze `stage-10/spec-snapshot.md` with
   "Snapshot at close of stage 10" header; announce
   stage-11 entry point.

File outbound expectations to stage-11/expectations.md
"From stage 10" at step 7 (the 5-item bundle in §8
below).

## 6. Blockers

None. All upstream dependencies are pinned:

- Stage 1 (foundation crate, including `newtype_index!`
  macro at `crates/regseq-model/src/idx.rs`) -- closed.
- Stage 8 (`Statement` vocabulary; embedded in
  `BasicBlock.statements`) -- closed (top-level
  `../spec.md` §22).
- Stage 9 (`Terminator` vocabulary; embedded in
  `BasicBlock.terminator`; §23.5 already previewed
  stage-10 `BasicBlock` shape) -- closed (top-level
  `../spec.md` §23).

## 7. Deferred items (stage-10-specific)

To be populated as Phase A surfaces them. Stage-specific
deferrals live here; cross-stage deferrals live in
`../status.md` "Deferred items (cross-stage)".

(none yet)

## 8. Outbound expectations to file at Phase B close

To file in `stage-11/expectations.md` "From stage 10"
once Phase B's Codex review APPROVES:

1. **`render_basic_block` deferral** (per D-10-K.1) --
   stage 11 owns the basic-block renderer alongside
   `render_place` / `render_operand` / `render_rvalue` /
   `render_statement` / `render_terminator`.
2. **Body walk implementation per D-10-5 Part C** --
   one pass over `body.basic_blocks.iter_enumerated()`
   plus a DFS-RPO traversal from `BlockId(0)` per
   §24.4.3, checking the RegseqModel-visible Part B
   positive invariants in fixed step order:
   B-NonEmpty pre-check (step 1); B-Successors-Valid
   pre-check (step 2; guards subsequent indexing);
   B-RPO + B-Reachable + B-Entry jointly via the
   DFS-RPO procedure (steps 3-6). Part A is
   bridge-enforced at construction (per §24.7); the
   walk does not re-check it. Mirrors stage-8 D-8-4
   Part C and stage-9 D-9-10 Part C walk-impl
   deferrals.
3. **`BlockId` sigil-rendering helper** (per D-10-K.2) --
   stage 11 introduces the helper alongside its first
   consumer (`render_basic_block` envelope label,
   `render_terminator`'s `Goto` / `SwitchInt` targets).
4. **Body-level basic-block table integration** (per
   D-10-2) -- `Body.basic_blocks: IndexVec<BlockId,
   BasicBlock>` lands as a public field of the stage-11
   `Body` struct.
5. **`LocalId::RETURN_PLACE` migration** (per D-10-3.2
   deferral) -- when stage 11 formally introduces
   `LocalId` as a typed-index, migrate its
   `RETURN_PLACE` const to use the new `newtype_index!`
   const-body arm so body-local IDs share one
   convention.

Filing happens at Phase B close (after per-stage Codex
APPROVES); not yet filed.

## 9. How to update this file

- **Status line at top:** updated at each phase transition
  (Open complete -> Phase A active -> Phase B active
  cluster N -> stage closed).
- **D-tag allocation table:** updated when a tag
  transitions [DRAFTED] -> [LOCKED] / [DEFERRED] /
  [REOPENED]. Sub-tags appended as they are allocated
  during Work.
- **Active cluster:** narrowed as Phase A progresses.
- **Codex review cadence:** updated per round with
  verdict + issue counts.
- **Inbound expectations checklist:** marked `[x]` as
  D-tags close inbound items.
- **Deferred items:** appended as Phase A surfaces them.
- **Outbound expectations:** moved to "filed" once
  written to the relevant later-stage's
  `expectations.md`.
