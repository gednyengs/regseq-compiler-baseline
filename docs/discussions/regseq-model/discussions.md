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
