# Stage 10 -- MIR Basic Blocks: Stage Memory

Stage-10-specific briefing for an LLM agent picking up the
basic-block work. Read in addition to (not instead of) the
top-level `../README.md` (architectural foundation, global
methodology rules) and `../status.md` (top-level progress
tracker).

This file is referenced **only** from `../status.md`'s
"Active focus" block. The top-level `../README.md` does not
reference it -- that's by convention, to keep top-level
context lean.

This file captures stage-10 *invariants* (mission, workflow,
principles, read order, Codex prompt template). Stage-10
*current state* (which decisions are locked, what's open,
active cluster, next concrete unit of work) lives in
`status.md`.

## 0. Mission

Stage 10 specifies the `BasicBlock` per-block container and
the body-level basic-block table that bind stage-8
`Statement` and stage-9 `Terminator` content into the unit
of MIR control flow. Unlike stages 7/8/9 (which specify
*vocabulary* enums), stage 10 is **structural**: it pins
the container shape, the body-level table shape, the
`BlockId` typed-index, and the ordering / reachability /
identity invariants that downstream consumers depend on.

Stage 10 sits **sixth in the MIR pipeline** (stages 5-11
form a strict bottom-up chain: places -> operands ->
rvalues -> statements -> terminators -> **basic blocks**
-> bodies). Its content is referenced by:

- Stage 11 `Body`: the body-level basic-block table is a
  field of `Body` (storage shape decided by D-10-2). The
  block-walk implementations stages 5/6/7/8/9 deferred
  (D-5-2 Part C, D-7-3 Part C, D-8-4 Part C, D-9-10 Part
  C) materialise as basic-block traversals at stage 11
  consuming stage-10's container shape.
- Stage 13 type-system pretty-printing: not directly
  consumed (basic-block rendering lives in stage 11
  alongside `render_place` / `render_operand` /
  `render_rvalue` / `render_statement` / `render_terminator`
  per D-10-K).
- Stage 14 backends: each `BasicBlock` lowers to a labelled
  C basic block: `bb_<id>: { <stmt_0>; ... <stmt_n>;
  <terminator>; }` where the labelling and ordering matter
  for goto-target resolution.

Stage 10 receives content from earlier stages:

- Stage 1 IDs (`BlockId` introduced here per the inbound
  expectation; `LocalId`, `ConstId`, `InstanceId`, `TyId`
  transitively through stages 8, 9). Stage 1 forward-
  declared `BlockId` conceptually but did not introduce
  the typed-index itself; stage 10 lands the `newtype_index!`
  application + the const-body macro arm extension + the
  `bbN` sigil rendering + the `Option<BlockId>` / `BlockId`
  4-byte size assertions.
- Stage 1 cleanup-pass whitelist: `ReorderBasicBlocks` in
  the pinned whitelist (see
  `crates/regseq-driver/src/callbacks.rs:25-36`) reverse-
  postorder reorders the IndexVec storage so that
  `BlockId(N)`'s successors are at index >= N (back-edges
  excepted). Stage 10 inherits this ordering as a positive
  invariant.
- Stage 8 `Statement` -- the per-block straight-line
  sequence (`BasicBlock.statements: Vec<Statement>` per
  the stage-9 §23.5 preview).
- Stage 9 `Terminator` -- the single control-flow exit
  (`BasicBlock.terminator: Terminator` per the stage-9
  §23.5 preview). rustc's `Option<Terminator>` is
  construction-only and collapses to non-Optional in the
  RegseqModel surface (rustc_public / Charon / mrustc
  all collapse the same way).

Stage 10 introduces no new top-level interner. Its content
is fully expressible in terms of stage-1 + stages 8, 9
vocabulary plus the body-local `BlockId`. Block content
lives inline inside the body's basic-block table, not in
any top-level interned table.

The output of stage 10 is `spec-snapshot.md` (the canonical
per-stage spec). This file briefs the agent on *how*
decisions about that spec are reached.

## 1. Workflow (the rhythm we use)

Stage 10 inherits the two-phase rhythm proven in stages 3,
4, 5, 6, 7, 8, and 9:

**Phase A -- Discussion pass.** In conversation: lay out
the open decisions in a cluster with options + recommendation
+ question(s). The user weighs in. Sub-questions are
answered. Reasoning chains are validated. Decisions emerge.

**Phase B -- Spec commit + adversarial review.** Locked
decisions land in `spec-snapshot.md` with the full audit
shape (Decision / Reasoning / Methodology rules / Prior-art
evidence / Worked example). A Codex review-correction loop
runs against the cluster:
1. Initial review prompt (template in Section 4).
2. Codex returns verdict (BLOCK / MINOR / APPROVE) +
   numbered issues.
3. Address each issue; record corrections in
   `review-log.md`.
4. Re-run Codex with a fresh prompt naming the corrections.
5. Repeat until APPROVE.

**Reviews are per-cluster, not per-decision and not
big-bang.** Stage 10's decision count is small (5 + K)
relative to stages 7/8/9, so a single combined cluster
covering D-10-1 through D-10-5 + D-10-K is the expected
shape (mirrors stages 6, 7, 8 single-cluster closures).
Parallel sessions are not anticipated.

`discussions.md` holds the distilled decision record;
`review-log.md` holds the round-by-round audit trail.

**Decision tag convention.** Stage 10 uses the prefixed
format `D-10-K` from day one (`D-10-1`, `D-10-2`, ..., with
sub-tags `D-10-1.1`, `D-10-1.2`, ..., per `../README.md`
§5.2). Stages 1 and 3 use legacy un-prefixed forms; stage 10
does not inherit those.

## 2. Stage-10-specific principles

Corollaries of `../README.md` "Methodology rules" applied
to the basic-block surface:

- **Store rustc's resolved basic-block form, not the surface
  expression.** The bridge consumes `mir::BasicBlocks<'tcx>`
  from `optimized_mir` with `mir_opt_level=0` and the
  cleanup-pass whitelist applied. Basic blocks at this stage
  are post-`ReorderBasicBlocks` (storage in reverse-postorder)
  and post-`RemoveStorageMarkers`/`SimplifyLocals-final`
  (stage-8-relevant Statement-side discharges have run).
  Mirror what rustc hands us at this surface.

- **Closed `BasicBlock` field set.** rustc_middle's
  `BasicBlockData<'tcx>` carries 4 fields (`statements`,
  `after_last_stmt_debuginfos`, `terminator: Option<...>`,
  `is_cleanup: bool`). RegseqModel keeps only the two
  C-relevant fields (`statements`, `terminator`) and
  collapses the Optional terminator to non-Optional. The
  three majority-of-peers projects (rustc_public, Charon
  ULLBC, mrustc) all use the same 2-field shape; stage-9's
  §23.5 already previewed this as the stage-10 vocabulary.

- **`is_cleanup` is dropped, not represented.** Per the
  stage-9 panic=abort discharge chain (D-9-9 rows 5-6:
  `AbortUnwindingCalls` + cleanup-block pruning leaves no
  `is_cleanup=true` blocks in `optimized_mir`), the field
  carries no information at our consumption point. Same
  discipline as stage 9's `unwind: UnwindAction` drop:
  fall-out of panic=abort + the cleanup-pass whitelist,
  not a parallel "is panic=abort" flag. **Compositional
  behavior beats special-case flags** -- per the README §2
  rule.

- **`after_last_stmt_debuginfos` is dropped.** Per stage-8
  D-8-3's debuginfo drop (no DWARF emission target;
  `StmtDebugInfos` removed from `Statement.kind`), the
  basic-block-level analog is symmetric: no DWARF
  emission, no debuginfo carry. Same discipline.

- **`Terminator` non-Optional.** rustc_middle stores
  `terminator: Option<Terminator<'tcx>>` and notes in the
  field docstring that None is construction-only (post-
  construction always Some; the `terminator()` accessor
  panics on None). RegseqModel stores `terminator:
  Terminator` (no Option). The bridge translator collapses
  the Option at the boundary; rustc_public, Charon, and
  mrustc all do the same.

- **Body-level container: bare `IndexVec<BlockId,
  BasicBlock>`, no CFG cache.** Per Q1 (crate split:
  `regseq-model` is rustc-free + zero analysis state;
  `regseq-analysis` owns derived analyses). rustc_middle's
  `BasicBlocks<'tcx>` wrapper carries `Arc<Cache>` with
  predecessors / RPO / dominators precomputed lazily;
  rustc_public, Charon, and mrustc all use bare-vector
  storage and recompute analyses on demand. RegseqModel
  follows the bare-vector majority: the IR is the storage,
  CFG analyses live in `regseq-analysis`. Per D-10-2.

- **`BlockId` typed-index, not bare `usize`.** Per stage-1's
  ID-conventions framework (every domain ID is a u32-backed
  `newtype_index!` instance with a sigil and `Option<X>`
  niche optimisation), stage 10 introduces `BlockId` as a
  proper typed-index. rustc_public's bare `BasicBlockIdx =
  usize` is rejected at our discipline level (no niche, no
  sigil, no type-safety against `LocalId` confusion).
  rustc, Charon, and mrustc all use typed forms (rustc's
  `BasicBlock` newtype_index, Charon's `Vector<BlockId,
  BlockData>`, mrustc's `unsigned int` typedef -- though
  mrustc's typedef is the weakest discipline). Per D-10-3.

- **`newtype_index!` const-body arm extension.** Per the
  stage-1 inbound expectation, the `newtype_index!` macro
  must accept a const-body arm so that
  `BlockId::START_BLOCK = BlockId::from_u32(0)` declares
  inline (matching rustc's `pub struct BasicBlock { const
  START_BLOCK = 0; }` grammar at `rustc_middle/src/mir/mod.rs:1289-1291`).
  The current empty-body arm must keep working unchanged.
  Per D-10-3.

- **Reverse-postorder ordering invariant.** Per the
  cleanup-pass whitelist (`ReorderBasicBlocks` set to true
  at `crates/regseq-driver/src/callbacks.rs:30`), the
  bridge consumes MIR with `BasicBlock(N)`'s successors at
  index >= N (back-edges excepted). RegseqModel inherits
  this ordering as a positive invariant: the
  `IndexVec<BlockId, BasicBlock>` storage IS in
  reverse-postorder; `iter_enumerated()` yields blocks in
  RPO. This is a stage-10 *positive* invariant, not a
  stage-1 inheritance: stage 1 specifies the cleanup-pass
  whitelist as a build-time configuration; stage 10
  formalises the ordering implication for the IR. Per
  D-10-4. **Charon does NOT preserve rustc's RPO** (its
  translator allocates `BlockId`s in DFS discovery order
  via worklist; see `stage-10/prior-art/charon.md`);
  RegseqModel's RPO-via-whitelist is the cleaner
  alternative.

- **`START_BLOCK = BlockId(0)` always.** Per rustc's
  `START_BLOCK = 0` const (`rustc_middle/src/mir/mod.rs:1290`)
  and the `ReorderBasicBlocks` invariant
  (`debug_assert_eq!(updater.map[START_BLOCK], START_BLOCK)`
  at `rustc_mir_transform/src/prettify.rs:33`), the entry
  block is always at index 0 regardless of reorder. Stage
  10 formalises this as a positive invariant. Per D-10-4.

- **Reachability is NOT an invariant.** RegseqModel
  carries all blocks the bridge receives, including blocks
  unreachable from `START_BLOCK`. rustc, rustc_public,
  Charon, and mrustc all carry unreachable blocks; the
  codegen-side `mono_reachable_*` traversals filter at
  consumption time, not at storage time. RegseqModel's
  consumers (stage-11 walks, stage-14 codegen) handle
  reachability filtering during traversal. Per D-10-5.

- **`BasicBlock` is body-local; no interning, no
  `BasicBlockId`.** BasicBlock lives inside body content
  (transitively at stage 11), not in a top-level interned
  table. `BlockId` is the body-local index *into* that
  body's basic-block table; it is NOT a project-level
  interner key. Equality and hashing on `BasicBlock` are
  structural over `(statements, terminator)`, walking
  through to stage-8 Statement and stage-9 Terminator's
  structural derives. Per D-10-1 and D-10-2 inline-storage
  rule.

- **F4 / F4-2 cross-build concerns are out of scope.**
  `BasicBlock` is body-local content; it does not
  participate in cross-build identity (the F4-2 instance
  ordering canonicalises `(InstanceKey)`, not
  `(BodyContents)`). Block storage and `BlockId` numbering
  are per-build, structural, and need not be canonicalised
  across build environments. Same scope reduction as
  stages 5, 6, 7, 8, 9.

- **Renderer style is decided deliberately.**
  `pretty.rs::render_basic_block` defers to stage 11
  alongside `render_place` (D-5-K), `render_operand`
  (D-6-K), `render_rvalue` (D-7-K), `render_statement`
  (D-8-K), `render_terminator` (D-9-K). What forces
  deferral: every basic block embeds Statement and
  Terminator content whose renderers are themselves
  deferred to stage 11. The `bbN` sigil rendering for
  `BlockId` (inbound from stage 1) is renderer-injected
  at the BasicBlock-rendering site (per
  `stage-10/prior-art/rustc_public.md` §6, where
  rustc_public's `function_body` writes `bb{index}: {`
  manually since `BasicBlockIdx` has no debug_format).
  Per D-10-K.

- **Compositional behavior beats special-case flags.**
  Same as stages 3-9. Examples for stage 10:
  - The drop of `is_cleanup` falls out of panic=abort +
    the cleanup-pass whitelist (stage 9 D-9-9 rows 5-6),
    not a parallel "is in panic=abort build" flag.
  - The drop of `after_last_stmt_debuginfos` falls out
    of stage-8 debuginfo discharge (D-8-3), not a
    parallel "no-DWARF" flag.
  - The non-Optional `terminator` falls out of the
    "post-construction" invariant (rustc's docstring
    at `rustc_middle/src/mir/mod.rs:1318-1324` already
    states it; rustc_public formalises it via the
    field type), not a parallel "construction-time
    None allowed" flag.
  - The RPO ordering invariant falls out of the
    cleanup-pass whitelist, not a parallel
    "is_in_rpo_order" flag on the body.

- **C-semantics for the basic-block surface.** Each
  `BasicBlock` lowers to a labelled C basic block:
  ```c
  bb_<N>: {
      <stmt_0>;
      ...
      <stmt_k>;
      <terminator>;  /* falls through to one of: goto, switch, return, abort, call+goto */
  }
  ```
  The `bb_<N>` label is the codegen-side translation of
  `BlockId(N)`. Per the C-shaping rule from `../README.md`
  "C semantics with information preservation": store what
  rustc gave us, render in C shape on demand at stage 14.

## 3. Read order for new agents picking up stage 10

In priority:

0. **If you are opening a specific cluster**: read this
   file + `status.md` + `discussions.md` entries for the
   decisions in your cluster (once they exist) + the
   relevant spec sections (top-level `../spec.md` Section
   2 (`BlockId` ID inventory + body-local ID constants;
   `IndexVec` semantics), Section 22 (`Statement`),
   Section 23 (`Terminator`)). Stage 10's `BasicBlock`
   content directly references stage-8 Statement and
   stage-9 Terminator vocabulary.

1. This file (`stage-10/memory.md`) -- stage-10 invariants.

2. `status.md` -- current state (locked decisions, active
   cluster, next concrete unit of work).

3. `spec-snapshot.md` -- the working spec (initially a
   copy of `../spec.md`; stage-10 contributions accumulate
   as new sections describing `BasicBlock`, `BasicBlocks`
   body-level container, `BlockId` introduction, RPO
   ordering invariant, and per-stage invariants registry).

4. `expectations.md` -- inbound forward refs (4 entries
   from stage 1 + bridge concerns).

5. `discussions.md` -- distilled decision records (created
   when the first decision lands; lazy).

6. `review-log.md` -- round-by-round Codex audit trail
   (created when the first review round runs; lazy).

7. `prior-art/<project>.md` -- per-stage primary-source
   research (fresh pass per `../README.md` §2 methodology
   rule -- do **not** reuse stage-0 summaries). Landed at
   stage open:
   - `prior-art/rustc.md` -- rustc primary source:
     `BasicBlock` newtype_index at `rustc_middle/src/mir/mod.rs:1262-1291`
     (with `START_BLOCK = 0` const-body grammar);
     `BasicBlockData<'tcx>` 4-field `#[non_exhaustive]`
     struct at `mod.rs:1308-1331` with the
     terminator-Optional-only-during-construction
     docstring; `BasicBlocks<'tcx>` wrapper at
     `basic_blocks.rs:14-19` with `Arc<Cache>` lazy CFG
     metadata (predecessors / RPO / dominators);
     `ReorderBasicBlocks` pass at
     `rustc_mir_transform/src/prettify.rs:14-42`
     (`is_required: false` but force-on via our cleanup
     whitelist; `START_BLOCK` invariance);
     `traversal.rs` reverse-postorder iterator;
     `is_cleanup` panic=abort discharge chain via
     `AbortUnwindingCalls`; codegen-side per-block walk
     references in `rustc_codegen_ssa/src/mir/mod.rs`
     and `rustc_codegen_cranelift/src/base.rs`;
     pretty-printer `bb{}` debug_format.
   - `prior-art/rustc_public.md` -- rustc_public primary
     source: `BasicBlock { statements, terminator }`
     2-field shape at `mir/body.rs:133-137` (the
     reference for stage-9's preview); `Body.blocks:
     Vec<BasicBlock>` flat at `body.rs:14-38` (no
     wrapper, no cache); `BasicBlockIdx = usize` bare
     alias at `body.rs:40` (the trade-off:
     serializability vs newtype discipline + niching +
     `bb{}` debug format); bridge translator silently
     drops `is_cleanup` / `after_last_stmt_debuginfos` at
     `unstable/convert/stable/mir.rs:24-33`;
     `Body::new` enforces only `locals.len() > arg_count`
     -- zero block-level invariants enforced; `function_body`
     pretty-printer at `pretty.rs:38-88` writes `bb{index}:
     {` manually.
   - `prior-art/charon.md` -- Charon primary source:
     ULLBC `BlockData { statements: Vec<Statement>,
     terminator: Terminator }` at
     `src/ast/ullbc_ast.rs:125-130` -- exact precedent
     for stage-9's preview shape; `Vector<BlockId,
     BlockData>` typed-IndexVec analog at
     `src/ast/ullbc_ast.rs` Body field, no cache;
     `generate_index_type!` macro at `src/ids/mod.rs:14-47`
     -- structural analog of `newtype_index!` with
     ZERO-as-impl-const + separate `pub static
     START_BLOCK_ID` (looser-locality alternative to the
     proposed const-body macro arm); translator at
     `translate_bodies.rs:1230-1265` allocates BlockIds in
     DFS discovery order via worklist, requires downstream
     `filter_unreachable_blocks` pass -- direct
     counter-example for RPO inheritance; **no dedicated
     block validator** (invariants enforced at construction
     sites: `unwrap` for terminator-non-None, `assert!`
     for `START_BLOCK_ID == BlockId::ZERO`); pretty-printer
     ~25 LOC across two `FmtWithCtx` impls at
     `src/pretty/fmt_with_ctx.rs:206-214,736-756`; sigil
     `bb{}` matches rustc.
   - `prior-art/mrustc.md` -- mrustc primary source:
     `BasicBlock { std::vector<Statement> statements,
     Terminator terminator }` 2-field shape at
     `src/mir/mir.hpp:726-730` -- direct match to
     stage-9's preview; bare `std::vector<BasicBlock>`
     at `Function::blocks` (`mir.hpp:753`), no wrapper,
     no cache; `typedef unsigned int BasicBlockId`
     (`mir.hpp:24`) -- bare typedef, weakest discipline
     of all peers; **NO RPO reorder pass** (block
     ordering is HIR-walk construction order,
     approximately RPO due to HIR structure but not
     enforced); `MIR_Validate` enforces
     terminator-non-DEAD, Incomplete-rejection,
     successor-bb-id-in-range, CFA structural reachability,
     BB0-as-entry, but NO is_cleanup check (no field), NO
     RPO check, NO all-blocks-reachable requirement;
     pretty-printer `MirDumper::dump_mir` uses `bbN: { ...
     }` envelope (`src/mir/dump.cpp:26-48`) -- sigil
     match. **Notable peer-design lesson**: mrustc +
     rustc_public form a 2-of-3 majority on "bare body-
     level container, no CFG cache" against rustc_middle's
     wrapper-with-cache. RegseqModel's
     `IndexVec<BlockId, BasicBlock>` candidate aligns with
     the bare-container side.

   Per-cluster top-up candidates (research only when the
   cluster's evidence demands it):
   - LLVM IR `BasicBlock` shape and the parent function's
     `BasicBlock` list (relevant if D-10-2's wrapper-vs-
     bare-vector decision needs an additional non-rustc
     peer).
   - Cranelift IR `Block` and `Layout` (relevant for
     D-10-4's RPO ordering invariant: Cranelift uses an
     explicit `Layout` linked-list distinct from the
     storage `EntityList<Block>`, an alternative to
     storage-order = RPO).
   - rustc-dev-guide MIR overview (https://rustc-dev-guide.rust-lang.org/mir/)
     for cross-reference of the `CFG` term.

8. `../README.md` -- top-level briefing (invariants only).

9. `../status.md` -- top-level progress tracker (cross-stage
   signals).

10. `../spec.md` -- merged top-level spec; consult Section
    2 (IDs and body-local ID constants), Section 22
    (`Statement`), Section 23 (`Terminator`).

## 4. Codex review cadence and prompt template

The cadence and current round status live in `status.md`
"Codex review cadence (status)." This section captures the
*template* (reusable across rounds and clusters).

Prompt template skeleton (paste-and-edit per round; per the
project root `CLAUDE.md` "Prompt shape" guidance):

```
<task>
Adversarial review of stage-10 basic-block spec, decisions <X>-<Y>, ROUND <N>.

[If N > 1: round context + summary of prior corrections]

Spec under review: docs/discussions/regseq-model/stage-10/spec-snapshot.md (focus: <sections>).
Out of scope: <e.g. unrelated D-tags, markdown style>.
Evidence files: <full list relevant to this cluster>.
Evaluate: <numbered criteria; 1. round-N-fix verification if N > 1; 2. new issues from corrections; 3. residual issues; 4-N. soundness/citation/consistency>.
</task>

<grounding_rules>
Cite file:line for every claim. Do not speculate. Mark hypotheses as hypotheses. Distinguish "what spec claims" from "what evidence file says."
</grounding_rules>

<structured_output_contract>
First line: VERDICT: <BLOCK | MINOR | APPROVE>
Then numbered issues: N. [BLOCK | MINOR | NIT] <tag> <line ref>
  <description> <suggested fix>
Hard cap 25; NITs only with budget.
Last line: TOTAL: <N> issues (<B> BLOCK / <M> MINOR / <T> NIT)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

Run via:
```bash
cat <<'EOF' | uv run python /<repo-root>/scripts/python/codex_runner.py --cwd /<repo-root> --effort high --timeout 1500 --message-only > /tmp/codex_d<cluster>_review_r<N>.txt 2>&1
<prompt>
EOF
```

with `run_in_background: true`. Capture output to `/tmp/...`.
Record each round in `review-log.md` with verdict, issue
list, and corrections applied.

## 5. What this file is NOT

- **Not the stage status tracker.** Use `status.md` for
  current state (locked decisions, active cluster, next
  concrete unit of work, blockers, deferred items).
- **Not a substitute for `spec-snapshot.md` (the spec) or
  `discussions.md` (the decision records).** Briefing only.
- **Not append-only.** Update sections in place when
  stage-10 invariants change (rare).
- **Not where decisions live.** Decisions live in
  `spec-snapshot.md` (and `discussions.md` for audit shape).
