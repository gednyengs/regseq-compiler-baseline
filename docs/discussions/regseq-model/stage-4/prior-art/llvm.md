# Prior art: LLVM constants

Stage-4 primary-source research on LLVM IR's `Constant` hierarchy.
Guiding meta-question: *how are constant values represented in
LLVM IR, and what shape does each variant carry?*

LLVM is the gold-standard "const value as first-class IR node"
design. It carries the deepest typed-null + relocation-as-
expression vocabulary of any C-family compiler, and its
undef/poison semantics are the field's reference for
distinguishing "unspecified bits" from "value-from-erroneous-
operation."

Evidence base in this document is the `main` checkout pulled to
`/tmp/regseq-stage4-research/llvm-project/`. File-paths and
line-numbers refer to that checkout. Where a symbol could be
ported across files (e.g., `Constant::getNullValue` is defined in
`Constants.cpp` but declared in `Constant.h`), the citation
points at the definition site.

## Source map

| File                                                  | Used for                                                                                       |
| ----------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `llvm/include/llvm/IR/Constant.h`                     | `Constant` base class API (266 lines).                                                         |
| `llvm/include/llvm/IR/Constants.h`                    | All leaf-class declarations (1690 lines).                                                      |
| `llvm/lib/IR/Constants.cpp`                           | Leaf-class definitions, `getNullValue` polymorphic factory, interner glue (3743 lines).        |
| `llvm/lib/IR/ConstantsContext.h`                      | `ConstantUniqueMap`, `ConstantAggrKeyType`, `ConstantExprKeyType`, `ConstantPtrAuthKeyType`.   |
| `llvm/lib/IR/LLVMContextImpl.h`                       | Per-`LLVMContext` interner storage maps.                                                       |
| `llvm/include/llvm/IR/GlobalValue.h`                  | `GlobalValue` linkage / visibility / DSO-locality fields (689 lines).                          |
| `llvm/include/llvm/IR/ConstantFold.h`                 | DataLayout-independent constant-folding interface (91 lines).                                  |
| `llvm/lib/IR/Verifier.cpp`                            | `visitConstantExprsRecursively`, `visitConstantExpr`, `visitConstantPtrAuth`.                  |
| `llvm/docs/LangRef.rst` §Constants, §Undefined Values, §Poison Values, §Constant Expressions, §Pointer Authentication Constants | LangRef semantic docs.                                                                         |

Out of scope: SelectionDAG node-level constants, optimisation
passes, IR analyses. The `ConstantFolder` interface is covered
only as the const-folding hook from constant construction.

## 1. The const-value type: the `Constant` hierarchy

LLVM does not have a single tagged-union `Const` enum. Constants
are subclasses of `Constant` (`Constant.h:43`), itself a subclass
of `User`, which is a subclass of `Value`. Subclass identity is
encoded in `Value::getValueID()` (a runtime tag); RTTI uses
`classof` predicates over `getValueID()` ranges (`Constant.h:173-176`).

The LangRef-visible classes (`Constants.h`):

### 1.1 `ConstantData` — leaf classes with no operands

`ConstantData` (`Constants.h:56-81`) is the abstract base for
constants whose data is intrinsic (no operand list). The
`User`-side allocation marker is `IntrusiveOperandsAllocMarker{0}`
(`Constants.h:57`). Comments at `Constants.h:48-55` are explicit:
*"These constants have no operands; they represent their data
directly. Since they can be in use by unrelated modules (and are
never based on GlobalValues), it never makes sense to RAUW them.
These do not have use lists. It is illegal to inspect the uses."*

| Class                   | Where                       | Payload                                                                                                                     |
| ----------------------- | --------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `ConstantInt`           | `Constants.h:87-277`        | `APInt Val` (`Constants.h:91`); type is the inherited `Type *` (an `IntegerType`).                                          |
| `ConstantByte`          | `Constants.h:281-415`       | `APInt Val`; backing type is `ByteType` (a recently-added type kind).                                                       |
| `ConstantFP`            | `Constants.h:420-497`       | `APFloat Val` (`Constants.h:424`); type is one of the float `Type` flavors.                                                 |
| `ConstantAggregateZero` | `Constants.h:502-538`       | No payload — type alone determines the value (any aggregate / vector zero).                                                 |
| `ConstantPointerNull`   | `Constants.h:701-725`       | No payload — `PointerType *Ty` is the value (typed null).                                                                   |
| `ConstantTokenNone`     | `Constants.h:1016-1034`     | No payload — empty token of `TokenType`.                                                                                    |
| `ConstantTargetNone`    | `Constants.h:1037-1061`     | No payload — default initialiser for `TargetExtType` if it has the `HasZeroInit` property (`Constants.cpp:2020-2022`).      |
| `ConstantDataSequential`| `Constants.h:736-838`       | `const char *DataElements` pointing into a `StringMap`-uniqued raw byte buffer; `unique_ptr<ConstantDataSequential> Next` for type-collisions on the same byte-pattern (`Constants.h:742-748`). |
| `ConstantDataArray`     | `Constants.h:846-924`       | Inherits CDS (subclass-tag is `ConstantDataArrayVal`).                                                                      |
| `ConstantDataVector`    | `Constants.h:932-1011`      | Inherits CDS, plus mutable `IsSplatSet`/`IsSplat` cache (`Constants.h:939-940`).                                            |
| `UndefValue`            | `Constants.h:1606-1645`     | No payload — type-only.                                                                                                     |
| `PoisonValue`           | `Constants.h:1654-1686`     | No payload — type-only. Subclass of `UndefValue`; `classof(V)` admits `UndefValueVal` AND `PoisonValueVal` (`Constants.h:1641-1644`). |

### 1.2 `ConstantAggregate` — operand-based aggregates

`ConstantAggregate` (`Constants.h:551-565`) is the abstract base
for aggregates whose elements are stored as `User` operands —
i.e., children are themselves `Constant *` and participate in the
use-def graph. Three concrete leaves:

| Class                | Where                  | Backing type                |
| -------------------- | ---------------------- | --------------------------- |
| `ConstantArray`      | `Constants.h:576-603`  | `ArrayType` of N elements.  |
| `ConstantStruct`     | `Constants.h:608-655`  | `StructType` (named or anon, with optional `Packed`).  |
| `ConstantVector`     | `Constants.h:660-696`  | `FixedVectorType` of N elements. |

Important canonicalisation rule: `ConstantArray::getImpl`
(`Constants.cpp:1470-1502`) and `ConstantStruct::get`
(`Constants.cpp:1531-1564`) both collapse all-zero / all-undef /
all-poison input arrays to `ConstantAggregateZero` /
`UndefValue` / `PoisonValue` *before* falling through to operand-
based storage. `ConstantArray::getImpl` additionally checks
`ConstantDataSequential::isElementTypeCompatible` and re-routes to
the data-array path when the element type is "simple" (small
integer / byte / half / bfloat / float / double — see
`Constants.h:875` for the canonical compatible-element-type list).
This means **the same value (e.g., `[i32 1, i32 2, i32 3]`) is
represented as `ConstantDataArray` rather than `ConstantArray`,
because every element is `ConstantInt`-typed**. Operand-based
`ConstantArray` is the residual case (the aggregate contains at
least one element that isn't a "simple" leaf, e.g., a
`GlobalValue`-pointer or a `ConstantExpr`).

### 1.3 Standalone Constant subclasses (not in `ConstantData` or `ConstantAggregate`)

| Class                | Where                       | What it represents                                                                                  |
| -------------------- | --------------------------- | --------------------------------------------------------------------------------------------------- |
| `BlockAddress`       | `Constants.h:1065-1107`     | Address of a basic block. 1 operand (the block). LangRef `:5349-5374`: only valid as `indirectbr` operand or for null-comparison; otherwise UB. |
| `DSOLocalEquivalent` | `Constants.h:1118-1147`     | Wrapper indicating a DSO-local stub for a `GlobalValue`. 1 operand. LangRef `:5378-5411`: PLT-stub-equivalent; ELF-only. |
| `NoCFIValue`         | `Constants.h:1157-1189`     | "Don't replace this function reference with a CFI jump table" wrapper. 1 operand. LangRef `:5413-5425`. |
| `ConstantPtrAuth`    | `Constants.h:1198-1276`     | ARM-style ptrauth-signed pointer. 5 operands: pointer, key (`i32`), discriminator (`i64`), addr-disc, deactivation-symbol. LangRef `:5429-5464`. |
| `ConstantExpr`       | `Constants.h:1291-1588`     | Constant computed from other constants via Instruction opcodes (PtrToInt / IntToPtr / BitCast / GEP / Trunc / etc.). |
| `GlobalValue` (and subclasses `Function`, `GlobalVariable`, `GlobalAlias`, `GlobalIFunc`) | `GlobalValue.h:49-` | **A global symbol is itself a `Constant`.** Its value is "the address of this symbol after linking." See §1.4. |

### 1.4 `GlobalValue` is a `Constant`

This is one of LLVM's load-bearing design choices and worth
calling out explicitly. From `GlobalValue.h:49`:

```cpp
class GlobalValue : public Constant {
```

So `Function *F` is-a `Constant *`, and `GlobalVariable *GV` is-a
`Constant *`. Code that takes a function's address simply uses
the `Function *` directly as a constant operand — no separate
"address-of-function" wrapper is needed. The function's type as a
constant value is `ptr` (after the opaque-pointer migration; was
`PointerType` of the function's signature pre-opaque-pointer).

`GlobalValue` carries `LinkageTypes` (`GlobalValue.h:52-64`:
External, AvailableExternally, LinkOnceAny/ODR, WeakAny/ODR,
Appending, Internal, Private, ExternalWeak, Common),
`VisibilityTypes` (`GlobalValue.h:67-71`: Default / Hidden /
Protected), DLL storage class, thread-local kind, `IsDSOLocal`
bit, partition flag (`GlobalValue.h:80-124`).

Implication for §4 (relocations): the operand graph naturally
expresses "this aggregate's element 3 points to function `@foo`"
by storing `Function *foo` as the element-3 operand. The
relocation IS the operand pointer. Identity is by C++ pointer
equality at the host level, by `getName()` at the linker level.

## 2. Type-tag placement

**Externalised on the inherited `Value::Type *`, not inline in
the variant.** Every Constant has a `Type *` via Value's
constructor; the leaf classes do not duplicate this.

`ConstantInt::Val` is `APInt` (`Constants.h:91`). `APInt` carries
its bit-width but NOT its signedness or "is this i32 or
SomePrimitive" semantic — both come from `getType()`. The
constructor asserts width match: `Constants.cpp:901-906`:

```cpp
ConstantInt::ConstantInt(Type *Ty, const APInt &V)
    : ConstantData(Ty, ConstantIntVal), Val(V) {
  assert(V.getBitWidth() ==
             cast<IntegerType>(Ty->getScalarType())->getBitWidth() &&
         "Invalid constant for type");
}
```

So `i32 5` and `i64 5` are different `Constant *`s (different
types ⇒ different APIntegers in the bit-width sense) AND different
intern slots. A user calling
`ConstantInt::get(Type *Ty, uint64_t V)` (`Constants.cpp:983-993`)
indirects through `cast<IntegerType>(Ty->getScalarType())` to
ask for the right APInt width.

`ConstantInt::getIntegerType()` (`Constants.h:198-200`) is a
convenience downcast.

`ConstantFP::Val` is `APFloat`. `APFloat` carries its
*semantics* (IEEE-754 family — `IEEEhalf`, `IEEEsingle`,
`IEEEdouble`, `IEEEquad`, `x87DoubleExtended`, `PPCDoubleDouble`),
which IS effectively the type tag at the value level; the LLVM
`Type *Ty` redundantly re-encodes this for type-system uniformity.

`ConstantPointerNull` has no payload at all — `PointerType *Ty`
*is* the value. Two null pointers in different address spaces
(or different pointee types pre-opaque-pointer) are distinct
`Constant *`s with distinct intern slots.

`UndefValue` and `PoisonValue` are similar — purely type-keyed,
no payload.

**Stage-4 contrast.** Stage 4's working assumption (memory.md)
is to inline the type tag in scalar variants (`Int(IntKind,
value)`, `Uint(UintKind, value)`, `Float(FloatKind, bits)`).
LLVM externalises. Both conventions fit equally well with
content-keyed identity (LLVM's `IntConstants` map keys by
`(Type, APInt)` via the per-Type pImpl slot — see §8). The
divergence is essentially ergonomic: LLVM's `Type *` is a stable
pointer and very cheap to get; Regseq's `TyId` is a `u32` and
inlining the tag into the variant avoids a `(Const, Ty)` pair
when the type is implied by the variant tag. Stage-4's choice is
not a soundness divergence; both schemes content-canonicalise
the same equivalence classes.

## 3. Aggregate constants

Two distinct storage paths, with canonicalisation rules
collapsing as densely as possible.

### 3.1 The split: `ConstantDataSequential` vs `ConstantAggregate`

`ConstantDataArray` (CDA) and `ConstantDataVector` (CDV) store
their elements as a raw byte buffer keyed by the bytes
themselves. The base class commentary at `Constants.h:728-735`:

> A vector or array constant whose element type is a simple
> 1/2/4/8-byte integer/byte or half/bfloat/float/double, and
> whose elements are just simple data values (i.e.
> ConstantInt/ConstantByte/ConstantFP). This Constant node has
> no operands because it stores all of the elements of the
> constant as densely packed data, instead of as Value*'s.

This is purely a memory-efficiency optimisation. A
`ConstantArray` of `[1024 x i32]` filled with consecutive integers
would otherwise need 1024 `Use` slots and 1024 `Constant *`
operand pointers; `ConstantDataArray` stores a 4096-byte buffer.

`ConstantArray`, `ConstantStruct`, `ConstantVector` inherit from
`ConstantAggregate`, which inherits the standard `User` machinery
— elements are stored as `Use *Operands`, each holding a
`Constant *`. This path is taken when the elements are not all
"simple data" (e.g., aggregate of pointers to globals, mix of
constants and `ConstantExpr`s, or struct of differently-shaped
fields).

The split is invisible to consumers: `Constant::getAggregateElement`
(`Constants.cpp:462-510`) hides it, returning the right
`Constant *` whether it came from `ConstantAggregate::getOperand`
or `ConstantDataSequential::getElementAsConstant`.

### 3.2 Canonicalisation collapses

`ConstantArray::getImpl` (`Constants.cpp:1470-1502`):

```cpp
// Empty arrays are canonicalized to ConstantAggregateZero.
if (V.empty()) return ConstantAggregateZero::get(Ty);

[...]

// If this is an all-zero array, return a ConstantAggregateZero object. If
// all undef, return an UndefValue, if "all simple", then return a
// ConstantDataArray.
Constant *C = V[0];
if (isa<PoisonValue>(C) && rangeOnlyContains(V.begin(), V.end(), C))
  return PoisonValue::get(Ty);

if (isa<UndefValue>(C) && rangeOnlyContains(V.begin(), V.end(), C))
  return UndefValue::get(Ty);

if (C->isNullValue() && rangeOnlyContains(V.begin(), V.end(), C))
  return ConstantAggregateZero::get(Ty);

if (ConstantDataSequential::isElementTypeCompatible(C->getType()))
  return getSequenceIfElementsMatch<ConstantDataArray>(C, V);

// Otherwise, we really do want to create a ConstantArray.
return nullptr;
```

So the `ConstantArray` path is reached only when elements are
neither all-zero nor all-undef nor all-poison, AND the element
type is non-simple. Cf. `ConstantStruct::get`
(`Constants.cpp:1531-1564`) for the analogous struct path (no
CDS-style data-storage path because struct elements may differ
in type).

### 3.3 `ConstantAggregateZero` and `UndefValue`/`PoisonValue` for aggregates

`ConstantAggregateZero` (CAZ) is a single Constant per Type; the
Type itself encodes the aggregate shape. So an all-zero
`{i32, i64, ptr}` struct has one CAZ, one all-zero
`[1024 x i8]` array has one CAZ, etc.

`UndefValue` and `PoisonValue` similarly have one entry per Type
(`Constants.cpp:2036-2042` and PV elsewhere). For aggregate types
(struct / array / vector), CAZ/UndefValue/PoisonValue answer
element-access queries by computing the right per-element value
on demand — `CAZ::getElementValue`, `UndefValue::getElementValue`,
`PoisonValue::getElementValue` (`Constants.h:521-530`,
`Constants.h:1632-1635`, `Constants.h:1675-1678`). The element
values are themselves singletons (CAZ-of-element-type,
UndefValue-of-element-type, PoisonValue-of-element-type).

**Stage-4 implication.** LLVM's CAZ pattern is stage-4-relevant
for the ZST variant (single canonical representative for any
zero-sized type) and for vtable-instance vacant slots. The
`ConstantAggregateZero` design (no payload, type-only key) is a
direct precedent for stage-4's `ZST` variant (`memory.md` §2,
sub-decision 2).

### 3.4 Aggregate elements: sequence-of-IDs, not byte buffer (in the operand-based path)

For `ConstantArray`/`ConstantStruct`/`ConstantVector`, the
elements are *interned* `Constant *`s (their addresses are
canonical because constants are interned). So an aggregate of
relocations is structurally
`{ ConstantArray-with-operands [ Function*foo, Function*bar, Function*baz ] }`.

The interner key is `ConstantAggrKeyType<C>::Operands`
(`ConstantsContext.h:297-335`), an `ArrayRef<Constant *>` —
elements are canonical pointers. Equality is operand-pointer
equality (`ConstantsContext.h:314-325`); hashing combines operand
pointers (`ConstantsContext.h:327`).

For `ConstantDataSequential`, the elements are densely packed
into a `StringRef`-keyed buffer at `LLVMContextImpl::CDSConstants`
(`LLVMContextImpl.h:1708`), with type collisions resolved via a
`unique_ptr<ConstantDataSequential> Next` chain
(`Constants.h:744-748` plus impl `Constants.cpp:3091-3115`).
**The byte-pattern is the equivalence class**; type is secondary
(linked-list lookup within a bucket).

## 4. Relocation handling

Relocations — "this constant's bits resolve at link time to the
address of `@foo`" — are first-class in LLVM, expressed via:

1. **`GlobalValue *` directly**, used as a constant operand
   (since GlobalValue IS a Constant per §1.4). Most common form.
2. **`ConstantExpr`** wrapping the GlobalValue with address
   arithmetic (`getelementptr`, `ptrtoint`, `inttoptr`,
   `bitcast`, `add`, `sub`, `xor`).
3. **`BlockAddress`**, **`DSOLocalEquivalent`**, **`NoCFIValue`**,
   **`ConstantPtrAuth`** as specialized relocation flavours.

### 4.1 Aggregate-with-relocation: example walk-through

LangRef gives the canonical worked example at `:5086-5090`:

```llvm
@X = global i32 17
@Y = global i32 42
@Z = global [2 x ptr] [ ptr @X, ptr @Y ]
```

Here `@Z`'s initialiser is a `ConstantArray` whose operand 0 is
`@X` (a `GlobalVariable *` cast to `Constant *`) and operand 1
is `@Y`. The aggregate is of type `[2 x ptr]`. After interning,
each operand is the *unique* GlobalValue pointer. `@Z`'s
relocation set is the union of relocations under its operand
graph; `Constant::needsRelocation()` (`Constants.cpp:692-694`)
computes this by walking operands recursively
(`Constants.cpp:696-745`).

### 4.2 `Constant::getRelocationInfo()` implementation

`Constants.cpp:696-745` shows the full relocation classifier:

```cpp
Constant::PossibleRelocationsTy Constant::getRelocationInfo() const {
  if (isa<GlobalValue>(this))
    return GlobalRelocation; // Global reference.

  if (const BlockAddress *BA = dyn_cast<BlockAddress>(this))
    return BA->getFunction()->getRelocationInfo();

  if (const ConstantExpr *CE = dyn_cast<ConstantExpr>(this)) {
    if (CE->getOpcode() == Instruction::Sub) {
      [...]
      // Relative pointers do not need to be dynamically relocated.
      if (auto *RHSGV =
              dyn_cast<GlobalValue>(RHSOp0->stripInBoundsConstantOffsets())) {
        auto *LHS = LHSOp0->stripInBoundsConstantOffsets();
        if (auto *LHSGV = dyn_cast<GlobalValue>(LHS)) {
          if (LHSGV->isDSOLocal() && RHSGV->isDSOLocal())
            return LocalRelocation;
        } else if (isa<DSOLocalEquivalent>(LHS)) {
          if (RHSGV->isDSOLocal())
            return LocalRelocation;
        }
      }
    }
  }

  PossibleRelocationsTy Result = NoRelocation;
  for (const Value *Op : operands())
    Result = std::max(cast<Constant>(Op)->getRelocationInfo(), Result);

  return Result;
}
```

Three classes (`Constant.h:244-256`):
- `NoRelocation` — pure data.
- `LocalRelocation` — static-linker-resolvable (DSO-local).
- `GlobalRelocation` — dynamic-linker-resolvable.

The ConstantExpr Sub-of-PtrToInt-of-Globals special case detects
"relative pointer" idioms (e.g., `(@dst - @base) as iN`) common
in vtables and avoids classifying them as needing dynamic
relocations when both endpoints are DSO-local.

### 4.3 ConstantExpr opcodes that compose relocations

LangRef `:5468-5516` enumerates the ConstantExpr opcodes:
- Casts: `trunc`, `ptrtoint`, `ptrtoaddr`, `inttoptr`, `bitcast`,
  `addrspacecast`.
- GEP: `getelementptr [inbounds] (TY, CSTPTR, IDX0, IDX1, ...)`.
- Vector-shape: `extractelement`, `insertelement`,
  `shufflevector`.
- Arithmetic: `add`, `sub`, `xor` only.

Note: the supported binop set is *very small*. Most arithmetic
ConstantExprs were removed in recent versions; folding now
materialises the result rather than carrying the expression.
`ConstantExpr::isDesirableBinOp` and `isSupportedBinOp`
(`Constants.h:1559-1563`) gate which opcodes survive as
expressions.

The ConstantExpr classes themselves (cast/binary/extract/insert/
shuffle/GEP) are subclasses of `ConstantExpr` defined in
`ConstantsContext.h:236-262` (template `OperandTraits`
specialisations); concrete classes are `CastConstantExpr`,
`BinaryConstantExpr`, etc.

Interning key is `ConstantExprKeyType`
(`ConstantsContext.h:389-507`): `Opcode` + `SubclassOptionalData`
+ `Ops: ArrayRef<Constant *>` + optional `ShuffleMask` (for
shufflevector) + optional `ExplicitTy` (for GEP) + optional
`InRange` (also for GEP). Hash combines all fields (`getHash` at
`ConstantsContext.h:480-483`).

### 4.4 Global linkage and `isDSOLocal`

`GlobalValue::isDSOLocal()` (`GlobalValue.h:307`) and the linkage
enum (`GlobalValue.h:52-64`) determine relocation class. Linkage
choices interact with cross-DSO assumptions: `LinkOnceAny`,
`WeakAny`, `Common`, `ExternalWeak`, `AvailableExternally` all
imply the symbol may be redefined across DSOs (and hence is not
DSO-local for relocation purposes); `Internal`, `Private`, and
explicit `dso_local`-marked Externals are.

**Stage-4 implication.** The Regseq build-equivalence quad
constrains us to a single-DSO model (we control the bridge's
output crate set). Stage-4's relocation key
`(kind_rank, def_path_hash, args_key, shim_disambig)`
(spec.md §9.7) is the equivalent of LLVM's `GlobalValue *` minus
the linkage / visibility metadata. We don't need DSO-locality
because we don't have a runtime linker model — every relocation
is static. The encoding is structurally simpler, but the
*intent* matches LLVM's "the operand IS the global symbol."

### 4.5 ConstantPtrAuth as a special relocation flavour

`ConstantPtrAuth` (`Constants.h:1198-1276`) bundles a pointer
constant with ARM-style ptrauth signing parameters: pointer (the
target), key (i32), discriminator (i64), address discriminator
(pointer or null), deactivation symbol (pointer or null).
LangRef `:5429-5464` documents the semantics.

The verifier (`Verifier.cpp:2832-2855`) enforces:
- Pointer operand must have pointer type.
- Self-type must equal pointer-operand type.
- Key must be i32.
- Discriminator must be i64.
- Address discriminator must be pointer.
- Deactivation symbol must be a GlobalValue or null.

Stage 4 has no equivalent yet; if Regseq targets Apple Arm64e
in future, ptrauth-bearing function pointers in vtables would
benefit from a similar wrapper variant.

## 5. Null pointer typing

`ConstantPointerNull` (`Constants.h:701-725`) is a typed null:
one entry per `PointerType` per `LLVMContext`.

`ConstantPointerNull::get(PointerType *T)` (`Constants.cpp:2003-2010`):

```cpp
ConstantPointerNull *ConstantPointerNull::get(PointerType *Ty) {
  std::unique_ptr<ConstantPointerNull> &Entry =
      Ty->getContext().pImpl->CPNConstants[Ty];
  if (!Entry)
    Entry.reset(new ConstantPointerNull(Ty));

  return Entry.get();
}
```

The interner is a `DenseMap<PointerType *, std::unique_ptr<ConstantPointerNull>>`
at `LLVMContextImpl.h:1700`. PointerType identity is cross-cutting
(same context-uniqued pointer-type ⇒ same null entry). Pre-opaque-
pointer, distinct pointee types gave distinct nulls; post-opaque-
pointer, the address-space is the only differentiator (modulo
the `PointerType` itself being interned).

Compare `Constant::getNullValue(Type *Ty)` (`Constants.cpp:390-420`),
the **type-polymorphic null factory**:

```cpp
Constant *Constant::getNullValue(Type *Ty) {
  switch (Ty->getTypeID()) {
  case Type::ByteTyID:
    return ConstantByte::get(Ty, 0);
  case Type::IntegerTyID:
    return ConstantInt::get(Ty, 0);
  case Type::HalfTyID:
  case Type::BFloatTyID:
  case Type::FloatTyID:
  case Type::DoubleTyID:
  [...]
    return ConstantFP::get(Ty->getContext(),
                           APFloat::getZero(Ty->getFltSemantics()));
  case Type::PointerTyID:
    return ConstantPointerNull::get(cast<PointerType>(Ty));
  case Type::StructTyID:
  case Type::ArrayTyID:
  case Type::FixedVectorTyID:
  case Type::ScalableVectorTyID:
    return ConstantAggregateZero::get(Ty);
  case Type::TokenTyID:
    return ConstantTokenNone::get(Ty->getContext());
  case Type::TargetExtTyID:
    return ConstantTargetNone::get(cast<TargetExtType>(Ty));
  default:
    llvm_unreachable("Cannot create a null constant of that type!");
  }
}
```

So "null of type T" dispatches to: zero of integer/byte/float for
scalars; `ConstantPointerNull` for pointers; `ConstantAggregateZero`
for aggregates; `ConstantTokenNone` for the token type;
`ConstantTargetNone` for target-extension types with the
`HasZeroInit` property.

The factory is type-polymorphic; the resulting `Constant *` is
type-monomorphic. Two `Constant::getNullValue(ptrTy_a)` and
`Constant::getNullValue(ptrTy_b)` for distinct
`ptrTy_a != ptrTy_b` produce distinct
`ConstantPointerNull *`s.

LangRef `:4968-4970`:
> The identifier '`null`' is recognized as a null pointer constant
> and must be of pointer type.

So syntactically the null literal is *only* for pointer types; the
type-polymorphic `getNullValue` is the C++-API convenience
distributing across type families.

**Stage-4 implication.** LLVM's design directly supports stage-4's
proposed `NullPtr(TyId)` variant: per-pointee-type null is the
right canonicalisation surface for vtable vacant slots
(spec.md §13.2). The equivalence class is "(`null`, this exact
pointer type)"; two null pointers to different pointee types are
distinct values. This matches how LLVM key-types are constructed
(`CPNConstants` is keyed by the pointer type itself).

If we go with the alternative typed-via-the-aggregate-context
(i.e., let the surrounding aggregate's field-type imply the null
pointer type), we lose the canonicalisation locality — the same
`Const::Null` variant would appear in many aggregates and
canonicalise differently per context. LLVM doesn't take that
route; it pays the per-type intern entry cost.

## 6. Uninit / padding: undef vs poison

LLVM has *two* distinct concepts here, and they're related in a
documented way.

### 6.1 `UndefValue` — unspecified bit-pattern

`UndefValue` (`Constants.h:1606-1645`) represents "the user of
this value may receive an unspecified bit-pattern." Critically,
the bit-pattern can vary across uses — multiple reads of the same
undef value can return different bits.

LangRef `:5092-5269` (Undefined Values section). Key quote:

> The string '`undef`' can be used anywhere a constant is
> expected, and indicates that the user of the value may receive
> an unspecified bit-pattern. Undefined values may be of any type
> (other than '`label`' or '`void`') and be used anywhere a
> constant is permitted.

LangRef `:5102-5107`:
> A '`poison`' value (described in the next section) should be
> used instead of '`undef`' whenever possible. Poison values are
> stronger than undef, and enable more optimizations. Just the
> existence of '`undef`' blocks certain optimizations.

### 6.2 `PoisonValue` — value of an erroneous operation

`PoisonValue` is a SUBCLASS of `UndefValue` (`Constants.h:1654`).
LangRef `:5271-5331` (Poison Values section). Key facts:

- Result of operations like `add nuw 0, 1` (overflow under nuw),
  `sdiv 1, 0` (div-by-zero), inbounds GEP that escapes inbounds.
- Most instructions *propagate* poison: `op poison, X` is poison.
- Notable exception: `select` does not always propagate (per
  LangRef).
- `freeze` instruction stops propagation: `freeze poison` returns
  some non-deterministic but well-defined value.
- Using poison as a UB-triggering operand IS immediate UB
  (e.g., load through poison ptr, divisor poison in udiv,
  branch condition poison).

> It is correct to replace a poison value with an undef value or
> any value of the type. — LangRef `:5289-5290`.

So poison is *stronger* than undef for optimisation purposes
(more freedom to substitute), but is also *meaner* about UB
propagation.

### 6.3 Per-type interning for both

`UVConstants` (`LLVMContextImpl.h:1704`) maps Type* →
`unique_ptr<UndefValue>`. `PVConstants` (`LLVMContextImpl.h:1706`)
maps Type* → `unique_ptr<PoisonValue>`. One UndefValue per Type,
one PoisonValue per Type, both per-context.

Aggregate-element queries: `UndefValue::getElementValue` /
`PoisonValue::getElementValue` walk the aggregate type and return
the per-element undef/poison singleton.

`Constant::isManifestConstant()` (`Constants.cpp:883-895`)
EXCLUDES UndefValue from "manifest constants":

```cpp
bool Constant::isManifestConstant() const {
  if (isa<UndefValue>(this))
    return false;
  if (isa<ConstantData>(this))
    return true;
  if (isa<ConstantAggregate>(this) || isa<ConstantExpr>(this)) {
    for (const Value *Op : operand_values())
      if (!cast<Constant>(Op)->isManifestConstant())
        return false;
    return true;
  }
  return false;
}
```

(NB: `PoisonValue` IS a subclass of `UndefValue`, so this check
also excludes poison.) The notion is that undef/poison are not
"committed bit patterns" — they're placeholders that the
optimiser is free to replace. A "manifest" constant has fully-
committed bits and can be hashed / serialised / compared
bitwise.

### 6.4 Padding bytes

LangRef `:5341-5342` (Well-Defined Values section):

> The padding of an aggregate isn't considered, since it isn't
> visible without storing it into memory and loading it with a
> different type.

So LLVM's **aggregate constants do not first-class their padding
bytes**. The shape of a `ConstantStruct {i32, i64}` is "the i32
field operand and the i64 field operand"; padding between them
(e.g., 4 bytes between offset 4 and offset 8 on a 64-bit ABI) is
*implicit* to the type's layout and does not appear in the
operand list. If you want padding bytes to be visible, you make
them explicit (e.g., add an `[4 x i8]` field).

This is a deliberate decision: padding is layout-derived, and
adding it to the const value breaks the "structural equality
implies semantic equality" invariant under different layout
assumptions.

When writing an aggregate to memory and reading it back as a
different type that exposes padding, LangRef permits the padding
bytes to take any value — LLVM encodes "padding visibility" via
`memcpy` / `memset` patterns at the IR level rather than at the
constant level.

**Stage-4 contrast.** Stage 4's working assumption (memory.md §2,
sub-decision 3) is that uninit bytes in aggregate constants ARE
first-class — i.e., distinguishable from any specific byte
pattern. LLVM does NOT do this. The closest LLVM analogue is
filling the relevant position with `UndefValue` (or
`PoisonValue`), but those require the position to be a
first-class field, not a padding slot.

This is a real divergence with practical consequences:
- LLVM's choice means two structurally-equal aggregate constants
  with different "padding contents" intern to the same Constant —
  there's no representation distinction.
- Stage 4's proposed choice means we'd distinguish them. This
  requires the variant set to support a `Uninit { size: u32 }`
  leaf (or similar) inside aggregates, AND it requires the
  bridge to actually produce different `Const`s for the
  difference. Whether rustc's MIR const-eval gives us byte-level
  uninit info that justifies the distinction is an open question
  — see the rustc.md prior-art note.
- LLVM's argument (padding "isn't visible without storing and
  reloading with a different type") essentially declares padding
  to be UB if observed, so the ambiguity is sound.

Recommendation: stage 4 should treat this as an explicit
decision sub-tag (likely `D-4-1.7`), not a default. The case for
keeping LLVM's non-distinction is a much smaller `Const` variant
set; the case for distinguishing is matching rustc's miri model,
which DOES distinguish (per the rustc.md fork's findings on
`InitMask`).

## 7. Validation phase

LLVM's `Verifier` runs after IR construction (typically at end-of-
module-build, or on demand). Constants are walked recursively
from the module's global initialisers and from instruction
operands.

### 7.1 `visitConstantExprsRecursively`

`Verifier.cpp:2783-2821`:

```cpp
void Verifier::visitConstantExprsRecursively(const Constant *EntryC) {
  if (EntryC->getNumOperands() == 0)
    return;

  if (!ConstantExprVisited.insert(EntryC).second)
    return;

  SmallVector<const Constant *, 16> Stack;
  Stack.push_back(EntryC);

  while (!Stack.empty()) {
    const Constant *C = Stack.pop_back_val();

    if (const auto *CE = dyn_cast<ConstantExpr>(C))
      visitConstantExpr(CE);

    if (const auto *CPA = dyn_cast<ConstantPtrAuth>(C))
      visitConstantPtrAuth(CPA);

    if (const auto *GV = dyn_cast<GlobalValue>(C)) {
      Check(GV->getParent() == &M, "Referencing global in another module!",
            EntryC, &M, GV, GV->getParent());
      continue;
    }

    for (const Use &U : C->operands()) {
      const auto *OpC = dyn_cast<Constant>(U);
      if (!OpC)
        continue;
      if (!ConstantExprVisited.insert(OpC).second)
        continue;
      Stack.push_back(OpC);
    }
  }
}
```

So validation: visit-set memoises traversal; `ConstantExpr`s and
`ConstantPtrAuth`s get class-specific shape checks; `GlobalValue`s
get a "live in this module" cross-module-reference check;
operands are recursively walked.

### 7.2 ConstantExpr-specific checks

`Verifier.cpp:2823-2830`:

```cpp
void Verifier::visitConstantExpr(const ConstantExpr *CE) {
  if (CE->getOpcode() == Instruction::BitCast)
    Check(CastInst::castIsValid(Instruction::BitCast, CE->getOperand(0),
                                CE->getType()),
          "Invalid bitcast", CE);
  else if (CE->getOpcode() == Instruction::PtrToAddr)
    checkPtrToAddr(CE->getOperand(0)->getType(), CE->getType(), *CE);
}
```

Surprisingly minimal — most ConstantExpr opcodes are validated
via the Instruction-level invariants when the expression is
folded into an Instruction (`getAsInstruction`,
`Constants.h:1548-1555`). The Verifier's class-specific work is
narrow.

### 7.3 GlobalVariable initialiser walk

`Verifier::visitGlobalVariable` (`Verifier.cpp:880-` and
:996-) walks each global's initialiser via
`visitConstantExprsRecursively`. So globals' initialisers are
the entry-points for the constant traversal.

### 7.4 Constant construction-time checks

Beyond the Verifier, `Constant` constructors assert structural
invariants (e.g., `ConstantInt::ConstantInt`'s width assertion at
`Constants.cpp:903-905`; `ConstantStruct` element-count check at
`Constants.cpp:1526-1527`; `ConstantPointerCast` constructions
with `castIsValid` at fold-time). These are pre-Verifier checks
— they assert IR well-formedness immediately on construction
rather than waiting for Verifier.

There is **no explicit "phase split"** in LLVM's design. Constants
do not have a "type-level" vs "value-level" partition; everything
is a `Constant` with a `Type`, and validation runs uniformly over
the operand graph.

The thing closest to a phase split: types are interned in
`LLVMContextImpl::IntegerTypes` /
`LLVMContextImpl::AnonStructTypes` etc. *before* constants
referencing them are interned (because constructing a Constant
takes a `Type *` argument), so there's an implicit ordering
"types first, constants second." But this is a layered-interner
fact, not a validation phase. Stage 4's proposed two-phase
validation has no LLVM precedent at this layer.

**Stage-4 implication.** LLVM is a confirmation point for "no
explicit phase split is necessary if your types are already
interned before constants are built." Stage 4's two-phase
discipline (spec.md §9.7) is needed because the bridge produces
the model in a different order than LLVM's per-LLVMContext
incremental build, AND because `GenericArg::Const(ConstId)`
participates in nominal type identity (so type-level consts must
be canonicalised before TyInterner runs). This is a Regseq-
specific constraint, not a universal one.

## 8. Cross-build determinism

### 8.1 Per-context interning (in-context determinism)

LLVM's interner is per-`LLVMContext`. Within one context, every
constant is canonical: structural equality implies pointer
equality. Two different `LLVMContext`s have separate intern
universes; pointers from one are not comparable to pointers from
another.

The interner storage layout (`LLVMContextImpl.h:1689-1720`):

```cpp
DenseMap<Type *, std::unique_ptr<ConstantAggregateZero>> CAZConstants;

using ArrayConstantsTy = ConstantUniqueMap<ConstantArray>;
ArrayConstantsTy ArrayConstants;

using StructConstantsTy = ConstantUniqueMap<ConstantStruct>;
StructConstantsTy StructConstants;

using VectorConstantsTy = ConstantUniqueMap<ConstantVector>;
VectorConstantsTy VectorConstants;

DenseMap<PointerType *, std::unique_ptr<ConstantPointerNull>> CPNConstants;
DenseMap<TargetExtType *, std::unique_ptr<ConstantTargetNone>> CTNConstants;
DenseMap<Type *, std::unique_ptr<UndefValue>> UVConstants;
DenseMap<Type *, std::unique_ptr<PoisonValue>> PVConstants;
StringMap<std::unique_ptr<ConstantDataSequential>> CDSConstants;
DenseMap<const BasicBlock *, BlockAddress *> BlockAddresses;
DenseMap<const GlobalValue *, DSOLocalEquivalent *> DSOLocalEquivalents;
DenseMap<const GlobalValue *, NoCFIValue *> NoCFIValues;
ConstantUniqueMap<ConstantPtrAuth> ConstantPtrAuths;
ConstantUniqueMap<ConstantExpr> ExprConstants;
```

Plus integer maps (zero-bitwidth-keyed, one-bitwidth-keyed,
general APInt-keyed) and FP maps for ConstantInt and ConstantFP.

### 8.2 `ConstantUniqueMap` — the generic interner

`ConstantsContext.h:553-` (the implementation continues past the
260-line read window). Quick summary:
- Templated on `ConstantClass`.
- Uses `DenseSet<ConstantClass *, MapInfo>`.
- `MapInfo` provides `getHashValue` and `isEqual` based on the
  per-class `ValType` (which is `ConstantAggrKeyType`,
  `ConstantExprKeyType`, or `ConstantPtrAuthKeyType`).
- Lookup key is `LookupKey = pair<TypeClass *, ValType>`.
- Hash combines type pointer and value-type's hash
  (`ConstantsContext.h:583-585`).

So `ConstantArray::get(ArrayType *Ty, ArrayRef<Constant *> V)`
(`Constants.cpp:1464-1468`) constructs the
`ConstantAggrKeyType<ConstantArray>{V}`, hashes it with the
type, looks up in the `ArrayConstants` map; on miss creates a
new ConstantArray and inserts.

### 8.3 Cross-context determinism: bitcode round-trip

LLVM does NOT make any cross-context determinism claim at the
ConstantImpl pointer level. Two different LLVMContexts processing
the same source produce the same *bitcode*, but the pointers are
unrelated.

Bitcode serialisation (in `lib/Bitcode/Writer/`) walks the IR's
constants in module order and emits them with stable indices
(per-bitcode-block local indices). On read, the bitcode reader
reconstructs each constant by calling the same factory functions
(`ConstantInt::get`, `ConstantArray::get`, etc.), producing
canonicalised `Constant *`s in the new context. The bitcode is
the cross-build/cross-context identity carrier.

There is no `StableHash` for constants in the rustc sense — the
bitcode itself is the stable representation, and the in-memory
`Constant *`s are local handles.

### 8.4 Stage-4 implication

LLVM is a **negative result** for cross-build hashing:
within-context structural canonicalisation is the strongest
identity property; cross-build identity is not modelled. Stage
4's `ConstIdentityKey` (spec.md §9.7) goes further — it requires
identical canonical-key bytes across builds, which LLVM doesn't
need because LLVM's "build" output is the bitcode, not the
in-memory IR.

Operationally this means we can't directly lift LLVM's interner
design — `ConstantUniqueMap`'s key types reference `Constant *`
operands, which are local pointers. Stage 4's relocation key
(stable identity tuple) replaces the operand-pointer with a
content-hash-bearing key that's stable across builds. This is the
single most important architectural divergence.

## Stage-4 applicability

Synthesising the eight findings against the D-4-1 candidate
sub-decisions:

### A. Variant set (D-4-1 framing)

LLVM's hierarchy provides reference for these stage-4 candidate
variants:

| Stage-4 candidate    | LLVM analogue                                                           | Notes                                                                                                                |
| -------------------- | ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `Bool`               | `ConstantInt` of i1 type                                                | LLVM unifies Bool with Int(i1).                                                                                      |
| `Int(IntKind, val)`  | `ConstantInt`                                                           | LLVM: APInt + `IntegerType *`; type tag externalised. Stage 4 inlines IntKind.                                       |
| `Uint(UintKind, val)`| `ConstantInt` (signed/unsigned is APInt-method-level, not type-level)    | LLVM does not split signed vs unsigned at the type or constant level (interpretation is per-instruction).            |
| `Float(FloatKind,bits)`| `ConstantFP`                                                          | APFloat carries semantics; stage-4-equivalent.                                                                       |
| `ZST`                | `ConstantAggregateZero` for aggregates; for scalar ZST: ConstantTokenNone or empty struct CAZ. | LLVM's CAZ design is a clean precedent.                                                                              |
| `Bytes(Vec<u8>)`     | `ConstantDataArray` of i8                                               | LLVM's CDS design is the memory-efficient form.                                                                      |
| `Aggregate(Vec<ConstId>)` | `ConstantArray` / `ConstantStruct` (operand-based)                 | Operand IDs are LLVM's `Constant *` operands. Stage 4's `ConstId` is a u32; same shape, different handle.            |
| `Enum { variant_idx, payload }` | (no direct analogue; LLVM lowers to ConstantStruct of {tag, payload-bytes-or-union}) | LLVM doesn't model enum semantics at the IR level — they're a frontend concept lowered to structs.                  |
| `Reloc(InstanceRef)` | `GlobalValue *` directly used as constant operand                       | LLVM's "GlobalValue is a Constant" choice is the strongest precedent. Stage 4 replaces `GlobalValue *` with stable identity tuple. |
| `NullPtr(TyId)`      | `ConstantPointerNull`                                                   | Direct precedent: per-pointer-type intern entry. Stage 4 should adopt this shape.                                   |
| `Uninit { size }`    | (closest: `UndefValue`, but per-type, not per-byte-range)               | LLVM does NOT first-class byte-level uninit inside aggregates. Stage 4 diverges here.                                |
| (undef/poison)       | `UndefValue` / `PoisonValue`                                            | Two distinct concepts. Stage 4's `Uninit` covers UndefValue's role; PoisonValue has no obvious Regseq use case (UB-from-erroneous-op is not a const concept in our domain). |
| (token)              | `ConstantTokenNone`                                                     | No Regseq need.                                                                                                       |
| (target ext)         | `ConstantTargetNone`                                                    | No Regseq need (we're target-agnostic at the IR level; target-specific intrinsics are a separate concept).            |
| (block address)      | `BlockAddress`                                                          | No Regseq need (no `indirectbr`-equivalent in our domain).                                                            |
| (DSO equivalent)     | `DSOLocalEquivalent`                                                    | No Regseq need (single-DSO model).                                                                                    |
| (no-CFI)             | `NoCFIValue`                                                            | No Regseq need.                                                                                                       |
| (ptrauth)            | `ConstantPtrAuth`                                                       | Possible future extension if Apple Arm64e becomes a target.                                                            |

The recommended minimum stage-4 variant set (memory.md §2.7-9):
`Bool`, `Char`, `Int`, `Uint`, `Float`, `ZST`, `Bytes`,
`Aggregate`, `Enum`, `Reloc`, `NullPtr`, `Uninit` — has **strong
precedent in LLVM** for every variant except `Char` (LLVM has no
Char type — it uses i32 / iN for Unicode scalar values), `Enum`
(LLVM lowers to struct), `Reloc-with-stable-identity` (LLVM uses
operand pointers), and `Uninit-as-byte-range` (LLVM's UndefValue
is per-Type only).

### B. Type-tag placement

LLVM externalises (`Type *Ty` is on the inherited Value).
Stage-4's plan to inline (`Int(IntKind, value)`) is a divergence
from LLVM but soundness-equivalent. Recommendation: lock the
inline form per the existing spec.md §9.7 wording (which already
shows `Int(IntKind, value)`).

### C. Aggregate field representation

LLVM uses operand pointers (`ConstantArray::Operands` are
`Constant *`s pointing into the same context's intern table).
Stage 4's plan to use `Vec<ConstId>` is structurally equivalent;
the only semantic difference is `ConstId` is a `u32` rather than
a host pointer.

LLVM's CDS optimisation (`ConstantDataArray` for simple-
element-typed dense arrays) is worth considering as a future
optimisation but is NOT a soundness concern at stage 4. The
spec-level shape can be `Aggregate(Vec<ConstId>)` uniformly;
implementation could later optimise homogeneous primitive-
element aggregates into a `Bytes(Vec<u8>)` representation if
profiling shows it matters. **For stage 4, prefer the uniform
representation** — premature optimisation here would add
canonicalisation surface (aggregate-of-int8 vs bytes-of-int8 are
the same value but different representations).

### D. Relocation handling

LLVM's design choice — *the global symbol IS the constant
operand* — is the cleanest precedent for stage-4's relocation
shape. The only divergence is replacing the host pointer with a
stable identity tuple. The two-tuple
`(InstanceId, /* in-build local */)` design would mirror LLVM
exactly but lose cross-build determinism; the four-tuple
`(kind_rank, def_path_hash, args_key, shim_disambig)` from
spec.md §9.7 is the cross-build-stable form.

LLVM's `ConstantExpr` (relocations with arithmetic — e.g.,
`@foo - @base` for relative pointers) has no v1 stage-4
precedent need: vtable instances reference shim function
addresses directly, no relative-pointer encoding required for
stage 1's pl011 driver pressure. **Stage 4 should NOT introduce
ConstantExpr-equivalents in v1.** If a future driver fixture
surfaces relative-pointer encoding need (e.g., RelativeVTablesABI-
style), revisit.

### E. Null pointer typing

`ConstantPointerNull` is a direct precedent for `NullPtr(TyId)`.
Stage 4 should adopt this shape. The intern entry per pointee
type (`CPNConstants` keyed by `PointerType *`) is exactly the
right canonicalisation surface for vtable vacant slots, where
each slot's pointee is the canonical vacant fn-ptr signature.

### F. Uninit / padding (the divergence)

LLVM's `UndefValue` and `PoisonValue` are PER-TYPE singletons,
not per-byte-range. Aggregate padding is implicit and ignored.

Stage 4's working assumption (memory.md §2.4) is that uninit
bytes are first-class. **This is a real divergence** that needs
deliberate justification:

- *Argument for*: matches rustc's miri model (per the rustc.md
  fork's findings). Distinguishing aggregate constants with
  different padding contents is needed for sound bitcast / load-
  reinterpret semantics, and for cross-build canonical equality
  to align with rustc's behaviour.
- *Argument against*: LLVM has shipped without this feature for
  decades. Padding is layout-dependent and observation requires
  UB-adjacent operations. Not first-classing it shrinks the
  variant set and simplifies canonicalisation.
- *Compromise*: first-class uninit at field-level (`Uninit {
  size: u32 }` as an aggregate field) but NOT at sub-byte
  resolution. This matches rustc's `InitMask` granularity
  (per-byte) without going to per-bit.

Stage-4 decision recommendation: keep `Uninit { size: u32 }` as
a leaf variant (per memory.md), but defer the question of when
the bridge actually emits it. The bridge can emit `Uninit` only
where rustc's `InitMask` says "uninit"; if no fixture-driven
pressure, the variant exists but is unused.

### G. Validation phase

LLVM's "no explicit phase split, types-before-constants by
construction order" is a confirmation point that Regseq's
two-phase validation is needed because of bridge ordering, NOT
because of any inherent property of constants. Stage-4 spec.md
§9.7's two-phase discipline is appropriate; LLVM provides no
counter-evidence.

### H. Cross-build determinism

LLVM provides a **negative result** here: it doesn't model
cross-build identity at the IR level (bitcode is the stable
representation). Stage-4's `ConstIdentityKey` is a stronger
property than LLVM offers. The architectural lesson is: don't
look to LLVM's `ConstantUniqueMap` design for cross-build
determinism cues — it's a per-context structural interner only.
Stage-4 needs to design its own canonicalisation that's
cross-build-stable, modelled on rustc's `HashStable for AllocId`
(see rustc.md §8 finding) rather than LLVM's pointer-based
interner.

## Open questions / hypotheses

1. **Hypothesis: LLVM's lack of byte-level uninit modelling is
   an active design choice, not an oversight.** The LangRef
   "padding isn't visible without ..." quote suggests this was
   considered. Verifying whether any LLVM bug or RFC explicitly
   discusses adding byte-level uninit would clarify whether
   stage 4's divergence is well-founded. *Not investigated;
   would require GitHub issue / mailing-list search.*

2. **Hypothesis: LLVM's per-context interner is sufficient for
   incremental builds because the on-disk bitcode is the stable
   representation, not the in-memory `Constant *`s.** This means
   any cross-build hash should be computed at the bitcode level,
   not the in-memory level. If Regseq's ConstIdentityKey is
   computed in-memory pre-serialisation, it must match what
   round-tripping through the textual / JSON form would
   produce. *Possible test: ensure that two builds running with
   different host pointer-allocator behavior produce identical
   ConstIdentityKey bytes for equivalent inputs.*

3. **Hypothesis: stage 4's `Reloc(InstanceRef)` doesn't need a
   `ConstantExpr`-equivalent in v1.** The pl011 driver pressure
   doesn't require relative-pointer relocations. If RelativeVTables
   or similar lowering is later adopted, ConstantExpr's
   sub-of-ptrtoint design provides a precedent. *Verify against
   pl011 fixture once stage 12 lands.*

4. **`ConstantPtrAuth` as a forward-compatibility consideration.**
   If Regseq targets Apple Arm64e, ptrauth-bearing function
   pointers in vtables would need a similar variant. Not in v1
   scope; but worth keeping the variant-set framework
   extensible. *No action at stage 4; flag for stage 14 / domain
   extensions.*

5. **CDS as a future optimisation, not a v1 spec concern.** The
   memory-efficiency win of `ConstantDataArray` over operand-
   based `ConstantArray` is real for large primitive-element
   arrays (e.g., `[1024 x i32]` of consecutive integers). Stage
   4 should specify the canonical shape uniformly
   (`Aggregate(Vec<ConstId>)`) and let implementations later
   add a CDS-style optimisation if profiling justifies it.
   *Action: do not bake CDS into the spec.*

6. **The `BlockAddress` / `DSOLocalEquivalent` / `NoCFIValue`
   precedents are not relevant.** None of these have driver-DSL
   analogues. Skip.

---

End of LLVM stage-4 prior art.
