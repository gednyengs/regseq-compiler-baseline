# Charon -- Prior-art notes (stage-3 type-system research)

Project: AeneasVerif/Charon. A library for analyzing Rust crates without
touching compiler internals: it consumes rustc + hax output and produces
either ULLBC (an MIR-shaped IR) or LLBC (with reconstructed control flow).
Used as the front-end for Aeneas (Rust verification) and Eurydice (Rust ->
C extraction for verified code).

This document quotes definitions verbatim from the Charon repository
(branch `main`) where the wording is load-bearing. All citations are
`https://github.com/AeneasVerif/charon/blob/main/<path>` (raw equivalents
were used for fetching).

Top-level repo layout (relevant pieces):

- `charon/src/ast/` -- the IR data types
  - `ast/types.rs` -- the `Ty` / `TyKind` enum (the type vocabulary)
  - `ast/types/vars.rs` -- DeBruijn variables for type-level params
  - `ast/expressions.rs` -- `FnPtr`, `ConstantExpr`, `ConstantExprKind`
  - `ast/krate.rs` -- `TranslatedCrate` (the top-level translation unit)
  - `ast/builtins.rs` -- mapping from `BuiltinTy` to canonical paths
  - `ast/hash_cons.rs` -- the global interning table
  - `ast/ullbc_ast.rs`, `ast/llbc_ast.rs` -- two body forms
- `charon/src/bin/charon-driver/translate/` -- rustc(hax) -> Charon
  - `translate_types.rs` -- `hax::TyKind` -> `TyKind`
  - `translate_closures.rs`, `translate_trait_objects.rs`
- `charon/src/transform/` -- post-translation passes
  - `transform/normalize/partial_monomorphization.rs`
  - `transform/normalize/transform_dyn_trait_calls.rs`
  - `transform/normalize/expand_associated_types.rs`
  - `transform/finish_translation/insert_ptr_metadata.rs`
- `charon/src/pretty/fmt_with_ctx.rs` -- the renderer
- `charon/src/options.rs` -- `--monomorphize`, `--monomorphize-mut`,
  `--treat-box-as-builtin`, `--hide-marker-traits`, etc.

Sources cited inline below.

---

## A. Type-vocabulary completeness

### A.1 `Ty` is a hash-consed handle, not the data itself

`charon/src/ast/types.rs` lines ~805-811:

```rust
/// A type.
///
/// Warning: the `DriveMut` impls of `Ty` needs to clone and re-hash the modified type to maintain
/// the hash-consing invariant. This is expensive, avoid visiting types mutably when not needed.
#[derive(Debug, Clone, Hash, PartialEq, Eq, SerializeState, DeserializeState, Drive, DriveMut)]
#[serde_state(state_implements = HashConsSerializerState)] // Avoid corecursive impls due to perfect derive
pub struct Ty(pub HashConsed<TyKind>);
```

`HashConsed<T>` is defined in `charon/src/ast/hash_cons.rs`:

```rust
/// Hash-consed data structure: a reference-counted wrapper that guarantees that two equal
/// value will be stored at the same address. This makes it possible to use the pointer address
/// as a hash value.
// Warning: a `derive` should not introduce a way to create a new `HashConsed` value without
// going through the interning table.
#[derive(PartialEq, Eq, Hash)]
pub struct HashConsed<T>(HashByAddr<Arc<T>>);
```

So `Ty` is `Arc<TyKind>` plus address-based hashing. Construction MUST go
through the global `intern_table` static (an `RwLock<TypeMap>`), so two
structurally-equal `Ty`s share an `Arc`. Equality and hashing on the
wrapper compare addresses. (See section C below for details.)

### A.2 `TyKind` -- the canonical enumeration

`charon/src/ast/types.rs` lines ~813-902:

```rust
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, VariantName, EnumIsA, EnumAsGetters,
    EnumToGetters, VariantIndexArity, SerializeState, DeserializeState,
    Drive, DriveMut,
)]
#[charon::variants_prefix("T")]
pub enum TyKind {
    /// An ADT.
    /// Note that here ADTs are very general. They can be:
    /// - user-defined ADTs
    /// - tuples (including `unit`, which is a 0-tuple)
    /// - built-in types (includes some primitive types, e.g., arrays or slices)
    ///
    /// The information on the nature of the ADT is stored in (`TypeId`)[TypeId].
    /// The last list is used encode const generics, e.g., the size of an array
    Adt(TypeDeclRef),
    #[charon::rename("TVar")]
    TypeVar(TypeDbVar),
    Literal(LiteralTy),
    /// The never type, for computations which don't return.
    /// ...
    /// Note that we eliminate the variables which have this type in a micro-pass.
    /// As statements don't have types, this type disappears eventually disappears
    /// from the AST.
    Never,
    // We don't support floating point numbers on purpose (for now)
    /// A borrow
    Ref(Region, Ty, RefKind),
    /// A raw pointer.
    RawPtr(Ty, RefKind),
    /// A trait associated type
    TraitType(TraitRef, TraitItemName),
    /// `dyn Trait`
    DynTrait(DynPredicate),
    /// Function pointer type. This is a literal pointer to a region of memory that
    /// contains a callable function.
    /// This is a function signature with limited generics: it only supports lifetime generics, not
    /// other kinds of generics.
    FnPtr(RegionBinder<FunSig>),
    /// The unique type associated with each function item. Each function item is given
    /// a unique generic type that takes as input the function's early-bound generics. This type
    /// is not generally nameable in Rust; it's a ZST (there's a unique value), and a value of that type
    /// can be cast to a function pointer or passed to functions that expect `FnOnce`/`FnMut`/`Fn` parameters.
    /// There's a binder here because charon function items take both early and late-bound
    /// lifetimes as arguments; ...
    FnDef(RegionBinder<FnPtr>),
    /// As a marker of taking out metadata from a given type
    /// The internal type is assumed to be a type variable
    PtrMetadata(Ty),
    /// An array type `[T; N]`
    Array(Ty, Box<ConstantExpr>),
    /// A slice type `[T]`
    Slice(Ty),
    /// A type that could not be computed or was incorrect.
    #[drive(skip)]
    Error(String),
}
```

Variants and what they hold:

1. `Adt(TypeDeclRef)` -- struct/enum/union/tuple/builtin via `TypeId`.
2. `TypeVar(TypeDbVar)` -- a generic type parameter (not erased; see E).
3. `Literal(LiteralTy)` -- bool, char, signed/unsigned int (incl. isize/usize), float (f16/f32/f64/f128).
4. `Never` -- `!`.
5. `Ref(Region, Ty, RefKind)` -- `&'r [mut] T`.
6. `RawPtr(Ty, RefKind)` -- `*const T` / `*mut T` (Charon reuses `RefKind`).
7. `TraitType(TraitRef, TraitItemName)` -- an unresolved associated type projection.
8. `DynTrait(DynPredicate)` -- trait object.
9. `FnPtr(RegionBinder<FunSig>)` -- `fn(...) -> ...` pointer type.
10. `FnDef(RegionBinder<FnPtr>)` -- the unique ZST per function item.
11. `PtrMetadata(Ty)` -- `<T as Pointee>::Metadata` projection marker.
12. `Array(Ty, Box<ConstantExpr>)` -- `[T; N]` with a const-evaluated length.
13. `Slice(Ty)` -- `[T]` (DST element).
14. `Error(String)` -- placeholder for translation failures.

Notable absences from `TyKind`:

- No `Tuple` variant -- tuples are `Adt(TypeDeclRef { id: TypeId::Tuple, .. })`.
- No `Coroutine` variant -- they are not supported (see E).
- No `Closure` variant -- closures are translated to synthesized ADTs by
  `translate_closures.rs` (see F).
- No `Param`/`Bound`/`Placeholder` -- generic params are `TypeVar(TypeDbVar)`
  (a DeBruijn variable). Late-bound regions appear via `RegionBinder<...>`.
- No `Foreign` variant -- `extern type` is also `Adt(...)`.
- No `Box` variant in `TyKind` -- `Box<T>` is `Adt(TypeDeclRef { id:
  TypeId::Builtin(BuiltinTy::Box), ... })` (see D).
- No `Str` variant -- `str` is `Adt(TypeDeclRef { id:
  TypeId::Builtin(BuiltinTy::Str), ... })` (see translate_types.rs:164).

### A.3 Auxiliaries used by `TyKind`

`TypeId` (types.rs ~728-764):

```rust
pub enum TypeId {
    /// A "regular" ADT type.
    /// Includes transparent ADTs and opaque ADTs (local ADTs marked as opaque,
    /// and external ADTs).
    #[charon::rename("TAdtId")]
    Adt(TypeDeclId),
    Tuple,
    /// Built-in type. Either a primitive type like array or slice, or a
    /// non-primitive type coming from a standard library
    /// and that we handle like a primitive type. Types falling into this
    /// category include: Box, Vec, Cell...
    /// The Array and Slice types were initially modelled as primitive in
    /// the [Ty] type. We decided to move them to built-in types as it allows
    /// for more uniform treatment throughout the codebase.
    #[charon::rename("TBuiltin")]
    Builtin(BuiltinTy),
}
```

`BuiltinTy` (types.rs ~932-937 -- only two cases survive after the
"move to builtins" refactor):

```rust
pub enum BuiltinTy {
    /// Boxes are de facto a primitive type.
    Box,
    /// Primitive type
    Str,
}
```

The source-of-truth path mapping (from `charon/src/ast/builtins.rs`):

```rust
impl BuiltinTy {
    pub fn get_name(self) -> Name {
        let name: &[_] = match self {
            BuiltinTy::Box => &["alloc", "boxed", "Box"],
            BuiltinTy::Str => &["Str"],
        };
        Name::from_path(name)
    }
}
```

Note that the doc comment on `TypeId::Builtin` says "Box, Vec, Cell"
but the current `BuiltinTy` enum is just `{ Box, Str }`. Vec/Cell are
handled as ordinary `Adt(TypeDeclId)` ADTs. Box's specialness is
optional, gated by `--treat-box-as-builtin` (see options.rs).

`TypeDeclRef` (types.rs ~766-771):

```rust
pub struct TypeDeclRef {
    pub id: TypeId,
    pub generics: BoxedArgs,
}
```

`LiteralTy` (types.rs ~774-803):

```rust
pub enum LiteralTy {
    Int(IntTy),
    UInt(UIntTy),
    Float(FloatTy),
    Bool,
    Char,
}
```

`IntTy { Isize, I8, I16, I32, I64, I128 }`,
`UIntTy { Usize, U8, U16, U32, U64, U128 }`,
`FloatTy { F16, F32, F64, F128 }`. `IntegerTy = Signed(IntTy) | Unsigned(UIntTy)`.

`RefKind` (also reused for raw pointers):

```rust
pub enum RefKind { Mut, Shared }
```

`Region` (types.rs ~28-38):

```rust
pub enum Region {
    /// Region variable. See `DeBruijnVar` for details.
    Var(RegionDbVar),
    /// Static region
    Static,
    /// Body-local region, considered existentially-bound at the level of a body.
    Body(RegionId),
    /// Erased region
    Erased,
}
```

So Charon does not flatten regions into a single token; it tracks
DeBruijn region variables, `'static`, function-body-local regions, and
explicitly erased regions, all in one enum.

`FunSig` (types.rs ~986-993):

```rust
pub struct FunSig {
    /// Is the function unsafe or not
    #[drive(skip)]
    pub is_unsafe: bool,
    pub inputs: Vec<Ty>,
    pub output: Ty,
}
```

(There is no ABI field on `FunSig`; ABI is not preserved at the IR level.
Calling-convention info is in trait/method metadata, not the type.)

`FnPtr` is defined in `charon/src/ast/expressions.rs`:

```rust
pub struct FnPtr {
    pub kind: Box<FnPtrKind>,
    pub generics: BoxedArgs,
}

pub enum FnPtrKind {
    #[charon::rename("FunId")]
    Fun(FunId),
    /// If a trait: the reference to the trait and the id of the trait method.
    Trait(TraitRef, TraitItemName, FunDeclId),
}

pub enum FunId {
    /// A "regular" function ...
    Regular(FunDeclId),
    /// A primitive function, coming from a standard library ...
    #[charon::rename("FBuiltin")]
    Builtin(BuiltinFunId),
}
```

So `TyKind::FnDef(RegionBinder<FnPtr>)` carries the actual `FnPtr` (a
named function plus its generics) under a region binder.

`DynPredicate` (types.rs ~996-1005):

```rust
/// The contents of a `dyn Trait` type.
pub struct DynPredicate {
    /// This binder binds a single type `T`, which is considered existentially quantified. The
    /// predicates in the binder apply to `T` and represent the `dyn Trait` constraints.
    /// E.g. `dyn Iterator<Item=u32> + Send` is represented as `exists<T: Iterator<Item=u32> + Send> T`.
    ///
    /// Only the first trait clause may have methods. We use the vtable of this trait in the `dyn
    /// Trait` pointer metadata.
    pub binder: Binder<Ty>,
}
```

The dyn-trait existential is encoded as a `Binder<Ty>` with all the
predicates collected as trait clauses on that type variable. The binder
also records `kind: BinderKind::Dyn` (see `Binder` definition in types.rs
~263-274 and `BinderKind::Dyn` in ~245-257).

`ClosureKind` and `ClosureInfo` (types.rs ~953-983):

```rust
pub enum ClosureKind { Fn, FnMut, FnOnce }

/// Additional information for closures.
pub struct ClosureInfo {
    pub kind: ClosureKind,
    /// The `FnOnce` implementation of this closure -- always exists.
    pub fn_once_impl: RegionBinder<TraitImplRef>,
    pub fn_mut_impl: Option<RegionBinder<TraitImplRef>>,
    pub fn_impl: Option<RegionBinder<TraitImplRef>>,
    /// The signature of the function that this closure represents.
    pub signature: RegionBinder<FunSig>,
}
```

`ClosureInfo` is metadata attached to the synthesized closure ADT, not a
case in `TyKind`. See section F.

### A.4 Const generics

There is no separate `ConstGeneric` enum in current Charon. Const
generics live in the unified `ConstantExpr` family (in
`charon/src/ast/expressions.rs`), and the same type is used for array
lengths inside `TyKind::Array(Ty, Box<ConstantExpr>)`:

```rust
pub struct ConstantExpr {
    pub kind: ConstantExprKind,
    pub ty: Ty,
}

pub enum ConstantExprKind {
    Literal(Literal),
    Adt(Option<VariantId>, Vec<ConstantExpr>),
    Array(Vec<ConstantExpr>),
    Global(GlobalDeclRef),
    TraitConst(TraitRef, TraitItemName),
    /// A reference to the vtable `static` item for this trait ref.
    VTableRef(TraitRef),
    Ref(Box<ConstantExpr>, Option<UnsizingMetadata>),
    Ptr(RefKind, Box<ConstantExpr>, Option<UnsizingMetadata>),
    /// A const generic var
    Var(ConstGenericDbVar),
    /// Function definition -- this is a ZST constant
    FnDef(FnPtr),
    FnPtr(FnPtr),
    PtrNoProvenance(u128),
    RawMemory(Vec<Byte>),
    Opaque(String),
}
```

So a const-generic parameter (`const N: usize`) appears as
`ConstantExprKind::Var(ConstGenericDbVar)` inside `Array(_, len)` when
the length is generic. The unified type also lets Charon represent
fully-evaluated lengths (`Literal(Literal::Scalar(...))`), references to
named consts (`Global`), and trait associated consts.

---

## B. Layout / representation

Charon does carry layout information, but it is per-target and may be
partial. Definitions in `charon/src/ast/types.rs`:

```rust
// rustc counts bytes in layouts as u64
pub type ByteCount = u64;

/// Simplified layout of a single variant.
///
/// Maps fields to their offset within the layout.
pub struct VariantLayout {
    pub field_offsets: IndexVec<FieldId, ByteCount>,
    /// Whether the variant is uninhabited, ...
    pub uninhabited: bool,
    /// The memory representation of the discriminant corresponding to this
    /// variant. ...
    pub tag: Option<ScalarValue>,
}

pub enum TagEncoding {
    /// Represents the direct encoding of the discriminant as the tag via integer casts.
    Direct,
    /// Represents the encoding of the discriminant in the niche of variant `untagged_variant`.
    Niche { untagged_variant: VariantId },
}

pub struct DiscriminantLayout {
    pub offset: ByteCount,
    pub tag_ty: IntegerTy,
    pub encoding: TagEncoding,
    // FIXME: Should probably contain the valid range of the tag, too.
}

/// Simplified type layout information.
///
/// Does not include information about niches.
/// If the type does not have a fully known layout (e.g. it is ?Sized)
/// some of the layout parts are not available.
pub struct Layout {
    pub size: Option<ByteCount>,
    pub align: Option<ByteCount>,
    pub discriminant_layout: Option<DiscriminantLayout>,
    pub uninhabited: bool,
    pub variant_layouts: IndexVec<VariantId, VariantLayout>,
}
```

`Layout` is attached to `TypeDecl`, not `Ty`:

```rust
pub struct TypeDecl {
    pub def_id: TypeDeclId,
    pub item_meta: ItemMeta,
    pub generics: GenericParams,
    pub src: ItemSource,
    pub kind: TypeDeclKind,
    /// The layout of the type for each target. Information may be partial because of generics or
    /// dynamically-sized types. If we cannot compute a layout, the target has no entry.
    pub layout: SeqHashMap<TargetTriple, Layout>,
    /// The metadata associated with a pointer to the type.
    pub ptr_metadata: PtrMetadata,
    /// The representation options of this type declaration as annotated by the user.
    /// Is `None` for foreign type declarations.
    pub repr: Option<ReprOptions>,
}
```

`PtrMetadata` (types.rs ~436-454):

```rust
pub enum PtrMetadata {
    /// Types that need no metadata, namely `T: Sized` types.
    #[charon::rename("NoMetadata")]
    None,
    /// Metadata for `[T]` and `str`, ... Of type `usize`.
    Length,
    /// Metadata for `dyn Trait`, referring to the vtable struct, ...
    /// Of type `&'static vtable`
    VTable(TypeDeclRef),
    /// Unknown due to generics, but will inherit from the given type.
    /// This is consistent with `<Ty as Pointee>::Metadata`.
    /// Of type `TyKind::Metadata(Ty)`.
    InheritFrom(Ty),
}
```

`ReprOptions`, `ReprAlgorithm`, `AlignmentModifier`:

```rust
pub enum ReprAlgorithm {
    /// The default layout algorithm. Used without an explicit `repr` or for `repr(Rust)`.
    Rust,
    /// The C layout algorithm as enforced by `repr(C)`.
    C,
}

pub enum AlignmentModifier {
    Align(ByteCount),
    Pack(ByteCount),
}

/// The representation options as annotated by the user.
///
/// NOTE: This does not include less common/unstable representations such as `#[repr(simd)]`
/// or the compiler internal `#[repr(linear)]`. Similarly, enum discriminant representations
/// are encoded in [`Variant::discriminant`] and [`DiscriminantLayout`] instead.
/// This only stores whether the discriminant type was derived from an explicit annotation.
pub struct ReprOptions {
    pub repr_algo: ReprAlgorithm,
    pub align_modif: Option<AlignmentModifier>,
    pub transparent: bool,
    pub explicit_discr_type: bool,
}
```

Observations:

- Layout is per-target -- `SeqHashMap<TargetTriple, Layout>`. `TranslatedCrate`
  carries `target_information: SeqHashMap<TargetTriple, TargetInfo>`.
- Layout is "simplified": no niche details, no valid-range metadata. The
  comment block calls out that this is by design and that `(u32, !)`
  layouts and similar edge cases are handled informally.
- Sizes/aligns are `Option<ByteCount>` because `?Sized` types and generic
  ADTs may have unknown layout.
- Layout is on the *declaration*, not the type expression. There is no
  "layout-of-`Ty`"; you compute it by combining the decl's layout with
  the substitution implied by its `TypeDeclRef.generics`.
- `repr` reflects the user's `#[repr(...)]` annotation. `simd` and
  `linear` are explicitly NOT modelled in `ReprOptions`.

---

## C. Identity, interning, canonical form

### C.1 Hash-consing of `Ty`

From `charon/src/ast/hash_cons.rs`:

```rust
/// Hash-consed data structure: a reference-counted wrapper that guarantees that two equal
/// value will be stored at the same address. This makes it possible to use the pointer address
/// as a hash value.
pub struct HashConsed<T>(HashByAddr<Arc<T>>);
```

Construction goes through a global `RwLock<TypeMap>` interning table.
Two `Ty` values are equal iff their inner `Arc`s point to the same
allocation. Hash is the address. The table is keyed per type `T`, so
both `Ty(HashConsed<TyKind>)` and `TraitRef(HashConsed<TraitRefContents>)`
are hash-consed using the same machinery.

`TraitRef` is also hash-consed (types.rs ~163-172):

```rust
/// A reference to a trait.
///
/// This type is hash-consed, `TraitRefContents` contains the actual data.
pub struct TraitRef(pub HashConsed<TraitRefContents>);

pub struct TraitRefContents {
    pub kind: TraitRefKind,
    /// Not necessary, but useful
    pub trait_decl_ref: PolyTraitDeclRef,
}
```

Likewise `ConstantExpr` is hash-consed:

```rust
pub struct ConstantExpr { pub kind: ConstantExprKind, pub ty: Ty }
```

(The `state_implements = HashConsSerializerState` annotation in serde
attributes is the cue.)

### C.2 Interning constraints

Mutating a `Ty` requires re-cloning and re-interning -- see the doc on
`Ty`:

> Warning: the `DriveMut` impls of `Ty` needs to clone and re-hash the modified type to maintain the hash-consing invariant. This is expensive, avoid visiting types mutably when not needed.

There is also a worry about cross-crate interning: the `intern_table`
module says "this likely means that if a crate uses charon both as a
direct dependency and as a dylib, then the static will be duplicated,
causing hashing and equality on `HashCons` to be broken."

### C.3 IDs vs handles

`TypeDecl` is keyed by `TypeDeclId` (a `usize` newtype). `TypeDeclRef`
combines the id with a `BoxedArgs`. `TyKind::Adt(TypeDeclRef)` is the
*use site* (with substitution); `TypeDecl` is the definition. So Charon
distinguishes:

- `TypeDeclId` -- nominal identity of the declaration.
- `TypeDeclRef { id, generics }` -- a reference to a (possibly
  monomorphized) instance of the declaration.
- `Ty` -- a hash-consed handle to a `TyKind`, structural up to interning.

### C.4 Equality model

Because `HashConsed` uses address equality, structurally-equal `Ty`s are
guaranteed to share an `Arc`, so `==` is fast. But this depends on the
single-process invariant: equality across two crates loading Charon as
both crate and dylib is undefined.

---

## D. Special-semantics markers

Charon does NOT have dedicated `TyKind` variants for `UnsafeCell`,
`MaybeUninit`, `PhantomData`, atomic types, or `NonNull`. They appear as
ordinary `Adt(TypeDeclRef { id: TypeId::Adt(<their decl id>), .. })`.
The repository's `ast/builtins.rs` (full file, 23 lines) only recognizes
`Box` and `Str` as built-ins. There is no enumeration of "special interior
mutability" types.

`Box` itself is special only optionally: gated by
`--treat-box-as-builtin` (see `options.rs`):

```
pub treat_box_as_builtin: bool,
```

When set, `Box<T, A>` is rewritten to the `BuiltinTy::Box` form and the
allocator parameter is hidden via `hide_allocator: bool`. Without it,
`Box<T, A>` is just an ADT.

There is also `--hide-marker-traits` and a list of marker traits Charon
treats specially as built-in `TraitRefKind::BuiltinOrAuto` impls:

`charon/src/ast/types.rs` lines ~136-158:

```rust
pub enum BuiltinImplData {
    // Marker traits (without methods).
    Sized,
    MetaSized,
    Tuple,
    Pointee,
    DiscriminantKind,
    // Auto traits (defined with `auto trait ...`).
    Auto,

    // Traits with methods.
    /// An impl of `Destruct` for a type with no drop glue.
    NoopDestruct,
    /// An impl of `Destruct` for a type parameter, which we could not resolve because
    /// `--add-drop-bounds` was not set.
    UntrackedDestruct,
    Fn,
    FnMut,
    FnOnce,
    Copy,
    Clone,
}
```

So Charon's "marker awareness" lives in the trait-ref system, not the
type system: `Sized`, `MetaSized`, `Pointee`, `DiscriminantKind`,
`Tuple`, plus auto traits, are recognized at the `TraitRefKind`
level via `BuiltinOrAuto { builtin_data: BuiltinImplData::..., .. }`.

Information preservation summary:

- Atomic types: not marked at the type level. They are `Adt`s and
  carry their `TypeDeclId`. `repr` (`Option<ReprOptions>`) records user
  `#[repr(...)]` flags.
- `UnsafeCell`: same -- ordinary `Adt`. The fact that it confers
  interior mutability is not encoded in `Ty`; consumers infer it from
  the path.
- `MaybeUninit`: same -- ordinary `Adt`. There is no
  "uninhabited/uninit-bit" track in `Ty`. There IS an `uninhabited:
  bool` flag on `Layout` and `VariantLayout`, but that is for
  `!`-containing types, not for `MaybeUninit`.
- `PhantomData`: same -- ordinary `Adt`. It is a regular ZST decl from
  `core::marker`.
- `NonNull`, `Cell`, `RefCell`: same -- regular `Adt`s.
- `Pin`, `ManuallyDrop`: same -- regular `Adt`s.

The comment on `TypeId::Builtin` mentions "Box, Vec, Cell..." as the
intended fate, but the actual `BuiltinTy` enum only has `Box` and `Str`
today.

---

## E. Post-monomorphization erasure

### E.1 Monomorphization mode

`charon/src/options.rs`:

```rust
/// Monomorphize the items encountered when possible. Generic items found in the crate are not
/// translated; only their concrete instances are.
#[clap(long, visible_alias = "mono")]
#[serde(default)]
pub monomorphize: bool,
```

There is a second mode, `--monomorphize-mut`, designed for Aeneas (so
that mutable references never appear inside generics). See
`charon/src/transform/normalize/partial_monomorphization.rs` (top-of-file
docs):

> This module implements partial monomorphization, which allows specializing generic items on some specific instantiation patterns. This is used by Aeneas to avoid nested mutable borrows: we transform `Iter<'a, &'b mut T>` to `{Iter::<_, &mut U>}<'a, 'b, T>`, where ...

There is also a "Soteria" preset (options.rs ~393-398):

```rust
Preset::Soteria => {
    self.extract_opaque_bodies = true;
    self.monomorphize = true;
    self.mir = Some(MirLevel::Elaborated);
    self.ullbc = true;
}
```

So full monomorphization is intended for an "extract-everything-and-leave-no-generics"
downstream consumer.

### E.2 What survives in `TyKind` after eager monomorphization

Even with `--monomorphize`, the `Ty` enum retains:

- `TypeVar(TypeDbVar)` -- because closures, dyn binders, late-bound
  function-pointer signatures, and a few intrinsic-shaped definitions
  still contain types that are bound under inner binders (e.g.
  `for<'a> fn(&'a u32)`).
- `Region(Var | Static | Body | Erased)` -- regions are NOT erased to a
  single token by Charon. The `Erased` variant is used where region
  inference has erased a lifetime.
- `RegionBinder<...>` and `Binder<...>` -- still used inside `FnPtr`,
  `FnDef`, and `DynPredicate`.

So Charon's monomorphization is "early-bound generics only": top-level
type/const/clause params get substituted away, but late-bound regions
(HRTB) are still represented with binders.

### E.3 Translation rejects / errors

`charon/src/bin/charon-driver/translate/translate_types.rs` lines 286-304
shows the variants that are explicitly NOT supported:

```rust
hax::TyKind::Infer(_) => {
    raise_error!(self, span, "Unsupported type: infer type")
}
hax::TyKind::Coroutine(..) => {
    raise_error!(self, span, "Coroutine types are not supported yet")
}
hax::TyKind::Bound(_, _) => {
    raise_error!(self, span, "Unexpected type kind: bound")
}
hax::TyKind::Placeholder(_) => {
    raise_error!(self, span, "Unsupported type: placeholder")
}
hax::TyKind::Error => {
    raise_error!(self, span, "Type checking error")
}
hax::TyKind::Todo(s) => {
    raise_error!(self, span, "Unsupported type: {:?}", s)
}
```

So:

- Coroutines (`async fn`-state machines, generators): hard error.
- Inference variables, type placeholders, raw bound vars: not expected
  to leak in, hard error if they do.

Other rustc cases are mapped:

- `hax::TyKind::Param(param) -> TyKind::TypeVar(<looked-up DeBruijn var>)`.
- `hax::TyKind::Foreign(item) -> TyKind::Adt(<TypeDeclRef of an opaque ADT>)`.
- `hax::TyKind::Closure(args) -> TyKind::Adt(<synthesized ADT>)` (see F).
- `hax::TyKind::Alias(Projection { ... }) -> TyKind::TraitType(...)`.
- `hax::TyKind::Alias(Opaque { hidden_ty, .. })` is unwrapped to
  `hidden_ty` -- so `impl Trait` opaque types are eagerly normalized
  away into their hidden type.
- `hax::TyKind::Arrow(sig) -> TyKind::FnPtr(sig)`.
- `hax::TyKind::FnDef { item, .. } -> TyKind::FnDef(fnref)`.
- `hax::TyKind::Dynamic(dyn_binder, region)` -> `TyKind::DynTrait(...)`,
  registering the vtable type as a side effect.

There is also `--unbind-item-vars` that lifts top-level item binders
into a `Free` variant of `DeBruijnVar`; from the doc on
`DeBruijnVar` (`charon/src/ast/types/vars.rs` ~57-89):

> To make consumption easier for projects that don't do heavy substitution, `--unbind-item-vars` changes the variables bound at the top-level (i.e. in the `GenericParams` of items) to be `Free`.

```rust
pub enum DeBruijnVar<Id> {
    /// A variable attached to the nth binder, counting from the innermost.
    Bound(DeBruijnId, Id),
    /// A variable attached to the outermost binder (the one on the item). ...
    Free(Id),
}

pub type RegionDbVar = DeBruijnVar<RegionId>;
pub type TypeDbVar = DeBruijnVar<TypeVarId>;
pub type ConstGenericDbVar = DeBruijnVar<ConstGenericVarId>;
pub type ClauseDbVar = DeBruijnVar<TraitClauseId>;
```

So Charon explicitly chooses NOT to flatten substitutions into the `Ty`
itself; even after monomorphization, type variables remain inside inner
binders.

---

## F. Bridge / consumption shape

### F.1 ULLBC vs LLBC -- types are identical

Both ULLBC (`charon/src/ast/ullbc_ast.rs`) and LLBC
(`charon/src/ast/llbc_ast.rs`) `pub use crate::ast::*;`. The types,
generics, traits, and item declarations are shared. The only difference
between the two ASTs is body shape (basic blocks vs structured statements).
There is no separate "LLBC `Ty`".

### F.2 Closure lowering

`charon/src/bin/charon-driver/translate/translate_closures.rs` (top-of-file
docs) describes the lowering:

> In rust, closures behave like ADTs that implement the FnOnce/FnMut/Fn traits automatically.
>
> Here we convert closures to a struct containing the closure's state (upvars), along with matching trait impls and fun decls (e.g. a Fn closure will have a trait impl for Fn, FnMut and FnOnce, along with 3 matching method implementations for call, call_mut and call_once).

Concretely a closure becomes:

```rust
struct {parent_fn::closure#0}<...> ( <captured upvars> );
impl<...> FnMut<(<args>,)> for {parent_fn::closure#0}<...> { ... }
impl<...> FnOnce<(<args>,)> for {parent_fn::closure#0}<...> { ... }
```

The synthesized `TypeDecl`'s `src: ItemSource` distinguishes it from
ordinary user types. `ClosureInfo` (types.rs ~970-983) captures the
closure-specific metadata: kind (Fn/FnMut/FnOnce), the three
`*_impl: Option<RegionBinder<TraitImplRef>>`, and the late-bound
`signature: RegionBinder<FunSig>`.

In `TyKind`, a closure value type is just `TyKind::Adt(<closure ADT
ref>)`. There is no `TyKind::Closure`.

### F.3 `dyn Trait` lowering

`translate_types.rs` lines 243-283 builds a `DynPredicate` and as a side
effect registers a vtable type:

```rust
hax::TyKind::Dynamic(dyn_binder, region) => {
    let region = self.translate_region(span, region)?;
    let binder = self.translate_dyn_binder(span, dyn_binder, |ctx, ty, ()| { ... })?;

    if let hax::ClauseKind::Trait(trait_predicate) = dyn_binder.predicates.predicates[0]
        .clause.kind.hax_skip_binder_ref()
    {
        // TODO(dyn): for now, we consider traits with associated types to not be dyn
        // compatible because we don't know how to handle them; for these we skip
        // translating the vtable.
        if self.trait_is_dyn_compatible(&trait_predicate.trait_ref.def_id)? {
            // Ensure the vtable type is translated. The first predicate is the one that
            // can have methods, i.e. a vtable.
            if self.monomorphize() {
                let item_src = TransItemSource::monomorphic_trait(
                    &trait_predicate.trait_ref.def_id,
                    TransItemSourceKind::VTable,
                );
                let _: TypeDeclId = self.register_and_enqueue(span, item_src);
            } else {
                let _: TypeDeclId = self.register_item(
                    span, &trait_predicate.trait_ref, TransItemSourceKind::VTable,
                );
            }
        }
    }
    TyKind::DynTrait(DynPredicate { binder })
}
```

The vtable struct is shaped per
`charon/src/bin/charon-driver/translate/translate_trait_objects.rs`
lines 525-538:

```rust
/// Construct the type of the vtable for this trait.
///
/// It's a struct that has for generics the generics of the trait + one parameter for each
/// associated type of the trait and its parents.
///
/// struct TraitVTable<TraitArgs.., AssocTys..> {
///   size: usize,
///   align: usize,
///   drop: fn(*mut dyn Trait<...>),
///   method_name: fn(&dyn Trait<...>, Args..) -> Output,
///   ... other methods
///   super_trait_0: &'static SuperTrait0VTable
///   ... other supertraits
/// }
```

The vtable fields are enumerated by `TrVTableField` / `VTableField`:

```rust
pub enum TrVTableField {
    Size,
    Align,
    Drop,
    Method(TraitItemName, hax::Binder<hax::TyFnSig>),
    SuperTrait(TraitClauseId, hax::Clause),
}
```

i.e., the vtable is a real `TypeDecl` (Struct), and the trait object
appears in `TyKind::DynTrait(DynPredicate)` with the vtable accessible
via `PtrMetadata::VTable(TypeDeclRef)` at the pointer-metadata layer.

The pass `transform/normalize/transform_dyn_trait_calls.rs` then rewrites
direct `<dyn Trait as Trait>::method(self, args)` calls into vtable
function-pointer calls:

```text
@0 := (move (*@receiver.ptr_metadata).method_check)(move (@receiver), move (@args))
```

So at consumption time, dyn-Trait method calls in LLBC have already been
lowered to vtable indirect calls. The trait-method form survives in
ULLBC; the dyn-trait pass is part of normalization.

### F.4 Function items vs function pointers

`TyKind::FnDef(RegionBinder<FnPtr>)` is a ZST type per Rust function
item; `TyKind::FnPtr(RegionBinder<FunSig>)` is the actual pointer type.
Cast from `FnDef` to `FnPtr` at the value level is preserved as
`Rvalue` cast operations -- see how `ConstantExprKind` distinguishes
`FnDef(FnPtr)` (ZST) from `FnPtr(FnPtr)` (the address).

### F.5 Top-level translation unit

`charon/src/ast/krate.rs` ~177-228:

```rust
pub struct TranslatedCrate {
    pub crate_name: String,
    pub options: crate::options::CliOpts,
    pub target_information: SeqHashMap<TargetTriple, TargetInfo>,
    pub files: IndexVec<FileId, File>,
    pub item_names: SeqHashMap<ItemId, Name>,
    pub short_names: SeqHashMap<ItemId, Name>,
    pub type_decls: IndexMap<TypeDeclId, TypeDecl>,
    pub fun_decls: IndexMap<FunDeclId, FunDecl>,
    pub global_decls: IndexMap<GlobalDeclId, GlobalDecl>,
    pub trait_decls: IndexMap<TraitDeclId, TraitDecl>,
    pub trait_impls: IndexMap<TraitImplId, TraitImpl>,
    pub unit_metadata: Option<GlobalDeclRef>,
    pub ordered_decls: Option<DeclarationsGroups>,
}
```

Five separate id-keyed maps for the five item kinds. Cross-kind
references go through `ItemId` (a tagged enum of ids). All
serialization is `serde_state` based, with the hash-consing interning
state carried through `state_implements = HashConsSerializerState`.

---

## G. Renderer / pretty-printer format

`charon/src/pretty/fmt_with_ctx.rs` lines ~2095-2155 implements
`FmtWithCtx` for `Ty`:

```rust
impl<C: AstFormatter> FmtWithCtx<C> for Ty {
    fn fmt_with_ctx(&self, ctx: &C, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind() {
            TyKind::Adt(tref) => match tref.id {
                TypeId::Tuple => {
                    let generics = tref.generics.fmt_explicits(ctx).format(", ");
                    let trailing_comma = if tref.generics.types.len() == 1 { "," } else { "" };
                    write!(f, "({generics}{trailing_comma})")
                }
                _ => write!(f, "{}", tref.with_ctx(ctx)),
            },
            TyKind::TypeVar(id) => write!(f, "{}", id.with_ctx(ctx)),
            TyKind::Literal(kind) => write!(f, "{kind}"),
            TyKind::Never => write!(f, "!"),
            TyKind::Ref(r, ty, kind) => {
                write!(f, "&{} ", r.with_ctx(ctx))?;
                if let RefKind::Mut = kind { write!(f, "mut ")?; }
                write!(f, "{}", ty.with_ctx(ctx))
            }
            TyKind::RawPtr(ty, kind) => {
                write!(f, "*")?;
                match kind {
                    RefKind::Shared => write!(f, "const")?,
                    RefKind::Mut => write!(f, "mut")?,
                }
                write!(f, " {}", ty.with_ctx(ctx))
            }
            TyKind::Array(ty, len) => {
                write!(f, "[{}; {}]", ty.with_ctx(ctx), len.with_ctx(ctx))
            }
            TyKind::Slice(ty) => {
                write!(f, "[{}]", ty.with_ctx(ctx))
            }
            TyKind::TraitType(trait_ref, name) => {
                write!(f, "{}::{name}", trait_ref.with_ctx(ctx),)
            }
            TyKind::DynTrait(pred) => {
                write!(f, "(dyn {})", pred.with_ctx(ctx))
            }
            TyKind::FnPtr(io) => {
                write!(f, "{}", io.with_ctx(ctx))
            }
            TyKind::FnDef(binder) => {
                let (regions, value) = binder.fmt_split(ctx);
                if !regions.is_empty() { write!(f, "for<{regions}> ",)? };
                write!(f, "{value}",)
            }
            TyKind::PtrMetadata(ty) => {
                write!(f, "PtrMetadata<{}>", ty.with_ctx(ctx))
            }
            TyKind::Error(msg) => write!(f, "type_error(\"{msg}\")"),
        }
    }
}
```

Output conventions:

- Tuples use `(T,)` for arity 1 (with the trailing comma) and `(T1, T2)`
  for arity 2+.
- Borrows: `&<region> [mut] <ty>` -- region is always rendered, even when
  it is `Erased` (which prints as nothing or as a placeholder, see the
  `Region` formatter).
- Raw pointers: `*const <ty>` / `*mut <ty>`.
- Arrays / slices: `[T; N]` / `[T]`.
- Associated types: `<trait_ref>::<name>`.
- `dyn Trait`: parenthesized -- `(dyn <pred>)`.
- Function pointers: rendered through `RegionBinder<FunSig>` formatter
  which produces `for<'a, 'b> fn(...) -> ...`.
- `FnDef`: renders as `for<...> <fnptr>` -- the late-bound regions are
  exposed.
- `PtrMetadata<T>`: explicit "metadata of T" surface syntax.
- `Error("msg")`: surfaces translation errors as `type_error("msg")`.
- Literals delegate to `LiteralTy`'s `Display`. Numeric types likely
  render as `i32`, `u64`, `f32`, etc.; bool as `bool`; char as `char`.

The renderer is text-only and not designed as an exchange format -- the
serialization for downstream tools (Aeneas, Eurydice) is the
`serde_state` JSON dump.

---

## Watch-outs and observations

1. Closures are always synthesized ADTs; closure type identity is by
   `TypeDeclId` of the synthesized struct (named like
   `{parent_fn::closure#N}`). There is no `TyKind::Closure`.

2. There is no `TyKind::Tuple`. Tuples are
   `Adt(TypeDeclRef { id: TypeId::Tuple, generics: <args> })`. The
   pretty-printer special-cases this on output. `unit` is the 0-arity tuple.

3. There is no `TyKind::Box`. `Box<T>` is either an ordinary
   `Adt(TypeId::Adt(<Box's TypeDeclId>))` or, with
   `--treat-box-as-builtin`, `Adt(TypeId::Builtin(BuiltinTy::Box))`.

4. `PhantomData`, `MaybeUninit`, `UnsafeCell`, atomic types, `Cell`,
   `RefCell`, `NonNull`, `Pin`, `ManuallyDrop` are NOT marked at the
   `Ty` level. They are ordinary ADTs. Consumers that care about
   interior mutability or uninhabitedness must inspect the path. The
   `Layout.uninhabited` and `VariantLayout.uninhabited` fields cover
   `!`-containment, not `MaybeUninit`.

5. Coroutines / generators / async-state-machines are not supported:
   `hax::TyKind::Coroutine(..) => raise_error!`. This is a hard error,
   not a fallback to opaque.

6. `impl Trait` opaque types are eagerly unfolded to their hidden type
   (`hax::AliasKind::Opaque { hidden_ty, .. }` -> recurse on
   `hidden_ty`). They do not survive as a `TyKind` variant.

7. Regions are NOT erased to a single token. Charon retains four
   variants: `Var`, `Static`, `Body(RegionId)`, `Erased`. Even after
   `--monomorphize`, `RegionBinder<...>` shows up in `FnPtr`/`FnDef`
   types. So a "post-monomorphization" Charon AST is more like
   "early-bound generics monomorphized; late-bound regions still
   binders".

8. Layout is on `TypeDecl`, not `Ty`. To get a layout for a `TyKind::Adt(tref)`
   you must consult `type_decls[tref.id].layout[target]` and combine it
   with `tref.generics` (Charon does not store per-instance layouts).
   `Layout` is also explicitly "simplified": no niche details, no
   valid-range info on tags.

9. Layout is per-target: a single translated crate can carry layouts
   for multiple `TargetTriple`s. `TranslatedCrate.target_information`
   stores per-target metadata (likely pointer size etc.).

10. `repr(simd)` and `repr(linear)` are deliberately NOT in
    `ReprOptions` (see the note above the struct). `repr(C)`,
    `repr(transparent)`, `repr(packed(n))`, `repr(align(n))`, and
    explicit discriminant types are.

11. Hash-consing is implemented as a process-global static interning
    map. The `intern_table` module file warns that linking Charon both
    statically and as a dylib can break equality. Not a concern for
    in-process use; relevant if a downstream tool embeds Charon twice.

12. `TyKind::PtrMetadata(Ty)` is an unusual variant: it is the type-level
    counterpart of the `<T as Pointee>::Metadata` projection, used for
    DST handling. Combined with `PtrMetadata` (the pointer-metadata
    enum), Charon carefully tracks fat-pointer metadata as both a value
    (length / vtable pointer) and a type (`PtrMetadata<T>`).

13. Const generics live in the same `ConstantExpr` algebra as ordinary
    constants, with `ConstantExprKind::Var(ConstGenericDbVar)` for the
    parameter case. `TyKind::Array(Ty, Box<ConstantExpr>)` is therefore
    aware of partially-evaluated lengths.

14. Trait associated types: pre-pass they appear as `TyKind::TraitType(TraitRef, TraitItemName)`.
    The `expand_associated_types` pass (gated by `--remove-associated-types`)
    rewrites them into extra type parameters; the
    `lift_associated_item_clauses` pass moves clauses-on-items into
    `ParentClause`s. Whether these survive depends on the consumer's
    flag selection.

15. `BuiltinImplData` enumerates the marker/built-in traits Charon
    knows about: `Sized`, `MetaSized`, `Tuple`, `Pointee`,
    `DiscriminantKind`, `Auto` (for any auto trait), plus the closure
    traits `Fn`/`FnMut`/`FnOnce`, plus `Copy`/`Clone`, plus
    `NoopDestruct`/`UntrackedDestruct` for drop bounds. Auto traits are
    homogenized into a single `Auto` discriminator -- the specific
    auto trait is recoverable from the `parent_trait_refs`.

16. `TraitRefKind::Dyn` is a synthetic marker for the
    "automatically-generated implementation for `dyn Trait`". This is
    how `transform_dyn_trait_calls` recognizes dyn-method calls (it
    matches `TraitRefKind::Dyn` on `trait_ref.kind`).

17. `FunSig` does NOT carry an ABI / calling convention. Calling
    conventions, including `extern "C"`, must be recovered (or are
    discarded) by consumers. This may matter for C lowering.

18. `Ty` has 14 `TyKind` variants. There are no extension hooks: there
    is no `TyKind::Custom` / `TyKind::User`; if an `extern type` or an
    unsupported variant is encountered, Charon either uses an opaque
    `TypeDecl` (`TypeDeclKind::Opaque`) or returns
    `TyKind::Error(String)`. The IR has no escape hatch for "unknown
    type" beyond `Error`.

---

## Source URLs

- `charon/src/ast/types.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/ast/types.rs
- `charon/src/ast/types/vars.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/ast/types/vars.rs
- `charon/src/ast/expressions.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/ast/expressions.rs
- `charon/src/ast/krate.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/ast/krate.rs
- `charon/src/ast/builtins.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/ast/builtins.rs
- `charon/src/ast/hash_cons.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/ast/hash_cons.rs
- `charon/src/ast/ullbc_ast.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/ast/ullbc_ast.rs
- `charon/src/ast/llbc_ast.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/ast/llbc_ast.rs
- `charon/src/options.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/options.rs
- `charon/src/pretty/fmt_with_ctx.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/pretty/fmt_with_ctx.rs
- `charon/src/bin/charon-driver/translate/translate_types.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/bin/charon-driver/translate/translate_types.rs
- `charon/src/bin/charon-driver/translate/translate_closures.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/bin/charon-driver/translate/translate_closures.rs
- `charon/src/bin/charon-driver/translate/translate_trait_objects.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/bin/charon-driver/translate/translate_trait_objects.rs
- `charon/src/transform/normalize/partial_monomorphization.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/transform/normalize/partial_monomorphization.rs
- `charon/src/transform/normalize/transform_dyn_trait_calls.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/transform/normalize/transform_dyn_trait_calls.rs
- `charon/src/transform/normalize/expand_associated_types.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/transform/normalize/expand_associated_types.rs
- `charon/src/transform/normalize/skip_trait_refs_when_known.rs` -- https://github.com/AeneasVerif/charon/blob/main/charon/src/transform/normalize/skip_trait_refs_when_known.rs
