# Stage 10 -- Prior Art: rustc primary source

Primary-source rustc evidence for `mir::BasicBlock` /
`mir::BasicBlockData<'tcx>` / `mir::BasicBlocks<'tcx>` and the
surrounding pipeline (the `ReorderBasicBlocks` cleanup pass that
fixes block-index ordering, the `AbortUnwindingCalls` /
`SimplifyCfg::PreOptimizations` interaction that prunes cleanup
blocks under `panic = "abort"`, the MIR validator's per-block
invariants, the visitor and pretty-printer for basic blocks, and
the two codegen consumers' per-body walks), scoped to what stage
10's candidate decision namespace needs:

- `D-10-1` Per-block `BasicBlock` struct shape (cardinality of the
  rustc 4-field surface; per-field choices: keep `statements`, drop
  `after_last_stmt_debuginfos`, collapse `Option<Terminator>` to
  non-Optional `Terminator`, drop `is_cleanup`).
- `D-10-2` Body-level basic-block container shape (rustc's
  `BasicBlocks<'tcx>` wrapper with `IndexVec<BasicBlock,
  BasicBlockData> + Arc<Cache>` for predecessors / RPO /
  dominators; cache empty-encoded; `as_mut_preserves_cfg` vs
  `as_mut` cache-invalidation discipline).
- `D-10-3` `BlockId` introduction (newtype_index! macro
  application; const-body grammar `pub struct BasicBlock { const
  START_BLOCK = 0; }`; `#[debug_format = "bb{}"]`).
- `D-10-4` Block-ordering invariant (the `ReorderBasicBlocks` pass
  rearranges the IndexVec in reverse postorder; `is_required:
  false` / `is_enabled: false` so only runs under explicit
  `-Zmir-enable-passes=+ReorderBasicBlocks`; preserves
  `START_BLOCK` at index 0; cross-references the
  `BasicBlocks::reverse_postorder()` cache).
- `D-10-5` Invariants registry (the `terminator: Option`
  only-during-construction docstring invariant; the `is_cleanup`
  inverted-tree restrictions; the `panic = "abort"` discharge path
  via `AbortUnwindingCalls` -> `simplify::remove_dead_blocks`; the
  `is_empty_unreachable()` helper; the MIR validator's per-block
  edge / cleanup-control-flow / cleanup-Return / start-block-no-
  predecessors checks).
- `D-10-K` Pretty-printer deferral analog (rustc's
  `mir/pretty.rs::write_basic_block` body-iteration shape; the
  `bb{}` debug_format that yields `bb0`, `bb1`, ...).

Out of scope here: rustc_public's flattening of basic-block shape
(separate lens, in `./rustc_public.md`); the stage-9 closure of
`TerminatorKind` (covered in `../../stage-9/prior-art/rustc.md`,
referenced only where load-bearing for D-10-5); the stage-8
closure of `StatementKind` (similar, `../../stage-8/prior-art/`);
the stage-1 `LocalId` / `BlockId`-skeleton choices already in
`Body`-level locals (covered in `../../stage-1/`).

All paths below are relative to the pinned-toolchain root:

```
~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/
```

(See `../../rustc-references.md` for the canonical statement of
this root path.)

Every factual claim cites a `[file:line]` in that pinned source.
Hypotheses are flagged HYPOTHESIS:. Inferences combining two
sources into a third are flagged INFERENCE:. The Regseq compiler
consumes `optimized_mir` outputs, so "what reaches the bridge"
means: the body returned by `optimized_mir` after
`run_analysis_to_runtime_passes` and `run_optimization_passes`
have completed.

Pass-manager semantics (`is_required(): bool` vs
`is_enabled(&Session): bool`) are used in the same sense
established by stage-8 [`../../stage-8/prior-art/rustc.md` §3] and
re-used by stage 9.

---

## 1. The `BasicBlock` newtype index (D-10-3)

`BasicBlock` is a typed-index newtype defined via the
`rustc_index::newtype_index!` macro
[`rustc_middle/src/mir/mod.rs:1262-1292`]:

```rust
rustc_index::newtype_index! {
    /// A node in the MIR [control-flow graph][CFG].
    ///
    /// There are no branches (e.g., `if`s, function calls, etc.) within a basic block, which makes
    /// it easier to do [data-flow analyses] and optimizations. Instead, branches are represented
    /// as an edge in a graph between basic blocks.
    ///
    /// Basic blocks consist of a series of [statements][Statement], ending with a
    /// [terminator][Terminator]. ...
    #[derive(HashStable)]
    #[encodable]
    #[orderable]
    #[debug_format = "bb{}"]
    pub struct BasicBlock {
        const START_BLOCK = 0;
    }
}
```

Five derive-style attributes plus the const-body grammar
[`mod.rs:1285-1291`]:

- `#[derive(HashStable)]` -- inserts a `HashStable` impl.
- `#[encodable]` -- inserts `Encodable` / `Decodable` impls.
- `#[orderable]` -- enables `Ord` / `PartialOrd` derivation.
- `#[debug_format = "bb{}"]` -- overrides `Debug` so `bb0`, `bb1`,
  ... is the printed form. INFERENCE: this is the rustc analog of
  the inbound stage-1 expectation "`bbN` sigil rendering".
- `const START_BLOCK = 0;` inside the struct body -- this is the
  **const-body arm** of `newtype_index!`. The parsed grammar
  associates one or more `const NAME = INT;` declarations with the
  newtype, each of which becomes a `pub const` on the type. The
  inbound expectation at `stage-10/expectations.md` "From stage 1"
  bullet 2 names exactly this grammar as a macro extension
  Regseq's local `newtype_index!` needs.

`BasicBlock::start_location()` is defined as a regular impl method
right after the macro [`mod.rs:1294-1298`]:

```rust
impl BasicBlock {
    pub fn start_location(self) -> Location {
        Location { block: self, statement_index: 0 }
    }
}
```

INFERENCE: the const-body arm is a **convenience** for
declaration-site `pub const X: $Name = $Name::from_u32(N)`
generation, not a semantic carrier. The macro could also accept
zero `const` entries. The empty-body form is what Regseq's
existing `newtype_index!` accepts today; the const-body arm is the
single-grammar-extension stage 10 must add for `BlockId::START_BLOCK
= 0` to land inline.

`BasicBlock` integers are 32-bit (the macro defaults to `u32`-
backed via `rustc_index::Idx`). Niche-optimisation for
`Option<BasicBlock>` is automatic because the macro reserves
`u32::MAX` (or `u32::MAX - 1`) as a niche; INFERENCE: the inbound
expectation "`Option<BlockId>` and `BlockId` 4-byte size
assertions" mirrors how stage 1 handles `LocalId`'s same niche
profile.

**Maps to D-10-3.** The cardinality (single const `START_BLOCK
= 0`), the integer width (4 bytes, niched), the sigil
(`bb{}`-format Debug), and the const-body macro grammar are all
direct rustc analogs.

---

## 2. The `BasicBlockData` per-block struct (D-10-1)

`BasicBlockData<'tcx>` [`rustc_middle/src/mir/mod.rs:1306-1331`]:

```rust
///////////////////////////////////////////////////////////////////////////
// BasicBlockData

/// Data for a basic block, including a list of its statements.
///
/// See [`BasicBlock`] for documentation on what basic blocks are at a high level.
#[derive(Clone, Debug, TyEncodable, TyDecodable, HashStable, TypeFoldable, TypeVisitable)]
#[non_exhaustive]
pub struct BasicBlockData<'tcx> {
    /// List of statements in this block.
    pub statements: Vec<Statement<'tcx>>,

    /// All debuginfos happen before the statement.
    /// Put debuginfos here when the last statement is eliminated.
    pub after_last_stmt_debuginfos: StmtDebugInfos<'tcx>,

    /// Terminator for this block.
    ///
    /// N.B., this should generally ONLY be `None` during construction.
    /// Therefore, you should generally access it via the
    /// `terminator()` or `terminator_mut()` methods. The only
    /// exception is that certain passes, such as `simplify_cfg`, swap
    /// out the terminator temporarily with `None` while they continue
    /// to recurse over the set of basic blocks.
    pub terminator: Option<Terminator<'tcx>>,

    /// If true, this block lies on an unwind path. This is used
    /// during codegen where distinct kinds of basic blocks may be
    /// generated (particularly for MSVC cleanup). Unwind blocks must
    /// only branch to other unwind blocks.
    pub is_cleanup: bool,
}
```

Four fields, `#[non_exhaustive]`. Field-by-field:

### 2.1 `statements: Vec<Statement<'tcx>>`

A flat heap-allocated vector of `Statement`s. No reordering, no
deduplication, no inline storage. Stage-8 [§22.1.1] has already
adopted the same `Vec<Statement>` shape. Stage-10 carries this
through unchanged at the per-block level.

INFERENCE: storing as `Vec<Statement>` rather than
`SmallVec<[Statement; N]>` is rustc's choice; the per-block
statement count is unbounded and the typical block has several to
dozens of statements before the terminator. INFERENCE: a
RegseqModel `Box<[Statement]>` would shave the unused `Vec`
capacity slot post-construction (one usize), but rustc keeps the
`Vec` form because it's a working IR; RegseqModel's bridge
output is read-only post-extraction so a `Box<[Statement]>` is a
plausible refinement -- not a load-bearing decision either way.

### 2.2 `after_last_stmt_debuginfos: StmtDebugInfos<'tcx>`

Pure debuginfo, attached to the trailing position in the block
when the last statement was eliminated [`mod.rs:1312-1314`
docstring]. The `StmtDebugInfos` type is per-statement debug-only;
stage 8 already dropped statement-level debuginfos
[`stage-8/spec-snapshot.md` D-8-3] because RegseqModel emits no
DWARF / debug records. INFERENCE: the **block-trailing**
debuginfo bucket is the same kind of construct -- pure DWARF
support state -- and inherits the same drop rationale. Stage 10
drops `after_last_stmt_debuginfos` for the same reason stage 8
dropped statement debuginfos. The pretty-printer evidence in §10
confirms it is purely a debug-print artifact.

### 2.3 `terminator: Option<Terminator<'tcx>>`

The Option wrapper exists **for construction-time mutation**
only. Three load-bearing pieces of evidence pin the
"non-None post-construction" invariant:

1. The docstring [`mod.rs:1318-1324`]: "N.B., this should generally
   ONLY be `None` during construction. ... The only exception is
   that certain passes, such as `simplify_cfg`, swap out the
   terminator temporarily with `None` while they continue to
   recurse over the set of basic blocks."

2. The accessor [`mod.rs:1356-1358`]:
   ```rust
   #[inline]
   pub fn terminator(&self) -> &Terminator<'tcx> {
       self.terminator.as_ref().expect("invalid terminator state")
   }
   ```
   Panics on None. Every consumer that needs the terminator goes
   through this accessor (see codegen-walk evidence in §9).

3. `simplify::CfgSimplifier::simplify` [`rustc_mir_transform/src/simplify.rs:159-160`]:
   ```rust
   let mut terminator =
       self.basic_blocks[bb].terminator.take().expect("invalid terminator state");
   ```
   The same panic-message pattern, used during the temporary
   `take`-then-restore dance. INFERENCE: pass authors may briefly
   leave a block in a None-terminator state, but every external
   consumer of the IR (codegen, validator, pretty-printer) sees
   only `Some(_)`.

For RegseqModel: the bridge consumes already-finalized MIR (post
`run_analysis_to_runtime_passes` + `run_optimization_passes`), so
the construction-time None state cannot reach the IR.
RegseqModel collapses to non-Optional `terminator: Terminator` --
the same shape rustc_public chose [`./rustc_public.md` §1].

### 2.4 `is_cleanup: bool`

A boolean tag marking unwind-path blocks
[`mod.rs:1326-1330`]. The docstring is terse:

> If true, this block lies on an unwind path. This is used
> during codegen where distinct kinds of basic blocks may be
> generated (particularly for MSVC cleanup). Unwind blocks must
> only branch to other unwind blocks.

Four binding constraints on cleanup blocks
[`rustc_middle/src/mir/syntax.rs:684-701`, doc-comment on
`UnwindAction`]:

1. All `unwind` fields in cleanup blocks must be
   `UnwindAction::Terminate` or `UnwindAction::Unreachable`.
2. `Return` terminators are not allowed in cleanup blocks.
   `Terminate` and `Resume` are.
3. All reachable-from-cleanup blocks must also be cleanup blocks
   (statically checked by the type system).
4. Control flow between cleanup blocks must form an "inverted
   tree" (used to ensure correct landing-pad codegen on MSVC).

The validator enforces (1) at
[`rustc_mir_transform/src/validate.rs:251-280`] (`check_unwind_edge`),
(2) at `validate.rs:519-522` (`Cannot 'Return' from cleanup basic
block`), (3) implicitly at `validate.rs:139-167` (`check_edge`),
and (4) at `validate.rs:173-249` (`check_cleanup_control_flow` --
inverted-tree dominators check + cycle check).

Under `panic = "abort"` (Linux/Zephyr drivers' canonical
configuration -- the regseq corpus pinned in
`stage-0/architecture.md`), the runtime-lowering pipeline runs
`AbortUnwindingCalls` [`rustc_mir_transform/src/abort_unwinding_calls.rs:24-144`]:

- Body-can-unwind is computed from the body's ABI
  [`abort_unwinding_calls.rs:46-61`].
- `UnwindResume` is rewritten to `Unreachable` (target doesn't
  unwind) or `UnwindTerminate(Abi)` (body cannot unwind)
  [`abort_unwinding_calls.rs:77-83`].
- For `Call` / `Drop` / `Assert` / `InlineAsm` terminators in
  non-cleanup blocks: if the call target cannot unwind, force
  `unwind_action = UnwindAction::Unreachable`
  [`abort_unwinding_calls.rs:116-122`].
- `super::simplify::remove_dead_blocks(body)` is called at the
  end [`abort_unwinding_calls.rs:138`].

The follow-up `SimplifyCfg::PreOptimizations` runs in
`run_runtime_cleanup_passes`
[`rustc_mir_transform/src/lib.rs:671`]; it calls
`simplify_cfg(...)` -> `remove_dead_blocks(body)`
[`simplify.rs:79-89, 349-413`]. `remove_dead_blocks` walks
`reachable_as_bitset(body)` [`simplify.rs:359`] and retains only
reachable blocks [`simplify.rs:380-383`]. INFERENCE: cleanup
blocks become unreachable once their incoming unwind edges are
flipped to `UnwindAction::Unreachable` by `AbortUnwindingCalls`,
so under panic=abort `is_cleanup=true` blocks reaching codegen are
exclusively those still wired to (now-unreachable) unwind paths
-- and `remove_dead_blocks` strips them.

INFERENCE: HYPOTHESIS: there is a residual "is_cleanup=true could
still leak through to optimized_mir under exotic ABI / target
configurations" risk. The relevant evidence: `AbortUnwindingCalls`
is `is_required: true` [`abort_unwinding_calls.rs:141-143`], so it
is NOT subject to opt-level gating; under panic=abort it always
runs. `simplify::remove_dead_blocks` is also not opt-level-gated
since it runs inside `SimplifyCfg::PreOptimizations`, whose gate
is `is_required: false` -- but that variant is scheduled
unconditionally in `run_runtime_cleanup_passes`
[`lib.rs:671`], and `is_required: false` means "may be skipped if
optimizations are suppressed via `Optimizations::Suppressed`," NOT
"opt-level-gated." [`stage-8/prior-art/rustc.md` §3] documented
this distinction; regseq's whitelist preserves the discharge path.

For RegseqModel: stage 9 already pinned the rejection of
`UnwindResume` and `UnwindTerminate` terminators based on the
panic=abort discharge [`../../spec.md` §23.6.1 rows 5-6, citing
`abort_unwinding_calls.rs:115-121`]. By the same chain, all
remaining `is_cleanup=true` blocks are unreachable post-cleanup-
pass and pruned. INFERENCE: RegseqModel can drop the `is_cleanup`
field entirely; there is no semantic content that `is_cleanup`
carries that any RegseqModel consumer would need (no MSVC; no
landing-pad codegen).

### 2.5 Field-set cardinality

rustc has 4 fields. RegseqModel's spec-9 preview at
`../../spec.md:8448-8453` already shows the 2-field form:

```rust
pub struct BasicBlock {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}
```

INFERENCE: this drops `after_last_stmt_debuginfos` (per §2.2) and
`is_cleanup` (per §2.4); it collapses `terminator: Option<...>` to
non-Optional `Terminator` (per §2.3). Stage 10's formal
contribution under D-10-1 is to lock those drops with the rustc
evidence cited above.

The `BasicBlockData` methods at `mod.rs:1333-1413` are
construction / mutation helpers (`new`, `new_stmts`, `terminator`,
`terminator_mut`, `is_empty_unreachable`, `mono_successors`,
`retain_statements`, `strip_nops`, `drop_debuginfo`) -- all
internal to rustc's MIR-build / MIR-transform pipeline. RegseqModel
is a downstream consumer; INFERENCE: the only methods relevant to
RegseqModel's read-side are `is_empty_unreachable` (§2.6) and
maybe `mono_successors` (covered in §6 of the rustc_public peer
since stage-9 already adopted the post-mono successor view).

### 2.6 The `is_empty_unreachable` helper

Defined at `mod.rs:1366-1369`:

```rust
/// Does the block have no statements and an unreachable terminator?
#[inline]
pub fn is_empty_unreachable(&self) -> bool {
    self.statements.is_empty() && matches!(self.terminator().kind, TerminatorKind::Unreachable)
}
```

Used by `remove_dead_blocks` [`simplify.rs:355-357`] to
deduplicate empty-Unreachable blocks (one such block kept; the
rest replaced with the kept index in successor maps). INFERENCE:
RegseqModel does NOT need to deduplicate -- the bridge consumes
post-`SimplifyCfg::PreOptimizations` MIR, so any deduplication has
already happened. But `is_empty_unreachable` may be a useful
analysis predicate for the eventual stage-14 backend (e.g., to
elide block emission for empty-Unreachable blocks). INFERENCE:
this is observation-only for stage 10, not a spec contribution.

---

## 3. The `BasicBlocks` body-level container (D-10-2)

The wrapper struct [`rustc_middle/src/mir/basic_blocks.rs:14-19`]:

```rust
#[derive(Clone, TyEncodable, TyDecodable, Debug, HashStable, TypeFoldable, TypeVisitable)]
pub struct BasicBlocks<'tcx> {
    basic_blocks: IndexVec<BasicBlock, BasicBlockData<'tcx>>,
    /// Use an `Arc` so we can share the cache when we clone the MIR body, as borrowck does.
    cache: Arc<Cache>,
}
```

Two private fields. `basic_blocks` is the indexed storage;
`cache` is a shared (across clones, via `Arc`) lazy CFG-metadata
cache.

### 3.1 The `Cache` struct

```rust
#[derive(Clone, Default, Debug)]
struct Cache {
    predecessors: OnceLock<Predecessors>,
    reverse_postorder: OnceLock<Vec<BasicBlock>>,
    dominators: OnceLock<Dominators<BasicBlock>>,
}
```

[`basic_blocks.rs:32-37`]. Three `OnceLock`-wrapped lazy caches:

- `predecessors: IndexVec<BasicBlock, SmallVec<[BasicBlock; 4]>>`
  [`basic_blocks.rs:22`] -- per-block list of immediate
  predecessors. The `SmallVec<[..; 4]>` inline-budget is
  motivated by `// Typically 95%+ of basic blocks have 4 or
  fewer predecessors.` [`basic_blocks.rs:21`].
- `reverse_postorder: Vec<BasicBlock>` -- the RPO traversal
  order of all blocks reachable from `START_BLOCK`.
- `dominators: Dominators<BasicBlock>` -- the dominator-tree
  result computed via `rustc_data_structures::graph::dominators`.

Each is computed lazily by a `get_or_init`:

- `predecessors()` [`basic_blocks.rs:51-64`] iterates
  `self.basic_blocks.iter_enumerated()` and pushes `bb` onto each
  successor's predecessor list. The walk goes through
  `data.terminator` and skips None-terminator entries.
- `reverse_postorder()` [`basic_blocks.rs:71-78`] runs
  `Postorder::new(&self.basic_blocks, START_BLOCK, None).collect()`
  and reverses. (The `Postorder` iterator is at
  `mir/traversal.rs:97-211`.)
- `dominators()` [`basic_blocks.rs:45-48`] calls
  `dominators(self)` -- the data-structures crate's standard
  algorithm.

### 3.2 Cache encoding (load-bearing for D-10-2)

The cache is **not serialized**. Three impls at
`basic_blocks.rs:163-177`:

```rust
impl<S: Encoder> Encodable<S> for Cache {
    #[inline]
    fn encode(&self, _s: &mut S) {}
}

impl<D: Decoder> Decodable<D> for Cache {
    #[inline]
    fn decode(_: &mut D) -> Self {
        Default::default()
    }
}

impl<CTX> HashStable<CTX> for Cache {
    #[inline]
    fn hash_stable(&self, _: &mut CTX, _: &mut StableHasher) {}
}
```

Encoding writes nothing, decoding produces `Default::default()`
(empty `OnceLock`s), HashStable hashes nothing. INFERENCE: the
cache is purely a local optimisation; cross-tcx (i.e., across
incremental recompilation) it is rebuilt on demand. The wrapper
struct exists for its **methods**, not its serialised content;
on the wire `BasicBlocks<'tcx>` is equivalent to `IndexVec<BasicBlock,
BasicBlockData>`.

### 3.3 Mutation discipline

Three accessors:

- `as_mut(&mut self) -> &mut IndexVec<...>`
  [`basic_blocks.rs:81-85`] -- invalidates the CFG cache via
  `invalidate_cfg_cache()` before returning the mutable handle.
- `as_mut_preserves_cfg(&mut self) -> &mut IndexVec<...>`
  [`basic_blocks.rs:88-101`] -- does NOT invalidate the cache,
  with a 4-bullet contract: number of blocks unchanged, set of
  successors per terminator unchanged, `SwitchInt.targets`
  unchanged. The caller promises not to violate these.
- `invalidate_cfg_cache(&mut self)` [`basic_blocks.rs:108-117`] --
  uses `Arc::get_mut` to clear in place if not aliased; otherwise
  swaps in a fresh `Arc`.

The `Deref<Target = IndexSlice<BasicBlock, BasicBlockData<'tcx>>>`
impl [`basic_blocks.rs:120-127`] gives ergonomic read access; all
read-side iteration / indexing flows through the slice.

### 3.4 Graph-trait impls

`BasicBlocks` implements four `rustc_data_structures::graph` traits
[`basic_blocks.rs:129-157`]:

- `DirectedGraph::Node = BasicBlock`, `num_nodes = self.basic_blocks.len()`.
- `StartNode::start_node = START_BLOCK`.
- `Successors`: `successors(node) = self.basic_blocks[node].terminator().successors()`.
- `Predecessors`: `predecessors(node) = self.predecessors()[node].iter().copied()`.

INFERENCE: the wrapper exists primarily so that
`BasicBlocks<'tcx>` can satisfy these traits and be dominator /
RPO / predecessor-aware. Without the wrapper, `IndexVec<BasicBlock,
BasicBlockData<'tcx>>` would not be a graph node-set in the
data-structures sense; the wrapper IS the graph.

### 3.5 RegseqModel implications

Three concrete choices stage 10 must make under D-10-2:

(a) **Wrapper vs bare IndexVec.** rustc has the wrapper because of
the cache + graph-traits. RegseqModel's `regseq-analysis` crate is
a separate crate per Q1 (see
`../../architecture.md`); CFG analyses would live there. INFERENCE:
RegseqModel can ship a bare `IndexVec<BlockId, BasicBlock>` at the
stage-10 layer and have `regseq-analysis` either (i) wrap it
locally or (ii) compute predecessors / dominators on demand
without caching. rustc's choice was driven by repeated
intra-pipeline use (every dataflow pass wants the cache);
RegseqModel's downstream consumers (codegen-side stage-14 backends,
phase-2 device-pressure runs) may have far less repeated use.

(b) **Cache eager vs deferred.** rustc's lazy `OnceLock` model
costs a u-word atomic plus initialised-flag per cached field. If
RegseqModel ships the wrapper, the same lazy model is the closest
peer. If RegseqModel ships bare, the cache is the analysis crate's
problem.

(c) **Encoding shape.** rustc encodes only `IndexVec<BasicBlock,
BasicBlockData>`. RegseqModel's serde shape (Q5
`textual-pretty-printer + serde-JSON`) is the same -- the cache,
if present, is empty-encoded. INFERENCE: choosing the wrapper does
not penalise the wire format; choosing bare does not lose any
semantically-meaningful content.

The decision space is real (ergonomics of trait impls vs
single-level-IR Q3 minimalism). Stage 10 frames it; stage-10 work
phase resolves it.

---

## 4. The `ReorderBasicBlocks` cleanup pass (D-10-4)

`prettify.rs::ReorderBasicBlocks` is a "prettify" pass --
named by its file's module-level doc comment
[`rustc_mir_transform/src/prettify.rs:1-5`]:

> These two passes provide no value to the compiler, so are off at
> every level.
>
> However, they can be enabled on the command line
> (`-Zmir-enable-passes=+ReorderBasicBlocks,+ReorderLocals`)
> to make the MIR easier to read for humans.

Definition [`prettify.rs:14-42`]:

```rust
/// Rearranges the basic blocks into a *reverse post-order*.
///
/// Thus after this pass, all the successors of a block are later than it in the
/// `IndexVec`, unless that successor is a back-edge (such as from a loop).
pub(super) struct ReorderBasicBlocks;

impl<'tcx> crate::MirPass<'tcx> for ReorderBasicBlocks {
    fn is_enabled(&self, _session: &Session) -> bool {
        false
    }

    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        let rpo: IndexVec<BasicBlock, BasicBlock> =
            body.basic_blocks.reverse_postorder().iter().copied().collect();
        if rpo.iter().is_sorted() {
            return;
        }

        let mut updater = BasicBlockUpdater { map: rpo.invert_bijective_mapping(), tcx };
        debug_assert_eq!(updater.map[START_BLOCK], START_BLOCK);
        updater.visit_body(body);

        permute(body.basic_blocks.as_mut(), &updater.map);
    }

    fn is_required(&self) -> bool {
        false
    }
}
```

Five things to extract:

1. **Pass-manager status.** Both `is_required: false`
   [`prettify.rs:39-41`] and `is_enabled(_) -> false`
   [`prettify.rs:21-23`]. INFERENCE: the pass runs **only** when
   the explicit gate flips it on -- either via the
   `-Zmir-enable-passes=+ReorderBasicBlocks` CLI flag or the
   programmatic `mir_enable_passes` hook used by
   `regseq-driver/src/callbacks.rs:25-36`.

2. **Algorithm.** Compute `rpo` from the cached
   `reverse_postorder()` (covered §3.1). Test whether the IndexVec
   is already in RPO; if yes, return early
   [`prettify.rs:28-30`]. Otherwise, invert the bijective mapping
   to build a `permutation: BasicBlock -> BasicBlock` map, walk
   the body's terminators rewriting all successors via
   `BasicBlockUpdater::visit_terminator`
   [`prettify.rs:117-120`], and finally permute the
   `body.basic_blocks` IndexVec via `permute(...)`
   [`prettify.rs:98-105`].

3. **`START_BLOCK` is preserved.** The `debug_assert_eq!(updater.map[START_BLOCK],
   START_BLOCK)` [`prettify.rs:33`] asserts that the entry block
   stays at index 0 after the permutation. RPO begins at the entry
   so this is guaranteed by the algorithm: `Postorder::new(...,
   START_BLOCK, None)` visits START_BLOCK last (after all
   reachable successors); reversing puts START_BLOCK at index 0.

4. **Idempotency.** After ReorderBasicBlocks runs once, a second
   run early-returns. INFERENCE: re-running is safe; this matters
   when the bridge wants to assert on the post-condition without
   worrying about double-applies.

5. **Schedule.** `ReorderBasicBlocks` is scheduled in
   `run_optimization_passes` [`lib.rs:781`] near the end of the
   list (right after `add_call_guards::CriticalCallEdges` at
   [`lib.rs:779`] and before `dump_mir::Marker("PreCodegen")` at
   [`lib.rs:784`]). INFERENCE: this places ReorderBasicBlocks at
   the very end of the optimization pipeline -- after all CFG-
   modifying optimizations have run, so the RPO ordering is
   established on the final block layout.

### 4.1 Cross-reference: `BasicBlocks::reverse_postorder()` cache

`reverse_postorder()` at `basic_blocks.rs:71-78` lazily computes
the same vector ReorderBasicBlocks reads. INFERENCE: under our
whitelist, the bridge's `tcx.optimized_mir(...)` call has just run
ReorderBasicBlocks, so the IndexVec is in RPO. The
`BasicBlocks::reverse_postorder()` cache, if subsequently invoked,
will compute `[BasicBlock(0), BasicBlock(1), ..., BasicBlock(n-1)]`
-- the identity. INFERENCE: under our pipeline, reverse-postorder
order **is** the storage order; an analysis can iterate
`basic_blocks.iter_enumerated()` and get RPO for free.

### 4.2 RegseqModel implications (D-10-4)

RegseqModel's `BlockId` storage inherits the RPO invariant from
the bridge's whitelist. Three concrete invariants stage 10
should pin:

- `BlockId(0)` is the entry block (also the inbound stage-1
  invariant; `START_BLOCK = BlockId::from_u32(0)`).
- For every non-back-edge successor `b'` of `BlockId(i)`, the
  successor's index is `i' > i`. (Direct rustc analog: the
  prettify.rs doc-comment "all the successors of a block are
  later than it in the IndexVec, unless that successor is a back-
  edge.")
- Iterating `basic_blocks.iter_enumerated()` yields blocks in RPO
  order.

INFERENCE: these are corollaries of running ReorderBasicBlocks;
RegseqModel does not need to re-run the pass, only to assert /
trust the post-condition.

---

## 5. Body-level pipeline integration

The bridge consumes `optimized_mir(def_id) -> &Body<'tcx>`
[`rustc_middle/src/ty/mod.rs:1671`]; `Body.basic_blocks:
BasicBlocks<'tcx>` [`mir/syntax.rs:152` -- referenced from stage
9, not re-derived here].

### 5.1 The optimized_mir pipeline

[`rustc_mir_transform/src/lib.rs:792-798`]:

```rust
fn optimized_mir(tcx: TyCtxt<'_>, did: LocalDefId) -> &Body<'_> {
    tcx.arena.alloc(inner_optimized_mir(tcx, did))
}
```

`inner_optimized_mir` calls `run_optimization_passes` at
`lib.rs:835`, which runs the long pipeline ending in
`prettify::ReorderBasicBlocks` and `prettify::ReorderLocals`.

The earlier phases:

- `run_analysis_to_runtime_passes` (analysis cleanup -> runtime
  lowering -> runtime cleanup) at `lib.rs:583-613`.
- `run_runtime_lowering_passes` includes
  `abort_unwinding_calls::AbortUnwindingCalls` (which is
  `is_required: true`) and `coroutine::StateTransform` at
  `lib.rs:651, 660`.
- `run_runtime_cleanup_passes` includes
  `simplify::SimplifyCfg::PreOptimizations` at `lib.rs:671`.
- `run_optimization_passes` is the optimization loop including
  `unreachable_prop::UnreachablePropagation`,
  `simplify::SimplifyCfg::AfterUnreachableEnumBranching`, ..., a
  final `simplify::SimplifyCfg::Final`, and the prettify passes.

INFERENCE: by the time the bridge sees a `Body<'tcx>`, the
following discharges have already happened (relevant to D-10-1
and D-10-5):

- All blocks have a `Some(Terminator)` (no construction state
  remains; `simplify_cfg` restores after `take`).
- `panic = "abort"` cleanup-block pruning is complete (per §2.4).
- Coroutine-only constructs are gone (`coroutine::StateTransform`
  ran, per stage-9's evidence in
  `../../stage-9/prior-art/rustc.md`).
- Reverse-postorder block ordering is in place (provided
  `mir_enable_passes` whitelisted ReorderBasicBlocks, which
  `regseq-driver/src/callbacks.rs:25-36` does).

### 5.2 The `MirPhase` ladder

Validator gates many checks on `body.phase`:

- `MirPhase::Analysis(AnalysisPhase::PostCleanup)` -- after the
  analysis-cleanup passes [`validate.rs:312-315`, statement
  retag-after-drop-lowering check].
- `MirPhase::Runtime(RuntimePhase::Initial)` -- after runtime
  lowering [`validate.rs:467-470`, `FalseEdge` post-runtime
  forbidden check].
- `MirPhase::Runtime(RuntimePhase::PostCleanup)` -- after runtime
  cleanup [`validate.rs:298-321`, multiple statement-kind
  removed-by-now checks].
- `MirPhase::Runtime(RuntimePhase::Optimized)` -- after
  optimization passes [`validate.rs:398-408`, critical-edge check;
  `validate.rs:443-447`, force-inline check].

INFERENCE: optimized_mir runs to `Optimized` phase, so all these
phase-gated invariants are enforced. RegseqModel does NOT need to
re-derive them; it inherits them as preconditions.

---

## 6. The MIR validator's per-block invariants (D-10-5)

`Validator::is_required: true` [`validate.rs:100-102`] -- always
runs in debug builds (the validator wraps the pipeline at multiple
points).

`CfgChecker` is the per-CFG-structure checker
[`validate.rs:111-121`]:

```rust
struct CfgChecker<'a, 'tcx> {
    when: &'a str,
    body: &'a Body<'tcx>,
    tcx: TyCtxt<'tcx>,
    unwind_edge_count: usize,
    reachable_blocks: DenseBitSet<BasicBlock>,
    value_cache: FxHashSet<u128>,
    can_unwind: bool,
}
```

Constructed with `reachable_blocks: traversal::reachable_as_bitset(body)`
[`validate.rs:72`]. `can_unwind` is body-dependent
[`validate.rs:47-65`].

### 6.1 `check_edge` invariants

[`validate.rs:139-171`]:

```rust
fn check_edge(&mut self, location: Location, bb: BasicBlock, edge_kind: EdgeKind) {
    if bb == START_BLOCK {
        self.fail(location, "start block must not have predecessors")
    }
    if let Some(bb) = self.body.basic_blocks.get(bb) {
        let src = self.body.basic_blocks.get(location.block).unwrap();
        match (src.is_cleanup, bb.is_cleanup, edge_kind) {
            (false, false, EdgeKind::Normal)
            | (true, true, EdgeKind::Normal) => {}
            (false, true, EdgeKind::Unwind) => {
                self.unwind_edge_count += 1;
            }
            _ => {
                self.fail(...)
            }
        }
    } else {
        self.fail(location, format!("encountered jump to invalid basic block {bb:?}"))
    }
}
```

Three invariants pinned here:

(a) **No jumps to START_BLOCK.** Validator fails on
    `bb == START_BLOCK` [`validate.rs:140-142`]. Equivalent to
    "start block has zero predecessors."

(b) **Block index in-range.** `self.body.basic_blocks.get(bb)`
    must succeed [`validate.rs:168-170`].

(c) **Cleanup edge invariants.** Three valid combinations of
    `(src.is_cleanup, dst.is_cleanup, edge_kind)`:
    non→non Normal, cleanup→cleanup Normal, non→cleanup Unwind.
    All others are rejected [`validate.rs:155-166`].

For RegseqModel: (a) and (b) survive as positive invariants
(START_BLOCK has no predecessors; all successors of every
terminator are valid `BlockId` indices in `0..n_blocks`). (c) is
RegseqModel-irrelevant because is_cleanup is dropped (per §2.4)
and unwind edges are gone (per stage-9 §23.6.1 rows 5-6).

### 6.2 `check_cleanup_control_flow` invariants

[`validate.rs:173-249`] enforces the "inverted-tree" topology
described in `mir/syntax.rs:684-701`. Cleanup-block dominators are
walked; the parent map is checked for shape-violations
("blocks dominated by X have edges to both Y and Z") and cycles
("Cleanup control flow violation: Cycle involving edge X -> Y").

INFERENCE: RegseqModel does not need this check because is_cleanup
is dropped.

### 6.3 Per-terminator visitor checks

[`validate.rs:353-525`] (`visit_terminator`):

- `SwitchInt`: target dedup invariant, edge-validity per target
  + otherwise [`validate.rs:358-375`].
- `Drop` / `Call` / `Assert` / `Yield`: edge + unwind edge
  validity [`validate.rs:377-453`].
- `FalseEdge` / `FalseUnwind` / `CoroutineDrop`: phase-gated
  forbidden after drop elaboration / runtime lowering
  [`validate.rs:466-501`].
- `UnwindResume`: only from cleanup blocks
  [`validate.rs:503-510`].
- `UnwindTerminate`: only from cleanup blocks
  [`validate.rs:512-516`].
- `Return`: NOT from cleanup blocks [`validate.rs:519-523`].
- `Unreachable`: no checks [`validate.rs:524`].

INFERENCE: stage 9 has already covered the terminator-level
invariants. Stage 10's per-block invariants are limited to the
edge-validity / start-block-no-predecessors / block-index-in-range
trio.

### 6.4 Reachability stance

The validator computes `reachable_blocks: DenseBitSet<BasicBlock>
= traversal::reachable_as_bitset(body)` at
`validate.rs:72`, but the only place it's read is
`check_cleanup_control_flow` [`validate.rs:201`] -- to skip
unreachable cleanup blocks.

INFERENCE: the validator does NOT enforce "every block must be
reachable from START_BLOCK." It tolerates dead blocks. The
discharge for dead blocks is `simplify::remove_dead_blocks`
[`simplify.rs:349`], which runs as part of `SimplifyCfg::*` (and
inside `AbortUnwindingCalls` for newly-dead cleanup blocks).
INFERENCE: rustc's stance is "dead blocks may exist, but cleanup
passes prune them at scheduled points."

For RegseqModel: HYPOTHESIS: stage 10 should mirror this -- store
all blocks as the bridge presents them, do NOT enforce a
reachability invariant on the IR. (The bridge's whitelist runs
`SimplifyCfg::PreOptimizations` and `SimplifyCfg::Final`, both
of which call `remove_dead_blocks`; INFERENCE: dead blocks in the
bridge output are vanishingly rare but technically possible if a
post-Final pass like `unreachable_prop::UnreachablePropagation`
[`lib.rs:735`] introduces one without a follow-up SimplifyCfg.
The pipeline does run another SimplifyCfg afterwards
[`SimplifyCfg::AfterUnreachableEnumBranching` at `lib.rs:736`,
`SimplifyCfg::Final` at `lib.rs:769`], so this is mostly a moot
concern.)

---

## 7. The `Body`-level basic-block iteration patterns

Two consumer-side iteration patterns to lift for stage 14
backend design:

### 7.1 rustc_codegen_ssa: `mono_reachable_reverse_postorder`

[`rustc_codegen_ssa/src/mir/mod.rs:204-320`] (`codegen_mir`):

```rust
if mir.basic_blocks.iter().any(|bb| {
    bb.is_cleanup || matches!(bb.terminator().unwind(), Some(mir::UnwindAction::Terminate(_)))
}) {
    start_bx.set_personality_fn(cx.eh_personality());
}

let cleanup_kinds =
    base::wants_new_eh_instructions(tcx.sess).then(|| analyze::cleanup_kinds(&mir));

let cached_llbbs: IndexVec<mir::BasicBlock, CachedLlbb<Bx::BasicBlock>> =
    mir.basic_blocks
        .indices()
        .map(|bb| {
            if bb == mir::START_BLOCK { CachedLlbb::Some(start_llbb) } else { CachedLlbb::None }
        })
        .collect();

...

let traversal_order = traversal::mono_reachable_reverse_postorder(mir, tcx, instance);
let memory_locals = analyze::non_ssa_locals(&fx, &traversal_order);

...

let mut unreached_blocks = DenseBitSet::new_filled(mir.basic_blocks.len());
// Codegen the body of each reachable block using our reverse postorder list.
for bb in traversal_order {
    fx.codegen_block(bb);
    unreached_blocks.remove(bb);
}

// FIXME: These empty unreachable blocks are *mostly* a waste. They are occasionally
// targets for a SwitchInt terminator, but the reimplementation of the mono-reachable
// simplification in SwitchInt lowering sometimes misses cases that
// mono_reachable_reverse_postorder manages to figure out.
// The solution is to do something like post-mono GVN. But for now we have this hack.
for bb in unreached_blocks.iter() {
    fx.codegen_block_as_unreachable(bb);
```

Key observations:

- Codegen pre-allocates a per-block cache slot for every block
  (`mir.basic_blocks.indices()`), even before knowing which are
  reachable. INFERENCE: the index space is dense and stable.
- Codegen iterates in `mono_reachable_reverse_postorder` (the
  monomorphization-aware RPO). Blocks not reached are codegened
  as unreachable stubs -- they remain in the index space because
  they may still be referenced as `SwitchInt` targets even when
  unreachable in the post-mono CFG.
- The walk reads `bb.is_cleanup` and `bb.terminator().unwind()` to
  decide whether to attach a personality function (for unwinding
  code-gen).

### 7.2 rustc_codegen_cranelift: `iter_enumerated()`

[`rustc_codegen_cranelift/src/base.rs:284-end`] (`codegen_fn_body`):

```rust
let reachable_blocks = traversal::mono_reachable_as_bitset(fx.mir, fx.tcx, fx.instance);

for (bb, bb_data) in fx.mir.basic_blocks.iter_enumerated() {
    let block = fx.get_block(bb);
    fx.bcx.switch_to_block(block);

    if !reachable_blocks.contains(bb) {
        // We want to skip this block, because it's not reachable. But we still create
        // the block so terminators in other blocks can reference it.
        fx.bcx.ins().trap(TrapCode::user(1 /* unreachable */).unwrap());
        continue;
    }

    if bb_data.is_cleanup {
        if cfg!(not(feature = "unwinding")) {
            continue;
        }

        fx.bcx.set_cold_block(block);
    }

    fx.bcx.ins().nop();
    for stmt in &bb_data.statements {
        fx.set_debug_loc(stmt.source_info);
        codegen_stmt(fx, block, stmt);
    }

    ...

    match &bb_data.terminator().kind {
        TerminatorKind::Goto { target } => { ... }
        ...
    }
}
```

INFERENCE: the cranelift backend iterates in IndexVec storage
order (which is RPO under our whitelist) and uses
`mono_reachable_as_bitset` only to gate full-codegen vs trap-stub
for unreachable blocks. Same dense-index-space assumption as the
SSA backend.

INFERENCE: both backends rely on (i) `IndexVec<BasicBlock,
BasicBlockData>` dense storage, (ii) per-block index stability
across the walk, (iii) `terminator()` non-None, (iv) per-statement
iteration via `&bb_data.statements`. All four directly map to
RegseqModel's stage-10 carry. None of them rely on
`after_last_stmt_debuginfos`.

---

## 8. Pretty-printer evidence (D-10-K)

`mir/pretty.rs::write_basic_block` [`pretty.rs:712-803`]:

```rust
fn write_basic_block(
    &self,
    block: BasicBlock,
    body: &Body<'tcx>,
    w: &mut dyn io::Write,
) -> io::Result<()> {
    let data = &body[block];

    // Basic block label at the top.
    let cleanup_text = if data.is_cleanup { " (cleanup)" } else { "" };
    writeln!(w, "{INDENT}{block:?}{cleanup_text}: {{")?;

    // List of statements in the middle.
    let mut current_location = Location { block, statement_index: 0 };
    for statement in &data.statements {
        ... // per-statement print + debuginfos
    }

    for debuginfo in data.after_last_stmt_debuginfos.iter() {
        writeln!(w, "{INDENT}{INDENT}// DBG: {debuginfo:?};")?;
    }

    // Terminator at the bottom.
    if data.terminator.is_some() {
        let indented_terminator = format!("{0}{0}{1:?};", INDENT, data.terminator().kind);
        ...
    }

    writeln!(w, "{INDENT}}}")
}
```

Five things to lift:

1. **Block-label format.** `"{block:?}{cleanup_text}: {{"` --
   relies on `BasicBlock`'s `Debug` impl from `#[debug_format =
   "bb{}"]` to produce `bb0`, `bb1`, .... Output:
   `    bb0: {`. Cleanup blocks render as `bb0 (cleanup): {`
   [`pretty.rs:723-724`].

2. **Statement loop.** Iterates `&data.statements` and indents
   each `{statement:?};`. Statement debuginfos rendered as `// DBG:`
   comment lines [`pretty.rs:728-763`].

3. **Trailing debuginfos.** `data.after_last_stmt_debuginfos` is
   rendered as `// DBG:` comment lines just before the terminator
   [`pretty.rs:765-767`]. INFERENCE: this is the only consumer of
   the `after_last_stmt_debuginfos` field beyond debug-info
   codegen; for RegseqModel which emits no debuginfo, the
   field's rendering is moot.

4. **Terminator at bottom.** Single line `{terminator.kind:?};`
   [`pretty.rs:771-788`]. Conditional on `data.terminator.is_some()`
   -- which the docstring at §2.3 said is true post-construction.

5. **Closing brace.** `writeln!(w, "{INDENT}}}")` at
   `pretty.rs:801`.

The body-level driver [`pretty.rs:385-390`]:

```rust
write_mir_intro(self.tcx, body, w, self.options)?;
for block in body.basic_blocks.indices() {
    writeln!(w)?;
    self.write_basic_block(block, body, w)?;
    if block.index() + 1 != body.basic_blocks.len() {
        writeln!(w)?;
    }
}
```

INFERENCE: body-level rendering walks `basic_blocks.indices()` --
the dense index space, in storage order. With the whitelist's RPO
ordering, this prints blocks in RPO (which produces the
human-readable "follow control flow" textual layout that
ReorderBasicBlocks was designed to facilitate).

### 8.1 RegseqModel implications (D-10-K)

Stage 10's `render_basic_block` will land at stage 11
alongside `render_place` / `render_operand` / `render_rvalue` /
`render_statement` / `render_terminator` (the existing stage-5
through stage-9 deferrals). The shape is direct: dense-index
walk; per-block label `bb{n}: {`; statements indented;
terminator at the bottom; closing brace.

INFERENCE: RegseqModel's renderer drops the ` (cleanup)` suffix
(no is_cleanup field) and the trailing debuginfo comments (no
debuginfo field). Statement / terminator rendering itself reuses
the deferred stage-8 / stage-9 helpers.

The `bb{}` debug-format sigil for `BlockId` (the inbound
expectation from stage 1 bullet 3) needs only the `Debug` impl
emitted by Regseq's `newtype_index!` macro -- the block-label
helper composes via `format!("{block:?}: {{")` exactly as rustc
does.

---

## 9. Construction-time None-terminator caveats (cross-reference for D-10-1)

The validator's "encountered jump to invalid basic block"
[`validate.rs:168-170`] would fire if a successor referenced an
out-of-range index, but it never fires on a None terminator
because its visitor first dereferences `terminator()` (panicking
on None) before checking edges. INFERENCE: the validator
implicitly assumes `terminator: Some(_)` -- this confirms the
docstring invariant pinned at §2.3.

The construction-time None state appears in three places:

- `BasicBlockData::new(terminator, is_cleanup)` allows None at
  construction [`mod.rs:1334-1336`].
- `simplify::CfgSimplifier::simplify` swaps in None during
  per-block work [`simplify.rs:159-160`].
- `simplify::remove_dead_blocks` tolerates `terminator.is_none()`
  for the deduplication-empty-unreachable check
  [`simplify.rs:354-356`].

INFERENCE: the bridge does not see any of these; they are all
internal to MIR-build / MIR-transform.

---

## 10. Summary: D-10 evidence map

- **D-10-1** (per-block struct shape): rustc has 4 fields
  [`mod.rs:1308-1331`]; RegseqModel keeps `statements: Vec<Statement>`
  unchanged, drops `after_last_stmt_debuginfos` (DWARF-only,
  same rationale as stage-8's debuginfo drop), collapses
  `terminator: Option<Terminator>` to non-Optional `Terminator`
  (post-construction invariant from `mod.rs:1318-1324` docstring +
  `mod.rs:1356-1358` accessor), and drops `is_cleanup: bool`
  (panic=abort + `AbortUnwindingCalls` discharge per
  `abort_unwinding_calls.rs:115-138` + `simplify::remove_dead_blocks`
  per `simplify.rs:349-413`). The 2-field final shape matches
  stage-9's preview at `../../spec.md:8448-8453`.

- **D-10-2** (body-level container): rustc wraps
  `IndexVec<BasicBlock, BasicBlockData<'tcx>>` in a
  `BasicBlocks<'tcx>` struct that adds a lazy `Arc<Cache>` for
  predecessors / RPO / dominators
  [`basic_blocks.rs:14-19, 32-37`]. Cache is empty-encoded
  [`basic_blocks.rs:163-177`], so the wrapper costs nothing on the
  wire but exists to host (a) the cache and (b) graph-trait impls
  [`basic_blocks.rs:129-157`]. RegseqModel's choice -- bare
  `IndexVec<BlockId, BasicBlock>` vs wrapper-with-cache -- is open;
  the per-Q1 architectural separation between `regseq-model` and
  `regseq-analysis` argues for the bare form, but
  ergonomic-trait-impl-bundling argues for the wrapper. Stage 10
  resolves.

- **D-10-3** (BlockId introduction): rustc's `BasicBlock` is a
  4-byte newtype-index with `#[debug_format = "bb{}"]`,
  `#[orderable]`, `#[encodable]`, and a const-body grammar
  declaring `START_BLOCK = 0` [`mod.rs:1262-1292`]. Niche
  optimisation for `Option<BasicBlock>` is automatic via the
  newtype_index! macro. The const-body grammar is the macro
  feature stage 10 must add to Regseq's local `newtype_index!`
  per the inbound expectation.

- **D-10-4** (block-ordering invariant): the
  `prettify::ReorderBasicBlocks` pass [`prettify.rs:14-42`]
  rearranges the IndexVec into reverse-postorder. Default-off
  (`is_required: false`, `is_enabled: false`); enabled in
  RegseqModel via `regseq_mir_passes()` whitelist
  [`crates/regseq-driver/src/callbacks.rs:25-36`]. After this pass:
  `BlockId(0)` is the entry; for every non-back-edge successor
  `b'` of `BlockId(i)`, `i' > i`. Idempotent. Cross-references
  the lazy `BasicBlocks::reverse_postorder()` cache
  [`basic_blocks.rs:71-78`], which under our whitelist
  computes the identity permutation.

- **D-10-5** (invariants registry): the rustc validator
  [`validate.rs`] enforces (a) `START_BLOCK` has zero
  predecessors [`validate.rs:140-142`]; (b) successors are
  in-range `BlockId` indices [`validate.rs:168-170`]; (c) cleanup-
  edge restrictions (irrelevant for RegseqModel post-§2.4); (d)
  cleanup inverted-tree topology (ditto); (e) per-terminator
  forbidden-by-phase checks (covered by stage 9). Reachability is
  NOT enforced -- dead blocks tolerated, pruned by
  `simplify::remove_dead_blocks` at scheduled points
  [`simplify.rs:349-413`]. The construction-time `terminator:
  None` state never reaches the bridge because all three sites
  that produce it (`BasicBlockData::new`, `CfgSimplifier::simplify`,
  `remove_dead_blocks`) are internal to MIR-build / MIR-transform.

- **D-10-K** (pretty-printer deferral): rustc's
  `mir/pretty.rs::write_basic_block` [`pretty.rs:712-803`] uses
  the `bb{}` debug-format-derived label, indents statements,
  optional `(cleanup)` tag, optional trailing debuginfos, and the
  terminator at the bottom. RegseqModel's `render_basic_block`
  drops the cleanup-tag and debuginfo paths (since those fields
  are gone) and otherwise mirrors the shape; defers to stage 11
  alongside the stage-5/6/7/8/9 renderer deferrals.
