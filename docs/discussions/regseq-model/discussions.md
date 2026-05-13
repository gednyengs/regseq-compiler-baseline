# Discussion record

Merged top-level audit trail for RegseqModel stage closures. Per
README §5.5 step 1, this file is assembled by the merger at
end-of-stage and contains the per-stage `discussions.md` content
verbatim under per-stage headers (mechanical concat; no
summarization). Per-stage discussions live in
`stage-N/discussions.md` and are the authoritative artefact
during stage work; this file is the merged record across
closed stages.

Stage 1 used legacy F-tag form and did not produce a
`discussions.md` under the new convention. Stages 3 and 4
are the first two stages to close under the new convention
per README §5.5 (the append-if-exists rule); this file was
created on stage 4's merger (the second stage to close).

---

## Stage 3 (type system)

# Stage 3 -- Discussion Record

Distilled record of the design dialogue that produced the stage-3
type-system spec. Each entry captures: the question raised, key
options considered, the decision (or current lean for open
decisions), the reasoning chain, and cross-references to
`spec-snapshot.md` (the spec) and `review-log.md` (the audit trail).

Plus a concise reference primer (Section 2) for foundational
concepts that came up during discussion -- Layout / LayoutData,
the layout-responsibility split between our IR and the C
compiler, and MIR's abstract treatment of niche encoding.

Status of decisions in this file mirrors `spec-snapshot.md` Section
5 (Open decisions table) and `memory.md` Section 1 (stage-3 memory).

---

## 1. Decision discussions

### D1.1 -- Variant scope: rigid post-monomorphization [LOCKED]

**Question.** Which `TyKind` variants does our `Ty` enum carry?
Which are rejected, and why?

**Key options.**
1. Full rigid set per rustc's `is_known_rigid()` (~22 variants).
2. Aggressive collapse à la rustc_codegen_clr (~14 variants).
3. Middle: ~16-19, keeping the distinctions consumers actually
   need.

**Decision.** Option 3, refined to **19 variants**: `Bool, Char,
Int, Uint, Float, Struct, Enum, Union, Str, Array, Slice, Ptr,
Ref, FnDef, FnPtr, Dynamic, Closure, Never, Tuple`. Rejection
list is split (D1.4) into rigid-but-DSL-rejected (Foreign, Box,
Coroutine family, Pat, UnsafeBinder) and non-rigid (Param, Bound,
Placeholder, Infer, Error, Alias::*).

**Reasoning.** Rustc has done the heavy lifting --
monomorphization substitutes Param/Bound; `normalize_erasing_regions`
substitutes Alias; `erase_and_anonymize_regions` rewrites free
regions. By the time the bridge consumes a fully folded type,
only rigid variants remain. Carrying typeck/intermediate variants
imports baggage we cannot interpret. Convergent vocabulary size
across rich Rust/LLVM-level closed-vocabulary projects is 14-22
(Charon 14, CLR 14, mrustc 15, LLVM 22); 19 sits in that band.

**Cross-refs.** `spec-snapshot.md` Section 1.2; `review-log.md` rounds
1, 3 (Coroutine rationale grounded in DSL scope), 4 (count
correction 28 -> 29 for rustc).

### D1.2 -- ADT kind separation: Struct / Enum / Union as siblings [LOCKED]

**Question.** Should our IR have a single `Ty::Adt` variant
(rustc-style) or three siblings `Ty::Struct`, `Ty::Enum`,
`Ty::Union`?

**Key options.**
1. Single `Ty::Adt(ItemId, GenericArgs)` à la rustc / mrustc /
   Charon.
2. Three siblings, each carrying inline structure + layout (à la
   rustc_codegen_clr ClassDef body shape).

**Decision.** Three siblings. Stage-1 spec already named them as
the targets stage 3 must populate; matches `rustc_codegen_clr`
which is the closest precedent for our use case (Rust -> C, one
IR, multiple printers).

**Reasoning.** The three ADT kinds have *distinct C lowerings*:
struct -> C struct with field offsets; enum -> tagged or niche-
encoded layout (potentially radically different shape); union ->
C union. Codegen always pattern-matches on which kind it has, so
flattening at the type level removes a hop. Per-sibling layout
also pre-positions for D2 (explicit-layout-in-IR).

**Cross-refs.** `spec-snapshot.md` Section 1.3; `expectations.md`
"Define the layout-pointing Ty variants."

### D1.3 -- Option/Result as ordinary `Ty::Enum` [LOCKED]

**Question.** Should `Option<T>` and `Result<T, E>` be first-
class `Ty` variants (as in `regseq-model-old`), or modeled as
ordinary two-variant enums?

**Key options.**
1. First-class `Option(TyId)` and `Result(TyId, TyId)` variants.
2. Ordinary `Ty::Enum` with two variants each.

**Decision.** Option 2 (ordinary enums). Explicitly *rejects* the
`regseq-model-old` design choice.

**Reasoning.** Niche optimization is a *layout-level* concern,
not a type-level one. Rustc's `LayoutData` already encodes whether
a given enum gets niche-encoded; treating `Option<&u32>` as
ordinary `Ty::Enum` produces niche-optimized layout from
`tcx.layout_of` "for free." The argument also doesn't generalize
-- if Option/Result are special, why not `Ordering`, `ControlFlow`,
`Poll`? No other surveyed project carves them out (rustc, mrustc,
Charon, CLR all model as ordinary enums). Per "re-derive from
evidence; do not import baggage," the old-IR choice is reference,
not authority.

**Cross-refs.** `spec-snapshot.md` Section 1.4; `spec-snapshot.md`
Section 4.4 (`Option<&u32>` worked example); `discussions.md`
Section 2.1 (Layout primer).

### D1.4 -- Rejection list (two categories) [LOCKED]

**Question.** Which variants does the bridge reject at type
construction, and how is the rejection categorized?

**Decision.** Two explicit categories.

*Category A -- Rigid but DSL-rejected.* `Foreign`, `Box`
(`Adt` flagged `IS_BOX`), `Coroutine` family
(Coroutine/CoroutineClosure/CoroutineWitness), `Pat`,
`UnsafeBinder`. Rigid per `is_known_rigid()`; rejected for v1
DSL scope.

*Category B -- Non-rigid (cannot survive monomorphization).*
`Param, Bound, Placeholder, Infer, Error, Alias::Projection,
Alias::Free, Alias::Inherent, Alias::Opaque`. Should never reach
the bridge after the mandatory fold; if they appear, the bridge's
typing-env or pass whitelist is broken.

**Reasoning per Category-A entry.**
- `Foreign`: `extern type T` is unstable; the opaque-struct pattern
  (`#[repr(C)] struct T { _private: [u8; 0] }`) is the universal
  alternative supported by every project that handles ADTs.
- `Box`: defined in `alloc`, not `core`. The DSL surface is
  `core`-only with no other crates required by the language; Box
  cannot appear in driver MIR. (User-confirmed DSL scope rule.)
- `Coroutine family`: regseq DSL is `#![no_std]` with no runtime
  facilities (`regseq-background.md:34-38`); driver specs do not
  invoke async. Stage 1 already established this position
  (`stage-1/spec-snapshot.md:81`).
- `Pat`: integer-range patterns only today; not in driver use.
- `UnsafeBinder`: requires unstable `feature(unsafe_binders)`;
  for self-referential lifetime-erased types; unlikely in driver
  code.

**Cross-refs.** `spec-snapshot.md` Section 1.5; `review-log.md` round 3
(Coroutine rationale grounded in cited DSL scope); round 1 (Box
rejection logic).

### D1.5 -- Lang-item flags on `Item` [LOCKED]

**Question.** Which lang-item-flagged ADT types does the bridge
recognize, and where does the recognition live?

**Key options.**
1. Five flags (Box, PhantomData, UnsafeCell, MaybeUninit,
   ManuallyDrop, Pin) on `Item`.
2. Three flags (PhantomData, UnsafeCell, ManuallyDrop), with
   Box/Pin/MaybeUninit handled differently or not at all.

**Decision.** Option 2. Three flags on `Item`:
- `is_phantom_data` -- omit field at C-emit time (0-byte ZST).
- `is_unsafe_cell` -- drop `const` qualifier on enclosing
  references.
- `is_manually_drop` -- suppress drop-glue Calls for the wrapped
  value.

All three are cached in rustc's `AdtFlags`, so the bridge uses
`adt_def.is_phantom_data()` / `is_unsafe_cell()` /
`is_manually_drop()`. `tcx.is_lang_item(...)` is reserved for
lang items not in `AdtFlags` (`MaybeUninit`, `CVoid`, `VaList`).

**Why not Box/Pin/MaybeUninit.**
- `Box`: rejected entirely (D1.4) -- not a "flag," it's a
  rejection signal via `IS_BOX`.
- `Pin`: no codegen consumer for synchronous driver code (move-
  blocking is enforced pre-MIR by typeck). Recognizing without a
  v1 consumer violates "no defensive fallbacks" + "ask who consumes
  it." Treated as ordinary `Ty::Struct` for v1; reintroduce when
  async support arrives.
- `MaybeUninit`: drop-suppression is achieved structurally by the
  composition of (a) `Ty::Union` no auto-drop + (b) inner
  `is_manually_drop` flag. No separate flag needed. (See Section
  2.3 / `spec-snapshot.md` Section 4.6.)

**Reasoning.** Each flag drives a *specific* and *necessary* C-
codegen decision that no general rule subsumes. Without
`is_phantom_data`, layouts break (C structs require non-zero size).
Without `is_unsafe_cell`, references emit `const T*` which conflicts
with interior mutability (C UB on write). Without
`is_manually_drop`, drop-glue Calls cause double-frees or drops on
escaped resources. Conflating "shape" and "lang-item identity"
inflates the variant set without semantic gain, so flags live on
`Item`, not as separate `Ty` variants.

**Cross-refs.** `spec-snapshot.md` Section 1.6; Section 4.3
(PhantomData omission); Section 4.5 (UnsafeCell); Section 4.6
(MaybeUninit composition); Section 4.7 (ManuallyDrop);
`review-log.md` round 1 (initial 5-flag set), round 3
(`AdtFlags` vs `tcx.is_lang_item` recognition path split).

### D1.6 -- Auxiliary variant decisions [LOCKED]

Three small consolidations:

**D1.6.a Str as marker variant.** `Ty::Str` carries no payload --
its sole role is to let the bridge distinguish `Ref(Str, _)`
(fat pointer with explicit `len`, no NUL terminator) from
`Ptr(Int(I8), _)` (thin pointer, NUL-terminated by C
convention). Rust's `str` is fundamentally different from C
`const char *`: UTF-8, length-prefixed, DST-only. C-side: 2-word
struct `{ const uint8_t *data; size_t len; }`.

**D1.6.b FnDef and FnPtr distinct.** `FnDef(ItemId, GenericArgs)`
is the anonymous ZST type of a specific function name (carries
identity); `FnPtr(FnSigId)` is a function-pointer (carries only
signature, address-bearing). Conflating loses the per-call-site
monomorphization key. CLR collapses fn-items to `Type::Void` --
the outlier; rustc, mrustc, Charon all keep distinct.

**D1.6.c Never separate variant.** `Ty::Never` represents `!`.
Distinct from any uninhabited Adt and from `c_void`. C lowering
of `fn() -> !` is `__attribute__((noreturn)) void f(void);` --
`void` is the syntactic return-type slot, `noreturn` carries
non-return semantics. Complementary, not equivalent.

**Cross-refs.** `spec-snapshot.md` Section 1.7; `review-log.md`
round 3 (Never lowering clarification).

### D2 -- Layout position: explicit `Layout` field on `Ty` [LOCKED]

**Question.** Where does layout information live? On the type, or
in a separate table?

**Key options.**
1. Explicit `Layout` field inline on `Ty::Struct/Enum/Union` (à la
   rustc_codegen_clr ClassDef).
2. Separate `IndexVec<TyId, Layout>` table à la rustc_public, LLVM,
   MLIR, Charon.
3. Hybrid: minimal layout in IR, full LayoutData on demand.

**Decision.** Option 1 (refined to "Option 3 hybrid" by D11): each
`Ty::Struct/Enum/Union` carries a `Layout` field inline. The
`Layout` is the *resolved result* of `tcx.layout_of(ty)`, copied
by the bridge at type construction. **Layout is not computed by
the bridge or codegen; rustc owns the layout algorithm; we own
preserving the answer.**

**Reasoning.** Per the methodology rule "C semantics with
information preservation": store the result, not the input.
Recomputing in the bridge duplicates work and risks divergence;
deferring to codegen requires back-pressure to query the bridge's
stored types or recomputation from scratch (no rustc downstream).
The "explicit-layout-in-IR" posture also fits our "one IR, two
printers" case (Linux + Zephyr both C, sharing layout rules) --
rustc_codegen_clr is the direct precedent. The cost (binding to
one ABI's layout) is what we want anyway: compiler instances are
per-target.

This pins the principle: **don't redo work the rustc compiler has
already done.**

**Cross-refs.** `spec-snapshot.md` Section 2.1; `discussions.md`
Section 2.1 (Layout / LayoutData primer); Section 2.2 (layout-
responsibility split).

### D3 -- Interning strategy [LOCKED]

**Question.** How does our IR achieve type identity?

**Key options.**
1. Structural interning with arena (rustc, rustc_codegen_c).
2. BiMap with `Interned<u32>` newtype handles (rustc_codegen_clr).
3. Process-global hash-cons via RwLock (Charon).
4. No interning, refcounted (mrustc).

**Decision.** Option 2 (BiMap). `TyInterner` carries
`storage: IndexVec<TyId, Ty>` plus `lookup: HashMap<Ty, TyId>`.
`Ty`'s `Hash + Eq` follow a **nominal/structural split**:
ADT-shaped variants (Struct/Enum/Union/FnDef/Closure) hash and
compare by `(item: ItemId, args: GenericArgs)` only -- with the
inline `Layout` (or closure capture struct, etc.) excluded. All
other variants derive `Hash + Eq` structurally. This nominal
identity for ADTs (a) mirrors rustc's own `AdtDef` interning
which keys on `DefId` only (`findings-rustc-draft.md` Theme C.2),
and (b) lets self-referential ADTs intern via `TyId` reservation
without cycle (see invariant 3 below).

`TyId` is u32-backed with niche-optimised `Option<TyId>`
(stage-1 `:117-128`). Stage 3 picks `NonMaxU32` for `TyId`'s
niche mechanism.

**F4-2 ArgsKey uses a deep canonicalizer, distinct from BiMap
equality.** Stage 1 specifies that `ArgsKey` compares type args
by structural `Ty` representation
(`stage-1/spec-snapshot.md:1254-1257`), with the canonical form
pinned in stage 3. The canonical form is pinned per Ty variant
kind (full schema: `spec-snapshot.md` Section 3.1):

- **ADT-shaped variants** (`Struct`, `Enum`, `Union`, `FnDef`,
  `Closure`): canonical encoding is exactly
  `(variant_kind_tag, DefPathHash, canonical_args)`. The inline
  `Layout` is **not traversed** (functionally determined by
  `(item, args)`; traversal would reintroduce self-reference
  cycles).
- **Non-ADT variants** (`Bool`, `Char`, `Int`, `Uint`, `Float`,
  `Str`, `Array`, `Slice`, `Ptr`, `Ref`, `FnPtr`, `Dynamic`,
  `Never`, `Tuple`): structural recursion over fields, with
  per-ID-type termination rules.

Per-IR-identifier termination rules (full text:
`spec-snapshot.md` Section 3.1; summarised here):

- `TyId` → resolve via `model.types.resolve` and recurse.
- `ItemId` → terminate at `DefPathHash` from
  `model.item_def_paths`.
- `FnSigId` → resolve via `model.sigs.resolve` and inline the
  `FnSig`'s contents (input/output Tys + calling-convention),
  recursing on inner Tys.
- `StringId` → string content via `model.strings.resolve`
  (rarely reached in practice -- `DefPathHash` already covers
  path content end-to-end).
- `ConstId` → canonical const-eval result (pinned in stage 4).

This canonicalizer is *separate from the BiMap's in-build
`Hash + Eq`* (which compares nested `TyId` integers shallowly --
sound within a build because BiMap dedup gives content-equal Tys
equal TyIds, but not portable across builds). The two notions
coexist because they answer different questions: "is this Ty
already interned?" (shallow, in-build) vs "is this Ty's content
equivalent across builds?" (deep, cross-build). InstanceId
ordering is reproducible across builds when reachable sets
match (`stage-1/spec-snapshot.md:1067-1070, :1232-1236,
:1262-1264`); TyId integer values are stable as a side effect
when the bridge walks instances in F4-2-sorted order, but are
not load-bearing for F4-2 stability.

**Reasoning.** Five concurrent constraints rule out the
alternatives:

1. *F4-2 ArgsKey uses a deep canonicalizer*, separate from the
   BiMap's shallow `Hash + Eq`. Cross-build determinism is
   delivered by walking content (via `resolve`) plus the
   nominal-identity rule for ADTs, not by TyId integer stability.
2. *No process-global state* (Q1, hygiene) rules out Charon's
   RwLock-keyed global hash-cons.
3. *Q2 eager translation* forbids anything keyed on
   `rustc::Ty<'tcx>` from outliving the bridge call.
4. *O(1) `Eq`/`Hash` on `TyId`* is required at every comparison
   site; mrustc-style refcount loses this. Dense u32 indices
   give O(1) at every site.
5. *Dense u32 IDs* per stage-1 lock at `:117-128`.

What survives: `rustc_codegen_clr`-style BiMap with the
nominal/structural identity refinement, which is the direct
precedent for our "Rust → C, single IR, multiple printers" use
case. Self-referential ADTs intern via TyId reservation:
allocate `TyId(K)` for `Ty::Struct{item, args, layout: PENDING}`,
recursively compute the layout (which may circle back to `TyId(K)`
via the nominal-identity hash hit), then patch the layout into
`storage[K]`. The `lookup` key remains valid because identity
hashing excludes layout content.

**Cross-refs.** `spec-snapshot.md` Section 3.1 (decision body),
Section 3.3 (worked walkthrough); `findings-rustc-draft.md`
Theme C.1 (rustc `intern_ty`); `prior-art/rustc-codegen-clr.md`
Theme C; `prior-art/charon.md` Theme C; `prior-art/mrustc.md`
Theme C; `discussions.md` Section 2.5 (TyInterner pattern primer).

### D4 -- Interning location [LOCKED]

**Question.** Where does the interner live -- in the bridge crate
or in the IR crate?

**Key options.**
1. Bridge-side BiMap; IR exposes only `IndexVec<TyId, Ty>`
   post-build.
2. IR-side: `RegseqModel.types: TyInterner` mirroring the
   stage-1 `StringInterner` precedent.
3. Process-global (Charon style) -- ruled out by Q1.

**Decision.** Option 2. The `types` field of `RegseqModel`
(stage-1 placeholder `IndexVec<TyId, Ty>` at
`stage-1/spec-snapshot.md:1002`) is refined to `TyInterner`,
matching `RegseqModel.strings: StringInterner` in shape. Public
methods are `new`, `intern(&mut self, Ty) -> TyId`,
`resolve(&self, TyId) -> &Ty`, `find(&self, &Ty) -> Option<TyId>`,
`len`, `iter`. The `find` method is an intentional asymmetry
from `StringInterner` -- it exposes the structural-identity
reverse lookup as a non-mutating consumer surface (used by
post-build invariant checks and the F4-2 ArgsKey dedup-cache),
where `StringInterner` currently has no corresponding method.
Custom serde: on-wire is `storage` only; `lookup` rebuilt at
deserialize.

A separate, bridge-local `HashMap<rustc::Ty<'tcx>, TyId>`
translation cache lives in regseq-driver -- *not* in regseq-model
(Q1: zero rustc deps). It dies with the bridge call (Q2). The
two layers compose: the bridge cache short-circuits rustc-Ty-keyed
re-translation; the IR's structural `lookup` is the final
authority on identity (and dedups synthesised `Ty` values that
never had a rustc-side origin -- ADT-shaped synthesised closures
becoming `Ty::Closure(synthesised_ItemId, GenericArgs)` per D6;
ADT-shaped synthesised vtable types becoming
`Ty::Struct(synthesised_ItemId, GenericArgs, layout)` per D5
(the vtable type Item -- trait-object fat pointers themselves
are *not* interned: D5.3-B says `Ty::Dynamic` survives in the
IR and codegen emits the fat-pointer C typedef on demand).
Drop-glue is a function decl, not a `Ty`, and does not flow
through `TyInterner`).

**Reasoning.** The original lean was bridge-side only, but
recognising the `StringInterner` precedent at
`stage-1/spec-snapshot.md:250-254` strictly improves the position:

- Borrow-checker-enforced mutability: `intern` needs `&mut self`;
  post-build consumers hold `&RegseqModel`, so accidental post-
  finalisation interning is rejected at compile time. No separate
  Builder/Frozen split required.
- Test ergonomics: `RegseqModel::new()` + `model.types.intern(...)`
  works identically to `model.strings.intern(...)`; integration
  tests construct hand-built models without rustc.
- Serialization symmetry: one custom-serde shape, two consumers.
- Co-locating the structural-equality logic with the value type
  that defines it.

**Cross-refs.** `spec-snapshot.md` Section 3.2; `architecture.md`
Q1 + Q2; `stage-1/spec-snapshot.md:250-272` (`StringInterner`
precedent), `:1002` (placeholder field declaration);
`discussions.md` Section 2.5.

### D5 -- `Dynamic` lowering [LOCKED]

**Question.** How does `Ty::Dynamic` represent trait objects in
our IR, where does the lowering to fat-pointer C output happen,
and how does work distribute across stages?

**Key options considered.**

*For variant content (D5.1):*
1. Mirror rustc verbatim: `Dynamic(BoundExistentialPredicates,
   Region)` -- full HRTB binder + predicates + outer region.
2. Charon-style `DynPredicate { binder }` -- single binder over
   `ExistentialPredicate` enum.
3. Slimmer flat shape: `{ principal: TraitRef, projections,
   auto_traits, region }` with `principal_item: ItemId`.
4. Most-deferred: `Ty::Dynamic(DynItemId)` opaque, predicates in
   a side table.

*For lowering location (D5.2):*
1. In the bridge (early materialization).
2. In a regseq-model pass (mid).
3. In codegen (late).
4. Distributed across stages (eventually adopted).

*For lowered shape (D5.3):*
- A: vtable type as nominal `Ty::Struct` Item.
- B: fat-pointer rewritten in IR / codegen-only.

**Decision.**

*Variant content (D5.1):*
```rust
Ty::Dynamic {
    auto_traits: Vec<AutoTrait>,    // sorted by enum-discriminant order
    vtable_ty:   TyId,              // -> synthesised vtable Ty::Struct (details in stage-12 expectations)
}

pub enum AutoTrait { Send, Sync, Unpin }   // closed v1 set
```

No `principal`, no `projections`, no `region` slot. Free regions
erased uniformly; HRTB binders (`for<'a>` on `dyn` types) are
**rejected** at the bridge in v1 per the D7/D8 refinement
(`spec-snapshot.md` Section 1.8) -- they survive rustc's region
erasure as `ReBound` and would collapse rustc-distinct types
under uniform IR erasure. Custom auto-traits rejected at the
bridge.

*Lowering location (D5.2):* Distributed across stages. Stage-3
commits to the `Ty::Dynamic` shape and `vtable_ty: TyId` slot,
populated by the bridge at type-translation time. Stage-9 owns
the post-rewrite Call terminator shape; stage-11 owns the
body-level dispatch-rewriting pass; stage-12 owns the
synthesis details (vtable type Item, vtable instance Item,
vtable shim Function Items, `RegseqModel.vtables` dedup table).
Codegen emits the fat-pointer C typedef on demand for
`Ref(t, _)` / `Ptr(t, _)` where `t` resolves to `Ty::Dynamic`
(per locked D7/D8 in Section 1.8).

*Lowered shape (D5.3):*
- D5.3-A: `vtable_ty: TyId` resolves to a `Ty::Struct` value;
  the `Ty::Struct`'s Item, DefPath, hash, layout, and surrounding
  synthesis machinery are stage-12 territory.
- D5.3-B: `Ty::Dynamic` survives in the IR through to codegen;
  the fat-pointer C shape `(data*, vtable*)` is emitted inline by
  codegen, mirroring the existing handling of `Ty::Str` and
  `Ty::Slice` per `spec-snapshot.md` Section 4.2. Pointer
  qualifications follow locked D7/D8 (Section 1.8) and the
  C-qualification policy in Section 4.1; four pointer-kind
  variants exist (`&dyn Trait`, `&mut dyn Trait`, `*const dyn
  Trait`, `*mut dyn Trait`) per the table in `spec-snapshot.md`
  Section 1.10.4.

**Reasoning.**

*Bottom-up grounding from C semantics.* The C target shape is a
vtable struct + per-impl static const instance + fat pointer.
This matches the Linux/Zephyr `*_operations` driver-dispatch
idiom one-to-one. Once that target is fixed, the IR layering
question reduces to "which IR entities exist for vtable struct
vs vtable instance vs fat pointer" -- and those entities split
naturally across stage 3 (type-level), 12 (Item-level), and
codegen (C output).

*Trait identities cannot use `ItemId`.* Stage-1 erases trait
declarations (`stage-1/spec-snapshot.md:608-611`), associated types
(`stage-1/spec-snapshot.md:605-607`), and closes `ItemKind` to six
variants (`stage-1/spec-snapshot.md:625-627`) excluding any
trait-related kind. So a `principal: TraitRef { item: ItemId,
args }` field as initially considered is structurally
impossible. The trait identity must flow through a different
carrier: stage-1 reserved the bridge-synthesised vtable Static
Item per `(Trait, SelfTy)` (`stage-1/spec-snapshot.md:910-927`)
keyed by `(trait_def_path_hash, self_ty_canonical_hash)`, and
SESSION-A's design extends that framework with a parallel vtable
type Item per `(Trait, projection_bindings)` (forwarded to
stage-12).<sup>†</sup> What remains in `Ty::Dynamic` is only what
isn't otherwise expressible: auto-traits (which affect type
identity but not codegen) and the `vtable_ty` link.

> <sup>†</sup> *Round 7 refinement (post-stage-close
> maintenance, see `review-log.md` Round 7.1 fix manifest item
> 4 / P0.4):* the stage-3 SESSION-A keying `(Trait,
> projection_bindings)` collapses generic-trait objects like
> `dyn Register<u32>` and `dyn Register<u64>` (same trait
> path, same projection bindings, different `trait_args`).
> Round 7 sharpened the keying to a full
> `PrincipalTraitKey { trait_path, trait_args,
> projection_bindings }` to prevent the collapse. The
> conceptual decision (per-trait-object-identity vtable type
> Items) is unchanged; only the per-key fields expanded. Live
> spec is `spec.md` §13.3.

*Auto-traits affect type identity, not C output.* `dyn Driver +
Send` and `dyn Driver` are distinct Rust types -- the BiMap must
dedup them as distinct `Ty::Dynamic` entries -- but the C-level
fat pointer and vtable struct are identical. The closed
`AutoTrait::{Send, Sync, Unpin}` enum captures all v1 cases;
custom auto-traits are unstable and rejected at the bridge.
Sort order is enum-discriminant order for canonical
content-equality.

*Region erasure.* After a bottom-up audit, no free-region
material has any C-relevant content in our IR. Type-checking
and borrow-checking happen pre-MIR; type correctness for any
HRTB obligations is verified before we see the type. **HRTB
binders are not erased silently in v1**, however -- they
survive rustc's `erase_and_anonymize_regions` as `ReBound`
(`findings-rustc-draft.md` W.13), and erasing them in our IR
would collapse rustc-distinct higher-ranked function-pointer /
trait-object types under the type-identity argument D15 uses
for `FnAbi`. v1 policy (D7/D8 refinement,
`spec-snapshot.md` Section 1.8): the bridge rejects HRTB-bearing
`Ty::FnPtr` and `Ty::Dynamic` at type translation. Section 2.6
of this document records the "erasure for free regions,
rejection for HRTB" stance as a named methodology principle.

*Stage distribution.* The original framing considered a
Charon-style monolithic "lowering pass" between bridge and
codegen. That framing collapses because each piece of work has
a natural home: type-level vtable `Ty::Struct` synthesis is
*Item* synthesis (stage 12 territory per stage-1's
"Bridge-synthesized Items" framework); statement-level
rewriting walks bodies (stage 11); the post-rewrite Call shape
is a terminator question (stage 9). Distributing the work
respects stage-1's reservations and gives each stage a focused,
implementable deliverable. SESSION-A produces the stage-3
contract plus three forward-expectations (stage-9, stage-11,
stage-12 expectations.md) so the downstream stages start with
the design analysis rather than from scratch.

*Fat-pointer symmetry with `&str` / `&[T]`.* All three are
unsized-pointee fat pointers in Rust; all three should stay as
`Ref(t, _)` (or `Ptr(t, _)`) in the IR with codegen
pattern-matching to emit the C typedef. Treating `&dyn Trait`
consistently (D5.3-B) avoids special-casing.

*Bridge-side `vtable_ty` population.* The bridge has all needed
information from rustc's `Dynamic` predicates at
type-translation time. The `vtable_ty: TyId` slot is populated
eagerly inline; the bridge mints (or looks up via stage-12's
`RegseqModel.vtables` dedup table) the vtable type Item, then
interns the corresponding `Ty::Struct`, then constructs
`Ty::Dynamic` with the resulting TyId.

**Cross-refs.**

- `spec-snapshot.md` Section 1.10 (D5 audit-shape spec).
- `spec-snapshot.md` Section 4.9 (`&dyn Driver` worked example).
- Section 2.6 of this document (region-erasure primer).
- `stage-9/expectations.md`, `stage-11/expectations.md`,
  `stage-12/expectations.md` (forward-expectations seeded by
  SESSION-A).
- `findings-rustc-draft.md` Theme A.4 (rustc `Dynamic`), E.6
  (HRTB ReBound preservation), W.13 (free vs bound region
  distinction in rustc).
- `prior-art/charon.md` F.3 (`DynPredicate` / vtable as
  TypeDecl); `prior-art/mrustc.md` F (`TRAITOBJ_PTR` /
  `emit_ctype_ptr`).
- `stage-1/spec-snapshot.md`:608-611 (trait declaration erasure);
  :605-607 (associated-type erasure); :625-627 (six-variant
  ItemKind); :884-895 (vtable Ty deferred to stage 9/12);
  :901-942 (Bridge-synthesized Items framework, vtable Static
  Item reservation).

### D6 -- Closure naming and synthesis [LOCKED]

**Question.** How does the bridge name and materialize closure
types in the IR? What Items are synthesised per closure, what is
their DefPath shape, and how does `Ty::Closure` connect to the
captures struct's layout?

**Key options considered.**

*For Item materialization (D6.1):*
1. One Item per closure (kind = Struct), with captures as
   fields. Fn/FnMut/FnOnce projections as separate Function
   Items.
2. One Item per closure-and-captures combo with multiple facets
   -- doesn't fit the closed six-kind ItemKind.

*For DefPath naming (D6.2):*
1. Reuse rustc's `DefPath` verbatim, including
   `DefPathData::Closure` segments.
2. Synthesise a fresh naming scheme.

*For `Ty::Closure` -> captures connection (D6.3):*
1. Implicit via shared `(ItemId, GenericArgs)`; consumers use
   `model.types.find` to navigate to the parallel `Ty::Struct`.
2. Explicit `captures_ty: TyId` slot in `Ty::Closure`.
3. Single `Ty::Closure` carries layout inline -- re-opens
   D2/D11.

**Decision.**

*D6.1 (Item materialization contract):* Per closure the bridge
synthesises:
- One captures-struct Item (`ItemKind::Struct`, fields = upvars,
  layout from `tcx.layout_of`).
- 1-3 Function Items for call methods, depending on
  `ClosureKind` and the trait hierarchy `Fn` : `FnMut` : `FnOnce`
  (an `Fn` closure materialises all three; `FnMut` two; `FnOnce`
  one).
- Drop-glue Function Item iff the captures need dropping.

The synthesis *mechanics* (how the Items are produced, ItemId
assignment, DefPath construction details) are forwarded to
`stage-12/expectations.md`.

*D6.2 (DefPath naming):* Reuse rustc's DefPath verbatim for the
captures-struct Item (using stage-1's `DefPathData::Closure`
segment, with `{namespace#N}` verbose rendering per
`stage-1/spec-snapshot.md:545-554`). For call-method Function
Items, the bridge constructs a child DefPath by appending a
`ValueNs(intern("call" / "call_mut" / "call_once"))` segment
with disambiguator `0`. No new `DefPathData` variants needed for
closures.

*D6.3 (`Ty::Closure` -> captures struct):* Implicit, via shared
`(ItemId, GenericArgs)`. `Ty::Closure(item, args)` and
`Ty::Struct { item, args, layout }` share one synthesised Item
(the captures struct Item). Consumers reach the layout-bearing
view via `model.types.find(&Ty::Struct { item, args, layout: _ })`,
which works because ADT-shaped identity ignores the layout
field per D3.

`Coroutine` / `CoroutineClosure` / `CoroutineWitness` are
rejected per D1.4 (out of v1 scope; async/coroutine).

**Reasoning.**

*Materialised projections, not abstract trait impls.* Stage-1
(`stage-1/spec-snapshot.md:600-611`) erases trait declarations and
impl blocks; impl methods are lifted to top-level Function
Items. Closures inherit the same model: their implicit
Fn/FnMut/FnOnce trait impls are erased; the call methods are
materialised as Function Items.

*Trait hierarchy determines the count.* `Fn` extends `FnMut`
extends `FnOnce`; a `Fn` closure satisfies all three traits, so
all three call methods are materialised. Skipping inherited
methods would mean a `Fn` closure couldn't be coerced to `&dyn
FnOnce`.

*DefPath reuse gives cross-build identity for free.* Rustc's
DefPaths feed the F4-2 deep canonicalizer (D3 reasoning
constraint 1) directly via `DefPathHash`. Inventing a fresh
naming scheme would require parallel cross-build stability
machinery for no benefit.

*Implicit connection avoids redundant duplication.* The shared
`(ItemId, GenericArgs)` *is* the connection; an explicit
`captures_ty: TyId` slot would carry no information not already
derivable. This contrasts intentionally with `Ty::Dynamic.vtable_ty`,
where the slot is information-bearing because the vtable Item is
*not* the same as anything else in `Ty::Dynamic`.

*Two `Ty` views of one Item.* `Ty::Closure` (identity view, no
layout) and `Ty::Struct(...)` (layout-bearing view) co-exist for
the same Item. The split keeps `Ty::Closure` available for
pretty-printing and dispatch-convention purposes (rendering
`{closure#0}`, distinguishing closure call sites from regular
struct value moves) while routing layout queries through the
existing `Ty::Struct` machinery (D2).

*Synthesis mechanics forwarded to stage 12.* The actual bridge
work to mint the closure-as-struct Item, the call-method
Function Items, and the drop-glue Function Item is Item-synthesis
work, fitting stage 12's domain per stage-1's "Bridge-synthesized
Items" framework (`stage-1/spec-snapshot.md:901-942`). SESSION-A
commits to the surface contract; `stage-12/expectations.md`
captures the implementation analysis.

**Cross-refs.**

- `spec-snapshot.md` Section 1.11 (D6 audit-shape spec).
- `spec-snapshot.md` Section 4.10 (closure worked example).
- `stage-12/expectations.md` (forward-expectation, including
  closure synthesis details).
- `findings-rustc-draft.md` Theme A.3 (`ClosureArgs`,
  `ClosureKind`).
- `prior-art/charon.md` F.2 (`translate_closures.rs`);
  `prior-art/mrustc.md` F (closure-as-struct emission);
  `prior-art/rustc-codegen-clr.md` B (`closure_typedef`).
- `stage-1/spec-snapshot.md`:472 (`DefPathData::Closure`); :545-554
  (verbose anon-namespace rendering); :600-611 (impl-method
  lifting); :625-627 (six-variant `ItemKind`).

### D7 / D8 -- Provenance enum on `Ref`; `Ty::Ptr` shape [LOCKED]

**Question.** What does the provenance information on pointer types
contain, and where does it live? Sub-questions: (a) which variants
appear in the alphabet (`SharedBorrow` / `UniqueBorrow` / `Raw` +
advisory bits?); (b) is the alphabet carried on `Ref` only, or also
on raw pointers; (c) does `Ty::Ref` redundantly carry both
`Mutability` and the new `Provenance` field?

**Key options.**

For the alphabet:
1. Three-variant baseline `{ SharedBorrow, UniqueBorrow, Raw }`,
   used uniformly on both `Ref` and raw pointers (Charon-style).
2. Three-variant baseline plus rustc's advisory bits (`frozen`,
   `unpin`, `global`).
3. Two-variant `{ SharedBorrow, UniqueBorrow }` carried on `Ref`
   only; `Raw` is encoded by the variant choice (raw pointer is a
   distinct `Ty` variant).

For the IR shape of `Ty::Ref`:
A. `Ref(TyId, Mutability, Provenance)` -- both fields, with an
   invariant linking them.
B. `Ref(TyId, Provenance)` -- drop `Mutability`; it is 1:1 redundant
   with `Provenance` for borrows.
C. `Ref(TyId, Mutability)` -- drop `Provenance`; rely on the
   `Mutability` enum.

For the `Ty::RawPtr` variant naming:
i.  Keep `Ty::RawPtr(TyId, Mutability)` -- mirrors rustc's
    `TyKind::RawPtr` 1:1.
ii. Rename to `Ty::Ptr(TyId, Mutability)` -- pairs with `Ty::Ref`
    symmetrically.

**Decision.** Three coupled changes:

- Alphabet: option **3**. `Provenance := { SharedBorrow,
  UniqueBorrow }`. Carried on `Ty::Ref` only. Raw provenance is
  implicit in the `Ty::Ptr` variant choice; advisory bits dropped.
- `Ty::Ref` shape: option **B**. `Ref(TyId, Provenance)`. The
  `Mutability` field is 1:1 redundant with `Provenance` for
  borrows and is dropped.
- Naming: option **ii**. `Ty::RawPtr` is renamed to `Ty::Ptr`.
  The rename is mechanical (~25 sites in our-IR text across
  `spec-snapshot.md`, `discussions.md`, `review-log.md`); foreign-
  quoted references to `TyKind::RawPtr` (rustc) and `RawPtr`
  (Charon) are unchanged.

The bridge translation rule is forced once the IR shape is locked:

```
TyKind::Ref(_region, ty, Mutability::Not) -> Ty::Ref(translate(ty), SharedBorrow)
TyKind::Ref(_region, ty, Mutability::Mut) -> Ty::Ref(translate(ty), UniqueBorrow)
TyKind::RawPtr(ty, mutbl)                 -> Ty::Ptr(translate(ty), mutbl)
```

The `_region` slot is **inspected at the parent level** before
being discarded. Post-erasure it is `ReErased` for free regions
(typical case) and `ReBound` for late-bound regions inside
`Binder<FnSig>` / `Binder<Predicate>` and `Dynamic` existential-
predicate scopes (see `findings-rustc-draft.md:675, 695-697`).
HRTB-bearing parents (`Ty::FnPtr` over `Binder<FnSig>` containing
`ReBound`, `Ty::Dynamic` over `Binder<Predicate>` containing
`ReBound`) are **rejected** at the parent level per the D7/D8
refinement (`spec-snapshot.md` Section 1.8) before any nested
`TyKind::Ref` translates; for non-HRTB parents every translated
`Ref` therefore has a free `ReErased` region, which is discarded.
No region survives in `Ty::Ref` for any type that enters the IR.

**Reasoning.**

Rust's mutability information is layered across three independent
axes:
1. *Binding mutability* (`let x` vs `let mut x`) -- property of the
   binding site; lives in MIR `LocalDecl`, not `Ty`.
2. *Reference mutability* (`&T` vs `&mut T`, or `*const T` vs
   `*mut T`) -- carried in the *type*. This is what
   `TyKind::Ref`'s third field and `TyKind::RawPtr`'s second field
   encode.
3. *Data mutability of the original owner* -- partially observable
   through `is_unsafe_cell` on `Item` (interior mutability).

The IR's `Provenance` and `Mutability` fields target axis 2
exclusively. For `Ref`, `Mutability::Not` and
`Provenance::SharedBorrow` are 1:1 synonyms on this axis; storing
both is redundant. We keep `Provenance` because its spelling
encodes the C-codegen-relevant connotation ("is this borrow
unique?") -- exactly the predicate that controls `restrict`
emission. This follows the methodology rule "store the result, not
the input."

For `Ty::Ptr`, axis-2 is the only signal that varies, and raw
pointers always have implicitly-`Raw` provenance with no
uniqueness contract. A `Provenance` field on `Ty::Ptr` would be
single-valued; including it is the kind of redundant flag the
methodology forbids.

The asymmetry between `Ref(_, Provenance)` and `Ptr(_, Mutability)`
is deliberate: `Ref`'s axis-2 is a uniqueness/aliasing contract;
`Ptr`'s axis-2 is a source-level `*const`/`*mut` marker with no
aliasing implication. Charon (option 1) collapses this distinction
to a uniform `RefKind` reused on both kinds; we diverge because
byte-exact C codegen wants the variant-level distinction (the
`restrict` rule fires for borrows only, and the rule is local on
the variant tag).

Advisory bits (option 2) are dropped because no v1 codegen consumer
materializes them: `frozen` would matter only if we added
`restrict` on shared `&T` (we don't); `unpin` matters only when
async/`Pin<P>` is in scope (rejected); `global` matters only for
Box (rejected). Per `rustc_abi/src/lib.rs:2134-2137`, "if the
information is absent, that must always be okay."

The rename `Ty::RawPtr -> Ty::Ptr` is purely cosmetic but pairs
the two variants by name (`Ref` / `Ptr`), which reads more
naturally given the size of the alphabet asymmetry between them.

**Cross-refs.** `spec-snapshot.md` Section 1.8 (full audit shape);
Section 1.1 variant table (revised rows for `Ref`, `Ptr`); Section
4.1 (worked example, replacement); Section 4.1.1 (bridge-translation
edge-cases table); Section 5 D7/D8 rows; `findings-rustc-draft.md`
Theme B.10 (PointerKind / PointeeInfo), Theme A.8 (`TyKind::Ref` /
`TyKind::RawPtr`, lines 191-196), Theme E.6 (post-erasure `Region`
for `Ref`, lines 695-697); `prior-art/charon.md` Theme A (RefKind
reused on both kinds); `prior-art/mrustc.md` (no per-`TypeRef`
flag for interior mutability); `prior-art/llvm-ir.md` (typeless
`ptr`); `prior-art/compcert.md` Theme A (`Tpointer` attributes).
For SESSION-B audit trail, `review-log.md` Round 3 (SESSION-B).

### D9 -- Volatile semantics: no `Ty` bit; volatile lives on the intrinsic `InstanceId` [LOCKED]

**Question.** Does our `Ty` carry a volatile bit (CompCert-style),
does a generic load/store `Statement` carry a `volatile: bool`
flag (CLR-style per-op), or is volatility purely a property of
specific intrinsic `InstanceId`s with no IR-level flag at any
level?

The framing extends to a meta-question: do we have a
QualType-style type-level qualifier wrapper at all? `const`,
`static`, and `restrict` were already handled by other locked
decisions but had not been articulated as a unified pattern.

**Key options.**
1. *Per-type* (CompCert-style `attr_volatile: bool` on `Ty`).
2. *Per-op flag on a generic load/store `Statement`* (CLR-style
   `LdInd { volatile: bool }`).
3. *Per-intrinsic-`InstanceId`* (no IR-level volatile flag at
   any level; volatility is a codegen-emission fact about
   specific MMIO intrinsic `InstanceId`s).

**Decision.** Option 3, plus articulation of the implicit "no
QualType layer" pattern that already governs `const`, `static`,
and `restrict`. The bridge rejects raw
`core::ptr::read_volatile` / `write_volatile` calls
(Call-terminator-level rejection in stage 9 / stage 14
enforcement); MMIO access flows through
`#[rseq::intrinsic("mmio_*")]` exclusively per the DSL
three-mechanism convention.

**Reasoning.**

The "no QualType layer" pattern: regseq-model's `Ty` represents
the unqualified Rust type; C-side qualifiers are emitted at
codegen time from per-decl / per-op information elsewhere in
the IR. Each qualifier already maps to a distinct site:

- `const` on `Ty::Ref` → `Provenance` on `Ty::Ref(TyId,
  Provenance)` (locked D7/D8 2-arg form) + transitive
  `is_unsafe_cell` flag on `Item` (D1.1 + D1.5 + D7/D8 LOCKED).
- `const` on `Ty::Ptr` → `Mutability` on `Ty::Ptr(TyId,
  Mutability)` only (`*const T` vs `*mut T`); the
  `is_unsafe_cell` flag does *not* apply to raw pointers per
  D1.5 (D1.1 + D7/D8 LOCKED).
- `static`    → `Item` decl (linkage / storage class; stage-12
  territory).
- `restrict`  → `Provenance::UniqueBorrow` on `Ty::Ref` (D1.1
  + D7/D8 LOCKED).
- `volatile`  → intrinsic `InstanceId` identity (D9, this
  entry).

Locating volatile on the intrinsic `InstanceId` follows the
same shape as the other three. Adopting Option 1 (CompCert
per-type) would be a one-off departure from the implicit
pattern. Option 2 puts a flag on a generic `Statement` that no
driver code in the closed DSL surface ever produces (raw
`*_volatile` is rejected; MMIO is intrinsic-only), so the flag
would be dead weight.

CompCert is the lone per-type prior because its trace semantics
must observe volatile operations as part of the verified
semantics; we emit C and have no trace-semantics requirement.
Among surveyed projects with explicit volatile evidence: LLVM,
mrustc, rustc_codegen_clr, and rustc-codegen-c all keep
volatile per-op or off the type entirely; MLIR's general
"special semantics off type" pattern is consistent; Charon's
Theme D does not specifically address volatile (its
no-volatile-marker-in-TyKind state is suggestive but not
load-bearing). Rustc itself routes volatile through intrinsics.

The DSL three-mechanism convention (`../regseq-background.md`
lines 14-27: Intrinsics operational / Properties declarative /
Traits integration) places MMIO unambiguously in intrinsic
territory. The pl011 fixture's `mmio.rs` (lines 35-71) confirms
intrinsic-only usage in practice. Q7 already locks the
closed-intrinsic-vocabulary mechanism. Option 3 is the fixed
point of those existing commitments.

The bridge-level rejection of raw
`core::ptr::{read,write}_volatile` is the analog of D1.4
Category A applied to function calls instead of types: rigid
Rust constructs that exist in `core` but lie outside the regseq
DSL surface are rejected with a clear diagnostic, not handled
defensively. The DSL surface rule plus "no defensive fallbacks"
jointly imply this posture.

**Cross-refs.** `spec-snapshot.md` Section 1.12 (no-QualType-layer
framing principle), Section 1.13 (D9 audit shape), Section 4.11
(MMIO worked example); `spec-snapshot.md` Section 1.6 (D1.5
`is_unsafe_cell` flag), Section 4.1 (reference / pointer worked
C lowerings), Section 4.5 (UnsafeCell aliasing example);
`../regseq-background.md` lines 14-27 (DSL three-mechanism
convention);
`tests/fixtures/pl011-full-spec/src/regseq/mmio.rs:35-71`
(pl011 intrinsic usage); `../stage-1/spec-snapshot.md` lines
869-899 (intrinsic call representation); `../architecture.md`
Q7 (intrinsic recognition mechanism);
`../stage-14/expectations.md` (MMIO-specific intrinsic
vocabulary entries and Call-translation rejection enforcement
per SESSION-C forward-expectation);
`prior-art/llvm-ir.md` Theme D (lines 311-336);
`prior-art/mrustc.md` Theme D (lines 516-518, 520-536);
`prior-art/charon.md` Theme D (lines 609 and 664);
`prior-art/rustc-codegen-clr.md` Theme D (lines 328-382);
`prior-art/rustc-codegen-c.md` Theme D (lines 224-242);
`prior-art/mlir.md` Theme D (lines 365-405);
`prior-art/compcert.md` Theme D (lines 415-434 covering
volatile);
`findings-rustc-draft.md` lines 570-572 (rustc's intrinsic-
level volatile);
`review-log.md` Round 3 (SESSION-C) audit trail.

### D10 -- Atomics: regseq-DSL `Atomic<T>` with intrinsic operations; no `Ty` bit [LOCKED]

**Question.** Three sub-questions, sharpened by the framing
principle established in D9:

1. *Substantive lock* -- does `Ty` carry an atomic bit? Are
   atomic types (`AtomicU32`, `AtomicBool`, ...) flagged on
   `Item` or carved out as a separate `Ty` variant?
2. *Ingest policy* -- analog of D9's bridge-side rejection of
   `core::ptr::{read,write}_volatile`. Should the bridge admit
   the `core::sync::atomic` operational API, reject it (forcing
   a DSL-defined `Atomic<T>`), or admit both?
3. *Memory ordering* -- does ordering need any stage-3
   representation, or is it entirely operation/InstanceId
   metadata (stage-14 territory)?

**Key options (Q2, the genuine choice).**

1. *Reject the `core::sync::atomic` operational API; provide
   regseq-DSL `Atomic<T>` with `#[rseq::intrinsic("atomic_*")]`
   operations* (D9-symmetric).
2. *Admit the `core::sync::atomic` operational API*; bridge
   recognises the rustc-internal `core::intrinsics::atomic_*`
   calls inside their MIR.
3. *Admit both*.

**Decision.** Option 1, locked with structural decomposition:
the bridge rejects (a) the `core::sync::atomic::Atomic*` type
family at type translation (`AtomicU8`...`AtomicI128` -- 128-bit
variants are cfg-gated on `target_has_atomic_load_store = "128"`
per `library/core/src/sync/atomic.rs:3884-3920`; rejection
scope is the family, not gated by the cfg --, `AtomicUsize`,
`AtomicIsize`, `AtomicBool`, `AtomicPtr<T>`, plus the unstable
generic alias `core::sync::atomic::Atomic<T>` per
`library/core/src/sync/atomic.rs:340-345`; the core alias is
distinct from the regseq-DSL `regseq::Atomic<T>` facade);
(b) calls to methods on those types and to the rustc-internal
`core::intrinsics::atomic_*` family at Call translation; with
(c) `core::sync::atomic::Ordering` (the value-level enum)
explicitly *not* in the rejection scope. The regseq DSL
provides `Atomic<T>` with `#[rseq::intrinsic("atomic_*")]`
operations as the canonical surface. The `core::sync::atomic`
module's free functions for memory barriers and compiler fences
(notably `fence` and `compiler_fence`) are not part of the
regseq Atomic construct (which is value-keyed; see
`docs/compiler-collateral/constructs/atomic-memory-access-and-ordering.md`
for the construct-level exclusion of barrier/fence elements
from Atomic) and are **rejected at the bridge in v1** at Call
translation, mirroring D9's `*_volatile` rejection and D10's
`Atomic*`-method rejection. Future admission via a barrier-
specific intrinsic or a construct-level decision remains open. Q1 resolves to ordinary
`Ty::Struct` (no `is_atomic` flag, no `Ty::Atomic<T>` variant)
-- the existing `is_unsafe_cell` flag (D1.5) on the inner
`UnsafeCell<T>` field handles `const`-dropping. Q3 resolves to
no stage-3 representation -- ordering survives in the IR at
the operation/InstanceId level.

**Reasoning.**

The substantive lock (Q1) is a direct application of the
"no QualType layer" pattern: atomic types have the same shape
as any other `UnsafeCell`-bearing struct, and the
`is_unsafe_cell` flag already produces the correct C lowering.

The ingest policy (Q2) was the genuine choice. Five
architectural strands favour Option 1:

- *Architectural symmetry with D9.* D9 rejected raw
  `core::ptr::{read,write}_volatile` and required
  `#[rseq::intrinsic("mmio_*")]`. Admitting `core::sync::atomic`
  would have given the DSL two different ingest patterns for
  what are structurally the same thing.
- *DSL surface coherence.* `regseq::Atomic<T>` instead of
  `core::sync::atomic::AtomicU32` is a one-import-line change.
- *DSL controls vocabulary instead of inheriting it.* Decouples
  the DSL surface from rustc evolution.
- *Single bridge recognition mechanism.* Q7's
  `#[rseq::intrinsic("NAME")]` attribute remains the only
  admission path.
- *Per-target codegen latitude is unaffected.* Linux's
  `_relaxed`/`_acquire`/`_release` plus unsuffixed
  fully-ordered (with AcqRel conservatively strengthened to
  unsuffixed since Linux has no AcqRel suffix); Zephyr's
  SeqCst-only public API needing GCC `__atomic_*` builtins for
  non-SeqCst -- both work the same way regardless.

Memory ordering (Q3) survives at the operation/InstanceId level
(either as an operand on the `Call` or via per-(op × ordering)
`InstanceId` identity); stage 14 picks the encoding shape.

The Linux/Zephyr C-side evidence further reinforces the
operation-level posture: both v1 codegen targets reject C11
`_Atomic` by convention. Linux uses
`atomic_t`/`atomic64_t`/`atomic_long_t`/`refcount_t`; Zephyr
uses `atomic_t` (long) with a SeqCst-only public API. Neither
target can or should consume a type-level atomic marker.

**Cross-refs.** `spec-snapshot.md` Section 1.12 (no-QualType-layer
framing), Section 1.13 (D9 -- the architecturally-symmetric
volatile decision), Section 1.14 (D10 audit shape), Section
4.12 (`Atomic<u32>` worked example with target-aware lowerings);
`spec-snapshot.md` Section 1.6 (D1.5 `is_unsafe_cell` flag);
`../regseq-background.md` lines 14-27;
`tests/fixtures/pl011-full-spec/src/regseq/mmio.rs:35-71`
(pl011 facade pattern); `../stage-1/spec-snapshot.md` lines
869-899; `../architecture.md` Q7;
`../stage-14/expectations.md` (atomic-specific intrinsic
vocabulary entries, ordering encoding choice, and D10's
bridge-side rejection enforcement per SESSION-C
forward-expectation);
`prior-art/llvm-ir.md` Theme D; `prior-art/mrustc.md` Theme D
(lines 520-536); `prior-art/charon.md` Theme D (lines 609 and
664); `prior-art/rustc-codegen-clr.md` Theme D (lines 328-382);
`prior-art/rustc-codegen-c.md` Theme D (lines 224-242);
`prior-art/mlir.md` Theme D; `prior-art/compcert.md` Theme D
(lines 436-451);
`findings-rustc-draft.md` Theme D;
external: Linux `Documentation/atomic_t.txt`,
`include/linux/atomic.h`; Zephyr `include/zephyr/sys/atomic.h`,
`include/zephyr/sys/atomic_builtin.h`,
`include/zephyr/sys/atomic_types.h`;
`review-log.md` Round 3 (SESSION-C) audit trail.

### D11 -- Layout content subset [LOCKED]

**Question.** Which fields of rustc's `LayoutData` does our
`Layout` mirror, and which do we drop?

**Discussion approach.** Decomposed into nine sub-questions:
load-bearing minimum; collapsing rustc's `FieldsShape` for our
3-variant Struct/Enum/Union split; source-vs-memory order; enum
`Variants` shape; tag-encoding fidelity; `largest_niche`;
`uninhabited`; universal drops; `repr` flags.

**Decision summary.** The full `Layout` schema is in `spec-snapshot.md`
Section 2.2. Three principles drove the per-field choices:

1. *Codegen-driven minimum.* Keep what codegen reads to emit
   correct C: `size`, `align`, per-field `(name, ty, offset)`,
   enum tag scalar + encoding + variants, `uninhabited` at both
   Layout and per-Variant level.
2. *Drop everything no consumer reads in v1.* `BackendRepr` (Rust
   ABI hint, not C ABI), `max_repr_align`, `unadjusted_abi_align`,
   `randomization_seed`, `largest_niche` (parent-only consumer;
   already baked into parent's `tag_encoding`).
3. *One repr escape: `pack: Option<u32>`.* `repr(packed)` is the
   only repr knob whose effect cannot be reproduced from
   `{size, align, offsets, tag.scalar_ty}` alone, because the C
   compiler re-introduces alignment padding unless
   `__attribute__((packed))` suppresses it. All other repr
   effects (`align(N)`, `transparent`, `repr(<int>)`, `repr(C)`
   vs `repr(Rust)` reordering) are observable. See Section 2.4
   for the full repr enumeration.

**Tag-encoding fidelity divergence.** We retain rustc's full
`Niche { untagged_variant, niche_variants, niche_start }` triple,
diverging from Charon (which keeps `untagged_variant` only).
Charon's verifier-IR posture lets it abstract over byte values;
we emit byte-exact C, so the formula `tag = (variant_idx -
niche_variants.start).wrapping_add(niche_start)` is load-bearing
for `SetDiscriminant` codegen on multi-niched enums.

**Variant representation.** We collapse rustc's recursive
`Variants::Multiple.variants: IndexVec<VariantIdx, LayoutData>`
to a flatter `Vec<VariantLayout>` carrying just field offsets +
per-variant `uninhabited`. Variant size/align is derivable from
`max(field.offset + sizeof(field.ty))`; the nested `LayoutData`
adds storage and serialization cost without codegen benefit.

**Reasoning.** Methodology rules drove every choice:

- "Don't redo work the rustc compiler has already done." Every
  field we keep is a *result* of rustc's layout algorithm, not an
  input.
- "Compositional behavior beats special-case flags." One `pack`
  field; everything else falls out of the resolved bytes.
- "Ask who consumes it." `largest_niche` has no v1 consumer
  because the parent's `tag_encoding` already encodes the chosen
  niche; the child's residual niche has no further use.
- "No defensive fallbacks." v1 codegen handles `pack: None` and
  `pack: Some(1)` natively; `pack: Some(N>1)` rejects with a
  clear diagnostic. The IR shape is full-fidelity so the consumer
  narrows, not the spec.

**Prior-art convergence.** `size`, `align`, per-field offsets are
universally stored across rustc_codegen_clr, mrustc, Charon (all
projects that do aggregate codegen themselves). Universal drops
(stored in *no* surveyed project): `BackendRepr`,
`randomization_seed`, `max_repr_align`, `unadjusted_abi_align`.

**Cross-refs.** `spec-snapshot.md` Section 2.2 (full schema +
mapping table); `spec-snapshot.md` Section 4.4 (niche example);
Section 4.8 (packed-struct example); `findings-rustc-draft.md`
Theme B (rustc `LayoutData`); `prior-art/rustc-codegen-clr.md`
Theme B (CLR's `ClassDef`); `prior-art/mrustc.md` Theme B
(`TypeRepr` + `VariantMode`); `prior-art/charon.md` Theme B
(Charon's `Layout` + `DiscriminantLayout`); `discussions.md`
Section 2.1 (Layout primer); Section 2.2 (responsibility split);
Section 2.4 (repr(X) enumeration).

### D12 -- Niche optimization preservation [LOCKED]

**Decision.** Niche encoding (e.g., `Option<&T>` -> one pointer)
is preserved formally via `TagEncoding::Niche { untagged_variant,
niche_variants, niche_start }` on `EnumShape::Multiple`. Locked
together with D11 (Section 2.2 of `spec-snapshot.md`).

**Reasoning.** Niche optimization is observable size-wise; if we
drop it, our C output bloats and `SetDiscriminant` cannot encode
the right tag bytes. Mirroring rustc's `TagEncoding::{Direct,
Niche{...}}` distinction with byte-exact fidelity is what enables
correct `Option<&T>`, `Result<T, !>`, and similar driver-side
patterns. (D11's full-fidelity-not-Charon-style choice is the
substantive lock; D12 is the formal corollary.)

**Cross-refs.** `spec-snapshot.md` Section 2.2 (TagEncoding shape),
Section 4.4 (worked example); `discussions.md` Section 2.1, 2.3.

### D13 -- Discriminant level: per-variant resolved tag value on `VariantLayout` [LOCKED]

**Question.** The shared D13 question -- "Preserve typeck-level
discriminant type (the `repr(<int>)` choice) AND layout-level
tag (the in-memory bytes), or only one?" with lean
"layout-level tag only -- the typeck-level discriminant is
recoverable from the layout's tag scalar type" -- conflates two
distinct things called "discriminant":

1. *Discriminant **type*** -- the `repr(<int>)` choice or
   rustc's default tag-size selection (e.g., `u32` for
   `repr(u32) enum E { ... }`).
2. *Per-variant discriminant **values*** -- the concrete tag
   bytes each variant maps to (e.g., `0`, `1`, `0xFF`,
   `0xDEAD` for a `repr(u32) enum Status { Idle = 0, Active = 1,
   Error = 0xFF, Reset = 0xDEAD }`).

Each needs a separate answer. The shared lean's "recoverable
from the layout's tag scalar type" claim only applies to (1).

**Key options for (2) -- per-variant discriminant values.**

1. *On `VariantLayout`* (two fields) -- add `discriminant: u128`
   (always present; semantic discriminant from
   `adt_def.discriminants(tcx)`) and `tag_value: Option<u128>`
   (layout-level tag bytes; `None` when no tag write occurs).
2. *On `EnumShape::Multiple` as parallel `Vec<u128>` and
   `Vec<Option<u128>>`*.
3. *On the enum's `Item` decl* (stage-12 territory).
4. *Recover from variant index + tag encoding* -- fails for
   user-assigned `repr(<int>)`.

**Decision.**

- (1) -- *Discriminant type*: settled by D11. The
  `TagScalar.scalar_ty: TyId` field on `EnumShape::Multiple`
  carries the resolved tag scalar (`Ty::Int(_)` / `Ty::Uint(_)`
  / `Ty::Ptr(_, _)` / `Ty::Ref(_, _)` per locked D7/D8). D11's
  schema-mapping table drops `ReprOptions.int` because the
  resolved scalar is authoritative. D13 carries this lock
  forward.
- (2) -- *Per-variant discriminant values*: Option 1.
  `VariantLayout` is refined with two fields:
  - `discriminant: u128` -- the Rust-level semantic
    discriminant (what `Discriminant(place)` returns; from
    `adt_def.discriminants(tcx)`'s `Discr.val: u128`). Always
    present for any `VariantLayout`.
  - `tag_value: Option<u128>` -- the layout-level tag bytes
    for `SetDiscriminant`:
    * `Some(v)` for direct-encoded variants (`v ==
      discriminant`).
    * `Some(v)` for niche-encoded niched variants (per D12's
      formula).
    * `None` for niche-encoded untagged variants (the inner
      data's bit pattern IS the tag).
    * `None` for `EnumShape::Single` variants (no tag scalar
      exists in the layout).

This refines D11's locked Section-2.2 schema by *two* fields.

**Reasoning.**

For `Direct` tag encoding with user-assigned discriminants
(common in driver code -- status codes, error codes, register
values), variant index and discriminant value are
*independent*. Codegen for `SetDiscriminant(Status::Error)` on
`repr(u32) enum Status { Idle = 0, Active = 1, Error = 0xFF,
Reset = 0xDEAD }` must write `0xFF` to the tag location, not
`2` (the variant index). Without per-variant tag-value storage,
codegen has no source for `0xFF` -- the value lives only in
rustc's typeck-level `adt_def.discriminants(tcx)`, which is
post-bridge unavailable. The shared D13 lean fails this case.

The Option 1 placement (on `VariantLayout`, two fields) wins on
five strands:

- *"Store the resolved result, not the input."* Per-variant
  discriminants are a resolved-by-rustc decision; we copy the
  answer.
- *Two senses, two fields.* The Rust-level semantic discriminant
  (always present per `adt_def.discriminants(tcx)`) and the
  layout-level tag bytes (absent for niche-untagged variants
  and Single-variant enums) diverge for niche-encoded enums;
  conflating them into a single field cannot honestly represent
  both.
- *Cohesion.* Discriminant data is intrinsic to the variant's
  layout (alongside its fields' offsets and the `uninhabited`
  flag).
- *Stage-3 / stage-12 boundary respected.* Per-variant
  discriminant data is layout-level (resolved tag bytes plus
  resolved Rust-level semantic discriminant, not AST-level
  user-source `= EXPR` clauses).
- *`u128` width* matches rustc's resolved `Discr.val: u128`.

Modest redundancy with D12 for niche-encoded enums:
`niche_variants`/`niche_start` (D12) tell codegen *where* to
write the tag and *how* the variant index maps to the bit
pattern; `tag_value: Some(v)` (D13) tells codegen *what value*
to write; `discriminant: u128` (D13) tells codegen *what
semantic discriminant to produce* in `Discriminant(place)`
lowering. Each is load-bearing for a different
`SetDiscriminant`/`Discriminant` codegen aspect.

**Cross-refs.** `spec-snapshot.md` Section 1.12 (no-QualType-layer
framing -- D13 is a layout-level extension orthogonal to S0
but consistent with its methodology), Section 2.2 (D11 -- the
schema D13 extends), Section 2.3 (D13 audit shape), Section 4.4
(existing niche-encoded `Option<&u32>` worked example), Section
4.13 (`repr(u32)` Status worked example with Direct encoding),
Section 4.14 (`Option<&u32>` niche-encoded with discriminant
fields);
`findings-rustc-draft.md` Theme B.4 (TagEncoding covers
Direct/Niche encoding shape, not per-variant values), Theme B.6
(Scalar / valid_range), Theme W.8 (typeck vs layout for
discriminant *type*);
`rustc_middle/src/ty/adt.rs:159` (`VariantDef.discr`,
typeck-level per-variant value);
`rustc_abi/src/lib.rs:1858-1880` (`Variants::Multiple` schema);
`prior-art/charon.md` Theme B lines 397-512 (Charon's
`VariantLayout.tag: Option<ScalarValue>` and
`Variant.discriminant`; direct precedent for Option 1);
`prior-art/mrustc.md` Theme B lines 295-319;
`prior-art/rustc-codegen-clr.md` Theme B lines 202-212 (CIL
offset-collision approach);
`review-log.md` Round 3 (SESSION-C) audit trail.

### D14 -- Renderer style [DEFERRED to stage 13]

**Disposition.** This decision was originally reserved at stage
3 as the renderer-style choice for `pretty.rs::render_ty`. It
was **deferred** to a dedicated stage 13 (type-system
pretty-printing) because the renderer's input depends on `Ty`
content references that only become fully populated at stage 11
(`FnSig`) and stage 12 (`Item` / DefPath). Locking the renderer
style at stage 3 would have produced a spec that changes shape
every time later stages fill in content.

The content below is preserved as **historical context and
input** for stage 13. Stage 13 mints a fresh `D-13-X` tag when
it makes the call -- this `D14` slot in the stage-3 namespace
remains DEFERRED indefinitely; no stage-3 decision occupies it.

**Question (preserved).** What format does
`pretty.rs::render_ty` produce?

**Key options (preserved).**
1. Rust-source-like (Charon: `&'r T`, `*const T`, `[T; N]`,
   `dyn Trait`, `for<'a> fn(...)`).
2. Constructor-style (mrustc MMIR, regseq-model-old `Debug`).
3. C-source-like (rustc_codegen_c declarator).

**Lean (preserved, not binding on stage 13).** Option 1.
Stage 1 chose "MIR-inspired pretty-printer"; matching the source
language at type level keeps the textual form readable for spec
authors.

**Cross-refs.** `../stage-13/expectations.md` (where the
deferred work now lives); `prior-art/charon.md` Theme G;
`stage-1/spec-snapshot.md` (pretty-printer scaffold).

### D15 -- fn-pointer ABI subset (`FnAbi` enum) [LOCKED]

**Question.** What ABI vocabulary does our IR carry on function
types? Sub-questions: (a) which variants of rustc's 28-element
`ExternAbi` are accepted; (b) is the `unwind: bool` parameter on
some rustc variants stored or rejected; (c) what is the shape of
the rejection diagnostic for unsupported ABIs; (d) where does the
enum live structurally.

**Key options.**

For the variant set:
1. Tight: `{ Rust, C }`. Reject everything else.
2. Wider: `{ Rust, C, System, Unadjusted }`. Reject the remaining
   24 ExternAbi variants.
3. Driver-portable: `{ Rust, C, System }`. Drop `Unadjusted` (rare
   in driver code).
4. Full mirror: all 28 ExternAbi variants.

For `unwind: bool` handling:
A. Store the flag: `FnAbi::C { unwind: bool }` etc.; codegen
   rejects `unwind: true`.
B. Flatten: `FnAbi::C` (no unwind component); bridge translates
   `C { unwind: false }` -> `FnAbi::C`, rejects `C { unwind: true }`.
C. Add separate variants: `FnAbi::C`, `FnAbi::CUnwind`.

For the rejection diagnostic:
α. Soft fallback to `FnAbi::C` with a warning.
β. Hard reject: `BridgeError::UnsupportedAbi { abi, span }`; refuse
   to translate the function.
γ. Reachability-aware reject: hard for body functions, soft for
   never-called extern declarations.

For the storage location:
I. Embedded in `FnSig`.
II. Separate `FnAbi` table indexed alongside `FnSig`.

**Decision.** Variant set option **1** (tight `{ Rust, C }`);
`unwind` handling option **B** (flatten, reject `unwind: true`);
diagnostic option **β** (hard reject); storage option **I**
(embedded in `FnSig`). The enum lives in the stage-3 type-system
vocabulary; stage 11 embeds `FnSig::abi: FnAbi` (forward note).

The bridge mapping table:

```
ExternAbi::Rust                       ->  FnAbi::Rust  (accept)
ExternAbi::C { unwind: false }        ->  FnAbi::C     (accept)
ExternAbi::C { unwind: true }         ->  reject (BridgeError::UnsupportedAbi)
[all non-Rust / non-C ExternAbi variants] ->  reject (BridgeError::UnsupportedAbi)
```

**Reasoning.**

The `FnAbi` field exists for two distinct reasons with different
stakes:

*Type-identity preservation (soundness).* Rustc treats
`fn() -> u32` and `extern "C" fn() -> u32` as **distinct types**
with no implicit coercion. If our `FnSig` omits the ABI, the
bridge maps two rustc-distinct types to one IR type, the interner
conflates them, and downstream MIR casts between the two become
vacuous. *This forces an `FnAbi` field on `FnSig`.* The variant
set must be at least as wide as the ABIs that appear in code we
accept.

*Codegen and downstream decisions.* Active and near-future
consumers: the pretty-printer (stage 13's `render_ty`, full enum); the future
FFI-validity linter (predicate "is this `FnAbi::C`?"). The
calling-convention attribute emitter (`__attribute__((stdcall))`
etc.) is a no-op for Linux x86-64 because all accepted ABIs
collapse to the platform default at the C level. **Symbol mangling is not a `FnAbi` consumer**: it
is governed by `Item.linkage` (the `Linkage` enum, see
`stage-1/spec-snapshot.md:77`) and `Instance.symbol_name` (default
`tcx.symbol_name`, see `stage-1/spec-snapshot.md:670-673`).
`extern "C" fn foo()` without `#[no_mangle]` still receives a
mangled symbol; the ABI does not control the mangled-vs-unmangled
choice.

The minimal variant set is therefore driven by the type-identity
argument: which ABIs appear in code we accept? For Linux v1
driver code, `Rust` (default) and `C` (FFI to kernel APIs) cover
the realistic surface. `System` resolves to `C` on Linux,
`Unadjusted` is compiler-internal, `RustCall` is closure-trait
territory (D6). Picking the tight set over the wider lean
({Rust, C, System, Unadjusted}) is the methodology-cleanest move:
"expand vocabulary lazily," "reject what shouldn't appear; never
speculate-handle." Adding `System`/`Unadjusted` later is a one-
line enum extension plus a matching bridge entry.

`unwind: true` (rustc's `extern "C-unwind"` variant) is rejected
because it represents a runtime capability drivers don't have:
Linux/Windows kernels run abort-on-panic semantics; cross-FFI
unwinding is meaningless. Mapping `C { unwind: true } -> FnAbi::C`
would silently strip type-system information -- a defensive
fallback. Hard rejection (option β) surfaces the problem at the
bridge with the source span; if a future use case for cross-FFI
unwinding materializes, lift the enum to include a `CUnwind`
variant or parameterize `C { unwind: bool }`.

Reachability-aware rejection (option γ) is appealing pragmatically
(don't fail the whole crate over an unused extern decl) but adds
reachability analysis the bridge doesn't otherwise need at this
stage. Defer to a stage-11 refinement if the v1 ergonomics
demand it.

Charon's choice to drop the ABI entirely (`prior-art/charon.md`
line 1143-1148: "Calling conventions, including extern \"C\",
must be recovered (or are lost)") is incompatible with our type-
identity posture. Charon's verifier-IR can abstract over calling
conventions; we cannot, because rustc-distinct types must remain
distinct in our model. mrustc's choice to keep the ABI as a
string (`m_abi: RcString`, included in type-equality at
`prior-art/mrustc.md:430`) confirms the soundness-driven design
is the conventional choice for direct C-targeting Rust compilers.

**Cross-refs.** `spec-snapshot.md` Section 1.9 (full audit shape);
Section 5 D15 row; `findings-rustc-draft.md` Theme A.9 (full
ExternAbi enumeration, lines 198-230); `prior-art/charon.md`
Theme F (FunSig drops ABI); `prior-art/mrustc.md` (m_abi as
string, included in type equality); `prior-art/llvm-ir.md`
(calling convention attribute, not part of FunctionType identity);
`stage-1/spec-snapshot.md` line 76 (`Abi` field reservation on
`FnSig`, deferred to stage 11); `review-log.md` Round 3
(SESSION-B audit trail).

### D16 -- `c_void` recognition: rejected at bridge in v1 [LOCKED]

**Question.** The shared D16 lean ("ordinary Adt; no flag for
v1; recognition deferred") addressed the *flagging* question
correctly (already locked by D1.5) but did not address the
bridge's translation behaviour when it encounters `c_void` in
driver code today. The substantive sub-question: in v1, does
the bridge (a) translate `c_void` as an ordinary `Ty::Enum`
(with the pinned-toolchain shape: 2-variant `repr(u8)` per
`library/core/src/ffi/mod.rs:49-76`, where the two hidden
variants `__variant1` / `__variant2` make the type inhabited
rather than zero-variant uninhabited) and let downstream
codegen handle the structurally-faithful but FFI-wrong
lowering, or (b) reject `c_void` at the bridge until a consumer
materialises and the flag is added?

**Key options.**

1. *α -- Translate as ordinary `Ty::Enum`* (the pinned-toolchain
   2-variant `repr(u8)` shape; no bridge rejection; codegen
   produces a structurally-faithful 1-byte struct that does not
   match the FFI-canonical `void *` shape).
2. *β -- Reject `c_void` at the bridge in v1* (D1.4-style;
   diagnostic recommends opaque-struct pattern; no `is_c_void`
   flag added until D16 reopens).
3. *γ -- Translate as ordinary, plus a stage-14 forward-ref*
   (α with extra paperwork).

**Decision.** Option β. The bridge rejects
`core::ffi::c_void` at type translation in v1, with a
diagnostic recommending the opaque-struct pattern. D1.5's "no
flag" lock carries forward unchanged; D16 hardens the deferral
into active rejection pending a future consumer.

The recognition mechanism is `tcx.is_lang_item(did,
LangItem::CVoid)` per D1.5's locked recognition-path note.
Rejection scope is the c_void type itself; pointers to c_void
(`*mut c_void`, `*const c_void`) reject transitively when the
inner type is encountered during pointer translation. Trigger
conditions for re-opening: a v1 driver fixture or stage-14
intrinsic surfaces a stable need for FFI-canonical
pointer-to-c_void lowering (`*mut c_void` -> C `void *` and/or
`*const c_void` -> C `const void *`, the two pointer forms
having distinct C spellings per locked D7/D8's `Ty::Ptr(TyId,
Mutability)` and Section 1.12's raw-pointer
`const`-derivation rule). When that materialises, D16 reopens
to add an `is_c_void` flag on `Item` and lift the rejection.

**Reasoning.**

Three architectural strands carry the decision:

1. *No v1 consumer.* The evidence sweep confirms zero v1 driver
   fixtures use `core::ffi::c_void`, and zero planned stage-14
   intrinsic signatures mention it. Per "ask who consumes it;
   what shape best serves them," there is no v1 consumer to
   serve.

2. *Symmetric posture with D9 / D10 / D1.4.* All three
   already-locked rejections share the same structure: a
   stable `core::*` API or feature is rejected at the bridge
   because it lies outside the regseq DSL's operational
   surface, with a diagnostic pointing to the canonical
   alternative. D16 fits the pattern; the diagnostic recommends
   the opaque-struct pattern (the same alternative D1.4 names
   for opaque C-side handles -- there is no D9/D10-style
   DSL-defined replacement because c_void's role is
   structural, not operational).

3. *The non-rejection alternative does not earn its weight.*
   Translating `c_void` as an ordinary `Ty::Enum` -- per the
   pinned-toolchain shape: 2-variant `repr(u8)` enum
   (`__variant1`, `__variant2`,
   `library/core/src/ffi/mod.rs:49-76`) with
   `EnumShape::Multiple { tag: u8, encoding: Direct, variants:
   [...] }` -- produces a structurally-faithful 1-byte tagged
   struct in C that is *not* the FFI-canonical `void *` shape
   c_void's lang-item identity entails. Stage-13 codegen would
   either commit to the FFI-wrong 1-byte struct lowering or
   perform lang-item recognition the bridge is not currently
   doing. Rejecting in v1 avoids both lowering commitments.

The decision hardens D1.5's "recognition deferred until
intrinsic signatures using `*mut c_void` motivate it" into
"actively rejected pending recognition" -- the only honest
interpretation given that an unrecognised `c_void` cannot be
lowered correctly without a recognition-driven C-lowering
decision.

**Cross-refs.** `spec-snapshot.md` Section 1.5 (D1.4 -- the
`Foreign(_)` rejection precedent that D16 mirrors), Section 1.6
(D1.5 -- the "no flag for v1; recognition deferred" locked
phrasing that D16 builds on; the
`tcx.is_lang_item(LangItem::CVoid)` recognition path is locked
there at lines 402-406), Section 1.7 (D1.6.c -- the `Never` vs
c_void distinction that forwarded the substantive lock to
D16), Section 1.15 (D16 audit shape), Section 4.15 (rejected
`*mut c_void` worked example);
`findings-rustc-draft.md` Theme D.2 lines 546-564 (c_void
declared as Target::Enum at `rustc_hir/src/lang_items.rs:279`;
recognition via `tcx.is_lang_item(did, LangItem::CVoid)`; not
in `AdtFlags`);
external Rust source: `library/core/src/ffi/mod.rs:49-76` (the
c_void declaration -- `#[lang = "c_void"]` attribute, with two
hidden `__variant1` / `__variant2` variants);
`prior-art/charon.md` Theme D lines 609 and 664-678 (Charon
catalogues marker types treated as ordinary ADTs without
type-level markers; c_void is *not* specifically named, so the
claim that Charon admits c_void the same way is *inference*
from Charon's general `TyKind::Adt(TypeDeclRef)` model with
limited builtins, not direct evidence -- the alpha-style
admittance posture, which D16 declines for the v1 codegen
reasons above);
`../stage-14/expectations.md` (D16 type-translation rejection
of `core::ffi::c_void` per SESSION-C forward-expectation);
`review-log.md` Round 3 (SESSION-C) audit trail.

---

## 2. Reference primer (concise)

Foundational concepts that grounded the discussions. Kept tight so
new agents can build the right mental model without reading the
full conversation.

### 2.1 Layout and LayoutData -- the universal compiler concept

A type's **layout** answers: (1) size, (2) alignment, (3) field
offsets for aggregates, (4) padding, (5) discriminant encoding for
sum types, (6) niche values (invalid bit patterns reusable for
parent-tag encoding). These are *target-dependent* (size of
`usize`, alignment rules) and *post-monomorphization* (you cannot
compute layout for `Vec<T>` without `T`).

Every typed-language compiler computes these. C compilers do
struct field offsets + padding + alignment. Itanium C++ ABI, Swift,
Java, Go all have analogues. CompCert's `composite` records,
LLVM's `DataLayout`, MLIR's `DataLayout` interface, mrustc's
`Target_GetTypeRepr` -- all the same conceptual answer to the same
universal question.

Compilers compute layout by structural recursion: primitives
hardcoded; struct = walk fields, align/place/advance, round up to
struct alignment; union = all fields at offset 0; array = stride =
round-up(elem-size, elem-align); enum = compute variant layouts +
choose tag encoding (Direct or Niche).

`#[repr(...)]` knobs constrain the algorithm: `repr(C)` follows C
struct rules, `repr(Rust)` allows reordering for compaction,
`repr(packed)` removes padding, `repr(transparent)` = single-field
inline, `repr(<int>)` forces discriminant int type.

Rustc's `LayoutData` (`findings-rustc-draft.md` Theme B) bundles:
`size`, `align`, `fields` (FieldsShape: Primitive/Array/Union/
Arbitrary-with-offsets), `variants` (Empty/Single/Multiple-with-
tag), `largest_niche`, `uninhabited`, plus ABI/codegen hints
(`backend_repr`, `max_repr_align`, etc.). Niche encoding
(`TagEncoding::Niche`) is rustc's specific brilliance: discriminant
encoded *within* a field's invalid bit patterns, eliminating the
explicit tag (so `Option<&T>` is one pointer, not a 16-byte
struct).

### 2.2 Layout responsibility split: C compiler vs our IR

**Naive transpilation works for** `repr(C)` types, `repr(transparent)`
newtypes around scalars, primitives, fixed-stride arrays, tuples
of equally-aligned fields, `repr(C)` enums with explicit `repr(<int>)`.
For these, the C compiler's layout matches rustc's by definition.

**Naive transpilation breaks for** `repr(Rust)` reordering (rustc
may reorder fields for compaction; naive C lays them out in source
order, producing different sizes); niche-optimized enums (no C-
side equivalent without explicit code); tagged-enum tag-size
choice (rustc picks based on variant count, may not match C's
default); ZST handling (Rust = 0 bytes; C = at least 1 byte for
declared field); `repr(packed)` / `repr(align(N))` (require C-side
attributes); compiler-synthesized types (closures, fat pointers,
niche-encoded enums).

**Conclusion** (locked as D2): explicit-layout-in-IR. Carry layout
on `Ty::Struct/Enum/Union/Tuple` as a *resolved-result copy* of
`tcx.layout_of(ty)`. Tuples reuse `LayoutKind::Struct` with
synthetic field names (no Item; layout lives inline). We do not recompute; rustc has already done
the work. Codegen consumes layout to: (a) verify naive C struct
emission via `static_assert(sizeof(S) == N)`; (b) drive
niche-encoded enum codegen; (c) insert padding when `repr(Rust)`
reordering is detected; (d) emit `__attribute__((packed/aligned))`
when needed.

### 2.3 MIR keeps niche encoding abstract; codegen lowers

`optimized_mir` operates at the *semantic* level on enum values.
Four abstract operations carry through to codegen:
- `Rvalue::Discriminant(Place)` -- "read THE discriminant of this
  place"; returns variant index, doesn't say *where* to read.
- `StatementKind::SetDiscriminant { place, variant_index }` --
  "set this place's variant"; doesn't say *what bytes to write*.
- `PlaceElem::Downcast(_, variant_index)` -- a no-op narrowing of
  a place's type to a specific variant.
- `TerminatorKind::SwitchInt(value, targets)` -- standard branch.

None of these mention bytes, offsets, niches, or tag widths. The
niche-aware byte-level lowering is a *codegen-time* transformation
using the stored `Layout`.

For us this means: **the bridge stores the Layout (per D2);
codegen does the niche-aware abstract-op-to-byte-op translation.**
Bridge work for Bodies translates MIR statements to our IR
as-is (keeping the abstract ops); the Layout we attached at type
construction is the bridge between abstract op and byte-level
codegen.

This is the right division because:
1. MIR stays semantic (readable, optimizable, closer to source
   intent).
2. Multiple targets can apply target-specific layout via the same
   stored Layout result (rustc has done the layout analysis once).
3. We do not redo work the Rust compiler has already done.

### 2.4 Rust `repr(X)` enumeration -- which kinds need IR-level info

Rust has roughly seven `repr(X)` kinds (`findings-rustc-draft.md`
Theme B.9, citing `rustc_abi/src/lib.rs:159-216`). The D11
question "should `Layout` carry `repr` flags?" reduces to one
sub-question per kind: *given the resolved `{size, align, field
offsets, tag.scalar_ty}`, can codegen emit matching C without
extra information?*

| repr | Effect on layout | Derivable from `{size, align, offsets, tag.scalar_ty}`? | Why |
| --- | --- | --- | --- |
| `repr(Rust)` (default) | Field reordering for compaction | **Yes** | Offsets non-monotonic vs source order reveals reorder; codegen emits in memory order (sort by offset) with explicit padding. |
| `repr(C)` | No reordering, sequential placement | **Yes** | Offsets monotonic in source order; codegen emits in source order; natural C layout matches. |
| `repr(transparent)` | Single non-ZST field; outer layout = inner | **Yes** | Single non-ZST field at offset 0 with parent's size/align; emit as single-field struct. |
| `repr(<int>)` (enum tag) | Forces tag integer type | **Yes** | Visible in `tag.scalar_ty`. |
| `repr(align(N))` | Forces struct align ≥ N | **Yes-ish** | `Layout.align` reflects N; codegen emits `__attribute__((aligned(layout.align)))` defensively, or compares to `max(field.ty.align)` and emits only on mismatch. Either is correct without IR-level repr info. |
| `repr(packed)` / `packed(N)` | Removes/reduces alignment padding between fields | **No (reliably)** | Detectable only via heuristics ("any field offset not a multiple of `field.ty.align`" OR "layout.align < max field align"); both fail edge cases like `#[repr(packed)] struct S { a: u32 }` (offset 0, looks unpacked). C compiler will re-insert padding without `__attribute__((packed))`. |
| `repr(simd)` / `repr(linear)` / `repr(scalable)` | SIMD / SVE | N/A -- driver DSL out of scope | Reject at bridge. |

**Why `packed` is uniquely non-derivable.** For every other repr,
the resolved bytes (size, align, field offsets) tell the full
story; reproducing those bytes preserves semantics regardless of
source-side repr. Packed is structurally different: even if codegen
declares each field at its rustc-computed offset, the C compiler
is allowed to (and does) re-add alignment padding around fields,
breaking offsets. `__attribute__((packed))` is the only directive
that suppresses that.

**Why `align(N)` does *not* need a flag.** It raises the struct's
alignment. If `layout.align > max(field.ty.align)`, `repr(align(N))`
was applied -- and `layout.align` *is* the N. Codegen either
computes the comparison or always emits aligned defensively. No
information loss.

**Decision (D11).** `Layout` carries one repr field:
`pack: Option<u32>`. Mirrors rustc's `pack: Option<Align>` shape
(byte count rather than rustc's compact `bytes_pow2: u8` encoding).
`Some(1)` is plain `repr(packed)`; `Some(N>1)` is
`repr(packed(N))`. v1 codegen handles `None` and `Some(1)`;
`Some(N>1)` rejects with diagnostic. The IR shape supports the
full set so future expansion does not require a re-spec.

`repr(simd)` / `repr(linear)` / `repr(scalable)` are out of v1
scope; the bridge rejects them at type construction (consistent
with D1.4 rejection-list discipline).

### 2.5 The TyInterner pattern -- BiMap inside the model

Stage-1 established the BiMap-in-model precedent at
`stage-1/spec-snapshot.md:250-254` for `StringInterner`. Stage 3
locks the symmetric structure for types as `TyInterner`, sitting
in `RegseqModel.types` (refining the stage-1
`IndexVec<TyId, Ty>` placeholder at `stage-1/spec-snapshot.md:1002`).

**The shape.**
```rust
// regseq-model crate -- zero rustc deps
pub struct TyInterner {
    storage: IndexVec<TyId, Ty>,   // canonical reverse: TyId -> Ty
    lookup:  HashMap<Ty, TyId>,    // structural forward: Ty -> TyId (dedup)
}

impl TyInterner {
    pub fn new() -> Self;
    pub fn intern(&mut self, ty: Ty) -> TyId;        // dedup-on-insert
    pub fn resolve(&self, id: TyId) -> &Ty;
    pub fn find(&self, ty: &Ty) -> Option<TyId>;     // structural-identity lookup; non-mutating
    pub fn len(&self) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = (TyId, &Ty)>;
}
```

`Ty` impls `Hash + Eq + Clone` with a custom `Hash + Eq` rule
matching the D3 nominal/structural identity split: ADT-shaped
variants (Struct/Enum/Union/FnDef/Closure) hash and compare by
`(variant_kind_tag, item, args)` -- the variant tag is
load-bearing so `Ty::Closure(item, args)` and the parallel
`Ty::Struct{item, args, layout}` for the same closure remain
distinct entries -- with Layout / capture-struct content
*excluded*. Non-ADT variants (Bool, Int, Ref, Array, etc.) use a
structural derive over all fields, except `Ty::Tuple` which
excludes its inline `Layout` from `Hash + Eq` for the same
"functionally determined by fields" reasoning as ADTs. Nested `TyId`-typed
fields in non-ADT variants compare by integer ID (sound within a
build because the BiMap's dedup ensures structurally-equal Tys
have equal TyIds; F4-2 ArgsKey uses a *separate* deep
canonicalizer per D3 reasoning constraint 1).

**Symmetry with `StringInterner`.**

| Property | `StringInterner` | `TyInterner` |
| --- | --- | --- |
| Storage half | `Vec<Arc<str>>` | `IndexVec<TyId, Ty>` |
| Lookup half | `HashMap<Arc<str>, StringId>` | `HashMap<Ty, TyId>` |
| Mutability rule | `intern` requires `&mut self` | same |
| Append-only invariant | yes | yes |
| Custom serde | wire = `storage` only; `lookup` rebuilt at deserialize | same |
| Sentinel pre-intern | `StringId(0) == ""` | (none -- no zero-Ty meaning) |

**Two layers of dedup, one authoritative.** The bridge holds an
*additional* short-circuit cache that is not part of the IR:

```text
regseq-model (no rustc deps)
    RegseqModel { types: TyInterner, strings: StringInterner, ... }

regseq-driver (rustc_private = true)
    BridgeBuildCtx<'tcx> {
        model:    &mut RegseqModel,
        ty_cache: HashMap<rustc::Ty<'tcx>, TyId>,   // bridge-local
    }

    // Generic path -- non-ADT variants only. ADT-shaped variants
    // (Struct/Enum/Union/FnDef/Closure) take the reservation path
    // shown in spec-snapshot.md Section 3.2 (translate_adt) per D3
    // invariant 3.
    fn translate_ty(&mut self, rustc_ty: rustc::Ty<'tcx>) -> TyId {
        if let Some(&id) = self.ty_cache.get(&rustc_ty) { return id; }
        if is_adt_shaped(rustc_ty) {
            return self.translate_adt(rustc_ty);
        }
        let our_ty = build_non_adt_ty_from_rustc(rustc_ty, self);   // recurses
        let id = self.model.types.intern(our_ty);
        self.ty_cache.insert(rustc_ty, id);
        id
    }
```

- `ty_cache` short-circuits re-translation when the same
  `rustc::Ty<'tcx>` appears twice (which happens often -- rustc
  itself structurally interns `Ty<'tcx>`, so pointer identity is
  preserved within a session). Bridge-local. Q1 forces it
  bridge-side: regseq-model cannot mention `rustc::Ty<'tcx>`. Q2
  forces it to die with the bridge call.
- `model.types.lookup` is the *authoritative* dedup. Synthesised
  types skip `ty_cache` and call the appropriate intern path
  directly: ADT-shaped synthesised types (vtable type structs
  per D5 and closures-as-structs per D6) go through
  `translate_adt`'s reservation flow with a synthesised
  `ItemId` and become
  `Ty::Struct(synthesised_ItemId, GenericArgs, layout)` /
  `Ty::Closure(synthesised_ItemId, GenericArgs)` respectively.
  Trait-object fat pointers themselves are *not* interned per
  D5.3-B (`Ty::Dynamic` survives in the IR and codegen emits
  the fat-pointer C typedef inline). Structural `lookup`
  collapses duplicates the same way regardless of path. (Drop-
  glue is a function decl, not a `Ty`; it does not flow through
  TyInterner.)

**Determinism mechanism.** F4-2 `ArgsKey` compares type args
through a **deep canonicalizer** distinct from the BiMap's
in-build `Hash + Eq`, defined in D3 reasoning constraint #1
(`spec-snapshot.md` Section 3.1). The canonicalizer walks `Ty`
content recursively, with per-IR-identifier termination rules:
`TyId`s resolve via `model.types.resolve(...)`; `ItemId`s in
ADT-shaped variants terminate at the cross-build-stable
`DefPathHash` from `model.item_def_paths`; `FnSigId`s expand to
their referenced `FnSig` contents (then recurse on inner Tys);
`StringId`s resolve to string content; `ConstId`s use the
canonical const-eval result (stage-4 rule). The canonicalizer
operates on `Ty` content, **not** on `TyId` integers.

Within a single bridge run, structurally-identical `Ty` values
intern to the same `TyId` (the BiMap's content-addressing
property). The canonicalizer can therefore short-circuit on TyId
equality as a fast path *within a single build*, but the
authoritative cross-build comparator is the deep walk.

`InstanceId` ordering is reproducible across builds because
structural `Ty` content is reproducible (rustc's monomorphized
types are deterministic per source). `TyId` integer values are
*content-addressed within a build* but not guaranteed identical
across builds -- a bridge that walks instances in a different
order may assign different `TyId` integers, yet the resulting
RegseqModel is content-equivalent. Snapshot tests rely on the
pretty-printer's content-based rendering, not on `TyId` integer
stability.

For ADT-shaped types, the nominal-identity rule additionally
ensures interning terminates on self-referential types: the
parent's `TyId` is reserved before its layout is computed, and
recursive references hit the reservation via the nominal hash.

**Borrow-checker boundary.** Post-bridge, consumers hold
`&RegseqModel`. `intern` requires `&mut self`, so it is rejected
at compile time. `resolve(id) -> &Ty` and `iter()` require only
shared access. No separate "frozen" type or runtime check is
needed; the borrow checker is the boundary.

**What the IR exposes vs what the bridge keeps.** When
`bridge::extract(tcx)` returns `RegseqModel`, only `TyInterner`
crosses the boundary; `ty_cache` is consumed by drop. The IR
exposes both directions through public methods: `resolve(id)`
returns `&Ty` for the forward direction, and `find(&self, ty: &Ty)
-> Option<TyId>` returns the structural-identity reverse lookup
without inserting (used by post-build invariant checks and the
F4-2 ArgsKey dedup-cache). The `lookup` field itself remains
private; the public `find` is the consumer-facing surface.

### 2.6 Region erasure as a RegseqModel-wide invariant

A previously-implicit principle, lifted to a named methodology
rule citable from any stage.

**Statement.** *No `Ty` variant, no IR aggregate, and no Item
field carries any region (lifetime) slot. Regions are typeck and
borrowck artifacts in rustc; the program reaching the bridge has
already been type-checked and borrow-checked; C has no region
analogue; therefore RegseqModel preserves no region material.*

**Where this principle was already implicit.**

- *D1.1 reasoning* (Section 1, D1.1 entry): "by the time the
  bridge consumes a fully folded type, only rigid variants
  remain" -- the fold runs `erase_and_anonymize_regions`, which
  collapses all *free* regions.
- *D7/D8 / `Ty::Ref` shape* (`spec-snapshot.md` Section 1.1
  variant table for the `Ref` row, Section 1.8 for the D7/D8
  decision): `Ref(TyId, Provenance)` -- explicitly no region
  slot, and `Mutability` dropped in favour of the
  redundant-with-Provenance encoding.
- *Stage-1 `GenericArg` enum* (`stage-1/spec-snapshot.md:655-659`):
  variants are `Type(TyId)` and `Const(ConstId)` only; the
  comment explicitly notes "Lifetimes: erased post-monomorphization,
  not represented."

The principle generalises beyond these specific decisions: it
applies to every IR aggregate that might otherwise carry a
region. Stage 11 (when it expands the `FnSig` shape) and stage 12
(when it expands `Item`) inherit the invariant; the merge session
should ensure those stages' specs do not introduce region slots.

**What this principle pins.**

- *Free regions* (`ReFree`, becoming `ReErased` after rustc's
  pass) leave no IR trace. The bridge's mandatory fold via
  `erase_and_anonymize_regions` removes them uniformly across
  all `Ty` variants.
- *Bound regions* (HRTB binders, e.g. `for<'a>` in `dyn for<'a>
  Trait<&'a T>`) **survive** rustc's region erasure as `ReBound`
  per `findings-rustc-draft.md` W.13 (rustc retains them because
  borrowck/miri/codegen need them). v1 policy: the bridge
  **rejects** HRTB-bearing `Ty::FnPtr` and `Ty::Dynamic` at
  type translation per the D7/D8 refinement
  (`spec-snapshot.md` Section 1.8). The same type-identity argument
  D15 uses for `FnAbi` applies to binders -- erasing them would
  collapse rustc-distinct types.
- *No erasure-via-silence for HRTBs.* `Ty::Dynamic` had been a
  candidate for an HRTB binder slot; D5 closes that question by
  rejecting HRTB-bearing dyn types instead of erasing the
  binder. Future stages adding new `Ty` variants must respect
  the same constraint or motivate an exception (and document
  the type-identity consequences).
- *No exception for `Item` fields, function signatures, or trait
  references.* The "free regions erase, HRTB rejects" rule is
  RegseqModel-wide, not per-variant.

**Why.**

1. *Type correctness was already verified.* Rustc verifies HRTB
   obligations and lifetime constraints during type-check and
   borrow-check, both pre-MIR. The IR consuming `optimized_mir`
   gets a typecheck-passing program; we don't re-verify.
2. *C has no region analogue.* No qualifier, no syntactic form,
   no RTTI in C corresponds to a Rust region. Carrying regions
   would force codegen to either ignore them (defensive
   redundancy) or re-derive constraints we've already discarded.
3. *Determinism is easier without regions.* Region representations
   in rustc are non-trivial (free vs bound, ReBound tracking,
   the `RegionKind` taxonomy). Excluding them keeps the F4-2
   deep canonicalizer simpler and reduces the bridge's
   normalisation surface.

**Methodology rules invoked.**

- "Don't redo work the rustc compiler has already done."
- "C semantics with information preservation" -- regions are not
  C-semantic information; preserving them would add
  non-C-relevant content.
- "No defensive fallbacks" -- no region-rejection guards "in
  case rustc forgets to erase one"; the bridge fold is
  authoritative.

**Cross-refs.**

- Section 1 D1.1 (variant-scope reasoning that cites region
  erasure for the generic-form fold);
  `spec-snapshot.md` Section 1.1 (`Ty::Ref` row);
  `stage-1/spec-snapshot.md`:655-659 (`GenericArg` no-lifetime
  comment).
- `findings-rustc-draft.md` Theme E.4
  (`erase_and_anonymize_regions`); E.6 (`ReBound` preserved by
  that fold -- v1 RegseqModel rejects HRTB-bearing `FnPtr` /
  `Dynamic` rather than erasing further; only free-region
  `ReErased` cases enter the IR); W.13 (free vs bound region
  distinction in rustc).

---

## 3. Cross-references

Per-decision spec section: `spec-snapshot.md` Sections 1.x, 2.x,
3.x, 4.x.

Per-round Codex review: `review-log.md`.

Inbound forward refs from stage 1: `expectations.md`.

Primary-source rustc evidence: `findings-rustc-draft.md`.

Per-project deep prior-art research: `prior-art/<project>.md`
(7 files).

Architectural framing: `../architecture.md` (Q1-Q7).

Top-level methodology rules: `../README.md` "Methodology rules".


---

## Stage 4 (constants and literals)

# Stage 4 -- Discussion Record

Distilled record of the design dialogue producing the stage-4
const-vocabulary spec. Each entry captures the question raised,
key options considered, the decision (or current lean for open
decisions), the reasoning chain, and cross-references to
`spec-snapshot.md` (the spec) and `review-log.md` (the audit
trail).

Status of decisions in this file mirrors `spec-snapshot.md` §9.7
and adjacent sections, plus `status.md` "Open decisions" /
"Locked decisions" tables.

D-tag convention: prefixed `D-4-K` form per `../README.md` §5.2
(stage 4 starts day-one with the new convention; stages 1 and 3
use legacy un-prefixed tags).

Status tags:
- `[DRAFT]` -- audit-shape recorded; pending Codex review.
- `[LOCKED]` -- closed at APPROVE on a Codex review round.
- `[REOPENED]` -- a later sub-decision or sibling D-tag forced a
  revision; superseded entry kept for history.
- `[DEFERRED to stage N]` -- moved to a later stage's scope.

---

## 1. Decision discussions

### D-4-1 -- `Const` enum variant set [LOCKED]

**Question.** What is the closed list of `Const` variants that
the bridge fills from rustc-evaluated const values, given the
post-monomorphisation guarantee and the F4-2 cross-build
fingerprint requirement?

**Key options.**

1. **Option A -- flat enum, per-flavour variants.** One variant
   per value flavour (scalars per type, ZST, aggregate, enum-value,
   relocation, null, uninit-bearing bytes). Variant tag IS the
   equality class. Each variant carries the type info it needs
   internally (no shared `Const { ty, kind }` wrapper).
2. **Option B -- rustc-mirror.** Mirror `mir::ConstValue`'s
   `Scalar / ZeroSized / Slice / Indirect` shape. Minimum bridge
   translation effort.
3. **Option C -- two-tier physical split.** Separate
   `TypeLevelConst` and `ValueLevelConst` types interned in two
   tables, structurally enforcing the type-level vs value-level
   phase split.

**Decision.** Option A, refined to **11 variants** (the parent
shape; per-variant payload pinned in sub-decisions D-4-1.1
through D-4-1.9):

```rust
pub enum Const {
    Bool(bool),
    Char(u32),
    Int(IntKind, u128),
    Uint(UintKind, u128),
    Float(FloatKind, u128),
    ZST,
    Bytes(Vec<Byte>),
    Aggregate(Vec<ConstId>),
    EnumValue { variant_idx: VariantIdx, payload: Vec<ConstId> },
    NullPtr(TyId),
    Reloc(InstanceRef),
}
```

**Reasoning.**

*Why not Option B (rustc-mirror).* rustc's stratification --
`ty::Const` / `mir::Const` / `mir::ConstValue` plus the
`Indirect`-with-`Allocation` lump -- exists because rustc has both
pre-eval and post-eval surfaces and layered IRs. The post-mono
guarantee that motivated Q3/Q5/Q6 of `architecture.md` lets us
skip pre-eval shapes entirely (rustc has already evaluated
everything that reaches RegseqModel). The `Indirect` lump would
defeat per-variant local identity-key derivation: every kind of
value-level const (statics, vtable instances, aggregates, fn-
pointer relocations) would funnel through one variant whose
payload is opaque bytes-with-provenance, and the bridge would
have to re-introspect the same way the codegen does. See
`prior-art/rustc.md` Findings 1 and 7 for the layering, and
`prior-art/README.md` §4.3 for the cross-cluster confirmation
that stratified universes exist for pre-eval / post-eval reasons
that don't apply here.

*Why not Option C (two-tier physical split).* 0-of-5 non-rustc
projects have a phase split (`prior-art/README.md` §2.4). In
Option A the type-level subset is identifiable by variant
discriminant -- `Bool`, `Char`, `Int`, `Uint`, `Float` are type-
level; the rest are value-level. That's a `match` arm, not a
separate type. Option C buys structural enforcement at the cost
of two interner tables, double the public API, and divergence
from spec.md §9.7's "one `ConstInterner`" language. The
validation phase ordering already enforces the dependency rule
mechanically; physical separation is cosmetic.

*Why per-flavour variants over a tagged scalar `Scalar { ty,
value }`.* Three considerations. (a) Spec.md §9.7 line 1560-1561
already pins inline scalar tagging. (b) Per-variant identity-key
derivation is local: each variant has a clear canonical
representation rule (`Int(I32, 1)` ≠ `Int(I64, 1)`; the variant
tag itself is part of the identity). (c) Three voices in the
prior-art evidence agree on inline scalar tags: spec.md §9.7
already, `prior-art/mlir.md` (`TypedAttrInterface`),
`prior-art/mrustc.md` (`MIR::Constant` scalar variants). The
externalised dissenters (`prior-art/llvm.md`, `prior-art/rustc.md`)
diverge for class-system reasons (LLVM's `Constant` is a
`Value` subclass with type already on the parent;
rustc's `mir::Const` carries the type for two-layer reasons we
don't replicate).

**Methodology rules invoked.**

- *Store the const-eval result, not the const expression.* The
  variant set carries fully-evaluated bytes / scalar values; no
  variant represents `ConstKind::Unevaluated` or expression-
  level shapes.
- *Cross-build canonicalisation uses stable identity.* `Reloc`
  and `Bytes::Reloc` carry `InstanceRef` (the 4-tuple), not
  `InstanceId`.
- *No defensive fallbacks.* No "Other" / "Unknown" / "Opaque"
  variant. If rustc gives the bridge something that does not
  fit a listed variant, the bridge raises a diagnostic; it does
  not silently widen.
- *Compositional behavior beats special-case flags.* No
  flag-style "is_string_literal" / "is_vtable_instance" markers
  on `Const` — distinctions ride on variant tag and
  consumer-context type. (Once the deferred
  `SyntheticItemKind::ConstMemoryStatic` family lands per
  D-4-1.9, `&str` backing also falls out compositionally as
  `Aggregate([Reloc(memory_static_inst_ref), Uint(Usize, len)])`
  — no new variant; the same compositional shape that vtable
  instance fat pointers will use. Until then, `&str` /
  `&[T]` reference-typed const handling is stage-4-deferred per
  the §9.7 "Stage-4 deferred: reference-typed const handling"
  subsection.) `Option<ConstId>` niche preservation (the
  4-byte invariant) imposes `ConstId = u32` (`NonMaxU32`)
  which the variant choices honour.

**Prior-art evidence.**

- `prior-art/rustc.md` Findings 1, 7 -- the two-layer
  stratification and post-mono guarantee at codegen. Confirms
  Option B's stratification is unnecessary for us.
- `prior-art/charon.md` Findings 1, 6 -- 13-variant
  `ConstantExprKind` pre-simplification; the `Byte` three-state
  for uninit; closest peer's design.
- `prior-art/llvm.md` Findings 1, 5 -- `Constant` hierarchy
  including per-pointer-type `ConstantPointerNull`; gold-
  standard for typed null.
- `prior-art/mlir.md` Findings 1, 2 -- attribute-not-value model
  with inline `TypedAttrInterface` tagging.
- `prior-art/mrustc.md` Finding 1 -- 10-variant pre-eval
  `MIR::Constant`; cross-check on what's tractable without the
  post-mono guarantee (and Finding 7 -- the cost of doing
  without it).
- `prior-art/compcert.md` Finding 1 -- 4-universe stratification;
  reference for the formal-verification pressure.
- `prior-art/regseq-model-old.md` Findings 1-9 -- 7-of-8 prior
  decisions don't survive re-derivation under stage-4 framing;
  the design choice we deliberately do not lift wholesale.
- `prior-art/README.md` §1, §2.1, §3.1, §4.3, §6 -- cross-cluster
  synthesis grounding the choice.

**Worked examples.** Pinned in sub-decisions D-4-1.1 through
D-4-1.9; assembled into `spec-snapshot.md` §9.7 worked-example
appendix.

**Cross-refs.** `spec-snapshot.md` §9.7 (variant definitions);
`status.md` "Open decisions" (sub-decision catalogue);
`review-log.md` rounds (none yet).

---

### D-4-1.1 -- Scalar grouping: per-type variants [LOCKED]

**Question.** How are scalar const values shaped at the variant
level: per-type variants with inline tag, or a single
`Scalar { ty, bits }` shape?

**Key options.**

1. Per-type variants: `Bool(bool)`, `Char(u32)`, `Int(IntKind,
   u128)`, `Uint(UintKind, u128)`, `Float(FloatKind, u128)`.
2. Single tagged scalar: `Scalar { ty: TyId, bits: u128 }`.

**Decision.** Option 1 (per-type variants). All five scalar
variants store their value in a `u128` (signed/unsigned variants
use widening/zero-extension; smaller floats zero-pad into the
high bits of the bit-pattern field). `Bool` carries a bare `bool`;
`Char` carries the 32-bit Unicode scalar value as `u32`.

**Reasoning.** Per-type variants make the variant tag itself
part of the equality class: `Int(I32, 1)` is not equal to
`Int(I64, 1)` is not equal to `Uint(U32, 1)` is not equal to
`Float(F32, 0x3f800000)` (the bit-pattern of `1.0f32`). With
single tagged scalar the equality rule depends on the `ty`
field's interaction with the `bits` field, which complicates
canonicalisation rules and gives the canonicaliser more places
to be wrong.

The `u128` width is mandatory for `I128` / `U128` and sufficient
for `F128`; smaller types carry their canonical-zero-extended
form (e.g., `Int(I8, -1)` stores as `u128::MAX` -- the two's-
complement extension of `-1` to 128 bits). Canonicalisation rule
in `ConstIdentityKey`: scalar variants are keyed by
`(variant_tag, kind_discriminant, value_bits)` where smaller
types' extension bits are deterministic by bridge construction.

`Bool(bool)` and `Char(u32)` get bare payloads because their
domains are smaller and natural. `Char` storage as `u32` (rather
than `char`) keeps the canonical bytes deterministic without
relying on Rust's `char` validity invariant in serialisation /
hashing -- the bridge has already validated the value is a valid
Unicode scalar at construction.

**Methodology rules invoked.** *Store the result, not the input;*
*no defensive fallbacks;* *information preservation* (smaller
scalar widths preserve via deterministic extension rather than
narrowing).

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 2 -- rustc keeps the type tag on
  the enclosing pair (externalised), but `ScalarInt` carries
  `(data: u128, size: u8)` -- the same widening pattern.
  Stage-4 inlines the type, which is the divergence we
  deliberately make.
- `prior-art/charon.md` Finding 2 -- `ScalarValue::{Signed(IntTy,
  _), Unsigned(UIntTy, _)}` confirms inline tag.
- `prior-art/mrustc.md` Finding 2 -- `MIR::Constant` scalar
  variants confirm inline tag; flagged warning on lossy `double`
  storage for `f16/f128`. Stage-4 avoids this with per-`FloatKind`
  bit-pattern storage in `u128`.
- `prior-art/regseq-model-old.md` Finding 1 -- 11 scalar-only
  variants confirm the inline-tag pattern but lack `Char`,
  `Float`, and `U128/I128` coverage; stage-4 corrects all three.

**Cross-refs.** `spec-snapshot.md` §9.7 (scalar variants);
`spec-snapshot.md` §10.0 (`Ty` enum scalar primitives -- the
type-side pairing).

---

### D-4-1.2 -- ZST canonicalisation: single representative [LOCKED]

**Question.** How are zero-sized const values represented? One
canonical `ZST` variant with no payload, or per-zst-type
representatives?

**Key options.**

1. Single `ZST` variant, no payload. All Rust ZSTs (`()`,
   fieldless structs, `PhantomData<T>`, etc.) collapse to one
   canonical representative.
2. `ZST(TyId)`, one per zst-type.

**Decision.** Option 1 (single `ZST` variant, no payload). The
type-of-this-ZST is supplied by the consumer's context (the
`InstanceKind::Const { value }`'s associated Item carries `ty`;
`GenericArg::Const(_)` does not admit ZSTs in v1).

**Reasoning.** ZSTs have no value bits -- they contribute zero
bytes to a byte-level encoding. Carrying type identity inside
the variant would force `ZST(unit_ty_id)` and
`ZST(phantom_data_ty_id)` to intern as distinct `ConstId`s
despite being byte-level identical, which blocks sharing across
contexts that re-use the canonical ZST. The Item-level type
already disambiguates: codegen knows from the Item or
GenericArg position which type the ZST inhabits.

The type-level subset (`GenericArg::Const(_)`) is restricted to
scalar primitives in v1 (per `spec.md` §9.7 line 1639), so a ZST
appearing as a generic-const argument is bridge-rejected per the
no-defensive-fallbacks methodology rule -- if `feature(adt_const_params)`
or unit-typed const generics ever appear in the driver DSL,
they're out-of-scope.

**Methodology rules invoked.** *No defensive fallbacks;*
*compositional behavior beats special-case flags* (single ZST
representative; type comes from context).

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 1 -- `ConstValue::ZeroSized` is a
  payload-free variant.
- `prior-art/llvm.md` Finding 1 -- `ConstantAggregateZero` is the
  ZST analogue (slightly different scope; LLVM uses it for
  zero-initialised aggregates, not just ZSTs, but the
  no-payload pattern matches).
- `prior-art/charon.md` Finding 1 -- `Adt(VariantId, [])` for
  empty-aggregate ZSTs; less canonical than stage-4's choice
  because it ties to a specific empty-variant context.

**Cross-refs.** `spec-snapshot.md` §9.7 (ZST canonical
representative); `discussions.md` D-4-1.7 (type-tag placement
implication).

---

### D-4-1.3 -- Aggregate field elements: `Vec<ConstId>` (interned children) [LOCKED]

**Question.** How are the field children of an aggregate const
represented: as interned `ConstId`s for sharing, or as inline
`Const` values?

**Key options.**

1. `Aggregate(Vec<ConstId>)` -- interned children share across
   structurally-equal aggregates.
2. `Aggregate(Vec<Const>)` -- inline children, no sharing.

**Decision.** Option 1 (`Vec<ConstId>`).

**Reasoning.** Two arguments. (a) **Sharing matters at our
scale.** Vtable instances reuse the same `NullPtr(canonical_vacant_sig_ty_id)`
and the same `Reloc(drop_shim_inst_ref)` across many vacant
slots and across many vtable types. Generic struct const literals
(`const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 }`) reuse
their scalar children. Inline children would force duplicate
materialisation; interned children share for free with the
existing `ConstInterner` content-uniqueness rule. (b) **The
identity-key shape stays simple.** `Vec<ConstId>` is the
*storage shape* of the variant; the *cross-build canonical
identity key* for an `Aggregate` recursively uses each child's
canonical const representation (per §12.3's deep-canonicaliser
pattern: a child `ConstId` resolves via `canonical_const`
(D-4-2's per-`Const`-variant byte-encoding) to the same
canonical content that interned the child's entry,
which is build-stable per the type-level / value-level
invariants). With inline children, the variant carries each
child's full content materialised; interner-mediated sharing
is lost. The two are operationally equivalent for canonical
equality but storage-shape-divergent — `Vec<ConstId>` is
chosen for the sharing benefit.

`prior-art/mlir.md` Finding 3 confirms: `ArrayAttr<Vec<Attribute>>`
and `DenseElementsAttr` both store children as
storage-pointer references (the MLIR attribute uniquer's
`Attribute` is a pointer-into-`AttributeStorage`). The MLIR
splat optimisation (single-element storage when all children
equal) is a candidate compression noted in
`prior-art/README.md` §5.1 -- defer to implementation-time.

The choice diverges from `prior-art/charon.md` (inline `Vec<ConstantExpr>`)
and `prior-art/mrustc.md` (flat byte buffer or eager-promotion
to fresh synthetic statics) and `prior-art/compcert.md`
(aggregates only as `init_data` lists for globals) -- the
divergences trade more interner activity for downstream code
clarity, which is the right trade-off for a verification-target
IR.

**Methodology rules invoked.** *Information preservation*
(structural shape preserved, not flattened to bytes);
*compositional behavior* (sharing falls out of the existing
interner mechanism; no new sharing primitive needed).

**Prior-art evidence.**

- `prior-art/mlir.md` Finding 3 -- `ArrayAttr<Vec<storage-ptr>>`
  confirms the indirected-children pattern.
- `prior-art/rustc.md` Finding 3 -- `ValTreeKind::Branch(Box<[Const]>)`
  uses inline children at `ty::Const` layer, but rustc has its
  own arena interning that gives functionally equivalent
  sharing; we'd need explicit IDs without the arena.
- `prior-art/charon.md` Finding 3 -- inline `Vec<ConstantExpr>`,
  no sharing; deliberately chosen divergence.
- `prior-art/mrustc.md` Finding 3 -- demote-to-static (flat byte
  buffers); deliberately chosen divergence.

**Cross-refs.** `spec-snapshot.md` §9.7 (`Aggregate` variant);
`discussions.md` D-4-1.9 (first-class aggregate vs demote-to-
static); `discussions.md` D-4-1.8 (the byte-level alternative
shape `Bytes` for unstructured aggregates).

---

### D-4-1.4 -- Enum payload: `EnumValue { variant_idx, payload }` [LOCKED]

**Question.** How are enum-typed const values represented? With
explicit variant index plus payload children, or via the byte
encoding alone?

**Key options.**

1. `EnumValue { variant_idx: VariantIdx, payload: Vec<ConstId> }`.
   Discriminant value is implicit via the consumer's Ty's
   `VariantLayout.discriminant` (per stage-3 D13).
2. `Bytes(Vec<Byte>)` containing the materialised tag bytes plus
   payload bytes.
3. `EnumValue { discriminant: u128, tag_value: Option<u128>,
   payload: Vec<ConstId> }` -- carry the layout-resolved
   discriminant inline.

**Decision.** Option 1 (`EnumValue { variant_idx, payload }`).
Discriminant value is **not** stored in `Const`; the consumer's
Ty (an `Ty::Enum` with a populated `Layout` per stage-3 D2 + D13)
carries it on `VariantLayout.discriminant`.

**Reasoning.** *Don't duplicate stage-3's work.* Stage-3 D13
explicitly added `VariantLayout.discriminant: u128` and
`tag_value: Option<u128>` so consumers can resolve discriminant
values from the type's layout. Carrying them on the const value
would force the bridge to compute (and store) the same
information twice, with the const-side and type-side values
required to match by an additional invariant. By keeping
`EnumValue` to `(variant_idx, payload)`, the const value is
keyed by which variant it inhabits and what data the payload
carries, and the discriminant byte representation is read from
the type at codegen / pretty-printing time.

`Bytes(Vec<Byte>)` is the fallback when an enum value carries
provenance/relocations that can't decompose structurally (a niche-
encoded enum where the niche is a function-pointer relocation,
hypothetically). Stage-4 prefers `EnumValue` when rustc's
const-eval gives a structured `ValTree::Branch` shape; the bridge
falls back to `Bytes` otherwise per the D-4-1.9 dispatch rule.

**Methodology rules invoked.** *Don't redo work the rustc
compiler has already done* (or, here, work stage-3 has already
done -- discriminant resolution lives on `VariantLayout`).
*Compositional behavior;* *information preservation* (the
variant identity is preserved structurally; the byte
representation is computable).

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 3 -- `ValTreeKind::Branch` for
  enum values is `(variant_idx, fields)` with discriminant
  resolved separately; structurally identical to stage-4's
  choice.
- `prior-art/charon.md` Finding 1 -- `Adt(VariantId, fields)`;
  same shape.
- `prior-art/llvm.md` Finding 3 -- LLVM has no enum concept at
  the IR level (enums are lowered to integer-tagged unions
  before reaching IR); their `ConstantStruct` over the lowered
  representation is the analogue.

**Cross-refs.** `spec-snapshot.md` §9.7 (`EnumValue` variant);
`spec-snapshot.md` §11.1 (`VariantLayout` -- consumer of the
variant index); stage-3 D13.

---

### D-4-1.5 -- Relocation reference: `InstanceRef` 4-tuple [LOCKED]

**Question.** How is "this const points to that function /
static / vtable" represented in a way that supports cross-build
identity?

**Key options.**

1. **`InstanceRef` 4-tuple**: `(regseq_kind_rank: u8,
   def_path_hash: DefPathHash, args_key: ArgsKey,
   fn_shim_disambig: u8)`. Mirrors §9.3's `RegseqInstanceKey`
   shape with the local `ItemId` projected to
   `def_path_hash`.
2. **Local `InstanceId`**: an `IndexVec<InstanceId, _>` index.
3. **`(DefPathHash, ArgsKey)` only**: drop the kind rank and
   shim disambiguator.

**Decision.** Option 1 (the 4-tuple).

**Reasoning.** *Cross-build canonicalisation requires
build-stable identity.* Two builds satisfying the build-
equivalence quad (equal source, equal pinned toolchain, equal
`RegseqModel.target`, equal bridge options) must produce
identical canonical const keys for relocations to the same
Instance. Local `InstanceId` is a counter assigned in F4-2 sort
order during construction; relying on it would propagate the
counter into the const key, which then propagates into the F4-2
fingerprint via `GenericArg::Const(ConstId)` inside `ArgsKey`,
forming a circularity (the counter depends on the sort which
depends on the fingerprint which depends on the counter). The
4-tuple is build-stable because each component is build-stable:
`regseq_kind_rank` is the §9.3-fixed `regseq_instance_kind_rank`
(`Function`=0, `Static`=1, `Const`=2, `Struct`=3, `Enum`=4,
`Union`=5); `def_path_hash` is rustc's stable cross-build hash
(or the deterministic bridge-synthesised hash per spec.md §13.4
for synthetic Items); `args_key` is the §12.3 deep canonicaliser;
`fn_shim_disambig` is the §9.3-fixed `rustc_instance_kind_sort_rank`
(e.g., `Item`=0, `DropGlue`=10, `VTableShim`=2, ...). The
identical shape — minus the local `ItemId` — appears in the
in-build `RegseqInstanceKey` (spec.md §9.3); reusing it for
`InstanceRef` keeps cross-build identity operationally
equivalent to in-build identity.

*Why `regseq_kind_rank` (not a "kind discriminant").* Stage-3
§9.3 pins exactly six Regseq InstanceKind ranks; nothing labelled
`DropGlue`, `VTableShim`, or `VTableInstance` is its own
Regseq kind. `DropGlue` is a *rustc shim flavor* of the
`Function` Regseq kind, captured in `fn_shim_disambig`; vtable
instances are bridge-synthesised `Static` Items (rank 1) marked
in `item_synthetic_kinds` per §13.2. So `regseq_kind_rank`
disambiguates Regseq emission kinds, and `fn_shim_disambig`
disambiguates rustc shim flavors within the `Function` kind.
Two different rustc shim flavors sharing a `(DefId, args)` —
which §9.3 explicitly calls out as the case `Item(foo)` and
`ReifyShim(foo)` — would otherwise collapse.

*Why no separate `shim_disambig: Option<ShimDisambig>` enum.*
Vtable method shims and vtable drop shims are bridge-
synthesised Function Items per §13.2. Their canonical identity
is the synthesis context (`PrincipalTraitKey`, `ConcreteImpl`,
`vtable_entry_index` or `"drop"`); spec.md §13.4 encodes this
in the synthesised DefPath segments (`vtable_method_shim` /
`vtable_drop_shim` segment, per-shim `vtable_entry_index`
segment, etc.), so the resulting synthesised `DefPathHash`
already disambiguates method-vs-drop and per-method-index. A
separate `shim_disambig` enum field would duplicate information
already carried by `def_path_hash` — failing the "no defensive
fallbacks" methodology rule (no v2 hedge for cases that cannot
arise under the documented spec surface).

`prior-art/README.md` §3.1 documents that 5-of-5 non-rustc
projects fail cross-build determinism. Only adjacent precedent
is rustc's `HashStable for AllocId` at
`compiler/rustc_middle/src/ty/impls_ty.rs:67-76` which resolves
the local AllocId to its `GlobalAlloc` target before hashing,
deliberately discarding the local handle. Stage-4's 4-tuple is
operationally equivalent: it captures the resolved target in
build-stable form.

**Methodology rules invoked.** *Cross-build canonicalisation
uses stable identity.* *No defensive fallbacks* (no fallback
to local `InstanceId`).

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 8 -- `HashStable for AllocId`
  precedent; the only direct prior art for cross-build
  content-addressing of relocations.
- `prior-art/charon.md` Finding 4 -- local-ID relocations; cited
  as the negative case stage-4 deliberately avoids.
- `prior-art/llvm.md` Finding 4 -- `GlobalValue`-as-Constant
  relocations; in-memory pointer; bitcode is the stable form
  but allocation-order-dependent.
- `prior-art/mlir.md` Finding 4 -- `SymbolRefAttr` is string-path-
  based; structurally simpler than 4-tuple but loses
  disambiguation between `Function` / `DropGlue` /
  `VTableMethodShim` instances at the same DefPath.
- `prior-art/mrustc.md` Finding 4 -- `HIR::Path` verbatim with
  `PTR_BASE = 0x1000` magic byte; serialisation hack stage-4
  avoids via structured 4-tuple.
- `prior-art/compcert.md` Finding 4 -- `Init_addrof`; identifier
  is local `positive`.
- `prior-art/regseq-model-old.md` Finding 4 -- no relocation
  shape at all in the old `LiteralValue` variant.
- `prior-art/README.md` §3.1 -- cross-cluster "novel territory"
  finding.

**Cross-refs.** `spec-snapshot.md` §9.7 (`InstanceRef`,
`Reloc(InstanceRef)` variant; the in-bytes `Bytes::Reloc(InstanceRef)`
case); `spec-snapshot.md` §13.4 (synthesised `DefPath`
segments and `DefPathHash` construction); `spec-snapshot.md`
§16 (F4 / F4-2 determinism).

---

### D-4-1.6 -- Null pointer typing: `NullPtr(TyId)` [LOCKED]

**Question.** How are typed null pointers represented? With
pointee-type inline (per-type variant), or as a single null
representative with type contextually supplied?

**Key options.**

1. `NullPtr(TyId)` -- pointee type inline; per-pointee-type
   canonicalisation.
2. `NullPtr` -- single representative; type from context.
3. Subsume into `Bytes`: a null pointer is `Bytes(Vec<Byte::Value(0); ptr_size>)`
   plus the consumer's pointer type.

**Decision.** Option 1 (`NullPtr(TyId)` with pointee type
inline).

**Reasoning.** *Vacant vtable slots and source-level typed
nulls force per-pointee-type canonicalisation.* Per stage-3
§13.2, vacant vtable slots are explicit fn-pointer fields with
`Ty::FnPtr(canonical_vacant_sig_id)` and a null value. The
vtable type Item carries a `Layout` with multiple vacant slots;
the same model also admits source-level
`core::ptr::null::<T>()` calls producing typed nulls
`NullPtr(T)`. If a single canonical-null `NullPtr` applied to
all of these, `ConstId`-level identity could not distinguish
"vacant slot of canonical-vacant-sig" from "source-level
`core::ptr::null::<u8>()`" — they are functionally distinct
constants (different downstream codegen lowerings, different
F4-2 fingerprints reachable through `GenericArg::Const(_)` if
ever exposed). A single `NullPtr` representative with type-
from-context would shift the canonicalisation burden onto the
consumer's identity scheme, fragmenting the F4-2 fingerprint
story.

`prior-art/llvm.md` Finding 5 confirms the choice: LLVM's
`ConstantPointerNull::get(PointerType*)` interns per-pointer-
type; the polymorphic factory `Constant::getNullValue(Type*)`
takes a type and dispatches. LLVM is the gold standard for
this design; charon's hybrid (`PtrNoProvenance(0)` + outer
`ty: Ty` -- `prior-art/charon.md` Finding 5) achieves the same
disambiguation through different mechanism.

The `Bytes` subsumption (Option 3) loses information -- byte-
level encoding cannot recover whether the source was a typed
null or just zero bytes that happen to be pointer-sized. Stage-
4's information-preservation methodology rule rules it out.

**Methodology rules invoked.** *Information preservation*
(distinguish typed null from zero bytes); *compositional
behavior* (typed null is its own variant; doesn't fold into a
shared "null thing" representative).

**Prior-art evidence.**

- `prior-art/llvm.md` Finding 5 -- `ConstantPointerNull::get(PointerType*)`;
  per-pointer-type interning. Direct gold-standard precedent.
- `prior-art/charon.md` Finding 5 -- outer `ty: Ty` + inner
  `PtrNoProvenance(0)`; functionally equivalent.
- `prior-art/mrustc.md` Finding 5 -- implicit (zero bytes,
  no reloc, type from context); fails vacant-vtable-slot
  canonicalisation.
- `prior-art/compcert.md` Finding 5 -- `Vnullptr` is untyped;
  same failure mode.
- `prior-art/mlir.md` Finding 5 -- no `NullAttr`; per-type
  materialised via `llvm.mlir.zero` op.

**Cross-refs.** `spec-snapshot.md` §9.7 (`NullPtr(TyId)` variant);
`spec-snapshot.md` §13.2 (vacant vtable slots; canonical vacant
signature `Ty::FnPtr(canonical_vacant_sig_id)`).

---

### D-4-1.7 -- Type tag placement: inline-only, no outer wrapper [LOCKED]

**Question.** Does `Const` have an outer type tag (a wrapper
`Const { ty: TyId, kind: ConstKind }`) or do per-variant
contents carry whatever type info they need internally?

**Key options.**

1. **Inline-only.** No wrapper. Each variant carries the type
   info it needs (scalars via `IntKind` / `UintKind` /
   `FloatKind`; nulls via `NullPtr(TyId)`; aggregates via
   children's types).
2. **Hybrid.** `Const { ty: TyId, kind: ConstKind }` where `kind`
   is the per-variant payload AND `ty` is an outer wrapper.
3. **Wrapper-only.** Variants carry only structural shape; type
   comes from the outer `ty` field.

**Decision.** Option 1 (inline-only).

**Reasoning.** *Each variant carries exactly the type info it
needs.* Scalars carry `IntKind` / `UintKind` / `FloatKind` --
the type-equivalent for primitive scalars without forcing a
TyId resolution. Nulls carry `NullPtr(TyId)` because the pointee
type is load-bearing for canonicalisation (D-4-1.6). Aggregates
do not carry type at the `Const` layer because the consumer's
context (a struct-typed Item, an enum's `EnumValue`) already
determines structure; codegen pattern-matches on the consumer's
`Ty`.

The hybrid (Option 2) would carry a redundant outer `TyId` for
scalars (the `IntKind` already determines the type) and a
duplicate-information outer `TyId` for nulls and aggregates
(matching the consumer's context). The wrapper-only option
(Option 3) loses the per-variant discrimination that makes
identity-key derivation local.

3 prior-art voices favor inline-tag for scalars (spec.md §9.7
already pins, `prior-art/mlir.md`, `prior-art/mrustc.md`); 2
favor externalised (`prior-art/llvm.md` Value-class
inheritance, `prior-art/rustc.md` `mir::Const::Val(ConstValue,
Ty)` two-layer); 2 use hybrid (`prior-art/charon.md`,
`prior-art/regseq-model-old.md`). The decision picks the
inline-tag side and additionally drops the outer wrapper that
the hybrid voices keep -- justified by the per-variant payload
analysis above.

A consequence: code paths that need the type of an arbitrary
`ConstId` (e.g., a debug renderer) walk per-variant: scalars
synthesise from `IntKind` / `UintKind` / `FloatKind`; `NullPtr`
reads its `TyId`; `ZST` returns "ZST" symbolically (the
consumer's context disambiguates); `Aggregate` and `EnumValue`
have no inherent type without consumer context. Renderers
should be invoked with the consumer's `Ty` already in scope --
which they are at every call site stage-4 has surveyed
(InstanceKind dispatch, GenericArg resolution, &str backing).

**Methodology rules invoked.** *No defensive fallbacks*
(redundant outer wrappers are a defensive fallback against
"what if the consumer doesn't supply context"); *information
preservation* (each variant carries what it needs, no more);
*compositional behavior* (type info comes from the consumer's
context for variants that don't carry it intrinsically).

**Prior-art evidence.**

- spec.md §9.7 line 1560-1561 -- already pins inline scalar tags.
- `prior-art/mlir.md` Finding 2 -- inline via
  `TypedAttrInterface`.
- `prior-art/mrustc.md` Finding 2 -- inline scalar tags.
- `prior-art/charon.md` Finding 2 -- hybrid (inline + outer);
  stage-4 deliberately drops outer.
- `prior-art/regseq-model-old.md` Finding 2 -- hybrid; stage-4
  drops outer per the re-derivation analysis at
  `prior-art/regseq-model-old.md` Finding 9.
- `prior-art/llvm.md` Finding 2 -- externalised (type on
  `Value::Type *`); stage-4 inlines for the per-variant identity-
  key locality argument.
- `prior-art/rustc.md` Finding 2 -- externalised
  (`mir::Const::Val(ConstValue, Ty)`); rustc has a two-layer
  reason (Ty-level Const vs MIR-level Const); stage-4 has one
  layer and inlines.
- `prior-art/README.md` §1.1, §2.1 -- cross-cluster split (3
  inline / 2 externalised / 2 hybrid); the synthesis flagged
  the inline-vs-hybrid question for explicit decision.

**Cross-refs.** `spec-snapshot.md` §9.7 (variant definitions
with inline payloads); D-4-1.1 (scalar tags inline);
D-4-1.6 (`NullPtr` carries `TyId` -- not a wrapper-style outer
field but a variant-intrinsic type carrier).

---

### D-4-1.8 -- Uninit / byte-level handling: `Bytes(Vec<Byte>)` three-state [LOCKED]

**Question.** How are byte-level const values (with potential
uninit bytes and potential pointer relocations) represented?

**Key options.**

1. **`Bytes(Vec<Byte>)` with three-state Byte:**
   ```
   Byte = Uninit | Value(u8) | Reloc(InstanceRef)
   ```
2. **`Bytes(Vec<u8>)` only**: lossy, collapse uninit-as-zero
   (mrustc's path).
3. **`Bytes(Vec<u8>)` + parallel `InitMask`**: rustc-style lazy
   bit-mask alongside the byte buffer.
4. **No Bytes variant**: every byte-level value goes through
   structured `Aggregate` / `EnumValue` / `Reloc`.

**Decision.** Option 1 (three-state `Byte`).

**Reasoning.** *Stage-4 must distinguish uninit from any byte
pattern* (spec.md §9.7 line 1614-1616). Option 2 (lossy) is
ruled out by the methodology's information-preservation rule.
Option 3 (parallel InitMask) is the rustc design and is sound,
but introduces dual-storage with a synchronisation invariant
(the byte buffer length and the InitMask length must match;
relocations live in a third side table); the three-state `Byte`
unifies all three concerns in one container at modest size cost
(one tag byte + one payload byte vs raw `u8`).

*Why three states (not four).* Uninit, materialised value, and
relocation are the only states the bridge encounters in rustc's
const-eval output. Padding bytes inside an aggregate ARE uninit
per Rust's semantics (rustc's `InitMask` marks them so).
Pointer-bearing bytes are the relocations. There is no fourth
state.

*Why per-byte tagging (not per-range).* rustc's `InitMask` is
range-based for storage efficiency. Per-byte is simpler in code
and matches `prior-art/charon.md`'s `Byte::{Uninit, Value(u8),
Provenance(Provenance, u8)}` design (MiniRust-derived). At
RegseqModel scale, individual const values are small (vtable
instances are dozens of bytes; string literals are dozens to
hundreds; aggregate consts rarely exceed kilobytes); per-byte
overhead is negligible. The MLIR splat optimisation
(`prior-art/README.md` §5.1) is available as a downstream
compression if it ever matters.

*Why `Reloc(InstanceRef)` per byte rather than `Reloc(InstanceRef,
u8 offset_in_pointer)`.* Each byte of a pointer-bearing slot
carries the same `InstanceRef`; the byte position within the
pointer is recoverable from the slot offset. Validation
invariant (pinned at D-4-2 with the identity-key shape): if
byte at offset `o` is `Reloc(r)`, then bytes
`[o..o + ptr_size]` are all `Reloc(r)`, `o` is aligned to
`ptr_align`, and no `Reloc(_)` byte straddles the alignment
boundary. Carrying offset-within-pointer in the byte payload
would let the bridge encode partial-pointer overlays (which
rustc's const-eval does not produce post-mono); per the no-
defensive-fallbacks rule, we don't pre-allocate for it.

**Methodology rules invoked.** *Information preservation*
(uninit ≠ zero; pointer ≠ scalar bytes); *no defensive
fallbacks* (no offset-within-pointer field; no "Other" Byte
state); *compositional behavior* (the same `InstanceRef` shape
appears in `Bytes::Reloc` and the top-level `Reloc(InstanceRef)`
variant).

**Prior-art evidence.**

- `prior-art/charon.md` Finding 6 -- `Byte::{Uninit, Value(u8),
  Provenance(Provenance, u8)}`; direct precedent. Stage-4 drops
  the `u8` payload in `Provenance` because we don't carry
  offset-within-pointer (post-mono guarantee).
- `prior-art/rustc.md` Finding 6 -- `InitMask` lazy bitmask +
  `Allocation::bytes` + `ProvenanceMap`; equivalent semantics,
  different storage. `prior-art/rustc.md` cites the
  rustc-source location for the lazy bitmask state machine.
- `prior-art/mrustc.md` Finding 6 -- `allocation_to_encoded`
  collapses uninit-as-zero (line 3089 NOTE); explicit lossy
  path stage-4 deliberately avoids.
- `prior-art/compcert.md` Finding 6 -- `Init_space n` becomes
  zero bytes at `Globalenvs.v:910-924`; same lossy path.
- `prior-art/mlir.md` Finding 6 -- no byte-level uninit;
  attribute-only at value level.
- `prior-art/llvm.md` Finding 6 -- `UndefValue` / `PoisonValue`
  per-Type singletons; value-level not byte-level.

**Cross-refs.** `spec-snapshot.md` §9.7 (`Bytes` variant; `Byte`
enum); `spec-snapshot.md` §13.2 (vtable instance values --
primary consumer of `Bytes::Reloc`); D-4-1.5 (`InstanceRef`
shape).

---

### D-4-1.9 -- Aggregate first-class vs demote-to-static [LOCKED]

**Question.** Should aggregate const values (struct / array /
tuple literals) be a first-class `Const` variant, or demoted to
synthesised `InstanceKind::Static` Items with a `Reloc(InstanceRef)`
in the const?

**Key options.**

1. **First-class:** `Aggregate(Vec<ConstId>)` and
   `EnumValue { variant_idx, payload: Vec<ConstId> }` are full
   `Const` variants. `Bytes(Vec<Byte>)` is the byte-level
   alternative when the aggregate carries provenance / mixed
   relocations.
2. **Demote-to-static:** drop `Aggregate` and `EnumValue` from
   `Const`. The bridge synthesises an immutable Static Item per
   aggregate value; the const value reduces to
   `Reloc(synthesised_static_inst_ref)`.

**Decision.** Option 1 (first-class).

**Reasoning.** *Addressability is the discriminating concern.*

True statics (vtable instances, source `static FOO`,
synthesised statics for vtable instance values per stage-3
§13.2) need C-level addresses -- they take program-image space
at fixed locations and downstream code can take pointers to
them. They live in `InstanceKind::Static` with stable storage
semantics.

Aggregate const values used by-value (`const ARR: [u32; 3] =
[1,2,3]; let x = ARR;`) do not need C-level addresses; they
lower to compound-literal expressions inline at use sites:
`int x[3] = {1, 2, 3};` or `(struct Foo){.a=1, .b=2}`.
Forcing every aggregate const into a synthesised Static would
over-allocate program-image memory, conflate two C-level
storage classes, and create thousands of named statics for
trivial array literals.

The asymmetry between `Aggregate` (first-class const) and
vtable-instance-Static (synthesised Item) is genuine, not a
coincidence -- they have different lifetimes in the C output.

`prior-art/mrustc.md` Finding 3 (eager-promote-to-static) and
`prior-art/compcert.md` Finding 2 (no value-level aggregate;
only `init_data` for globals) and `prior-art/regseq-model-old.md`
Finding 3 (no aggregate variant at all; only runtime
`Rvalue::Aggregate`) all take the demote path. Their motivations
differ:
- mrustc has no const-eval result form to canonicalise; demote-
  to-static lets the C codegen emit each aggregate as a named
  initialiser.
- CompCert's `init_data` is the operational semantics of "a
  global variable with this initial value"; aggregate at value
  level would force a value semantics for compile-time-only
  aggregates, complicating the proof.
- regseq-model-old had no const-eval integration; aggregates
  appeared only as runtime constructions.

None of those motivations apply to stage-4. We have rustc's
post-mono const-eval result; we have a use-by-value pattern in
the driver DSL (`const`-bound array/struct literals); we want
codegen to emit them inline.

**Methodology rules invoked.** *Compositional behavior over
special-case flags* (the bridge picks `Aggregate` vs `Bytes` vs
`Reloc` based on what rustc gives it; no special-case demotion
rule); *information preservation* (structural aggregate shape
preserved at IR level; codegen chooses storage class).

*Bridge construction rule (forward to D-4-2 / spec.md §15;
authoritative listing in `spec-snapshot.md` §9.7's "Bridge
construction" tables).* Given a rustc `mir::ConstValue` post-
mono, the bridge dispatches:
- `ConstValue::Scalar(Scalar::Int(_))` → `Bool` / `Char` /
  `Int(...)` / `Uint(...)` / `Float(...)` / `NullPtr(_)` per
  the scalar's type.
- `ConstValue::Scalar(Scalar::Ptr(_))` → `Reloc(InstanceRef)`
  per the pointer's `AllocId` resolution to a `GlobalAlloc`.
- `ConstValue::ZeroSized` → `ZST`.
- `ConstValue::Indirect { alloc, offset }`:
  - if `alloc` has no provenance entries and the value-side `Ty`
    has structural shape → walk via valtree-to-aggregate; emit
    `Aggregate(Vec<ConstId>)` or `EnumValue { variant_idx,
    payload }`.
  - otherwise → emit `Bytes(Vec<Byte>)` from `alloc.bytes` and
    `alloc.provenance`, with `Byte::Uninit` for ranges absent
    from `alloc.init_mask`.

`ConstValue::Slice { data, meta }` and any
`ConstValue::Indirect` arm targeting `GlobalAlloc::Memory` are
**deferred** — they require a new `SyntheticItemKind::ConstMemoryStatic`
family in §13.3 / §13.4 to back the resulting
`Aggregate([Reloc(memory_static_inst_ref), Uint(Usize, meta)])`
shape. See `spec-snapshot.md` §9.7 "Stage-4 deferred:
reference-typed const handling."

The bridge does NOT walk through `valtree_to_const_val` (per
`prior-art/rustc.md` open question (c)) -- that path materialises
fresh `AllocId`s for non-primitive aggregates, breaking
determinism. Stage-4's bridge walks `ValTree::Branch` directly
to `Aggregate` / `EnumValue`, and walks `mir::ConstValue::Indirect`
for already-encoded byte-level values.

**Prior-art evidence.**

- `prior-art/rustc.md` Findings 1, 3 -- `ValTreeKind::Branch` is
  the structured-aggregate source; `ConstValue::Indirect` is
  the byte-level source. Both available post-mono.
- `prior-art/mlir.md` Finding 3 -- `ArrayAttr<Vec<Attribute>>`;
  first-class structured aggregate at attribute level.
- `prior-art/charon.md` Findings 1, 3 -- 13 `ConstantExprKind`
  variants including `Adt` first-class; closest peer's choice
  matches stage-4.
- `prior-art/llvm.md` Finding 3 -- `ConstantArray`,
  `ConstantStruct` first-class.
- `prior-art/mrustc.md` Finding 3 -- demote-to-static; cited as
  cross-check, not adopted.
- `prior-art/compcert.md` Finding 2 -- demote-to-init_data;
  cited as cross-check, not adopted.
- `prior-art/regseq-model-old.md` Findings 3, 9 -- no aggregate
  variant; re-derivation analysis confirms the old choice does
  not survive stage-4 framing.
- `prior-art/README.md` §2.2 -- cross-cluster framing of the
  fork.

**Cross-refs.** `spec-snapshot.md` §9.7 (`Aggregate`,
`EnumValue`, `Bytes` variants); `spec-snapshot.md` §13.2
(vtable instances -- the synthesised-Static path that does not
demote); `spec-snapshot.md` §15 (bridge translation rules
extending with `mir::ConstValue` dispatch).

---

### D-4-2 -- `ConstIdentityKey` per-variant byte-form schema [LOCKED]

**Question.** Given the locked 11-variant `Const` enum (D-4-1),
the locked `Byte` three-state, and the locked `InstanceRef`
4-tuple, what is the byte-level canonical form of
`ConstIdentityKey` per variant — such that two `Const` values
with semantically identical content produce byte-identical
identity-key bytes across builds, and so that
`canonical_args.recurse(ConstId)` (the §12.3 `ConstId`
walk-rule row) has a concrete encoding to call?

**Key options.**

1. **Option A -- mirror §12.3's per-`Ty` table directly.** A
   parallel per-`Const`-variant table with `u8` variant tags,
   tag-specific payloads, `u32` LE length prefixes, recursive
   `canonical_const(ConstId)` for nested children. Same
   tagged-tree convention; same append-only policy; same LE
   default for multi-byte integers.
2. **Option B -- value-bit-only key (skip variant tags).**
   Drop variant tags and key only on payload bits; rely on
   the consumer's `Ty` to disambiguate.
3. **Option C -- per-`Const` ad-hoc encoding.** Each variant
   gets a bespoke encoding optimized for its own shape (e.g.,
   variable-width integers, sentinel-based `Byte` encoding,
   compact aggregate forms).

**Decision.** Option A — closed across nine sub-decisions
D-4-2.1 through D-4-2.9 pinning the byte form of each variant
class and the recursion / acyclicity rules. The result extends
§9.7 with a new "Per-`Const`-variant byte-encoding tags"
subsection mirroring §12.3's per-`Ty` byte-encoding-tags table,
and replaces the §12.3 `ConstId` walk-rule stub with a concrete
recursion rule pointing at the new subsection.

**Reasoning.**

*Why not Option B (value-bit-only key).* Skipping variant tags
collapses canonically-distinct values into one key (e.g.,
`Bool(false)` and `ZST` would both reduce to "no payload";
`Int(I32, 0)` and `Uint(U32, 0)` would both reduce to "16
zero bytes"). D-4-1.1 made the variant tag itself part of the
equality class; the byte-form must preserve that — variant tag
is non-negotiable.

*Why not Option C (per-variant ad-hoc).* Two costs: (a) every
deviation from §12.3's conventions creates a special case the
canonicaliser must handle (variable-width integers need a
decoding rule; sentinel-based `Byte` encoding needs an escape
mechanism because `Value(0xFE)` is a legal byte). (b) Symmetry
with §12.3 reduces validation surface — the same byte-stream
walker handles both the per-`Ty` and per-`Const` trees with
shared rules for tags, lengths, recursion, and endianness.

*Why mirror §12.3 directly.* Three voices: (i) §12.3's per-`Ty`
table is the load-bearing precedent — the same conventions
(`u8` tag, LE multi-byte ints, `u32` LE length prefixes,
recursive walks) work for every encoded IR identifier. (ii) The
`ConstId` walk-rule row in §12.3 already calls into
`canonical_const(ConstId)` from the `GenericArgs` container's
`GenericArg::Const(ConstId)` arm (the only `ConstId` call site
in the per-`Ty` walk; `Ty::Array(elem, n: u64)` carries a
literal `u64`, not a `ConstId`); the symmetric form makes the
cross-walk uniform. (iii) The `GenericArgs` container encoding
in §12.3 is
itself the precedent for the `u32` LE length prefix — D-4-2
reuses the same prefix width for `Vec<Byte>`, `Vec<ConstId>`,
and `EnumValue.payload`.

**Methodology rules invoked.**

- *Store the result, not the input.* Each variant's encoding
  pins a fixed byte form, never a derivation procedure.
- *Cross-build canonicalisation uses stable identity.* Every
  recursive sub-component (`canonical_ty`, `def_path_hash`,
  `args_key` raw bytes) is already cross-build-stable;
  recursion preserves that property.
- *No defensive fallbacks.* No "Other" / "Opaque" escape
  encoding; the 11 variants are exhaustive (per D-4-1).
- *No forward-looking placeholders.* The deferred reference-
  typed `ConstMemoryStatic` family takes tag value `11+` when
  D-4-3+ lands; no tag values are reserved.
- *Compositional behavior beats special-case flags.* Each
  variant's encoding follows from its declared shape; no
  special-case rules.

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 6 -- rustc's
  `ScalarInt::data: u128` widening pattern is the precedent
  for D-4-2.2's `u128` LE value-bit storage.
- `prior-art/charon.md` Finding 6 -- `Byte` three-state from
  D-4-1.8 has a direct sub-tag analogue here (D-4-2.5).
- `prior-art/llvm.md` Finding 5 -- per-pointer-type
  `ConstantPointerNull` confirms `NullPtr(TyId)` recursion
  into `canonical_ty` (D-4-2.7).
- `prior-art/mlir.md` Finding 2 -- `TypedAttrInterface`'s
  inline-tag pattern matches D-4-2.1's variant-tag-as-
  equality-class.
- `prior-art/README.md` §3.1 -- documents the negative
  cross-cluster result for cross-build content-addressed const
  identity (5-of-5 non-rustc projects fail; rustc's
  `HashStable for AllocId` is the only adjacent precedent and
  the inspiration D-4-1.5's `InstanceRef` 4-tuple already
  inherits). Cited here as the framing for D-4-2's recursive
  `canonical_const` byte form: every recursive sub-component
  of the per-`Const` byte form (`canonical_ty`,
  `def_path_hash`, `args_key.to_be_bytes()`) inherits the same
  build-stable-projection discipline pinned in D-4-1.5.

**Worked examples.** Pinned in sub-decisions D-4-2.1 through
D-4-2.9; assembled into `spec-snapshot.md` §9.7's
"Worked examples" block within the new Per-`Const`-variant
byte-encoding tags subsection.

**Cross-refs.** `spec-snapshot.md` §9.7 (per-`Const`-variant
byte-encoding tags subsection); `spec-snapshot.md` §12.3
(`ConstId` walk-rule row); `spec-snapshot.md` §3.3 Phase B
step 9 (acyclicity sub-check); `status.md` "Open decisions";
`review-log.md` rounds (none yet).

---

### D-4-2.1 -- Variant tag width + numeric assignment + append-only [LOCKED]

**Question.** What `u8` discriminator value gets assigned to
each of the 11 variants, and how is future drift prevented?

**Key options.**

1. `u8` disc, contiguous `0..=10`, append-only.
2. `u16` disc, more headroom but breaks symmetry with §12.3.
3. `u8` disc with reserved gaps for future variants.

**Decision.** Option 1. Numeric assignment in source-declaration
order:

```text
0  = Bool        1 = Char        2 = Int        3 = Uint
4  = Float       5 = ZST         6 = Bytes      7 = Aggregate
8  = EnumValue   9 = NullPtr    10 = Reloc
```

Append-only: future variants (e.g., the deferred
`ConstMemoryStatic`-backed reference-typed family per
§13.3 / §13.4) take tag values `11+`; existing tags are
immutable so cross-build hashes for already-admitted consts
never change. No tag values reserved.

**Reasoning.** Mirrors §12.3's per-`Ty` table directly — same
`u8` width, same contiguous-from-zero assignment, same
append-only stability promise. 11 variants in `u8` leave 245
currently-unassigned tag values available for append-only
future variants (the no-forward-looking-placeholders rule means
those values are not pre-reserved for any specific extension —
they're simply the unused range of the `u8` discriminant
space). `u16` would create asymmetry between the per-`Ty` and
per-`Const` walks. Reserved gaps would formalise "where future
variants land," which violates the no-forward-looking-
placeholders rule.

**Methodology rules invoked.** *No forward-looking placeholders;*
*compositional behavior beats special-case flags;* *cross-build
canonicalisation via stable identity.*

**Prior-art evidence.**

- `prior-art/llvm.md` Findings 1, 5 -- LLVM's `Constant`
  hierarchy uses a per-class type-id (variant-tag-equivalent).
- `prior-art/mlir.md` Finding 2 -- `TypedAttrInterface`
  uses inline-tagged attributes.
- `prior-art/charon.md` Finding 1 -- 13-variant
  `ConstantExprKind` uses per-variant tags.
- `prior-art/mrustc.md` Finding 1 -- 10-variant
  `MIR::Constant` uses per-variant tags.
- `prior-art/regseq-model-old.md` Finding 1 -- 11-variant
  scalar-only enum uses per-variant tags.
  Cross-project: 5-of-5 projects use a tag-then-payload
  variant-tag convention; D-4-2.1's `u8` width and
  contiguous-from-zero assignment is the local materialisation.

**Cross-refs.** `spec-snapshot.md` §9.7 (per-`Const`-variant
byte-encoding tags); `spec-snapshot.md` §12.3 (per-`Ty`
parallel).

---

### D-4-2.2 -- Scalar payload byte forms [LOCKED]

**Question.** How does each scalar variant's payload (`Bool`,
`Char`, `Int(k, _)`, `Uint(k, _)`, `Float(k, _)`) serialise
byte by byte?

**Key options.**

1. **Per-`Ty`-mirroring** with `u128` LE for value-bit storage,
   `u8` discriminants for `IntKind` / `UintKind` / `FloatKind`
   reusing the §10 `#[repr(u8)]` enum-definition assignments
   (the same values §12.3's per-`Ty` table reuses for `Ty::Int(k)`,
   `Ty::Uint(k)`, `Ty::Float(k)`).
2. **Per-variant minimal width** (e.g., `Int(I8, _)` emits 1
   byte; `Int(I32, _)` emits 4 bytes). Saves 12-15 bytes per
   small-width scalar but requires a per-`IntKind` decoding
   rule.
3. **`u64` LE compromise** with `(low_u64, high_u64)` pair for
   `I128`/`U128`/`F128` only.

**Decision.** Option 1.

```text
Bool(b)        : u8 (0x00 | 0x01)
Char(cp)       : u32 LE Unicode scalar value
Int(k, v)      : u8 IntKind disc + u128 LE value (sign-extended for narrow)
Uint(k, v)     : u8 UintKind disc + u128 LE value (zero-extended for narrow)
Float(k, v)    : u8 FloatKind disc + u128 LE bit-pattern (low-order
                 holds IEEE bits; high-order zero for narrow widths)
```

`IntKind` / `UintKind` / `FloatKind` discriminants reuse the
`#[repr(u8)]` numeric assignments pinned in §10's enum
definitions (the normative source: `IntKind { I8=0, ...,
Isize=5 }`, `UintKind { U8=0, ..., Usize=5 }`, `FloatKind
{ F16=0, F32=1, F64=2, F128=3 }`). §12.3's per-`Ty` table reuses
these; D-4-2's per-`Const` table reuses them again.

**Reasoning.** D-4-1.1 pinned `u128` storage on the value side
("smaller widths sign-extend to `u128`") for canonicalisation
determinism — the byte form follows directly. Option 2's
per-width minimal-byte encoding violates "store the result, not
the input" because it requires a decoding rule keyed on the
`IntKind` discriminant; the canonical form should be readable
without conditional logic on width. Option 3 is half-uniform
which is worse than fully-uniform.

The float padding rule was load-bearing in Codex review R5/R6
of D-4-1: bit-pattern occupies low-order bits, high-order bits
are zero for narrow widths (`1.0f32` stores as
`0x3f800000_u128` with the upper 96 bits zero). The byte form
mirrors this directly — first 4 bytes of the `u128` LE encoding
hold the IEEE-754 binary32 bit-pattern; bytes 4..16 are `0x00`.

**Methodology rules invoked.** *Store the result, not the
input;* *information preservation* (every scalar's full bit
content survives unchanged into the byte form); *cross-build
canonicalisation via stable identity*.

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 2 -- `ScalarInt::data: u128`
  is the same widening pattern; the byte form follows.
- `prior-art/mrustc.md` Finding 2 -- `MIR::Constant`'s
  per-scalar variants demonstrate the inline-tag-then-bits
  pattern.
- `prior-art/charon.md` Finding 2 -- `ScalarValue` per-flavor
  variants use the same width-extending discipline.

**Cross-refs.** `spec-snapshot.md` §9.7 (scalar variants
declarations); `spec-snapshot.md` §10 (`IntKind` / `UintKind` /
`FloatKind` `#[repr(u8)]` enum definitions — normative source
for the disc values); `spec-snapshot.md` §12.3 (per-`Ty` table
reuses the same disc values for `Ty::Int(k)`, `Ty::Uint(k)`,
`Ty::Float(k)`).

---

### D-4-2.3 -- Length-prefix width [LOCKED]

**Question.** What integer width encodes the lengths of
`Vec<Byte>`, `Vec<ConstId>`, and `EnumValue.payload`?

**Key options.**

1. `u32` LE (matches §12.3's `GenericArgs` container).
2. `u16` LE — saves 2 bytes per variable-length payload but
   caps at 65 535 entries.
3. varint — saves bytes for small payloads but adds a per-call
   decoding rule.

**Decision.** Option 1 (`u32` LE for all three positions).

**Reasoning.** Matches §12.3's existing precedent ("`u32` LE
arg_count" in the `GenericArgs` container encoding) — uniformity
across the per-`Ty` and per-`Const` walks. `u16` is too tight:
a 256 KB const blob has 262 144 byte entries, exceeding `u16`'s
65 535 cap. `varint` saves marginal bytes for typical payloads
but introduces a decoding rule that breaks §12.3's
"lengths emit as little-endian `u32` prefixes" invariant and
complicates canonical-form auditing.

**Methodology rules invoked.** *Compositional behavior beats
special-case flags* (one length-prefix rule, not three);
*cross-build canonicalisation via stable identity*.

**Prior-art evidence.**

- `prior-art/llvm.md` Finding 7 -- `ConstantDataArray` length
  is fixed-width-encoded in bitcode.
- `prior-art/charon.md` Finding 5 -- `ConstantExprKind::Adt`
  fields use a fixed-width length prefix.
- `prior-art/mrustc.md` Finding 7 -- bincode-style
  serialisation of variable-length payloads uses `u32`-equivalent
  prefixes.
  Cross-project: no project surveyed used varint for canonical-
  key purposes; `u32` LE matches the §12.3 `GenericArgs`
  precedent already pinned in stage-1.

**Cross-refs.** `spec-snapshot.md` §9.7 (per-`Const`-variant
byte-encoding tags `Bytes` / `Aggregate` / `EnumValue` rows);
`spec-snapshot.md` §12.3 (`GenericArgs` container precedent).

---

### D-4-2.4 -- `EnumValue.variant_idx` encoding [LOCKED]

**Question.** How does `variant_idx: VariantIdx` (a `u32`
layout-local index per `spec-snapshot.md` §3 `VariantIdx`
type-alias declaration) lay out in the canonical key, and where
is it placed relative to the `payload` length prefix?

**Key options.**

1. `u32` LE, placed before the `payload` length prefix.
2. `u32` LE, placed after the `payload` length prefix (or
   appended at the end).
3. `u16` LE (variant indices unlikely to exceed 65 535 in
   practice, but no enforced cap).

**Decision.** Option 1 (`u32` LE, before `payload` length).

```text
EnumValue { variant_idx, payload } :
  tag(8)
  + u32 LE variant_idx
  + u32 LE payload_count
  + payload_count * canonical_const(ConstId)
```

**Reasoning.** `VariantIdx` is post-mono-stable (set at HIR
resolution; preserved through monomorphisation). The
placement-before-payload mirrors how `EnumValue` reads as
"first identify the variant, then the payload" — same lexical
order as the field declaration in D-4-1.4 and §9.7's
`Const::EnumValue` declaration (`variant_idx` declared first,
`payload` second). `u32` width
uses the same rationale as D-4-2.3; `u16` would create a
special case (discriminant-on-`Ty::Enum` is `u32`-stored per
§11.1, so the byte form should match).

**Methodology rules invoked.** *Store the result, not the
input;* *cross-build canonicalisation via stable identity*.

**Prior-art evidence.**

- `prior-art/charon.md` Finding 4 -- `Aggregate` carries
  `VariantId(usize)` analogue; the byte-form discipline mirrors
  there.
- `prior-art/regseq-model-old.md` Finding 3 -- `Variant(idx,
  payload)` shape predates this in regseq-model-old; preserves
  the first-identify-variant-then-payload reading.

**Cross-refs.** `spec-snapshot.md` §9.7 (`EnumValue` declaration);
`spec-snapshot.md` §11.1 (`VariantLayout.discriminant` and
`EnumShape.variants` — the consumer-context table that
`variant_idx` indexes into); `spec-snapshot.md` §3.3 step 9
(in-bounds validation).

---

### D-4-2.5 -- `Byte` sub-discriminator + payload layout [LOCKED]

**Question.** Within `Bytes(Vec<Byte>)`'s payload, how is each
`Byte` (the three-state `Uninit | Value(u8) | Reloc(InstanceRef)`)
encoded?

**Key options.**

1. **`u8` sub-disc + payload.** `Uninit = 0` (no payload);
   `Value(b) = 1` + 1 byte; `Reloc(r) = 2` + InstanceRef bytes
   (per D-4-2.8).
2. **Sentinel-based compact.** `0xFE = Uninit`, `0xFF = Reloc`
   (followed by InstanceRef), all other byte values =
   `Value(byte_value_itself)`. Saves 1 byte per `Value`.
3. **Run-length encoded.** Compress runs of `Uninit` and
   `Value` into `(count, value)` pairs.

**Decision.** Option 1.

```text
byte_sub_tag = u8
  0  = Uninit             (no payload)
  1  = Value(u8)          + 1 byte
  2  = Reloc(InstanceRef) + 34 bytes (per D-4-2.8)
```

**Reasoning.** Matches §12.3's tagged-tree convention (1-byte
sub-disc + tag-specific payload). Option 2 saves bytes in
typical-case `Value` runs but `Value(0xFE)` and `Value(0xFF)`
are legal byte values, so the sentinel scheme requires escape
encoding — and the escape encoding rule complicates
canonical-form auditing more than the saved bytes are worth.
Option 3 introduces stateful encoding (run boundaries depend on
neighbouring values), which violates the per-element
independence that the rest of the canonical scheme maintains.

The well-formedness invariant (maximal-relocation-runs are
pointer-shaped, per §9.7's `Byte` enum
"Byte-sequence well-formedness invariants" subsection) is
enforced in §3.3 Phase B step 9, not at encoding time — the
byte form encodes whatever sequence is in `Vec<Byte>`
faithfully; validation rejects ill-formed sequences.

**Methodology rules invoked.** *Store the result, not the
input;* *compositional behavior beats special-case flags;*
*no defensive fallbacks*.

**Prior-art evidence.**

- `prior-art/charon.md` Finding 6 -- `Byte` three-state
  precedent (D-4-1.8); the sub-tag pattern follows from the
  three-state shape directly.
- `prior-art/llvm.md` Finding 7 -- `ConstantDataArray` /
  `ConstantArray` use a tagged constant-element approach,
  cross-checking the per-element-tag pattern.

**Cross-refs.** `spec-snapshot.md` §9.7 (`Byte` enum,
`Bytes(Vec<Byte>)` variant, byte-sequence well-formedness
invariant); `spec-snapshot.md` §3.3 step 9 (validation rules).

---

### D-4-2.6 -- Aggregate / EnumValue recursion + acyclicity [LOCKED]

**Question.** How does `canonical_const(ConstId)` recurse into
nested `ConstId` children, and is recursion termination
validated or assumed?

**Key options.**

1. **Depth-first inline + three-state-DFS-validated acyclicity.**
   Recursion inlines child bytes into parent's stream; Phase B
   step 9 walks the const-graph with three-state DFS marks
   (`Unseen` / `Visiting` / `Done`) and rejects back-edges to
   `Visiting` nodes.
2. **Depth-first inline + assumed acyclicity.** Recursion
   inlines; no validation pass — termination assumed by const-
   eval semantics.
3. **Depth-bounded recursion** (e.g., limit to a configured
   max depth) with the depth bound counted as an invariant.

**Decision.** Option 1.

Recursion shape: for each child `ConstId`, look up the `Const`,
recurse to compute its canonical key, append the resulting
bytes to the parent's stream. The recursion edges are exactly
`Aggregate.0` and `EnumValue.payload`; `Bytes::Reloc` and
`Reloc` carry `InstanceRef` (not `ConstId`), so they do not
extend the const-recursion graph.

Acyclicity validation: extends §3.3 Phase B step 9 with a
**three-state DFS** (`Unseen` / `Visiting` / `Done`) over
`Aggregate` / `EnumValue` payload reachability. Entering a node
sets it to `Visiting`; leaving sets it to `Done`. An edge to a
`Visiting` node is a back-edge (cycle) and is a
model-corruption error; an edge to a `Done` node is skipped
(shared DAG children are legitimate — same canonical-content
child can appear under multiple parents). A single-bit
visited-set is insufficient: it cannot distinguish "currently
on the recursion stack" from "already validated."

Validation-order refinement: §3.3 step 4's `ConstInterner`
content-uniqueness pass is restricted to the type-level subset
(scalar variants `Bool` / `Char` / `Int` / `Uint` / `Float`,
whose canonical-content keys do not recurse through `ConstId`).
Value-level lookup injectivity and canonical-form
well-formedness recompute the canonical keys for value-level
entries — which DOES recurse — and therefore must run AFTER
acyclicity is established. Step 9 is ordered:
(1) `NullPtr` / `Reloc` / `Bytes::Reloc` sub-reference resolution;
(2) `Bytes` / `Aggregate` / `EnumValue` content checks;
(3) const-graph acyclicity via three-state DFS;
(4) value-level canonical-content key recomputation +
    lookup-map injectivity over the value-level subset.
This bounds recursive key computation to the verified DAG,
matching the same defence posture used for §3.3 Phase C's
vtable rebuild duplicate-check.

**Reasoning.** Well-typed Rust + const-eval termination
guarantee acyclicity at construction time — the bridge cannot
produce a cyclic const-graph from any legitimate input. But the
artefact is serialised; downstream stages re-validate the model
without the construction trace. A tampered serialised
`ConstInterner` could embed a cycle that wasn't present at
bridge time, and canonical-key computation would not terminate.
Option 2's "assumed acyclicity" requires trust in the upstream
artefact source; option 1 verifies. The cost is one DFS pass
linear in const-graph size — same defence posture used for
§3.3 Phase C's vtable rebuild duplicate-check.

Option 3 (depth-bounded) introduces a configurable invariant
(what's the depth bound?) that doesn't actually defend against
cycles, only catches them late.

**Methodology rules invoked.** *No defensive fallbacks*
(reject corruption, don't paper over);
*cross-build canonicalisation via stable identity* (acyclicity
is part of the canonicality guarantee).

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 4 -- rustc's const-eval
  query system relies on memoization with cycle detection;
  same fundamental discipline.
- `prior-art/charon.md` Finding 5 -- charon's
  `ConstantExprKind::Adt(_, _, fields)` recurses into
  `Box<ConstantExpr>` children; cycle detection is implicit
  via the construction trace, but charon's construction
  context is closer to ours so the analogue holds.

**Cross-refs.** `spec-snapshot.md` §9.7 (`Aggregate` /
`EnumValue` declarations; `canonical_const(ConstId)` recursion
rule); `spec-snapshot.md` §3.3 Phase B step 9 (acyclicity
sub-check addition).

---

### D-4-2.7 -- `NullPtr(TyId)` type encoding [LOCKED]

**Question.** How does the `TyId` field in `NullPtr(TyId)`
serialise into the canonical key?

**Key options.**

1. **Recursive `canonical_ty(TyId)`.** Resolve via
   `model.types.resolve(...)` and recurse on the resolved `Ty`
   per §12.3's per-IR-identifier termination rule for `TyId`.
2. **Raw `TyId` integer.** Emit the local `u32` index directly.
3. **`def_path_hash`-only** (for nominal types) with structural
   recursion for non-nominal pointee types.

**Decision.** Option 1.

```text
NullPtr(t) : tag(9) + canonical_ty(t)
```

**Reasoning.** `TyId` is a build-local index — emitting it
directly would defeat cross-build stability (two builds with
different `TyId` allocation orders would produce different
keys for `null::<i32>()`). `canonical_ty(t)` produces
cross-build-stable bytes for any `Ty` content. This is the only
`Const` variant whose canonical key directly recurses into a
`Ty` — and it must, because `null::<T>()` ≠ `null::<U>()` is
constitutive of value identity (per D-4-1.6). Option 3 splits
the recursion rule conditionally on `Ty` shape, complicating
the byte form for no benefit since `canonical_ty` already
handles every `Ty` variant uniformly.

**Methodology rules invoked.** *Cross-build canonicalisation
via stable identity;* *compositional behavior beats
special-case flags*.

**Prior-art evidence.**

- `prior-art/llvm.md` Finding 5 -- `ConstantPointerNull` is
  per-pointer-type, confirming type-as-part-of-identity for
  null pointers.
- `prior-art/charon.md` Finding 3 -- `ConstantExprKind`'s
  raw-pointer / null cases similarly carry the pointee type.

**Cross-refs.** `spec-snapshot.md` §9.7 (`NullPtr` variant
declaration; D-4-1.6 rationale); `spec-snapshot.md` §12.3
(per-IR-identifier `TyId` walk-rule row).

---

### D-4-2.8 -- `Reloc(InstanceRef)` byte layout [LOCKED]

**Question.** How does the `InstanceRef` 4-tuple (used by
`Reloc(InstanceRef)` and `Byte::Reloc(InstanceRef)`) lay out
byte by byte?

**Key options.**

1. **Field-order, fixed-width.** Emit each component in §9.7's
   `InstanceRef` struct-declaration order: `regseq_kind_rank`
   (`u8`) + `def_path_hash` (16 bytes) + `args_key` (16 bytes
   via `to_be_bytes()`) + `fn_shim_disambig` (`u8`) = 34 bytes
   total.
2. **Length-prefixed** (in case future `InstanceRef` variants
   widen the field set).
3. **Reordered for alignment** (e.g., 16-byte fields first,
   then `u8` fields).

**Decision.** Option 1.

```text
InstanceRef encoding (34 bytes):
  u8       regseq_kind_rank
  16 bytes def_path_hash       (StableCrateId u64 LE + local_hash u64 LE
                                per §12.3 per-IR-identifier rule)
  16 bytes args_key            (`ArgsKey.0.to_be_bytes()` — the canonical
                                big-endian `u128` serialisation; native
                                memory layout / `transmute` MUST NOT be
                                used)
  u8       fn_shim_disambig
```

The `args_key` field is emitted as `ArgsKey.0.to_be_bytes()`
(the standard big-endian `u128` serialisation). This is the
documented BE exception to §12.3's general
"multi-byte integers are little-endian" rule, anchored on the
BE-load convention in §12.3 (`ArgsKey::EMPTY.0` is "loaded as
a big-endian u128"). Since the serialised model stores
`ArgsKey` only as a `u128`, the canonical byte form must be
specified by operation on the `u128` value (not by appeal to
"underlying bytes," which leaves a native-endian /
`transmute` interpretation open). `to_be_bytes()` reproduces
the original SHA-256-leading-16-bytes sequence that the bridge
computed before storing into `ArgsKey.0`.

**Reasoning.** Field-order matches the struct declaration —
auditors can verify the byte form by reading the declaration
top-to-bottom. Length-prefixing (Option 2) is a no-forward-
looking-placeholder violation: there is no plan for
`InstanceRef` to widen, and the spec is closed at 4 fields.
Reordering for alignment (Option 3) produces a byte form
inconsistent with the source-level field order, which makes
auditing harder.

The `args_key`-as-`to_be_bytes()` choice resolves the
tension between §12.3's two endianness statements (the BE-load
convention for `ArgsKey.0` vs the LE-default for multi-byte
integers): the byte stream emits `ArgsKey.0.to_be_bytes()` as
the documented BE exception, anchored on the same BE-load
convention §12.3 already pins for `ArgsKey::EMPTY`.

**Methodology rules invoked.** *Store the result, not the
input;* *no forward-looking placeholders* (no length prefix
for unbounded extensibility); *cross-build canonicalisation
via stable identity*.

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 8 -- rustc's `DefPathHash`
  StableCrateId + local_hash decomposition is the precedent
  for the 16-byte `def_path_hash` layout; reused here.
- `prior-art/README.md` §3.1 -- 5-of-5 non-rustc projects fail
  cross-build determinism; rustc's `HashStable for AllocId`
  (the resolve-then-hash discipline) is the only adjacent
  precedent. D-4-1.5's `InstanceRef` 4-tuple already embeds
  this discipline at the value-shape level; D-4-2.8 pins the
  byte-form serialisation that preserves it (each component
  emits its own cross-build-stable byte form;
  `args_key.to_be_bytes()` is the resolve-then-hash result).

**Cross-refs.** `spec-snapshot.md` §9.7 (`InstanceRef` struct
declaration; `Reloc` and `Byte::Reloc` variants);
`spec-snapshot.md` §12.3 (`def_path_hash` byte form;
`ArgsKey::EMPTY` definition; per-IR-identifier termination
rules).

---

### D-4-2.9 -- `ZST` identity (tag-only) [LOCKED]

**Question.** Does `ZST` carry any payload bytes in the
identity key?

**Key options.**

1. **Tag-only** (no payload bytes).
2. **Tag + sentinel byte** (e.g., `0x00` to make every variant
   carry at least one payload byte).
3. **Tag + consumer-`TyId`** (would force ZST to behave like
   `NullPtr(TyId)`).

**Decision.** Option 1.

```text
ZST : tag(5)         (single byte, no payload)
```

**Reasoning.** D-4-1.2 pinned single canonical `ZST` (no
payload — type from consumer context); D-4-1.7 pinned
inline-only with no outer wrapper. The byte form follows
directly: variant tag is the entire encoding.

Option 2 (sentinel byte) buys nothing — the variant tag
already disambiguates `ZST` from every other variant. Option
3 (consumer-`TyId`) reintroduces the type-tag wrapper that
D-4-1.7 explicitly rejected, undoes the dedup-explosion
mitigation from D-4-1.2 (one canonical `ZST` for all
zero-sized types), and would break the niche-preservation
discipline (D-4-1's contract that no variant payload widens
`ConstId`).

**Methodology rules invoked.** *Compositional behavior beats
special-case flags;* *cross-build canonicalisation via stable
identity*.

**Prior-art evidence.**

- `prior-art/charon.md` Finding 7 -- charon collapses ZSTs
  similarly; the byte-level analogue is tag-only.

**Cross-refs.** `spec-snapshot.md` §9.7 (`ZST` variant
declaration; D-4-1.2 single-canonical rationale; D-4-1.7
inline-only rationale).

---

### D-4-3 -- Reference-typed const handling: `SyntheticItemKind::ConstMemoryStatic` family [LOCKED]

**Question.** How does the bridge represent reference-typed
const values whose storage is backed by an anonymous
`GlobalAlloc::Memory` allocation (`&str` / `&[T]` literals,
plus thin-pointer references into anonymous memory) such that
two builds with the same source produce byte-identical
canonical content for the resulting `Const` value?

**Key options.**

1. **Option A -- bridge-synthesised content-keyed Static.**
   Synthesise an immutable `Static` Item per unique memory-
   content blob, with a content-derived `DefPathHash`.
   Reference-typed const values then compose from D-4-1's
   existing variants: wide-pointer references become
   `Aggregate([Reloc, Uint(Usize, len)])`; thin-pointer
   references become `Reloc(synth_static_inst_ref)`.
   Structurally parallel to vtable instance synthesis (§13.2 /
   §13.3 / §13.4).
2. **Option B -- new top-level `Const` variant for memory-
   blobs.** Add a `MemoryConst { bytes: Vec<Byte>, meta: u64 }`
   variant that inlines the content directly.
3. **Option C -- bridge-rejection.** Continue rejecting
   `GlobalAlloc::Memory` arms; reference-typed consts remain
   permanently out of scope.

**Decision.** Option A. Closed across ten sub-decisions
D-4-3.1 through D-4-3.10 pinning the family name and payload,
the `ContentHash` construction, the §13.3 enum extension, the
§13.4 segment / DefPathHash construction, the bridge synthesis
rule, the synthesised Static's `Ty` and mutability, the bridge
dispatch arms (with explicit non-zero-offset rejection), the
§3.3 Phase B step 9 dedup check, and the closure of the
outbound expectation in `stage-12/expectations.md`.

**Reasoning.**

*Why not Option B (new `Const` variant).* The existing
11-variant `Const` set (D-4-1) is closed and exhaustive over
post-mono `mir::ConstValue` shapes. Adding a `MemoryConst`
variant duplicates `Const::Bytes` (which already encodes
arbitrary byte sequences with init/provenance) and wide-
pointer values (which `Aggregate([Reloc, Uint])` already
expresses compositionally). It violates D-4-1.9's first-class-
aggregate decision retroactively — D-4-1.9 was made *because*
this case was on the horizon. A new variant adds an
identity-key encoding obligation (D-4-2 would need extending)
for no signal that doesn't already live elsewhere.

*Why not Option C (bridge-rejection).* Every Rust program
containing a string literal, byte slice, or `&[T]` literal
hits the `GlobalAlloc::Memory` arm. Permanent rejection makes
the bridge unusable for any real driver code. Rejection is
the wrong methodology answer here — the no-defensive-fallbacks
rule applies to *partially-handled* cases (where the bridge
might silently widen), not to *known-essential* cases that
need a closed encoding.

*Why bridge-synthesised content-keyed Static (Option A).*
Three voices: (i) the vtable family at §13.2 / §13.3 / §13.4
already solves the structurally-parallel problem (anonymous
bridge-fabricated Items needing cross-build-stable identity);
the same pattern fits memory blobs uniformly. (ii) Content-
keyed dedup means two bytes-identical allocations across
builds (or even within one build, e.g., the same string
literal appearing in two crates) collapse to one synth Item —
which is what consumers expect. (iii) Composition over D-4-1's
existing variants means D-4-3 is mechanical extension of the
established axes (one new `SyntheticItemKind` variant; one new
§13.4 family row; one new §13.4 payload-bytes row; revised
bridge dispatch arms; one new §3.3 step 9 sub-bullet) — no new
design space.

The non-zero-offset edge case (`Indirect { alloc_id, offset != 0 }`
over `GlobalAlloc::Memory`) is **bridge-rejected** with a
no-defensive-fallbacks rule rather than handled in v1: drivers
in practice don't produce these (named statics carry shared
structures where field-pointers might be needed); and
extending `InstanceRef` with an offset field would be a
substantial change to a stage-4-locked `InstanceRef` shape (D-4-1.5 / §9.7). The case is
rejected unless a future spec revision admits it; no slot is
reserved in the dispatch table or `InstanceRef` shape.

*Why whole-family scope (stage-4 + stage-12 in one decision).*
The two halves are tightly coupled: D-4-3's stage-4
contribution (bridge dispatch arms producing
`Aggregate([Reloc(synth_static_inst_ref), Uint])`) names a
`synthesised_static_inst_ref` that the §13.3 / §13.4 family
extension must produce. Splitting across stages leaves
forward-looking placeholders in the spec until stage 12 closes
the gap, violating the no-forward-looking-placeholders rule.
The §13 changes are mechanical mirror of the vtable family
precedent — one new enum variant, one new segments row, one
new payload-bytes row — and don't merit independent stage-12
framing time. `stage-12/expectations.md:357-375` already
admits this path.

**Methodology rules invoked.**

- *Store the result, not the input.* The synthesised Item
  carries `content_hash` (the cross-build-stable identity);
  the bridge does not store the rustc `AllocId`.
- *Cross-build canonicalisation via stable identity.*
  `ContentHash` is computed via the established
  SHA-256-truncated hash family (parallel to `ArgsKey` /
  `ProjectionBindingsKey`); the synthesised `DefPathHash` is
  computed per §13.4's existing rule with the new family's
  canonical payload bytes.
- *No defensive fallbacks.* Non-zero offsets and
  `feature(type_id)` are rejection-enforced; the bridge raises
  a diagnostic naming the rustc shape, never silently widens.
- *No forward-looking placeholders.* Non-zero offset handling
  is **rejected** rather than reserved; no slot in the
  dispatch table or `InstanceRef` shape is reserved for a
  future extension. If the case ever becomes load-bearing, a
  future spec revision admits it append-only.
- *Compositional behavior beats special-case flags.*
  `Aggregate([Reloc, Uint])` falls out of D-4-1.9's first-
  class-aggregate decision; no new variant.

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 9 -- `mir::ConstValue::Slice`
  and `Indirect`-via-`GlobalAlloc::Memory` are the rustc
  shapes that need closure. The bridge-rejection of `Memory`
  in D-4-1 was justified by deferral; D-4-3 closes the
  deferral.
- `prior-art/charon.md` Finding 8 -- charon synthesises
  per-content statics (`StaticGlobalDecl` family) for
  reference-typed constants; closest peer's design and the
  precedent for content-keyed synthesis.
- `prior-art/llvm.md` Finding 6 -- LLVM's
  `internal constant` global per string literal is the
  C-codegen-side counterpart of D-4-3's synthesised Static
  (downstream codegen lowers `ConstMemoryStatic` to exactly
  this form).
- `prior-art/README.md` §3.1 -- 5-of-5 non-rustc projects
  fail cross-build determinism; rustc's `HashStable` /
  resolve-then-hash discipline is the only adjacent
  precedent. D-4-3 inherits the same discipline (resolve
  the anonymous AllocId to its memory-content target before
  hashing).
- `prior-art/regseq-model-old.md` Finding 5 -- old
  `LiteralValue` had no string-literal handling; stage-4
  closes via the synthesised-Static path rather than
  introducing a new `LiteralValue` variant (which would
  duplicate D-4-1.9's compositional aggregate).

**Worked examples.** Pinned in sub-decisions D-4-3.6 through
D-4-3.8; assembled into `spec-snapshot.md` §9.7's
"Reference-typed const examples" block within the new
"Reference-typed const handling (D-4-3)" subsection.

**Cross-refs.** `spec-snapshot.md` §9.7 ("Reference-typed
const handling (D-4-3)" subsection; bridge dispatch table
`Memory(memory)` row; bridge dispatch table `Slice` /
`Indirect`-over-Memory rows); `spec-snapshot.md` §13.3
(`ContentHash` type; `SyntheticItemKind::ConstMemoryStatic`
variant); `spec-snapshot.md` §13.4 (const-memory family
segments + DefPathHash construction); `spec-snapshot.md`
§3.3 Phase B step 9 (`ConstMemoryStatic` dedup sub-bullet);
`stage-12/expectations.md` "From stage 4" section (closed by
D-4-3); `status.md` "In-flight decisions" / "Open decisions
(post-D-4-3)"; `review-log.md` D-4-3 rounds.

---

### D-4-3.1 -- Family name + payload shape [LOCKED]

**Question.** What name and payload does the new
`SyntheticItemKind` variant carry?

**Key options.**

1. `ConstMemoryStatic { content_hash: ContentHash }` (the
   proposed name from the D-4-1 deferred subsection).
2. Rename (e.g., `MemoryBlobStatic`, `ConstAllocStatic`).
3. Extended payload (carry the byte length or a
   `Ty::Array(u8, N)` reference inline).

**Decision.** Option 1.

```rust
ConstMemoryStatic { content_hash: ContentHash },
```

**Reasoning.** The proposed name is already in three spec
locations + `stage-12/expectations.md`; rename costs cross-ref
churn for no semantic benefit. "ConstMemoryStatic" reads
correctly: an immutable Static synthesised to back a
reference-typed const. Payload = just `content_hash` because
the *identity* of the synth Item is the content; the byte
length and `Ty::Array(u8, N)` are *consequences* of the
content (length is computed from `Allocation.bytes.len()`;
`Ty` is computed from length), not part of identity. Same
discipline as `VTableInstance { principal, self_ty }` carrying
only identity-bearing fields and computing the `vtable_ty`
shape from those.

**Methodology rules invoked.** *Store the result, not the
input;* *compositional behavior beats special-case flags*.

**Prior-art evidence.**

- `prior-art/charon.md` Finding 8 -- charon's static-global
  synthesis carries content as a payload; identity is by
  declaration shape. Stage-4 inverts: identity is by content
  hash; content is on the resolved Static's `init`.

**Cross-refs.** `spec-snapshot.md` §13.3
(`SyntheticItemKind::ConstMemoryStatic` variant declaration;
`ContentHash` type definition).

---

### D-4-3.2 -- `ContentHash` construction [LOCKED]

**Question.** What is the canonical hash construction for
`ContentHash`?

**Key options.**

1. SHA-256 truncated to leading 16 bytes BE-loaded into
   `u128`, identical to `ArgsKey` / `ProjectionBindingsKey`
   (the established stage-4 hash family).
2. Full SHA-256 (32 bytes).
3. Non-cryptographic (SipHash, FxHash, etc.).

**Decision.** Option 1.

```rust
pub struct ContentHash(pub u128);
```

Canonical input = the canonical D-4-2
`Const::Bytes(Vec<Byte>)` byte form of the memory allocation
per §9.7's per-`Const`-variant byte-encoding tags (variant
tag(6) + u32 LE byte_count + per-Byte sub-tag-encoded payload,
including any internal `Byte::Reloc(InstanceRef)` for
relocation-bearing memory).

**Reasoning.** Matches `ArgsKey` and `ProjectionBindingsKey`
— one hash family in the stage; consumers know the collision
model (§13.3 honesty note: < 2^-50 collision probability for
realistic driver-crate volume). Reusing D-4-2's `Bytes` byte
form means `ContentHash` naturally captures all three sources
of allocation distinctness uniformly: (a) initialised payload
bytes; (b) `init_mask` ranges (encoded as `Byte::Uninit`
runs); (c) `provenance` entries (encoded as
`Byte::Reloc(InstanceRef)` runs). No new canonicalisation is
introduced.

**Methodology rules invoked.** *Cross-build canonicalisation
via stable identity;* *compositional behavior beats
special-case flags*.

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 8 -- `HashStable for AllocId`
  resolves to content before hashing; ContentHash inherits
  the same discipline.
- `prior-art/charon.md` Finding 8 -- per-content statics
  use a hash-of-content key (charon's `static_id` is
  content-derived).

**Cross-refs.** `spec-snapshot.md` §13.3 (`ContentHash`
canonical encoding subsection; collision honesty note);
`spec-snapshot.md` §9.7 (per-`Const`-variant byte-encoding
tags `Bytes` row -- the canonical input grammar).

---

### D-4-3.3 -- §13.3 `SyntheticItemKind` variant placement [LOCKED]

**Question.** Where in the `SyntheticItemKind` enum does
`ConstMemoryStatic` go?

**Decision.** Append after `VTableDropShim`, per the §13.3
append-only stability rule.

**Reasoning.** Append-only is the §13.3 invariant: existing
discriminants are immutable so cross-build hashes for
already-admitted synth Items never change. The §13.4
`synthetic_kind_discriminant` byte in the canonical payload
(per `spec-snapshot.md`'s "synth.local_hash = ..." rule)
depends on this stability; reordering would break every
existing synth Item's `DefPathHash`.

**Methodology rules invoked.** *No forward-looking
placeholders;* *cross-build canonicalisation via stable
identity*.

**Prior-art evidence.**

- §13.3's existing append-only rule at
  `spec-snapshot.md` directly applies to D-4-3.

**Cross-refs.** `spec-snapshot.md` §13.3
(`SyntheticItemKind` enum -- the append-only stability
contract is the relevant rule).

---

### D-4-3.4 -- §13.4 segments for the const-memory family [LOCKED]

**Question.** What's the canonical `DefPath.segments` form
for `ConstMemoryStatic` Items?

**Key options.**

1. Three segments: marker + family-name + content-hash hex
   (mirroring `VTableType`'s depth).
2. Two segments: marker + content-hash hex (no family-name
   segment).
3. Four segments: marker + "const" + "memory_static" +
   content-hash hex (split for matching the closure family's
   deeper hierarchy).

**Decision.** Option 1.

```text
ConstMemoryStatic family
  segments = [
    TypeNs("<regseq_synth>"),
    TypeNs("const_memory_static"),
    TypeNs(<content_hash hex>),
  ]
```

**Reasoning.** Matches the vtable family's depth and pattern.
The "const_memory_static" segment is the family discriminator
— auditors reading segments alone can identify the synth kind
without parsing the trailing hex. A two-segment form would be
ambiguous if future synth families share `<regseq_synth>` +
hex without a family-name segment (the hex alone is not
self-describing). A four-segment form is over-elaboration —
the closure family's depth comes from its ClosureCallKind sub-
discriminator, which has no analogue here.

**Methodology rules invoked.** *Compositional behavior beats
special-case flags;* *cross-build canonicalisation via stable
identity*.

**Prior-art evidence.**

- `prior-art/llvm.md` Finding 6 -- LLVM's
  `@.str.<index>` / `@__const.<fn>.<name>` naming convention
  uses a family-discriminator-then-identity pattern. D-4-3.4
  mirrors this structurally.

**Cross-refs.** `spec-snapshot.md` §13.4 (synthetic
`DefPath.segments` encoding; vtable family precedent).

---

### D-4-3.5 -- §13.4 DefPathHash canonical payload bytes [LOCKED]

**Question.** What canonical payload bytes feed the family-
specific SHA-256 input for `ConstMemoryStatic`?

**Key options.**

1. Just the 16-byte content hash
   (`content_hash.0.to_be_bytes()`).
2. Add the byte length explicitly (16 + 8 bytes).
3. Add the original Rust element type (e.g., `u8`, `u32`)
   as a field tag.

**Decision.** Option 1.

```text
ConstMemoryStatic
  ++ ContentHash bytes (16 B; equivalently
     content_hash.0.to_be_bytes() per §13.3 ContentHash byte form)
```

**Reasoning.** The content hash is the identity by
construction; the byte length is content-disambiguated by
SHA-256 already (two allocations of different lengths can't
collide except probabilistically below the threat-model
bound). Adding the byte length would duplicate. The original
Rust element type isn't part of identity (D-4-3.7's type-
erase-to-bytes rationale); two allocations with identical
bytes share the synth Static regardless of element type.

**Methodology rules invoked.** *Store the result, not the
input;* *no forward-looking placeholders*.

**Prior-art evidence.**

- §13.4's existing per-family payload bytes pattern
  (`spec-snapshot.md`'s VTableInstance / VTableMethodShim
  rows) is the direct precedent.

**Cross-refs.** `spec-snapshot.md` §13.4 (synthetic
`DefPathHash` construction; per-family canonical payload
bytes table -- D-4-3 row).

---

### D-4-3.6 -- Bridge synthesis rule (lazy) [LOCKED]

**Question.** When does the bridge create a `ConstMemoryStatic`
Item, and what storage does it carry?

**Key options.**

1. **Lazy.** When the bridge dispatches a
   `mir::ConstValue::Slice` / `Indirect`-over-Memory arm, look
   up the underlying `Allocation`, compute its canonical D-4-2
   `Bytes` byte form, hash to get `ContentHash`, look up an
   existing `ConstMemoryStatic { content_hash }` in
   `item_synthetic_kinds`; if absent, synthesise a new Static
   Item.
2. **Eager.** Pre-walk all `GlobalAlloc::Memory` entries
   upfront, before the const dispatch.

**Decision.** Option 1 (lazy). Synthesised Item carries:

- `item.kind = ItemKind::Static`
- `item_def_paths[item]` synthesised per §13.4
- `item_synthetic_kinds[item] = ConstMemoryStatic { content_hash }`
- A corresponding `Instance` with
  `kind = InstanceKind::Static { ty: array_ty, init: Some(c),
  mutability: Mutability::Not }`, where `array_ty` is the
  `TyId` for `Ty::Array(Ty::Uint(U8), N)` with
  `N = memory.bytes.len() as u64` (resolved against the
  `TyInterner`; created if absent), and `c` is the `ConstId`
  for the `Const::Bytes(Vec<Byte>)` computed from the
  allocation in step 2 of the lazy lookup. `init: None` is
  invalid for `ConstMemoryStatic` Items (rejected in §3.3
  Phase B's `ConstMemoryStatic` immutable-statics rule).

(Per the stage-1 Item schema, per-kind rich data — including
the `Static`'s `ty` and `init` — lives on the `Instance`, not
the `Item`. The `Item` itself carries only `def_path`, `kind`,
`span`, flags, and `opaque_handle_eligible`. The
`Ty::Array(U8, N)` shape and the `Const::Bytes` initialiser
live on the corresponding `Instance.kind`.)

**Reasoning.** Lazy synthesis matches closure / vtable family
precedent (the bridge's existing pattern is to synth Items
on-demand during dispatch, not pre-emptively). Eager would
require walking rustc's full alloc table upfront, which
doesn't compose with rustc's lazy `tcx.global_alloc(id)` query
model and conflicts with the bridge's existing per-Item
discovery walk. Dedup-by-`ContentHash` lookup means lazy
synthesis still dedupes correctly across all dispatch sites:
two arms with the same content land at the same `ContentHash`
and reuse the existing synth Item.

**Methodology rules invoked.** *Compositional behavior beats
special-case flags* (one synthesis pathway, not separate
eager / lazy modes); *cross-build canonicalisation via stable
identity*.

**Prior-art evidence.**

- `prior-art/charon.md` Finding 8 -- charon's static-global
  synthesis is also lazy / on-demand.
- `prior-art/rustc.md` Finding 9 -- rustc's
  `tcx.eval_static_initializer` and `tcx.global_alloc` are
  query-driven (lazy by default); D-4-3.6 fits this model.

**Cross-refs.** `spec-snapshot.md` §9.7 ("Reference-typed
const handling (D-4-3)" subsection -- bridge synthesis rule
steps 1-5).

---

### D-4-3.7 -- Synthesised Static's Ty + Mutability [LOCKED]

**Question.** What `Ty` and `Mutability` does the synthesised
Static carry?

**Key options.**

1. `Ty::Array(u8, N)` + `Mutability::Not`.
2. The original Rust element type (e.g., `Ty::Array(u32, M)`
   for `&[u32]`).
3. `Ty::Array(u8, N)` + mutability inherited from the rustc
   allocation's flags.

**Decision.** Option 1.

```text
ty:         Ty::Array(Ty::Uint(U8), N)    where N = memory.bytes.len() as u64
mutability: Mutability::Not
```

**Reasoning.** *Type-erasure to bytes.* The Static is content-
keyed — its identity is the bytes, not the original element
type. Two Memory allocations with identical bytes (e.g., the
same byte sequence appearing as `&[u8]` and as `&str`) must
dedup to one synth Static, which only works if the Static is
type-erased to bytes. The wrapping `Aggregate([Reloc, Uint])`
(for `Slice` arms) carries the typed-pointer side; the Static
itself doesn't need to "be" the element type. C codegen
lowers as `static const uint8_t mem_<hash>[N] = { ... };` —
natural fit.

*Mutability.* Reference-typed consts are by definition
immutable in Rust source (`&str`, `&[T]`, `&dyn Trait`).
Mutable references can never be const. The synth Static's
mutability is a synthesis-time choice tied to the
reference-typed-const semantics, not a passthrough from
rustc's allocation flags. Matches `VTableInstance` precedent
(also `Mutability::Not`).

**Methodology rules invoked.** *Store the result, not the
input;* *no defensive fallbacks* (mutability is fixed by
reference-typed-const semantics, not derived from
allocation flags that might drift).

**Prior-art evidence.**

- `prior-art/llvm.md` Finding 6 -- LLVM's
  `private constant [N x i8]` for string literals is exactly
  the C-codegen-side analogue of D-4-3.7's chosen form.
- §13.2's `VTableInstance` precedent (`spec-snapshot.md:798`)
  -- `Mutability::Not` is the established discipline for
  bridge-synthesised statics.

**Cross-refs.** `spec-snapshot.md` §9.7 ("Reference-typed
const handling (D-4-3)" -- bridge synthesis rule step 4
specifying `Ty::Array(u8, N)` and `Mutability::Not`);
`spec-snapshot.md` §13.2 (`VTableInstance` immutability
precedent).

---

### D-4-3.8 -- Bridge dispatch arms (and offset handling) [LOCKED]

**Question.** How do `mir::ConstValue::Slice { data, meta }`
and `mir::ConstValue::Indirect { alloc_id, offset }`-over-
Memory arms map to RegseqModel `Const`?

**Key options.**

1. **Slice → wide-pointer Aggregate; Indirect-thin → bare
   `Reloc`.** Slice carries explicit `meta`; Indirect over
   sized pointee is a thin pointer.
2. Both produce `Aggregate([Reloc, Uint])`.
3. Add a new top-level `Const` variant.

**Decision.** Option 1.

```text
mir::ConstValue::Slice { data, meta }                      ->  Aggregate([
                                                                   Reloc(synth_static_inst_ref(data)),
                                                                   Uint(Usize, meta),
                                                               ])
mir::ConstValue::Indirect { alloc_id, offset = 0 }         ->  Reloc(synth_static_inst_ref(alloc_id))
  over GlobalAlloc::Memory, sized pointee Ty
mir::ConstValue::Indirect { alloc_id, offset != 0 }        ->  bridge-rejected
  over GlobalAlloc::Memory
```

**Reasoning.** *Wide vs thin pointer split.* `Slice { data,
meta }` is rustc's post-mono representation for `&str` /
`&[T]` — explicitly wide-pointer-shaped (data half + meta
half). `Indirect { alloc_id, offset = 0 }` over sized pointee
is rustc's representation for `&[T; N]` / `&T` (thin pointer).
The distinction is real at the value level — wrapping both in
`Aggregate([Reloc, Uint])` would force-fabricate a `meta`
field for thin pointers (no rustc source for it), violating
*store the result, not the input*. Option 1 distinguishes
them per their actual semantics.

*Non-zero offset rejection.* Non-zero offsets into Memory
allocations occur when const-eval produces a pointer into the
middle of a larger anonymous allocation (e.g., `&FOO.field`
where `FOO` is itself anonymous-memory-backed). Drivers in
practice don't produce these — driver code uses named statics
for shared structures, so any "field of an anonymous memory
blob" pattern would produce a `GlobalAlloc::Static`, not
`GlobalAlloc::Memory`. Rejecting at the bridge with a
diagnostic preserves the no-defensive-fallbacks discipline;
extending `InstanceRef` with an offset field would be a
substantial change to a stage-4-locked `InstanceRef` shape (D-4-1.5 / §9.7) for a case
that may never become load-bearing. The case is rejected
unless a future spec revision admits it; no slot is reserved
in the dispatch table or `InstanceRef` shape.

**Methodology rules invoked.** *Store the result, not the
input;* *no defensive fallbacks;* *no forward-looking
placeholders* (the rejection is a hard rejection, not a
reserved slot).

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 9 -- `mir::ConstValue::Slice`
  and `Indirect`-over-Memory are the two distinct rustc
  shapes; D-4-3.8 mirrors the distinction.
- `prior-art/charon.md` Finding 8 -- charon's
  `Const::Slice` / `Const::Ref` distinction parallels Slice
  / Indirect-thin.

**Cross-refs.** `spec-snapshot.md` §9.7 (bridge dispatch
table -- `Slice` row, `Indirect`-over-Memory rows including
non-zero offset rejection; "Reference-typed const handling
(D-4-3)" subsection -- worked examples).

---

### D-4-3.9 -- §3.3 Phase B step 9 dedup check [LOCKED]

**Question.** Does the spec need a new Phase B step 9 sub-
bullet for `ConstMemoryStatic` dedup, or is it covered by
existing checks?

**Key options.**

1. New sub-bullet ordered after the value-level lookup
   injectivity pass: recompute `ContentHash` from each synth
   Static's `init` and verify it matches the stored
   `content_hash` field; verify no two `ConstMemoryStatic`
   Items share a `content_hash`.
2. Rely on existing checks (lookup injectivity over
   `item_synthetic_kinds` should suffice).
3. Move the dedup check to Phase C (alongside the vtable
   rebuild).

**Decision.** Option 1.

```text
- ConstMemoryStatic dedup (D-4-3). For every Item I with
  item_synthetic_kinds[I] = ConstMemoryStatic { content_hash },
  recompute ContentHash from the resolved Static's
  init: Some(c) content (where c is the ConstId for the
  Const::Bytes(Vec<Byte>) value; init: None is rejected by
  the ConstMemoryStatic immutable-statics rule earlier in
  this Phase B pass) per the canonical D-4-2 Const::Bytes
  byte form per §9.7's per-Const-variant byte-encoding tags,
  and verify it matches the stored content_hash field;
  verify no two ConstMemoryStatic Items share a content_hash.
  (Ordered after the value-level lookup injectivity pass --
  the recomputation depends on canonical Const::Bytes bytes
  for the synth Static's init.)
```

**Reasoning.** *Why a new sub-bullet.* Existing step 9 checks
(value-level lookup injectivity over `ConstInterner`; sub-
reference resolution for `Reloc` / `Bytes::Reloc`) cover the
*Const* level — they ensure no two `Const` values share a
canonical key. They do *not* cover the *Item* level — they
don't verify that two anonymous memory allocations with
identical content collapse to one synth Item. That's a
separate invariant: `ConstMemoryStatic`'s `content_hash`
field is the identity, and content-equal allocations must
produce content-equal `content_hash` values, which must
land at the same Item.

*Why ordered after value-level lookup injectivity.* The
recomputation step needs to read the canonical bytes of each
synth Static's `init: ConstId`, which is a value-level
`Const::Bytes` entry. Reading is safe only after the value-
level pass has verified canonical-form well-formedness for
those entries.

*Why not Phase C.* Phase C is for transient-map rebuild
(vtables, etc.); the `ConstMemoryStatic` dedup is a
content-uniqueness invariant, structurally a Phase B check.

**Methodology rules invoked.** *No defensive fallbacks*
(reject corruption rather than silently accept duplicate
content-hash Items); *cross-build canonicalisation via stable
identity*.

**Prior-art evidence.**

- §3.3 step 9's existing value-level pass (D-4-2)
  established the recomputation-and-match pattern; D-4-3.9
  applies it at the Item level.
- §3.3 Phase C step 10's vtable-rebuild duplicate-rejection
  pattern is the structural analogue (reject duplicate
  principal keys for `VTableType`); D-4-3.9 rejects
  duplicate `content_hash` for `ConstMemoryStatic`.

**Cross-refs.** `spec-snapshot.md` §3.3 Phase B step 9
(`ConstMemoryStatic` dedup sub-bullet); `spec-snapshot.md`
§13.3 (`ContentHash` definition); `spec-snapshot.md` §9.7
(per-`Const`-variant byte-encoding tags -- the canonical
`Const::Bytes` byte form recomputed in this check).

---

### D-4-3.10 -- Stage-12 `expectations.md` closure [LOCKED]

**Question.** How does `stage-12/expectations.md`'s outbound
"From stage 4" entry get marked closed?

**Decision.** Edit `stage-12/expectations.md`'s "From stage 4"
section to flip the `SyntheticItemKind::ConstMemoryStatic`
family bullet from open / pending to "closed by D-4-3 in
stage 4," with cross-refs to the spec sections that landed
(§13.3 enum extension; §13.4 segments + DefPathHash; §9.7
dispatch arms; §3.3 step 9 dedup check).

**Reasoning.** Bookkeeping; not a design decision. The whole-
family scope (per the Phase A sign-off) means D-4-3 closes
both the stage-4 and stage-12 halves; the outbound expectation
recording the gap is no longer accurate. Updating preserves
the audit trail per the project-wide convention
(`../README.md` "Forward expectations live in
`stage-N/expectations.md`").

**Methodology rules invoked.** None applicable — audit-trail
bookkeeping. (The project-wide outbound-expectations
convention at `../README.md` is the relevant rule, not a
stage-4 methodology rule.)

**Prior-art evidence.** None applicable -- bookkeeping rule.

**Cross-refs.** `stage-12/expectations.md` "From stage 4"
section; `discussions.md` D-4-3 parent (Cross-refs).

---

### D-4-K -- `pretty.rs::render_const` renderer-style decision: defer to stage 14 [LOCKED]

**Question.** What rendering style does `pretty.rs::render_const`
adopt, and at which stage is the renderer filled?

**Key options.**

1. **Option A -- defer to stage 14.** Update the stub message
   to `"stage 14 fills render_const (deferred per D-4-K)"`;
   add an outbound expectation in `stage-14/expectations.md`
   (alongside the existing `render_instance` outbound);
   record the deferral in this audit entry. No spec rendering-
   style call is made at stage 4. Stage 14 is the latest of
   `render_const`'s dependencies (stage 12 for
   `render_def_path`; stage 13 for `render_ty`; stage 14 for
   `render_instance`); landing the renderer there ensures all
   transitive dependencies are filled.
2. **Option B -- land a debug-grade renderer at stage 4.**
   Produce structural output that does not recurse into
   `render_ty` / `render_def_path(ItemId)` / `render_instance`
   (e.g., `Const::Int(I32, 0xfffffffffffffffffffffffffffffff9)`
   for `-7i32`; `Const::Reloc(Static @ defhash=<hex>, args=
   <hex>, shim=0)` for relocations). Useful for stage-4-
   internal snapshot tests; throwaway once stage 14 fills its
   own `render_const`.
3. **Option C -- land a complete renderer at stage 4** by
   having `render_const` call into stub `render_ty` /
   `render_def_path(ItemId)` / `render_instance`. Doesn't
   compose -- those stubs panic, so `render_const` would
   panic transitively for almost every input.
4. **Option D -- defer to stage 13.** Stage 13 owns
   `render_ty`, so the rendering-style decision could co-locate
   there. But `Reloc(InstanceRef)` depends on `render_instance`
   (stage 14); a stage-13 `render_const` would still panic
   transitively in the `Reloc` arm -- the same Option-C
   failure mode for one variant. Splitting "stage 13 owns
   non-Reloc; stage 14 closes Reloc arm" leaves the renderer
   half-implemented through the gap; not preferred.

**Decision.** Option A (defer to stage 14).

**Reasoning.**

*Why not Option C.* Mechanically broken. `render_ty`
(in §18 of the spec / `pretty.rs::render_ty`),
`render_def_path(ItemId)` (`pretty.rs::render_def_path`), and
`render_instance` (`pretty.rs::render_instance`) all
`unimplemented!()`; calling into them would panic for every
`Const` variant except the 5 self-contained scalars
(`Bool` / `Char` / `Int` / `Uint` / `Float`).

*Why not Option B.* The debug-grade form is a temporary
artefact. Once stage 14 fills `render_const`, the natural
`render_const` becomes the Rust-source-style form that recurses
into `render_ty` / `render_def_path` / `render_instance` (e.g.,
`5i32` / `(1, 2, 3)` / `Foo { a: 5 }` /
`null::<*const i32>()`). A debug-grade renderer at stage 4
would be replaced wholesale; the stage-14 fill would not
incrementally extend it. Methodology cost: the debug-grade
form would invite snapshot tests during stage-4 implementation
work, which would then need migration when stage 14 lands the
real renderer. The store-result-not-input rule applies: store
the *spec-blessed* renderer (stage 14's), not an interim
debugging facility.

*Why not Option D.* Stage 13's `render_ty` is one of
`render_const`'s three dependencies, but `render_instance`
(stage 14) is also load-bearing for `Reloc(InstanceRef)`.
Deferring to stage 13 would land a `render_const` that panics
transitively for `Reloc` -- a common variant (every function
pointer, every vtable instance, every `&str` literal via the
D-4-3 synthesised-Static path). That's the same Option-C
failure mode applied to one variant; not preferred. Stage 13
remains the natural home for the *style coordination* (D-4-K
inherits stage 13's `render_ty` style choice), but the
*implementation* belongs at stage 14 when all dependencies
have filled.

*Why Option A.* Extends the D14 → stage-13 precedent (see
`pretty.rs::render_ty` doc-comment + `stage-13/expectations.md`
"From stage 3") one stage further: where `render_ty` defers to
stage 13 (its only late dependency is `render_def_path` from
stage 12), `render_const` defers to stage 14 (its latest
dependency is `render_instance` from stage 14). Of the 11
`Const` variants, 5 are self-contained scalars; 5 depend on
`render_ty` (stage 13); `Reloc` additionally depends on
`render_def_path(ItemId)` (stage 12) and `render_instance`
(stage 14). The latest-dependency stage is the right home;
locking the renderer style at stage 4 would produce a spec
that changes shape every time later stages fill content.

The §18.2 spec (`spec-snapshot.md`) already locates "remaining
renderers" under stage 13+ ownership in blanket language;
D-4-K refines the per-method ownership table to call out the
dependency chain explicitly: `render_const` lands at stage 14,
not 13.

**Methodology rules invoked.**

- *Store the result, not the input.* The spec-blessed renderer
  belongs to stage 14 (where all its dependencies are filled);
  recording the deferral here, not landing an interim form,
  preserves the rule.
- *No defensive fallbacks.* A debug-grade renderer that
  doesn't recurse into the real type/instance renderers is a
  workaround for the dependency gap, not a fix. The
  no-defensive-fallbacks rule says "don't paper over the gap;
  defer until the gap is real-fixed."
- *No forward-looking placeholders.* The deferral is a *real*
  deferral (handed over to stage 14 with the dependency
  analysis on record), not a reserved slot. Stage 14 is free
  to choose any renderer style consistent with stage 13's
  `render_ty`; no specific style is pre-committed by D-4-K.

**Prior-art evidence.**

- `pretty.rs::render_ty` doc-comment -- the D14 → stage-13
  precedent for `render_ty`, with the same dependency-driven
  reasoning. D-4-K extends it one stage further per the
  longer dependency chain.
- `stage-13/expectations.md` "From stage 3" -- the existing
  "From stage 3" entry for `render_ty`; D-4-K's "From stage 4"
  outbound (filed in `stage-14/expectations.md`) mirrors this
  shape.
- `prior-art/charon.md` -- charon's renderer style (Theme G in
  the stage-3 prior-art file) is the lean for `render_ty`
  and would carry naturally to `render_const` in
  Rust-source-style form. Cited for stage 14's evaluation,
  not binding here.

**Cross-refs.** `crates/regseq-model/src/pretty.rs`
`render_const` method (updated stub message +
doc-comment); `spec-snapshot.md` §18.2 (Renderer status --
per-method ownership table updated to include `render_const`
at stage 14); `stage-14/expectations.md` "From stage 4"
section (outbound expectation filed);
`stage-13/expectations.md` (top-of-file note pointing at
stage-14 for `render_const`); `stage-4/status.md`
"Inbound expectations" (stage-1 inbound "Fill
`pretty.rs::render_const` stub" closed by D-4-K with deferral
to stage 14).

---

## 2. Reference primer

(To be filled as concepts surface during stage-4 work that
deserve concise per-stage explanation. Stage 3 used Section 2
for layout-primer / niche-encoding-primer / TyInterner-pattern-
primer / region-erasure-as-invariant material -- analogous
material may surface here for the const-eval result form,
the AllocId-to-GlobalAlloc resolution, the type-level vs
value-level partition, etc.)

---

## 3. Cross-references

- `../README.md` §5.1 (stage lifecycle), §5.2 (decision tags),
  §5.9 (spec.md update guidelines).
- `memory.md` §1 (workflow rhythm), §2 (stage-4-specific
  principles), §4 (Codex review prompt template).
- `status.md` "Open decisions" (live D-tag catalogue).
- `prior-art/README.md` (cross-cluster prior-art synthesis).
- `prior-art/<project>.md` (per-project primary-source
  research).
- Top-level `../spec.md` §9.7 (`ConstInterner` and the const
  vocabulary stub stage 4 fills); §3.3 (model validation order);
  §13.2 (vtable family -- primary consumer of `Bytes::Reloc`);
  §15.1 (rejections).

---

## Stage 5 (MIR places and projections)

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
   compatibility.** Carry all 8 of rustc's variants,
   including `OpaqueCast` (absent from our input under both
   solver paths per H1 REFINED) and `UnwrapUnsafeBinder`
   (gated behind unstable `unsafe_binders`; no MirPhase
   restriction, would reach input unchanged if a driver opted
   in; no v1 driver use case per H2 REFINED). Future
   toolchain changes wouldn't force spec revisions; lower
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

*Cost analysis.* `TyId` is `u32` per the stage-1 ID-width
rule (top-level `spec.md` §2.2 backing representation), so
4 bytes per `Field` projection. `Place` is
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
   `unimplemented!("stage 11 fills render_place (deferred)")`.
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


---

## Stage 6 (MIR operands)

# Stage 6 -- Discussion Record

Distilled record of the design dialogue producing the
stage-6 `Operand` vocabulary spec. Each entry captures the
question raised, key options considered, the decision (or
current lean for open decisions), the reasoning chain, and
cross-references to `spec-snapshot.md` (the spec) and
`review-log.md` (the audit trail; created on first review
round).

Status of decisions in this file mirrors `spec-snapshot.md`'s
"Operand vocabulary" section (§20) plus adjacent additions to
§18.2, plus `status.md` "Open decisions" / "Locked decisions"
tables.

D-tag convention: prefixed `D-6-K` form per `../README.md`
§5.2 (stages 4+ use the new convention from day one; stages
1 and 3 use legacy un-prefixed tags).

Status tags:
- `[DRAFT]` -- audit-shape recorded; pending Codex review.
- `[LOCKED]` -- closed by an approved lockability decision
  (APPROVE, or MINOR-NIT-only stable per the stage-5
  diminishing-returns precedent).
- `[REOPENED]` -- a later sub-decision or sibling D-tag
  forced a revision; superseded entry kept for history.
- `[DEFERRED to stage N]` -- moved to a later stage's scope.

---

## 1. Decision discussions

### D-6-1 -- `Operand` core vocabulary [LOCKED]

**Question.** What is the closed `Operand` value shape that
the bridge fills from rustc's `mir::Operand<'tcx>` and that
downstream consumers (stages 7-9, codegen) read as the
RHS-atom surface of MIR?

**Key options.**

1. **Option A -- mirror rustc's 4-variant shape verbatim.**
   Adopt rustc's `Copy(Place) / Move(Place) / Constant(Box<ConstOperand>) / RuntimeChecks(RuntimeChecks)`
   structurally; preserve the `ConstOperand` wrapper and
   the `RuntimeChecks` 4th variant. Simplest mirror.
2. **Option B -- 3 variants + reject `RuntimeChecks` on
   encounter.** Adopt `Copy(Place) / Move(Place) / Constant(ConstId)`;
   reject `mir::Operand::RuntimeChecks` per the closed-
   vocabulary methodology rule.
3. **Option C -- 3 variants + lower `RuntimeChecks` at
   bridge time.** Adopt `Copy(Place) / Move(Place) / Constant(ConstId)`;
   the bridge intercepts `mir::Operand::RuntimeChecks(checks)`,
   resolves via `checks.value(tcx.sess)`, interns as
   `Const::Bool(b)`, and emits as `Operand::Constant(ConstId)`.
4. **Option D -- collapse `Copy` and `Move` into a single
   `Use(Place)` variant.** Mirror mrustc's pre-`TODO` shape;
   recover the move-vs-copy semantics via type-driven
   validity tracking in a downstream pass.

**Decision.** Option C (the parent decision; per-aspect
sub-decisions pinned in D-6-1.1 through D-6-1.4):

```rust
pub enum Operand {
    Copy(Place),
    Move(Place),
    Constant(ConstId),
}
```

Plus the bridge-side `RuntimeChecks` lowering rule (§20.3.1):
resolve session-flag query at bridge time, intern as
`Const::Bool`, emit as `Operand::Constant(ConstId)`.

**Reasoning.** Three forces converge on Option C:

1. **Closed-vocabulary discipline.** The methodology rule
   ("closed `PlaceElem` vocabulary" / "no defensive
   fallbacks" -- analogous for stage 6) argues for the
   smallest variant set that captures the IR's content.
   `RuntimeChecks` is per-consumer session-derived; per Q4
   the bridge runs inside one rustc invocation per build
   with one consuming `Session` in scope. The value is
   therefore build-pinned at the bridge boundary and
   indistinguishable from a literal bool at the IR level
   for that build.
2. **Source-acceptance.** Reject-on-encounter (Option B) is
   strictest but rejects legitimate driver source that
   uses `cfg!(ub_checks)` etc. -- a needless friction.
   Lower-and-eliminate (Option C) accepts the source while
   keeping the IR vocabulary closed.
3. **Peer evidence.** mrustc has no `RuntimeChecks` analog
   and resolves session-flag-derived values upstream;
   Charon explicitly lowers `RuntimeChecks` to
   `Rvalue::NullaryOp` + `Move(local)` rather than
   rejecting (`prior-art/charon.md` D-6-1 mapping). Both
   peers' choices match Option C's spirit.

For Copy/Move preservation (Option D rejected): see D-6-1.4.

For `ConstId` payload (vs `Box<ConstOperand>`): see D-6-1.3.

**Methodology rules invoked.**

- "Closed vocabulary; unknown rejected" (`../README.md` §2)
  -- the closed-vocabulary discipline that bounds the
  variant set.
- "No defensive fallbacks" -- there is no `RuntimeChecks`
  fallback variant; the bridge handles the rustc form
  deterministically via the lowering rule.
- "Compositional behavior beats special-case flags" --
  `RuntimeChecks` semantics fall out of `Const::Bool` +
  the existing `Operand::Constant` arm; no parallel flag.
- "C semantics with information preservation" -- store
  rustc's resolved form (Copy/Move tag, interned const),
  render in C shape on demand. Drop redundant metadata
  (`user_ty`, use-site `span`).
- "Re-derive from evidence; do not import baggage" --
  rustc's 4-variant shape is mirror-able but the
  `RuntimeChecks` rationale (multi-crate-graph) does not
  apply here; re-derive yields 3 variants + lowering rule.

**Prior-art evidence.**

- `prior-art/rustc.md` §1 (rustc 4-variant Operand), §4
  (RuntimeChecks per-crate session resolution), §5
  (codegen_operand match), §10 (D-6-1 mapping).
- `prior-art/mrustc.md` -- explicit `TODO: Split "Use" into
  "Copy" and "Move"` at `src/mir/mir.hpp:645` (peer regret
  of Option-D-style collapse); no `RuntimeChecks`
  equivalent.
- `prior-art/charon.md` -- 3-variant LLBC `Operand`;
  preserves Copy/Move; lowers `RuntimeChecks` to
  `Rvalue::NullaryOp` + `Move(local)` (peer precedent for
  Option C).

**Worked example.** A function whose body contains
`if cfg!(overflow_checks) { panic!("...") } else { ... }`
expands at MIR construction to a use of
`mir::Operand::RuntimeChecks(OverflowChecks)` as the
discriminant for a SwitchInt terminator. Bridge translation
walks the operand:

```text
mir::Operand::RuntimeChecks(OverflowChecks)
  -> resolve via sess.overflow_checks() -> b: bool
  -> Const::Bool(b)
  -> ConstInterner::intern(Const::Bool(b)) -> id: ConstId
  -> Operand::Constant(id)
```

Downstream stages see `Operand::Constant(id)` referencing
`Const::Bool(true)` (or `false`) -- indistinguishable from
the source having written the literal directly. The
SwitchInt terminator (stage 9) can statically resolve the
branch in any consumer that const-folds.

**Cross-refs.**

- `spec-snapshot.md` §20 (Operand vocabulary), §20.3
  (bridge translation), §20.3.1 (RuntimeChecks lowering
  rule).
- Sub-decisions D-6-1.1 through D-6-1.4.

---

### D-6-1.1 -- `Operand` variant cardinality [LOCKED]

**Question.** Should the RegseqModel `Operand` enum carry
3 variants (`Copy / Move / Constant`) or rustc's 4 (adding
`RuntimeChecks`)?

**Key options.** Subsumed by parent D-6-1's options
(Option A = 4 variants verbatim, Option B = 3 + reject,
Option C = 3 + lower at bridge time). This sub-decision
pins the variant *cardinality*; D-6-1.2 pins the bridge
treatment of the dropped variant.

**Decision.** 3 variants. The rustc `RuntimeChecks` variant
is dropped from the IR; its content is lowered to
`Operand::Constant(ConstId)` at bridge time per D-6-1.2.

**Reasoning.** Per Q4 (`rustc_driver::Callbacks`
integration), the bridge runs inside one rustc invocation
per build with one consuming `Session` in scope. The rustc
multi-consumer rationale for keeping `RuntimeChecks` as a
non-`Const` variant ("value can differ across crates within
a single crate graph", `rustc_middle/src/mir/syntax.rs:1338-1340`)
maps onto our pipeline as "different builds may produce
different lowered consts" -- but within one build the
consuming session is fixed. There is no IR consumer for
whom the "this came from a session flag" provenance carries
non-constant information -- the bool is the same bool at
IR consumer time. Q6 (multi-crate strategy) does not break
this: even when MIR originates from regseq-aware dependency
crates compiled separately, the bridge consumes via
`tcx.optimized_mir(def_id)` against the bridge-host's
session.

**Methodology rules invoked.** "Closed vocabulary" (smallest
variant set covering content); "Re-derive from evidence"
(rustc's rationale doesn't transfer to our single-consumer
pipeline).

**Prior-art evidence.** `prior-art/charon.md` -- LLBC has
3-variant `Operand` (no `RuntimeChecks` at operand level).
`prior-art/mrustc.md` -- no `RuntimeChecks` analog.

**Worked example.** Source-level `cfg!(overflow_checks)`
expands at MIR construction to a `RuntimeChecks(OverflowChecks)`
operand. With Option A (4 variants), the IR carries this as
a 4th `Operand` arm and every consumer (rvalue evaluation,
backend codegen, validation walks) must handle it explicitly.
With this decision (3 variants + bridge-side lowering), the
IR carries it as `Operand::Constant(ConstId)` referencing a
`Const::Bool(b)` -- consumers see a uniform shape across
all literal-bool sites regardless of source provenance.

**Cross-refs.** `spec-snapshot.md` §20.1; D-6-1 parent;
D-6-1.2 (lowering rule).

---

### D-6-1.2 -- `RuntimeChecks` bridge-side lowering rule [LOCKED]

**Question.** Given D-6-1.1 (drop `RuntimeChecks` variant),
how does the bridge handle a source-level
`mir::Operand::RuntimeChecks(checks)` -- reject, silently
skip, or lower?

**Key options.**

1. **Option (a) -- preserve as 4th variant.** Subsumed by
   D-6-1.1's rejection.
2. **Option (b) -- reject on encounter.** Closed-vocabulary
   strict path; bridge raises `unsupported_construct!`
   diagnostic.
3. **Option (c) -- lower at bridge time.** Resolve via
   `checks.value(tcx.sess)`, intern as `Const::Bool`, emit
   as `Operand::Constant(ConstId)`.

**Decision.** Option (c). The bridge:

1. Resolves the boolean: `b = checks.value(tcx.sess)` per
   the `rustc_middle::mir::syntax.rs:1585-1592` resolution
   table.
2. Constructs `Const::Bool(b)`.
3. Interns via `ConstInterner::intern` (§9.7) yielding
   `ConstId`.
4. Emits `Operand::Constant(ConstId)`.

**Reasoning.** Option (b) would reject legitimate driver
source (`cfg!(ub_checks)`, `core::intrinsics::contract_checks()`,
etc.) for a reason invisible to the user (the IR's
closed-vocabulary discipline). Option (c) accepts the
source while preserving the IR-level discipline; the only
cost is a small bridge-side rewrite.

Resolution well-definedness: per Q4
(`rustc_driver::Callbacks`), the bridge runs inside one
rustc invocation per build with one consuming `Session`.
`checks.value(tcx.sess)` resolves against that single
session. Q6 (multi-crate) does not introduce ambiguity at
the bridge boundary: cross-crate MIR consumed via
`tcx.optimized_mir(def_id)` is resolved against the
bridge-host's session, mirroring rustc's standard codegen
behavior (the consumer's session determines `RuntimeChecks`
values, not the producer's).

Const-uniqueness via `ConstIdentityKey` (D-4-2 byte-form
schema) ensures the same boolean value at multiple sites
resolves to the same `ConstId` within a build, preserving
the F4 / F4-2 within-build invariants that downstream stages
rely on. The build-equivalence quad in §12.4 / §16
(equal source, equal pinned toolchain, equal
`RegseqModel.target`, equal bridge options) does not
currently list `ub_checks` / `contract_checks` /
`overflow_checks` among its equality dimensions; two
builds differing only on those flags fall under the quad's
equivalence but produce different lowered consts at
`RuntimeChecks`-derived operand sites. If cross-build
alignment of those operands is required, the appropriate
fix is to widen the quad in §12.4 (out of stage 6's scope).

**Methodology rules invoked.** "C semantics with
information preservation" (resolve at the level rustc has
already done the work); "Compositional behavior beats
special-case flags" (the variant's content composes from
existing `Const::Bool` + `Operand::Constant`).

**Prior-art evidence.** `prior-art/charon.md` -- Charon
lowers rustc `RuntimeChecks` operands to
`Rvalue::NullaryOp` + materialised bool local +
`Operand::Move(local)`. RegseqModel's lowering is *more*
aggressive (directly to `Const::Bool`) because we don't
need to preserve the origin for verification semantics --
the bool is just a constant once evaluated.

**Worked example.** See D-6-1 worked example (the
`cfg!(overflow_checks)` translation chain).

**Cross-refs.** `spec-snapshot.md` §20.3 (translation table)
and §20.3.1 (lowering rule statement).

---

### D-6-1.3 -- `Constant` payload shape [LOCKED]

**Question.** What does the `Constant` variant carry --
rustc's `Box<ConstOperand>` wrapper, an inline `Const`
struct, or a `ConstId` reference into the model's
`ConstInterner`?

**Key options.**

1. **Option A -- preserve `Box<ConstOperand>` verbatim.**
   `Constant(Box<ConstOperand>)` mirroring rustc; carries
   `span`, `user_ty`, `const_` inline.
2. **Option B -- inline `Const` struct.** Drop the box and
   wrapper but keep an inline `Const` (no
   `ConstInterner` reference); `Constant(Const)` directly.
3. **Option C -- `ConstId` reference.** `Constant(ConstId)`;
   resolve via the model's `ConstInterner` (§9.7); drop
   the wrapper entirely.

**Decision.** Option C. The wrapper is dropped entirely:

- `co.const_` flows through `translate_const` (the §9.7
  bridge construction dispatch for `mir::Const<'tcx>` ->
  RegseqModel `Const`) and is interned via
  `ConstInterner::intern` (§9.7).
- `co.user_ty` is dropped. NLL/borrowck-only metadata; dead
  in `optimized_mir`.
- `co.span` is dropped at the operand level. For
  Statement-level uses, the use-site span is naturally
  carried by the containing Statement's `SourceInfo` at
  stage 8. For Call / TailCall arguments, rustc preserves a
  finer-grained per-argument span via `Spanned<Operand>` at
  the Terminator level (`rustc_middle/src/mir/syntax.rs:811-820, 850-857`);
  that span is captured at the Terminator's argument-list
  slot in stage 9, not in the Operand itself.

**Reasoning.** Three drops, each justified independently:

- **`user_ty` drop.** rustc's `ConstOperand.user_ty` is
  documented as "Needed for NLL to impose user-given type
  constraints" (`rustc_middle/src/mir/syntax.rs:1352-1354`).
  NLL runs *before* MIR optimization passes; by
  `optimized_mir` the field's downstream consumer (NLL) has
  already fired. No bridge consumer reads it.
- **`span` drop at the operand level.** For Statement-level
  uses, the use-site source location is in the Statement's
  `SourceInfo` (`rustc_middle/src/mir/statement.rs:14-19`).
  For Call / TailCall arguments, where rustc *does*
  preserve a per-argument span outside the Terminator's
  own `source_info`, that span lives in the
  `Spanned<Operand>` wrapper at the Terminator level
  (`syntax.rs:811-820, 850-857`); stage 9 captures the
  wrapper, not the Operand. Either way the operand-internal
  `span` is redundant.
- **Wrapper drop.** With `user_ty` and `span` gone, the
  wrapper carries only `const_`. A wrapper around a single
  field is dead structure; the bare `ConstId` reference is
  cleaner.

**Methodology rules invoked.** "Re-derive from evidence; do
not import baggage" (rustc's wrapper is for upstream
consumers that have already run); "C semantics with
information preservation" (preserve what carries downstream
information; drop what is redundant or upstream-dead).

**Prior-art evidence.** `prior-art/rustc.md` §2 (wrapper
fields and their consumers); §8 (rustc_public preserves
the wrapper because it is a stable-IR republisher, not
post-`optimized_mir` pruning -- our pruning calculus is
different).

`prior-art/charon.md` -- Charon stores `Operand::Const(Box<ConstantExpr>)`
inline (no `ConstInterner` analog). RegseqModel's
`ConstId`-by-reference is a stage-4-architectural
consequence, not a stage-6-original choice.

**Worked example.** A `let x: u32 = 42;` binding produces
MIR `_1 = const 42_u32`. The rustc-side
`Operand::Constant(box ConstOperand { span: <site>, user_ty: None, const_: Const::Val(...) })`
is translated by the bridge as:

```text
mir::Operand::Constant(box co)
  -> co.const_
       -> translate_const(co.const_) -> Const::Uint(U32, 42)
       -> ConstInterner::intern(Const::Uint(U32, 42)) -> id: ConstId
  -> co.span: dropped (Statement.source_info at stage 8 carries the use-site span)
  -> co.user_ty: dropped (None here; would also be dropped for an
     ascription like `Vec::<u32>::new()` because user_ty is NLL-only)
  -> Operand::Constant(id)
```

A second `_2 = const 42_u32` later in the same body produces
the *same* `ConstId` via `ConstInterner`'s content-uniqueness
(D-4-2 byte-form schema), so the two operand sites share the
same payload identity automatically.

**Cross-refs.** `spec-snapshot.md` §20.1 (Constant variant
docstring), §20.3 (wrapper-strip translation rule).
Outbound: `stage-9/expectations.md` "From stage 6"
(stage 9 must preserve per-argument spans via the
`Spanned<Operand>` wrapper at Call / TailCall Terminator
sites; preservation shape is stage 9's choice).

---

### D-6-1.4 -- Copy/Move preservation [LOCKED]

**Question.** Should the RegseqModel `Operand` preserve
both `Copy(Place)` and `Move(Place)` as distinct variants,
or collapse to a single `Use(Place)` (mrustc-style)?

**Decision.** Preserve both variants distinct.

**Reasoning.** Three independent forces support
preservation:

1. **Backend optionality at zero spec cost.** A C backend
   that wants to specialise (e.g., emit `memcpy` for large
   moves where the source becomes dead, or elide a final
   move into a return slot) needs the variant tag. Even if
   the v1 backend doesn't use the distinction, preserving
   it costs nothing in spec complexity and keeps the
   option open.
2. **Move's distinguishing Call-site semantics.**
   `Operand::Move`'s `Call`-argument in-place passing
   (rustc `syntax.rs:1322-1330`) is semantically distinct
   from `Copy`. A backend that wants to model parameter
   passing precisely (e.g., for ABI verification) needs
   the distinction. Codegen-level operational equivalence
   (`codegen_operand` collapse in
   `rustc_codegen_ssa/src/mir/operand.rs:1046-1056`) does
   not extend to all consumers.
3. **Peer evidence.** mrustc's `Use(LValue)` collapse
   carries an explicit `TODO: Split "Use" into "Copy" and
   "Move"` (`src/mir/mir.hpp:645`) -- the closest existing
   peer regrets the collapse. Charon preserves the
   distinction in its verification pipeline. rustc itself
   keeps both variants. Three peers, three independent
   reasons to preserve.

**Methodology rules invoked.** "C semantics with
information preservation" (preserve information rustc gives
us where downstream consumers may want it); "No defensive
fallbacks" does not apply -- the distinction is rustc-given
content, not a defensive fallback.

**Prior-art evidence.** `prior-art/rustc.md` §1
(`Operand::Copy` / `Operand::Move` docstrings -- semantic
distinction); §5 (codegen_operand collapse with
`move_annotation` debug-info diff); §6 (annotation
conditions).

`prior-art/mrustc.md` -- `src/mir/mir.hpp:645` regretted
collapse; type-driven validity tracking compensates but at
visible cost.

`prior-art/charon.md` -- preserves Copy/Move through
verification IR; pretty-printer emits `copy <p>` / `move <p>`
keyword-prefix style.

**Worked example.** A function `fn consume(s: String) -> usize`
takes its argument by-move. The MIR call site emits
`mir::Operand::Move(_2)` for the argument. Translation:
`Operand::Move(Place::from(_2))`. A future backend wanting
to elide an intermediate copy on the way to the call frame
can consult the `Move` tag; a backend that doesn't, gets
the same C value-load as `Copy` and is unaffected.

**Cross-refs.** `spec-snapshot.md` §20.1 (variant docstrings),
§20.4 (C-semantics lowering -- bare load semantics
identical, variant tag informational).

---

### D-6-2 -- Operand storage and interning [LOCKED]

**Question.** Where does an `Operand` live in the model --
inline at use sites, or via an `OperandId` indirection
into a top-level interner?

**Key options.**

1. **Option A -- inline at use sites.** `Operand` is
   embedded by value into stages 7-9 payloads (`Rvalue`,
   `Statement`, `Terminator`). No `OperandId`, no
   interner.
2. **Option B -- `OperandId` interner.** Mirror stage 4's
   `Const` interning; introduce `OperandInterner` and
   reference operands by `OperandId`.

**Decision.** Option A. Operands live inline at use sites;
no `OperandId`; no operand-level interner.

**Reasoning.** Two concrete arguments against interning:

1. **No downstream consumer of operand-content-uniqueness.**
   `Const` interning exists because `Const`-content-identity
   matters for downstream consumers: stage-3
   `GenericArg::Const(ConstId)` participates in
   `Ty`-identity; stage-9 instance-key construction reads
   `ConstId`s; F4-2 cross-build identity threads through
   them. Operands have no analogous consumer -- no IR
   structure consumes
   "two operand uses are content-equivalent."
2. **Heterogeneous, small payloads.** Operand variants are
   small (~24 bytes worst case) and structurally
   heterogeneous; interning's overhead (a hash lookup at
   construction; ID dereference at use) buys nothing.

**Methodology rules invoked.** "Compiler-theory-first" --
interning is a deduplication optimisation; without a
deduplication consumer, it's premature abstraction. "Eager
translation" (Q2) does not require interning; it requires
fully-owned IR, which inline-by-value satisfies trivially.

**Prior-art evidence.** rustc's MIR has no operand
interner (`prior-art/rustc.md` §7). `rustc_public` follows
the same pattern (§8). mrustc inlines at use sites with
spaceship `ord(...)` for structural equality
(`prior-art/mrustc.md`). Charon has a generic
`HashConsed<T>` infrastructure but explicitly does not apply
it to `Operand` (`prior-art/charon.md` D-6-2 mapping).
Four-out-of-four peers store inline.

**Worked example.** A function with two consecutive
statements `_2 = _1` and `_3 = _1` produces MIR with two
`Rvalue::Use(Operand::Copy(Place::from(_1)))` operand
slots. Under Option B, both would resolve to the same
`OperandId` via interning -- a single deduplication step
saving ~16 bytes per repeat. Under Option A (this
decision), each `Rvalue::Use` carries its own
`Operand::Copy(Place { local: LocalId(1), projection: Box<[]> })`
inline. The "saved" 16 bytes per duplicate is dwarfed by
the cost of a parallel `OperandInterner` table, the
`OperandId` type and serde derives, and the indirection at
every use site -- all to deduplicate something no
downstream consumer asks about. Inline storage is
strictly cheaper.

**Cross-refs.** `spec-snapshot.md` §20.2 (identity and
storage).

---

### D-6-3 -- Operand-site invariants [LOCKED]

**Question.** What invariants does the bridge enforce at
operand construction? Specifically: does `Operand::Copy(p)`
require that `p`'s type is `Copy` (rustc's
pre-drop-elaboration invariant)?

**Key options.**

1. **Option A -- enforce rustc's pre-drop-elaboration
   type-`Copy` invariant on `Operand::Copy(p)`.** Mirror
   rustc's documented invariant; reject if `p`'s type is
   non-`Copy`.
2. **Option B -- enforce a weaker invariant.** Require
   only that the place be loadable (e.g., its type is
   `Sized` and validation rules pass), without specifically
   checking the `Copy` bound.
3. **Option C -- enforce nothing at operand construction.**
   Inherit Place validity from §19.7 transitively; do not
   add operand-site checks.

**Decision.** Option C. No type-`Copy` enforcement on
`Operand::Copy(p)`. One invariant only:

- **I-Operand-Place-Validity** (§20.5) -- discharged by
  inheritance: enforcing Place validity (§19.7) transitively
  covers all `Operand::Copy` / `Operand::Move` slots.

No type-coherence enforcement on `Operand::Constant(id)` at
operand site; consumers (stages 7-9) own that check.

**Reasoning.** The rustc-side type-`Copy` invariant on
`Operand::Copy` is documented as required *pre-drop-
elaboration only* and *explicitly gone post-drop-elaboration*
(`rustc_middle/src/mir/syntax.rs:1312-1313`):

> "Before drop elaboration, the type of the place must be
> `Copy`. After drop elaboration there is no such
> requirement."

RegseqModel reads `optimized_mir`, which is post-drop-
elaboration. The invariant has been discharged before the
IR reaches us; the bridge would be re-checking already-
checked content. Per "no defensive fallbacks," we do not
add the check.

Place validity (§19.7) is non-vacuous only once stages 7-9
materialise operand use sites; the walk piggybacks on the
§3.3 Phase B body-validation traversal that stage 11 owns
(D-5-2 deferral).

**Methodology rules invoked.** "No defensive fallbacks" --
do not re-check what rustc has discharged; "Compositional
behavior beats special-case flags" -- I-Operand-Place-Validity
is fully derived from §19.7 + §3.3 Phase B; no operand-side
parallel walk.

**Prior-art evidence.** `prior-art/rustc.md` §1 (the
pre-drop-elaboration / post-drop-elaboration distinction),
§10 D-6-3 mapping. `prior-art/mrustc.md` -- mrustc enforces
no operand-construction invariants; the move/copy semantic
is checked at use sites via per-local validity tracking.
`prior-art/charon.md` -- Charon enforces no type-`Copy` on
`Operand::Copy(p)`; `simplify_constants` micro-pass
establishes a separate post-pass invariant (orthogonal to
this question).

**Worked example.** A function `fn drop_string(s: String)`
contains an `Operand::Move(_1)` at its drop point. In
pre-drop-elaboration MIR, this would also appear as
`Operand::Move(_1)`; in *theory* an `Operand::Copy(_1)` at
that site would have been rejected because `String` is not
`Copy`. In `optimized_mir` (post-drop-elaboration), the
type-`Copy` constraint is gone -- rustc itself drops the
check. RegseqModel reads `optimized_mir`; if a hand-crafted
fixture were to write `Operand::Copy(_1)` at this site, the
bridge would not flag it. Place validation (§19.7) still
runs and catches structural problems (e.g., `_1` not in
`local_decls`); the type-`Copy` semantic check is rustc's
upstream concern.

**Cross-refs.** `spec-snapshot.md` §20.5 (operand-site
invariants), §19.7 (Place validation, inherited).

---

### D-6-K -- `pretty.rs::render_operand` deferral [LOCKED]

**Question.** Where does `pretty.rs::render_operand` land
-- stage 6 (alongside the Operand vocabulary), stage 11
(alongside `render_place`, latest first-batch dependency),
or stage 14 (alongside `render_const`, latest dependency
across all variants)?

**Key options.**

1. **Option A -- stage 6 (immediate).** Land
   `render_operand` alongside the variant set. Requires
   the `Constant` arm to call `render_const` (stage 14,
   not yet filled) and the `Copy`/`Move` arms to call
   `render_place` (stage 11, not yet filled).
2. **Option B -- stage 11 (latest first-batch
   dependency).** Co-locate with `render_place` (D-5-K).
   The `Constant` arm late-binds to `render_const` once
   stage 14 fills it.
3. **Option C -- stage 14 (latest dependency overall).**
   Wait for both `render_place` (stage 11) and
   `render_const` (stage 14) before landing
   `render_operand`. Maximally deferred.

**Decision.** Option B (stage 11). Filed as outbound
expectation in `stage-11/expectations.md` "From stage 6"
alongside the existing D-5-K `render_place` deferral.

**Reasoning.** Per-variant rendering input dependency:

| Variant         | Rendering input                                 | Stage of input |
| --------------- | ----------------------------------------------- | -------------- |
| `Copy(p)`       | `render_place(p, body)` + `copy ` keyword prefix | 11            |
| `Move(p)`       | `render_place(p, body)` + `move ` keyword prefix | 11            |
| `Constant(id)`  | `render_const(id)`                              | 14            |

The `Copy` / `Move` arms depend on `render_place` (stage
11) which itself depends on `Body` materialisation +
`LocalId` `_N` rendering. The `Constant` arm depends on
`render_const` (stage 14, per D-4-K).

`render_operand` could land at stage 11 or stage 14. Stage
11 is chosen because:

1. `render_place` lands at stage 11 (D-5-K). Co-locating
   `render_operand` with `render_place` keeps the rendering
   surface coherent.
2. The `Constant` arm can call `render_const` once stage 14
   fills it; the dependency chain is acyclic.
3. Snapshot tests covering `Operand`-bearing body content
   become available at stage 11 (when bodies materialise),
   blocking on `render_operand` rather than waiting for
   stage 14.

This mirrors `render_place`'s dependency analysis (D-5-K):
land at the latest of the *first batch* of dependencies
that unblock testing, with the late-binding arm calling
its later-landing dependency.

**Methodology rules invoked.** "Renderer style is decided
deliberately" (per stage-5 D-5-K precedent; deferral
target chosen by dependency-chain analysis, not by default).

**Prior-art evidence.** D-5-K precedent: `render_place`
deferred to stage 11 with the same dependency-on-Body-
materialisation argument. `prior-art/charon.md` --
Charon's pretty-printer uses keyword-prefix style
(`copy <p>` / `move <p>` / `const <c>`); one valid prior-art
point for stage 11's eventual `render_operand`.

**Worked example.** Rendering `Operand::Move(_3)` requires
two pieces: the `move ` keyword prefix (self-contained
text) and `render_place(_3, body)`. The latter returns
`_3` (the `LocalId` `_N` sigil rendering, stage 11) when
the place has no projections, or `*_3.field` etc. when it
does (still stage 11, since field-name resolution reads
from `LocalDecl.ty`'s layout). The full output is `move _3`.
Now consider `Operand::Constant(id)` where `id` resolves
to `Const::Int(I32, -1)` -- rendering produces `const -1_i32`,
which requires `render_const(id)` (stage 14). At stage 11
where `render_operand` lands, the `Copy` and `Move` arms
work end-to-end; the `Constant` arm calls into a stage-14-
filled `render_const` (which lands later in the dependency
chain) -- the call is statically valid as soon as both
landed; tests for the `Constant` arm gate on stage 14.

**Cross-refs.** `spec-snapshot.md` §20.6 (rendering
deferral), §18.2 (per-method ownership table -- updated
with `render_operand` row). Outbound: `stage-11/expectations.md`
"From stage 6" (filed at §3 Work step 4).


## Stage 7 (MIR rvalues)

# Stage 7 -- Discussion Record

Distilled record of the design dialogue producing the
stage-7 `Rvalue` vocabulary spec. Each entry captures the
question raised, key options considered, the decision (or
current lean for open decisions), the reasoning chain, and
cross-references to `spec-snapshot.md` (the spec) and
`review-log.md` (the audit trail; created on first review
round).

Status of decisions in this file mirrors `spec-snapshot.md`'s
"Rvalue vocabulary" section (§21) plus adjacent additions to
§18.2, plus `status.md` "Open decisions" / "Locked decisions"
tables.

D-tag convention: prefixed `D-7-K` form per `../README.md`
§5.2.

Status tags:
- `[LOCKED]` -- audit-shape recorded; pending Codex review.
- `[LOCKED]` -- closed by an approved lockability decision
  (APPROVE, or MINOR-NIT-only stable per the stage-5
  diminishing-returns precedent).
- `[REOPENED]` -- a later sub-decision or sibling D-tag
  forced a revision; superseded entry kept for history.
- `[DEFERRED to stage N]` -- moved to a later stage's scope.

---

## 1. Decision discussions

### D-7-1 -- `Rvalue` core vocabulary [LOCKED]

**Question.** What is the closed `Rvalue` value shape that
the bridge fills from rustc's `mir::Rvalue<'tcx>` and that
downstream consumers (stages 8-9 body content, codegen) read
as the RHS-expression surface of MIR?

**Key options.**

1. **Option A -- mirror rustc's 13-variant shape verbatim.**
   Adopt rustc's `mir::Rvalue<'tcx>` structurally (Use,
   Repeat, Ref with Region, ThreadLocalRef, RawPtr, Cast,
   BinaryOp with Box pair, UnaryOp, Discriminant, Aggregate
   with full AggregateKind, ShallowInitBox, CopyForDeref,
   WrapUnsafeBinder). Carry every variant; preserve every
   borrowck / NLL / runtime-pre-lowering carrier.
2. **Option B -- 11-variant closed surface.** Drop only the
   two cleanup-pass-discharged variants (`CopyForDeref` and
   `ShallowInitBox`); preserve `ThreadLocalRef` and
   `WrapUnsafeBinder`. Reject-on-encounter via D-7-3 for the
   discharged pair.
3. **Option C -- 9-variant closed surface.** Drop the four
   constructs that cannot reach a well-formed driver
   `optimized_mir`: `CopyForDeref`, `ShallowInitBox`
   (cleanup-pass-discharged), `ThreadLocalRef` (DSL-surface
   incompatibility with Linux-kernel-module / Zephyr driver
   targets), `WrapUnsafeBinder` (nightly-experimental + zero
   driver-corpus precedent + no C-shape; type-level
   rejection of `UnsafeBinder` in §15.1 cascades to
   the rvalue level by composition).

**Decision.** Option C (the parent decision; per-aspect
sub-decisions pinned in D-7-1.1 through D-7-1.8):

```rust
pub enum Rvalue {
    Use(Operand),
    Repeat(Operand, u64),
    Ref(BorrowKind, Place),
    RawPtr(RawPtrKind, Place),
    Cast(CastKind, Operand, TyId),
    BinaryOp(BinOp, Operand, Operand),
    UnaryOp(UnOp, Operand),
    Discriminant(Place),
    Aggregate(AggregateKind, Box<[Operand]>),
}
```

Plus bridge-side translation rules folded into §15
(extending stage-6 D-6-1.2's bridge-lowering pattern to
`RawPtrKind::FakeForPtrMetadata` and to the various rustc-
internal annotations dropped per sub-decisions D-7-1.4,
D-7-1.5, D-7-1.6, D-7-1.8). Reject-on-encounter rules for
the four dropped rustc variants plus the cleanup-pass-
discharged BorrowKind / CastKind / AggregateKind sub-
variants land in D-7-3.

**Reasoning.** Three forces converge on Option C:

1. **Closed-vocabulary discipline grounded in cleanup-pass
   evidence.** rustc's required runtime-lowering passes
   discharge two variants: `EraseDerefTemps` eliminates
   `CopyForDeref` (`rustc_mir_transform/src/lib.rs:658`),
   and `ElaborateBoxDerefs` eliminates `ShallowInitBox`
   (`rustc_mir_transform/src/lib.rs:659`). Both `is_required:
   true` -- run regardless of opt-level. Both are also
   `bug!()`-rejected by codegen at `rustc_codegen_ssa/src/mir/rvalue.rs:712-713`.
   This is twice-confirmed evidence: a well-formed
   `optimized_mir` body cannot contain either variant.
   Carrying them as RegseqModel `Rvalue` arms would violate
   the closed-vocabulary methodology rule by parking
   variants the bridge will never produce.
2. **DSL-surface incompatibility excludes ThreadLocalRef.**
   `Rvalue::ThreadLocalRef(DefId)` is generated only by
   source-level `#[thread_local]` static items, gated on
   `#![feature(thread_local)]`. The two driver target form-
   factors (Linux kernel module, Zephyr device driver) do
   not provide TLS infrastructure to driver code: Linux
   kernel modules use `DEFINE_PER_CPU` (exposed through
   rust-for-linux bindings, not `#[thread_local]`); Zephyr
   exposes thread-local-like storage via `k_thread`
   structures (also not Rust TLS). A driver author who
   writes `#[thread_local]` produces source whose
   `optimized_mir` cannot link or run. The "No defensive
   fallbacks" methodology rule applies: do not carry a
   variant whose only producer is source that cannot ship.
3. **Type-level rejection cascades to WrapUnsafeBinder.**
   Stage 3 already rejects `UnsafeBinder` at type
   translation (`spec.md:4583`, §15.1 type-level rejection
   table: "unstable for-binders feature"). `Rvalue::WrapUnsafeBinder(Operand, Ty)`
   produces a value of an `UnsafeBinder<...>` type --
   precisely the type stage 3 rejects. By composition with
   stage 3, the variant is unreachable in valid IR. This is
   the strongest grounding available: a hard type-level wall
   rather than a soft pass-discharge or a source-level claim.

For per-variant payload shapes, sub-enum subsets, storage
choices, and per-site invariants, see sub-decisions
D-7-1.1 through D-7-1.8 (variant set + payload), D-7-2
(storage), D-7-3 (invariants), and D-7-K (renderer).

**Methodology rules invoked.**

- "Closed `Rvalue` vocabulary" (`../memory.md` §2 stage-7-
  specific principle) -- the closed-vocabulary discipline
  that bounds the variant set. This is a stage-7
  specialization of the README §2 "core is opaque" rule's
  "items outside the recognized vocabulary are rejected"
  pattern, applied at the Rvalue surface.
- "No defensive fallbacks" -- do not carry variants whose
  producers cannot reach the bridge (cleanup-pass-discharged,
  type-level-rejected, target-form-factor-incompatible).
- "Cleanup-pass artefacts are reject-on-encounter, not
  variants" (`../memory.md` §2 stage-7-specific principle)
  -- the discharged pair lives in D-7-3 rejection rules,
  not in the enum.
- "Compositional behavior beats special-case flags" --
  WrapUnsafeBinder drop falls out of stage 3's UnsafeBinder
  type rejection (`spec.md:4583`) plus stage 7's variant-
  cardinality rule; no parallel flag.
- "Re-derive from evidence; do not import baggage" --
  rustc's 13-variant shape carries multiple variants whose
  rationale (early-MIR optimisation hooks, async runtime
  concerns) does not transfer to RegseqModel's `optimized_mir`-
  consuming, `#![no_std]`-driver-targeting context.
- "C semantics with information preservation" -- preserve
  what rustc gives us for free for the surviving 9 variants
  (Operand sub-content, TyId references, BorrowKind /
  RawPtrKind / CastKind / AggregateKind discriminators) and
  drop borrowck / NLL / Miri-internal annotations that have
  no C-shape consequence.

**Prior-art evidence.**

- `prior-art/rustc.md` §1 (13-variant Rvalue ground truth);
  §11 (CopyForDeref / ShallowInitBox / WrapUnsafeBinder
  doc-side and consumer-side evidence); §13 (cleanup-pass
  whitelist context: which discharges are required and
  which are opt-level-gated); §15 (D-7-K candidate mapping).
- `prior-art/mrustc.md` §1 (15-variant peer surface; no
  CopyForDeref / ShallowInitBox / WrapUnsafeBinder analogs --
  C-targeting peer that already lacks these constructs);
  §11 (per-D-tag mapping).
- `prior-art/charon.md` §1 (11-variant LLBC peer surface;
  validates that 11 is workable for a verification-friendly
  downstream); §11 (translation-pass evidence: Charon
  rejects ThreadLocalRef and WrapUnsafeBinder at translation
  time, validating Option C's drop targets); §14 (D-7-K
  mapping).

**Worked example.** A typical driver function

```rust
fn read_status(reg: &mut Reg) -> u32 {
    let raw: *const u32 = &raw const reg.status;
    let v = unsafe { core::ptr::read_volatile(raw) };
    v & 0x0F
}
```

produces MIR `Rvalue`s spanning:

- `Rvalue::RawPtr(RawPtrKind::Const, Place::from(reg.status))`
  (the `&raw const` expression),
- `Rvalue::Use(Operand::Move(_2))` (where `_2` holds the
  intrinsic call result),
- `Rvalue::BinaryOp(BinOp::BitAnd, Operand::Copy(_3),
  Operand::Constant(0x0F))` (the masking operation).

All three reach stage 7 as variants of the 9-variant closed
surface; no `CopyForDeref`, no `ThreadLocalRef`, no
`ShallowInitBox`, no `WrapUnsafeBinder` is produced by this
driver code in the first place. The closed surface matches
the producible IR exactly.

**Cross-refs.**

- `spec-snapshot.md` §21 (Rvalue vocabulary), §21.6 (bridge
  translation table for §15), §21.5 (rejection rules).
- Sub-decisions D-7-1.1 through D-7-1.8.

---

### D-7-1.1 -- Rvalue variant cardinality [LOCKED]

**Question.** Should the RegseqModel `Rvalue` enum carry
all 13 rustc variants, 11 (drop only cleanup-pass-
discharged), or 9 (drop additionally `ThreadLocalRef` and
`WrapUnsafeBinder`)?

**Key options.** Subsumed by parent D-7-1's options
(Option A = 13 verbatim, Option B = 11 + reject 2,
Option C = 9 + reject 4). This sub-decision pins the
variant *cardinality*; D-7-3 pins the rejection rules for
the dropped variants.

**Decision.** 9 variants:

```rust
pub enum Rvalue {
    Use(Operand),
    Repeat(Operand, u64),
    Ref(BorrowKind, Place),
    RawPtr(RawPtrKind, Place),
    Cast(CastKind, Operand, TyId),
    BinaryOp(BinOp, Operand, Operand),
    UnaryOp(UnOp, Operand),
    Discriminant(Place),
    Aggregate(AggregateKind, Box<[Operand]>),
}
```

The four rustc variants `CopyForDeref`, `ShallowInitBox`,
`ThreadLocalRef`, and `WrapUnsafeBinder` are dropped from
the IR; rejection-on-encounter rules land in D-7-3.

**Reasoning.** Each drop has its own evidence basis:

| Variant | Evidence basis |
| ------- | -------------- |
| `CopyForDeref(Place)` | Eliminated by required `EraseDerefTemps` runtime-lowering pass (`rustc_mir_transform/src/lib.rs:658`); codegen-side `bug!("CopyForDeref in codegen")` (`rustc_codegen_ssa/src/mir/rvalue.rs:712`); twice-confirmed unreachable in `optimized_mir`. |
| `ShallowInitBox(Operand, Ty)` | Eliminated by required `ElaborateBoxDerefs` runtime-lowering pass (`rustc_mir_transform/src/lib.rs:659`); codegen-side `bug!("ShallowInitBox in codegen")` (`rustc_codegen_ssa/src/mir/rvalue.rs:713`); twice-confirmed unreachable. |
| `ThreadLocalRef(DefId)` | DSL-surface claim: source-level `#[thread_local]` requires TLS infrastructure not provided by Linux-kernel-module / Zephyr-driver target form-factors. A driver author who writes `#[thread_local]` produces source that cannot link or run. Per "No defensive fallbacks," the bridge does not carry the variant. |
| `WrapUnsafeBinder(Operand, Ty)` | §15.1 Type-level rejection table (`spec.md:4583`) already rejects `UnsafeBinder` at type translation (`unstable for-binders feature`). The variant produces a value of an `UnsafeBinder<...>` type -- precisely the type stage 3's translation rules reject. By composition with stage 3, the variant is unreachable in valid IR. |

The 9-variant set is the largest reachable closed surface
that compose-derives from the cleanup-pass + type-system +
target-form-factor evidence. Each dropped variant has at
least one strong-evidence ground; the cleanup-pass-
discharged pair has two; `WrapUnsafeBinder` has the
strongest (type-system rejection cascade).

**Methodology rules invoked.** "Closed `Rvalue` vocabulary"
(`../memory.md` §2 stage-7-specific principle: the smallest
variant set covering the reachable content; specializes
the README §2 "core is opaque" rule's "items outside the
recognized vocabulary are rejected" pattern at the Rvalue
surface). "No defensive fallbacks" (`../README.md` §2
methodology rule: do not handle the unhandleable).
"Cleanup-pass artefacts are reject-on-encounter, not
variants" (`../memory.md` §2 stage-7-specific corollary).
"Compositional behavior beats special-case flags"
(`../README.md` §2 methodology rule: WrapUnsafeBinder drop
falls out of §15.1 type-level rejection plus stage-7's
variant-cardinality rule).

**Prior-art evidence.**

- `prior-art/rustc.md` §11 (codegen rejection sites),
  §13 (cleanup-pass whitelist context).
- `prior-art/mrustc.md` §1 (peer C-targeting compiler --
  has none of CopyForDeref / ShallowInitBox / WrapUnsafeBinder;
  has BORROW unified for Ref + RawPtr; absence of Coroutine
  variants tracks the same `#![no_std]` driver-corpus
  reality).
- `prior-art/charon.md` §11 (translation-pass rejection
  matrix: CopyForDeref lowered to plain Copy, ShallowInitBox
  preserved structurally but never reached in
  optimized-MIR-derived input, ThreadLocalRef rejected,
  WrapUnsafeBinder rejected). Validates Option C's reject-
  list against an independent peer making similar choices.

**Worked example.** A driver function whose only structural
oddity is taking the address of an array element

```rust
fn first_byte(buf: &[u8; 16]) -> u8 { buf[0] }
```

produces an `optimized_mir` body containing
`Rvalue::Use(Operand::Copy(Place { local: _1, projection:
[Deref, ConstantIndex { offset: 0, ... }] }))` -- a plain
`Use` plus a 2-element place projection. None of
`CopyForDeref`, `ShallowInitBox`, `ThreadLocalRef`, or
`WrapUnsafeBinder` appears. The 9-variant surface admits
this body without any defensive arm.

By contrast, hand-crafted MIR or `optimized_mir` from a
crate that uses `#[thread_local] static FOO: u32 = 0;` would
emit `Rvalue::ThreadLocalRef(<def_id of FOO>)`. The bridge
rejects with an `unsupported_construct!` diagnostic per
D-7-3, naming "thread-local statics not supported in driver
code (target form-factor lacks TLS infrastructure)."

**Cross-refs.** `spec-snapshot.md` §21.1 (variant set),
§21.5 (rejection rules); D-7-1 parent; D-7-3 (rejection
rule list).

---

### D-7-1.2 -- `Use(Operand)` payload [LOCKED]

**Question.** What does the `Use` variant carry?

**Key options.**

1. **Option A -- direct embed.** `Use(Operand)` carries the
   stage-6 `Operand` by value.
2. **Option B -- collapse Use into Operand-as-Rvalue.**
   Eliminate the variant; allow `Operand` to appear
   wherever an `Rvalue` is expected.

**Decision.** Option A. Direct embed of stage-6 `Operand`:

```rust
Use(Operand),
```

**Reasoning.** Option B would require the type system to
treat `Operand` and `Rvalue` interchangeably at every
`Statement::Assign(Place, Rvalue)` site -- a structural
divergence from rustc's MIR with no compensating benefit.
The variant tag disambiguates "this rvalue is a pure
operand-load" from "this rvalue is a computation;" backend
lowering and pretty-printing both want the disambiguation.

Codegen at `rustc_codegen_ssa/src/mir/rvalue.rs:653` is pure
delegation: `mir::Rvalue::Use(ref operand) =>
self.codegen_operand(bx, operand)`. mrustc preserves the
variant as `RValue::Use(LValue)` (`src/mir/mir.hpp:644-650`,
per `prior-art/mrustc.md` §1). Charon preserves it as
`Rvalue::Use(Operand)` (`prior-art/charon.md` §2). Three
peers converge on direct embed.

**Methodology rules invoked.** "C semantics with
information preservation" (preserve the rustc-given variant
tag).

**Prior-art evidence.** `prior-art/rustc.md` §2;
`prior-art/mrustc.md` §1 (RValue::Use);
`prior-art/charon.md` §2.

**Worked example.** `let x = y;` produces
`StatementKind::Assign(_x, Rvalue::Use(Operand::Copy(_y)))`
in rustc MIR; translates to
`Statement::Assign(_x, Rvalue::Use(Operand::Copy(_y)))` in
RegseqModel -- direct shape-preserving translation.

**Cross-refs.** `spec-snapshot.md` §21.1 (Use variant
docstring).

---

### D-7-1.3 -- `Repeat(Operand, count)` count form [LOCKED]

**Question.** How is the array length carried -- as a
direct `u64`, or as a `ConstId` reference into stage 4's
`ConstInterner`?

**Key options.**

1. **Option A -- direct `u64`.** `Repeat(Operand, u64)`.
   Compositional with stage 3's
   `Ty::Array(elem_ty, n: u64)` canonical-form rule
   (`spec.md:4525`).
2. **Option B -- `ConstId`.** `Repeat(Operand, ConstId)`.
   Resolve through stage-4 `ConstInterner`; ConstId would
   resolve to a `Const::Uint`.

**Decision.** Option A:

```rust
Repeat(Operand, u64),
```

**Reasoning.** Three concrete arguments:

1. **Compositional with stage 3's `Ty::Array` length.**
   Stage 3's translation table (`spec.md:4525`) lowers
   rustc `Array(t, n)` to `Ty::Array(translate(t), n)` with
   `n` as post-mono `u64`. The same value (an array length)
   should be stored the same way at the rvalue level;
   maintaining two storage forms for one value would be a
   compositional break.
2. **Type-system const, not value-system const.** rustc's
   `Repeat(Operand<'tcx>, ty::Const<'tcx>)` carries a
   *type-const* (`ty::Const`) -- the same kind that
   parameterises `Ty::Array`. After monomorphisation,
   type-consts for array lengths are concrete non-negative
   integers; `u64` is sufficient. The stage-4
   `ConstInterner` handles *value-consts* (`mir::Const`),
   a different kind. Routing array lengths through the
   value-const interner would conflate the two systems.
3. **No interning payoff.** `ConstId` indirection adds
   value when a value carries cross-build identity (F4-2
   InstanceKey participation, structural-aggregate
   children) or when content uniqueness has multiple
   downstream consumers (D-6-2 reasoning). A bare `u64` is
   structurally trivial and has no analogous consumer.

**Methodology rules invoked.** "Compositional behavior
beats special-case flags" (one storage form per concept;
do not introduce parallel flag-style markers); "Re-derive
from evidence; do not import baggage" (rustc's `ty::Const`
carries genericity machinery dead post-mono; do not import).

**Prior-art evidence.** `prior-art/rustc.md` §3 (type-const
post-mono is concrete u64; cross-stage anchor to stage 3
`Ty::Array` canonical form); `prior-art/mrustc.md` §9
(`SizedArray.count is HIR::ArraySize, not Constant` --
peer-validated separation of array-length-const from
value-const); `prior-art/charon.md` §10 (Charon stores the
count as a `TyConst` analog separate from value `Operand`s,
peer-validated).

**Worked example.** `[0u32; 16]` produces rustc MIR
`Rvalue::Repeat(Operand::Constant(box ConstOperand { ...
const_: Const::Val(0u32) }), <ty::Const wrapping 16>)`.
Translation:

```text
mir::Rvalue::Repeat(elem, len_const)
  -> elem -> translate_operand(elem) -> Operand::Constant(<intern Const::Uint(U32, 0)>)
  -> len_const -> evaluate as concrete u64 -> 16u64
  -> Rvalue::Repeat(Operand::Constant(...), 16u64)
```

The result is a uniform `u64` count that mirrors the same
value in the destination type `Ty::Array(elem_ty, 16u64)`.

**Cross-refs.** `spec-snapshot.md` §21.1 (Repeat variant
docstring), §21.6 (bridge translation -- type-const
evaluation rule).

---

### D-7-1.4 -- `Ref(BorrowKind, Place)` payload + BorrowKind subset [LOCKED]

**Question.** Two woven sub-questions:
(1) Does `Ref` carry rustc's `Region<'tcx>` slot? (2) What
is the closed `BorrowKind` subset?

**Key options.** For sub-question 1: keep the Region slot
or drop it. For sub-question 2:

1. **Option A -- 2-variant subset (`SharedBorrow` /
   `UniqueBorrow`).** Drop `Fake` (cleanup-discharged) and
   the `MutBorrowKind` sub-discriminator (codegen-collapsed).
2. **Option B -- 3-variant: preserve `TwoPhaseBorrow`.**
   Carry the auto-ref distinction (method-call auto-ref).
3. **Option C -- 3-variant: preserve `ClosureCapture`.**
   Carry the closure-internal distinction.

**Decision.** Drop the Region slot; 2-variant `BorrowKind`:

```rust
Ref(BorrowKind, Place),

pub enum BorrowKind {
    SharedBorrow,
    UniqueBorrow,
}
```

The discriminator values match stage 3's
`Ty::Ref(translate(t), SharedBorrow|UniqueBorrow)` enum
(`spec.md:4527-4528`, where the discriminator type is
`Provenance` per `spec.md:2458`).

**Phase B realization.** stage-3 already defines
`Provenance { SharedBorrow, UniqueBorrow }` at
`spec.md:2458` with content identical to the abstract
`BorrowKind` proposed here. `spec-snapshot.md` §21.3.1
reuses the stage-3 `Provenance` enum directly; the
realized payload is `Ref(Provenance, Place)`. The
abstract `BorrowKind` name in this audit-trail entry is
an alias for stage-3's `Provenance`; no parallel
sibling-named enum is introduced. This is a
"compositional behavior beats special-case flags"
realization (`../README.md` §2 methodology rule).

**Reasoning.** Two threads:

**Region slot drop.** Stage 3 already pins region erasure
at the type level: `Ty::Ref` carries no region slot; §17
(`spec.md:4846` -- the §17 cross-cutting note) states "no
region material survives in `Ty::Ref` for any type that
enters the IR." The rvalue-level region drop follows from
the type-level rule by composition; no parallel justification
needed. Codegen confirms by ignoring the slot:
`rustc_codegen_ssa/src/mir/rvalue.rs:506-511` patterns
`(_, bk, place)` and synthesises the result type with
`tcx.lifetimes.re_erased`. Charon and mrustc both drop
region from their rvalue-Ref analogs (`prior-art/charon.md`
§3; `prior-art/mrustc.md` §1's BORROW variant has no
region).

**BorrowKind subset.** Three independent collapses:

1. **`Fake` is cleanup-discharged.** The required
   `CleanupPostBorrowck` pass (`is_required: true`) NOPs
   every `Rvalue::Ref(_, BorrowKind::Fake(_), _)` assignment
   statement at `rustc_mir_transform/src/cleanup_post_borrowck.rs:33-43`.
   Cannot reach `optimized_mir`.
2. **`MutBorrowKind` sub-discriminator is codegen-collapsed.**
   `rustc_codegen_ssa/src/mir/rvalue.rs:506-511` invokes
   `bk.to_mutbl_lossy()`, which collapses all `Mut { kind:
   ... }` flavours (`Default`, `TwoPhaseBorrow`,
   `ClosureCapture`) to a single `Mutability::Mut`. The
   sub-discriminator carries no information that affects
   what the C backend emits.
3. **`TwoPhaseBorrow` is borrowck-internal.** The variant
   captures borrowck's "reservation-then-activation" model;
   once borrowck has accepted the body, the borrow has its
   full mutable kind. After borrowck (which runs before
   `optimized_mir`), the distinction has no operational
   meaning. **`ClosureCapture` is closure-internal.** Stage
   14 will lift closures to structs+methods (per
   `../README.md` "C semantics with information
   preservation"). The `ClosureCapture` mut-flavour is an
   annotation on closure-environment field borrows that
   doesn't survive lifting -- closure capture borrows
   become plain `&`/`&mut` of struct fields.

The 2-variant subset is the smallest closed set covering the
reachable post-cleanup content. Stage 3 already chose this
discriminator at the type level (via `Provenance`); stage 7
matches at the rvalue level for consistency.

**Methodology rules invoked.** "Compositional behavior
beats special-case flags" (`../README.md` §2: region
erasure cascades from stage-3 type-level rule; reuse
`Provenance` discriminator naming/content). "Cleanup-pass
artefacts are reject-on-encounter, not variants"
(`../memory.md` §2 stage-7-specific corollary: `Fake`
lives in D-7-3, not in the enum). "C semantics with
information preservation" (`../README.md` §2: the
`MutBorrowKind` sub-discriminator and the `Region` slot
have no C-shape consequence -- drop them).

**Prior-art evidence.**

- `prior-art/rustc.md` §4 (Ref payload, BorrowKind 3-variant
  enum, MutBorrowKind 3-variant sub-enum, FakeBorrowKind
  closures-only origin, codegen `bk.to_mutbl_lossy()`
  collapse, CleanupPostBorrowck Fake-NOP discharge).
- `prior-art/mrustc.md` §1 (mrustc's BORROW variant: `type:
  HIR::BorrowType`, `is_raw: bool`, `val: LValue` -- no
  region, no FakeBorrowKind analog; peer-validates region
  drop and Fake non-existence at the C-target level).
- `prior-art/charon.md` §3 (Charon's `Ref { place, kind,
  ptr_metadata }`; `kind: BorrowKind` is `Shared / Mut /
  TwoPhaseMut / Shallow / UniqueImmutable`; `Shallow` is the
  Fake counterpart -- Charon preserves it because LLBC
  serves verification semantics where the borrow kind
  affects soundness, RegseqModel does not).

**Worked example.** `let r = &mut buf[0];` produces rustc
MIR `Rvalue::Ref(<region '0>, BorrowKind::Mut { kind:
MutBorrowKind::Default }, Place { local: _buf, projection:
[ConstantIndex { offset: 0, ... }] })`. Translation:

```text
mir::Rvalue::Ref(_region, BorrowKind::Mut { kind: _ }, place)
  -> drop _region (stage-3 region-erasure cascade)
  -> collapse Mut { kind: _ } to UniqueBorrow
       (codegen-validated by bk.to_mutbl_lossy())
  -> place -> translate_place(place)
  -> Rvalue::Ref(BorrowKind::UniqueBorrow, Place { local: _buf,
       projection: [ConstantIndex { offset: 0, ... }] })
```

A `Rvalue::Ref(_, BorrowKind::Fake(_), _)` would never
reach this translation: required `CleanupPostBorrowck` has
NOPed the containing statement before `optimized_mir`. If
hand-crafted IR carried it, D-7-3 rejects with an
`unsupported_construct!` diagnostic naming "fake borrow
NOPed by required CleanupPostBorrowck."

**Cross-refs.** `spec-snapshot.md` §21.1 (Ref variant
docstring), §21.3.1 (Provenance reuse note), §21.6 (bridge
translation -- region-drop + MutBorrowKind collapse +
Provenance reuse).

---

### D-7-1.5 -- `RawPtr(RawPtrKind, Place)` payload + RawPtrKind subset [LOCKED]

**Question.** What is the closed `RawPtrKind` subset, and
how does the bridge handle rustc's `FakeForPtrMetadata`
(which is *not* cleanup-pass-discharged and can survive into
`optimized_mir`)?

**Key options.**

1. **Option A -- 3-variant: preserve `FakeForPtrMetadata`.**
   Closes the rustc set. Carries an annotation that has no
   C-shape consequence; downstream consumers ignore.
2. **Option B -- 2-variant + bridge-side collapse to `Not`.**
   `Const`/`Mut`. Bridge translation rule:
   `FakeForPtrMetadata -> Const` (matching rustc codegen's
   own `to_mutbl_lossy()` mapping at
   `rustc_middle/src/mir/statement.rs:994-1003`, which uses
   `Mutability::Not` as a `*const` approximation since "we
   have no type corresponding to a fake borrow").
3. **Option C -- 2-variant + bridge-side collapse to
   `Mut`.** Treat the syntax.rs:189-197 doc comment
   "operationally equivalent to `RawPtrKind::Mut`" as the
   collapse target. Diverges from rustc codegen.

**Decision.** Option B:

```rust
RawPtr(RawPtrKind, Place),

pub enum RawPtrKind {
    Const,
    Mut,
}
```

Bridge translation rule (filed in §15 / §21.6):
`mir::RawPtrKind::FakeForPtrMetadata -> RawPtrKind::Const`.

**Phase B realization.** stage-3 already defines
`Mutability { Not, Mut }` at `spec.md:2461` with content
conceptually identical to the abstract 2-variant
`RawPtrKind`. `spec-snapshot.md` §21.3.2 reuses the
stage-3 `Mutability` enum directly; the realized payload
is `RawPtr(Mutability, Place)`. The mapping from rustc's
3-variant `RawPtrKind` to stage-3 `Mutability` is:
`RawPtrKind::Const -> Mutability::Not`,
`RawPtrKind::Mut -> Mutability::Mut`,
`RawPtrKind::FakeForPtrMetadata -> Mutability::Not` (the
bridge collapse rule above; matches rustc codegen's
`to_mutbl_lossy()` at `statement.rs:1001`). The abstract
`RawPtrKind` name in this audit-trail entry is an alias
for stage-3's `Mutability` with the documented mapping;
no parallel sibling-named enum is introduced. This is a
"compositional behavior beats special-case flags"
realization (`../README.md` §2 methodology rule).

**Reasoning.** `FakeForPtrMetadata` is reachable in driver
code: source-level constructs like
`<*const [T]>::len()` and `<*const dyn Trait>::metadata()`
generate `Rvalue::RawPtr(RawPtrKind::FakeForPtrMetadata,
Place)` to read pointer metadata without dereferencing the
data. Unlike `BorrowKind::Fake`, `FakeForPtrMetadata` is
NOT discharged by `CleanupPostBorrowck`; it survives.
Reject-on-encounter would reject valid driver code.

The bridge collapse target (`Not`) matches rustc codegen's
own choice. `RawPtrKind::to_mutbl_lossy()` at
`rustc_middle/src/mir/statement.rs:994-1003` maps
`FakeForPtrMetadata` to `Mutability::Not` ("we have no type
corresponding to a fake borrow, so use `*const` as an
approximation," per the inline comment at
`statement.rs:999-1000`); rustc codegen invokes this
mapping at `rustc_codegen_ssa/src/mir/rvalue.rs:513-515`
when synthesising the RawPtr result type via
`Ty::new_ptr(tcx, ty, kind.to_mutbl_lossy())`. The
syntax.rs:189-197 doc comment "operationally equivalent
to `RawPtrKind::Mut`, but will never incur a retag"
describes Miri retag semantics (a stack-borrows /
tree-borrows abstract-machine concept); rustc's actual
codegen behaviour is the `to_mutbl_lossy()` mapping to
`Not`. Documentation and codegen disagree; the codegen
rule is the operational ground truth and is what
RegseqModel mirrors.

Charon also collapses `FakeForPtrMetadata` to `Shared` /
read-only (see `prior-art/charon.md` §4); peer-validates
that read-only is the correct collapse target when the
output language has no fake-borrow representation.
RegseqModel adopts the rustc-codegen target (`Not` /
`*const`), which agrees with Charon's choice.

The bridge-lowering pattern matches stage-6 D-6-1.2:
when an rvalue/operand variant carries information whose
only meaning is in a domain we do not model (Miri
abstract-machine retags here; per-crate-graph session
flags in D-6-1.2), collapse at bridge time to the
documented operationally-equivalent form.

**Methodology rules invoked.** "C semantics with
information preservation" (preserve mutability as a C-shape
qualifier; drop the retag-internal sub-discriminator);
"Re-derive from evidence; do not import baggage" (rustc's
3-variant enum carries Miri infrastructure not mirrored in
RegseqModel; collapse rather than carry); "Compositional
behavior beats special-case flags" (the bridge-lowering
pattern is the same pattern as D-6-1.2 -- one mechanism for
"this carries information meaningful only in a domain we
do not model").

**Prior-art evidence.**

- `prior-art/rustc.md` §6 (RawPtr payload + RawPtrKind
  3-variant enum + FakeForPtrMetadata "operationally
  equivalent to Mut" doc-comment).
- `prior-art/mrustc.md` §1 (mrustc's BORROW variant uses
  `is_raw: bool` to distinguish ref-vs-raw-ptr; the
  `BorrowType` carries `Shared`/`Unique`/`Owned`-style
  values not Const/Mut; mutability is implicit in the
  borrow type. Validates the 2-variant collapse target -- a
  C-targeting peer also collapses the discriminator to
  Mut/Const-equivalent).
- `prior-art/charon.md` §4 (Charon's `RawPtrKind` 3-variant
  enum: `Mut`/`Const`/`FakeForPtrMetadata`; translation rule
  collapses `FakeForPtrMetadata` to `Shared` for
  verification reasons).

**Worked example.** `(buf as *const [u8]).len()` produces
rustc MIR

```text
_2 = &raw const (*_buf);
_3 = (_2 as &[u8]); ; conceptual unsizing
_4 = Len(_3);
```

After elaboration, the metadata-extraction may surface as
`Rvalue::RawPtr(RawPtrKind::FakeForPtrMetadata,
Place::from(_buf))` followed by a metadata projection.
Translation:

```text
mir::Rvalue::RawPtr(RawPtrKind::FakeForPtrMetadata, place)
  -> collapse FakeForPtrMetadata -> Mutability::Not
  -> place -> translate_place(place)
  -> Rvalue::RawPtr(Mutability::Not, translate_place(place))
```

The C backend lowers identically to a `*const` raw-ptr
construction (matching rustc codegen's own choice via
`to_mutbl_lossy()`); downstream stage-14 code that consumes
the metadata reads it via the place's `Ty::Ptr`
discriminator (stage 3, §10) plus `Ty::Slice`/`Ty::Dynamic`
pointee-shape information.

**Cross-refs.** `spec-snapshot.md` §21.1 (RawPtr variant
docstring), §21.3.2 (Mutability reuse note), §21.6.2
(FakeForPtrMetadata collapse rule).

---

### D-7-1.6 -- `Cast(CastKind, Operand, TyId)` payload + CastKind subset [LOCKED]

**Question.** What is the closed `CastKind` subset
(including the `PointerCoercion` sub-enum), and how are
rustc's diagnostic-only / structurally-duplicated
annotations handled?

**Key options.** Several sub-questions woven together:

1. **CastKind cardinality.** All 11 rustc variants?
   Curated subset?
2. **PointerCoercion cardinality.** All 6 sub-variants?
   Or drop the cleanup-discharged pair?
3. **CoercionSource handling.** Preserve the diagnostic-
   only `AsCast` / `Implicit` annotation, or drop?
4. **`hir::Safety` on `ReifyFnPointer` / `ClosureFnPointer`.**
   Preserve, or drop as redundant with destination
   `Ty::FnPtr`'s signature?

**Decision.** 11-variant `CastKind` + 4-variant
`PointerCoercion` sub-enum. Drop `CoercionSource` and
`hir::Safety` from `ReifyFnPointer` / `ClosureFnPointer`.

```rust
Cast(CastKind, Operand, TyId),

pub enum CastKind {
    PointerExposeProvenance,
    PointerWithExposedProvenance,
    PointerCoercion(PointerCoercion),
    IntToInt,
    FloatToInt,
    FloatToFloat,
    IntToFloat,
    PtrToPtr,
    FnPtrToPtr,
    Transmute,
    Subtype,
}

pub enum PointerCoercion {
    ReifyFnPointer,
    UnsafeFnPointer,
    ClosureFnPointer,
    Unsize,
}
```

Bridge translation rules (filed in §15 / §21.6):

- `CastKind::PointerCoercion(MutToConstPointer | ArrayToPointer, _)`
  -> reject (cleanup-discharged via D-7-3).
- `CastKind::PointerCoercion(p, _coercion_source)` -> drop
  `CoercionSource`.
- `PointerCoercion::ReifyFnPointer(_safety)` /
  `ClosureFnPointer(_safety)` -> drop `hir::Safety`.

**Reasoning.** Four threads:

1. **CastKind cardinality.** All 11 rustc variants are
   reachable in `optimized_mir`:
   `PointerExposeProvenance` / `PointerWithExposedProvenance`
   (stable address-exposure semantics);
   `PointerCoercion` (function pointer reification, unsizing);
   `IntToInt` / `FloatToInt` / `FloatToFloat` / `IntToFloat`
   / `PtrToPtr` / `FnPtrToPtr` (arithmetic and pointer
   conversions); `Transmute` (`mem::transmute` survives);
   `Subtype` (added by `Subtyper` runtime-lowering pass at
   `rustc_mir_transform/src/lib.rs:644`). Drivers exercise
   most of these; aggressive curation would reject valid
   code.
2. **PointerCoercion cardinality.** Rustc's 6-variant
   `PointerCoercion` (`rustc_middle/src/ty/adjustment.rs:11-40`)
   has two cleanup-discharged members: `MutToConstPointer`
   and `ArrayToPointer` are rewritten to `CastKind::PtrToPtr`
   by required `CleanupPostBorrowck`
   (`rustc_mir_transform/src/cleanup_post_borrowck.rs:44-60`).
   Codegen also rejects them with `bug!` at
   `rustc_codegen_ssa/src/mir/rvalue.rs:448-452`. Twice-
   confirmed unreachable -- drop, send to D-7-3 rejection.
   The remaining 4 (`ReifyFnPointer`, `UnsafeFnPointer`,
   `ClosureFnPointer`, `Unsize`) survive and have C-shape
   consequences (function-pointer reification, fat-pointer
   unsizing).
3. **CoercionSource drop.** `rustc_middle/src/mir/syntax.rs:1531-1536`
   defines `CoercionSource` as a 2-variant enum
   (`AsCast` / `Implicit`). Its only purpose is diagnostic
   -- to distinguish "user wrote `x as T`" from "compiler
   inserted the coercion." Codegen at
   `rustc_codegen_ssa/src/mir/rvalue.rs:408-447` ignores it
   entirely. The diagnostic value is recoverable at
   bridge-rejection time via the containing Statement's
   `SourceInfo` span (stage 8); operand-internal
   provenance tags add no information stage-8 spans don't
   already carry.
4. **`hir::Safety` drop from ReifyFnPointer / ClosureFnPointer.**
   The variant carries the *source* fn-item's safety
   (`Safe` vs `Unsafe`). The result type (the destination
   `Ty<'tcx>` carried in `Cast`'s third slot) is a
   `Ty::FnPtr` whose `FnSig` already encodes the
   *destination* safety (per stage 3 §14: `FnSig` content
   includes `FnAbi` and safety). Source-safety and
   destination-safety are equivalent for these PointerCoercion
   variants -- the cast preserves safety. Carrying both is
   structural duplication; the type-system has the answer
   via `Ty::FnPtr(FnSigId)`.

The 11+4 surface is the smallest closed set covering the
reachable post-cleanup content; the two annotation drops
(`CoercionSource`, `hir::Safety`) follow the D-6-1.3 pattern
of dropping carriers whose only meaning is in a domain we
do not separately model (diagnostic origin; type-system
duplication).

**Methodology rules invoked.** "Closed `Rvalue`
vocabulary" (`../memory.md` §2 stage-7-specific principle:
smallest reachable set; specializes README §2 "core is
opaque" rule's "items outside the recognized vocabulary
are rejected" pattern). "Cleanup-pass artefacts are
reject-on-encounter, not variants" (`../memory.md` §2
stage-7-specific corollary: `MutToConstPointer` /
`ArrayToPointer` -> D-7-3). "Compositional behavior beats
special-case flags" (`../README.md` §2: `hir::Safety` is
in the destination `Ty::FnPtr`; do not duplicate).
"Re-derive from evidence; do not import baggage"
(`../README.md` §2: `CoercionSource` is rustc-internal
diagnostic provenance not load-bearing post-bridge).

**Prior-art evidence.**

- `prior-art/rustc.md` §7 (CastKind 11-variant enum;
  PointerCoercion 6-variant enum; `CoercionSource`
  diagnostic-only doc; `MutToConstPointer` / `ArrayToPointer`
  CleanupPostBorrowck discharge + codegen `bug!`).
- `prior-art/mrustc.md` §4 (mrustc's `Cast` carries no
  `CastKind` discriminator -- a structurally different
  choice; the cast operation is implicit in the destination
  type. Validates that some peers go *flatter* than
  RegseqModel's 11+4 surface; we keep CastKind because the
  operation kind affects backend codegen paths).
- `prior-art/charon.md` §5 (Charon folds `Cast` into
  `UnaryOp::Cast(...)` and has 6 effective CastKind variants;
  structurally divergent from RegseqModel's choice but
  validates that closed-CastKind subsets are workable).

**Worked example.** `let f: fn(u32) -> u32 = my_fn;`
produces rustc MIR `Rvalue::Cast(CastKind::PointerCoercion(
PointerCoercion::ReifyFnPointer(Safety::Safe),
CoercionSource::Implicit), Operand::Constant(<my_fn DefId>),
<destination Ty: fn(u32) -> u32>)`. Translation:

```text
mir::Rvalue::Cast(CastKind::PointerCoercion(pc, _coercion_source),
                  operand, dest_ty)
  -> drop CoercionSource
  -> match pc:
       PointerCoercion::ReifyFnPointer(_safety) -> drop hir::Safety
       PointerCoercion::ClosureFnPointer(_safety) -> drop hir::Safety
       PointerCoercion::UnsafeFnPointer / Unsize -> as-is
       PointerCoercion::MutToConstPointer / ArrayToPointer -> reject (D-7-3)
  -> dest_ty -> translate_ty -> TyId
  -> Rvalue::Cast(CastKind::PointerCoercion(PointerCoercion::ReifyFnPointer),
                  translate_operand(operand), TyId)
```

A would-be `Cast(CastKind::PointerCoercion(MutToConstPointer,
_), _, _)` -- which CleanupPostBorrowck has already rewritten
to `PtrToPtr` before `optimized_mir` -- triggers D-7-3
rejection if hand-crafted IR includes it.

**Cross-refs.** `spec-snapshot.md` §21.1 (Cast variant
docstring), §21.3.3 (CastKind / PointerCoercion sub-enums),
§21.6.3 (bridge translation -- CoercionSource drop, hir::Safety
drop, PointerCoercion::{MutToConstPointer,ArrayToPointer}
rejection).

---

### D-7-1.7 -- `BinaryOp` + `UnaryOp` payloads + operand-pair storage [LOCKED]

**Question.** Three sub-questions: (1) `BinOp` variant set,
(2) `UnOp` variant set, (3) `BinaryOp` operand-pair storage
(`Box<(Operand, Operand)>` vs flat).

**Key options.** For the 26-variant `BinOp` and 3-variant
`UnOp`, the question is curate vs preserve. For storage:

1. **Option A -- flat: `BinaryOp(BinOp, Operand, Operand)`.**
   Charon precedent. Larger enum; zero allocation per
   BinaryOp.
2. **Option B -- boxed: `BinaryOp(BinOp, Box<(Operand,
   Operand)>)`.** rustc precedent. Smaller enum; one extra
   allocation per BinaryOp.

**Decision.** Preserve all 26 `BinOp` variants and all 3
`UnOp` variants verbatim. Flat operand-pair storage:

```rust
BinaryOp(BinOp, Operand, Operand),
UnaryOp(UnOp, Operand),

pub enum BinOp {
    // 26 variants verbatim from rustc:
    Add, AddUnchecked, AddWithOverflow,
    Sub, SubUnchecked, SubWithOverflow,
    Mul, MulUnchecked, MulWithOverflow,
    Div, Rem,
    BitXor, BitAnd, BitOr,
    Shl, ShlUnchecked,
    Shr, ShrUnchecked,
    Eq, Lt, Le, Ne, Ge, Gt,
    Cmp,
    Offset,
}

pub enum UnOp {
    Not,
    Neg,
    PtrMetadata,
}
```

**Reasoning.** Three threads:

**BinOp preservation.** Each of rustc's 26 variants
(`rustc_middle/src/mir/syntax.rs:1615-1703`) maps to a
distinct C lowering or runtime helper:

- Wrapping arith (`Add`/`Sub`/`Mul`/`Div`/`Rem`) -> C
  operator with explicit unsigned-wrapping for unsigned
  types.
- UB-on-overflow (`AddUnchecked`/`SubUnchecked`/`MulUnchecked`,
  emitted by `unchecked_add`/`unchecked_sub`/etc.
  intrinsics) -> C operator (UB-on-overflow lowering is a
  stage-14 backend concern, not a stage-7 spec concern).
- Overflow-detecting (`AddWithOverflow`/`SubWithOverflow`/
  `MulWithOverflow`, emitted by overflow-check lowering;
  return `(T, bool)`) -> C-helper-style intrinsic returning
  a 2-field struct.
- Bitwise (`BitXor`/`BitAnd`/`BitOr`) -> C operator.
- Shifts (`Shl`/`ShlUnchecked`/`Shr`/`ShrUnchecked`) -> C
  operator with masking per `RHS.rem_euclid(LHS::BITS)`
  semantics.
- Comparisons (`Eq`/`Lt`/`Le`/`Ne`/`Ge`/`Gt`) -> C operator.
- Three-way comparison (`Cmp`, returns
  `core::cmp::Ordering` as `i8`) -> conditional ladder.
- Pointer offset (`Offset`, `<*const T>::offset` semantics)
  -> C pointer arithmetic.

Each has a stage-14 backend lowering; collapsing any pair
(e.g., merging `Add` and `AddUnchecked`) would lose
information rustc paid to compute. Preserve verbatim.

**UnOp preservation.** All 3 variants reach `optimized_mir`:
`Not` (logical inversion `!`), `Neg` (arithmetic negation
`-`), `PtrMetadata` (extracts metadata from a fat pointer).
`PtrMetadata` is `MirPhase::Runtime`-only
(`rustc_middle/src/mir/syntax.rs:1609`); `optimized_mir` IS
Runtime; reachable. Each has a direct C lowering. Preserve.

**Pair storage (flat over boxed).** rustc's
`Box<(Operand<'tcx>, Operand<'tcx>)>` is a size-budget
optimisation rustc needs and we don't:

- rustc's `static_assert_size!(Rvalue<'_>, 40)` at
  `rustc_middle/src/mir/syntax.rs:1752` reflects rustc's
  body-iteration access pattern (multiple compiler passes
  over the same body; cache pressure is real).
- RegseqModel constructs bodies once at bridge time and
  consumes them once at codegen time; per-Rvalue size is
  not load-bearing at our scale.
- Charon stores `BinaryOp(BinOp, Operand, Operand)` flat
  (`prior-art/charon.md` §6); validates that flat works at
  production scale for a verification frontend with similar
  body-construction-then-consumption access pattern.
- Pattern-match ergonomics: `Rvalue::BinaryOp(op, lhs, rhs)
  =>` is cleaner than `Rvalue::BinaryOp(op, box (lhs, rhs))
  =>`. Stage-14 backend code reads better.

After stage 6's `Operand::Constant` shrinkage to bare
`ConstId` (D-6-1.3), stage-7's `Operand` is dominated by
`Place` (~24 bytes). Two operands flat ~48 bytes payload +
tag puts the BinaryOp variant around 56 bytes; with Box,
the variant is around 16 bytes but the Rvalue enum's max
variant is dominated by other variants (e.g.,
`Cast(CastKind, Operand, TyId)` ~32-36 bytes) so the
Rvalue's overall enum size goes from ~40 (boxed) to ~56
(flat). At hundreds of Rvalues per body and bodies
accessed twice, the total memory delta is in the noise.

The size-budget micro-optimisation rustc needs does not
apply to RegseqModel; the allocation cost rustc pays
(one Box per BinaryOp) is the cost of the optimisation.
Stage 6's "drop Box where possible" pattern (D-6-1.3 dropped
`Box<ConstOperand>` to inline `ConstId`) extends here.

**Methodology rules invoked.** "C semantics with information
preservation" (preserve every BinOp / UnOp variant -- each
informs distinct C lowering); "Re-derive from evidence; do
not import baggage" (rustc's Box is a rustc-internal size-
budget concern; do not import); "Compositional behavior
beats special-case flags" (the flat-storage choice extends
D-6-1.3's pattern of "inline when possible").

**Prior-art evidence.**

- `prior-art/rustc.md` §8 (BinaryOp Box-pair, BinOp 26-variant
  enum, size-assert), §9 (UnaryOp, UnOp 3-variant enum,
  PtrMetadata Runtime-phase qualifier).
- `prior-art/mrustc.md` §5 (mrustc's `BinOp` operands stored
  flat; peer-validated -- no Box at the C-target level), §6
  (`UniOp` operand: LValue, not Param -- a different
  structural choice we don't follow), §7 (mrustc's `eBinOp`:
  21 variants, simpler than rustc -- mrustc lacks
  `*WithOverflow` / `*Unchecked` because the C target
  doesn't model overflow-detecting arith at the IR level;
  validates that 26 is reasonable for a richer-arith C
  target where overflow checks matter).
- `prior-art/charon.md` §6 (Charon's `BinaryOp(BinOp, Operand,
  Operand)` flat; UnOp `Not` / `Neg` / `Cast` -- Charon
  folds Cast into UnOp, structurally divergent; preserves
  flat-pair storage).

**Worked example.** `a.wrapping_add(b)` produces rustc MIR
`Rvalue::BinaryOp(BinOp::Add, box (Operand::Copy(_a),
Operand::Copy(_b)))`. Translation:

```text
mir::Rvalue::BinaryOp(op, box (lhs, rhs))
  -> Rvalue::BinaryOp(op, translate_operand(lhs), translate_operand(rhs))
  -> Rvalue::BinaryOp(BinOp::Add, Operand::Copy(_a), Operand::Copy(_b))
```

The Box is unwrapped at bridge time; downstream stages see
flat operand positions.

**Cross-refs.** `spec-snapshot.md` §21.1 (BinaryOp / UnaryOp
variant docstrings), §21.3.4 / §21.3.5 (BinOp / UnOp
sub-enums), §21.6 (bridge translation -- Box unwrap rule).

---

### D-7-1.8 -- `Discriminant` + `Aggregate` payloads + `AggregateKind` subset [LOCKED]

**Question.** Three threads woven: (1) `Discriminant`
payload, (2) `Aggregate` operand-vec storage, (3)
`AggregateKind` subset including the Adt-encoding
question, the Coroutine-variant fate, and the
`UserTypeAnnotationIndex` slot.

**Key options.** Major options on the AggregateKind shape:

1. **Option A -- single Adt variant, rustc-shaped.**
   `Adt(TyId, VariantIdx, Option<FieldIdx>)`. rustc's
   five-slot encoding minus `(DefId, GenericArgsRef)`
   collapse and `UserTypeAnnotationIndex` drop. Carries
   possibly-invalid combinations (e.g., `Enum(TyId, 0,
   Some(field))` is structurally representable but
   semantically invalid).
2. **Option B -- three variants matching stage-3
   NominalKind.** `Struct(TyId)` / `Enum(TyId, VariantIdx)`
   / `Union(TyId, FieldIdx)`. Compositional with stage 3's
   `NominalKind { Struct, Enum, Union, FnDef, Closure }`
   (`spec.md:2896`) and the parallel `Ty::Struct` /
   `Ty::Enum` / `Ty::Union` translation table
   (`spec.md:4555-4557`). No representable invalid states.

**Decision.** Discriminant: direct embed. Aggregate
operand storage: `Box<[Operand]>` (matches stage-5 PlaceElem
storage convention). AggregateKind: 7-variant subset under
**Option B** (three-way Adt split), with `Tuple(TyId)` for
symmetric type-naming across all 7 variants:

```rust
Discriminant(Place),
Aggregate(AggregateKind, Box<[Operand]>),

pub enum AggregateKind {
    Array(TyId),                       // destination array type Ty::Array(elem, n)
    Tuple(TyId),                       // destination tuple type Ty::Tuple { fields, layout }
    Struct(TyId),                      // TyId resolves to Ty::Struct
    Enum(TyId, VariantIdx),            // TyId resolves to Ty::Enum; active variant
    Union(TyId, FieldIdx),             // TyId resolves to Ty::Union; active field
    Closure(TyId),                     // TyId resolves to Ty::Closure (identity view per §13.1)
    RawPtr(TyId, Mutability),          // pointee TyId; mutability ties to Ty::Ptr discriminator
}
```

Bridge translation rules (filed in §15 / §21.6):

- `Adt(did, vidx, args, _user_ty, active_field)` ->
  - struct (per `tcx.adt_def(did).is_struct()`):
    `Struct(<TyId of Ty::Struct{translate(did), translate(args), layout}>)`
  - enum (per `is_enum()`): `Enum(<TyId>, vidx)`
  - union (per `is_union()`): `Union(<TyId>, active_field.unwrap())`
  Drop the `Option<UserTypeAnnotationIndex>` slot.
- `Closure(did, args)` -> `Closure(<TyId of
  Ty::Closure(translate(did), translate(args))>)` --
  identity view, per §13.1's two-views design.
- `Coroutine(...)` / `CoroutineClosure(...)` -> reject
  (D-7-3); double-grounded by §15.1 type-level rejection
  (`spec.md:4581`) plus StateTransform discharge.
- `Tuple` -> `Tuple(translate(rvalue.ty(body, tcx)))`.
  rustc's `Aggregate(Tuple, ops)` does not carry a type
  slot; the bridge obtains the destination tuple type via
  `rvalue.ty(body, tcx)` (rustc's MIR Rvalue type-of
  helper). Translation routes through stage-3
  `Ty::Tuple { fields, layout }` interning where
  `fields = elem_tys` and `layout` is sourced from
  `tcx.layout_of` per §15 (parallel to stage-3's tuple
  translation rule at `spec.md:4530`).

**Reasoning.** Five threads, organised by structural area:

1. **Discriminant direct.** `Rvalue::Discriminant(Place)`
   reads the discriminant of an enum place and returns it
   as an integer of type `discriminant_ty`. The payload is
   trivially a `Place`; no curation.

2. **Operand-vec storage.** rustc's `IndexVec<FieldIdx,
   Operand>` is a `Vec` with a typed index; the FieldIdx
   typing is purely cosmetic. Stage-5 D-5-1 chose
   `Box<[PlaceElem]>` for projection storage (immutable
   IR; saves the `Vec` capacity word). Stage 7 follows
   the same convention for AggregateKind operands.

3. **Adt three-way split (Option B).** rustc's encoding
   (`rustc_middle/src/mir/syntax.rs:1545-1550`) is
   "three-states-in-five-slots":
   - Struct: `VariantIdx == FIRST_VARIANT (0)`,
     `active_field == None`.
   - Enum: `VariantIdx == <active variant>`,
     `active_field == None`.
   - Union: `VariantIdx == FIRST_VARIANT (0)`,
     `active_field == Some(<field>)`.
   The five-slot single-variant encoding admits
   structurally-representable-but-semantically-invalid
   combinations (e.g., `Adt(<enum TyId>, 0, Some(field))`).
   Stage-3's type taxonomy already separates the three
   nominal kinds: `Ty::Struct { item, args, layout }` /
   `Ty::Enum { item, args, layout }` / `Ty::Union { item,
   args, layout }` (`spec.md:4555-4557`); the
   `NominalKind` enum (`spec.md:2896`: `Struct, Enum,
   Union, FnDef, Closure`) is the canonical discriminator.
   The three-way `AggregateKind` split aligns 1:1 with the
   type-side discriminator -- compositional with stage 3,
   no representable invalid states.

4. **Adt collapse via TyId.** rustc's
   `Adt(DefId, VariantIdx, GenericArgsRef, ...)` carries a
   `(DefId, GenericArgsRef)` pair that identifies a
   monomorphised Adt. Stage 3's `Ty::Struct`/`Enum`/`Union`
   already store `(item, args, layout)` for the same
   monomorphised Adt; both encodings carry the *same*
   information. Replacing the `(DefId, GenericArgsRef)` pair
   with a `TyId` resolving to `Ty::Struct` / `Ty::Enum` /
   `Ty::Union` composes through stage-3's type interner
   without information loss. The Adt's `AdtFlags` cache
   (`spec.md:1213-1304`) is also reachable through the type
   interner. Stage-3's `find_nominal(NominalKind, item,
   args)` lookup (`spec.md:3306, 4968, 5188`) is the bridge
   construction primitive that produces the TyId in the
   first place; AggregateKind references the result.

5. **Closure identity view.** §13.1 (`spec.md:3231,
   3237-3304`) pins each closure as TWO `Ty` entries
   sharing one synthesised Item:
   - `Ty::Closure(item, args)` -- closure-type-identity
     view (canonical form).
   - `Ty::Struct(synthesised_item, args, layout)` -- the
     parallel captures-struct layout view, reached via
     `find_nominal(NominalKind::Struct, item, args)`.
   AggregateKind::Closure references the *identity view*
   (`Ty::Closure`); the layout view is reachable for
   codegen via the parallel-lookup at stage-14 backend
   time. This preserves "what kind of construction is
   this" in the AggregateKind tag while the type-system
   handles the layout-view duplication.

6. **Coroutine drop (double-grounded).** §15.1 type-level
   rejection table (`spec.md:4581`) already rejects
   `Coroutine` / `CoroutineClosure` / `CoroutineWitness`
   at type translation: "async / coroutine state machines;
   outside `#![no_std]` driver DSL." The corresponding
   AggregateKind variants would construct values of
   `Ty::Coroutine`/etc. -- types stage 3 rejects. By
   composition, the AggregateKind variants are unreachable
   in valid IR. Independently, the `StateTransform`
   runtime-lowering pass (`rustc_mir_transform/src/lib.rs:660`)
   rewrites coroutine bodies into state-machine struct
   construction; `rustc_middle/src/mir/syntax.rs:1457-1458`
   states "After coroutine lowering, `Coroutine` aggregate
   kinds are disallowed too." Two independent paths to
   unreachability -- the type-level rejection is the
   stronger, harder wall.

7. **UserTypeAnnotationIndex drop (D-6-1.3 precedent).**
   rustc's `AggregateKind::Adt(_, _, _, Option<UserTypeAnnotationIndex>,
   _)` 4th slot is an NLL/borrowck-only carrier. Required
   `CleanupPostBorrowck` clears the body-level annotation
   table (`body.user_type_annotations.raw.clear()`,
   `cleanup_post_borrowck.rs:81-85`); the codegen consumer
   (`rustc_codegen_ssa/src/mir/rvalue.rs:670-672`) ignores
   the slot. Same drop pattern as D-6-1.3 (which dropped
   `user_ty` from `ConstOperand`).

8. **Tuple(TyId) for symmetry.** rustc's `AggregateKind::Tuple`
   is the only zero-payload variant; the tuple's destination
   type is reconstructable from operand types but doing so
   at the C backend means running type-of inference at
   lowering time. Carrying `TyId` makes the destination type
   explicit. Stage 3's `Ty::Tuple { fields, layout }` is
   interned (where `fields = elem_tys` is the translated
   element list and `layout` is sourced from `tcx.layout_of`
   per `spec.md:4530`); the bridge can supply the TyId at
   translation time. This
   makes the AggregateKind surface uniformly "every variant
   names its destination TyId" -- a structural symmetry
   that matches stage-7's compositional preferences.

**Methodology rules invoked.** "Compositional behavior
beats special-case flags" (`../README.md` §2: three-way
Adt split via stage-3 NominalKind; closure identity view
via §13.1; Coroutine drop via §15.1 type-level rejection).
"Closed `Rvalue` vocabulary" (`../memory.md` §2
stage-7-specific principle: smallest reachable set; no
Coroutine/CoroutineClosure). "C semantics with
information preservation" (`../README.md` §2: carry TyId
for every construction; drop UserTypeAnnotationIndex
which has no C-shape consequence). "Re-derive from
evidence; do not import baggage" (`../README.md` §2:
rustc's three-states-in-five-slots Adt encoding is
rustc-internal; re-derive cleaner).

**Prior-art evidence.**

- `prior-art/rustc.md` §10 (Discriminant payload, Aggregate
  payload, AggregateKind 7-variant enum, size_asserts),
  §13 (cleanup-pass whitelist context for
  UserTypeAnnotationIndex clearing).
- `prior-art/mrustc.md` §10 (mrustc's aggregate kinds split
  per-shape: `MakeTuple`/`MakeArray`/`MakeStruct`/`MakeEnumVariant`
  -- a peer that already adopted the per-shape split, validating
  Option B; mrustc has no closure-construction at this level
  because mrustc's closures are pre-lifted; mrustc has no
  Coroutine analog).
- `prior-art/charon.md` §9 (Charon's `AggregateKind`:
  3-variant peer surface -- `Aggregate { kind, fields }`
  with `kind: AggregateKind = Adt | Array | Closure`;
  Charon's Adt covers Struct/Enum/Union via the Ty's
  NominalKind discriminator. Validates that closed
  AggregateKind subsets are workable; structurally less
  granular than RegseqModel's 7-variant choice).

**Worked example.** A struct construction
`Foo { x: 1, y: 2 }`:

```text
mir::Rvalue::Aggregate(box AggregateKind::Adt(<Foo did>,
                            FIRST_VARIANT, <args>, None, None),
                       [Operand::Constant(<1>), Operand::Constant(<2>)])
  -> dispatch on tcx.adt_def(<Foo did>).is_struct() -> true
  -> look up TyId of Ty::Struct{<translated did>, <translated args>, layout}
       via find_nominal(NominalKind::Struct, item, args)
  -> drop UserTypeAnnotationIndex (cleared by CleanupPostBorrowck)
  -> drop Option<FieldIdx> active_field (None for struct)
  -> Rvalue::Aggregate(AggregateKind::Struct(TyId),
       Box::new([Operand::Constant(<1>), Operand::Constant(<2>)]))
```

A closure expression `|x| x + cap`:

```text
mir::Rvalue::Aggregate(box AggregateKind::Closure(<closure did>, <args>),
                       [Operand::Copy(<cap_local>)])
  -> look up TyId of Ty::Closure(<translated did>, <translated args>)
       (identity view per §13.1)
  -> Rvalue::Aggregate(AggregateKind::Closure(TyId),
       Box::new([Operand::Copy(<cap_local>)]))
```

The captures-struct layout view (`Ty::Struct` per §13.1) is
reachable for codegen at stage 14 via
`find_nominal(NominalKind::Struct, item, args)`.

A would-be coroutine construction
`AggregateKind::Coroutine(<async_block_did>, <args>)`
triggers D-7-3 rejection if hand-crafted IR includes it;
in practice, both §15.1 type-level rejection
(`spec.md:4581`) and `StateTransform` runtime-lowering
remove it before `optimized_mir`.

**Cross-refs.** `spec-snapshot.md` §21.1 (Discriminant /
Aggregate variant docstrings), §21.3.6 (AggregateKind
sub-enum), §21.6.4 (bridge translation -- Adt three-way
dispatch, UserTypeAnnotationIndex drop, Coroutine
rejection, Closure identity-view selection, Tuple TyId
materialisation), §13.1 (closure-family two-views design;
referenced for the identity-view rule).

---

### D-7-2 -- Rvalue storage and interning [LOCKED]

**Question.** Where does an `Rvalue` live in the model --
inline at use sites, or via an `RvalueId` indirection into
a top-level interner?

**Key options.**

1. **Option A -- inline at use sites.** `Rvalue` is
   embedded by value into stage-8 `Statement::Assign(Place,
   Rvalue)` and stage-9 terminator payloads (transitively).
   No `RvalueId`, no interner.
2. **Option B -- `RvalueId` interner.** Mirror stage 4's
   `Const` interning; introduce `RvalueInterner` and
   reference rvalues by `RvalueId`.

**Decision.** Option A. Rvalues live inline at use sites;
no `RvalueId`; no rvalue-level interner.

**Reasoning.** Two concrete arguments against interning:

1. **No downstream consumer of rvalue-content-uniqueness.**
   `Const` interning exists because `Const`-content-identity
   matters for downstream consumers (stage-3
   `GenericArg::Const(ConstId)` participates in `Ty`-identity;
   stage-9 instance-key construction reads `ConstId`s; F4-2
   cross-build identity threads through them). Rvalues have
   no analogous consumer -- no IR structure consumes "two
   rvalue uses are content-equivalent."
2. **Heterogeneous, larger payloads.** Rvalue variants are
   structurally heterogeneous and larger than Operand (the
   max payload is `BinaryOp(BinOp, Operand, Operand)` after
   D-7-1.7's flat-pair choice, ~56 bytes). Interning's
   overhead (a hash lookup at construction; ID dereference
   at use) buys nothing without a content-uniqueness
   consumer.

This is the same reasoning as D-5-2 (places) and D-6-2
(operands). Stages 5, 6, 7 all conclude inline; the chain
is consistent.

**Methodology rules invoked.** "Compiler-theory-first"
(interning is a deduplication optimisation; without a
deduplication consumer, it's premature abstraction); "Eager
translation" (Q2) does not require interning; it requires
fully-owned IR, which inline-by-value satisfies trivially.

**Prior-art evidence.** rustc's MIR has no rvalue interner
(`prior-art/rustc.md` §1 and §14). `rustc_public` follows
the same pattern (`prior-art/rustc.md` §12). mrustc inlines
at use sites (`prior-art/mrustc.md` §1). Charon has a
generic `HashConsed<T>` infrastructure but does not apply
it to Rvalue (`prior-art/charon.md` §1, §11). Four-out-of-
four peers store inline.

**Worked example.** A function with two consecutive
statements `_2 = _1 + 1` and `_3 = _1 + 1` produces MIR with
two `Rvalue::BinaryOp(BinOp::Add, Operand::Copy(_1),
Operand::Constant(<1>))` rvalue slots. Under Option B, both
would resolve to the same `RvalueId` via interning -- a
single deduplication step saving ~56 bytes per repeat.
Under Option A (this decision), each `StatementKind::Assign`
carries its own `Rvalue::BinaryOp(...)` inline. The "saved"
~56 bytes per duplicate is dwarfed by the cost of a
parallel `RvalueInterner` table, the `RvalueId` type and
serde derives, and the indirection at every use site -- all
to deduplicate something no downstream consumer asks
about. Inline storage is strictly cheaper.

F4-2 cross-build identity is OUT of scope for Rvalue (per
D-7-3 part D): rvalue is body-local content; the F4-2
instance-ordering canonicalisation reads `(InstanceKey)`,
not `(BodyContents)`.

**Cross-refs.** `spec-snapshot.md` §21.4 (storage and
identity), parallel to §19.2 (Place storage) and §20.2
(Operand storage).

---

### D-7-3 -- Rvalue-site invariants [LOCKED]

**Question.** What invariants does the bridge enforce at
rvalue construction? Specifically: which dropped variants
and discharged-but-representable sub-discriminator values
are rejected, and what positive well-formedness invariants
hold?

**Key options.** Whether to enforce per-variant rejection
rules at bridge time (Option A) or defer to stage 11 walk
(Option B). Since the dropped variants do not exist in the
RegseqModel `Rvalue` enum, "rejection" is structural by
type (the bridge's `translate_rvalue` returns
`BridgeError::UnsupportedConstruct` rather than
constructing a non-existent variant). Sub-discriminator
rejection (Fake / ArrayToPointer / MutToConstPointer /
Coroutine kinds) is enforced at bridge translation time.

**Decision.** Bridge-time rejection for all dropped
constructs (Part A). Positive invariants specified at
stage 7 (Part B); walk-enforcement implementation deferred
to stage 11's body-level traversal (Part C; same split as
D-5-2 places). F4 / F4-2 cross-build identity OUT of
scope (Part D).

### Part A: Rejection rules (cleanup-pass-discharged or unreachable constructs)

| Construct | Rejection ground | Source |
| --------- | ---------------- | ------ |
| `mir::Rvalue::CopyForDeref(_)` | Eliminated by required `EraseDerefTemps` runtime-lowering pass (listed at `rustc_mir_transform/src/lib.rs:658`; `is_required(): true` at `rustc_mir_transform/src/erase_deref_temps.rs:38`); codegen `bug!()`s (`rustc_codegen_ssa/src/mir/rvalue.rs:712`) | D-7-1.1 |
| `mir::Rvalue::ShallowInitBox(_, _)` | Eliminated by required `ElaborateBoxDerefs` runtime-lowering pass (listed at `rustc_mir_transform/src/lib.rs:659`; `is_required(): true` at `rustc_mir_transform/src/elaborate_box_derefs.rs:224`); codegen `bug!()`s (`rustc_codegen_ssa/src/mir/rvalue.rs:713`) | D-7-1.1 |
| `mir::Rvalue::ThreadLocalRef(_)` | DSL-surface claim: `#[thread_local]` cannot produce a working driver on Linux/Zephyr targets; "No defensive fallbacks" rule | D-7-1.1 |
| `mir::Rvalue::WrapUnsafeBinder(_, _)` | §15.1 Type-level rejection table (`spec.md:4583`) rejects `UnsafeBinder` at type translation; rvalue-level rejection cascades by composition | D-7-1.1 |
| `mir::Rvalue::Ref(_, BorrowKind::Fake(_), _)` | NOPed by required `CleanupPostBorrowck` (NOP code at `rustc_mir_transform/src/cleanup_post_borrowck.rs:33-43`; `is_required(): true` at `rustc_mir_transform/src/cleanup_post_borrowck.rs:88`) | D-7-1.4 |
| `mir::Rvalue::Cast(CastKind::PointerCoercion(MutToConstPointer \| ArrayToPointer, _), _, _)` | Rewritten to `PtrToPtr` by required `CleanupPostBorrowck` (rewrite code at `rustc_mir_transform/src/cleanup_post_borrowck.rs:44-60`; `is_required(): true` at `rustc_mir_transform/src/cleanup_post_borrowck.rs:88`) | D-7-1.6 |
| `mir::Rvalue::Aggregate(box AggregateKind::Coroutine(_, _) \| CoroutineClosure(_, _), _)` | Type-level rejection in §15.1 (`spec.md:4581`) plus `StateTransform` runtime-lowering discharge | D-7-1.8 |

Diagnostic per row: `unsupported_construct!` style, naming
the discharging mechanism so the driver author sees a
useful error message (e.g., "thread-local statics not
supported in driver code (target form-factor lacks TLS
infrastructure)" for ThreadLocalRef).

### Part B: Positive invariants (well-formedness rules; walk-enforcement deferred to stage 11)

- **I-Rvalue-Operand-Wellformed.** Every embedded
  `Operand` slot satisfies stage-6 D-6-3 invariants
  (inherited via §20.x I-Operand-Place-Validity at
  `spec.md:5833`). Every embedded `Place` slot satisfies
  stage-5 D-5-2 invariants (inherited via §19.7 Place
  validation at `spec.md:5527`).
- **I-Rvalue-TyId-Interned.** Every `TyId` slot (in
  `Cast`, `AggregateKind::Array` / `Tuple` / `Struct` /
  `Enum` / `Union` / `Closure` / `RawPtr`) resolves through
  stage-3's type interner (the same primitive that
  `find_nominal(NominalKind::Struct, item, args)` uses at
  `spec.md:3306, 4968, 5188`).
- **I-Rvalue-Aggregate-Operand-Count.** Number of
  operands matches the destination type's structural shape:
  - `Array(TyId)`: count == `n` for `Ty::Array(_, n)`.
  - `Tuple(TyId)`: count == `fields.len()` for
    `Ty::Tuple { fields, layout }`.
  - `Struct(TyId)`: count == struct field count.
  - `Enum(TyId, VariantIdx)`: count == named-variant field
    count.
  - `Union(TyId, FieldIdx)`: count == 1 (unions construct
    exactly one active field).
  - `Closure(TyId)`: count == captures-struct field count
    (reached via parallel `Ty::Struct` view per §13.1).
  - `RawPtr(TyId, Mutability)`: count == 2 (`(data_ptr,
    metadata)` -- per `rustc_middle/src/mir/syntax.rs:1561-1566`,
    even thin pointers carry both operands; the unit
    metadata for thin pointers is still required).
- **I-Rvalue-Repeat-Count-Wellformed.** `Repeat(Operand,
  count)` count is `u64`; trivially non-negative
  (unsigned).
- **I-Rvalue-VariantIdx-Wellformed.** `Enum(TyId,
  VariantIdx)`'s VariantIdx is in range `[0, variant_count)`
  of the enum (parallel to D-5-1's PlaceElem::Downcast(VariantIdx)
  invariant).
- **I-Rvalue-FieldIdx-Wellformed.** `Union(TyId,
  FieldIdx)`'s FieldIdx is in range `[0, field_count)` of
  the union.

### Part C: Walk-implementation placement

Rules at stage 7 (this section); walk implementation at
stage 11 (with the body-walker that visits
`Statement::Assign(Place, Rvalue)`). Same split as D-5-2
for Place validation (rules at stage 5; walk at stage 11).

### Part D: F4 / F4-2 cross-build identity -- OUT of scope

Rvalue is body-local content; does not participate in
cross-build identity. The F4-2 instance ordering
canonicalises `(InstanceKey)`, not `(BodyContents)`. Same
scope reduction as D-5-2 and D-6-3.

**Reasoning.** Each rejection rule has an explicit
discharge path documented in the prior-art file referenced;
the table is a consolidation, not a new claim. Positive
invariants follow the principle "enforce structural
well-formedness; trust upstream-discharged semantic
invariants" -- rustc has already discharged type-coherence
checks (drop-elaboration, NLL, borrowck) before
`optimized_mir`; the bridge enforces what is structurally
visible in the IR (operand count, index bounds, TyId
interner-membership) and inherits what stages 5/6 already
enforce. Walk-implementation placement at stage 11 mirrors
stage-5's split: rules can be specified standalone, but the
walker that exercises them needs `Body` machinery (stage
11). F4-2 OUT-of-scope mirrors stages 5/6 (body-local
content; no cross-build identity consumer).

**Methodology rules invoked.** "No defensive fallbacks"
(do not re-check what rustc has discharged); "Compositional
behavior beats special-case flags" (`../README.md` §2:
Rvalue invariants inherit from D-5-2 / D-6-3 transitively
for embedded Place / Operand slots; stage 7 adds only
structural shape invariants). "Cleanup-pass artefacts are
reject-on-encounter, not variants" (`../memory.md` §2
stage-7-specific corollary: the rejection table
implements this principle).

**Prior-art evidence.**

- `prior-art/rustc.md` §11 (codegen rejection sites for
  CopyForDeref / ShallowInitBox), §13 (cleanup-pass
  whitelist context: which discharges are required); §15
  D-7-10 mapping.
- `prior-art/mrustc.md` §11 D-7-3 (mrustc has no analogous
  rejection chain because mrustc lacks the upstream
  borrowck / cleanup-pass infrastructure that produces these
  constructs in the first place; not directly comparable).
- `prior-art/charon.md` §11 (Charon's translation-pass
  rejection matrix: `CopyForDeref` lowered to plain Copy;
  `ShallowInitBox` preserved structurally but never reached
  in optimized-MIR-derived input; `ThreadLocalRef` rejected;
  `WrapUnsafeBinder` rejected; Coroutine variants rejected).
  Validates the rejection-list structure against an
  independent peer.

**Worked example.** Hand-crafted IR carrying a fake borrow:

```text
StatementKind::Assign(_2,
    Rvalue::Ref(<region>, BorrowKind::Fake(FakeBorrowKind::Shallow), <place>))
```

Bridge translation:

```text
translate_rvalue(mir::Rvalue::Ref(_, BorrowKind::Fake(_), _))
  -> emit BridgeError::UnsupportedConstruct {
       construct: "Rvalue::Ref(BorrowKind::Fake(_))",
       discharge: "NOPed by required CleanupPostBorrowck",
       reference: "rustc_mir_transform/src/cleanup_post_borrowck.rs:33-43",
     }
```

The driver author sees a structured diagnostic naming both
the construct and the rustc mechanism that should have
discharged it; the IR builds to a rejected state rather
than carrying a representable but invalid variant.

A well-formed Aggregate construction:

```text
Rvalue::Aggregate(AggregateKind::Struct(<TyId of Foo>),
                  Box::new([Operand::Constant(<1>), Operand::Constant(<2>)]))
```

passes I-Rvalue-Aggregate-Operand-Count if `Foo` has 2
fields; passes I-Rvalue-TyId-Interned if `<TyId of Foo>`
resolves through the type interner; passes
I-Rvalue-Operand-Wellformed by inheritance from D-6-3 for
each operand.

**Cross-refs.** `spec-snapshot.md` §21.5 (rejection rules
+ positive invariants), §21.5.3 (walk-implementation
deferral note); §19.7 (inherited Place validation); §20.5
(inherited I-Operand-Place-Validity).

---

### D-7-K -- `pretty.rs::render_rvalue` deferral [LOCKED]

**Question.** Where does `pretty.rs::render_rvalue` land
-- stage 7 (alongside the Rvalue vocabulary), stage 11
(alongside `render_place` and `render_operand`), or stage
14 (alongside `render_const` and the renderer for the
remaining pieces)?

**Key options.**

1. **Option A -- stage 7 (immediate).** Land
   `render_rvalue` alongside the variant set. Requires
   sub-renderers `render_place` (stage 11 per D-5-K) and
   `render_operand` (stage 11 per D-6-K) and `render_const`
   (stage 14 per D-4-K) to be in place; they are not.
2. **Option B -- stage 11 (latest first-batch
   dependency).** Co-locate with `render_place` (D-5-K) and
   `render_operand` (D-6-K). The `Cast` / `Aggregate` arms
   may late-bind to `render_const` if needed for
   constant-payload sub-rendering, but most arms use
   only `render_place` / `render_operand`.
3. **Option C -- stage 14 (latest dependency overall).**
   Wait for `render_const` (stage 14) before landing.
   Maximally deferred.

**Decision.** Option B (stage 11). Filed as outbound
expectation in `stage-11/expectations.md` "From stage 7"
alongside the existing D-5-K `render_place` and D-6-K
`render_operand` deferrals.

**Reasoning.** Per-variant rendering input dependency:

| Variant | Rendering input | Stage of input |
| ------- | --------------- | -------------- |
| `Use(o)` | `render_operand(o, body)` | 11 |
| `Repeat(o, n)` | `render_operand(o, body)` + literal `u64` | 11 |
| `Ref(prov, p)` | `&` / `&mut` keyword (per `Provenance` discriminator) + `render_place(p, body)` | 11 |
| `RawPtr(mutbl, p)` | `&raw const` / `&raw mut` keyword (per `Mutability` discriminator) + `render_place(p, body)` | 11 |
| `Cast(ck, o, ty)` | `render_operand(o, body)` + CastKind name + `render_ty(ty)` | 11 (operand), 13 (ty) |
| `BinaryOp(op, l, r)` | BinOp name + `render_operand(l, body)` + `render_operand(r, body)` | 11 |
| `UnaryOp(op, o)` | UnOp name + `render_operand(o, body)` | 11 |
| `Discriminant(p)` | `discriminant(` + `render_place(p, body)` + `)` | 11 |
| `Aggregate(kind, ops)` | AggregateKind dispatch + per-element `render_operand` + per-variant field/variant names resolved directly from `kind`'s `TyId` via the §12 type interner (no head-local lookup needed -- every `AggregateKind` variant carries the destination TyId in its payload). What forces deferral to stage 11 is the operand-rendering subtree, not field-name resolution. | 11 |

The dominant input dependency is stage 11 (`render_place`
and `render_operand` both deferred there). The `Cast` arm's
`render_ty` is stage 13 (per D-3 §13 deferred D14 -- Ty
renderer at stage 13); `render_rvalue` late-binds to
`render_ty` once stage 13 fills it. The load-bearing
first-batch dependency is stage 11; the Cast arm becomes
fully functional once stage 13 lands.

`render_rvalue` could land at stage 11 or stage 13. Stage
11 is chosen because:

1. `render_place` and `render_operand` both land at stage
   11 (D-5-K, D-6-K). Co-locating `render_rvalue` keeps the
   rendering surface coherent.
2. The Cast arm's `render_ty` dependency can late-bind
   (stage 11's `render_rvalue` calls into a stage-13-filled
   `render_ty` once stage 13 lands; the call is statically
   valid as soon as both are in place; tests for the Cast
   arm gate on stage 13).
3. Snapshot tests covering `Rvalue`-bearing body content
   (every `Statement::Assign` and most `Terminator`
   payloads) become available at stage 11 (when bodies
   materialise), blocking on `render_rvalue` rather than
   waiting for stage 13.

This mirrors `render_place`'s and `render_operand`'s
dependency analyses (D-5-K, D-6-K): land at the latest of
the *first batch* of dependencies that unblock testing,
with later-binding arms calling later-landing dependencies.

**Methodology rules invoked.** "Renderer style is decided
deliberately" (stage-7-specific principle from
`memory.md` §2; deferral target chosen by dependency-chain
analysis, not by default).

**Prior-art evidence.** D-5-K and D-6-K precedents:
`render_place` and `render_operand` deferred to stage 11
with the same dependency-on-Body-materialisation argument.
`prior-art/charon.md` -- Charon's pretty-printer renders
rvalues with keyword-prefix style for Ref/RawPtr (`& <p>`,
`&raw const <p>`) and operator names for BinaryOp/UnaryOp;
one valid prior-art point for stage 11's eventual
`render_rvalue`.

**Worked example.** Rendering `Rvalue::BinaryOp(BinOp::Add,
Operand::Copy(_1), Operand::Constant(<5>))` requires three
pieces:

- BinOp name: `+` (literal mapping; self-contained).
- `render_operand(Operand::Copy(_1), body)`: returns
  `copy _1` (when the place has no projections; stage 11
  via `render_place`).
- `render_operand(Operand::Constant(<5>), body)`: returns
  `const 5_i32` (when the const resolves to `Const::Int(I32,
  5)`; stage 11's `render_operand` calls into stage-14
  `render_const`).

Full output: `copy _1 + const 5_i32`. At stage 11 where
`render_rvalue` lands, the Use / Repeat / Ref / RawPtr /
BinaryOp / UnaryOp / Discriminant / Aggregate arms work
end-to-end via `render_place` / `render_operand` plus the
§12 type-interner lookups for AggregateKind field/variant
names (each variant carries the destination TyId in its
payload). The Cast arm late-binds to stage-13 `render_ty`
for full destination-type rendering and becomes fully
functional once stage 13 lands; tests for the Cast arm
gate on stage 13.

**Cross-refs.** `spec-snapshot.md` §21.8 (rendering
deferral), §18.2 (per-method ownership table -- updated
with `render_rvalue` row). Outbound:
`stage-11/expectations.md` "From stage 7" (filed at stage-7
close per the merger contract).


---

## Stage 8 (MIR statements)


### D-8-1 -- `Statement` core vocabulary [LOCKED]

**Question.** What is the closed `Statement` value shape
that the bridge fills from rustc's `mir::StatementKind<'tcx>`
and that downstream consumers (stage 10 basic blocks --
`BasicBlock.statements: Vec<Statement>`; stage 11 bodies;
stage 14 backends) read as the per-block sub-terminator
instruction sequence?

**Key options.**

1. **Option A -- mirror rustc verbatim (13 variants).**
   Carry every `mir::StatementKind<'tcx>` arm: `Assign`,
   `FakeRead`, `SetDiscriminant`, `StorageLive`,
   `StorageDead`, `Retag`, `PlaceMention`,
   `AscribeUserType`, `Coverage`, `Intrinsic`,
   `ConstEvalCounter`, `Nop`, `BackwardIncompatibleDropHint`.
2. **Option B -- mirror rustc_public (12 variants).**
   Drop only `BackwardIncompatibleDropHint` (which is
   semantically a Nop).
3. **Option C -- the closed minimum-reachable surface (3
   variants).** Drop every variant that is cleanup-pass-
   discharged or out-of-pipeline under our compilation
   contract (`mir_opt_level=0` + cleanup-pass whitelist
   per `crates/regseq-driver/src/callbacks.rs:25-66`).
   Surviving variants: `Assign`, `SetDiscriminant`,
   `Intrinsic`.
4. **Option D -- mrustc-tighter (2 variants).** Drop
   `SetDiscriminant` (fold into `Rvalue::EnumVariant`)
   and `Intrinsic` (lift to `Terminator::Call` per
   mrustc's `CallTarget::Intrinsic{name, params}`).

**Decision.** Option C. Closed 3-variant
`StatementKind`:

```rust
pub enum StatementKind {
    Assign(Place, Rvalue),
    SetDiscriminant(Place, VariantIdx),
    Intrinsic(NonDivergingIntrinsic),
}
```

The 10 dropped distinct rustc variants
(`FakeRead`, `AscribeUserType`,
`BackwardIncompatibleDropHint`, `Coverage`,
`Assign(_, Rvalue::Ref(_, BorrowKind::Fake, _))`,
`PlaceMention`, `Nop`, `StorageLive`, `StorageDead`,
`Retag`, `ConstEvalCounter`) are reject-on-encounter
at the bridge with diagnostics naming the discharging
mechanism. Sub-decisions D-8-1.1 -- D-8-1.4 specialize
this composite into per-aspect commitments.

**Reasoning.** Option A admits unreachable
constructs into the IR, defeating the bridge's job of
canonicalising rustc's wide construction surface into
the post-required-cleanup-pass-discharge surface.
Option B keeps cleanup-pass-discharged constructs that
cannot reach our optimized_mir input -- all 9 variants
beyond Assign / SetDiscriminant / Intrinsic are
strip-target by required passes (CleanupPostBorrowck
NOPs FakeRead, AscribeUserType,
BackwardIncompatibleDropHint, Coverage markers,
Fake-borrow Assigns; RemovePlaceMention strips
PlaceMention and Nop; RemoveStorageMarkers
[force-on via `mir_enable_passes` whitelist] strips
StorageLive / StorageDead / Nop; AddRetag /
CtfeLimit / InstrumentCoverage are out-of-pipeline).
Option D's mrustc-tighter folding (SetDiscriminant
into Rvalue::EnumVariant; Intrinsic to Call) imposes
a structural transformation rustc itself does not
perform: real driver state-machine code emits in-place
tag-writes (`*self = State::Configured`) that the
optimized_mir actually contains as `SetDiscriminant`
statements; folding them into atomic-construction
Rvalues misrepresents what we receive. Lifting
non-diverging intrinsics to Calls breaks rustc's
explicit choice ("avoids adding a new block and
terminator for simple intrinsics" per
`rustc_middle/src/mir/syntax.rs:448-451`) and pollutes
Q7's intrinsic-recognition namespace
(`#[rseq::intrinsic("NAME")]` recognised at Calls;
mixing rustc's NonDivergingIntrinsic into the same
Call space muddles the recognizer).

Option C is the largest reachable closed surface that
compose-derives from the cleanup-pass + build-flag +
pipeline-driver evidence. Each dropped variant has a
named, evidence-based discharge mechanism (D-8-4 Part
A enumerates them with citations).

**Methodology rules invoked.** "Closed `Statement`
vocabulary" (`../memory.md` §2 stage-8-specific
principle: smallest variant set covering the
reachable content; specializes the README §2 "core is
opaque" rule's "items outside the recognized
vocabulary are rejected" pattern at the Statement
surface). "No defensive fallbacks"
(`../README.md` §2 methodology rule: do not handle
the unhandleable). "Cleanup-pass artefacts are
reject-on-encounter, not variants"
(`../memory.md` §2 stage-8-specific corollary).
"Out-of-pipeline constructs are reject-on-encounter,
not variants" (`../memory.md` §2 stage-8-specific
corollary covering build-flag-gated variants).
"Compositional behavior beats special-case flags"
(`../README.md` §2 methodology rule: the cleanup-
discharge cascade composes with stage-3 type
rejection plus pipeline-driver pass whitelist; no
parallel side-channel needed).

**Prior-art evidence.**

- `prior-art/rustc.md` §1-3 (full 13-variant surface
  + `Statement` wrapper struct + cleanup-pass
  discharge mechanics: CleanupPostBorrowck NOP rules,
  RemovePlaceMention / RemoveStorageMarkers retain
  filters, `is_required` vs `is_enabled` taxonomy,
  pass-manager override semantics).
- `prior-art/rustc_public.md` §1-3 (12-variant
  reference; documents the BackwardIncompatibleDropHint
  drop precedent and the bridge's per-variant
  treatment; closest existing owned-IR analog).
- `prior-art/mrustc.md` §1 (8-variant peer-narrowest
  surface as Option D evidence: SetDiscriminant
  folded; intrinsics lifted to Call).
- `prior-art/charon.md` §1-3 (9-variant ULLBC: Assume
  fans out to Assert; CopyNonOverlapping is a
  dedicated boxed Statement variant; DOES preserve
  StorageLive / StorageDead [Charon's input is pre-
  cleanup MIR; ours is post]; Statement-level
  rejection is absent in Charon -- absorb-or-drop
  pattern).

**Worked example.** A driver function body with one
of each retained variant and one of the discharged
variants:

```rust
enum State { Idle, Configured }
fn advance(s: &mut State, p: *mut u32, q: *const u32) {
    let _scratch = 42_u32;     // generates a temporary local
    *s = State::Configured;
    unsafe {
        core::intrinsics::assume(!p.is_null());
        core::ptr::copy_nonoverlapping(q, p, 1);
    }
}
```

After cleanup-pass discharge, the optimized_mir body
contains:

- An `Assign(Place, Rvalue)` for `_scratch = 42`.
- A `SetDiscriminant((*_1), VariantIdx(1))` for the
  `*s = State::Configured` write.
- An `Intrinsic(NonDivergingIntrinsic::Assume(...))`
  for the assume call.
- An `Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(...))`
  for the copy_nonoverlapping call.

None of `FakeRead` / `AscribeUserType` /
`StorageLive` / `StorageDead` / `Nop` /
`PlaceMention` / `Retag` / `ConstEvalCounter` /
`Coverage` / `BackwardIncompatibleDropHint` appears.
The 3-variant surface admits this body without any
defensive arm. By contrast, hand-crafted MIR with
`#[thread_local]` on a static would inject a non-
applicable construct at the rvalue level (caught by
§21 D-7); with `-Z mir-emit-retag` would inject
`Retag` (caught by §22.5.1 row 10).

**Cross-refs.** `spec-snapshot.md` §22.1 (variant
set), §22.5 (rejection rules + invariants); D-8-1.1
through D-8-1.4 (per-variant payload sub-decisions);
D-8-3 (wrapper struct); D-8-4 (rejection table +
positive invariants).

---

### D-8-1.1 -- Statement variant cardinality [LOCKED]

**Question.** Should the RegseqModel `StatementKind`
enum carry all 13 rustc variants, 12 (drop only
`BackwardIncompatibleDropHint`), 3 (drop everything
cleanup-discharged + out-of-pipeline), or 2 (drop also
`SetDiscriminant` and `Intrinsic` per mrustc)?

**Key options.** Subsumed by parent D-8-1's options
(Option A = 13 verbatim, Option B = 12 + reject 1,
Option C = 3 + reject 10, Option D = 2 + reject 11).
This sub-decision pins the variant *cardinality*;
D-8-4 pins the rejection rules for the dropped
variants; D-8-1.2 -- D-8-1.4 pin the per-variant
payload shapes for the kept variants.

**Decision.** 3 variants:

```rust
pub enum StatementKind {
    Assign(Place, Rvalue),
    SetDiscriminant(Place, VariantIdx),
    Intrinsic(NonDivergingIntrinsic),
}
```

The 10 dropped distinct rustc variants are reject-on-
encounter; details in D-8-4 (rejection table).

**Reasoning.** Each drop has its own evidence basis,
with the discharge mechanism cited per row:

| Variant | Evidence basis |
| ------- | -------------- |
| `FakeRead(_, _)` | NOPed by required `CleanupPostBorrowck` (`rustc_mir_transform/src/cleanup_post_borrowck.rs:40`; `is_required(): true` at line 88); resulting Nop swept by required `RemovePlaceMention` (`remove_place_mention.rs:18-20`) and whitelisted `RemoveStorageMarkers` (`remove_storage_markers.rs:18-20`). Documented "Disallowed after drop elaboration" (`rustc_middle/src/mir/syntax.rs:357`). |
| `AscribeUserType(_, _)` | NOPed at `cleanup_post_borrowck.rs:33`; user-type-ascriptions cleared at `cleanup_post_borrowck.rs:81-85`. "Disallowed after drop elaboration" (`rustc_middle/src/mir/syntax.rs:426`). |
| `BackwardIncompatibleDropHint { .. }` | NOPed at `cleanup_post_borrowck.rs:41`. Lint-only marker semantically equivalent to Nop (`rustc_middle/src/mir/syntax.rs:462-463`); rustc_public drops the variant (`rustc_public/src/mir/body.rs:476-490`). |
| `Coverage(BlockMarker | SpanMarker)` | NOPed at `cleanup_post_borrowck.rs:35-39`. Mid-pipeline coverage markers; not needed after `InstrumentCoverage`. |
| `Assign(_, Rvalue::Ref(_, BorrowKind::Fake, _))` | Whole Assign NOPed at `cleanup_post_borrowck.rs:34`. Compositional with stage-7 rvalue rejection of `BorrowKind::Fake` (§21.5.1 row 5). |
| `PlaceMention(_)` | Removed by required `RemovePlaceMention` (`remove_place_mention.rs:18`; `is_required: true` at line 24; `is_enabled` returns `!sess.opts.unstable_opts.mir_preserve_ub` which is off by default). Pure `let _ = PLACE;` desugar artefact. |
| `Nop` | Swept by both `RemovePlaceMention` (`remove_place_mention.rs:18`) and `RemoveStorageMarkers` (`remove_storage_markers.rs:20`). Only transient post-`make_nop`; not in optimized_mir. |
| `StorageLive(_)` | Removed by `RemoveStorageMarkers` (`remove_storage_markers.rs:18`). The pass's `is_enabled = mir_opt_level > 0 && !sess.emit_lifetime_markers()` (`remove_storage_markers.rs:10-12`) would gate it off at our default `mir_opt_level=0`, BUT the bridge driver flips it on via `mir_enable_passes` (`crates/regseq-driver/src/callbacks.rs:27` registers `("RemoveStorageMarkers", true)`). The pass-manager override at `rustc_mir_transform/src/pass_manager.rs:227-238` consults `mir_enable_passes` ahead of `is_enabled`, so the gate is bypassed. **The OPEN QUESTION raised in `memory.md` §2 is RESOLVED by this trace: the pass DOES run.** |
| `StorageDead(_)` | Same as `StorageLive(_)`. |
| `Retag(_, _)` | Only emitted by `AddRetag` (`add_retag.rs:85, 118, 177`); `is_enabled` requires `-Z mir-emit-retag`. The bridge driver does not enable that flag. Out-of-pipeline. |
| `ConstEvalCounter` | Only injected by `CtfeLimit` (`ctfe_limit.rs:36`) for const-eval-bound bodies (`mir_for_ctfe`). Driver bodies are run-time bodies (`optimized_mir`); CtfeLimit does not run on them. |
| `Coverage(non-marker variants)` | Only emitted by `InstrumentCoverage` under `-C instrument-coverage`. Driver builds do not enable coverage instrumentation. |

The 3-variant set is the largest reachable closed
surface that compose-derives from the cleanup-pass +
required-pass-discharge + build-flag-gating evidence
under our compilation contract. Each dropped variant
has at least one strong-evidence ground.

The mrustc-tighter alternative (2 variants -- drop
SetDiscriminant + Intrinsic) is rejected because:
- mrustc folds SetDiscriminant into
  `RValue::EnumVariant` because mrustc only
  constructs whole enums, never tag-writes in place;
  this does not match what optimized_mir for real
  driver code contains (state-machine transitions
  emit `SetDiscriminant` for in-place tag updates,
  not full reconstructions).
- mrustc lifts intrinsics to `Terminator::Call`
  because mrustc has no Statement-level intrinsic
  variant; rustc explicitly chose Statement for non-
  diverging cases ("avoids adding a new block and
  terminator for simple intrinsics" per
  `rustc_middle/src/mir/syntax.rs:448-451`); lifting
  to Call inflates basic-block count for code that
  is structurally straight-line, AND collides with
  Q7's regseq DSL intrinsic recognition at Call
  sites (the recognizer would see rustc's Assume /
  CopyNonOverlapping mixed with regseq DSL
  semantic intrinsics).

**Methodology rules invoked.** "Closed `Statement`
vocabulary"; "No defensive fallbacks"; "Cleanup-pass
artefacts are reject-on-encounter, not variants";
"Out-of-pipeline constructs are reject-on-encounter,
not variants"; "Compositional behavior beats
special-case flags". (Same set as D-8-1's parent
audit; specialised here at the cardinality decision.)

**Prior-art evidence.**

- `prior-art/rustc.md` §3 (per-variant cleanup-
  discharge trace), §10 (the rejection-point table
  used as a template for D-8-4 Part A), §13
  (`is_required` / `is_enabled` semantics in the
  pass manager).
- `prior-art/rustc_public.md` §1 (12-variant baseline:
  the BackwardIncompatibleDropHint drop precedent),
  §6 (preservation of FakeRead / AscribeUserType
  motivated by pre-runtime consumers' use cases --
  RegseqModel inputs are post-runtime).
- `prior-art/mrustc.md` §1 (8-variant peer-narrowest
  as Option D evidence; the SetDiscriminant fold and
  Intrinsic-to-Call lift discussed under D-8-1's
  rejection of Option D).
- `prior-art/charon.md` §1, §2 (9-variant ULLBC: the
  Assume-to-Assert lift and CopyNonOverlapping-as-
  dedicated-Statement-variant fanout discussed under
  D-8-1.4's rejection of fanout).

**Worked example.** Same as D-8-1 parent's worked
example (the `advance` function with `*s =
State::Configured` plus assume + copy_nonoverlapping
unsafe block).

**Cross-refs.** `spec-snapshot.md` §22.1 (variant
definition), §22.1.3 (variants dropped vs rustc),
§22.5.1 (rejection rules); D-8-1 (parent); D-8-4
(rejection table + invariants).

---

### D-8-1.2 -- `Assign(Place, Rvalue)` payload [LOCKED]

**Question.** What does the `Assign` variant carry?

**Key options.**

1. **Option A -- direct flat tuple `(Place, Rvalue)`.**
   Inline place and rvalue; no Box.
2. **Option B -- mirror rustc `Box<(Place, Rvalue)>`.**
   Box the payload tuple to maintain a 16-byte
   `StatementKind` size budget.
3. **Option C -- struct with named fields
   `Assign { place, rvalue }`.** Stylistic alternative
   to A.

**Decision.** Option A. Direct flat tuple:

```rust
Assign(Place, Rvalue),
```

Plus the type-equality invariant `place_ty(p) ==
rvalue_ty(rv)` modulo lifetime erasure as
I-Statement-Assign-Type-Equality (D-8-4 Part B-1;
walk-enforcement deferred to stage 11). Plus the
compositional rule that drop-suppression is the
explicit Drop terminator's job at stage 9 -- no
drop-suppression flag on the variant.

**Reasoning.** Option B's Box exists in rustc to
maintain the 16-byte `StatementKind` size assert at
`rustc_middle/src/mir/syntax.rs:1753`. RegseqModel
has no comparable size budget at the statement
surface; rustc's Box is purely a memory-layout
optimization rustc needs and we don't, exactly
mirroring stage-7 D-7-1.7's flat-`BinaryOp` decision
(rejected rustc's `Box<(Operand, Operand)>` for the
same reason). rustc_public's
`Assign(Place, Rvalue)` (`rustc_public/src/mir/body.rs:478`)
is the direct precedent; mrustc's flat
`Assign { dst, src }` and Charon's
`Assign(Place, Rvalue)` confirm the peer pattern.

Option C (struct-with-named-fields) is rejected for
consistency with stage-7's tuple-for-2-3-field
precedent (D-7-1.4 `Ref(Provenance, Place)`, D-7-1.5
`RawPtr(Mutability, Place)`). The two fields
(Place / Rvalue) are not order-confusable -- they
have distinct types -- so named fields add nothing
beyond stylistic verbosity. (Compare D-8-1.4's
`CopyNonOverlapping{src, dst, count}` where named
fields ARE correctness-essential because all three
operands have type `Operand` and order confusion
would silently produce wrong-direction copy bugs.)

The type-equality invariant is rustc's well-formedness
rule at `rustc_middle/src/mir/syntax.rs:324`
("Assignments in which the types of the place and
rvalue differ are not well-formed"); pinned by
rustc's validator
(`rustc_mir_transform/src/validate.rs:1467-1582`).
RegseqModel does not duplicate the type at the
variant level (no `assign_ty: TyId` slot); the type
is recoverable from `place_ty(lhs)` (§19's
type-coherence walk) and `rvalue_ty(rhs)` (§21's
per-variant type-recovery rules). The invariant
lives in §22.5.2 / D-8-4 Part B-1 with walk-
enforcement deferred to stage 11.

The drop-suppression non-flag is settled by rustc
semantics: `Assign` does NOT call drop on the
previous LHS value
(`rustc_middle/src/mir/syntax.rs:312-317`); drop is
the explicit `Drop` terminator's job at stage 9. No
parallel `is_drop_assign` flag needed -- compositional
with stage-9 drop terminator placement.

**Methodology rules invoked.** "Compositional
behavior beats special-case flags" (no
`is_drop_assign` flag; drop falls out of stage-9
terminator placement). "C semantics with information
preservation" (preserve rustc's variant meaning;
strip the rustc-specific Box-budget artefact).

**Prior-art evidence.**

- `prior-art/rustc.md` §4 (`Assign` payload + the
  type-equality invariant + `Box` size-budget
  rationale).
- `prior-art/rustc_public.md` §3 (flat
  `Assign(Place, Rvalue)`; no Box; same flat shape).
- `prior-art/mrustc.md` §1 (flat
  `Assign { dst, src }`; flat shape; struct-with-
  named-fields stylistic choice that doesn't apply
  to RegseqModel's tuple precedent).
- `prior-art/charon.md` §2 (flat `Assign(Place,
  Rvalue)`; flat shape).

**Worked example.** `let y = x + 1;` produces an
`Assign(_2, BinaryOp(Add, Move(_1), Constant(c1)))`
where `place_ty(_2)` resolves to `Ty::Uint(U32)` (from
the local-decl) and `rvalue_ty(BinaryOp(Add, Move(_1),
Constant(c1)))` also resolves to `Ty::Uint(U32)` (per
stage-7 BinOp type-recovery rules). I-Statement-
Assign-Type-Equality holds.

**Cross-refs.** `spec-snapshot.md` §22.2.1 (payload),
§22.5.2 (I-Statement-Assign-Type-Equality); D-8-1
(parent); D-8-4 (Part B invariants).

---

### D-8-1.3 -- `SetDiscriminant(Place, VariantIdx)` payload [LOCKED]

**Question.** What does the `SetDiscriminant` variant
carry?

**Key options.**

1. **Option A -- flat tuple `(Place, VariantIdx)`.**
   Inline; no Box; no struct fields.
2. **Option B -- mirror rustc `{ place: Box<Place>,
   variant_index: VariantIdx }`.** Box the place;
   struct with named fields.
3. **Option C -- struct with flat place, named
   fields `{ place: Place, variant_index: VariantIdx
   }`.** rustc_public's choice.
4. **Option D -- mrustc fold into `Rvalue::EnumVariant`.**
   Drop the variant entirely; require atomic full-
   enum construction via Rvalue.

**Decision.** Option A. Flat tuple:

```rust
SetDiscriminant(Place, VariantIdx),
```

Plus the variant_index in-range invariant
I-Statement-SetDiscriminant-VariantIdx (D-8-4 Part
B-2; reads `Ty::Enum.layout`'s `LayoutKind::Enum.shape`
cases per §11.1). Plus the target-type-must-be-Ty::Enum
invariant I-Statement-SetDiscriminant-Target (D-8-4
Part B-3), compositional with the §10 `Ty` taxonomy
(direct `Ty::Enum { item, args, layout }` match; not
via a `Ty::Adt` umbrella).

**Reasoning.** Option B's Box exists for rustc's 16-
byte size assert; not inherited (same reasoning as
D-8-1.2). Option C (struct-with-flat-place) is a
stylistic question -- the project leans tuple per
stage-7 precedent (D-7-1.4 `Ref(Provenance, Place)`,
D-7-1.5 `RawPtr(Mutability, Place)`, D-7-1.8
`Aggregate(AggregateKind, Box<[Operand]>)`); 2-field
payloads with distinct types use tuple. Option D is
rejected at D-8-1's parent level (mrustc's
atomic-construction model misrepresents what
optimized_mir actually contains for state-machine
driver code: in-place tag-writes are real and
SetDiscriminant captures them).

The target-type rule:
- Rustc's validator at
  `rustc_mir_transform/src/validate.rs:1551-1567`
  allows `place_ty(p).kind() ∈ { ty::Adt(..),
  ty::Coroutine(..), ty::Alias(ty::Opaque, ..) }`.
- `ty::Coroutine(..)` is rejected at stage 3 §15.1;
  the variant cannot reach optimized_mir with a
  coroutine-typed target.
- `ty::Alias(ty::Opaque, ..)` is normalized away by
  the required `PostAnalysisNormalize` pass
  (`rustc_mir_transform/src/lib.rs:642`); opaque
  types resolve to their underlying type before
  optimized_mir.
- `ty::Adt(..)` corresponds to RegseqModel's three
  direct Adt-shaped Ty variants `Ty::Struct{..}`,
  `Ty::Enum{..}`, `Ty::Union{..}` per §10. Only
  `Ty::Enum` has a discriminant to write;
  `Ty::Struct` and `Ty::Union` have no tag location.

Bridge rule: `place_ty(p) = Ty::Enum { item, args,
layout }` per §10. Other shapes (`Ty::Struct{..}`,
`Ty::Union{..}`, `Ty::Tuple{..}`, primitive types,
references, raw pointers, function types, etc.)
reject at the bridge with a diagnostic citing the
§10 `Ty` variant taxonomy. Compositional with
stage-3 `Ty` taxonomy; not a parallel
`target_kind: Enum` flag.

The variant carries no redundant Item / Layout slot;
the target type is fully recoverable from
`place_ty(p)` via stage-3 / stage-5 / §10 vocabulary
at render / codegen time (matches stage-7
Aggregate's TyId-lookup-via-§12-type-interner
pattern). The variant_index in-range check uses
`Ty::Enum`'s `LayoutKind::Enum.shape` cases per
§11.1: `EnumShape::Empty -> 0`,
`EnumShape::Single { .. } -> 1`,
`EnumShape::Multiple { variants, .. } ->
variants.len()`.

**Methodology rules invoked.** "Compositional
behavior beats special-case flags" (no `target_kind`
flag; the target-type rule falls out of stage-3
NominalKind taxonomy).

**Prior-art evidence.**

- `prior-art/rustc.md` §5 (`SetDiscriminant` payload
  + the target-type validator rule at
  `validate.rs:1467-1582`).
- `prior-art/rustc_public.md` §3 (struct-with-flat-
  place precedent for Option C).
- `prior-art/charon.md` §2 (flat tuple
  `SetDiscriminant(Place, VariantId)` precedent for
  Option A).
- `prior-art/mrustc.md` §1 (the fold-into-
  EnumVariant Option D evidence, rejected).

**Worked example.** `*s = State::Configured;` where
`State` has variants `Idle, Configured, Running`
produces `SetDiscriminant((*_1), VariantIdx(1))`.
- `place_ty((*_1)) = Ty::Enum { item: <ItemId of
  State>, args: [], layout: <Layout of State> }`
  (B-3 satisfied via the direct `Ty::Enum` match).
- `VariantIdx(1) < 3`: the bound check reads
  `Ty::Enum`'s `LayoutKind::Enum.shape ->
  EnumShape::Multiple { variants, .. } ->
  variants.len() == 3`; `1 < 3` (B-2 satisfied).

By contrast, hand-crafted MIR with a struct target
`SetDiscriminant(_2 /* type Foo struct */, _)` would
have `place_ty(_2) = Ty::Struct{..}`, which fails
B-3 at the bridge with a diagnostic citing "target of
SetDiscriminant must be Ty::Enum; got Ty::Struct."

**Cross-refs.** `spec-snapshot.md` §22.2.2 (payload),
§22.5.2 (I-Statement-SetDiscriminant-VariantIdx,
I-Statement-SetDiscriminant-Target); D-8-1 (parent);
D-8-4 (Part B invariants).

---

### D-8-1.4 -- `Intrinsic(NonDivergingIntrinsic)` payload [LOCKED]

**Question.** What does the `Intrinsic` variant carry,
and how is the `NonDivergingIntrinsic` sub-enum shaped?

**Key options.**

1. **Option A -- nested flat
   `Intrinsic(NonDivergingIntrinsic)` with sub-enum
   `Assume(Operand) | CopyNonOverlapping(CopyNonOverlapping)`
   and named-fields struct
   `CopyNonOverlapping { src, dst, count }`.** All
   flat (no Box at any level).
2. **Option B -- mirror rustc
   `Intrinsic(Box<NonDivergingIntrinsic>)`** with same
   nested sub-enum. Box at the Intrinsic arm.
3. **Option C -- fan out into top-level Statement
   variants** `Assume(Operand)` /
   `CopyNonOverlapping(CopyNonOverlapping)`. Charon-
   ish (Charon actually transforms Assume to Assert
   and gives CopyNonOverlapping its own boxed
   Statement variant).
4. **Option D -- lift to `Terminator::Call` per
   mrustc.** No Statement-level intrinsic variant.

**Decision.** Option A. Nested flat:

```rust
Intrinsic(NonDivergingIntrinsic),

pub enum NonDivergingIntrinsic {
    Assume(Operand),
    CopyNonOverlapping(CopyNonOverlapping),
}

pub struct CopyNonOverlapping {
    pub src: Operand,
    pub dst: Operand,
    pub count: Operand,
}
```

Plus per-arm operand-type invariants:
I-Statement-Intrinsic-Assume-Bool (D-8-4 Part B-4),
I-Statement-Intrinsic-CopyNonOverlapping-Types (D-8-4
Part B-5).

**Reasoning.** Option D is rejected at D-8-1's parent
level (collides with Q7 intrinsic-recognition
namespace at Calls; inflates basic-block count for
straight-line code; rustc explicitly chose Statement
to avoid this per `rustc_middle/src/mir/syntax.rs:448-451`).

Option B's Box exists for rustc's 16-byte size assert;
not inherited (same reasoning as D-8-1.2 / D-8-1.3).

Option C (fanout) is rejected on three grounds:
1. **Semantic grouping**: rustc's
   `NonDivergingIntrinsic` sub-enum has meaning -- both
   arms are rustc-internal hint primitives that don't
   diverge. Fanning out loses the grouping.
2. **Closed-surface size**: fanout would expand the
   Statement closed surface from 3 to 4 variants
   without semantic gain.
3. **Future-proofing**: rustc may add new
   `NonDivergingIntrinsic` arms (this is a rustc
   extension point). With nested, future arms land as
   sub-enum extensions (a small blast radius); with
   fanout, they would land as Statement-enum changes
   (the spec's `StatementKind` would have to grow,
   touching many more sites).

Charon's actual transformation (Assume -> Assert;
CopyNonOverlapping -> dedicated boxed Statement) is
specifically a *transformation of meaning* for
verification artefacts (Assert with
`on_failure: UndefinedBehavior` is a verification-
friendly shape; we don't want UB labels on rustc's
boolean hints). Not appropriate for our pipeline.

`CopyNonOverlapping`'s named-fields struct is **the one
correctness-essential named-fields use** in stage 8.
All three fields are `Operand`, so a tuple shape
`(Operand, Operand, Operand)` would be order-
confusable: `memcpy(dst, src, count)` and
`copy_nonoverlapping(src, dst, count)` swap arguments
1 and 2; a tuple-shape RegseqModel could silently
produce wrong-direction copies at the bridge or
codegen sites if a maintainer reads tuple positions in
the wrong order. Named fields prevent this.

(Compare D-8-1.3's `SetDiscriminant(Place, VariantIdx)`:
two fields, distinct types, not order-confusable;
tuple is correct.)

The per-arm operand-type invariants (B-4, B-5) are
rustc's validator pins; pinned at
`rustc_mir_transform/src/validate.rs:1467-1582`. Walk-
enforcement deferred to stage 11.

**Methodology rules invoked.** "Closed `Statement`
vocabulary" (nested keeps the surface at 3; fanout
would expand to 4). "Compositional behavior beats
special-case flags" (no fanout-flag at the Statement
level). "C semantics with information preservation"
(preserve rustc's NonDivergingIntrinsic semantic
group; strip rustc-specific Box-budget artefacts).

**Prior-art evidence.**

- `prior-art/rustc.md` §6 (`Intrinsic` payload +
  `NonDivergingIntrinsic` definition + the
  `CopyNonOverlapping` struct + the per-variant
  validator pins).
- `prior-art/rustc_public.md` §3 (nested flat
  precedent: `Intrinsic(NonDivergingIntrinsic)` with
  same sub-enum + struct shapes; no Box).
- `prior-art/mrustc.md` §1 (Option D evidence:
  intrinsics flow through `Terminator::Call`; no
  Statement-level intrinsic variant).
- `prior-art/charon.md` §2 (Option C evidence: fan
  out with transformation; rejected for semantic
  grouping + future-proofing reasons).

**Worked example.** `assume(_5)` produces
`Intrinsic(NonDivergingIntrinsic::Assume(Move(_5)))`
where `operand_ty(Move(_5)) == Ty::Bool` (B-4
satisfied).

`copy_nonoverlapping(q, p, n)` produces
`Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(
CopyNonOverlapping {
  src: Move(_2),  // *const u32
  dst: Move(_1),  // *mut u32
  count: Move(_3) // usize
}))` where:
- `operand_ty(Move(_2)) = Ty::Ptr(u32_ty_id, Mutability::Not)`
  (raw pointer; one of the deref-able pointer-like
  types accepted under the `builtin_deref(true)`
  predicate at
  `rustc_mir_transform/src/validate.rs:1521`),
- `operand_ty(Move(_1)) = Ty::Ptr(u32_ty_id, Mutability::Mut)`
  (raw pointer; same predicate at line 1531),
- `pointee(operand_ty(_2)) == pointee(operand_ty(_1))
  == u32_ty_id` (B-5 pointee-equality satisfied;
  validator's analog at
  `validate.rs:1542`),
- `operand_ty(Move(_3)) == Ty::Uint(UintKind::Usize)`
  (B-5 count-type satisfied; validator's analog at
  `validate.rs:1547`).

B-5 satisfied. Stage-14 codegen emits
`memcpy(_1, _2, _3 * sizeof(uint32_t))`. Note the
src/dst swap between the rustc input
`copy_nonoverlapping(q=_2, p=_1, n=_3)` and the C
output `memcpy(dst=_1, src=_2, ...)` -- the named-
fields struct prevents accidental order confusion at
the codegen site.

References (`Ty::Ref(_, _)`) are also admitted by
B-5 in the post-translation surface; rustc's
validator additionally accepts `Box<T>` via
`builtin_deref(true)` per
`rustc_middle/src/mir/syntax.rs:496`, but RegseqModel
narrows the admissible set: stage-3 §15.1 rejects
`Box<T>` at type translation
(`spec.md:4546` -- "alloc construct outside the
no-alloc driver DSL"), so a Box-typed operand cannot
reach a stage-8 `CopyNonOverlapping` site. The B-5
admissible set is therefore the strict subset of
rustc's `builtin_deref(true)` accept-list that
survives the §15.1 cascade: `Ty::Ptr` and `Ty::Ref`.
Driver code overwhelmingly uses raw pointers at the
`copy_nonoverlapping` call site (the intrinsic
signature takes `*const T` / `*mut T`); references
are admitted when rustc emits them through deref
coercions.

**Cross-refs.** `spec-snapshot.md` §22.2.3 (Intrinsic
payload), §22.3 (sub-enum + sub-struct), §22.5.2
(I-Statement-Intrinsic-Assume-Bool,
I-Statement-Intrinsic-CopyNonOverlapping-Types);
D-8-1 (parent); D-8-4 (Part B invariants).

---

### D-8-2 -- Statement storage and interning [LOCKED]

**Question.** Does `Statement` participate in any
top-level interning? Is there a `StatementId` handle?
Does Statement participate in cross-build identity (F4
/ F4-2)?

**Key options.**

1. **Option A -- inline storage, no interning.** Body-
   local content; lives inline in basic-block
   storage; no `StatementId`; F4 / F4-2 cross-build
   identity out of scope.
2. **Option B -- intern Statements with `StatementId`.**
   Hash-cons repeated identical statements; expose a
   handle.

**Decision.** Option A. No Statement interning.

**Reasoning.** Compositional with stage-5 D-5-2,
stage-6 D-6-2, stage-7 D-7-2 -- Place, Operand, and
Rvalue are all body-local with no interning. Statement
is body-local content built from Place + Rvalue +
Operand + VariantIdx + LocalId, all of which are
either body-local or already interned at their own
level (TyId, LocalId via stage-1's interners). There
is no payload at the Statement level that warrants
its own intern-table.

Every prior-art peer stores statements inline:
- rustc: `BasicBlockData.statements: Vec<Statement>`.
- rustc_public:
  `BasicBlock.statements: Vec<Statement>`
  (`rustc_public/src/mir/body.rs:133-137`); `Tables<'tcx, B>`
  interns 8 families (DefIds, AllocIds, Spans, Types,
  Instances, TyConsts, MirConsts, Layouts) -- not
  Statements; `Statement` and `StatementKind`
  notably do NOT derive `Hash`
  (`rustc_public_bridge/src/lib.rs:45-54`).
- mrustc: `BasicBlock::statements: std::vector<Statement>`.
- Charon: `Vec<Statement>` with positional
  `(BlockId, usize)` identity (LLBC adds
  `StatementId` only after CFG reconstruction;
  ULLBC -- the analog of our `optimized_mir`-level IR
  -- has no Statement interning).

Hash-consing repeated statements is a marginal-gain
optimization for runtime memory; the structural-
equality semantic stays simpler without it. Stage-1's
F4-2 cross-build identity discipline canonicalises
`(InstanceKey)`, not body-content.

Storage container choice (`Vec<Statement>` vs
`Box<[Statement]>`) is a stage-10 (`BasicBlock`)
decision, not stage-8's; the `Statement` surface
introduces no top-level Box indirection beyond what
individual variant payloads need internally (none
do, after D-8-1.2 -- D-8-1.4 flat decisions).

**Methodology rules invoked.** "Compositional behavior
beats special-case flags" (no `StatementId`; identity
falls out of structural equality + body-local scope).

**Prior-art evidence.**

- `prior-art/rustc.md` §3.5 (no statement-interner;
  Statement::source_info span identity is per-build).
- `prior-art/rustc_public.md` §4, §11 (Tables families
  exclude Statement; Statement does not derive Hash).
- `prior-art/mrustc.md` §3 (std::vector storage, no
  interner).
- `prior-art/charon.md` §4 (inline Vec<Statement> with
  positional identity; no interner at ULLBC level).

**Worked example.** Two assign statements that happen
to be structurally identical
(`Assign(_2, Use(Constant(c1)))` appearing twice in
two different basic blocks) both live inline at their
respective `BasicBlock.statements` vector positions;
no shared backing handle. Equality at construction
time is structural; F4-2 cross-build canonicalisation
does not see them.

**Cross-refs.** `spec-snapshot.md` §22.4 (identity and
storage); D-5-2 (Place inline); D-6-2 (Operand
inline); D-7-2 (Rvalue inline).

---

### D-8-3 -- `Statement` wrapper struct shape [LOCKED]

**Question.** What metadata does the `Statement`
wrapper struct carry around the `StatementKind`
payload? What span / scope / debuginfo fields are
preserved or dropped?

**Key options.**

1. **Option A -- minimum-span wrapper:
   `{ kind, span: Span }`.** Drop `source_info.scope`
   and `debuginfos`.
2. **Option B -- mirror rustc:
   `{ source_info: SourceInfo, kind, debuginfos:
   StmtDebugInfos }`.** Preserve everything.
3. **Option C -- mrustc-style zero-metadata: just the
   `StatementKind`.** Drop the wrapper; spans go to
   diagnostics only and are discarded.
4. **Option D -- Charon-style fidelity wrapper:
   `{ span, kind, comments_before: Vec<String> }`.**
   Add a comments-preservation field for source-
   fidelity output.

**Decision.** Option A. Minimum-span wrapper:

```rust
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}
```

Field order `kind, span` matches rustc_public
(`rustc_public/src/mir/body.rs:470-474`).

The body-level `var_debug_info: Vec<VarDebugInfo>`
analog (named-local mapping per
`rustc_middle/src/mir/mod.rs:264, 1235-1257`) is a
distinct, stage-11 concern -- not on the Statement
wrapper. Outbound expectation to stage 11: preserve
named-local mapping at the body level (see "Outbound
expectations" below).

**Reasoning.** Option B preserves Rust-level
debuginfo material that has no C-shape meaning:
- `source_info.scope` indexes into `body.source_scopes`
  for DWARF lexical-scope display. RegseqModel emits
  C, not DWARF; the C backend produces C source which
  gets compiled by clang/gcc with their own debug-
  info pipelines. Carrying scope is dead weight.
- `debuginfos: StmtDebugInfos` carries per-statement
  DWARF instrumentation hints (e.g., `AssignRef(Local,
  Place)` for tracking reference assignments to
  ssa-locals; `InvalidAssign(Local)` for poison
  markers; defined at
  `rustc_middle/src/mir/statement.rs:21, 1015-1077`).
  These are translated by codegen to LLVM debug
  intrinsics. RegseqModel does not emit DWARF; the
  field is rejected on stage-1 C-shape preservation
  grounds (carried as the project-wide stance against
  Rust-level debuginfo storage).

Option C (no wrapper) is rejected because spans are
load-bearing for diagnostic emission: when the bridge
rejects a construct, the diagnostic must cite the
source location. Stage-6 D-6-1.3 already deferred
operand-level spans to "the containing Statement's
SourceInfo at stage 8" (`stage-6/spec-snapshot.md:5681`,
:5745); without a Statement-level span, that fallback
breaks. mrustc's no-span discipline is too aggressive
for our diagnostic story.

Option D's `comments_before` is Charon-specific (Aeneas
formal-verification artefacts want source-comment
fidelity); RegseqModel emits C, not formal-verification
artefacts. No analog in any other peer; rejected.

The span-only choice (drop scope) follows from:
- Stage-6 forward ref needs span only;
- Stage-1 §5.3 already pinned `Span` (12 bytes,
  inline, no `SpanId` interning; source-file +
  byte-range -- exactly what diagnostics need).

The `debuginfos` drop follows from:
- No DWARF emission target;
- Stage-1 spec rejected debuginfo storage;
- mrustc, rustc_public both confirm the drop is
  viable for their respective targets.

**Critical clarification: named-local preservation is
NOT lost by this drop.** The user-source variable name
mapping (e.g., "local _3 corresponds to user variable
`temp`") lives in rustc's body-level
`var_debug_info: Vec<VarDebugInfo>`
(`rustc_middle/src/mir/mod.rs:264, 1235-1257`) -- a
separate field on `Body`, NOT on `Statement`. That is
a stage-11 design concern. Stage 1's debuginfo
rejection covered DWARF-emission instrumentation
(StmtDebugInfos) but did NOT preempt name-mapping at
the Body level, which is distinct semantic content
needed for output readability (Q5 pretty-printer +
generated-C reviewer experience). Outbound
expectation filed to stage 11.

**Methodology rules invoked.** "C semantics with
information preservation" (preserve C-shape-relevant
information; strip rustc-specific debuginfo
infrastructure that has no C analog). "Drop
NLL/borrowck-only carriers" (`debuginfos`
StmtDebugInfo arms include borrowck-side
instrumentation hints; same discipline as stage-3 +
stage-6 +stage-7 dropping `UserTypeAnnotationIndex`).

**Prior-art evidence.**

- `prior-art/rustc.md` §3.1, §3.2, §3.3 (`Statement`
  wrapper struct + `SourceInfo`'s scope field +
  `StmtDebugInfos` semantics).
- `prior-art/rustc_public.md` §2 (the Option-A
  precedent: drop scope and debuginfos; keep span;
  closest existing analog).
- `prior-art/mrustc.md` §4 (Option C precedent: no
  wrapper at all; spans go to diagnostics only).
- `prior-art/charon.md` §3 (Option D evidence:
  `comments_before` for Aeneas; rejected).

**Worked example.** A bridge translation of a single
statement:

```text
mir::Statement {
    source_info: SourceInfo {
        span: rustc_span(file="driver.rs", lo=120, hi=137),
        scope: SourceScope(3),
    },
    kind: StatementKind::Assign(box (place, rvalue)),
    debuginfos: StmtDebugInfos(vec![]),
}
```

translates to:

```text
Statement {
    kind: StatementKind::Assign(translate_place(place), translate_rvalue(rvalue)),
    span: Span { file: <interned src/driver.rs>, lo: 120, hi: 137 },
}
```

The `scope: SourceScope(3)` and `debuginfos: vec![]`
fields are dropped at translation. The `span` carries
the source location for any diagnostic the bridge
might raise about this statement (e.g., a B-1 type-
equality violation downstream).

**Outbound expectations** (to file in
`stage-11/expectations.md` "From stage 8" at merger
close):

> **Named-local preservation.** Stage 11's `Body` /
> `LocalDecls` design must preserve user-source
> variable names (rustc's body-level
> `var_debug_info: Vec<VarDebugInfo>` per
> `[rustc_middle/src/mir/mod.rs:264, 1235-1257]`) for:
> (a) pretty-printer readability per Q5;
> (b) generated-C readability so reviewers see
> `temp = ...;` not `_3 = ...;`;
> (c) downstream debugger / linter usability on the
> emitted C. Stage 1 spec rejected DWARF-emission
> instrumentation (the `StmtDebugInfos` per-statement
> hints) but did NOT preempt name-mapping at the Body
> level, which is distinct semantic content.
> Recommended provision:
> `LocalDecl.debug_name: Option<StringId>` populated
> from rustc's `var_debug_info`, scoped within the
> body, used by `render_local` / `render_place` and
> by the C-backend identifier emission policy.

**Cross-refs.** `spec-snapshot.md` §22.1.1 (wrapper
struct definition + the named-local note); D-8-K
(consolidated outbound expectations bundle to file
at merger close).

---

### D-8-4 -- Statement-site invariants [LOCKED]

**Question.** What rejection rules and positive
invariants apply at the Statement site? Where does
walk-implementation live? Does Statement participate
in cross-build identity?

**Key options.** Five-part registry. The shape is
mostly synthesis (Parts A and B accumulate from D-8-
1.1 -- D-8-1.4); the open dimensions are:
- (a) Part A row ordering / grouping.
- (b) Part B explicit-vs-implicit treatment of
  inherited Place / Rvalue invariants.
- (c) Part C walk-implementation phase placement.
- (d) Part D cross-build identity.
- (e) Part E peer comparison level of detail.

**Decision.** Five-part registry as described in
spec-snapshot.md §22.5:

- **Part A**: 12-row rejection table grouped by
  discharge mechanism (Group 1 CleanupPostBorrowck-
  NOPed: FakeRead, AscribeUserType,
  BackwardIncompatibleDropHint,
  Coverage(BlockMarker | SpanMarker), Fake-borrow
  Assigns; Group 2 always-running cleanup-pass
  strip: PlaceMention, Nop, StorageLive, StorageDead;
  Group 3 build-flag-gated: Retag, ConstEvalCounter,
  Coverage(non-marker)). Each row cites the
  responsible pass / absent flag with file:line.
- **Part B**: 8 positive invariants, with B-0a /
  B-0b / B-0c inherited invariants explicit (rather
  than implicit-by-composition):
  - B-0a I-Statement-Place-Wellformed (inherits
    §19.7 from D-5-2).
  - B-0b I-Statement-Rvalue-Wellformed (inherits
    §21.5.2 from D-7-3, including the §21.5.1 7-row
    rejection table transitively). Applies to
    operands embedded in rvalues reachable through
    `Assign`'s payload.
  - B-0c I-Statement-Operand-Wellformed (inherits
    §20.5 directly). Applies to operands embedded in
    `Intrinsic` payloads (Assume's operand,
    CopyNonOverlapping's src/dst/count operands)
    that do NOT pass through an enclosing rvalue;
    distinct from B-0b.
  - B-1 I-Statement-Assign-Type-Equality (D-8-1.2).
  - B-2 I-Statement-SetDiscriminant-VariantIdx
    (D-8-1.3).
  - B-3 I-Statement-SetDiscriminant-Target (D-8-1.3).
  - B-4 I-Statement-Intrinsic-Assume-Bool (D-8-1.4).
  - B-5 I-Statement-Intrinsic-CopyNonOverlapping-Types
    (D-8-1.4).
- **Part C**: walk-implementation deferred to stage
  11. Same split as §19.7 / §21.5.3 walk-impl
  deferral. Compositional with B-0a / B-0b: the
  stage-11 visitor walks Place / Rvalue / Statement
  invariants in a single pass.
- **Part D**: F4 / F4-2 cross-build identity OUT of
  scope (body-local content per D-8-2; same scope
  reduction as D-5-2 / D-6-2 / D-7-2).
- **Part E**: peer comparison informational. rustc 3
  invariant clusters; rustc_public none; Charon none;
  mrustc fatal MIR_BUG. RegseqModel sides with rustc
  + mrustc on validator discipline.

**Reasoning.** Part A's grouping by discharge
mechanism makes the rejection table readable: a
consumer trying to understand "why is StorageLive
dropped" reads Group 2 and sees the
RemoveStorageMarkers + pass-manager-override trace
in one place; mixing cleanup-NOPed and build-flag-
gated rows would obscure the discharge ordering.

Part B's B-0a / B-0b explicit-rather-than-implicit
treatment is a documentation-clarity choice: the
spec is more legible when inherited invariants are
listed rather than implied by walk-traversal
composition. The cost is duplicate text (also lives
in §19.7 / §21.5.2); the benefit is that a stage-11
walker implementer can read §22.5.2 and see the full
list of rules to apply at every Statement, in one
place.

Part C's walk-impl-at-stage-11 deferral matches the
D-5-2 / D-7-3 precedent: rules at the owning stage,
walk implementation at the body-level (§3.3 Phase B
body validation). Implementing a stage-8-local
visitor would have to be retrofitted at stage 11
when bodies materialise.

Part D restates the cross-build identity scope-
reduction for the registry's completeness.

Part E's peer-comparison level of detail (4 rows,
one per peer, with one-line behaviour summaries)
matches stage-7 D-7-3's Part C peer-comparison
table.

**Methodology rules invoked.** "Compositional
behavior beats special-case flags" (no parallel
"validator discipline level" flag; the walk-impl
falls out of stage-11 body validation). "Spec is
mutable" (Part B's explicit B-0a / B-0b listings
duplicate text from §19.7 / §21.5.2 -- accepted as
documentation cost; if maintenance burden becomes
real, switch to implicit-by-composition).

**Prior-art evidence.**

- `prior-art/rustc.md` §3.7 (validator pinpoints for
  the three Statement-level invariant clusters at
  `validate.rs:1467-1582`).
- `prior-art/rustc_public.md` §5 (no construction-
  site invariants; pre-runtime consumer accommodation).
- `prior-art/charon.md` §4 (no Statement-level
  rejection; absorb-or-drop pattern).
- `prior-art/mrustc.md` §6, §7 (two-layer validator
  with fatal `MIR_BUG`; the stricter discipline that
  RegseqModel matches).

**Worked example.** A bridge walk of a stage-11 body
visiting `BasicBlock.statements[i]` for each i:

```text
1. Match the rustc statement against §22.5.1 (Part A).
   - If rustc_kind matches a row, raise a diagnostic.
2. Else, dispatch on RegseqModel `kind`:
   - For `Assign(p, rv)`:
     - Apply B-0a (visit `p` per §19.7).
     - Apply B-0b (visit `rv` per §21.5.2 + §21.5.1).
     - Apply B-1 (`place_ty(p) == rvalue_ty(rv)`).
   - For `SetDiscriminant(p, vi)`:
     - Apply B-0a (visit `p`).
     - Apply B-2 (`vi < |variants(target_enum)|`).
     - Apply B-3 (`place_ty(p) = Ty::Enum { item,
       args, layout }` per §10).
   - For `Intrinsic(NonDivergingIntrinsic::Assume(op))`:
     - Apply B-0c (the §20.5 operand-site invariants
       apply directly to `op`; this is NOT B-0b
       because the operand is not embedded in an
       enclosing rvalue).
     - Apply B-4 (`operand_ty(op) == Ty::Bool`).
   - For `Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(c))`:
     - Apply B-0c to `c.src`, `c.dst`, `c.count`
       (each is a Statement-level operand; §20.5
       applies directly).
     - Apply B-5 (the deref-able-pointee + pointee-
       equality + count-as-usize rules; cite
       `validate.rs:1517-1549`).
```

Single-pass traversal; B-0a / B-0b handled inline as
the visitor recurses into Place / Rvalue / Operand
content.

**Cross-refs.** `spec-snapshot.md` §22.5 (full
registry); D-8-1.1 (Part A discharge-mechanism trace);
D-8-1.2 (B-1); D-8-1.3 (B-2, B-3); D-8-1.4 (B-4, B-5).

---

### D-8-K -- `pretty.rs::render_statement` deferral [LOCKED]

**Question.** Should `render_statement` be implemented
at stage 8 alongside the closed surface, or deferred
to stage 11?

**Key options.**

1. **Option A -- defer to stage 11.** Match the
   D-5-K / D-6-K / D-7-K pattern. Stage-8 spec
   defines the variants; stage-11 fills the renderer
   alongside `render_place`, `render_operand`,
   `render_rvalue`.
2. **Option B -- implement at stage 8 with stub
   renderers for embedded payloads.** Synthesise
   placeholder `render_place` / `render_operand` /
   `render_rvalue` calls; replace at stage 11.

**Decision.** Option A. Defer to stage 11.

**Reasoning.** Every stage-8 variant payload routes
through stages 5/6/7 renderers transitively:
- `Assign(Place, Rvalue)` needs `render_place` (D-5-K
  -> stage 11) + `render_rvalue` (D-7-K -> stage 11).
- `SetDiscriminant(Place, VariantIdx)` needs
  `render_place` (D-5-K -> stage 11) + variant-name
  lookup direct from `place_ty(p)`'s
  `LayoutKind::Enum.shape` per the §11.1 cases
  (`EnumShape::Single { variant, .. } -> variant.name`;
  `EnumShape::Multiple { variants, .. } ->
  variants[vidx].name`). No late-binding to later
  stages required because the §10 / §11 vocabulary is
  already pinned at construction time.
- `Intrinsic(NonDivergingIntrinsic)` needs
  `render_operand` (D-6-K -> stage 11) for the
  embedded operands in both arms.

Every variant's renderer is gated on at least one
D-X-K -> stage-11 deferral. Implementing Option B at
stage 8 would require synthesising stub renderers for
Place / Rvalue / Operand -- discarded work when stage
11's actual renderers land. Co-locating
`render_statement` with the rest of the body-
rendering subtree at stage 11 keeps the rendering
surface coherent and unblocks body-content snapshot
tests when bodies materialise.

The stage-7 D-7-K precedent applies verbatim: same
deferral pattern, same operand-rendering subtree
driver, same stage-11 outbound expectation.

**Methodology rules invoked.** "Renderer style is
decided deliberately" (`../memory.md` §2 stage-8-
specific corollary). "Compositional behavior beats
special-case flags" (the deferral falls out of
embedded-payload renderers; no parallel
`render_at_stage_8` flag).

**Prior-art evidence.**

- `prior-art/rustc_public.md` §11 (`pretty_statement`
  exists at `pretty.rs:90-126` with `// FIX-ME: Make
  them pretty.` fall-throughs for AscribeUserType,
  Coverage, Intrinsic; partial implementation that
  would need completion at our level).
- `prior-art/charon.md` §27 (full FmtWithCtx impl
  with MIR-style surface; a stage-8-equivalent
  renderer is feasible at this stage, but Charon's
  body-context infrastructure ships alongside).
- `prior-art/mrustc.md` §13 (full per-variant
  `Statement::operator<<` at
  `src/mir/mir.cpp:689-722`; ships alongside
  body-context infrastructure).

**Worked example.** A stage-11 body-content snapshot
test for the §22.7.1 `add_one` example would render:

```text
bb0: {
    _2 = const 1_u32;
    _3 = Add(move _1, move _2);
    _0 = move _3;
    return;
}
```

`render_statement(_2 = const 1_u32)` calls into
`render_place(_2)` (D-5-K -> stage 11) and
`render_rvalue(Use(Constant(c1)))` (D-7-K -> stage
11; transitively `render_operand(Constant(c1))` and
`render_const(c1)` -- D-6-K + D-4-K -> stage 11 +
stage 14). The full call graph terminates only after
stage 11 (and stage 14 for `render_const`).

**Outbound expectations** (to file in
`stage-11/expectations.md` "From stage 8" at merger
close, as a three-item bundle):

1. **`render_statement` deferral.** Implement at
   stage 11 alongside `render_place` (D-5-K),
   `render_operand` (D-6-K), `render_rvalue`
   (D-7-K). Driver: every variant's payload either
   embeds a Place (D-5-K-deferred), an Rvalue
   carrying Operands (D-7-K + D-6-K-deferred), or
   Operands directly (D-6-K-deferred).
2. **Statement walk implementation** (from D-8-4
   Part C). Body-level visitor walks
   `BasicBlock.statements`, applies §22.5.1
   rejection table + §22.5.2 positive invariants in
   one body-level pass; compositional with stage-5
   Place walk + stage-7 Rvalue walk inheritance.
3. **Named-local preservation** (from D-8-3).
   `LocalDecl.debug_name: Option<StringId>`
   populated from rustc's body-level `var_debug_info`
   per `rustc_middle/src/mir/mod.rs:264, 1235-1257`,
   used by `render_local` / `render_place` /
   C-backend identifier emission.

**Cross-refs.** `spec-snapshot.md` §22.8 (rendering
deferral); §18.2 per-method ownership table update
for `render_statement` lands at merger close;
D-8-3 (named-local preservation); D-8-4 (walk-impl
deferral).

---

## 2. Outbound expectations summary

To file in `stage-11/expectations.md` "From stage 8"
at merger close:

1. `render_statement` deferral (D-8-K).
2. Statement walk implementation (D-8-4 Part C).
3. Named-local preservation (D-8-3).

Top-level `spec.md` §18.2 per-method ownership table
update for `render_statement` (with stage-11 owner)
lands at merger close.

No earlier-stage decisions reopened by stage 8. No
cross-stage friction surfaced during Phase A.


## Stage 9 (MIR terminators)

# Stage 9 -- Discussion Record

Distilled record of the design dialogue producing the
stage-9 `Terminator` vocabulary spec. Each entry captures
the question raised, key options considered, the decision,
the reasoning chain, methodology rules invoked, prior-art
evidence, and cross-references to `spec-snapshot.md` (the
spec) and `review-log.md` (the round-by-round audit
trail; created on first review round).

Status of decisions in this file mirrors `spec-snapshot.md`'s
"Terminator vocabulary" section (§23) plus adjacent
additions to §18.2, plus `status.md` "D-tag allocation".

D-tag convention: prefixed `D-9-K` form per `../README.md`
§5.2.

Status tags:
- `[LOCKED]` -- audit-shape recorded; per-stage Phase A
  closed; ready for Phase B Codex review.
- `[REOPENED]` -- a later sub-decision or sibling D-tag
  forced a revision; superseded entry kept for history.
- `[DEFERRED to stage N]` -- moved to a later stage's scope.

Phase A closed 2026-05-05 with all 11 D-tags + 5
sub-decisions LOCKED across a single combined cluster.

---

## 1. Decision discussions

### D-9-1 -- `Terminator` core vocabulary [LOCKED]

**Question.** What is the closed `TerminatorKind` value
shape that the bridge fills from rustc's
`mir::TerminatorKind<'tcx>` and that downstream consumers
(stage 10 basic blocks -- `BasicBlock.terminator:
Terminator`; stage 11 bodies; stage 14 backends) read as
the per-block control-flow-edge surface?

**Key options.**
1. **Mirror rustc verbatim (15 variants).** Carry every
   `mir::TerminatorKind<'tcx>` arm including borrowck
   artifacts (FalseEdge, FalseUnwind), coroutine ops
   (Yield, CoroutineDrop), unwinding terminators
   (UnwindResume, UnwindTerminate), specialized variants
   (TailCall, InlineAsm), and the raw Drop variant.
2. **Mirror rustc_public (10 variants).** Drop the 5
   borrowck/coroutine artifacts; preserve Drop and
   InlineAsm.
3. **Charon-style ~7 variants** with Assert as a Statement
   (not Terminator).
4. **mrustc-style ~10 variants** with Drop as a Statement.
5. **RegseqModel 6 variants.** Smallest closed surface
   that covers well-formed driver MIR post-pipeline:
   Goto, SwitchInt, Return, Unreachable, Call, Assert.

**Decision.** Option 5 -- **6 variants** (D-9-1.1
cardinality), with Drop collapsed at the bridge (D-9-2),
Assert kept at Terminator level (D-9-1.6), and 8 rustc
variants reject-on-encounter (D-9-9).

**Reasoning chain.**
1. The `optimized_mir` pipeline plus our cleanup-pass
   whitelist eliminates 6 rustc variants
   unconditionally (FalseEdge / FalseUnwind via
   `CleanupPostBorrowck`; Yield / CoroutineDrop via
   `coroutine::StateTransform`; UnwindResume /
   UnwindTerminate via `AbortUnwindingCalls` +
   `SimplifyCfg::PreOptimizations`).
2. The DSL surface excludes 2 rustc variants
   structurally: TailCall (requires unstable
   `explicit_tail_calls`) and InlineAsm (DSL provides
   domain-specific intrinsics for low-level operations).
3. Drop is post-`ElaborateDrops` semantically equivalent
   to a call to `drop_in_place::<T>(p)`; collapsing to
   Call at the bridge is the C-shape lowering per
   README §2 "C-semantics with information preservation."
4. Assert is structurally a guarded conditional branch,
   not a statement-level side-effect; its `target` and
   (post-AbortUnwindingCalls implicit) panic-edge make
   it a control-flow construct.

**Methodology rules invoked.**
- README §2 "C-semantics with information preservation"
  (drives Drop-as-Call collapse).
- README §2 "No defensive fallbacks" (rejected variants
  do not get carried defensively).
- README §2 "Closed `Ty` enum" pattern (closed-form
  surface; reject-on-encounter for non-conforming).
- README §2 "Compositional behavior beats special-case
  flags" (per-variant dispatch via the closed enum, not
  flag-shaped discriminators).

**Prior-art evidence.**
- rustc 15-variant `TerminatorKind<'tcx>` at
  `rustc_middle/src/mir/syntax.rs:703-987`.
- rustc_public 10-variant `TerminatorKind` at
  `rustc_public/src/mir/body.rs:154-192`.
- Charon's ~7-variant `RawTerminator` (Assert as
  Statement) at `charon-lib/src/ast/ullbc_ast.rs:41`.
- mrustc's ~10-variant tagged union (Drop as Statement)
  at `src/mir/mir.hpp:710-715`.

**Cross-refs.**
- `spec-snapshot.md` §23.1.2 (TerminatorKind enum +
  comparison table).
- Sub-decisions: D-9-1.1 (cardinality), D-9-1.2 (Goto
  payload), D-9-1.3 (SwitchInt payload), D-9-1.4
  (Return / Unreachable payloads), D-9-1.5 (Call
  payload), D-9-1.6 (Assert payload + Terminator-level
  classification).

---

### D-9-1.1 -- TerminatorKind cardinality [LOCKED]

**Question.** Specifically how many variants in the
closed `TerminatorKind` set, and which?

**Key options.**
- **5 variants:** Goto, SwitchInt, Return, Unreachable,
  Call (Assert demoted to Statement Charon-style; D-9-2
  collapses Drop).
- **6 variants** (recommended): + Assert.
- **7 variants:** + Drop preserved.
- **8+ variants:** + Drop + InlineAsm.

**Decision.** **6 variants** -- Goto, SwitchInt, Return,
Unreachable, Call, Assert.

**Reasoning.**
1. The 6-variant cardinality follows from three
   sub-claims locked elsewhere: (a) D-9-2 collapses Drop
   at the bridge (no Drop variant); (b) D-9-1.6 keeps
   Assert at Terminator level (not Statement); (c)
   D-9-9 rejects InlineAsm + TailCall (DSL-only) and
   the 6 pipeline-discharged variants.
2. Each rejected variant is provably unreachable
   (pipeline-discharged) or out-of-DSL (DSL-only); no
   variant is added defensively.
3. The 6-variant set is the **smallest closed surface**
   that fully expresses well-formed driver MIR
   post-pipeline.

**Methodology rules invoked.**
- README §2 "Closed `Ty` enum" (closed-form pattern at
  the variant level).
- README §2 "No defensive fallbacks."

**Prior-art evidence.** Comparative cardinality:
- rustc 15 / rustc_public 10 / Charon ~7 (Assert
  demoted) / mrustc ~10 (Drop demoted) / **RegseqModel
  6**. The narrowest precedented closed surface;
  intermediate-narrow forms (Charon/mrustc) trade
  Terminator slots for Statement slots, which our
  D-9-1.6 / D-9-2 decisions both reject.

**Cross-refs.**
- `spec-snapshot.md` §23.1.2 (closed 6-variant enum) +
  §23.1.3 (what's dropped).
- D-9-2 (Drop collapse), D-9-1.6 (Assert classification),
  D-9-9 (rejection table).

---

### D-9-1.2 -- `Goto(BlockId)` payload [LOCKED]

**Question.** What is the payload shape of `Goto`?

**Decision.** Single-field flat tuple
`Goto(BlockId)` -- the target block.

**Reasoning.** Universal across rustc / rustc_public /
Charon / mrustc; no alternatives worth surveying. The
`BlockId` resolves to a basic block in the enclosing
body's `basic_blocks` table (stage-10 vocabulary). Bridge
translation: `mir::TerminatorKind::Goto { target } ->
Goto(<bridge_block_id>)`.

**Cross-refs.** `spec-snapshot.md` §23.2.1.

---

### D-9-1.3 -- `SwitchInt { discr, targets }` payload [LOCKED]

**Question.** What is the payload shape of `SwitchInt`?

**Decision.** Two named fields:
```rust
SwitchInt {
    discr:   Operand,
    targets: SwitchTargets,
}
```
where `SwitchTargets` storage is per D-9-6.

**Reasoning.**
1. `discr`'s type ∈ {`Ty::Bool`, `Ty::Char`, `Ty::Int(_)`,
   `Ty::Uint(_)`} (per §23.6.2 B-1; stage-3 type-taxonomy
   match). Enum-discriminant switches reach SwitchInt
   only via a preceding `Rvalue::Discriminant` (stage 7)
   projection that produces an integer-typed Operand;
   SwitchInt itself never sees an enum-typed discr.
2. `targets` carries the (case_value -> block) mapping
   plus the otherwise destination per the explicit-split
   form at D-9-6.

**Methodology rules invoked.** README §2 "Compositional
behavior beats special-case flags" -- discr-type
constraint falls out of stage-3 `Ty` interning + stage-6
operand-type lookup; no parallel `is_typed_switch` flag.

**Prior-art evidence.** Universal payload shape
across rustc (`syntax.rs:715-719`), rustc_public
(`body.rs:158-161`), Charon (similar), mrustc.

**Cross-refs.** `spec-snapshot.md` §23.2.2; D-9-6 for
SwitchTargets storage.

---

### D-9-1.4 -- `Return` and `Unreachable` payloads [LOCKED]

**Question.** What are the payloads of `Return` and
`Unreachable`?

**Decision.** Both unit variants -- no payload.

**Reasoning.**
1. `Return`'s return value lives implicitly at
   `LocalId::RETURN_PLACE` (`_0` per stage-1's
   body-local ID convention); type comes from
   `FnSig.output()` (resolved at stage 11). The
   Terminator carries no operand because the return
   value is at `_0` post-`optimized_mir`.
2. `Unreachable` is the terminating UB marker; no
   structural information is associated with it. Emitted
   naturally from `loop {}` patterns and as the output
   of cleanup-pass simplifications under panic=abort.

**Prior-art evidence.** Universal as unit variants
across rustc / rustc_public / Charon / mrustc.

**Cross-refs.** `spec-snapshot.md` §23.2.3, §23.2.4.

---

### D-9-1.5 -- `Call { callee, args, destination, target }` payload [LOCKED]

**Question.** What is the payload shape of `Call`?

**Decision.** Four named fields:
```rust
Call {
    callee:      CallCallee,            // D-9-3
    args:        Box<[Operand]>,        // D-9-4 (c)
    destination: Place,
    target:      Option<BlockId>,
}
```

**Reasoning** (per-field):
- `callee: CallCallee` per D-9-3 -- `Direct(InstanceId)`
  / `Indirect(Operand)` two-arm enum.
- `args: Box<[Operand]>` per D-9-4 (c) -- bare Operand
  sequence; per-arg spans dropped. `Box<[T]>` matches
  stage-7's `Aggregate` storage-shape rule (slim-pointer
  + length, no `Vec` overhead).
- `destination: Place` always present in
  `optimized_mir` (rustc pre-allocates the destination
  Place post-cleanup; the source-MIR `Option<Place>`
  shape is gone).
- `target: Option<BlockId>` -- `Some(bb)` indicates
  successor block on normal return; `None` indicates a
  divergent call (`panic!()`, `-> !` callee,
  `core::intrinsics::unreachable()`). rustc passthrough
  on semantics.

**Dropped from rustc Call payload** (per D-9-8):
`unwind`, `call_source`, `fn_span`. Each subsumed or
diagnostic-only with no driver-relevant consumer.

**Methodology rules invoked.** README §2 "C-semantics
with information preservation" (drop fields with no
consumer); "No defensive fallbacks" (don't preserve
fn_span / call_source against hypothetical future
codegen-warning use).

**Prior-art evidence.** Field-by-field comparison:
- rustc has `func`, `args: Box<[Spanned<Operand>]>`,
  `destination: Place`, `target: Option<BasicBlock>`,
  `unwind`, `call_source`, `fn_span`
  (`syntax.rs:811-832`).
- rustc_public has `func: Operand`, `args: Vec<Operand>`,
  `destination: Place`, `target: Option<BasicBlockIdx>`,
  `unwind` (`body.rs:171-177`). Drops Spanned wrapper
  on args (`mir.rs:752` does `arg.node.stable(...)`).

**Cross-refs.** `spec-snapshot.md` §23.2.5; D-9-3
(CallCallee), D-9-4 (per-arg spans), D-9-8 (wrapper +
dropped fields).

---

### D-9-1.6 -- Assert as Terminator (not Statement) [LOCKED]

**Question.** Is `Assert` classified as a Terminator (a
control-flow-edge instruction) or as a Statement (a
trapping side-effect)?

**Key options.**
- **(A) Terminator** -- rustc / rustc_public preserve
  Assert at the Terminator level. The `target` field
  carries the success-edge; the panic-edge is
  structurally implicit under panic=abort post-
  `AbortUnwindingCalls`.
- **(B) Statement** -- Charon classifies Assert as a
  StatementKind (`charon-lib/src/ast/ullbc_ast.rs:41`).
  Translator demotes rustc's terminator-level Assert to
  a Statement push + replaces the terminator with a
  plain `Goto`.

**Decision.** **(A) Terminator.**

**Reasoning.** Three load-bearing reasons:

1. **Bridge structural transparency.** Strategy A
   (Terminator) is a one-to-one variant rename + payload
   trim. Strategy B (Statement) requires either
   Goto-synthesis noise (preserve block boundaries) or
   a real CFG flattening pass (Charon's actual choice;
   merges Assert chains across single-predecessor
   blocks). The flattening violates our
   `mir_opt_level=0` discipline of pushing CFG opts
   upstream of the bridge.

2. **CFG-shape stability.** Stage-11 body-level passes
   (e.g., D-9-3 candidate (b)'s vtable-indirect rewrite
   pass) iterate terminators and need "this block has a
   control branch" as a terminator-variant property,
   not a statement-walk derivation. Compiler-theory-
   wise, exit semantics belong on the terminator.

3. **No information advantage to demotion.** Codegen
   output is identical in both strategies (both lower
   to `if (!cond) abort_helper(...); goto target;`).
   Strategy B's Statement-level uniformity buys nothing
   under our straight-through C codegen.

**Methodology rules invoked.**
- README §2 "Compositional behavior beats special-case
  flags" (Strategy B's flattening introduces
  CFG-shape-special-casing that doesn't compose
  cleanly).
- README §2 "C-semantics with information
  preservation" (preserve the structural
  guarded-conditional-branch semantics).

**Prior-art evidence.**
- rustc preserves Assert as Terminator
  (`syntax.rs:875-881`). Compiler-theory aligned with
  guarded-branch classification.
- rustc_public preserves Assert as Terminator
  (`body.rs:178-184`).
- Charon demotes to Statement (`ullbc_ast.rs:41`).
  Motivated by Aeneas verification-semantics target,
  not C codegen.
- mrustc has separate `Panic` Terminator (different
  framing -- explicit divergent terminator with
  unconditional abort, no boolean-guard structure).

**Charon's precedent does not transfer.** Their target
is verification semantics, not C codegen. RegseqModel's
codegen consumes terminator-shaped Assert directly; the
demotion would shift cost without payoff.

**Decision payload** (locked together with the
Terminator-level classification):
```rust
Assert {
    cond:     Operand,
    expected: bool,
    msg:      AssertKind,    // 8-variant per D-9-5
    target:   BlockId,
}
```
Drop `unwind` per D-9-8.

**Cross-refs.** `spec-snapshot.md` §23.2.6; D-9-5
(AssertKind variant subset).

---

### D-9-2 -- Drop-as-Call collapse at the bridge [LOCKED]

**Question.** How does the bridge handle rustc's
`TerminatorKind::Drop`? (a) Preserve as a separate
Terminator variant; (b) collapse at the bridge to a
synthetic Call to drop-glue; (c) collapse at codegen
(preserve as Statement-level).

**Decision.** **(b) Bridge-side collapse** to a synthetic
`TerminatorKind::Call` targeting the drop-glue Instance.

**Reasoning.** Three load-bearing reasons:

1. **C-semantics with information preservation.**
   README §2: "RegseqModel uses C-shaped lowerings
   rather than mirroring Rust-internal compiler shapes.
   The bridge does the desugaring (drop-glue calls
   become regular Calls...)." Drop-as-Call IS the C
   shape; preserving Drop as a variant means every
   downstream consumer must duplicate the collapse
   logic.

2. **Resolves construction invariant 9 closure
   correctness uniformly.** Stage 1's `validate`
   invariant 9 says every `InstanceId` in the model
   must resolve in the instance-closure. Drop-glue
   instances are part of that closure (synthesised by
   the bridge as part of mono-item discovery). With a
   separate `Drop` variant, the closure-walking
   visitor would need a special case: "if you see
   Drop, look up `tcx.resolve_drop_in_place(place_ty)`
   to get the synthetic instance and check its
   closure-membership." With the collapse, the walker
   simply visits every Call's `InstanceId`. Pattern
   uniformity -> simpler invariant.

3. **No information loss.** Post-`ElaborateDrops`,
   rustc's Drop is "an unconditional call to
   drop_in_place with a pointer to the place." The
   variant's extras (`replace: bool`, `drop:
   Option<BasicBlock>`, `async_fut: Option<Local>`)
   are diagnostic-only or async-coroutine-only -- both
   subsumed (per D-9-8) or unreachable under our DSL
   (coroutines type-rejected at stage 3 §15.1).

**Methodology rules invoked.**
- README §2 "C-semantics with information preservation."
- README §2 "Compositional behavior beats special-case
  flags" (uniform Call surface vs Drop-special-case
  Visitor logic).

**Prior-art evidence.**
- **rustc-internal precedent (load-bearing).** rustc's
  codegen-SSA performs the same collapse internally
  inside the backend dispatch loop at
  `rustc_codegen_ssa/src/mir/block.rs:599-693` --
  `codegen_drop_terminator` resolves drop-glue via
  `Instance::resolve_drop_in_place(tcx, ty)` and
  dispatches through `helper.do_call(...)` exactly as
  if the source was an explicit `drop_in_place::<T>(p)`
  call. The bridge performs the collapse one stage
  earlier so all RegseqModel consumers see uniform
  Call.
- **Charon precedent.** Charon's optional
  `--desugar-drops` pass at
  `charon-lib/src/transform/normalize/desugar_drops.rs:23-72`
  is structurally identical: rewrites `Drop { place,
  target, ... }` into a `Call` with `&raw mut place`
  arg, fresh unit-typed return slot, copied `target`.
  RegseqModel applies the same shape unconditionally
  at the bridge instead of as an opt-in normalize
  pass.
- **Charon's belt-and-braces ZST elision.** Charon's
  `filter_trivial_drops` pass collapses no-op-destruct
  Drops directly to `Goto`. RegseqModel defers this to
  stage-14 codegen peephole (per policy b.1; see
  `spec-snapshot.md` §23.4 step 2).
- **Stage-1 inbound expectation.** Per
  `expectations.md` "From stage 1": "Stage 9 lowers
  `Drop` terminators to regular `Call` terminators
  targeting the drop-glue `Instance` for the type."

**Bridge translation rule (5 steps):** see
`spec-snapshot.md` §23.4. Closes stage-1 inbound
"Drop lowered to Call" expectation; closes stage-1
inbound bridge concern about drop-glue Instance
interning.

**Outbound to stage 11.** Fresh `LocalId` allocation
for the `&raw mut place` temp + fresh unit-typed
return slot are stage-11 LocalDecls concerns. Filed in
`stage-11/expectations.md` "From stage 9".

**Cross-refs.** `spec-snapshot.md` §23.4 (5-step
bridge rule with full citations); §23.6.2 B-7 (closes
construction invariant 9 uniformly via `Direct`
walks).

---

### D-9-3 -- Call callee shape supporting vtable-indirect dispatch [LOCKED]

**Question.** What does `Call.callee` look like in the
closed 6-variant `TerminatorKind`? The shape must serve
three call shapes that arise in driver MIR: (1) direct
calls to monomorphized functions, (2) function-pointer
calls (`let f: fn(...) = ...; f(x)`), (3) virtual
dispatch through `dyn Trait`.

**Key options.**

1. **(a) Reopen stage-1 InstanceKind to add `Virtual { idx:
   usize }`.** Public Regseq InstanceKind grows from 6
   emission variants to 7. `Call.callee: InstanceId`
   stays uniform; the InstanceKind discriminator
   distinguishes Direct vs Virtual.

2. **(b) Widen Call.callee to `CallCallee::{Direct(InstanceId),
   Indirect(Operand)}`.** Closed 2-arm enum at the
   callee position. `Direct` carries a static
   `InstanceId` (handles case 1). `Indirect(Operand)`
   carries a runtime function-pointer value (handles
   cases 2 and 3 uniformly).

**Decision.** **(b)** -- `CallCallee::{Direct(InstanceId),
Indirect(Operand)}`. Stage-1 InstanceKind variant set
stays intact at 6 variants (no new variant added).
**However**, post-locking review (R3) surfaced a
schema-extension requirement: the `InstanceKind::Function`
variant gains a `virtual_idx: Option<u32>` field to
preserve rustc's vtable-slot index for Virtual
instances (per §9.4 construction invariant: `virtual_idx.is_some()`
iff `flavor == FunctionInstanceFlavor::Virtual`).
Stage-11's vtable-indirect rewrite reads this field to
determine which vtable slot to load. The schema
addition is a **stage-9 D-9-3 cross-stage revision**
(the schema-extension procedure anticipated by the
stage-1 inbound expectation "Possibly extend
`DefPathData` for synthetic vtable scopes" and the
broader "stage-1 revision triggered by stage 9"
methodology pattern). The mirror invariant lives at
§23.6.2 B-8.

**Reasoning.** Five load-bearing reasons:

1. **Required for completeness regardless of dyn
   Trait.** Function-pointer calls (case 2:
   `let f: fn(...) = ...; f(args)`) need a
   runtime-resolved callee position. Stage-1's
   emission-only InstanceKind closure cannot model
   "the callee is determined at runtime from a runtime
   value." Option (a) has no clean home for case 2.

2. **Closes the function-pointer-call gap that option
   (a) would leave open.** The current spec
   (`spec.md` §9.4) already exposes the rustc-internal
   Virtual discriminator via `InstanceKind::Function.flavor:
   FunctionInstanceFlavor` with the `Virtual` variant
   (the `flavor` field was added by a post-stage-1
   spec revision and is now canonical in top-level
   `spec.md`; the frozen `stage-1/spec-snapshot.md`
   historical artifact predates this revision and
   shows only `Function { sig, body }`). Stage-9 D-9-3
   further extends `Function` with `virtual_idx:
   Option<u32>` to preserve rustc's vtable-slot index
   (also a cross-stage revision; see §9.4 construction
   invariant + §23.6.2 B-8). With these schema
   pieces, the bridge can emit `Direct(<id>)` with
   `flavor=Virtual` + `virtual_idx=Some(idx)` and
   stage 11 can detect + rewrite it.
   However, function-pointer calls (`let f: fn(...) =
   ...; f(args)`) need a runtime-resolved callee
   position, and `InstanceKind::Function.flavor` cannot
   model "the callee is determined at runtime from a
   runtime value" -- the InstanceId is statically
   resolved by definition. Option (a) handles only
   case 3 (vtable dispatch); case 2 (function-pointer
   calls) requires a callee-shape distinction
   regardless. Option (b) handles cases 2-3 uniformly
   through the `Indirect(Operand)` arm.

3. **Proper IR variant per compiler theory.** Direct/
   Indirect call distinction is universal across mature
   IRs:
   - LLVM IR: `call` direct vs indirect on SSA value.
   - Cranelift IR: `call` vs `call_indirect`.
   - mrustc: `CallTarget = Value(LValue) |
     Path(::HIR::Path) | Intrinsic` at
     `src/mir/mir.hpp:595-602`.
   - Charon: `FnOperand::{Regular(FnPtr), Move(Place)}`
     at `charon-lib/src/ast/gast.rs:386-391`.
   Making it explicit at the IR level surfaces a
   structural distinction into the type system, where
   the type checker enforces it. Aligns with the
   project's compiler-theory-first methodology rule.

4. **C-codegen shape match.** C distinguishes
   `name(args)` (direct) from `(*ptr)(args)` (indirect).
   The IR mirrors this directly under (b).

5. **Cleans up invariant 9 closure-walk.** Direct's
   InstanceId is closure-checked; Indirect's Operand
   has no InstanceId to check (function pointer's value
   is a runtime concern, not a closure concern). Option
   (a) would require a Virtual special case in the
   closure-walking visitor (skip the closure check
   because the InstanceId points to a vtable shim, not
   a single Instance).

**Methodology rules invoked.**
- README §2 "Compiler-theory-first" -- prefer proper
  IR variants over narrow workarounds. The Direct/
  Indirect split is the proper IR variant; option (a)
  is a narrow workaround that handles only case 3
  partially.
- README §2 "Compositional behavior beats special-case
  flags" -- option (a) introduces a Virtual variant
  that's structurally different from the emission-only
  InstanceKind variants; option (b) factors the
  distinction at the call site where it belongs.
- README §2 "C semantics with information preservation"
  -- preserve the structural Direct/Indirect distinction
  the C language exposes.

**Prior-art evidence.**
- **rustc-internal favors (a)-shape at the resolution
  layer** but uses (b)-shape at the IR level: `Call.func:
  Operand` is uniformly an Operand (the discriminator is
  recovered from the Operand's type/Constant resolution).
  Source: `rustc_middle/src/mir/syntax.rs:811-832`.
- **rustc_public favors (a)-shape:** exposes
  `InstanceKind { Item, Intrinsic, Virtual { idx },
  Shim }` at `rustc_public/src/mir/mono.rs:30-37`;
  bridge translator lifts `ty::InstanceKind::Virtual(_,
  idx)` directly into the public InstanceKind at
  `rustc_public_bridge/src/unstable/convert/stable/ty.rs:949-951`.
- **Charon favors (b)-shape:** `FnOperand::{Regular(FnPtr),
  Move(Place)}` callee split at
  `charon-lib/src/ast/gast.rs:386-391`;
  `transform_dyn_trait_calls` pass at
  `charon-lib/src/transform/normalize/transform_dyn_trait_calls.rs:31-122`
  synthesizes vtable function-pointer load via
  `PtrMetadata -> Deref -> Field(method_field_id)`
  projection chain and mutates `call.func =
  FnOperand::Move(method_field_place)` at line 119.
- **mrustc favors (b)-flavored shape:**
  `CallTarget::{Path, Value, Intrinsic}` callee union
  at `src/mir/mir.hpp:595-602`; `MIR_Cleanup_Virtualize`
  pass at `src/mir/cleanup.cpp:548-684` flips
  `Call.fcn = Path(<dyn Trait>::method)` to
  `Value(<vtable_loaded_lv>)`.

**Foreclosed alternative.** Candidate (a) -- relying
on the stage-1 `FunctionInstanceFlavor::Virtual`
discriminator alone -- handles only case 3 (vtable
dispatch); case 2 (function-pointer calls) requires a
callee-shape distinction regardless. Option (b)'s
`Indirect(Operand)` arm subsumes case 2 cleanly. Once
(b) is in the IR for case 2, using it uniformly for
case 3 (post-rewrite) is structurally cleaner than
maintaining a parallel discriminator-based path. (a)
is foreclosed not because the discriminator is missing
-- it isn't -- but because (b) handles strictly more
of the call-shape design space.

**Bridge translation rule (4 cases):**

| Rustc `Call.func` shape | RegseqModel `callee` |
|-------------------------|----------------------|
| `Operand::Constant(c)` where `c.const_.ty() = Ty::FnDef(def_id, args)` resolves to a non-Virtual Instance | `Direct(<bridge-interned InstanceId>)` -- resolved Regseq Instance has `kind = InstanceKind::Function { flavor: <non-Virtual variant>, .. }` |
| `Operand::Constant(c)` where the resolved rustc Instance has `InstanceKind::Virtual(_, idx)` | `Direct(<bridge-interned InstanceId>)` -- resolved Regseq Instance has `kind = InstanceKind::Function { flavor: FunctionInstanceFlavor::Virtual, virtual_idx: Some(idx), .. }` (per top-level `spec.md` §9.4 with the D-9-3 schema extension). This is the **pre-rewrite shape**; stage-11's vtable-indirect rewrite (per §23.6.2 B-3/B-4/B-5/B-6 post-rewrite invariant timing + B-8 call-site projection of the §9.4 virtual_idx biconditional) walks every `Direct(InstanceId)`, reads `Instance.kind.flavor` + `virtual_idx`, and rewrites to `Indirect(Operand)` synthesizing the vtable load through index `virtual_idx.unwrap()` |
| `Operand::Move(p)` / `Copy(p)` where `p.ty() = Ty::FnPtr(...)` | `Indirect(Operand::Move(p))` / `Indirect(Operand::Copy(p))` |
| Anything else | bridge-rejection (mistyped callee shape) |

**Outbound to stage 11.** Body-level rewrite pass
recognising rustc `InstanceKind::Virtual`-resolved Calls
and rewriting them to `Indirect(Operand)` form via
synthesised `PtrMetadata -> Deref -> Field(method_idx)`
projection statements + fresh LocalIds. Charon's
`transform_dyn_trait_calls.rs:31-122` is the direct
template. Filed in `stage-11/expectations.md`.

**Outbound to stage 12.** Vtable shim Item synthesis
(already filed by stage 1; cross-ref). Under D-9-3
(b), stage 9 introduces no new InstanceKind variant
and therefore no Virtual-shim-scope DefPathData need.
**Stage 12's separate DefPathData encoding choice for
vtable type Items and vtable instance Items** (per
`stage-12/expectations.md` "From stage 3" Sections A
and B -- "overload an existing variant, or add a new
`DefPathData::VTableType` variant per stage-1's
anticipated revision procedure") **remains a stage-12
decision**; D-9-3 does not constrain it.

**Closes inbound expectations:**
- Stage-1: "Vtable-indirect dispatch (Call terminator
  shape)" -- closed.
- Stage-1: "Possibly extend `DefPathData` for synthetic
  vtable scopes" -- closed only as it relates to
  stage-9's call-callee shape (no Virtual-shim-scope
  variant needed). Stage-12's separate vtable Item
  DefPathData encoding choice is **not** closed by
  D-9-3 and remains a stage-12 decision.
- Stage-3 D5: "Define Call terminator shape supporting
  vtable-indirect dispatch" -- closed.

**Cross-refs.** `spec-snapshot.md` §23.3.1 (CallCallee
enum) + §23.7 (bridge translation rules) + §23.8.6
(virtual dispatch worked example).

---

### D-9-4 -- Per-arg span preservation for Call args [LOCKED]

**Question.** What does the `args` field of
`TerminatorKind::Call` look like? Stage 6 D-6-1.3 dropped
the operand-internal span and listed two candidates;
rustc_public surfaced a third.

**Key options.**
- **(a) Inline wrapper:** `args: Box<[ArgWithSpan]>`
  where `pub struct ArgWithSpan { operand: Operand,
  span: Span }`. Mirrors rustc's `Spanned<Operand>`.
- **(b) Parallel slabs:** `args: Box<[Operand]>` plus
  `arg_spans: Box<[Span]>`. Decouples span from operand
  storage.
- **(c) Drop spans entirely:** `args: Box<[Operand]>`.
  No per-arg span preserved. Direct precedent:
  rustc_public at
  `rustc_public_bridge/src/unstable/convert/stable/mir.rs:752`
  does `args.iter().map(|arg|
  arg.node.stable(...)).collect()` -- strips the
  `Spanned` wrapper unconditionally.

**Decision.** **(c) Drop spans entirely.** `args:
Box<[Operand]>` -- bare Operand sequence; no per-arg
span carried. Per-arg diagnostic granularity falls back
to the Terminator-level `span` (the call site as a
whole).

**Reasoning.** Three load-bearing reasons:

1. **No load-bearing consumer.** Stage-11
   `validate(...)` walk uses structural type-equality /
   arity-match invariants (not span-keyed); stage-11
   body-level rewrite passes operate on Operand
   structure (not source position); stage-14 codegen
   does not emit spans. Compiler-internal driver-source
   diagnostics run pre-bridge at the rustc/HIR layer
   (per-arg spans there, not at RegseqModel level).
   Speculative future consumers (phase-2 device-pressure
   debugging, codegen-time arg warnings) have workable
   Terminator-level fallbacks; for driver code where
   calls typically take 0-4 args, granularity loss is
   bounded.

2. **No defensive preservation.** Stage 6 D-6-1.3
   deferred the question to stage 9 specifically so the
   consumer landscape could be examined with full IR
   visibility. Examined; no consumer warrants the
   storage overhead.

3. **rustc_public peer alignment + consistency with
   D-9-8.** rustc_public (closest existing owned-IR
   analog) drops per-arg spans; D-9-8 drops
   `source_info.scope`, `Call.call_source`,
   `Call.fn_span`, `Call.unwind` with the same "no
   driver-relevant consumer" rationale. Per-arg spans
   fall in the same category.

**Methodology rules invoked.**
- README §2 "No defensive fallbacks" -- per-arg spans
  preservation against hypothetical future
  codegen-warning use is exactly the pattern the rule
  rejects.
- README §2 "C semantics with information preservation"
  -- preserve information rustc gives us *for free*,
  *interrogated per construct*: per-arg spans fail
  the "who consumes them" interrogation.

**Prior-art evidence.**
- **rustc preserves `Spanned<Operand>`** at
  `rustc_middle/src/mir/syntax.rs:820`. Per-arg span
  granularity is a rustc design feature.
- **rustc_public drops per-arg spans** at
  `rustc_public_bridge/src/unstable/convert/stable/mir.rs:752`
  (`arg.node.stable(...)` strips the Spanned wrapper).
  Direct precedent for option (c).
- Charon and mrustc both store args as
  `Vec<Operand>` -- no per-arg spans (consistent with
  option (c)).

**Bridge translation:** for each rustc
`Call.args: Box<[Spanned<Operand<'tcx>>]>`,
`args.iter().map(|sp| translate_operand(&sp.node)).collect()`
-- strips the `Spanned` wrapper, drops `sp.span`.

**Supersedes inbound expectation:** stage-6 D-6-1.3
"`Spanned<Operand>` per-arg span preservation". The
inbound listed (a) inline-wrapper and (b)
parallel-slabs as the two preservation candidates and
expected stage 9 to pick one; D-9-4 instead **revises
the expectation** by introducing a third option (c)
drop-entirely, motivated by the consumer-landscape
analysis. Stage-6 D-6-1.3 is closed by supersession,
not by satisfaction-as-written.

**Cross-refs.** `spec-snapshot.md` §23.2.5 (Call
payload args field) + §23.7 (bridge translation rules).

---

### D-9-5 -- AssertKind variant subset [LOCKED]

**Question.** `Assert.msg: AssertKind` carries the
panic-message data. Rustc's `AssertKind<O>` has 11
variants. Which subset does RegseqModel keep?

**Key options.**
- **(α) All 11 variants** (rustc_public-aligned).
- **(β) 8-variant subset** -- drop 3 coroutine-only
  variants.
- **(γ) Charon-style 3-variant `AbortKind` collapse**
  with `msg` field discarded wholesale.

**Decision.** **(β) 8-variant subset.** Drop
`ResumedAfterReturn`, `ResumedAfterPanic`,
`ResumedAfterDrop`; keep BoundsCheck, Overflow,
OverflowNeg, DivisionByZero, RemainderByZero,
MisalignedPointerDereference, NullPointerDereference,
InvalidEnumConstruction.

**Reasoning.** Three load-bearing reasons:

1. **Type-rejection cascade consistency.** Stages 5-8
   each reject variants whose precondition is
   type-rejected at stage 3 (coroutines, Box, etc.).
   The 3 coroutine-only Assert variants fit this
   discipline cleanly: their types cannot reach
   RegseqModel (stage-3 §15.1 rejects `Coroutine`
   types), so the variants are structurally
   unreachable. Carrying unreachable variants is the
   same anti-pattern as carrying unreachable types.

2. **Diagnostic-quality preservation for
   driver-relevant cases.** Driver code does encounter
   the 8 retained variants:
   - **BoundsCheck** -- array indexing bounds checks.
   - **Overflow / OverflowNeg** -- arithmetic overflow
     in MMIO field manipulation, saturating
     arithmetic.
   - **DivisionByZero / RemainderByZero** -- rare but
     possible in driver clock-rate computation.
   - **MisalignedPointerDereference / NullPointerDereference**
     -- hot path for raw-pointer driver code.
   - **InvalidEnumConstruction** -- enum-construction
     validation.
   Each lowers to a specific
   `__rust_abort_with_msg(format_str, operands...)`
   call providing useful runtime diagnostics. (γ)'s
   collapse to bare `panic` forfeits this.

3. **rustc_public-aligned with a single deliberate
   divergence.** rustc_public preserves all 11; we drop
   3 on the type-rejection-cascade rule. The divergence
   is principled, documentable as "AssertKind variants
   whose precondition is type-rejected at stage 3 are
   pruned," and aligned with the stage-7/-8 discipline.

**Methodology rules invoked.**
- README §2 "No defensive fallbacks" (don't carry
  unreachable variants).
- README §2 "C-semantics with information preservation"
  (preserve diagnostic content where it serves
  driver-relevant cases).

**Prior-art evidence.**
- rustc 11-variant `AssertKind<O>` at
  `rustc_middle/src/mir/syntax.rs:1066-1078`.
- rustc_public preserves all 11 at
  `rustc_public/src/mir/body.rs:260-273`.
- Charon collapses to 3-variant `AbortKind` at
  `charon-lib/src/ast/...` -- forfeits diagnostic
  content; aligned with verification-IR target, not
  C codegen.
- Stage-7 D-7-1 dropped variants whose preconditions
  type-rejected at stage 3 (Coroutine constructions);
  stage-8 D-8-1.3 narrowed similarly. RegseqModel-
  consistent pattern.

**Bridge translation.** 8 variants translate
one-to-one (operands routed through stage-6 Operand
machinery, BinOp through stage-7 BinOp machinery); 3
coroutine variants emit a bridge-rejection diagnostic
(double-discharged with `coroutine::StateTransform`).

**Cross-refs.** `spec-snapshot.md` §23.3.3 (AssertKind
enum); D-9-1.6 (Assert payload); §23.6.1 row 3/4
(coroutine variant rejection).

---

### D-9-6 -- SwitchTargets storage shape [LOCKED]

**Question.** What is the storage shape of
`SwitchTargets`? Two candidates:

- **(a) Mirror rustc** -- "values + targets, last
  targets entry is otherwise" with `targets.len() ==
  values.len() + 1` invariant. Schema:
  `values: Box<[u128]>` + `targets: Box<[BlockId]>`
  with end-of-vector otherwise convention.
- **(b) Explicit split** -- `cases: Box<[(u128,
  BlockId)]>` plus `otherwise: BlockId`. Decouples the
  invariant from the storage shape.

**Decision.** **(b) Explicit split.**

```rust
pub struct SwitchTargets {
    pub cases:     Box<[(u128, BlockId)]>,
    pub otherwise: BlockId,
}
```

**Reasoning.** Three load-bearing reasons:

1. **Explicit > implicit.** The otherwise destination
   is intrinsically different from a case block
   (catch-all, not value-mapped). (a)'s positional
   encoding hides that distinction; (b) makes it
   structurally explicit.

2. **No runtime invariant to enforce.** (a) requires
   `targets.len() == values.len() + 1` walk-pass-check;
   (b) lets the type system enforce the otherwise
   field's presence as a struct field.

3. **Compositional with the zero-cases edge case.**
   Under (b), `cases: []` + `otherwise: bb` is natural;
   under (a), `values: []` + `targets: [otherwise]` is
   a len-1 special case.

**Methodology rules invoked.**
- README §2 "Compositional behavior beats special-case
  flags" -- "the last entry is special-case otherwise"
  is exactly the kind of positional-special-case the
  rule pushes against.

**Prior-art evidence.**
- rustc uses (a) at `rustc_middle/src/mir/syntax.rs:1005-1031`.
  Rustc's rationale (in source comments at 1014-1029):
  keeping targets in a single sliceable type is
  important because `TerminatorKind::successors`
  returns `impl Iterator` -- multiple concrete iterator
  types would require boxed trait objects. **This
  rationale does not apply to RegseqModel**: we don't
  share an iterator across variants the way rustc does.
- rustc_public uses an (b)-shape variant:
  `SwitchTargets { branches: Vec<(u128, BasicBlockIdx)>,
  otherwise: BasicBlockIdx }`. Closer peer for our
  case.
- Charon splits by type: `SwitchKind` variants
  (`If(BlockId, BlockId)` for bool, `SwitchInt(IntegerTy,
  Vec<(ScalarValue, BlockId)>, BlockId)` for integer).
  Explicit otherwise field across all variants.

**Bridge translation:** see `spec-snapshot.md` §23.7
(translate one-pass over rustc's parallel arrays).
The case-value width is `u128`; rustc's `Pu128`
(packed wrapper for HashStable) translates to bare
`u128`.

**Cross-refs.** `spec-snapshot.md` §23.3.2; D-9-1.3
(SwitchInt payload using SwitchTargets); §23.6.2 B-1
(discr type invariant).

---

### D-9-7 -- No Terminator interning [LOCKED]

**Question.** Is `Terminator` interned (top-level table
with `TerminatorId` indices) or stored inline at the
basic-block boundary?

**Decision.** **Inline storage.** No top-level
`terminators` interner table; no `TerminatorId` index
type. `BasicBlock.terminator: Terminator` is the
storage form (stage-10 vocabulary preview).

**Reasoning.** Three load-bearing reasons:

1. **Universal precedent.** All four prior-art
   projects store terminators inline at the basic-block
   boundary:
   - rustc: `BasicBlockData<'tcx>.terminator:
     Option<Terminator<'tcx>>`.
   - rustc_public: `BasicBlock.terminator: Terminator`
     at `rustc_public/src/mir/body.rs:135-139`.
   - Charon: `BlockData.terminator: Terminator` at
     `charon-lib/src/ast/ullbc_ast.rs`.
   - mrustc: `BasicBlock.terminator: Terminator` at
     `src/mir/mir.hpp`.
   No project interns.

2. **No deduplication win.** Inter-body terminator
   sharing is theoretically possible (every `Return`
   is identical; `Unreachable` is identical) but
   vanishingly thin. `Goto(bb)`, `SwitchInt`, `Call`,
   `Assert` all carry body-local data (`BlockId`,
   embedded `Operand`/`Place`, target lists, callee
   `InstanceId`) -- practically zero cross-body
   byte-equality. The two unit variants are 1 byte
   each; even with thousands of bodies, savings are
   negligible against the indirection cost.

3. **Pattern-match consistency.** Stages 5-8 lock
   inline storage at every level (Place projections
   D-5-x, Operand D-6-2, Rvalue D-7-2, Statement
   D-8-2). The "body-local content lives inside the
   body" rule is established. Interning Terminator
   alone breaks the rule for no payoff.

**Methodology rules invoked.**
- README §2 "Closed `Ty` enum" pattern (closed-form
  vocabulary; structural identity).
- Pattern consistency with stages 5-8 inline storage.

**F4 / F4-2 cross-build identity is OUT of scope.**
Terminator content is per-build structural;
cross-build canonicalisation operates on
`(InstanceKey)` per stage-1 F4-2, not
`(BodyContents)`. Same scope reduction as stages 5-8.

**Cross-refs.** `spec-snapshot.md` §23.5; stage-10
preview shape `BasicBlock { statements, terminator }`.

---

### D-9-8 -- Terminator wrapper struct shape [LOCKED]

**Question.** What does the `Terminator` wrapper struct
(holding `TerminatorKind` plus per-terminator metadata)
look like, and which rustc-side fields survive into
RegseqModel?

**Decision.**
```rust
pub struct Terminator {
    pub kind: TerminatorKind,
    pub span: Span,
}
```

Direct precedent: rustc_public at
`rustc_public/src/mir/body.rs:140`. Mirrors stage-8
D-8-3 `Statement { kind, span: Span }`.

**Dropped fields:**

| Field | Source | Drop reason |
|-------|--------|-------------|
| `source_info.scope: SourceScope` | wrapper | Debuginfo-only DWARF-emission index. RegseqModel does not emit DWARF (per stage-1 spec rejection). |
| `Call.call_source: CallSource` | variant payload | 5-variant diagnostic enum (`OverloadedOperator`/`MatchCmp`/`Misc`/`Use`/`Normal`); rustc itself uses it only for "better error messages." |
| `Call.fn_span: Span` | variant payload | Function-position span without the receiver -- per-call diagnostic refinement subsumed by either the wrapper-level `span` or the per-arg span at D-9-4. |
| `Drop.replace: bool` | variant payload | Diagnostic-only flag. Subsumed by D-9-2 collapse to Call. |
| `Drop.drop: Option<BasicBlock>` | variant payload | Async-drop coroutine yield-point; coroutines type-rejected at stage 3 §15.1; subsumed by D-9-2. |
| `Drop.async_fut: Option<Local>` | variant payload | Async-drop future local; coroutines type-rejected; subsumed by D-9-2. |
| `unwind: UnwindAction` (on Call/Drop/Assert/InlineAsm) | variant payload | Under panic=abort with `AbortUnwindingCalls` having run, only `Continue` and `Unreachable` survive at our consumption point and collapse semantically. The field carries no driver-relevant information. |

**Sub-decision: `unwind: UnwindAction` -- drop entirely.**

`UnwindAction` has 4 variants: `Continue`, `Unreachable`,
`Terminate(reason)`, `Cleanup(BasicBlock)`. Under our
pinned config:
- `panic = "abort"` is required for driver `no_std`
  builds.
- `AbortUnwindingCalls` runs at
  `rustc_mir_transform/src/lib.rs:651` (in
  `run_runtime_lowering_passes`, before
  `optimized_mir`); under panic=abort it rewrites
  unwind edges to `UnwindAction::Unreachable` at
  `rustc_mir_transform/src/abort_unwinding_calls.rs:47`.
- Cleanup blocks become unreachable;
  `SimplifyCfg::PreOptimizations` (in our cleanup-pass
  whitelist) prunes them.

Surviving values at our consumption point: `Continue`
and `Unreachable` -- both semantically equivalent
because there are no cleanup edges to traverse.
`Cleanup(_)` and `Terminate(_)` are gone. **Dropping is
sound.**

**Edge case** (acknowledged but not addressed): the
rustc prior-art flagged `extern "C-unwind"` as a
potential way for unwinding to leak into drop glue even
under panic=abort. Under our DSL surface, FFI is `extern
"C"` only (Regseq does not surface `extern "C-unwind"`).
If a future DSL extension introduces unwinding-FFI,
that's a new spec event with its own decision; the
methodology rule "no defensive fallbacks" applies -- we
don't preserve `unwind` defensively against a
hypothetical that isn't on the roadmap.

**Methodology rules invoked.**
- README §2 "C-semantics with information preservation"
  (preserve fields with consumers; drop those without).
- README §2 "No defensive fallbacks" (don't preserve
  `unwind` against a hypothetical DSL extension).

**Prior-art evidence.**
- rustc `Terminator { source_info, kind }` at
  `rustc_middle/src/mir/terminator.rs` -- preserves
  scope.
- rustc_public `Terminator { kind, span }` at
  `rustc_public/src/mir/body.rs:140` -- drops scope.
  Direct precedent.
- mrustc no source-info on terminator (spans are
  diagnostic-only and discarded).

**Cross-refs.** `spec-snapshot.md` §23.1.1 (wrapper
struct definition); D-9-4 (per-arg spans).

---

### D-9-9 -- Cleanup-pass-discharged rejection table [LOCKED]

**Question.** Which rustc `TerminatorKind<'tcx>` variants
does the bridge reject, what diagnostic does each
produce, and what is the discharge mechanism (the ground
for asserting the variant won't appear under our pinned
config)?

**Decision.** 9-row rejection table with two-category
framing:
- **Pipeline-discharged (rows 1-6):** rustc's required
  passes eliminate these before `optimized_mir`.
  Encountering one in well-formed `optimized_mir`
  indicates a pipeline-config bug.
- **DSL-only (rows 7-8):** rustc supports the variant
  in `optimized_mir`; the Regseq DSL surface does not
  include the source-level construct. Encountering one
  indicates a driver-source error.
- **Bridge-collapsed (row 9):** Drop is converted to
  Call per D-9-2; row 9 is in the table for
  completeness so the implementer doesn't list Drop as
  "supported as-is."

The full table appears at `spec-snapshot.md` §23.6.1.

**Reasoning.** The two-category framing reflects a real
structural distinction surfaced by the rustc prior-art:

- **Rows 1-6 are pipeline-discharged unconditionally.**
  `CleanupPostBorrowck` (rows 1-2; `is_required: true`)
  rewrites FalseEdge/FalseUnwind to Goto;
  `coroutine::StateTransform` (rows 3-4) discharges
  Yield/CoroutineDrop; `AbortUnwindingCalls` + cleanup-
  block pruning (rows 5-6) eliminates UnwindResume/
  UnwindTerminate under panic=abort.

- **Rows 7-8 are NOT pipeline-discharged.** TailCall
  reaches `optimized_mir` whenever
  `#![feature(explicit_tail_calls)]` is set; InlineAsm
  reaches it unconditionally. The bridge cannot rely on
  rustc to remove them; rejection must be DSL-level
  (a wellformedness-of-driver-source property), not
  pipeline-level.

This distinction matters for the diagnostic message:
rows 1-6 carry an "internal error: pipeline
misconfiguration" framing distinct from rows 7-8's
"driver source error" framing.

**Methodology rules invoked.**
- README §2 "No defensive fallbacks" -- variants whose
  precondition is satisfied by the pinned toolchain
  are not carried defensively.
- README §2 "Closed `Ty` enum" pattern (closed-form
  surface; reject-on-encounter for non-conforming).

**Prior-art evidence.**
- **rustc-internal rejection points** (the "Rejection
  points" table in `rustc-references.md`):
  `rustc_codegen_ssa/src/mir/block.rs:1550-1555` --
  `bug!("coroutine ops in codegen")` for
  Yield/CoroutineDrop; `bug!("borrowck false edges in
  codegen")` for FalseEdge/FalseUnwind. Same placement
  pattern as our rejection table; we use
  `unsupported_construct!` instead of `bug!` so the
  driver author sees a useful message.
- **rustc_public's bridge translator** at
  `rustc_public_bridge/src/unstable/convert/stable/mir.rs:704-790`
  -- 5 explicit drops (`TailCall` via `todo!()`;
  Yield / CoroutineDrop / FalseEdge / FalseUnwind via
  `unreachable!()`). The `unreachable!()` vs
  `todo!()` discipline distinction (correctness
  precondition violated vs incomplete implementation)
  surfaces here too -- we don't `panic`, but we
  surface the framing in our diagnostic message.

**Walk-pass enforcement deferred to stage 11** per
D-9-10 Part C. The 9-row table specifies rejection at
the bridge translation site; the walk-pass is
mostly tautological under the closed-enum surface but
covers the case where some future construction path
might violate the surface.

**Cross-refs.** `spec-snapshot.md` §23.6.1 (full
9-row table).

---

### D-9-10 -- Terminator-site invariants registry [LOCKED]

**Question.** What invariants do well-formed Terminator
content satisfy, beyond those carried over from earlier
stages? How are they enforced, and at what stage?

**Decision.** 4-part registry.

**Part A: rejection table.** Cross-references D-9-9
(9-row rejection table; 8 rejected rustc variants + 1
collapsed via D-9-2).

**Part B: positive invariants.** 2 inherited + 9
stage-9-introduced (11 total). B-8 was added at the
stage-9 R4/R5 per-stage review iteration alongside the
§9.4 schema extension `virtual_idx: Option<u32>`; B-9
was added at the post-merge R5 review (target /
callee-output coherence).

*Inherited (2):* stage-9 Terminator embeds Place
(Call.destination) and Operand (multiple sites);
Statement-level and Rvalue-level invariants do not
propagate (those constructs do not appear in Terminator
content).
- **B-0a** -- All embedded Place content satisfies
  Place invariants per stage-5 §19.7.
- **B-0c** -- All embedded Operand content satisfies
  Operand invariants per stage-6 §20.5.

*Stage-9-introduced (9):*
- **B-1** -- `SwitchInt.discr` Operand has type ∈
  {`Ty::Bool`, `Ty::Char`, `Ty::Int(_)`, `Ty::Uint(_)`}.
- **B-2** -- `Assert.cond` Operand has type `Ty::Bool`.
- **B-3** -- Call destination type matches callee
  return type **when `target: Some(_)`** (Direct: via
  `lookup_instance(...).fn_sig.output()`; Indirect:
  via Operand type's `Ty::FnPtr.output()`). Divergent
  calls (`target: None`) are excluded; their callee
  return type is `Ty::Never` per B-9.
- **B-4** -- Call args length matches callee signature
  arity.
- **B-5** -- Call callee is callable (Direct: InstanceId
  always callable by stage-1 instance-closure
  construction; Indirect: Operand type ∈ {Ty::FnPtr(_)}).
- **B-6** -- Per-arg type match. Catches stage-11
  rewrite-pass bugs (e.g., the vtable-indirect rewrite
  passing the wrong receiver type).
- **B-7** -- Closes construction invariant 9: every
  `Direct(InstanceId)` referenced from any
  Terminator::Call resolves in `model.instances`.
- **B-8** -- Terminator-site projection of the §9.4
  global biconditional. For every `Direct(InstanceId)`
  in any Terminator::Call whose resolved Instance has
  `flavor=Virtual`, `virtual_idx.is_some()`. The
  global biconditional `virtual_idx.is_some() iff
  flavor == Virtual` (covering all Function instances)
  lives at §9.4 + §3.3 step 8 model validation; B-8
  here is the per-call-site projection.
- **B-9** -- `target` / callee-output coherence.
  `target.is_none()` iff callee output ==
  `Ty::Never`. Catches bridge-translation bugs that
  emit a diverging-target Call with a non-`!`-returning
  callee or vice versa.

**Part C: walk-implementation deferred to stage 11.**
Same precedent as stage-8 D-8-4 Part C. Stage 11 owns
`Body::validate(...)` walk + visitor-trait extensions
+ actual `assert!` checks + bridge-rejection diagnostic
emission for §23.6.1 row encounters.

**Part D: F4 / F4-2 cross-build identity OUT of
scope.** Per D-9-7 (no Terminator interning), Terminator
is body-local content. Cross-build identity operates on
`(InstanceKey)` per stage-1 F4-2.

**Reasoning.** The 4-part structure provides a complete
contract for stage 11 to consume:
- Part A says what to reject (boundary check at the
  bridge).
- Part B says what to positively check (in-body
  walks).
- Part C names the consumer (stage 11).
- Part D scopes out F4/F4-2 (no cross-build identity).

**Methodology rules invoked.**
- README §2 "Compositional behavior beats special-case
  flags" -- B-0a/c inherited; no parallel
  re-checking of Place/Operand invariants.
- Pattern consistency with stage-8 D-8-4 5-part
  registry.

**Prior-art evidence.** rustc uses `validate.rs`
per-Terminator rules with `bug!()` (`rustc_mir_transform/src/validate.rs`);
rustc_public uses light validation; Charon uses
walk-based well-formedness checks
(`charon-lib/src/transform/check_*`); mrustc uses
two-layer validator (`MIR_Validate` + `MIR_Validate_Full`)
with fatal `MIR_BUG`. RegseqModel takes the
walk-pass-deferred-to-stage-11 discipline matching
stages 5-8.

**Outbound to stage 11.** Walk implementation per
Part C; visitor-trait extensions covering Terminator,
CallCallee, SwitchTargets, AssertKind. Filed in
`stage-11/expectations.md` "From stage 9".

**Closes inbound expectation:** stage-1's deferred
construction invariant 9 (per `expectations.md`
"From stage 1") -- closed by B-7 (rules) + outbound to
stage 11 (walk).

**Cross-refs.** `spec-snapshot.md` §23.6 (full 4-part
registry); §23.6.1 (rejection table); §23.6.2
(positive invariants); §23.6.3 (walk-implementation
deferral).

---

### D-9-K -- `pretty.rs::render_terminator` deferral [LOCKED]

**Question.** When does `pretty.rs::render_terminator`
get implemented?

**Decision.** **Deferred to stage 11**, alongside
`render_place` (D-5-K), `render_operand` (D-6-K),
`render_rvalue` (D-7-K), and `render_statement` (D-8-K).
Stage 11 owns all five `render_*` methods together;
the deferral cascade D-5-K -> D-6-K -> D-7-K -> D-8-K
-> D-9-K closes here.

Four public renderers to implement at stage 11 (plus
one internal dispatch helper not in the §18.2 ownership
table):
- `render_terminator(&Terminator, &Body)` -- top-level
  public entry point. Internally dispatches per-variant
  on `TerminatorKind` (the dispatch helper is a private
  detail of stage-11's implementation, not a separate
  public renderer).
- `render_call_callee(&CallCallee, &Body)` -- the
  D-9-3 (b) two-arm dispatch. Public.
- `render_switch_targets(&SwitchTargets, &Body)` --
  the D-9-6 explicit-split form. Public.
- `render_assert_kind(&AssertKind, &Body)` -- the
  D-9-5 8-variant dispatch. Public.

**Reasoning.** Stage-9 Terminator references content
that needs render-time resolution from sources only
fully populated at stage 11 (and partially at stage 12).

Per-variant dependency:

| Variant | Rendering input | Stage |
|---------|-----------------|-------|
| `Goto(BlockId)` | block-label rendering convention | 11 (Body owns BlockId space) |
| `SwitchInt { discr, targets }` | render_operand + render_switch_targets | 11 |
| `Return` / `Unreachable` | self-contained keywords | -- |
| `Call { callee, args, destination, target }` | render_call_callee + render_operand + render_place + block-label | 11 + 12 (CallCallee::Direct) |
| `Assert { cond, expected, msg, target }` | render_operand + bool literal + render_assert_kind + block-label | 11 |

`CallCallee::Direct(InstanceId)` rendering late-binds to
**stage 12**: the InstanceId resolves to an `Instance`,
whose `Item.def_path` is rendered via
`render_def_path(ItemId)` per stage-1's helper (stub at
`crates/regseq-model/src/pretty.rs`; stage-12 fills the
body per `stage-12/expectations.md`). The
`render_call_callee` `Direct` arm calls
`render_def_path` and the result is the C function
symbol name.

`CallCallee::Indirect(Operand)` delegates entirely to
`render_operand` -- no stage-12 dependency.

The InstanceId-rendering subtree (Call.callee::Direct)
adds a stage-12 dependency; render_terminator lands at
stage 11 *with a forward-reference to stage-12 def-path
rendering* -- matching how stage 8's render_statement
lands at stage 11 with hooks to render_operand /
render_place / render_rvalue.

**Methodology rules invoked.**
- "Land at the latest first-batch dependency"
  pattern from D-5-K, D-6-K, D-7-K, D-8-K.
- README §2 "Spec before implementation" --
  rendering is implementation; defer until the spec
  is fully closed.

**Prior-art evidence.** Direct cascade from stages
5-8: every body-content renderer defers to stage 11.
No project-specific precedent needed.

**§18.2 per-method ownership table update.** Adds
4 public renderer rows for stage-9 deferred methods
(not the private `render_terminator_kind` dispatch
helper):
- `render_terminator(&Terminator, &Body)` -- stage 11.
- `render_call_callee(&CallCallee, &Body)` -- stage 11.
- `render_switch_targets(&SwitchTargets, &Body)` --
  stage 11.
- `render_assert_kind(&AssertKind, &Body)` -- stage 11.

**Outbound to stage 11.** Three-item bundle (mirroring
D-8-K's three-item outbound):
1. `render_terminator` deferral with the four
   sub-renderers.
2. Terminator walk implementation (D-9-10 Part C).
3. Body-level vtable-indirect rewrite pass per D-9-3
   (b).

Plus the bridge concerns from D-9-2 (Drop-as-Call):
fresh LocalIds for `&raw mut place` temp + fresh
unit-typed return slot. Filed in
`stage-11/expectations.md` "From stage 9".

**Cross-refs.** `spec-snapshot.md` §23.9 (rendering
deferral note + per-variant dependency table); §18.2
(per-method ownership table update).

---

## 2. Outbound expectations summary

To file in `stage-11/expectations.md` "From stage 9":

1. `render_terminator` deferral with 4 sub-renderers
   (D-9-K).
2. Terminator walk implementation (D-9-10 Part C).
3. Body-level vtable-indirect rewrite pass (D-9-3 (b)
   outbound; closes stage-3 D5 inbound shape question).
4. Fresh LocalId allocation for `&raw mut place` temp
   (D-9-2).
5. Fresh unit-typed return slot for drop-glue Call
   (D-9-2).

To cross-ref in `stage-12/expectations.md` "From
stage 9":

- "Possibly extend `DefPathData`" question CLOSED
  with "no extension required" disposition under
  D-9-3 (b).
- Vtable shim Item synthesis cross-ref to existing
  "From stage 3" Section A/C entries (informational).

Top-level `spec.md` §18.2 per-method ownership table
update for `render_terminator`, `render_call_callee`,
`render_switch_targets`, `render_assert_kind` (all
stage-11 owner) lands at merger close.

## 3. Earlier-stage decisions touched

**Stage-1 inbound expectations closed:**
- "Drop lowered to Call" (closed by D-9-2).
- "Vtable-indirect dispatch (Call terminator shape)"
  (closed by D-9-3 (b)).
- "Possibly extend `DefPathData` for synthetic vtable
  scopes" (closed only as it relates to stage-9's
  call-callee shape; under D-9-3 (b), no Virtual-shim-
  scope DefPathData variant is needed at stage 9.
  Stage-12's separate vtable Item DefPathData encoding
  choice remains open per
  `stage-12/expectations.md` "From stage 3" Sections A
  and B; D-9-3 does not constrain it.)
- Construction invariant 9 (closed by D-9-10 B-7
  rules + outbound walk to stage 11).

**Stage-3 D5 inbound closed:**
- "Define Call terminator shape supporting
  vtable-indirect dispatch" (closed by D-9-3 (b)).

**Stage-6 D-6-1.3 inbound superseded:**
- "`Spanned<Operand>` per-arg span preservation"
  (superseded by D-9-4: the inbound listed (a) and
  (b) preservation shapes; D-9-4 introduced (c)
  drop-entirely on consumer-landscape grounds).
  Closure is by revision, not by satisfaction.

No earlier-stage decisions reopened by stage 9. No
cross-stage friction surfaced during Phase A discussion.

---

---

## Stage 10 (MIR basic blocks)

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

Phase A closed 2026-05-05 with all 6 top-level D-tags + 21
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
   D-10-2.3's model-vs-analysis line.

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
walks. Outbound expectation filed in
`stage-11/expectations.md` "From stage 10" (item 2 of
the 5-item outbound bundle).

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
4. The walk-impl is non-vacuous: a "every successor
   j is either back-edge (`j <= i`) or `j > i`"
   formulation would be trivially true; the joint
   DFS-RPO procedure asserts a real enforcement
   (storage permutation matches the CFG's RPO).
5. Pre-checks for B-NonEmpty (step 1) and
   B-Successors-Valid (step 2) precede the DFS-RPO
   core so that an out-of-range successor surfaces
   as a clean B-Successors-Valid diagnostic rather
   than as an `IndexVec`-indexing panic.

**Methodology rules invoked.** None new; established
deferral pattern + the §24.4.3 enforcement procedure.

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
- `stage-11/expectations.md` "From stage 10" item 2.

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
5. Walk-ownership split: Part A rows 1-2 are
   bridge-enforced at construction (rustc-side fields
   gone post-construction); the stage-11 walk
   enforces only Part B invariants. RegseqModel's
   `BasicBlock` shape makes Part-A conditions
   structurally inexpressible post-construction, so
   the walk inherits the guarantee without
   re-checking.
6. B-Reachable as a positive invariant (Part B):
   reachability is enforced at the bridge boundary
   by the stage-11 DFS-RPO walk (`§24.4.3` step 4 --
   a DFS from `BlockId(0)` that yields fewer blocks
   than `body.basic_blocks.len()` flags a violation).
   The spec does not depend on rustc-pipeline
   scheduling for soundness; rustc's
   `SimplifyCfg::PreOptimizations` +
   `remove_dead_blocks`
   (`rustc_mir_transform/src/lib.rs:666-680`,
   `rustc_mir_transform/src/simplify.rs:79-85,349-383`)
   produces reachable-only bodies in practice but
   that's an upstream property, not a load-bearing
   spec assumption.
   (Note: `ReorderBasicBlocks::invert_bijective_mapping`
   does NOT structurally enforce reachability --
   `rustc_index::slice::invert_bijective_mapping` uses
   `debug_assert_eq!` over `self.len()` not
   `body.basic_blocks.len()`, and `prettify.rs:28-30`
   early-returns before inversion if `rpo` is already
   sorted. Reachability comes from upstream
   `SimplifyCfg::PreOptimizations` +
   `remove_dead_blocks`, not from
   `ReorderBasicBlocks` itself.) Per the README "no
   defensive fallbacks" rule: the walk verifies; we
   do not duplicate rustc-pipeline assumptions in
   spec text.

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
  load-bearing.
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
  diagnostic rather than an indexing panic.
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
- D-10-K.3: §18.2 per-method ownership table carries
  one row for `render_basic_block` (stage-11-owned).
  The sigil helper is a stage-11 internal helper,
  not public-surface; no row.

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
  row for `render_basic_block`).
- Sub-decisions: D-10-K.1 (`render_basic_block`
  deferral), D-10-K.2 (sigil helper deferral), D-10-K.3
  (§18.2 row).

---

## 2. Outbound expectations

Filed in `stage-11/expectations.md` "From stage 10":

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

These updates land in top-level `spec.md` as part of
the merger's in-place refinement of earlier-stage
content per README §5.9.

---

## Stage 11 (MIR bodies)

# Stage 11 -- Discussion Record

Distilled record of the design dialogue producing the
stage-11 body-level vocabulary spec. Each entry captures
the question raised, key options considered, the decision,
the reasoning chain, methodology rules invoked, prior-art
evidence, and cross-references to `spec-snapshot.md` (the
spec) and `review-log.md` (the round-by-round audit
trail; created on first review round).

Status of decisions in this file mirrors `spec-snapshot.md`'s
"Body-level vocabulary" section (§25) plus adjacent
in-place updates to §14 (FnSig placement note) and §18.2
(per-method ownership renderer-row reframing per D-11-K).

D-tag convention: prefixed `D-11-K` form per `../README.md`
§5.2.

Status tags:
- `[LOCKED]` -- audit-shape recorded; per-stage Phase A
  closed; ready for Phase B Codex review.
- `[REOPENED]` -- a later sub-decision or sibling D-tag
  forced a revision; superseded entry kept for history.
- `[DEFERRED to stage N]` -- moved to a later stage's scope.

Phase A closed 2026-05-05 with all 8 top-level D-tags +
1 sub-decision (D-11-6.1) + 1 K-tag (D-11-K) LOCKED across
a single combined cluster.

---

## 1. Decision discussions

### D-11-1 -- `Body` field set [LOCKED]

**Question.** What is the closed `Body` function-level
container shape that the bridge fills from rustc's
`mir::Body<'tcx>` and that downstream consumers (stage 12
Items, stage 14 backends) read as the unit of MIR? Stage 11
must define the field set, layout convention for the
local_decls partition, and the relationship to the parent
Item.

**Options considered.**
- (a) Flat 4-field shape: `Body { basic_blocks, local_decls,
  arg_count, span }`. rustc + rustc_public precedent.
- (b) Charon-style with `Locals` substructure:
  `Body { basic_blocks, locals: Locals, span }` where
  `Locals { arg_count, locals: IndexVec<LocalId, LocalDecl> }`.
  Co-locates partition info with data.

**Decision.** (a) flat 4-field shape. Per spec-snapshot.md
§25.1: `Body { basic_blocks: IndexVec<BlockId, BasicBlock>,
local_decls: IndexVec<LocalId, LocalDecl>, arg_count: usize,
span: Span }`.

**Reasoning.**
- rustc_public is structurally closest peer (post-mono,
  owned IR, no `'tcx`). Its 6-field shape (`{ blocks,
  locals, arg_count, var_debug_info, spread_arg, span }`)
  collapses to 4 in RegseqModel after dropping
  `var_debug_info` (lifted to per-LocalDecl per D-8-3 +
  D-11-2.5) and `spread_arg` (rust-call ABI rejected per
  D15).
- Charon's `Locals` substructure pays off because Charon's
  locals carry richer state (binder kind, mut flag, phase
  markers) that benefit from grouping. RegseqModel's
  4-field LocalDecl (D-11-2) is simpler; substructure adds
  layer without proportional benefit.
- Semantic operations (`return_local()`, `arg_locals()`,
  `inner_locals()`, `fresh_var()`) work cleanly as methods
  on Body directly without requiring substructure.
- Body has no `fn_sig` field (D-11-3 reframe; FnSig on
  Item-side per 4-peer convergence).
- Body has no `item: ItemId` back-pointer (D-11-1.7 zero
  shape; matches D-11-5 alt (c)).
- 14 rustc Body fields are dropped; rejection rationale per
  field documented in §25.1's rejection table.

**Methodology rules invoked.**
- *C-semantics with information preservation* -- store
  rustc's resolved body form (post-mono, post-cleanup);
  the 17->4 collapse follows mechanically from RegseqModel
  binding constraints.
- *Compositional behavior beats special-case flags* -- no
  parallel `is_post_mono` / `is_no_debuginfo` / etc. flags;
  the dropped fields fall out compositionally from each
  binding constraint.
- *Re-derive from evidence; do not import baggage* -- field
  set re-derived from kickoff evidence + 4-peer prior-art,
  not lifted from rustc's full Body shape.

**Prior-art evidence.**
- rustc: 17-field `Body<'tcx>` at `mir/mod.rs:208-335`;
  rationale per dropped field at `prior-art/rustc.md:32`
  §1.
- rustc_public: 6 fields `{ blocks, locals, arg_count,
  var_debug_info, spread_arg, span }` at `body.rs:13-38`;
  see `prior-art/rustc_public.md`. Closest peer to
  RegseqModel.
- Charon: 3 fields `{ basic_blocks, locals: Locals, span }`
  with `Locals { arg_count, locals }` substructure at
  `gast.rs:31-41`; see `prior-art/charon.md`.
- mrustc: 3-field minimum `{ locals, drop_flags, blocks }`
  at `mir.hpp:1117-1130`; see `prior-art/mrustc.md`.

**Worked example.**
```rust
// Constructed Body for `fn add(x: u32, y: u32) -> u32 { x + y }`:
Body {
    basic_blocks: IndexVec::from_iter(vec![
        BasicBlock { statements: ..., terminator: Return },
    ]),
    local_decls: IndexVec::from_iter(vec![
        LocalDecl { ty: t_u32, mutability: Mut, span: ..., debug_name: None },     // _0 RETURN_PLACE
        LocalDecl { ty: t_u32, mutability: Not, span: ..., debug_name: Some("x") }, // _1 arg
        LocalDecl { ty: t_u32, mutability: Not, span: ..., debug_name: Some("y") }, // _2 arg
    ]),
    arg_count: 2,
    span: ...,
}
```

**Spec cross-reference.** §25.1.

**Sub-decisions.**
- **D-11-1.1** Cardinality: 4 fields. [LOCKED]
- **D-11-1.2** `basic_blocks: IndexVec<BlockId, BasicBlock>`
  inherited from stage-10 D-10-2; placement on Body
  confirmed. [LOCKED]
- **D-11-1.3** `local_decls: IndexVec<LocalId, LocalDecl>`
  flat (no Locals substructure). [LOCKED]
- **D-11-1.4** `arg_count: usize` flat alongside local_decls.
  Layout convention `[0]=RETURN_PLACE, [1..=arg_count]=args,
  [arg_count+1..]=temps` matches rustc + rustc_public + Charon.
  Method shorthands on Body without substructure layer.
  [LOCKED]
- **D-11-1.5** `span: Span` whole-body diagnostic location;
  per-construct diagnostics use their own spans. rustc +
  rustc_public + Charon all keep this. [LOCKED]
- **D-11-1.6** Rejection table for 14 dropped rustc Body
  fields with one-line rationale per drop (post-mono / no
  DWARF / borrowck-discharged / coroutine-rejected /
  collector-discharged / instrumentation-rejected /
  debuginfo-lifted-to-LocalDecl / Item-side-linkage). See
  §25.1 rejection table. [LOCKED]
- **D-11-1.7** No body-level `item: ItemId` field; Body↔Item
  linkage Item-side per D-11-5 (zero shape). Matches
  rustc_public + Charon (no body-side back-pointer);
  rustc's `MirSource` is closest analog and is dropped per
  the rejection table. [LOCKED]

---

### D-11-2 -- `LocalDecl` field set [LOCKED]

**Question.** What is the closed `LocalDecl` per-local
container shape that the bridge fills from rustc's
`mir::LocalDecl<'tcx>` (combined with body-level
`var_debug_info`) and that the joint validation walk plus
stage-13 renderers consume?

**Options considered.**
- (a) 3-field `{ ty, span, mutability }` (rustc_public).
- (b) 3-field `{ index, name, ty }` (Charon; no
  mutability, no span).
- (c) 4-field `{ ty, mutability, span, debug_name }` (lift
  named-local from body-level var_debug_info per D-8-3).
- (d) Bare `TypeRef` (mrustc).

**Decision.** (c) 4-field shape per stage-8 D-8-3's outbound:
`LocalDecl { ty: TyId, mutability: Mutability, span: Span,
debug_name: Option<StringId> }`.

**Reasoning.**
- The named-local lift to per-LocalDecl is justified by
  C-shape ergonomics: per-local lookup at render time +
  codegen identifier emission without secondary scan over
  body-level `var_debug_info`. rustc itself partitions
  `var_debug_info` per-`Local` for codegen consumption
  via `FunctionCx.per_local_var_debug_info` -- structurally
  identical to the design D-8-3 specified.
- `mutability` and `span` preserve information rustc gives
  us for free (information-preservation rule); cost is
  small (1 byte + 4 bytes per LocalDecl); bridge has the
  data. Charon's drop is purchasable because Charon is a
  verification IR; RegseqModel's C-codegen + author
  debugging benefits from preservation.
- `debug_name = None` covers all synthetic locals
  (compiler-synthesized rustc temps; bridge-introduced
  Drop-as-Call locals; rewrite-pass-introduced vtable-
  rewrite locals). No parallel `is_synthetic` flag needed
  (compositional rule).
- Charon's `Local.name: Option<Symbol>` is direct positive
  precedent for the lift. mrustc's commented-out
  `local_names` is negative precedent (their HIR-from-MIR
  flow doesn't preserve, so they rejected). RegseqModel
  inherits debug-friendliness as a design priority.

**Methodology rules invoked.**
- *C-semantics with information preservation* -- preserve
  mutability + span + debug_name; rustc has the info; storing
  it costs little.
- *Compositional behavior beats special-case flags* -- no
  `is_synthetic` flag; `debug_name=None` distinguishes synthetic.

**Prior-art evidence.**
- rustc: 5 fields + 9-variant `LocalInfo` at
  `mir/mod.rs:951-1054`; LocalInfo is mostly diagnostic /
  borrowck (almost entirely upstream-discharged for our
  purposes). See `prior-art/rustc.md:104` §2 (with §2.1-2.5
  sub-sections at lines 134, 142, 146, 150, 160).
- rustc_public: 3 fields `{ ty, span, mutability }` at
  `body.rs:126-131`; named-locals body-level via
  `var_debug_info` + `VarDebugInfo::local()` helper at
  `body.rs:732-739`. See `prior-art/rustc_public.md:176`
  §2.
- Charon: 3 fields `{ index, name: Option<Symbol>, ty }` at
  `gast.rs:47-52`; positive precedent for debug_name lift.
  See `prior-art/charon.md:139` §2.
- mrustc: bare `HIR::TypeRef` per local; commented-out
  `local_names` (rejected). See `prior-art/mrustc.md:141`
  §2.

**Spec cross-reference.** §25.2.

**Sub-decisions.**
- **D-11-2.1** Cardinality: 4 fields. [LOCKED]
- **D-11-2.2** `ty: TyId` (stage-3 type interner reference).
  [LOCKED]
- **D-11-2.3** `mutability: Mutability` (stage-3 D11 enum
  `{ Not, Mut }`); information-preservation rationale.
  [LOCKED]
- **D-11-2.4** `span: Span` (stage-1 file-local Span);
  whole-LocalDecl diagnostic location; `Span::dummy()` for
  synthetic locals. [LOCKED]
- **D-11-2.5** `debug_name: Option<StringId>` lifted from
  body-level `var_debug_info` at construction time per
  stage-8 D-8-3 outbound. Closes D-8-3's outbound. The
  bridge translates each `VarDebugInfo` entry whose `value =
  VarDebugInfoContents::Place(p)` with empty projection
  into a per-LocalDecl.debug_name for `p.local`. Composite
  / non-Place / non-empty-projection entries are
  bridge-rejected. [LOCKED]
- **D-11-2.6** Rejection table for 3 dropped rustc LocalDecl
  fields (`local_info` 9-variant enum / `user_ty` / `source_info.scope`)
  with one-line rationale per drop. See §25.2's rejection
  table. [LOCKED]

---

### D-11-3 -- `FnSig` location reframe [LOCKED]

**Question.** Where does the function signature live in
RegseqModel's IR — on the Body, on the Item, or as a
side-table? Stage-1 inbound framed `FnSig` as a structural
field of Body (with `abi: FnAbi` as a structural field of
FnSig per Q2). Stage-11 must answer the placement question
left open by stage 1.

**Options considered.**
- (a) Body field: `Body { ..., fn_sig: FnSig }`. Stage-1
  inbound reading.
- (b) Item field: `Item.kind = Function { fn_sig, ... }`
  per stage-12's Item.kind expansion.
- (c) Instance field: `InstanceKind::Function { flavor,
  virtual_idx, fn_sig, ... }` extending stage-9 §9.4.
- (d) Drop `fn_sig` field; reach via `local_decls[0..=arg_count]`
  + abi on Body.
- (e) FnSig on Item only; Body has no fn_sig at all.

**Decision.** (e) FnSig on Item-side; Body has no fn_sig
field. 4-peer convergence (rustc, rustc_public, Charon,
mrustc); supersedes stage-1 inbound's Body-side framing.

**Reasoning.**
- 4-peer evidence (rustc-internal `tcx.fn_sig(def_id)`,
  rustc_public `FnDef::fn_sig() -> PolyFnSig` at
  `ty.rs:713-717`, Charon `FunDecl.signature: FunSig`
  separate from `FunDecl.body`, mrustc HIR-layer `Function`
  signature) all keep signature Item-side. No peer puts it
  Body-side.
- Function-defining Items carry the signature; bodies
  consume content shaped by it but don't duplicate the
  signature object. Foreign declarations (extern fns
  with no body) still have signatures via the Item-side
  field — alt (a) Body-side cannot accommodate this case.
- §14's canonical FnSig schema (5 fields: `inputs`, `output`,
  `abi`, `c_variadic`, `safety`) is independent of placement;
  stage-3 D15 already locked the abi enum. Stage-1 inbound's
  schema framing stands; only the placement reframes.
- Stage-1's `sigs` side-table framing fails Q2 (eager
  translation; fully-owned IR; no side-channel lookups).
  Item-side structural field IS the eager owned representation.
- Argument types remain reachable from Body via positional
  layout (`local_decls[1..=arg_count]`); return type via
  `local_decls[RETURN_PLACE]`. Body-side schema is not lossy;
  signature is just not duplicated.
- Stage 12 (Items) absorbs the precise schema (`Item.kind =
  Function { fn_sig, ... }` per its expanded variant set);
  stage 11 only locks the placement decision.

**Methodology rules invoked.**
- *Spec is mutable* -- stage-1 inbound's framing is reframed
  based on evidence-driven 4-peer convergence; the schema
  carries through but the placement was an open question.
- *Re-derive from evidence; do not import baggage* -- the
  Item-side placement is the convergent answer evidence
  surfaces; stage-1's Body-side reading was a default to be
  pressure-tested, not a binding choice.
- *Compositional behavior beats special-case flags* -- no
  parallel `has_signature` flag on Body; foreign declarations
  use `Item.body = None` + Item.kind=Function.fn_sig=Some.

**Prior-art evidence.**
- rustc: signature reached via `tcx.fn_sig(def_id) ->
  EarlyBinder<PolyFnSig>` at `ty/mod.rs`; `Body::return_ty()`
  reads `local_decls[RETURN_PLACE].ty` directly. See
  `prior-art/rustc.md:173` §3 (with §3.1-3.4 sub-sections
  at lines 194, 202, 211, 217).
- rustc_public: `FnDef::fn_sig() -> PolyFnSig` at `ty.rs:713-717`;
  no body-level fn_sig field at `body.rs:13-38`. Argument
  types reached via `Body::arg_locals()` helper. See
  `prior-art/rustc_public.md:63` §1 (Body field set; covers
  the no-fn_sig observation directly).
- Charon: `FunDecl.signature: FunSig` separate structural
  field from `FunDecl.body: Body`; `FunSig { is_unsafe,
  generics, inputs, output }` carries no abi (verification
  IR specifics). See `prior-art/charon.md:209` §3.
- mrustc: HIR-layer `Function::m_sig` carrying `m_variadic`,
  `m_unsafe`; no MIR-layer FnSig. See
  `prior-art/mrustc.md:200` §3.

**Cross-stage friction documentation.** Stage-1 inbound
expectation framed FnSig as "a structural field of FnSig
... structural, not a side-channel lookup." Stage-1's
framing pinned: (1) FnSig is structural and owned (Q2),
(2) abi is inside FnSig, not in a separate side-channel.
The stage-1 framing did NOT pin location (Body vs Item);
the implied "Body" reading was a default. D-11-3 reframes
the location to Item-side without contradicting the
stage-1 schema or Q2/D15. The reframe is a "Spec is
mutable" application.

**Spec cross-reference.** §14 (placement note added);
§25.4 (Body has no fn_sig field; signature reachable
through `local_decls`).

**Sub-decisions.**
- **D-11-3.1** Body has no `fn_sig` field (negative pin;
  closes the stage-1 inbound's Body-side framing). [LOCKED]
- **D-11-3.2** FnSig location: Item-side per (e); 4-peer
  convergence. Stage-12 finalizes the precise placement
  (`Item.kind = Function { fn_sig, ... }` likely; could be
  refined). [LOCKED]
- **D-11-3.3** Argument types reached via
  `Body.local_decls[1..=arg_count].ty` (positional
  convention; matches rustc + rustc_public + Charon).
  [LOCKED]
- **D-11-3.4** Return type reached via
  `Body.local_decls[RETURN_PLACE].ty` (where `RETURN_PLACE
  = LocalId(0)` per D-11-4); matches rustc's `Body::return_ty()`
  + rustc_public's positional helpers. [LOCKED]
- **D-11-3.5** Outbound expectation to
  `stage-12/expectations.md` "From stage 11 (per D-11-3)":
  stage 12 absorbs FnSig as structural field of
  `Item.kind = Function`. The canonical FnSig schema is the
  pinned 5-field form per §14.3 (`inputs: Vec<TyId>`,
  `output: TyId`, `abi: FnAbi`, `c_variadic: bool`,
  `safety: Safety`); D-11-3 does NOT revise §14's schema.
  Stage 12 absorbs the §14 schema as a structural field of
  `Item.kind = Function`; the placement decision is
  stage-11's contribution, the schema is stage-1/stage-3's
  pinned form. [LOCKED]
- **D-11-3.6** Stage-1 inbound's "FnSig as structural field
  of Body" framing is **superseded** by (e); the reframe
  is documented in this audit-shape entry per the
  "Spec is mutable" methodology rule. [LOCKED]

---

### D-11-4 -- `LocalId` typed-index [LOCKED]

**Question.** How is `LocalId` declared in the IR? Stage-1
forward-declared it conceptually only; stage-11 lands the
typed-index introduction, the `RETURN_PLACE` const,
size/niche assertions, and the sigil convention.

**Options considered.**
- (a) `newtype_index!` macro with `const RETURN_PLACE = 0;`
  using stage-10 D-10-3.2's const-body arm. Mirrors stage-10's
  `BlockId::START_BLOCK` pattern.
- (b) Plain u32 newtype with manual associated const + manual
  niche.

**Decision.** (a) `newtype_index!` with const-body declaration.
Per spec-snapshot.md §25.3.

**Reasoning.**
- Symmetric with stage-10 D-10-3 (BlockId via newtype_index!
  + macro arm extension + niche/size assertions + sigil
  convention). Mechanical inheritance of established pattern.
- Stage-10 D-10-3.2 already extended the macro grammar with
  the const-body arm; stage 11 consumes it. Closes stage-10's
  deferred `LocalId::RETURN_PLACE` migration outbound.
- rustc + Charon use proper typed newtype indexes;
  rustc_public + mrustc use bare aliases (negative precedent
  for the alt-(b) bare-typedef approach).
- `RETURN_PLACE` naming matches rustc convention and stage-1
  inbound text (vs rustc_public's `RETURN_LOCAL`); the
  "PLACE" suffix signals its role in `Place { local:
  RETURN_PLACE, projection: [] }`.

**Methodology rules invoked.**
- *Re-derive from evidence; do not import baggage* -- typed
  index discipline derived from stage-1 ID conventions +
  rustc/Charon evidence.
- *Compiler-theory-first* -- typed indexes prevent confusion
  between `LocalId` and `BlockId` at compile time; bare
  typedefs (mrustc/rustc_public) lose this safety.

**Prior-art evidence.**
- rustc: `newtype_index!` Local with `const RETURN_PLACE = 0`
  at `mir/mod.rs:842-850`. Const-body grammar landed at
  `rustc_index_macros/src/newtype.rs:87`. See
  `prior-art/rustc.md:223` §4 (with §4.1-4.2 sub-sections
  at lines 256, 272).
- rustc_public: bare `pub type Local = usize` with
  `RETURN_LOCAL = 0` at `body.rs:850`; weaker discipline.
  See `prior-art/rustc_public.md`.
- Charon: `generate_index_type!` macro at `src/ids/mod.rs`;
  ZERO universal constant + `MAX_INDEX = u32::MAX` niche.
  See `prior-art/charon.md`.
- mrustc: typedef-asymmetry (`BasicBlockId` typed at
  `mir.hpp:20`, locals raw `unsigned int`); D-11-4 closes
  this asymmetry. See `prior-art/mrustc.md`.

**Worked example.**
```rust
// crates/regseq-model/src/idx.rs
newtype_index! {
    pub struct LocalId {
        const RETURN_PLACE = 0;
    }
}

// crates/regseq-model/src/lib.rs (assertions appended)
const _: () = assert!(size_of::<LocalId>() == 4);
const _: () = assert!(size_of::<Option<LocalId>>() == 4);
```

**Spec cross-reference.** §25.3.

**Sub-decisions.**
- **D-11-4.1** `LocalId` introduced via `newtype_index!`
  in `crates/regseq-model/src/idx.rs`. Mechanical inheritance
  from stage-1 ID conventions + stage-10 D-10-3.1 BlockId
  parallel. Closes stage-1 inbound "Define LocalId via
  newtype_index!". [LOCKED]
- **D-11-4.2** `const RETURN_PLACE = 0` declared inline
  using stage-10 D-10-3.2's const-body macro arm. Closes
  stage-10 D-10-3.2's deferred migration. Closes stage-1's
  RETURN_PLACE convention framing. [LOCKED]
- **D-11-4.3** `size_of::<LocalId>() == 4` and
  `size_of::<Option<LocalId>>() == 4` static assertions
  appended to `crates/regseq-model/src/lib.rs` alongside
  the stage-10 BlockId assertions. Closes stage-1 inbound
  niche optimization. [LOCKED]
- **D-11-4.4** Sigil textual convention `LocalId(N) -> _N`
  pinned at stage 11; rendering site **deferred to stage
  13 per D-11-K**. [LOCKED]

---

### D-11-5 -- `BodyId` vs `ItemId` resolution [LOCKED]

**Question.** Does RegseqModel allocate a separate `BodyId`
for bodies, or do bodies live as a structural field of Item
without a distinct ID? Stage-1 inbound called this an open
question explicitly.

**Options considered.**
- (a) `BodyId == ItemId` numerically (one ID space; same
  numeric value indexes both).
- (b) Distinct `BodyId`; bodies hold back-link to ItemId.
- (c) No `BodyId`; `Item.body: Option<Body>` structural
  field.

**Decision.** (c) no BodyId. Bodies live as a structural
field of Item per spec-snapshot.md §25.5.

**Reasoning.**
- 4-peer convergence: rustc has no BodyId (`tcx.instance_mir(InstanceKind)`
  is the only fetch path); rustc_public has no BodyId
  (`Instance::body() -> Option<Body>` at `mono.rs:53-55`);
  Charon has no BodyId (`FunDecl.body: Body` direct
  structural field); mrustc HIR-layer has no BodyId.
- Composes with Q3 (single-level IR; bodies are content of
  items, not peers).
- Composes with Q2 (eager translation, fully-owned IR;
  `Option<Body>` is owned content, not a query).
- Composes with D-11-3 alt (e) (FnSig on Item-side);
  signature + body co-located on the same Item creates a
  coherent function representation.
- Saves an ID space + lookup table.

**Methodology rules invoked.**
- *Re-derive from evidence; do not import baggage* -- no
  peer allocates BodyId; the stage-1 open question resolves
  per evidence.
- *Compositional behavior beats special-case flags* -- no
  parallel BodyId table; bodies live structurally.

**Prior-art evidence.**
- rustc: no BodyId; `tcx.instance_mir(InstanceKind)` at
  `ty/mod.rs:1671`. See `prior-art/rustc.md:290` §5 (with
  §5.1-5.2 sub-sections at lines 308, 316).
- rustc_public: `Instance::body() -> Option<Body>` at
  `mono.rs:53-55` returning owned Body. Same shape used by
  `ClosureDef::body()` and `CoroutineDef::body()`. See
  `prior-art/rustc_public.md`.
- Charon: `FunDecl.body: Body` direct structural field at
  `gast.rs` FunDecl. See `prior-art/charon.md`.
- mrustc: HIR-layer `Function::m_code: ExprPtr` (different
  architecture but same effect: no separate BodyId). See
  `prior-art/mrustc.md`.

**Spec cross-reference.** §25.5.

**Sub-decisions.**
- **D-11-5.1** No `BodyId` allocation; no separate ID space
  for bodies. 4-peer convergence. [LOCKED]
- **D-11-5.2** Bodies live as a structural field of `Item`
  per (c); stage-12 finalizes the precise placement
  (`Item.body` top-level vs `ItemKind::Function.body`
  variant-internal). [LOCKED]
- **D-11-5.3** `Option<Body>` accommodates: foreign
  declarations (`extern "C" fn` with no body) -> `body =
  None`; non-Function ItemKinds (Struct, Enum, Union, Static,
  Const, vtable type Items) -> body variant-specific.
  Stage-11 pins `Option<Body>` as the wrapper; stage-12 picks
  the structural placement. [LOCKED]
- **D-11-5.4** Body-fetch convention: walkers reach Body
  via Item; no reverse lookup. Composes with D-11-1.7 zero-
  shape Body↔Item linkage. [LOCKED]
- **D-11-5.5** Outbound expectation to `stage-12/expectations.md`
  "From stage 11 (per D-11-5)": stage 12 absorbs Body as
  structural component of Item per (c). Joins the D-11-3
  outbound (FnSig on Item) into the broader stage-12
  Item.kind=Function expansion. [LOCKED]

---

### D-11-6 -- Joint validation walk shape and ownership [LOCKED]

**Question.** Where does the body-level validation walk
live, and what is its visitor-trait shape? 5 deferred
walk-implementations (D-5-2, D-7-3, D-8-4 Part C, D-9-10
Part C, D-10-5 Part C) converge on stage 11 plus the
construction invariants 1+9 fold-in.

**Options considered.**
- (a) Direct-loop walk in a `validate(model)` function
  (mrustc's `MIR_Validate` pattern at `check.cpp:445`).
- (b) Visitor trait with override hooks (rustc_public's
  `MirVisitor` + Charon's `VisitAst`/`VisitBody` bi-modal
  shape).

**Decision.** (b) visitor with override hooks. Located in
`crates/regseq-model/src/visit.rs` per stage-10 model-vs-
analysis line. 10 `visit_X` / `super_X` hook pairs covering
body, basic_block, local_decl, statement, terminator, place,
rvalue, operand, ty, call_callee.

**Reasoning.**
- 3-peer positive (rustc, rustc_public, Charon) + 1 negative
  (mrustc); convergent on visitor-with-override-hooks.
- mrustc's direct-loop walk forfeits visitor composability;
  every new check requires re-implementing traversal logic.
  Negative precedent.
- rustc_public's `make_mir_visitor!` macro at `visit.rs:42-517`
  is the load-bearing template (17 visit/super pairs;
  exhaustive-destructuring discipline). RegseqModel uses the
  same template adapted to its smaller surface.
- Validation enforces invariants the bridge claims to
  uphold — that's a model concern (live with `RegseqModel::validate`),
  not an analysis. Per stage-10 model-vs-analysis line:
  the IR knows how to validate itself; CFG analyses
  (predecessors, dominators, RPO iteration) live in
  `regseq-analysis`.
- All 5 deferred walks fold cleanly into the visitor's
  hooks (D-5-2 → visit_place; D-7-3 → visit_rvalue;
  D-8-4 → visit_statement; D-9-10 → visit_terminator;
  D-10-5 §24.4.3 → visit_body pre-check + traversal).
- Read-only `Visitor` trait at stage 11; `MutVisitor` for
  transformation passes (per D-11-7) is a phase-1 impl
  extension — those passes do direct mutation rather than
  generic visitor (mrustc's `MIR_Cleanup_Virtualize` at
  `cleanup.cpp:548-684` precedents direct mutation).

**Methodology rules invoked.**
- *Compiler-theory-first* -- visitor pattern is the standard
  compiler-IR walk; rich literature on traversal patterns.
- *Compositional behavior beats special-case flags* -- 10
  hook pairs compose into one walk; alternative (multiple
  passes) duplicates traversal cost.
- *Re-derive from evidence; do not import baggage* -- hook
  surface is RegseqModel-specific (10 vs rustc_public's 17),
  not lifted wholesale.

**Prior-art evidence.**
- rustc: `make_mir_visitor!` macro at `mir/visit.rs:1-160`;
  rich surface with PlaceContext / MutatingUseContext.
  See `prior-art/rustc.md:332` §6 (with §6.1-6.6 sub-sections
  covering codegen-SSA walk / cranelift walk /
  BuilderMethods abstraction / make_mir_visitor! template /
  pre-walk pattern / RegseqModel implications).
- rustc_public: `MirVisitor` + `MutMirVisitor` via
  `make_mir_visitor!` at `visit.rs:42-517`. 17 visit/super
  hook pairs; exhaustive-destructuring discipline. See
  `prior-art/rustc_public.md:385` §5 (with §5.1-5.5
  sub-sections at lines 393, 427, 460, 502, 531).
- Charon: bi-modal `VisitAst` (full crate) + `VisitBody`
  (body-only) at `src/ast/visitor.rs:130-191`. See
  `prior-art/charon.md:408` §6.
- mrustc: direct-loop in `MIR_Validate` at
  `check.cpp:445` (negative precedent). See
  `prior-art/mrustc.md:341` §6.

**Worked example.**
```rust
// crates/regseq-model/src/visit.rs (sketch)
pub trait Visitor {
    fn visit_body(&mut self, body: &Body) { self.super_body(body) }
    fn super_body(&mut self, body: &Body) {
        // pre-checks: B-NonEmpty, B-Successors-Valid (§25.6.2)
        // DFS-RPO traversal: B-RPO + B-Reachable + B-Entry
        for decl in &body.local_decls { self.visit_local_decl(decl); }
        for bb in &body.basic_blocks { self.visit_basic_block(bb); }
    }
    // ... 9 more visit_X / super_X hook pairs ...
}
```

**Spec cross-reference.** §25.6 (visitor trait + 5 deferred
walk absorption + body-level §24.4.3 procedure).

**Sub-decisions.**
- **D-11-6.1** Construction invariants 1 (TyId closure) and
  9 (Direct(InstanceId) closure) fold into existing visitor
  hooks (visit_local_decl + visit_ty for inv 1;
  visit_terminator::Call for inv 9); no separate pre-pass.
  [LOCKED]
- **D-11-6.2** Visitor trait location:
  `crates/regseq-model/src/visit.rs`. Per Q1 + stage-10
  model-vs-analysis line. [LOCKED]
- **D-11-6.3** Hook surface: 10 visit/super pairs (body,
  basic_block, local_decl, statement, terminator, place,
  rvalue, operand, ty, call_callee). Subset of rustc_public's
  17 — RegseqModel doesn't carry SwitchTargets / VarDebugInfo
  / per-renderer hooks because rendering is stage-13's per
  D-11-K and var_debug_info is consumed at construction.
  [LOCKED]
- **D-11-6.4** Mutability discipline: read-only `Visitor`
  trait at stage 11. `MutVisitor` for transformation passes
  is phase-1 impl extension; not specified at stage 11
  (passes per D-11-7 do direct mutation, not generic
  visitor). [LOCKED]
- **D-11-6.5** Body-level walk procedure (§24.4.3
  absorption): `visit_body` implements 6-step DFS-RPO with
  B-NonEmpty + B-Successors-Valid pre-checks GUARDING
  subsequent indexing; B-Reachable + B-RPO assertions in
  steps 4-5; B-Entry implicit from start point + step-1
  pre-check. [LOCKED]
- **D-11-6.6** Walk-implementation absorption summary: 5
  deferred walks (D-5-2 / D-7-3 / D-8-4 Part C / D-9-10
  Part C / D-10-5 Part C) all close at stage-11 joint
  visitor; per-construct invariants from each owning stage's
  spec sections (§19.7, §20.5, §21.5, §22.5, §23.6, §24.4.3)
  enforced at the appropriate hooks. [LOCKED]

---

### D-11-7 -- Body-level transformation pass timing [LOCKED]

**Question.** When do body-level transformation passes
run — at bridge construction time, as post-construction
passes, or hybrid? Two passes are in scope: Drop-as-Call
collapse (per stage-9 D-9-2 + §23.4) and vtable-indirect
rewrite (per stage-3 D5 + stage-9 D-9-3 (b)).

**Options considered.**
- (a) Both at bridge construction time (atomic per-rustc-
  terminator translation).
- (b) Both post-construction passes (Charon's
  `desugar_drops` + `transform_dyn_trait_calls`).
- (c) Hybrid: Drop-as-Call at bridge time; vtable-rewrite
  post-construction.

**Decision.** (c) hybrid timing per spec-snapshot.md §25.7.

**Reasoning.**
- **Drop-as-Call at bridge time is FORCED** by D-9-1.1's
  rejection of the `Drop` variant from `TerminatorKind`.
  The bridge MUST translate `mir::TerminatorKind::Drop` —
  there is no Drop variant in RegseqModel's vocabulary to
  defer. Bridge-time is the only valid timing.
- **Vtable-rewrite at post-construction is structurally
  indicated.** Bridge-time rewrite would couple stage-12
  vtable Item synthesis with bridge body construction
  (vtable types must be Item-collected before the rewrite
  can reference them). Post-construction matches Q2 (eager
  translation produces fully-owned IR; passes mutate
  fully-owned IR) cleanly.
- rustc's codegen-time direct dispatch (via
  `Instance::Virtual` + `tcx.vtable_entries`) is structurally
  inadmissible for C-emission under Q2 (no `'tcx`
  post-bridge; codegen has no `tcx` access).
- Charon + mrustc both run vtable-rewrite post-construction.
  Convergent precedent.

**Cross-stage friction documentation (D-11-7.6).**
Charon's compact in-place projection-chain rewrite at line
119 of `transform_dyn_trait_calls.rs` does:
```
call.func = FnOperand::Move(method_field_place)
```
where `method_field_place = (*receiver.ptr_metadata).method`
uses a `PtrMetadata` PlaceElem variant in Charon's
projection vocabulary.

**Stage-5 D-5-1 locked a 6-variant `PlaceElem` set: Deref /
Field / Index / ConstantIndex / Subslice / Downcast. NO
`PtrMetadata` variant.**

Without `PtrMetadata` as a PlaceElem, RegseqModel cannot
inline the metadata extraction into a Place projection
chain. The metadata MUST be extracted as a separate
`Rvalue::UnaryOp(UnOp::PtrMetadata, ...)` Statement. The
chain expands to 4-5 fresh locals (`_t_recv`, `_t_meta`,
`_t_fn`, `_t_data`, optionally `_t_raw` for Ref source-
normalization) vs Charon's 0-fresh-locals form.

The verbosity is **forced by stage-5's locked vocabulary**,
not a stage-11 design choice. Reopening D-5-1 to add
`PtrMetadata` is out of stage-11 scope (would cascade
through stage-5's projection-typing rules + walk + invariants
+ §19.4 type-narrowing). `UnOp::PtrMetadata` IS in stage-7
D-7-1.6's locked `UnaryOp` enum — that's how the metadata
extraction happens via Rvalue rather than via Place.

**D-9-3 reopening question (mid-Phase-A).** During Phase A
discussion, the question was raised: should stage-9 D-9-3 be
reopened to alt (a) (keep Direct(InstanceId) for virtual;
codegen-side dispatch on `flavor: Virtual` via existing
`virtual_idx` payload)? Alt (a) would have removed the
stage-11 vtable-rewrite pass entirely.

Conclusion: **don't reopen, proceed with hybrid (c)**:
- The cross-stage friction with stage-5 D-5-1 (no
  `PtrMetadata` PlaceElem) forces multi-statement rewrite
  shape regardless of which stage owns the rewrite — not a
  stage-9-specific cost.
- Stage-9's IR-uniformity preference (one Call code path
  post-rewrite) is reasonable; D-9-3 trade-off was
  IR-uniformity-vs-codegen-simplicity, and stage-9 picked
  IR-uniformity with 12 rounds of Codex review pedigree.
- Reopening cost (Phase A re-do for stage-9, merger
  contract re-open, ~10-15 stage-9 findings re-confirm) is
  unfavorable for a stylistic preference shift.
- Stage-11 absorbs the rewrite-pass cost; D-11-7 hybrid (c)
  timing locks.

**Pass scheduling (D-11-7.5).** Vtable-rewrite runs
**before** the joint validation walk. Stage-9 §23.6.2
B-3/B-4/B-6 invariants assume post-rewrite shape;
running validation pre-rewrite would emit spurious
B-3/B-4/B-6 errors at every Virtual-flavor Call site
(pre-rewrite, args[0] type does not match
`callee_fn_sig(callee_id).inputs()[0]` for Virtual calls;
the helper resolves the signature Item-side per
§23.6.2's stage-11 reframe). Stage-9 expectations.md
explicitly says: "Stage-11 sequencing: rewrite first,
validate after."

**Methodology rules invoked.**
- *Compiler-theory-first* -- post-construction transformation
  passes are standard compiler practice; the timing decision
  derives from coupling-avoidance (Q2 + stage-12 Item
  synthesis dependency).
- *No defensive fallbacks* -- rustc-style codegen-time
  direct dispatch is structurally inadmissible for our
  targets, not a fallback to consider.
- *Compositional behavior beats special-case flags* --
  rewrite produces post-rewrite invariants compositionally;
  no parallel "is post-rewrite" flag.
- *C-semantics with information preservation* -- post-rewrite
  IR carries the projection-chain explicitly; the synthesis
  is C-shape (data + function-pointer pairs) not Rust-shape.
- *Spec is mutable (rejected as too costly here)* --
  reopening stage-9 D-9-3 is allowed by methodology but
  cost-benefit is unfavorable.

**Prior-art evidence.**
- rustc: deliberately does NOT rewrite — codegen-time
  direct dispatch via `Instance::Virtual` +
  `meth::VirtualIndex::from_index(idx).get_fn(...)` reading
  `tcx.vtable_entries`. Structurally inadmissible for our
  C-emission target. See `prior-art/rustc.md:399` §7
  (with §7.1 sub-section at line 403 covering the
  codegen-SSA Virtual-call path).
- rustc_public: silent (no transformation passes). See
  `prior-art/rustc_public.md`.
- Charon: `desugar_drops` post-construction pass
  (`desugar_drops.rs:23-72`); `transform_dyn_trait_calls`
  post-construction pass (`transform_dyn_trait_calls.rs:31-122`)
  registered in INITIAL_CLEANUP_PASSES at `mod.rs:104, 111`.
  In-place projection-chain rewrite at line 119; load-bearing
  precedent for D-11-7 with the cross-stage friction note.
  See `prior-art/charon.md`.
- mrustc: HIR-level Drop handling (different architecture);
  `MIR_Cleanup_Virtualize` at `cleanup.cpp:548-684`
  post-construction post-monomorphization timing; single-
  field `Call.fcn` rewrite at line 685 matching
  `CallCallee::Indirect(Operand)`. Load-bearing precedent.
  See `prior-art/mrustc.md`.

**Spec cross-reference.** §25.7 (Drop-as-Call collapse rules
+ vtable-rewrite procedure with cross-stage friction note +
pass scheduling).

**Sub-decisions.**
- **D-11-7.1 [master]** Hybrid (c) timing: Drop-as-Call at
  bridge construction time + vtable-indirect rewrite as
  post-construction pass. [LOCKED]
- **D-11-7.2** Drop-as-Call at bridge time. Forced by
  D-9-1.1. Allocates `_t_drop_arg` (`*mut <place_ty>`) and
  `_t_drop_ret` (`()`) per Drop site via D-11-8 API. Inserts
  `Statement::Assign(_t_drop_arg, Rvalue::RawPtr(Mut, place))`
  appended to current basic block's statements list. Per
  stage-9 §23.4 step 4. [LOCKED]
- **D-11-7.3** Vtable-indirect rewrite as post-construction
  pass. Charon `transform_dyn_trait_calls` + mrustc
  `MIR_Cleanup_Virtualize` peer precedents. Avoids coupling
  stage-12 Item synthesis with bridge body construction.
  [LOCKED]
- **D-11-7.4** Multi-statement rewrite procedure follows
  stage-9 expectations §23.3 (5 fresh locals incl. source-
  normalization for Ref receivers; explicit Statement-level
  extraction sites). Stage-11 spec references the procedure
  rather than duplicating it. [LOCKED]
- **D-11-7.5** Pass scheduling: vtable-rewrite precedes
  joint validation walk (D-11-6); §23.6.2 B-3/B-4/B-6
  invariants assume post-rewrite shape. [LOCKED]
- **D-11-7.6** Cross-stage friction with stage-5 D-5-1:
  spec records the forcing relationship (no `PtrMetadata`
  PlaceElem variant ⇒ multi-statement shape is the only
  legal expression). Reopening D-5-1 is out of stage-11
  scope. [LOCKED]
- **D-11-7.7** Stage-12 dependency notice: vtable-rewrite
  consumes Items synthesized by stage 12 (vtable type Items
  per `(Trait, projection_bindings)`; vtable instance Items
  per `(Trait, SelfTy)`; vtable shim Function Items). Stage
  12 owns synthesis; stage 11 confirms consumption. Phase-1
  implementation sequencing requires stage-12 synthesis to
  land before stage-11 rewrite executes. [LOCKED]

---

### D-11-8 -- LocalDecls allocation API [LOCKED]

**Question.** What is the API for allocating fresh
LocalDecls during bridge construction (Drop-as-Call) and
post-construction passes (vtable-rewrite)?

**Options considered.**
- (a) Single method on Body: `Body::push_local(decl) -> LocalId`.
  Both consumers use the same method.
- (b) Builder pattern: `BodyBuilder::push_local` for
  construction; direct `body.local_decls.push` for passes.
- (c) Split APIs (mrustc-style): `BodyBuilder::push_local`
  + `BodyMutator::push_local` with pending-statements
  buffer.

**Decision.** (a) single method on Body + convenience
helper for synthetic locals. Per spec-snapshot.md §25.8.

**Reasoning.**
- (c) split with buffering is not needed. mrustc's
  `MirMutator` buffer exists because their rewrite walks
  DURING basic-block iteration (mid-iteration mutation
  safety). RegseqModel's rewrite walks `basic_blocks_iter()`;
  for each block, builds the new statements + new Call
  atomically, then replaces `block.statements` (extending)
  + `block.terminator` as a unit. Single-block-rewrite-as-
  unit pattern. No buffer needed. Drop-as-Call at bridge
  time is similarly atomic per rustc terminator.
- (b) Builder is unnecessary intermediate. Post-construction
  passes need to mutate Body directly (insert statements,
  replace terminator), so a Builder→Body pipeline doesn't
  simplify.
- (a) is the minimum sufficient API. Same method, two
  consumers (bridge + passes). Charon's layered design is
  overkill for our locked LocalDecl shape (4 fields per
  D-11-2; no rich helpers needed).
- Convenience helper `push_synthetic_local(ty)` defaults
  `(Mutability::Mut, Span::dummy(), debug_name: None)`
  for bridge-introduced + rewrite-pass-introduced locals.

**Methodology rules invoked.**
- *Compositional behavior beats special-case flags* -- one
  API method composed with the convenience helper for
  synthetic locals; no parallel "is_synthetic" allocation
  channel.
- *No defensive fallbacks* -- mrustc's buffering pattern
  exists for a problem we don't have; we don't add it
  defensively.
- *C-semantics with information preservation* -- helper
  preserves the synthetic-local profile uniformly.

**Prior-art evidence.**
- rustc: pub-field-push discipline (no encapsulating
  method); `LocalDecl::new(ty, span)` defaults to Mut +
  `LocalInfo::Boring` + outermost-span; direct
  `body.local_decls.push(...)`. See `prior-art/rustc.md`
  D-11-8 section (the rustc.md prior-art file is organized
  by D-tag; the LocalDecls allocation discussion follows
  the §6 visitor section at line 332+).
- rustc_public: `Body::new` consumes complete `Vec`
  collections; **no incremental push API**; `pub(super)`
  privacy enforces entry via `new`. Construct-once shape;
  doesn't accommodate post-construction passes. See
  `prior-art/rustc_public.md`.
- Charon: layered — leaf `Locals::new_var(name, ty) ->
  Place` + higher-level `BodyTransformCtx::fresh_var` /
  `borrow_to_new_var` / `raw_borrow_to_new_var` trait
  helpers. Auto-`StorageLive`-on-fresh-var (does NOT
  transfer per D-8-1). See `prior-art/charon.md`.
- mrustc: split APIs — `MirBuilder::new_temporary`
  (`mir_builder.cpp:232`) + `MirMutator::new_temporary`
  (`cleanup.cpp:25-100`) with pending-statements buffering.
  See `prior-art/mrustc.md`.

**Worked example.**
```rust
// Bridge Drop-as-Call collapse
let _t_drop_arg = body.push_synthetic_local(ptr_to_place_ty);
let _t_drop_ret = body.push_synthetic_local(unit_ty);
// ... emit Statement + synthesized Call

// Vtable-rewrite pass
let _t_recv = body.push_synthetic_local(args[0].ty(...));
let _t_meta = body.push_synthetic_local(ty_ptr_to_vtable);
let _t_fn   = body.push_synthetic_local(fn_ptr_ty);
let _t_data = body.push_synthetic_local(ty_ptr_to_unit);
// ... emit projection-chain Statements + rewrite Call
```

**Spec cross-reference.** §25.8.

**Sub-decisions.**
- **D-11-8.1 [master]** Single allocation API on Body (alt
  a). Both bridge and post-construction passes consume the
  same method. [LOCKED]
- **D-11-8.2** Method shape: `Body::push_local(&mut self,
  decl: LocalDecl) -> LocalId`. Returns the freshly-
  allocated LocalId (positional). [LOCKED]
- **D-11-8.3** Convenience helper: `Body::push_synthetic_local(&mut self,
  ty: TyId) -> LocalId`. Defaults `debug_name = None`,
  `mutability = Mut`, `span = Span::dummy()`. For bridge-
  introduced + rewrite-pass-introduced locals. [LOCKED]
- **D-11-8.4** No BodyBuilder pattern; Body is mutable
  directly during both bridge construction and post-
  construction passes. Composes with D-11-7 hybrid timing.
  [LOCKED]
- **D-11-8.5** No buffering pattern; single-block-rewrite-
  as-unit traversal makes mid-iteration mutation a non-
  issue. Charon's auto-`StorageLive`-on-fresh-var pattern
  explicitly does NOT transfer per D-8-1. [LOCKED]
- **D-11-8.6** Consumer enumeration: bridge per-rustc-local
  population (`push_local` with full LocalDecl); bridge
  Drop-as-Call collapse (2× `push_synthetic_local` per Drop
  site); vtable-rewrite pass (4-5× `push_synthetic_local`
  per virtual-call site). [LOCKED]

---

### D-11-K -- Categorical rendering deferral to stage 13 [LOCKED]

**Question.** Where does rendering of body-level constructs
land — at stage 11 (per the per-stage K-tag pattern from
D-5-K through D-10-K), or at stage 13 (categorically, as a
coherent rendering-decision surface)?

**Options considered.**
- (a) Per-renderer retargeting at stage 11: each predecessor
  stage's K-tag (D-5-K through D-10-K + stage-1 _N sigil +
  stage-10 bbN sigil) lands at stage 11 individually.
- (b) Categorical retargeting to stage 13: single master
  decision moves all body-level rendering ownership to stage
  13 alongside `render_ty` (D14 deferred from stage 3).

**Decision.** (b) categorical retargeting to stage 13. Per
spec-snapshot.md §18.2 + the §19.8 / §20.6 / §21.8 / §22.8
/ §23.9 / §24.9 in-place updates.

**Reasoning.**
- Stage 13 is the project's pretty-printing-decisions stage.
  Renderer style, sigil conventions, and per-method ownership
  form a coherent surface that benefits from a single stage
  owning them.
- Stage-11's deferrals (inherited from D-5-K through D-10-K
  + stage-1 _N + stage-10 bbN) were originally targeted there
  because Body materialization is a renderer dependency —
  but stage 13 also depends on Body (via signature rendering,
  body-content rendering subtree) plus type-system rendering
  (`render_ty` from D14). All renderers landing together at
  stage 13 produces a coherent rendering decision; stage 11
  doesn't have to commit to a rendering style for body-content
  while stage 13 separately commits to a rendering style for
  types — the styles are decided as a unit.
- Stage 11 retains all structural spec ownership (Body /
  LocalDecl / FnSig location / LocalId definition / walks /
  passes). Only RENDERING moves; structural ownership is
  unchanged.
- Single master decision over per-renderer retargeting
  produces a cleaner audit-shape entry + cleaner merger-
  contract bookkeeping.

**Methodology rules invoked.**
- *Spec is mutable* -- categorical retargeting is the kind
  of cross-stage scope decision the methodology accommodates.
- *Compositional behavior beats special-case flags* -- single
  master decision composes; per-renderer retargeting would
  duplicate the rationale 8 times.
- *Compiler-theory-first* -- coherent rendering-decision
  surface is sound design; rendering style affects
  readability uniformly across IR constructs.

**Prior-art evidence.**
- rustc: scattered Debug/Display impls per type with
  `#[debug_format = "_{}"]` / `#[debug_format = "bb{}"]`
  attributes on newtype indexes; the macro-emitted Debug
  impl is the natural rendering spot. See
  `prior-art/rustc.md:537` §9.
- rustc_public: in-crate pretty-printer (`pretty.rs`); no
  deferral pattern. See `prior-art/rustc_public.md:681`
  §8.
- Charon: co-locates renderer with IR (`fmt_with_ctx.rs`).
  See `prior-art/charon.md:719` §9.
  See `prior-art/charon.md`.
- mrustc: interleaves rendering with construction (opposite
  philosophy). See `prior-art/mrustc.md`.

(Note: rendering-decision-stage as a distinct project stage
is RegseqModel-specific; no peer has the same staging
discipline.)

**Spec cross-reference.** §18.2 (per-method ownership table
reframed); §19.8 / §20.6 / §21.8 / §22.8 / §23.9 / §24.9
(per-stage renderer deferral notes flipped to stage 13 with
attribution to predecessor D-X-K).

**Sub-decisions.**
- **D-11-K.1 [master]** Categorical rendering deferral: all
  body-level renderers + `_N` and `bbN` sigil rendering
  wirings move to stage 13. Single decision, not per-renderer
  retargeting. [LOCKED]
- **D-11-K.2** Per-renderer enumeration (the items moving):
  9 public renderers (`render_place` originally D-5-K,
  `render_operand` D-6-K, `render_rvalue` D-7-K,
  `render_statement` D-8-K, `render_terminator` +
  `render_call_callee` + `render_switch_targets` +
  `render_assert_kind` D-9-K, `render_basic_block` D-10-K),
  plus 1 internal helper (`render_terminator_kind`), plus
  2 sigil rendering wirings (`_N` for `LocalId` from
  stage-1 inbound, `bbN` for `BlockId` from D-10-K.2) =
  **12 items total**. Stage-11 spec attribution to
  predecessor stages preserved. [LOCKED]
- **D-11-K.3** Operational consequence at stage-11 merger
  contract close (per README §5.5 step 4): D-5-K, D-6-K,
  D-7-K, D-8-K, D-9-K, D-10-K all flip to `[REOPENED]` in
  top-level `decisions.md` with annotation `"(retargeted to
  stage 13 per D-11-K)"`. Cross-stage bookkeeping: rendering
  items move out of `stage-11/expectations.md` into a new
  `stage-13/expectations.md` "From stage 11 (per D-11-K)"
  bundle preserving attribution. [LOCKED]
- **D-11-K.4** §18.2 per-method ownership table treatment:
  body-level renderer rows are RETARGETED in place from
  "stage 11 (deferred)" to "stage 13 (per D-11-K)" at the
  stage-11 merger contract. Rows remain in §18.2 with the
  new ownership; stage 13 will refine them further (e.g.,
  expanded per-renderer detail) when it lands. Stage-11
  spec-snapshot.md §18.2 has been pre-emptively updated
  to reflect the retargeted state with attribution to
  predecessor D-X-K decisions inheriting their dependency
  analysis. [LOCKED]
- **D-11-K.5** Stage-11 retains structural spec ownership:
  validation walks (D-11-6), transformation passes (D-11-7),
  construction-invariant fold-ins (D-11-6.1), data structures
  (D-11-1 through D-11-5 + D-11-8) all stay at stage 11. This
  is a `RENDERING-only` retargeting; structural ownership
  unchanged. [LOCKED]
- **D-11-K.6** Cross-references: stage-11 spec-snapshot.md
  notes the categorical deferral at every site that would
  have referenced a body-level renderer (joint visitor's
  `visit_X` hooks don't render — rendering is stage-13's
  `render_X`). Stage-13 expectations.md inbound bundle (filed
  at Phase B close) preserves attribution to predecessor
  stages so stage 13 understands the renderer dependency
  graph (Cast arm late-binds to render_ty; Aggregate
  field/variant names resolve via §11 layout vocabulary;
  etc.) inherited from each originating stage's audit shape.
  [LOCKED]

---

## 2. Phase A discussion threads (substantive)

### Stage-1 inbound reframe (D-11-3)

The user's hands-on engagement with D-11-3 ratified the
4-peer-evidence-driven reframe. Stage-1 inbound's framing
"FnSig as a structural field of Body" was a default reading
(stage-1 didn't pin location explicitly; only schema +
abi-as-structural). The 4-peer evidence (rustc, rustc_public,
Charon, mrustc) converged on Item-side placement. Stage-11
locks alt (e) per "Spec is mutable" methodology; stage-1's
schema framing carries through.

Recorded in spec-snapshot.md §14 (placement note) + §25.4
(reframe documentation).

### D-9-3 reopening question (D-11-7)

Mid-Phase-A, the question was raised whether stage-9 D-9-3
should be reopened to alt (a) (keep `Direct(InstanceId)` for
virtual; codegen-side dispatch on `flavor: Virtual` via
existing `virtual_idx` payload) to remove the stage-11
vtable-rewrite pass.

The cost-benefit analysis covered:
- Reopening cost: stage-9 Phase A re-do, merger contract
  re-open, ~10-15 stage-9 findings re-confirm
- Stage-11 rewrite cost: 4-5 fresh locals per virtual-call
  site, multi-statement projection chain, pass-scheduling
  complexity
- Cross-stage friction with stage-5 D-5-1 forces multi-
  statement shape regardless of which stage owns the rewrite
- Stage-9's IR-uniformity preference (one Call code path)
  is reasonable

User decision: **don't reopen, proceed with hybrid (c)**.
Recorded in this discussions.md D-11-7 entry's reasoning
section.

### Categorical D-11-K reframe scope (cross-cutting)

The user introduced the categorical D-11-K decision at
stage-11 phase 0 ("I want to defer those to stage 13 since
stage 13 is particularly responsible for deciding on how
to pretty print or render IR constructs"). This pre-decision
shaped stage-11's entire scope (rendering moves; structural
spec stays). D-11-K formalizes the user's pre-decision as
a master decision with full audit shape.

The cross-stage operational bookkeeping (move rendering
items out of stage-11/expectations.md into a new
stage-13/expectations.md "From stage 11 (per D-11-K)"
bundle) is performed at stage-11 merger contract close per
the §5.5 7-step protocol; tracked as task #5 in the project's
task list. The §18.2 per-method ownership table updates have
been pre-emptively applied to stage-11/spec-snapshot.md so
the categorical retargeting is visible during Phase B Codex
review.

---

## 3. Cross-references

- Top-level `../README.md` -- methodology rules referenced
  in each entry's "Methodology rules invoked" section.
- Top-level `../status.md` -- stage progression.
- `memory.md` -- stage-11 invariants, Codex prompt template
  in §4.
- `status.md` -- D-tag allocation table (mirrors this file's
  status flags); Phase A close inventory; Phase B work plan
  in §5.
- `expectations.md` -- inbound forward refs from stages 1, 3,
  5, 6, 7, 8, 9, 10. Each entry's "Spec cross-reference"
  section indicates which inbound items the D-tag closes.
- `spec-snapshot.md` §25 -- new Body-level vocabulary
  section (containing §25.1 through §25.11).
- `spec-snapshot.md` §14 -- in-place clarification on FnSig
  placement (Item-side per D-11-3).
- `spec-snapshot.md` §18.2 -- in-place reframing of
  per-method ownership table per D-11-K's categorical
  retargeting.
- `spec-snapshot.md` §19.8 / §20.6 / §21.8 / §22.8 / §23.9 /
  §24.9 -- in-place per-stage renderer-deferral note
  flips to stage 13 per D-11-K.
- `prior-art/{rustc,rustc_public,charon,mrustc}.md` --
  primary-source evidence cited per entry.
- `review-log.md` -- Phase B per-round Codex audit (created
  at first review round).

---

## 4. Outbound expectations to file at merger contract

To file in `stage-12/expectations.md` "From stage 11"
(per D-11-3 + D-11-5):
- FnSig migration to `Item.kind = Function { fn_sig, ... }`
  per D-11-3. The canonical FnSig schema is the pinned
  5-field form per §14.3 (`inputs`, `output`, `abi`,
  `c_variadic`, `safety`); D-11-3 reframes the placement
  (Item-side instead of Body-side) without revising
  §14's schema. Stage 12 absorbs the §14 schema as a
  structural field of `Item.kind = Function`.
- `Body` materialisation under Item per D-11-5 (schema
  placement: top-level `Item.body: Option<Body>` vs
  variant-internal `ItemKind::Function.body`; stage-12
  picks).
- Construction invariant 9 closure (D-9-10 B-7); stage-11
  visitor checks `Direct(InstanceId)` membership in
  `model.instances`; stage-12 owns `model.instances` integrity.

To file in `stage-13/expectations.md` "From stage 11 (per
D-11-K)":
- 12-item rendering bundle (9 public renderers + 1 internal
  helper + 2 sigil wirings per D-11-K.2) with attribution
  to predecessor
  stages (D-5-K through D-10-K originating; D-11-K
  retargets).
- §18.2 per-method ownership table consolidation: stage 13
  owns all rendering ownership rows including existing
  `render_ty` (D14) plus the 9 public body-content
  renderer rows + 1 internal helper (per D-11-K.2's
  12-item enumeration) + 2 sigil wirings.

Filing happens at stage-11 merger contract step 7 (per
README §5.5 7-step protocol), after per-stage Codex
review APPROVES.

## Stage 12 (Rust Items)

  forced a revision; superseded entry kept for history.
- `[DEFERRED to stage N]` -- moved to a later stage's scope.

Phase A closed 2026-05-06 with all 7 top-level D-tags +
29 sub-decisions LOCKED across a single combined cluster.

---

## 1. Decision discussions

### D-12-1 -- Item schema + ItemKind shape [LOCKED]

**Question.** Stage-1 deferred the full Item schema to
stage 12. Stage-11 D-11-3 + D-11-5 anticipated Item-side
absorption of `fn_sig: FnSigId` and `body: Option<Body>`
under the `Function` ItemKind variant. Stage 12 must lock
the Item struct, the ItemKind cardinality and per-variant
data shape, the Item-level typed-metadata fields anticipated
by stage-1 R5.2 (`linkage: Linkage`, `inline_hint:
InlineHint`, `is_unsafe: bool`, etc.), the accessor pattern
for variant-discriminated data, and the relationship to
stage-14's anticipated `DomainAttr` framework.

**Options considered.**
- (A) Stage-11's framing baked literally: ItemKind 6-variant
  with `Function { fn_sig: FnSigId, body: Option<Body>, ... }`
  struct-variant carrying post-mono per-Function data on Item.
  Non-Function variants stay unit; per-mono data lives on
  InstanceKind::Static.{ty, init, mutability} / Const.{ty,
  value} / {Struct, Enum, Union}.{layout}.
- (B) **Restored symmetry**: ItemKind 6-variant mostly unit;
  per-mono data (fn_sig, body) moves to `InstanceKind::Function
  { fn_sig: FnSigId, body: Option<Body>, flavor, virtual_idx }`.
  Item-level metadata (linkage, inline_hint, is_unsafe, is_naked,
  link_section, lang-item flags, opaque_handle_eligible) lives
  on `Item` directly with sensible defaults for non-fn variants.
  Stage-11 D-11-3 + D-11-5 reopen.

**Decision.** (B) restored-symmetry. Per spec-snapshot.md §8:

```rust
pub struct Item {
    pub def_path: DefPath,
    pub kind:     ItemKind,
    pub span:     Span,

    // Cross-variant Item-level metadata (D-12-1.3)
    pub linkage:        Linkage,
    pub link_section:   Option<StringId>,

    // Function-relevant flags (Item-level with defaults for non-fn variants)
    pub inline_hint:    InlineHint,
    pub is_unsafe:      bool,
    pub is_naked:       bool,

    // Lang-item flags (D-12-3; closed 3-flag set per §8.2)
    pub is_phantom_data:  bool,
    pub is_unsafe_cell:   bool,
    pub is_manually_drop: bool,

    // Bridge-computed outcome (already pinned at §8.0)
    pub opaque_handle_eligible: bool,
}

pub enum ItemKind {
    Function,
    Static { mutability: Mutability },
    Const,
    Struct,
    Enum,
    Union,
}
```

**Reasoning.**
- (A) is incoherent under the rest of the spec's pre-mono
  Item model. §16.1 F4 sort comparator is `(crate_name,
  crate_stable_id, segments)` with no GenericArgs — Items
  are 1:1 with rustc DefIds (pre-mono). §13.4 source-derived
  Items use rustc's DefPathHash verbatim (per-DefId, not
  per-(DefId, GenericArgs)). §9.3 RegseqInstanceKey carries
  `generic_args_key` alongside `item_id` (one Item, multiple
  Instances per generic — implies Item is pre-mono).
- But stage-3 §15.1 rejects `Param/Bound/Placeholder/Infer/
  Alias` TyKind variants. So a stored FnSig must be
  already-substituted (post-mono); Item.kind.fn_sig under
  (A) cannot store the generic shape.
- The two views are incompatible. (B) resolves the
  contradiction: Items are pre-mono (1:1 with rustc DefIds);
  per-mono FnSig + Body live on Instance per the symmetric
  pattern with Static/Const/Struct/Enum/Union.
- Stage-11's "4-peer convergence" citation (rustc,
  rustc_public, Charon, mrustc) was re-examined and found
  to actually support per-Instance body. rustc_public's
  `Instance::body() -> Option<Body>` at `mono.rs:53-55` is
  the cited pattern (per-Instance, NOT per-Item).
- Item-level Function-flags at the Item layer (rather than
  per-variant on `ItemKind::Function`) keeps the schema
  flat: a `static` is never naked or inline; the defaults
  ("off") are well-defined.
- Stage-9 placement of `mutability: Mutability` on
  `InstanceKind::Static` was a denormalization (statics
  can't be generic; 1:1 Item:Instance always; mutability
  is a source-level keyword choice). Moving to
  `ItemKind::Static { mutability }` honors stage-1
  intent (`stage-1/spec-snapshot.md:840-841`).

**Methodology rules invoked.**
- *Compositional behavior beats special-case flags* — per-
  mono data on Instance, source-level alphabet on Item; no
  parallel "is_function_with_body" / "is_static_mutable"
  flags duplicating variant-discriminator info.
- *C-semantics with information preservation* — preserve
  rustc's per-DefId metadata that's load-bearing for C
  codegen (linkage, inline, naked, link_section); other
  CodegenFnAttrs flags (target_features, optimize, ...) fold
  into emission_name or are out-of-v1-scope.
- *Spec is mutable* — stage-11 D-11-3 + D-11-5 reopen
  cleanly under Option B; stage-12 sees the full
  cross-stage picture and can identify the contradiction
  stage-11's local view missed.
- *Re-derive from evidence; do not import baggage* — Item
  schema re-derived from stage-1 R5.2 + stage-3 use-of-
  Item.linkage + stage-4 stage-3-scoped attribute-table
  framing + rustc evidence on per-DefId metadata + 4-peer
  prior-art.

**Prior-art evidence.**
- rustc-internal: 14-variant ItemKind at `rustc_hir/src/
  hir.rs:4397-4464` with first-class Struct/Enum/Union;
  dedicated `ForeignItem` struct + ForeignItemKind at
  `rustc_hir/src/hir.rs:4567-4602`; `has_body: bool` flag
  distinguishes declared-but-unimplemented domestic Fn
  from foreign decls. Per `prior-art/rustc.md`.
- rustc_public: minimal 4-variant `ItemKind { Fn, Static,
  Const, Ctor(CtorKind) }` at `rustc_public/src/lib.rs:120-
  131` with ADT recovered via type system; `is_foreign_item()`
  boolean predicate at `lib.rs:173`; **`Instance::body() ->
  Option<Body>` at `mono.rs:53-55`** is the per-Instance body
  pattern that actually supports Option B (not the per-Item
  framing stage-11 cited). Per `prior-art/rustc_public.md`.
- Charon: orthogonal split `ItemKind { TopLevel, Closure,
  TraitDecl, TraitImpl }` (scope) + `TypeDeclKind { Struct,
  Enum, Union, Opaque, Alias }` (ADT) at `src/ast/gast.rs:
  97-130` + `src/ast/types.rs:460-476`; `Result<Body, Opaque>`
  body encoding at `gast.rs:150` (closest peer to
  RegseqModel's `body: None` for foreign per D-12-2). Per
  `prior-art/charon.md`.
- mrustc: category-based tagged unions (TypeItem / ValueItem
  / MacroItem); Linkage-based foreign encoding; TraitMarkings
  cached trait flags. Per `prior-art/mrustc.md`.

**Worked example.**
```rust
// Source: pub static FOO: u32 = 42;
let foo_item = Item {
    def_path: ...crate::FOO...,
    kind: ItemKind::Static { mutability: Mutability::Not },
    span: src.rs:5:1..5:25,
    linkage: Linkage::Default,
    link_section: None,
    inline_hint: InlineHint::None,        // default for non-fn
    is_unsafe: false,                     // default for non-fn
    is_naked: false,                      // default for non-fn
    is_phantom_data: false,
    is_unsafe_cell: false,
    is_manually_drop: false,
    opaque_handle_eligible: false,
};
// Single Instance for the static (statics can't be generic):
let foo_instance = Instance {
    item: foo_item_id,
    generic_args: vec![],
    emission_name: ...,
    span: src.rs:5:1..5:25,
    kind: InstanceKind::Static { ty: t_u32, init: Some(c_42) },
    // mutability NOT here -- per D-12-1.2 (c) it lives on ItemKind::Static
};

// Source: pub fn add<T: Add>(x: T, y: T) -> T { x + y }
// Generic; one Item, multiple Instances post-mono:
let add_item = Item {
    def_path: ...crate::add...,
    kind: ItemKind::Function,
    linkage: Linkage::Default,
    inline_hint: InlineHint::None,
    is_unsafe: false,
    is_naked: false,
    ...
};
// One Instance per monomorphization (T=u32, T=u64, ...):
let add_u32_instance = Instance {
    item: add_item_id,
    generic_args: vec![GenericArg::Type(t_u32)],
    kind: InstanceKind::Function {
        fn_sig: t_fn_u32_u32_to_u32,         // SUBSTITUTED post-mono
        body:   Some(monomorphized_body_u32), // SUBSTITUTED post-mono
        flavor: FunctionInstanceFlavor::Item,
        virtual_idx: None,
    },
    ...
};
let add_u64_instance = Instance { ..., generic_args: vec![GenericArg::Type(t_u64)], ... };
```

**Spec cross-reference.** §8.0 (Item schema + ItemKind),
§9.4 (InstanceKind::Function + InstanceKind::Static),
§14.3 (FnSig placement reframe under Option B),
§23.6.2 (callee_fn_sig helper updated).

**Sub-decisions.**

#### D-12-1.1 -- Variant cardinality [LOCKED]

**Decision.** Closed 6-variant flat ItemKind:
`{ Function, Static, Const, Struct, Enum, Union }`.

**Reasoning.** rustc_public's 4-variant minimum projection
(without Struct/Enum/Union) is a stable-IR concern (external
tools recover ADT kind via type system); RegseqModel
consumes ADT layout for C codegen and needs first-class
ADT discriminants. rustc-internal's 14-variant set lifts/
erases items RegseqModel rejects (Trait, TraitAlias, Impl,
AssocTy/AssocFn/AssocConst, Macro, ExternCrate, Use,
ForeignMod, Mod, GlobalAsm, TyAlias) per §8.1 lifting policy
and §15.1 type rejections. Charon's orthogonal split
ItemKind/TypeDeclKind adds indirection without semantic value
under Q3 single-level IR. mrustc's category-based tagged
unions force every consumer to dispatch on category before
kind. The 6-variant flat set is the minimum that mirrors
InstanceKind 1:1 (per stage-1 §8.1 framing).

#### D-12-1.2 -- Per-variant ItemKind data shape (Option B + Static.mutability) [LOCKED]

**Decision.** Under Option B, `ItemKind::Function` becomes
unit (per-mono data moves to `InstanceKind::Function`).
`ItemKind::Static { mutability: Mutability }` carries
source-level mutability per stage-1 intent at
`stage-1/spec-snapshot.md:840-841`. Const / Struct / Enum
/ Union stay unit. **Reopens stage-9** (the decision that
placed `mutability` on `InstanceKind::Static`).

**Reasoning.** Statics can't be generic in Rust; 1:1
Item:Instance always. Mutability is a source-level keyword
choice (`static` vs `static mut`); placing it on Item
(source-level alphabet) per stage-1 intent matches Option B's
symmetry argument: source-level metadata on Item, per-mono
data on Instance. (a) status quo (mutability on InstanceKind)
contradicts stage-1 intent; (c) duplicating across both
denormalizes; (b) cleanest. **Cross-stage reopening** to
flag at merger contract close: `InstanceKind::Static` becomes
2-field `{ ty: TyId, init: Option<ConstId> }`.

#### D-12-1.3 -- Item-level typed fields [LOCKED]

**Decision.** Per stage-1 R5.2 + the Option-B simplification
(Function-specific flags become Item-level with sensible
defaults for non-fn variants), the Item-level typed fields:

| Field | Type | Applies-to | Default for non-applicable variants |
|---|---|---|---|
| `linkage` | `Linkage` (closed 4-variant) | Function + Static | n/a (always meaningful) |
| `link_section` | `Option<StringId>` | Function + Static | None |
| `inline_hint` | `InlineHint` (closed 4-variant) | Function | InlineHint::None |
| `is_unsafe` | `bool` | Function (`unsafe fn`) | false |
| `is_naked` | `bool` | Function (`#[naked]`) | false |

**Linkage** closed 4-variant:
```rust
#[repr(u8)]
pub enum Linkage {
    Default,    // ordinary defined fn/static; mangled name
    NoMangle,   // #[no_mangle] / #[export_name]; symbol_name overrides
    Foreign,    // extern "C" { ... } declaration
    Weak,       // #[linkage = "weak"]
}
```

**InlineHint** closed 4-variant:
```rust
#[repr(u8)]
pub enum InlineHint {
    None,    // no #[inline]
    Hint,    // #[inline]
    Always,  // #[inline(always)]
    Never,   // #[inline(never)]
}
```

**Visibility omitted.** Stage-1 R5.2 named it but no v1
driver use case requires it; not load-bearing for one-CU C
codegen. Reopens if phase-2 surfaces analysis-side need.

**Reasoning.** rustc's `CodegenFnAttrs` carries 17+ flags +
many sub-fields (`symbol_name`, `link_name`, `target_features`,
`linkage`, `import_linkage`, `alignment`, `optimize`, etc.).
Most are folded into `Instance.emission_name` per §9.2
(mangling is symbol-emission-time concern; not Item-level
analysis surface) or are out of v1 driver scope (target
features, optimize hints, alignment overrides). Lift only
the load-bearing subset:
- `inline` is observable in C output (`static inline` /
  `__attribute__((always_inline))` / `__attribute__((noinline))`);
  rustc tracks on every fn (cheap). 4-variant closed mirror
  of rustc's `InlineAttr`.
- `is_naked` is structural for ISR entry stubs (kernel-driver
  use case); without it, codegen breaks raw-asm bodies.
- `is_unsafe` is Codex-review-relevant for analyses ("all
  unsafe fn calls must be inside controlled boundaries"); per
  stage-1 R5.2 explicit naming.
- `linkage` is load-bearing per stage-1 R5.2 + stage-3
  references; closed 4-variant `{ Default, NoMangle, Foreign,
  Weak }` covers driver use cases. Other rustc Linkage variants
  (LinkOnceAny, LinkOnceODR, Appending, Common, Internal,
  Private, External-vs-default-external) are LLVM-specific
  and rejected at bridge per D-12-2.3 + §17.4
  closed-vocabulary rule.
- `link_section` is load-bearing for kernel-driver
  auto-discovery (Linux `MODULE_DEVICE_TABLE`, Zephyr
  `device_handles`, `.init_array` / `.driver_data`); full
  string flexibility (closed-vocab not justified — section
  names are kernel/runtime-defined, not RegseqModel-defined).

**Function-flags at Item level (not per-variant on
`ItemKind::Function`).** Cleaner schema; flat match arms;
defaults are well-defined for non-fn variants (a `static` is
never naked or inline). Per "compositional behavior beats
special-case flags" methodology rule.

#### D-12-1.4 -- Option B InstanceKind::Function reframe [LOCKED]

**Decision.** Restore symmetry across all 6 InstanceKind
variants. Per §9.4:

```rust
pub enum InstanceKind {
    Function {
        fn_sig:      FnSigId,                  // post-mono substituted signature
        body:        Option<Body>,             // post-mono substituted body; None for foreign
        flavor:      FunctionInstanceFlavor,   // existing
        virtual_idx: Option<u32>,              // existing
    },
    Static { ty: TyId, init: Option<ConstId> },
    Const  { ty: TyId, value: ConstId },
    Struct { layout: TyId },
    Enum   { layout: TyId },
    Union  { layout: TyId },
}
```

**Reopens stage-11 D-11-3 + D-11-5** — tracked separately;
flips both to `[REOPENED]` in `decisions.md` at stage-12
merger contract close per README §5.5 step 4 with annotation
`(revised in stage 12 per D-12-1.4)`.

**Reasoning.** Stage-11 D-11-3 (FnSig location: Item-side)
and D-11-5 (no BodyId; bodies as `Item.body: Option<Body>`)
were placed on Item but contradicted the rest of the spec's
pre-mono Item model:

- §16.1 F4 sort comparator `(crate_name, crate_stable_id,
  segments)` carries no GenericArgs — Items are 1:1 with
  rustc DefIds (pre-mono).
- §13.4 source-derived Items use rustc's DefPathHash
  verbatim (per-DefId, not per-(DefId, GenericArgs)).
- §9.3 RegseqInstanceKey carries `generic_args_key`
  alongside `item_id` — implies one Item per DefId, multiple
  Instances per generic.
- §3.1 invariant 2 says `item_def_paths[item_id]` is
  rustc's `DefPathHash` for source-derived items —
  per-DefId.
- Stage-3 §15.1 rejects Param/Bound/Placeholder/Infer/Alias
  TyKind variants — so a generic FnSig with Param TyIds
  CAN'T be stored on Item. If `Item.kind.fn_sig` exists,
  it must be already-substituted (post-mono).
- §23.6.2 `callee_fn_sig` helper retrieved FnSig from Item
  with no substitution step — implies post-mono.

These views are incompatible. Option B resolves: Items
pre-mono (1:1 with DefIds), per-mono FnSig + Body on
Instance.

Stage-11's "4-peer convergence" citation was actually
supportive of per-Instance body placement:
- rustc-internal **directly establishes** per-Instance
  body: `tcx.fn_sig(def_id)` returns
  `EarlyBinder<PolyFnSig>` (generic); body fetched per
  Instance via `tcx.instance_mir(InstanceKind)` per
  `rustc_middle/src/ty/mod.rs:1671`.
- rustc_public **directly establishes** per-Instance
  body: `FnDef::fn_sig()` is generic; per-Instance body
  via **`Instance::body() -> Option<Body>`** at
  `rustc_public/src/mir/mono.rs:53-55`, structurally
  separate from `CrateItem::body()`.
- Charon **supports** per-mono body but doesn't directly
  distinguish Item-side vs Instance-side: with
  `--monomorphize`, `FunDecl` IS post-mono —
  `FunDecl.body: Result<Body, Opaque>` is per-FunDecl
  (Charon doesn't carry the same Item/Instance split).
- mrustc is **informational only**: prior-art evidence
  (per `prior-art/mrustc.md`) covers Function structures
  with linkage metadata but did not directly verify
  body-storage placement.

The two directly-establishing peers (rustc-internal +
rustc_public) carry **bodies per-Instance**, supporting
Option B. Stage-11's body-on-Item framing was a
misreading of the rustc_public citation (which actually
establishes `Instance::body()`, not `CrateItem::body()`,
as the per-mono accessor); Option B restores faithfulness
to that evidence.

**Spec edits required (cross-stage):**
- `spec.md` §25.4 (FnSig location): reframe FnSig as
  per-Instance, not Item-side.
- `spec.md` §25.5 (Body-vs-Item linkage): reframe Body as
  per-Instance structural field on InstanceKind::Function.
- `spec.md` §23.6.2 callee_fn_sig helper: drop Item
  indirection step:
  ```text
  callee_fn_sig(callee_id: InstanceId) -> &FnSig :=
      &model.sigs[model.instances[callee_id].kind.as_function().fn_sig]
  ```
- `spec.md` §9.4 InstanceKind: gain `fn_sig` + `body` on
  `Function` variant.
- `spec.md` §3.1 invariant 10: reword as "Body presence
  matches `InstanceKind::Function.body` shape per Option B."

#### D-12-1.5 -- DomainAttr sibling-table forward-pointer [LOCKED]

**Decision.** Stage 12 adds NO schema for `#[rseq::*]`
property attributes. Stage-14 owns the `DomainAttr` sibling-
table architecture per stage-4 spec-snapshot.md:1287-1299.
Stage 12 only updates §8.0 commentary + §3 forward-pointer
to fix the merge artifact that erased stage-4's
"scoped to stage 3" qualifier on the "no general attribute
table" wording.

**Reasoning.** Stage-1 R5.2 anticipated `DomainAttr` on
Item / Instance; stage-4 explicitly scoped the
"no general attribute table" claim to stage-3 only and
committed to stage-14 sibling tables on `RegseqModel` keyed
by `ItemId` / `InstanceId` / `FieldPath` / `Span`. The
merged spec.md §8.0 dropped the stage-3-scoping qualifier
when absorbing stage-4 content (merge artifact). Stage 12
restores the framing: typed Item fields per D-12-1.3 land
at stage 12; structured DSL property attributes
(`#[rseq::*]`) land in stage-14 sibling tables.

**Why no schema on Item.** Stage-4's commitment is
sibling-table architecture, NOT per-Item fields. Pre-staging
a placeholder field on Item would risk locking a shape
stage-14 hasn't designed (e.g., would the field be
`Vec<DomainAttr>`? `HashMap<DomainKey, Value>`?). Stage 14
adds the table when it lands per §17.4 closed-vocabulary
rule.

**Spec edits.** §8.0 commentary replaces "no general
attribute table on Item" with explicit stage-12-vs-stage-14
split. §3 root struct gets a forward-pointer comment for
stage-14's anticipated `domain_attrs_*` sibling tables (no
field added).

#### D-12-1.6 -- Accessor pattern (hybrid + named struct) [LOCKED]

**Decision.** Hybrid: `InstanceKind::as_function() ->
Option<&FunctionInstanceData>` convenience method + match
arms for exhaustive handling. Named `FunctionInstanceData`
struct extracted from struct-variant for ergonomic
destructuring + accessor return type.

```rust
impl InstanceKind {
    pub fn as_function(&self) -> Option<&FunctionInstanceData> { ... }
    pub fn as_function_mut(&mut self) -> Option<&mut FunctionInstanceData> { ... }
}

pub struct FunctionInstanceData {
    pub fn_sig:      FnSigId,
    pub body:        Option<Body>,
    pub flavor:      FunctionInstanceFlavor,
    pub virtual_idx: Option<u32>,
}
```

**Reasoning.** The `§23.6.2 callee_fn_sig` helper assumes a
single-variant query (`inst.kind.as_function().fn_sig`); a
match-arm-only API would force re-issuing the destructure at
each call site. The hybrid pattern matches rustc-internal's
convention (`as_*` accessors throughout `ItemKind` / `TyKind`).
Naming `FunctionInstanceData` (vs `FunctionData` / `FnInstance`)
mirrors the parent's `InstanceKind::Function` variant +
clarifies the input domain.

#### D-12-1.7 -- Stage-11 reopening process [LOCKED]

**Decision.** Stage-11 D-11-3 + D-11-5 flip to `[REOPENED]`
in `decisions.md` at stage-12 merger contract close per
README §5.5 step 4, with annotation `(revised in stage 12
per D-12-1.4)`. Stage-11's `spec-snapshot.md` is FROZEN
(historical artifact); stage-11 content in top-level
`spec.md` IS edited in-place per README §5.9 (live, not
historical; refined content updates in place).

**Reasoning.** Same precedent as stage-11 D-11-3 itself
(which superseded stage-1 inbound's Body-side FnSig
framing). Cross-stage drift detected at stage 12; stage-11
spec content gets minimal in-place edits (§25.4, §25.5,
§23.6.2, §9.4, §3.1 invariant 10) at stage-12 merger.
Stage-11's other decisions (D-11-1 Body shape, D-11-4
LocalId, D-11-6 visitor, D-11-7 hybrid timing, D-11-8
push API, D-11-K rendering deferral) NOT affected.

#### D-12-1.8 -- Instance-side pull-through helpers [LOCKED]

**Decision.** Methods on `RegseqModel` for Item metadata
pull-throughs given `InstanceId`. v1 inventory:

```rust
impl RegseqModel {
    // Identity / source-level traceback
    pub fn instance_def_path(&self, id: InstanceId) -> &DefPath { ... }
    pub fn instance_span(&self, id: InstanceId) -> Span { ... }

    // Source-level metadata fields per D-12-1.3
    pub fn instance_linkage(&self, id: InstanceId) -> Linkage { ... }
    pub fn instance_link_section(&self, id: InstanceId) -> Option<StringId> { ... }
    pub fn instance_inline_hint(&self, id: InstanceId) -> InlineHint { ... }
    pub fn instance_is_unsafe(&self, id: InstanceId) -> bool { ... }
    pub fn instance_is_naked(&self, id: InstanceId) -> bool { ... }

    // Lang-item flags
    pub fn instance_is_phantom_data(&self, id: InstanceId) -> bool { ... }
    pub fn instance_is_unsafe_cell(&self, id: InstanceId) -> bool { ... }
    pub fn instance_is_manually_drop(&self, id: InstanceId) -> bool { ... }

    // Bridge-computed outcome
    pub fn instance_opaque_handle_eligible(&self, id: InstanceId) -> bool { ... }

    // Post-Option-B InstanceKind::Function pull-throughs
    pub fn instance_fn_sig(&self, id: InstanceId) -> Option<&FnSig> { ... }
    pub fn instance_body(&self, id: InstanceId) -> Option<&Body> { ... }
}
```

**Reasoning.** Every Instance has `Instance.item: ItemId`;
Item-level metadata is reachable via simple pull-through.
Methods-on-`RegseqModel` (option a) chosen over methods-on-
`Instance` (option b — clutters call sites with `&model`
parameter) and wrapper-type `InstanceRef<'m>` (option c —
introduces new type; deferable). The wrapper-type design
can be added as a thin layer if call-site ergonomics
demand. v1 simplicity wins: model owns the data; helpers
naturally live with the data. Composes with §23.6.2
`callee_fn_sig` helper pattern (existing convention).

`instance_<field>()` naming convention prefixes input domain;
field names mirror underlying Item field names (no synonyms).

---

### D-12-2 -- Foreign / extern declaration dispatch [LOCKED]

**Question.** Bridge dispatch rule for foreign declarations
(`extern "C" { fn foo(); }` and `extern "C" { static FOO:
u32; }`). Three coupled signals exist post-D-12-1:
`Item.linkage == Linkage::Foreign`, `InstanceKind::Function.
body == None`, `InstanceKind::Static.init == None`. Stage 12
must pin the canonical bridge dispatch rule (which signal is
authoritative) and how rustc's `is_foreign_item` predicate
maps to model-side state.

**Options considered.**
- (a) Body=None / init=None as primary signal; linkage as
  derived. Body is a structural InstanceKind field; linkage
  is computed from rustc's `tcx.is_foreign_item` at bridge
  construction.
- (b) `Linkage::Foreign` as primary signal; Body=None /
  init=None as consequences. Linkage is a source-level
  classifier on Item; consumers query Item.linkage.

**Decision.** Both signals are bridge-populated from the
same source predicate (`tcx.is_foreign_item(did)`); they
are consistent by construction. Consumers can use either:
`Item.linkage == Linkage::Foreign` (Item-level analysis) or
`InstanceKind::Function.body == None` (Instance-level
codegen). The bridge populates per the per-Item mapping
in D-12-2.3.

**Reasoning.** rustc's `is_foreign_item(did)` (parent
`DefKind == ForeignMod` per `rustc_middle/src/hir/mod.rs:
165-168`) is the single-source-of-truth predicate. Both
`Item.linkage = Linkage::Foreign` and `InstanceKind.body /
.init = None` derive from it. Redundancy is acceptable
because the redundant signals serve different consumer
contexts:
- Item-level analyses (e.g., "list all foreign symbols")
  use `Item.linkage == Linkage::Foreign`.
- Instance-level codegen (e.g., "emit declaration vs
  definition") uses `body.is_none() / init.is_none()`.

The methodology rule "compositional behavior beats
special-case flags" doesn't apply here because the signals
serve different semantic levels (source-level alphabet vs
per-mono emission unit); both are useful.

**Methodology rules invoked.**
- *Store the result rustc gives us for free* — both signals
  derive from `is_foreign_item` at bridge construction;
  cheap to populate.
- *C-semantics with information preservation* — preserve
  the distinction between source-level "this is foreign"
  (analysis) and per-mono "no body to emit" (codegen).

**Prior-art evidence.**
- rustc-internal: dedicated `ForeignItem` struct at
  `rustc_hir/src/hir.rs:4567-4602` with `ForeignItemKind {
  Fn, Static, Type }`. RegseqModel rejects this option per
  D-12-1.4 (stage-11's `body: Option<Body>` shape kept; no
  separate ForeignItem variant). Per `prior-art/rustc.md`.
- rustc_public: boolean `is_foreign_item()` predicate at
  `rustc_public/src/lib.rs:173`. Closest peer to RegseqModel
  D-12-2's combined signal pattern. Per
  `prior-art/rustc_public.md`.
- Charon: `Result<Body, Opaque>` body encoding at
  `gast.rs:150` (comment: "Opaque functions are: external
  functions, or local functions tagged as opaque"). Closest
  peer to RegseqModel's `body: Option<Body>` pattern. Per
  `prior-art/charon.md`.
- mrustc: Linkage-based; ABI on Function struct; no
  dedicated foreign type. Per `prior-art/mrustc.md`.

**Spec cross-reference.** §9.4 (InstanceKind dispatch),
§8.0 (Item.linkage), §3.4 (bridge construction-pipeline).

**Sub-decisions.**

#### D-12-2.1 -- Foreign Function dispatch [LOCKED]

**Decision.** `InstanceKind::Function.body: None ⟺ tcx.
is_foreign_item(callee_did)`. When `is_foreign_item` returns
true:
- `body: None` — rustc's `is_mir_available(did)` is `false`
  per `rustc_monomorphize/src/collector.rs:1059-1062`.
- `fn_sig: FnSigId` is **populated** — `tcx.fn_sig(did)` is
  callable for foreign fns; signature required for indirect-
  call shape and C-prototype emission.
- `flavor: FunctionInstanceFlavor::Item` (foreign decls are
  not shims).
- `virtual_idx: None`.

#### D-12-2.2 -- Foreign Static dispatch [LOCKED]

**Decision.** `InstanceKind::Static.init: None ⟺ tcx.
is_foreign_item(static_did)`. When `is_foreign_item` returns
true:
- `init: None` — rustc has no const-eval result for foreign
  statics; value provided at link time.
- `ty: TyId` is **populated** — `tcx.type_of(did)` is callable.
- Mutability lives on `ItemKind::Static { mutability }` per
  D-12-1.2 (c); InstanceKind::Static is now 2-field
  `{ ty, init }`.

#### D-12-2.3 -- Linkage translation table [LOCKED]

**Decision.** Bridge translation map from rustc to RegseqModel
`Linkage`:

| Source signal | RegseqModel `Linkage` |
|---|---|
| `tcx.is_foreign_item(did) == true` | `Linkage::Foreign` |
| `CodegenFnAttrFlags::NO_MANGLE` set OR `#[export_name]` (`symbol_name.is_some()`) | `Linkage::NoMangle` |
| `CodegenFnAttrs.linkage == Some(WeakAny | WeakODR)` | `Linkage::Weak` |
| Otherwise (default external + mangled) | `Linkage::Default` |

Other rustc `Linkage` variants (LinkOnceAny, LinkOnceODR,
Appending, Common, Internal, Private, External-when-distinct-
from-default) are rejected at the bridge with
`BridgeError::UnsupportedLinkage`. Driver fixtures don't
use these; rejecting keeps closed-vocabulary discipline
intact.

#### D-12-2.4 -- link_name placement [LOCKED]

**Decision.** `link_name` is NOT on Item per D-12-1.3
(omitted). The mangled / overridden symbol lives on
`Instance.emission_name: StringId` per §9.2 (rustc's
`tcx.symbol_name(MonoItem::*)` mangled name). Bridge populates
`Instance.emission_name` from rustc's symbol-name query,
which already incorporates `#[link_name]` overrides for
foreign items.

**Reasoning.** Mangling is symbol-emission-time concern,
not Item-level analysis surface. Folding into
`Instance.emission_name` consolidates: one place for
the mangled name; consumers read `instance.emission_name`
without needing a parallel `Item.link_name` lookup.

---

### D-12-3 -- Lang-item recognition mechanism [LOCKED]

**Question.** Stage-1 + §8.2 currently pin 3 lang-item flags
on Item (`is_phantom_data`, `is_unsafe_cell`,
`is_manually_drop`). Stage 12 must lock the recognition
mechanism formally and document the closed-vocabulary
discipline for non-flagged lang-items (CVoid, MaybeUninit,
Pin, Box, Drop trait, drop_in_place, Fn/FnMut/FnOnce traits,
panic_fn, etc.). Specifically: where is the recognition done
(bridge construction time vs at use site), and is there an
opaque catch-all (Charon-style `lang_item: Option<String>`)
for lang-items not in the closed flag set?

**Options considered.**
- (a) Closed 3-flag set per current §8.2; AdtFlags-predicate-
  based recognition; non-flagged lang-items queried on-demand
  at consumer sites; reject opaque LangItem tag.
- (b) Expand to 5+ flag set including Pin, Box, etc.; or
  include MaybeUninit explicitly.
- (c) Catch-all `lang_item: Option<LangItem>` opaque tag
  (rustc has 208 LangItem variants per Fork 2 evidence) or
  `lang_item: Option<String>` (Charon-style) on every Item.

**Decision.** (a) closed 3-flag set + AdtFlags predicates +
query-on-demand for non-flagged + reject opaque.

**Reasoning.**
- Each currently-flagged lang-item is load-bearing for
  type-system or codegen lowering: PhantomData triggers ZST
  handling; UnsafeCell triggers §17.2.1 trait-object dispatch
  carve-out; ManuallyDrop triggers no-auto-drop union
  semantics. AdtFlags-predicate recognition is rustc's
  canonical fast path (O(1) bitflag tests) per
  `rustc_middle/src/ty/adt.rs:30-63, 414-440`.
- Non-flagged lang-items (CVoid, Pin, Box, etc.) are consumed
  at specific bridge call sites (type translation, drop-as-
  call collapse, etc.) and never need Item-walk-time access.
  Storing them on Item would denormalize and force every
  Item-walk to consider an irrelevant flag.
- `Pin` deferred: reintroduce `is_pin` when async support
  lands per current §8.2 commitment.
- `Box`, `CVoid`: rejected at bridge per stage-3 D1.4 / D16;
  no flag needed (rejection happens at type translation).
- `MaybeUninit`: composition (Union no-auto-drop + inner
  ManuallyDrop) suffices per stage-3 D1.5 reasoning.
- Opaque LangItem tag (208-variant rustc enum) violates
  §17.4 closed-vocabulary rule. Adding new lang-item
  recognition is a methodology event (per current §8.2
  "Reopens when a v1 driver fixture or stage-14 intrinsic
  surfaces a stable need"); opaque tag would let
  toolchain-bump-introduced variants slip in without that
  gate.
- Charon-style `lang_item: Option<String>` admits silent
  typo / drift; Charon's choice is justified by Charon's
  source-of-truth-is-rustc design; RegseqModel is a
  closed-vocabulary IR per §17.4 + Q3 single-level IR.

**Methodology rules invoked.**
- *Closed `Ty` enum* (§17.4 generalized to all closed
  vocabularies in RegseqModel) — opaque tag rejected.
- *Compositional behavior beats special-case flags* —
  MaybeUninit composes from existing Union + ManuallyDrop;
  no parallel flag.
- *Store the result rustc gives us for free, interrogated
  per construct* — only store load-bearing-for-codegen
  outcomes; query the rest on-demand.
- *No defensive fallbacks* — toolchain-bump LangItem
  expansions reject explicitly; no silent admit-as-string
  fallback.

**Prior-art evidence.**
- rustc-internal: 208-variant `LangItem` enum at
  `rustc_hir/src/lang_items.rs:79-451`; `tcx.is_lang_item(did,
  LangItem::X)` query pattern; `tcx.lang_items().X()` accessor
  pattern; AdtFlags bits for PhantomData/UnsafeCell/
  ManuallyDrop/Pin/Box at `rustc_middle/src/ty/adt.rs:30-63`.
  Per `prior-art/rustc.md`.
- rustc_public: NO lang-item exposure in stable API. Per
  `prior-art/rustc_public.md`.
- Charon: string-based `lang_item: Option<String>` at
  `src/ast/meta.rs:231` — RegseqModel rejects this shape.
  Per `prior-art/charon.md`.
- mrustc: TraitMarkings cached trait flags
  (`has_a_deref`, `has_drop_impl`, `is_copy`, ...). Per
  `prior-art/mrustc.md`.

**Spec cross-reference.** §8.2 (Item-level lang-item flags),
§17.4 (closed-vocabulary rule).

**Sub-decisions.**

#### D-12-3.1 -- Closed 3-flag set carries [LOCKED]

**Decision.** No expansion in v1. Carries forward §8.2's
3-flag set unchanged: `is_phantom_data`, `is_unsafe_cell`,
`is_manually_drop`. `is_pin` deferred per current §8.2;
`is_c_void` / `is_box` rejected (recognition at type
translation; bridge rejects).

#### D-12-3.2 -- Recognition via AdtFlags predicates [LOCKED]

**Decision.** Bridge populates the 3 flags via rustc's
predicate methods at type translation time:
```text
when bridge translates an Adt to an Item:
    item.is_phantom_data  = adt_def.is_phantom_data();   // AdtFlags::IS_PHANTOM_DATA
    item.is_unsafe_cell   = adt_def.is_unsafe_cell();    // AdtFlags::IS_UNSAFE_CELL
    item.is_manually_drop = adt_def.is_manually_drop();  // AdtFlags::IS_MANUALLY_DROP
```

#### D-12-3.3 -- Non-flagged lang-items queried on-demand [LOCKED]

**Decision.** Lang items NOT in the §8.2 closed flag set are
recognized at the consumer site via `tcx.is_lang_item(did,
LangItem::X)` or `tcx.lang_items().X()` accessor — never
stored on `Item`.

#### D-12-3.4 -- Reject opaque LangItem tag and string-based field [LOCKED]

**Decision.** No `Option<LangItem>` opaque tag; no
Charon-style `lang_item: Option<String>` field on Item.
Closed-vocabulary discipline per §17.4. Adding new
lang-item recognition is a methodology event (Codex review
of the new flag's semantic placement); opaque tag would
defeat the gate.

---

### D-12-4 -- Construction-invariant closure (stage-1 deferrals 3, 4, 6) [LOCKED]

**Question.** Stage 1 deferred three construction invariants
to stage 12 pending stage-12 schema commits:
- **Invariant 3** (F4 sort monotonicity): items are placed
  in F4-sorted order; validate must check.
- **Invariant 4** (BridgeInput spec_crate_stable_id half):
  `crates[primary_crate].stable_crate_id ==
  bridge_input.spec_crate_stable_id`.
- **Invariant 6** (def_path.crate_id bound):
  `item.def_path.crate_id.index() < crates.len()`.

Stage 12 must specify the algorithms, landing site within
the §3.3 Phase B validation order, and (for invariant 4) the
`BridgeInput` schema + validate signature change.

**Options considered.**
- (a) Lift all three to a new validation phase. Too coarse;
  duplicates structure §3.3 Phase B already establishes.
- (b) Land within §3.3 Phase B step 3 (Items / crates /
  target / build / invariants). All three checks operate on
  Items + crates which are already validated in this phase.
  Cohesive landing.

**Decision.** (b) within §3.3 Phase B step 3, reordered to
(a)..(g) per the new sub-step ordering below.

**Reasoning.**
- All three checks operate on `items` and `crates` —
  already-validated tables at Phase B step 3.
- Ordering matters: invariant 6 (crate_id bound) before
  invariant 3 (F4 monotonicity walk) because the F4 sort
  key reads `crates[item.def_path.crate_id].name` — bound
  check must precede sort-key derivation.
- Invariant 4 depends only on `crates` (already validated)
  and `primary_crate` (already model-checkable per stage-1);
  can land anywhere in the block.
- BridgeInput parameter on validate is a controlled
  signature change; v1 schema is single-field
  `{ spec_crate_stable_id }` with open-extension for future
  bridge-supplied facts.

**Methodology rules invoked.**
- *Spec is mutable* — invariant ordering refined to capture
  the bound-before-walk dependency.
- *No defensive fallbacks* — invariants are model-checkable
  from RegseqModel + BridgeInput; no rustc-side re-derivation
  needed.

**Prior-art evidence.** N/A — these are stage-1-deferred
mechanical invariants; algorithms are bound checks +
monotonicity walks. Direct primary-source for the
walk-discipline shape: rustc's MIR validator at
`rustc_mir_transform/src/validate.rs` uses a similar
walk-and-check pattern.

**Spec cross-reference.** §3.1 invariants 3, 4, 6;
§3.3 Phase B step 3 (revised); §16.1 (F4 sort comparator);
new `BridgeInput` struct.

**Sub-decisions.**

#### D-12-4.1 -- Invariant 3 closure (F4 sort monotonicity) [LOCKED]

**Decision.** Walk `items` index-pair-wise; for each adjacent
pair `(items[i], items[i+1])`, compute the F4 sort key per
§16.1 and check **non-strict** monotonicity (`key[i] <=
key[i+1]`). Out-of-order is `ValidationError::F4OutOfOrder
{ item_id, prev_key, next_key }`.

```text
fn validate_f4_monotonicity(model: &RegseqModel) -> Result<(), ValidationError> {
    for i in 0..model.items.len().saturating_sub(1) {
        let key_i   = f4_sort_key(model, ItemId::from(i));
        let key_ipp = f4_sort_key(model, ItemId::from(i + 1));
        if key_i > key_ipp {
            return Err(ValidationError::F4OutOfOrder { ... });
        }
    }
    Ok(())
}
```

**Tie semantics.** Adjacent items with identical sort keys
are NOT a violation — F4 admits ties on identical DefPath.
Check is `>` (strict descent), not `>=`.

**Tests.** `tests/model.rs`: positive (3-item F4-sorted) +
negative (3-item reverse-F4) cases.

**Landing site.** Phase B step 3 (d).

#### D-12-4.2 -- Invariant 6 closure (def_path.crate_id bound) [LOCKED]

**Decision.** Walk `items`; for each item, check
`item.def_path.crate_id.index() < model.crates.len()`.
Out-of-bounds is `ValidationError::DefPathCrateIdOutOfBounds
{ item_id, crate_id, crates_len }`.

**Landing site.** Phase B step 3 (c) — before invariant 3
walk per the bound-before-key-derivation dependency.

#### D-12-4.3 -- Invariant 4 closure (BridgeInput.spec_crate_stable_id) [LOCKED]

**Decision.** Add `BridgeInput` struct to `regseq-model`
(zero rustc deps; pure data type per Q1):

```rust
pub struct BridgeInput {
    pub spec_crate_stable_id: StableCrateId,
}
```

Validate signature changes from `RegseqModel::validate(&self)
-> Result<...>` to `RegseqModel::validate(&self,
bridge_input: &BridgeInput) -> Result<...>`.

Algorithm:
```text
fn validate_spec_crate_stable_id(
    model: &RegseqModel,
    bridge_input: &BridgeInput,
) -> Result<(), ValidationError> {
    let primary = &model.crates[model.primary_crate];
    if primary.stable_crate_id != bridge_input.spec_crate_stable_id {
        return Err(ValidationError::SpecCrateStableIdMismatch { ... });
    }
    Ok(())
}
```

**Test ergonomic helper.** `BridgeInput` exposes
`BridgeInput::matching(model: &RegseqModel) -> Self` for
test fixtures that don't care about invariant 4
specifically (constructs a `BridgeInput` with
`spec_crate_stable_id` matching the model's
`crates[primary_crate].stable_crate_id`).

**Landing site.** Phase B step 3 (e).

**Phase B step 3 final order:**
```text
(a) Validate `crates`: stable_crate_id uniqueness + sorted (existing — invariant 5).
(b) Validate `items` shape: length parity (existing — invariant 2 length parity).
(c) Validate `item.def_path.crate_id < crates.len()` for every item (D-12-4.2).
(d) Validate F4 sort monotonicity over `items` (D-12-4.1).
(e) Validate `crates[primary_crate].stable_crate_id == bridge_input.spec_crate_stable_id` (D-12-4.3).
(f) Validate `target` well-formedness (existing — invariant 15).
(g) Validate `build` well-formedness (existing — invariant 17).
```

---

### D-12-5 -- Bridge construction-pipeline order [LOCKED]

**Question.** Stages 1, 3, 4, 9, 11 each framed pieces of
the bridge construction pipeline (rustc mono-item collector
discovery; vtable / closure / ConstMemoryStatic synthesis
per §13; F4 sort; ItemId assignment; side-table population;
Instance construction). Stage 12 must consolidate these
pieces into a single canonical pipeline contract: the
ordering, where synthesis interleaves, and when ItemIds are
assigned. The model-side observable property (post-sort
F4-deterministic ItemIds) is what consumers depend on; the
contract must commit to this shape without over-specifying
bridge-implementation details.

**Options considered.**
- (a) Implementation-detail delegation: pipeline is bridge's
  business; spec only commits to externally-observable
  invariants (F4-sorted Items; valid cross-references; etc.).
- (b) **Canonical 5-phase pipeline contract**: spec pins the
  ordering Discovery -> Translation+Synthesis -> F4 Sort ->
  ItemId Assignment + side-tables -> Instance Construction.
  Synthesis interleaves with translation. ItemIds post-sort.
  Cross-reference handling during construction is bridge
  choice (placeholder vs DefPathHash key).

**Decision.** (b) canonical 5-phase pipeline contract; new
§3.4 subsection.

**Reasoning.**
- (a) is too loose. Stage-3 D5/D6 + stage-4 D-4-3 + stage-9
  + stage-11 inbound bundles cite synthesis triggers (e.g.,
  `Cast::Unsize` triggers vtable instance Item registration)
  that need a contract on when synthesis happens. Without
  the contract, bridge implementations could diverge.
- (b) catalogs the pipeline that already implicitly exists
  across stage spec contributions; gives bridge implementers
  a single landing site for the contract.
- The 5-phase shape is the natural decomposition: Discovery
  is a one-shot rustc query; Translation+Synthesis is the
  interleaved walk; F4 Sort is a one-shot pass over the
  accumulated set; ItemId Assignment is the post-sort
  permutation; Instance Construction is the per-mono walk
  that resolves cross-references.
- Pin externally-observable property (post-sort
  F4-deterministic ItemIds); leave cross-reference handling
  during construction (placeholder ItemIds vs DefPathHash
  key) as bridge-implementation choice — it's not externally
  observable.

**Methodology rules invoked.**
- *Compositional behavior beats special-case flags* —
  pipeline order falls out of Q2 (eager mono baking) + the
  F4 comparator + the synthesis-trigger rules in §13; no
  parallel pipeline-stage tags.
- *C-semantics with information preservation* — pipeline
  contract preserves rustc's mono-item closure (Phase 1)
  and F4 deterministic ordering (Phase 3) as the two
  primary external commitments.

**Prior-art evidence.**
- rustc-internal: `rustc_monomorphize/src/collector.rs:1803`
  (`collect_crate_mono_items` entry); `rustc_codegen_ssa/
  src/base.rs:681` (`codegen_crate` outer loop). Per
  rustc-references.md.
- rustc_public: `MonoItem` set + per-Instance `body()`
  fetch pattern (no separate sort phase; rustc_public
  doesn't commit to Item ordering).
- Charon: monomorphize mode runs after extraction; phase-
  ordering is similar but Charon doesn't make the F4-
  determinism commitment.
- mrustc: standard collector + monomorphize flow.

**Spec cross-reference.** New §3.4 (canonical 5-phase
pipeline); §13 (synthesis triggers); §16.1 (F4 sort
comparator).

**Sub-decisions.**

#### D-12-5.1 -- Canonical 5-phase pipeline structure [LOCKED]

**Decision.** Pinned 5-phase pipeline:

```text
Phase 1  -- DISCOVERY.
    rustc's `collect_crate_mono_items(tcx, strategy)` returns
    Vec<MonoItem> covering primary crate + regseq-aware deps
    per Q6.

Phase 2  -- TRANSLATION (with interleaved Item synthesis).
    Bridge walks discovered mono-items. During translation,
    synthesis triggers fire and produce additional Items per §13:
      - Ty::Dynamic       -> VTableType Item per §13.2 (D5)
      - Cast::Unsize      -> VTableInstance + VTableShim Items per §13.2 (D5)
      - Ty::Closure       -> ClosureCaptures Item per §13.1 (D6)
      - closure call site -> ClosureCall Function Items per §13.1 (D6)
      - non-trivial drop  -> ClosureDropGlue Function Item per §13.1 (D6)
      - reference-typed const -> ConstMemoryStatic Item per §13.3 (D-4-3)
      - Drop terminator   -> drop-glue Function Item synthesis
        (the call target for the Phase-5 Drop-as-Call rewrite)
        per stage-9 §23.4
    Note: the Drop-as-Call collapse itself (rewriting Drop into
    Statement+Call inside a Body) is a Phase-5 body-construction
    step, not a Phase-2 Item-synthesis trigger; Phase 2 only
    synthesises the drop-glue Function Item.

Phase 3  -- F4 SORT.
    Sort accumulated Items (source + synthetic) per §16.1
    comparator: (crate_name, crate_stable_id, segments).

Phase 4  -- ItemId ASSIGNMENT + side-table population.
    Sorted Items get sequential ItemIds: ItemId(0..items.len()).
    `item_def_paths[ItemId]` and `item_synthetic_kinds[ItemId]`
    populated in parallel.

Phase 5  -- INSTANCE construction + cross-reference resolution
    + body-content rewrites + validation. `instances` per §9.2
    (one Instance per (Item, GenericArgs, rustc shim flavor)
    tuple). Sorted by F4-2 RegseqInstanceKey per §16.3.
    Cross-references resolved to final ItemIds.
    `RegseqModel.vtables` dedup table populated. Body-content
    rewrites sequence: (a) Drop-as-Call collapse (per §25.7.1;
    rewrite Drop terminator into Statement+Call referencing
    the Phase-2-synthesised drop-glue Function Item), then
    (b) vtable-indirect rewrite (per §25.7.2; intra-Phase-5
    pass over materialised Bodies), then (c) joint validation
    walk per §25.6.
```

#### D-12-5.2 -- Synthesis interleaving [LOCKED]

**Decision.** Bridge-synthesised Items are produced **during**
Phase 2 translation, NOT in a separate pre-pass that runs
before discovery or after.

**Reasoning.** Synthesis triggers are construct-driven
(`Ty::Dynamic` encountered → vtable type Item); the bridge
can't pre-enumerate triggers without first translating.
Synthesis must complete before Phase 3 F4 sort (sort needs
the full Item set).

**Cache discipline.** Each synthesis trigger checks a
**family-specific Phase-2 working cache** keyed by a stable
Phase-2-available handle (NOT final `ItemId`, which doesn't
exist until Phase 4 per D-12-5.3). Examples:
- Vtable family: `PrincipalTraitKey -> Phase2Handle`.
- Closure family: `closure_did -> Phase2Handle` (rustc's
  closure DefPathHash is stable and Phase-2-available).
- ConstMemoryStatic family: `ContentHash -> Phase2Handle`.

Cache hit returns the existing Phase2Handle; cache miss
registers a new synthetic Item shape (with synthesised
DefPathHash per §13.4 as the natural Phase-2 stable key).
Idempotent: revisiting the same trigger during Phase 2
yields the previously-registered Phase2Handle.

The model-side `RegseqModel.vtables: HashMap<PrincipalTraitKey,
ItemId>` (and analogous final-form caches) is populated in
**Phase 5** (after final ItemIds exist via the Phase-4
assignment pass per D-12-5.3). Phase 2 working caches use
Phase2Handles; Phase 5 walks them and re-keys to final
ItemIds.

#### D-12-5.3 -- Post-sort ItemId assignment [LOCKED]

**Decision.** Final `ItemId` values are assigned at Phase 4
(after F4 sort), not incrementally during Phase 2. F4 +
build-equivalence quad (§12.4) commit to deterministic
ItemIds across builds with the same source. Incremental
assignment would tie ItemIds to rustc's collector traversal
order, which can vary with incremental compilation cache
state.

**Cross-reference handling during construction.** Bridge
implementation choice; not externally observable. Two viable
patterns:
- **(a) Placeholder ItemIds + remap.** Bridge assigns
  provisional registration-order indices during Phase 2;
  computes the F4 sort permutation at Phase 3; remaps all
  cross-references to final ItemIds at Phase 4 / 5.
- **(b) DefPathHash as canonical key.** Bridge uses
  DefPathHash for cross-references during Phase 2; final
  ItemIds assigned at Phase 4; cross-references resolved
  via the `DefPathHash → ItemId` map after sort.

Both produce identical model-side state. Spec only commits
to **post-sort F4-deterministic ItemIds** as the externally-
observable property.

#### D-12-5.4 -- Spec landing site (new §3.4) [LOCKED]

**Decision.** New subsection §3.4 "Bridge construction-
pipeline order" lands at stage-12 merger; sibling to §3.1
invariants, §3.2 Local IDs, §3.3 validation. Catalogs the
5-phase contract; cross-refs §13 (synthesis), §16.1 (F4
sort), §3.1 invariants 2 / 3 / 9 (item_def_paths /
F4-monotonicity / instance closure-completeness).

---

### D-12-6 -- `render_def_path(ItemId)` stub fill [LOCKED]

**Question.** Stage-1 ships a `render_def_path` rendering
helper and an eager `Item.def_path.rendered: StringId`
cache. Stage-1 stub at `crates/regseq-model/src/pretty.rs`
panics with "stage 12" message. Stage 12 must specify the
fill body.

**Options considered.**
- (a) Re-render at print time using `render_def_path_value
  (&DefPath)`. Forces the canonical-rendering rule to run
  every print.
- (b) **Cache lookup**: read `Item.def_path.rendered`
  directly and resolve via `model.strings`. One-line body;
  trivial.

**Decision.** (b) cache lookup. One-line body:

```rust
pub fn render_def_path(&self, id: ItemId) -> String {
    self.model.strings.resolve(self.model.items[id].def_path.rendered).to_owned()
}
```

**Reasoning.**
- The eager `Item.def_path.rendered` field is the cached
  output of `render_def_path_value(&DefPath)` (the value-form
  helper shipped by stage 1, used at bridge construction
  time).
- §3.3 Phase B "DefPath.rendered matches canonical rendering"
  check (existing) re-derives the canonical rendered form
  from `crate_id + segments` and verifies equality with
  `rendered` — so the cached form is authoritative and
  validation-checked.
- (a) would do redundant work (re-derive on every print)
  for no benefit.

**Methodology rules invoked.**
- *Store the result rustc gives us for free* — eager
  rendering is computed once at construction, cached, and
  re-used. Pretty-printer trusts the cache.

**Prior-art evidence.** N/A — stub-fill-with-cache-lookup is
standard. Stage-3's `render_ty` follows similar pattern at
stage-13 (per D14 deferral).

**Spec cross-reference.** §7.2 (DefPath.rendered eager
canonical rendering); §18.2 per-method ownership table
(updates per D-12-6.3).

**Sub-decisions.**

#### D-12-6.1 -- One-line body [LOCKED]

**Decision.** Body as written above. No re-rendering at print
time; pure cache resolution.

#### D-12-6.2 -- Coexistence with `render_def_path_value(&DefPath)` [LOCKED]

**Decision.** Both helpers coexist with documented division
of labor:
- `render_def_path(ItemId)` — ItemId-indexed convenience
  accessor; short-circuits via the cached `rendered` field.
- `render_def_path_value(&DefPath)` — value-level renderer;
  used for paths NOT attached to an Item (e.g.,
  `PrincipalTraitKey.trait_path` is a DefPathHash for an
  erased Trait def with no Item; or diagnostic dumps of
  synthetic DefPaths during bridge Phase 2 before ItemIds
  are assigned).

Both call the same canonical rendering rule (§7.2 verbose
form); the ItemId form short-circuits via cache.

#### D-12-6.3 -- §18.2 per-method ownership table update [LOCKED]

**Decision.** §18.2 row for `render_def_path` flips from
"deferred to stage 12" to "stage 12; D-12-6". Stage-1's
`render_def_path_value(&DefPath)` row remains unchanged.

---

### D-12-K -- Categorical Item-level rendering deferral to stage 13 [LOCKED]

**Question.** Stage-13 owns body-level rendering per
stage-11 D-11-K (the cascade absorbing D-5-K through D-10-K
+ stage-1 `_N` sigil + stage-10 `bbN` sigil). Stage 12
introduces Item-level metadata (linkage, inline_hint,
is_unsafe, is_naked, link_section, lang-item flags,
opaque_handle_eligible) that needs rendering. Plus the
ItemKind variant rendering, full Item display rendering,
and FunctionData (now FunctionInstanceData under Option B)
pretty-printing. Where does the rendering for these
Item-level constructs land?

**Options considered.**
- (a) Stage 12 owns Item-level rendering. But stage 12 is
  the last spec stage before stage 13 (pretty-printing),
  and stage 13 already owns the rendering style decisions
  for body-level content per D-11-K. Splitting Item-level
  vs body-level rendering across two stages introduces
  inconsistent style.
- (b) **Categorical deferral compositional with D-11-K**:
  all Item-level rendering except `render_def_path`
  (D-12-6) defers to stage 13. Stage 13 owns Item-level +
  body-level rendering as a single coherent rendering-
  decision surface.

**Decision.** (b) categorical deferral.

**Reasoning.**
- Item-level metadata renders coherently with body-level
  content. A Function Item's display couples its source-
  level metadata (linkage, inline_hint) to its rendered
  body (via the Option-B `InstanceKind::Function { fn_sig,
  body }` lookup). Stage 13 already owns body-level
  rendering per D-11-K; coupling Item-level rendering to
  the same stage keeps the rendering surface a single
  coherent design.
- `render_def_path` is special: its body is a trivial
  cache lookup that doesn't make a "rendering style"
  decision. The canonical form is locked at stage 1's
  `render_def_path_value`; stage 12 just retrieves it.
- D-11-K rationale (stage 13 picks rendering style, sigil
  conventions, per-method ownership coherently across
  body-level constructs) extends naturally to Item-level
  renderers.

**Methodology rules invoked.**
- *Spec is mutable* — D-12-K extends D-11-K's cascade
  cleanly; no contradiction with existing rendering
  decisions.
- *Compositional behavior beats special-case flags* —
  Item-level rendering doesn't introduce a parallel "is
  Item display" flag; existing per-method ownership table
  (§18.2) carries the deferral attribution.

**Prior-art evidence.** N/A — the deferral-cascade pattern
is RegseqModel-internal, established by stage-11 D-11-K.

**Spec cross-reference.** §18.2 per-method ownership table
(updates per D-12-K.3); stage-11 D-11-K (cascade
precedent).

**Sub-decisions.**

#### D-12-K.1 -- Deferral list [LOCKED]

**Decision.** The following Item-level rendering items
defer to stage 13:

1. `render_item(ItemId)` (full-Item display renderer).
2. ItemKind variant rendering (per-variant formatting;
   includes `Static { mutability }` per D-12-1.2).
3. `Linkage` enum rendering (4-variant closed enum).
4. `InlineHint` enum rendering (4-variant closed enum).
5. Function-flag rendering (`is_unsafe`, `is_naked`).
6. `link_section: Option<StringId>` rendering decorator.
7. Lang-item flag rendering (3 flags from §8.2).
8. `opaque_handle_eligible` flag rendering decorator.

`render_def_path(ItemId)` is the only non-deferred
Item-level renderer (stays at stage 12 per D-12-6).

#### D-12-K.2 -- Outbound bundle to file at merger close [LOCKED]

**Decision.** At stage-12 merger contract close (per
README §5.5 step 7), file an outbound expectation bundle
in `stage-13/expectations.md` "From stage 12 (per D-12-K)"
with the deferral list above. No predecessor K-tag
retargeting (D-12-K is purely additive — D-11-K already
covered all body-level rendering; D-12-K covers Item-level
rendering, which D-11-K didn't address).

#### D-12-K.3 -- §18.2 per-method ownership table updates [LOCKED]

**Decision.** §18.2 gets new rows for the 7-entry deferral
bundle, each marked "deferred to stage 13 per D-12-K".
Existing `render_def_path` row updates to "stage 12;
D-12-6" per D-12-6.3.

---

## 2. Phase A discussion threads (substantive)

Three substantive Phase A threads warrant standalone
recording for Phase B audit-shape capture (in addition to
their per-D-tag entries above).

### Option B reframe of stage-11 D-11-3 + D-11-5 (D-12-1.4)

The load-bearing finding of stage-12 Phase A. Mid-cluster
discussion surfaced the asymmetry between Function and
non-Function ItemKind variants: stages had committed to
per-mono data on InstanceKind for Static/Const/Struct/Enum/
Union, but stage-11 placed `fn_sig` and `body` on Item under
the `Function` ItemKind variant.

The contradiction surfaced: the rest of the spec
(F4 sort comparator + §13.4 source DefPathHash + §9.3
RegseqInstanceKey) carries Item as pre-mono (1:1 with
rustc DefIds), but stage-3 §15.1 rejects Param/Bound
TyKind variants — so a stored FnSig must be already-
substituted (post-mono). The two views are incompatible.

Stage-11's "4-peer convergence" citation
(rustc, rustc_public, Charon, mrustc) was re-examined and
found to actually support per-Instance body, NOT per-Item
body:
- rustc-internal: `tcx.fn_sig` is generic; body fetched
  per-Instance via `tcx.instance_mir(InstanceKind)`.
- rustc_public: `FnDef::fn_sig()` is generic; **`Instance::
  body() -> Option<Body>`** at `rustc_public/src/mir/mono.rs:53-55`
  (per-Instance) is the actually-cited pattern, structurally
  separate from `CrateItem::body()`.
- Charon: `FunDecl` is post-mono under `--monomorphize`
  (supports per-mono body but doesn't directly distinguish
  Item-side vs Instance-side).
- mrustc: informational only (prior-art evidence didn't
  directly verify body-storage placement).

The two directly-establishing peers (rustc-internal +
rustc_public) carry bodies per-Instance. RegseqModel's
stage-11 framing of body-on-Item was a misreading of the
rustc_public citation (the cited 4-peer pattern was
`Instance::body()`, not `CrateItem::body()`); Option B
restores faithfulness to that evidence.

**Resolution:** lock Option B at D-12-1.4. Reopens stage-11
D-11-3 + D-11-5 (tracked separately; flips to `[REOPENED]`
in `decisions.md` at stage-12 merger close per D-12-1.7).
Stage-11 spec content requires minimal in-place edits
(~50-100 lines of in-place updates to §25.4, §25.5,
§23.6.2, §9.4, §3.1 invariant 10). Stage-11's other
decisions (D-11-1 Body shape, D-11-4 LocalId, D-11-6
visitor, D-11-7 hybrid timing, D-11-8 push API, D-11-K
rendering deferral) are NOT affected.

### Static.mutability placement reopens stage-9 (D-12-1.2)

Stage-1 `spec-snapshot.md:840-841` explicitly placed
mutability on `ItemKind::Static { mutability, .. }`; the
current merged spec.md §9.4 has it on
`InstanceKind::Static`. Investigation revealed the move to
InstanceKind happened during stage-9 closure without
re-litigating stage-1's intent.

Option B's symmetry argument extends here: source-level
metadata on Item; per-mono data on Instance. Mutability is
a source-level keyword choice (`static` vs `static mut`);
statics can't be generic in Rust → 1:1 Item:Instance always
→ no per-mono variation possible.

**Resolution:** lock D-12-1.2 (c). Move mutability to
`ItemKind::Static { mutability: Mutability }`; remove from
`InstanceKind::Static` (now 2-field `{ ty, init }`).
**Reopens a stage-9 decision** — specific D-tag identifying
at stage-12 merger contract close.

### Stage-3-scoped attribute-table wording (D-12-1.5)

Stage-4 `spec-snapshot.md:1287-1299` explicitly scoped the
"no general attribute table on Item" wording to **stage 3
only**, AND committed to stage-14 `DomainAttr` sibling
tables on `RegseqModel` keyed by `ItemId` / `InstanceId` /
`FieldPath` / `Span`. The merged spec.md §8.0 dropped the
"scoped to stage 3" qualifier when absorbing stage-4
content (merge artifact).

**Resolution:** D-12-1.5 restores the framing in §8.0
commentary. Stage 12 adds typed Item fields per D-12-1.3;
stage 14 adds structured DSL property attributes
(`#[rseq::*]`) as RegseqModel sibling tables. No stage-12
schema for DomainAttr.

---

## 3. Cross-references

- `../README.md` -- top-level project mission, methodology
  rules, architectural decisions Q1-Q7.
- `../decisions.md` -- registry of all per-stage D-tags;
  stage-12 entries appended at merger contract close.
- `../discussions.md` -- merged audit trail; stage-12
  contributions absorbed at merger contract close.
- `../spec.md` -- merged top-level spec; stage-12
  contributions land per README §5.9 update guidelines.
- `expectations.md` -- inbound forward refs from stages 1,
  3, 9, 11; stage 4 already CLOSED by D-4-3.
- `memory.md` -- stage-12 invariants (workflow, principles,
  read order, Codex prompt template).
- `status.md` -- stage-12 current-state tracker (locked
  decisions, active focus, next concrete unit of work).
- `prior-art/{rustc,rustc_public,charon,mrustc}.md` --
  per-stage primary-source research framing the D-tags.
- `spec-snapshot.md` -- stage-12 working spec with the
  Phase B contributions.
- `review-log.md` -- stage-12 round-by-round Codex audit
  trail (lazy; created at first review round).

Cross-stage references (load-bearing for Phase B):
- `../stage-1/spec-snapshot.md:82-89` -- stage-1 Rust→C
  lowering reference table; `Linkage`, `InlineHint`,
  `is_unsafe`, `DomainAttr` rows pinned to stage 12 / 14.
- `../stage-1/spec-snapshot.md:820-824` -- stage-1
  `instance.item`'s attributes / ABI / visibility on Item;
  R5.2 binding commitment.
- `../stage-1/review-log.md:391-407` -- R5.2 resolution
  pinning stage-12 typed-field commitment.
- `../stage-3/spec-snapshot.md:884-887, 949, 1022` --
  stage-3 use of `Item.linkage` as load-bearing for
  symbol-mangling decisions.
- `../stage-4/spec-snapshot.md:1279-1299` -- stage-4
  stage-3-scoped attribute-table commentary + stage-14
  forward expectation for `DomainAttr` sibling tables.
- `../stage-11/spec-snapshot.md:§25.4 + §25.5` -- stage-11
  D-11-3 + D-11-5 framing (FnSig + Body on Item; reopened
  by D-12-1.4 Option B).
- `../stage-14/expectations.md:152-157` -- stage-14
  `DomainAttr` parsing commitment.

---

## 4. Outbound expectations to file at merger contract close

Per README §5.5 step 7 + D-12-K.2 / D-12-1.7, the
following outbound expectations are filed at stage-12
merger contract close:

### To `stage-13/expectations.md` "From stage 12 (per D-12-K)"

Categorical Item-level rendering deferral bundle (per
D-12-K.1):

```text
1. render_item(ItemId) -- full-Item display renderer.
2. ItemKind variant rendering -- per-variant formatting hooks.
3. Linkage enum rendering -- 4-variant {Default, NoMangle, Foreign, Weak}.
4. InlineHint enum rendering -- 4-variant {None, Hint, Always, Never}.
5. Function-flag rendering (is_unsafe, is_naked) + lang-item flag rendering
   (is_phantom_data, is_unsafe_cell, is_manually_drop).
6. link_section: Option<StringId> rendering decorator.
7. opaque_handle_eligible flag rendering decorator.
```

Predecessor-stage attribution: stage 12 (Items).

### To `stage-14/expectations.md` "From stage 12"

Carry over stage-1 + stage-4 forward expectations
(unchanged; stage 14 owns the property vocabulary):

```text
1. Q7 intrinsic recognition mechanism builds on
   stage-12's lang-item-recognition pattern (closed-
   vocabulary + tcx.intrinsic_raw(did) + per-callee
   #[rseq::intrinsic("NAME")] attribute lookup at call
   translation). Stage-12's 3-flag set is the template;
   stage-14 generalises.
2. DomainAttr sibling tables on RegseqModel root keyed by
   ItemId / InstanceId / FieldPath / Span per stage-4
   §1287-1299. Stage-12 confirmed the architecture (no
   per-Item field; sibling-table-only). Stage-14 picks
   the per-property keying.
```

### Cross-stage reopenings flagged at merger close

Per D-12-1.7 + D-12-1.2 (c) sub-decision findings:

- **Stage-11 D-11-3** (FnSig location: Item-side) -> flip
  to `[REOPENED]` in `decisions.md` with annotation
  `(revised in stage 12 per D-12-1.4)`. FnSig moves to
  `InstanceKind::Function`.
- **Stage-11 D-11-5** (no BodyId; bodies as `Item.body`)
  -> flip to `[REOPENED]` similarly. Bodies move to
  `InstanceKind::Function.body: Option<Body>`.
- **Stage-9 InstanceKind::Static.mutability placement** ->
  flip to `[REOPENED]` per D-12-1.2 (c). Mutability moves
  to `ItemKind::Static`. Specific stage-9 D-tag identified
  at merger contract close via `decisions.md` review.

---

## 5. F-12 cross-stage findings (informational)

Findings surfaced during stage-12 evidence pass that are
NOT stage-12 D-tags but worth recording for cross-stage
coherence. Action items at merger close.

### F-12-Reify -- ReifyShim payload drift (stage-9 territory)

Rustc's `ty::InstanceKind::ReifyShim` payload has expanded
since stage-9 was written: now `(DefId, Option<ReifyReason>)`
distinguishing vtable-vs-fn-pointer reification contexts
(per `rustc_middle/src/ty/instance.rs:40-60` documentation
block). This may break stage-9's `RegseqInstanceKey`
injectivity invariant: two `ReifyShim` instances with
different `ReifyReason` would share `(rank, did, args)` and
collapse.

**Stage-12 disposition:** stage 12 is Item-level work; this
is an Instance-level concern (stage-9 territory). No
stage-12 schema change.

**Action at merger close:** flag in stage-12 outbound
findings for stage-9's next review cycle. If codegen / phase-2
device pressure surfaces a real `ReifyReason` collision (two
distinct shims sharing `(did, args)`), stage 9 reopens to
extend `FunctionInstanceFlavor::ReifyShim` with the
`ReifyReason` discriminator or extend `RegseqInstanceKey`
similarly. Otherwise document in `../status.md` "Deferred
items (cross-stage)" and leave to future stage-9 review.

**Cross-refs:**
- `rustc_middle/src/ty/instance.rs:40-185`.
- spec.md §9.3 (RegseqInstanceKey).
- spec.md §9.4 (FunctionInstanceFlavor::ReifyShim).

---

## Stage 13 (Pretty-printing)

Distilled record of the design dialogue producing the
stage-13 pretty-printer spec. Each entry captures the
question raised, options considered, the decision, the
reasoning chain, methodology rules invoked, prior-art
evidence, and cross-references to
`pretty-printer-snapshot.md` (the new top-level spec
file's draft, per D-13-9) and `spec-snapshot.md`
(working copy of `../spec.md` for the small in-place
edits per D-13-9.2). Round-by-round Codex audit trail
lives in `review-log.md` (created on first review round).

Status of decisions in this file mirrors
`pretty-printer-snapshot.md` (the bulk pretty-printer
spec) plus minimal in-place edits to `spec-snapshot.md`
at §15 / §18 / §19.8 / §20.6 / §21.8 / §22.8 / §23.9 /
§24.9 / §25.x (stub + cross-references per D-13-9.2)
and `../README.md` at §4 / §5.5 step 5 / §5.9 rule 6
(methodology extension per D-13-9.3).

D-tag convention: prefixed `D-13-K` form per
`../README.md` §5.2.

Status tags:
- `[LOCKED]` -- audit-shape recorded; per-stage Phase A
  closed; ready for Phase B Codex review.
- `[CLOSED-BY-ABSORPTION]` -- subsumed into a
  later D-tag during Phase A; retained for history.
- `[REOPENED]` -- a later sub-decision or sibling D-tag
  forced a revision; superseded entry kept for history.

Phase A closed 2026-05-06 with all 8 main D-tags +
41 sub-decisions LOCKED + 1 D-tag closed-by-absorption
(D-13-8 → D-13-9.6) across a single combined cluster.

Two mid-cluster substantive threads surfaced during
discussion:
1. The **building-block-vs-composer architectural
   division** (D-13-4.6) -- prompted by the user's
   question "shouldn't we be rendering Instances
   instead of Items?"; resolved by clarifying that
   stage 13 owns NEW rendering BUILDING BLOCKS
   (`render_ty`, `render_item`, body-level renderers),
   inherited building blocks (`render_def_path`
   stage 12 per D-12-6, `render_def_path_value` +
   `render_span` stage 1) are consumed but not
   stage-13-owned, and stage 14 owns COMPOSERS
   (`render_const` per D-4-K, `render_instance`,
   top-level `render`). `render_item` stays as a
   public stage-13 API entry per D-12-K.1.
2. The **pretty-printer spec location reframe**
   (D-13-9 emergent) -- prompted by the user's
   suggestion "could we create a separate
   pretty-print.md instead of adding to spec.md?";
   resolved by promoting it to a new D-13-9, locking
   the full pretty-printer spec into a top-level
   companion file `pretty-printer.md` (matching the
   project's existing `architecture.md` / `decisions.md`
   / `regseq-background.md` / `rustc-references.md`
   side-file pattern). The methodology rule extension
   to README §5.9 / §5.5 / §4 is part of D-13-9.3.

The reframe under D-13-9 absorbs D-13-8 (sigil-table
review + ownership-table consolidation): both land as
sections inside the new `pretty-printer.md` (§3 sigils,
§8 ownership table) rather than as in-place edits to
`spec.md` §18.

---

## 1. Decision discussions

### D-13-1 -- Master rendering style + method shape [LOCKED]

**Question.** Stage 13's broadened scope (per stage-11
D-11-K + stage-12 D-12-K cascades) covers 20 inbound
items as a coherent rendering-decision surface. Stage 13
must lock the master rendering style ONCE for the whole
surface (D-11-K's load-bearing rationale) -- the textual
form (Rust-source-style vs constructor-style vs
C-source-like) and the method shape (context-bearing
printer vs free fns vs Display impls).

**Options considered.**

Aspect A1 (textual rendering style):
- (a) Rust-source-style with sigil decorations (`&'a mut Vec<u8>`).
  4-peer prior-art convergence: rustc-internal
  `mir/pretty.rs` + `ty/print/pretty.rs`; rustc_public
  `mir/pretty.rs`; Charon `pretty/fmt_with_ctx.rs`;
  project's own `def_path.rs` / `render_def_path_value`
  precedent. Stage-3 D14 lean preserved as input.
- (b) Constructor-style (`Ref(Region("a"), Mut, Adt(Vec, [u8]))`).
  mrustc `operator<<` debug-log path
  (`prior-art/mrustc.md`) -- counter-precedent.
- (c) C-source-like declarator (`Vec<u8> *`). Awkward
  for `dyn`, generics, lifetimes. No project precedent.
- (d) Hybrid (Rust-source types, MIR-form bodies).
  No primary precedent.

Aspect A2 (method shape):
- (a) Methods on `PrettyPrinter<'a>` returning `String`.
  Already shipped at stage 1
  (`crates/regseq-model/src/pretty.rs`); Q5 pinned
  context-bearing printer (IDs need `&self.model`
  lookup).
- (b) Free `pretty_X(&mut W: io::Write, ...)` fns.
  rustc_public structural pattern; closest external
  analog but RegseqModel already shipped (a).
- (c) `Display` / `Debug` impls on IR types.
  rustc-internal pattern. Incompatible with Q5
  (printer needs context for ID resolution).

**Decision.** **(a) + (a)**: Rust-source-style with
sigil decorations rendered via `PrettyPrinter<'a>`
methods returning `String`.

**Reasoning.**
- A1 (a) has 4-peer prior-art convergence: rustc-internal,
  rustc_public, Charon, and the project's own
  `render_def_path_value` precedent (`crate::module::Item`,
  `{closure#0}`) all commit to Rust-source-style. mrustc's
  dual-rendering split (sigil-style for MIR dumps +
  constructor-form for debug logs; the same value
  rendering two different ways) is the explicit
  anti-pattern that motivates picking ONE style.
- A2 (a) is structurally mandated by Q5 (context required
  for ID resolution) and already shipped. A2 (c) is
  ruled out by Q5; A2 (b) is the closest external
  analog (rustc_public's structural pattern) but
  RegseqModel's shipped methods-on-PrettyPrinter
  shape is the natural carry-forward. Per-method
  dispatch is lifted from rustc_public's free-fn
  pattern at the API-shape level (one method per
  public renderer, per-arm internal match) without
  changing return type.
- The synthesis takes rustc-internal's textual choices
  and rustc_public's structural pattern (per-method
  dispatch on `PrettyPrinter` vs Debug-impls), yielding
  a coherent surface continuous with the project's
  existing scaffold.

**Methodology rules invoked.**
- *Compositional behavior beats special-case flags* --
  the master style applies uniformly; per-renderer style
  dissent is rejected by construction.
- *Spec is mutable* -- stage-3 D14 deferred the renderer-
  style call to stage 13; stage 13 makes the call with
  fully-populated inputs (FnSig per stage 11; Item /
  DefPath per stage 12).
- *Snapshot stability* (stage-13 §2 principle) --
  the choice subordinates per-renderer freedom to
  whole-surface coherence so snapshot tests remain
  stable when individual renderers refine.

**Prior-art evidence.**
- `prior-art/rustc.md:23-49` (Per-D-tag relevance D-13-1):
  rustc-internal hybrid Rust-source-style with sigil
  decorations; primary-source citations at
  `rustc_middle/src/mir/pretty.rs:1083-1180` (Rvalue
  arms), `:1251-1260` (Operand),
  `rustc_middle/src/ty/print/pretty.rs:712`
  (`pretty_print_type` entry).
- `prior-art/rustc_public.md:18-44` (D-13-1): same
  Rust-source-style; free `pretty_X` fns on `&mut W:
  io::Write` -- closest structural analog;
  primary-source citations at `rustc_public/src/mir/
  pretty.rs:38, 90, 128, 171, 204, 229, 326, 347, 397`.
- `prior-art/charon.md:30-42` (D-13-1): Rust-source-style
  via AstFormatter trait + FmtWithCtx trait + WithCtx
  wrapper threading `FmtCtx { translated, generics,
  locals, indent_level }` context. Primary source in
  `charon/charon/src/pretty/fmt_with_ctx.rs`.
- `prior-art/mrustc.md:23-44` (D-13-1): counter-precedent
  -- `MirDumper` emits Rust-source-leaning sigil-style
  for MIR dumps but parallel `operator<<` overloads
  emit constructor-form for debug logs (dual-rendering
  split anti-pattern). Primary sources in
  `mrustc/src/mir/dump.cpp` and `mrustc/src/mir/mir.cpp`.

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §2 (Master rendering
  style) -- the canonical lock for the whole surface.
- `pretty-printer-snapshot.md` §1 (Mission and consumer
  model) -- the consumer-facing rationale.

**Sub-decisions.**

#### D-13-1.1 -- Textual style: Rust-source-style with sigil decorations [LOCKED]

**Decision.** Rust-source-style with sigil decorations
per A1 (a). Reads continuously for engineers familiar
with rustc's `--emit=mir` output and the Rust-source
language; coherent with project's existing
`render_def_path_value` precedent.

#### D-13-1.2 -- Method shape: PrettyPrinter methods returning String [LOCKED]

**Decision.** Methods on `PrettyPrinter<'a>` returning
`String`. Existing scaffold preserved; Q5 context-required
constraint respected. v2 `&mut String` overload is
non-breaking extension if hot-path consumer surfaces;
no v1 evidence demands it.

#### D-13-1.3 -- Stage-3 D14 closes by absorption [LOCKED]

**Decision.** Stage-3 D14 (renderer-style call) closes
by absorption into D-13-1. The historical D14 entry in
stage-3 discussions (preserved as deferred-decision
context) is referenced as preserved-input rationale;
D-13-1's Phase B audit-shape capture cites it. At stage-13
merger close, D14's status flips to
`[CLOSED-BY-ABSORPTION]` in `decisions.md` with
annotation `(absorbed into D-13-1 per stage 13)`.

---

### D-13-2 -- Body envelope + indentation + sigil placement [LOCKED]

**Question.** Per D-13-1's master Rust-source-style,
the body envelope shape and indentation policy must lock
once for the entire body-level rendering surface. Stage
13 also wires the `_N` (LocalId) and `bbN` (BlockId)
sigil rendering sites at first consumer per stage-1 +
D-10-K.2 + D-11-K. The envelope's boundary with
`render_item` (D-13-4 Item-level shape) needs explicit
naming.

**Options considered.**

Aspect B1 (body envelope shape):
- (a) rustc-converged: `fn $path(_1: ArgTy, _2: ArgTy)
  -> RetTy { let mut _0: RetTy; let _3: T; ...; bbN: {
  stmt; stmt; terminator -> [edges]; } }`. rustc-internal
  `mir/pretty.rs:539, :612, :714`; rustc_public
  `mir/pretty.rs:38`; Charon precedent.
- (b) Implicit locals (omit declarations; infer types
  at use sites). No primary precedent; loses type
  information needed for `place_ty` walks at snapshot
  stability.
- (c) Body-only without function header. Splits envelope
  ownership across D-13-2 + D-13-4; incoherent.

Aspect B2 (indentation):
- (a) 4-space INDENT, 8-space stmt indent (rustc-converged
  `INDENT = "    "` at `mir/pretty.rs:19`).
- (b) 2-space (project Rust style elsewhere; not
  pretty-printing precedent).
- (c) Configurable. Adds surface area without evidence.

Aspect B3 (sigil wirings):
- (a) Inline at first consumer; `format!("_{}", id.index())`
  for `LocalId`, `format!("bb{}", id.index())` for
  `BlockId`. rustc + rustc_public both inline these in
  Local / BasicBlock Debug impls.
- (b) Dedicated helper methods on `PrettyPrinter`. Adds
  API surface for textually trivial output.

**Decision.** **B1 (a) + B2 (a) + B3 (a)**: rustc-converged
envelope, 4-space INDENT (8-space stmt indent inside
`bbN: {}`), inline sigil wiring at first consumer.

**Reasoning.**
- B1 (a) preserves type information that snapshot
  tests + `place_ty` walks need; rustc + rustc_public
  + Charon converge on this envelope so RegseqModel
  pretty-print output reads continuously for engineers
  familiar with `--emit=mir`.
- B2 (a) is direct rustc adoption; matches `--emit=mir`
  baseline indentation.
- B3 (a) is the §2 "Compositional behavior beats
  special-case flags" principle (no parallel
  `render_local_sigil` / `render_block_sigil` helpers
  duplicating trivial output).

**Methodology rules invoked.**
- *Compositional behavior beats special-case flags*
  (B3 sigil inlining).
- *Compiler-theory-first* (envelope and indentation
  inherited from rustc + rustc_public converged
  evidence rather than re-derived).
- *C semantics with information preservation* --
  type info on local declarations preserves what
  rustc gives us for free at consumer sites.

**Prior-art evidence.**
- `prior-art/rustc.md:51-67` (D-13-2 envelope +
  indentation): primary-source citations at
  `rustc_middle/src/mir/pretty.rs:539
  (write_mir_intro)`, `:612 (write_mir_sig)`,
  `:714 (write_basic_block)`, `:19 (INDENT)`,
  `:21 (ALIGN = 40)`.
- `prior-art/rustc_public.md:46-55` (D-13-2): same
  envelope; primary-source citations at
  `rustc_public/src/mir/pretty.rs:38 (function_body)`,
  `:91 (statement INDENT)`, `:173 (terminator INDENT)`.
- `prior-art/charon.md:44-52` (D-13-2): AstFormatter
  threads `indent_level` in `FmtCtx`; same envelope
  shape. Primary source in
  `charon/charon/src/pretty/fmt_with_ctx.rs`.

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §3 (Body envelope,
  indentation, sigil conventions; absorbs §18.1
  sigil table per D-13-9).

**Sub-decisions.**

#### D-13-2.1 -- Body envelope shape [LOCKED]

**Decision.** rustc-converged envelope `fn $path(...)
-> Ret { let _N: T; ...; bbN: { stmt; stmt; terminator
-> [edges]; } }`.

#### D-13-2.2 -- Indentation = 4-space INDENT, 8-space stmt indent [LOCKED]

**Decision.** `INDENT = "    "` (4 spaces); statements
inside `bbN: {}` rendered at 8-space indent (envelope's
4 + INDENT step 4).

#### D-13-2.3 -- Sigil wirings inline at first consumer [LOCKED]

**Decision.** No dedicated `render_local_sigil` /
`render_block_sigil` methods. `LocalId` renders as
`format!("_{}", id.index())` at `render_place` call
sites + locals declaration block. `BlockId` renders as
`format!("bb{}", id.index())` at `render_basic_block`
header + successor labels in `render_terminator`.

#### D-13-2.4 -- D-13-2 / D-13-4 boundary [LOCKED]

**Decision.** Function header `fn $path(...) -> Ret`
(plus all Item-level attribute prefixes per the D-12-K
bundle: linkage, inline_hint, link_section, is_unsafe,
is_naked, lang-item flags, opaque_handle_eligible)
belongs to **D-13-4** (Item-level shape). D-13-2 owns
**body content only** -- the `{ let _N: T; ...; bbN: {
... } }` portion. D-13-4's `render_item` composes
attribute block + kind-keyword + path; the per-Instance
body content (inside `{ }`) is composed by stage-14
`render_instance` which delegates to D-13-2's envelope
and the body-level renderers.

#### D-13-2.5 -- Locals block formatting [LOCKED]

**Decision.** One declaration per line. `let mut _N:
<render_ty(T)>;` for mutable locals (per stage-11 D-11-2
`LocalDecl.mutability == Mutability::Mut`); `let _N:
<render_ty(T)>;` for immutable. RETURN_PLACE always
emits as `let mut _0: <ret_ty>;` (rustc-converged --
return place is logically mutable since the body
assigns to it). Debug name (per stage-11 D-11-2
`LocalDecl.debug_name: Option<StringId>`) renders as
standalone `debug NAME => _N;` lines AFTER the locals
declarations block, gated by D-13-3.4
`include_debug_names`.

---

### D-13-3 -- Span-rendering mode (and debuginfo gating) [LOCKED]

**Question.** Spans (per stage-1 / `Span` schema) and
debug names (per stage-11 D-11-2
`LocalDecl.debug_name`) are debuginfo classes that
inflate snapshot tests when included. Stage-13 §2
principle ("Snapshot stability is the load-bearing
consumer") rejects always-on rendering. The choices:
default mode, toggle mechanism (since rustc's compile-
time `-Z` flag pattern doesn't translate), span shape
when toggle on, debug-name shape when toggle on,
synthetic-span suppression policy.

**Options considered.**

C1 (default mode):
- (a) Default-off, opt-in via toggle (rustc-internal
  pattern; `mir/pretty.rs:51-58`
  `PrettyPrintMirOptions.include_extra_comments` gated
  by `-Z mir-include-spans`).
- (b) Always-off, no toggle infrastructure (rustc_public
  pattern; `mir/pretty.rs` ships zero span content).
- (c) Always-on (no primary precedent; rejected per §2
  snapshot stability principle).

C2 (toggle mechanism):
- (a) `PrettyPrinter` option-field set at construction
  via `with_options(model, opts)` constructor; existing
  `new(model)` keeps signature, defaults all-off.
- (b) Per-method opt-in flag.
- (c) Thread-local context (rustc_public-style
  `with(|cx| ...)`).

C3 (span shape when toggle on):
- (a) Trailing inline `// <render_span>` aligned to
  col 40 (rustc-internal `mir/pretty.rs:21 ALIGN = 40`;
  `:736-751` call sites).
- (b) Out-of-line span block at body end.
- (c) Inline non-aligned.

C4 (debug-name shape when toggle on):
- (a) Standalone `debug NAME => _N;` lines after
  locals declarations, gated by separate
  `include_debug_names: bool` field
  (rustc `--emit=mir`-converged + finer control).
- (b) Same flag as spans (`include_extra_comments`-style
  umbrella per rustc).
- (c) Always-on (debug-name rendering is deterministic
  per-source, but variable across rustc nightly versions
  if `var_debug_info` extraction changes).

C5 (synthetic-span suppression):
- (a) Suppress when `SourceFileId::SYNTHESIZED`
  (no trailing comment for spans on synthesised
  constructs).
- (b) Render literal `<synthesized>:1:1: 1:1` for
  uniformity.

**Decision.** **C1 (a) + C2 (a) + C3 (a) + C4 (a) +
C5 (a)**:
- Default-off, opt-in via `PrettyPrintOptions`
  option-field constructed via
  `PrettyPrinter::with_options(model, opts)`.
- Existing `PrettyPrinter::new(model)` keeps signature,
  defaults options to all-off.
- When `include_spans=true`: trailing inline
  `// <render_span>` aligned to col 40. Spans on
  synthesised constructs (`SourceFileId::SYNTHESIZED`)
  suppress (no trailing comment).
- When `include_debug_names=true`: standalone
  `debug NAME => _N;` lines after locals declarations.
- `PrettyPrintOptions` shape: `{ include_spans: bool,
  include_debug_names: bool }` with implicit `Default`
  impl (all-off).

**Reasoning.**
- C1 (a) subsumes C1 (b) at zero cost: the toggle
  infrastructure is a few lines, and snapshot tests
  default-off keeps insta baselines compact while
  diagnostic / debug consumers can opt in.
- C2 (a) is the idiomatic Rust approach for the
  rustc compile-time-flag pattern; backwards-compatible
  (existing constructor preserved); testable; no hidden
  state.
- C3 (a) inherits rustc-internal's exact format directly
  (the `render_span` method already shipped at stage 1
  produces `path:l:c: l:c` matching rustc's
  `span_to_diagnostic_string` shape).
- C4 (a) separates spans from debug names because
  their snapshot-stability profiles differ: debug
  names from `var_debug_info` can drift across rustc
  versions even on stable source. Finer control at
  minimal cost (one extra bool field).
- C5 (a) keeps synthetic-Item snapshots clean; the
  literal `<synthesized>:1:1: 1:1` carries no positional
  information so suppression doesn't lose data, and
  synthetic spans are bridge-introduced (vtable shims,
  drop glue, ConstMemoryStatic) -- they would otherwise
  proliferate noise in test fixtures involving
  bridge synthesis.

**Methodology rules invoked.**
- *Snapshot stability* (stage-13 §2 principle) --
  default-off keeps insta baselines compact and
  unaffected by debuginfo drift.
- *Compositional behavior beats special-case flags* --
  the toggle is the single mechanism (option-field +
  default impl); no parallel "is_debug_name_field" /
  "skip_synthetic_spans" flag at per-renderer sites.

**Prior-art evidence.**
- `prior-art/rustc.md:69-87` (D-13-3): primary-source
  citations at `rustc_middle/src/mir/pretty.rs:51-58
  (PrettyPrintMirOptions.include_extra_comments)`,
  `:736-751 (call sites)`, `:21 (ALIGN = 40)`.
- `prior-art/rustc_public.md:57-67` (D-13-3): zero span
  content at all (rustc_public's `mir/pretty.rs` ships
  no span output), evidencing the always-off candidate.
- `crates/regseq-model/src/pretty.rs:67-80`:
  `render_span` already produces
  `path:lo_line:lo_col: hi_line:hi_col` form; D-13-3
  wires the gating layer on top.

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §4 (Span-rendering and
  debuginfo modes).

**Sub-decisions.**

#### D-13-3.1 -- Default mode: default-off, opt-in via toggle [LOCKED]

#### D-13-3.2 -- Toggle mechanism: PrettyPrintOptions struct + with_options constructor [LOCKED]

`PrettyPrintOptions { include_spans: bool,
include_debug_names: bool }` with implicit `Default`
impl producing all-false. Constructor:
`PrettyPrinter::with_options(model, opts)` returns
`PrettyPrinter<'a>`. Existing `PrettyPrinter::new(model)`
preserved unchanged (delegates to `with_options(model,
PrettyPrintOptions::default())`).

#### D-13-3.3 -- Span shape (when toggle on) [LOCKED]

Trailing inline aligned to col 40: `<stmt>; // <render_span>`.
Direct rustc adoption.

#### D-13-3.4 -- Debug-name shape (when toggle on) [LOCKED]

Standalone `debug NAME => _N;` lines AFTER locals
declarations block. Per-stage-11 D-11-2's optional
`LocalDecl.debug_name: Option<StringId>` -- only locals
with `Some(name)` contribute debug lines; locals with
`None` are silent.

#### D-13-3.5 -- Synthetic-span suppression [LOCKED]

When `include_spans=true`, spans on `SourceFileId::SYNTHESIZED`
suppress (no trailing comment). Synthetic constructs
(vtable shims, drop glue, ConstMemoryStatic per stage-13
§13 + §6 family) thus render clean even with the toggle
on.

---

### D-13-4 -- Item-level rendering shape [LOCKED]

**Question.** Per D-12-K's 7-entry bundle (`render_item`,
ItemKind variant rendering, Linkage rendering, InlineHint
rendering, Function-flag + lang-item-flag rendering,
link_section rendering, opaque_handle_eligible rendering),
stage 13 must lock the Item-level rendering shape: the
attribute-decorator syntax, the default-suppression
policy, the per-flag rendering form (rustc source-faithful
where the rustc attribute exists), the per-ItemKind
variant rendering, and the `render_item` envelope
formatting.

The user's mid-cluster question "shouldn't we be
rendering Instances instead of Items?" surfaced an
architectural division concern, captured as
sub-decision D-13-4.6.

**Options considered.**

E1 (attribute syntax):
- (a) Rust-source `#[attr]` / `#[attr = "..."]` /
  `#[attr(...)]` form. Direct D-13-1 consequence.
- (b) C-style `[attr=value]`. Clashes with D-13-1.
- (c) Bare key=value. No precedent.

E2 (default-suppression):
- (a) Suppress all default/false-valued fields.
  Stage-13 §2 snapshot-stability principle.
- (b) Always render every field. Predictable but
  noisy; defeats §2.
- (c) Tunable via `PrettyPrintOptions`. No v1 evidence.

E3 (per-flag rendering): per the F-3 table (see
Decision below).

E4 (per-ItemKind rendering): kind-keyword + DefPath
form, per the F-4 table (see Decision below).

E5 (`render_item` envelope formatting):
- (a) Multi-line when ≥1 attribute renders, one per
  line; kind-keyword + path on final line. One-line
  bare `kind $path` when zero attributes survive
  suppression.
- (b) Width-driven (one-line when total ≤ 80 cols).
- (c) Per-call mode flag.

**Decision.** **E1 (a) + E2 (a) + E3 (table) + E4
(table) + E5 (a)**:
- All attribute prefixes use Rust-source `#[...]` form.
- All default-valued fields suppress (Linkage::Default,
  InlineHint::None, link_section: None, is_unsafe:
  false, is_naked: false, lang-item flags all-false,
  opaque_handle_eligible: false).
- Per-flag rendering: see table in §1.4.3 of
  `pretty-printer-snapshot.md` -- rustc source-faithful
  forms (`#[no_mangle]`, `#[inline(always)]`,
  `#[link_section = "..."]`, `#[lang = "..."]`,
  `#[naked]`); RegseqModel-internal
  `#[opaque_handle]` for the bridge-outcome flag.
  `is_unsafe = true` rendered as `unsafe` keyword
  prefix on `fn` (Rust-source modifier keyword form).
- Per-ItemKind kind-keyword + DefPath: `[unsafe ]fn
  $path` / `static [mut ]$path` / `const $path` /
  `struct $path` / `enum $path` / `union $path`.
- Multi-line layout when ≥1 attribute renders (one
  attribute per line); one-line bare `kind $path`
  when zero attributes survive suppression.

**Reasoning.**
- E1 (a) is direct D-13-1 inheritance.
- E2 (a) is the §2 snapshot-stability principle:
  default-valued fields rendering would tickle every
  snapshot when bridge populates a new field default.
  Common case is the cleanest case.
- E3's per-flag rendering favours rustc source-faithful
  forms because they're the most readable and continue
  the project's `render_def_path_value` precedent of
  matching what rustc renders. The one bridge-outcome
  flag (opaque_handle_eligible) gets a RegseqModel-
  internal annotation form `#[opaque_handle]` (no
  rustc source equivalent). The
  `unsafe` keyword (not attribute) for `is_unsafe`
  matches rustc source modifier convention; only `fn`
  accepts the keyword (statics aren't unsafe; `unsafe
  static` is non-syntactic).
- E4 mirrors rustc's HIR pretty-printing: kind-keyword
  + path is the source-level wrapper. ADT layouts and
  function signatures live on Instance per Option B,
  so render_item shows pre-mono Item-level data only.
- E5 (a) is a simple rule with no width parameter;
  multi-line for instrumented Items keeps snapshot
  diffs stable when an attribute is added/removed.
  One-line for bare Items keeps the common case
  compact.

**Methodology rules invoked.**
- *Compositional behavior beats special-case flags* --
  default-suppression falls out of "render only if
  non-default"; no parallel `should_render: bool`
  per Item.
- *C semantics with information preservation* --
  Item-level metadata (linkage, inline hint, link
  section, is_unsafe / is_naked, lang-item flags,
  opaque_handle_eligible) is the source-level alphabet
  RegseqModel consumes for C codegen; the rendering
  preserves it visibly when set, suppresses when
  default.
- *Snapshot stability* (§2 principle).

**Prior-art evidence.**
- `prior-art/rustc.md:89-101` (D-13-4): rustc HIR +
  MIR pretty-printer's attribute prefix conventions
  (`#[no_mangle]`, `unsafe fn`, `#[inline(always)]`);
  primary-source pattern at `rustc_middle/src/mir/
  pretty.rs:612 (write_mir_sig)` for the function-
  header form; HIR attribute-printing infrastructure
  (general rustc precedent; not single-line citable).
- `prior-art/rustc_public.md:69-79` (D-13-4): doesn't
  directly cover Item-level rendering (renders bodies,
  not Items via `mir/pretty.rs`); serves as evidence
  that Item-level rendering shape has no fixed
  external precedent beyond rustc-source-faithful
  patterns.
- `prior-art/charon.md:64-80` (D-13-4): Charon's
  Item rendering uses Rust-source-faithful attribute
  forms via the FmtCtx + WithCtx wrapper. Primary
  source in `charon/charon/src/pretty/fmt_with_ctx.rs`.

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §5 (Item-level rendering
  shape).

**Sub-decisions.**

#### D-13-4.1 -- Attribute syntax = Rust-source `#[...]` form [LOCKED]

#### D-13-4.2 -- Default-suppression for all default/false fields [LOCKED]

#### D-13-4.3 -- Per-flag rendering table [LOCKED]

| Field | Rendered form |
|-------|---------------|
| `linkage = NoMangle` | `#[no_mangle]` |
| `linkage = Foreign` | `#[foreign]` |
| `linkage = Weak` | `#[linkage = "weak"]` |
| `linkage = Default` | suppressed |
| `inline_hint = Hint` | `#[inline]` |
| `inline_hint = Always` | `#[inline(always)]` |
| `inline_hint = Never` | `#[inline(never)]` |
| `inline_hint = None` | suppressed |
| `is_unsafe = true` | `unsafe` keyword on kind-keyword (`unsafe fn`) |
| `is_unsafe = false` | suppressed |
| `is_naked = true` | `#[naked]` |
| `is_naked = false` | suppressed |
| `is_phantom_data = true` | `#[lang = "phantom_data"]` |
| `is_unsafe_cell = true` | `#[lang = "unsafe_cell"]` |
| `is_manually_drop = true` | `#[lang = "manually_drop"]` |
| any lang-flag = false | suppressed |
| `link_section = Some("...")` | `#[link_section = "..."]` |
| `link_section = None` | suppressed |
| `opaque_handle_eligible = true` | `#[opaque_handle]` |
| `opaque_handle_eligible = false` | suppressed |

#### D-13-4.4 -- Per-ItemKind variant rendering [LOCKED]

| ItemKind | Rendered form |
|----------|---------------|
| `Function` | `[#[attrs]\n][unsafe ]fn $path` |
| `Static { mutability: Not }` | `[#[attrs]\n]static $path` |
| `Static { mutability: Mut }` | `[#[attrs]\n]static mut $path` |
| `Const` | `[#[attrs]\n]const $path` |
| `Struct` | `[#[attrs]\n]struct $path` |
| `Enum` | `[#[attrs]\n]enum $path` |
| `Union` | `[#[attrs]\n]union $path` |

`$path` resolves through `render_def_path(ItemId)`
(stage-12 D-12-6; cached at `Item.def_path.rendered`).

#### D-13-4.5 -- render_item envelope formatting [LOCKED]

Always multi-line when ≥1 attribute renders, one
attribute per line; kind-keyword + path on final line.
One-line bare `kind $path` when zero attributes survive
suppression.

#### D-13-4.6 -- Building-block-vs-composer architectural division [LOCKED]

**Question (mid-cluster surfacing; user-prompted).**
Should `render_item` be the headline pretty-printer
entry, or should `render_instance` be?

**Decision.** The architectural division is:
- **Stage 13 owns NEW rendering BUILDING BLOCKS:**
  `render_ty`, `render_item`, all body-level renderers
  (D-11-K bundle).
- **Inherited building blocks** (consumed by composers
  but not stage-13-owned): `render_def_path` (stage 12
  per D-12-6), `render_def_path_value` + `render_span`
  (stage 1, shipped).
- **Stage 14 owns rendering COMPOSERS:** `render_const`
  (D-4-K), `render_instance` (composes `render_item`'s
  wrapper output with per-Instance content -- FnSig +
  body + layout + init + value), top-level `render`
  (iterates Instances, calls `render_instance`).

`render_item` stays a **public** API entry per
D-12-K.1; it's a useful Item-table summary view ("list
all Items in the model"), AND the building block that
`render_instance` composes. Making it private (option
b) would lose the Item-table summary use case.
Moving its ownership to stage 14 (option c) would
require reopening stage-12 D-12-K -- heavyweight for a
naming question.

**Reasoning.** The headline consumer is `render_instance`
(per Q2 eager mono baking, codegen iterates Instances),
but stage 13's role IS the building blocks because:
(i) Items are stage-12-territory schema; rendering style
for the wrapper is naturally stage-13 master-style
territory; (ii) D-11-K's "rendering style ONCE for the
whole surface" rationale extends compositionally to
Item-level rendering (D-12-K compositionally extending
D-11-K is precisely this principle); (iii) Stage 14
inherits D-13-1 master style without re-litigating.

A typical Function Instance rendering composes:
```
[render_item output]: attribute block + `[unsafe ]fn $path`  (stage 13, D-13-4)
+ [render_ty over FnSig]: `(_1: T1, _2: T2) -> Ret`           (stage 13, D-13-5)
+ [body envelope + body-level renderers]: `{ let _N: T; bbN: { ... } }`  (stage 13, D-13-2 + D-13-6)
```

`render_item` produces only the first row; `render_instance`
(stage 14) composes all three.

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §1 (Mission and consumer
  model -- the architectural division);
  `pretty-printer-snapshot.md` §5 (Item-level rendering
  shape).

---

### D-13-5 -- `render_ty` per-arm specifications [LOCKED]

**Question.** Stage 3's closed 19-variant `Ty` enum
(`spec.md:2999-3019`) is the surface `render_ty` covers
under D-13-1 master Rust-source-style. Stage 13 must
specify per-variant rendering for each of the 19 variants,
the `Ty::Dynamic` rendering (which integrates the
synthesised vtable's DefPath plus auto-traits), the
`render_generic_args` helper (used by `FnDef`, `Closure`,
`Struct`, `Enum`, `Union`), and the carve-outs deferred
from D-13-1 (lifetime suppression, default-value
suppression in composites).

**Options considered.** Per-variant rendering forms have
strong rustc + Charon convergence; the genuine choices
concentrate at `Ty::Dynamic` (F2) and `render_generic_args`
(F3). See `pretty-printer-snapshot.md` §6 for the
canonical per-arm table.

F2 (`Ty::Dynamic` rendering):
- (a) `dyn <render_def_path(vtable_item)> + AutoTrait
  + AutoTrait`. Vtable Item's DefPath encodes the trait
  + impl per stage-3 §13.4.
- (b) `dyn vtable@<render_ty(vtable_ty)>` (treats
  vtable opaque; loses auto-trait readability).
- (c) Decode principal trait's name from vtable Item's
  DefPath and render `dyn <Trait> + Auto + Auto`
  (cleanest output but requires DefPath-segment
  decoding fragile to §13.4 encoding changes).

F3 (`render_generic_args` form):
- (a) `<T1, T2, K1>` Rust-source turbofish-tail form;
  empty `args` renders empty string. rustc-converged.
- (b) Always show `<>` even when empty. Clutters
  non-generic items.
- (c) `::<T1, T2, K1>` turbofish prefix. Rustc uses
  turbofish only in expression position; type position
  uses bare `<...>`.

**Decision.** **F1 (per-variant table) + F2 (a) +
F3 (a) + F4 (lifetime carve-out non-issue + suppress
empty composites)**:
- 19 per-variant render specs per `pretty-printer-
  snapshot.md` §6.
- `Ty::Dynamic` renders `dyn <render_def_path(vtable_item)>
  + AutoTrait + AutoTrait` with auto-traits in
  stage-3 §10.1 sorted order; empty `auto_traits: []`
  omits the `+` suffix.
- `render_generic_args` produces bare `<...>` non-empty,
  empty string when args empty. Per-arg dispatch:
  `Type` → `render_ty`; `Const` → `render_const`
  (forward-call to stage-14 D-4-K); `Lifetime` →
  none (Q2 region erasure).
- Lifetime suppression non-issue: RegseqModel's
  `Ty::Ref` carries `Provenance` (shared/unique borrow
  flavor), NOT rustc-style lifetime regions per
  stage-3 §10.1.
- Default-value suppression for empty composites:
  empty `auto_traits: []` (no `+` suffix); empty
  `GenericArgs` (no `<>`); `Tuple { fields: [] }` →
  `()`.

**Reasoning.**
- F1 directly adopts rustc-internal `ty/print/pretty.rs`
  per-arm precedent for the 19 variants (most are
  mechanical: `bool`, `i32`, `[T; N]`, `&T`, etc.);
  the Rust-source-style is preserved continuously.
  `FnDef` is rendered as rustc-faithful curly-brace
  form `fn(<arg_tys>) -> <ret_ty> {<path><args>}` --
  Rust-source-faithful is impossible (FnDef is not
  expressible as a type expression in stable Rust)
  so curly-brace rustc-internal form is adopted.
  The Instance lookup for `(item, args) -> InstanceId
  -> InstanceKind::Function.fn_sig` is fine since
  `&self.model` is available to the printer.
- F2 (a) is the natural rendering given RegseqModel's
  `Ty::Dynamic` schema: `vtable_ty: TyId` resolves
  to the synthesised vtable struct's Item, whose
  DefPath encodes the trait + impl. Decoding (option
  c) is fragile -- §13.4 encoding could change.
  Treating opaque (option b) loses readability.
- F3 (a) is rustc-converged Rust-source form.
- F4's lifetime carve-out resolves trivially because
  RegseqModel doesn't carry lifetime regions per
  Q2 + stage-3 §10.1; nothing to suppress.

**Methodology rules invoked.**
- *Compiler-theory-first* (per-arm forms inherited
  from rustc + Charon convergent evidence).
- *Compositional behavior beats special-case flags*
  (default-value suppression of empty composites
  falls out of conditional-render logic; no parallel
  `has_args: bool` flags).
- *C semantics with information preservation*
  (rendering preserves the schema's distinguishing
  shapes -- e.g., `&T` vs `&mut T`, `*const T` vs
  `*mut T`).

**Prior-art evidence.**
- `prior-art/rustc.md:103-129` (D-13-5): per-arm
  coverage; primary-source citations at
  `rustc_middle/src/ty/print/pretty.rs:712
  (pretty_print_type entry)`, `:1037 (Array)`,
  `:1044 (Slice)`, `:737 (Tuple arity-1)`,
  `:796 (Adt defers to print_def_path)`,
  `:763 (FnPtr defers to sig_tys)`,
  `:797 (Dynamic)`, `:736 (Never)`, `:810 (Foreign)`.
- `prior-art/charon.md:82-97` (D-13-5): AstFormatter
  approach with per-Ty-variant arms; same Rust-source-
  style; primary sources in
  `charon/charon/src/pretty/fmt_with_ctx.rs`.
- `prior-art/rustc_public.md:81-89` (D-13-5): punts
  type rendering to the thread-local context
  (`with(|cx| ...)` callback) -- the public Ty's Debug
  impl invokes the rustc-internal type printer via
  the bridge.

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §6 (`render_ty` per-arm
  specs).

**Sub-decisions.**

#### D-13-5.1 -- Per-variant rendering table [LOCKED]

19 variants per `pretty-printer-snapshot.md` §6.

#### D-13-5.2 -- Ty::Dynamic = `dyn <vtable-DefPath> + Auto + Auto` [LOCKED]

Auto-traits in stage-3 §10.1 sorted order; empty
`auto_traits` omits the `+` suffix. Vtable Item's
DefPath (`<regseq_synth>::vtable::Trait::for::<ImplTy>`
form per §13.4) encodes the principal trait + impl.

#### D-13-5.3 -- render_generic_args helper [LOCKED]

Form `<T1, T2, K1>` non-empty, empty string when args
empty. `GenericArg` is the closed 2-variant enum
`{ Type(TyId), Const(ConstId) }` per `spec.md` §9.1
(lifetimes erased post-monomorphisation; not
represented). Per-arg dispatch: `GenericArg::Type(ty)`
→ `render_ty(ty)`; `GenericArg::Const(c)` →
`render_const(c)` (forward-call to stage-14 D-4-K).

#### D-13-5.4 -- Lifetime suppression carve-out (non-issue) [LOCKED]

RegseqModel's `Ty::Ref` carries `Provenance` (borrow
flavor), NOT rustc-style lifetime regions per
stage-3 §10.1. So `&T` for SharedBorrow / `&mut T` for
UniqueBorrow renders without any `'a` token. The
carve-out deferred from D-13-1.4 resolves trivially
here.

#### D-13-5.5 -- Forward-call to stage-14 render_const [LOCKED]

For `GenericArg::Const(c)` arms, `render_ty` calls
`self.render_const(c)` -- which is stage-14-owned per
D-4-K. The forward-call is the same pattern stage 4
already used (deferred `render_const` to stage 14
with similar forward references). At stage-14 close
the forward-call resolves to the implementation; at
stage-13 spec time it's a documented dependency.

#### D-13-5.6 -- GenericArgs structural verification (Phase B audit) [VERIFIED]

Phase B Codex review R1 verified `GenericArgs` shape
against `spec.md` §9.1 (line 1828): the enum is the
closed 2-variant `{ Type(TyId), Const(ConstId) }`. No
`Lifetime` arm exists; the defensive
"render-as-nothing" arm initially drafted in §6.7 /
D-13-5.3 was removed at R1. Lifetimes are erased at
the bridge per Q2 region erasure and never appear in
`GenericArgs`.

---

### D-13-6 -- Body-level renderer per-arm specifications [LOCKED]

**Question.** Per stage-11 D-11-K bundle, stage 13
owns per-arm rendering for 9 public renderers
(`render_place`, `render_operand`, `render_rvalue`,
`render_statement`, `render_terminator`,
`render_call_callee`, `render_switch_targets`,
`render_assert_kind`, `render_basic_block`) plus 1
internal helper (`render_terminator_kind`). The
underlying vocabularies are stage-5 through stage-10
closed (PlaceElem, Operand, Rvalue, Statement,
TerminatorKind, BasicBlock).

Most arms are rustc-converged direct adoptions; five
genuine choices concentrate the design surface (G1
projection chain shape; G2 Constant operand wrapping;
G3-Q1 CastKind disambiguation; G3-Q2 AggregateKind
RawPtr arm; G5 render_call_callee Direct arm).

**Decision.** **Mass adoption of rustc-converged forms
+ five sub-decisions** per `pretty-printer-snapshot.md`
§7. Concretely:
- G1 (`render_place`): rustc pre/post-wrap projection
  chain pattern. Field/variant name resolution by name
  when resolvable (via place_ty walk +
  `LayoutKind::Struct/Union/Enum`-active-variant /
  Tuple `fields[idx].name`). Both `FieldLayout.name`
  and `VariantLayout.name` are non-optional `StringId`
  per `spec.md` §11; missing-name is invalid IR
  rejected by validation, not papered over at render
  time per `../README.md` §2 "no defensive fallbacks".
- G2 (`render_operand`): `Constant(id)` renders bare
  `<render_const(id)>` (rustc-converged); rejects
  Charon's `const (...)` explicit wrapper.
- G3 (`render_rvalue`): 9-variant table per the spec
  + AggregateKind 7-variant sub-table; CastKind shown
  only when ambiguous (rustc-converged minimal form;
  ambiguous variants `PointerCoercion(ReifyFnPointer
  | UnsafeFnPointer | ClosureFnPointer | Unsize)` plus
  `Subtype`); RawPtr aggregate rendered as
  `(<op_addr>, <op_meta>) as *const/mut <pointee_ty>`
  (two-operand form per `spec.md` §21.3.6 schema;
  both operand positions required even for thin
  pointers).
- G4 (`render_statement`): 3-variant table;
  `SetDiscriminant` renders variant name from
  `place_ty(p)`'s `LayoutKind::Enum.shape`
  (`VariantLayout.name` non-optional per `spec.md`
  §11).
- G5 (`render_terminator` + sub-renderers): 6-variant
  TerminatorKind table; `render_call_callee.Direct(id)`
  resolves Instance, renders
  `<render_def_path(instance.item)><render_generic_args(instance.generic_args)>`
  (Instance-keyed `generic_args` per `spec.md` §9.2);
  `render_assert_kind` direct adoption of
  rustc_public's 8-template `pretty_assert_message`
  patterns.
- G6 (`render_basic_block`): `bbN: { ... }` envelope,
  4-/8-space indentation, trailing `;` per-statement,
  no trailing `;` on terminator.

**Reasoning.**
- G1's rustc pre/post-wrap projection chain pattern is
  chosen over rustc_public's simpler post-only chain
  because the per-projection wrapping (`Deref` →
  `(*X)`; `Field` → `(X.<name>: <ty>)`; `Downcast` →
  `(X as <variant>)`) preserves the projection
  semantics visibly. Field/variant name resolution
  via the §11 layout schema; both name fields are
  non-optional `StringId` so missing-name is invalid
  IR rejected by validation per the "no defensive
  fallbacks" methodology rule.
- G2's bare `Constant` rendering inherits the rustc
  + project precedent; the const value's rendering
  via `render_const` already encodes its const-ness
  (`42i32`, `true`, `&"..."`).
- G3's CastKind disambiguation policy (b) is the
  rustc-converged minimal form; snapshot stability
  prefers minimal output. Ambiguous-only annotation
  by rule (§7.3.1) keeps the visible-cast set fixed
  to the variants where source / target types fail
  to disambiguate.
- G3-Q2's RawPtr aggregate renders as
  `(<op_addr>, <op_meta>) as *const/mut <pointee_ty>`
  (two-operand form). The schema requires both operand
  positions per `spec.md` §21.3.6 (mirroring rustc's
  `RawPtr(Ty<'tcx>, Mutability)` shape per
  `rustc_middle/src/mir/syntax.rs:1561-1566`);
  thin pointers carry the unit value `()` in operand
  position 1.
- G4's `SetDiscriminant` variant-name rendering reads
  `discriminant(p) = <variant_name>` (over rustc's
  numeric form `discriminant(p) = <variant_idx>`) for
  diagnostic clarity; the place_ty walk to resolve the
  name is already needed for `Downcast` projections.
  `VariantLayout.name` is non-optional `StringId` per
  `spec.md` §11, so this is always resolvable.
- G5's `Direct(InstanceId)` rendering via Instance
  lookup (`render_def_path(instance.item)` +
  `render_generic_args(instance.generic_args)`) is
  consistent with `Ty::FnDef` per D-13-5.1 (both
  resolve through Instance-keyed `generic_args` to
  render path + args).
  `render_assert_kind`'s direct adoption of
  rustc_public's `pretty_assert_message` template
  patterns is the explicit precedent (`prior-art/
  rustc_public.md`).
- G6's envelope inherits stage-10 D-10-1 / D-10-K.1
  shape (`bbN: { stmt; stmt; terminator; }`); the
  4-/8-space indentation matches D-13-2.2.

**Methodology rules invoked.**
- *Compiler-theory-first* (per-arm forms inherited
  from rustc + rustc_public + Charon convergent
  evidence; G3-Q2 RawPtr two-operand form locked at
  R1 against `spec.md` §21.3.6 schema).
- *Compositional behavior beats special-case flags*
  (sigil rendering inlined per D-13-2.3; field/variant
  name resolution dispatches per place_ty layout walk
  rather than a parallel name-resolution flag).
- *Snapshot stability* (CastKind disambiguation
  by-rule: ambiguous-only annotation per §7.3.1
  rather than always-on noise).

**Prior-art evidence.**
- `prior-art/rustc.md:131-149` (D-13-6): primary-source
  citations at `rustc_middle/src/mir/pretty.rs:1293-1316
  (pre/post-wrap projection chain)`,
  `:1155-1180 (per-Aggregate variant arms)`,
  `:923-1025 (per-TerminatorKind arms / fmt_head +
  fmt_successor_labels)`, `:805 (per-Statement arms)`.
- `prior-art/rustc_public.md:91-118` (D-13-6):
  primary-source citations at
  `rustc_public/src/mir/pretty.rs:229
  (pretty_assert_message enumeration)`,
  `:397 (pretty_aggregate)`,
  `:171 (pretty_terminator_head)`,
  `:204 (pretty_successor_labels)`,
  `:326 (pretty_operand)`.
- `prior-art/charon.md:98-123` (D-13-6):
  per-construct rendering arms via FmtCtx + WithCtx
  wrapper in `charon/charon/src/pretty/fmt_with_ctx.rs`.

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §7 (Body-level renderer
  per-arm specifications).

**Sub-decisions.**

#### D-13-6.1 -- render_place [LOCKED]

Pre/post-wrap projection chain preserving operator
precedence (Field's `(X.<name>: <ty>)` wrap applies
AROUND prior result; outer parens preserve
deref-then-field semantics). Field/variant name
resolution via place_ty walk; both `FieldLayout.name`
and `VariantLayout.name` are non-optional `StringId`
per `spec.md` §11 -- missing-name is invalid IR
rejected by validation, not handled by a defensive
render-side fallback.

#### D-13-6.2 -- render_operand [LOCKED]

Bare `<render_const>` for `Constant(id)`; rejects
Charon `const (...)` wrapper.

#### D-13-6.3 -- render_rvalue [LOCKED]

9-variant table + AggregateKind 7-variant sub-table;
CastKind minimal form (G3-Q1 (b)); RawPtr aggregate
two-operand `(<op_addr>, <op_meta>) as *const/mut <pointee_ty>`
form per `spec.md` §21.3.6.

#### D-13-6.4 -- render_statement [LOCKED]

3-variant table; `SetDiscriminant` renders variant
name from `place_ty(p)`'s `LayoutKind::Enum.shape`
(`VariantLayout.name` non-optional per `spec.md` §11).

#### D-13-6.5 -- render_terminator + sub-renderers [LOCKED]

6-variant TerminatorKind table; `render_terminator`
output ends with `]` (no trailing `;`); the `;` is
appended by `render_basic_block` at composition time
(§7.10). `render_call_callee.Direct(id)` =
`<path><args>` via Instance lookup;
`render_assert_kind(&AssertKind, &Body)` direct adoption
of rustc_public's 8-template patterns (BoundsCheck,
Overflow per BinOp, OverflowNeg, DivisionByZero,
RemainderByZero, MisalignedPointerDereference,
NullPointerDereference, InvalidEnumConstruction). The
`&Body` parameter is required because operand
placeholders inside AssertKind variants render via
`render_operand`.

#### D-13-6.6 -- render_basic_block [LOCKED]

`bbN: { stmt; ...; terminator; }` envelope;
4-/8-space indentation; trailing `;` per-statement
(emitted by `render_statement`); trailing `;` after
terminator is appended HERE at composition time
(`render_terminator` itself produces `<head> -> [labels]`
with no `;`).

---

### D-13-7 -- Worked-example specimens for snapshot stability [LOCKED]

**Question.** Stage-13 §2 principle: "Worked-example
specimens are spec content. ≥1 canonical fragment per
public renderer in `spec-snapshot.md` pin the textual
contract for snapshot stability; they are spec-side
content, not test fixtures." Stage 13 must lock the
specimen format, the budget, the location (per the
emergent D-13-9 spec split, location is in
`pretty-printer.md` §9 not `spec.md` §18), and the
mirroring relationship to insta-snapshot tests.

**Options considered.**

H1 (specimen format):
- (a) Paired blocks: input value (constructed via plain
  Rust ctor + brief comment) + expected rendered output.
  Spec-side verifiable; reader can mentally construct
  the input and check the output.
- (b) Source-code paired: actual Rust input source +
  rendered RegseqModel output. Captures bridge translation
  context but blurs renderer boundary.
- (c) Free-form prose with embedded snippets. Loose;
  harder to verify against.

H2 (specimen budget):
- (a) ≥1 per public renderer + ≥1 per closed-enum-variant
  where variant rendering differs **at the per-arm spec
  level** -- i.e., variants whose rendering follows
  different structural templates. Variants distinguished
  only by policy decoration (e.g., `CastKind` arms whose
  rendering follows the same `<op> as <ty> [(CastKind)]`
  template, with the bracketed annotation toggling per
  the §7.3.1 ambiguity rule) get one representative
  specimen per policy bucket, not one per arm. Total
  ~75 specimens.
- (a-narrowed) Collapse Ty-variant families further;
  collapse AggregateKind sub-arms; collapse AssertKind
  8 templates to one representative (~40 specimens).
- (b) Minimal: ≥1 per public renderer, no per-variant
  coverage (~17 specimens).

H3 (specimen location):
- (a) Embedded inline in §18 spec contributions.
- (b) Collected in single appendix at end of §18.
- (c) Per-stage in `stage-13/spec-snapshot.md` only,
  not top-level.

  (Per emergent D-13-9, location is in
  `pretty-printer.md` §9 not `spec.md` §18; the
  inline-vs-appendix question still applies within
  `pretty-printer.md`.)

H4 (insta-snapshot mirroring):
- (a) 1:1 mirroring: every specimen has a matching
  insta snapshot test in `crates/regseq-model/tests/
  pretty.rs`.
- (b) Specimens illustrative, insta tests independent.
- (c) Specimens mirror a subset; broad insta coverage
  handles edge cases.

**Decision.** **H1 (a) + H2 (a) + H3 (a, scoped to
pretty-printer.md per D-13-9) + H4 (a)**:
- Specimens use paired ctor-input + output-text format.
- ~75 specimens covering ≥1 per public renderer +
  ≥1 per closed-enum-variant whose rendering follows
  a structurally distinct template at the per-arm
  spec level. Variants distinguished only by policy
  decoration (e.g., `CastKind` annotated arms whose
  rendering follows the same `<op> as <ty> [(CastKind)]`
  template, with the bracketed annotation toggling per
  the §7.3.1 ambiguity rule) get one representative
  specimen per policy bucket, not one per arm.
- Inline in `pretty-printer.md` §9 (Worked-example
  specimens) per D-13-9.
- 1:1 mirrored to insta snapshots in
  `crates/regseq-model/tests/pretty.rs` at end-of-phase-1
  implementation sweep.

**Reasoning.**
- H1 (a) keeps the renderer contract crisp: input ctor
  paired with output text. Source-translation context
  (where the value comes from in the bridge) is out
  of D-13-7 scope (stages 5-12 territory).
- H2 (a) per-variant coverage at the per-arm-spec
  level is the pure-discipline-ideal; closes per-variant
  rendering ambiguity at the cost of ~500-800 lines of
  spec content. Per the §2 "Worked-example specimens
  are spec content" principle the cost is justified.
  Variants distinguished only by policy decoration
  (`CastKind` annotated arms; `PointerCoercion`
  sub-variants) get one representative specimen per
  policy bucket (one bare `IntToInt` specimen + one
  annotated `ReifyFnPointer` specimen) rather than one
  per arm; per-template specimens for each individual
  ambiguous variant are unnecessary because the
  rendering template is identical (only the decoration
  string changes verbatim from the variant name).
- H3 (a) inline keeps specimens adjacent to rendering
  rules; reader sees specimen + rule together. Per
  D-13-9 spec split, "inline in §18" becomes "inline
  in `pretty-printer.md` §9."
- H4 (a) 1:1 mirroring catches spec drift directly
  through test failure; canonical test = canonical
  specimen.

**Methodology rules invoked.**
- *Worked-example specimens are spec content* (stage-13
  §2 principle).
- *Snapshot stability* (specimens pin the textual
  contract; insta tests enforce it).

**Prior-art evidence.**
- `prior-art/rustc.md:151-159` (D-13-7): rustc has its
  own MIR test fixtures in the testsuite; convention
  well-established via `--emit=mir` reference outputs
  and golden tests; primary-source location at
  `tests/mir-opt/` and similar fixture directories
  outside the pretty-printer source.
- `prior-art/rustc_public.md:120-128` (D-13-7): tests
  in `tests/ui/stable-mir-print/` (fixture directory;
  not in the pretty-printer source file).
- (No primary precedent for spec-embedded specimens at
  the per-arm spec level granularity -- RegseqModel's
  spec-content discipline per the §2 "Worked-example
  specimens are spec content" principle is its own
  pattern.)

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §9 (Worked-example
  specimens).

**Sub-decisions.**

#### D-13-7.1 -- Specimen format = paired blocks [LOCKED]

Format template:
```text
Input: render_<X>(value) where value is:
    <minimal ctor> // or "the result of <bridge translation rule>"

Output:
    <rendered text>
```

#### D-13-7.2 -- Specimen budget ~71 [LOCKED]

Per `pretty-printer-snapshot.md` §9, specimens cover:
- render_ty: ~20 (1 per Ty-variant family)
- render_place: ~1 (long chain)
- render_operand: ~3 (Copy / Move / Constant)
- render_rvalue: ~17 (9 variants + 8 AggregateKind sub-arms)
- render_statement: ~4 (3 StatementKind + 2 Intrinsic sub-arms; minus overlap)
- render_terminator + sub-renderers: ~17 (6 TerminatorKind + 2 CallCallee + 1 SwitchTargets + 8 AssertKind)
- render_basic_block: ~2 (multi-statement + empty-statements)
- render_item: ~7 (per ItemKind + 1 stress with all attribute prefixes)

Total ~71. Phase B may narrow to ~40 if Codex review
or operational constraints surface; the specimen list
is auditable per-renderer.

#### D-13-7.3 -- Specimen location = inline in pretty-printer.md §9 [LOCKED]

Per D-13-9 spec split. (Inline-vs-appendix question
within `pretty-printer.md` resolved as inline in §9,
adjacent to the §7 body-level renderer specs.)

#### D-13-7.4 -- Insta-snapshot 1:1 mirroring [LOCKED]

End-of-phase-1 implementation sweep wires up insta
tests in `crates/regseq-model/tests/pretty.rs`
mirroring specimens 1:1.

---

### D-13-8 -- §18.1 sigil-table review + §18.2 ownership-table consolidation [CLOSED-BY-ABSORPTION]

**Question (original Open phase framing).** §18.1
sigil table in `spec.md` had identified drift (stale
`BodyId -> Body/N` row in code doc-comment at
`crates/regseq-model/src/pretty.rs:22-34` post-stage-11
D-11-5; stale stage-landing comments at lines 35-36).
§18.2 per-method ownership table was retargeted by
stage-11 D-11-K and stage-12 D-12-K to stage-13 ownership
across all rendering ownership rows. D-13-8 was scoped
to confirm or refine the canonical sigil set + consolidate
the ownership-table presentation.

**Disposition.** **Closed by absorption into D-13-9.**
With the emergent D-13-9 reframe (pretty-printer spec
moves to top-level `pretty-printer.md`), the sigil-table
review lands as `pretty-printer.md` §3 (Body envelope,
indentation, sigil conventions) and the ownership-table
consolidation lands as `pretty-printer.md` §8 (Per-method
ownership table). D-13-8 had no substantive content
beyond what D-13-9 absorbed; the table-cleanup work IS
the §3 + §8 sections of `pretty-printer.md`.

**Status flip at merger contract close.** D-13-8
flips to `[CLOSED-BY-ABSORPTION]` in `decisions.md`
with annotation `(absorbed into D-13-9 per Phase A
discussion)`. The drift items surfaced under D-13-8
(stale `BodyId` sigil; stale stage-landing comments;
stale stub message at `pretty.rs:138-142`) remain
implementation-time concerns per the
spec-before-implementation rule -- they are flagged as
F-13-Code-Drift in §5 below for the end-of-phase-1
implementation sweep.

---

### D-13-9 -- Pretty-printer spec location (companion file) [LOCKED]

**Question (mid-cluster surfacing; user-prompted).**
"Could we create a separate `pretty-printer.md` where
we capture pretty-printer spec instead of adding to
`spec.md`?" Stage-13's pretty-printer spec was
projected to grow `spec.md` §18 by ~1000-2000 lines
(rendering rules + ~71 worked-example specimens). The
user surfaced the architectural question: pretty-printing
is consumer-side concern, separable from the IR's
structural definition. The project already half-implements
the side-file pattern (`architecture.md`, `decisions.md`,
`regseq-background.md`, `rustc-references.md` are
top-level companion files for cross-cutting concerns).

**Options considered.**

I1 (scope of separation):
- (a) Full pretty-printer spec moves to `pretty-printer.md`
  (master style, envelope, per-arm rules, sigil table,
  ownership table, specimens). `spec.md` §18 becomes
  one-paragraph pointer.
- (b) Specimens-only separation (rules stay in §18).
  Half-pregnant; rendering contract split across two
  files.
- (c) Specimens + per-arm rules; sigil + ownership
  tables stay in §18. Splits "structural" from
  "textual" but the line is fuzzy.

I2 (cross-references back to spec.md):
- (a) Drop the deferred-renderer rows from `spec.md`
  entirely (§15 / §19.8 / §20.6 / §21.8 / §22.8 /
  §23.9 / §24.9 / §25.x). Each construct's spec section
  ends with "Rendering: see `pretty-printer.md` §<N>."
- (b) Keep the rows pointing to `pretty-printer.md`.
  Adds noise.
- (c) Keep rows + 1-line summaries in `spec.md`;
  full spec in `pretty-printer.md`. Duplicates content.

I3 (methodology rule extension): README §5.9 needs
rule 6; §5.5 step 5 amendment; §4 folder layout
addition.

I4 (`pretty-printer.md` initial structure):
10-section structure proposed.

**Decision.** **I1 (a) + I2 (a) + I3 (methodology
rule extension) + I4 (10-section structure)**:
- Full pretty-printer spec moves to top-level
  `pretty-printer.md`. `spec.md` §18 becomes
  one-paragraph pointer.
- Drop deferred-renderer rows in `spec.md` §15 / §19 /
  §20 / §21 / §22 / §23 / §24 / §25; replace each with
  one-line "Rendering: see `pretty-printer.md` §<N>"
  pointer.
- README §5.9 gains rule 6 (pretty-printer spec lives
  in `pretty-printer.md`); §5.5 step 5 amended to direct
  pretty-printer spec contributions to `pretty-printer.md`;
  §4 folder layout adds `pretty-printer.md` entry.
- `pretty-printer.md` initial 10-section structure:
  (1) Mission and consumer model; (2) Master rendering
  style; (3) Body envelope, indentation, sigil
  conventions (absorbs §18.1 sigil table); (4)
  Span-rendering and debuginfo modes; (5) Item-level
  rendering shape; (6) Type rendering -- render_ty
  per-arm specs; (7) Body-level rendering -- per-arm
  specs for the 9 public renderers + 1 internal helper;
  (8) Per-method ownership table (absorbs §18.2); (9)
  Worked-example specimens; (10) Cross-references to
  `spec.md` (per-construct rendering pointers).

**Reasoning.**
- I1 (a) cleanest separation: pretty-printing is
  consumer-side concern (snapshot tests, diagnostics),
  factors cleanly from IR-structure spec (producer-side).
  Matches existing top-level side-file pattern. Future
  renderer extensions (additional output formats) extend
  `pretty-printer.md` without touching IR-structure
  spec.
- I2 (a) drops obsolete placeholder rows; the deferred-
  renderer rows in `spec.md` were placeholders pending
  stage 13's ownership and have no further role with
  the spec split.
- I3 is the methodology rule extension stage 13 introduces
  for future renderer-related stages (e.g., stage 14's
  `render_const` and `render_instance`) to inherit.
- I4's 10-section structure mirrors §18 organization
  while absorbing §18.1 (sigils) and §18.2 (ownership
  table) as `pretty-printer.md` §3 and §8.

**Methodology rules invoked.**
- *Spec is mutable* (the side-file pattern is consistent
  with project's existing top-level files).
- *Compositional behavior beats special-case flags*
  (the spec split is a structural reorganization, not
  a parallel system; the merger contract at §5.5 step
  5 absorbs the new file via the existing concat +
  in-place-update mechanism).

**Prior-art evidence.**
- Project's existing top-level companion files
  (in-tree, not external prior-art):
  `docs/discussions/regseq-model/architecture.md` (Q1-Q7
  settled architectural decisions),
  `docs/discussions/regseq-model/decisions.md` (D-tag
  registry),
  `docs/discussions/regseq-model/regseq-background.md`
  (DSL background),
  `docs/discussions/regseq-model/rustc-references.md`
  (pinned rustc-src paths). Pattern is established;
  D-13-9 extends it with `pretty-printer.md`.
- (No external prior-art for the consumer-side /
  producer-side spec split; D-13-9 is RegseqModel-
  internal architectural coherence with the existing
  side-file convention.)

**Spec cross-reference.**
- `pretty-printer-snapshot.md` (whole file -- this is
  D-13-9's principal artifact).
- `spec-snapshot.md` §15 / §18 / §19 / §20 / §21 /
  §22 / §23 / §24 / §25 (cross-reference pointers
  per D-13-9.2).
- `../README.md` §4 / §5.5 step 5 / §5.9 rule 6
  (methodology rule extension per D-13-9.3).

**Sub-decisions.**

#### D-13-9.1 -- Scope of separation = full [LOCKED]

Full pretty-printer spec to `pretty-printer.md`.
`spec.md` §18 becomes one-paragraph pointer.

#### D-13-9.2 -- Cross-references back to spec.md = drop deferred rows + one-line pointers [LOCKED]

`spec.md` deferred-renderer rows (§15.X, §19.8, §20.6,
§21.8, §22.8, §23.9, §24.9, §25.x) drop; each
construct's section gains one-line "Rendering: see
`pretty-printer.md` §<N>" pointer.

#### D-13-9.3 -- Methodology rule extension [LOCKED]

`../README.md` §5.9 gains rule 6: "Pretty-printer
spec lives in top-level `pretty-printer.md`, not
`spec.md`. The two files are co-equal merger artifacts;
integrating per-stage `spec-snapshot.md` content into
either is a stage-13-introduced split. The split
rationale: pretty-printing is consumer-side; IR
structure is producer-side; the line keeps each file
focused on a single audience."

`../README.md` §5.5 step 5 amended: "If stage N's
spec contributions are pretty-printer content,
integrate into `pretty-printer.md` instead of
`spec.md` per §5.9 rule 6. The integration rules
(live-not-historical; topic-based; audit-shape
stripped; no forward-looking placeholders; D-tag
inline references avoided) apply identically."

`../README.md` §4 folder layout gains entry:
`pretty-printer.md           -- pretty-printer spec
(per stage-13 D-13-9; merger artifact alongside
spec.md)`.

#### D-13-9.4 -- pretty-printer.md initial 10-section structure [LOCKED]

(1) Mission and consumer model.
(2) Master rendering style (D-13-1).
(3) Body envelope, indentation, sigil conventions
    (D-13-2 + sigil table; absorbs `spec.md` §18.1).
(4) Span-rendering and debuginfo modes (D-13-3).
(5) Item-level rendering shape (D-13-4).
(6) Type rendering -- `render_ty` per-arm specs
    (D-13-5).
(7) Body-level rendering -- per-arm specs for the 9
    public renderers + 1 internal helper (D-13-6).
(8) Per-method ownership table (absorbs `spec.md`
    §18.2; consolidated under D-13-9.6).
(9) Worked-example specimens (D-13-7).
(10) Cross-references to `spec.md` (per-construct
    rendering pointers).

#### D-13-9.5 -- Working surface = stage-13/pretty-printer-snapshot.md [LOCKED]

Stage-13 maintains `stage-13/pretty-printer-snapshot.md`
for the new top-level file's draft (the bulk content)
+ minimal in-place edits to existing `stage-13/spec-snapshot.md`
for the cross-references per D-13-9.2. At stage-13
merger close, `pretty-printer-snapshot.md` integrates
into top-level `pretty-printer.md` (which is created
fresh -- D-13-9 introduces it); `spec-snapshot.md`
edits integrate into top-level `spec.md` per the
existing merger contract.

#### D-13-9.6 -- D-13-8 closes by absorption [LOCKED]

D-13-8 (§18.1 sigil-table review + §18.2 ownership-table
consolidation) closes by absorption into D-13-9. Sigil-
table review lands as `pretty-printer.md` §3; ownership-
table consolidation lands as `pretty-printer.md` §8.
D-13-8 has no substantive content beyond what D-13-9
absorbs. At merger contract close, D-13-8 flips to
`[CLOSED-BY-ABSORPTION]` in `decisions.md` with
annotation `(absorbed into D-13-9 per Phase A
discussion)`.

---

## 2. Phase A discussion threads (substantive)

Two substantive Phase A threads warrant standalone
recording for Phase B audit-shape capture (in addition
to their per-D-tag entries above).

### Building-block-vs-composer architectural division (D-13-4.6)

A user-prompted mid-cluster surfacing during D-13-4
discussion. The user asked "shouldn't we be rendering
Instances instead of Items?" -- a direct architectural
challenge to the D-12-K bundle's `render_item` ownership
assignment.

The challenge surfaced a real concern: per Q2 (eager
mono baking), the codegen consumer is Instance-keyed;
Function bodies / Static initialisers / Const values /
ADT layouts all live on `InstanceKind` post stage-12
D-12-1.4 Option B reframe. So the headline pretty-printer
consumer should naturally be `render_instance`, not
`render_item`.

**Resolution via building-block-vs-composer split.**
Three options were considered:
- (a) Keep `render_item` public per D-12-K.1 (current plan).
- (b) Make `render_item` private/internal to PrettyPrinter.
- (c) Move `render_item` ownership to stage 14 entirely
  (retarget D-12-K -- heavyweight stage-12 reopen).

(a) was selected. The division: **stage 13 owns NEW
rendering BUILDING BLOCKS (`render_ty`, `render_item`,
all body-level renderers); inherited building blocks
(`render_def_path` stage 12 per D-12-6,
`render_def_path_value` + `render_span` stage 1) are
consumed by composers but not stage-13-owned; stage 14
owns rendering COMPOSERS (`render_const` per D-4-K,
`render_instance` composing render_item + per-Instance
content, top-level `render` iterating Instances).**

The user's challenge clarified that:
- The headline consumer IS `render_instance` (stage 14).
- `render_item` is a useful building block AND an
  Item-table summary view ("list all Items in the
  model"); both rationales preserve its public-API
  status per D-12-K.1.
- Stage 13 owns the building blocks because Items are
  stage-12-territory schema; rendering style for the
  wrapper is naturally stage-13 master-style territory
  (D-11-K's "rendering style ONCE for the whole
  surface" rationale extends compositionally to
  Item-level rendering).

Locked as D-13-4.6.

### Pretty-printer spec location reframe (D-13-9 emergent)

A user-prompted mid-cluster surfacing during D-13-7
discussion. The user asked "could we create a separate
`pretty-printer.md` instead of adding to `spec.md`?"

The challenge surfaced the structural concern: stage
13's pretty-printer spec was projected to grow
`spec.md` §18 by ~1000-2000 lines (rendering rules + ~71
worked-example specimens). The project already
half-implements the side-file pattern (`architecture.md`,
`decisions.md`, `regseq-background.md`,
`rustc-references.md` are top-level companion files
for cross-cutting concerns).

**Resolution via spec split.** Three scopes considered:
- (a) Full pretty-printer spec to `pretty-printer.md`
  (selected).
- (b) Specimens-only separation (rules stay in §18) --
  half-pregnant.
- (c) Specimens + per-arm rules; sigil + ownership
  tables stay in §18 -- fuzzy line.

(a) was selected. The separation:
- `pretty-printer.md` carries the full pretty-printer
  spec (10 sections per D-13-9.4).
- `spec.md` §18 becomes one-paragraph pointer.
- Deferred-renderer rows in `spec.md` §15 / §19.8 /
  §20.6 / §21.8 / §22.8 / §23.9 / §24.9 / §25.x drop
  + replace with one-line "Rendering: see
  `pretty-printer.md` §<N>" pointers.
- README §5.9 / §5.5 step 5 / §4 get methodology
  rule extension per D-13-9.3.

Reframe consequences:
- D-13-8 closes by absorption into D-13-9 (sigil-table
  review + ownership-table consolidation land in
  `pretty-printer.md` §3 + §8).
- Stage 13's working surface gains
  `stage-13/pretty-printer-snapshot.md` for the new
  top-level file's draft.
- Stage-13 merger contract integrates two target
  files (`pretty-printer.md` created fresh; `spec.md`
  in-place edited).

Locked as D-13-9 (emergent main D-tag, not a
sub-decision).

---

## 3. Cross-references

- `../README.md` -- top-level project mission, methodology
  rules, architectural decisions Q1-Q7. Stage 13's
  D-13-9.3 introduces methodology rule extension at §5.9
  rule 6 / §5.5 step 5 / §4 folder layout.
- `../decisions.md` -- registry of all per-stage D-tags;
  stage-13 D-13-1 through D-13-7 and D-13-9 enter as
  `[LOCKED]` at merger close; D-13-8 enters as
  `[CLOSED-BY-ABSORPTION]`.
- `../spec.md` -- merged top-level spec; stage-13
  contributions via in-place edits per D-13-9.2 (drop
  deferred-renderer rows + add one-line cross-references).
- `../pretty-printer.md` -- new top-level file introduced
  by D-13-9; stage-13 contributions land here as the
  bulk authoring per D-13-9.5.
- `../stage-3/discussions.md` -- historical D14 entry
  (preserved as deferred-decision context); referenced
  by D-13-1.3 closure-by-absorption.
- `../stage-11/discussions.md` D-11-K -- master cascade
  retargeting body-level rendering ownership to stage 13;
  D-13-1 through D-13-6 inherit per the cascade.
- `../stage-12/discussions.md` D-12-K -- compositional
  Item-level rendering cascade; D-13-4 inherits.
- `prior-art/rustc.md` -- rustc-internal MIR + Ty
  pretty-printer evidence.
- `prior-art/rustc_public.md` -- rustc_public stable IR
  pretty-printer evidence; closest structural analog.
- `prior-art/charon.md` -- LLBC IR pretty-printer
  evidence; AstFormatter precedent.
- `prior-art/mrustc.md` -- counter-precedent;
  dual-rendering split anti-pattern.

---

## 4. Outbound expectations to file at merger contract close

Per `../README.md` §5.5 step 7, the following outbound
expectations are filed at stage-13 merger contract
close:

### To `../stage-14/expectations.md` "From stage 13"

Stage 14 owns rendering composers (`render_const`,
`render_instance`, top-level `render`); stage-13
master style decisions inherit. Outbound bundle:

```text
1. render_const inherits D-13-1 master rendering style
   (Rust-source-style with sigil decorations + PrettyPrinter
   methods returning String). Stage-4 D-4-K already
   deferred render_const to stage 14; the master style
   for it is now locked at stage 13.

2. render_instance inherits D-13-1 master style and
   composes:
   - render_item (D-13-4 wrapper output: attribute block +
     [unsafe ]fn / static [mut ] / const / struct / enum /
     union kind-keyword + DefPath)
   - render_ty over FnSig for Function Instances
     (signature: `(arg_tys) -> ret_ty`)
   - body envelope per D-13-2 (`{ let _N: T; ...; bbN: { ... } }`)
   - body-level renderers per D-13-6 (per-Statement,
     per-Terminator, per-BasicBlock arms)
   - layout / init / value rendering per per-Instance
     content (Function: body; Static: ty + init; Const: value;
     Struct/Enum/Union: layout)

3. render top-level inherits D-13-2 envelope + iterates
   model.instances (Q2 codegen-keyed). Per-Instance dump
   composed via render_instance.

4. Span / debuginfo gating per D-13-3 (PrettyPrintOptions
   with include_spans / include_debug_names) inherits
   uniformly across composers; render_instance and
   render_const honour the same toggles.

5. Worked-example specimens (D-13-7) per render_const +
   render_instance arms: stage 14 adds specimens for
   each render_const arm (per the 11-variant Const enum
   from D-4-1) + render_instance per-InstanceKind arms,
   inline in pretty-printer.md §9 alongside stage-13's
   specimens.

6. D-13-9 spec split inherits: stage-14's pretty-
   printer spec contributions land in pretty-printer.md,
   not spec.md, per the methodology rule extension at
   ../README.md §5.9 rule 6.
```

Predecessor-stage attribution: stage 13 (pretty-printing).

### To `../README.md` (in-place edit at merger close)

Per D-13-9.3, stage-13 merger contract integrates the
methodology rule extension into `../README.md`:
- §5.9 gains rule 6 (pretty-printer spec lives in
  `pretty-printer.md`).
- §5.5 step 5 amended (pretty-printer content directs
  to `pretty-printer.md`).
- §4 folder layout adds `pretty-printer.md` entry.

### Cross-stage reopenings flagged at merger close

Per stage-13 sub-decisions:

- **Stage-3 D14** (renderer-style call deferred to stage
  13). At stage-13 merger close, D14's status flips to
  `[CLOSED-BY-ABSORPTION]` in `decisions.md` with
  annotation `(absorbed into D-13-1 per stage 13)`.

---

## 5. F-13 cross-stage findings (informational)

Findings surfaced during stage-13 Phase A that are NOT
stage-13 D-tags but worth recording for cross-stage
coherence. Action items at merger close.

### F-13-Code-Drift -- pretty.rs scaffold drift (implementation-time)

The in-code `crates/regseq-model/src/pretty.rs` doc
comments + `render_def_path(ItemId)` stub message
reference state that pre-dates stage-9 / stage-11 /
stage-12 reframes:
- Doc-comment sigil table at lines 22-34 includes a
  stale `BodyId -> Body/N` row. No `BodyId` exists
  post stage-11 D-11-5 + stage-12 D-12-1.4 Option B
  reframe (bodies live as `InstanceKind::Function.body:
  Option<Body>`; no separate ID type).
- Lines 35-36 doc comment says `LocalId`/`BlockId`
  sigils land in stages 10/11. D-11-K retargeted both
  to stage 13.
- Lines 138-142 stub message for `render_def_path(ItemId)`
  says "stage 12 fills". D-12-6 locked the body but
  spec-before-implementation rule defers the code edit.

**Stage-13 disposition.** Implementation-time concerns
per the spec-before-implementation rule. Stage 13's
spec contributions resolve the spec-side drift (§3
sigil table in `pretty-printer.md` is canonical
post-Option-B). The code-side cleanup happens at the
end-of-phase-1 implementation sweep alongside
filling the stub bodies.

**Cross-refs:**
- `crates/regseq-model/src/pretty.rs:22-36, 138-142`
  (stale code).
- `pretty-printer-snapshot.md` §3 (canonical
  post-Option-B sigil table).
- `../status.md` "Deferred items (cross-stage)"
  entry to add at stage-13 merger close noting the
  end-of-phase-1 cleanup task.
