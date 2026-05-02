# Prior-art notes: mrustc (type representation and C emission)

Source survey of mrustc, John Hodge / "thepowersgang"
(https://github.com/thepowersgang/mrustc, branch master).

mrustc is a Rust-to-C bootstrap compiler written in C++. Its primary goal is
bootstrapping rustc; the C output is treated as "high-level assembly" passed to
gcc/MSVC. The README notes mrustc "tends to assume that the code it's compiling
is valid (and any errors in the generated code are mrustc bugs)" -- i.e. it
trusts the front-end and crashes loudly on invalid input rather than diagnosing
it. That posture is similar to the Regseq stance of "no defensive fallbacks".

Files surveyed (raw URLs; line numbers are from `master` at time of fetch):

- src/hir/type.hpp                    https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/hir/type.hpp
- src/hir/type_ref.hpp                https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/hir/type_ref.hpp
- src/hir/generic_ref.hpp             https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/hir/generic_ref.hpp
- src/hir/type.cpp                    https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/hir/type.cpp
- src/hir/hir.hpp                     https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/hir/hir.hpp
- src/hir_typeck/static.hpp           https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/hir_typeck/static.hpp
- src/hir_typeck/static.cpp           https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/hir_typeck/static.cpp
- src/mir/mir.hpp                     https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/mir/mir.hpp
- src/trans/target.hpp                https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/trans/target.hpp
- src/trans/target.cpp                https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/trans/target.cpp
- src/trans/codegen.hpp               https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/trans/codegen.hpp
- src/trans/codegen.cpp               https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/trans/codegen.cpp
- src/trans/codegen_c.cpp             https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/trans/codegen_c.cpp
- src/trans/codegen_mmir.cpp          https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/trans/codegen_mmir.cpp
- src/trans/enumerate.cpp             https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/trans/enumerate.cpp
- src/trans/monomorphise.hpp          https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/trans/monomorphise.hpp
- src/trans/codegen_c_structured.cpp  https://raw.githubusercontent.com/thepowersgang/mrustc/master/src/trans/codegen_c_structured.cpp

Architectural note: there is no separate "post-monomorph" type IR. Both
`src/mir/mir.hpp` and `src/trans/codegen.hpp` use `::HIR::TypeRef` directly.
Monomorphisation is implemented as a tree-rewrite over the same `TypeRef`
representation (`Trans_Monomorphise`, `monomorphise.hpp:17`).

---

## A. Type-vocabulary completeness

The single type representation is `HIR::TypeRef` (a refcounted handle around
`HIR::TypeInner { unsigned m_refcount; TypeData m_data; }`). The kind discriminator
is the tag of the `TypeData` tagged union (`src/hir/type.hpp:236-285`):

```cpp
TAGGED_UNION(TypeData, Diverge,
    (Infer, struct {
        unsigned int index;
        InferClass  ty_class;
        bool is_lit() const { ... }
        }),
    (Diverge, struct {}),
    (Primitive, ::HIR::CoreType),
    (Path, TypeData_Path),  // TODO: Pointer wrap
    (Generic, GenericRef),
    (TraitObject, TypeData_TraitObject),  // TODO: Pointer wrap
    (ErasedType, /*::std::unique_ptr<*/TypeData_ErasedType),  // TODO: Pointer wrap
    (Array, struct {
        TypeRef inner;
        ArraySize  size;
        }),
    (Slice, struct {
        TypeRef inner;
        }),
    (Tuple, ::std::vector<TypeRef>),
    (Borrow, struct {
        ::HIR::LifetimeRef  lifetime;
        ::HIR::BorrowType   type;
        TypeRef inner;
        }),
    (Pointer, struct {
        ::HIR::BorrowType   type;
        TypeRef inner;
        }),
    (NamedFunction, struct {
        ::HIR::Path path;
        TypeData_NamedFunction_Ty   def;
        TypeData_FunctionPointer decay(const Span& sp) const;
        }),
    (Function, TypeData_FunctionPointer),   // TODO: Pointer wrap, this is quite large
    (NodeType, TypeData_NodeType)
    );
```

Variant-by-variant cataloguing of what is stored:

- **Infer** -- inference variable. Carries an integer slot and an
  `InferClass { None, Integer, Float }` (literal class).
  Defined in `src/hir/type_ref.hpp:93-98`.
- **Diverge** -- the `!` type. No payload.
- **Primitive** -- a `CoreType` enum. From `src/hir/type.hpp:41-54`:
  ```cpp
  enum class CoreType
  {
      Usize, Isize,
      U8, I8,
      U16, I16,
      U32, I32,
      U64, I64,
      U128, I128,
      F16, F32, F64, F128,
      Bool,
      Char, Str,
  };
  ```
  `Str` is treated as a primitive (the unsized character sequence type).
- **Path** -- nominal path types (struct, union, enum, extern type, opaque
  associated type). Carries a resolved binding pointer
  (`src/hir/type.hpp:111-152`):
  ```cpp
  TAGGED_UNION_EX(TypePathBinding, (), Unbound, (
      (Unbound, struct {}),
      (Opaque, struct {}),
      (ExternType, const ::HIR::ExternType*),
      (Struct, const ::HIR::Struct*),
      (Union, const ::HIR::Union*),
      (Enum, const ::HIR::Enum*)
      ), ...);
  struct TypeData_Path
  {
      ::HIR::Path path;
      TypePathBinding binding;
      ::std::unique_ptr<::HIR::GenericParams> hrtbs;   // HRTBs for vtable paths ONLY
      bool is_closure() const { ... }       // path begins with "closure#"
      bool is_generator() const { ... }     // "generator#"
      bool is_future() const { ... }        // "future#"
  };
  ```
  Note: closures/generators/futures are conveyed as Path types whose final
  component carries a magic prefix. They are detected with substring tests on
  the path component, not by a distinct enum tag.
- **Generic** -- a `GenericRef { RcString name; uint32_t binding; }`
  (`src/hir/generic_ref.hpp:26-80`). The `binding` field is encoded as
  `group * 256 + idx`, with the group enum
  `enum GenericGroup { GENERIC_Impl, GENERIC_Item, GENERIC_Placeholder, GENERIC_Hrtb };`
  and special values `GENERIC_Self = 0xFFFF`, `GENERIC_ErasedSelf = 0xFFFE`.
  After full monomorphisation no `Generic` variants remain (codegen rejects
  them; see Section E).
- **TraitObject** -- `dyn Trait + Markers + 'lifetime`
  (`src/hir/type.hpp:154-159`):
  ```cpp
  struct TypeData_TraitObject
  {
      ::HIR::TraitPath    m_trait;
      ::std::vector< ::HIR::GenericPath > m_markers;
      ::HIR::LifetimeRef  m_lifetime;
  };
  ```
  The principal trait carries its associated-type bindings inside
  `TraitPath`. Auto-trait markers (Send/Sync) are kept as `m_markers`.
- **ErasedType** -- `impl Trait` (`src/hir/type.hpp:160-204`):
  ```cpp
  struct TypeData_ErasedType
  {
      bool m_is_sized;
      ::std::vector< ::HIR::TraitPath>    m_traits;
      ::std::vector< ::HIR::LifetimeRef>  m_lifetime_bounds;
      TypeData_ErasedType_Inner   m_inner;
      ::HIR::PathParams  m_use;
      enum class Use { OmittedOld, Omitted2024, Present, } m_use_present;
  };
  TAGGED_UNION(TypeData_ErasedType_Inner, Alias,
      (Fcn, struct { ::HIR::Path m_origin; unsigned int m_index; }),
      (Known, HIR::TypeRef),
      (Alias, struct {
          ::HIR::PathParams   params;
          ::std::shared_ptr<TypeData_ErasedType_AliasInner>   inner;
          })
  );
  ```
  After monomorphisation it carries the resolved underlying type in `Known`.
- **Array** -- inner `TypeRef` plus `ArraySize`:
  ```cpp
  TAGGED_UNION_EX(ArraySize, (), Unevaluated, (
      (Unevaluated, ConstGeneric),
      (Known, uint64_t)
      ), ...);
  ```
  Post-monomorphisation only the `Known` variant survives.
- **Slice** -- inner `TypeRef` only.
- **Tuple** -- `std::vector<TypeRef>` (also represents the unit type).
- **Borrow** -- `{ LifetimeRef lifetime; BorrowType type; TypeRef inner; }`
  with `enum class BorrowType { Shared, Unique, Owned };`. `Shared` = `&T`,
  `Unique` = `&mut T`, `Owned` = box-like move-borrow used internally.
- **Pointer** -- raw pointer `*const T` / `*mut T`, sharing `BorrowType`.
  (`Owned` is also accepted at this layer but is internal-use.)
- **NamedFunction** -- a Rust "fn item" type, naming a specific function
  resolution rather than a function pointer. Stores the resolved item
  (`src/hir/type.hpp:214-221`):
  ```cpp
  TAGGED_UNION_EX(TypeData_NamedFunction_Ty, (), Function, (
      (Function, const ::HIR::Function* ),
      (EnumConstructor, struct { const ::HIR::Enum* e; size_t v; }),
      (StructConstructor, const ::HIR::Struct*)
      ), ...);
  ```
  Convertible to a `TypeData_FunctionPointer` via `decay(const Span&)`.
- **Function** -- a function pointer, `TypeData_FunctionPointer`
  (`src/hir/type.hpp:205-213`):
  ```cpp
  struct TypeData_FunctionPointer
  {
      GenericParams   hrls;   // Higher-ranked lifetimes
      bool    is_unsafe;
      bool    is_variadic;
      RcString    m_abi;
      TypeRef m_rettype;
      ::std::vector<TypeRef>  m_arg_types;
  };
  ```
  ABI is stored as a string ("Rust", "C", "system", "rust-call", ...).
- **NodeType** -- closures / generators / async blocks at the HIR layer
  (`src/hir/type.hpp:223-234`):
  ```cpp
  TAGGED_UNION_EX(TypeData_NodeType, (), Closure, (
      (Closure, const ::HIR::ExprNode_Closure*),
      (Generator, const ::HIR::ExprNode_Generator*),  // Aka a coroutine
      (Async, const ::HIR::ExprNode_AsyncBlock*)
      ), ...);
  ```
  Eliminated before codegen (see Section E).

Notable absent kinds: there is no separate `Never`/`Never2` distinction
(Diverge covers `!`); no separate "FnDef" vs "FnPtr" -- `NamedFunction` covers
fn items; there is no `Bound` / `Projection` post-resolution (associated types
either resolve to a real path with `binding` populated, or remain `Path` with
`TypePathBinding::Opaque` during inference).

## B. Layout / representation

Layout is **not** stored on `TypeRef`. It is computed on demand by the trans
backend through a global cache rooted at:

```cpp
extern const TypeRepr* Target_GetTypeRepr(const Span& sp,
    const StaticTraitResolve& resolve, const ::HIR::TypeRef& ty);
extern bool Target_GetSizeOf(...);
extern bool Target_GetAlignOf(...);
extern bool Target_GetSizeAndAlignOf(...);
```
(`src/trans/target.hpp:175-184`).

The shape of the cached layout (`src/trans/target.hpp:82-163`):

```cpp
struct TypeRepr
{
    size_t  align = 0;
    size_t  size = 0;

    struct FieldPath {
        size_t  index;
        size_t  size;
        ::std::vector<size_t>   sub_fields;
    };

    TAGGED_UNION(VariantMode, None,
    (None, struct {}),
    (Linear, struct {
        FieldPath   field;
        size_t  offset;
        size_t  num_variants;
        bool uses_niche() const { return !field.sub_fields.empty(); }
        bool is_niche(unsigned var_idx) const { ... }
        bool is_tag(unsigned var_idx) const { ... }
        }),
    (Values, struct {
        FieldPath   field;
        ::std::vector<uint64_t> values;
        bool is_tag(unsigned var_idx) const { ... }
        }),
    (NonZero, struct {
        FieldPath   field;
        unsigned    zero_variant;
        })
    );
    VariantMode variants;

    struct Field {
        size_t  offset;
        ::HIR::TypeRef  ty;
    };
    ::std::vector<Field>    fields;

    size_t get_offset(...) const;
    std::pair<unsigned, bool> get_enum_variant(...) const;
};
```

Key consequences:
- The bridge `MIR -> RegseqModel`-equivalent in mrustc is "just put the
  `HIR::TypeRef` in the right slots". Layout is derived by codegen.
- Layout records contain explicit field offsets (post-reordering) and a
  discriminant scheme (`None`, `Linear`, `Values`, `NonZero`).
- The repr discriminator is *not* on `TypeRef` -- it is on the `HIR::Struct`,
  `HIR::Union`, `HIR::Enum` definitions (`src/hir/hir.hpp`):
  ```cpp
  class Struct {
      enum class Repr { Rust, C, Simd, Transparent };
      Repr    m_repr;
      Data    m_data;
      unsigned    m_forced_alignment = 0;
      unsigned    m_max_field_alignment = 0;    // for packed
      TraitMarkings   m_markings;
      StructMarkings  m_struct_markings;
      ...
  };
  class Union {
      enum class Repr { Rust, C, Transparent };
      ...
  };
  class Enum {
      enum class Repr { Auto, Usize, U8, U16, U32, U64, Isize, I8, I16, I32, I64 };
      bool    m_is_c_repr;
      Repr    m_tag_repr;
      ...
  };
  ```
- Layout selection switches on these flags, e.g. `src/trans/target.cpp:1165-1183`:
  ```cpp
  switch(str.m_repr)
  {
  case ::HIR::Struct::Repr::C:
  case ::HIR::Struct::Repr::Simd:
      sorting = StructSorting::None;
      break;
  case ::HIR::Struct::Repr::Transparent:
  case ::HIR::Struct::Repr::Rust:
      if( str.m_struct_markings.dst_type != HIR::StructMarkings::DstType::None )
          sorting = StructSorting::AllButFinal;
      else
          sorting = StructSorting::All;
      break;
  }
  ```
- DST shape is kept on the *struct* via `StructMarkings` (`src/hir/hir.hpp:288-321`):
  ```cpp
  struct StructMarkings
  {
      bool    can_unsize = false;
      unsigned int unsized_param = ~0u;
      enum class DstType { None, Possible, Slice, TraitObject, } dst_type = DstType::None;
      unsigned int unsized_field = ~0u;
      enum class Coerce { None, Passthrough, Pointer, } coerce_unsized = Coerce::None;
      unsigned int coerce_unsized_index = ~0u;
      unsigned int coerce_param = ~0u;
      bool is_nonzero = false;             // #[rustc_nonnull_optimization_guaranteed]
      bool bounded_max = false;            // #[rustc_layout_scalar_valid_range_end]
      U128 bounded_max_value;
  };
  ```
- 128-bit integer emulation is a *backend* option, not a type property:
  `BackendOptsC::m_emulated_i128` (`src/trans/target.hpp:65`). The C printer
  emits either a `__int128`-style scalar or a struct fallback based on it
  (see `src/trans/codegen_c.cpp:603` ff).

## C. Identity, interning, canonical form

There is NO interning of `TypeRef`s. Each `TypeData` is held in a refcounted
`TypeInner` allocated on first construction (`src/hir/type.hpp:287-303`):

```cpp
class TypeInner
{
    friend class TypeRef;
    unsigned    m_refcount;
public:
    TypeData   m_data;
private:
    TypeInner(TypeData d): m_refcount(1), m_data(mv$(d)) {}
};
```

Refcounting is hand-rolled; the public `TypeRef` is `(TypeInner* m_ptr)`,
manually incremented in copy-construct and decremented in dtor. `clone()` is
a refcount bump; `clone_shallow()` and `get_unique()` provide
copy-on-write of the top layer.

Static globals exist for a few common types (`new_self()`, `new_unit()`,
`new_diverge()`) which return a refcount-clone of a thread-local singleton --
this is the closest thing to interning, and only for those three.

Equality and ordering are structural and recursive (`src/hir/type.cpp:434-528`,
`554-627`). They are tag-compared first, then content-compared; pointer
equality (`m_ptr == x.m_ptr`) is an early-exit optimisation but not relied on:

```cpp
bool ::HIR::TypeRef::operator==(const ::HIR::TypeRef& x) const
{
    if( m_ptr == x.m_ptr )
        return true;
    if( !m_ptr || !x.m_ptr )
        return false;
    if( data().tag() != x.data().tag() )
        return false;
    TU_MATCH_HDRA( (data(), x.data()), {)
    TU_ARMA(Infer, te, xe) { return te.index == xe.index; }
    TU_ARMA(Diverge, te, xe) { return true; }
    TU_ARMA(Primitive, te, xe) { return te == xe; }
    TU_ARMA(Path, te, xe) { return te.path == xe.path; }
    TU_ARMA(Generic, te, xe) { return te.binding == xe.binding; }
    TU_ARMA(TraitObject, te, xe) {
        if( te.m_trait != xe.m_trait ) return false;
        if( te.m_markers.size() != xe.m_markers.size() ) return false;
        for(unsigned i = 0; i < te.m_markers.size(); i++)
            if( te.m_markers[i] != xe.m_markers[i] ) return false;
        return true;          // lifetimes deliberately NOT compared
        }
    TU_ARMA(ErasedType, te, xe) { return te.m_inner == xe.m_inner; }
    TU_ARMA(Array, te, xe) {
        if( te.inner != xe.inner ) return false;
        if( xe.size != te.size ) return false;
        return true;
        }
    TU_ARMA(Slice, te, xe) { return te.inner == xe.inner; }
    TU_ARMA(Tuple, te, xe) { ... }
    TU_ARMA(Borrow, te, xe) {
        if( te.type != xe.type ) return false;
        // lifetimes deliberately NOT compared
        return te.inner == xe.inner;
        }
    TU_ARMA(Pointer, te, xe) {
        if( te.type != xe.type ) return false;
        return te.inner == xe.inner;
        }
    TU_ARMA(NamedFunction, te, xe) { return te.path == xe.path; }
    TU_ARMA(Function, te, xe) {
        if( te.is_unsafe != xe.is_unsafe ) return false;
        if( te.m_abi != xe.m_abi ) return false;
        if( te.m_arg_types.size() != xe.m_arg_types.size() ) return false;
        for(unsigned i = 0; i < te.m_arg_types.size(); i++)
            if( te.m_arg_types[i] != xe.m_arg_types[i] ) return false;
        return te.m_rettype == xe.m_rettype;
        }
    TU_ARMA(NodeType, te, xe) { return te == xe; }
    }
    throw "";
}
```

Notes:
- Lifetimes are deliberately ignored in equality of `Borrow` and `TraitObject`
  (matching the post-region-erasure model).
- Function-pointer equality ignores `hrls` and `is_variadic` (the latter
  appears to be an oversight).
- There is no separate hash function, no canonical form, no interning table.
  Maps keyed by `TypeRef` use the structural `<` ordering instead
  (e.g. `std::map<HIR::TypeRef, bool>` caches in `static.hpp:40-43`).
- `clone_shallow()` provides copy-on-write so mutators in the type-checker can
  edit a `TypeData` without disturbing other holders.
- The `Path` variant compares only `te.path == xe.path`; the `binding`
  pointer is treated as derived information (it's expected to agree if the
  paths agree).

`get_sort_path()` (`src/hir/type.hpp:396-412`) returns the leading
`SimplePath` for sorting type lists.

## D. Special-semantics markers

mrustc keeps NO per-`TypeRef` flag for interior mutability, "may be
uninitialized", etc. The compiler instead asks the resolver questions about a
given `HIR::TypeRef` when it needs to.

Interior mutability check (`src/hir_typeck/static.cpp:2831-...`):

```cpp
// Check if the passed type contains an UnsafeCell
// Returns `Fuzzy` if generic, `Equal` if it does contain an UnsafeCell, and `Unequal` if it doesn't (shared=immutable)
HIR::Compare StaticTraitResolve::type_is_interior_mutable(const Span& sp, const ::HIR::TypeRef& ty) const
{
    TU_MATCH_HDRA( (ty.data()), {)
    ...
    TU_ARMA(Path, e) {
        ...
        TU_ARMA(Struct, pbe) {
            const HIR::GenericPath& p = e.path.m_data.as_Generic();
            if( p.m_path == m_crate.get_lang_item_path(sp, "unsafe_cell") ) {
                return HIR::Compare::Equal;
            }
            // recursively scan tuple/named struct fields
            ...
```

`UnsafeCell` is detected by lang-item path identity. The check then recurses
through struct/enum/union/array/tuple/borrow children. Three-valued result:
`Equal` (yes), `Unequal` (no), `Fuzzy` (don't know yet because of generics).

`PhantomData` and `Box` are detected the same way
(`src/hir_typeck/static.cpp:3295-3330`):

```cpp
const ::HIR::TypeRef* StaticTraitResolve::is_type_owned_box(const ::HIR::TypeRef& ty) const
{
    if( ! ty.data().is_Path() ) return nullptr;
    const auto& te = ty.data().as_Path();
    if( ! te.path.m_data.is_Generic() ) return nullptr;
    const auto& pe = te.path.m_data.as_Generic();
    if( pe.m_path != m_lang_Box ) return nullptr;
    return &pe.m_params.m_types.at(0);
}
const ::HIR::TypeRef* StaticTraitResolve::is_type_phantom_data(const ::HIR::TypeRef& ty) const
{
    if( ! ty.data().is_Path() ) return nullptr;
    const auto& te = ty.data().as_Path();
    if( ! te.path.m_data.is_Generic() ) return nullptr;
    const auto& pe = te.path.m_data.as_Generic();
    if( pe.m_path != m_lang_PhantomData ) return nullptr;
    return &pe.m_params.m_types.at(0);
}
```

`MaybeUninit` has no special handling at all -- a search of `codegen_c.cpp`
and `target.cpp` for `MaybeUninit` returns zero hits. mrustc treats
`MaybeUninit<T>` as just another transparent struct; storage is emitted with
the same layout as `T`. Likewise, **`volatile` is not used in C output for
Rust types** (`__asm__ __volatile__` shows up only inside intrinsic
implementations, e.g. `xgetbv` and `pause`).

Atomics: there is no atomic type kind. The C codegen emits
`#include <stdatomic.h>` (`codegen_c.cpp:291`) and synthesises atomic
operations inside intrinsic helpers like `__mrustc_atomicloop<N>`. Atomics
are realised by **operations**, not by a marker on the type. The target tells
codegen which integer widths support atomics:

```cpp
struct TargetArch {
    ...
    struct Atomics {
        bool u8 = true; bool u16 = true; bool u32 = true;
        bool u64 = false; bool ptr = true;
        ...
    } m_atomics;
    ...
};
```
(`src/trans/target.hpp:21-62`).

PhantomData is handled at the *value* layer in codegen (skip its assignment
and skip stores in cast lowering) -- e.g. `codegen_c.cpp:3667-3678`:

```cpp
// Don't emit assignment of PhantomData
if( vals[j].is_LValue() && m_resolve.is_type_phantom_data(ty) )
{
    continue ;
}
```
and in `emit_rvalue_cast` (`codegen_c.cpp:4237-4240`):

```cpp
if (m_resolve.is_type_phantom_data(ve.type)) {
    m_of << "/* PhantomData cast */\n";
    return;
}
```

ZSTs are detected purely by computed size (`Target_GetSizeAndAlignOf`
returning `s == 0`). They are emitted as `// ZST` comments and elided from
struct layouts when the option `disallow_empty_structs` is set
(`codegen_c.cpp:1877-1880` and `codegen_c.cpp:2266-2287`). The unit type is
typedef'd to `tUNIT`.

DST detection is via `MetadataType` (`src/hir_typeck/static.hpp:16-22`):

```cpp
enum class MetadataType {
    Unknown,        // Unknown still
    None,           // Sized pointer
    Zero,           // No metadata, but still unsized
    Slice,          // usize metadata
    TraitObject,    // VTable pointer metadata
};
```

This is a *property of a type*, computed by `metadata_type()`
(`src/hir_typeck/static.cpp:3013-...`), not stored on `TypeRef`. Codegen calls
it from `emit_ctype_ptr` (`codegen_c.cpp:8418-8440`) to decide what wide
pointer to emit (see Section F).

## E. Post-monomorphization erasure

mrustc does **not** introduce a separate post-monomorph type IR. Both the MIR
container and the codegen interface are typed with `::HIR::TypeRef`:

- `::std::vector< ::HIR::TypeRef>  locals;` in the MIR function
  (`src/mir/mir.hpp:749`).
- `virtual void emit_type(const ::HIR::TypeRef& ) {}` and
  `virtual void emit_struct(... const ::HIR::Struct& ...) = 0;` in the
  codegen interface (`src/trans/codegen.hpp:35-42`).

What changes is which TypeData variants may legally appear. The C backend's
`emit_ctype` (`codegen_c.cpp:8251-8359`) treats some variants as bugs (i.e.
erased before this point):

```cpp
TU_ARMA(Generic, te) {
    MIR_BUG(*m_mir_res, "Generic in trans - " << ty);
    }
TU_ARMA(TraitObject, te) {
    MIR_BUG(*m_mir_res, "Raw trait object - " << ty);
    }
TU_ARMA(ErasedType, te) {
    MIR_BUG(*m_mir_res, "ErasedType in trans - " << ty);
    }
TU_ARMA(Slice, te) {
    MIR_BUG(*m_mir_res, "Raw slice object - " << ty);
    }
...
case ::HIR::TypeData::TAG_NodeType:
    MIR_BUG(*m_mir_res, "NodeType during trans - " << ty);
    break;
```

So at codegen time:
- `Generic` is gone (resolved by monomorphisation).
- `ErasedType` is gone (resolved to its underlying `Known` type).
- Bare `TraitObject` and bare `Slice` are gone *as values*: they only appear
  as the inner type of a `Borrow`/`Pointer`, where the wide-pointer rule in
  `emit_ctype_ptr` handles them.
- `NodeType` is gone: closures, generators, and async blocks are lowered to
  named struct paths (with components beginning `closure#`, `generator#`,
  `future#`). `enumerate.cpp:981-983` confirms this:
  ```cpp
  TU_ARMA(NodeType, te) {
      BUG(sp, "NodeType type hit in enumeration - " << ty);
      }
  ```
- `Infer` is gone (no inference variables after typeck).
- `ArraySize::Unevaluated` is collapsed to `Known` by const-eval.
- `Diverge` (`!`) survives but emits as a placeholder `tBANG` typedef.
- `NamedFunction` (fn-item types) survive; codegen emits each as a unique
  empty/single-byte struct (a ZST per fn item; see `codegen_c.cpp:1969-1975`),
  preserving the fn-item-vs-fn-pointer distinction in the C type system.

Bottom line: erasure is by *invariant on the TypeData variant set*, not by
swapping to a new type. Anything that survives is the resolved C-shape.

## F. Bridge / consumption shape: how Ty lowers to C

The mapping from `TypeData` to C is implemented by
`emit_ctype(const ::HIR::TypeRef& ty, ::FmtLambda inner, bool is_extern_c=false)`
in `codegen_c.cpp:8251-8360`. The relevant arms:

```cpp
TU_ARMA(Primitive, te) {
    switch(te)
    {
    case ::HIR::CoreType::Usize:    m_of << "uintptr_t"; break;
    case ::HIR::CoreType::Isize:    m_of << "intptr_t"; break;
    case ::HIR::CoreType::U8:  m_of << "uint8_t"; break;
    case ::HIR::CoreType::I8:  m_of << "int8_t"; break;
    case ::HIR::CoreType::U16: m_of << "uint16_t"; break;
    case ::HIR::CoreType::I16: m_of << "int16_t"; break;
    case ::HIR::CoreType::U32: m_of << "uint32_t"; break;
    case ::HIR::CoreType::I32: m_of << "int32_t"; break;
    case ::HIR::CoreType::U64: m_of << "uint64_t"; break;
    case ::HIR::CoreType::I64: m_of << "int64_t"; break;
    case ::HIR::CoreType::U128: m_of << "uint128_t"; break;     // typedef of __int128 OR an emulated struct
    case ::HIR::CoreType::I128: m_of << "int128_t"; break;
    case ::HIR::CoreType::F16:  m_of << "f16"; break;
    case ::HIR::CoreType::F32:  m_of << "float"; break;
    case ::HIR::CoreType::F64:  m_of << "double"; break;
    case ::HIR::CoreType::F128: m_of << "f128"; break;
    case ::HIR::CoreType::Bool: m_of << "RUST_BOOL"; break;
    case ::HIR::CoreType::Char: m_of << "RUST_CHAR"; break;
    case ::HIR::CoreType::Str:
        MIR_BUG(*m_mir_res, "Raw str");
    }
    m_of << " " << inner;
    }
TU_ARMA(Path, te) {
    TU_MATCH_HDRA( (te.binding), { )
    TU_ARMA(Struct, tpb)     { m_of << "struct s_" << Trans_Mangle(te.path); }
    TU_ARMA(Union, tpb)      { m_of << "union u_" << Trans_Mangle(te.path); }
    TU_ARMA(Enum, tpb)       { m_of << "struct e_" << Trans_Mangle(te.path); }
    TU_ARMA(ExternType, tpb) { m_of << "struct x_" << Trans_Mangle(te.path); }
    TU_ARMA(Unbound, tpb) { MIR_BUG(*m_mir_res, "Unbound type path in trans - " << ty); }
    TU_ARMA(Opaque, tpb)  { MIR_BUG(*m_mir_res, "Opaque path in trans - " << ty); }
    }
    m_of << " " << inner;
    }
TU_ARMA(Array, te) {
    m_of << "t_" << Trans_Mangle(ty) << " " << inner;
    }
TU_ARMA(Tuple, te) {
    if( te.size() == 0 )
        m_of << "tUNIT";
    else {
        m_of << "TUP_" << te.size();
        for(const auto& t : te) m_of << "_" << Trans_Mangle(t);
    }
    m_of << " " << inner;
    }
TU_ARMA(Borrow, te)        { emit_ctype_ptr(te.inner, inner); }
TU_ARMA(Pointer, te)       { emit_ctype_ptr(te.inner, inner); }
TU_ARMA(NamedFunction, te) { m_of << "t_" << Trans_Mangle(ty) << " " << inner; }
TU_ARMA(Function, te)      { m_of << "t_" << Trans_Mangle(ty) << " " << inner; }
```

So mrustc's "C type-name" function is structural and *name-mangling-driven*:
arrays, tuples, fn pointers, fn items each get a synthesised typedef whose
name is `Trans_Mangle(ty)`-derived; nominal types get a prefix-based name
(`s_` struct / `u_` union / `e_` enum / `x_` extern type) followed by the
mangled path. The actual struct/union body is emitted separately by
`emit_struct`/`emit_union`/`emit_enum`/`emit_type` against the cached
`TypeRepr*` (Section B).

Pointer lowering (`codegen_c.cpp:8418-8440`):

```cpp
void emit_ctype_ptr(const ::HIR::TypeRef& inner_ty, ::FmtLambda inner) {
    switch( this->metadata_type(inner_ty) )
    {
    case MetadataType::Unknown:
        BUG(sp, inner_ty << " unknown metadata type");
    case MetadataType::None:
    case MetadataType::Zero:
        emit_ctype(inner_ty, FMT_CB(ss, ss << "*" << inner;));
        break;
    case MetadataType::Slice:
        m_of << "SLICE_PTR " << inner;
        break;
    case MetadataType::TraitObject:
        m_of << "TRAITOBJ_PTR " << inner;
        break;
    }
}
```

The wide-pointer struct typedefs are runtime-printed boilerplate:
`typedef struct { void* PTR; size_t META; } SLICE_PTR;` and
`typedef struct { void* PTR; void* META; } TRAITOBJ_PTR;` (paraphrased from
the prelude in `codegen_c.cpp`). All trait-object dispatch happens through
the META vtable pointer.

Struct emission uses `TypeRepr` directly. The `emit_struct_inner` body
(`codegen_c.cpp:1754-1938`) sorts repr fields by offset, injects
`uint8_t _paddingN[n]` between fields, marks ZST fields with `// ZST`
comments, and handles trailing unsized fields specially:

```cpp
if( const auto* te = ty.data().opt_Slice() ) {
    emit_ctype( te->inner, FMT_CB(ss, ss << "_" << fld << "[0]";) );
    has_unsized = true;
}
else if( ty.data().is_TraitObject() ) {
    m_of << "unsigned char _" << fld << "[0]";
    has_unsized = true;
}
else if( ty == ::HIR::CoreType::Str ) {
    m_of << "uint8_t _" << fld << "[0]";
    has_unsized = true;
}
else if( TU_TEST1(ty.data(), Path, .binding.is_ExternType()) ) {
    m_of << "// External";
    has_unsized = true;
}
```

`repr(packed)` is realised as `#pragma pack(push, N)` ... `#pragma pack(pop)`
around the struct body (`codegen_c.cpp:1786-1790`, `1934-1937`); manual
`align(N)` is realised with `__declspec(align(N))` on MSVC and
`__attribute__((__aligned__(N)))` on GCC (`codegen_c.cpp:1791-1801`,
`1898-1924`). The compiler then emits a
`typedef char sizeof_assert_<mangled>[ (sizeof(...) == N) ? 1 : -1 ];`
plus an `alignof_assert_` line as static guards
(`codegen_c.cpp:1959-1963`, `2055-2060`).

Function-pointer typedefs are produced by `emit_type_fn`
(`codegen_c.cpp` near line 1716-1745):

```cpp
emit_ctype(te.m_rettype);
m_of << "(*"; emit_ctype(ty); m_of << ")(";
... per-arg: emit_ctype(...) ...
if( te.is_variadic ) m_of << ", ...";
m_of << " )";
```

with `__stdcall` injected for `extern "system"` on MSVC.

NamedFunction (fn-item types) gets a per-item ZST typedef
(`codegen_c.cpp:1969-1975`):

```cpp
TU_ARMA(NamedFunction, te) {
    m_of << "typedef struct "; emit_ctype(ty); m_of << " {";
    if( m_options.disallow_empty_structs ) {
        m_of << " char _unused; ";
    }
    m_of << "} "; emit_ctype(ty); m_of << ";\n";
    }
```

Closures: by codegen time these are nominal Path types
(`closure#0`-style names). They are emitted exactly like ordinary structs
through `emit_struct` -- no special closure path in the C backend.

## G. Renderer / pretty-printer format

Two distinct printers exist:

### G.1 Rust-syntax printer (debug)

`HIR::TypeRef::fmt(std::ostream&)` (`src/hir/type.cpp:224-378`) is the canonical
pretty-printer; `operator<<` simply forwards to it. It targets a
debug/Rust-flavoured form, with binding hints in `/* */`:

```cpp
TU_ARMA(Infer, e) {
    os << "_";
    if( e.index != ~0u || e.ty_class != ::HIR::InferClass::None ) {
        os << "/*";
        if(e.index != ~0u)  os << e.index;
        switch(e.ty_class)
        {
        case ::HIR::InferClass::None:   break;
        case ::HIR::InferClass::Float:  os << ":f"; break;
        case ::HIR::InferClass::Integer:os << ":i"; break;
        }
        os << "*/";
    }
    }
TU_ARMA(Diverge, e)   { os << "!"; }
TU_ARMA(Primitive, e) { os << e; }   // "u32", "f64", "bool", "char", ...
TU_ARMA(Path, e) {
    os << e.path;
    TU_MATCH(::HIR::TypePathBinding, (e.binding), (be),
    (Unbound, os << "/*?*/";),
    (Opaque, os << "/*O*/";),
    (ExternType, os << "/*X*/";),
    (Struct, os << "/*S*/";),
    (Union, os << "/*U*/";),
    (Enum, os << "/*E*/";)
    )
    }
TU_ARMA(Generic, e) { os << e; }     // "<name>/*<group:idx>*/"
TU_ARMA(TraitObject, e) {
    os << "dyn (";
    if( e.m_trait.m_path != ::HIR::GenericPath() ) os << e.m_trait;
    for(const auto& tr : e.m_markers) os << "+" << tr;
    if( e.m_lifetime != LifetimeRef::new_static() ) os << "+" << e.m_lifetime;
    os << ")";
    }
TU_ARMA(ErasedType, e) {
    os << "impl ";
    for(const auto& tr : e.m_traits) {
        if( &tr != &e.m_traits[0] ) os << "+";
        os << tr;
    }
    if( !e.m_lifetime_bounds.empty() ) for(const auto& lft : e.m_lifetime_bounds) os << "+" << lft;
    os << "+use" << e.m_use;
    os << "/*";
    TU_MATCH_HDRA( (e.m_inner), {)
    TU_ARMA(Known, ee) { os << "= " << ee; }
    TU_ARMA(Fcn, ee) { os << "fn " << ee.m_origin << "#" << ee.m_index; }
    TU_ARMA(Alias, ee) { os << "type" << ee.params << " " << ee.inner->path; }
    }
    os << "*/";
    }
TU_ARMA(Array, e) { os << "[" << e.inner << "; " << e.size << "]"; }
TU_ARMA(Slice, e) { os << "[" << e.inner << "]"; }
TU_ARMA(Tuple, e) {
    os << "(";
    for(const auto& t : e) os << t << ", ";
    os << ")";
    }
TU_ARMA(Borrow, e) {
    os << "&";
    os << e.lifetime << " ";
    switch(e.type)
    {
    case ::HIR::BorrowType::Shared: os << "";  break;
    case ::HIR::BorrowType::Unique: os << "mut ";  break;
    case ::HIR::BorrowType::Owned:  os << "move "; break;
    }
    os << e.inner;
    }
TU_ARMA(Pointer, e) {
    switch(e.type)
    {
    case ::HIR::BorrowType::Shared: os << "*const "; break;
    case ::HIR::BorrowType::Unique: os << "*mut ";  break;
    case ::HIR::BorrowType::Owned:  os << "*move "; break;
    }
    os << e.inner;
    }
TU_ARMA(NamedFunction, e) {
    os << "fn{" << (e.def.is_Function() && !e.def.as_Function() ? "!" : "") << e.path <<"}";
    }
TU_ARMA(Function, e) {
    if( !e.hrls.m_lifetimes.empty() ) os << "for" << e.hrls.fmt_args() << " ";
    if( e.is_unsafe ) os << "unsafe ";
    if( e.m_abi != "" ) os << "extern \"" << e.m_abi << "\" ";
    os << "fn(";
    for(const auto& t : e.m_arg_types) os << t << ", ";
    if( e.is_variadic ) os << "...";
    os << ") -> " << e.m_rettype;
    }
TU_ARMA(NodeType, e) { e.fmt(os); }   // "closure[<ptr>]" / "generator[<ptr>]" / "async[<ptr>]"
```

It also has a thread-local recursion guard (prints `RECURSE` on cycles --
`src/hir/type.cpp:231-245`) and an `e.index/InferClass` debug suffix on
inference vars.

`GenericRef::fmt` formats as `<name>/*I:<idx>*/` (or `M:`, `P:`, `H:`)
(`src/hir/type.cpp:73-91`). `LifetimeRef::operator<<`
(`src/hir/generic_ref.hpp:129-162`) formats as `'static`, `'_`, `'#omitted`,
`'I<n>` etc. -- i.e. lifetimes are *kept in the printer* for debugging,
even though they are ignored by `operator==`.

### G.2 MMIR printer (textual MIR dump)

`codegen_mmir.cpp` provides a more compact form for the MMIR text dump
(`src/trans/codegen_mmir.cpp:45-118`). It prints mangled names instead of
human paths and refuses pre-monomorph variants:

```cpp
::std::ostream& operator<<(::std::ostream& os, const Fmt<::HIR::TypeRef>& x)
{
    TU_MATCH_HDRA( (x.e.data()), {)
    TU_ARMA(Infer, te)     BUG(Span(), "" << x.e);
    TU_ARMA(Diverge, te)   { os << "!"; }
    TU_ARMA(Primitive, te) { os << te; }
    TU_ARMA(Path, te)      { os << Trans_Mangle(te.path); }
    TU_ARMA(Generic, te)   BUG(Span(), "" << x.e);
    TU_ARMA(TraitObject, te) {
        auto path = te.m_trait.m_path.clone();
        os << "dyn " << Trans_Mangle(path);
        }
    TU_ARMA(ErasedType, te) BUG(Span(), "" << x.e);
    TU_ARMA(Array, te) { os << "[" << fmt(te.inner) << "; " << te.size << "]"; }
    TU_ARMA(Slice, te) { os << "[" << fmt(te.inner) << "]"; }
    TU_ARMA(Tuple, te) {
        if( te.empty() ) os << "()"; else os << Trans_Mangle(x.e);
        }
    TU_ARMA(Borrow, te) {
        switch(te.type) {
        case ::HIR::BorrowType::Shared: os << "&";  break;
        case ::HIR::BorrowType::Unique: os << "&mut ";  break;
        case ::HIR::BorrowType::Owned:  os << "&move "; break;
        }
        os << fmt(te.inner);
        }
    TU_ARMA(Pointer, te) {
        switch(te.type) {
        case ::HIR::BorrowType::Shared: os << "*const ";  break;
        case ::HIR::BorrowType::Unique: os << "*mut ";    break;
        case ::HIR::BorrowType::Owned:  os << "*move ";   break;
        }
        os << fmt(te.inner);
        }
    TU_ARMA(NamedFunction, te) { os << "fn " << Trans_Mangle(te.path); }
    TU_ARMA(Function, e) {
        if( e.is_unsafe ) os << "unsafe ";
        if( e.m_abi != "" ) os << "extern \"" << e.m_abi << "\" ";
        os << "fn(";
        for(const auto& t : e.m_arg_types) os << fmt(t) << ", ";
        os << ") -> " << fmt(e.m_rettype);
        } break;
    case ::HIR::TypeData::TAG_NodeType:
        BUG(Span(), "Unexpected type in trans: " << x.e);
        break;
    }
    return os;
}
```

This is the closest thing to a "post-monomorph canonical text dump". Its
shape is: same structure as the HIR printer but with `Trans_Mangle(...)`
replacing path printing and pre-monomorph variants treated as compiler
bugs. Tuple types print as `()` for unit and `Trans_Mangle(ty)` (the
mangled tuple typedef name) otherwise -- the actual tuple structure is
visible through the typedef header that codegen also emits.

The MMIR struct/union/enum dumper (`codegen_mmir.cpp:321-...`) emits
field-offset listings:

```cpp
m_of << "type " << fmt(ty) << " {\n";
...
m_of << "\t" << e.offset << " = " << fmt(e.ty) << ";\n";
```

i.e. layout is part of the dump form (offset = type), keyed off the same
`TypeRepr`.

---

## Watch-outs and observations

1. **No interning, no hash-cons.** Refcounting is per-instance; structural
   equality is the only canonical-form mechanism. Identity-based
   optimisations rely on `m_ptr == x.m_ptr` as a *fast path*, never as a
   correctness condition. Maps keyed on types use the structural `<`.
2. **`::HIR::TypeRef` is the single type IR.** Same type used by AST/HIR
   (with inference variables and generics), by typeck, by MIR
   (post-typeck), by monomorphisation (which rewrites instances), and by
   codegen. The "post-monomorph" property is a *contract on which variants
   may appear* (`Generic`, `Infer`, `ErasedType`, `NodeType`, raw `Slice`,
   raw `TraitObject` are illegal), enforced by `MIR_BUG(...)` in
   `emit_ctype`. There is no separate `TyMir` / `TySolved` layer.
3. **Lifetimes are kept on the type but excluded from equality.** Borrow
   and TraitObject carry `LifetimeRef`s for diagnostics/printing, but
   `operator==` and `ord` ignore them. This matches "post-region-erasure":
   the data is there for debug, not for semantics.
4. **Closures/generators/futures vanish into nominal Path types.** The
   `NodeType` variant exists only at the pre-monomorph layer (HIR/typeck).
   By the time codegen runs they are ordinary `Path` types whose final
   path component starts with `closure#`, `generator#`, or `future#`.
   `is_closure()` and `is_generator()` on `TypeData_Path` are substring
   tests. There is no per-closure trait-object representation in the
   type IR -- closures are just structs.
5. **Function items vs function pointers are distinct types.**
   `NamedFunction` (a fn item) and `Function` (a fn pointer) are separate
   variants. NamedFunctions are emitted as ZSTs in C
   (`typedef struct { ... } t_<mangled>;`), preserving the type-level
   distinction even though there is no runtime data.
6. **Layout is computed lazily, not stored in the type.** Sizes,
   alignments, field offsets, discriminant schemes all live on `TypeRepr`,
   which is built from `HIR::Struct`/`HIR::Union`/`HIR::Enum` definitions
   and cached. The `TypeRef` itself never knows its layout. The repr
   selector (Rust / C / Transparent / Simd / packed / aligned-N) lives on
   the *definition*, not on the type, because the same struct definition
   is referenced uniformly through `Path { binding: Struct(...) }`.
7. **No type-level interior-mutability marker.** `UnsafeCell` is detected
   by lang-item path identity inside
   `StaticTraitResolve::type_is_interior_mutable`, which recurses
   structurally. The compiler asks the question on demand; the answer is
   not memoised on the type. Same for `PhantomData`, `Box`, `non_zero`.
8. **No `MaybeUninit`, `volatile`, or atomic markers in the type IR.**
   `MaybeUninit<T>` is treated as a transparent wrapper struct (codegen
   does nothing special). `volatile` is never emitted on Rust-level types
   in C output (the only `__volatile__` uses are inside intrinsic
   helpers). Atomicity is a property of *operations* (intrinsic
   implementations and `<stdatomic.h>`), not of types; the target spec
   advertises which integer widths support atomics.
9. **DSTness is queried via `MetadataType`.** Five-valued enum
   `{ Unknown, None, Zero, Slice, TraitObject }`. Computed by
   `StaticTraitResolve::metadata_type` from the type structure. Used by
   `emit_ctype_ptr` to choose between thin pointer, `SLICE_PTR`, and
   `TRAITOBJ_PTR`. `Zero` is for `extern type` (unsized but no metadata).
   `Unknown` is treated as a bug at trans time, matching mrustc's
   "no defensive fallbacks" stance.
10. **C-name generation is mangling-driven for synthesised types.**
    Arrays, tuples, function pointers, fn-item types each get a typedef
    whose name is `t_<mangled>` / `TUP_<arity>_<mangled-fields>`. Nominal
    types get a `s_/u_/e_/x_` prefix and the mangled path. The structural
    contents of these typedefs are emitted separately, driven by
    `TypeRepr`. The pretty-printer for debug (`HIR::TypeRef::fmt`)
    produces Rust-flavoured text; the MMIR printer produces a more
    compact, mangling-friendly form intended for the textual MIR dump.
11. **Equality ignores `is_variadic` and `hrls` on function pointers.**
    This appears to be an oversight in `operator==`
    (`src/hir/type.cpp:510-522`) but does not affect the C output because
    differing variadic-ness produces a different mangled name and
    therefore a different typedef. Worth knowing if RegseqModel decides
    to base its identity on structural equality.
12. **Union `Repr` does not include `Simd`** (only `Rust`, `C`,
    `Transparent`); only `Struct::Repr` has `Simd`. Worth keeping in mind
    when modelling repr.
13. **Enum repr is split into two fields.** `Enum::m_is_c_repr` (bool)
    and `Enum::m_tag_repr` (`Auto`/`U8..U64`/`I8..I64`/`Usize`/`Isize`).
    The two combine to drive layout selection.
14. **`ArraySize::Unevaluated` is a `ConstGeneric`.** Until const-eval has
    run, an array size can be a generic-bound expression. After
    monomorphisation only `Known(uint64_t)` survives.
15. **`Diverge` (`!`) survives all the way to the C backend.** It is
    emitted as a placeholder typedef `tBANG`. Functions that "return `!`"
    use it as the return type. RegseqModel will need to decide whether to
    keep it or eliminate it earlier.
16. **`InferClass` is part of the `Infer` variant**, not a separate
    metadata field. It tags integer/float literal type variables for
    fallback resolution. Erased before MIR.
17. **Refcount-based shallow-COW is the editing model.**
    `TypeRef::get_unique()` returns `m_data` after, if needed, replacing
    `*this` with `clone_shallow()`. Mutators in the type-checker rely on
    this; readers may share. RegseqModel does not necessarily need this
    if its types are constructed once and never mutated post-bridge.
18. **No serialisable canonical form.** mrustc has no on-disk type IR --
    the type system exists only in process memory. The textual MMIR dump
    is for debugging, not round-tripping.
