# Prior Art: MLIR (Multi-Level IR) -- Type System and Dialect Conversion

Primary-source research for the stage-3 type-system spec of RegseqModel.
All quoted text was retrieved from primary sources on the LLVM project's
`main` branch (April 2026) and from the MLIR documentation site. Citations
appear inline and are collected at the end of the document.

This file deliberately does NOT consult `stage-0/prior-art/mlir.md`. It
quotes the canonical headers, ODS files, and docs directly.

---

## A. Type-vocabulary completeness -- builtin types and dialect extension

MLIR is, by design, an open-vocabulary type system. The MLIR Language
Reference states:

> "Each value in MLIR has a type defined by the type system. MLIR has an
> open type system (i.e. there is no fixed list of types), and types may
> have application-specific semantics."

The grammar admits three categories:

```
type ::= type-alias | dialect-type | builtin-type
```

(LangRef.md, "Type System".)

### A.1 Builtin dialect

The Builtin dialect provides the base vocabulary that any other dialect
can use without explicitly depending on it. Per the Builtin docs:

> "This dialect is implicitly loaded in every `MLIRContext`, and
> available directly to all users of MLIR."

The builtin type set, as enumerated on `mlir.llvm.org/docs/Dialects/Builtin/`,
is:

Floating-point family:
- BFloat16Type ("bf16")
- Float4E2M1FNType, Float6E2M3FNType, Float6E3M2FNType
- Float8E3M4Type, Float8E4M3Type, Float8E4M3B11FNUZType,
  Float8E4M3FNType, Float8E4M3FNUZType, Float8E5M2Type,
  Float8E5M2FNUZType, Float8E8M0FNUType
- Float16Type, Float32Type, Float64Type, Float80Type, Float128Type
- FloatTF32Type ("tf32")

Composite / abstract:
- ComplexType ("complex<T>")
- FunctionType (input types -> result types)
- GraphType (input types -> result types) -- a function-like sibling
- IndexType (platform-dependent integer width)
- IntegerType (parametric width + signedness)
- MemRefType (shaped reference to memory)
- NoneType ("a unit type")
- OpaqueType ("type of a non-registered dialect")
- RankedTensorType, UnrankedTensorType
- TupleType ("fixed-sized collection of other types")
- UnrankedMemRefType
- VectorType (multi-dimensional SIMD vector)

### A.2 ODS definitions (verbatim)

From `mlir/include/mlir/IR/BuiltinTypes.td`:

```
def Builtin_Integer : Builtin_Type<"Integer", "integer",
    [VectorElementTypeInterface, QuantStorageTypeInterface,
     DeclareTypeInterfaceMethods<DenseElementTypeInterface,
       ["getDenseElementBitSize", "convertToAttribute",
        "convertFromAttribute"]>]>
  // parameters: "unsigned":$width, "SignednessSemantics":$signedness
```

```
def Builtin_Index : Builtin_Type<"Index", "index",
    [DeclareTypeInterfaceMethods<DenseElementTypeInterface,
       ["getDenseElementBitSize", "convertToAttribute",
        "convertFromAttribute"]>, VectorElementTypeInterface]>
```

```
def Builtin_Complex : Builtin_Type<"Complex", "complex",
    [DeclareTypeInterfaceMethods<DenseElementTypeInterface,
       ["getDenseElementBitSize", "convertToAttribute",
        "convertFromAttribute"]>]>
  // parameters: "Type":$elementType
```

```
def Builtin_Function : Builtin_FunctionLike<"Function", "function">
def Builtin_Graph    : Builtin_FunctionLike<"Graph",    "graph">
  // parameters: "ArrayRef<Type>":$inputs, "ArrayRef<Type>":$results
```

```
def Builtin_MemRef : Builtin_Type<"MemRef", "memref",
    [PtrLikeTypeInterface, ShapedTypeInterface], "BaseMemRefType">
  // parameters: shape, elementType, layout, memorySpace
```

```
def Builtin_RankedTensor : Builtin_Type<"RankedTensor", "tensor",
    [ShapedTypeInterface, ValueSemantics], "TensorType">
  // parameters: shape, elementType, encoding

def Builtin_UnrankedTensor : Builtin_Type<"UnrankedTensor", "unranked_tensor",
    [ShapedTypeInterface, ValueSemantics], "TensorType">
  // parameters: "Type":$elementType
```

```
def Builtin_Vector : Builtin_Type<"Vector", "vector",
    [ShapedTypeInterface, ValueSemantics], "Type">
  // parameters: shape, elementType, scalableDims
```

```
def Builtin_Tuple   : Builtin_Type<"Tuple",   "tuple">
  // parameters: "ArrayRef<Type>":$types

def Builtin_None    : Builtin_Type<"None",    "none">

def Builtin_Opaque  : Builtin_Type<"Opaque",  "opaque">
  // parameters: "StringAttr":$dialectNamespace, StringRefParameter:$typeData

def Builtin_UnrankedMemRef : Builtin_Type<"UnrankedMemRef", "unranked_memref",
    [PtrLikeTypeInterface, ShapedTypeInterface], "BaseMemRefType">
  // parameters: "Type":$elementType, "Attribute":$memorySpace
```

Float definitions (verbatim mnemonics from `BuiltinTypes.td`):

- `Builtin_Float8E5M2`     -- "f8E5M2"
- `Builtin_Float8E4M3`     -- "f8E4M3"
- `Builtin_Float8E4M3FN`   -- "f8E4M3FN"
- `Builtin_Float8E5M2FNUZ` -- "f8E5M2FNUZ"
- `Builtin_Float8E4M3FNUZ` -- "f8E4M3FNUZ"
- `Builtin_Float8E4M3B11FNUZ` -- "f8E4M3B11FNUZ"
- `Builtin_Float8E3M4`     -- "f8E3M4"
- `Builtin_Float4E2M1FN`   -- "f4E2M1FN"
- `Builtin_Float6E2M3FN`   -- "f6E2M3FN"
- `Builtin_Float6E3M2FN`   -- "f6E3M2FN"
- `Builtin_Float8E8M0FNU`  -- "f8E8M0FNU"
- `Builtin_BFloat16`       -- "bf16"
- `Builtin_Float16`        -- "f16"
- `Builtin_FloatTF32`      -- "tf32"
- `Builtin_Float32`        -- "f32"
- `Builtin_Float64`        -- "f64"
- `Builtin_Float80`        -- "f80"
- `Builtin_Float128`       -- "f128"

### A.3 Dialect extension mechanism

LangRef:

> "Similarly to operations, dialects may define custom extensions to the
> type system."

Grammar:

```
dialect-type ::= `!` (opaque-dialect-type | pretty-dialect-type)
opaque-dialect-type ::= dialect-namespace dialect-type-body
pretty-dialect-type ::= dialect-namespace `.` pretty-dialect-type-lead-ident
                        dialect-type-body?
dialect-type-body   ::= `<` dialect-type-contents+ `>`
```

Examples (verbatim from LangRef):

```
!tf<string>
!foo<something<abcd>>
!tf.string
!foo.something<abcd>
```

A new dialect type is declared in TableGen (ODS) and registered via the
dialect's `initialize()`:

```cpp
void MyDialect::initialize() {
  addTypes<
#define GET_TYPEDEF_LIST
#include "MyDialect/Types.cpp.inc"
  >();
}
```

> "Note that when registering, the full definition of the storage classes
> must be visible."
> ("Defining Dialects -- Attributes and Types".)

### A.4 Special-purpose constraint

`IntegerType` width is bounded by a static constant in `BuiltinTypes.td`:

```
static constexpr unsigned kMaxWidth = (1 << 24) - 1;
```

Signedness is captured by an enum:

```
enum SignednessSemantics : uint32_t {
  Signless, /// No signedness semantics
  Signed,   /// Signed integer
  Unsigned, /// Unsigned integer
};
```

Note that signless (the default in core arithmetic dialects) is a
distinct first-class state, not a sentinel.

---

## B. Layout / representation

MLIR keeps layout out of the type's identity and routes layout queries
through dedicated interfaces.

`mlir/include/mlir/Interfaces/DataLayoutInterfaces.h` header:

> "Defines the interfaces for the data layout specification, operations
> to which they can be attached, types subject to data layout and
> dialects containing data layout entries."

The `DataLayout` query API exposes:

- `getTypeSize(Type t)`
- `getTypeSizeInBits(Type t)`
- `getTypeABIAlignment(Type t)`
- `getTypePreferredAlignment(Type t)`
- `getTypeIndexBitwidth(Type t)`
- `getEndianness()`
- `getAllocaMemorySpace()`
- `getProgramMemorySpace()`
- `getGlobalMemorySpace()`
- `getStackAlignment()`

> "Default handler for the type size request. Computes results for
> built-in types and dispatches to the DataLayoutTypeInterface for other
> types."

In other words: the `Type` itself does not carry size/alignment.
`DataLayoutTypeInterface` is implemented per-type, and
`DataLayoutOpInterface` (typically attached to a module-level op) carries
a `DataLayoutSpecInterface` (a list of `DataLayoutEntryInterface`s) that
overrides defaults per type or per dialect. This decouples the *vocabulary*
of types from the *target* layout, so the same MemRef/Integer can be
asked about size differently in different lowerings.

MemRef itself carries layout-affecting parameters as part of its
identity: `shape`, `elementType`, `layout` (an affine map / strided
layout attribute), and `memorySpace`. From `BuiltinTypes.td`:

> "The layout must avoid internal aliasing, i.e., two distinct tuples of
> _in-bounds_ indices must be pointing to different elements in memory."

VectorType carries `scalableDims` for SVE-style scalable vectors:

> "The dimensions of a vector type can be fixed-length, scalable, or a
> combination of the two. The scalable dimensions in a vector are
> indicated between square brackets ([ ])."

Address space / memory space is modelled as a generic `Attribute` on
MemRef, not a fixed enum -- another extension hook.

---

## C. Identity, interning, canonical form -- MLIRContext uniquing

This is one of MLIR's defining design choices.

`mlir/include/mlir/IR/Types.h` (header doc):

> "Instances of the Type class are uniqued, have an immutable identifier
> and an optional mutable component. They wrap a pointer to the storage
> object owned by MLIRContext. Therefore, instances of Type are passed
> around by value."

> "Some types are 'primitives' meaning they do not have any parameters."
> "Parametric types have additional information that differentiates the
> types of the same class." Type parameters form "the unique immutable
> key," whereas "the mutable component of the type can be modified after
> the type is created, but cannot affect the identity."

The `Type` class is a trivial pointer wrapper:

```cpp
using ImplType = TypeStorage;
using AbstractTy = AbstractType;

MLIRContext *getContext() const;
Dialect &getDialect() const { return impl->getAbstractType().getDialect(); }
TypeID getTypeID() { return impl->getAbstractType().getTypeID(); }
const AbstractTy &getAbstractType() const { return impl->getAbstractType(); }
ImplType *getImpl() const { return impl; }
```

> "Types are constructed and uniqued via the 'detail::TypeUniquer' class."

`mlir/include/mlir/IR/TypeSupport.h`:

> "This file defines support types for registering dialect extended types."

Two collaborators:

- **AbstractType** holds "all of the static information common to all
  instances of a registered Type" -- TypeID, owning dialect, registered
  interfaces, traits, sub-element walkers.
- **TypeStorage** is the "Base storage class appearing in a Type" -- it
  carries the per-instance parameter values plus a pointer to the
  AbstractType.

The uniquer:

> "A utility class to get, or create, unique instances of types within an
> MLIRContext. ... [It] manages all creation and uniquing of types."

A safety check:

> "storage uniquer isn't initialized: the dialect was likely not loaded,
> or the type wasn't added with addTypes<...>() in the
> Dialect::initialize() method."

Per the "Defining Dialects -- Attributes and Types" doc, every storage
class must define:

- `KeyTy` -- "A type that uniquely identifies an instance" (typically a
  tuple of the immutable parameters)
- `static Storage *construct(StorageAllocator &allocator, const KeyTy &key)`
- `bool operator==(const KeyTy &key) const`
- `static llvm::hash_code hashKey(const KeyTy &key)`
- `getKey()`, `getAsKey()`

Mutable components are explicitly carved out:

> "Attributes and Types are immutable objects uniqued within an
> MLIRContext. That being said, some parameters may be treated as
> 'mutable' and modified after construction."
> "The mutable component must not participate in the storage `KeyTy`."

Mutable storage exposes:

```cpp
LogicalResult mutate(StorageAllocator &allocator, Args... args);
```

Two consequences worth flagging:

1. Type identity is `ptr_eq` after uniquing -- structural equality
   collapses to pointer equality, which makes hashing and DenseMap keys
   trivial.
2. Types are scoped to a particular `MLIRContext`. Crossing contexts
   requires re-construction. This is closed-vocabulary inside an open
   *system*: each context's set of registered types is fixed by the
   dialects loaded into it.

---

## D. Special-semantics markers

MLIR does not embed semantics into the type's *class* slot the way a
closed enum would. Instead, semantic tags are layered on top of the
uniqued storage in three orthogonal ways:

### D.1 Parameters as carriers of semantics

The most direct example is `IntegerType`'s `SignednessSemantics`
parameter, with three states (`Signless`, `Signed`, `Unsigned`). It is
part of the uniquing key, so `i32`, `si32`, and `ui32` are three
distinct types.

`MemRefType`'s `memorySpace` is an `Attribute` parameter -- any dialect
can attach its own address-space marker without growing the type
vocabulary.

### D.2 Type Interfaces (declared methods)

Examples seen in `BuiltinTypes.td`:

- `ShapedTypeInterface` (MemRef, Tensor, Vector)
- `PtrLikeTypeInterface` (MemRef, UnrankedMemRef)
- `DenseElementTypeInterface` (Integer, Index, Complex)
- `VectorElementTypeInterface` (Integer, Index)
- `QuantStorageTypeInterface` (Integer)
- `DataLayoutTypeInterface` (queried by `DataLayout`)

Type interfaces are virtual method tables registered on
`AbstractType`; `isa<>`/`dyn_cast<>` and trait checks become the runtime
discriminator.

### D.3 Type Traits (compile-time markers)

E.g. `ValueSemantics` on tensor and vector types -- a trait that simply
asserts SSA-value semantics rather than reference semantics.
`AbstractType::hasTrait<...>()` answers these queries.

The takeaway: MLIR's "special semantics" are sets of interfaces/traits
attached to a uniqued storage object, not subclasses or sentinel
discriminants on the type itself.

---

## E. Generic-Rust handling

Not applicable. MLIR does not natively consume Rust generics. Skipped
per the research scope.

---

## F. Bridge / consumption shape -- the Dialect Conversion framework

This is the part most directly relevant to a bridge -> C-codegen step.

### F.1 Top-of-file purpose

`mlir/include/mlir/Transforms/DialectConversion.h`:

> "This file declares a generic pass for converting between MLIR
> dialects."

### F.2 Three entry points

From `DialectConversion.h`:

> "**Partial Conversion**: This method converts as many operations to
> the target as possible, ignoring operations that failed to legalize."

> "**Full Conversion**: This method returns failure if the conversion of
> any operation fails, or if there are unreachable blocks in any of the
> regions nested within 'ops'."

> "**Analysis Conversion**: This method analyzes which operations would
> be successfully converted to the target if a conversion was applied."

`docs/DialectConversion.md` confirms and elaborates:

> "A partial conversion will legalize as many operations to the target
> as possible, but will allow pre-existing operations that were not
> explicitly marked as 'illegal' to remain unconverted."

> "A full conversion legalizes all input operations, and is only
> successful if all operations are properly legalized to the given
> conversion target."

> "An analysis conversion will analyze which operations are legalizable
> to the given conversion target if a conversion were to be applied."
> (No transformation is applied.)

### F.3 ConversionTarget -- legality declaration

> "**Legal**: This action signals that every instance of a given
> operation is legal, i.e. any combination of attributes, operands,
> types, etc. are valid."

> "**Dynamic**: This action signals that only some instances of a given
> operation are legal. This allows for defining fine-tune constraints,
> e.g. saying that `arith.addi` is only legal when operating on 32-bit
> integers."

> "**Illegal**: This action signals that no instance of a given
> operation is legal. Operations marked as 'illegal' must always be
> converted for the conversion to be successful."

Key methods on `ConversionTarget`:

- `addLegalDialect`, `addLegalOp` -- whitelist legal forms
- `addIllegalDialect`, `addIllegalOp` -- mark must-be-rewritten forms
- `addDynamicallyLegalOp` -- legality decided by callback
- `markUnknownOpDynamicallyLegal` -- "for operations (and dialects) that
  do not have a set legalization action, treat them as dynamically
  legal and invoke the given callback"

Operations not in any of these buckets are "unknown"; partial
conversions will leave them alone, full conversions will fail on them.

### F.4 ConversionPattern + adaptor

> "**ConversionPattern**: Base class for the conversion patterns. This
> pattern class enables type conversions, and other uses specific to
> the conversion framework."

> "**OpConversionPattern**: Wrapper around ConversionPattern that
> allows for matching and rewriting against an instance of a derived
> operation class as opposed to a raw Operation."

The pattern receives an `operands`/`adaptor` argument:

> "[It contains] the most recent replacement values of the respective
> operands of the matched operation."

This is what makes type changes safe inside the framework: the pattern
is rewritten in terms of *already-converted* operand values, so it is
not the pattern's job to chase down upstream replacements.

> "The framework will automatically build a graph of conversions to
> convert non-legal operations into a set of legal ones."

### F.5 TypeConverter -- the lowering boundary

The `TypeConverter` is the central object for type-system bridging
between dialects. From `DialectConversion.h`:

- `addConversion`: "Register a conversion function. A conversion
  function must be convertible to any of the following forms (where
  `T` is `Value` or a class derived from `Type`, including `Type`
  itself)..."
- `convertType`: "Convert the given type. This function returns
  failure if no valid conversion exists, success otherwise."
- `convertTypes`: "Convert the given types, filling 'results' as
  necessary. This returns 'failure' if the conversion of any of the
  types fails, 'success' otherwise."
- `convertSignatureArgs`: "This method allows for converting specific
  arguments of a signature."
- `convertBlockSignature`: "This function converts the type signature
  of the given block, by invoking 'convertSignatureArg' for each
  argument."
- `isLegal`: "Return true if the given type is legal for this type
  converter, i.e. the type converts to itself."

A single source type may map to *N* target types -- function and block
signatures can fan out under the converter.

`docs/DialectConversion.md`:

> "A context-unaware conversion function converts a `Type` into a
> `Type`. A context-aware conversion function converts a `Value` into a
> type."

The context-aware form lets the conversion read attributes or producers
of the SSA value, not just the type's identity.

### F.6 Materializations and `unrealized_conversion_cast`

This is the mechanism that lets the framework run incrementally and
keep the IR well-typed at every intermediate state.

From `DialectConversion.h`:

> "**Source Materialization**: This is used when some uses of the
> original value persist beyond the main conversion. ... This method
> registers a materialization that will be called when converting a
> replacement value back to its original source type."

> "**Target Materialization**: This method registers a materialization
> that will be called when converting a value to a target type
> according to a pattern's type converter."

From `docs/DialectConversion.md`:

> "Source Materialization: Converts replacement values back to original
> ('source') types when users that expects the original ('source')
> type" remain after conversion.

> "Target Materialization: Converts values to the type that is expected
> by a conversion pattern according to its type converter."

> "The framework inserts `builtin.unrealized_conversion_cast` operations
> when materializations are required but unavailable through the type
> converter callbacks, or when `ConversionConfig::buildMaterializations`
> is disabled."

`builtin.unrealized_conversion_cast`, from the Builtin dialect docs:

> "Represents an unrealized conversion from one set of types to
> another, that is used to enable the inter-mixing of different type
> systems."

> "This operation should not be attributed any special representational
> or execution semantics, and is generally only intended to be used to
> satisfy the temporary intermixing of type systems during the
> conversion of one type system to another."

Its arity is variadic: 0..N inputs -> 1..N outputs. Pairs of these
(`A -> B` followed by `B -> A`) are the "I owe you" instrument that
balances a multi-pass lowering before the final converters reduce them
to nothing.

### F.7 The shape of a typical lowering pipeline

Putting the above together, an MLIR pass that lowers between two type
systems looks like:

1. Build a `ConversionTarget` -- what's legal in the *target* dialect.
2. Build a `TypeConverter` -- per-type rules for source -> target type,
   plus optional argument/source/target materialization callbacks.
3. Populate a `RewritePatternSet` with `OpConversionPattern`s -- each
   uses the type converter on its operands and results via the adaptor.
4. Call `applyPartialConversion` / `applyFullConversion` /
   `applyAnalysisConversion`.

This is the "TypeConverter at the lowering boundary" pattern -- exactly
the structure we noted as a candidate for our bridge -> C-codegen step.
See "Watch-outs and observations" below.

---

## G. Renderer / pretty-printer format

### G.1 Textual form

Every type has a textual form that round-trips through the parser. The
ODS field `assemblyFormat` in `BuiltinTypes.td` (and in dialects in
general) declaratively defines parser+printer pairs:

> "Attributes and types defined in ODS with a mnemonic can define an
> `assemblyFormat` to declaratively describe custom parsers and
> printers."

Available directives include `params`, `struct`, `custom<Foo>`,
`qualified`. The default printer is:

> "$_printer << $_self".

If the format is hairier than ODS can express, a type sets
`hasCustomAssemblyFormat = 1` and supplies C++ `parse()`/`print()`.

Verifier hook:

> "Setting `genVerifyDecl = 1` generates: `static LogicalResult
> verify(function_ref<InFlightDiagnostic()> emitError, parameters...)`.
> These methods are used to verify the parameters provided to the
> attribute or type class on construction, and emit any necessary
> diagnostics."

So a type's invariants are checked at *construction* time, not just at
print time -- because once a uniqued storage is in the context, every
later reference is by pointer.

Type aliases (`!avx_m128 = vector<4 x f32>`) keep textual MLIR readable
without affecting identity:

> "MLIR supports defining named aliases for types. A type alias is an
> identifier that can be used in the place of the type that it defines.
> These aliases *must* be defined before their uses."

> "Alias names may not contain a '.', since those names are reserved
> for dialect types."

### G.2 Bytecode form

MLIR also has a bytecode encoding intended to be a stable on-disk form
that round-trips with the textual form. (The `mlir.llvm.org/docs/Bytecode/`
page returned 404 at the time of this research; primary-source quotes
about the bytecode encoding details were not captured here. The
existence and intent of the bytecode form is referenced from
`PassManager.md` and `BytecodeWriter`/`BytecodeReader` headers in tree.)

### G.3 Verifier-on-every-pass

`mlir/include/mlir/Pass/PassManager.h`:

```cpp
/// Runs the verifier after each individual pass.
void enableVerifier(bool enabled = true);

/// A flag that indicates if the IR should be verified in between passes.
bool verifyPasses : 1;
```

This is the "verifier between every pass" guarantee -- on by default,
toggleable. Crash reproducers serialize `verify_each: true` as part of
their config (per `PassManagement.md`), confirming the same flag flows
through the reproducer machinery.

IR-printing instrumentation flags (from `PassManagement.md`):

- `mlir-print-ir-before-all` / `mlir-print-ir-after-all`
- `mlir-print-ir-after-change`
- `mlir-print-ir-module-scope`

These are independent of verification and document the standard
invariant that any printed IR after any pass should re-parse.

---

## Watch-outs and observations

These are observations from primary sources, not RegseqModel design
proposals.

### Open vocabulary, closed per context

> "MLIR has an open type system (i.e. there is no fixed list of
> types)..." (LangRef.)

But in practice, every `MLIRContext` has a *fixed* set of registered
dialects, and each dialect's storage is initialised in
`Dialect::initialize()` via `addTypes<...>()`. So the system is
"open" at the language level, "closed" at the per-tool level. This is
an interesting middle ground between a hard-coded enum and a fully
dynamic type universe: extensibility is paid for at dialect-load time,
not at type-construction time.

### Identity by uniquing, not by structural compare

`Type` is a pointer. Equality is pointer equality. Hashing is pointer
hashing. Structural equality is enforced once, at construction, by the
uniquer. This eliminates an entire class of bugs (two types that "look
the same" but compare unequal) and makes typed maps/sets cheap. For
RegseqModel's enum-based `Ty`, the analogous decision would be whether
`Ty: Eq + Hash` is structural or interned; MLIR's choice is interning.

### Layout is a query interface, not a type field

MemRef carries layout because layout *is* part of MemRef's identity at
the language level. But target-dependent quantities -- size, alignment,
endianness, alloca/global/program memory spaces -- are routed through
`DataLayoutTypeInterface`/`DataLayoutOpInterface` and a per-module
`DataLayoutSpec`. Types do not bake in size. This is critical for a
multi-target IR.

### Three orthogonal "tag" mechanisms

The combination of *parameters* (signedness, encoding, memorySpace),
*type interfaces* (Shaped, PtrLike, DenseElement, ...) and *type traits*
(`ValueSemantics`) gives MLIR three orthogonal ways to mark special
semantics. None of them grow the closed enum; all of them are
discoverable via `isa<>`/`hasTrait<>`/interface dispatch.

### Dialect Conversion as a model for a bridge -> C-codegen lowering

The shape of the framework is worth highlighting because it maps
naturally onto a "RegseqModel -> C" bridge:

- A `ConversionTarget` says what the target language admits as
  legal -- in our case, "what the C surface can express" (concrete
  scalar types, fixed-layout structs, function signatures with
  by-value/by-pointer ABIs, etc.).
- A `TypeConverter` carries the rules `RegseqTy -> CTy`, possibly with
  fan-out (e.g. a struct splitting into multiple arguments at the ABI
  boundary). The fan-out form is precisely what `convertType ->
  SmallVector<Type>` and `convertSignatureArgs` already model.
- `OpConversionPattern`s (in our case, RegseqIR ops -> C statements /
  expressions) consume converted operand values via an adaptor, so
  patterns never have to chase upstream replacements themselves.
- `unrealized_conversion_cast` is the IOU instrument that keeps the IR
  well-typed mid-rewrite. Whether or not we want such an instrument in
  RegseqModel is a design question, but its role -- "let me change a
  type in step N and finalize the conversion in step M without leaving
  malformed IR in between" -- is one of the cleanest engineering
  benefits of the framework.
- Three modes (partial / full / analysis) cleanly separate "rewrite as
  much as you can," "everything must lower," and "tell me if this
  would lower."

### Verification on every pass

`enableVerifier(true)` is the default. Combined with construction-time
verification on each new storage instance, the invariant chain is:
type valid at construction -> op valid at construction (via op-level
verifier) -> module valid after every pass (via pass-manager verifier).
Round-tripping printed IR is a corollary, not the primary guarantee.

---

## Sources (primary)

- `mlir/include/mlir/IR/BuiltinTypes.td`
  -- https://raw.githubusercontent.com/llvm/llvm-project/main/mlir/include/mlir/IR/BuiltinTypes.td
- `mlir/include/mlir/IR/BuiltinTypes.h`
  -- https://raw.githubusercontent.com/llvm/llvm-project/main/mlir/include/mlir/IR/BuiltinTypes.h
- `mlir/include/mlir/IR/Types.h`
  -- https://raw.githubusercontent.com/llvm/llvm-project/main/mlir/include/mlir/IR/Types.h
- `mlir/include/mlir/IR/TypeSupport.h`
  -- https://raw.githubusercontent.com/llvm/llvm-project/main/mlir/include/mlir/IR/TypeSupport.h
- `mlir/include/mlir/Transforms/DialectConversion.h`
  -- https://raw.githubusercontent.com/llvm/llvm-project/main/mlir/include/mlir/Transforms/DialectConversion.h
- `mlir/include/mlir/Interfaces/DataLayoutInterfaces.h`
  -- https://raw.githubusercontent.com/llvm/llvm-project/main/mlir/include/mlir/Interfaces/DataLayoutInterfaces.h
- `mlir/include/mlir/Pass/PassManager.h`
  -- https://raw.githubusercontent.com/llvm/llvm-project/main/mlir/include/mlir/Pass/PassManager.h
- `mlir/docs/LangRef.md`
  -- https://raw.githubusercontent.com/llvm/llvm-project/main/mlir/docs/LangRef.md
- `mlir/docs/DialectConversion.md`
  -- https://raw.githubusercontent.com/llvm/llvm-project/main/mlir/docs/DialectConversion.md
- Builtin dialect docs -- https://mlir.llvm.org/docs/Dialects/Builtin/
- Defining Dialects: Attributes and Types
  -- https://mlir.llvm.org/docs/DefiningDialects/AttributesAndTypes/
- Pass Management -- https://mlir.llvm.org/docs/PassManagement/
- (Bytecode page returned 404 at fetch time:
  https://mlir.llvm.org/docs/Bytecode/ -- not quoted.)
