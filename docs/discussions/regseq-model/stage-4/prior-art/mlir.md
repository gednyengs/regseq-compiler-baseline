# MLIR -- Prior-art notes (stage-4 const-vocabulary research)

Project: LLVM/MLIR. Different model from RegseqModel: MLIR represents
constant values as **attributes** -- immutable, uniqued, parallel to
the SSA value graph -- carried by **`ConstantLike` ops** that
materialize them as SSA values. Investigated specifically for whether
the attribute-vs-op-value distinction offers anything stage 4 should
adopt or learn from.

This document cites the LLVM/MLIR repository (cloned at
`/tmp/regseq-stage4-research/llvm-project`, default branch). Citations
are `mlir/<path>:<line>` -- relative to the cloned repo's
`mlir/` subtree. All findings are file-line grounded except where I
explicitly mark a hypothesis.

## Source map

Files read in order of relevance:

- `mlir/include/mlir/IR/Attributes.h` -- the `Attribute` base wrapper
  (handle, hash by impl pointer)
- `mlir/include/mlir/IR/AttributeSupport.h` -- `AbstractAttribute`,
  `AttributeStorage`, `AttributeUniquer`
- `mlir/include/mlir/IR/BuiltinAttributes.h` and `BuiltinAttributes.td`
  -- the builtin attribute family (Integer, Float, String, Array,
  Dictionary, DenseTypedElements, DenseString, DenseResource,
  DenseArray, Sparse, Symbol, Type, Unit, Distinct)
- `mlir/include/mlir/IR/BuiltinAttributeInterfaces.td` --
  `TypedAttrInterface`, `BlobAttrInterface`, `ElementsAttrInterface`
- `mlir/lib/IR/AttributeDetail.h` -- internal storage classes
  (`DenseTypedElementsAttrStorage`, `StringAttrStorage`,
  `DistinctAttrStorage`)
- `mlir/include/mlir/IR/OpDefinition.h` -- the `ConstantLike` op trait
- `mlir/include/mlir/Dialect/Arith/IR/ArithOps.td` and
  `mlir/lib/Dialect/Arith/IR/ArithOps.cpp` -- `arith.constant`
- `mlir/include/mlir/Dialect/LLVMIR/LLVMOps.td` --
  `llvm.mlir.{constant,undef,poison,zero,addressof}` ops
- `mlir/include/mlir/Support/StorageUniquer.h` -- the underlying
  uniquer mechanics
- `mlir/docs/Dialects/Builtin.md` -- the builtin dialect documentation

---

## 1. What is the const-value type?

**MLIR has no single `Const` enum.** Constants live in two coupled
universes:

1. **The attribute universe.** An `Attribute` is an immutable, uniqued,
   immortal handle into a per-`MLIRContext` storage uniquer
   (`Attributes.h:25-33`). All constant data is encoded as some
   subclass of `Attribute`. Attributes are **not** SSA values; they do
   not appear in the dataflow graph.
2. **The op universe.** Ops marked with the **`ConstantLike` trait**
   (`OpDefinition.h:1242-1256`) have zero operands, one result, and
   are foldable to a single attribute. The op is the SSA-value
   producer; the attribute is the value's content. The `arith.constant`
   op (`ArithOps.td:222-267`) is the canonical example:
   `%1 = arith.constant 42 : i32` produces an SSA value typed `i32`
   whose constant value is `IntegerAttr(42, i32)`.

The constant vocabulary therefore lives in two complementary
catalogues: the **builtin attribute family** (Section 1.1) and the
**dialect-supplied `ConstantLike` ops** (Section 1.2).

### 1.1 Builtin attribute family

From `BuiltinAttributes.td` (the TableGen definitions are the
authoritative attribute list):

| Attribute               | TableGen line | Payload                                        | Notes                                                                                                        |
| ----------------------- | ------------- | ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `IntegerAttr`           | 691-785       | `(type: AttributeSelfType, value: APInt)`      | All integer/index constants (signless, signed, unsigned). Special-cased boolean as `BoolAttr` (line 838).    |
| `FloatAttr`             | 628-685       | `(type: AttributeSelfType, value: APFloat)`    | All float constants. APFloat preserves bit-exact representation including NaN payloads.                      |
| `StringAttr`            | 1084-1160     | `(value: StringRef, type: AttributeSelfType)`  | Strings and identifier-like values. Optional type (defaults to `NoneType`). Carries dialect-prefix routing. |
| `TypeAttr`              | 1232-1259     | `(value: Type)`                                | A type carried as a constant value (used for type parameters in attributes).                                 |
| `ArrayAttr`             | 85-149        | `(value: ArrayRef<Attribute>)`                 | Heterogeneous array of attributes. Recursive — elements are arbitrary attributes.                            |
| `DictionaryAttr`        | 531-622       | `(value: ArrayRef<NamedAttribute>)`            | Sorted-by-name dictionary of attributes; each `NamedAttribute` is `(StringAttr name, Attribute value)`.     |
| `DenseArrayAttr`        | 167-231       | `(elementType: Type, size: i64, rawData: ...)` | Flat 1-D array of primitive bytes. Specialized as `DenseI8ArrayAttr` etc. (header lines 759-765).            |
| `DenseTypedElementsAttr`| 237-388       | `(type: ShapedType, rawData: ArrayRef<char>)`  | The workhorse for tensor/vector constants. Splat-optimised: identical-element data stored as a single elt.  |
| `DenseStringElementsAttr`| 394-458      | `(type: ShapedType, value: ArrayRef<StringRef>)` | String-elements specialisation.                                                                            |
| `DenseResourceElementsAttr`| 464-525    | `(type: ShapedType, rawHandle: ResourceHandle)`| Out-of-line large blobs (e.g. weights). Indirected via a resource manager.                                  |
| `SparseElementsAttr`    | 870-1020      | `(type: ShapedType, indices: DenseIntElementsAttr, values: DenseElementsAttr)` | COO encoding for sparse tensors.                                          |
| `SymbolRefAttr`         | 1166-1226     | `(rootReference: StringAttr, nestedReferences: ArrayRef<FlatSymbolRefAttr>)`   | Named reference to an op nested under a `SymbolTable`-bearing op.        |
| `UnitAttr`              | 1265-1300     | (no payload — singleton)                       | Marker attribute; presence is the value.                                                                    |
| `DistinctAttr`          | `BuiltinAttributes.h:1029-1044` | `(referencedAttr: Attribute)`        | Wraps another attribute and forces non-structural identity (every `create` allocates fresh).                |
| `OpaqueAttr`            | 832-864       | `(dialectNamespace, attrData, type)`           | Raw string form for unregistered dialects' attributes.                                                      |
| `AffineMapAttr`         | 39-79         | `(value: AffineMap)`                           | Affine-domain payload; not directly relevant to stage 4.                                                    |
| `IntegerSetAttr`        | 791-826       | `(value: IntegerSet)`                          | Same.                                                                                                       |
| `StridedLayoutAttr`     | 1026-1077     | `(offset: i64, strides: ArrayRef<i64>)`        | Memref layout; not directly relevant.                                                                       |

**Closed list?** No. Each dialect can register additional attributes
via `Dialect::addAttributes<...>()`. So the builtin set is *the
core*, but the surface MLIR consumers see is open. Stage 4's `Const`
enum is closed by methodology rule (`README.md` §2 "no defensive
fallbacks"); MLIR's openness is a structural difference. *Hypothesis*:
the openness is justified for MLIR because IR plurality is the
design; for RegseqModel, closedness is justified because the C-side
backend has a finite, known surface.

### 1.2 `ConstantLike` ops (dialect-supplied)

`ConstantLike` is an op trait (`OpDefinition.h:1242-1256`):

```cpp
template <typename ConcreteType>
class ConstantLike : public TraitBase<ConcreteType, ConstantLike> {
public:
  static LogicalResult verifyTrait(Operation *op) {
    static_assert(ConcreteType::template hasTrait<OneResult>(),
                  "expected operation to produce one result");
    static_assert(ConcreteType::template hasTrait<ZeroOperands>(),
                  "expected operation to take zero operands");
    return success();
  }
};
```

The trait certifies "one result, zero operands, foldable to an
attribute." A grep across the bundled MLIR (`grep -rn "ConstantLike"
mlir/include/mlir/Dialect`) finds:

- **`arith.constant`** (`ArithOps.td:222`) -- the standard scalar /
  vector / tensor int-or-float constant. One mandatory attribute:
  `value: TypedAttrInterface`.
- **`llvm.mlir.constant`** (`LLVMOps.td:2233-2310`) -- LLVM dialect
  constants for scalars, vectors, strings, structs, arrays-of-structs.
  Mandatory `value` attribute whose required type depends on the
  result type.
- **`llvm.mlir.zero`** (`LLVMOps.td:2207-2231`) -- per-type zero. No
  attribute payload. Lowers to `llvm::Constant::getNullValue($_resultType)`.
  This is how MLIR represents null pointers and zero-initialized
  aggregates of LLVM types.
- **`llvm.mlir.undef`** (`LLVMOps.td:2162-2182`) -- LLVM's `undef`.
- **`llvm.mlir.poison`** (`LLVMOps.td:2184-2205`) -- LLVM's `poison`.
- **`llvm.mlir.addressof`** (`LLVMOps.td:1288-...`) -- pointer-to-
  global by name. Argument: `FlatSymbolRefAttr:$global_name`.
- **`shape.const_size`**, **`shape.const_shape`**, **`shape.const_witness`**,
  **`tosa.const`**, **`tosa.const_shape`**, **`smt.constant`**, **`func.constant`**
  -- domain-specific constant ops.

Each `ConstantLike` op typically carries **either** a single mandatory
attribute (the value) **or** no attributes (for type-determined
constants like `llvm.mlir.zero`, `llvm.mlir.undef`,
`llvm.mlir.poison`).

The MLIR `Matchers.h:52-93` library exposes `m_Constant(...)` matchers
that route constant-folding analysis through this trait, so any new
dialect can plug into the constant-folding infrastructure simply by
marking its constant op `ConstantLike`.

---

## 2. Type-tag placement for scalars

**MLIR places the type tag inline on the attribute itself, via
`TypedAttrInterface`.** From
`BuiltinAttributeInterfaces.td:19-35`:

```tablegen
def TypedAttrInterface : AttrInterface<"TypedAttr"> {
  let cppNamespace = "::mlir";
  let description = [{
    This interface is used for attributes that have a type. The type
    of an attribute is understood to represent the type of the data
    contained in the attribute and is often used as the type of a
    value with this data.
  }];
  let methods = [InterfaceMethod<
    "Get the attribute's type",
    "::mlir::Type", "getType"
  >];
}
```

`IntegerAttr`, `FloatAttr`, `StringAttr`, `OpaqueAttr`, and the
`ElementsAttrInterface` family all implement `TypedAttrInterface`.
The TableGen `parameters` clause for these attributes uses
`AttributeSelfTypeParameter<"">:$type` (e.g.,
`BuiltinAttributes.td:656` for FloatAttr,
`BuiltinAttributes.td:717` for IntegerAttr) -- the type is
**stored as a self-parameter** on the attribute storage, not derived
or shared.

The `arith.constant` op enforces type/attribute coherence at
construction (`ArithOps.cpp:238-250`):

```cpp
bool arith::ConstantOp::isBuildableWith(Attribute value, Type type) {
  // The value's type must be the same as the provided type.
  auto typedAttr = dyn_cast<TypedAttr>(value);
  if (!typedAttr || typedAttr.getType() != type)
    return false;
  // Integer values must be signless.
  if (auto intType = dyn_cast<IntegerType>(getElementTypeOrSelf(type))) {
    if (!intType.isSignless())
      return false;
  }
  return llvm::isa<IntegerAttr, FloatAttr, ElementsAttr>(value);
}
```

So in MLIR there is a **double type tag**: the SSA value's type
(`Op::result.type`) AND the attribute's type (`TypedAttr::getType()`).
The op verifies they match. *Interpretation*: the attribute's type is
intrinsic (stored on the attribute's storage); the op's result type
is derived/asserted to equal it.

**Stage-4 implication.** MLIR confirms the **inline-on-the-attribute**
design that spec.md §9.7 already uses (`Int(IntKind, value)`,
`Float(FloatKind, bits)`). It does NOT confirm or refute the question
of whether stage 4 needs an *additional* outer wrapper-level `ty`
field beyond the inline scalar tag -- MLIR's outer level is the
**op's result type**, not a wrapper field on the attribute. RegseqModel
does not have this op-vs-attribute split, so the analogy is partial.

Convergent signal across MLIR + LLVM (sibling `llvm.md` finds the type
on `Value::Type`) + charon (sibling `charon.md` -- inline AND outer):
the type tag should be **inline on the value**. The outer wrapper is
project-specific.

---

## 3. Aggregate constants

MLIR's aggregate-const story is **richer than any sibling project**.
Three coexisting strategies, optimised for different access patterns:

### 3.1 `ArrayAttr` -- heterogeneous, attr-per-element

`BuiltinAttributes.td:85-149`. Payload is `ArrayRef<Attribute>`. Each
element is an arbitrary attribute. Use cases: structs of mixed type
(an `ArrayAttr` containing `[IntegerAttr(1, i32), FloatAttr(2.0, f32)]`),
attribute-list parameters, dictionary values.

Storage: `ArrayRefParameter<"Attribute", "">:$value` -- so the
underlying storage holds a flat `ArrayRef<Attribute>` (each element
is an 8-byte attribute pointer; the elements themselves are uniqued
elsewhere). Sharing is automatic: an `ArrayAttr([a, b])` and another
`ArrayAttr([a, c])` share `a`.

### 3.2 `DenseTypedElementsAttr` -- homogeneous, raw bytes

`BuiltinAttributes.td:237-388`. Payload is
`(type: ShapedType, rawData: ArrayRef<char>)`. The shape is in the
type (`tensor<10xi32>`), the data is a contiguous byte buffer. Element
bitwidth comes from `getDenseElementBitWidth(eltType)`
(`AttributeDetail.h:35-38`).

**Splat optimisation** (`AttributeDetail.h:74-103`): if all elements
are equal, the storage holds **one element**, and iterators replicate
on read:

```cpp
static KeyTy getKey(ShapedType ty, ArrayRef<char> data) {
  if (data.empty()) return KeyTy(ty, data, 0);
  size_t storageSize = llvm::divideCeil(elementWidth, CHAR_BIT);
  if (data.size() == storageSize)
    return KeyTy(ty, data, llvm::hash_value(data));
  // Detect splat by comparing first element to all others:
  for (size_t i = storageSize, e = data.size(); i != e; i += storageSize)
    if (memcmp(data.data(), &data[i], storageSize))
      return KeyTy(ty, data, llvm::hash_combine(hashVal, data.drop_front(i)));
  // All-equal: store only the first element.
  return KeyTy(ty, firstElt, hashVal);
}
```

The `SplatElementsAttr` view (`BuiltinAttributes.h:674-683`) is a
classof-filtered subclass of `DenseElementsAttr` exposed when
`isSplat()` returns true.

### 3.3 `DenseResourceElementsAttr` -- out-of-line blob

`BuiltinAttributes.td:464-525`. For very-large constants (think:
trained-model weights), the data is held by an external "resource
manager" keyed by a `DialectResourceBlobHandle<BuiltinDialect>`
handle. The attribute carries the handle, not the data. The data is
managed separately and serialized in `dialect_resources` blocks of
the bytecode.

### 3.4 `SparseElementsAttr` -- COO encoding

`BuiltinAttributes.td:870-1020`. Payload is `(type, indices: DenseIntElementsAttr, values: DenseElementsAttr)`.
COO (coordinate-list) encoding: indices is a 2-D `[N, ndims]` tensor
of 64-bit positions; values is a 1-D `[N]` tensor of element values.
Reads at unspecified positions return zero (`getZeroValue<T>()`).

### 3.5 `DistinctAttr` -- breaks structural identity

`BuiltinAttributes.h:1029-1044` and `AttributeDetail.h:275-336`. A
distinct attribute wraps another attribute and forces every `create`
call to allocate a fresh storage instance:

```cpp
class DistinctAttributeUniquer {
public:
  template <typename T, typename... Args>
  static T get(MLIRContext *context, Args &&...args) {
    DistinctAttrStorage *storage = ...allocateStorage(context, ...);
    storage->initializeAbstractAttribute(
        AbstractAttribute::lookup(DistinctAttr::getTypeID(), context));
    return storage;
  }
};
```

Every `DistinctAttr::create(referencedAttr)` allocates a new storage
and returns a fresh handle. Used to model uniquely-named entities
(e.g. distinct memory allocations) that **structurally** look
identical but **semantically** differ.

**Stage-4 implication.** Aggregate constants in stage 4's spec.md
§9.7 are described as "aggregate field sequences (preserving order)"
and §13.2 vtable instances as "aggregate constants containing
function pointers ...". The **ID-referenced children** plan that
spec.md proposes (`Vec<ConstId>` per the conversation context) is
structurally analogous to MLIR's `ArrayAttr` (`Vec<Attribute>`) —
**children are uniqued elsewhere; the aggregate carries handles, not
inline content.** This is the right design for sharing.

The DenseTypedElements model (raw bytes + element type) is *not*
right for stage 4 because it requires homogeneous-element shaped
types -- not a fit for arbitrary Rust struct fields. But its splat
optimisation is a useful idea that maps naturally onto stage 4's
ZST and uniform-byte aggregates (e.g., zeroed buffers).

DistinctAttr is interesting prior art for **breaking content-keyed
identity when address identity matters** -- e.g., two distinct static
allocations that happen to have identical initial bytes. *Hypothesis*:
stage 4 will likely NOT need this (every Rust static has a unique
DefPath, so the relocation 4-tuple already distinguishes). Worth
revisiting if a fixture surfaces a case where structural collapse
would be wrong.

---

## 4. Relocation handling

**MLIR represents relocations differently from LLVM IR.** There is
no `ConstantExpr`-equivalent attribute. Instead:

- **`SymbolRefAttr`** (`BuiltinAttributes.td:1166-1226`) names a
  symbol by string path. Payload is
  `(rootReference: StringAttr, nestedReferences: ArrayRef<FlatSymbolRefAttr>)`.
  `FlatSymbolRefAttr` is the single-segment specialisation (in
  `BuiltinAttributes.h:864-898`), e.g. `@flat_reference`. Nested
  references resolve through nested `SymbolTable`-bearing ops.
- **`llvm.mlir.addressof`** op (`LLVMOps.td:1288-...`) is the
  ConstantLike op that **materializes a SymbolRefAttr as an SSA
  pointer value**:
  ```tablegen
  def LLVM_AddressOfOp : LLVM_Op<"mlir.addressof",
      [Pure, ConstantLike, DeclareOpInterfaceMethods<SymbolUserOpInterface>]> {
    let arguments = (ins FlatSymbolRefAttr:$global_name);
    let results = (outs LLVM_AnyPointer:$res);
    ...
  }
  ```
  Result is an LLVM dialect pointer (`!llvm.ptr`), value semantics
  is "address of the named global / function / alias."

There is **no constant-form aggregate that embeds a function-pointer
relocation** at the attribute level. To express "this struct's third
field is the address of `@foo`," MLIR uses an op-level construction:
the static initializer body materializes `@foo` via
`llvm.mlir.addressof`, and the global op's `value` attribute is set
through op-graph lowering, not via a single attribute.

`SymbolUserOpInterface` (declared on AddressOfOp) participates in
symbol-table verification: every `SymbolRefAttr` must resolve to a
visible symbol in the enclosing symbol-table hierarchy. This is the
**lookup-by-name** model.

**Cross-build identity.** `SymbolRefAttr` payload is *just the
string*. There is no DefPathHash, no canonical-args tuple, no shim
disambig. Two builds producing the same MLIR module with `@foo`
will hash the symbol's name string identically -- which is sufficient
for in-process dedup, but says nothing about whether the two "@foo"
references in two builds are the *semantically* same global. Stage 4's
4-tuple `(kind_rank, def_path_hash, args_key, shim_disambig)` is
substantially richer: rustc's name mangling for monomorphised
generics already requires the canonical-args dimension.

**Stage-4 implication.** MLIR's relocation-by-name model is **not a
direct fit for stage 4**. MLIR's symbols are user-authored names
within a known module; stage 4's relocations are post-monomorphisation
synthetic names that must agree across two builds. The `SymbolRefAttr`
shape would lose the args/shim disambig dimensions that F4-2 requires.
A `Reloc(InstanceRef)` variant carrying the explicit 4-tuple (as the
stage-4 plan has it) is right.

What **is** worth borrowing: the **name-based dedup principle**. Once
you have a stable name (in our case, the 4-tuple), use it as the
content-keyed identity, not the local handle. MLIR confirms this
practice.

---

## 5. Null pointer typing

**MLIR has no `NullAttr`.** Null is represented at the **op level**
via `llvm.mlir.zero` (`LLVMOps.td:2207-2231`):

```tablegen
def LLVM_ZeroOp
    : LLVM_Op<"mlir.zero", [Pure, ConstantLike]>,
      LLVM_Builder<"$res = llvm::Constant::getNullValue($_resultType);">
{
  let summary = "Creates a zero-initialized value of LLVM dialect type.";
  ...
  let results = (outs LLVM_Type:$res);
  ...
  let hasVerifier = 1;
  let hasFolder = 1;
}
```

The op takes no operands and no value attribute -- the **result type
is the only payload**. The lowering builds `llvm::Constant::getNullValue`
for the result type, which produces:

- For pointer types: `llvm::ConstantPointerNull` (typed null).
- For integer/float types: zero scalar.
- For aggregate types: zero-initialized aggregate.

So the type tag is on the SSA result; null is a function of the
type, not a value-side flavor. To express "typed null pointer to
`!llvm.struct<i32, f32>`", you write
`%0 = llvm.mlir.zero : !llvm.ptr<struct<i32, f32>>` (or just
`!llvm.ptr` since LLVM dialect uses opaque pointers now).

**Stage-4 implication.** MLIR's "null-as-op-result-typed" model is
analogous to LLVM IR's `ConstantPointerNull::get(PointerType*)`
(per-pointer-type) but more uniform (`mlir.zero` works for any LLVM
type). Stage-4's planned `NullPtr(TyId)` variant is closer to LLVM IR
than MLIR -- the type tag lives with the value rather than being
op-derived.

The MLIR design's elegance is that **null-of-T is computable from
T alone**, so no value-side variant is needed. Stage 4 could in
principle do the same: drop `NullPtr(TyId)` as a variant and instead
ship a `(NullPtrConstId, ty: TyId)` lookup that the canonicaliser
synthesises on demand. This trades one variant for one
canonicaliser-side rule. *Hypothesis*: keeping `NullPtr(TyId)` as a
first-class variant is simpler for stage 4 because the variant tag
provides a cheap discriminator for the `is_value_level` classification.

---

## 6. Uninit / padding

**MLIR does not distinguish uninit at the attribute level.** The
attribute universe is fully-defined: every byte in
`DenseTypedElementsAttr.rawData` is committed, every element of
`ArrayAttr` is a real attribute.

Uninit lives at the **op level**, in the LLVM dialect:

- **`llvm.mlir.undef`** (`LLVMOps.td:2162-2182`): produces an LLVM
  `undef` value. ConstantLike, no operands, no attributes.
- **`llvm.mlir.poison`** (`LLVMOps.td:2184-2205`): produces an LLVM
  `poison` value. Semantic difference per LLVM's LangRef: undef is
  arbitrary bits, poison propagates more aggressively.

Both are **ops, not attributes**. A struct constant with an undef
field cannot be expressed as a single attribute; the construction
must be op-level (build the struct via a sequence of
`llvm.insertvalue` ops starting from `llvm.mlir.undef`).

The general MLIR builtin attribute family also has **no padding-byte
distinguisher**. `DenseTypedElementsAttr.rawData` is a contiguous
byte buffer with **no per-byte init/uninit annotation**, unlike rustc's
`InitMask` (sibling `rustc.md`) or charon's
`Byte::{Uninit, Value(u8), Provenance(_, u8)}` (sibling `charon.md`).

*Hypothesis*: MLIR's design here reflects its target domain — tensor /
ML constants where padding is a layout concern but not a
canonicalisation concern. Tensor element bytes are always defined.
Aggregate-level uninit-handling is delegated to the dialect (LLVM
dialect has the `undef`/`poison` ops; the builtin dialect does not).

**Stage-4 implication.** MLIR is a NEGATIVE result for stage 4's
uninit handling: it does not provide a model. Stage 4 should not look
to MLIR for prior art on byte-level uninit. Look instead to charon's
`Byte::{Uninit, Value(u8), Provenance(_, u8)}` (closest fit) and
rustc's `InitMask` (the source-of-truth API).

The op-vs-attribute split is itself worth noting: in MLIR, undef is
an SSA-construction concern, not a constant-content concern. RegseqModel
does NOT have this split (consts are first-class IR values, not
op-results), so the MLIR design pattern is structurally inapplicable.

---

## 7. Validation phase

MLIR validation is **not phased the way RegseqModel's deserialise is.**
Instead:

### 7.1 Construction-time invariants

Attributes are built through `AttributeUniquer::get` (`AttributeSupport.h:215-265`),
which calls the storage class's static `getKey(...)` and `construct(...)`
methods. The `getKey` step canonicalises (e.g., DenseTypedElements'
splat detection at `AttributeDetail.h:74-103`). The `construct` step
allocates and copies into bump-allocator-managed memory. **After
construction, the attribute is immortal and immutable.**

Attribute-level invariants (e.g., "FloatAttr's APFloat semantics
matches its FloatType") are checked via `genVerifyDecl = 1` in the
TableGen and a `verify` static method on the storage class
(BuiltinAttributes.cpp implementations). Invariant violations at
construction return `nullptr`.

### 7.2 Op-level validation

`Operation::verify()` is called when an op is created or when the
verifier pass runs. Op verifications check:

- **`ConstantLike` trait** (`OpDefinition.h:1242-1256`): one result,
  zero operands. Static asserts at compile time.
- **Op-specific predicates**: e.g., `arith.constant` checks that
  the value attribute's type matches the result type
  (`ArithOps.cpp:238-250` `isBuildableWith`).
- **Symbol-table consistency**: ops with `SymbolUserOpInterface`
  (e.g. `llvm.mlir.addressof`) verify that their `SymbolRefAttr`
  resolves to a visible symbol.

### 7.3 No cross-table phase split

**There is no equivalent to RegseqModel's Phase A (parse) /
Phase B (validate, ordered by inter-table dependencies) /
Phase C (rebuild transient maps) split.** MLIR loads bytecode in a
streaming fashion and validates per-op as it goes. No "first all
consts, then all sigs, then all types" ordering -- attributes are
self-contained units (the type they reference is itself a uniqued
attribute, and the loader resolves the closure transitively at parse
time).

**Stage-4 implication.** MLIR is a **negative result** for stage 4's
two-phase const-validation discipline. MLIR does not need the split
because:
- Attributes carry their type inline (no late-bound type reference).
- There is no `ConstId` indirection; attribute identity IS the storage
  pointer.
- There is no "validate consts before types" ordering pressure
  because there is no tabular layout.

So MLIR's prior art neither validates nor refutes stage 4's
two-phase discipline. The justification rests on F4-2's content-
uniqueness contract for `ConstId`, which is a stage-4-specific
concern that MLIR's flat-uniqued-attributes design avoids by
construction.

What MLIR *does* validate: the **construction-time enforcement of
type-attribute coherence** is robust prior art for the inline-tag
discipline. Every IntegerAttr is built with a matched IntegerType;
constructing a mismatched pair fails at the `getKey`/`verify` step.
Stage 4's `Const::Int(IntKind, value)` construction should similarly
enforce that the `value` fits the `IntKind`'s width at construction
(panic / assert in debug; bounds-check in validate).

---

## 8. Cross-build determinism

**MLIR's storage uniquer is build-local.** From
`StorageUniquer.h:73-` (excerpted from the read):

> A utility class to get or create instances of "storage classes". ...
> For non-parametric storage classes ... For parametric storage
> classes ... must respect: Define a `KeyTy` ... Provide a method
> `bool operator==(const KeyTy &) const` ... Provide a static
> construction method ...

The uniquer uses `llvm::DenseMap` keyed on `KeyTy` (or `hashKey(KeyTy)`)
into per-`MLIRContext` storage. The hash and equality are structural,
but the *storage pointer* is the in-memory canonical identity --
two `MLIRContext` instances will allocate different storage for
identical attributes.

**Bytecode is the cross-build representation, not pointers.** The MLIR
bytecode format (`mlir/include/mlir/IR/BuiltinDialectBytecode.td` and
`mlir/lib/Bytecode/`) serializes attributes structurally. Loading
bytecode in a fresh `MLIRContext` produces fresh storage pointers
that are structurally equal to (but not pointer-equal to) the
originals.

For stage 4's question — "do two builds with equivalent inputs
produce equal `ConstId`s for equivalent values?" — MLIR's answer is
**no by default** (`ConstId`-equivalent storage pointers differ
across builds), with the recovery path being **bytecode-level
structural equality**, which is necessary-but-insufficient: bytecode
encodes the per-context interner numbering, which is allocation-order
dependent. *Hypothesis*: two MLIR builds could produce bytecode that
agrees on attribute *content* but disagrees on attribute *numbering*
-- I have not verified this against the bytecode encoder source, but
it is the expected behaviour given the storage uniquer design.

**`DistinctAttr`** (`AttributeDetail.h:288-310`) is the explicit
escape hatch for **non-structural identity**: every `create` call
allocates a fresh storage instance. The address of the storage is
the unique identifier. The pretty printer assigns deterministic
numeric IDs at print time, so output is reproducible -- but the
in-memory identity is allocation-order-dependent.

**Stage-4 implication.** MLIR is a **negative result** for stage 4's
cross-build determinism goal. The closest project that achieves
cross-build content-keyed identity for non-trivial relocation-bearing
constants is rustc itself (sibling `rustc.md` finding 8: `HashStable
for AllocId` resolves to the target via `tcx.try_get_global_alloc()`
and hashes the *target*, deliberately discarding the local handle).

MLIR's lesson: **structural in-memory uniquing is the easy half**.
Cross-build is a separate problem that requires a serialisation
format whose canonical encoding is allocation-order-independent.
Stage 4's spec.md §9.7 plan -- "ConstIdentityKey is computed from the
const-eval result, not from any allocation-order rustc handle" --
is essentially RegseqModel's solution to this problem, and it is
NOT something MLIR's design demonstrates how to do.

---

## Stage-4 applicability summary

Mapping MLIR's design choices to stage-4 D-4-1 sub-decisions:

| D-4-1 sub-decision                     | MLIR position                                                              | Stage-4 takeaway                                                                                                                                |
| -------------------------------------- | -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Closed enum vs open hierarchy          | Open (each dialect adds attributes; even the "builtin" set is non-closed)  | DIVERGE. Stage 4 closes per the no-fallbacks methodology rule. MLIR's openness is not a fit.                                                    |
| Variant set                            | ~17 builtin attributes + dialect-supplied                                  | Adopt: Integer, Float, String, Array, Sparse, Symbol, Type, Unit. Skip: Affine, IntegerSet, Strided, Opaque, Distinct (no analog needed).         |
| Type tag inline                        | Yes -- `TypedAttrInterface` mandates `getType()` per attribute              | CONFIRM Stage 4's inline tag plan (`Int(IntKind, value)`, `Float(FloatKind, bits)`).                                                            |
| Outer wrapper-level `ty` field         | Op result type plays this role; no attribute-side wrapper                  | INCONCLUSIVE for stage 4 (stage 4 has no op-vs-attribute split).                                                                                  |
| Aggregate children: IDs vs inline      | `ArrayAttr` uses Vec<Attribute> = Vec<storage-pointer> -> implicit IDs    | CONFIRM stage 4's `Vec<ConstId>` plan -- this is the right model for sharing.                                                                    |
| Aggregate: byte-buffer vs structured   | Both: `DenseTypedElementsAttr` (bytes) + `ArrayAttr` (structured)         | Stage 4 needs the structured path (Rust struct fields are heterogeneous); the byte-buffer is not a fit.                                          |
| Splat optimisation                     | First-class for `DenseTypedElementsAttr`                                   | Adopt for stage 4's ZST and uniform-byte aggregate cases. ZST is already a candidate variant.                                                    |
| Relocation form                        | `SymbolRefAttr` (string-path) + `llvm.mlir.addressof` op                   | DIVERGE. Stage 4's 4-tuple `(kind_rank, def_path_hash, args_key, shim_disambig)` is structurally richer.                                          |
| Null-pointer variant                   | None -- `llvm.mlir.zero` op materializes per-type                          | DIVERGE. Stage 4's `NullPtr(TyId)` is more granular and aligns with LLVM's `ConstantPointerNull`.                                              |
| Uninit / padding                       | `llvm.mlir.undef`/`llvm.mlir.poison` ops; no byte-level uninit at attribute | DIVERGE. Stage 4 needs byte-level uninit -- look to charon's `Byte` design and rustc's `InitMask`, NOT MLIR.                                     |
| Validation phase                       | Construction-time per-attribute; no two-phase split                        | NOT APPLICABLE. MLIR avoids the cycle by self-containing types in attributes; stage 4 has the cycle and must resolve it.                          |
| Cross-build determinism                | NOT solved -- in-memory storage-pointer identity, build-local              | NOT a model. Look to rustc's `HashStable for AllocId` for the only project that solves this.                                                     |

---

## Open questions / hypotheses

1. **Should stage 4 adopt MLIR's `DistinctAttr` pattern for any
   variant?** *Hypothesis*: probably not. Stage-4 relocations carry
   the def_path_hash + args_key, which already disambiguates two
   structurally-identical-looking instance refs that point to
   different items. MLIR's DistinctAttr exists because MLIR's
   identity is the storage pointer; stage 4's identity is the
   content-keyed `ConstIdentityKey`, which makes
   structural-collapse-when-it-shouldn't a non-issue.

2. **The op-vs-attribute split as a future RegseqModel direction?**
   MLIR's separation of "constant content (attribute)" from "constant
   materialisation (op)" enables interesting things: re-typing the
   same value as a different SSA type, lazy materialisation across
   regions, dialect-extension. Stage 4 is committed to the value-as-
   IR-node model (`Const` is a first-class IR shape, not an op
   payload), but the split could be revisited if a future stage finds
   it productive. *Hypothesis*: not productive for stage 4; the
   Rust-to-C consumer chain is too narrow to benefit from the
   plurality MLIR's design serves.

3. **Splat optimisation for stage 4's aggregate variant?**
   *Hypothesis*: yes, low cost. A vtable's vacant slots, a static
   array's zero-initialised tail, and a struct's all-default-fields
   are all natural splat candidates. The implementation is a few
   lines: at intern-time, check if all `Vec<ConstId>` children are
   pointer-equal; if so, store the single child + length. Defer the
   decision to stage-4 implementation time.

4. **Should stage 4's ZST variant be type-parameterised?** MLIR's
   `llvm.mlir.zero` is type-derived: `mlir.zero : !T` for any T.
   Stage 4 has spec.md pinning `ZST` as a single canonical
   representative. **Question**: is "the ZST value" enough, or do we
   need `ZST(TyId)` to discriminate two ZST types? *Hypothesis*: the
   type tag lives on the consumer (an `InstanceKind::Const { value:
   ConstId, ty: TyId }`), so the `ConstId` doesn't need the type.
   Confirm at framing time.

5. **Symbol-table-style validation for stage-4 relocations?** MLIR's
   `SymbolUserOpInterface` verifies that every `SymbolRefAttr`
   resolves. Stage 4 should similarly verify that every
   `Reloc(InstanceRef)` resolves to an `InstanceId` in
   `model.instances` (after F4-2 sort). This is a specific Phase B
   check to add to spec.md §3.3 step 4 (ConstInterner validation).
   Currently the spec language is "content uniqueness (no duplicate
   canonical values)"; should be expanded to "content uniqueness +
   reachability of every Reloc target." *Hypothesis*: this is
   already implied by the Phase B "resolves" pattern used elsewhere,
   but spec.md should make it explicit.

---

## Summary

MLIR represents constant values as **uniqued, immutable Attributes**,
materialized as SSA values by **`ConstantLike` ops** (`arith.constant`,
`llvm.mlir.{constant,zero,undef,poison,addressof}`). Type tags are
inline on each typed attribute via `TypedAttrInterface`; aggregate
constants split into multiple shape-optimised forms (`ArrayAttr` for
heterogeneous, `DenseTypedElementsAttr` for raw-byte-homogeneous,
`DenseResourceElementsAttr` for out-of-line blobs, `SparseElementsAttr`
for COO sparse).

For stage 4's D-4-1 framing, MLIR contributes:

- **Strong convergent signal** for inline type tags on scalar variants.
- **Confirmation** that aggregate children should be ID-referenced
  (Vec<ConstId>) for sharing, mirroring MLIR's Vec<Attribute>.
- **Splat-optimisation precedent** for stage-4 ZST and uniform-byte
  aggregates.
- **Negative results** on uninit handling (op-level, not attribute-
  level — not portable to stage 4), validation phase split (not
  needed in MLIR's design — does not validate or refute stage 4's
  approach), and cross-build determinism (NOT solved by MLIR — in-
  memory pointer identity only).
- **Structurally distinct** relocation model (string-by-name +
  symbol-table) that diverges from stage 4's content-keyed 4-tuple
  plan; not a fit.

The strongest takeaway: MLIR's **construction-time enforcement of
type-attribute coherence** (a misconstructed `IntegerAttr` fails at
intern) is a discipline stage 4 should mirror at the
`ConstInterner::intern` boundary. Validation is not deferred to a
later phase when the invariant can be checked at construction.
