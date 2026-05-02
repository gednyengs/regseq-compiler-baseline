# Stage 3 -- rustc type-system research (draft)

Status: empirical evidence draft. Not the final stage-3
`findings.md` -- this file is a staging area for the rustc-side
half of the research (the other half is prior-art). Synthesis
into the merged `findings.md` happens after both drafts land.

All citations are against the pinned toolchain rooted at
`~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/`
(see `../rustc-references.md`). Paths below are relative to that
root unless otherwise stated. Quoted variant lists are verbatim
from the source as of this nightly.

This document records what rustc *does*. It does not propose
RegseqModel design choices.

---

## Theme A -- Type-vocabulary completeness

### A.1 The interner-generic `TyKind`

The shared, interner-generic enum lives at
`rustc_type_ir/src/ty_kind.rs:67-259`. It is generic over an
`Interner` and is the single source of truth for every variant
both `rustc_middle::ty::Ty<'tcx>` and the `rustc_public` owned
form ultimately specialise. Verbatim variant list:

| Variant | Args | line |
| --- | --- | --- |
| `Bool` | -- | `:69` |
| `Char` | -- | `:73` |
| `Int(IntTy)` | signed primitive | `:76` |
| `Uint(UintTy)` | unsigned primitive | `:79` |
| `Float(FloatTy)` | float primitive | `:82` |
| `Adt(I::AdtDef, I::GenericArgs)` | struct/enum/union | `:91` |
| `Foreign(I::ForeignId)` | `extern type T` -- unsized opaque FFI | `:94` |
| `Str` | `str` (DST pointee) | `:97` |
| `Array(I::Ty, I::Const)` | `[T; N]` | `:100` |
| `Pat(I::Ty, I::Pat)` | pattern newtype, integer ranges only today | `:107` |
| `Slice(I::Ty)` | `[T]` (DST pointee) | `:110` |
| `RawPtr(I::Ty, Mutability)` | `*const T` / `*mut T` | `:113` |
| `Ref(I::Region, I::Ty, Mutability)` | `&'a T` / `&'a mut T` | `:117` |
| `FnDef(I::FunctionId, I::GenericArgs)` | the *anonymous ZST* type of a function decl/def | `:130` |
| `FnPtr(ty::Binder<I, FnSigTys<I>>, FnHeader<I>)` | `fn(...) -> T` pointer | `:148` |
| `UnsafeBinder(UnsafeBinderInner<I>)` | "higher-ranked type with some lifetimes erased" | `:155` |
| `Dynamic(I::BoundExistentialPredicates, I::Region)` | `dyn Trait + 'a` | `:158` |
| `Closure(I::ClosureId, I::GenericArgs)` | anonymous closure type | `:165` |
| `CoroutineClosure(I::CoroutineClosureId, I::GenericArgs)` | `async \|...\|` closure | `:172` |
| `Coroutine(I::CoroutineId, I::GenericArgs)` | anonymous coroutine type | `:179` |
| `CoroutineWitness(I::CoroutineId, I::GenericArgs)` | "types stored inside a coroutine" -- only inside `CoroutineArgs` | `:204` |
| `Never` | `!` | `:207` |
| `Tuple(I::Tys)` | `(A, B, ...)` | `:210` |
| `Alias(AliasTyKind, AliasTy<I>)` | projection / opaque / free type-alias / inherent assoc | `:215` |
| `Param(I::ParamTy)` | type parameter | `:218` |
| `Bound(BoundVarIndexKind, ty::BoundTy<I>)` | bound type variable in a `for<>` binder | `:236` |
| `Placeholder(ty::PlaceholderType<I>)` | placeholder for `Bound` during higher-ranked subtyping | `:246` |
| `Infer(InferTy)` | type-inference variable | `:254` |
| `Error(I::ErrorGuaranteed)` | error sentinel for diagnostic propagation | `:258` |

`AliasTyKind` (the four sub-kinds of `Alias`) at
`rustc_type_ir/src/ty_kind.rs:30-43`:

- `Projection` -- `<T as Trait>::Item`. "Can get normalized away if monomorphic enough."
- `Inherent` -- inherent assoc type from an `impl`.
- `Opaque` -- usually `impl Trait` returns/TAITs. "Can only be normalized away in PostAnalysis mode or its defining scope."
- `Free` -- a type alias that checks its trait bounds. Used only when the alias references opaque types. "Can always be normalized away."

`is_known_rigid()` at `:289-322` partitions the variant set into "rigid" (everything before `Alias`/`Param`/`Bound`/`Placeholder`/`Infer`/`Error`) and "non-rigid". This rigid boundary corresponds exactly to what `rustc_public::TyKind` collapses into the `RigidTy` newtype (see `rustc_public/src/ty.rs:307-312, 546-569`); the public form exposes only `RigidTy`, `Alias`, `Param`, `Bound`. There are no public-side `Placeholder`, `Infer`, `Error`, or `UnsafeBinder` variants. (`UnsafeBinder` is unstable and only flows through typechecking; comment at `ty_kind.rs:152-155` states it is used "to represent types in positions where a lifetime is literally inexpressible, such as self-referential types.")

### A.2 Primitive sub-enums

In `rustc_ast_ir/src/lib.rs`:

- `IntTy` (`:28-35`): `Isize, I8, I16, I32, I64, I128`.
- `UintTy` (`:107-114`): `Usize, U8, U16, U32, U64, U128`.
- `FloatTy` (`:186-191`): `F16, F32, F64, F128`. Bit-widths at `:213-220` (16/32/64/128).
- `Mutability` (`:248-252`): `Not, Mut`. Comment notes "Order is deliberate, so that Not < Mut".

`IntTy::normalize(target_width)` at `:72-82` rewrites `Isize` to a fixed-width variant given a 16/32/64 target. `UintTy::normalize` mirrors this. `bit_width()` returns `None` for `Isize`/`Usize` (`:61-69, :140-148`) -- target-dependent.

### A.3 Function types -- `FnDef` vs `FnPtr` vs closures

`FnDef` doc-comment at `rustc_type_ir/src/ty_kind.rs:119-130`: "The anonymous type of a function declaration/definition. Each function has a unique type." For `fn foo() -> i32 { 3 }`, `let bar = foo` gives `bar` the type `fn() -> i32 {foo}` -- a ZST whose name carries the function's `DefId`. *Distinct* from `FnPtr` at `:148`. The doc-comment at `:132-147` notes `FnPtr` is split into `Binder<FnSigTys>` plus `FnHeader` "to reduce the size of `TyKind` by 8 bytes" -- they recompose via `FnSigTys::with(hdr)` at `:931-938`.

`FnSig<I>` at `:734-743`:
```
inputs_and_output: I::Tys,
c_variadic:        bool,
safety:            I::Safety,
abi:               I::Abi,
```

`FnHeader<I>` at `:967-971`:
```
c_variadic: bool,
safety:     I::Safety,
abi:        I::Abi,
```

`FnSigTys<I>` at `:913-915`:
```
inputs_and_output: I::Tys,
```

`Closure` (`:165`), `CoroutineClosure` (`:172`), `Coroutine` (`:179`), `CoroutineWitness` (`:204`) all carry `(I::*Id, I::GenericArgs)`. The args are *structured*.

`ClosureArgs<I>` at `rustc_type_ir/src/ty_kind/closure.rs:108-115` wraps `args: I::GenericArgs`; `split()` at `:148-150` decomposes them into `ClosureArgsParts<I>` (`:120-131`):
```
parent_args:               I::GenericArgsSlice,
closure_kind_ty:           I::Ty,
closure_sig_as_fn_ptr_ty:  I::Ty,
tupled_upvars_ty:          I::Ty,
```

`CoroutineClosureArgsParts<I>` at `:222-247` adds:
```
parent_args:                  I::GenericArgsSlice,
closure_kind_ty:              I::Ty,
signature_parts_ty:           I::Ty,
tupled_upvars_ty:             I::Ty,
coroutine_captures_by_ref_ty: I::Ty,
```

`tupled_upvars_ty()` (`closure.rs:172-174, :280-282`); `upvar_tys()` (`:161-168, :270-277`) panics if called before upvar capture analysis has finished, asserts the tuple shape.

`ClosureKind` (`Fn`/`FnMut`/`FnOnce`) is encoded as a *type* in the closure-kind-ty slot (cf. `closure.rs:179-198`).

### A.4 Trait objects

`Dynamic(I::BoundExistentialPredicates, I::Region)` at `ty_kind.rs:158`. Doc-comment: "A trait object. Written as `dyn for<'b> Trait<'b, Assoc = u32> + Send + 'a`." The `Region` is the trait-object's outer lifetime bound. The `BoundExistentialPredicates` carry the principal trait, projection bounds, and auto-trait bounds.

Public-side `ExistentialPredicate` at `rustc_public/src/ty.rs:1214-1218`:
```
Trait(ExistentialTraitRef),
Projection(ExistentialProjection),
AutoTrait(TraitDef),
```

### A.5 ADT structure (`Adt`)

`AdtDef<'tcx>` is interned -- `rustc_middle/src/ty/adt.rs:175-178`. Equality and hashing keyed *only* on `DefId` (`:111-147`); assertion at `:127-132` enforces that two `AdtDefData` for the same `did` must agree on every other field.

`AdtDefData` (`:99-109`):
```
did:      DefId,
variants: IndexVec<VariantIdx, VariantDef>,
flags:    AdtFlags,
repr:     ReprOptions,
```

`AdtKind` at `:257-261`: `Struct, Union, Enum`.

`VariantDef` at `rustc_middle/src/ty/mod.rs:1117-1135`:
```
def_id:  DefId,
ctor:    Option<(CtorKind, DefId)>,
name:    Symbol,
discr:   VariantDiscr,
fields:  IndexVec<FieldIdx, FieldDef>,
tainted: Option<ErrorGuaranteed>,
flags:   VariantFlags,
```

`FieldDef` at `:1326-1333`:
```
did:    DefId,
name:   Symbol,
vis:    Visibility<DefId>,
safety: hir::Safety,
value:  Option<DefId>,
```

`AdtDef` does *not* embed field types directly. Doc-comment at `adt.rs:74-98`: "`AdtDef` does not actually include the *types* of its fields; it includes just their `DefId`s." Field types are looked up via `tcx.type_of(field.did)` and (since they may be generic) returned as `EarlyBinder<Ty<'tcx>>`. This is the avoidance mechanism for `struct S { x: Box<S> }`.

`AdtDef::all_field_tys` at `adt.rs:227-234` is the canonical traversal.

### A.6 Box, never, foreign, tuples

- **Box.** No dedicated `TyKind` variant. `Box<T>` is `TyKind::Adt(AdtDef, GenericArgs)` whose `AdtDef` carries `IS_BOX` (set when lang item is `OwnedBox`, see `rustc_middle/src/ty/adt.rs:312-314` and `rustc_hir/src/lang_items.rs:352`). Detection: `AdtDef::is_box()` (`adt.rs:421`); also `rustc_public/src/ty.rs:462-467`.
- **Never.** Dedicated `TyKind::Never` at `:207`.
- **Foreign.** Dedicated `TyKind::Foreign(I::ForeignId)` at `:94`, doc "An unsized FFI type that is opaque to Rust. Written as `extern type T`." Unsized (`rustc_middle/src/ty/util.rs:238`).
- **Tuples.** Dedicated `TyKind::Tuple(I::Tys)` at `:210`. Unit type is `Tuple(empty list)`; `is_unit()` at `rustc_public/src/ty.rs:320-322` matches that.

### A.7 Patterns, slices, arrays

- `TyKind::Array(I::Ty, I::Const)` at `:100`.
- `TyKind::Slice(I::Ty)` at `:110` -- the *pointee*; appears behind a pointer/reference.
- `TyKind::Pat(I::Ty, I::Pat)` at `:107` -- "pattern newtype", integer-range only today.

### A.8 Pointers vs references

- `TyKind::RawPtr(I::Ty, Mutability)` at `:113`.
- `TyKind::Ref(I::Region, I::Ty, Mutability)` at `:117`.

`Mutability` from `rustc_ast_ir/src/lib.rs:248`. The `Region` slot for `Ref` becomes `ReErased` after `tcx.erase_and_anonymize_regions(...)` (Theme E).

### A.9 ABI on function types

`FnHeader.abi: I::Abi`. Full set at `rustc_abi/src/extern_abi.rs:20-119`:
```
C { unwind: bool },
System { unwind: bool },
Rust,
RustCall,
RustCold,
RustInvalid,
RustPreserveNone,
Unadjusted,
Custom,
EfiApi,
Aapcs { unwind: bool },
CmseNonSecureCall,
CmseNonSecureEntry,
GpuKernel,
PtxKernel,
AvrInterrupt,
AvrNonBlockingInterrupt,
Msp430Interrupt,
RiscvInterruptM,
RiscvInterruptS,
X86Interrupt,
Cdecl { unwind: bool },
Stdcall { unwind: bool },
Fastcall { unwind: bool },
Thiscall { unwind: bool },
Vectorcall { unwind: bool },
SysV64 { unwind: bool },
Win64 { unwind: bool },
```

"Custom" (`:56-59`) -- "An ABI that rustc does not know how to call or define. Functions with this ABI can only be created using `#[naked]` functions or `extern "custom"` blocks". Public-side `Abi` at `rustc_public/src/ty.rs:1116-1145` mirrors these.

---

## Theme B -- Layout / representation

Layouts live in `rustc_abi/src/lib.rs` and `rustc_abi/src/layout/ty.rs`. Computed on demand by `tcx.layout_of(typing_env.as_query_input(ty))` (see `rustc_middle/src/ty/layout.rs:391, :746`). Layouts are *not* embedded in `TyKind`; derived from `(ty, typing_env)`.

### B.1 `LayoutData`

`rustc_abi/src/lib.rs:2010-2068`:
```
fields:               FieldsShape<FieldIdx>,
variants:             Variants<FieldIdx, VariantIdx>,
backend_repr:         BackendRepr,
largest_niche:        Option<Niche>,
uninhabited:          bool,
align:                AbiAlign,
size:                 Size,
max_repr_align:       Option<Align>,
unadjusted_abi_align: Align,
randomization_seed:   Hash64,
```

Interned wrapper `Layout<'a>` at `rustc_abi/src/layout/ty.rs:68-70`:
```
pub struct Layout<'a>(pub Interned<'a, LayoutData<FieldIdx, VariantIdx>>);
```

`TyAndLayout<'a, Ty>` at `:127-131`:
```
ty:     Ty,
layout: Layout<'a>,
```

### B.2 `FieldsShape`

`rustc_abi/src/lib.rs:1615-1648`:
```
Primitive,
Union(NonZeroUsize),
Array { stride: Size, count: u64 },
Arbitrary {
    offsets: IndexVec<FieldIdx, Size>,
    in_memory_order: IndexVec<u32, FieldIdx>,
},
```

- `Primitive` -- "Scalar primitives and `!`, which never have fields" (`:1616-1617`).
- `Union(count)` -- "All fields start at no offset" (`:1619`). `NonZeroUsize` so 0-field unions are unrepresentable.
- `Array { stride, count }` -- "Array/vector-like placement, with all fields of identical types" (`:1622-1623`).
- `Arbitrary { offsets, in_memory_order }` -- struct-like; `offsets` in *source* order; `in_memory_order` maps memory-order indices to source indices. Doc-comment at `:1626-1647`: "Fields are guaranteed to not overlap, but note that gaps before, between and after all the fields are NOT always padding ... For example, enum variants leave a gap at the start, where the discriminant field in the enum layout goes."

### B.3 `Variants`

`rustc_abi/src/lib.rs:1858-1880`:
```
Empty,
Single { index: VariantIdx },
Multiple {
    tag:          Scalar,
    tag_encoding: TagEncoding<VariantIdx>,
    tag_field:    FieldIdx,
    variants:     IndexVec<VariantIdx, LayoutData<FieldIdx, VariantIdx>>,
},
```

- `Empty` -- "A type with no valid variants. Must be uninhabited."
- `Single { index }` -- "Single enum variants, structs/tuples, unions, and all non-ADTs." `index` always `0` for non-multi-variant.
- `Multiple { ... }` -- enum layout. Each variant has its own `LayoutData`; parent's `fields` shape includes the tag (`tag_field`).

### B.4 `TagEncoding`

`rustc_abi/src/lib.rs:1885-1922`:
```
Direct,
Niche {
    untagged_variant: VariantIdx,
    niche_variants:   RangeInclusive<VariantIdx>,
    niche_start:      u128,
},
```

For `Niche`: variant `i != untagged_variant` has `tag = (i - niche_variants.start).wrapping_add(niche_start)`. Example given: `Option<(usize, &T)>` where `None` is "the null pointer in the second tuple field" and `Some` carries no extra tag. "Other variants that are not `untagged_variant` and that are outside the `niche_variants` range cannot be represented; they must be uninhabited" (`:1909-1912`).

### B.5 `Niche`

`rustc_abi/src/lib.rs:1926-1930`:
```
offset:      Size,
value:       Primitive,
valid_range: WrappingRange,
```

`Primitive` at `:1351-1362`:
```
Int(Integer, bool),  // bool = signedness
Float(Float),
Pointer(AddressSpace),
```

`WrappingRange` at `:1397-1402` (`start: u128`, `end: u128`, inclusive, wrap-around if `start > end`).

### B.6 `Scalar`

`rustc_abi/src/lib.rs:1511-1528`:
```
Initialized {
    value:       Primitive,
    valid_range: WrappingRange,
},
Union {
    value: Primitive,
},
```

`Scalar::Union` (`:1520-1527`) "never have niches and even allow undef, so there is no `valid_range`."

### B.7 `BackendRepr`

`rustc_abi/src/lib.rs:1717-1735`:
```
Scalar(Scalar),
ScalarPair(Scalar, Scalar),
ScalableVector { element: Scalar, count: u64 },
SimdVector    { element: Scalar, count: u64 },
Memory { sized: bool },
```

"How the value will be lowered to the calling convention" (*not* psABI guarantee) -- doc `:1707-1716`.

### B.8 `Integer` and `Float` (layout-side primitives)

Layout-side; don't carry signedness (lives on `Primitive::Int`) and don't include `Isize`/`Usize` (resolve via target's `ptr_sized_integer()`).

`Integer` at `:1156-1162`: `I8, I16, I32, I64, I128`.
`Float` at `:1316-1320`: `F16, F32, F64, F128`.
`IntegerType` at `:122-129`:
```
Pointer(bool),       // Pointer(true) means isize
Fixed(Integer, bool),
```
`IntegerType` is what `repr(<int>)` on enums encodes.

### B.9 `ReprOptions` and `ReprFlags`

`rustc_abi/src/lib.rs:159-174`:
```
int:                 Option<IntegerType>,
align:               Option<Align>,
pack:                Option<Align>,
flags:               ReprFlags,
scalable:            Option<ScalableElt>,
field_shuffle_seed:  Hash64,
```

`ReprFlags` (`:82-107`):
```
IS_C                            = 1 << 0,
IS_SIMD                         = 1 << 1,
IS_TRANSPARENT                  = 1 << 2,
IS_LINEAR                       = 1 << 3,
RANDOMIZE_LAYOUT                = 1 << 4,
PASS_INDIRECTLY_IN_NON_RUSTIC_ABIS = 1 << 5,
IS_SCALABLE                     = 1 << 6,
```

Convenience accessors at `:176-205`: `simd()`, `scalable()`, `c()`, `packed() = self.pack.is_some()`, `transparent()`, `linear()`. `discr_type()` at `:214-216` returns `IntegerType::Pointer(true)` (= `isize`) as default enum tag type.

`inhibit_struct_field_reordering()` at `:231-233` -- true for any of `IS_C`, `IS_SIMD`, `IS_SCALABLE`, `IS_LINEAR`, or any `repr(<int>)`. `field_shuffle_seed` (`:92-95`) drives randomized layout when not inhibited.

### B.10 `PointerKind` and `PointeeInfo`

`rustc_abi/src/lib.rs:2123-2151`:
```
pub enum PointerKind {
    SharedRef  { frozen: bool },
    MutableRef { unpin:  bool },
    Box        { unpin:  bool, global: bool },
}
pub struct PointeeInfo {
    safe:  Option<PointerKind>,
    size:  Size,
    align: Align,
}
```

Comments load-bearing for the C-codegen story:
- `SharedRef.frozen` (`:2125`) "indicates the absence of any `UnsafeCell`."
- `MutableRef.unpin` (`:2127`) "indicates the absence of any pinned data."
- `Box.global` (`:2129`) "indicates whether this box uses the global allocator or a custom one."
- `PointeeInfo.safe = None` (`:2140-2142`) means "this is a raw pointer".

Doc at `:2134-2137`: "if the information is wrong, that can cause UB, but if the information is absent, that must always be okay."

This is rustc's mechanism for what stage-3/expectations.md calls the `provenance` field. Pre-computed during layout, keyed on `(ty, offset)`.

### B.11 Layout entry point

Query `tcx.layout_of(typing_env.as_query_input(ty))` yields `Result<TyAndLayout<'tcx>, &'tcx LayoutError<'tcx>>` (`rustc_middle/src/ty/layout.rs:755`). Codegen consumes layouts exclusively through this query.

---

## Theme C -- Identity, interning, canonical form

### C.1 `Ty<'tcx>` as `Interned<WithCachedTypeInfo<TyKind<'tcx>>>`

The interner is `CtxtInterners<'tcx>` at `rustc_middle/src/ty/context.rs:921-952`. Relevant fields:
```
type_:                       InternedSet<'tcx, WithCachedTypeInfo<TyKind<'tcx>>>,
const_lists:                 InternedSet<'tcx, List<ty::Const<'tcx>>>,
args:                        InternedSet<'tcx, GenericArgs<'tcx>>,
type_lists:                  InternedSet<'tcx, List<Ty<'tcx>>>,
canonical_var_kinds:         InternedSet<'tcx, List<CanonicalVarKind<'tcx>>>,
region:                      InternedSet<'tcx, RegionKind<'tcx>>,
poly_existential_predicates: InternedSet<'tcx, List<PolyExistentialPredicate<'tcx>>>,
predicate:                   InternedSet<'tcx, WithCachedTypeInfo<ty::Binder<'tcx, PredicateKind<'tcx>>>>,
clauses:                     InternedSet<'tcx, ListWithCachedTypeInfo<Clause<'tcx>>>,
projs:                       InternedSet<'tcx, List<ProjectionKind>>,
place_elems:                 InternedSet<'tcx, List<PlaceElem<'tcx>>>,
const_:                      InternedSet<'tcx, WithCachedTypeInfo<ty::ConstKind<'tcx>>>,
pat:                         InternedSet<'tcx, PatternKind<'tcx>>,
const_allocation:            InternedSet<'tcx, Allocation>,
bound_variable_kinds:        InternedSet<'tcx, List<ty::BoundVariableKind<'tcx>>>,
layout:                      InternedSet<'tcx, LayoutData<FieldIdx, VariantIdx>>,
adt_def:                     InternedSet<'tcx, AdtDefData>,
external_constraints:        InternedSet<'tcx, ExternalConstraintsData<TyCtxt<'tcx>>>,
predefined_opaques_in_body:  InternedSet<'tcx, List<(ty::OpaqueTypeKey<'tcx>, Ty<'tcx>)>>,
fields:                      InternedSet<'tcx, List<FieldIdx>>,
local_def_ids:               InternedSet<'tcx, List<LocalDefId>>,
captures:                    InternedSet<'tcx, List<&'tcx ty::CapturedPlace<'tcx>>>,
valtree:                     InternedSet<'tcx, ty::ValTreeKind<TyCtxt<'tcx>>>,
patterns:                    InternedSet<'tcx, List<ty::Pattern<'tcx>>>,
outlives:                    InternedSet<'tcx, List<ty::ArgOutlivesPredicate<'tcx>>>,
```

`intern_ty` at `:995-1011`:
```
fn intern_ty(&self, kind: TyKind<'tcx>, sess: &Session, untracked: &Untracked) -> Ty<'tcx> {
    Ty(Interned::new_unchecked(
        self.type_
            .intern(kind, |kind| {
                let flags = ty::FlagComputation::<TyCtxt<'tcx>>::for_kind(&kind);
                let stable_hash = self.stable_hash(&flags, sess, untracked, &kind);
                InternedInSet(self.arena.alloc(WithCachedTypeInfo {
                    internee: kind,
                    stable_hash,
                    flags: flags.flags,
                    outer_exclusive_binder: flags.outer_exclusive_binder,
                }))
            })
            .0,
    ))
}
```

`Ty<'tcx>` is structurally interned over `TyKind`. Two syntactically-equal `TyKind` values produce the same `Ty<'tcx>` pointer. No separate "hash key" -- `TyKind`'s `Hash`/`PartialEq` derives suffice.

### C.2 ADT identity

`AdtDefData::eq` (`rustc_middle/src/ty/adt.rs:111-136`) and `Hash` (`:142-147`) key only on `did: DefId`. Two `AdtDef` references for the same `DefId` point at the same `AdtDefData`. Deep-equality assertion at `:127-132` (under `cfg!(debug_assertions)`) sanity-checks variants/flags/repr; not part of equality semantics.

`mk_adt_def` populates the interner; subsequent lookups reuse the same interned `AdtDefData` regardless of how many `Adt(DefId, GenericArgs)` references exist.

### C.3 Layouts

`layout: InternedSet<'tcx, LayoutData<FieldIdx, VariantIdx>>` at `context.rs:942`. Structural interning -- two types with the same `LayoutData` produce the same `Layout<'a>` pointer.

### C.4 Public-form identity

`rustc_public::Ty` (`rustc_public/src/ty.rs:1593`) is a newtype index (`pub struct Ty(usize)`), assigned by the bridge's `Tables` at population time. Identity is *not* structural -- it is an opaque ID with a bridge-maintained table.

### C.5 The `Symbol` interner

Type names live in `Symbol`, interned globally per session. Lang-item recognition (Theme D) keys on `Symbol` identity. References at `rustc_ast_ir/src/lib.rs:50, :129, :204`.

---

## Theme D -- Special-semantics markers

There are *no* dedicated `TyKind` variants for `UnsafeCell`, `MaybeUninit`, `PhantomData`, `ManuallyDrop`, `Box`, `Pin`, or atomic types. All recognised via lang-items, with some additionally cached as `AdtFlags` bits.

### D.1 Lang items recognised at `AdtDef` construction

`rustc_middle/src/ty/adt.rs:283-326` (in `AdtDefData::new`):
```
if tcx.is_lang_item(did, LangItem::PhantomData)  { flags |= AdtFlags::IS_PHANTOM_DATA; }
if tcx.is_lang_item(did, LangItem::OwnedBox)     { flags |= AdtFlags::IS_BOX; }
if tcx.is_lang_item(did, LangItem::ManuallyDrop) { flags |= AdtFlags::IS_MANUALLY_DROP; }
if tcx.is_lang_item(did, LangItem::UnsafeCell)   { flags |= AdtFlags::IS_UNSAFE_CELL; }
if tcx.is_lang_item(did, LangItem::UnsafePinned) { flags |= AdtFlags::IS_UNSAFE_PINNED; }
if tcx.is_lang_item(did, LangItem::Pin)          { flags |= AdtFlags::IS_PIN; }
```

`AdtFlags` (`adt.rs:30-63`):
```
NO_ADT_FLAGS                  = 0,
IS_ENUM                       = 1 << 0,
IS_UNION                      = 1 << 1,
IS_STRUCT                     = 1 << 2,
HAS_CTOR                      = 1 << 3,
IS_PHANTOM_DATA               = 1 << 4,
IS_FUNDAMENTAL                = 1 << 5,
IS_BOX                        = 1 << 6,
IS_MANUALLY_DROP              = 1 << 7,
IS_VARIANT_LIST_NON_EXHAUSTIVE = 1 << 8,
IS_UNSAFE_CELL                = 1 << 9,
IS_UNSAFE_PINNED              = 1 << 10,
IS_PIN                        = 1 << 11,
IS_PIN_PROJECT                = 1 << 12,
```

Convenience predicates at `adt.rs:332-454` (`is_struct`, `is_union`, `is_enum`, `is_phantom_data`, `is_box`, `is_unsafe_cell`, `is_manually_drop`, `is_unsafe_pinned`, `is_pin`, `is_fundamental`, `is_pin_project`).

### D.2 Lang items NOT cached in `AdtFlags`

`MaybeUninit` is *not* in `AdtFlags`. From `rustc_hir/src/lang_items.rs`:
```
UnsafeCell,    sym::unsafe_cell,    unsafe_cell_type,    Target::Struct,  GenericRequirement::None;     // :233
UnsafePinned,  sym::unsafe_pinned,  unsafe_pinned_type,  Target::Struct,  GenericRequirement::None;     // :234
VaList,        sym::va_list,        va_list,             Target::Struct,  GenericRequirement::None;     // :236
Pin,           sym::pin,            pin_type,            Target::Struct,  GenericRequirement::None;     // :274
OwnedBox,      sym::owned_box,      owned_box,           Target::Struct,  GenericRequirement::Minimum(1);  // :352
PhantomData,   sym::phantom_data,   phantom_data,        Target::Struct,  GenericRequirement::Exact(1); // :355
ManuallyDrop,  sym::manually_drop,  manually_drop,       Target::Struct,  GenericRequirement::None;     // :357
MaybeUninit,   sym::maybe_uninit,   maybe_uninit,        Target::Union,   GenericRequirement::None;     // :360
GlobalAlloc,   sym::global_alloc_ty,global_alloc_ty,     Target::Struct,  GenericRequirement::None;     // :353
CVoid,         sym::c_void,         c_void,              Target::Enum,    GenericRequirement::None;     // :279
```

Recognition path for unflagged lang-item ADTs: `tcx.is_lang_item(did, LangItem::MaybeUninit)`.

`MaybeUninit` is the only one with `Target::Union`. `CVoid` is `Target::Enum` -- in the pinned toolchain a 2-variant `repr(u8)` enum with hidden `__variant1`/`__variant2` variants making it inhabited (see `library/core/src/ffi/mod.rs:49-76`; not zero-variant uninhabited).

### D.3 Atomic types

Atomic types are *not* lang items. They are ordinary struct ADTs in `core::sync::atomic` whose field is `UnsafeCell<T>`. Their special semantics come from the *intrinsics* called on them (`atomic_load`, `atomic_store`, etc.), not from any type-level marker. No `AdtFlags::IS_ATOMIC`, no `LangItem::AtomicU32`.

### D.4 Volatile

Same as atomics -- `read_volatile`/`write_volatile` are intrinsic-level. No volatile-bearing type variant.

### D.5 Variance, drop check, send/sync

Computed predicates rather than `TyKind` variants. `PhantomData<T>`'s purpose is to influence variance and drop-check; once `IS_PHANTOM_DATA` is set, layout sees a 1-ZST adt and codegen treats its value as nonexistent. Variance influence consumed before MIR by typeck.

---

## Theme E -- Post-monomorphization erasure

### E.1 `instance_mir` returns *unmonomorphized* body

`tcx.instance_mir(instance.def)` at `rustc_middle/src/ty/mod.rs:1671-1704`:
```
match instance {
    InstanceKind::Item(def) => match def_kind {
        DefKind::Const | DefKind::Static { .. } | DefKind::AssocConst
        | DefKind::Ctor(..) | DefKind::AnonConst | DefKind::InlineConst
            => self.mir_for_ctfe(def),
        _   => self.optimized_mir(def),
    },
    // VTableShim, ReifyShim, Intrinsic, FnPtrShim, Virtual,
    // ClosureOnceShim, ConstructCoroutineInClosureShim, FutureDropPollShim,
    // DropGlue, CloneShim, ThreadLocalShim, FnPtrAddrShim,
    // AsyncDropGlueCtorShim, AsyncDropGlue
    _ => self.mir_shims(instance),
}
```

For `InstanceKind::Item`, the body is `tcx.optimized_mir(def)` -- **generic**. Types may still be `TyKind::Param`, `TyKind::Alias`, `TyKind::Bound`, etc. Substitution and normalization happen *at use site*.

### E.2 `instantiate_mir_and_normalize_erasing_regions` is the per-use folder

`rustc_middle/src/ty/instance.rs:885-899`:
```
pub fn instantiate_mir_and_normalize_erasing_regions<T>(
    &self,
    tcx: TyCtxt<'tcx>,
    typing_env: ty::TypingEnv<'tcx>,
    v: EarlyBinder<'tcx, T>,
) -> T
where
    T: TypeFoldable<TyCtxt<'tcx>>,
{
    if let Some(args) = self.args_for_mir_body() {
        tcx.instantiate_and_normalize_erasing_regions(args, typing_env, v)
    } else {
        tcx.normalize_erasing_regions(typing_env, v.instantiate_identity())
    }
}
```

Codegen path applies this folder per-MIR-value. From `rustc_codegen_ssa/src/mir/mod.rs:125-137` (`FunctionCx::monomorphize`):
```
pub fn monomorphize<T>(&self, value: T) -> T
where T: Copy + TypeFoldable<TyCtxt<'tcx>>,
{
    self.instance.instantiate_mir_and_normalize_erasing_regions(
        self.cx.tcx(),
        self.cx.typing_env(),
        ty::EarlyBinder::bind(value),
    )
}
```

Used at `mir/mod.rs:259` (`fx.monomorphize(decl.ty)`), `:403` (`fx.monomorphize(arg_decl.ty)`), etc. -- every type read from the body funneled through `monomorphize`.

### E.3 `normalize_erasing_regions`

`rustc_middle/src/ty/normalize_erasing_regions.rs:34-62`:
1. `value = self.erase_and_anonymize_regions(value)` (line 54) -- replace free regions with `ReErased`, anonymize bound regions.
2. If still `has_aliases()`, fold with `NormalizeAfterErasingRegionsFolder` to eliminate `Alias` (line 60).

Result contains:
- No `TyKind::Alias::Projection` (normalised).
- No `TyKind::Alias::Free` (always normalisable per `ty_kind.rs:42`).
- `TyKind::Alias::Opaque` only outside its defining scope; per `ty_kind.rs:36-38`, "Can only be normalized away in PostAnalysis mode or its defining scope". In `TypingEnv::fully_monomorphized()` (codegen typing env) opaques are revealed.
- `TyKind::Alias::Inherent` -- doc-comment at `:34-35` doesn't promise normalisation; flows through the same normaliser; codegen has no special-case rejection.

### E.4 `erase_and_anonymize_regions`

`rustc_middle/src/ty/erase_regions.rs`. `fold_region` at `:64-72`:
```
match r.kind() {
    ty::ReBound(..) => r,
    _ => self.tcx.lifetimes.re_erased,
}
```

`ReEarlyParam`, `ReLateParam`, `ReStatic`, `ReVar`, `RePlaceholder`, `ReErased`, `ReError` are *all* rewritten to `ReErased`. Only `ReBound` survives. Doc-comment at `:64-67`: "We must not erase bound regions. `for<'a> fn(&'a ())` and `fn(&'free ())` are different types".

`fold_binder` at `:56-62` calls `tcx.anonymize_bound_vars(t)`, which strips bound-variable *names* but keeps De Bruijn structure.

Early-out at `:27-29` skips fold if value has no binder vars and no free regions.

### E.5 What survives in fully-monomorphized result

| `TyKind` variant | After monomorphize? | Note |
| --- | --- | --- |
| `Bool, Char, Int, Uint, Float` | yes | trivial |
| `Adt` | yes | concrete `GenericArgs` |
| `Foreign` | yes | concrete |
| `Str, Slice, Array, Pat` | yes | concrete |
| `RawPtr, Ref` | yes | `Ref`'s region is `ReErased` for free regions, kept for `ReBound` |
| `FnDef, FnPtr` | yes | concrete |
| `Closure, CoroutineClosure, Coroutine, CoroutineWitness` | yes | concrete with monomorphized args |
| `Dynamic` | yes | concrete `Region` slot |
| `Never, Tuple` | yes | trivial |
| `Alias::Projection` | **no** | normalised |
| `Alias::Free` | **no** | normalised |
| `Alias::Opaque` | **no** in fully-monomorphized env (revealed); yes in pre-codegen typing envs | per `ty_kind.rs:36-38` |
| `Alias::Inherent` | **no** | normalised (no codegen rejection path) |
| `Param` | **no** | substituted by `instantiate_mir` if `has_polymorphic_mir_body()` is true |
| `Bound` | **no** at type level (only inside binders) -- the bound *region* survives in `Ref` |
| `Placeholder` | **no** | typeck-only |
| `Infer` | **no** | typeck-only; codegen asserts `!instance.args.has_infer()` at `rustc_codegen_ssa/src/mir/mod.rs:177` |
| `Error` | **no** in successful builds | propagated to error reporting |
| `UnsafeBinder` | unstable; typeck-only -- comment at `ty_kind.rs:152-155` |

`assert!(!instance.args.has_infer())` at `mir/mod.rs:177` is rustc's codified "by the time we get here, no inference variables remain in `args`".

### E.6 Erasure on `Region` specifically

Post-erasure `Region` for `Ref`:
- `ReErased` -- common case for free regions.
- `ReBound(BoundVarIndexKind, BoundRegion)` -- only inside `for<'a>` binder scope (`Binder<FnSig>`, `Dynamic` existential predicates).

`RegionKind` variants at `rustc_type_ir/src/region_kind.rs:132-180`:
```
ReEarlyParam(I::EarlyParamRegion),
ReBound(BoundVarIndexKind, BoundRegion<I>),
ReLateParam(I::LateParamRegion),
ReStatic,
ReVar(RegionVid),
RePlaceholder(PlaceholderRegion<I>),
ReErased,
ReError(I::ErrorGuaranteed),
```

After `erase_and_anonymize_regions`, only `ReBound` and `ReErased` should appear.

### E.7 Substitution: `args_for_mir_body`

`rustc_middle/src/ty/instance.rs:867-869`:
```
fn args_for_mir_body(&self) -> Option<GenericArgsRef<'tcx>> {
    self.def.has_polymorphic_mir_body().then_some(self.args)
}
```

`InstanceKind` variants at `rustc_middle/src/ty/instance.rs:64-186`:
```
Item(DefId),                                        // :71
Intrinsic(DefId),                                   // :78
VTableShim(DefId),                                  // :85
ReifyShim(DefId, Option<ReifyReason>),              // :104
FnPtrShim(DefId, Ty<'tcx>),                         // :109
Virtual(DefId, usize),                              // :119
ClosureOnceShim { call_once: DefId, track_caller: bool },  // :127
ConstructCoroutineInClosureShim { ... },            // :134
ThreadLocalShim(DefId),                             // :147
FutureDropPollShim(DefId, Ty<'tcx>, Ty<'tcx>),      // :150
DropGlue(DefId, Option<Ty<'tcx>>),                  // :157
CloneShim(DefId, Ty<'tcx>),                         // :165
FnPtrAddrShim(DefId, Ty<'tcx>),                     // :172
AsyncDropGlueCtorShim(DefId, Ty<'tcx>),             // :178
AsyncDropGlue(DefId, Ty<'tcx>),                     // :184
```

Only `Item` returns a body needing `args_for_mir_body`-style substitution; `*Shim` variants have already-baked bodies (per `:914-922` "We're using `instantiate_identity` as e.g. `FnPtrShim` is separately generated for every instantiation of the `FnDef`, so the MIR body is already instantiated").

---

## Watch-outs and open questions

### W.1 `optimized_mir` is generic; per-use folding is mandatory

Biggest correctness risk: forgetting to fold every type read out of the body through `instantiate_mir_and_normalize_erasing_regions` before storing it. Rustc codegen funnels every read through `fx.monomorphize(...)`. The construction pass needs the same discipline; otherwise the resulting RegseqModel will leak `Param`, `Alias`, and free-region values that the C-shape methodology rule disallows.

### W.2 `Alias::Opaque` is environment-sensitive

`Opaque` revealed only in `PostAnalysis` mode or in defining scope (`ty_kind.rs:36-38`). Codegen uses `TypingEnv::fully_monomorphized()` which reveals; in non-codegen typing env, opaques will *not* normalise away. Decide explicitly which typing env the bridge will use.

### W.3 `FnDef` is a ZST and a distinct type

`FnDef(FunctionId, GenericArgs)` at `ty_kind.rs:130` is the *type of the function name*, not a function-pointer type. Size 0. Shows up in MIR `Operand::Constant` for direct calls. Conflating `FnDef` with `FnPtr` will silently break monomorphization keying.

### W.4 `FnPtr` vs `Binder<FnSig>` representational split

`TyKind::FnPtr(Binder<FnSigTys>, FnHeader)` is *split* (per `ty_kind.rs:142-147`, "to reduce the size of `TyKind` by 8 bytes"). Most code reassembles via `FnSigTys::with(FnHeader)` at `:931-938`.

### W.5 Closures, coroutines, coroutine-closures are three distinct variants

`Closure` (`:165`), `CoroutineClosure` (`:172`), `Coroutine` (`:179`), `CoroutineWitness` (`:204`) -- four distinct variants. Same `(DefId-equivalent, GenericArgs)` shape but args structured differently per `ClosureArgsParts`/`CoroutineClosureArgsParts`/`CoroutineArgsParts`. Construction pass must call the correct `split()` for each. `CoroutineWitness` "should only appear as part of the `CoroutineArgs`" (`:181-183`) -- if at top level, coroutine transform has not run.

### W.6 `Pat` is integer-range only today

`TyKind::Pat(Ty, Pat)` at `:107` -- "Only supports integer range patterns for now." Reject if not used and revisit when support broadens.

### W.7 `UnsafeBinder` is unstable and unlikely to appear

`UnsafeBinder` at `:155`. Should never appear without `feature(unsafe_binders)`. Reject by default.

### W.8 Discriminant type defaults to `isize`

`ReprOptions::discr_type()` at `rustc_abi/src/lib.rs:214-216` returns `IntegerType::Pointer(true)` (= `isize`) when no explicit `repr(<int>)`. Typeck discr type; layout may shrink it ("Layout can later decide to use a smaller type for the tag", `:212`). Distinguish typeck-level discr type from in-memory tag size.

### W.9 `Niche` encoding makes some variants unrepresentable

Doc at `lib.rs:1909-1912`: variants outside `niche_variants` and not equal to `untagged_variant` "cannot be represented; they must be uninhabited". Niched enum types carry an obligation that "impossible" variants are uninhabited. Preserve via `LayoutData.uninhabited` for C-codegen lowering (e.g. `Result<T, !>`-style).

### W.10 `MaybeUninit` has no `AdtFlags` bit

Unlike `Box`/`PhantomData`/`UnsafeCell`/`ManuallyDrop`/`Pin`/`UnsafePinned`, `MaybeUninit` requires explicit `tcx.is_lang_item(did, LangItem::MaybeUninit)`. If recognition is frequent, cache locally.

### W.11 `PointeeInfo.safe = None` does not mean "not a pointer"

`PointeeInfo.safe = None` (`lib.rs:2140-2142`) means "this is a raw pointer", *not* "no pointer info". Raw pointers still pointers; just no validity/dereferenceability guarantees. Conflating breaks `SharedRef`/`MutableRef`/`Box`/`Raw` provenance distinction.

### W.12 `frozen` and `unpin` are advisory, not load-bearing

`PointerKind::SharedRef.frozen` and `MutableRef.unpin` (`lib.rs:2125-2128`) are advisory. Doc at `:2134-2137`: "if the information is wrong, that can cause UB, but if the information is absent, that must always be okay." Per locked D7/D8 (`stage-3/spec-snapshot.md` Section 1.8), RegseqModel **drops** these advisory bits entirely: `Provenance` carries only `SharedBorrow`/`UniqueBorrow`, and `UniqueBorrow` follows `Mutability::Mut` directly (every `TyKind::Ref(_, _, Mutability::Mut)` maps to `UniqueBorrow` regardless of `unpin`). The advisory absence is sound per the rustc doc above.

### W.13 Free regions are erased; bound regions are not

`erase_and_anonymize_regions` keeps `ReBound` intact (`erase_regions.rs:64-72`). Matters for `dyn for<'b> Trait<'b, ...>` and HRTB function pointers. **v1 policy:** RegseqModel detects surviving `ReBound` in HRTB-bearing `Ty::FnPtr` and `Ty::Dynamic` after the mandatory region-erasure fold and **rejects** them at the bridge per `stage-3/spec-snapshot.md` Section 1.8 (D7/D8 refinement) and Section 1.10 (D5). Only non-HRTB types -- whose regions become `ReErased` -- enter the IR. Reopens to encode binder rank/shape if a future fixture or stage-14 intrinsic surfaces a stable need.

### W.14 `is_known_rigid` is the natural rigid/non-rigid split

`TyKind::is_known_rigid()` at `ty_kind.rs:289-322` is the exact split `rustc_public::TyKind` uses for `RigidTy` (`rustc_public/src/ty.rs:307-312`). If RegseqModel's `Ty` chooses a similar two-level shape, this is the canonical split.

### W.15 `c_void` is a `Target::Enum`

From `lang_items.rs:279`: `CVoid` targets `Target::Enum`, not `Target::Struct`. In the pinned toolchain `library/core/src/ffi/mod.rs:49-76` declares it as a 2-variant `repr(u8)` enum with hidden `__variant1`/`__variant2` variants -- inhabited, not zero-variant uninhabited. Detection `tcx.is_lang_item(did, LangItem::CVoid)`; no `AdtFlags::IS_CVOID`.

### W.16 `repr(Rust)` is the absence of any `IS_*` flag

`ReprFlags` (`lib.rs:82-107`) has no `IS_RUST` bit. Default repr is empty flag set. Match "`repr(Rust)` vs anything else" by inverting `c() && !simd() && !transparent() && pack.is_none() && align.is_none() && int.is_none()` (modulo flags at `:101-104`).

### W.17 `Adt` recursion lives in `tcx.type_of(field.did)`

`AdtDefData` does not embed field types (`adt.rs:74-98`). Construction pass calls `tcx.type_of(field.did)` -- returns `EarlyBinder<Ty<'tcx>>` (still generic in parent's params). Substitution is caller's responsibility. Forgetting leaks `Param` into the constructed model.

### W.18 Variant fields and tuple struct ctors

`VariantDef.ctor: Option<(CtorKind, DefId)>` at `ty/mod.rs:1124`. Tuple-struct/tuple-variant constructors are functions whose `DefId` is recorded here. Bridge treating only `DefKind::Fn` as callable misses tuple constructors; check `ctor` when walking ADTs.

### W.19 `IntegerType::Pointer(false)` vs `Pointer(true)`

Comment-only convention (`lib.rs:122-129`): "`Pointer(true)` means `isize`". Off-by-one risk.

---

## Cross-reference index

- `rustc_type_ir/src/ty_kind.rs` -- canonical `TyKind`; `AliasTyKind`; `FnSig`/`FnSigTys`/`FnHeader`; `is_known_rigid`.
- `rustc_type_ir/src/ty_kind/closure.rs` -- closure/coroutine/coroutine-closure args structure.
- `rustc_type_ir/src/region_kind.rs` -- `RegionKind`.
- `rustc_ast_ir/src/lib.rs` -- `IntTy`, `UintTy`, `FloatTy`, `Mutability`, `Movability`.
- `rustc_abi/src/lib.rs` -- layout types (`LayoutData`, `FieldsShape`, `Variants`, `TagEncoding`, `Niche`, `BackendRepr`, `Scalar`, `Primitive`, `Integer`, `Float`, `WrappingRange`); `ReprOptions`, `ReprFlags`, `IntegerType`; `PointerKind`, `PointeeInfo`.
- `rustc_abi/src/layout/ty.rs` -- `Layout<'a>`, `TyAndLayout`, `FieldIdx`, `VariantIdx`.
- `rustc_abi/src/extern_abi.rs` -- `ExternAbi`.
- `rustc_middle/src/ty/adt.rs` -- `AdtDef`, `AdtDefData`, `AdtFlags`, `AdtKind`, lang-item caching.
- `rustc_middle/src/ty/mod.rs` -- `VariantDef`, `FieldDef`, `instance_mir`.
- `rustc_middle/src/ty/instance.rs` -- `InstanceKind`, `instantiate_mir_and_normalize_erasing_regions`, `args_for_mir_body`.
- `rustc_middle/src/ty/normalize_erasing_regions.rs` -- `normalize_erasing_regions` flow.
- `rustc_middle/src/ty/erase_regions.rs` -- `erase_and_anonymize_regions` folder.
- `rustc_middle/src/ty/context.rs` -- `CtxtInterners`, `intern_ty`.
- `rustc_middle/src/ty/layout.rs` -- `TyAndLayout` typedef, `LayoutOfHelpers` trait.
- `rustc_codegen_ssa/src/mir/mod.rs` -- `FunctionCx::monomorphize`, `codegen_mir`, `assert!(!instance.args.has_infer())`.
- `rustc_hir/src/lang_items.rs` -- lang-item declaration list (`UnsafeCell`, `Pin`, `OwnedBox`, `PhantomData`, `ManuallyDrop`, `MaybeUninit`, `CVoid`, etc.).
- `rustc_public/src/ty.rs` -- public-facing collapsed `TyKind` (`RigidTy`/`Alias`/`Param`/`Bound`); `Abi`; `ExistentialPredicate`; `ClosureKind`.
