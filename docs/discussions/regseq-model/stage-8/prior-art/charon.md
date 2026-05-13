# Stage 8 -- Prior Art: Charon primary source

Primary-source Charon (ULLBC / LLBC) evidence for `StatementKind`,
its `Statement` wrapper, the rustc-MIR -> Charon translation
behaviour governing what gets carried as a Statement vs. lifted to
a Terminator vs. dropped, and the post-translation reshaping
passes touching statement-level constructs. Scoped to stage 8's
decision namespace: D-8-1 variant set + payload shapes (D-8-1.1
cardinality, D-8-1.2 Assign payload, D-8-1.3 SetDiscriminant
placement, D-8-1.4 intrinsic placement); D-8-2 statement interning
vs inline storage; D-8-3 `Statement` wrapper struct; D-8-4
statement-site invariants + rejection rules; D-8-K
renderer/pretty-printer. Fresh pass per `../../README.md` §2
methodology rule.

Charon is INRIA's Rust front-end translator feeding the Aeneas
verification pipeline. It lowers rustc MIR (via the `hax`
extraction layer) into LLBC ("Low-Level Borrow Calculus") and the
basic-block-shaped predecessor ULLBC ("Unstructured LLBC"). LLBC
is the headline IR (Ho & Protzenko, ICFP 2022, arxiv 2206.07185).
**For stage 8 the relevant layer is ULLBC** -- direct rustc-MIR
translation target, basic-block-shaped, structurally analogous to
RegseqModel's bridge output. LLBC is informational: its
`StatementKind` is fused Statement+Terminator after CFG
reconstruction.

Repository: `https://github.com/AeneasVerif/charon` (main).
Local clone pinned at commit
`46ab49f627e1a8e72b17c437f05ae4d1c4c456df` ("Merge pull request
#919 from Nadrieril/raw-consts").

Citation convention: every factual claim cites `[file:line]` in
the Charon source tree at this commit, paths relative to
`/Users/nyengele/Documents/sekekama/charon-1/charon/`.
`HYPOTHESIS:` flags speculation. `INFERENCE:` flags cross-site
synthesis.

Stage 8 framing: **Statement** (RegseqModel) is the per-instruction
record of a non-control-flow basic-block operation -- analogous to
rustc_middle's `Statement<'tcx>` + its `kind: StatementKind<'tcx>`.
RegseqModel's closed-surface candidate is a 3-variant
`Statement::{Assign, SetDiscriminant, Intrinsic}` with no storage
markers, no fake reads, no ascriptions. Charon is the most
relevant non-rustc peer: (a) `'tcx`-free, (b) Serde-derived for
verification consumers, (c) consumes rustc MIR via hax, (d) has
had to make the same "what counts as a Statement, what gets
hoisted, what gets dropped" choices.

Primary files inspected:

- `src/ast/ullbc_ast.rs` -- ULLBC IR types (`StatementKind`,
  `Statement`, `TerminatorKind`, `BlockData`, `BodyContents`).
- `src/ast/llbc_ast.rs` -- LLBC IR types (informational).
- `src/ast/gast.rs` -- shared shapes: `CopyNonOverlapping`,
  `Assert`, `AbortKind`, `DropKind`, `Body`, `GExprBody`, `Locals`.
- `src/ast/ullbc_ast_utils.rs` -- `Statement::new`, `BodyBuilder`.
- `src/ast/llbc_ast_utils.rs` -- LLBC `Statement::new`.
- `src/bin/charon-driver/translate/translate_bodies.rs` -- rustc-MIR
  -> ULLBC body translator (`translate_statement`,
  `translate_terminator`, `translate_basic_block`, `translate_body`).
- `src/transform/finish_translation/insert_storage_lives.rs` --
  materialises StorageLive for unannotated locals.
- `src/transform/finish_translation/insert_assign_return_unit.rs`
  -- synthesises Assign(return_place, unit) for unit-returning fns.
- `src/transform/simplify_output/simplify_constants.rs` --
  factors non-Literal constants out of operand position.
- `src/transform/simplify_output/remove_nops.rs` -- removes Nop.
- `src/transform/simplify_output/ops_to_function_calls.rs` --
  reshapes some Assign rvalue patterns into function calls.
- `src/transform/resugar/reconstruct_asserts.rs` -- folds
  panic-branch idiom into `StatementKind::Assert`.
- `src/transform/resugar/reconstruct_fallible_operations.rs` --
  removes rustc dynamic-check Asserts, rewires BinOps.
- `src/transform/resugar/reconstruct_intrinsics.rs` -- desugars
  `offset_of` calls into `Assign(_, NullaryOp(OffsetOf, ...))`.
- `src/transform/normalize/desugar_drops.rs` -- rewrites Drop
  terminators into Call terminators.
- `src/transform/control_flow/ullbc_to_llbc.rs` -- ULLBC->LLBC
  bridge.
- `src/pretty/fmt_with_ctx.rs` -- `FmtWithCtx` impls for both
  ULLBC and LLBC `Statement`s.

---

## 1. ULLBC `StatementKind` enum shape

Charon does **not** name the raw statement type `RawStatement`.
The convention is `StatementKind` -- mirroring rustc_middle.
Comments call it a "raw statement"
[`src/ast/ullbc_ast.rs:18-47`]:

```rust
/// A raw statement: a statement without meta data.
#[derive(Debug, Clone, EnumIsA, EnumAsGetters, VariantName,
         Serialize, Deserialize, Drive, DriveMut)]
pub enum StatementKind {
    Assign(Place, Rvalue),
    SetDiscriminant(Place, VariantId),
    /// Equivalent to std::intrinsics::copy_nonoverlapping; this is not modelled as a function
    /// call as it cannot diverge
    CopyNonOverlapping(Box<CopyNonOverlapping>),
    StorageLive(LocalId),
    StorageDead(LocalId),
    Deinit(Place),
    /// A built-in assert, which corresponds to runtime checks that we remove, namely: bounds
    /// checks, over/underflow checks, div/rem by zero checks, pointer alignement check.
    Assert(Assert),
    /// Does nothing. Useful for passes.
    Nop,
    #[charon::opaque] #[drive(skip)]
    Error(String),
}
```

Cardinality: **9 variants** -- `Assign`, `SetDiscriminant`,
`CopyNonOverlapping`, `StorageLive`, `StorageDead`, `Deinit`,
`Assert`, `Nop`, `Error`. RegseqModel's 3-variant candidate is
`{Assign, SetDiscriminant, Intrinsic}`. Differences:

- Charon **fans out** rustc's `Intrinsic` into dedicated variants
  (`CopyNonOverlapping`) and absorbs `Intrinsic::Assume` into
  `Assert` (§5).
- Charon **keeps** rustc's `StorageLive` / `StorageDead` /
  `Deinit` / `Nop` as Statement variants.
- Charon **adds** `Error(String)` for translator failure recovery
  -- a Charon-specific error-as-IR-node, no analog in RegseqModel
  or rustc.
- Charon **adds** `Assert(Assert)` as a Statement (not Terminator),
  absorbing both `Assume` intrinsic and `TerminatorKind::Assert`
  via translation-time rewrite (§7).

Absences from rustc_middle's `StatementKind<'tcx>` (rustc's
pinned-nightly variants: `Assign`, `FakeRead`, `SetDiscriminant`,
`Deinit`, `StorageLive`, `StorageDead`, `Retag`, `PlaceMention`,
`AscribeUserType`, `Coverage`, `Intrinsic`, `ConstEvalCounter`,
`Nop`, `BackwardIncompatibleDropHint`): Charon drops
`FakeRead` [`translate_bodies.rs:827`], `Retag`
[`translate_bodies.rs:824`], `PlaceMention`
[`translate_bodies.rs:827`], `AscribeUserType`
[`translate_bodies.rs:831`], `Coverage`
[`translate_bodies.rs:833`], `ConstEvalCounter`
[`translate_bodies.rs:836`], `BackwardIncompatibleDropHint`
[`translate_bodies.rs:838`], and rustc's `Nop`
[`translate_bodies.rs:839`] -- all return `None` so
`translate_basic_block` doesn't push them. Charon also has no
generic `Intrinsic` variant -- the two `NonDivergingIntrinsic`
arms get separate destinations.

No `static_assert_size!` on `StatementKind`. Charon is unconcerned
with the 16-byte budget rustc enforces. INFERENCE: serde-derived
on-disk form makes the variant set the wire format -- adding a
variant breaks Aeneas deserialisation. RegseqModel carries the
same concern via its serde JSON snapshot tests (Q5).

Maps to `D-8-1.1` (cardinality: Charon 9 vs RegseqModel candidate
3), `D-8-1` (variant set + payload shapes).

---

## 2. `Statement` wrapper struct

Charon wraps the raw kind in a per-statement record
[`src/ast/ullbc_ast.rs:49-57`]:

```rust
pub struct Statement {
    pub span: Span,
    pub kind: StatementKind,
    /// Comments that precede this statement.
    // This is filled in a late pass after all the control-flow manipulation.
    #[drive(skip)]
    pub comments_before: Vec<String>,
}
```

Three fields:

- `span: Span` -- owned, serde-serialisable
  `{data: SpanData, generated_from_span: Option<SpanData>}` pair
  [`src/ast/meta.rs:60-83`] that separately tracks macro call-site
  vs macro definition-site. Charon resolves rustc's interned
  `Span` into this owned shape at translation time.
- `kind: StatementKind` -- discriminator + payload.
- `comments_before: Vec<String>` -- comments scraped from source
  text by `translate_body_comments`
  [`src/bin/charon-driver/translate/translate_bodies.rs:1189-1228`];
  finalised in a late `recover_body_comments` pass.

Construction is via a single helper
[`src/ast/ullbc_ast_utils.rs:24-32`]:

```rust
impl Statement {
    pub fn new(span: Span, kind: StatementKind) -> Self {
        Statement { span, kind, comments_before: vec![] }
    }
}
```

LLBC's `Statement` adds one field for in-IR identity
[`src/ast/llbc_ast.rs:74-86`]:

```rust
pub struct Statement {
    pub span: Span,
    #[charon::rename("statement_id")]
    pub id: StatementId,
    pub kind: StatementKind,
    #[drive(skip)] pub comments_before: Vec<String>,
}
```

ULLBC has no `id` field -- statements are positional within
`BlockData.statements: Vec<Statement>`, so `(BlockId, usize)` is
implicit identity. LLBC's restructured control flow scatters
statements across nested `Block`s, where positional identity
breaks down -- hence the dense-integer `StatementId`.

Stage 8 implications for D-8-3:

- **Charon wraps `StatementKind` in `Statement`** with span and
  comments distinct from the raw kind. Direct precedent for
  RegseqModel's wrapper-struct candidate over flat-enum.
- **Span is separately-allocated owned, not interned.** Parallel
  to RegseqModel's stage-1 `Span` design.
- **`comments_before: Vec<String>` is novel** to Charon, motivated
  by Aeneas's source-fidelity needs. HYPOTHESIS: stage 11/12 may
  encounter parallel needs for C codegen comment preservation.
- **Wrapper is `Box`-free.** `Statement` lives inline in
  `Vec<Statement>`. The only Box inside is on the
  `CopyNonOverlapping(Box<CopyNonOverlapping>)` payload
  [`ullbc_ast.rs:28`] for size reasons.

Maps to `D-8-3` (wrapper struct: span + kind + comments),
`D-8-2` (inline `Vec<Statement>`, no interning -- see §11).

---

## 3. `Assign(Place, Rvalue)` -- the analog of rustc's primary statement

`Assign` is a 2-tuple variant
[`src/ast/ullbc_ast.rs:23`]:

```rust
Assign(Place, Rvalue),
```

Translation [`src/bin/charon-driver/translate/translate_bodies.rs:783-787`]:

```rust
hax::StatementKind::Assign((place, rvalue)) => {
    let t_place = self.translate_place(span, place)?;
    let t_rvalue = self.translate_rvalue(span, rvalue, t_place.ty())?;
    Some(StatementKind::Assign(t_place, t_rvalue))
}
```

Key shape choices:

- **Tuple variant, not struct variant. No `Box<(Place, Rvalue)>`.**
  Inline storage. `Rvalue` alone has 11 deep-payload variants
  (stage-7 charon.md §1) but Charon doesn't box -- size budget
  isn't enforced.
- **No type-equality check at construction.** Translation passes
  `t_place.ty()` *into* `translate_rvalue` so the rvalue is built
  knowing the destination type; no `assert_eq!(place.ty(),
  rvalue.ty())` post-construction.
- **Type direction is into-rvalue, not check-after-assignment.**
  INFERENCE: Charon resolves type-driven rvalue choices (e.g.,
  reconstructing unsize-cast metadata, distinguishing slice from
  array references) using the destination type. RegseqModel's
  stage-3 type system parallels: place-typing is source of truth,
  rvalue type derived. Direct prior art for `Assign` payload
  semantics.
- **No type-ascription channel.** rustc's `AscribeUserType` is
  dropped wholesale (§10).

The Box absence is rustc-budget-artefact evidence: rustc boxes
`Assign` for the `static_assert_size!(StatementKind<'_>, 16)`
budget; Charon stores the pair inline because no budget is
enforced. Stage 8 should treat this as evidence that the rustc
Box is a size-budget artefact, not a load-bearing semantic.

Maps to `D-8-1.2` (Assign payload: flat tuple, no Box, no
in-Statement type-equality), `D-8-1.1` (same payload as rustc's
`Assign(Box<(Place, Rvalue)>)` minus the Box).

---

## 4. `SetDiscriminant(Place, VariantId)` -- the analog of rustc's tag-write

`SetDiscriminant` is a 2-tuple variant
[`src/ast/ullbc_ast.rs:25`]:

```rust
/// A call. For now, we don't support dynamic calls (i.e. to a function pointer in memory).
SetDiscriminant(Place, VariantId),
```

(The doc comment "A call. For now, we don't support dynamic
calls" is stale -- it doesn't describe `SetDiscriminant`. This is
a Charon source-comment bug, observable at the cited line.)

Translation [`translate_bodies.rs:788-795`]: direct one-to-one
mapping from rustc's `StatementKind::SetDiscriminant {place,
variant_index}`:

```rust
hax::StatementKind::SetDiscriminant { place, variant_index } => {
    let t_place = self.translate_place(span, place)?;
    let variant_id = translate_variant_id(*variant_index);
    Some(StatementKind::SetDiscriminant(t_place, variant_id))
}
```

Key shape choices:

- **Kept as a Statement variant.** Not lifted to an Rvalue (the
  read-side `Rvalue::Discriminant(Place)` is the parallel; see
  stage-7 charon.md §8), not lowered to a tag-write
  `Assign(field_proj, const)`, not turned into an intrinsic call.
- **Tuple variant**, not struct variant; both `Place` and
  `VariantId` inline.
- **`VariantId` is a type-table index.** `translate_variant_id`
  resolves `mir::VariantIdx` to Charon's table-indexed form.
- **LLBC carries the comment "Not used today because we take MIR
  built."** [`src/ast/llbc_ast.rs:25-26`]. INFERENCE: Charon's hax
  MIR phase rarely emits `SetDiscriminant` because closer-to-source
  phases use `Assign(_, Aggregate(...))` for full construction.
  RegseqModel reads `optimized_mir` (post-runtime lowering) where
  `SetDiscriminant` is a real surface -- the bridge will see it.

Maps to `D-8-1.3` (SetDiscriminant kept as Statement, 2-tuple
shape identical to rustc), `D-8-1.1`.

---

## 5. Intrinsic handling: how Charon dispatches `mir::StatementKind::Intrinsic`

rustc's `StatementKind::Intrinsic(Box<NonDivergingIntrinsic>)`
carries two arms: `Assume(Operand)` and `CopyNonOverlapping(...)`.
Charon **fans these into two distinct Statement variants** rather
than carrying a generic `Intrinsic(...)` variant.

### 5.1 `Assume` -> `Assert(UB)`

Translation [`translate_bodies.rs:805-812`]:

```rust
// This asserts the operand true on pain of UB. We treat it like a normal assertion.
hax::StatementKind::Intrinsic(hax::NonDivergingIntrinsic::Assume(op)) => {
    let op = self.translate_operand(span, op)?;
    Some(StatementKind::Assert(Assert {
        cond: op,
        expected: true,
        on_failure: AbortKind::UndefinedBehavior,
    }))
}
```

`Assert` is a 3-field struct [`src/ast/gast.rs:457-466`]:

```rust
pub struct Assert {
    pub cond: Operand,
    #[drive(skip)] pub expected: bool,
    pub on_failure: AbortKind,
}
```

`AbortKind` is 3-variant `Panic(Option<Name>) | UndefinedBehavior
| UnwindTerminate` [`src/ast/gast.rs:411-419`]. INFERENCE:
`Assume` becomes a UB-flavoured Assert -- the same Statement
variant used by `TerminatorKind::Assert` lowering (§7) and
panic-branch resugaring (§8.1), distinguished only by
`on_failure`. This is a *lowering* not a rejection.

### 5.2 `CopyNonOverlapping` -> dedicated variant

Translation [`translate_bodies.rs:813-822`]:

```rust
hax::StatementKind::Intrinsic(hax::NonDivergingIntrinsic::CopyNonOverlapping(
    hax::CopyNonOverlapping { src, dst, count },
)) => {
    ...
    Some(StatementKind::CopyNonOverlapping(Box::new(
        CopyNonOverlapping { src, dst, count },
    )))
}
```

`CopyNonOverlapping` is a 3-field struct of `Operand`s
[`src/ast/gast.rs:400-405`]. Choices:

- **Dedicated `StatementKind` variant**, not a generic
  `Intrinsic(name, args)`.
- **Boxed payload** (3 `Operand`s would inflate the enum). This
  is Charon's only Box in `StatementKind`, purely
  size-amortisation.
- **Doc justifies the placement**: "not modelled as a function
  call as it cannot diverge" [`src/ast/ullbc_ast.rs:27-28`].

### 5.3 Implications for D-8-1.4

Charon has no generic `Intrinsic(...)` Statement variant. Every
non-diverging intrinsic is given a named Statement variant or
absorbed into an existing variant. Trade-off vs. RegseqModel's
3-variant `Intrinsic(IntrinsicKind, Vec<Operand>)` candidate:
Charon's per-intrinsic-variant shape gives type-safe payloads
per intrinsic; RegseqModel's flat shape gives uniform dispatch
over an extensible kind tag. INFERENCE: Charon's choice is
verification-driven (Aeneas wants per-intrinsic match arms with
typed payloads); RegseqModel's C-codegen target may prefer the
flat dispatch shape because codegen's per-intrinsic logic is
kind-tag-driven anyway.

Maps to `D-8-1.4` (intrinsic placement -- Charon picks
per-intrinsic Statement variants, not a generic Intrinsic).

---

## 6. `StorageLive`, `StorageDead`, `Deinit`, `Nop`, `Error`

Charon **keeps storage markers and `Nop`** as Statement variants
where RegseqModel's 3-variant candidate plans to drop them.

### 6.1 `StorageLive(LocalId)` / `StorageDead(LocalId)`

1-tuple variants over `LocalId` [`src/ast/ullbc_ast.rs:33,37`].
Direct one-to-one translation [`translate_bodies.rs:796-803`].
Furthermore, the `insert_storage_lives` finish-translation pass
[`src/transform/finish_translation/insert_storage_lives.rs:80-110`]
*adds* `StorageLive` statements at body entry for locals that the
upstream MIR did not annotate (rustc considers "always-initialised"
locals exempt):

```rust
let new_statements = locals_with_missing_storage.map(|local| {
    ullbc_ast::Statement::new(first_span,
        ullbc_ast::StatementKind::StorageLive(local))
});
first_block.statements.splice(0..0, new_statements);
```

INFERENCE: Charon's verification semantics rely on per-local
allocation tracking, so keeping (and completing) the storage-marker
stream is load-bearing for Aeneas. RegseqModel targets C codegen
where `auto` storage duration is implicit -- storage markers carry
no information that survives lowering. Stage 8's "drop storage
markers" candidate is C-codegen-target-consistent; Charon's
preserve-them choice is verification-target-specific.

### 6.2 `Deinit(Place)`

[`src/ast/ullbc_ast.rs:38`]: variant declared but **not
translated from rustc**. The `match &*statement.kind` at
`translate_bodies.rs:782-840` exhausts `hax::StatementKind`
without a `Deinit` arm; the variant is only referenced in
pretty-print and pass machinery
[`src/pretty/fmt_with_ctx.rs:1422,1465`,
`src/transform/control_flow/ullbc_to_llbc.rs:1439`]. HYPOTHESIS:
Charon's hax MIR phase doesn't surface `Deinit` (it appears only
in later MIR phases hax doesn't read), so the variant is
theoretically reachable but operationally dead. RegseqModel's
3-variant candidate has no Deinit -- consistent with optimized_mir
also not delivering it (it's a pre-cleanup artefact).

### 6.3 `Nop`

[`src/ast/ullbc_ast.rs:43`]: rustc-side `Nop` is **dropped at
translation** (returns `None` at `translate_bodies.rs:839`).
Charon's `Nop` exists as **pass-time tombstone**: the
`reconstruct_fallible_operations` pass overwrites removed
statements with `Nop` in-place
[`src/transform/resugar/reconstruct_fallible_operations.rs:471-475`]:

```rust
for i in 0..statements.len() - keep_len {
    statements[i].kind = StatementKind::Nop;
}
```

then `remove_nops` filters them out
[`src/transform/simplify_output/remove_nops.rs:14-16`]:

```rust
if blk.statements.iter().any(|st| st.kind.is_nop()) {
    blk.statements.retain(|st| !st.kind.is_nop())
}
```

INFERENCE: this is a *compiler-internal* role (in-place tombstone
to avoid index shifts during pass mutation), not an IR-shape role.
RegseqModel can avoid a `Nop` variant by mutating
`Vec<Statement>` with `.retain` / `.swap_remove` directly.

### 6.4 `Error(String)`

[`src/ast/ullbc_ast.rs:44-46`]: Charon's error-as-IR-node mechanism
-- when a Statement's translation fails partway, the translator
substitutes an `Error(...)` placeholder rather than aborting the
body. INFERENCE: graceful-degradation for verification consumers
that want to verify around bad regions. RegseqModel's fail-fast
architecture has no analog; stage 8 should not adopt an `Error`
Statement variant.

Maps to `D-8-1.1` (storage markers + Nop + Deinit + Error
contribute 5 variants Charon keeps that RegseqModel's 3-variant
candidate omits), `D-8-1` (variant set diverges substantially
between Charon and RegseqModel due to verification-vs-codegen
target).

---

## 7. `Assert(Assert)` -- absorbing rustc's TerminatorKind::Assert

`StatementKind::Assert(Assert)` [`src/ast/ullbc_ast.rs:41`]
catches **two upstream sources**:

1. **`mir::NonDivergingIntrinsic::Assume`** (§5.1) ->
   `Assert{on_failure: UndefinedBehavior}`.
2. **`mir::TerminatorKind::Assert`** (the runtime-check terminator
   rustc emits for arithmetic / array bounds / etc.) ->
   `Assert{on_failure: Panic(None)}`. Translation in the
   terminator-translation path [`translate_bodies.rs:907-922`]:

   ```rust
   TerminatorKind::Assert {
       cond, expected, msg: _, target,
       unwind: _, // We model unwinding as an effet, we don't represent it in control flow
   } => {
       let assert = Assert {
           cond: self.translate_operand(span, cond)?,
           expected: *expected,
           on_failure: AbortKind::Panic(None),
       };
       statements.push(Statement::new(span, StatementKind::Assert(assert)));
       let target = self.translate_basic_block_id(*target);
       ullbc_ast::TerminatorKind::Goto { target }
   }
   ```

The pattern: take the rustc *terminator* `Assert {cond, target,
...}`, **emit a Statement** at the end of the current block's
`statements`, and **replace the terminator with `Goto {target}`**.
The terminator's "fail-and-unwind-or-jump-to-target" branching
collapses into a statement-level check.

This is **a unification move**: Charon merges
`Assert`-as-terminator and `Assume`-as-intrinsic into one Statement
variant, distinguishing only via the `on_failure` tag
(`UndefinedBehavior` vs `Panic(None)`). The `msg: _` field
(diagnostic-only panic message) is dropped. The `unwind: _` field
documents that Charon "model[s] unwinding as an effet [sic]" --
not preserved in control flow.

Implications:

- Charon's bridge is allowed to **push to the current block's
  `Vec<Statement>` during terminator translation** -- the
  signature `translate_terminator(..., statements: &mut
  Vec<Statement>)` makes this explicit. Direct precedent for
  stage 8's "Statement::Assert can absorb a terminator"
  candidate.
- Failure-mode is encoded in the `Assert` payload's `on_failure`,
  not on the Statement wrapper.

Maps to `D-8-1.4` (Charon absorbs both Assume and
Assert-terminator into one Statement variant), `D-8-4`
(statement-site invariants -- Charon's `Assert.on_failure`
encodes the abort kind in the Statement payload).

---

## 8. Post-translation passes touching Statements

Charon runs multiple micro-passes that reshape statement-level
constructs after translation. Direct prior art for stage-8
absorption / lowering choices.

### 8.1 `reconstruct_asserts` -- folds the rustc panic-branch idiom

[`src/transform/resugar/reconstruct_asserts.rs:1-65`]: identifies
blocks that are "panic landing pads" (`StorageLive`-only
statements + `TerminatorKind::Abort` terminator); for every
`Switch::If(bid0, bid1)` where one branch lands in such a block,
replaces the terminator with `Goto {target: non-panic-side}` and
pushes `StatementKind::Assert(Assert{cond, expected,
on_failure})` at the end of the current block's statements
[lines 47-59]. INFERENCE: re-creates user-intent `assert!` calls
that rustc lowered to explicit `if cond { panic!() } else {...}`.

### 8.2 `reconstruct_fallible_operations` -- erases rustc's runtime checks

[`src/transform/resugar/reconstruct_fallible_operations.rs:1-487`].
Pass header doc:

```text
//! Micro-pass: remove the dynamic checks for array/slice bounds, overflow, and division by zero.
//! Note that from a semantic point of view, an out-of-bound access or a division by zero
//! must lead to a panic in Rust... we take this into account in the semantics of our
//! array/slice manipulation and arithmetic functions, on the verification side.
```

Pattern-matches on rustc's runtime-check idioms (e.g., bounds
check `l := use(copy a.metadata); b := copy x < copy l;
assert(move b == true)` [lines 96-124]) and:

- **Removes the assert**: overwrites matching prefix statements
  with `StatementKind::Nop` [lines 471-475].
- **Marks the underlying op as panic-on-overflow**: rewrites
  `BinOp::AddChecked` into `BinOp::Add(OverflowMode::Panic)`
  [lines 437-445].

INFERENCE: Charon distinguishes the rustc-MIR
runtime-check-then-panic pattern (removed; panic semantics survive
in the rewritten BinOp) from user-written asserts (preserved as
Statement-level Assert). For stage 8, this poses the question:
should the bridge model rustc's runtime-check Asserts as
Statement variants at all, or let C codegen emit equivalent
checks itself?

### 8.3 `reconstruct_intrinsics` -- desugars `offset_of` calls

[`src/transform/resugar/reconstruct_intrinsics.rs:6-53`]: when a
`TerminatorKind::Call` targets the `offset_of` lang-item, the
pass replaces it with `StatementKind::Assign(_,
NullaryOp(OffsetOf, ...))` plus `Goto`:

```rust
let rval = Rvalue::NullaryOp(
    NullOp::OffsetOf(tref.clone(), variant_id, field_id),
    Ty::mk_usize(),
);
ctx.insert_assn_stmt(call.dest.clone(), rval);
term.kind = TerminatorKind::Goto { target: *target };
```

INFERENCE: Charon pulls non-diverging intrinsics *out of
Call-terminator into Assign-statement* -- the reverse of §5's
Assume->Assert lowering. Stage 8 D-8-1.4 question: can intrinsic
calls become Statements? Charon: yes, for non-diverging
fully-resolvable intrinsics like offset_of.

### 8.4 `simplify_constants` -- normalises operand-position constants

[`src/transform/simplify_output/simplify_constants.rs:1-12`]: pass
header:

```text
//! The MIR constant expressions lead to a lot of duplication...
//! This pass removes all those occurrences so that only the
//! `ConstantExpression::Literal`. It does so by introducing intermediate statements.
```

For `Operand::Const(c)` where `c` is a non-Literal constant
(e.g., `Ref`, `Aggregate`), the pass synthesises a fresh local,
emits intermediate `Assign(tmp, ...)` Statements to build the
value, and replaces the original operand with `Move(tmp)`.
INFERENCE: Charon uses Statement-list mutation as a late
normalisation tool; the statements list is `Vec<Statement>` (not
frozen), so mid-pipeline insertion is well-typed.

### 8.5 `insert_assign_return_unit`

[`src/transform/finish_translation/insert_assign_return_unit.rs:13-25`]:
for unit-returning functions, synthesises
`Assign(return_place, Rvalue::unit_value())` at body entry. An
Aeneas-specific requirement (uninitialised return slot would
otherwise produce ⊥); not directly relevant to RegseqModel.

Maps to `D-8-2` (Statements live in mutable `Vec<Statement>`;
mid-pipeline insertion is first-class), `D-8-4` (statement-site
invariants are inter-pass invariants, not just translation-time
checks), `D-8-1.4` (runtime-check removal shows the
"runtime-check-then-panic" lowering target).

---

## 9. ULLBC -> LLBC bridge: Statement reshape after CFG reconstruction

The `ullbc_to_llbc` pass
[`src/transform/control_flow/ullbc_to_llbc.rs:1427-1445`] is the
sole site where ULLBC `StatementKind`s get re-emitted into LLBC.
All 9 ULLBC variants map 1:1 to LLBC variants of the same name
(Assign, SetDiscriminant, CopyNonOverlapping, StorageLive,
StorageDead, Deinit, Assert, Nop, Error).

**Where the cardinality grows is the Terminator side.** LLBC's
`StatementKind` adds `Drop`, `Call`, `Abort`, `Return`, `Break`,
`Continue`, `Switch`, `Loop` -- all absorbed from ULLBC
`TerminatorKind` during CFG reconstruction
[`src/ast/llbc_ast.rs:21-72`]. LLBC fuses
Statement-vs-Terminator; ULLBC keeps them distinct.

INFERENCE: ULLBC is the layer RegseqModel's stage 8 should
benchmark against -- basic-block-shaped, Terminator-distinct.
LLBC's structured-control-flow / Statement-fused shape is
downstream territory.

Maps to `D-8-1.1` (basic-block-layer cardinality: 9 in ULLBC),
`D-8-2` (no Statement interning across the ULLBC->LLBC bridge --
statements are cloned and re-pushed into new block structures).

---

## 10. Statement-site rejection vs absorption (D-8-4)

Translation establishes a clear **"absorb, drop, or fail"**
gradient for rustc `StatementKind` variants. Cross-tabulating
[`translate_bodies.rs:782-840`]:

| rustc `mir::StatementKind` | Charon behaviour | `[file:line]` |
|-|-|-|
| `Assign((place, rvalue))` | **Absorb** as `StatementKind::Assign` | translate_bodies.rs:783-787 |
| `SetDiscriminant { place, variant_index }` | **Absorb** as `StatementKind::SetDiscriminant` | translate_bodies.rs:788-795 |
| `StorageLive(local)` | **Absorb** as `StatementKind::StorageLive` | translate_bodies.rs:796-799 |
| `StorageDead(local)` | **Absorb** as `StatementKind::StorageDead` | translate_bodies.rs:800-803 |
| `Intrinsic(NonDivergingIntrinsic::Assume(op))` | **Lower** to `StatementKind::Assert(Assert{cond:op, expected:true, on_failure:UB})` | translate_bodies.rs:805-812 |
| `Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping{src,dst,count})` | **Lower** to dedicated `StatementKind::CopyNonOverlapping` | translate_bodies.rs:813-822 |
| `Retag(_, _)` | **Drop** (`None`) | translate_bodies.rs:824 |
| `FakeRead(..)` | **Drop** (`None`) | translate_bodies.rs:827 |
| `PlaceMention(..)` | **Drop** (`None`) | translate_bodies.rs:827 |
| `AscribeUserType(_, _)` | **Drop** (`None`) | translate_bodies.rs:831 |
| `Coverage(_)` | **Drop** (`None`) | translate_bodies.rs:833 |
| `ConstEvalCounter` | **Drop** (`None`) | translate_bodies.rs:836 |
| `BackwardIncompatibleDropHint { .. }` | **Drop** (`None`) | translate_bodies.rs:838 |
| `Nop` | **Drop** (`None`) | translate_bodies.rs:839 |
| `Deinit(..)` | **Not handled** -- variant declared but no translation arm | translate_bodies.rs:782-840 |

Patterns:

- **Drop** for borrowck / lint / stacked-borrows / coverage /
  interpreter scaffolding (`Retag`, `FakeRead`, `PlaceMention`,
  `AscribeUserType`, `Coverage`, `ConstEvalCounter`,
  `BackwardIncompatibleDropHint`, rustc's own `Nop`) -- all carry
  zero post-borrowck semantic content.
- **Absorb** Charon-target-relevant operational primitives
  (`Assign`, `SetDiscriminant`, `StorageLive`, `StorageDead`) as
  same-named variants.
- **Lower** intrinsic-flavoured constructs to specialised
  Statement variants (`Assume` -> `Assert`, `CopyNonOverlapping`
  -> dedicated).

**Charon issues no `raise_error!` / `unimplemented!` at the
Statement layer.** Compare to the Rvalue-level rejections (stage-7
charon.md §11): `Rvalue::ThreadLocalRef` and
`Rvalue::WrapUnsafeBinder` are hard-rejected. **No rustc
`StatementKind` triggers a translation-time error.** Variants that
*would* be problematic (coroutines, tail calls, yield) live at the
Terminator level, where they are rejected
[`translate_bodies.rs:953-956`]:

```rust
TerminatorKind::CoroutineDrop
| TerminatorKind::TailCall { .. }
| TerminatorKind::Yield { .. } => {
    raise_error!(self, span, "Unsupported terminator: {:?}", terminator.kind);
}
```

Implications for D-8-4:

- **No Statement-level rejection.** Bridge worst-case is a
  drop-to-`None`. Statement-level operations are uniformly
  absorbable or ignorable in Charon's experience.
- **RegseqModel's 3-variant candidate excludes exactly the
  variants Charon either drops or treats as semantic-content-free
  pass machinery.** Direct support for RegseqModel's tighter
  surface.
- **RegseqModel's `{Assign, SetDiscriminant, Intrinsic}` includes
  exactly Charon's semantically-load-bearing variants** (modulo
  the per-intrinsic vs flat-Intrinsic divergence at D-8-1.4).

Maps to `D-8-4` (no Statement-level rejection in Charon),
`D-8-1.1` (variant overlap with RegseqModel candidate).

---

## 11. Statement storage in basic blocks (D-8-2)

ULLBC's basic block holds statements as a plain `Vec<Statement>`
[`src/ast/ullbc_ast.rs:127-130`]:

```rust
#[charon::rename("Block")]
pub struct BlockData {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}
```

Key observations:

- **Inline storage.** Each `Statement` is stored by value; no
  heap indirection, no statement-id table, no `&Stmt` interning.
- **No `StatementId` at the ULLBC layer.** Identity is positional
  `(BlockId, usize)`. The block container is
  `pub type BodyContents = Vector<BlockId, BlockData>`
  [`src/ast/ullbc_ast.rs:14-15`], where `Vector<K, V>` is
  Charon's typed-index dense array (parallel to rustc's
  `IndexVec<K, V>`).
- **`StatementId` exists only at the LLBC layer**
  [`src/ast/llbc_ast.rs:14-15, 79-80`], globally-fresh per body.
  INFERENCE: LLBC's restructured control flow scatters statements
  across nested `Block`s, breaking positional identity; ULLBC's
  basic-block locality preserves it.
- **Mutation via normal `Vec` operations.** `.retain(...)`
  (`remove_nops`), `.splice(...)` (`insert_storage_lives`),
  `.insert(...)` (`reconstruct_asserts`, `simplify_constants`),
  direct `[i].kind = ...` overwrite
  (`reconstruct_fallible_operations`).
- **No `StatementKind` interning anywhere.** Each statement is
  its own copy. Charon interns *types* (parallel to rustc) but
  not statements.
- **Body envelope: `GExprBody<T>`** [`src/ast/gast.rs:46-58`]
  wraps `BodyContents` with `span`, `locals`, and
  `comments: Vec<(usize, Vec<String>)>` (the per-line scrape
  that feeds `comments_before` via `recover_body_comments`).

Implications for D-8-2:

- **`Vec<Statement>` is load-bearing**: each statement carries
  its own span/metadata in-place; passes preserve those during
  mutation.
- **Per-Statement IDs are a layer-shape decision**, not an
  intrinsic property: ULLBC positional, LLBC id'd. RegseqModel's
  bridge-output is the ULLBC analog -- positional is consistent.

Maps to `D-8-2` (inline `Vec<Statement>` with positional
identity, no interning at the basic-block layer).

---

## 12. Pretty-printer (D-8-K)

ULLBC's `Statement` pretty-print
[`src/pretty/fmt_with_ctx.rs:1386-1430`] is a total match over
all 9 variants:

```rust
impl<C: AstFormatter> FmtWithCtx<C> for ullbc::Statement {
    fn fmt_with_ctx(&self, ctx: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tab = ctx.indent();
        for line in &self.comments_before {
            writeln!(f, "{tab}// {line}")?;
        }
        match &self.kind {
            StatementKind::Assign(place, rvalue) =>
                write!(f, "{tab}{} := {}", place.with_ctx(ctx), rvalue.with_ctx(ctx)),
            StatementKind::SetDiscriminant(place, variant_id) =>
                write!(f, "{tab}@discriminant({}) := {}", place.with_ctx(ctx), variant_id),
            StatementKind::CopyNonOverlapping(box CopyNonOverlapping { src, dst, count }) =>
                write!(f, "{tab}copy_nonoverlapping({}, {}, {})",
                       src.with_ctx(ctx), dst.with_ctx(ctx), count.with_ctx(ctx)),
            StatementKind::StorageLive(var_id) =>
                write!(f, "{tab}storage_live({})", var_id.with_ctx(ctx)),
            StatementKind::StorageDead(var_id) =>
                write!(f, "{tab}storage_dead({})", var_id.with_ctx(ctx)),
            StatementKind::Deinit(place) =>
                write!(f, "{tab}deinit({})", place.with_ctx(ctx)),
            StatementKind::Assert(assert) => write!(f, "{tab}{}", assert.with_ctx(ctx)),
            StatementKind::Nop => write!(f, "{tab}nop"),
            StatementKind::Error(s) => write!(f, "{tab}@Error({})", s),
        }
    }
}
```

Observations:

- **MIR-style surface syntax**: `:=` for Assign,
  `@discriminant(...)` for SetDiscriminant, function-call shape
  for `copy_nonoverlapping(...)`, `storage_live(...) /
  storage_dead(...)` for storage markers, `nop` for Nop,
  `@Error(...)` for the error-as-IR-node case.
- **`comments_before` emitted as `// line` prefixes**, making the
  field user-visible.
- **`AstFormatter` is context-bearing**: explicit context
  threading via `ctx.indent()` and `x.with_ctx(ctx)` rather than
  thread-local lookup. Statement printing composes via Place /
  Rvalue / Operand / VarId printing.
- **No separate `render_statement` helper.** Display goes through
  `FmtWithCtx::fmt_with_ctx` and a `with_ctx(ctx)` adapter.

LLBC's pretty-print [`src/pretty/fmt_with_ctx.rs:1432-...`] is
structurally similar but covers the larger LLBC StatementKind
variant set (Drop, Call, Abort, Return, Break, Continue, Switch,
Loop in addition to the ULLBC variants).

Implications for D-8-K:

- **Charon has a non-deferred, fully-implemented pretty-printer
  for Statements.** This is parallel to LLBC's `fmt_with_ctx`
  layer for all IR types. RegseqModel's `D-N-K` deferral pattern
  (defer pretty.rs to stage 11/12/14) is a methodology choice
  about implementation timing -- Charon ships pretty-print
  inline because verification consumers debug against the
  printed form. INFERENCE: stage 8 can defer `render_statement`
  to stage 11 without losing any structural information; the
  match-arm shape is fully determined by the variant set.

Maps to `D-8-K` (renderer / pretty-printer for Statements --
Charon's `FmtWithCtx` impl is a complete prior-art surface;
deferral is a timing decision, not a structural one).

---

## 13. Cross-reference vs `rustc_middle::mir::StatementKind`

Charon's choices that diverge from rustc:

| Aspect | rustc_middle | Charon ULLBC |
|-|-|-|
| Variant count | 14 | 9 |
| `Assign` payload | `Box<(Place, Rvalue)>` (size budget) | `(Place, Rvalue)` inline |
| `SetDiscriminant` | `{place: Box<Place>, variant_index}` struct variant | `(Place, VariantId)` tuple variant |
| Intrinsic | single `Intrinsic(Box<NonDivergingIntrinsic>)` | fanned into `Assert` (absorbs `Assume`) + `CopyNonOverlapping` |
| `Assert` placement | `TerminatorKind::Assert` | `StatementKind::Assert` (absorbs both `Assume` and rustc's terminator) |
| `FakeRead`, `PlaceMention`, `AscribeUserType`, `Coverage`, `ConstEvalCounter`, `Retag`, `BackwardIncompatibleDropHint` | distinct StatementKind variants | dropped at translation |
| `Deinit` | emitted post-`drop_elaboration` | declared but never produced from rustc |
| Wrapper | `Statement {source_info: SourceInfo, kind}` (scope + span) | `Statement {span, kind, comments_before}` (novel comments field, no scope) |
| Error variant | none | `Error(String)` graceful-degradation tombstone |
| Identity | positional `(BasicBlock, idx)` | positional in ULLBC; dense `StatementId` in LLBC |

Significant divergences:

- Charon drops 7/14 rustc variants; RegseqModel candidate drops
  11/14 (more aggressive, subsumes Charon's drop set plus storage
  markers/Deinit and uses uniform Intrinsic).
- Charon's per-intrinsic-variant + absorption strategy genuinely
  diverges from rustc's flat `Intrinsic(...)`.
- `comments_before: Vec<String>` is Charon-only.

---

## 14. Synthesis: what Charon says about Statement vocabulary in an `'tcx`-free, Serde-derived, owned IR

Stage 8's question: RegseqModel and Charon are both
rustc-MIR-consuming, `'tcx`-free, Serde-derived, owned-IR designs
for non-rustc consumers (Aeneas verification; C codegen). What
does Charon's choice say about which Statement variants survive?

1. **Borrowck / lint / stacked-borrows / coverage / interpreter
   variants carry no semantic content downstream.** Both Charon
   and RegseqModel drop FakeRead, Retag, PlaceMention,
   AscribeUserType, Coverage, ConstEvalCounter,
   BackwardIncompatibleDropHint. Robust design point for any
   non-rustc-internal consumer.

2. **Storage allocation/deallocation variants are
   target-dependent.** Charon keeps StorageLive/StorageDead/Deinit
   for verification (and completes them via `insert_storage_lives`);
   RegseqModel can drop them because C handles allocation
   implicitly.

3. **`Assign` and `SetDiscriminant` are universally retained.**
   Load-bearing for every downstream consumer.

4. **`Assert` placement is a control-flow-modelling decision.**
   Charon chose Statement-layer with `AbortKind` discriminator;
   rustc keeps it at Terminator-layer. Stage 8 vs stage 9
   territory.

5. **Intrinsic handling is the largest divergence point.** Three
   candidates: (a) per-intrinsic Statement variants -- Charon's
   choice; (b) absorption into related existing variant
   (`Assume` -> `Assert`) -- Charon's choice; (c) flat
   `Intrinsic(kind, args)` -- RegseqModel candidate. Charon
   avoids (c) entirely. INFERENCE: verification target prefers
   per-variant type-safety; codegen target may prefer flat
   dispatch.

6. **Basic-block-layer Statement identity is positional, not
   interned.** ULLBC's `Vec<Statement>` carries the shape
   RegseqModel needs. Per-Statement IDs (LLBC's `StatementId`)
   are a post-CFG-reconstruction concern.

7. **`Statement` always wraps `StatementKind` with distinct
   metadata.** Both Charon (span + comments_before) and rustc
   (source_info) do this; metadata content is target-specific.

8. **Mutating `Vec<Statement>` (insert/replace/remove) is a
   first-class IR operation.** Charon's pass framework relies
   on it pervasively.

9. **No StatementKind is translation-rejected in Charon.** Every
   rustc `StatementKind` is absorb-or-drop; translation-time
   errors live at Rvalue and Terminator levels. INFERENCE:
   stage 8's D-8-4 ("which StatementKinds get rejected at the
   bridge?") may have an empty answer set.

10. **`StatementKind::Error` is consumer-specific.** Aeneas wants
    graceful degradation; RegseqModel's fail-fast architecture
    (per stage-1) doesn't benefit. Stage 8 should not adopt it.

---

## 15. Summary mapping to D-8-K candidate decisions

| Tag | Decision | Charon evidence anchor |
|-|-|-|
| `D-8-1` | StatementKind variant set + per-variant payload shapes | §1, §3-§7; closer to 9 variants in ULLBC by absorbing Assert and per-intrinsic naming |
| `D-8-1.1` | Variant cardinality | §1, §10; Charon: 9 ULLBC variants. RegseqModel candidate: 3. Charon retains storage markers + Nop + Deinit + Error + per-intrinsic variants that RegseqModel candidate omits |
| `D-8-1.2` | Assign payload shape | §3; tuple-variant `(Place, Rvalue)`, inline (no Box); type direction is into-rvalue from place type |
| `D-8-1.3` | SetDiscriminant placement | §4; preserved as a dedicated 2-tuple Statement variant `(Place, VariantId)`, not lifted to Rvalue, not lowered |
| `D-8-1.4` | Intrinsic placement | §5, §7; per-intrinsic Statement variants (`CopyNonOverlapping`) + absorption into existing variants (`Assume` -> `Assert`); **Charon does not have a generic `Intrinsic(kind, args)` shape** |
| `D-8-2` | Statement interning vs inline storage | §11; ULLBC uses inline `Vec<Statement>`, positional identity, no interning. LLBC adds `StatementId` after CFG restructuring |
| `D-8-3` | Statement wrapper struct | §2; Charon's `Statement { span, kind, comments_before }` (3 fields; LLBC adds `id`). The wrapper carries metadata distinct from the kind |
| `D-8-4` | Statement-site invariants + rejection | §10; **Charon does not reject any rustc StatementKind variant at translation.** All variants are either absorbed (8 cases), dropped to None (7 cases), or lowered to a same-name-different-shape variant (2 cases). Translation-time rejection happens only at Rvalue and Terminator levels |
| `D-8-K` | render_statement defer to stage 11+ | §12; Charon ships pretty-print inline via `FmtWithCtx`. Deferral is a methodology choice, not a structural one; the match-arm shape is fully determined once the variant set closes |

This kickoff prior-art pass establishes the absorb / drop /
lower gradient at the Statement layer, the wrapper-struct pattern
with span + comments, the `Vec<Statement>` storage convention,
and the per-intrinsic variant strategy that diverges from
RegseqModel's flat-Intrinsic candidate. Mid-stage deeper reads
(e.g., into Aeneas's verification semantics for `Assume` vs
`Assert`, or into Charon's `desugar_drops` lowering of
Drop-as-terminator into Drop-as-Statement at LLBC) can land
additional citations as specific D-tags demand them.
