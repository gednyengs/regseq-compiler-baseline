# Stage 10 -- Prior Art: Charon primary source

Primary-source Charon (ULLBC) evidence for **MIR basic blocks**:
the per-block `BlockData` shape, the body-level basic-block
container, the `BlockId` newtype-index machinery, the rustc-MIR ->
ULLBC translator's block construction order, the (lack of a)
dedicated block validator, and the basic-block pretty-printer.
Scoped to stage 10's candidate decision namespace:

- `D-10-1` Per-block `BasicBlock` struct shape (cardinality +
  per-field choices).
- `D-10-2` Body-level basic-block container shape (`IndexVec<BlockId, BasicBlock>` storage; cache vs no-cache; wrapper vs bare).
- `D-10-3` `BlockId` introduction (newtype-index application;
  const-body arm; size/niche assertions).
- `D-10-4` Block-ordering invariant (RPO inheritance from
  `ReorderBasicBlocks`).
- `D-10-5` Per-stage invariants registry (rejection table +
  positive invariants + walk deferral + F4/F4-2 OUT note).
- `D-10-K` `pretty.rs::render_basic_block` deferral and `bbN`
  sigil rendering.

Charon is INRIA's Rust front-end translator feeding the Aeneas
verification pipeline. ULLBC ("Unstructured LLBC") is the
basic-block-shaped IR retained directly out of MIR; LLBC ("Low-
Level Borrow Calculus") is the structurally-reconstructed sibling
that fuses statements and terminators. **For stage 10 the
relevant layer is ULLBC** -- ULLBC's `BlockData` is the direct
rustc-MIR `BasicBlockData<'tcx>` analog and the structurally-
closer peer to RegseqModel's stage-9 `BasicBlock` preview at
`spec.md:8448-8453`. LLBC's `Block { span, statements }`
[`llbc_ast.rs:88-92`] fuses control flow into nested
statement-list constructs and is *not* RegseqModel's target.

Repository: `https://github.com/AeneasVerif/charon` (main).
Local clone pinned at commit
`46ab49f627e1a8e72b17c437f05ae4d1c4c456df` (2025-11-27, "Merge
pull request #919 from Nadrieril/raw-consts"), same commit as
the stage-{6,7,8,9} Charon files.

Citation convention: every factual claim cites `[file:line]` in
the Charon source tree at this commit, paths relative to
`/Users/nyengele/Documents/sekekama/charon-1/charon/`.
`HYPOTHESIS:` flags speculation. `INFERENCE:` flags cross-site
synthesis.

This file does not duplicate stage-{6,7,8,9}/prior-art/charon.md;
it references them where load-bearing and avoids re-deriving
Operand / Rvalue / Statement / Terminator material -- those are
embedded inside the per-block container but their per-construct
analyses live in the earlier files.

---

## 1. ULLBC `BlockData` struct shape (D-10-1)

ULLBC's per-block container is a 2-field struct
[`src/ast/ullbc_ast.rs:125-130`]:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Drive, DriveMut)]
#[charon::rename("Block")]
pub struct BlockData {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}
```

**Field set.** Two fields only: `statements: Vec<Statement>`
(straight-line sub-terminator instructions) and
`terminator: Terminator` (the block's exit edge). No `is_cleanup`
flag, no debuginfo accumulator, no per-block span.

**Drops vs rustc_middle.** Compared to
`rustc_middle::mir::BasicBlockData<'tcx>` (4 fields: `statements`,
`after_last_stmt_debuginfos`, `terminator: Option<Terminator>`,
`is_cleanup: bool`), ULLBC drops three things:

- *`after_last_stmt_debuginfos`*: silently dropped at the
  translator. `translate_basic_block` [`translate_bodies.rs:250-275`]
  reads only `block.statements` and `block.terminator.as_ref().unwrap()`.
- *`is_cleanup: bool`*: silently dropped at the translator;
  Charon does not consult `block.is_cleanup` anywhere. Cleanup-
  block detection is replaced by `Terminator::UnwindResume`
  [`ullbc_ast.rs:112`] surviving as a first-class kind, not a
  per-block flag.
- *`terminator: Option<...>`*: collapsed to non-Optional. The
  unwrap at `translate_bodies.rs:268` (`block.terminator.as_ref().unwrap()`)
  is the runtime enforcement of the rustc post-construction
  invariant ("not None except during construction" --
  `rustc_middle/src/mir/mod.rs:1318-1324`).

`Span` lives on each `Statement` and `Terminator` individually
[`ullbc_ast.rs:50-57, 115-123`], not on the block.
**Match for RegseqModel.** Stage-9's "Stage-10 vocabulary preview"
at `spec.md:8448-8453` proposes the same 2-field shape (no
`is_cleanup`, no debuginfo, terminator non-Optional). Charon is
exact precedent for that proposal -- including the silent-drop
discipline for the rustc fields RegseqModel wants gone.

`BlockData` derives `Serialize, Deserialize` [`ullbc_ast.rs:125`]
for Aeneas's verification consumers, plus `Drive, DriveMut` for
visitor traversal -- the latter is Charon-novel infrastructure
RegseqModel does not need.

`#[charon::rename("Block")]` [`ullbc_ast.rs:126`]: the Aeneas-
side OCaml deserialization expects the type to be named
`Block`, not `BlockData`. Internal/external naming divergence
is a Charon ergonomic concern, not a load-bearing precedent for
RegseqModel.

---

## 2. Body-level basic-block container (D-10-2)

ULLBC's body-level container is a typed indexed vector with no
cache layer [`ullbc_ast.rs:14-16`]:

```rust
#[charon::rename("Blocks")]
pub type BodyContents = Vector<BlockId, BlockData>;
pub type ExprBody = GExprBody<BodyContents>;
```

`Vector<I, T>` [`src/ids/vector.rs:22-30`] is Charon's owned
analog of rustc's `IndexVec<I, T>`:

```rust
pub struct Vector<I, T>
where
    I: Idx,
{
    vector: IndexVec<I, Option<T>>,
    elem_count: usize,
}
```

Internally it holds `IndexVec<I, Option<T>>` plus an
`elem_count` (count of non-`None` slots). The `Option<T>`
wrapper exists to support `reserve_slot` /
`set_slot` [`vector.rs:88-99`] -- the translator pre-allocates
`BlockId`s for forward references before filling them in
(`translate_basic_block_id` at
[`translate_bodies.rs:235-248`]).

**No CFG cache.** `Vector` has no analog of rustc's `Arc<Cache>`
(`predecessors`, `reverse_postorder`, `dominators`)
[`rustc_middle/src/mir/basic_blocks.rs:32-37`]. Predecessor /
RPO / dominator computation, when needed, lives in transform
passes that compute it on demand from `BlockData::targets()`
[`ullbc_ast_utils.rs:73-89`]. There is no eager-cache layer.

**No wrapper struct around the body table.** `BodyContents` is a
`pub type` alias, not a wrapper struct. The body-level
indirection lives one level up at `GExprBody<T>` [`gast.rs:48-58`]:

```rust
pub struct GExprBody<T> {
    pub span: Span,
    pub locals: Locals,
    pub comments: Vec<(usize, Vec<String>)>,
    pub body: T,
}
```

`T = BodyContents = Vector<BlockId, BlockData>` for ULLBC,
`T = Block` (the LLBC structured-form root) for LLBC.

**INFERENCE:** Charon's choice is "bare typed-indexed-vector at
the basic-block layer; cache-layer at zero; CFG analyses are the
job of separate passes." This precisely matches RegseqModel's
crate split (Q1 architectural decision: `regseq-analysis` is
separate from `regseq-model`). It is direct precedent for the
no-cache option in D-10-2.

Charon's `Vector::reserve_slot` design is *not* a precedent
RegseqModel needs to follow. RegseqModel's bridge consumes
already-translated MIR with all `BlockId`s pre-assigned by
rustc, so forward-reference reservation is unnecessary; a plain
`IndexVec<BlockId, BasicBlock>` (no `Option<>` wrapper) suffices.

---

## 3. `BlockId` newtype-index machinery (D-10-3)

`BlockId` is generated by Charon's `generate_index_type!` macro
[`src/ids/mod.rs:14-47`]:

```rust
generate_index_type!(BlockId, "Block");
```

Expansion (from the macro body at `mod.rs:19-46`):

```rust
index_vec::define_index_type! {
    #[derive(derive_generic_visitor::Drive, derive_generic_visitor::DriveMut)]
    #[drive(skip)]
    pub struct BlockId = usize;
    MAX_INDEX = std::u32::MAX as usize;
}

impl BlockId {
    pub const ZERO: Self = Self { _raw: 0 };
    pub fn is_zero(&self) -> bool { self.index() == 0 }
    pub fn to_pretty_string(self) -> String {
        format!("@Block{}", self)
    }
}

impl std::fmt::Display for BlockId { /* index().to_string() */ }
```

**Comparison points for RegseqModel:**

- *Backing store.* Charon uses `usize` (capped via
  `MAX_INDEX = u32::MAX as usize`). RegseqModel uses `u32` per
  the stage-1 `Idx` trait (`crates/regseq-model/src/idx.rs`).
  Both keep the wire form to 32 bits. The `usize` choice is
  ergonomic; `u32` is stricter and gives a 4-byte size assertion
  for `Option<BlockId>` niche (D-10-3 sub-decision).

- *Const associated.* `pub const ZERO` [`mod.rs:29`] is the
  Charon analog of RegseqModel's `pub const START_BLOCK`
  inbound from stage 1 (`spec.md:167`). Charon uses a generic
  `ZERO` plus a separate `pub static START_BLOCK_ID: BlockId =
  BlockId::ZERO` [`ullbc_ast.rs:11-12`]; RegseqModel proposes
  inlining the role-named constant into the macro
  `newtype_index!` invocation (the const-body arm extension).
  Charon's split is a fine alternative that does not require
  macro work, but it loses the "const declared at the type
  definition site" property that the inbound expectation
  preserves.

- *Pretty-name.* Charon's `to_pretty_string()` formats as
  `"@Block{n}"` [`mod.rs:33-35`]. The `bbN` short sigil is
  Charon-novel: it appears only in the body pretty-printer
  (next section) and is *not* the canonical `BlockId` Display.
  The Display impl emits the raw integer [`mod.rs:38-44`]. For
  RegseqModel the inbound expectation pins `bbN` as the
  canonical sigil for both Display and pretty-printer use.

- *Visitor-trait derive.* Charon hangs `Drive`/`DriveMut` from
  the `derive_generic_visitor` crate off the macro. RegseqModel
  has no equivalent visitor framework; this is not a
  precedent to mirror.

---

## 4. Block-ordering invariant (D-10-4)

**Charon does NOT preserve rustc's reverse-postorder.** The body
translator at `translate_bodies.rs:1230-1265` builds `BlockId`s
in *depth-first discovered order* via a worklist:

```rust
// Register the start block
let id = self.translate_basic_block_id(rustc_index::Idx::new(mir::START_BLOCK.as_usize()));
assert!(id == START_BLOCK_ID);

// For as long as there are blocks in the stack, translate them
while let Some(hax_block_id) = self.blocks_stack.pop_front() {
    let hax_block = body.basic_blocks.get(hax_block_id).unwrap();
    let block_id = self.translate_basic_block_id(hax_block_id);
    let block = self.translate_basic_block(&body, hax_block)?;
    self.blocks.set_slot(block_id, block);
}
```

`translate_basic_block_id` [`translate_bodies.rs:235-248`]
allocates a fresh `BlockId` (via `Vector::reserve_slot`) the
first time it sees a hax `BasicBlock` and records the mapping.
The *order in which IDs are allocated* is the order in which
blocks are first referenced through terminator successors,
seeded from `START_BLOCK`. This is closer to a preorder
traversal than RPO.

**Consequences for ID semantics:**

- `START_BLOCK_ID = BlockId::ZERO` [`ullbc_ast.rs:11-12`] is
  enforced by construction: the body translator's first call to
  `translate_basic_block_id(START_BLOCK)` is on an empty
  `blocks_map`, so it gets the first reserved slot
  (`BlockId(0)`). The `assert!(id == START_BLOCK_ID)` at
  `translate_bodies.rs:1248` codifies this invariant.

- Successor edges in Charon's `BlockId` space do *not* satisfy
  rustc's "after `ReorderBasicBlocks`, all successors are at
  higher indices except back-edges" property
  [`rustc_mir_transform/src/prettify.rs:14-17`]. Subsequent
  Charon transforms (`merge_goto_chains`, `prettify_cfg`,
  `filter_unreachable_blocks`) renumerate freely.

**INFERENCE:** RegseqModel's choice (D-10-4) sits between rustc
and Charon. RegseqModel inherits rustc's RPO ordering by virtue
of the `ReorderBasicBlocks` cleanup pass running in the bridge's
mandatory whitelist (`crates/regseq-driver/src/callbacks.rs:25-36`).
The bridge consumes MIR *after* `ReorderBasicBlocks` has run, so
the `body.basic_blocks` IndexVec is already RPO-ordered, and
RegseqModel's bridge can simply iterate `iter_enumerated()` to
preserve that ordering into `IndexVec<BlockId, BasicBlock>`. No
worklist, no `reserve_slot`, no renumeration. Charon's example
shows the alternative -- and demonstrates why it requires a
separate `filter_unreachable_blocks` pass downstream (next
section).

---

## 5. Block-level invariants and validators (D-10-5)

**Charon has no dedicated basic-block validator.** There is no
"check_blocks" or "validate_body" pass that asserts
terminator-non-None, successor-validity, or successor in-range.
Invariants are enforced by construction at translator and pass
boundaries:

- *Terminator non-None* enforced via `block.terminator.as_ref().unwrap()`
  at `translate_bodies.rs:268`. If rustc's post-construction
  invariant fails, Charon panics; no diagnostic.

- *START_BLOCK identity* enforced via
  `assert!(id == START_BLOCK_ID)` at `translate_bodies.rs:1248`.

- *Reachability* is *not* an invariant. Charon explicitly carries
  unreachable blocks. The `filter_unreachable_blocks` pass at
  `transform/normalize/filter_unreachable_blocks.rs` is a
  standalone normalization step that does a DFS from `BlockId(0)`,
  retains only reached blocks, and renumerates IDs:

  ```rust
  // src/transform/normalize/filter_unreachable_blocks.rs:13-39
  pub struct Transform;
  impl UllbcPass for Transform {
      fn transform_body(&self, _ctx: &mut TransformCtx, b: &mut ExprBody) {
          let mut explored: HashSet<BlockId> = HashSet::new();
          let mut to_explore: Vec<BlockId> = vec![BlockId::from_usize(0)];
          while let Some(bid) = to_explore.pop() {
              if explored.contains(&bid) { continue; }
              explored.insert(bid);
              to_explore.append(&mut b.body[bid].targets());
          }
          // Renumerate ...
      }
  }
  ```

  Pass docstring (`filter_unreachable_blocks.rs:1-2`): "Some
  passes like `reconstruct_asserts` lead to the apparition of
  'dangling' blocks, which are referenced nowhere and thus
  become unreachable. This pass filters those out."

- *Successor validity (in-range BlockIds)* enforced at access
  time via `b.body[bid].targets()` -- `BlockData::targets`
  [`ullbc_ast_utils.rs:73-89`] returns a `Vec<BlockId>`
  unchecked, and downstream indexing into `body[bid]` panics on
  out-of-range (the `Vector::Index` impl at
  `vector.rs:382-401` is unchecked). No explicit validator.

**INFERENCE:** Charon demonstrates two things relevant to D-10-5:

1. *Reachability is not load-bearing for IR storage.* All blocks
   (reachable or not) live in the body until an explicit pass
   filters them. RegseqModel can adopt the same stance: bridge
   stores all blocks; the invariants registry does not require
   reachability-from-START.

2. *Walk-impl deferral is the natural shape.* Charon's
   "construction-site enforcement" instead of a dedicated
   validator pass mirrors stage 9's pattern (D-9-10 Part C
   defers walk-implementation to stage 11). RegseqModel's Part
   C deferral has direct Charon precedent.

The `BlockData::targets` and `BlockData::targets_ignoring_unwind`
helpers [`ullbc_ast_utils.rs:73-104`] together with Charon's
`is_cleanup`-free design also give the same body-level
successors enumeration RegseqModel will need for stage-11
walk-implementation -- without any `is_cleanup` plumbing.

---

## 6. Pretty-printer: `bbN` sigil and per-block format (D-10-K)

Charon's basic-block pretty-printing is split across two impls.

**Per-block body** (statements + terminator)
[`src/pretty/fmt_with_ctx.rs:206-214`]:

```rust
impl<C: AstFormatter> FmtWithCtx<C> for ullbc::BlockData {
    fn fmt_with_ctx(&self, ctx: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for statement in &self.statements {
            writeln!(f, "{};", statement.with_ctx(ctx))?;
        }
        write!(f, "{};", self.terminator.with_ctx(ctx))?;
        Ok(())
    }
}
```

One statement per line ending in `;`, terminator on its own line
ending in `;`. No header, no footer; just the contents.

**Body-level driver** (block headers, indentation, surrounding
braces) [`src/pretty/fmt_with_ctx.rs:736-756`]:

```rust
impl<C: AstFormatter> FmtWithCtx<C> for GExprBody<ullbc_ast::BodyContents> {
    fn fmt_with_ctx(&self, ctx: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn fmt_body<C: AstFormatter>(
            f: &mut fmt::Formatter<'_>,
            ctx: &<<C as AstFormatter>::Reborrow<'_> as AstFormatter>::Reborrow<'_>,
            body: &Vector<BlockId, BlockData>,
        ) -> Result<(), fmt::Error> {
            let tab = ctx.indent();
            let ctx = &ctx.increase_indent();
            for (bid, block) in body.iter_indexed_values() {
                writeln!(f)?;
                writeln!(f, "{tab}bb{}: {{", bid.index())?;
                writeln!(f, "{}", block.with_ctx(ctx))?;
                writeln!(f, "{tab}}}")?;
            }
            Ok(())
        }
        self.fmt_with_ctx_and_callback(ctx, f, fmt_body::<C>)
    }
}
```

The output shape is:

```
bb0: {
  s_0_0;
  s_0_1;
  ...
  terminator_0;
}
bb1: {
  ...
}
```

**Sigil.** `bb{}` at line 748 -- exactly the `bbN` form RegseqModel
inherits from rustc's `#[debug_format = "bb{}"]` and from the
stage-1 inbound expectation. Charon and rustc agree; RegseqModel
follows.

**Iteration order.** The pretty-printer iterates
`body.iter_indexed_values()` -- i.e., in stored `BlockId` order.
Because Charon's `BlockId` order is depth-first-discovery, the
output reflects that. RegseqModel's inheritance of RPO from the
cleanup-pass whitelist means the same iter pattern produces RPO-
ordered output -- closer to rustc's pretty-printer output.

**Deferral feasibility.** The complete Charon basic-block
pretty-printer is ~25 lines (impl `FmtWithCtx for BlockData` +
`fmt_body` callback for `GExprBody<BodyContents>`). The
infrastructure dependencies are:

- `Statement::FmtWithCtx` (stage 8 deliverable, already deferred
  to stage 11 per D-8-K).
- `Terminator::FmtWithCtx` (stage 9 deliverable, already deferred
  to stage 11 per D-9-K).
- The `bb{}` sigil (already pinned by stage-1 inbound).

`render_basic_block` cannot land before `render_statement` and
`render_terminator` -- they are direct calls inside the per-block
body printer. **Direct precedent for D-10-K:** defer alongside
the stage-{5,6,7,8,9} deferrals; stage 11 lands them all together.

---

## Summary: Charon evidence per D-10 tag

- **D-10-1 (BasicBlock struct shape).** Charon's
  `BlockData { statements: Vec<Statement>, terminator: Terminator }`
  [`ullbc_ast.rs:127-130`] matches RegseqModel's stage-9 preview
  exactly: 2 fields, terminator non-Optional, no `is_cleanup`,
  no debuginfo, no per-block span. Drops are silent at the
  translator. Strong precedent for the proposed shape.

- **D-10-2 (Body-level container).** Charon uses bare
  `Vector<BlockId, BlockData>` [`ullbc_ast.rs:14-15`] with no
  cache layer. CFG analyses (predecessors, reachability) live in
  separate transform passes. Direct precedent for the
  no-cache / no-wrapper option, and aligned with RegseqModel's
  Q1 crate split (`regseq-model` vs `regseq-analysis`).
  RegseqModel does not need Charon's `Option<T>` slot-reservation
  layer because the bridge consumes pre-numbered MIR.

- **D-10-3 (BlockId newtype).** Charon's `generate_index_type!`
  [`src/ids/mod.rs:14-47`] is the structural analog of
  RegseqModel's `newtype_index!`. Charon uses `usize` capped at
  `u32::MAX`; RegseqModel uses `u32`. Charon's `pub const ZERO`
  [`mod.rs:29`] sits inside the impl block (not the macro
  invocation), with a separate `pub static START_BLOCK_ID`
  [`ullbc_ast.rs:11-12`] for the named-constant role -- a viable
  alternative to RegseqModel's proposed const-body macro arm,
  but with looser locality.

- **D-10-4 (Block-ordering invariant).** Charon does *not*
  inherit rustc's RPO; the body translator allocates `BlockId`s
  in depth-first discovery order via a worklist
  [`translate_bodies.rs:1230-1265`]. RegseqModel's choice (RPO
  inherited from `ReorderBasicBlocks` in the cleanup-pass
  whitelist) is the alternative; Charon's design demonstrates
  why the alternative requires a downstream
  `filter_unreachable_blocks` pass to clean up dangling blocks.

- **D-10-5 (Invariants registry; walk deferral).** Charon has no
  dedicated block validator; invariants are enforced at
  construction sites (`unwrap` for terminator-non-None,
  `assert!` for START_BLOCK identity) and unreachable blocks
  are filtered by an explicit normalization pass
  [`transform/normalize/filter_unreachable_blocks.rs:13-39`].
  Direct precedent for stage 10's Part C walk-impl deferral
  (stage 11 picks it up) and for *not* requiring reachability as
  an invariant.

- **D-10-K (Pretty-printer + bbN sigil).** Charon's basic-block
  pretty-printer is ~25 LOC across two `FmtWithCtx` impls
  [`src/pretty/fmt_with_ctx.rs:206-214,736-756`]; the sigil is
  `bb{}` [line 748] matching rustc. The implementation has
  hard dependencies on `Statement` and `Terminator` formatters,
  which are themselves deferred to stage 11. Direct precedent
  for D-10-K's deferral alongside D-{5,6,7,8,9}-K.

---

End of stage-10 prior-art-Charon document. Companion files for
this stage: `rustc.md` (the canonical `BasicBlock` /
`BasicBlockData` / `BasicBlocks` source), `rustc_public.md` (the
owned, `'tcx`-free, serde-able mirror), and `mrustc.md` (the
Rust-to-C peer with its own MIR `BasicBlock` shape).
