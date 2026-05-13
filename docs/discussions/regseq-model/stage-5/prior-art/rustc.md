# Stage 5 -- Prior Art: rustc primary source

Primary-source rustc evidence for `mir::Place<'tcx>` and
`mir::PlaceElem<'tcx>`, scoped to what stage 5's first decision
cluster D-5-1 (core vocabulary) needs. Fresh pass per
`../../README.md` §2 methodology rule (do not reuse stage-0
summaries).

All paths below are relative to the pinned-toolchain root:

```
~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/
```

(See `../../rustc-references.md` for the canonical statement of
this root path.)

Every factual claim cites a `[file:line]` in that pinned source.
Hypotheses are flagged. Inferences from one site to another are
labelled INFERENCE.

---

## 1. `Place<'tcx>` struct shape

`Place<'tcx>` is a 2-field struct with a head `Local` and a
projection slice that is interned in the typing context
[`rustc_middle/src/mir/syntax.rs:1188-1194`]:

```rust
#[derive(Copy, Clone, PartialEq, Eq, Hash, TyEncodable, HashStable, TypeFoldable, TypeVisitable)]
pub struct Place<'tcx> {
    pub local: Local,

    /// projection out of a place (access a field, deref a pointer, etc)
    pub projection: &'tcx List<PlaceElem<'tcx>>,
}
```

Key facts:

- `Place` is `Copy` (both fields are `Copy`: `Local` is a u32
  newtype index, `&'tcx List<_>` is a reference).
- The `projection` slice is shared via `'tcx`-arena interning
  (see Section 6). Two `Place` values with the same head and
  the same projection list share the same `&List<_>` pointer.
- The `Place` struct itself is NOT interned -- only the
  projection list is. `Place` values are constructed inline at
  use sites.
- `PartialEq` / `Eq` / `Hash` are derived: identity is
  structural over `(local, projection_pointer)`. Because the
  projection list is interned, structural equality on the
  pointer is equivalent to structural equality on the slice
  contents.

The companion type `PlaceRef<'tcx>` is the borrowed form
[`rustc_middle/src/mir/statement.rs:351-355`]:

```rust
pub struct PlaceRef<'tcx> {
    pub local: Local,
    pub projection: &'tcx [PlaceElem<'tcx>],
}
```

`Place::as_ref()` produces a `PlaceRef` cheaply
[`rustc_middle/src/mir/statement.rs:406-409`]; most read-only
walks use `PlaceRef`. RegseqModel's stage-5 spec only needs to
reflect `Place` (the storage form); `PlaceRef` is a borrow-only
ergonomic helper.

---

## 2. `PlaceElem<'tcx>` -- the closed variant set

`PlaceElem<'tcx>` is a type alias over the generic
`ProjectionElem<V, T>`
[`rustc_middle/src/mir/syntax.rs:1287-1289`]:

```rust
/// Alias for projections as they appear in places, where the base is a place
/// and the index is a local.
pub type PlaceElem<'tcx> = ProjectionElem<Local, Ty<'tcx>>;
```

The generic `ProjectionElem<V, T>` enum
[`rustc_middle/src/mir/syntax.rs:1196-1285`] has **8 variants
in source-declaration order**:

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[derive(TyEncodable, TyDecodable, HashStable, TypeFoldable, TypeVisitable)]
pub enum ProjectionElem<V, T> {
    Deref,
    Field(FieldIdx, T),
    Index(V),
    ConstantIndex {
        offset: u64,
        min_length: u64,
        from_end: bool,
    },
    Subslice {
        from: u64,
        to: u64,
        from_end: bool,
    },
    Downcast(Option<Symbol>, VariantIdx),
    OpaqueCast(T),
    UnwrapUnsafeBinder(T),
}
```

(Index by `V`, `Field`/`OpaqueCast`/`UnwrapUnsafeBinder` by `T`.
For `PlaceElem<'tcx>`, `V = Local` and `T = Ty<'tcx>`.)

NOTE: The `try_map` impl
[`rustc_middle/src/mir/statement.rs:323-344`] confirms the
8-variant set by exhaustive match -- `Deref`, `Downcast`,
`Field`, `ConstantIndex`, `Subslice`, `OpaqueCast`,
`UnwrapUnsafeBinder`, `Index`. All 8 variants are listed.

WARN: This contradicts a list referenced in
`../status.md` "Open decisions" and in `../memory.md`
Section 2 ("Deref, Field, Index, ConstantIndex, Subslice,
Downcast, OpaqueCast, Subtype"). Two corrections are needed:

1. `Subtype` is **not** a `PlaceElem` variant -- it is a
   `CastKind` variant on `Rvalue::Cast`
   [`rustc_middle/src/mir/syntax.rs:1525`]:
   ```rust
   /// A `Subtype` cast is applied to any `StatementKind::Assign` where
   /// type of lvalue doesn't match the type of rvalue, the primary goal is making subtyping
   /// explicit during optimizations and codegen.
   ...
   Subtype,
   ```
   `Subtype` lives inside the `CastKind` enum (an Rvalue
   concept, stage 7), NOT inside `ProjectionElem` (stage 5).

2. `UnwrapUnsafeBinder(T)` IS a `PlaceElem` variant that the
   D-5-1 status enumeration omits. It was added for the
   `unsafe_binders` feature (still gated; see Section 9). It
   carries a `Ty<'tcx>` payload (the wrapped inner type the
   binder unwraps to).

Stage 5's vocabulary count is therefore **8 variants in rustc's
projection enum**, of which D-5-1 must decide which are
**reachable in our `optimized_mir` input** (see Section 7),
which are **absent due to solver/phase behavior** (e.g.,
`OpaqueCast` -- absent under both solver paths per H1
REFINED), and which are **rejected if encountered as a
closed-vocabulary policy** (e.g., `UnwrapUnsafeBinder` -- has
no MirPhase restriction so would reach input unchanged if a
driver opted into the unstable feature, but no v1 driver use
case so rejected per H2 REFINED + the "no defensive
fallbacks" methodology rule).

---

## 3. Per-variant payload semantics

Reading the variant docs in
[`rustc_middle/src/mir/syntax.rs:1196-1285`] plus the
type-narrowing arms in
[`rustc_middle/src/mir/statement.rs:214-256`] and the codegen
walk in
[`rustc_codegen_ssa/src/mir/place.rs:352-401`]:

### 3.1 `Deref`

No payload [`rustc_middle/src/mir/syntax.rs:1199`]. The doc
above the `Place` struct
[`rustc_middle/src/mir/syntax.rs:1180-1187`] explains:

> On a `Deref` projection, we do an actual load of the inner
> place, with all the usual consequences (the inner place must
> be based on an aligned pointer, it must point to allocated
> memory, the aliasing model must allow reads, this must not be
> a data race).

Codegen lowers `Deref` to `bx.load_operand(cg_base).deref(...)`
[`rustc_codegen_ssa/src/mir/place.rs:354`].

`Deref` is the only variant flagged "indirect" by
`is_indirect()`
[`rustc_middle/src/mir/statement.rs:262-274`] -- meaning all
other variants refer to the same memory region as the base.

### 3.2 `Field(FieldIdx, T)`

`FieldIdx` is a `newtype_index!` u32 newtype
[`rustc_abi/src/layout/ty.rs:36-40`] indexing into the
field-vector of either:

- a non-enum ADT's single variant (struct, union),
- a specific enum variant after `Downcast`,
- a tuple,
- a closure's `upvar_tys`,
- a coroutine's `upvar_tys` or `prefix_tys`,

per the dispatch in
[`rustc_middle/src/mir/statement.rs:118-173`] (`PlaceTy::field_ty`).

The second slot `T` (which is `Ty<'tcx>` for `PlaceElem<'tcx>`)
is the field's type, stored inline. The doc on `field_ty`
[`rustc_middle/src/mir/statement.rs:111-117`] notes:

> Most clients of `PlaceTy` can instead just extract the relevant type
> directly from their `PlaceElem`, but some instances of
> `ProjectionElem<V, T>` do not carry a `Ty` for `T`.
> Note that the resulting type has not been normalized.

So `Field(FieldIdx, Ty)` carries the post-narrowing type
**inline**, redundantly with what `field_ty` would compute. This
makes type narrowing cheap for `Field` projections (no need to
re-derive from layout each step). The `projection_ty` arm
[`rustc_middle/src/mir/statement.rs:241-246`] confirms: it
calls `handle_field` which can simply return `fty` for the
`PlaceElem<'tcx>` case (see the `projection_ty` wrapper at
[`rustc_middle/src/mir/statement.rs:186-192`] passing
`|_, _, _, ty| ty`).

The `ProjectionKind` alias
[`rustc_middle/src/mir/statement.rs:347-349`] strips this `T` to
`()`:

```rust
pub type ProjectionKind = ProjectionElem<(), ()>;
```

so `(FieldIdx, ())` is sufficient when only the structural shape
matters.

### 3.3 `Index(V)`

For `PlaceElem<'tcx>`, `V = Local`
[`rustc_middle/src/mir/syntax.rs:1289`]. The doc
[`rustc_middle/src/mir/syntax.rs:1209-1222`] explains:

> Index into a slice/array.
>
> Note that this does not also dereference, and so it does not
> exactly correspond to slice indexing in Rust. In other words,
> ... `x[i]` is turned into a `Deref` followed by an `Index`,
> not just an `Index`.

The `Local` payload is the head of a separate `Place::from(local)`
that holds the runtime index value. Codegen at
[`rustc_codegen_ssa/src/mir/place.rs:369-374`] confirms:

```rust
mir::ProjectionElem::Index(index) => {
    let index = &mir::Operand::Copy(mir::Place::from(index));
    let index = self.codegen_operand(bx, index);
    let llindex = index.immediate();
    cg_base.project_index(bx, llindex)
}
```

The codegen wraps the `Local` into a fresh `Place` then loads
it as an `Operand::Copy`. From the IR-shape perspective, `Index`
**carries a `Local`, not an `Operand`** -- a load-bearing fact
for stage 5: `Place` does not have a circular dependency on
stage-6 `Operand`. The runtime index value must be produced by
a separately-tracked local (typically a `_N` containing the
index).

### 3.4 `ConstantIndex { offset, min_length, from_end }`

Three fields, all primitive [`rustc_middle/src/mir/syntax.rs:1239-1251`]:

- `offset: u64` -- a 0-based offset from the start
  (`from_end == false`) or 1-based offset from the end
  (`from_end == true`).
- `min_length: u64` -- the indexed thing must be at least this
  long, otherwise UB. For arrays, this is always the exact
  length.
- `from_end: bool` -- always `false` when indexing an array.

The doc above says:

> These endpoint-relative indices are generated by slice/array
> patterns.

Worked example
[`rustc_middle/src/mir/syntax.rs:1232-1238`]:

```
[X, _, .., _, _] => { offset: 0, min_length: 4, from_end: false },
[_, X, .., _, _] => { offset: 1, min_length: 4, from_end: false },
[_, _, .., X, _] => { offset: 2, min_length: 4, from_end: true },
[_, _, .., _, X] => { offset: 1, min_length: 4, from_end: true },
```

Codegen
[`rustc_codegen_ssa/src/mir/place.rs:375-384`] dispatches
on `from_end`: false case loads `offset` as `usize`; true case
computes `len - offset`.

### 3.5 `Subslice { from, to, from_end }`

[`rustc_middle/src/mir/syntax.rs:1252-1267`]:

- `from: u64` -- start offset.
- `to: u64` -- end offset.
- `from_end: bool` -- whether `to` counts from the start or end
  of the array/slice. For `PlaceElem`s, `true` iff the base is
  a slice; `false` for arrays.

Doc:

> If `from_end` is true `slice[from..slice.len() - to]`.
> Otherwise `array[from..to]`.

Doc also notes a phase-dependent restriction:

> This projection cannot have `ConstantIndex` or additional
> `Subslice` projections after it before runtime MIR.

Combined with the `RuntimePhase::Initial` allowance at
[`rustc_middle/src/mir/syntax.rs:140`]:

> And the following variants are allowed:
> * `PlaceElem::ConstantIndex` / `PlaceElem::Subslice` after
>   `PlaceElem::Subslice`

INFERENCE: in our `optimized_mir` input
(`MirPhase::Runtime(RuntimePhase::Optimized)`, see Section 7),
`Subslice` followed by `ConstantIndex` or another `Subslice` IS
permitted. The pre-runtime restriction is lifted.

The narrowing arm
[`rustc_middle/src/mir/statement.rs:224-237`] computes:

```rust
ProjectionElem::Subslice { from, to, from_end } => {
    PlaceTy::from_ty(match structurally_normalize(self.ty).kind() {
        ty::Slice(..) => self.ty,
        ty::Array(inner, _) if !from_end => Ty::new_array(tcx, *inner, to - from),
        ty::Array(inner, size) if from_end => {
            let size = size.try_to_target_usize(tcx)
                .expect("expected subslice projection on fixed-size array");
            let len = size - from - to;
            Ty::new_array(tcx, *inner, len)
        }
        _ => bug!("cannot subslice non-array type: `{:?}`", self),
    })
}
```

The result type for slices is unchanged (`Slice`); for arrays
it is a fresh array of the corresponding length.

### 3.6 `Downcast(Option<Symbol>, VariantIdx)`

[`rustc_middle/src/mir/syntax.rs:1269-1274`]:

> "Downcast" to a variant of an enum or a coroutine.
>
> The included Symbol is the name of the variant, used for
> printing MIR.
>
> This operation itself is never UB, all it does is change the
> type of the place.

`VariantIdx` is the source-order variant index
[`rustc_abi/src/layout/ty.rs:60-67`] -- always `0..variant_count`
even for enums with custom discriminants and uninhabited
variants. It also carries `FIRST_VARIANT = 0`.

The optional `Symbol` is **printing-only** -- per the doc above.
Stage 5 may safely drop it on translation, since the variant is
identified by `VariantIdx` against the enum's variant table
(stage-3-pinned). Keeping it would marginally help debug
rendering at the cost of carrying a transient name; dropping
it is the C-semantics-preserving choice (no name in C; the
variant arm is identified by index).

The narrowing arm
[`rustc_middle/src/mir/statement.rs:238-240`] does NOT change
the place's type; it only sets `variant_index = Some(index)`:

```rust
ProjectionElem::Downcast(_name, index) => {
    PlaceTy { ty: self.ty, variant_index: Some(index) }
}
```

This is the source of the special "downcast carries an extra
variant_index" piece of state on `PlaceTy<'tcx>`
[`rustc_middle/src/mir/statement.rs:94-103`]:

```rust
pub struct PlaceTy<'tcx> {
    pub ty: Ty<'tcx>,
    /// Downcast to a particular variant of an enum or a coroutine, if included.
    pub variant_index: Option<VariantIdx>,
}
```

The narrowing core also asserts that after a `Downcast`, the
only legal next projection is `Field`
[`rustc_middle/src/mir/statement.rs:211-213`]:

```rust
if self.variant_index.is_some() && !matches!(elem, ProjectionElem::Field(..)) {
    bug!("cannot use non field projection on downcasted place")
}
```

So `Downcast(_, v)` followed by `Field(f, t)` is the canonical
pattern for accessing an enum variant's field at index `f`. Any
other projection following a `Downcast` is a bridge bug at the
rustc level.

### 3.7 `OpaqueCast(T)`

[`rustc_middle/src/mir/syntax.rs:1276-1280`]:

> Like an explicit cast from an opaque type to a concrete type,
> but without requiring an intermediate variable.
>
> This is unused with `-Znext-solver`.

Generated during MIR build (see e.g.
[`rustc_mir_build/src/builder/expr/as_place.rs:107`,
`241-242`, `804`,
`rustc_mir_build/src/builder/matches/match_pair.rs:117`]).
Stripped by the `PostAnalysisNormalize` pass at the runtime
lowering boundary
[`rustc_mir_transform/src/post_analysis_normalize.rs:35-59`]:

```rust
fn visit_place(...) {
    if !self.tcx.next_trait_solver_globally() {
        // `OpaqueCast` projections are only needed if there are opaque types on which projections
        // are performed. After the `PostAnalysisNormalize` pass, all opaque types are replaced with their
        // hidden types, so we don't need these projections anymore.
        if place.projection.iter().any(|elem| matches!(elem, ProjectionElem::OpaqueCast(_))) {
            place.projection = self.tcx.mk_place_elems(
                &place.projection.into_iter()
                    .filter(|elem| !matches!(elem, ProjectionElem::OpaqueCast(_)))
                    .collect::<Vec<_>>(),
            );
        };
    }
    self.super_place(place, _context, _location);
}
```

The MirPhase declaration confirms `OpaqueCast` is **disallowed
at `RuntimePhase::Initial` and later**
[`rustc_middle/src/mir/syntax.rs:127-134`]:

```rust
pub enum RuntimePhase {
    /// In addition to the semantic changes, beginning with this phase, the following variants are
    /// disallowed:
    /// * `TerminatorKind::Yield`
    /// * `TerminatorKind::CoroutineDrop`
    /// * `Rvalue::Aggregate` for any `AggregateKind` except `Array`
    /// * `Rvalue::CopyForDeref`
    /// * `PlaceElem::OpaqueCast`
    /// * `LocalInfo::DerefTemp`
    ...
}
```

And codegen panics on `OpaqueCast`
[`rustc_codegen_ssa/src/mir/place.rs:363-365`]:

```rust
mir::ProjectionElem::OpaqueCast(ty) => {
    bug!("encountered OpaqueCast({ty}) in codegen")
}
```

CONCLUSION (H1 REFINED, see "Hypothesis cross-check
results"): in our `optimized_mir` input (which is
`MirPhase::Runtime(RuntimePhase::Optimized)`, see Section 7),
**`OpaqueCast` will not appear under either solver path**.
Under the legacy solver, it is constructed during MIR build
and stripped at the analysis-to-runtime boundary by
`PostAnalysisNormalize`. Under the next solver
(`next_trait_solver_globally()`), the construction site at
`rustc_mir_build/src/builder/matches/match_pair.rs:104-118`
is symmetrically gated, so `OpaqueCast` is never even
constructed. Either way, `OpaqueCast` is absent from our
input -- D-5-1 rejects it as a closed-vocabulary diagnostic.

### 3.8 `UnwrapUnsafeBinder(T)`

[`rustc_middle/src/mir/syntax.rs:1282-1284`]:

> A transmute from an unsafe binder to the type that it wraps.
> This is a projection of a place, so it doesn't necessarily
> constitute a move out of the binder.

The narrowing arm shares logic with `OpaqueCast`
[`rustc_middle/src/mir/statement.rs:247-252`]:

```rust
ProjectionElem::OpaqueCast(ty) => PlaceTy::from_ty(handle_opaque_cast_and_subtype(ty)),

// FIXME(unsafe_binders): Rename `handle_opaque_cast_and_subtype` to be more general.
ProjectionElem::UnwrapUnsafeBinder(ty) => {
    PlaceTy::from_ty(handle_opaque_cast_and_subtype(ty))
}
```

Codegen handles it via `project_type`
[`rustc_codegen_ssa/src/mir/place.rs:366-368`]:

```rust
mir::ProjectionElem::UnwrapUnsafeBinder(ty) => {
    cg_base.project_type(bx, self.monomorphize(ty))
}
```

This is a recent addition tied to the `unsafe_binders`
feature gate. CONCLUSION (H2 REFINED, see "Hypothesis
cross-check results"): the Regseq DSL surface does NOT
formally exclude `unsafe_binders` -- `RegseqCallbacks` /
`RegseqAwareCallbacks` carry no feature-gate allow/deny list
(`crates/regseq-driver/src/callbacks.rs:60-119`), so a driver
could in principle write `#![feature(unsafe_binders)]`. But
no plausible device-driver pattern needs `unsafe<...>` types
(the feature targets advanced lifetime patterns in libraries,
not `#![no_std]` driver code), and the MirPhase enumerations
(`syntax.rs:107-148`) do NOT list `UnwrapUnsafeBinder` as
disallowed in any phase, so it would reach our input
unchanged if a driver opted in. D-5-1 chose option (a):
reject `UnwrapUnsafeBinder` outright as a closed-vocabulary
diagnostic. Rationale: "no defensive fallbacks; the toolchain
is pinned" applies -- carry no slot for a variant we have no
use case for. If a future driver does opt in, the
toolchain-bump methodology event triggers spec revision.

---

## 4. Type-narrowing helpers

The canonical "what type do you get after walking this
projection" entry points live on `Place` and `PlaceTy<'tcx>`.

### 4.1 `Place::ty` / `Place::ty_from`

[`rustc_middle/src/mir/statement.rs:435-452`]:

```rust
pub fn ty_from<D>(
    local: Local,
    projection: &[PlaceElem<'tcx>],
    local_decls: &D,
    tcx: TyCtxt<'tcx>,
) -> PlaceTy<'tcx>
where
    D: ?Sized + HasLocalDecls<'tcx>,
{
    PlaceTy::from_ty(local_decls.local_decls()[local].ty)
        .multi_projection_ty(tcx, projection)
}

pub fn ty<D: ?Sized>(&self, local_decls: &D, tcx: TyCtxt<'tcx>) -> PlaceTy<'tcx>
where
    D: HasLocalDecls<'tcx>,
{
    Place::ty_from(self.local, self.projection, local_decls, tcx)
}
```

The pattern: take the head local's type from `local_decls`,
fold the projection chain through `multi_projection_ty`. Stage
5's place-validation walk in §3.3 Phase B will look just like
this -- start from the local's type, fold each `PlaceElem`
through a per-variant narrowing function.

### 4.2 `PlaceTy::multi_projection_ty`

[`rustc_middle/src/mir/statement.rs:175-181`]:

```rust
pub fn multi_projection_ty(
    self,
    tcx: TyCtxt<'tcx>,
    elems: &[PlaceElem<'tcx>],
) -> PlaceTy<'tcx> {
    elems.iter().fold(self, |place_ty, &elem| place_ty.projection_ty(tcx, elem))
}
```

Plain left-fold: each step's output `PlaceTy` becomes the next
step's input. This is the linear walk that stage-5's spec
should describe (without prescribing implementation).

### 4.3 `PlaceTy::projection_ty` / `projection_ty_core`

`projection_ty` is a thin wrapper
[`rustc_middle/src/mir/statement.rs:183-192`]:

```rust
pub fn projection_ty<V: ::std::fmt::Debug>(
    self,
    tcx: TyCtxt<'tcx>,
    elem: ProjectionElem<V, Ty<'tcx>>,
) -> PlaceTy<'tcx> {
    self.projection_ty_core(tcx, &elem, |ty| ty, |_, _, _, ty| ty, |ty| ty)
}
```

Three callbacks (`structurally_normalize`, `handle_field`,
`handle_opaque_cast_and_subtype`) all default to identity for
the `PlaceElem<'tcx>` case because the `Ty<'tcx>` payload is
already stored inline on `Field` and `OpaqueCast` /
`UnwrapUnsafeBinder` variants.

`projection_ty_core` is the per-variant arm match
[`rustc_middle/src/mir/statement.rs:194-256`]; the per-variant
narrowing logic from Sections 3.1-3.8 above is concentrated
here. Re-summarised:

| Variant | Narrowing rule |
|---|---|
| `Deref` | `structurally_normalize(self.ty).builtin_deref(true).unwrap()` -- inner pointee type. |
| `Index` / `ConstantIndex` | `structurally_normalize(self.ty).builtin_index().unwrap()` -- element type of array/slice. |
| `Subslice` | Slice -> same `Slice` ty; Array -> array of computed length. |
| `Downcast(_, idx)` | Same `ty`, sets `variant_index = Some(idx)`. |
| `Field(f, fty)` | `handle_field(...)`; for `PlaceElem<'tcx>` with the inline `Ty`, just `fty`. |
| `OpaqueCast(ty)` | `handle_opaque_cast_and_subtype(ty)`; for `PlaceElem<'tcx>`, just `ty`. |
| `UnwrapUnsafeBinder(ty)` | Same as `OpaqueCast` -- `handle_opaque_cast_and_subtype(ty)`. |

INFERENCE: the per-variant narrowing rules are simple enough
that stage-5 can describe them inline in `spec-snapshot.md`
without delegating to a "bridge calls into rustc" pattern. The
narrowing is structural over `Ty` content already pinned in
stage 3.

### 4.4 `PlaceTy::field_ty`

Used by `handle_field` for callers that did not have the
inline `Ty` on `Field`
[`rustc_middle/src/mir/statement.rs:111-173`]. The dispatch
covers:

- `ty::Adt(adt_def, args)` enum + `variant_idx` set ->
  `adt_def.variant(variant_index).fields[f].ty(tcx, args)`.
- `ty::Coroutine(def_id, args)` + `variant_idx` set ->
  `state_tys(def_id, tcx).nth(variant_index)?.nth(f.index())`.
- `ty::Adt(adt_def, args)` non-enum (struct/union) ->
  `adt_def.non_enum_variant().fields[f].ty(tcx, args)`.
- `ty::Closure(_, args)` -> `args.as_closure().upvar_tys()[f]`.
- `ty::CoroutineClosure(_, args)` ->
  `args.as_coroutine_closure().upvar_tys()[f]`.
- `ty::Coroutine(_, args)` (no variant_idx) ->
  `args.as_coroutine().prefix_tys()[f]`.
- `ty::Tuple(tys)` -> `tys[f]`.
- All other shapes: `bug!`.

INFERENCE: stage 5's place-validation can rely on the inline
`Ty` from `Field(FieldIdx, Ty)` and avoid re-deriving from the
ADT (since rustc gave us the answer). That matches the
"store rustc's resolved place form" methodology rule pinned in
stage-5 `memory.md` Section 2.

### 4.5 Other helpers worth noting

- `Place::is_indirect` -- true iff any projection is `Deref`
  [`rustc_middle/src/mir/statement.rs:373-375`].
- `Place::is_indirect_first_projection` -- true iff the FIRST
  projection is `Deref`
  [`rustc_middle/src/mir/statement.rs:388-390`]. The doc
  comment notes this is equivalent to `is_indirect` at
  `AnalysisPhase::PostCleanup` and later phases (where `Deref`
  can only be the first projection).
- `Place::is_stable_offset` -- true iff every projection has a
  stable offset (none of `Deref` or `Index`)
  [`rustc_middle/src/mir/statement.rs:379-381`,
  `258-288`].
- `Place::local_or_deref_local` -- returns `Some(local)` when
  the place is just a local, or just a `Deref` of a local
  [`rustc_middle/src/mir/statement.rs:392-397`,
  `465-471`].
- `Place::as_local` -- returns `Some(local)` when there are no
  projections [`rustc_middle/src/mir/statement.rs:399-404`].
- `Place::iter_projections` -- yields `(prefix_PlaceRef, elem)`
  pairs in evaluation order
  [`rustc_middle/src/mir/statement.rs:411-423`]. Stage-5
  validation walks may use the same iteration shape.
- `ProjectionElem::can_use_in_debuginfo` -- per-variant flag
  for VarDebugInfo eligibility
  [`rustc_middle/src/mir/statement.rs:300-315`]. Not directly
  relevant to D-5-1; recorded for completeness.

---

## 5. Storage representation and interning

Three things to record:

### 5.1 `Place::projection` is interned in `TyCtxt`

[`rustc_middle/src/ty/context.rs:937`]:

```rust
place_elems: InternedSet<'tcx, List<PlaceElem<'tcx>>>,
```

Stage 5's spec choice: this rustc-side interning is a
performance optimization within the `'tcx` arena; per Q2
(eager translation, fully-owned IR; `architecture.md`),
RegseqModel does NOT inherit `'tcx` interning. After the
bridge runs, projections are owned -- a `Vec<PlaceElem>` or
`Box<[PlaceElem]>` per `Place` value.

INFERENCE: D-5-1 should choose between:

(a) `Box<[PlaceElem]>` -- mirrors rustc's slice-shape, slightly
    cheaper allocation than `Vec` (no spare capacity), used by
    stable-MIR / `rustc_public/src/mir/body.rs` (see Section
    8 below).
(b) `Vec<PlaceElem>` -- ergonomic, simpler `Default`, easy
    extension if any builder code ever appends.

D-5-1 trade-off: `Box<[_]>` matches "store the result, not the
input" (we never extend a projection list once translated);
`Vec` is marginally easier to construct. Either is consistent
with the methodology -- pick deliberately during Phase A.

### 5.2 `Place` itself is NOT interned

The `Place` struct
[`rustc_middle/src/mir/syntax.rs:1188-1194`] is a 2-field
value type, constructed inline. Two `Place`s with identical
content compare equal via derived `PartialEq` / `Eq`, but they
do not share storage.

NOTE: this matches the stage-5-specific principle "Place is
body-local; no interning, no PlaceId" already pinned in
`../memory.md` Section 2.

### 5.3 Constructor APIs

The relevant `TyCtxt` constructors
[`rustc_middle/src/ty/context.rs:2819`,
`3078`,
`3218-3223`]:

```rust
place_elems: pub mk_place_elems(PlaceElem<'tcx>),
...
Place { local: place.local, projection: self.mk_place_elems(&projection) }
...
pub fn mk_place_elems_from_iter<I, T>(self, iter: I) -> T::Output
where
    I: Iterator<Item = T::Item>,
    T: CollectAndApply<PlaceElem<'tcx>, &'tcx List<PlaceElem<'tcx>>>,
{
    T::collect_and_apply(iter, |xs| self.mk_place_elems(xs))
}
```

Not directly relevant to RegseqModel (we own our projections);
recorded as the canonical interning entry points the bridge
will read from when extracting `Place` content.

---

## 6. `MirPhase` and what shape we receive

`optimized_mir` is provided in
[`rustc_mir_transform/src/lib.rs:792-794`]:

```rust
fn optimized_mir(tcx: TyCtxt<'_>, did: LocalDefId) -> &Body<'_> {
    tcx.arena.alloc(inner_optimized_mir(tcx, did))
}
```

The end-of-phase target for that pipeline is set in
[`rustc_mir_transform/src/lib.rs:786`] at the end of
`run_optimization_passes`:

```rust
Some(MirPhase::Runtime(RuntimePhase::Optimized)),
```

So `optimized_mir` returns bodies at
`MirPhase::Runtime(RuntimePhase::Optimized)` -- the most
restricted phase.

The phase declarations
[`rustc_middle/src/mir/syntax.rs:43-148`] document the
restrictions cumulatively:

### 6.1 From `AnalysisPhase::PostCleanup` (line 105-122)

> Beginning in this phase, the following variants are
> disallowed:
> * `TerminatorKind::FalseUnwind`
> * `TerminatorKind::FalseEdge`
> * `StatementKind::FakeRead`
> * `StatementKind::AscribeUserType`
> * `StatementKind::Coverage` with `CoverageKind::BlockMarker`
>   or `CoverageKind::SpanMarker`
> * `Rvalue::Ref` with `BorrowKind::Fake`
> * `CastKind::PointerCoercion` with any of the following:
>   * `PointerCoercion::ArrayToPointer`
>   * `PointerCoercion::MutToConstPointer`
>
> Furthermore, **`Deref` projections must be the first
> projection within any place** (if they appear at all).

The bolded sentence is load-bearing for stage 5: in
`optimized_mir`, **`Deref` only appears as the first
projection element** in any `Place`. Compound chains like
`Field(...).Deref` cannot occur.

### 6.2 From `RuntimePhase::Initial` (line 127-148)

> In addition to the semantic changes, beginning with this
> phase, the following variants are disallowed:
> * `TerminatorKind::Yield`
> * `TerminatorKind::CoroutineDrop`
> * `Rvalue::Aggregate` for any `AggregateKind` except `Array`
> * `Rvalue::CopyForDeref`
> * **`PlaceElem::OpaqueCast`**
> * `LocalInfo::DerefTemp`

So `OpaqueCast` is **gone** in our input (Section 3.7;
H1 REFINED -- absent under both solver paths for different
reasons: legacy strips it via `PostAnalysisNormalize`;
next-solver never constructs it because the construction
site at
`mir_build/src/builder/matches/match_pair.rs:104-118` is
symmetrically gated).

### 6.3 From `RuntimePhase::PostCleanup` (line 144-146)

> Beginning with this phase, the following variant is
> disallowed:
> * **`ProjectionElem::Deref` of `Box`**

So `Box` derefs are elaborated to raw-pointer derefs by
`ElaborateBoxDerefs`
[`rustc_mir_transform/src/elaborate_box_derefs.rs:156-158`,
referenced from
`rustc_mir_transform/src/lib.rs:659`]. Our input never sees a
`Deref` whose base type is `Box<T>`. CONCLUSION (H3
CONFIRMED, see "Hypothesis cross-check results" and §10.4):
the elaborated form is **not** a long inline chain like
`Field(0)/Field(0)/Field(0)/Deref` over the `Box` head local;
instead, `elaborate_box_derefs.rs:53-91` introduces a fresh
`*const T` temp local via `patch.new_temp`, inserts a
`Cast(Transmute, Copy(box.0.0), *const T)` statement, and
rewrites `place.local` to that temp -- so our `Place` sees
ordinary `[Deref, ...]` chains over raw-pointer locals, with
the I-Deref-first invariant preserved.

### 6.4 What this means for D-5-1

In `optimized_mir`, the **realized** `PlaceElem` vocabulary is:

| Variant | Reachable in our input? | Notes |
|---|---|---|
| `Deref` | YES | First projection only (per AnalysisPhase::PostCleanup); never of `Box` directly. |
| `Field(FieldIdx, Ty)` | YES | Common case. May follow a `Downcast`. |
| `Index(Local)` | YES | Carries a `Local`, not an `Operand`. |
| `ConstantIndex {...}` | YES | May appear after `Subslice`. |
| `Subslice {...}` | YES | May appear after `Subslice` (per RuntimePhase::Initial). |
| `Downcast(Option<Symbol>, VariantIdx)` | YES | Always followed by `Field` (rustc bugs otherwise). |
| `OpaqueCast(Ty)` | NO | Absent under both solver paths (H1 REFINED): legacy strips it via `PostAnalysisNormalize`; next-solver never constructs it (gated construction site at `mir_build/.../match_pair.rs:104-118`). |
| `UnwrapUnsafeBinder(Ty)` | NO (rejected) | Tied to unstable `unsafe_binders` feature; no MirPhase restriction. DSL does not formally gate feature opt-in (H2 REFINED), but no driver use case + closed-vocabulary diagnostic per D-5-1.1. |

D-5-1 closed-vocabulary candidates: 7 reachable variants minus
`UnwrapUnsafeBinder` (if rejected) = **6 variants** as the
core set. Adding `UnwrapUnsafeBinder` as a no-op type
refinement variant for forward compatibility = **7 variants**.

---

## 7. Cleanup-pass interactions (regseq driver whitelist)

The whitelisted passes per the project README §1 are:
`RemoveStorageMarkers`, `RemoveZsts`, `SimplifyLocals`,
`ReorderBasicBlocks`, `ReorderLocals`, `CheckAlignment`,
`CheckNull`, `CheckEnums`. Their effect on places:

### 7.1 `RemoveStorageMarkers`
[`rustc_mir_transform/src/remove_storage_markers.rs`]:

```rust
fn run_pass(&self, _tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
    for data in body.basic_blocks.as_mut_preserves_cfg() {
        data.retain_statements(|statement| match statement.kind {
            StatementKind::StorageLive(..)
            | StatementKind::StorageDead(..)
            | StatementKind::Nop => false,
            _ => true,
        })
    }
}
```

Removes `StorageLive` / `StorageDead` / `Nop` statements. **No
effect on `PlaceElem` vocabulary or projection chains.**

### 7.2 `RemoveZsts`
[`rustc_mir_transform/src/remove_zsts.rs`]:

```rust
fn visit_statement(&mut self, statement: &mut Statement<'tcx>, loc: Location) {
    let place_for_ty = match statement.kind {
        StatementKind::Assign(box (place, ref rvalue)) => {
            rvalue.is_safe_to_remove().then_some(place)
        }
        ...
    };
    if let Some(place_for_ty) = place_for_ty
        && let ty = place_for_ty.ty(self.local_decls, self.tcx).ty
        && self.known_to_be_zst(ty)
    {
        statement.make_nop(true);
    } else {
        self.super_statement(statement, loc);
    }
}
```

For each statement whose target place has a ZST type, the
statement becomes `Nop`. ZST operands become
`Constant(ZeroSized)`.

Critical question for D-5-1: **does this collapse field
projections through ZST fields?** Answer: **NO**. The pass
operates on whole-statement granularity (the target `Place`'s
final type), not on projection-chain granularity. A
`Field(idx, ZstTy)` projection chain is not rewritten; only
statements whose final result is ZST are no-op'd. INFERENCE:
field projections through ZST fields **remain in the IR** and
stage 5 must handle them.

Note also `is_required(): true`
[`rustc_mir_transform/src/remove_zsts.rs:31-33`] -- the pass
runs even at `mir_opt_level=0` when included in the pipeline.

### 7.3 `SimplifyLocals`
[`rustc_mir_transform/src/simplify.rs:415-459`]:

Removes unused locals and remaps remaining ones into a dense
range. Walks all places to update head locals via
`LocalUpdater`. The `Local` IDs change (e.g., `_5` may become
`_3`); the projection chain content does not.

INFERENCE: stage 5's `Place.local: LocalId` reflects the
post-simplification numbering. The bridge sees the renumbered
locals; consumer-facing identity is preserved structurally
(same Place projection -> same head local).

### 7.4 `ReorderBasicBlocks` / `ReorderLocals`
[`rustc_mir_transform/src/prettify.rs`]:

Off by default (`is_enabled: false`); run when explicitly
requested via `-Zmir-enable-passes=+ReorderBasicBlocks,+ReorderLocals`.
Both are pure renumberings:

- `ReorderBasicBlocks` puts blocks in reverse-postorder
  [`prettify.rs:14-42`].
- `ReorderLocals` puts locals in use-order, preserving args
  and `RETURN_PLACE` [`prettify.rs:44-96`].

Effect on `Place`: head `Local` integer changes; projection
chain content unchanged.

### 7.5 `CheckAlignment` / `CheckNull` / `CheckEnums`
[`rustc_mir_transform/src/check_alignment.rs`,
`check_null.rs`, `check_enums.rs`]:

All gated on `sess.ub_checks()`. Each pass **inserts new
statements/terminators** carrying UB-check assertions. None of
them modify existing `Place` values.

INFERENCE: from a stage-5 perspective, these passes only add
new sites that consume `Place` (the place being checked).
They do not rewrite the projection vocabulary.

### 7.6 Summary

**None of the whitelisted passes modify the `PlaceElem`
vocabulary or rewrite projection chains.** The passes either
remove statements (`RemoveStorageMarkers`, `RemoveZsts`),
renumber locals/blocks (`SimplifyLocals`, `ReorderBasicBlocks`,
`ReorderLocals`), or insert new check statements
(`CheckAlignment`, `CheckNull`, `CheckEnums`).

The shape of `Place` we receive is therefore determined by the
`MirPhase::Runtime(RuntimePhase::Optimized)` declaration
restrictions (Section 6), NOT by the cleanup-pass whitelist.

---

## 8. `OpaqueCast` and `UnwrapUnsafeBinder` semantics

Already covered in Sections 3.7 and 3.8. Recap for D-5-1
framing:

**`OpaqueCast`**: a place-level type refinement from an
opaque type to its hidden (concrete) type. Generated during
MIR build for code that involves opaque types
(`impl Trait`, TAIT). Absent under both solver paths in our
input (H1 REFINED -- legacy strips it via
`PostAnalysisNormalize`; next-solver never constructs it
because the construction site at
`mir_build/src/builder/matches/match_pair.rs:104-118` is
symmetrically gated). **D-5-1 chose option (a): reject if
encountered (no closed-vocabulary slot reserved).**

**`UnwrapUnsafeBinder`**: place-level transmute from an
unsafe binder type to its wrapped inner type. Tied to the
unstable `unsafe_binders` feature. H2 REFINED clarified the
DSL surface: `RegseqCallbacks` carries no formal feature-
gate allow/deny list (`callbacks.rs:60-119`), so a driver
could in principle write `#![feature(unsafe_binders)]` --
but no plausible device-driver pattern needs `unsafe<...>`
types (the feature targets advanced lifetime patterns in
libraries, not `#![no_std]` driver code). **D-5-1 chose
option (a): reject if encountered.** Rationale follows
"no defensive fallbacks; the toolchain is pinned" -- carry
no closed-vocabulary slot for a variant we have no use case
for. Diagnostic cites the closed-vocabulary policy and the
absence of a v1 driver use case (NOT the feature's
instability per se -- a future stable `unsafe_binders` would
not change the rejection).

(See "Hypothesis cross-check results" below for the full
H1 / H2 verdicts and citations. The two-option trade-off
(a) reject / (b) carry-as-no-op was decided in favour of
(a) at D-5-1.1 LOCKED.)

---

## 9. Stable-MIR analog (`rustc_public`)

For comparison only; not normative. The new SMIR-replacement
crate `rustc_public` is the closest existing analog to what
RegseqModel's body shape will look like
[`rustc_public/src/lib.rs`]. The owned `Place` and projection
shape live in
[`rustc_public/src/mir/body.rs`].

Per the rustc-references doc
(`../../rustc-references.md` line 84), the relevant entry
is `Place` at line 680 of `rustc_public/src/mir/body.rs`
under this nightly. NOTE: contents not read in this pass --
called out as a comparison candidate worth a separate
prior-art file (`stage-5/prior-art/stable-mir.md`) if D-5-1
or a later cluster wants to evaluate the
opaque-IDs-plus-context pattern that `rustc_public` uses.

---

## 10. Surprises and D-5-1 framing notes

### 10.1 `Subtype` is NOT a `PlaceElem` variant

Already flagged in Section 2. The status.md / memory.md
enumeration should be corrected to remove `Subtype` and add
`UnwrapUnsafeBinder` (with the rejection / carry choice
explicit per Section 8). This is a doc-only correction; it
does not change D-5-1's substantive options.

### 10.2 `Deref` is first-projection-only in our input

Section 6.1. This is a real shape constraint stage 5 should
record as part of the place-validation rules in §3.3 Phase B:

> In `optimized_mir` input, `Deref` may appear at most once
> per place, and only as the first projection element.

This is equivalent to "the place is at most one indirection
deep at the head; further projections are direct narrowings".
It simplifies validation considerably.

### 10.3 `OpaqueCast` is gone

Section 3.7. D-5-1 should reject without reserving a slot.

### 10.4 `Box` deref does not appear

Section 6.3. Stage 5's place validation will not see a direct
`Deref` of `Box<T>`: `ElaborateBoxDerefs` runs before our
`MirPhase::Runtime(Optimized)` input. Per Section 6.1's
`Deref`-first restriction, the "long chain ending in `Deref`"
pattern (e.g., `Field(0).Field(0).Field(0).Deref` over a
`Box`) does NOT arise. The actual elaborated form is now
**CONFIRMED** by H3 (see "Hypothesis cross-check results" --
`elaborate_box_derefs.rs:53-91`): a fresh `*const T` temp
local is introduced via `patch.new_temp`; a
`Cast(Transmute, Copy(box.0.0), *const T)` statement is
inserted; `place.local` is rewritten to the temp; `Deref` is
preserved as the first projection of the resulting `Place`.
Stage 5's input therefore sees ordinary `[Deref, ...]` chains
over raw-pointer locals. See the H3 record for the worked
example.

### 10.5 `Downcast` is always followed by `Field`

Section 3.6. The narrowing core asserts this
[`rustc_middle/src/mir/statement.rs:211-213`]. Stage 5's
place-validation rule mirrors it: a `Downcast(_, v)`
projection element must be immediately followed by a
`Field(_, _)` element. CONCLUSION (H4 REFUTED, see
"Hypothesis cross-check results"): the previous open
question -- whether Downcast-as-last arises in
`SetDiscriminant`-style writes -- is closed. Exhaustive
enumeration of the 3 `SetDiscriminant` construction sites in
rustc found that none use Downcast: `coroutine.rs:367` uses
`Place::from(SELF_ARG)`; `shim.rs:770-773` uses
`Place::return_place()`;
`mir_build/.../instruction.rs:33` is the test-only
`mir!` macro parser. The `variant_index` field of
`SetDiscriminant` alone discriminates. D-5-1.6 codifies the
strict form: `Downcast` must be non-last AND immediately
followed by `Field`.

### 10.6 `Field`'s inline `Ty` is the post-narrowing type

Section 3.2. Stage 5 should preserve this -- carrying the
`TyId` inline on `Field(FieldIdx, TyId)` lets the bridge skip
re-deriving the type from layout each step, matching the
"store the result, not the input" methodology rule.

### 10.7 `Index`'s `Local` payload is not an `Operand`

Section 3.3. This is **load-bearing** for the dependency-chain
ordering: stage 5's `Place` does not depend on stage-6
`Operand`. The runtime index value is held by a separately-
referenced local (`LocalId`), which the consumer of the
`Index` projection loads as needed.

### 10.8 `Subslice` chaining IS allowed at runtime MIR

Section 3.5 + Section 6.2. In our input, `Subslice` may be
followed by `ConstantIndex` or another `Subslice`. Stage 5's
validation rules should not assume the pre-runtime "no chaining
after Subslice" restriction.

---

## 11. Open questions for D-5-1 Phase A

The findings above ground the following open questions for the
opening Phase A discussion:

1. **Closed `PlaceElem` vocabulary count**: 6 (omit
   `UnwrapUnsafeBinder`) or 7 (admit it as a no-op type
   refinement)? Section 8 / 10.

2. **`Field`'s second slot**: keep the inline `TyId` (matches
   rustc's `Field(FieldIdx, Ty)`) or just `FieldIdx` (rely on
   re-derivation from layout)? Section 3.2 + 4.4. The
   methodology rule "store the result, not the input" argues
   for keeping the inline `TyId`.

3. **`Downcast`'s variant name**: keep the `Option<Symbol>` or
   drop it? Section 3.6. The doc explicitly calls it
   "printing-only"; dropping is the C-semantics-preserving
   choice.

4. **Projection list storage shape**: `Box<[PlaceElem]>` (mirror
   rustc's slab) or `Vec<PlaceElem>`? Section 5.1.

5. **Validation phase placement**: place-validation lives in
   §3.3 Phase B alongside other stage-3/4 validations. Does
   it run before or after stage-11's `LocalDecls` are
   materialised? Stage 5's validation NEEDS the head
   `LocalId`'s type; if `LocalDecls` is stage-11-only, the
   validation walk defers to stage 11. INFERENCE: this is a
   deferral to later stage similar to D-4-K's pattern.

6. **`Deref`-first invariant**: codify the
   "Deref appears at most once, only as first projection"
   shape constraint as a stage-5 invariant. Section 6.1 +
   10.2.

7. **`Downcast` followed by `Field`**: codify as a stage-5
   invariant. Section 3.6 + 10.5.

8. **Renderer style decision (D-5-K)** -- **CLOSED**.
   `pretty.rs::render_place` deferred to **stage 11**.
   Original framing of this question proposed stage-13 or
   stage-12 deferral options; D-5-K LOCKED at stage 11 on
   the D-5-2/D-5-K combined R1 BLOCK finding that `Item`
   carries no field/variant name tables -- field and
   variant names live in §11 layout schema's
   `FieldLayout.name` / `VariantLayout.name`, reachable at
   render time via the head local's `LocalDecl.ty`. `Item`
   (stage 12) is not a `render_place` dependency, and
   `render_ty` (stage 13) is not a dependency either
   because `Field`'s inline `TyId` is consumed by
   validation/codegen (not rendering). Stage 11 is the
   latest of the actual dependencies (`Body` /
   `LocalDecls` materialisation + `LocalId` `_N`
   rendering). See `discussions.md` D-5-K for the audit
   shape and retarget rationale.

These are framing inputs for Phase A; questions 1-7 were
closed via Phase A discussion (see `discussions.md` D-5-1.1
through D-5-1.6 + D-5-2); question 8 was closed at D-5-K
post-retarget.

---

## Hypothesis cross-check results

Cross-checked 2026-05-03 against the pinned toolchain. All
hypothesis statuses below cite primary source.

- **H1 [REFINED]:** `OpaqueCast` is reliably absent from
  `optimized_mir` under both solver paths, but for different
  reasons -- not for the single reason H1 stated.

  Original H1 framing was: "PostAnalysisNormalize strips
  OpaqueCast, but its strip code path is gated under
  `!next_trait_solver_globally()`, so under next-solver
  something else must remove them."

  Cross-check finding: under next-solver, OpaqueCast is
  never **constructed** in the first place. The single MIR-
  side construction site is gated symmetrically.

  Evidence:
  - Construction site (only one for `optimized_mir` input):
    [`rustc_mir_build/src/builder/matches/match_pair.rs:104-118`]
    The `place_builder.project(ProjectionElem::OpaqueCast(...))`
    call at line 117 is wrapped in `if !cx.tcx.next_trait_solver_globally()`
    at line 104. Under next-solver, the OpaqueCast projection
    is simply not inserted during pattern lowering.
  - Strip site (legacy solver only):
    [`rustc_mir_transform/src/post_analysis_normalize.rs:42-57`]
    Wrapped in `if !self.tcx.next_trait_solver_globally()`. Under
    legacy: removes OpaqueCast projections. Under next-solver:
    no-op for OpaqueCast.
  - The other surface mention
    [`rustc_mir_build/src/thir/cx/expr.rs:1380-1402`]
    handles `HirProjectionKind::OpaqueCast` in **closure-capture
    THIR lowering** by emitting `ExprKind::Use { source: ... }`
    -- a no-op THIR wrapper, NOT a MIR `ProjectionElem::OpaqueCast`
    construction.
  - Other `as_place.rs` references at lines 107, 229, 241-242,
    804 are filter / iterate / assert helpers operating on
    upvar projection chains, not construction sites.
  - Validator confirmation:
    [`rustc_mir_transform/src/validate.rs:874-881`] fails on
    OpaqueCast at `MirPhase::Runtime(RuntimePhase::Initial)`
    or later. Under next-solver, never constructed; under
    legacy, stripped at PostAnalysisNormalize. Either way,
    Runtime-phase MIR has no OpaqueCast projections.

  Implication for D-5-1: the closed-vocabulary count is
  unaffected (OpaqueCast remains absent from our input). The
  framing in spec-snapshot should say "absent from input
  under both solver paths" rather than "stripped" -- the
  legacy-only strip is one of two reasons.

  Cross-checked: yes (CONFIRMED via construction-side
  symmetry).

- **H2 [REFINED]:** Regseq DSL does NOT formally exclude
  `unsafe_binders` opt-in -- a driver author could in
  principle write `#![feature(unsafe_binders)]` -- but no
  plausible device driver pattern uses `unsafe<...>` types.
  D-5-1 should treat `UnwrapUnsafeBinder` as a closed-
  vocabulary rejection per the no-defensive-fallbacks rule;
  if a future driver triggers the rejection, the compiler
  surfaces a clear diagnostic.

  Original H2 framing was: "UnwrapUnsafeBinder will not appear
  because `unsafe_binders` is unstable."

  Cross-check finding: the DSL surface does not gate feature
  opt-in. The original framing is correct in outcome (the
  variant should be rejected) but wrong in reason (it's not
  formally excluded by the DSL).

  Evidence:
  - DSL background:
    [`docs/discussions/regseq-model/regseq-background.md:32-55`]
    Spec environment is `#![no_std]` Rust crates with limited
    `core`. No statement about feature-gate allow-list or
    deny-list. The "three mechanisms" (intrinsics, properties,
    traits) describe the Regseq vocabulary, not Rust feature
    restrictions.
  - Bridge / driver callbacks:
    [`crates/regseq-driver/src/callbacks.rs:60-81`]
    (`RegseqAwareCallbacks::config`) and
    [`crates/regseq-driver/src/callbacks.rs:103-119`]
    (`RegseqCallbacks::config`) configure `mir_opt_level=0`,
    `always_encode_mir=true`, `mir_enable_passes=regseq_mir_passes()`,
    `overflow_checks=false`, `debug_assertions=false`, and
    `no_codegen=true` (RegseqCallbacks only). No feature
    allow-list or deny-list. The "INTENTIONALLY NOT SET"
    comments at lines 71-80 and 113-118 list things the
    callbacks deliberately do not configure (e.g.,
    `feature(register_tool)` injection); they do not gate
    other unstable features.
  - Methodology rule:
    [`docs/discussions/regseq-model/README.md`] "Source declares
    its own compiler-relevant attributes" -- the spec author
    controls feature opt-in.
  - `unsafe_binders` use case (rustc tracking):
    [`compiler/rustc_middle/src/mir/syntax.rs:1196-1285`]
    `ProjectionElem::UnwrapUnsafeBinder(T)` carries a `Ty`
    payload (the wrapped inner type the binder unwraps to).
    The feature is for higher-rank trait bounds and binder
    elision in advanced library code (RFC 3621); no
    plausible `#![no_std]` device driver pattern needs this.

  Implication for D-5-1: closed-vocabulary count is unchanged
  (UnwrapUnsafeBinder remains rejected). Spec-snapshot
  language should describe the rejection as "Regseq does not
  support this variant; if it appears in input, the bridge
  raises a closed-vocabulary diagnostic" rather than "the
  feature is unstable so it cannot appear."

  Cross-checked: yes (REFINED -- same outcome via different
  reasoning).

- **H3 [CONFIRMED]:** The elaborated `Box`-deref shape uses a
  temporary local holding the raw pointer, with `Deref` as
  the first projection of the rewritten `Place`.

  Evidence:
  - Pass entry:
    [`rustc_mir_transform/src/elaborate_box_derefs.rs:158-227`]
    `ElaborateBoxDerefs::run_pass` walks the body with
    `ElaborateBoxDerefVisitor`.
  - Per-place rewrite:
    [`rustc_mir_transform/src/elaborate_box_derefs.rs:53-91`]
    `visit_place` matches on `PlaceElem::Deref` as first
    projection and `base_ty.boxed_ty().is_some()`. When both
    hold:
    1. Allocates fresh temp `ptr_local: *const T` via
       `self.patch.new_temp(ptr_ty, ...)` (line 72).
    2. Inserts assignment `ptr_local = Cast(Transmute,
       Copy(box_local.0.0), *const T)` via
       `self.patch.add_assign(...)` (lines 74-85). The
       `box_local.0.0` chain is `[Field(0, Unique<T>),
       Field(0, NonNull<T>)]` per `build_projection`
       at lines 33-38.
    3. Replaces `place.local` with `ptr_local` (line 87).
       The original `Deref` and any subsequent projections
       are preserved via `self.super_place(...)` at line 90.
  - Helper signature:
    [`rustc_mir_transform/src/elaborate_box_derefs.rs:33-38`]
    `build_projection(unique_ty, nonnull_ty) -> [PlaceElem<'tcx>; 2]`
    returns `[Field(0, Unique<T>), Field(0, NonNull<T>)]`.
  - Doc comment:
    [`rustc_mir_transform/src/elaborate_box_derefs.rs:1-6`]
    "transforms derefs of Box into a deref of the pointer
    inside Box."

  Worked example (for D-5-1 spec-snapshot):
  - Source: `let b: Box<u32> = ...; let x = *b;`
  - Pre-elaboration MIR Place for `*b`:
    `Place { local: _b, projection: [Deref] }` where `_b: Box<u32>`.
  - Post-elaboration MIR (the bridge sees this):
    ```
    _N: *const u32 = Cast(Transmute, Copy(_b.0.0), *const u32)
    _x = (*_N)
    ```
    where `_x` reads the place
    `Place { local: _N, projection: [Deref] }` with
    `_N: *const u32`. The original `_b.0.0` chain is
    consumed by the inserted Cast operand, not preserved
    in the user-visible Place.

  Side note: Lines 188-221 contain a separate
  elaboration for `VarDebugInfoContents::Place` (debug info
  only) that DOES expand the `Box -> Unique -> NonNull -> *const T -> Deref`
  full chain inline -- this is debug-info-only and not the
  shape the bridge sees in body Places.

  Cross-checked: yes (CONFIRMED).

- **H4 [REFUTED]:** `StatementKind::SetDiscriminant`'s `place`
  field is NOT constructed with a `Downcast` projection in
  any of rustc's normal MIR construction sites. The
  `variant_index` field of `SetDiscriminant` alone identifies
  the variant; the place points at the enum directly.

  Original H4 framing was: "Downcast-as-last-projection arises
  in SetDiscriminant-style writes."

  Cross-check finding: refuted by exhaustive enumeration of
  construction sites.

  Evidence:
  - Exhaustive enumeration: searching for
    `StatementKind::SetDiscriminant {` (struct-literal
    construction) across the entire rustc tree returns 3
    files [grep output across `compiler/`]:
    1. [`rustc_mir_transform/src/coroutine.rs:362-371`]
       (`set_discr` helper for coroutine state machines):
       constructs `SetDiscriminant { place: Box::new(self_place), variant_index: state_disc }`
       where `self_place = Place::from(SELF_ARG)` (line 363).
       No Downcast projection.
    2. [`rustc_mir_transform/src/shim.rs:770-773`]
       (drop-shim coroutine clone path): constructs
       `SetDiscriminant { place: Box::new(Place::return_place()), variant_index }`.
       No Downcast projection.
    3. [`rustc_mir_build/src/builder/custom/parse/instruction.rs:33`]
       (custom `mir!` macro parser, test/intrinsic infrastructure):
       parses user-written `SetDiscriminant`. Not a normal
       MIR-construction path. The custom `mir!` macro is
       not in the Regseq DSL surface.
  - Validator constraint:
    [`rustc_mir_transform/src/validate.rs:1551-1567`] checks
    that the place's resulting type is `ty::Adt`,
    `ty::Coroutine`, or `ty::Alias(Opaque, ..)`. This
    constraint is satisfied by both rustc construction sites
    (which point at enum locals directly).
  - All other rustc files mentioning `StatementKind::SetDiscriminant`
    pattern-match on existing statements (visit / dataflow /
    optimization passes), they do not construct.

  Implication for D-5-1: the "Downcast is always followed by
  Field" structural invariant can be codified unconditionally.
  No exception clause needed for SetDiscriminant. (Per
  [`rustc_middle/src/mir/statement.rs:211-213`], rustc's own
  `PlaceTy::projection_ty` asserts that any `Downcast` in a
  Place projection sequence is followed by `Field` -- the
  assertion stands without SetDiscriminant counter-evidence.)

  Cross-checked: yes (REFUTED).

All four hypotheses now resolved. None changes the
closed-vocabulary count for D-5-1 (`OpaqueCast` and
`UnwrapUnsafeBinder` both remain rejected variants), but two
refinements affect spec-snapshot language:
- H1: "OpaqueCast absent under both solver paths" (not just
  "stripped").
- H2: "UnwrapUnsafeBinder is rejected per closed-vocabulary
  rule" (not "the DSL excludes it").

H3 confirms the worked-example shape for `Box`-deref. H4
removes a potential exception to the "Downcast followed by
Field" invariant.

---

## Out-of-scope items observed

The following surfaced during the read but are out of scope per
the directive; recorded so they don't get lost:

- `PlaceTy` is itself a published type
  [`rustc_middle/src/mir/statement.rs:94-103`] consumers use to
  carry "downcast variant index" alongside the type. Whether
  RegseqModel needs an analogous "place-type-with-variant"
  type for downstream stage 8 (statements like
  `SetDiscriminant`) is a stage-8 question.

- `ProjectionKind = ProjectionElem<(), ()>` alias
  [`rustc_middle/src/mir/statement.rs:347-349`] is rustc's
  pattern for "structural shape only, no payloads". RegseqModel
  may not need it.

- `place_elems` interning helpers in `TyCtxt`
  [`rustc_middle/src/ty/context.rs:2819-3223`] are bridge-side
  read APIs; not relevant to RegseqModel's owned shape.

- `Place::project_deeper` and `Place::iter_projections`
  [`rustc_middle/src/mir/statement.rs:411-433`] are mutation /
  iteration helpers; relevant to bridge construction code, not
  to the stage-5 spec.
