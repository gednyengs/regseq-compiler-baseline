# Stage 10 -- Prior Art: rustc_public primary source

`rustc_public` (formerly `rustc_smir`, the in-tree home of the
Stable-MIR project) is the rustc-internal crate that defines an
owned, `'tcx`-free, `serde::Serialize`-able mirror of `rustc_middle`'s
MIR. Its purpose is "the WIP public interface to rustc internals...
for third-party tools to invoke" -- documented at the crate root
[`rustc_public/src/lib.rs:1-3,13-17`]:

```rust
//! The WIP public interface to rustc internals.
//!
//! For more information see <https://github.com/rust-lang/project-stable-mir>
...
//! This crate shall contain all type definitions and APIs that we
//! expect third-party tools to invoke to interact with the compiler.
//!
//! The goal is to eventually be published on
//! [crates.io](https://crates.io).
```

Stages 5-9 already cover the same Place / Operand / Rvalue /
Statement / Terminator surfaces -- this file does not re-derive
that material. Its scope is the **per-block container**
(`BasicBlock`), the **body-level basic-block table** (`Body.blocks`),
the **typed-ID equivalent** (`BasicBlockIdx`), and the **rustc_middle
-> rustc_public translator** that derives the slimmed shape from the
upstream `BasicBlockData<'tcx>` / `BasicBlocks<'tcx>` pair.

All paths below are relative to the pinned-toolchain root:

```
~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/
```

(See `../../rustc-references.md` for the canonical statement of this
root path.)

Every factual claim cites a `[file:line]` in that pinned source.
Hypotheses are flagged as **HYPOTHESIS:**. Cross-source synthesis
across two or more sites is flagged as **INFERENCE:**. Citations to
sibling prior-art files (rustc, mrustc, charon) are out of scope --
this file documents `rustc_public` only.

Stage 10's candidate D-tag namespace, restated for self-containment:

- **D-10-1** -- per-block `BasicBlock` struct shape (cardinality +
  per-field choices: `statements`, `terminator`, plus what to drop).
- **D-10-2** -- body-level basic-block container shape (storage
  type; cache-vs-no-cache for predecessors/RPO/dominators; wrapper
  vs bare).
- **D-10-3** -- `BlockId` introduction (typed-newtype vs bare-usize;
  inbound `newtype_index!` const-body arm; size assertions).
- **D-10-4** -- block-ordering invariant (does the IndexVec/Vec
  order coincide with reverse-postorder, and how does it get there).
- **D-10-5** -- per-stage invariants registry (which rustc-side
  fields elide silently; what `Body::new` enforces; what successor
  in-range invariant exists).
- **D-10-K** -- pretty-printer (`bbN` block-label sigil; basic-block
  rendering shape).

---

## 1. `BasicBlock` two-field struct

`rustc_public::BasicBlock` is a closed two-field struct
[`rustc_public/src/mir/body.rs:133-137`]:

```rust
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct BasicBlock {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}
```

Compared to `rustc_middle::mir::BasicBlockData<'tcx>`'s four fields
(`statements: Vec<Statement<'tcx>>`,
`after_last_stmt_debuginfos: StmtDebugInfos<'tcx>`,
`terminator: Option<Terminator<'tcx>>`, `is_cleanup: bool`)
[`rustc_middle/src/mir/mod.rs:1306-1331`], rustc_public:

1. **Drops `after_last_stmt_debuginfos`** (the post-block debug-info
   trailer carrying source-level variable visibility ranges).
2. **Drops `is_cleanup: bool`** (the MSVC unwind-cleanup-block tag).
3. **Collapses `terminator: Option<Terminator<'tcx>>` to
   non-optional `terminator: Terminator`** -- the Optional is gone
   from the type itself. The Option in rustc_middle is documented as
   a construction-time concession only ("N.B., this should generally
   ONLY be `None` during construction"
   [`rustc_middle/src/mir/mod.rs:1318-1323`]); rustc_public bakes
   "post-construction => always Some" into the type rather than the
   docstring.

The two surviving fields are both `pub` (no encapsulation; consumers
read them as a closed pair). The full field set is structurally
exhaustive: the visitor's `super_basic_block` destructures
`let BasicBlock { statements, terminator } = bb;`
[`rustc_public/src/mir/visit.rs:139-145`] without any `..` or `_`
catch-all (the visitor module's preamble explicitly forbids both
[`rustc_public/src/mir/visit.rs:31-35`]). This makes the two-field
shape a load-bearing API contract, not a documentation
recommendation.

There is **no `BasicBlock`-level `Span`**. Spans live one level
down: each `Statement` carries its own span
[`rustc_public/src/mir/body.rs` -- inherited from stage-8 prior art],
each `Terminator` carries its own span
[`rustc_public/src/mir/body.rs:139-143`], and the body carries an
outer `span: Span` covering the whole function
[`rustc_public/src/mir/body.rs:36-37`]. The block itself has no
source-location field.

There is **no `BasicBlock` constructor or accessor**: the type is
plain field-init data. Compare to `rustc_middle`'s
`BasicBlockData::new` / `terminator()` / `terminator_mut()` /
`is_empty_unreachable()` / `mono_successors()` accessors at
[`rustc_middle/src/mir/mod.rs:1333-1413`] -- rustc_public exposes
none of these. Consumers reach successors through
`block.terminator.successors()` (the `Terminator` accessor)
[`rustc_public/src/mir/body.rs:145-149`].

### 1.1 Bridge translator: where `BasicBlockData` becomes `BasicBlock`

The `Stable<'tcx> for mir::Body<'tcx>` impl drives the per-block
translation [`rustc_public/src/unstable/convert/stable/mir.rs:15-49`]:

```rust
impl<'tcx> Stable<'tcx> for mir::Body<'tcx> {
    type T = crate::mir::Body;

    fn stable<'cx>(
        &self,
        tables: &mut Tables<'cx, BridgeTys>,
        cx: &CompilerCtxt<'cx, BridgeTys>,
    ) -> Self::T {
        crate::mir::Body::new(
            self.basic_blocks
                .iter()
                .map(|block| crate::mir::BasicBlock {
                    terminator: block.terminator().stable(tables, cx),
                    statements: block
                        .statements
                        .iter()
                        .map(|statement| statement.stable(tables, cx))
                        .collect(),
                })
                .collect(),
            ...
```

Five observations:

1. **`block.terminator()` is the call that collapses
   `Option<Terminator>` to `Terminator`.** That accessor is the one
   defined at [`rustc_middle/src/mir/mod.rs:1356-1358`] which
   `expect("invalid terminator state")`s on the inner `Option`. By
   convention the bridge runs over post-construction MIR
   (`optimized_mir`) where the Option is always `Some`. **INFERENCE:**
   rustc_public deliberately moves the "Option is None only during
   construction" invariant from a runtime `expect` into the type, by
   running the unwrap at exactly the bridge boundary.
2. **No `is_cleanup` read.** The translator does not branch on the
   field, does not store it in the produced `BasicBlock`, and does
   not gate any rejection on it. Cleanup blocks are translated like
   any other block; the slimming is purely structural.
3. **No `after_last_stmt_debuginfos` read.** Same silent drop. The
   bridge does not even iterate that field.
4. **`self.basic_blocks.iter()`** -- that is, `BasicBlocks<'tcx>`'s
   `Deref<Target = IndexSlice<BasicBlock, BasicBlockData<'tcx>>>`
   yielding `iter()` over the underlying `IndexVec`
   [`rustc_middle/src/mir/basic_blocks.rs:120-127`]. The bridge does
   **not** call `reverse_postorder()`. **INFERENCE:** the produced
   `Vec<BasicBlock>` preserves the upstream IndexVec index order
   verbatim; whether that order *is* RPO depends entirely on whether
   `ReorderBasicBlocks` ran upstream. rustc_public does not impose
   RPO at the boundary; it inherits whatever the source ordered.
5. **`crate::mir::Body::new(...)`** is invoked at the end. That
   constructor (`body.rs:47-62`) asserts only that
   `locals.len() > arg_count`; **it does not assert anything about
   `blocks`**. There is no minimum-block-count assert (e.g.,
   "non-empty"), no block-0-must-have-START_BLOCK-shape assert, no
   successor-in-range assert. Block-level invariants are entirely
   structural / responsibility of the producer.

---

## 2. `Body.blocks: Vec<BasicBlock>` -- bare flat container

The body-level basic-block table is a flat `Vec<BasicBlock>`
[`rustc_public/src/mir/body.rs:14-38`]:

```rust
#[derive(Clone, Debug, Serialize)]
pub struct Body {
    pub blocks: Vec<BasicBlock>,

    /// Declarations of locals within the function.
    pub(super) locals: LocalDecls,

    /// The number of arguments this function takes.
    pub(super) arg_count: usize,

    /// Debug information pertaining to user variables, including captures.
    pub var_debug_info: Vec<VarDebugInfo>,

    /// Mark an argument (which must be a tuple) as getting passed as
    /// its individual components.
    ///
    /// This is used for the "rust-call" ABI such as closures.
    pub(super) spread_arg: Option<Local>,

    /// The span that covers the entire function body.
    pub span: Span,
}
```

Three load-bearing differences from `rustc_middle::mir::BasicBlocks<'tcx>`:

1. **No wrapper struct.** rustc_middle wraps the storage as
   `pub struct BasicBlocks<'tcx> { basic_blocks: IndexVec<...>,
   cache: Arc<Cache> }` [`rustc_middle/src/mir/basic_blocks.rs:14-19`].
   rustc_public inlines the field as `pub blocks: Vec<BasicBlock>`
   directly inside `Body`. There is no `BasicBlocks` newtype, no
   `Deref` impl, no graph-trait impls.
2. **No CFG cache.** rustc_middle's `Cache` carries
   `predecessors: OnceLock<Predecessors>`,
   `reverse_postorder: OnceLock<Vec<BasicBlock>>`,
   `dominators: OnceLock<Dominators<BasicBlock>>`
   [`rustc_middle/src/mir/basic_blocks.rs:32-37`]. rustc_public
   carries none of these. No `predecessors()` accessor, no
   `dominators()` accessor, no `reverse_postorder()` accessor on
   `Body`. **HYPOTHESIS:** the cache is dropped because (a) it is
   skipped in encode anyway via the `Encodable for Cache` impl
   that writes nothing [`rustc_middle/src/mir/basic_blocks.rs:162-165`],
   so wire format gains nothing, and (b) `OnceLock` + `Arc` are
   not portable under the "owned + serde::Serialize" goal stated at
   `rustc_public/src/lib.rs:13-17`. The cache is "compute on demand
   from the storage"; consumers that need predecessors/dominators
   build them themselves from `block.terminator.successors()`
   [`rustc_public/src/mir/body.rs:145-149`].
3. **Bare `Vec<T>` storage**, not `IndexVec<Idx, T>`. The element
   type is `BasicBlock`; the index type is `BasicBlockIdx = usize`
   (see §3 below). rustc_public does not pull `IndexVec` into the
   public API; the standard `Vec` plus a `usize` alias is the public
   surface.

### 2.1 What rustc_public visitors expect of the body-level container

The visitor's `super_body!` macro destructures `Body` exhaustively
and iterates `blocks` linearly
[`rustc_public/src/mir/visit.rs:385-433`]:

```rust
($self:ident, $body:ident, ) => {
    let Body { blocks, locals: _, arg_count, var_debug_info, spread_arg: _, span } = $body;

    for bb in blocks {
        $self.visit_basic_block(bb);
    }
    ...
};
```

The iteration is forward over `blocks` (`Vec`'s natural order) with
no reachability filter, no reverse-postorder forcing, and no
predecessor lookup. `Successors = Vec<BasicBlockIdx>`
[`rustc_public/src/mir/body.rs:151`] is the only successor-discovery
primitive, exposed at the *terminator* level (`Terminator::successors`
[`body.rs:145-149`]) -- the body itself does not aggregate the CFG.

**INFERENCE:** rustc_public's design pattern is "container is
storage; CFG analyses are recipes". The wrapper struct + cache
pattern lives in `rustc_middle` because rustc itself is doing those
analyses dozens of times per body and benefits from amortization;
rustc_public consumers run a translation pass once (or a handful of
times) and rebuild on demand if needed.

---

## 3. `BasicBlockIdx = usize`: bare integer, no newtype

```rust
pub type BasicBlockIdx = usize;
```

[`rustc_public/src/mir/body.rs:40`]

This is a `pub type` alias to `usize` -- not a newtype, not an
`Idx`-trait wrapper, not phantom-tagged. Compare with rustc_middle's
`pub struct BasicBlock { const START_BLOCK = 0; }` newtype-index
wrapper [`rustc_middle/src/mir/mod.rs:1262-1291`] which carries a
`#[debug_format = "bb{}"]` attribute, an `#[orderable]` attribute,
an `#[encodable]` attribute, and a `start_location()` method
[`rustc_middle/src/mir/mod.rs:1294-1298`].

`BasicBlockIdx` appears at every call-site that names an index into
the block table:

- `Successors = Vec<BasicBlockIdx>` (terminator successor list)
  [`body.rs:151`].
- `TerminatorKind::Goto { target: BasicBlockIdx }` [`body.rs:155-157`].
- `TerminatorKind::Drop { ..., target: BasicBlockIdx, ... }`
  [`body.rs:166-170`].
- `TerminatorKind::Call { ..., target: Option<BasicBlockIdx>, ... }`
  [`body.rs:171-177`].
- `TerminatorKind::Assert { ..., target: BasicBlockIdx, ... }`
  [`body.rs:178-184`].
- `TerminatorKind::InlineAsm { ..., destination: Option<BasicBlockIdx>, ... }`
  [`body.rs:185-192`].
- `UnwindAction::Cleanup(BasicBlockIdx)` [`body.rs:257`].
- `SwitchTargets::branches: Vec<(u128, BasicBlockIdx)>`,
  `SwitchTargets::otherwise: BasicBlockIdx` [`body.rs:877-880,890-895`].

**Trade-off observation.** Bare `usize` has zero runtime cost and
zero size overhead, but loses three properties that newtype indices
buy:

1. **Type discipline** -- you cannot accidentally pass a `LocalIdx`
   where a `BasicBlockIdx` is expected; the compiler refuses.
   rustc_public has both `Local` (also a type alias to a numeric
   index, see [`rustc_public/src/mir/body.rs` -- `Local` declaration])
   and `BasicBlockIdx`, both as `usize` aliases, so this discipline
   is sacrificed.
2. **Built-in pretty-printing sigil** -- `bb{N}` is a debug-format
   property of the rustc_middle type
   [`rustc_middle/src/mir/mod.rs:1288`]; with a bare `usize` you get
   plain integer formatting and the renderer must inject the prefix
   manually (rustc_public does this in its pretty-printer; see §6
   below).
3. **`Option<BasicBlockIdx>` niche optimisation** -- a `usize` has
   no niche, so `Option<BasicBlockIdx> = Option<usize>` is two
   words on a 64-bit target. rustc_middle's `BasicBlock` newtype
   wraps a `u32` and reserves a niche, giving a 4-byte
   `Option<BasicBlock>` (this is the optimisation stage 10's
   inbound expectation references for `Option<BlockId>`).
   **HYPOTHESIS:** rustc_public chose serializability and "looks
   like a `usize` to third-party Python/JS tools" over the niche
   optimisation; cf. the crates.io publication goal at
   [`rustc_public/src/lib.rs:13-17`].

**INFERENCE for D-10-3.** RegseqModel keeps the rustc_middle
discipline (`BlockId` newtype with `START_BLOCK = 0`, `bbN` debug
format, niched `Option<BlockId>`) per stage 1's typed-ID convention
-- not because rustc_public is wrong, but because RegseqModel's
audience is the bridge + backend consumers within the same crate
graph (where Rust-level type discipline and 4-byte niching matter),
not third-party multi-language tools (where serde-flat `usize` is
friendlier). The `newtype_index!` const-body macro arm and the
4-byte size-of assertion both fall out of this choice.

---

## 4. Block ordering: rustc_public preserves upstream IndexVec order

The bridge translation iterates `self.basic_blocks.iter()` and
`.collect()`s into a `Vec<BasicBlock>`
[`rustc_public/src/unstable/convert/stable/mir.rs:24-34`]:

```rust
self.basic_blocks
    .iter()
    .map(|block| crate::mir::BasicBlock { ... })
    .collect()
```

The `iter()` is the one inherited from
`Deref<Target = IndexSlice<BasicBlock, BasicBlockData<'tcx>>>`
[`rustc_middle/src/mir/basic_blocks.rs:120-127`], which iterates the
underlying `IndexVec` in `BasicBlock`-index order: block 0 first,
block 1 second, etc. This is **not** `reverse_postorder()`.

Three consequences for D-10-4:

1. **rustc_public makes no RPO guarantee at its own surface**. The
   produced `Vec<BasicBlock>`'s element-i is upstream block-i; if
   upstream had been reordered (e.g., by `ReorderBasicBlocks`
   [`rustc_mir_transform/src/prettify.rs:14-42`]), the element order
   inherits that. If upstream had not been reordered, the `Vec`
   reflects whatever rustc's MIR builder happened to assign.
2. **The block-0 = entry invariant is preserved**. rustc_middle's
   `START_BLOCK` is `BasicBlock(0)`
   [`rustc_middle/src/mir/mod.rs:1289-1291`] and the `permute` step
   in `ReorderBasicBlocks` `debug_assert_eq!`s
   `updater.map[START_BLOCK] == START_BLOCK`
   [`rustc_mir_transform/src/prettify.rs:33`], so block 0 stays
   block 0 across reorderings. By transitivity through the bridge's
   index-preserving iteration, **rustc_public's `body.blocks[0]` is
   always the function entry** -- but as an inherited consequence,
   not a rustc_public-enforced invariant.
3. **No `mono_reachable_*` call.** rustc_codegen's per-body walk
   uses `traversal::mono_reachable_reverse_postorder(...)`
   [`rustc_middle/src/mir/traversal.rs:228-240`] to skip blocks
   monomorphized-out by `if const { false }` etc.; rustc_public's
   bridge does not. All upstream blocks are translated, including
   any that are unreachable post-monomorphization.
   **INFERENCE:** rustc_public's audience reads the structural body
   first and does its own reachability when needed; the translation
   layer is structural, not analytical.

**For D-10-4 in RegseqModel.** The `ReorderBasicBlocks` pass IS in
the regseq-driver cleanup-pass whitelist
[`crates/regseq-driver/src/callbacks.rs:25-36`]; the bridge will
read MIR that has *already* been RPO-reordered. So the pattern
rustc_public uses (preserve IndexVec order verbatim) yields RPO at
the RegseqModel boundary -- inherited, not enforced. The
spec-side question is whether to **state RPO as a positive
invariant** (so consumers / regseq-analysis can rely on it) or
**leave it implicit** (matching rustc_public's stance). rustc_public
chose implicit; RegseqModel's choice is open.

---

## 5. What `Body::new` enforces -- and what it doesn't

```rust
pub fn new(
    blocks: Vec<BasicBlock>,
    locals: LocalDecls,
    arg_count: usize,
    var_debug_info: Vec<VarDebugInfo>,
    spread_arg: Option<Local>,
    span: Span,
) -> Self {
    assert!(
        locals.len() > arg_count,
        "A Body must contain at least a local for the return value and each of the function's arguments"
    );
    Self { blocks, locals, arg_count, var_debug_info, spread_arg, span }
}
```

[`rustc_public/src/mir/body.rs:47-62`]

A constructor is only required because two of `Body`'s fields
(`locals` and `arg_count`) are private (`pub(super)`). The
`assert!` is **strictly about local-table well-formedness**; nothing
about `blocks`.

No assert is present for any of the following:

- `!blocks.is_empty()` -- a body could have zero blocks.
- `blocks[0]` is the entry block (no `START_BLOCK` shape check).
- `blocks[i].terminator.successors()` are all in `0..blocks.len()`
  (no in-range successor check).
- Reachability from `blocks[0]` covers all of `blocks` (no
  unreachable-block prohibition).
- Last block ends in `Return` / `Unreachable` / `Abort` (no terminal
  shape check).

**INFERENCE for D-10-5.** rustc_public's invariants registry is
"none beyond locals." Block-level invariants are the responsibility
of the producer (the bridge). The producer is in turn responsible
because the upstream `rustc_middle::mir::BasicBlockData` /
`mir::Body` already enforce these invariants at the rustc level
(rustc's MIR validator
`rustc_mir_transform/src/validate.rs` runs as a pass; producers
that bypass it produce invalid MIR which reaches the bridge
unwell -- but rustc_public takes "well-formed input" for granted).

For RegseqModel the equivalent question is: **does the bridge
produce already-validated content, or does the spec state
positive invariants the consumer can rely on?** rustc_public says
"producer's responsibility, no spec invariants". RegseqModel's
prior stages (5-9) chose the opposite: positive invariants
listed in a §N.M registry plus a rejection table. Stage 10 should
follow the stages-5-9 pattern (positive registry) rather than the
rustc_public minimalism, because the spec is the consumer
contract, not just a translation tax.

### 5.1 Where rustc_public *implicitly* enforces structure

Two structural invariants are enforced by Rust's type system, not
by an `assert!`:

1. **`terminator: Terminator` is non-Optional** at the type level
   [`body.rs:135-137`]. Pattern-matching consumers cannot accidentally
   forget to handle the construction-time-`None` case because that
   case has been eliminated structurally.
2. **`statements: Vec<Statement>` is exhaustively destructurable**.
   Visitor consumers see the closed two-field
   `BasicBlock { statements, terminator }` shape
   [`visit.rs:139-145`]; adding a field would be a breaking change
   surfaced at every match site. **INFERENCE:** this is the
   "pattern-matching exhaustiveness as documentation" idiom -- the
   visitor convention preamble at [`visit.rs:31-35`] makes it
   explicit.

---

## 6. Pretty-printer: `bb{N}` sigil rendering

`function_body` in the rustc_public pretty-printer renders the
body's blocks with the `bb{index}` sigil
[`rustc_public/src/mir/pretty.rs:38-88`]:

```rust
pub(crate) fn function_body<W: Write>(writer: &mut W, body: &Body, name: &str) -> io::Result<()> {
    write!(writer, "fn {name}(")?;
    ...
    body.blocks
        .iter()
        .enumerate()
        .map(|(index, block)| -> io::Result<()> {
            writeln!(writer, "    bb{index}: {{")?;
            let _ = block
                .statements
                .iter()
                .map(|statement| -> io::Result<()> {
                    pretty_statement(writer, &statement.kind)?;
                    Ok(())
                })
                .collect::<Vec<_>>();
            pretty_terminator(writer, &block.terminator.kind)?;
            writeln!(writer, "    }}").unwrap();
            Ok(())
        })
        .collect::<Result<Vec<_>, _>>()?;
    writeln!(writer, "}}")?;
    Ok(())
}
```

Three observations:

1. **Sigil is injected manually**: `writeln!(writer, "    bb{index}: {{")`.
   Because `BasicBlockIdx` is bare `usize`, `Display` / `Debug` on
   the index alone yields `0`, `1`, `2`, ... -- the renderer
   prepends `bb` itself. Compare with rustc_middle where the sigil
   is the type's own debug format
   [`rustc_middle/src/mir/mod.rs:1288`] and any `format!("{:?}",
   bb)` call yields `bb0`, `bb1`, ... directly.
2. **Iteration order is `body.blocks.iter().enumerate()`** -- the
   `Vec` order, not RPO. The rendered file thus reflects whatever
   ordering reached rustc_public from upstream.
3. **Per-block layout**: indent `    `, `bbN: {`, then each
   statement on its own line, then the terminator, then `    }`.
   No prefix annotations, no `(cleanup)` tag (rustc_middle emits
   `(cleanup)` after the block label when `is_cleanup: true` -- see
   `rustc_middle/src/mir/pretty.rs`); rustc_public has no such tag
   because the field was dropped.

**INFERENCE for D-10-K.** RegseqModel's pretty-printer can pick
either approach: (a) put `bb{N}` in the `BlockId` newtype's
`#[debug_format = "bb{}"]` attribute (matching rustc_middle and
making `format!("{:?}", id)` yield the sigil directly), or (b) keep
the sigil at the renderer level (matching rustc_public and the
existing `pretty.rs` scaffold pattern at
`crates/regseq-model/src/pretty.rs:29` which already documents
`BlockId` -> `bbN` as a per-renderer rule). The stage-1 inbound
expectation explicitly mentions adding `BlockId`-rendering to
`pretty.rs`, suggesting the renderer-level approach.

---

## 7. Summary mapping rustc_public evidence to D-10 tags

| D-tag | rustc_public says | Implication for RegseqModel |
|---|---|---|
| **D-10-1** | Two-field `BasicBlock { statements: Vec<Statement>, terminator: Terminator }` [`body.rs:133-137`]. `is_cleanup` and `after_last_stmt_debuginfos` silently dropped at the bridge [`stable/mir.rs:24-33`]. `terminator: Option` collapsed to non-optional via `.terminator()` accessor at the bridge [`stable/mir.rs:27`]. No `BasicBlock`-level span. Fields are `pub` and visitor-exhaustive. | Mirror the slimming -- two fields, terminator non-Optional, no is_cleanup, no debuginfo, no block-level span. Stage 10 is owning the formalisation of what stage 9 already previewed at `spec.md:8448-8453`. |
| **D-10-2** | `Body.blocks: Vec<BasicBlock>` -- bare flat container, no wrapper [`body.rs:14-38`]. **No CFG cache** (no predecessors / RPO / dominators methods on `Body`) -- consumers compute on demand from `Terminator::successors` [`body.rs:145-151`]. | The cache-vs-no-cache choice tilts toward no-cache: rustc_public ships zero CFG cache and the regseq-analysis crate (per Q1 architectural split) is the natural home for predecessor/dominator computation. Storage-type choice tilts toward `IndexVec<BlockId, BasicBlock>` (typed-ID discipline RegseqModel keeps) rather than rustc_public's bare `Vec`. |
| **D-10-3** | `BasicBlockIdx = usize` -- bare type alias, no newtype [`body.rs:40`]. Trade-off: serializability + crates.io-publishability; **loses** type discipline, `bb{}` debug format, and `Option<...>` niche. | RegseqModel keeps the rustc_middle pattern (typed `BlockId` newtype, `START_BLOCK = 0` const, niched `Option<BlockId>`, `bb{}` debug format) per stage 1's typed-ID convention -- audience is the in-tree bridge + backend, not third-party tools. The `newtype_index!` const-body arm extension and the 4-byte size-of assertion both fall out of this. |
| **D-10-4** | Translator preserves upstream `IndexVec` index order via `self.basic_blocks.iter()` [`stable/mir.rs:24`]. Does **not** call `reverse_postorder()`. RPO at the rustc_public surface is *inherited* from upstream, not *enforced*. Block-0-as-entry is preserved by transitivity through `START_BLOCK` permutation invariant [`prettify.rs:33`]. | RegseqModel's bridge runs after `ReorderBasicBlocks` (pinned in the cleanup whitelist at `callbacks.rs:25-36`), so RPO holds at the boundary by inheritance. Spec choice: state RPO as a positive invariant (the stages-5-9 pattern) rather than leave it implicit (rustc_public's pattern). The reasons are consumer-contract clarity and analysis-side exploitability. |
| **D-10-5** | `Body::new` asserts only `locals.len() > arg_count` [`body.rs:47-62`]. **No block-level invariants enforced**: no min-block-count, no entry-block shape, no successor-in-range, no reachability, no terminal shape. Structural invariants (terminator-non-Optional, two-field exhaustiveness) live in the type system instead. | rustc_public sets the floor -- "type-level non-Optional terminator + visitor exhaustiveness". Stage 10 should add a positive registry on top (RPO ordering, START_BLOCK identity, successors in `0..n_blocks`, all-blocks-stored regardless of post-mono reachability) plus a rejection table for the slimmed-away rustc fields, per the stages-5-9 precedent. |
| **D-10-K** | Pretty-printer renders `bb{index}: {` per block via `body.blocks.iter().enumerate()` [`pretty.rs:38-88`]. Sigil is renderer-injected because `BasicBlockIdx` is bare `usize`. Block layout: indent + label + statements + terminator + close-brace. No `(cleanup)` annotation (the field was dropped). | RegseqModel can either (a) bake `bb{}` into the `BlockId` newtype's `debug_format` attribute (rustc_middle pattern) so any `{:?}` yields the sigil, or (b) inject the sigil at `render_basic_block` time (rustc_public + stage-1 `pretty.rs` scaffold pattern). The stage-1 inbound expectation reads as renderer-level. The body-block walk shape (forward iteration over storage) is the same either way. |
