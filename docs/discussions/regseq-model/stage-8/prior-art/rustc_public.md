# Stage 8 -- Prior Art: rustc_public primary source

`rustc_public` (formerly `rustc_smir`, the in-tree home of the Stable
MIR project) is the rustc-internal crate that defines an owned,
`'tcx`-free, `serde::Serialize`-able mirror of `rustc_middle`'s MIR.
Its purpose is "the WIP public interface to rustc internals... for
third-party tools to invoke" -- documented at the crate root
[`rustc_public/src/lib.rs:1-3,13-14`]:

```rust
//! The WIP public interface to rustc internals.
//!
//! For more information see <https://github.com/rust-lang/project-stable-mir>
//!
//! # Note
//!
//! This API is still completely unstable and subject to change.
...
//! This crate shall contain all type definitions and APIs that we
//! expect third-party tools to invoke to interact with the compiler.
```

This makes `rustc_public` the **closest existing analog to
RegseqModel's body-level IR shape**: both are owned, `'tcx`-free,
serde-serializable mirrors of `mir::Body<'tcx>` derived through a
hand-written translator from rustc internals. They differ only in
consumer (Stable-MIR external tools vs RegseqModel's C backend) and
in the cleanup-pass discharge applied before observation. Its design
choices are direct prior art for stage 8's `Statement` decision
namespace.

This file is the rustc_public prior-art evidence pass for stage 8
(`Statement`). Stages 5 (`Place`), 6 (`Operand`), 7 (`Rvalue`) are
closed; stage 8 specifies the variant set (`D-8-1`), interning vs
inline storage (`D-8-2`), the wrapper struct shape (`D-8-3`),
construction-site invariants (`D-8-4`), and renderer / pretty-printer
(`D-8-K`).

All paths below are relative to the pinned-toolchain root:

```
~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/
```

(See `../../rustc-references.md` for the canonical statement of this
root path.)

Every factual claim cites a `[file:line]` in that pinned source.
Hypotheses are flagged as **HYPOTHESIS:**. Cross-source synthesis
across two or more sites is flagged as **INFERENCE:**. Citations to
sibling prior-art files (mrustc, charon) are out of scope here -- this
file documents `rustc_public` only.

Stage 8's candidate D-tag namespace, restated for self-containment:

- **D-8-1** -- `Statement` variant set + per-variant payload shapes.
  - **D-8-1.1** -- variant cardinality (RegseqModel candidate: 3
    variants `Assign | SetDiscriminant | Intrinsic`; rustc_public
    candidate: 12).
  - **D-8-1.2** -- `Assign` payload shape.
  - **D-8-1.3** -- `SetDiscriminant` payload shape.
  - **D-8-1.4** -- `Intrinsic` payload shape (and nesting through
    `NonDivergingIntrinsic` / `CopyNonOverlapping`).
- **D-8-2** -- statement interning vs inline storage in the body.
- **D-8-3** -- `Statement` wrapper struct (`kind` + `span` vs
  rustc_middle's `kind` + `source_info` + `debuginfos`).
- **D-8-4** -- statement-site invariants and rejection rules at the
  bridge.
- **D-8-K** -- renderer / pretty-printer for statements.

---

## 1. `StatementKind` enum shape

`rustc_public::StatementKind` is a closed 12-variant enum
[`rustc_public/src/mir/body.rs:476-490`]:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum StatementKind {
    Assign(Place, Rvalue),
    FakeRead(FakeReadCause, Place),
    SetDiscriminant { place: Place, variant_index: VariantIdx },
    StorageLive(Local),
    StorageDead(Local),
    Retag(RetagKind, Place),
    PlaceMention(Place),
    AscribeUserType { place: Place, projections: UserTypeProjection,
                      variance: Variance },
    Coverage(Coverage),
    Intrinsic(NonDivergingIntrinsic),
    ConstEvalCounter,
    Nop,
}
```

Key facts:

- **12 variants, closed.** No `Other` / `Opaque` arm. There are no
  per-variant doc comments on this enum (contrast rustc_middle, which
  documents each variant in detail at
  `rustc_middle/src/mir/syntax.rs:311-471`).
- **No Hash derive.** The `derive(...)` set is `Clone, Debug, Eq,
  PartialEq, Serialize`
  [`rustc_public/src/mir/body.rs:476`]. Surrounding types in the same
  file -- e.g. `Rvalue` [`body.rs:492`], `BorrowKind` [`body.rs:910`]
  -- *do* derive `Hash`. The omission on `Statement` /
  `StatementKind` is structural: rustc_public never interns or
  hash-deduplicates statements (see §3 below).
- **No `BackwardIncompatibleDropHint` variant.** rustc_middle's
  13th variant [`rustc_middle/src/mir/syntax.rs:466-471`] is dropped
  by the bridge -- see §6 for the translation evidence.
- **No size assert.** `grep static_assert_size` across the entire
  rustc_public and rustc_public_bridge crates returns nothing
  (verified). rustc_middle's
  `static_assert_size!(StatementKind<'_>, 16)`
  [`rustc_middle/src/mir/syntax.rs:1753`] is not mirrored. The
  rustc_public team has explicitly chosen to be size-budget-free for
  the owned-IR mirror -- payloads can flatten where rustc_middle had
  to box.

Maps to **D-8-1** (variant set + payload shapes), **D-8-1.1**
(cardinality).

---

## 2. `Statement` wrapper struct: `kind` + `span` only

The `Statement` wrapper is a flat 2-field struct
[`rustc_public/src/mir/body.rs:470-474`]:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}
```

Compare rustc_middle's wrapper
[`rustc_middle/src/mir/statement.rs:14-22`]:

```rust
/// A statement in a basic block, including information about its source code.
#[derive(Clone, TyEncodable, TyDecodable, HashStable, TypeFoldable, TypeVisitable)]
#[non_exhaustive]
pub struct Statement<'tcx> {
    pub source_info: SourceInfo,
    pub kind: StatementKind<'tcx>,
    /// Some debuginfos appearing before the primary statement.
    pub debuginfos: StmtDebugInfos<'tcx>,
}
```

Three drift points:

1. **`source_info: SourceInfo` -> `span: Span`.** rustc_middle's
   `SourceInfo` is `{ span, scope }`
   [`rustc_middle/src/mir/mod.rs:822-830`]:

   ```rust
   pub struct SourceInfo {
       pub span: Span,
       pub scope: SourceScope,
   }
   ```

   rustc_public **does** define a `SourceInfo` type with the same
   `{ span, scope }` shape [`rustc_public/src/mir/body.rs:752-756`]:

   ```rust
   pub type SourceScope = u32;
   pub struct SourceInfo {
       pub span: Span,
       pub scope: SourceScope,
   }
   ```

   But it uses `SourceInfo` only for `VarDebugInfo`
   [`rustc_public/src/mir/body.rs:711-717`] -- not for `Statement`.
   The bridge translation site is the smoking gun
   [`rustc_public/src/unstable/convert/stable/mir.rs:68-79`]:

   ```rust
   impl<'tcx> Stable<'tcx> for mir::Statement<'tcx> {
       type T = crate::mir::Statement;
       fn stable<'cx>(...) -> Self::T {
           Statement {
               kind: self.kind.stable(tables, cx),
               span: self.source_info.span.stable(tables, cx),
           }
       }
   }
   ```

   The `source_info.scope` field is *explicitly dropped on the floor*
   at the statement boundary while preserved at the
   `VarDebugInfo` boundary
   [`rustc_public/src/unstable/convert/stable/mir.rs:51-65`]. This is
   a deliberate consumer choice: **the per-statement scope is
   debuginfo-only metadata, and a serialized owned-IR consumer
   doesn't need it on every statement.** INFERENCE: the rustc_public
   team's belief is that lexical-scope information is recoverable
   from `VarDebugInfo.source_info.scope` for the locals it covers,
   without needing per-statement scope tracking.

2. **`debuginfos: StmtDebugInfos` is dropped entirely.**
   rustc_middle's `StmtDebugInfos` is `Vec<StmtDebugInfo<'tcx>>`
   [`rustc_middle/src/mir/statement.rs:1014-1015`] where
   `StmtDebugInfo` is a 2-variant enum
   [`rustc_middle/src/mir/statement.rs:1076-1080`]:

   ```rust
   pub enum StmtDebugInfo<'tcx> {
       AssignRef(Local, Place<'tcx>),
       InvalidAssign(Local),
   }
   ```

   The rustc_middle Body also carries `after_last_stmt_debuginfos:
   StmtDebugInfos` [`rustc_middle/src/mir/mod.rs:1314`] for
   debuginfo trailing the last statement of a block. **rustc_public's
   bridge translates neither of these.** The body translator at
   `rustc_public/src/unstable/convert/stable/mir.rs:22-34` constructs
   each `BasicBlock` with `terminator` and `statements` only -- no
   `debuginfos` field, no `after_last_stmt_debuginfos`. Per-statement
   debuginfo is silently elided. INFERENCE: the rustc_public team
   considers `StmtDebugInfo` (a recently-added borrowck-elaboration
   artefact for tracking now-NOPed assignments after `make_nop`)
   not part of the stable owned-IR contract.

3. **No `#[non_exhaustive]`.** rustc_middle marks its `Statement`
   wrapper `#[non_exhaustive]`
   [`rustc_middle/src/mir/statement.rs:16`]; rustc_public does not
   [`rustc_public/src/mir/body.rs:470`]. **HYPOTHESIS:** rustc_public
   intends `Statement` to be pattern-matched exhaustively by external
   consumers -- consistent with the closed-vocabulary discipline of
   the whole crate.

Maps to **D-8-3** (Statement wrapper).

---

## 3. Inline storage in `Body.blocks[i].statements: Vec<Statement>`

`Body` and `BasicBlock` carry statements inline, not via a separate
interner [`rustc_public/src/mir/body.rs:13-38, 133-137`]:

```rust
#[derive(Clone, Debug, Serialize)]
pub struct Body {
    pub blocks: Vec<BasicBlock>,
    pub(super) locals: LocalDecls,
    pub(super) arg_count: usize,
    pub var_debug_info: Vec<VarDebugInfo>,
    pub(super) spread_arg: Option<Local>,
    pub span: Span,
}
...
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct BasicBlock {
    pub statements: Vec<Statement>,
    pub terminator: Terminator,
}
```

There is no `StatementId` type, no `StatementInterner`, no
`Tables<'tcx>` slot for statements. Verify the latter at the bridge
table definition [`rustc_public_bridge/src/lib.rs:45-54`]:

```rust
pub struct Tables<'tcx, B: Bridge> {
    pub def_ids: IndexMap<DefId, B::DefId>,
    pub alloc_ids: IndexMap<AllocId, B::AllocId>,
    pub spans: IndexMap<rustc_span::Span, B::Span>,
    pub types: IndexMap<Ty<'tcx>, B::Ty>,
    pub instances: IndexMap<ty::Instance<'tcx>, B::InstanceDef>,
    pub ty_consts: IndexMap<ty::Const<'tcx>, B::TyConstId>,
    pub mir_consts: IndexMap<mir::Const<'tcx>, B::MirConstId>,
    pub layouts: IndexMap<rustc_abi::Layout<'tcx>, B::Layout>,
}
```

Eight interned families: DefIds, AllocIds, Spans, Types, Instances,
TyConsts, MirConsts, Layouts. **No statements.** The interning
discipline applies to identity-bearing or canonicalisation-bearing
data -- statements have neither (they are positional / single-use,
identified by their `(BasicBlockIdx, statement_index)` location, not
by content).

This is consistent with rustc_public not deriving `Hash` on
`Statement` / `StatementKind` (§1): you need `Hash` to intern, and
rustc_public doesn't intern statements, so it doesn't need `Hash`.

INFERENCE: a positional location (block index + statement index) is
sufficient identity for a statement. Statement *position* matters
(execution order, dataflow); statement *identity* (content-equality)
does not. This is the same discipline rustc_middle follows -- its
`Statement` derives `PartialEq` but no `Hash` either
[`rustc_middle/src/mir/syntax.rs:309`: `derive(Clone, Debug,
PartialEq, TyEncodable, TyDecodable, Hash, HashStable)`] -- correction:
rustc_middle's `StatementKind` *does* derive `Hash`, but
`mir::Statement` does not. INFERENCE: the rustc_public choice
omits `Hash` even from `StatementKind` because no consumer in the
crate's intended use case needs to dedup statements.

Maps to **D-8-2** (interning vs inline).

---

## 4. Per-variant deep-dive

For each of the 12 variants, what's the payload shape, what does
rustc_middle do, what does rustc_public's bridge do, and what
survives into the owned IR.

The bridge translation site for the whole enum lives at
[`rustc_public/src/unstable/convert/stable/mir.rs:130-187`]:

```rust
impl<'tcx> Stable<'tcx> for mir::StatementKind<'tcx> {
    type T = crate::mir::StatementKind;
    fn stable<'cx>(...) -> Self::T {
        match self {
            mir::StatementKind::Assign(assign) => crate::mir::StatementKind::Assign(
                assign.0.stable(tables, cx),
                assign.1.stable(tables, cx),
            ),
            mir::StatementKind::FakeRead(fake_read_place) =>
                crate::mir::StatementKind::FakeRead(
                    fake_read_place.0.stable(tables, cx),
                    fake_read_place.1.stable(tables, cx),
                ),
            mir::StatementKind::SetDiscriminant { place, variant_index } =>
                crate::mir::StatementKind::SetDiscriminant {
                    place: place.as_ref().stable(tables, cx),
                    variant_index: variant_index.stable(tables, cx),
                },
            mir::StatementKind::StorageLive(place) =>
                crate::mir::StatementKind::StorageLive(place.stable(tables, cx)),
            mir::StatementKind::StorageDead(place) =>
                crate::mir::StatementKind::StorageDead(place.stable(tables, cx)),
            mir::StatementKind::Retag(retag, place) =>
                crate::mir::StatementKind::Retag(
                    retag.stable(tables, cx), place.stable(tables, cx)),
            mir::StatementKind::PlaceMention(place) =>
                crate::mir::StatementKind::PlaceMention(place.stable(tables, cx)),
            mir::StatementKind::AscribeUserType(place_projection, variance) =>
                crate::mir::StatementKind::AscribeUserType {
                    place: place_projection.as_ref().0.stable(tables, cx),
                    projections: place_projection.as_ref().1.stable(tables, cx),
                    variance: variance.stable(tables, cx),
                },
            mir::StatementKind::Coverage(coverage) =>
                crate::mir::StatementKind::Coverage(opaque(coverage)),
            mir::StatementKind::Intrinsic(intrinstic) =>
                crate::mir::StatementKind::Intrinsic(intrinstic.stable(tables, cx)),
            mir::StatementKind::ConstEvalCounter =>
                crate::mir::StatementKind::ConstEvalCounter,
            // BackwardIncompatibleDropHint has no semantics, so it is translated to Nop.
            mir::StatementKind::BackwardIncompatibleDropHint { .. } =>
                crate::mir::StatementKind::Nop,
            mir::StatementKind::Nop => crate::mir::StatementKind::Nop,
        }
    }
}
```

This match is exhaustive over rustc_middle's 13 variants and produces
rustc_public's 12 variants. Every variant either flows through 1:1,
unboxes a payload, restructures a tuple into a named-field struct, or
collapses to `Nop`.

### 4.1 `Assign(Place, Rvalue)`

rustc_middle [`syntax.rs:341`]: `Assign(Box<(Place<'tcx>,
Rvalue<'tcx>)>)` -- boxed to keep the enum size budget
(`static_assert_size!(StatementKind<'_>, 16)`,
[`syntax.rs:1753`]; flat would inflate it).

rustc_public [`body.rs:478`]: `Assign(Place, Rvalue)` -- **flat, no
Box.** The bridge translates the boxed pair into two positional
arguments by calling `assign.0.stable(...)` and `assign.1.stable(...)`
[`stable/mir.rs:138-141`]. The unboxing is purely a payload-layout
choice; there is no semantic change.

**Why unbox?** rustc_public has no enum-size budget. The owned
`Place` and owned `Rvalue` are themselves larger than rustc_middle's
`Place<'tcx>` / `Rvalue<'tcx>` (the owned analogs carry serialized
payloads instead of `'tcx`-interned references), so the box would
have inflated the heap-indirection cost without buying back size. The
serde-serialization use case also prefers flat shapes.

INFERENCE: this confirms rustc_public's design philosophy that an
**owned-IR mirror is free to flatten** what an interned-IR mirror has
to box. Stage 7's prior-art note (`stage-7/prior-art/rustc.md` §12)
already records the same pattern for `Rvalue::BinaryOp` and
`Rvalue::Aggregate` flattening in rustc_public.

Maps to **D-8-1.2** (Assign payload shape).

### 4.2 `FakeRead(FakeReadCause, Place)`

rustc_middle [`syntax.rs:358`]: `FakeRead(Box<(FakeReadCause,
Place<'tcx>)>)` -- boxed.

rustc_public [`body.rs:479`]: `FakeRead(FakeReadCause, Place)` --
flat. Bridge unboxes [`stable/mir.rs:142-145`].

`FakeReadCause` is a 5-variant enum
[`rustc_public/src/mir/body.rs:430-438`]:

```rust
pub enum FakeReadCause {
    ForMatchGuard,
    ForMatchedPlace(LocalDefId),
    ForGuardBinding,
    ForLet(LocalDefId),
    ForIndex,
}
```

(The rustc_middle equivalent at `syntax.rs:566-629` documents each
cause; rustc_public preserves all five.)

**Phase qualification**: rustc_middle documents `FakeRead` as
"Disallowed after drop elaboration"
[`rustc_middle/src/mir/syntax.rs:357`]. The MIR phase guard means
`FakeRead` should not survive into runtime MIR. INFERENCE: stages
later than borrowck (which is where drop elaboration runs) should
not observe `FakeRead`. RegseqModel consumes `optimized_mir` --
`MirPhase::Runtime(RuntimePhase::PostCleanup)` per stage-7 prior-art
[`rustc_mir_transform/src/lib.rs:612`] -- so this variant is
**bridge-rejection candidate** for RegseqModel. rustc_public does
not exploit this fact (it preserves `FakeRead` for consumers that
might inspect pre-runtime MIR).

Maps to **D-8-1** (variant set), **D-8-4** (rejection rules).

### 4.3 `SetDiscriminant { place: Place, variant_index: VariantIdx }`

rustc_middle [`syntax.rs:365`]: `SetDiscriminant { place:
Box<Place<'tcx>>, variant_index: VariantIdx }` -- struct-shaped with
boxed `place`.

rustc_public [`body.rs:480`]: `SetDiscriminant { place: Place,
variant_index: VariantIdx }` -- **struct-shaped, place is unboxed.**
Bridge unboxes via `place.as_ref().stable(...)`
[`stable/mir.rs:146-151`]:

```rust
mir::StatementKind::SetDiscriminant { place, variant_index } => {
    crate::mir::StatementKind::SetDiscriminant {
        place: place.as_ref().stable(tables, cx),
        variant_index: variant_index.stable(tables, cx),
    }
}
```

The struct-shape (named fields) is preserved verbatim. Only the box
indirection is removed.

Maps to **D-8-1.3** (SetDiscriminant payload shape).

### 4.4 `StorageLive(Local)` / `StorageDead(Local)`

rustc_middle [`syntax.rs:385,388`]: `StorageLive(Local)` /
`StorageDead(Local)` -- already flat.

rustc_public [`body.rs:481-482`]: identical. Bridge passes through
1:1 [`stable/mir.rs:153-159`].

**Cleanup-pass interaction**: the rustc_middle pass
`RemoveStorageMarkers` (in RegseqModel's pinned cleanup-pass
whitelist, per `../../README.md`) removes both `StorageLive` and
`StorageDead` from optimized_mir. rustc_public does not run this
pass on its consumers' behalf -- it preserves whatever phase the
consumer asks for. INFERENCE: rustc_public sees these variants
because some of its consumers care about pre-cleanup MIR
(borrowck-tier analyses); RegseqModel can drop them because its
cleanup-pass discharge eliminates them upstream.

Maps to **D-8-1** + **D-8-1.1** (cardinality justification: variant
*reachable* in rustc_public's input is *not reachable* in
RegseqModel's input).

### 4.5 `Retag(RetagKind, Place)`

rustc_middle [`syntax.rs:401`]: `Retag(RetagKind, Box<Place<'tcx>>)`
-- boxed `Place`.

rustc_public [`body.rs:483`]: `Retag(RetagKind, Place)` -- flat.
Bridge unboxes [`stable/mir.rs:160-162`].

`RetagKind` is 4-variant
[`rustc_public/src/mir/body.rs:441-447`]:

```rust
pub enum RetagKind {
    FnEntry,
    TwoPhase,
    Raw,
    Default,
}
```

rustc_middle [`syntax.rs:392-400`] documents `Retag` as "currently
only interpreted by miri and only generated when `-Z mir-emit-retag`
is passed". For RegseqModel's pinned toolchain, `-Z mir-emit-retag`
is *not* set, so this variant is **bridge-rejection candidate**: it
should never appear in well-formed input and is reject-on-encounter.
HYPOTHESIS: this matches the same "stacked-borrows annotation that
no production compiler emits" treatment stage 7 applied to
`MutBorrowKind::TwoPhaseBorrow` (operationally equivalent to
`Default`, kept for borrowck-internal precision).

rustc_public preserves the variant because some consumers (Miri-like
tools) do generate retags.

Maps to **D-8-1**, **D-8-4** (rejection at the bridge).

### 4.6 `PlaceMention(Place)`

rustc_middle [`syntax.rs:409`]: `PlaceMention(Box<Place<'tcx>>)` --
boxed.

rustc_public [`body.rs:484`]: `PlaceMention(Place)` -- flat. Bridge
unboxes [`stable/mir.rs:163-165`].

`PlaceMention` exists to preserve "a trace of a scrutinee matched
against a wildcard binding... `let _ = PLACE;` desugars to a single
`PlaceMention(PLACE)`" [`rustc_middle/src/mir/syntax.rs:403-408`].
"When executed at runtime, this computes the given place, but then
discards it without doing a load. `let _ = *ptr;` is fine even if the
pointer is dangling." -- so it has runtime semantics (compute the
place expression, discard the value).

INFERENCE: this is a borrowck-adjacent statement that has trivial
runtime semantics (Nop-equivalent for codegen). RegseqModel could
either drop it (relying on `let _ = PLACE` having no observable C
effect) or lower it to a deliberate read (parallel to a `volatile`-
equivalent for register-MMIO contexts where the read *is* the
side-effect). Stage 8 must decide; the variant cannot be silently
elided without a behaviour audit.

Maps to **D-8-1**, **D-8-4**.

### 4.7 `AscribeUserType { place, projections, variance }`

rustc_middle [`syntax.rs:427`]: `AscribeUserType(Box<(Place<'tcx>,
UserTypeProjection)>, ty::Variance)` -- positional with boxed pair.

rustc_public [`body.rs:485`]: `AscribeUserType { place: Place,
projections: UserTypeProjection, variance: Variance }` --
**restructured to a named-field struct, fully unboxed.** Bridge
[`stable/mir.rs:166-172`]:

```rust
mir::StatementKind::AscribeUserType(place_projection, variance) => {
    crate::mir::StatementKind::AscribeUserType {
        place: place_projection.as_ref().0.stable(tables, cx),
        projections: place_projection.as_ref().1.stable(tables, cx),
        variance: variance.stable(tables, cx),
    }
}
```

This is the most aggressive restructuring in the whole `StatementKind`
translation: the rustc_middle tuple-with-trailing-positional layout
becomes a 3-field record. INFERENCE: rustc_public's design preference
for owned-IR clarity wins over wire-compatibility with rustc_middle
when the tuple form is awkward.

`UserTypeProjection` is itself a stripped-down owned type
[`rustc_public/src/mir/body.rs:843-848`]:

```rust
pub struct UserTypeProjection {
    pub base: UserTypeAnnotationIndex,
    pub projection: Opaque,
}
```

The `projection` field is an `Opaque` (debug-string blob, not a
structured projection list). This is a deliberate downgrade:
rustc_public's consumers don't need to walk user-type-projection
internals.

`Variance` is 4-variant [`rustc_public/src/mir/body.rs:449-455`]:
`Covariant | Invariant | Contravariant | Bivariant`. Direct mirror.

**Phase qualification**: rustc_middle [`syntax.rs:426`] says
`AscribeUserType` is "Disallowed after drop elaboration". Same fate
as `FakeRead` -- not reachable in optimized_mir. INFERENCE:
RegseqModel can reject this variant; rustc_public preserves it for
pre-runtime consumers.

Stage 7 prior art [`stage-7/prior-art/rustc.md` §10] already
documents that `body.user_type_annotations.raw.clear()` is run by
`CleanupPostBorrowck` [`cleanup_post_borrowck.rs:81-85`] -- the
*payload index space* is wiped. Whether `AscribeUserType` *statements*
are also wiped (rather than just left dangling with stale indices)
needs verification; cleanup_post_borrowck.rs only NOPs Fake-borrow
assignments and rewrites pointer coercions. **HYPOTHESIS:**
`AscribeUserType` statements are not explicitly removed by any
required pass, so they may persist into optimized_mir with cleared
projection indices. Stage 8 must verify before locking the
rejection rule.

Maps to **D-8-1**, **D-8-4**.

### 4.8 `Coverage(Coverage)`

rustc_middle [`syntax.rs:439-446`]: `Coverage(CoverageKind)` where
`CoverageKind` is a structured enum (not shown here; lives in
`rustc_middle/src/mir/coverage.rs`).

rustc_public [`body.rs:486`]: `Coverage(Coverage)` where the inner
type is a transparent type alias to `Opaque`
[`rustc_public/src/mir/body.rs:425-428`]:

```rust
/// The rustc coverage data structures are heavily tied to internal
/// details of the coverage implementation that are likely to change,
/// and are unlikely to be useful to third-party tools for the
/// foreseeable future.
pub(crate) type Coverage = Opaque;
```

Bridge translates by wrapping with `opaque(...)`
[`stable/mir.rs:173-175`]:

```rust
mir::StatementKind::Coverage(coverage) => {
    crate::mir::StatementKind::Coverage(opaque(coverage))
}
```

The doc comment is the rationale: rustc_public **deliberately
declines to expose CoverageKind's internal structure**. This is a
strict opacity choice -- the variant's existence is preserved (for
visitor exhaustiveness), but its payload is a debug-string blob.

INFERENCE: this is the rustc_public team's prior-art statement
that **a closed-IR mirror can have variants whose payloads are
"present but black-box"** when the upstream design churns too fast
to track. Stage 8's `Coverage` decision (drop entirely vs
preserve-as-opaque vs preserve-structured) maps directly onto this
choice. RegseqModel's input is `optimized_mir` without
`-C instrument-coverage`, so coverage statements **should not appear**
under the pinned configuration -- this becomes a **bridge-rejection
candidate** even more confidently than rustc_public can claim.

Maps to **D-8-1**, **D-8-4**.

### 4.9 `Intrinsic(NonDivergingIntrinsic)`

rustc_middle [`syntax.rs:450`]: `Intrinsic(Box<NonDivergingIntrinsic
<'tcx>>)` -- boxed.

rustc_public [`body.rs:487`]: `Intrinsic(NonDivergingIntrinsic)` --
**flat, the box is removed.** Bridge calls
`intrinstic.stable(tables, cx)` [`stable/mir.rs:176-178`] on the
unboxed inner.

`NonDivergingIntrinsic` is itself a 2-variant enum
[`rustc_public/src/mir/body.rs:464-468`]:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum NonDivergingIntrinsic {
    Assume(Operand),
    CopyNonOverlapping(CopyNonOverlapping),
}
```

Compare rustc_middle [`syntax.rs:485-507`]:

```rust
pub enum NonDivergingIntrinsic<'tcx> {
    Assume(Operand<'tcx>),
    CopyNonOverlapping(CopyNonOverlapping<'tcx>),
}
```

Direct 1:1 mirror. The bridge translation is structural
[`stable/mir.rs:494-518`]:

```rust
impl<'tcx> Stable<'tcx> for mir::NonDivergingIntrinsic<'tcx> {
    type T = crate::mir::NonDivergingIntrinsic;
    fn stable<'cx>(...) -> Self::T {
        use rustc_middle::mir::NonDivergingIntrinsic;
        use crate::mir::CopyNonOverlapping;
        match self {
            NonDivergingIntrinsic::Assume(op) =>
                crate::mir::NonDivergingIntrinsic::Assume(op.stable(tables, cx)),
            NonDivergingIntrinsic::CopyNonOverlapping(copy_non_overlapping) =>
                crate::mir::NonDivergingIntrinsic::CopyNonOverlapping(
                    CopyNonOverlapping {
                        src: copy_non_overlapping.src.stable(tables, cx),
                        dst: copy_non_overlapping.dst.stable(tables, cx),
                        count: copy_non_overlapping.count.stable(tables, cx),
                    }),
        }
    }
}
```

`CopyNonOverlapping` itself is 1:1
[`rustc_public/src/mir/body.rs:457-462`]:

```rust
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CopyNonOverlapping {
    pub src: Operand,
    pub dst: Operand,
    pub count: Operand,
}
```

vs rustc_middle [`syntax.rs:631-638`]:

```rust
pub struct CopyNonOverlapping<'tcx> {
    pub src: Operand<'tcx>,
    pub dst: Operand<'tcx>,
    /// Number of elements to copy from src to dest, not bytes.
    pub count: Operand<'tcx>,
}
```

Same field set, same order, same shapes. Only the `'tcx` is gone and
the Operand becomes the owned form.

The two-level nesting (`Statement -> Intrinsic(NonDivergingIntrinsic)
-> Assume(Operand) | CopyNonOverlapping(CopyNonOverlapping)`) is
*preserved* by rustc_public. INFERENCE: this is intentional. The
"non-diverging" qualifier in the type name expresses an invariant
(these intrinsics never panic / never unwind / always return) that
rustc_public's design preserves *in the type structure* rather than
flattening it away. A consumer that pattern-matches on `Intrinsic(_)`
gets the qualifier as a static guarantee.

Maps to **D-8-1.4** (Intrinsic payload shape and nesting).

### 4.10 `ConstEvalCounter`

rustc_middle [`syntax.rs:456`]: unit variant. Used by const-eval to
count interpreter steps; "Other than in the const eval interpreter,
this is a no-op."

rustc_public [`body.rs:488`]: unit variant, 1:1
[`stable/mir.rs:179`].

Not relevant to runtime MIR. **HYPOTHESIS:** does not appear in
optimized_mir for non-const-eval bodies; **bridge-rejection
candidate** for RegseqModel. rustc_public preserves it because
const-eval-internal consumers exist.

Maps to **D-8-1**, **D-8-4**.

### 4.11 `Nop`

rustc_middle [`syntax.rs:459`]: unit. "No-op. Useful for deleting
instructions without affecting statement indices."

rustc_public [`body.rs:489`]: unit, 1:1
[`stable/mir.rs:184`].

`Nop` is the *fallback* for the rustc_middle ->
rustc_public translation: `BackwardIncompatibleDropHint` collapses to
`Nop` [`stable/mir.rs:181-183`]:

```rust
// BackwardIncompatibleDropHint has no semantics, so it is translated to Nop.
mir::StatementKind::BackwardIncompatibleDropHint { .. } => {
    crate::mir::StatementKind::Nop
}
```

So `Nop` carries traffic from a 13th rustc_middle variant. INFERENCE:
RegseqModel's bridge can do the same collapse. Or, since
RegseqModel's cleanup-pass whitelist includes `RemoveStorageMarkers`
and other simplification passes, **HYPOTHESIS:** all `Nop`
statements may also be removed by upstream optimization (no specific
pass whitelisted -- needs verification). If `Nop` appears in
optimized_mir under the pinned config, it must be emitted as a no-op
in C lowering or filtered at the bridge.

Maps to **D-8-1** + **D-8-1.1** (cardinality: `Nop` reachability
governs whether the variant is needed).

### 4.12 What's gone: `BackwardIncompatibleDropHint`

The 13th rustc_middle variant
[`rustc_middle/src/mir/syntax.rs:466-471`]:

```rust
/// Marker statement indicating where `place` would be dropped.
/// This is semantically equivalent to `Nop`, so codegen and MIRI should
/// interpret this statement as such.
/// The only use case of this statement is for linting in MIR to detect
/// temporary lifetime changes.
BackwardIncompatibleDropHint {
    /// Place to drop
    place: Box<Place<'tcx>>,
    /// Reason for backward incompatibility
    reason: BackwardIncompatibleDropReason,
},
```

The bridge translates it to `Nop`
[`stable/mir.rs:180-183`]. The inline comment is the rationale:
"BackwardIncompatibleDropHint has no semantics, so it is translated
to Nop."

This is the **single explicit drop** in rustc_public's
StatementKind translation. rustc_public's design philosophy here:
**a statement with zero runtime semantics is collapsed**, even if
rustc_middle preserves it for diagnostic-tier tooling (the Edition-2024
lint that detects when temporary lifetimes will change in
backward-incompatible ways). INFERENCE: rustc_public's statement
surface is **carved at the runtime-semantics joint**, not the
diagnostic-tier joint. RegseqModel inherits this carve.

Maps to **D-8-1.1** (cardinality: 12 vs 13).

---

## 5. Visitor surface

The visitor `super_statement` enumerates the 12-variant surface
exhaustively [`rustc_public/src/mir/visit.rs:161-200`]:

```rust
fn super_statement(&mut self, stmt: &$($mutability)? Statement,
                   location: Location) {
    let Statement { kind, span } = stmt;
    self.visit_span(span);
    match kind {
        StatementKind::Assign(place, rvalue) => {
            self.visit_place(place, PlaceContext::MUTATING, location);
            self.visit_rvalue(rvalue, location);
        }
        StatementKind::FakeRead(_, place) | StatementKind::PlaceMention(place) => {
            self.visit_place(place, PlaceContext::NON_MUTATING, location);
        }
        StatementKind::SetDiscriminant { place, .. }
        | StatementKind::Retag(_, place) => {
            self.visit_place(place, PlaceContext::MUTATING, location);
        }
        StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
            self.visit_local(local, PlaceContext::NON_USE, location);
        }
        StatementKind::AscribeUserType { place, projections, variance: _ } => {
            self.visit_place(place, PlaceContext::NON_USE, location);
            self.visit_user_type_projection(projections);
        }
        StatementKind::Coverage(coverage) => visit_opaque(coverage),
        StatementKind::Intrinsic(intrinsic) => match intrinsic {
            NonDivergingIntrinsic::Assume(operand) => {
                self.visit_operand(operand, location);
            }
            NonDivergingIntrinsic::CopyNonOverlapping(CopyNonOverlapping {
                src, dst, count,
            }) => {
                self.visit_operand(src, location);
                self.visit_operand(dst, location);
                self.visit_operand(count, location);
            }
        },
        StatementKind::ConstEvalCounter | StatementKind::Nop => {}
    }
}
```

Notable visitor design choices:

- **`Statement` is destructured into `{ kind, span }` literally** at
  the entry [`visit.rs:162`]. Confirms the `kind` + `span` shape
  (§2) is the externally-promised surface.
- **`PlaceContext` carries access-mode metadata** per variant.
  `Assign` writes -> `MUTATING`. `FakeRead` / `PlaceMention` read ->
  `NON_MUTATING`. `SetDiscriminant` / `Retag` write -> `MUTATING`.
  `StorageLive` / `StorageDead` / `AscribeUserType` -> `NON_USE`
  (debug-only / lifetime-only). The visitor encodes the
  per-variant access semantics that the type doesn't express.
- **`Coverage(coverage) => visit_opaque(coverage)`** -- the visitor
  has a special path for opaque payloads, consistent with
  `Coverage = Opaque` (§4.8).
- **`Intrinsic` flattens to two sub-paths** matching
  `NonDivergingIntrinsic`'s 2 variants. The 3-operand
  `CopyNonOverlapping` case visits `src`, `dst`, `count` in that
  order.
- **`ConstEvalCounter | Nop => {}`** -- empty arms confirm these
  variants have no inner data to visit.
- **`Location(stmt.span)`** is the per-statement location key
  [`visit.rs:142`], reinforcing that **statements are identified by
  span**, not by content-hash. INFERENCE: this is the same identity
  discipline that justifies omitting `Hash` on `StatementKind`
  (§3 INFERENCE).

The visitor doc comment block [`visit.rs:1-37`] makes the
exhaustiveness contract explicit:

> ALL MATCHES MUST BE EXHAUSTIVE IN FIELDS AND VARIANTS. That means
> you never write `..` to skip over fields, nor do you write `_` to
> skip over variants in a `match`.

This is a strong stance: the visitor authors expect the variant set
to be *closed* and *stable* over short-term release windows. Adding
a 13th rustc_public variant would break every external visitor.
INFERENCE: this is why the bridge eats `BackwardIncompatibleDropHint`
to `Nop` instead of adding a passthrough variant -- preserving the
12-variant external contract.

Maps to **D-8-1**, **D-8-4** (visitor as walk-impl reference).

---

## 6. Pretty-printer surface (`pretty_statement`)

rustc_public *does* provide a pretty-printer
[`rustc_public/src/mir/pretty.rs:90-126`]:

```rust
fn pretty_statement<W: Write>(writer: &mut W, statement: &StatementKind)
    -> io::Result<()>
{
    const INDENT: &str = "        ";
    match statement {
        StatementKind::Assign(place, rval) => {
            write!(writer, "{INDENT}{place:?} = ")?;
            pretty_rvalue(writer, rval)?;
            writeln!(writer, ";")
        }
        // FIXME: Add rest of the statements
        StatementKind::FakeRead(cause, place) => {
            writeln!(writer, "{INDENT}FakeRead({cause:?}, {place:?});")
        }
        StatementKind::SetDiscriminant { place, variant_index } => {
            writeln!(writer, "{INDENT}discriminant({place:?}) = {};",
                     variant_index.to_index())
        }
        StatementKind::StorageLive(local) => {
            writeln!(writer, "{INDENT}StorageLive(_{local});")
        }
        StatementKind::StorageDead(local) => {
            writeln!(writer, "{INDENT}StorageDead(_{local});")
        }
        StatementKind::Retag(kind, place) =>
            writeln!(writer, "Retag({kind:?}, {place:?});"),
        StatementKind::PlaceMention(place) => {
            writeln!(writer, "{INDENT}PlaceMention({place:?};")
        }
        StatementKind::ConstEvalCounter => {
            writeln!(writer, "{INDENT}ConstEvalCounter;")
        }
        StatementKind::Nop => writeln!(writer, "{INDENT}nop;"),
        StatementKind::AscribeUserType { .. }
        | StatementKind::Coverage(_)
        | StatementKind::Intrinsic(_) => {
            // FIX-ME: Make them pretty.
            writeln!(writer, "{INDENT}{statement:?};")
        }
    }
}
```

The renderer is invoked from the body-level pretty-printer
[`rustc_public/src/mir/pretty.rs:73-80`]:

```rust
let _ = block
    .statements
    .iter()
    .map(|statement| -> io::Result<()> {
        pretty_statement(writer, &statement.kind)?;
        Ok(())
    })
    .collect::<Vec<_>>();
```

Three renderer-side observations:

1. **Output mimics MIR's standard text format.** `_x = rvalue;`,
   `discriminant(_x) = N;`, `StorageLive(_x);`, etc. This is
   debug-only output -- not a re-parseable serialization
   (rustc_public uses `serde::Serialize` for the round-trippable
   form).

2. **Three variants are partially un-implemented.** `AscribeUserType`
   / `Coverage` / `Intrinsic` fall through to `{statement:?}`
   (Debug-derived) with a `// FIX-ME: Make them pretty.` marker.
   This signals the rustc_public team **does not yet have a stable
   external-text shape for these three** -- consistent with their
   weak runtime relevance.

3. **`pretty_statement` takes `&StatementKind`, not `&Statement`.**
   The wrapping span is dropped at the rendering level. This is
   a minor design tell: `span` is metadata, not a renderable concept
   in the MIR text format.

INFERENCE: the renderer's existence + the `// FIXME: Add rest of the
statements` comment at line 98 indicate that rustc_public considers
a per-variant render to be **part of the contract** but not yet
fully realized. Stage 11 in the RegseqModel project (per `D-8-K`
deferral pattern from D-5-K / D-6-K / D-7-K) inherits this stance:
the renderer is downstream of the variant set being locked.

Maps to **D-8-K**.

---

## 7. Bridge invariants and rejection rules

The bridge translation [`rustc_public/src/unstable/convert/stable/mir.rs:130-187`]
**enforces no invariants on construction** beyond what Rust's type
system itself enforces. There are no `bug!()` calls in the
StatementKind translator, no `assert!` checks on payload contents,
no phase-guard rejection. Every rustc_middle variant either flows
through 1:1, restructures to a flatter shape, or collapses to `Nop`
(only `BackwardIncompatibleDropHint` does this).

The only construction-time invariant on `Body` itself is
[`rustc_public/src/mir/body.rs:55-60`]:

```rust
pub fn new(...) -> Self {
    // If locals doesn't contain enough entries, it can lead to panics in
    // `ret_local`, `arg_locals`, and `inner_locals`.
    assert!(
        locals.len() > arg_count,
        "A Body must contain at least a local for the return value
         and each of the function's arguments"
    );
    ...
}
```

That is, **rustc_public expects callers to construct only
well-formed Bodies** but checks only the locals/arg-count invariant
at the body level. Per-statement well-formedness is delegated
upstream to rustc_middle's MIR validator.

INFERENCE: this is a meaningful rustc_public-side decision. By
*not* re-validating at the bridge, rustc_public:

- Trusts rustc_middle's MIR phase guard to have rejected
  ill-formed `FakeRead` / `AscribeUserType` etc. before the bridge
  sees them.
- Does *not* enforce phase-specific exclusion at the
  rustc_public boundary -- consumers are expected to consult
  `MirPhase` themselves if they care.
- Trades input strictness for surface portability across phases:
  the same `StatementKind` enum serves pre-borrowck consumers
  (who need `FakeRead`, `AscribeUserType`) and post-runtime
  consumers (who don't).

RegseqModel **departs from this stance.** RegseqModel's bridge
consumes only `optimized_mir` (`MirPhase::Runtime(RuntimePhase::
PostCleanup)`) plus a curated cleanup-pass whitelist; the bridge
*can* enforce per-variant phase-guard rejection at construction.
The §17.x `unsupported_construct!` pattern from earlier stages
(stage-7 `D-7-10` for `CopyForDeref` / `ShallowInitBox`, stage-7
§13 for `BorrowKind::Fake`) is the equivalent rejection idiom.

Maps to **D-8-4** (rejection rules at the bridge).

---

## 8. Cross-reference vs `rustc_middle` (drift / lossy / lossless)

Tabulating the 12-variant rustc_public surface against rustc_middle
[`rustc_middle/src/mir/syntax.rs:311-472`]:

| Variant | rustc_middle shape | rustc_public shape | Drift call |
|-|-|-|-|
| `Assign` | `Box<(Place, Rvalue)>` [`syntax.rs:341`] | `(Place, Rvalue)` [`body.rs:478`] | Lossless (unbox) |
| `FakeRead` | `Box<(FakeReadCause, Place)>` [`syntax.rs:358`] | `(FakeReadCause, Place)` [`body.rs:479`] | Lossless (unbox) |
| `SetDiscriminant` | `{ place: Box<Place>, variant_index }` [`syntax.rs:365`] | `{ place: Place, variant_index }` [`body.rs:480`] | Lossless (unbox) |
| `StorageLive` | `(Local)` [`syntax.rs:385`] | `(Local)` [`body.rs:481`] | Identical |
| `StorageDead` | `(Local)` [`syntax.rs:388`] | `(Local)` [`body.rs:482`] | Identical |
| `Retag` | `(RetagKind, Box<Place>)` [`syntax.rs:401`] | `(RetagKind, Place)` [`body.rs:483`] | Lossless (unbox) |
| `PlaceMention` | `(Box<Place>)` [`syntax.rs:409`] | `(Place)` [`body.rs:484`] | Lossless (unbox) |
| `AscribeUserType` | `(Box<(Place, UserTypeProjection)>, Variance)` [`syntax.rs:427`] | `{ place, projections, variance }` [`body.rs:485`] | Lossless restructure (tuple -> struct, unbox) |
| `Coverage` | `(CoverageKind)` [`syntax.rs:439-446`] | `(Opaque)` [`body.rs:486`] | **Lossy** (CoverageKind structure -> debug string) |
| `Intrinsic` | `Box<NonDivergingIntrinsic>` [`syntax.rs:450`] | `(NonDivergingIntrinsic)` [`body.rs:487`] | Lossless (unbox) |
| `ConstEvalCounter` | unit [`syntax.rs:456`] | unit [`body.rs:488`] | Identical |
| `Nop` | unit [`syntax.rs:459`] | unit [`body.rs:489`] | Identical (with fan-in: `BackwardIncompatibleDropHint` collapses here) |
| `BackwardIncompatibleDropHint` | `{ place: Box<Place>, reason }` [`syntax.rs:466-471`] | -- | **Dropped** (collapsed to `Nop`) |

Summary of drift carving:

- **One drop** (`BackwardIncompatibleDropHint` -> `Nop`).
- **One lossy** (`Coverage` -> `Opaque`).
- **Eight unboxed** (six positional, two struct-fielded).
- **Two restructured** (`SetDiscriminant`, `AscribeUserType` -> named
  fields; the latter also unflattens a tuple to a record).
- **Four identical** (the two unit variants and the two `Local`-only
  variants).

INFERENCE: the rustc_public team's design rule for
`StatementKind` translation is:

1. Strip every `Box` -- the owned mirror has no enum-size budget.
2. Convert positional payloads to named fields **only when the
   tuple form is awkward** (`SetDiscriminant` already had named
   fields; `AscribeUserType` did not, and got promoted).
3. Mark internal-detail-prone payloads as `Opaque` rather than
   re-mirroring fragile structure (`Coverage`).
4. Collapse zero-runtime-semantics variants (`BackwardIncompatibleDropHint`
   -> `Nop`).
5. Preserve nested-enum structure where it carries an invariant
   (`NonDivergingIntrinsic` is not flattened into
   `StatementKind::Assume(Operand)` /
   `StatementKind::CopyNonOverlapping(...)` -- the
   "non-diverging" group survives as a sub-enum).

---

## 9. Cross-reference vs RegseqModel candidate (D-8-1.1 = 3 variants)

RegseqModel's stage-8 candidate cardinality is **3 variants**:
`Assign | SetDiscriminant | Intrinsic`. rustc_public's cardinality is
12. The 9-variant gap is the question stage 8 must answer.

Per-variant reachability under RegseqModel's input regime
(`optimized_mir` at `mir_opt_level=0` + cleanup-pass whitelist:
`RemoveStorageMarkers, RemoveZsts, SimplifyLocals,
ReorderBasicBlocks, ReorderLocals, CheckAlignment, CheckNull,
CheckEnums`):

| Variant | Reachable under RegseqModel input? | Evidence |
|-|-|-|
| `Assign` | YES (essential) | Universal load-bearing |
| `FakeRead` | NO | "Disallowed after drop elaboration" [`syntax.rs:357`]; drop elaboration runs before optimized_mir |
| `SetDiscriminant` | YES | Required for enum construction in runtime MIR |
| `StorageLive` | NO | `RemoveStorageMarkers` is in the whitelist; removed before bridge sees body |
| `StorageDead` | NO | Same as `StorageLive` |
| `Retag` | NO | Only emitted under `-Z mir-emit-retag` [`syntax.rs:392-400`]; not set in pinned toolchain |
| `PlaceMention` | HYPOTHESIS: rare but possible | "let _ = PLACE" desugar; no required pass removes it. Stage 8 must verify behaviour-preserving treatment |
| `AscribeUserType` | HYPOTHESIS: not reachable | "Disallowed after drop elaboration" [`syntax.rs:426`]; verify no statement-level elaboration |
| `Coverage` | NO | `-C instrument-coverage` not set in pinned toolchain; coverage statements not emitted |
| `Intrinsic` | YES | `Assume` and `CopyNonOverlapping` are emitted from runtime intrinsic lowering |
| `ConstEvalCounter` | NO | Const-eval-only [`rustc_middle/src/mir/syntax.rs:452-456`]; runtime MIR for non-const-eval bodies should not contain it |
| `Nop` | HYPOTHESIS: possible | `RemoveStorageMarkers` may leave Nops; SimplifyLocals does not specifically remove them. Stage 8 must verify |

The 3-variant candidate (`Assign | SetDiscriminant | Intrinsic`)
matches the YES rows. The HYPOTHESIS rows (`PlaceMention`,
`AscribeUserType`, `Nop`) are the open verification points -- if
they are reachable, RegseqModel must either widen the cardinality or
specify bridge-side handling (rejection or collapse).

INFERENCE: **rustc_public can have 12 variants because its input is
multi-phase MIR**; RegseqModel can have far fewer variants because
its input is single-phase optimized MIR. The 9 dropped variants
correspond exactly to:

- 3 borrowck-phase artefacts (`FakeRead`, `AscribeUserType`,
  `PlaceMention` -- the latter only borderline runtime-relevant)
- 2 storage-marker variants (`StorageLive`, `StorageDead`) removed
  by the cleanup whitelist
- 1 stacked-borrows annotation (`Retag`) gated behind a debug flag
- 1 coverage variant gated behind `-C instrument-coverage`
- 1 const-eval-internal counter (`ConstEvalCounter`)
- 1 deletion marker (`Nop`) potentially removable by simplification

Each of these maps to **a rustc-side configuration choice
RegseqModel doesn't make**. RegseqModel's tighter cardinality is the
**direct consequence of input regime, not of design preference.**

This mirrors the stage-7 prior-art finding [`stage-7/prior-art/rustc.md`
§13]: the cleanup-pass whitelist plus phase-guard discharge eliminate
variants from the input, allowing the model surface to be tighter
than rustc_middle's general-purpose 13-variant surface.

Maps to **D-8-1.1** (cardinality justification) + **D-8-4**
(per-variant rejection rules).

---

## 10. Body-level interactions (Body, BasicBlock, debuginfos)

The Body translation site
[`rustc_public/src/unstable/convert/stable/mir.rs:15-49`]:

```rust
impl<'tcx> Stable<'tcx> for mir::Body<'tcx> {
    type T = crate::mir::Body;
    fn stable<'cx>(...) -> Self::T {
        crate::mir::Body::new(
            self.basic_blocks
                .iter()
                .map(|block| crate::mir::BasicBlock {
                    terminator: block.terminator().stable(tables, cx),
                    statements: block.statements.iter()
                        .map(|statement| statement.stable(tables, cx))
                        .collect(),
                })
                .collect(),
            self.local_decls.iter().map(|decl| ...).collect(),
            self.arg_count,
            self.var_debug_info.iter().map(|info| info.stable(tables, cx)).collect(),
            self.spread_arg.stable(tables, cx),
            self.span.stable(tables, cx),
        )
    }
}
```

Three observations relevant to stage 8:

1. **`block.statements.iter().map(...)` flows directly into the
   owned `Vec<Statement>`** with no per-statement filtering, no
   per-statement annotation. The bridge is a structural mirror;
   it does not editorialize the statement stream.

2. **`block.statements[i].debuginfos` is silently skipped**
   (statements are translated through `Statement::stable`
   [`stable/mir.rs:68-79`] which only reads `kind` and `source_info`).
   Same for `block.after_last_stmt_debuginfos`
   [`rustc_middle/src/mir/mod.rs:1314`] -- not even visited.

3. **`var_debug_info` is preserved** -- distinguishing
   variable-level debug metadata (preserved with full SourceInfo) from
   per-statement debug metadata (dropped). This is the same carve as
   §2's "scope dropped on Statement, preserved on VarDebugInfo".

INFERENCE: the rustc_public team's body-level information model is
a **reduced shape** -- per-statement debug info is judged
non-essential for the owned-IR contract, while per-variable debug
info is judged essential. Stage 8 inherits this carve more or less
directly: RegseqModel's C lowering needs variable names for
generated identifiers (preserve `VarDebugInfo`) but does not need
per-assignment-statement debug records.

Maps to **D-8-3** (Statement wrapper -- specifically the `kind +
span` minimal shape).

---

## 11. Compiler interface dispatch

The `with(|cx| ...)` pattern is rustc_public's mechanism for
invoking the bridge from outside a `'tcx` context
[`rustc_public/src/compiler_interface.rs:864-871`]:

```rust
pub(crate) fn with<R>(f: impl for<'tcx>
    FnOnce(&CompilerInterface<'tcx>) -> R) -> R
{
    assert!(TLV.is_set());
    TLV.with(|tlv| {
        let ptr = tlv.get();
        assert!(!ptr.is_null());
        f(unsafe { *(ptr as *const &CompilerInterface<'_>) })
    })
}
```

The `TLV` (thread-local value) carries the active `CompilerInterface`
which holds `tables: RefCell<Tables<'tcx, BridgeTys>>` and `cx:
RefCell<CompilerCtxt<'tcx, BridgeTys>>`
[`rustc_public/src/compiler_interface.rs:75-78`]:

```rust
pub tables: RefCell<Tables<'tcx, BridgeTys>>,
pub cx: RefCell<CompilerCtxt<'tcx, BridgeTys>>,
```

Tying back to **D-8-2** (interning vs inline): every owned type that
*does* intern (DefId, Span, Ty, Const, etc.) routes lookups through
this TLV-carried `Tables`. Statements *do not* — they are inline
data and need no `with(...)` dispatch.

INFERENCE: this is structural confirmation that statements live in
the **owned-data half** of rustc_public's two-tier model, not the
**interned-handle half**. The interned half exists for
identity-bearing data (ContainerType identifiers, content-hashable
constants); the owned half exists for everything positionally-
identified, which includes statements.

Maps to **D-8-2**.

---

## 12. Synthesis: what rustc_public's design says about stage 8

rustc_public is the strongest single piece of prior art for the
stage-8 owned-IR question, because it is the *only* in-tree owned,
serializable mirror of rustc MIR -- and it has been refined by the
Stable-MIR project's own multi-year iteration.

What the rustc_public team's design choices say about their beliefs:

1. **`StatementKind` is a 12-variant closed enum, not a closed
   constellation of types.** They believe statements are best
   modelled as a single sum type with per-variant payloads, not as
   a trait-object + per-variant struct. The exhaustiveness contract
   in the visitor [`visit.rs:23-36`] is the structural commitment.

2. **The `kind + span` shape on the `Statement` wrapper is
   sufficient.** rustc_middle's `source_info.scope` and
   `debuginfos: StmtDebugInfos` are deliberately dropped at the
   bridge. They believe per-statement scope and per-statement
   debug info are *non-essential* for an owned-IR mirror -- a
   strong structural claim.

3. **Owned IR can flatten what interned IR has to box.** Every
   `Box` in rustc_middle's `StatementKind` payloads is removed at
   the bridge. They believe heap-indirection inside the variants
   is a size-budget concession unique to the `'tcx`-interned
   regime, not a structural property of the IR.

4. **Variants are preserved unless they have zero runtime semantics
   *and* zero structural information.** Only
   `BackwardIncompatibleDropHint` meets this bar (it is
   "semantically equivalent to `Nop`" per
   [`syntax.rs:462`] and carries only a diagnostic-tier
   "reason"). Every other variant -- even the ones with weak
   runtime relevance like `FakeRead`, `AscribeUserType`,
   `Coverage`, `ConstEvalCounter` -- is preserved in the surface.

5. **Phase-guard rejection is left to the upstream MIR
   validator, not enforced at the bridge.** They believe the cost
   of re-validating at the boundary outweighs the benefit, given
   that consumers can consult `MirPhase` themselves if needed.

6. **Statements are inline `Vec<Statement>` data, never
   interned.** They believe statement *position* is the relevant
   identity, not statement *content* -- consistent with no `Hash`
   derive on `Statement` / `StatementKind`.

7. **Coverage payloads are too internal-volatile to mirror.**
   `Coverage = Opaque` is the explicit fallback for upstream
   structures the rustc_public team does not commit to track.

What RegseqModel inherits:

- **Statement-as-sum-type discipline.** Stage 8's candidate of 3
  closed variants is the same shape, narrower set.
- **Owned-IR-flattens-boxed-IR principle.** Stage 8 should not
  re-introduce `Box` inside `Statement` payloads; the
  `Assign(Place, Rvalue)` flat shape is the rustc_public-confirmed
  baseline.
- **`kind + span` shape on the wrapper.** rustc_public confirms
  that the `source_info.scope` / `debuginfos` slots are non-load-
  bearing for owned-IR consumers. RegseqModel can adopt the same
  carve.
- **Inline `Vec<Statement>` storage in the body.** No
  StatementInterner.

What RegseqModel departs from:

- **Tighter cardinality (3 vs 12).** RegseqModel's input is single-
  phase optimized MIR after a curated cleanup pass; rustc_public's
  input is multi-phase MIR. The 9 dropped variants are reachable
  in rustc_public's regime but unreachable in RegseqModel's.
- **Bridge-side rejection of unreachable variants.** RegseqModel
  applies the `unsupported_construct!` discipline (per stages 5-7)
  rather than rustc_public's "preserve everything; consumer
  decides" stance. The §17 rejection sites are the rustc_middle
  equivalents to rustc_public's silent passthrough.
- **No `Coverage = Opaque` escape hatch.** RegseqModel rejects
  Coverage outright; under the pinned toolchain it cannot appear.
  rustc_public's Opaque mechanism is a strict-superset capability
  RegseqModel does not exercise.

Maps across the namespace:

| Tag | rustc_public evidence anchor | RegseqModel position |
|-|-|-|
| **D-8-1** | 12-variant enum [`body.rs:476-490`] | 3-variant subset |
| **D-8-1.1** | 12 variants, drops `BackwardIncompatibleDropHint` only | 3 variants, drops 9 more by phase-guard discharge |
| **D-8-1.2** | `Assign(Place, Rvalue)` flat [`body.rs:478`] | Adopt flat shape verbatim |
| **D-8-1.3** | `SetDiscriminant { place, variant_index }` [`body.rs:480`] | Adopt struct-shape verbatim |
| **D-8-1.4** | `Intrinsic(NonDivergingIntrinsic)` with nested 2-variant + 3-field `CopyNonOverlapping` [`body.rs:464-468, 457-462`] | Adopt nested shape; preserves "non-diverging" group invariant |
| **D-8-2** | Inline `Vec<Statement>` in `BasicBlock` [`body.rs:135`]; no StatementInterner in `Tables` [`bridge/lib.rs:45-54`] | Adopt inline storage |
| **D-8-3** | `Statement { kind, span }` [`body.rs:470-474`]; drops `source_info.scope` and `debuginfos` at bridge [`stable/mir.rs:68-79`] | Adopt 2-field shape |
| **D-8-4** | No bridge invariants beyond Body-level locals/arg_count [`body.rs:55-60`]; no per-variant rejection | Depart: enforce phase-guard rejection at the bridge |
| **D-8-K** | `pretty_statement` exists [`pretty.rs:90-126`]; partial coverage with `// FIXME` markers | Defer to stage 11 (consistent with D-5-K / D-6-K / D-7-K precedent) |

This rustc_public.md is the kickoff evidence pass; mid-stage deeper
reads on specific D-tags will land additional citations as needed
(e.g., for `D-8-1.1` `PlaceMention` / `AscribeUserType` / `Nop`
reachability under `mir_opt_level=0`, for `D-8-4` exact
`unsupported_construct!` site shape parity with stages 5-7).
