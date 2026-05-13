# Stage 10 -- Discussion Record

Distilled record of the design dialogue producing the
stage-10 `BasicBlock` vocabulary spec. Each entry captures
the question raised, key options considered, the decision,
the reasoning chain, methodology rules invoked, prior-art
evidence, and cross-references to `spec-snapshot.md` (the
spec) and `review-log.md` (the round-by-round audit
trail; created on first review round).

Status of decisions in this file mirrors `spec-snapshot.md`'s
"Basic-block vocabulary" section (§24) plus adjacent
additions to §18.2 and §23.5, plus `status.md`
"D-tag allocation".

D-tag convention: prefixed `D-10-K` form per `../README.md`
§5.2.

Status tags:
- `[LOCKED]` -- audit-shape recorded; per-stage Phase A
  closed; ready for Phase B Codex review.
- `[REOPENED]` -- a later sub-decision or sibling D-tag
  forced a revision; superseded entry kept for history.
- `[DEFERRED to stage N]` -- moved to a later stage's scope.

Phase A closed 2026-05-05 with all 6 top-level D-tags + 18
sub-decisions LOCKED across a single combined cluster.

---

## 1. Decision discussions

### D-10-1 -- `BasicBlock` struct shape [LOCKED]

**Question.** What is the closed `BasicBlock` per-block
container shape that the bridge fills from rustc's
`mir::BasicBlockData<'tcx>` and that downstream consumers
(stage 11 bodies, stage 14 backends) read as the unit of
MIR control flow?

**Key options.**
1. **Mirror rustc verbatim (4 fields).** Carry
   `statements: Vec<Statement>`,
   `after_last_stmt_debuginfos: StmtDebugInfos`,
   `terminator: Option<Terminator>`, `is_cleanup: bool`.
2. **Mirror rustc_public's slim shape (2 fields).** Drop
   debuginfo + `is_cleanup`; collapse `Option<Terminator>`
   to non-Optional via the bridge boundary.
3. **mrustc-style 2 fields with debuginfo merge.** Same
   2-field shape but with statement-level debuginfo
   re-attached pre-bridge.
4. **RegseqModel 2 fields, finalized payload.** 2-field
   shape per stage-9 §23.5 preview with the `statements`
   field in `Box<[T]>` finalized form (not `Vec<T>`).

**Decision.** Option 4 -- **2 fields**
(`statements: Box<[Statement]>`, `terminator: Terminator`).
Drops `is_cleanup` (D-10-1.4) and `after_last_stmt_debuginfos`
(D-10-1.5); collapses `Option<Terminator>` to non-Optional
(D-10-1.3); no `BasicBlock`-level metadata beyond these
fields (D-10-1.6).

**Reasoning chain.**
1. The `optimized_mir` pipeline plus our cleanup-pass
   whitelist eliminates `is_cleanup=true` blocks
   structurally (per stage-9 D-9-9 rows 5-6:
   `AbortUnwindingCalls` rewrites cleanup-edges + cleanup-
   block pruning under panic=abort). The field carries
   no information at our consumption point.
2. RegseqModel emits no DWARF (per stage-8 D-8-3
   debuginfo discharge); `after_last_stmt_debuginfos` is
   the basic-block-level analog of the per-statement
   `debuginfos: StmtDebugInfos` field already dropped at
   stage 8. Same disposition.
3. rustc's docstring at `mod.rs:1318-1324` notes
   `terminator: Option<Terminator>` is None *only during
   construction*; `BasicBlockData::terminator()` accessor
   panics on None at `mod.rs:1357`. Three of four peers
   (rustc_public, Charon, mrustc) collapse the Option at
   the bridge boundary. RegseqModel mirrors the structural
   guarantee at the type level rather than encoding it as
   a runtime invariant.
4. `Box<[Statement]>` rather than `Vec<Statement>`:
   RegseqModel's existing pattern for finalized
   single-fill payloads (stage-7 `Aggregate(Box<[Operand]>)`,
   stage-9 `SwitchTargets.cases: Box<[(u128, BlockId)]>`)
   is `Box<[T]>`; basic-block statements have the same
   write-once / read-many lifecycle. Saves 8 bytes per
   block (no spare-capacity field) at no information
   loss.
5. No `BasicBlock`-level metadata: rustc / rustc_public /
   Charon / mrustc all carry zero block-level metadata.
   Spans live on Statement (stage 8 §22.1.1) and
   Terminator (stage 9 §23.1.1); the basic block as a
   whole has no span surface.

**Methodology rules invoked.**
- README §2 "Compositional behavior beats special-case
  flags" (drop `is_cleanup`: derives compositionally
  from panic=abort + cleanup-pass whitelist; no shadow
  flag).
- README §2 "C semantics with information preservation"
  (drop debuginfo: no DWARF emission target, so
  preservation is a no-op).
- README §2 "Re-derive from evidence; do not import
  baggage" (the 4-field rustc shape is consultative; the
  2-field shape is what the evidence actually demands).
- README §2 "No defensive fallbacks" (terminator
  non-Optional collapses the construction-only None at
  the type level rather than at every walk site).

**Prior-art evidence.**
- rustc 4-field `BasicBlockData<'tcx>` at
  `rustc_middle/src/mir/mod.rs:1308-1331`; terminator
  Optional-only-during-construction docstring at
  `mod.rs:1318-1324`.
- rustc_public 2-field `BasicBlock { statements,
  terminator }` at `rustc_public/src/mir/body.rs:133-137`;
  bridge translator silently drops `is_cleanup` /
  `after_last_stmt_debuginfos` at
  `rustc_public_bridge/src/unstable/convert/stable/mir.rs:24-33`.
- Charon ULLBC `BlockData { statements, terminator }` at
  `src/ast/ullbc_ast.rs:125-130`.
- mrustc `BasicBlock { statements, terminator }` at
  `src/mir/mir.hpp:726-730`.
- Stage-7 `Aggregate(Box<[Operand]>)` precedent for
  `Box<[T]>` finalized payloads.
- Stage-9 §23.5 informal "Stage-10 vocabulary preview"
  (using `Vec<Statement>`; revised to `Box<[Statement]>`
  per D-10-1.2 Phase-A reasoning).

**Cross-refs.**
- `spec-snapshot.md` §24.1 (BasicBlock struct + sub-fields).
- `spec-snapshot.md` §23.5 (in-place update: stage-9's
  "preview" replaced with forward-ref to §24).
- Sub-decisions: D-10-1.1 (cardinality), D-10-1.2
  (`statements` storage shape), D-10-1.3 (`terminator`
  non-Optional), D-10-1.4 (drop `is_cleanup`), D-10-1.5
  (drop `after_last_stmt_debuginfos`), D-10-1.6 (no
  metadata).

---

### D-10-1.1 -- `BasicBlock` cardinality [LOCKED]

**Question.** Specifically how many fields in
`BasicBlock`, and which?

**Decision.** **2 fields.** `statements` and `terminator`.
Drops 2 of rustc's 4 fields (`is_cleanup`,
`after_last_stmt_debuginfos`).

**Reasoning chain.**
1. Direct match to the 3-of-4 peer consensus
   (rustc_public, Charon ULLBC, mrustc).
2. Stage-9's §23.5 informal preview already proposed this
   shape; stage-10 owns the formal pin.
3. Each dropped field has a concrete discharge: D-10-1.4
   (`is_cleanup` -> panic=abort), D-10-1.5
   (`after_last_stmt_debuginfos` -> no DWARF).

**Methodology rules invoked.**
- README §2 "Compositional behavior beats special-case
  flags" (each drop is compositional, not flag-shaped).

**Prior-art evidence.**
- 3-of-4 peer consensus on 2-field shape (cited above
  under D-10-1).

**Cross-refs.**
- `spec-snapshot.md` §24.1.1.
- Parent: D-10-1.

---

### D-10-1.2 -- `statements: Box<[Statement]>` [LOCKED]

**Question.** Should the `statements` field use
`Vec<Statement>` (mutable, peer-aligned) or
`Box<[Statement]>` (finalized, RegseqModel-pattern-aligned)?

**Key options.**
1. **`Vec<Statement>`.** All four peers (rustc,
   rustc_public, Charon, mrustc) use this; spare
   capacity is wasted post-construction (24 bytes /
   block).
2. **`Box<[Statement]>`.** Aligns with stage-7
   `Aggregate(Box<[Operand]>)` and stage-9
   `SwitchTargets.cases: Box<[(u128, BlockId)]>`
   precedent for finalized payloads (16 bytes / block).

**Decision.** Option 2 -- **`Box<[Statement]>`**.

**Reasoning chain.**
1. Internal consistency outweighs peer consistency:
   stage 7 / stage 9 already chose `Box<[T]>` for
   finalized payloads; basic-block statements have the
   same shape (one-shot fill, immutable consumer).
2. The bridge knows the final length when constructing
   each block (it consumes a complete
   `Vec<Statement<'tcx>>` from rustc's
   `BasicBlockData.statements`); `Box<[Statement]>`
   *uses* that completeness rather than carrying
   construction-time spare capacity.
3. 8 bytes saved per block. Cumulatively meaningful
   across thousands of blocks per RegseqModel; zero
   information loss; consumer ergonomics unchanged
   (`Box<[T]>` derefs to `&[T]`).
4. Stage-8 spec text and stage-9 §23.5 preview both
   informally referenced `Vec<Statement>`. Per README
   §5.9 "later stage refines earlier stage's content"
   rule, the merger updates those forward references
   to `Box<[Statement]>` in place at top-level merge.

**Methodology rules invoked.**
- README §2 "C semantics with information preservation"
  (preserve completeness at the storage type when rustc
  has resolved it).
- README §2 "Re-derive from evidence; do not import
  baggage" (peer alignment is consultative; internal
  pattern alignment is load-bearing).

**Prior-art evidence.**
- All four peers use `Vec<T>` for the statements list
  (rustc `Vec<Statement<'tcx>>`, rustc_public
  `Vec<Statement>`, Charon `Vec<Statement>`, mrustc
  `std::vector<Statement>`).
- RegseqModel internal pattern: stage-7
  `Rvalue::Aggregate(_, Box<[Operand]>)`, stage-9
  `SwitchTargets.cases: Box<[(u128, BlockId)]>`.

**Cross-refs.**
- `spec-snapshot.md` §24.1.2.
- Parent: D-10-1.
- Stage-8 §22 forward references touched up at top-level
  merge (D-10-K outbound bundle includes the merger
  cross-touch).

---

### D-10-1.3 -- `terminator: Terminator` non-Optional [LOCKED]

**Question.** Should the `terminator` field be
`Option<Terminator>` (mirror rustc's construction-time
Optional) or non-Optional (encode the post-construction
invariant at the type level)?

**Decision.** **Non-Optional `Terminator`.**

**Reasoning chain.**
1. rustc's `BasicBlockData.terminator: Option<...>` is
   documented as None-only-during-construction;
   `BasicBlockData::terminator()` accessor panics on
   None.
2. RegseqModel consumes post-construction MIR (the
   `optimized_mir` query result); the construction-time
   None state is structurally inaccessible at our
   consumption boundary.
3. Three of four peers (rustc_public, Charon, mrustc)
   collapse to non-Optional. rustc_public's bridge
   translator at
   `rustc_public_bridge/src/unstable/convert/stable/mir.rs:24-33`
   makes the collapse explicit.
4. Type-level encoding of structural invariants is the
   stronger discipline; `Option<Terminator>` would add a
   runtime `unwrap` at every walk site for zero gain.

**Methodology rules invoked.**
- README §2 "No defensive fallbacks" (don't carry
  construction-time states the bridge has resolved).
- README §2 "Compositional behavior beats special-case
  flags" (the type-level guarantee is the rule, not a
  flag-shaped runtime check).

**Prior-art evidence.**
- rustc docstring at `rustc_middle/src/mir/mod.rs:1318-1324`.
- rustc_public non-Optional at
  `rustc_public/src/mir/body.rs:136`; bridge translator at
  `rustc_public_bridge/src/unstable/convert/stable/mir.rs:24-33`.
- Charon non-Optional at `src/ast/ullbc_ast.rs:128`.
- mrustc non-Optional at `src/mir/mir.hpp:729`.

**Cross-refs.**
- `spec-snapshot.md` §24.1.3.
- Parent: D-10-1.
- Becomes positive invariant B-Terminator-Some in D-10-5
  Part B (structural / type-level).

---

### D-10-1.4 -- Drop `is_cleanup` [LOCKED]

**Question.** Should `BasicBlock` carry a Boolean
`is_cleanup` flag (mirror rustc) or drop it?

**Decision.** **Drop.**

**Reasoning chain.**
1. Stage-9 D-9-9 rows 5-6 establish that under
   panic=abort + the cleanup-pass whitelist
   (`AbortUnwindingCalls` + `SimplifyCfg::PreOptimizations`),
   `is_cleanup=true` blocks are pruned before MIR
   reaches the bridge.
2. The field carries no driver-relevant information at
   our consumption point: it labels MSVC-style unwind
   paths, which don't exist in our pipeline.
3. Compositional discharge (panic=abort + cleanup-pass
   whitelist) replaces a parallel "is panic=abort"
   marker per the README compositional rule.
4. Defensive rejection at the bridge if a `is_cleanup=true`
   block survives is captured in D-10-5 Part A row 1
   (pipeline-misconfiguration diagnostic).

**Methodology rules invoked.**
- README §2 "Compositional behavior beats special-case
  flags".
- README §2 "No defensive fallbacks" (drop, don't
  carry-defensively-just-in-case).

**Prior-art evidence.**
- Stage-9 D-9-9 cleanup-block discharge chain
  (`AbortUnwindingCalls` at
  `rustc_mir_transform/src/abort_unwinding_calls.rs:115-121,137-138`).
- rustc_public silently drops at
  `rustc_public_bridge/src/unstable/convert/stable/mir.rs:24-33`.
- Charon never carries a cleanup flag.
- mrustc never carries a cleanup flag.

**Cross-refs.**
- `spec-snapshot.md` §24.1.4.
- Parent: D-10-1.
- Cross-ref: D-10-5 Part A row 1 (defensive rejection).

---

### D-10-1.5 -- Drop `after_last_stmt_debuginfos` [LOCKED]

**Question.** Should `BasicBlock` carry the basic-block-
level `after_last_stmt_debuginfos: StmtDebugInfos<'tcx>`
trailing-debuginfo container?

**Decision.** **Drop.**

**Reasoning chain.**
1. RegseqModel emits no DWARF (per stage-8 D-8-3
   debuginfo discharge); `after_last_stmt_debuginfos` is
   the basic-block-level analog of the per-statement
   `debuginfos: StmtDebugInfos` field already dropped at
   stage 8.
2. Same disposition as the per-statement debuginfo: silent
   drop at the bridge, NOT a defensive rejection. (Listed
   in D-10-5 Part A row 3 for table completeness only.)

**Methodology rules invoked.**
- Inheritance from stage-8 D-8-3 (debuginfo discharge).

**Prior-art evidence.**
- Stage-8 D-8-3 debuginfo drop pattern.
- rustc_public silently drops via the same translator
  pass that drops `is_cleanup`.

**Cross-refs.**
- `spec-snapshot.md` §24.1.5.
- Parent: D-10-1.
- Cross-ref: D-10-5 Part A row 3 (silent-drop completeness
  entry).

---

### D-10-1.6 -- No `BasicBlock`-level metadata [LOCKED]

**Question.** Should `BasicBlock` carry any block-level
metadata (span, scope, ...)?

**Decision.** **No metadata.** Spans live on Statement and
Terminator individually; the basic block as a whole has
no span / scope / metadata surface.

**Reasoning chain.**
1. rustc's `BasicBlockData` carries no block-level span
   or scope; spans propagate through the per-statement
   `Statement.source_info.span` and per-terminator
   `Terminator.source_info.span`.
2. rustc_public, Charon, mrustc all match: zero
   block-level metadata.
3. Stage-8 `Statement { kind, span: Span }` (D-8-3) and
   stage-9 `Terminator { kind, span: Span }` (D-9-8)
   each carry their own span; aggregating again at the
   block level would be redundant.

**Methodology rules invoked.**
- README §2 "C semantics with information preservation"
  (no information at the block level to preserve).

**Prior-art evidence.**
- All four peers carry zero block-level metadata.

**Cross-refs.**
- `spec-snapshot.md` §24.1.6.
- Parent: D-10-1.

---

### D-10-2 -- Body-level basic-block container [LOCKED]

**Question.** What shape does the body-level basic-block
table take? Wrapper struct with CFG cache (à la rustc) or
bare typed-vector storage (à la rustc_public / Charon /
mrustc)?

**Key options.**
1. **rustc-style wrapper-with-cache.** `BasicBlocks<'tcx>`
   wraps `IndexVec<BasicBlock, BasicBlockData>` plus
   `Arc<Cache>` for predecessors / RPO / dominators
   (lazy via OnceLock).
2. **rustc_public-style flat Vec.** `Body.blocks:
   Vec<BasicBlock>` with `BasicBlockIdx = usize`; no
   wrapper, no cache; bare `usize` indexing.
3. **Charon-style typed-Vector.** `Vector<BlockId,
   BlockData>` (Charon's typed-IndexVec analog); no
   wrapper, no cache.
4. **mrustc-style untyped Vec.** `std::vector<BasicBlock>`
   with bare-int indices; no wrapper, no cache.
5. **RegseqModel typed-IndexVec, no wrapper, no cache.**
   `IndexVec<BlockId, BasicBlock>` directly; CFG analyses
   live in `regseq-analysis`. Hybrid of rustc's typed
   indexing + the 3-of-4-peer bare-storage discipline.

**Decision.** Option 5 -- bare
**`IndexVec<BlockId, BasicBlock>`** storage with no
wrapper, no cache, no alias. CFG analyses (predecessors,
dominators, ...) live in `regseq-analysis` per Q1
architectural decision.

A principled rule emerged in Phase A discussion and is
documented in §24.2: *"the model stores facts the bridge
owns or rustc has resolved (types, layouts, const values,
instance identities); the model does not store analyses
derivable from those facts via standard CFG/SSA
algorithms. The single exception -- RPO ordering of
`basic_blocks` storage -- is justified by being the
layout itself, not a separate field; and by being the
order rustc's `ReorderBasicBlocks` already produces."*

**Reasoning chain.**
1. Q1 architectural decision: `regseq-model` is rustc-free
   with zero analysis state; derived analyses live in
   `regseq-analysis`. A cache field on the body-level
   container would put analysis state inside the model
   crate.
2. Information-preservation rule applies asymmetrically:
   rustc gives us *block ordering* for free (via
   `ReorderBasicBlocks` in the cleanup whitelist --
   captured as B-RPO at D-10-4.1); rustc does NOT give
   us predecessors / dominators for free (those are
   computed lazily on first access; cleared by the
   cleanup-pass whitelist via `as_mut()` at
   `rustc_mir_transform/src/prettify.rs:36`).
3. 3-of-4 peer alignment: rustc_public, Charon, mrustc
   all use bare-vector storage with no cache. Only
   rustc's wrapper-with-cache exists (motivated by
   in-compiler reuse, not applicable to RegseqModel's
   one-pass codegen).
4. Typed `IndexVec<BlockId, BasicBlock>` (rather than
   bare `Vec<BasicBlock>` à la rustc_public, or
   bare-integer-indexed à la mrustc): aligns with
   stage-1 §2.6 canonical-storage convention; gives
   type-safety against `LocalId`/`BlockId` confusion;
   matches all crate-level interner tables in
   RegseqModel.
5. No type alias: a `pub type BasicBlocks =
   IndexVec<BlockId, BasicBlock>` adds nothing
   semantically. Per README "Don't add features beyond
   what the task requires".
6. **Reachability is not stored as a cache or analysis
   field on `regseq-model`** (the load-bearing point
   for D-10-2.3). However, B-Reachable (every block
   reachable from `BlockId::START_BLOCK`) IS a
   stage-10 positive invariant per D-10-5 Part B,
   verified at the bridge boundary by the stage-11
   DFS-RPO walk procedure (§24.4.3 step 4). The
   invariant is enforced *as walk-checked validity*,
   not as cached analysis state -- consistent with
   D-10-2.3's model-vs-analysis line. (Round-1 Codex
   review surfaced the original "Reachability is NOT
   enforced" framing as conflicting with the rooted-RPO
   semantics of B-RPO; the resolution moved
   reachability into Part B as a checked invariant.)

**Phase-A discussion thread (predecessors / dominators
storage).** During discussion, the question arose whether
RegseqModel should preserve some of rustc's analyses
(e.g., dominators) rather than recomputing in
`regseq-analysis`. After per-analysis evaluation
(predecessors are cheap one-pass; dominators are
Lengauer-Tarjan; both derivable), the answer landed at
"no" with the principled rule articulated above.
Rationale: serde wire format hygiene (don't ship derived
data alongside primitive content), F4/F4-2 cross-build
identity surface (don't add canonicalization for
derivable fields), cache invalidation discipline (rustc's
`Arc<Cache>` is non-trivial; we shouldn't inherit the
burden for un-demonstrated benefit), and 3-of-4 peer
alignment.

**Methodology rules invoked.**
- README §2 "Compositional behavior beats special-case
  flags" (analyses derive compositionally from primitive
  content; storing them is shadow data).
- README §2 "Re-derive from evidence; do not import
  baggage" (rustc's wrapper-with-cache is consultative;
  3-of-4 peer alignment + Q1 are load-bearing).
- README §2 "C semantics with information preservation"
  (preserve what rustc *gave* us, not what rustc *would
  compute on demand*).
- Q1 architectural decision (`architecture.md`).

**Prior-art evidence.**
- rustc `BasicBlocks<'tcx>` wrapper at
  `rustc_middle/src/mir/basic_blocks.rs:14-19`;
  `Arc<Cache>` lazy CFG metadata at lines 32-37.
- rustc_public `Body.blocks: Vec<BasicBlock>` flat at
  `rustc_public/src/mir/body.rs:14-38`; no cache.
- Charon `Vector<BlockId, BlockData>` typed at
  `src/ast/ullbc_ast.rs`; no cache.
- mrustc `std::vector<BasicBlock>` at
  `src/mir/mir.hpp:753`; no cache.

**Cross-refs.**
- `spec-snapshot.md` §24.2 (body-level container; the
  principled rule lives in §24.2.3).
- Sub-decisions: D-10-2.1 (storage type), D-10-2.2 (no
  wrapper), D-10-2.3 (no cache), D-10-2.4 (no alias).

---

### D-10-2.1 -- Storage type: `IndexVec<BlockId, BasicBlock>` [LOCKED]

**Question.** Among `IndexVec<BlockId, BasicBlock>`,
`Vec<BasicBlock>`, and `Box<[BasicBlock]>`, which storage
type for the body-level basic-block table?

**Decision.** **`IndexVec<BlockId, BasicBlock>`.**

**Reasoning chain.**
1. Stage-1 §2.6 pins `IndexVec<I, T>` as the canonical
   typed-index dense storage; every existing
   crate-level interner table (`model.types`,
   `model.consts`, `model.items`, `model.bodies`, ...)
   uses it.
2. Type-safety: prevents accidental cross-indexing
   (`LocalId` where `BlockId` is expected).
3. 2-of-4 peers use a typed-vec equivalent (rustc's
   `IndexVec<BasicBlock, BasicBlockData>`,
   Charon's `Vector<BlockId, BlockData>`); 2-of-4 use
   bare storage (rustc_public's `Vec<BasicBlock>` +
   `BasicBlockIdx = usize`, mrustc's
   `std::vector<BasicBlock>` + `typedef unsigned int
   BasicBlockId`). RegseqModel chooses typed for
   stage-1 ID discipline (niche encoding, sigil hook,
   type-safety against cross-ID confusion), not for
   peer-majority alignment.
4. `Vec<BasicBlock>` (à la rustc_public): rejected for
   the same reason D-10-3 rejects bare `usize` indices
   (no type-safety).
5. `Box<[BasicBlock]>`: rejected because it diverges
   from every other crate-level interner table; the Vec
   backing of `IndexVec` carries unused mutability that
   matches `model.types` / `model.consts` / etc.

**Methodology rules invoked.**
- README §2 "Compositional behavior beats special-case
  flags" (use the canonical storage type).

**Prior-art evidence.**
- Stage-1 §2.6 `IndexVec<I, T>` canonical-storage rule.
- rustc `IndexVec<BasicBlock, BasicBlockData>` at
  `rustc_middle/src/mir/basic_blocks.rs:15`.
- Charon `Vector<BlockId, BlockData>` typed at
  `src/ast/ullbc_ast.rs`.

**Cross-refs.**
- `spec-snapshot.md` §24.2.1.
- Parent: D-10-2.

---

### D-10-2.2 -- No wrapper struct [LOCKED]

**Question.** Wrap the storage in a `BasicBlocks` struct
(à la rustc) or expose the bare `IndexVec` directly?

**Decision.** **Bare type, no wrapper.**

**Reasoning chain.**
1. The only reason to wrap is to attach methods. The
   methods rustc attaches are CFG-related (predecessors,
   reverse_postorder, dominators). Per Q1, those belong
   in `regseq-analysis`.
2. A wrapper that exposes nothing beyond the inner
   `IndexVec` is dead weight.
3. 3-of-4 peer alignment: rustc_public, Charon, mrustc
   all use bare storage. rustc's wrapper exists *because
   of* the cache (D-10-2.3); without the cache, the
   wrapper has no content.

**Methodology rules invoked.**
- README §2 "Don't add features beyond what the task
  requires" (no consumer for the wrapper).

**Prior-art evidence.**
- 3-of-4 peer bare-storage consensus.

**Cross-refs.**
- `spec-snapshot.md` §24.2.2.
- Parent: D-10-2.

---

### D-10-2.3 -- No CFG cache [LOCKED]

**Question.** Cache CFG analyses (predecessors / RPO /
dominators) inside `regseq-model`, or recompute on
demand in `regseq-analysis`?

**Decision.** **No cache in `regseq-model`. CFG analyses
live in `regseq-analysis`.**

A principled rule emerged from Phase A discussion and
lands in §24.2.3:

> The model stores facts the bridge owns or rustc has
> resolved (types, layouts, const values, instance
> identities). The model does not store analyses
> derivable from those facts via standard CFG/SSA
> algorithms. The single exception -- RPO ordering of
> `basic_blocks` storage -- is justified by being the
> layout itself, not a separate field; and by being the
> order rustc's `ReorderBasicBlocks` already produces.

**Reasoning chain.**
1. By the time the bridge consumes the body, rustc's
   CFG cache is empty (the cleanup-pass whitelist runs
   `ReorderBasicBlocks`, which calls
   `reverse_postorder()` once, then immediately
   invalidates the cache via `as_mut` at
   `rustc_mir_transform/src/prettify.rs:36`). So
   "preserve what rustc gave us" doesn't apply -- there
   is nothing free to preserve.
2. Populating a `dominators` field would require the
   bridge to *invoke* the computation; the work doesn't
   disappear, only the boundary moves.
3. RPO ordering IS a stored-analysis exception, but it
   costs zero extra bytes (the IndexVec needs *some*
   order; using the RPO one is free).
4. Predecessors / dominators / loop structure / liveness
   all derivable; storing them is shadow data that must
   be kept in sync with the underlying CFG.
5. Serde wire format hygiene: derived analyses on the
   wire muddy the contract that primitive content alone
   round-trips.
6. F4/F4-2 cross-build identity surface: derived
   analyses shouldn't participate in cross-build
   identity, but `pub` fields on `Body` would need
   either canonicalization or carve-out.
7. 3-of-4 peer alignment: rustc_public, Charon, mrustc
   all decline to cache. rustc_middle's `Arc<Cache>` is
   motivated by being inside the compiler (long-lived,
   reused across passes); RegseqModel is consumed once
   per session per body.

**Methodology rules invoked.**
- README §2 "Compositional behavior beats special-case
  flags" (predecessors / dominators are derivable).
- README §2 "C semantics with information preservation"
  (preserve what rustc gave us, not what rustc would
  compute on demand).
- Q1 architectural decision (`architecture.md`).

**Prior-art evidence.**
- rustc `Arc<Cache>` empty-encoded by Encoder at
  `rustc_middle/src/mir/basic_blocks.rs:162-176`
  (cache is *not* part of the persistent IR; it's
  rebuild-on-load).
- 3-of-4 peer no-cache consensus.

**Cross-refs.**
- `spec-snapshot.md` §24.2.3 (cache + principled rule).
- Parent: D-10-2.

---

### D-10-2.4 -- No type alias [LOCKED]

**Question.** Introduce `pub type BasicBlocks = IndexVec<BlockId, BasicBlock>`
as a convenience alias, or use the bare type at field
declaration sites?

**Decision.** **No alias.**

**Reasoning chain.**
1. The field appears in maybe two places: the stage-11
   `Body` struct and any analysis helper signature.
2. A semantic alias adds nothing; it would only matter if
   the alias gained methods (which we explicitly
   rejected at D-10-2.2).
3. If a future stage / phase-2 device-pressure surfaces
   real ergonomic friction, introduce the alias then.

**Methodology rules invoked.**
- README §2 "Don't add features beyond what the task
  requires".

**Prior-art evidence.**
- rustc_public, Charon, mrustc all use bare types.
- rustc's `BasicBlocks<'tcx>` is a wrapper, not an alias
  (motivated by the cache; not applicable here).

**Cross-refs.**
- `spec-snapshot.md` §24.2.4.
- Parent: D-10-2.

---

### D-10-3 -- `BlockId` introduction [LOCKED]

**Question.** How does RegseqModel introduce `BlockId` as
a body-local typed-index, and what auxiliary work
(macro-side, lib.rs assertions, sigil convention) lands
with it?

**Key options.**
1. **rustc_public-style bare alias** (`pub type BasicBlockIdx = usize`).
2. **Stage-1-pattern typed-index** with named const
   declared via a separate `impl` block.
3. **Stage-1-pattern typed-index** with named const
   declared inside the macro via a new const-body arm
   (matching rustc's `pub struct BasicBlock { const
   START_BLOCK = 0; }` grammar at
   `rustc_middle/src/mir/mod.rs:1289-1291`).

**Decision.** Option 3 -- **stage-1-pattern typed-index
with const-body macro arm**.

**Reasoning chain.**
1. Bare `usize` rejected on stage-1 ID-discipline
   grounds: no niche, no sigil hook, no type-safety
   against `LocalId` confusion.
2. Const-body arm vs separate `impl` block: the inbound
   expectation in `expectations.md` "From stage 1"
   explicitly asked for the const-body arm; rustc uses
   exactly this grammar; semantic locality of named
   consts at the macro site is the discipline payoff.
3. `LocalId::RETURN_PLACE` migration to the new arm is
   deferred to stage 11 (LocalId formally lands at
   stage 11 alongside `Body.local_decls`).
4. Sigil convention `bbN` is universal across all four
   peers; the *rendering wiring* (where `bbN` appears in
   `pretty.rs` output) is renderer-injected at the
   BasicBlock-rendering site, deferred to stage 11
   under D-10-K.

**Methodology rules invoked.**
- README §2 "Re-derive from evidence; do not import
  baggage" (rustc grammar match is consultative; the
  macro extension's payoff is internal-discipline
  alignment).
- README §2 "Compositional behavior beats special-case
  flags" (named consts inside the macro vs separate
  `impl`-block side-channel).

**Prior-art evidence.**
- rustc `pub struct BasicBlock { const START_BLOCK = 0; }`
  at `rustc_middle/src/mir/mod.rs:1289-1291`;
  `#[debug_format = "bb{}"]` at `mod.rs:1288`.
- rustc_public `BasicBlockIdx = usize` bare alias at
  `rustc_public/src/mir/body.rs:40` (peer divergence;
  declined).
- Charon `generate_index_type!` macro at
  `src/ids/mod.rs:14-47` with `pub const ZERO` inside
  the impl + a separate `pub static START_BLOCK_ID:
  BlockId = BlockId::ZERO` at `src/ast/ullbc_ast.rs:11-12`
  (looser-locality alternative; declined).
- mrustc `typedef unsigned int BasicBlockId` at
  `src/mir/mir.hpp:24` (weakest discipline; declined).
- Existing stage-1 `newtype_index!` macro at
  `crates/regseq-model/src/idx.rs` (auto-generates
  `pub const ZERO`, `Debug` rendering as `TypeName(N)`).
- Existing crate-level-ID size assertions at
  `crates/regseq-model/src/lib.rs:40-57`.

**Cross-refs.**
- `spec-snapshot.md` §24.3 (BlockId typed-index +
  sub-sections).
- Sub-decisions: D-10-3.1 (newtype_index! application),
  D-10-3.2 (const-body arm + LocalId migration timing),
  D-10-3.3 (size/niche assertions), D-10-3.4 (sigil
  convention).
- Closes 4 inbound expectations from `expectations.md`
  "From stage 1".

---

### D-10-3.1 -- `BlockId` via `newtype_index!` [LOCKED]

**Question.** How does `BlockId` get declared in
`crates/regseq-model/src/idx.rs`?

**Decision.** Apply `newtype_index!` per stage-1 ID
conventions:
```rust
newtype_index! {
    pub struct BlockId {
        const START_BLOCK = 0;
    }
}
```
(Form requires the const-body arm extension landed under
D-10-3.2.)

**Reasoning chain.**
1. Mechanical inheritance from stage-1's typed-ID
   conventions (u32-backed, niche-encoded via
   `NonMaxU32`, derives `Idx` / `Ord` / `Debug`).
2. Closes the first inbound expectation ("Define
   `BlockId` via `newtype_index!`").

**Methodology rules invoked.** None new; pure stage-1
inheritance.

**Prior-art evidence.**
- Stage-1 `newtype_index!` macro shape at
  `crates/regseq-model/src/idx.rs:75-160`.
- Stage-1's existing crate-level IDs as form precedent.

**Cross-refs.**
- `spec-snapshot.md` §24.3.1.
- Parent: D-10-3.

---

### D-10-3.2 -- `newtype_index!` const-body arm [LOCKED]

**Question.** Should the macro be extended with a
const-body arm, and what grammar does that arm accept?

**Decision.** **Extend the macro with a const-body arm**
matching rustc's grammar:
```
pub struct $Name {
    const NAME_1 = INT_LITERAL_1;
    const NAME_2 = INT_LITERAL_2;
    ...
}
```
where each `const` line generates `pub const NAME_K: $Name
= $Name::from_u32(INT_LITERAL_K);` inside the impl. The
existing empty-body arm continues to work unchanged.

`LocalId::RETURN_PLACE` migration to this arm is deferred
to stage 11 (where LocalId formally lands).

**Reasoning chain.**
1. Inbound expectation explicitly asked for this
   extension.
2. Matches rustc's grammar at
   `rustc_middle/src/mir/mod.rs:1289-1291`.
3. Multi-const line support (rather than single-const)
   accommodates future IDs that may declare multiple
   named consts inline; cost is negligible.
4. Integer-literal-only (rather than general const
   expression): per README "Don't add features beyond
   what the task requires"; the use cases (RETURN_PLACE
   = 0, START_BLOCK = 0) only need integer literals.
5. LocalId migration: deferred -- LocalId is stage-11
   territory (`Body.local_decls` is stage-11 content);
   migrating in advance breaks the "spec before
   implementation" discipline. The macro arm itself
   lands when stage 10 ships at impl-sweep time; LocalId
   migration ships at stage 11's portion of the same
   sweep.

**Methodology rules invoked.**
- README §2 "Re-derive from evidence; do not import
  baggage" (rustc grammar match is consultative).
- README §2 "Don't add features beyond what the task
  requires" (integer-literal-only, not general const
  expressions).
- README §2 "Spec before implementation" (LocalId
  migration deferred to stage 11).

**Prior-art evidence.**
- rustc grammar at
  `rustc_middle/src/mir/mod.rs:1289-1291`.
- Stage-1 macro's current empty-body-only signature at
  `crates/regseq-model/src/idx.rs:75-78`.

**Cross-refs.**
- `spec-snapshot.md` §24.3.2.
- Parent: D-10-3.
- Outbound bundle: D-10-K item 5 (LocalId::RETURN_PLACE
  migration to stage 11).

---

### D-10-3.3 -- Size / niche assertions [LOCKED]

**Question.** Where do the `size_of::<BlockId>() == 4` and
`size_of::<Option<BlockId>>() == 4` static assertions
land?

**Decision.** Append both to
`crates/regseq-model/src/lib.rs`, matching the existing
crate-level-ID pattern at lines 40-57:
```rust
const _: () = assert!(core::mem::size_of::<BlockId>() == 4);
const _: () = assert!(core::mem::size_of::<Option<BlockId>>() == 4);
```

**Reasoning chain.**
1. Direct stage-1 pattern inheritance.
2. The `Option<BlockId> == 4` assertion is non-vacuous --
   it catches a regression where the niche encoding
   silently breaks.
3. Closes the inbound expectation "When stage 10 adds
   `BlockId`, append the corresponding `Option<BlockId>`
   and `BlockId` 4-byte size assertions".

**Methodology rules invoked.** None new; pattern
inheritance.

**Prior-art evidence.**
- Existing crate-level-ID size assertions at
  `crates/regseq-model/src/lib.rs:40-57`.

**Cross-refs.**
- `spec-snapshot.md` §24.3.3.
- Parent: D-10-3.

---

### D-10-3.4 -- Sigil textual convention [LOCKED]

**Question.** What textual sigil represents `BlockId(N)`,
and where does the rendering live?

**Decision.** **Convention: `BlockId(N)` -> `bbN`** (e.g.,
`BlockId(0)` -> `bb0`, `BlockId(3)` -> `bb3`). The
rendering site is renderer-injected at the
BasicBlock-rendering envelope inside `pretty.rs`; the
*helper that emits the textual form* is introduced at
stage 11 alongside its first consumer (D-10-K.2).

The macro's auto-generated `Debug` impl is unchanged:
renders `BlockId(N)`. The `bbN` sigil belongs to
RegseqModel's *custom textual format* (Q5 primary), not
to Rust's `Debug`.

**Reasoning chain.**
1. Universal peer alignment: rustc's
   `#[debug_format = "bb{}"]`, rustc_public's
   manual `bb{index}` rendering at
   `rustc_public/src/mir/pretty.rs:38-88`, Charon's
   `bb{}` block-label render at
   `src/pretty/fmt_with_ctx.rs:748` (within
   `:736-756`; `:206-214` covers block-content
   rendering, not the label), mrustc's `bbN`
   envelope at `src/mir/dump.cpp:26-48`.
2. Auto-generated `Debug` rendering as `BlockId(N)`
   stays uniform with all other typed IDs (stage-1
   convention); diverges from the textual format
   intentionally (textual format is the renderer's
   target, not Rust's `Debug`).
3. Renderer-injected (rather than macro-generated): the
   sigil-rendering helper is consumed only by
   stage-11-deferred renderers (D-10-K.1's
   `render_basic_block`, D-9-K's `render_terminator`'s
   Goto / SwitchInt targets). No stage-10-or-earlier
   consumer exists; introducing the helper now would be
   a feature without a consumer.
4. Closes the inbound expectation "Add `BlockId`-rendering
   to `pretty.rs`. The spec sigil is `bbN`".

**Methodology rules invoked.**
- README §2 "Don't add features beyond what the task
  requires" (helper deferred to first-consumer site).

**Prior-art evidence.**
- All four peers use the `bbN` textual convention.
- rustc_public's renderer-injected pattern at
  `rustc_public/src/mir/pretty.rs:38-88` -- direct precedent.

**Cross-refs.**
- `spec-snapshot.md` §24.3.4.
- Parent: D-10-3.
- Cross-ref: D-10-K.2 (rendering wiring deferral).

---

### D-10-4 -- Block-ordering invariants [LOCKED]

**Question.** What block-ordering invariants does
RegseqModel encode, and how are they enforced?

**Decision.** Two positive invariants:
- **B-RPO** -- `Body.basic_blocks: IndexVec<BlockId,
  BasicBlock>` is stored in reverse-postorder of the CFG
  rooted at `BlockId::START_BLOCK`. Inherited from
  rustc's `ReorderBasicBlocks` cleanup pass,
  force-enabled via `regseq_mir_passes()` at
  `crates/regseq-driver/src/callbacks.rs:30`.
- **B-Entry** -- `BlockId::START_BLOCK == BlockId(0)` is
  the entry block of every `Body`. Inherited from
  rustc's `START_BLOCK = 0` const at
  `rustc_middle/src/mir/mod.rs:1290` and preserved by
  `ReorderBasicBlocks` via the explicit fixed-point at
  `rustc_mir_transform/src/prettify.rs:33`.

Walk-implementation deferred to stage 11. No defensive
fallback for non-RPO inputs (per "no defensive fallbacks"
rule + pinned-toolchain contract).

**Reasoning chain.**
1. The cleanup-pass whitelist at
   `crates/regseq-driver/src/callbacks.rs:25-36` includes
   `ReorderBasicBlocks` set to `true`. After this pass,
   the IndexVec storage IS in RPO; iterating
   `iter_enumerated()` yields blocks in RPO order.
2. `START_BLOCK` is preserved as `BlockId(0)` even after
   the reorder permutation (debug_assert at
   `prettify.rs:33`). The entry-block identity is
   invariant under any pipeline-conformant transform.
3. RPO storage is a graph-structural property encoded
   *as the layout itself* -- zero extra bytes (the
   IndexVec needs *some* order; RPO is the natural one).
   This is the single carve-out to the "model does not
   store derivable analyses" rule (D-10-2.3).
4. Walk-implementation deferred to stage 11 mirrors the
   established pattern across stages 5-9 (D-5-2 Part C,
   D-7-3 Part C, D-8-4 Part C, D-9-10 Part C).
5. No back-edge carve-out in the invariant text: RPO is
   a graph property; whether a given edge is forward
   (`j > i`) or back (`j <= i`) falls out of the topology.
   The spec text shouldn't enumerate back-edges.
6. No defensive fallback: per pinned-toolchain
   methodology, the cleanup whitelist is part of the
   contract.

**Methodology rules invoked.**
- README §2 "C semantics with information preservation"
  (RPO is rustc-resolved free).
- README §2 "Compositional behavior beats special-case
  flags" (RPO encoded as layout; B-Entry encoded as the
  `START_BLOCK` const value).
- README §2 "No defensive fallbacks" (no non-RPO carve-out).

**Prior-art evidence.**
- `ReorderBasicBlocks` pass at
  `rustc_mir_transform/src/prettify.rs:14-42`;
  `is_required: false` at line 39 but force-on via our
  whitelist.
- `regseq_mir_passes()` at
  `crates/regseq-driver/src/callbacks.rs:25-36` (sets
  `ReorderBasicBlocks` to true).
- `prettify.rs:33` `debug_assert_eq!(updater.map[START_BLOCK],
  START_BLOCK)` -- the START_BLOCK fixed-point.
- rustc `START_BLOCK = 0` const at
  `rustc_middle/src/mir/mod.rs:1290`.
- Charon translator at
  `translate_bodies.rs:1230-1265` allocates BlockIds in
  DFS discovery order via worklist (counter-example;
  direct precedent for "RPO is not free everywhere").

**Cross-refs.**
- `spec-snapshot.md` §24.4 (block-ordering invariants).
- Sub-decisions: D-10-4.1 (B-RPO), D-10-4.2 (B-Entry),
  D-10-4.3 (walk deferral to stage 11), D-10-4.4 (no
  defensive fallback).
- Cross-ref: D-10-5 Part B (B-RPO and B-Entry land in
  the per-stage invariants registry).

---

### D-10-4.1 -- B-RPO storage order [LOCKED]

**Question.** How is the RPO storage-order invariant
phrased?

**Decision.** Single-sentence positive invariant:
> `Body.basic_blocks: IndexVec<BlockId, BasicBlock>` is
> stored in reverse-postorder of the CFG rooted at
> `BlockId::START_BLOCK`. Inherited from
> `ReorderBasicBlocks` in the cleanup whitelist.
> Iterating the storage yields blocks in RPO; consumers
> may exploit this without re-deriving.

No back-edge carve-out enumerated.

**Reasoning chain.** See D-10-4 reasoning chain steps
1, 3, 5.

**Methodology rules invoked.** See D-10-4.

**Prior-art evidence.** See D-10-4.

**Cross-refs.**
- `spec-snapshot.md` §24.4.1.
- Parent: D-10-4.

---

### D-10-4.2 -- B-Entry `START_BLOCK = BlockId(0)` [LOCKED]

**Question.** How is the entry-block identity invariant
phrased?

**Decision.** Single-sentence positive invariant:
> `BlockId::START_BLOCK == BlockId(0)` is the entry block
> of every `Body`. Inherited from rustc's `START_BLOCK = 0`
> const + preserved by `ReorderBasicBlocks` via the
> explicit fixed-point at `prettify.rs:33`.

The named const introduced under D-10-3.2 carries the
semantic meaning: `BlockId::START_BLOCK` is "the entry
block by IR convention," not just "the const value 0".

**Reasoning chain.** See D-10-4 reasoning chain step 2.

**Methodology rules invoked.** See D-10-4.

**Prior-art evidence.** See D-10-4.

**Cross-refs.**
- `spec-snapshot.md` §24.4.2.
- Parent: D-10-4.

---

### D-10-4.3 -- Walk-impl deferred to stage 11 [LOCKED]

**Question.** Where does the walk-implementation that
enforces B-RPO, B-Reachable, and B-Entry land?

**Decision.** Stage 11 owns the body-walk validator that
checks B-RPO + B-Reachable + B-Entry **jointly via a
single DFS-RPO procedure** alongside the other deferred
walks. Outbound expectation filed at Phase B close
(item 2 of the 5-item outbound bundle).

**Reasoning chain.**
1. Body-level walks belong at stage 11 (Body owns the
   basic-block table; walks live with the owner).
2. Mirrors D-5-2 Part C / D-7-3 Part C / D-8-4 Part C /
   D-9-10 Part C walk-impl deferrals.
3. The joint walk procedure (§24.4.3) runs in fixed
   step order: **steps 1-2 pre-clear B-NonEmpty and
   B-Successors-Valid** (so the DFS cannot indexing-
   panic on an out-of-range successor); **steps 3-6
   perform the DFS-RPO + B-Reachable + B-RPO + B-Entry
   joint check** -- step 3 DFS from `BlockId(0)`
   using `Terminator.successors()` and post-order +
   reverse to obtain RPO; step 4 length comparison
   enforces B-Reachable; step 5 positional comparison
   enforces B-RPO; step 6 confirms B-Entry from the
   step-3 starting point combined with B-NonEmpty.
   The DFS-RPO core mirrors rustc's
   `BasicBlocks::reverse_postorder()` at
   `rustc_middle/src/mir/basic_blocks.rs:72-78`; the
   pre-check ordering avoids the indexing-panic
   pattern observed in rustc's traversal at
   `rustc_middle/src/mir/traversal.rs:124-134`.
4. Vacuity-of-original-walk fix (added at Round 1
   closure per Codex issue 1): the original
   walk-impl text in §24.4.3 said "every successor j
   is either back-edge (`j <= i`) or `j > i`",
   which is vacuously true. The joint DFS-RPO
   procedure replaces it with a real enforcement.
5. Walk-ordering refinement (added at Round 2 closure
   per Codex issue 4): pre-checks for B-NonEmpty
   (step 1) and B-Successors-Valid (step 2) precede
   the DFS-RPO core, so an out-of-range successor
   surfaces as a clean B-Successors-Valid diagnostic
   rather than as an `IndexVec`-indexing panic.

**Methodology rules invoked.** None new; established
deferral pattern + the §24.4.3 enforcement procedure
from Round 1 closure.

**Prior-art evidence.**
- Stage-5 / stage-7 / stage-8 / stage-9 walk-impl
  deferral precedents.
- rustc's `BasicBlocks::reverse_postorder()` at
  `rustc_middle/src/mir/basic_blocks.rs:72-78` (the
  reference RPO computation pattern).
- rustc's `ReorderBasicBlocks` joint
  RPO-and-bijection invariants at
  `rustc_mir_transform/src/prettify.rs:25-37`.

**Cross-refs.**
- `spec-snapshot.md` §24.4.3 (joint DFS-RPO procedure
  detailed).
- Parent: D-10-4.
- `stage-11/expectations.md` "From stage 10" item 2 at
  Phase B close.

---

### D-10-4.4 -- No defensive fallback for non-RPO inputs [LOCKED]

**Question.** Should the spec carve out a "what if rustc
gives us non-RPO blocks" path?

**Decision.** **No defensive fallback.**

**Reasoning chain.**
1. The cleanup-pass whitelist is part of the
   pinned-toolchain contract (per `architecture.md` Q6
   and `crates/regseq-driver/src/callbacks.rs:25-36`).
2. Mis-configuration is a setup bug, not a runtime
   concern.
3. Stage-11's walk validator can raise a diagnostic if
   it observes RPO violation post-construction, but
   stage-10 does not carve out the case in the spec
   surface.

**Methodology rules invoked.**
- README §2 "No defensive fallbacks" (toolchain pinned;
  don't carry handlers for "what if X is missing").

**Prior-art evidence.** None directly applicable; this is
a stage-10 application of the README rule.

**Cross-refs.**
- `spec-snapshot.md` §24.4.4.
- Parent: D-10-4.

---

### D-10-5 -- Per-stage invariants registry [LOCKED]

**Question.** How are stage-10's invariants organized in
the spec, mirroring the stage-8 D-8-4 / stage-9 D-9-10
patterns?

**Decision.** **5-part registry** in §24.6:

- **Part A**: 3-row rejection table (2 defensive
  rejections + 1 silent-drop completeness entry).
  Bridge-enforced at construction (per §24.7); not
  stage-11-walk-enforced (RegseqModel's `BasicBlock`
  shape makes Part-A conditions structurally
  inexpressible).
- **Part B**: 8 positive invariants (2 inherited + 6
  stage-10-introduced).
- **Part C**: walk-implementation deferred to stage 11
  (Part B walk-checked invariants only; Part A is
  bridge-enforced).
- **Part D**: F4 / F4-2 cross-build identity OUT of
  scope.
- **Part E**: forward-looking considerations (no
  stage-10 non-invariants; per-instance const-folding
  noted as a future-stage concern).

**Part A (rejection table) -- 3 rows:**

| # | rustc construct | Category | Discharge | Diagnostic |
|---|---|---|---|---|
| 1 | `BasicBlockData.is_cleanup = true` | Pipeline-discharged (panic=abort) | Stage-9 D-9-9 rows 5-6: `AbortUnwindingCalls` + cleanup-block pruning | "is_cleanup=true block survived panic=abort cleanup-block pruning" |
| 2 | `BasicBlockData.terminator = None` post-construction | Defensive (rustc invariant) | rustc's docstring; `terminator()` accessor panics | "terminator: None survived MIR construction" |
| 3 | `BasicBlockData.after_last_stmt_debuginfos` non-empty | Silent drop (NOT rejected) | Stage-8 D-8-3 debuginfo discharge | n/a -- silent drop |

**Part B (positive invariants) -- 8:**

- **B-0a** (inherited) -- All embedded Statement content
  satisfies stage-8 §22 invariants.
- **B-0b** (inherited) -- All embedded Terminator content
  satisfies stage-9 §23 invariants.
- **B-RPO** -- Storage order (D-10-4.1).
- **B-Reachable** -- All blocks reachable from
  `BlockId::START_BLOCK` via `Terminator.successors()`.
  **Verified by the stage-11 DFS-RPO walk procedure**
  (§24.4.3 step 4: a DFS from `BlockId(0)` that yields
  fewer blocks than `body.basic_blocks.len()` flags a
  B-Reachable violation). The spec asserts B-Reachable
  at the bridge boundary; rustc's standard pipeline
  produces reachable-only bodies in practice (the
  scheduling at `rustc_mir_transform/src/lib.rs:666-680`
  includes `SimplifyCfg::PreOptimizations` and
  `remove_dead_blocks` at
  `rustc_mir_transform/src/simplify.rs:79-85,349-383`
  which prune unreachable blocks), but the spec does
  not lean on this pipeline scheduling for soundness
  -- the stage-11 walk is the authoritative check.
  Added at Round-1 closure to resolve the
  RPO-vs-unreachable inconsistency Codex flagged
  (Round 1 issue 2); evidence re-grounded at Round-2
  closure (Round 2 issue 1) after the original
  `invert_bijective_mapping` panic claim was
  identified as rustc-source-inaccurate
  (`rustc_index::slice::invert_bijective_mapping`
  uses `debug_assert_eq!` over `self.len()`, not
  against `body.basic_blocks.len()`; and
  `prettify.rs:28-30` early-returns before inversion
  if `rpo` is already sorted).
- **B-Entry** -- `START_BLOCK = BlockId(0)` (D-10-4.2).
- **B-Terminator-Some** -- Every `BasicBlock.terminator`
  is well-formed (no `Option::None`); D-10-1.3 type-level
  guarantee.
- **B-Successors-Valid** -- Every `BlockId` referenced as
  a successor in any Terminator is `< body.basic_blocks.len()`.
  Body-level invariant; walk-checked at stage 11.
- **B-NonEmpty** -- Every body has at least one `BasicBlock`
  (the entry block). Walk-checked at stage 11.

**Part C (walk-impl deferred to stage 11):** Mirrors
D-5-2 / D-7-3 / D-8-4 / D-9-10 walk-impl deferrals. The
body-walk visits each basic block, checking the
**Part-B invariants only**: B-RPO + B-Reachable +
B-Entry jointly via the §24.4.3 DFS-RPO procedure;
B-Successors-Valid as per-Terminator range checks;
B-NonEmpty as a pre-check. Part A is bridge-enforced
at construction (per §24.7); RegseqModel's `BasicBlock`
shape makes Part-A conditions structurally
inexpressible post-construction, so the walk inherits
the post-construction guarantee without re-checking.

**Part D (F4 / F4-2 OUT):** `BasicBlock` is body-local
content; doesn't participate in cross-build identity.

**Part E (forward-looking considerations):** No
stage-10 non-invariants. Per-instance const-folding of
type-dependent branches (analogous to rustc's codegen-
side `mono_reachable_reverse_postorder`) is NOT
applied at the bridge; if a future stage adds
bridge-side per-instance const-folding, that stage
owns the policy. Not a stage-10 spec weakening:
B-Reachable holds at the bridge output as established
in Part B.

**Reasoning chain.**
1. Mirrors the established stage-8 / stage-9 5-part
   registry pattern.
2. Each rejection-table entry has a specific discharge
   (cited in the table).
3. Each positive invariant is either inherited (B-0a /
   B-0b), structural (B-Entry / B-Terminator-Some), or
   walk-checked at stage 11 (B-RPO / B-Reachable /
   B-Successors-Valid / B-NonEmpty).
4. B-Successors-Valid placement at stage 10: BlockId
   range only makes sense once a body exists (body-level
   invariant), but we co-locate basic-block-related
   invariants at stage 10 even though walks run at
   stage 11. Mirrors stage-8 D-8-4 (statements-as-vec
   invariants live at stage 8 even though walks run at
   stage 11).
5. Walk-ownership split (clarified at Round 1 closure
   per Codex issue 3): Part A rows 1-2 are
   bridge-enforced at construction (rustc-side fields
   gone post-construction); the stage-11 walk
   enforces only Part B invariants. RegseqModel's
   `BasicBlock` shape makes Part-A conditions
   structurally inexpressible post-construction, so
   the walk inherits the guarantee without
   re-checking.
6. B-Reachable promotion (added at Round-1 closure
   per Codex issue 2; evidence re-grounded at Round-2
   closure per Codex issue 1): reachability was
   originally filed as a non-invariant in Part E.
   Round-1 review identified the inconsistency between
   B-RPO's rooted-traversal semantics and the
   "reachability not enforced" framing; the
   resolution moved reachability into Part B as
   B-Reachable. Round-1's initial evidence claimed
   `ReorderBasicBlocks::invert_bijective_mapping`
   panics on non-bijection (so unreachable blocks
   cannot exist when the pass runs), but Round-2
   review showed this is rustc-source-inaccurate
   (`rustc_index::slice::invert_bijective_mapping`
   uses `debug_assert_eq!` over `self.len()`, not
   `body.basic_blocks.len()`; also
   `prettify.rs:28-30` early-returns before inversion
   if `rpo` is already sorted). The corrected
   evidence: B-Reachable is verified by the stage-11
   DFS-RPO walk procedure independently of
   rustc-pipeline assumptions; rustc's standard
   pipeline (`SimplifyCfg::PreOptimizations` +
   `remove_dead_blocks` at
   `rustc_mir_transform/src/lib.rs:666-680`,
   `rustc_mir_transform/src/simplify.rs:79-85,349-383`)
   produces reachable-only bodies in practice but
   the spec does not lean on this for soundness.
   Per the README "no defensive fallbacks" rule: the
   walk verifies; we do not duplicate rustc-pipeline
   assumptions in spec text.

**Methodology rules invoked.**
- README §2 "Compositional behavior beats special-case
  flags" (each rejection entry has a structural
  discharge, not a flag).
- README §2 "No defensive fallbacks" (Part-A
  bridge-enforcement reflects pinned-toolchain
  contract; the stage-11 walk doesn't re-check
  bridge-side invariants).

**Prior-art evidence.**
- Stage-8 D-8-4 5-part registry pattern.
- Stage-9 D-9-10 4-part-plus-E registry pattern.
- rustc's standard cleanup-pipeline scheduling:
  `SimplifyCfg::PreOptimizations` and
  `remove_dead_blocks` at
  `rustc_mir_transform/src/lib.rs:666-680` and
  `rustc_mir_transform/src/simplify.rs:79-85,349-383`
  prune blocks unreachable from `BlockId(0)`. Note:
  `SimplifyCfg::is_required()` returns `false` at
  `rustc_mir_transform/src/simplify.rs:91-103`, so
  the pass is scheduled-but-not-required; the spec
  treats rustc's scheduling as informative, not
  load-bearing (see Round-2 issue 1 closure for the
  re-grounded evidence).
- rustc's `Postorder::new(.., START_BLOCK, ..)` at
  `rustc_middle/src/mir/traversal.rs:106-119`: the
  reverse-postorder traversal is rooted at
  `BlockId(0)`, so its result enumerates only
  reachable blocks. The stage-11 walk's "RPO list
  length equals `body.basic_blocks.len()`" check
  (§24.4.3 step 4) is the verification that all
  blocks are reachable.
- rustc's traversal indexing pattern at
  `rustc_middle/src/mir/traversal.rs:124-134`:
  visits `self.basic_blocks[bb]` during DFS; an
  invalid successor would panic. The §24.4.3 walk
  pre-checks B-Successors-Valid (step 2) before the
  DFS to surface a clean B-Successors-Valid
  diagnostic rather than an indexing panic
  (Round-2 issue 4 closure).
- mrustc's `MIR_Validate` per-block rules at
  `src/mir/check.cpp:636-737` (informational; mrustc
  enforces CFA structural reachability, matching our
  B-Reachable).
- Charon's explicit `filter_unreachable_blocks` pass
  (counter-precedent for "reachability handled at
  construction"; Charon allocates BlockIds in DFS
  discovery order then filters later, while
  RegseqModel inherits reachability via rustc's
  scheduling and re-verifies via the stage-11 walk).

**Cross-refs.**
- `spec-snapshot.md` §24.6 (invariants registry; Part A
  -> §24.6.1, Part B -> §24.6.2, Part C -> §24.6.3,
  Part D -> §24.6.4, Part E -> §24.6.5).
- Aggregator over: D-10-1 (field-drops -> rejection
  rows), D-10-3 (BlockId), D-10-4 (B-RPO + B-Entry +
  B-Reachable joint walk-impl).

---

### D-10-K -- Pretty-printer renderer deferral [LOCKED]

**Question.** Where do `render_basic_block` and the
`BlockId` sigil-rendering helper land?

**Decision.** **Both deferred to stage 11.**
- D-10-K.1: `render_basic_block` materializes at stage 11
  alongside `render_place` (D-5-K), `render_operand`
  (D-6-K), `render_rvalue` (D-7-K), `render_statement`
  (D-8-K), and `render_terminator` (D-9-K). Closes the
  D-5-K through D-9-K cascade.
- D-10-K.2: `BlockId` sigil-rendering helper introduced
  at stage 11 alongside its first consumer (whichever of
  the three deferred consumers lands first in stage-11
  implementation order: `render_basic_block` envelope
  label, `render_terminator`'s Goto target,
  `render_terminator`'s SwitchInt targets).
- D-10-K.3: §18.2 per-method ownership table gets one
  new row at Phase B merge: `render_basic_block` ->
  "Deferred to stage 11 (D-10-K.1)". The sigil helper is
  a stage-11 internal helper, not public-surface; no row.

**Reasoning chain.**
1. Three forced-deferral drivers stack:
   (a) the body of the envelope embeds `render_statement`
   (D-8-K, deferred); (b) the terminator line embeds
   `render_terminator` (D-9-K, deferred); (c) the block
   label `bbN:` embeds the BlockId sigil-rendering
   helper. None of the three is materializable before
   stage 11.
2. The sigil-rendering helper has no consumer at
   stage-10 or earlier; introducing it now would be a
   feature without a consumer. Per README "Don't add
   features beyond what the task requires".
3. §18.2 row update mirrors stages 5-9-K precedent.

**Methodology rules invoked.**
- README §2 "Don't add features beyond what the task
  requires".
- Established D-5-K through D-9-K deferral pattern.

**Prior-art evidence.**
- D-5-K through D-9-K deferral cascade (each per-stage
  spec).
- rustc_public's `function_body` renderer at
  `rustc_public/src/mir/pretty.rs:38-88` writing
  `bb{index}: {` manually -- direct precedent for
  renderer-injected sigils.

**Cross-refs.**
- `spec-snapshot.md` §24.9 (pretty-printer deferral;
  §24.7 is bridge translation rules, §24.8 is the
  worked example).
- `spec-snapshot.md` §18.2 (per-method ownership table
  row added at Phase B merge).
- Sub-decisions: D-10-K.1 (`render_basic_block`
  deferral), D-10-K.2 (sigil helper deferral), D-10-K.3
  (§18.2 row).

---

## 2. Outbound expectations to file at Phase B close

To file in `stage-11/expectations.md` "From stage 10"
once Phase B's per-stage Codex review APPROVES:

1. **`render_basic_block` deferral** (D-10-K.1) --
   stage 11 owns the basic-block renderer.
2. **Body walk implementation** (D-10-5 Part C) -- one
   pass over `body.basic_blocks.iter_enumerated()`
   plus a DFS-RPO traversal from `BlockId(0)` per
   §24.4.3, checking the RegseqModel-visible Part B
   positive invariants in fixed step order:
   B-NonEmpty pre-check (step 1); B-Successors-Valid
   pre-check (step 2; guards subsequent indexing);
   B-RPO + B-Reachable + B-Entry jointly via the
   DFS-RPO procedure (steps 3-6). Part A is
   bridge-enforced at construction (per §24.7); the
   walk does not re-check it. Mirrors stage-8 / stage-9
   walk-impl deferrals.
3. **`BlockId` sigil-rendering helper** (D-10-K.2) --
   introduce alongside first consumer.
4. **Body-level basic-block table integration**
   (D-10-2) -- `Body.basic_blocks: IndexVec<BlockId,
   BasicBlock>` lands as a public field of stage-11's
   `Body`.
5. **`LocalId::RETURN_PLACE` migration** (D-10-3.2
   deferral) -- when stage 11 introduces `LocalId` as a
   typed-index, migrate its `RETURN_PLACE` const to use
   the new `newtype_index!` const-body arm.

---

## 3. Stage-8 / Stage-9 spec-side touch-ups at top-level merge

Per README §5.9 "later stage refines earlier stage's
content" rule, stage-10's commit triggers two in-place
updates at top-level merge:

1. **§22 (Statement vocabulary)** -- forward references
   to `BasicBlock.statements: Vec<Statement>` update to
   `Box<[Statement]>` per D-10-1.2 lock.
2. **§23.5 (Terminator identity and storage)** -- the
   "Stage-10 vocabulary preview" block (`pub struct
   BasicBlock { pub statements: Vec<Statement>, ... }`)
   becomes a forward-reference to §24 with the locked
   `Box<[Statement]>` form.

These are stage-10-side spec-snapshot edits at Phase B
step 4; the merger lifts them to top-level `spec.md` at
hand-off.
