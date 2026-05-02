# Stage 3 RegseqModel Spec Overview

The `RegseqModel` snapshot at the close of stage-3 work — what's locked, how the IDs are organized, and what stages 4–14 will refine. Stage 3 closed with 15 decisions locked (D1-D13, D15, D16); D14 (renderer style) was deferred to stage 13 (type-system pretty-printing) — see `../stage-13/expectations.md`.

---

## 1. Top-level `RegseqModel`

Every IR entity lives in one of these tables. Single-namespace per kind; body-local IDs (`LocalId`, `BlockId`) live inside individual `Body` values, not at the top level.

```rust
pub struct RegseqModel {
    pub crates:         IndexVec<CrateId, CrateInfo>,        // crate inventory
    pub primary_crate:  CrateId,                              // points at the spec crate

    pub strings:        StringInterner,                       // Vec<Arc<str>> + HashMap
    pub source_files:   IndexVec<SourceFileId, SourceFile>,   // [0] = "<synthesized>"

    pub types:          TyInterner,                           // refined per D4 (BiMap)
    pub consts:         IndexVec<ConstId, Const>,             // stage 4

    pub items:          IndexVec<ItemId, Item>,               // stage 12
    pub item_def_paths: IndexVec<ItemId, DefPathHash>,        // F4 cross-build identity

    pub sigs:           IndexVec<FnSigId, FnSig>,             // stage 11
    pub bodies:         IndexVec<BodyId, Body>,               // stage 11
    pub instances:      IndexVec<InstanceId, Instance>,       // primary mono table
}
```

The stage-1 placeholder `types: IndexVec<TyId, Ty>` is refined by **D4** into `types: TyInterner` (a structural BiMap). The stage-1 `Vec<GenericArg>` shape is consumed throughout `Ty::Struct/Enum/Union/FnDef/Closure` arguments.

---

## 2. ID System

### 2.1 Inventory

| ID             | Width | Indexes into           | Scope          | Stage that fills it                                  |
| -------------- | ----- | ---------------------- | -------------- | ---------------------------------------------------- |
| `CrateId`      | u32   | `model.crates`         | crate-level    | stage 1                                              |
| `StringId`     | u32   | `model.strings`        | crate-level    | stage 1 (`StringId(0) == ""` sentinel)               |
| `SourceFileId` | u32   | `model.source_files`   | crate-level    | stage 1 (`[0]` = synthesized sentinel)               |
| `TyId`         | u32   | `model.types`          | crate-level    | **stage 3**                                          |
| `ConstId`      | u32   | `model.consts`         | crate-level    | stage 4                                              |
| `ItemId`       | u32   | `model.items`          | crate-level    | stage 12                                             |
| `BodyId`       | u32   | `model.bodies`         | crate-level    | stage 11                                             |
| `FnSigId`      | u32   | `model.sigs`           | crate-level    | stage 11 (minimum schema pinned in stage 3 D15+M-11) |
| `InstanceId`   | u32   | `model.instances`      | crate-level    | stage 1                                              |
| `LocalId`      | u32   | `Body.local_decls`     | **body-local** | stage 11 (`RETURN_PLACE = LocalId(0)`)               |
| `BlockId`      | u32   | `Body.basic_blocks`    | **body-local** | stage 10 (`START_BLOCK = BlockId(0)`)                |
| `VariantIdx`   | u32   | `VariantLayout` arrays | layout-local   | stage 3 (D11)                                        |

Stage 14 may add more (`RegisterId`, `IsrId`) under the same conventions.

### 2.2 Conventions (every ID obeys all of these)

| Aspect            | Rule                                                                                                                                                               |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Backing           | u32, niche-encoded so `Option<Id>` is 4 bytes. `NonZeroU32` shifted, `NonMaxU32` newtype, or rustc-style top-256 reservation — implementation choice.              |
| Trait             | `Idx { fn new(usize) -> Self; fn index(self) -> usize; }` (mirrors `rustc_index::Idx`) plus `Ord` for stable dump ordering.                                        |
| Generation        | `newtype_index! { pub struct FooId {} }` macro analogue.                                                                                                           |
| Const sentinel    | `pub const ZERO: Self = Self::from_u32(0);` — used by `SourceFileId::ZERO`, `StringId::ZERO`, body-local `RETURN_PLACE`/`START_BLOCK`.                             |
| Const constructor | `pub const fn from_u32(idx: u32) -> Self` (the `Idx` trait method is non-`const`).                                                                                 |
| Derives           | `Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug`.                                                                                                        |
| Serde             | Hand-rolled or `#[serde(into = "u32", try_from = "u32")]`. **Never** `#[serde(transparent)]` — the on-wire form is the logical index, not the niche-encoded inner. |
| Display           | NOT required on the ID. Pretty-printer renders with kind prefix (`Item/7`, `bb3`, `_2`).                                                                           |
| Storage container | `IndexVec<I, T>` (allocation-order; F4-2 sorts as needed).                                                                                                         |

### 2.3 What's NOT interned

- **Spans** — stored inline in IR nodes as `(SourceFileId, byte_start, byte_end)`. No `SpanId`. (F2 in stage-1 research.)
- **Layouts** — carried inline on `Ty::Struct/Enum/Union/Tuple`, not interned.
- **`FnSig` content** — referenced by `FnSigId`, but the canonicalizer expands its contents (no nominal identity for function signatures).

---

## 3. The `Ty` Enum (19 variants, **closed at stage 3**)

The variant set is **closed**. Stage 14 domain extensions extend `InstanceKind` / `Item` flags / `Instance` tags, not `Ty`. Every variant is a *rigid* post-monomorphization type per `TyKind::is_known_rigid()`.

| Variant                                       | Carries                                           | Layout?          | Identity bucket                                      | C-side intent                                                                     |
| --------------------------------------------- | ------------------------------------------------- | ---------------- | ---------------------------------------------------- | --------------------------------------------------------------------------------- |
| `Bool`                                        | —                                                 | —                | structural                                           | `_Bool` / `uint8_t`                                                               |
| `Char`                                        | —                                                 | —                | structural                                           | `uint32_t` (Unicode scalar value)                                                 |
| `Int(IntKind)`                                | `I8`/`I16`/`I32`/`I64`/`I128`/`Isize`             | —                | structural                                           | `intN_t` / `intptr_t`                                                             |
| `Uint(UintKind)`                              | `U8`/`U16`/`U32`/`U64`/`U128`/`Usize`             | —                | structural                                           | `uintN_t` / `size_t`                                                              |
| `Float(FloatKind)`                            | `F16`/`F32`/`F64`/`F128`                          | —                | structural                                           | `float` / `double` / `_Float16` / `_Float128`                                     |
| `Str`                                         | marker                                            | no (unsized)     | structural                                           | only behind `Ref`/`Ptr`; fat pointer                                              |
| `Never`                                       | marker                                            | no (uninhabited) | structural                                           | `void` + `__attribute__((noreturn))` in fn-return position; unreachable elsewhere |
| `Array(TyId, u64)`                            | element type + length                             | —                | structural                                           | C array `T[N]`                                                                    |
| `Slice(TyId)`                                 | element type                                      | no (unsized)     | structural                                           | only behind `Ref`/`Ptr`; fat pointer                                              |
| `Ref(TyId, Provenance)`                       | pointee + borrow flavor                           | —                | structural                                           | `T*` / `const T*` / `T* restrict`                                                 |
| `Ptr(TyId, Mutability)`                       | pointee + `Not`/`Mut`                             | —                | structural                                           | `T*` / `const T*` (raw)                                                           |
| `FnDef(ItemId, GenericArgs)`                  | callable item + post-mono args                    | —                | **nominal**                                          | ZST — function identity                                                           |
| `FnPtr(FnSigId)`                              | sig handle (carries `FnAbi`, etc.)                | —                | structural (FnSig content expanded by canonicalizer) | C function pointer `T (*)(args)`                                                  |
| `Dynamic { auto_traits, vtable_ty }`          | sorted `AutoTrait` set + vtable's `TyId`          | —                | structural                                           | only behind `Ref`/`Ptr`; fat pointer (codegen-emitted)                            |
| `Closure(ItemId, GenericArgs)`                | synthesised captures-struct Item + post-mono args | —                | **nominal**                                          | identity view; layout via parallel `Ty::Struct`                                   |
| `Tuple { fields: Vec<TyId>, layout: Layout }` | element types + resolved layout                   | **inline**       | structural-with-layout-excluded                      | anonymous C struct (synthetic field names `_0`, `_1`, ...)                        |
| `Struct { item: ItemId, args: GenericArgs, layout: Layout }` | item + args + resolved layout      | **inline**       | **nominal**                                          | C `struct`                                                                        |
| `Enum { item: ItemId, args: GenericArgs, layout: Layout }`   | item + args + resolved layout      | **inline**       | **nominal**                                          | tagged C struct or niche-encoded                                                  |
| `Union { item: ItemId, args: GenericArgs, layout: Layout }`  | item + args + resolved layout      | **inline**       | **nominal**                                          | C `union`                                                                         |

### 3.1 Auxiliary closed enums on `Ty`

| Enum         | Variants                                       | Defined by                                                                          |
| ------------ | ---------------------------------------------- | ----------------------------------------------------------------------------------- |
| `Provenance` | `SharedBorrow`, `UniqueBorrow`                 | D7/D8 (advisory `frozen`/`unpin`/`global` dropped)                                  |
| `Mutability` | `Not`, `Mut` (rustc's enum, mirrored)          | stage 1                                                                             |
| `AutoTrait`  | `Send`, `Sync`, `Unpin` (sorted in this order) | D5 (closed v1; user-defined auto-traits rejected)                                   |
| `FnAbi`      | `Rust`, `C`                                    | D15 (rustc's other 26 `ExternAbi` variants rejected; `C { unwind: true }` rejected) |

---

## 4. Layout Schema (D2 + D11 + D13)

### 4.1 `Layout`

Carried inline on `Ty::Struct/Enum/Union/Tuple`. Resolved by `tcx.layout_of(ty)` and copied — never recomputed.

| Field         | Type          | Carries                                                               |
| ------------- | ------------- | --------------------------------------------------------------------- |
| `size`        | `u64`         | byte size of the type                                                 |
| `align`       | `u64`         | ABI alignment (we drop pref-align)                                    |
| `pack`        | `Option<u32>` | `Some(N)` for `repr(packed(N))`; the only repr knob that's structural |
| `uninhabited` | `bool`        | parent-level uninhabited (e.g., `enum E {}`, struct with `!` field)   |
| `kind`        | `LayoutKind`  | the structural payload (Struct/Union/Enum)                            |

### 4.2 `LayoutKind`

| Variant                               | Fields                    | Used by                                                                        |
| ------------------------------------- | ------------------------- | ------------------------------------------------------------------------------ |
| `Struct { fields: Vec<FieldLayout> }` | named fields with offsets | `Ty::Struct` AND `Ty::Tuple` (tuple field names are synthetic `_0`, `_1`, ...) |
| `Union { fields: Vec<FieldLayout> }`  | fields all at offset 0    | `Ty::Union`                                                                    |
| `Enum { shape: EnumShape }`           | discriminated payload     | `Ty::Enum`                                                                     |

### 4.3 `FieldLayout`

```rust
pub struct FieldLayout {
    pub name:   StringId,    // interned via RegseqModel.strings
    pub ty:     TyId,
    pub offset: u64,         // bytes from the parent's start
}
```

### 4.4 `EnumShape` and `VariantLayout`

| `EnumShape` variant                                                                | Carries                                             |
| ---------------------------------------------------------------------------------- | --------------------------------------------------- |
| `Empty`                                                                            | zero-variant uninhabited enum (no `VariantLayout`s) |
| `Single { variant: VariantLayout }`                                                | single-variant enum                                 |
| `Multiple { tag: TagScalar, encoding: TagEncoding, variants: Vec<VariantLayout> }` | tagged or niche-encoded                             |

```rust
pub struct VariantLayout {
    pub name:         StringId,
    pub fields:       Vec<FieldLayout>,
    pub uninhabited:  bool,         // enables Result<T, !> Err-arm elision
    pub discriminant: u128,         // D13: Rust-level semantic value (Discriminant(place))
    pub tag_value:    Option<u128>, // D13: layout-level tag bytes (SetDiscriminant writes)
}
```

`tag_value` semantics:

| Case                                                 | `tag_value`                                                                                         |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Single variant (`EnumShape::Single`)                 | `None` (no tag)                                                                                     |
| Direct-encoded multiple variant                      | `Some(v)`, `v == discriminant`                                                                      |
| Niche-encoded niched variant                         | `Some(v)`, `v` per D12 niche formula                                                                |
| Niche-encoded **untagged** variant (data IS the tag) | **`None`** — payload is written via the normal field-assignment path, **not** via `SetDiscriminant` |

### 4.5 `TagScalar` and `TagEncoding`

```rust
pub struct TagScalar {
    pub offset:      u64,
    pub scalar_ty:   TyId,            // resolved to Int / Uint / Ptr / Ref
    pub valid_range: WrappingRange,   // always present (per bridge invariant)
}

pub enum TagEncoding {
    Direct,
    Niche {
        untagged_variant: VariantIdx,
        niche_variants:   VariantRange,   // inclusive
        niche_start:      u128,
    },
}
```

### 4.6 What's dropped from rustc's `LayoutData`

| Dropped                                                    | Why                                                       |
| ---------------------------------------------------------- | --------------------------------------------------------- |
| `BackendRepr`                                              | Rust calling-convention hint; irrelevant for C signatures |
| `max_repr_align`, `unadjusted_abi_align`                   | layout-algorithm intermediates                            |
| `randomization_seed`                                       | nightly perf knob                                         |
| `largest_niche`                                            | parent-only consumer; already baked into `tag_encoding`   |
| `ReprOptions::align`, `int`, `IS_C`/`IS_TRANSPARENT` flags | resolved into the bytes already                           |
| `IS_SIMD`/`IS_LINEAR`/`IS_SCALABLE`                        | DSL out of scope; rejected at bridge                      |

---

## 5. Type Identity (D3)

Three identity buckets in the `Ty` BiMap:

| Bucket                                      | Variants                                                             | `Hash + Eq` rule                      | Layout role                                                            |
| ------------------------------------------- | -------------------------------------------------------------------- | ------------------------------------- | ---------------------------------------------------------------------- |
| **Nominal**                                 | `Struct`, `Enum`, `Union`, `FnDef`, `Closure`                        | `(variant_kind_tag, item, args)`      | inline `Layout` *excluded* (functionally determined by `(item, args)`) |
| **Tuple** (structural-with-layout-excluded) | `Tuple`                                                              | `(variant_kind_tag, fields)`          | inline `Layout` *excluded* (functionally determined by `fields`)       |
| **Other structural**                        | Bool/Char/Int/Uint/Float/Str/Array/Slice/Ptr/Ref/FnPtr/Dynamic/Never | full structural recursion over fields | no inline layout                                                       |

The `variant_kind_tag` makes `Ty::Closure(item, args)` and `Ty::Struct { item, args, layout }` for the same closure **distinct entries** (D6.3 relies on this — closures have an identity view + a layout-bearing view sharing the same `Item`).

### 5.1 BiMap shape (`TyInterner`)

```rust
pub struct TyInterner {
    storage: IndexVec<TyId, Ty>,
    lookup:  HashMap<Ty, TyId>,
}

impl TyInterner {
    pub fn new() -> Self;
    pub fn intern(&mut self, ty: Ty) -> TyId;
    pub fn resolve(&self, id: TyId) -> &Ty;
    pub fn find(&self, ty: &Ty) -> Option<TyId>;   // structural lookup; non-mutating
    pub fn len(&self) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = (TyId, &Ty)>;
}
```

Bridge holds a separate transient `HashMap<rustc::Ty<'tcx>, TyId>` translation cache (Q1 forces it bridge-side; Q2 dies with the bridge call).

### 5.2 `ArgsKey` deep canonicalizer (F4-2)

Distinct from the BiMap's in-build `Hash + Eq`. Walks `Ty` content with per-IR-identifier termination rules:

| Inside             | Rule                                                                                                                         |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------- |
| ADT-shaped variant | encode as `(variant_kind_tag, DefPathHash, canonical_args)`; layout NOT traversed (cycle-breaking + functionally determined) |
| Other variant      | structural recursion over fields                                                                                             |
| `TyId` field       | resolve via `model.types.resolve(...)` and recurse on the resolved `Ty`                                                      |
| `ItemId`           | replace with `DefPathHash` from `model.item_def_paths` (cross-build terminator)                                              |
| `FnSigId`          | resolve and **expand** FnSig content per the minimum schema (M-11): `inputs`, `output`, `abi`, `c_variadic`, `safety`        |
| `StringId`         | resolve to content                                                                                                           |
| `ConstId`          | canonical const-eval result (pinned in stage 4)                                                                              |

### 5.3 Cross-build property

| Property                         | Status                                                                     |
| -------------------------------- | -------------------------------------------------------------------------- |
| Set of distinct `Ty` values      | **identical** across builds under pinned toolchain                         |
| Each `Ty`'s structural shape     | **identical**                                                              |
| Each `Layout`                    | **identical**                                                              |
| `InstanceId` / `ItemId` ordering | **deterministic** per F4 / F4-2 (computed over `Ty` content, not integers) |
| `TyId` integer values            | **NOT guaranteed stable** — bridge discovery walk may differ between runs  |
| Cross-build identity check       | use the deep `ArgsKey` canonicalizer (compares `Ty` content)               |
| Snapshot diffing                 | content-based via the pretty-printer (resolves `TyId` to `Ty`)             |

A bridge implementation MAY produce reproducible `TyId` integers as a recommended deterministic property, but no IR contract relies on it.

---

## 6. Bridge Translation Map (rustc → RegseqModel)

What the bridge does for each rustc `TyKind` after `instantiate_mir_and_normalize_erasing_regions`:

| rustc `TyKind`                                           | RegseqModel                              | Notes                                                                                                 |
| -------------------------------------------------------- | ---------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `Bool`, `Char`, `Int(_)`, `Uint(_)`, `Float(_)`, `Never` | `Ty::Bool` / `Char` / `Int(...)` / etc.  | direct                                                                                                |
| `Str`                                                    | `Ty::Str`                                | only valid behind `Ref`/`Ptr`                                                                         |
| `Array(t, n)`                                            | `Ty::Array(translate(t), n)`             | `n` is post-mono `u64`                                                                                |
| `Slice(t)`                                               | `Ty::Slice(translate(t))`                | only valid behind `Ref`/`Ptr`                                                                         |
| `Ref(_region, t, Mutability::Not)`                       | `Ty::Ref(translate(t), SharedBorrow)`    | non-HRTB parent only; HRTB parent rejected                                                            |
| `Ref(_region, t, Mutability::Mut)`                       | `Ty::Ref(translate(t), UniqueBorrow)`    | same                                                                                                  |
| `RawPtr(t, mutbl)`                                       | `Ty::Ptr(translate(t), mutbl)`           |                                                                                                       |
| `Tuple([t0..tn])`                                        | `Ty::Tuple { fields, layout }`           | layout from `tcx.layout_of`                                                                           |
| `Adt(struct)`                                            | `Ty::Struct { item, args, layout }`      |                                                                                                       |
| `Adt(enum)`                                              | `Ty::Enum { item, args, layout }`        |                                                                                                       |
| `Adt(union)`                                             | `Ty::Union { item, args, layout }`       |                                                                                                       |
| `FnDef(did, args)`                                       | `Ty::FnDef(item, args)`                  | ZST                                                                                                   |
| `FnPtr(binder)`                                          | `Ty::FnPtr(intern(FnSig))`               | **HRTB-bearing rejected**                                                                             |
| `Dynamic(preds, _region, _kind)`                         | `Ty::Dynamic { auto_traits, vtable_ty }` | non-HRTB only; **HRTB-bearing rejected**; `vtable_ty` resolves to a stage-12-synthesised `Ty::Struct` |
| `Closure(did, args)`                                     | `Ty::Closure(synthesised_item, args)`    | parallel `Ty::Struct { item: synthesised_item, args, layout }` interned for layout view               |

### 6.1 Rejections

| Rejection                                                                 | Phase                               | Reason                                                                                |
| ------------------------------------------------------------------------- | ----------------------------------- | ------------------------------------------------------------------------------------- |
| Non-rigid `TyKind` (Param/Bound/Placeholder/Infer/Alias/Error/Pat)        | type translation                    | should not survive normalization                                                      |
| `Foreign(_)`                                                              | type translation (D1.4)             | unstable; opaque-struct pattern recommended                                           |
| `Box<T>`                                                                  | type translation (D1.4)             | unstable; allocator-bearing                                                           |
| `Coroutine` family                                                        | type translation (D1.4)             | async/coroutine out of scope                                                          |
| `UnsafeBinder`                                                            | type translation (D1.4)             | unstable                                                                              |
| `Pat(...)`                                                                | type translation (D1.4)             | unstable                                                                              |
| `core::ffi::c_void`                                                       | type translation (D16)              | recognized via `tcx.is_lang_item(LangItem::CVoid)`; opaque-struct pattern recommended |
| `core::sync::atomic::Atomic*` types                                       | type translation (D10)              | use `regseq::Atomic<T>`                                                               |
| `core::sync::atomic::Atomic*::*()` methods + `core::intrinsics::atomic_*` | call translation (D10)              | use `#[rseq::intrinsic("atomic_*")]`                                                  |
| `core::sync::atomic::{fence, compiler_fence}`                             | call translation (D10 v1 policy)    | future barrier-construct decision                                                     |
| `core::ptr::{read,write}_volatile`                                        | call translation (D9)               | use `#[rseq::intrinsic("mmio_*")]`                                                    |
| `extern "C" { unwind: true }` and other `ExternAbi` variants              | type translation (D15)              | only `Rust` and `C` accepted                                                          |
| **HRTB-bearing `Ty::FnPtr`** (`for<'a> fn(&'a T)`)                        | type translation (D7/D8 refinement) | `ReBound` survives erasure; would collapse rustc-distinct types                       |
| **HRTB-bearing `Ty::Dynamic`** (`dyn for<'a> Trait<&'a T>`)               | type translation (D7/D8 refinement) | same reason                                                                           |
| Custom auto-traits (via `feature(auto_traits)`)                           | type translation (D5)               | closed v1 set is `{Send, Sync, Unpin}`                                                |

---

## 7. Lang-Item Flags on `Item` (D1.5)

These are flags on **`Item`**, not `Ty`. The bridge populates them via `adt_def.is_*()` predicates:

| Flag               | Recognition                  | rustc cache                  |
| ------------------ | ---------------------------- | ---------------------------- |
| `is_phantom_data`  | `adt_def.is_phantom_data()`  | `AdtFlags::IS_PHANTOM_DATA`  |
| `is_unsafe_cell`   | `adt_def.is_unsafe_cell()`   | `AdtFlags::IS_UNSAFE_CELL`   |
| `is_manually_drop` | `adt_def.is_manually_drop()` | `AdtFlags::IS_MANUALLY_DROP` |

Lang items recognized at **call** time, not flagged on `Item`:

| Lang item                                           | Disposition                                                                              |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `MaybeUninit`                                       | not flagged; composition (Union no-auto-drop + inner `ManuallyDrop`) suffices            |
| `CVoid`                                             | not flagged; recognized via `tcx.is_lang_item(LangItem::CVoid)` only to **reject** (D16) |
| `Pin`                                               | reintroduce `is_pin` when async support lands                                            |
| `OwnedBox`, `VaList`, `GlobalAlloc`, `UnsafePinned` | not in v1 scope                                                                          |

---

## 8. Synthesised Items (`Item`s the bridge mints, not in source)

| Synthesised Item                            | Per                                                | Synthesised by | Why                                                                                                                    |
| ------------------------------------------- | -------------------------------------------------- | -------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Closure captures-struct Item                | per closure                                        | bridge (D6.1)  | `Ty::Closure` and `Ty::Struct` views share this Item                                                                   |
| Closure call-method Function Item(s)        | per closure × (1 if FnOnce / 2 if FnMut / 3 if Fn) | bridge (D6.1)  | rustc closure trait method projections; **ABI rewritten from `RustCall` to `Rust`** after tuple-receiver normalization |
| Closure drop-glue Function Item             | per closure with non-trivial captures drop         | bridge (D6.1)  | standard drop-glue handling                                                                                            |
| Vtable type Item (`ItemKind::Struct`)       | per `(Trait, projection_bindings)`                 | stage 12       | `Ty::Dynamic.vtable_ty` resolves to its `Ty::Struct`                                                                   |
| Vtable instance Item (`ItemKind::Static`)   | per `(Trait, ConcreteImpl)`                        | stage 12       | the `static const` vtable values                                                                                       |
| **Vtable method shim** Function Item        | per `(Trait, ConcreteImpl, method)`                | stage 12 (C.1) | bridges erased-receiver vtable slot to concrete impl method                                                            |
| **Vtable drop-in-place shim** Function Item | per `(Trait, ConcreteImpl)`                        | stage 12 (C.2) | bridges erased-receiver vtable slot to concrete `drop_in_place::<SelfTy>`                                              |
| Drop-glue Function Item (general)           | per type with non-trivial drop                     | bridge         | rustc's `DropGlue` is `core::ptr::drop_in_place::<T>`                                                                  |

The `RegseqModel.vtables: HashMap<(DefPathHash, ArgsKey), ItemId>` dedup table lives bridge-side, rebuilt on deserialize.

---

## 9. `FnSig` Minimum Canonical Schema (D15 + M-11)

Stage 11 owns the full `FnSig` body table; stage 3 pins the minimum the canonicalizer needs:

| Field        | Type                      | Carries                                               |
| ------------ | ------------------------- | ----------------------------------------------------- |
| `inputs`     | `Vec<TyId>`               | positional argument types                             |
| `output`     | `TyId`                    | return type                                           |
| `abi`        | `FnAbi`                   | `Rust` or `C` (load-bearing for type identity per Q2) |
| `c_variadic` | `bool`                    | trailing `...`; distinct types in rustc               |
| `safety`     | `Safety { Safe, Unsafe }` | `unsafe fn` marker; distinct types in rustc           |

Stage 11 may add fields beyond this minimum (parameter names for diagnostics, etc.) — those don't affect type identity and are ignored by the canonicalizer. **No side-channel ABI table** — `FnSig::abi` is the only place ABI is stored (Q2 invariant).

---

## 10. What's Deferred / Open

| Item                                                    | Stage                                                         | Status                                                                |
| ------------------------------------------------------- | ------------------------------------------------------------- | --------------------------------------------------------------------- |
| **D14** — pretty-printer renderer style for `Ty`        | stage 13 (deferred from stage 3)                              | **DEFERRED** — see `../stage-13/expectations.md`                       |
| Const-generics canonicalization                         | stage 4                                                       | deferred (consumed by `ArgsKey` rule)                                 |
| Stage-9 dynamic-call terminator shape                   | stage 9                                                       | choice between Virtual InstanceKind variant vs. widened `Call.callee` |
| Body-level dyn-Trait dispatch rewriting                 | stage 11                                                      | walks bodies; rewrites `<dyn T as T>::method` calls                   |
| `LocalId`/`BlockId` const-body macro arm                | stage 10 or 11                                                | first stage to land does it                                           |
| Pin reintroduction (`is_pin` flag)                      | when async support lands                                      | future                                                                |
| MaybeUninit flag                                        | revisit if Union+ManuallyDrop composition proves insufficient | future                                                                |
| `c_void` flag (`is_c_void`)                             | when intrinsic uses `*mut c_void` motivate it                 | future (D16 reopen)                                                   |
| HRTB encoding                                           | when fixture/intrinsic surfaces a stable need                 | future (D7/D8 reopen)                                                 |
| `core::sync::atomic::{fence, compiler_fence}` admission | future barrier-construct decision                             | future                                                                |

Phase 2 (pl011 device pressure) effectively waits on enough of the above to stabilize — phase-1 spec gaps surfaced by hand-lowering get fixed back here.