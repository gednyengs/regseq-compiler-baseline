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
