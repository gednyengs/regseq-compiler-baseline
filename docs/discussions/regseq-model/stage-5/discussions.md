# Stage 5 -- Discussion Record

Distilled record of the design dialogue producing the stage-5
`Place` and `PlaceElem` vocabulary spec. Each entry captures the
question raised, key options considered, the decision (or
current lean for open decisions), the reasoning chain, and
cross-references to `spec-snapshot.md` (the spec) and
`review-log.md` (the audit trail).

Status of decisions in this file mirrors `spec-snapshot.md`'s
"Place and projections" section and adjacent additions, plus
`status.md` "Open decisions" / "Locked decisions" tables.

D-tag convention: prefixed `D-5-K` form per `../README.md` §5.2
(stages 4+ use the new convention from day one; stages 1 and 3
use legacy un-prefixed tags).

Status tags:
- `[DRAFT]` -- audit-shape recorded; pending Codex review.
- `[LOCKED]` -- closed at APPROVE on a Codex review round.
- `[REOPENED]` -- a later sub-decision or sibling D-tag forced a
  revision; superseded entry kept for history.
- `[DEFERRED to stage N]` -- moved to a later stage's scope.

---

## 1. Decision discussions

### D-5-1 -- `Place` and `PlaceElem` core vocabulary [LOCKED]

(Locked at R10 = MINOR (2 NITs) per user decision. R10
corrections applied but not verified by Codex; residual
cross-reference cleanup deferred to the top-level merger
Codex review per README §5.5 step 6. See `review-log.md`
"Cluster closure (2026-05-04): D-5-1 LOCKED at R10" for the
methodology-deviation rationale and trajectory summary.)

**Question.** What is the closed `Place` / `PlaceElem` value
shape that the bridge fills from rustc's `mir::Place<'tcx>` /
`mir::PlaceElem<'tcx>` and that downstream consumers (stages
6-11, codegen) read as the lvalue surface of MIR?

**Key options.**

1. **Option A -- mirror rustc's PostCleanup-Runtime shape, but
   trimmed.** Adopt rustc's `Place<'tcx>` + `PlaceElem<'tcx>`
   structurally; reject `OpaqueCast` (absent from our
   `optimized_mir` input under both solver paths per H1
   REFINED -- legacy strips it via `PostAnalysisNormalize`,
   next-solver never constructs it) and `UnwrapUnsafeBinder`
   (gated behind unstable `unsafe_binders` feature; the DSL
   does not formally exclude opt-in but no driver use case
   per H2 REFINED -- closed-vocabulary diagnostic if
   encountered).
   Carry post-narrowing types inline where rustc does (`Field`'s
   `Ty<'tcx>` slot). Codify rustc's structural invariants
   (`Deref`-first from the `PostCleanup` phase;
   `Downcast`-followed-by-`Field` from `PlaceTy::projection_ty`'s
   successor assertion plus exhaustive cross-check) explicitly.
   Owned storage replaces `&'tcx List<...>` interning.
2. **Option B -- minimal flat vocabulary.** Reduce `PlaceElem`
   to just `{ Deref, Field, Index, Constant, Range, Variant }`
   collapsing rustc's distinctions (e.g., merge `ConstantIndex`
   and `Subslice` into a generic `Range`). Strip `Field`'s inline
   `TyId` and `Downcast`'s symbol. Bridge does extra translation
   work to project rustc's shape onto the simpler one.
3. **Option C -- richer vocabulary admitting forward
   compatibility.** Carry all 8 of rustc's variants (including
   the unreachable `OpaqueCast` and `UnwrapUnsafeBinder`) so
   future toolchain changes don't force spec revisions; lower
   each as a no-op type refinement where appropriate.

**Decision.** Option A, refined to **6 variants** (the parent
shape; per-variant payload pinned in sub-decisions D-5-1.1
through D-5-1.6):

```rust
pub struct Place {
    pub local: LocalId,
    pub projection: Box<[PlaceElem]>,
}

pub enum PlaceElem {
    Deref,
    Field(FieldIdx, TyId),
    Index(LocalId),
    ConstantIndex { offset: u64, min_length: u64, from_end: bool },
    Subslice { from: u64, to: u64, from_end: bool },
    Downcast(VariantIdx),
}
```

Plus two structural invariants enforced in §3.3 Phase B
place validation:

- **I-Deref-first.** If `place.projection[i]` is `Deref`, then
  `i == 0`.
- **I-Downcast-followed-by-Field.** If `place.projection[i]`
  is `Downcast(_)`, then `i < projection.len() - 1` AND
  `projection[i+1]` is `Field(_, _)`.

**Reasoning.**

*Why not Option B (minimal flat).* Collapsing `ConstantIndex`
and `Subslice` into a generic `Range` discards rustc's
distinction between fixed-length array indexing and slice
sub-ranging. The two have different runtime semantics
(`ConstantIndex` produces a single element; `Subslice`
produces a slice with derived length per `prior-art/rustc.md`
§3.5). Re-deriving the distinction on the consumer side is
work the bridge already has from rustc's tagged enum;
"store the result, not the input" applies. Stripping
`Field`'s `TyId` slot would force layout lookups at every
narrowing step (see D-5-1.2's reasoning).

*Why not Option C (admit forward compatibility).* The
methodology rule "no defensive fallbacks; the toolchain is
pinned" forbids carrying spec slots for variants we have no
use case for. `OpaqueCast` is confirmed absent under both
solver paths (`prior-art/rustc.md` §3.7 "Hypothesis cross-
check results" H1 REFINED); `UnwrapUnsafeBinder` is gated
behind unstable `unsafe_binders` with no plausible driver
use case (`prior-art/rustc.md` H2 REFINED). If a future
toolchain bump forces them in, the toolchain-bump
methodology event triggers Codex review and the spec is
revised then -- not pre-emptively now. Same posture stage 1
took for `DefPathData` extensibility (filed as a stage-9
revision pattern in `stage-9/expectations.md`).

*Why structural invariants are codified, not loose.*
Rustc-side mechanisms guarantee a tighter shape than the
`ProjectionElem` static type admits, with distinct grounds
per invariant. The `Deref`-first restriction is a phase-level
invariant of `PostCleanup` documented on the `PlaceElem`
enum (`syntax.rs:119-120`); the
`Downcast`-followed-by-`Field` constraint is asserted at
runtime in `PlaceTy::projection_ty` (`statement.rs:211-213`)
and is empirically not violated by any rustc construction
site (H4 cross-check). Codifying
each as a stage-5 invariant turns a free runtime-asserted
constraint into a typed contract for downstream consumers
(stage 7 Rvalues taking `&Place`; stage 8 Statements;
codegen). The strict form (rather than the loose
"if-there's-a-successor" form) reflects "reject what
shouldn't appear": no rustc construction site produces
`Downcast`-as-last (H4 REFUTED across the 3 enumerated
sites at `coroutine.rs:367`, `shim.rs:770-773`,
`mir_build/.../instruction.rs:33`).

*Why owned storage replaces `'tcx`-interning.* Q2
(eager translation; no `'tcx` survives the bridge) is
binding. Rustc's `Place::projection` is interned as
`&'tcx List<PlaceElem<'tcx>>`; Regseq translates this to
owned `Box<[PlaceElem]>` per D-5-1.4 (see that sub-decision
for the storage-shape choice).

**Methodology rules invoked.**

- *No defensive fallbacks; the toolchain is pinned.* The
  6-variant set is closed and exhaustive for our input;
  `OpaqueCast` and `UnwrapUnsafeBinder` are bridge-rejection
  diagnostics. No "Other" / "Unknown" variant. No
  forward-compat slots. (The "reject if it appears
  unexpectedly" clause of the README §2 rule body covers the
  closed-vocabulary rejection mechanically.)
- *Store the result, not the input.* `Field`'s post-narrow
  `TyId` is carried inline (D-5-1.2) because rustc has
  already done the narrowing and we don't re-derive.
- *C semantics with information preservation.* Each variant
  has a clear C lvalue mapping (`Deref` -> `*`, `Field` ->
  `.f`, `Index` -> `[i]`, `ConstantIndex` -> `[N]`,
  `Subslice` -> a slice expression, `Downcast` -> tagged-
  union arm projection). Names recovered render-side from
  layout-side `FieldLayout.name` / `VariantLayout.name`
  (D-5-K) preserve information for diagnostics.
- *Compositional behavior beats special-case flags.* No
  flag-style "is_box_deref" / "is_synthetic_index"
  markers on `Place`; distinctions ride on variant tags and
  the projection chain.

**Prior-art evidence.**

- `prior-art/rustc.md` §1 -- `Place<'tcx>` struct shape
  (head + projection), arena interning, `is_indirect`
  semantics.
- `prior-art/rustc.md` §2 -- the 8-variant `ProjectionElem`
  enum verbatim from `syntax.rs:1196-1285`; `try_map`
  impl confirming the enumeration; the
  `PlaceElem<'tcx> = ProjectionElem<Local, Ty<'tcx>>`
  alias.
- `prior-art/rustc.md` §3 -- per-variant payload semantics
  with primary-source citations.
- `prior-art/rustc.md` §6 -- the `MirPhase` machinery and
  what shape we receive (`Runtime(Optimized)` is later
  than `PostCleanup`).
- `prior-art/rustc.md` §7 -- cleanup-pass interactions
  for the regseq driver whitelist.
- `prior-art/rustc.md` §10 -- the surprises that
  shaped D-5-1 framing (`Deref`-first restriction,
  `Downcast`-followed-by-`Field` assertion, Box-deref
  elaboration shape).
- `prior-art/rustc.md` "Hypothesis cross-check results"
  H1 (REFINED), H2 (REFINED), H3 (CONFIRMED), H4
  (REFUTED).

**Worked example.** A simple field-then-index access
(originally `arr.elements[i]` where `arr` is a struct with
field `elements: [u32; 16]`):

```text
Place {
    local: _3,                              // arr
    projection: [
        Field(FieldIdx(0), TyId(<[u32; 16]>)),  // .elements
        Index(_5),                              // [_5] (where _5 holds the index value)
    ],
}
```

Traversal: head `_3` is the `arr` struct; `Field(0, ...)`
narrows to `[u32; 16]`; `Index(_5)` narrows to `u32`.
Validation walks the chain; codegen lowers to
`_3.elements[_5]`.

**Cross-references.** `spec-snapshot.md` "Place and
projections" section. Sub-decisions: D-5-1.1 (vocabulary
count), D-5-1.2 (Field's TyId slot), D-5-1.3 (Downcast's
Symbol drop), D-5-1.4 (projection storage shape), D-5-1.5
(Deref-first invariant), D-5-1.6 (Downcast-followed-by-
Field invariant). Related: D-5-2 (validation phase
placement), D-5-K (render_place deferral).

---

### D-5-1.1 -- Closed `PlaceElem` vocabulary count [LOCKED]

**Question.** Of rustc's 8 `ProjectionElem` variants, how
many does Regseq's closed-vocabulary `PlaceElem` admit? Reject
`UnwrapUnsafeBinder` (6-variant set) or admit it (7-variant
set)?

**Key options.**

1. **Option A -- 6-variant closed vocabulary.** Reject both
   `OpaqueCast` (confirmed absent under both solver paths)
   and `UnwrapUnsafeBinder` (gated behind unstable
   `unsafe_binders`). Bridge raises a diagnostic if either
   appears.
2. **Option B -- 7-variant closed vocabulary.** Admit
   `UnwrapUnsafeBinder` as a closed-vocabulary slot,
   lowered as a no-op type refinement (mirroring rustc's
   `project_type` codegen at `place.rs:366-368`). Reject
   only `OpaqueCast`.

**Decision.** Option A. The closed vocabulary is 6 variants
in source-declaration order: `Deref`, `Field`, `Index`,
`ConstantIndex`, `Subslice`, `Downcast`. Both `OpaqueCast`
and `UnwrapUnsafeBinder` are bridge-rejection events.

**Reasoning.**

*Why not Option B.* `UnwrapUnsafeBinder` is the projection
form for the unstable `unsafe_binders` feature (RFC 3621).
Three pieces of evidence push toward rejection:

(i) DSL surface: H2 (REFINED) found that `RegseqCallbacks`
    has no formal feature-gate restriction
    (`callbacks.rs:60-119`); a Regseq driver could in
    principle write `#![feature(unsafe_binders)]`. But no
    plausible device-driver pattern needs `unsafe<...>` types
    -- the feature targets advanced lifetime patterns in
    libraries (HRTB-adjacent), not `#![no_std]` driver code.

(ii) Methodology: "no defensive fallbacks; the toolchain is
     pinned" forbids carrying slots for variants without a
     use case. Admitting one for "forward compatibility"
     violates the rule directly.

(iii) Toolchain-bump path: if `unsafe_binders` ever stabilises
      and a future Regseq driver needs it, the toolchain-bump
      methodology event triggers a Codex review of the
      addition (same pattern documented in
      `stage-9/expectations.md` for `DefPathData` extensions).
      The spec is revised then, not pre-emptively now.

*Why `OpaqueCast` is uncontroversial.* H1 (REFINED) confirmed
`OpaqueCast` is absent from `optimized_mir` under both solver
paths -- never constructed under next-solver
(`mir_build/src/builder/matches/match_pair.rs:104-118` is
gated by `if !cx.tcx.next_trait_solver_globally()`); stripped
under legacy by `PostAnalysisNormalize`. The `MirPhase`
declaration (`syntax.rs:127-134`) lists `OpaqueCast` as
disallowed at `RuntimePhase::Initial` and later. Our
`Runtime(Optimized)` input never sees it. Rejection is the
straightforward closed-vocabulary outcome.

**Methodology rules invoked.**

- *No defensive fallbacks; the toolchain is pinned.* The
  closed 6-variant `PlaceElem` set is exhaustive for our
  input; `OpaqueCast` and `UnwrapUnsafeBinder` are
  bridge-rejection diagnostics if encountered. (The "reject
  if it appears unexpectedly" clause of the README §2 rule
  body covers the closed-vocabulary rejection mechanically.)
- *Source declares its own compiler-relevant attributes.*
  If a driver opts into `unsafe_binders` via a `#![feature(...)]`
  declaration, that's the source's choice; the Regseq compiler
  can refuse to lower the resulting MIR with a diagnostic
  rather than silently accepting.

**Prior-art evidence.**

- `prior-art/rustc.md` §2 -- 8-variant enumeration.
- `prior-art/rustc.md` §3.7 -- `OpaqueCast` semantics and
  strip path.
- `prior-art/rustc.md` §3.8 -- `UnwrapUnsafeBinder`
  semantics and gated-feature context.
- `prior-art/rustc.md` "Hypothesis cross-check results"
  H1 REFINED -- `OpaqueCast` absent under both solver
  paths: next-solver never constructs it; legacy
  constructs then strips it via `PostAnalysisNormalize`.
- `prior-art/rustc.md` "Hypothesis cross-check results"
  H2 REFINED -- `UnwrapUnsafeBinder` not in DSL inputs by
  use-case argument; closed-vocabulary rejection rule with
  diagnostic.

**Cross-references.** Parent: D-5-1. Spec section: §15.1
rejections row for `OpaqueCast` and `UnwrapUnsafeBinder`.

---

### D-5-1.2 -- `Field`'s second slot [LOCKED]

**Question.** Should `PlaceElem::Field` carry the post-narrow
field `TyId` inline (matching rustc's
`Field(FieldIdx, Ty<'tcx>)`) or just the `FieldIdx` (forcing
re-derivation from layout)?

**Key options.**

1. **Option A -- inline `TyId`.** `PlaceElem::Field(FieldIdx, TyId)`.
   Type narrowing during a place walk is O(1) per step; no
   layout lookups needed. Mirrors rustc.
2. **Option B -- `FieldIdx`-only.** `PlaceElem::Field(FieldIdx)`.
   Narrowing requires a layout lookup at each `Field` step
   (read the projected ADT's `VariantLayout.fields[idx]`
   per stage-3 D11).

**Decision.** Option A. `PlaceElem::Field(FieldIdx, TyId)`.

**Reasoning.**

*Why Option A.* Rustc's `Field(FieldIdx, Ty)` carries the
post-narrowing field type **inline**, redundantly with what
`PlaceTy::field_ty` (`statement.rs:111-173`) would compute
from layout. The doc on `field_ty` explicitly says:

> Most clients of `PlaceTy` can instead just extract the
> relevant type directly from their `PlaceElem`, but some
> instances of `ProjectionElem<V, T>` do not carry a `Ty`
> for `T`. Note that the resulting type has not been
> normalized.

The `ProjectionKind = ProjectionElem<(), ()>` alias at
`statement.rs:347-349` strips both `V` and `T` to `()` for
contexts where only the structural shape matters --
confirming the slot is informational, not structural. The
methodology rule "store the result, not the input" applies
directly: rustc has resolved the field type; we store the
resolution.

*Why not Option B.* Stripping the `TyId` would force the
§3.3 Phase B place-validation walk to do a layout lookup at
each `Field` step. With (A), each step is O(1). Codegen
also consumes Place to emit C field loads -- needs the
field's `TyId` anyway. With (A) it's already present; with
(B) it's looked up redundantly. The validation contract
becomes significantly cheaper to state and execute with the
inline `TyId`.

*Cost analysis.* `TyId` is `u32` (stage 1 D-5-1's id-width
choice), so 4 bytes per `Field` projection. `Place` is
body-local (no interning); the cost lands on owned storage.
For typical body sizes (pl011 fixture peak: ~36-line bodies
with bounded projection chains), this is negligible. Even at
hundreds of `Field`-bearing Places per body, the additional
memory is sub-kilobyte.

**Methodology rules invoked.**

- *Store the result, not the input.* The narrowing has
  already happened in rustc; we record it.
- *C semantics with information preservation.* Codegen
  lowering reads the field's `TyId` directly; preserving it
  makes the lowering call-site clean.

**Prior-art evidence.**

- `prior-art/rustc.md` §3.2 -- `Field`'s payload semantics;
  the `PlaceTy::field_ty` doc; the `ProjectionKind` alias
  showing the slot is structurally optional.
- `prior-art/rustc.md` §4.4 -- the narrowing arm at
  `statement.rs:241-246`: `handle_field` returns `fty`
  directly for the `PlaceElem<'tcx>` case (the
  `projection_ty` wrapper at `statement.rs:186-192` passes
  the closure `|_, _, _, ty| ty`).

**Cross-references.** Parent: D-5-1. Worked example: see
parent's worked example.

---

### D-5-1.3 -- `Downcast`'s `Option<Symbol>` [LOCKED]

**Question.** Should `PlaceElem::Downcast` preserve rustc's
`Option<Symbol>` variant-name slot or drop it in favour of
`VariantIdx`-only identification?

**Key options.**

1. **Option A -- drop the symbol.** `PlaceElem::Downcast(VariantIdx)`.
   Variant identified by index against the enum's variant
   table (stage-3 `Ty::Enum`'s `VariantLayout` indexed by
   `VariantIdx`).
2. **Option B -- keep as interned `Option<SymbolId>`.**
   `PlaceElem::Downcast(Option<SymbolId>, VariantIdx)`. Symbol
   interned via stage-1's `StringInterner`. Variant name
   preserved per projection.

**Decision.** Option A. `PlaceElem::Downcast(VariantIdx)`.

**Reasoning.**

*Why Option A.* The variant-name slot in rustc is
**printing-only** -- the `ProjectionElem::Downcast` doc
(`syntax.rs:1269-1274`) states this verbatim:

> "Downcast" to a variant of an enum or a coroutine.
>
> The included Symbol is the name of the variant, used for
> printing MIR.

The narrowing arm (`statement.rs:238-240`) ignores the
symbol entirely:

```rust
ProjectionElem::Downcast(_name, index) => {
    PlaceTy { ty: self.ty, variant_index: Some(index) }
}
```

Only `index` reaches `variant_index`. C-semantics
reinforces dropping: tagged-union arms in C are identified
by index/discriminant, not by name. The symbol has no
codegen role.

*Why not Option B.* Keeping the symbol pays interning cost
+ per-projection storage cost for a value that rustc itself
calls printing-only. Render-side recovery is direct:
`pretty.rs::render_place` (D-5-K, deferred to stage 11)
reads variant names from the enum's
`LayoutKind::Enum.shape.variants[idx].name` (or
`Single.variant.name`) -- pinned in stage 3 / §11 layout
schema (`VariantLayout.name: StringId`), reachable at
render time via the head local's `LocalDecl.ty`. No
stage-12 `Item` lookup is needed (`Item` carries
`def_path` / `kind` / `span` / flags /
`opaque_handle_eligible`, not variant name tables).

*Compositional argument.* Variant names belong at the
variant's declaration site -- specifically at
`VariantLayout.name` per stage-3 D11 (the layout-side
schema, not stage-12 `Item.variants`). Duplicating the
name in every `Place` that downcasts to that variant
violates "compositional behavior beats special-case flags"
-- it's a redundant carrying of declaration-site data.

**Methodology rules invoked.**

- *Store the result, not the input.* The narrowing
  function needs only the index; the symbol is a presentation
  artifact.
- *C semantics with information preservation.* C identifies
  tagged-union arms by discriminant; the variant name is
  recoverable from the enum declaration at render time.
- *Compositional behavior beats special-case flags.* The
  variant name lives at the declaration site, not on every
  use site.

**Prior-art evidence.**

- `prior-art/rustc.md` §3.6 -- `Downcast` variant doc, the
  "printing-only" annotation, the narrowing arm ignoring the
  symbol, the assertion that `Downcast` must be followed by
  `Field`.

**Cross-references.** Parent: D-5-1. Sibling: D-5-1.6
(`Downcast`-followed-by-`Field` invariant). Render-side
recovery: D-5-K (render_place deferral).

---

### D-5-1.4 -- Projection list storage shape [LOCKED]

**Question.** What owned-storage type backs `Place.projection`,
given that rustc's `&'tcx List<PlaceElem<'tcx>>` arena
interning is not inherited (Q2 -- eager translation; no
`'tcx` survives the bridge)?

**Key options.**

1. **Option A -- `Box<[PlaceElem]>`.** Owned, immutable,
   contiguous. 16 bytes (ptr + len). Matches rustc's slab
   semantics post-translation. Empty case is allocation-free
   (Rust's `Box::<[T]>::default()` produces a dangling slice
   for non-ZST `T`, no allocation).
2. **Option B -- `Vec<PlaceElem>`.** Owned, growable. 24 bytes
   (ptr + len + cap). The growability is unused after
   construction; capacity may differ from length.
3. **Option C -- `SmallVec<[PlaceElem; N]>`** (e.g. N=4). Avoids
   heap allocation for typical chain lengths (hypothesis: ~70%
   of MIR Places have projection length 1-2). Adds the
   `smallvec` dependency.

**Decision.** Option A. `projection: Box<[PlaceElem]>`.

**Reasoning.**

*Why Option A.* Place is body-local; once the bridge
constructs it, it's never modified. `Box<[_]>` conveys "fixed-
size, immutable, owned slice" -- the precise semantic truth.
`Vec` conveys "this might grow," which is misleading for
readers and analysis tools. Eight bytes saved per `Place`
over `Vec`; `Place` is structurally embedded inside
`Operand` / `Rvalue` / `Statement` / `Terminator` payloads
at stages 6-9, so the saving compounds. Matches rustc's
`&'tcx List<...>` semantics one-to-one modulo interning.

*Why not Option B.* The growability is unused; `Vec`
overstates the contract. An analysis tool reading the spec
should not have to decide whether mutating `Place.projection`
is meaningful (it isn't).

*Why not Option C.* `SmallVec` is a real perf win at impl
time -- if profiling at end-of-phase-1 confirms the
1-2-element-projection hypothesis, swapping to `SmallVec` is
a mechanical impl change (the consumer-facing slice API via
`&[PlaceElem]` is unchanged). But adding a `smallvec`
dependency to the spec's structural commitment is premature:
stage-1's foundation crate has zero non-rustc dependencies,
and the spec should preserve that posture. The spec commits
to `Box<[PlaceElem]>` as the public field shape; if profiling
later motivates the swap, that lands via a follow-up spec
revision (the change is mechanical but not invisible to
consumers that pattern-match on the field type, so it does
require revising the spec's storage commitment).

**Methodology rules invoked.**

- *Store the result, not the input.* Place's projection is
  immutable post-construction; the storage type should reflect
  this.
- *C semantics with information preservation.* The C lvalue
  derived from a `Place` is itself fixed once derived; the
  storage shape mirrors that.

**Prior-art evidence.**

- `prior-art/rustc.md` §5 -- storage representation and
  interning. Rustc's `Place::projection` is
  `&'tcx List<PlaceElem<'tcx>>` (arena-interned at
  `context.rs:937`); RegseqModel does not inherit the
  interning per Q2.
- `prior-art/rustc.md` §5.1 -- the explicit
  `Box<[PlaceElem]>` vs `Vec<PlaceElem>` framing for owned
  storage.

**Cross-references.** Parent: D-5-1.

---

### D-5-1.5 -- Codify the `Deref`-first invariant [LOCKED]

**Question.** Should the spec codify rustc's `PostCleanup`-
phase restriction that `Deref` appears at most once and only
as the first projection?

**Key options.**

1. **Option A -- codify strictly.** Spec: "If
   `place.projection[i]` is `Deref`, then `i == 0`." §3.3
   Phase B validation walks the chain; bridge raises a
   diagnostic if violated.
2. **Option B -- don't codify.** Allow `Deref` at any
   position. Looser shape; no spec contract for downstream
   consumers.

**Decision.** Option A. Codified as invariant **I-Deref-first**.

**Reasoning.**

*Why Option A.* Rustc's `PostCleanup` phase restricts `Deref`
to first-projection-only. The load-bearing sentence in the
`MirPhase` declaration at `syntax.rs:119-120` (recorded
verbatim in `prior-art/rustc.md` §6.1) reads:

> Furthermore, **`Deref` projections must be the first
> projection within any place** (if they appear at all).

Our `optimized_mir` input is `MirPhase::Runtime(Optimized)`
-- strictly later than `PostCleanup`, so the restriction
holds. Box-deref elaboration (H3 CONFIRMED) preserves this:
`elaborate_box_derefs.rs:53-91` introduces a fresh `*const T`
temp via `patch.new_temp`, inserts a `Cast(Transmute, ...)`,
rewrites `place.local` to the temp, and **preserves `Deref`
as the first projection** of the resulting Place. Compound
chains like `Field.Deref` simply cannot occur in our input.

The methodology rule "no defensive fallbacks; the toolchain
is pinned" applies: rustc gives us this invariant for free;
we don't carry spec capacity for shapes that can't appear.

*Consumer benefit.* Downstream consumers (stage 8 statements
that take `&Place`, codegen lowering Place to C lvalue trees)
get a strong contract. C-lowering becomes simply
`*head_local.projection_chain`, with the projection chain
over a non-pointer base; no need to handle nested-deref
shapes that rustc never produces.

*Why not Option B.* Allowing non-first `Deref` would model a
shape that doesn't exist in our input, weakening the
downstream contract for no gain.

*Validation cost.* O(n) per Place where n is projection
length; trivial check (for each i > 0, projection[i] !=
Deref). Often n <= 3 in practice.

**Methodology rules invoked.**

- *No defensive fallbacks; the toolchain is pinned.*
  Includes the "reject if it appears unexpectedly" clause:
  if a non-first `Deref` ever lands, the bridge raises a
  diagnostic via §3.3 Phase B place validation.
- *C semantics with information preservation.* The C lvalue
  shape is unambiguous; the spec's structural invariant
  preserves the C-shaping promise.

**Prior-art evidence.**

- `prior-art/rustc.md` §6.1 -- the `MirPhase` declaration
  documenting the `PostCleanup` restriction.
- `prior-art/rustc.md` §10.2 -- the surprise note:
  "`Deref` is first-projection-only in our input ... compound
  chains like `Field.Deref` cannot occur."
- `prior-art/rustc.md` "Hypothesis cross-check results"
  H3 CONFIRMED -- the elaborated Box-deref shape preserves
  `Deref` as first projection.

**Worked example.** A `*box.field` access (originally
`(*box).field` where `box: Box<S>` and `S` has `field: u32`):

After elaboration (rustc has already lowered the Box-deref):

```text
// Cast inserted by elaborate_box_derefs:
//   _4 = Transmute<*const S>(Copy(_3.0.0))   (stmt-level Rvalue)

// The field access:
Place {
    local: _4,                                  // raw *const S temp
    projection: [
        Deref,                                  // *_4 (first, only Deref)
        Field(FieldIdx(0), TyId(<u32>)),        // .field
    ],
}
```

The `Deref` is at position 0; the `Field` follows. I-Deref-
first holds.

**Cross-references.** Parent: D-5-1. Validation rule
location: §3.3 Phase B (D-5-2 names the phase placement).

---

### D-5-1.6 -- Codify the `Downcast`-followed-by-`Field` invariant [LOCKED]

**Question.** Should the spec codify rustc's assertion that
`Downcast(_)` must be followed by `Field(_, _)`, and should
the codification be strict (Downcast is non-last AND followed
by Field) or loose (if successor exists, it is Field)?

**Key options.**

1. **Option A -- strict codification.** Spec: "If
   `place.projection[i]` is `Downcast(_)`, then
   `i < projection.len() - 1` AND `projection[i+1]` is
   `Field(_, _)`." Validation asserts both. Downcast-as-last
   is rejected.
2. **Option B -- loose codification.** Spec: "If
   `place.projection[i]` is `Downcast(_)` and
   `i+1 < projection.len()`, then `projection[i+1]` is
   `Field(_, _)`." Allows Downcast-as-last (matches rustc's
   actual assertion shape).

**Decision.** Option A. Codified as invariant
**I-Downcast-followed-by-Field**.

**Reasoning.**

*What rustc enforces.* `PlaceTy::projection_ty` asserts at
`statement.rs:211-213`:

```rust
if self.variant_index.is_some() && !matches!(elem, ProjectionElem::Field(..)) {
    bug!("cannot use non field projection on downcasted place")
}
```

The assertion fires when narrowing the projection following
a `Downcast`. If `Downcast` is the last projection, no next
`projection_ty` call happens, so the assertion is silent.
Rustc thus formally allows `Downcast`-as-last; the assertion
only constrains successors.

*What rustc actually produces.* H4 (REFUTED) -- exhaustive
enumeration of the 3 `SetDiscriminant` construction sites in
rustc found that none use `Downcast`:

- `coroutine.rs:367` uses `Place::from(SELF_ARG)`.
- `shim.rs:770-773` uses `Place::return_place()`.
- `mir_build/.../instruction.rs:33` is the test-only `mir!`
  macro parser.

The `variant_index` field of `SetDiscriminant` alone
discriminates. No rustc construction site produces
`Downcast`-as-last in practice.

*Why Option A (strict).* The methodology rule "the toolchain
is pinned" + "no defensive fallbacks" + "reject what shouldn't
appear" together push for the strict form. Downcast-as-last
is a shape we don't see and have no codegen / consumer use
case for. The loose form (B) would accept a shape that's
empirically not produced; that's the no-defensive-fallbacks
rule violated.

*Toolchain-bump robustness.* If a future rustc adds a
construction site that produces `Downcast`-as-last, the
toolchain-bump methodology event triggers a Codex review of
the addition. The spec is revised then. Meanwhile, the strict
form gives downstream consumers a stronger contract.

*Why not Option B.* Modelling a shape we don't see weakens
the downstream contract for no compensating gain.

*Validation cost.* O(n) per Place; trivial check.

**Methodology rules invoked.**

- *No defensive fallbacks; the toolchain is pinned.*
  Includes the "reject if it appears unexpectedly" clause:
  the strict form of the invariant rejects shapes the
  pinned toolchain does not produce, and the bridge raises
  a diagnostic if `Downcast`-as-last ever appears.

**Prior-art evidence.**

- `prior-art/rustc.md` §3.6 -- the assertion at
  `statement.rs:211-213` plus the doc on `Downcast` semantics.
- `prior-art/rustc.md` §10.5 -- the surprise note: "Downcast
  is always followed by Field" (as confirmed by H4).
- `prior-art/rustc.md` "Hypothesis cross-check results"
  H4 REFUTED -- exhaustive `SetDiscriminant` enumeration
  found no `Downcast`-as-last construction.

**Worked example.** Accessing a field of an enum variant
(originally `match x { E::V { field } => field, ... }`):

```text
Place {
    local: _2,                                  // x: E
    projection: [
        Downcast(VariantIdx(1)),                // as E::V (variant index 1)
        Field(FieldIdx(0), TyId(<inner_ty>)),   // .field
    ],
}
```

The `Downcast` at position 0 is non-last; `Field` follows.
I-Downcast-followed-by-Field holds.

A would-be-rejection example (hypothetical; not produced by
rustc): `Downcast` as last projection:

```text
Place {
    local: _2,
    projection: [
        Downcast(VariantIdx(1)),                // last: rejected
    ],
}
```

This shape would be a bridge-rejection event with a
diagnostic.

**Cross-references.** Parent: D-5-1. Sibling: D-5-1.5
(I-Deref-first). Validation rule location: §3.3 Phase B
(D-5-2 names the phase placement).

---

### D-5-2 -- Place validation phase placement [LOCKED]

**Question.** Place validation has 5 rule categories
(type-coherence walk, head-local existence, FieldIdx range,
VariantIdx range, structural invariants). Where do the rules
live and where does the walk implementation land?

**Key options.**

1. **Option A -- Stage 5 owns rules 1-5; walk lands at stage
   11.** Single Phase B step at stage 11 walks body content,
   materialises Places, applies all 5 rules. Rules attributed
   to stage 5; implementation deferred to stage 11.
2. **Option B -- Split.** Stage 5 owns rules 1, 3, 4, 5
   (type-coherence + structural invariants + range checks);
   stage 11 owns rule 2 (head-local existence). Same walk,
   contributed by both stages.
3. **Option C -- Defer entirely to stage 11.** Stage 5 says
   nothing about validation; stage 11 decides rules + walk
   together.

**Decision.** Option A. Stage 5 owns all 5 rules; walk
implementation lands in stage 11's body-validation Phase B
step (TBD when stage 11 opens). Outbound expectation filed
in `stage-11/expectations.md` "From stage 5".

**Reasoning.**

*Why Option A.* Stage 5 should fully specify Place's contract
-- including what makes a Place valid. Even rule 2's reference
to `body.LocalDecls` is just naming a downstream-stage data
structure; the rule statement (head LocalId must resolve to a
LocalDecls entry) is part of Place's well-formedness contract,
which is stage-5's domain.

*D-4-K precedent.* Stage 4 owns `render_const` rules per
variant; stage 14 implements when the latest dependency
(`render_def_path`, `render_instance`) is ready. Same shape:
rules where the spec lives; implementation where dependencies
are pinned. D-4-K filed an outbound expectation in
`stage-14/expectations.md`; D-5-2 follows the same pattern
filing in `stage-11/expectations.md`.

*Single-step walk efficiency.* A single Phase B walk over
body content applying all 5 rules is cheaper than splitting
the walk into stage-5-rules + stage-11-rules passes. One
traversal, one rule set, one diagnostic surface.

*Why not Option B (split).* Splitting attribution by
"vocabulary owned by" produces no benefit. The rule
statements all hang together as "Place validation"; carving
rule 2 off into a stage-11 audit-shape entry would create
artificial boundaries in `discussions.md` for no semantic
gain. Stage 5 is the natural home for the full validation
contract.

*Why not Option C (full deferral).* Loses the structural
invariants we locked in D-5-1.5 and D-5-1.6 -- those are
**stage-5** contracts (rooted in stage-5's `PlaceElem`
vocabulary), not stage-11's to invent. The deferral rule
carries Place-contract content stage-11 wouldn't be the
natural author of.

**The 5 validation rules** (full statement; see
`spec-snapshot.md` §3.3 Phase B for the full audit-shape):

1. **Type-coherence walk.** Each `PlaceElem` step narrows
   the type from the previous step's type, per the rules
   (full per-variant table in `spec-snapshot.md` §19.4):
   - `Deref` over `Ty::Ref(inner, _)` or `Ty::Ptr(inner, _)`
     produces the resolved `inner`.
   - `Field(idx, ty)` requires the predecessor type to be
     `Ty::Struct{..}`, `Ty::Union{..}`, `Ty::Tuple{..}`, or
     `Ty::Closure(..)` (when active variant is `None`), or
     `Ty::Enum{..}` (when active variant is `Some(v)`); the
     field at `idx` in the appropriate field list has type
     `ty`. (The inline `ty` is consistent with the field
     selected by the same predecessor dispatch as rule 3
     below.) After consumption, the active variant is
     `None`. The coroutine family is rejected at type
     translation (§15.1) and therefore not in the
     predecessor set.
   - `Index(_)` requires the predecessor type to be
     `Ty::Array(_, _)` or `Ty::Slice(_)`; produces the
     element type.
   - `ConstantIndex { from_end: false, ... }` requires the
     predecessor type to be `Ty::Array(_, _)` or
     `Ty::Slice(_)`; produces the element type.
   - `ConstantIndex { from_end: true, ... }` requires the
     predecessor type to be `Ty::Slice(_)`; produces the
     element type. The cross combination (`from_end: true`
     over an array) does not appear in our input and is
     rejected by Place validation. The doc-level invariant
     from `prior-art/rustc.md` §3.4 is that `from_end` is
     always `false` when indexing an array (the MIR builder
     folds the array length into the offset rather than
     emitting a from-end form).
   - `Subslice { from, to, from_end: false }` requires the
     predecessor type to be `Ty::Array(elem, _)` and
     produces `Ty::Array(elem, to - from)`.
   - `Subslice { from, to, from_end: true }` requires the
     predecessor type to be `Ty::Slice(elem)` and yields
     `Ty::Slice(elem)` (unchanged).
   - The cross combinations (`Subslice { from_end: true }`
     over an array; `Subslice { from_end: false }` over a
     slice) do not appear in our input and are rejected by
     Place validation (§19.7) as bridge bugs. The doc-level
     invariant from `prior-art/rustc.md` §3.5 is that for
     `PlaceElem`s `from_end == true` iff the base is a
     slice; rustc's narrowing function
     (`rustc_middle/src/mir/statement.rs:224-234`) handles
     both array arms as a general utility called in non-
     `PlaceElem` contexts, but the `PlaceElem` doc fixes
     the input shape per the invariant.
   - `Downcast(VariantIdx)` requires the predecessor type to
     be `Ty::Enum{..}` (coroutine variant is §15.1-rejected);
     does not change the type but sets the active variant
     to `Some(idx)`.

2. **Head-local existence.** The head `local: LocalId` must
   resolve to a valid entry in the enclosing body's
   `LocalDecls` (stage-11 data).

3. **FieldIdx range.** For each `Field(idx, _)`, `idx`
   must be in range for the predecessor type's field list,
   dispatched by predecessor:
   `LayoutKind::Struct.fields.len()` for `Ty::Struct{..}`;
   for `Ty::Closure(item, args)`, look up the parallel
   `Ty::Struct(item, args, _)` via
   `find_nominal(NominalKind::Struct, item, args)` and use
   that struct's `LayoutKind::Struct.fields.len()` (closure-
   as-struct identity-vs-layout split per stage-3 D6.3);
   `LayoutKind::Union.fields.len()` for `Ty::Union{..}`; the
   tuple's `fields.len()` for `Ty::Tuple{..}`; the active
   variant's `VariantLayout.fields.len()` for `Ty::Enum{..}`
   after `Downcast`.

4. **VariantIdx range.** For each `Downcast(idx)`, let
   `shape` be the `EnumShape` carried in the predecessor
   `Ty::Enum{..}`'s `layout.kind` (per §11.1, the layout's
   inner `LayoutKind::Enum.shape`). Then `idx` must be in
   `0..variant_count(shape)`, where `variant_count`
   dispatches on `EnumShape`: `Empty -> 0`,
   `Single { .. } -> 1`,
   `Multiple { variants, .. } -> variants.len()`.

5. **Structural invariants.** I-Deref-first
   (D-5-1.5) and I-Downcast-followed-by-Field (D-5-1.6).

**Methodology rules invoked.**

- *Spec before implementation.* Phase-1 implementation is
  deferred to end-of-phase-1 per the README §2 binding
  rule; the rule-statements-here / implementation-when-
  dependencies-are-ready pattern follows directly. Stage 5
  specifies the contract; stage 11 implements when its
  dependencies (Body / LocalDecls) materialise.
- *Forward expectations live in `stage-N/expectations.md`.*
  D-5-2's outbound expectation lands in
  `stage-11/expectations.md` per the convention.
- *Compositional behavior beats special-case flags.* The 5
  rules compose cleanly into a single walk over body
  content; no special-case per-rule infrastructure (e.g.,
  no separate "deref-validator" / "downcast-validator"
  passes). The walk traverses Places mechanically and
  applies each rule's predicate.
- *Spec is mutable.* If stage 11 surfaces a constraint that
  calls for revising rule 2 (e.g., body validation needs a
  different walk order), the spec is revised then -- per
  README §2's "Spec is mutable" rule.

**Prior-art evidence.**

- `prior-art/rustc.md` §4 -- type-narrowing helpers
  (`PlaceTy::projection_ty`, `field_ty`,
  `multi_projection_ty`) ground the rule-1 statement
  per-variant.
- `prior-art/rustc.md` §3 -- per-variant payload semantics
  ground rule 1's per-variant constraint.
- D-4-K precedent in `../stage-4/discussions.md` -- the
  pattern of rules-here, implementation-deferred-to-where-
  dependencies-are-ready.

**Cross-references.** `spec-snapshot.md` §3.3 Phase B
addition. Outbound expectation: `stage-11/expectations.md`
"From stage 5". Sibling: D-5-1.5, D-5-1.6 (the structural
invariants encoded as rule 5).

---

### D-5-K -- `pretty.rs::render_place` deferral to stage 11 [LOCKED]

**Question.** Where does `pretty.rs::render_place` -- the
function that renders a Place to its textual form -- land?
At stage 5 with placeholder forms, or deferred to a later
stage when its name-lookup dependencies are ready?

**Key options.**

1. **Option A -- land at stage 5 with placeholder forms.**
   Output like `_N.field_K.variant_J[i_M]` -- pure numeric
   sigils, no name lookups. Functional immediately; names
   added later as a rewrite when name lookups become
   available.
2. **Option B -- defer to stage 11.** Stage 5 leaves a stub:
   `unimplemented!("stage 11 fills render_place (deferred per D-5-K)")`.
   Mirrors D-4-K -> stage-14 pattern (defer to the latest
   real dependency). Implementation lands fully named when
   stage 11 materialises `Body` / `LocalDecls` and supplies
   `LocalId` rendering.
3. **Option C -- defer to stage 12.** Originally proposed
   under the misimpression that field/variant names live in
   `Item.fields[].name` / `Item.variants[].name`. **Rejected
   on evidence cross-check:** `Item` carries `def_path`,
   `kind`, `span`, flags, `opaque_handle_eligible` -- not
   field/variant name tables (`spec-snapshot.md` §8 Item
   schema). Field and variant names actually live in the
   §11 layout schema (`FieldLayout.name: StringId`,
   `VariantLayout.name: StringId`), already pinned in
   stage 3. Stage 12 (Items / DefPath) does not introduce
   any data `render_place` needs.

**Decision.** Option B. Defer to stage 11. Outbound
expectation filed in `stage-11/expectations.md` "From
stage 5" (alongside D-5-2's place-validation walk
expectation).

**Reasoning.**

*Per-variant dependency analysis.*

| Variant | Rendering dependency | Stage |
|---|---|---|
| `Deref` | Self-contained glyph (`*`). | -- |
| `Field(FieldIdx, _)` | Predecessor type's layout (stage 3, locked) -> `LayoutKind::Struct.fields[idx].name` for `Ty::Struct{..}` / `Ty::Closure(..)`-via-parallel-Struct; `LayoutKind::Union.fields[idx].name` for `Ty::Union{..}`; tuple `fields[idx].name` for `Ty::Tuple{..}`; active variant's `VariantLayout.fields[idx].name` for `Ty::Enum{..}` after `Downcast`. Layout content is reachable at render time via the head local's `LocalDecl.ty` (stage-11 `Body`). | 11 (Body materialisation; Layout content is stage 3) |
| `Index(LocalId)` | LocalId rendering (`_N`) -- per `stage-11/expectations.md` "From stage 1". | 11 |
| `ConstantIndex { offset, ... }` | Self-contained numeric formatting. | -- |
| `Subslice { from, to, ... }` | Self-contained numeric formatting. | -- |
| `Downcast(VariantIdx)` | Enum's `LayoutKind::Enum.shape` -> `EnumShape::Multiple.variants[idx].name` or `Single.variant.name`. Layout content is stage 3, locked; reachable at render time via the head local's `LocalDecl.ty`. | 11 (same as Field) |

Plus: head `LocalId` rendering -- stage 11.

The latest dependency is **stage 11** (`Body` /
`LocalDecls` materialisation; `LocalId` `_N` sigil). All
name-lookup data is already locked in stage 3 (§11 layout
schema's `FieldLayout.name` and `VariantLayout.name`).
Stage 12 (Items + DefPath) is not a `render_place`
dependency: `Item` carries no field/variant name tables
(`spec-snapshot.md` §8). Stage 13's `render_ty` (per
stage-3 D14 deferral) is also not a dependency because
`Field`'s inline `TyId` is consumed by validation and
codegen, not by rendering.

*D-4-K precedent.* Defer to the latest real dependency in
the chain so the implementation lands complete, not
staged. Stage 4 deferred `render_const` to stage 14 (the
latest of `render_ty`, `render_def_path`, `render_instance`)
for the same reason. Here the latest real dependency is
stage 11, not stage 12: the dependency analysis has to be
grounded in the actual schema, not assumed parallels with
other renderers.

*Why not Option A.* Placeholder forms (`_N.field_K.variant_J`)
produce diagnostic text harder to read than fully named
output. The value-add of a debug renderer is named output;
intermediate placeholder versions create rewriting
churn. They also pollute consumer-side test fixtures
(insta snapshots) with placeholder shapes that get
regenerated when names land.

*Why not Option C (originally accepted, retired on
cross-check).* The evidence cross-check during the D-5-2
+ D-5-K combined Codex review (R1, BLOCK finding
`d5k-name-source-and-target`) revealed that `Item` does not
carry `fields[].name` or `variants[].name` -- the
field/variant names live in `FieldLayout.name` /
`VariantLayout.name` per §11, locked at stage 3. The
"latest dependency is stage 12" reasoning was based on a
misreading of the Item schema. Retargeting to stage 11
matches the actual schema.

*Stage 5 testing posture.* Stage 5 doesn't need
`render_place` for its own testing surface. Serde
round-trip tests cover serialisation; struct-level
`#[derive(Debug)]` covers stage-5-internal diagnostics;
snapshot tests of body content (which embed Places) live
at stages 6-11+, by which point stage 11's dependency is
met.

**Methodology rules invoked.**

- *Spec before implementation.* Phase-1 implementation is
  deferred to end-of-phase-1; this is consistent with
  D-5-K's deferral pattern (the `render_place`
  implementation lands when its dependencies are pinned and
  the impl sweep proceeds).
- *Forward expectations live in `stage-N/expectations.md`.*
  D-5-K's outbound expectation lands in
  `stage-11/expectations.md` per the convention (alongside
  D-5-2's outbound).
- *Compositional behavior beats special-case flags.*
  `render_place`'s output is composed mechanically from the
  projection chain plus existing layout-side name tables
  (`FieldLayout.name`, `VariantLayout.name`); there are no
  placeholder-rendering / special-case-rendering flags on
  `Place` or `PlaceElem`. Names are recovered from the same
  schema that codegen and validation consume.

**Bookkeeping / cross-section consistency.** The §18.2
per-method ownership table records `render_place` as
D-5-K-deferred to stage 11 (mirroring stage-4's update for
`render_const` deferred to stage 14, with the per-stage-
target derived from the actual dependency chain). This is a
spec consistency obligation rather than a README §2 binding
methodology rule.

**Prior-art evidence.**

- `prior-art/rustc.md` §3 -- per-variant payload semantics
  ground the variant-by-variant dependency analysis.
- `spec-snapshot.md` §11.0 (`FieldLayout.name`),
  §11.1 (`VariantLayout.name`) -- the actual location of
  field and variant names in the locked stage-3 / §11
  layout schema.
- `spec-snapshot.md` §8 (`Item` schema) -- shows `Item`
  carries `def_path` / `kind` / `span` / flags /
  `opaque_handle_eligible`; no `fields[]` / `variants[]`
  tables.
- D-4-K precedent in `../stage-4/discussions.md` -- the
  deferral-to-latest-dependency pattern for renderer-style
  decisions; the per-stage-target is derived from the
  actual dependency chain, not an assumed parallel with
  other renderers.

**Cross-references.** Parent: D-5-1 (the variant set
`render_place` consumes). `spec-snapshot.md` §19.8 (the
deferral commitment) + §18.2 update. Outbound expectation:
`stage-11/expectations.md` "From stage 5" (combined with
D-5-2's outbound for the place-validation walk). Sibling:
D-4-K (`render_const` deferred to stage 14 by the same
pattern). Related: D-5-2 (Place validation walk also
defers to stage 11; stage 11 is the natural integration
point for both Place-related deferrals).

---

## 2. Pedagogical primer (background concepts)

(For agents picking up stage 5 cold; supplements the
methodology-rule citations in the per-decision audit
shapes.)

### 2.1 What `optimized_mir` gives us

Regseq's bridge consumes `tcx.optimized_mir(def_id)` with
`mir_opt_level=0` and the cleanup-pass whitelist
(RemoveStorageMarkers, RemoveZsts, SimplifyLocals,
ReorderBasicBlocks, ReorderLocals, CheckAlignment, CheckNull,
CheckEnums) applied. The phase is `MirPhase::Runtime(Optimized)`
-- strictly later than `PostCleanup` and `Initial`. This
matters for `PlaceElem` shape because:

- `PostCleanup` introduces the `Deref`-first restriction
  (`prior-art/rustc.md` §6.1).
- `Runtime::Initial` strips `OpaqueCast` (under legacy solver)
  and disallows it onward (`prior-art/rustc.md` §3.7).
- The cleanup-pass whitelist does NOT rewrite projection
  chains -- it removes statements
  (`RemoveStorageMarkers`, `RemoveZsts`), renumbers
  locals/blocks (`SimplifyLocals`, `ReorderBasicBlocks`,
  `ReorderLocals`), or inserts UB-check sites
  (`CheckAlignment`, `CheckNull`, `CheckEnums`)
  (`prior-art/rustc.md` §7).

Stage 5's vocabulary is therefore narrower than rustc's
8-variant `ProjectionElem` enum: 6 mandatory variants are
reachable; `OpaqueCast` is absent under both solver paths;
`UnwrapUnsafeBinder` is gated and rejected.

### 2.2 The narrowing chain

`PlaceTy<'tcx>` is rustc's "place-type-with-variant" type
(`statement.rs:94-103`). It carries:

```rust
pub struct PlaceTy<'tcx> {
    pub ty: Ty<'tcx>,
    pub variant_index: Option<VariantIdx>,
}
```

The `variant_index: Option<VariantIdx>` field exists because
of `Downcast`: after a `Downcast`, the type is unchanged but
the *active variant* is selected. The next `Field` step
projects through that variant's field layout (rather than the
enum's general layout, which doesn't have a flat field list).
Stage 5 inherits this concept implicitly: the type-coherence
walk in §3.3 Phase B place-validation rule 1 tracks which
variant is "active" after a `Downcast`.

RegseqModel does not need its own `PlaceTy`-equivalent type
unless stage 8 (statements like `SetDiscriminant`) calls for
it. The type-coherence walk during validation can carry
`(Ty, Option<VariantIdx>)` ephemerally per walk; persisting
this state on `Place` itself would mean re-deriving it
every time consumers want it. This is a stage-8 question
(filed in `prior-art/rustc.md` "Out-of-scope items
observed").

### 2.3 The Box-deref elaboration shape

Rustc's `elaborate_box_derefs` pass (H3 CONFIRMED) preserves
the `Deref`-first invariant by introducing a fresh raw-pointer
local per Box-deref site rather than rewriting the projection
chain. The shape transformation:

Before elaboration (early MIR):
```text
*box_local         // Place { local: box_local, projection: [Deref] }
```

After elaboration (post-PostCleanup, in our input):
```text
// New statement inserted by elaborate_box_derefs:
//   _N = Transmute<*const T>(Copy(box_local.0.0))
// (where _N is a fresh raw-pointer temp via patch.new_temp)

*_N                // Place { local: _N, projection: [Deref] }
```

So our `Place` will see ordinary `[Deref, ...]` chains over
raw-pointer locals, not `Box<T>` head locals. The Box-deref
elaboration is invisible at our consumption boundary: the
stage-5 spec sees the post-elaboration shape, with `Deref`
preserved as first projection.

This also means `Box<T>` doesn't appear as a head local
type for a Place that begins with `Deref` -- the elaboration
has converted such Places to raw-pointer-based Deref shapes.
A consumer of stage-5 Places does not need to special-case
`Box<T>`-deref vs raw-ptr-deref; both look the same.

---

## 3. Open questions / follow-ups

(none currently active for stage 5)

---

## 4. References

- `../README.md` -- top-level briefing, methodology rules.
- `memory.md` -- stage-5 invariants (workflow, principles,
  read order, Codex prompt template).
- `status.md` -- stage-5 current state.
- `spec-snapshot.md` -- the working spec.
- `expectations.md` -- inbound expectations register.
- `review-log.md` -- round-by-round Codex review trail
  (created when the first review round runs).
- `prior-art/rustc.md` -- primary-source rustc evidence
  for D-5-1.
- `../spec.md` -- top-level merged spec.
- `../discussions.md` -- top-level merged audit trail.
