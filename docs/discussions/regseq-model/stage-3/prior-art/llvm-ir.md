# Prior Art: LLVM IR Type System

Primary-source research for stage-3 of the RegseqModel `Ty` enum. All
material below was read directly from the LLVM `main` branch headers,
implementation files, and rendered LangRef. Citations point to the
canonical paths so future readers can re-anchor against a fixed commit.

## Sources

- `llvm/include/llvm/IR/Type.h`
  https://raw.githubusercontent.com/llvm/llvm-project/main/llvm/include/llvm/IR/Type.h
- `llvm/include/llvm/IR/DerivedTypes.h`
  https://raw.githubusercontent.com/llvm/llvm-project/main/llvm/include/llvm/IR/DerivedTypes.h
- `llvm/include/llvm/IR/DataLayout.h`
  https://raw.githubusercontent.com/llvm/llvm-project/main/llvm/include/llvm/IR/DataLayout.h
- `llvm/lib/IR/DataLayout.cpp`
  https://raw.githubusercontent.com/llvm/llvm-project/main/llvm/lib/IR/DataLayout.cpp
- `llvm/include/llvm/IR/LLVMContext.h`
  https://raw.githubusercontent.com/llvm/llvm-project/main/llvm/include/llvm/IR/LLVMContext.h
- `llvm/lib/IR/LLVMContextImpl.h`
  https://raw.githubusercontent.com/llvm/llvm-project/main/llvm/lib/IR/LLVMContextImpl.h
- `llvm/lib/IR/Type.cpp`
  https://raw.githubusercontent.com/llvm/llvm-project/main/llvm/lib/IR/Type.cpp
- `llvm/docs/OpaquePointers.rst`
  https://raw.githubusercontent.com/llvm/llvm-project/main/llvm/docs/OpaquePointers.rst
- `llvm/docs/LangRef.rst` (rendered: https://llvm.org/docs/LangRef.html)


## A. Type-vocabulary completeness -- the full Type hierarchy

LLVM's authoritative variant set is the `enum TypeID` inside
`Type.h`. Quoted verbatim (Type.h, lines roughly 47-72):

```cpp
enum TypeID {
  // PrimitiveTypes
  HalfTyID = 0,        ///< 16-bit floating point type
  BFloatTyID,          ///< 16-bit floating point type (7-bit significand)
  FloatTyID,           ///< 32-bit floating point type
  DoubleTyID,          ///< 64-bit floating point type
  X86_FP80TyID,        ///< 80-bit floating point type (X87)
  FP128TyID,           ///< 128-bit floating point type (112-bit significand)
  PPC_FP128TyID,       ///< 128-bit floating point type (two 64-bits, PowerPC)
  VoidTyID,            ///< type with no size
  LabelTyID,           ///< Labels
  MetadataTyID,        ///< Metadata
  X86_AMXTyID,         ///< AMX vectors (8192 bits, X86 specific)
  TokenTyID,           ///< Tokens
  // Derived types... see DerivedTypes.h file.
  IntegerTyID,         ///< Arbitrary bit width integers
  ByteTyID,            ///< Arbitrary bit width bytes
  FunctionTyID,        ///< Functions
  PointerTyID,         ///< Pointers
  StructTyID,          ///< Structures
  ArrayTyID,           ///< Arrays
  FixedVectorTyID,     ///< Fixed width SIMD vector type
  ScalableVectorTyID,  ///< Scalable SIMD vector type
  TypedPointerTyID,    ///< Typed pointer used by some GPU targets
  TargetExtTyID,       ///< Target extension type
};
```

Concrete C++ class hierarchy (DerivedTypes.h):

- `Type` (base)
  - `IntegerType`        -- arbitrary-width integer (`IntegerType::get(C, NumBits)`)
  - `ByteType`           -- arbitrary-width "byte" (provenance-carrying integer; same shape as IntegerType but distinct ID)
  - `FunctionType`       -- return type + parameter types + isVarArg flag
  - `StructType`         -- literal anonymous OR identified named/opaque
  - `ArrayType`          -- element type + uint64_t element count
  - `PointerType`        -- *opaque*; carries only an address-space number (post LLVM-15)
  - `VectorType` (abstract)
    - `FixedVectorType`     -- exact element count
    - `ScalableVectorType`  -- "vscale x N"
  - `TargetExtType`      -- target-defined opaque/parametric type (e.g., `target("spirv.Image", void, i32 1)`)
  - `TypedPointerType`   -- typed pointer retained for "some GPU targets"
- Singleton primitive `Type` instances (no derived class):
  - `VoidTy`, `LabelTy`, `HalfTy`, `BFloatTy`, `FloatTy`, `DoubleTy`,
    `X86_FP80Ty`, `FP128Ty`, `PPC_FP128Ty`, `X86_AMXTy`, `MetadataTy`,
    `TokenTy`.

Categorizations (Type.h):

- `isFirstClassType()` -- "valid type for a Value." Implementation
  (Type.cpp):
  ```cpp
  bool Type::isFirstClassType() const {
    switch (getTypeID()) {
      default:           return true;
      case FunctionTyID:
      case VoidTyID:     return false;
      case StructTyID:   return !cast<StructType>(this)->isOpaque();
    }
  }
  ```
  Note that `label`, `token`, `metadata` ARE first-class even though they
  are not "single value" types in the codegen sense.
- `isSingleValueType()` -- comment in Type.h: "Return true if the type is
  a valid type for a register in codegen. This includes all first-class
  types except struct and array types."
- `isAggregateType()` -- comment in Type.h: "valid as the first operand
  of an insertvalue or extractvalue instruction. This includes struct
  and array types, but does not include vector types."

Key observation for RegseqModel: LLVM separates *type vocabulary*
(`TypeID`) from *role* (first-class / single-value / aggregate). The
roles are query methods on the same `Type`, not separate variants.


## B. Layout / representation -- DataLayout, target-specificity

LLVM keeps layout strictly out of the `Type` class. `Type::isSized` and
`Type::getPrimitiveSizeInBits` are the only size-related methods on
`Type`, and the comment on `isSized` is explicit (Type.h):

> Return true if it makes sense to take the size of this type. To get
> the actual size for a particular target, it is reasonable to use the
> DataLayout subsystem to do this.

`getPrimitiveSizeInBits` (Type.cpp) hard-codes only the primitives
(Half=16, Float=32, Double=64, etc.) plus IntegerType bitwidth and
vector shapes. Anything else returns `TypeSize::getFixed(0)` -- it
explicitly does not size aggregates or pointers. Those go through
`DataLayout`.

### DataLayout structure (DataLayout.h)

Header comment (line 8-14):

> This file defines layout properties related to datatype size/offset/
> alignment information. It uses lazy annotations to cache information
> about how structure types are laid out and used.

Primary fields (verbatim names, lines ~120-155):

```cpp
bool BigEndian;
bool VectorsAreElementAligned;
unsigned AllocaAddrSpace;
unsigned ProgramAddrSpace;
unsigned DefaultGlobalsAddrSpace;
MaybeAlign StackNaturalAlign;
MaybeAlign FunctionPtrAlign;
FunctionPtrAlignType TheFunctionPtrAlignType;
SmallVector<unsigned char, 8> LegalIntWidths;
SmallVector<PrimitiveSpec, 6>  IntSpecs;
SmallVector<PrimitiveSpec, 4>  FloatSpecs;
SmallVector<PrimitiveSpec, 10> VectorSpecs;
SmallVector<PointerSpec,   8>  PointerSpecs;
Align StructABIAlignment  = Align::Constant<1>();
Align StructPrefAlignment = Align::Constant<8>();
```

Public layout API (DataLayout.h, lines indicated):

```cpp
TypeSize getTypeSizeInBits(Type *Ty);          // line 596
TypeSize getTypeStoreSize(Type *Ty);           // line 608
TypeSize getTypeAllocSize(Type *Ty);           // line 640
Align    getABITypeAlign(Type *Ty);            // line 647
Align    getPrefTypeAlign(Type *Ty);           // line 659
const StructLayout *getStructLayout(StructType *Ty);  // line 736
IntegerType *getIntPtrType(LLVMContext &C, unsigned AS = 0);  // line 668
Type        *getIndexType(Type *PtrTy);        // line 703
```

`StructLayout` (DataLayout.h, verbatim):

```cpp
class StructLayout final : private TrailingObjects<StructLayout, TypeSize> {
  TypeSize StructSize;
  Align    StructAlignment;
  unsigned IsPadded   : 1;
  unsigned NumElements: 31;
public:
  TypeSize getSizeInBytes() const { return StructSize; }
  TypeSize getSizeInBits()  const { return 8 * StructSize; }
  Align    getAlignment()   const { return StructAlignment; }
  bool     hasPadding()     const { return IsPadded; }
  unsigned getElementContainingOffset(uint64_t FixedOffset) const;
  ArrayRef<TypeSize> getMemberOffsets() const { ... }
  TypeSize getElementOffset(unsigned Idx) const;
  TypeSize getElementOffsetInBits(unsigned Idx) const;
};
```

### DataLayout textual grammar (`target datalayout = "..."`)

Grammar fragments documented inside DataLayout.cpp parsing code:

- `[ifv]<size>:<abi>[:<pref>]` -- integer / float / vector spec
- `a<size>:<abi>[:<pref>]`      -- aggregate spec
- `p[<n>]:<size>:<abi>[:<pref>[:<idx>]]` -- pointer spec
                                          (n=address space)
- `e` / `E`                     -- little / big endian
- `n<size>[:<size>]...`         -- native integer widths
- `S<size>`                     -- stack natural alignment
- `F<type><abi>`                -- function pointer alignment
- `P<address space>`            -- program address space
- `A<address space>`            -- alloca address space
- `G<address space>`            -- default globals address space
- `m:<mangling>`                -- mangling mode
- `ni:<address space>[:...]`    -- non-integral address spaces

This is a per-module, target-specific string. The `Type` itself never
sees it -- a query like `DL.getTypeAllocSize(structTy)` walks the
struct, consults the `IntSpecs`/`FloatSpecs`/`PointerSpecs` tables, and
returns a target-specific answer. The same `StructType` value yields
different sizes under different `DataLayout`s.

Lesson for RegseqModel: `Ty` should be target-agnostic; layout is a
*query* against a separate `DataLayout`-like artifact, not a field on
the variant.


## C. Identity, interning, canonical form -- LLVMContext

Foundational invariant (Type.h, top-of-class comment, verbatim):

```
The instances of the Type class are immutable: once they are created,
they are never changed.  Also note that only one instance of a particular
type is ever created.  Thus seeing if two types are equal is a matter of
doing a trivial pointer comparison. To enforce that no two equal instances
are created, Type instances can only be created via static factory methods
in class Type and in derived classes.  Once allocated, Types are never
free'd.
```

`Type` constructor is `protected`; allocation is bump-allocated via
`LLVMContextImpl::Alloc`. `Type::Context` is a `LLVMContext &` reference
so every `Type*` belongs to exactly one context.

LLVMContext.h:

> LLVMContext (opaquely) owns and manages the core "global" data of
> LLVM's core infrastructure, including the type and constant uniquing
> tables ... LLVMContext itself provides no locking guarantees, so you
> should be careful to have one context per thread.

Interning tables in LLVMContextImpl.h, verbatim:

```cpp
DenseMap<unsigned, ByteType *>    ByteTypes;
DenseMap<unsigned, IntegerType *> IntegerTypes;

using FunctionTypeSet = DenseSet<FunctionType *, FunctionTypeKeyInfo>;
FunctionTypeSet FunctionTypes;

using StructTypeSet = DenseSet<StructType *, AnonStructTypeKeyInfo>;
StructTypeSet     AnonStructTypes;       // literal structs (structural)
StringMap<StructType *> NamedStructTypes; // identified structs (nominal)

using TargetExtTypeSet = DenseSet<TargetExtType *, TargetExtTypeKeyInfo>;
TargetExtTypeSet TargetExtTypes;

DenseMap<std::pair<Type *, uint64_t>,    ArrayType *>  ArrayTypes;
DenseMap<std::pair<Type *, ElementCount>, VectorType *> VectorTypes;

PointerType *AS0PointerType = nullptr;   // common-case fast path
DenseMap<unsigned, PointerType *>                    PointerTypes;
DenseMap<std::pair<Type *, unsigned>, TypedPointerType *> ASTypedPointerTypes;
```

Singleton primitives (LLVMContextImpl.h, verbatim):

```cpp
Type        VoidTy, LabelTy, HalfTy, BFloatTy, FloatTy, DoubleTy,
            MetadataTy, TokenTy;
Type        X86_FP80Ty, FP128Ty, PPC_FP128Ty, X86_AMXTy;
IntegerType Int1Ty, Int8Ty, Int16Ty, Int32Ty, Int64Ty, Int128Ty;
ByteType    Byte1Ty, Byte8Ty, Byte16Ty, Byte32Ty, Byte64Ty, Byte128Ty;
```

Canonical-form rule, by example (Type.cpp):

- `IntegerType::get(C, 32)` returns the cached `Int32Ty`; arbitrary
  widths get a single shared `IntegerType` per width.
- `PointerType::get(C, 0)` returns the singleton `AS0PointerType`.
  Address-space-N pointers are uniqued in `PointerTypes[N]`.
- `ArrayType::get(elem, N)` keys on `(elem-pointer, N)` -- so structural
  uniqueness across elements comes "for free" because elements are
  themselves uniqued.
- `FunctionType::get(ret, params, isVA)` uses a custom
  `FunctionTypeKeyInfo` and bump-allocates a single trailing-array of
  `Type*` after the FunctionType header.
- `StructType::get(ctx, etypes, packed)` -- literal struct -- uses
  `AnonStructTypeKeyInfo` for *structural* uniquing; sets
  `SCDB_IsLiteral`.
- `StructType::create(ctx, name)` -- identified struct -- DOES NOT
  intern; named structs are *nominal* (two structs with the same body
  but different names are distinct).
- `VectorType::get(elem, EC)` dispatches to FixedVectorType or
  ScalableVectorType, both keyed on `(elem, ElementCount)`.
- `TargetExtType::get(C, name, types, ints)` interns by
  (name, type-list, int-list).

Key design split:

- *Literal* aggregates (anonymous structs, arrays, vectors, function
  types, pointers) are STRUCTURALLY interned.
- *Identified* structs are NOMINAL -- they have an identity beyond their
  shape, can be opaque (`%T = type opaque`) and forward-declared, and
  can be made recursive via `setBody()` after creation.

This dual model is the answer to "how do you handle recursive types in
a structural system": make recursion the exclusive territory of named
nominal types.


## D. Special-semantics markers (NOT type variants)

LLVM keeps semantic markers OFF the type system:

- **Atomicity** is a property of `load`/`store`/atomic instructions, not
  the type. There is no `AtomicTy` variant.
- **Volatile** is a flag on memory operations, not on the type.
- **Address space** is the one piece of memory semantics that LLVM does
  put into the type -- `PointerType` carries an `unsigned AddressSpace`
  (in `SubclassData`). Textually `ptr addrspace(N)`.
- **Metadata** has its own `MetadataTy` first-class type, but actual
  metadata (`!dbg`, attribute lists, etc.) hangs off Instructions.
- **Intrinsics** (e.g., `llvm.memcpy`, `llvm.lifetime.start`) are
  Functions of regular FunctionType; only their *name* (`llvm.*`) marks
  them.
- **Token type** (`token`) exists specifically to mark SSA values
  produced by certain intrinsics that must not be optimized as ordinary
  values; it is a first-class type but not a single-value type.
- **Calling convention**, parameter attributes (`byval`, `sret`,
  `noalias`, `nocapture`), and function attributes are stored on the
  Function/CallSite, NOT in FunctionType. Two functions with different
  calling conventions share one FunctionType.

Lesson for RegseqModel: keep `Ty` to *shape*; route atomicity,
volatility, ABI markers, intrinsicness, and effect annotations through
operation/attribute layers.


## E. Generics

N/A. LLVM IR has no generics. Polymorphism in LLVM is handled by
*monomorphization at the frontend* before IR emission. The IR is
fully monomorphic. (Token type and TargetExtType provide a bounded form
of opaque parameterization, but neither models user-defined generics.)


## F. Bridge / consumption shape -- how IR types feed codegen

The pipeline is: `Type` -> `DataLayout` query -> `MVT/EVT`
(SelectionDAG) or `LLT` (GlobalISel) -> target-specific register
classes. Two notable points relevant to RegseqModel's "how does Ty feed
emission":

1. The IR `Type` does NOT carry any target legality information.
   Whether `i31` or `<7 x i32>` is "legal" is determined by the target
   in codegen; legalization (promotion/expansion/widening) is a
   separate pipeline phase that consumes `Type` + `DataLayout`.
2. `LegalIntWidths` in `DataLayout` is purely advisory -- it tells
   optimizers which widths to prefer; it does not constrain the IR.

Comment in Type.h on `IntegerType`: *"SelectionDAG type legalization
requires power-of-2 IntegerTypes"* -- but this is a downstream
constraint, not an upstream one. Frontends emit `i7`, `i23`, etc., and
the codegen pipeline legalizes them. RegseqModel can mirror this: keep
`Ty` permissive; let a separate emitter/legalizer reject or rewrite.

For aggregates feeding codegen, `getStructLayout(StructType*)` returns
the cached `StructLayout` with member offsets and total size; backends
walk this when lowering. Note that `StructLayout` is *cached* on the
DataLayout, not on the StructType -- another instance of the
"layout != type" discipline.


## G. Renderer / pretty-printer -- LangRef textual grammar

The textual `.ll` syntax (LangRef Type System section):

| Variant                | Syntax                                       |
|------------------------|----------------------------------------------|
| Void                   | `void`                                       |
| Function               | `<ret-type> ( <arg-type>, ... )`             |
|                        | example: `i32 (i32, i32)`                    |
|                        | varargs: `i32 (i32, ...)`                    |
| Integer                | `i<N>`  (N from 1 to 2^23)                   |
|                        | examples: `i1`, `i8`, `i32`, `i1942652`      |
| Floating-point         | `half`, `bfloat`, `float`, `double`,         |
|                        | `fp128`, `x86_fp80`, `ppc_fp128`             |
| X86 AMX                | `x86_amx`                                    |
| Pointer (opaque)       | `ptr`                                        |
| Pointer w/ addr-space  | `ptr addrspace(<N>)`  e.g., `ptr addrspace(5)` |
| Array                  | `[ <N> x <type> ]`     e.g., `[40 x i32]`    |
| Vector (fixed)         | `< <N> x <type> >`     e.g., `<4 x i32>`     |
| Vector (scalable)      | `< vscale x <N> x <type> >`                  |
|                        | e.g., `<vscale x 4 x i32>`                   |
| Struct (literal)       | `{ <type>, <type>, ... }`                    |
|                        | e.g., `{ i32, i32, i32 }`                    |
| Struct (literal,packed)| `<{ <type>, ... }>`    e.g., `<{ i8, i32 }>` |
| Struct (identified)    | `%name = type { ... }`                       |
|                        | e.g., `%T1 = type { <2 x i32>, ... }`        |
| Struct (opaque)        | `%name = type opaque`                        |
| Label                  | `label`                                      |
| Token                  | `token`                                      |
| Metadata               | `metadata`                                   |
| Target extension       | `target(\"<name>\", <types>..., <ints>...)`  |
|                        | e.g., `target("spirv.Image", void, i32 1)`   |

Round-trip property: the LLVM AsmWriter prints, then the AsmParser
re-parses -- and the resulting `Type*` is pointer-equal to the original
(within the same context) for any literal/structural type, exactly
because of interning. For identified structs, the *name* is what
re-binds; identity is preserved by lookup in `NamedStructTypes`.


## Watch-outs and observations

### The opaque-pointer transition (LLVM-15) -- the central lesson

OpaquePointers.rst rationale, condensed from primary text:

- "LLVM IR pointers can be cast back and forth between pointers with
  different pointee types. The pointee type does not necessarily
  represent the actual underlying type in memory."
- Pointee types created "lots of redundant no-op bitcasts" with
  cascading costs in memory, compile-time, and code complexity.
- Memory optimizations (SROA, GVN, AA) work on offsets, not pointee
  types, so the pointee type was *epistemically wrong* -- it claimed
  knowledge the optimizer did not actually use.
- The fix: drop the pointee from the type; push the type *down to the
  operation*. `load i64* %p` becomes `load i64, ptr %p`. GEP carries
  `getSourceElementType()`. Stores carry the value's type.
- Quote from OpaquePointers.rst paralleling an earlier transition:
  "Currently there is no distinction between signed and unsigned
  integer types, but rather each integer operation contains flags to
  signal how to treat the integer."

Migration timeline (LLVM 14 prep, 15 default-on, 16 typed-ptr
best-effort, 17 opaque-only) and matching C-API renames
(`LLVMBuildLoad` -> `LLVMBuildLoad2`, `LLVMBuildGEP` -> `LLVMBuildGEP2`,
etc.).

Direct lesson for RegseqModel `Ty`:

- A type variant should encode information that consumers actually use.
  If load/store optimization works on offsets, then the pointer's
  pointee is decorative, and decorative information rots into
  inconsistency.
- Move type information to the operation site when the operation is
  what actually needs it. RegseqModel should not put a pointee on
  pointer types unless every consumer (lowering, alias analysis, C
  emission) is going to consult it.
- Note: LLVM kept `TypedPointerType` for "some GPU targets" -- so the
  variant exists, but the canonical IR uses `PointerType` (opaque). If
  RegseqModel needs typed-pointer reasoning for C emission, model it
  as a parallel auxiliary structure rather than baking it into the
  primary `Ty`.

### Nominal vs structural

LLVM is the textbook example of mixing both:

- Literal structs, arrays, vectors, functions, pointers, integers,
  floats: structural / interned by shape.
- Identified structs: nominal / identity by name; can be opaque, can be
  recursive.

The split is principled, not historical: recursion needs nominal
identity, and nominal identity is also what gives debuggers and ABI
tools a stable name to refer to. RegseqModel should consider the same
split rather than picking one regime for everything. In particular,
cyclic types pretty much *force* a nominal channel.

### Layout discipline

The cleanest take-away: `Type` answers *what shape*, `DataLayout`
answers *how big and where*. The same `Type` value is sized differently
under different `DataLayout`s. `StructLayout` is cached on the
DataLayout, not on the StructType. RegseqModel should keep `Ty`
target-agnostic and route all "how big" / "what offset" / "what
alignment" questions through a separate query surface.

### Singletons + bump-allocation + per-context interning

Common primitives (`Int1Ty`, `Int32Ty`, `FloatTy`, etc.) are stored as
direct fields in `LLVMContextImpl`, not in maps. Maps are used for
parametric variants. Allocation is via a context-scoped
`BumpPtrAllocator`; types are never freed. This is a deliberate design
where type lifetime equals context lifetime. RegseqModel can pick a
weaker strategy (e.g., arena-per-crate) but the structural lesson --
"identity is pointer-equality after interning" -- is broadly applicable
and dramatically simplifies later passes.

### `isFirstClassType` is a method, not a flag

The role classification (`isFirstClassType`, `isSingleValueType`,
`isAggregateType`) is computed from `TypeID`, not stored. This is
possible because the variant set is small and stable. RegseqModel
should similarly compute role predicates from the variant rather than
introducing new variants for "aggregate" vs "scalar".

### What LLVM does NOT have in the type system

Worth listing as a counter-design exercise:

- No atomicity, no volatility, no ordering.
- No mutability / const.
- No lifetime / region.
- No effect / capability annotations.
- No generics, no traits, no type classes.
- No alignment override at the type level (alignment lives on
  alloca/load/store/GlobalVariable, or comes from DataLayout).
- No name on a literal aggregate; names are exclusive to identified
  structs.
- No inheritance / subtyping (LLVM has no subtype relation in the IR;
  the cast instructions are explicit and total).

This minimalism is a deliberate design choice and one of the reasons
LLVM IR has remained stable across decades of frontend churn.
