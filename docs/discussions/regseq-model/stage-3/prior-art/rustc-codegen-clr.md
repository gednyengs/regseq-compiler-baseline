# Prior Art: rustc_codegen_clr (FractalFir)

Primary-source notes for stage-3 (`Ty` enum). All quotes are verbatim from the
listed files at the URLs given. The repository was inspected on 2026-04-28
against the `main` branch.

Repository:
  https://github.com/FractalFir/rustc_codegen_clr

Workspace layout (root `Cargo.toml`):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/Cargo.toml

```
[workspace]
members = [ "cilly", "dotnet_aot",
    "mycorrhiza", "rustc_codegen_clr_call", "rustc_codegen_clr_ctx",
    "rustc_codegen_clr_place", "rustc_codegen_clr_type",
    "rustc_codgen_clr_operand", ]
```

Crate roles (deduced from contents):

- `cilly`: the IR. Contains the type vocabulary (`Type`), CIL operation
  tree (`CILNode`, `CILRoot`), class definitions (`ClassDef`/`ClassRef`),
  the `Assembly` interner, and four printers/exporters: `c_exporter`,
  `il_exporter` (.NET ILASM), `java_exporter`, `cillyir_exporter`.
- `rustc_codegen_clr_type`: rustc-MIR -> cilly type translation
  (Ty -> ClassDef/ClassRef + explicit offsets).
- `rustc_codegen_clr_ctx`, `rustc_codegen_clr_place`,
  `rustc_codgen_clr_operand`, `rustc_codegen_clr_call`: per-MIR-element
  helpers feeding into cilly.
- `mycorrhiza`: .NET interop bindings (System.* class refs etc.).
- `dotnet_aot`: AOT-related glue.
- Top-level `src/` is the rustc backend plugin entry.

The IR-relevant files (everything under `cilly/src/v2/`) are the primary
focus, since `cilly` re-exports `pub use crate::v2::*;` from its
`lib.rs`:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/lib.rs

Note: the older `cilly/src/{cil_node.rs,cil_root.rs,cil_tree.rs,...}`
files (`V1Node`, `V1Root`) are still in tree but the `v2` module is
canonical; `cilroot.rs` carries large `From<V1Root> for CILRoot` blocks
that translate the legacy IR into v2. The duplicate `src/type/type.rs`
in the legacy crate is empty (1 byte). All current type-IR work is in
`cilly`.

---

## A. Type-vocabulary completeness

The canonical type enum lives at:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/tpe/mod.rs

```
#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Type {
    Ptr(Interned<Type>),
    Ref(Interned<Type>),
    Int(Int),
    ClassRef(Interned<ClassRef>),
    Float(Float),
    PlatformString,
    PlatformChar,
    PlatformGeneric(u32, GenericKind),
    PlatformObject,
    Bool,
    Void,
    PlatformArray {
        elem: Interned<Type>,
        dims: NonZeroU8,
    },
    FnPtr(Interned<FnSig>),
    SIMDVector(SIMDVector),
}
#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum GenericKind {
    MethodGeneric,
    CallGeneric,
    TypeGeneric,
}
```

`Int` and `Float` are flat enums (no payload):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/tpe/int.rs
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/tpe/float.rs

```
pub enum Int {
    U8, U16, U32, U64, U128, USize,
    I8, I16, I32, I64, I128, ISize,
}

pub enum Float {
    F16, F32, F64, F128,
}
```

Observations:

- The vocabulary is CIL-flavored, not Rust-flavored. There is no
  `Type::Slice`, `Type::Str`, `Type::Tuple`, `Type::Adt`, `Type::Enum`
  variant. All composite Rust types collapse onto `ClassRef` (a
  reference to a `ClassDef`). Slice/str/dynamic become a `ClassRef` to
  a synthesized "FatPtr<T>" / "Dyn" class (see theme B).
- `Ptr` and `Ref` are distinct. The contained type is interned. `Ref`
  is the .NET managed byref (`&` in CIL); `Ptr` is the unmanaged
  pointer (`*` in CIL). The C printer renders both as `T*`
  (theme G).
- `PlatformString`, `PlatformChar`, `PlatformObject`,
  `PlatformGeneric(u32, GenericKind)`, `PlatformArray { elem, dims }`
  are .NET-isms. They are present in the IR even though the C printer
  collapses several of them (`PlatformString -> char*`,
  `PlatformObject -> void*`, `PlatformArray -> elem**...*`) and panics
  / `todo!()`s on `PlatformGeneric`.
- `FnPtr(Interned<FnSig>)` carries an interned signature; `FnSig` is
  `{ inputs: Box<[Type]>, output: Type }`, no calling convention or
  varargs marker:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/fnsig.rs
- `SIMDVector` is a first-class variant.
- There is no `Never` and no `Bottom`. Rust `!`, `()` (unit),
  `FnDef`, `Foreign`, and `Bound` all map to `Type::Void` in the
  rustc->cilly translator (`rustc_codegen_clr_type/src/type.rs`,
  `get_type`):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/rustc_codegen_clr_type/src/type.rs

---

## B. Layout and representation

Layout is committed in the IR via the `ClassDef` struct in
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/class.rs

```
pub struct ClassDef {
    name: Interned<IString>,
    is_valuetype: bool,
    generics: u32,
    extends: Option<Interned<ClassRef>>,
    fields: Vec<(Type, Interned<IString>, Option<u32>)>,
    static_fields: Vec<StaticFieldDef>,
    methods: Vec<MethodDefIdx>,
    access: Access,
    explict_size: Option<NonZeroU32>,
    align: Option<NonZeroU32>,
    has_nonveralpping_layout: bool,
}
```

The triple `(Type, Interned<IString>, Option<u32>)` is
`(field_type, field_name, optional_explicit_byte_offset)`. Combined
with `explict_size` and `align`, this is a fully explicit layout
descriptor: byte offset per field, total size, abi alignment,
"non-overlapping" flag (true = struct/tuple, false = enum/union).

Where layout gets baked in (rustc->cilly):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/rustc_codegen_clr_type/src/type.rs

`fat_ptr_to(...)` synthesizes a class for every fat-pointer pointee:

```
let def = ClassDef::new(
    name, true, 0, None,
    vec![
        (ctx.nptr(Type::Void),
         ctx.alloc_string(cilly::DATA_PTR),
         Some(0)),
        (Type::Int(Int::USize),
         ctx.alloc_string(cilly::METADATA),
         Some(8)),
    ],
    vec![],
    Access::Public,
    Some(NonZeroU32::new(16).unwrap()),
    Some(NonZeroU32::new(8).unwrap()),
    true,
);
```

So a Rust fat pointer becomes a `ClassRef` to a `FatPtr<elem>` value-type
struct with two named fields `d` (data, `*void`, offset 0) and `m`
(metadata, `usize`, offset 8), explicit size 16, alignment 8. The
field-name string constants are exported from the cilly crate root:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/lib.rs

```
pub const METADATA: &str = "m";
pub const DATA_PTR: &str = "d";
pub const ENUM_TAG: &str = "v";
```

Note: hard-coded offsets `0` and `8` and size `16`. This implicitly
assumes a 64-bit target.

Structs: `struct_(...)` (line 571 of `rustc_codegen_clr_type/src/type.rs`)
zips `adt.variant(0).fields` with `FieldOffsetIterator`
(`adt.rs::FieldOffsetIterator`) and lifts each rustc field offset into
`Some(offset)`. `explict_size` is `layout.size().bytes()` and `align`
is `layout.align().abi.bytes()`. `has_nonverlaping_layout` is
`unique_checks.len() == fields.len()`.

Enums: `enum_(...)` (line 685) lays out the discriminant via
`handle_tag` (which calls `enum_tag_info` to get tag type and offset,
inserts a synthetic field named `cilly::ENUM_TAG` ("v")), then for
every variant emits `(tpe, "<variant>_<field>", Some(offset))`. The
overlap of variant fields is *implicit* in the offsets (variant
fields share offsets with each other), and `has_nonveralpping_layout`
is set to `false`. There is no `enum-specific` IR node: an enum is a
`ClassDef` with overlapping non-disjoint offsets, plus a tag field.
This is exactly the "explicit-layout-in-IR" pattern: variant overlap
is encoded as offset collisions on the `(tpe, name, Some(offset))`
field list.

Unions: `union_(...)` (line 746) lays out all fields at their declared
offsets, with `has_nonveralpping_layout = false`.

Closures: `closure_typedef(...)` (line 515) - same shape, fields
labeled `f_0`, `f_1`, etc.

DSTs (str, [T]): in `get_type`, `TyKind::Str` returns `Type::Int(Int::U8)`
and `TyKind::Slice(inner)` returns `get_type(inner, ctx)` (the element
type). The enclosing pointer/reference is checked with
`pointer_to_is_fat`, and a fat-pointer class is synthesized via
`fat_ptr_to` whenever the pointee is unsized. So slice/str types only
appear in the IR as the `elem` parameter to a `FatPtr<elem>` ClassDef.

`dyn Trait`: `TyKind::Dynamic` returns a `ClassRef` to a class named
`"Dyn"`, with no fields (lines 132-150 of `rustc_codegen_clr_type/src/type.rs`).

`ClassDef::layout_check(...)` cross-checks: if a class has overlapping
fields, none of those fields may be GC-references:

```
pub fn layout_check(&self, asm: &Assembly) -> Result<(), LayoutError> {
    if !self.has_nonveralpping_layout() {
        for (t, name, _offset) in self.fields() {
            if t.is_gcref(asm) {
                return Err(LayoutError::ManagedRefInOverlapingField { ... });
            }
        }
    }
    Ok(())
}
```

This is the only post-construction layout invariant we found.

---

## C. Identity, interning, canonical form

Interning is done by `BiMap`:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/bimap.rs

```
pub struct BiMap<Value: Eq + Hash>(pub Vec<Value>, pub FxHashMap<Value, Interned<Value>>);
```

The forward direction is `FxHashMap<Value, Interned<Value>>`; the
reverse is the indexed `Vec<Value>`. `Interned<T>` is just a
`NonZeroU32` newtype with `PhantomData<T>`:

```
pub struct Interned<T: ?Sized> {
    pd: PhantomData<T>,
    idx: BiMapIndex,  // NonZeroU32
}
```

`BiMap::alloc(val)` is the dedup point: it does an `Entry::Occupied` /
`Entry::Vacant` lookup keyed on `val`, and only pushes when vacant.
Equality is structural (`derive(Hash, PartialEq, Eq)` on every IR
type).

`Assembly` holds one `BiMap` per IR kind:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/asm.rs

```
pub struct Assembly {
    strings: StringMap,                 // BiMap<IString>
    types: TypeMap,                     // BiMap<Type>
    class_refs: BiMap<ClassRef>,
    class_defs: FxHashMap<ClassDefIdx, ClassDef>,
    nodes: BiMap<CILNode>,
    roots: BiMap<CILRoot>,
    sigs: BiMap<FnSig>,
    method_refs: BiMap<MethodRef>,
    fields: BiMap<FieldDesc>,
    statics: BiMap<StaticFieldDesc>,
    method_defs: FxHashMap<MethodDefIdx, MethodDef>,
    sections: FxHashMap<String, Vec<u8>>,
    pub(crate) const_data: BiMap<Box<[u8]>>,
}
```

Important: `class_defs` is a *plain* `FxHashMap`, not a `BiMap`. The
class definition is keyed by `ClassDefIdx(Interned<ClassRef>)`. So the
identity of a class is its (interned) `ClassRef` (name + assembly +
is_valuetype + generics tuple), and the `ClassDef` is the body. There
is exactly one definition per class identity; `merge_defs` (line 610
of `class.rs`) reconciles repeat insertions, asserting matching
`name`, `is_valuetype`, `generics`, `extends`, `access`.

Canonical form for `Type`:

- Two pointers/refs/fnptrs are equal iff their interned operands are
  equal. Because `Type` is `Copy` and 16 bytes-ish, the variants
  themselves hash structurally; equality of `Interned<Type>` is just
  index equality. So the IR has *both* structural equality (on
  `Type`) and pointer-equal-by-index (on `Interned<Type>`).
- `Type::PlatformString` is *not* canonicalized to a particular
  `ClassRef`; the IR keeps it as a distinct variant. The
  `is_assignable_to` rules in `tpe/mod.rs` define a small subtype
  lattice mostly between platform aliases and integer classes
  (`System.Int128 <-> Type::Int(Int::I128)`, etc.).

`Type::mangle(&Assembly)` produces a deterministic ASCII tag per type
(used to synthesize method/class names). It is NOT a hash; it is the
public canonical string form. Quoted from `tpe/mod.rs`:

```
assert_eq!(Type::PlatformString.mangle(&asm),"st");
assert_eq!(Type::Int(Int::I128).mangle(&asm),"i128");
```

---

## D. Special-semantics markers (UnsafeCell, atomics, volatile)

`UnsafeCell`: there is no special handling. A grep of `cilly/src/v2/*`
and `rustc_codegen_clr_type/src/*` returns no occurrences of
`UnsafeCell`. The Rust `UnsafeCell<T>` is just a `TyKind::Adt` and
flows through `struct_(...)` like any other one-field struct. The IR
has no "interior mutability" marker.

`volatile`: not a type marker; it is a per-load and per-store flag on
the operation node. From
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/cilnode.rs

```
LdInd {
    addr: Interned<CILNode>,
    tpe: Interned<Type>,
    volatile: bool,
},
```

and from
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/cilroot.rs

```
/// addr, value, type
StInd(Box<(Interned<CILNode>, Interned<CILNode>, Type, bool)>),
```

(Tuple field 3 is the `is_volatile` flag; see the `Display` impl at
line 691 of `cilroot.rs` which prints `is_volitile:{is_volitile}`.)

`atomics`: not a type variant and not an operation flag - they are
*lowered to method calls* in the IR. The atomic builtin library is at
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/builtins/atomics.rs

It registers methods with names like `atomic_xchng_u8`,
`atomic_cmpxchng16_i32`, `cmpxchng8`, `cmpxchng16`. An atomic load is
represented as a normal `LdInd { volatile: true, .. }` plus a runtime
fence/method call; an atomic xchg becomes a method body that does the
load-store pair. On the .NET side, `compare_exchange` lowers to an
`Interlocked.CompareExchange` call (see `class.rs::ClassRef::interlocked`
plus the atomic builtins' codegen). On the C side, the C header
preamble wires those names to gcc/clang `__atomic_*` builtins:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/c_exporter/c_header.h

```
#define __atomic_compare_exchange_4 __atomic_compare_exchange_n
#define __atomic_compare_exchange_8 __atomic_compare_exchange_n
```

with software fallbacks for non-multithreaded compilers (SDCC, LCC).

Net summary of theme D: `cilly` does not bake any "special semantics"
into `Type`. Volatility is a per-op bit; atomicity is a method-call
convention plus runtime helpers; `UnsafeCell` is invisible to the IR.

---

## E. Post-monomorphization erasure

Yes - inherited from rustc. Entry point is `get_type(ty, ctx)` which
calls `ctx.monomorphize(ty)` first:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/rustc_codegen_clr_type/src/type.rs

```
pub fn get_type<'tcx>(ty: Ty<'tcx>, ctx: &mut MethodCompileCtx<'tcx, '_>) -> Type {
    let ty = ctx.monomorphize(ty);
    if is_zst(ty, ctx.tcx()) {
        return Type::Void;
    }
    match ty.kind() {
        TyKind::Bound(_, _inner) => Type::Void,
        TyKind::Bool => Type::Bool,
        TyKind::Char => Type::Int(Int::U32),
        TyKind::Closure(def, args) => { ... }
        TyKind::Dynamic(_list, _) => { ... }
        TyKind::Float(float) => from_float(float),
        TyKind::Foreign(_foregin) => Type::Void,
        TyKind::FnDef(_did, _subst) => Type::Void,
        ...
    }
}
```

Notable consequences:
- ZSTs erase to `Type::Void`; `FnDef` / `Foreign` / `Never` / `Bound`
  also erase to `Type::Void`.
- `Type::Void` becomes a load-bearing "absent" marker. The C printer
  has a `nonvoid_c_type` helper that maps it to a synthesized
  `RustVoid` opaque struct when used as a field type, because C does
  not allow void fields.
- The `PlatformGeneric(u32, GenericKind)` variant exists, but in the
  rustc->cilly translator it is *never produced for Rust generics*
  - those have all been monomorphized away. `PlatformGeneric` is
  reserved for cases where cilly synthesizes generic .NET method/type
  refs (e.g., `Vector{N}<T>` SIMD intrinsics in il_exporter; line 1363
  of `cilly/src/v2/il_exporter/mod.rs`).
- `ClassDef::generics: u32` is the *count* of generic parameters - a
  vestigial CIL feature for the .NET backend, irrelevant to C, and
  always 0 in monomorphized Rust output.

---

## F. Bridge / consumption shape (dual printer architecture)

`cilly/src/v2/mod.rs` declares parallel exporter modules:

```
/// Code exporting C source files
pub mod c_exporter;
/// Exports modules to IR builders. Used for quickly implementing intrinsics
pub mod cillyir_exporter;
...
/// Exports IR to .NET bytecode
pub mod il_exporter;
/// Exports IR to JVM bytecode
pub mod java_exporter;
```

and a single `Exporter` trait:

```
pub trait Exporter {
    type Error: std::fmt::Debug;
    fn export(&mut self, asm: &Assembly, target: &Path) -> Result<(), Self::Error>;
}
```

Each exporter takes the same `&Assembly` (the entire IR) and writes
target-specific output. The IR is *frozen* at this point - layout,
class definitions, atomic-method names, etc. are already committed.

The author's stated rationale (README.md, "C support" section,
verbatim):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/README.md

> While .NET is the main focus of my work, this project can also be
> used to compile Rust to C, by setting the C_MODE enviroment flag to
> 1. This may seem like a strange and unrelated feature, but the
> project was written in such a way that this is not only possible,
> but relatively easy. My representation of .NETs IR maps nicely to
> C, which means that I was able to add support for compiling Rust to
> C in 2-3K LOC. Almost all of the codebase is reused, with the C and
> .NET specific code only present in the very last stage of
> compilation. This means that, instead of having to maintain 2
> separate projects, I can maintain one project. Bug fixes to the
> .NET side of things also fix C bugs.

And from `src/lib.rs`:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/src/lib.rs

> The project first translates MIR to CIL in a very precise, but
> inefficient fasion. This is a deliberate choice - it recduces the
> chance of bugs, and enables easy checking of the resulting CIL.
> Since any given MIR statement will always result in the same ops,
> and the ops from each statement are kept separate, any misformed
> piece of CIL byecode can be easily traced back to a particular MIR
> statement.

So the design contract is:

1. `MIR -> cilly v2 IR` is a single, target-agnostic lowering done by
   `rustc_codegen_clr_type` / `rustc_codgen_clr_operand` / etc.
2. cilly v2 IR carries *enough* information (explicit layout, fat
   pointer struct names, enum tags as ordinary fields, atomic method
   names as ordinary method calls) to be printed by *any* exporter
   without re-running rustc.
3. Each exporter is a "dumb" printer plus a fixed runtime preamble
   (e.g., `c_header.h`, the `MissingMethodPatcher` for atomic helpers).

---

## G. Renderer / pretty-printer format (CIL printer + C printer)

C type printer (`cilly/src/v2/c_exporter/utilis.rs`,
`fn c_tpe(field_tpe: Type, asm: &Assembly) -> String`):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/c_exporter/utilis.rs

```
pub(super) fn c_tpe(field_tpe: Type, asm: &Assembly) -> String {
    match field_tpe {
        Type::Ptr(type_idx) | Type::Ref(type_idx) => format!("{}*", c_tpe(asm[type_idx], asm)),
        Type::Int(int) => match int {
            Int::U8 => "uint8_t".into(),
            ...
            Int::U128 => "__uint128_t".into(),
            Int::USize => "uintptr_t".into(),
            ...
            Int::I128 => "__int128".into(),
            Int::ISize => "intptr_t".into(),
        },
        Type::ClassRef(class_ref_idx) => {
            if asm.class_ref_to_def(class_ref_idx).is_some_and(|def| {
                asm[def].has_nonveralpping_layout() && asm[def].explict_size().is_some()
            }) {
                format!("struct {}", escape_ident(&asm[asm[class_ref_idx].name()]))
            } else {
                format!("union {}", escape_ident(&asm[asm[class_ref_idx].name()]))
            }
        },
        Type::Float(float) => match float {
            Float::F16 => "_Float16".into(),
            Float::F32 => "float".into(),
            Float::F64 => "double".into(),
            Float::F128 => "_Float128".into(),
        },
        Type::PlatformString => "char*".into(),
        Type::PlatformChar => "char".into(),
        Type::PlatformGeneric(_, generic_kind) => todo!(),
        Type::PlatformObject => "void*".into(),
        Type::Bool => "bool".into(),
        Type::Void => "void".into(),
        Type::PlatformArray { elem, dims } => format!(
            "{elem}{dims}",
            elem = c_tpe(asm[elem], asm),
            dims = "*".repeat(dims.get() as usize)
        ),
        Type::FnPtr(_) => "void*".into(),
        Type::SIMDVector(vec) => {
            format!("__simdvec{elem}_{count}",
                elem = std::convert::Into::<Type>::into(vec.elem()).mangle(asm),
                count = vec.count())
        }
    }
}
```

Decisions worth noting:

- `ClassRef` with non-overlapping layout AND explicit size becomes
  `struct <name>`; otherwise `union <name>`. This means rustc enums
  *always* print as `union`, and rustc structs/tuples print as `struct`.
- `Type::FnPtr(_)` collapses to `void*` in C - the C printer drops
  the signature when used at type position. (Function pointers carry
  signature only at call sites.)
- `Type::Ref` and `Type::Ptr` are indistinguishable in C output. The
  `&T` vs `*T` distinction was useful in CIL but is erased in C.
- `Type::Void` requires special handling at field position: see
  `nonvoid_c_type`:

```
pub(super) fn nonvoid_c_type(field_tpe: Type, asm: &Assembly) -> String {
    match field_tpe {
        Type::Void => "RustVoid".into(),
        _ => c_tpe(field_tpe, asm),
    }
}
```

Class layout printer (C, `cilly/src/v2/c_exporter/mod.rs::export_class`,
line 1234):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/c_exporter/mod.rs

For non-overlapping classes (struct, tuple, closure):

```
writeln!(type_defs, "typedef struct {class_name}{{")?;
let mut fields = class.fields().to_vec();
fields.sort_by(|(_, _, a_offset), (_, _, b_offset)| {
    a_offset.unwrap().cmp(&b_offset.unwrap())
});
let mut last_offset = 0;
let mut pad_count = 0;
for (field_tpe, fname, offset) in &fields {
    ...
    if offset != last_offset {
        writeln!(type_defs, "uint8_t pad_{pad_count}[{}];\n", offset - last_offset)?;
        pad_count += 1;
    }
    last_offset = offset + asm.sizeof_type(*field_tpe);
    let field_tpe = c_tpe(*field_tpe, asm);
    writeln!(type_defs, "{field_tpe} {fname};")?;
}
if last_offset != class.explict_size().unwrap().get() {
    let size = class.explict_size().unwrap().get();
    ...
    writeln!(type_defs, "uint8_t pad_{pad_count}[{}];\n", size - last_offset)?;
}
writeln!(type_defs, "}} {class_name};")?;
```

So the printer materializes explicit padding bytes from offset gaps,
and a tail pad to match `explict_size`. There is *no* reliance on the
C compiler's struct layout - everything is byte-exact.

For overlapping classes (enum, union):

```
writeln!(type_defs, "typedef union {class_name}{{")?;
for (field_tpe, fname, offset) in class.fields() {
    let Some(offset) = offset else { ... continue };
    let field_tpe = c_tpe(*field_tpe, asm);
    let pad = if *offset != 0 {
        format!("char pad[{offset}];")
    } else { "".into() };
    writeln!(type_defs, "struct {{{pad} {field_tpe} f;}}{fname};")?;
}
if let Some(size) = class.explict_size() {
    writeln!(type_defs, "char force_size[{size}];", size = size.get())?;
}
if class.fields().is_empty() {
    writeln!(type_defs, "FORCE_NOT_ZST")?;
}
writeln!(type_defs, "}} {class_name};")?;
```

So an enum is rendered as a `union` of one anonymous-`struct`
per-field, where each struct has `char pad[offset]; FieldType f;`.
The `FORCE_NOT_ZST` macro is from
`cilly/src/v2/c_exporter/c_header.h`:

```
#if !(defined(__SDCC) || defined(_MSC_VER) || defined(__COMPCERT__) || defined(__LCC__))
#define FORCE_NOT_ZST 
#else
#define FORCE_NOT_ZST char force_not_zst;
#endif
```

CIL type printer (`cilly/src/v2/il_exporter/mod.rs::type_il`):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/il_exporter/mod.rs

```
fn type_il(tpe: &Type, asm: &Assembly) -> String {
    match tpe {
        Type::Ptr(inner) => format!("{}*", type_il(&asm[*inner], asm)),
        Type::Ref(inner) => format!("{}&", type_il(&asm[*inner], asm)),
        Type::Int(int) => match int {
            super::Int::U8 => "uint8".into(),
            ...
            super::Int::U128 => "valuetype [System.Runtime]System.UInt128".into(),
            super::Int::USize => "native uint".into(),
            ...
        },
        Type::ClassRef(cref) => class_ref(*cref, asm),
        Type::Float(float) => match float {
            super::Float::F32 => "float32".into(),
            super::Float::F64 => "float64".into(),
            ...
        },
        ...
        Type::PlatformString => "string".into(),
        Type::PlatformObject => "object".into(),
    }
}
```

CIL class layout printer:

```
let explicit = if class_def.has_explicit_layout() {
    "explicit"
} else {
    "auto"
};
...
writeln!(out, ".class {vis} ansi {sealed} {explicit} '{name}' extends {extends}{{")?;
if let Some(size) = class_def.explict_size() {
    writeln!(out, ".size {size}", size = size.get())?;
}
for (tpe, name, offset) in class_def.fields() {
    if let Some(offset) = offset {
        writeln!(out, ".field [{offset}] {tpe} '{name}'")
    } else {
        writeln!(out, ".field {tpe} '{name}'")
    }?;
}
```

So the CIL printer relies on the .NET runtime to honor explicit
`[offset]` field placement, while the C printer hand-rolls padding.
The same `(Type, name, Some(offset))` tuples drive both. The
"non-overlapping flag" (`has_nonveralpping_layout`) is *only*
consulted by the C printer for choosing struct-vs-union; the IL
printer instead asks `has_explicit_layout()` (returns true if any
field has a `Some(offset)` or if `explict_size().is_some()`).

---

## Watch-outs and observations

1. **Single-target-shape IR.** The `Type` enum is target-neutral *in
   the sense that* both printers consume it, but it is not
   target-neutral in design: the platform vocabulary
   (`PlatformString`, `PlatformChar`, `PlatformObject`,
   `PlatformGeneric`, `PlatformArray`) is a CIL-shaped concept that
   the C printer has to lossily map. `PlatformGeneric` literally
   `todo!()`s in C output. For our Linux/Zephyr split this is
   meaningless (both targets are C), but the shape is a warning that
   "one IR, two printers" only works when the two targets share an
   ABI shape - and even then, one printer can be a
   second-class citizen.

2. **Hard-coded 64-bit assumptions.** `fat_ptr_to` writes literal
   offsets `Some(0)`, `Some(8)`, `explict_size = 16`, `align = 8`.
   The IR captures *concrete* layout decisions, not parametric ones.
   This is a deliberate consequence of "explicit layout in IR": once
   you commit to absolute byte offsets, you have committed to a target
   pointer width. Re-targeting requires re-running the rustc-side
   layout pass.

3. **Layout commitment cost: target ABI is not negotiable post-IR.**
   Because every `ClassDef` field carries `Some(offset)` and the
   class carries `explict_size` and `align` from
   `layout.layout.size().bytes()` and `layout.layout.align().abi.bytes()`,
   the IR is parameterized by a single rustc `LayoutComputation` run.
   Two implications for any project considering this pattern:
   - You cannot re-target the same IR snapshot to a different ABI
     (e.g., 32-bit to 64-bit, or a different alignment regime). You
     would have to re-run the rustc pipeline.
   - Custom-ABI reasoning (packed structs, custom alignment, MMIO
     layouts) must already be reflected in the rustc-computed layout
     before the IR is produced. There is no "let the printer decide"
     escape hatch.

4. **No special-semantics in the type system.** UnsafeCell is
   invisible. Volatile is a per-load/per-store boolean, not a type
   bit. Atomics are method-call patterns matched by name. This is a
   pragmatic, lightweight design for a backend that trusts rustc to
   have already enforced the safety properties at the MIR level.
   Anyone needing to *reason* over types post-IR (e.g., to decide
   whether a struct contains atomic fields, whether a region is
   shared-mutable, whether a register is volatile-as-attribute) would
   have to thread that information through some other channel.

5. **Identity is structural-by-interning.** The `BiMap` makes every
   structurally identical `Type`/`ClassRef`/`FnSig` a single
   `Interned<...>` index. This collapses two questions (structural
   equality and pointer identity) into one. There is no separate
   "definition path" key for types - a class is identified by
   `(name: Interned<IString>, asm: Option<Interned<IString>>, is_valuetype: bool, generics: Box<[Type]>)`,
   so two types with the same mangled name from different rustc
   `DefId`s that happen to collide would alias. In practice rustc's
   symbol mangling avoids that. For us, this means relying on a
   carefully chosen name-mangling scheme to keep identity correct.

6. **Erasure is total.** ZSTs, `FnDef`, `Foreign`, `Never`, and
   higher-kinded `Bound` types all erase to `Type::Void`. This makes
   `Type::Void` overloaded: it means "C void return", "Rust unit",
   "ZST struct", "absent argument". The downstream printers then have
   to special-case it (`nonvoid_c_type` -> `RustVoid` synthetic
   struct). This is a tax of total erasure; an alternative is to keep
   distinct `Unit` / `Never` / `ZST` markers, at the cost of a wider
   match in every printer.

7. **The dual-printer pattern's portability to Linux/Zephyr.**
   Strongly transferable. The pattern works because:
   (a) the IR is post-monomorphization (no generic noise),
   (b) layout is already committed (printers don't disagree on
       offsets),
   (c) every "abstract" rustc concept that doesn't have a clean C/CIL
       equivalent (atomics, fat pointers, dyn, closures) has been
       compiled down to a *named* `ClassDef` plus a *named* method,
       which both printers can render.
   For Linux/Zephyr both targets are C, so the divergence is *much*
   smaller than CLR vs C: it's mostly preamble text (which Linux
   kernel vs Zephyr headers to include), per-target intrinsic name
   mappings (e.g., `READ_ONCE` vs whatever Zephyr's MMIO accessor is),
   and per-target name-mangling rules. None of those touch the
   `Type` enum. The genuine risk is the *opposite* of what FractalFir
   faces: because both our targets are so close, we may be tempted to
   skip cleaving printer-only concerns out of the IR, and end up with
   target-specific knobs leaking into `Ty`. The discipline cilly
   shows ("everything that can be a method call should be a method
   call; everything that can be a `ClassDef` should be a `ClassDef`")
   is the more important takeaway than the dual-printer plumbing
   itself.

8. **Cost of explicit-layout-in-IR.**
   Concrete losses of flexibility:
   - The IR is bound to one ABI (theme E).
   - Layout bugs surface as IR-corruption, not as printer-mismatch.
     The `assert!(offset >= last_offset, ...)` in `export_class` and
     the `layout_check` for managed-ref-in-overlapping-field are the
     only safety nets; everything else trusts the offsets to be
     correct.
   - There is no opportunity to defer layout decisions to a target
     that has different padding rules (e.g., `#pragma pack`, kernel
     `__attribute__((packed))` structures). For us that's likely
     fine, but for any IR that aspires to be linkable into C
     codebases with mixed packing, this is a hard constraint.
   Concrete wins:
   - Two printers stay in lockstep automatically.
   - Layout-dependent optimizations (field reordering, niche
     placement) can run on the IR exactly once.
   - ABI bugs are localized to one rustc->IR layout pass instead of
     being scattered across N target backends.

9. **Class-ref vs class-def asymmetry.** `ClassRef` is interned
   (`BiMap<ClassRef>`); `ClassDef` is keyed by its `ClassDefIdx`
   (which wraps an `Interned<ClassRef>`). The implication is that you
   can refer to a class by `Type::ClassRef(...)` long before its
   `ClassDef` is materialized, and `class_ref_to_def(...)` returns
   `Option`. This separation supports forward-references during
   construction (essential for cyclic types) but means downstream
   passes must handle the `None` case. In our setting, where we may
   want to reason about pointee types, this asymmetry is worth
   adopting.

10. **Pretty-printing is monolithic.** The C printer is one ~1700
    line `mod.rs`, the IL printer is ~1500. There is no separate
    `Display`/`Debug` impl on `Type` for pretty-printing - the
    printer functions are free-standing. `Type::mangle` is the only
    "always-available" textual form, and it is for symbol names not
    human reading. If we want a debug pretty-printer separate from
    the target printers, we will have to write it.

---

## File map (primary sources used)

- Workspace root and crate layout:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/Cargo.toml
- README (design rationale):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/README.md
- Architectural rationale on rustc side:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/src/lib.rs
- cilly crate root, common constants:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/lib.rs
- cilly v2 module declarations + Exporter trait:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/mod.rs
- Type enum, GenericKind, mangle, is_assignable_to:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/tpe/mod.rs
- Int / Float enums:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/tpe/int.rs
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/tpe/float.rs
- ClassDef / ClassRef, layout fields:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/class.rs
- Interning (BiMap, Interned):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/bimap.rs
- Assembly (the IR container):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/asm.rs
- FnSig:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/fnsig.rs
- CILNode, LdInd { volatile }:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/cilnode.rs
- CILRoot, StInd volatile bit:
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/cilroot.rs
- Atomic builtins (lowered to method calls):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/builtins/atomics.rs
- C exporter (top-level, `export_class`):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/c_exporter/mod.rs
- C type printer (`c_tpe`, `nonvoid_c_type`):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/c_exporter/utilis.rs
- C runtime preamble (atomics, FORCE_NOT_ZST, _Thread_local):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/c_exporter/c_header.h
- IL exporter (CIL printer):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/cilly/src/v2/il_exporter/mod.rs
- rustc -> cilly type translator (struct_, enum_, union_, fat_ptr_to,
  closure_typedef, get_type):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/rustc_codegen_clr_type/src/type.rs
- ADT layout helpers (FieldOffsetIterator, enum_tag_info,
  enum_variant_offsets):
  https://github.com/FractalFir/rustc_codegen_clr/blob/main/rustc_codegen_clr_type/src/adt.rs

Author retrospective (referenced for context, not for verbatim
quotes): https://fractalfir.github.io/generated_html/rustc_codegen_clr_v0_1_0.html
The retrospective does motivate the explicit-layout choice for enums
("Without the ability to make the data of different variants overlap,
we lose almost all the advantages of a Rust enum.") but does not go
into IR-level detail.
