# Charon -- Prior-art notes (stage-4 const-vocabulary research)

Project: AeneasVerif/Charon. Closest peer to RegseqModel: post-mono Rust
-> owned IR (ULLBC / LLBC), verification-target consumers (Aeneas /
Eurydice), determinism aspirations. Investigated specifically for how
constant values are represented and what stage-4 should adopt or
deliberately avoid.

This document cites the Charon repository (cloned at
`/tmp/regseq-stage4-research/charon`, default branch). Citations are
`charon/src/<path>:<line>` -- relative to the cloned repo root. All
findings are file-line grounded except where I explicitly mark a
hypothesis.

## Source map

Files read in order of relevance:

- `charon/src/ast/values.rs` -- `Literal`, `ScalarValue`, `FloatValue`
  (the leaf primitive vocabulary)
- `charon/src/ast/expressions.rs` -- `ConstantExprKind`, `ConstantExpr`,
  `Byte`, `Provenance`, `FnPtr`, `FnPtrKind`, `UnsizingMetadata`
- `charon/src/ast/types.rs` -- `LiteralTy`, `RefKind`, `Ty`/`TyKind`
  (hash-consed)
- `charon/src/ast/types/vars.rs` -- `ConstGenericVarId`,
  `ConstGenericDbVar`, `ConstGenericParam`
- `charon/src/ast/gast.rs` -- `GlobalDecl`, `GlobalDeclRef`,
  `FunDeclRef`, `GlobalKind`
- `charon/src/ast/hash_cons.rs` -- the global interning table
  (`HashConsed<T>`, `HashConsId`, dedup serializer)
- `charon/src/bin/charon-driver/translate/translate_constants.rs` --
  the rustc(hax) -> Charon `ConstantExpr` bridge
- `charon/src/transform/simplify_output/simplify_constants.rs` --
  post-translation pass that demotes most `ConstantExprKind` variants
  to statements
- `charon/src/transform/simplify_output/inline_anon_consts.rs` --
  inlines anon-const initializer bodies
- `charon/src/options.rs` -- `raw_consts`, `unsized_strings`,
  `monomorphize`, presets
- `charon/src/transform/normalize/partial_monomorphization.rs` --
  monomorphization pass

---

## 1. What is the const-value type?

Charon's const vocabulary is **layered**, with the post-translation
shape **substantially smaller** than the construction-time shape. There
is no separate `ConstGeneric` enum: generic-arg-position constants and
value-position constants share the **same** type, `ConstantExpr`.

### 1.1 The leaf primitive: `Literal`

`charon/src/ast/values.rs:41-52`:

```rust
pub enum Literal {
    Scalar(ScalarValue),
    Float(FloatValue),
    Bool(bool),
    Char(char),
    ByteStr(Vec<u8>),
    Str(String),
}
```

Six variants. `Scalar` and `Float` carry their own type tags
(`ScalarValue` / `FloatValue` -- next sub-section); `Bool`, `Char`,
`ByteStr`, `Str` are type-monomorphic by Rust language rules and so
need no explicit type tag.

`ScalarValue` (`values.rs:75-85`):

```rust
pub enum ScalarValue {
    Unsigned(UIntTy, u128),
    Signed(IntTy, i128),
}
```

Two scalar variants -- signed vs unsigned. Each carries the type tag
(`IntTy` / `UIntTy`) **inline**.

`FloatValue` (`values.rs:93-99`):

```rust
pub struct FloatValue {
    pub value: String,    // String repr (so Eq/Ord/Hash derive cleanly)
    pub ty: FloatTy,
}
```

Floats are stored as their **String** representation rather than raw
bits. The doc comment (`values.rs:87-90`) explains: "however, instead
of storing the float value itself, we store its String representation.
This allows to derive the Eq and Ord traits, which are not implemented
for floats." This is a notable deviation from the obvious "bits"
choice -- and a hypothesis on motive: the `Hash, PartialOrd, Ord`
derives at `values.rs:34-37` would not work for native floats.

### 1.2 The wrapper: `ConstantExpr` + `ConstantExprKind`

`charon/src/ast/expressions.rs:680-683`:

```rust
pub struct ConstantExpr {
    pub kind: ConstantExprKind,
    pub ty: Ty,
}
```

The wrapper carries an **external type tag** (`ty: Ty`) -- this is
visible whether or not the inner kind already includes a literal-type
hint. Stage-4 contrast called out below.

`ConstantExprKind` (`expressions.rs:604-676`) -- thirteen variants,
documented inline. This is the *construction-time* universe (more on
the post-simplify shape below):

- `Literal(Literal)` -- the leaf primitive case.
- `Adt(Option<VariantId>, Vec<ConstantExpr>)` -- enum/struct/tuple/unit
  aggregate (variant index `Some(_)` for enums, `None` for structs /
  tuples / units).
- `Array(Vec<ConstantExpr>)` -- array aggregate.
- `Global(GlobalDeclRef)` -- reference to a top-level static / const.
- `TraitConst(TraitRef, TraitItemName)` -- trait-associated constant.
- `VTableRef(TraitRef)` -- reference to a vtable static item for a
  trait ref.
- `Ref(Box<ConstantExpr>, Option<UnsizingMetadata>)` -- shared
  reference to a const value (with optional unsizing metadata).
- `Ptr(RefKind, Box<ConstantExpr>, Option<UnsizingMetadata>)` -- raw
  pointer to a (possibly mutable) static, with optional unsizing
  metadata.
- `Var(ConstGenericDbVar)` -- a const-generic variable (de Bruijn).
- `FnDef(FnPtr)` -- function definition (ZST const).
- `FnPtr(FnPtr)` -- function pointer.
- `PtrNoProvenance(u128)` -- a pointer value with no provenance (e.g.
  the null pointer); the `u128` is the address bits.
- `RawMemory(Vec<Byte>)` -- bytes-with-provenance fallback for
  unstructured const data (unions, partially evaluated values).
- `Opaque(String)` -- "Charon doesn't handle this yet"; carries an
  error message.

Note the `#[charon::variants_prefix("C")]` annotation
(`expressions.rs:603`) -- in the OCaml export the variants are prefixed
with `C` (e.g. `CLiteral`, `CAdt`).

### 1.3 The post-simplify shape

`simplify_constants.rs:1-7`:

> The MIR constant expressions lead to a lot of duplication: there are
> for instance constant ADTs which duplicate the "regular" aggregated
> ADTs in the operands, constant references, etc. This reduces the
> number of cases to handle and eases the function translation in
> Aeneas. This pass removes all those occurrences so that only the
> `ConstantExpression::Literal`. It does so by introducing intermediate
> statements.

The `simplify_constants` pass (`simplify_output/simplify_constants.rs`,
~185 lines) systematically demotes `Adt`, `Array`, `Global`,
`PtrNoProvenance`, `Ref`, `Ptr`, `FnPtr`, `VTableRef` to *statement
sequences* in the body. After the pass (and modulo `--raw-consts`),
the live const variants in the final LLBC are reduced to:

- `Literal(Literal)` -- the leaf scalar / bool / char / byte-string /
  string case.
- `Var(ConstGenericDbVar)` -- the const-generic variable case.
- (Plus the residual cases the pass cannot demote: `RawMemory`,
  `Opaque`, `TraitConst`, `FnDef` in the rare cases noted in
  `simplify_constants.rs:117-128`, etc.)

`simplify_constants.rs:161-163`: the pass is gated by
`!options.raw_consts`. The `Aeneas` preset enables this pass; the
`OnlyCargo` and `Soundness` presets disable it (`options.rs:353,361`).

The doc comment on `ConstantExpr` (`expressions.rs:561-586`) confirms:

> Only the `ConstantExprKind::Literal` and `ConstantExprKind::Var`
> cases are left in the final LLBC. The other cases come from a
> straight translation from the MIR.

**Implication for stage 4.** Charon's post-simplify shape is the
"value-as-Literal" minimum -- everything else has been desugared into
an aggregation statement plus a temporary local. RegseqModel does NOT
intend to do this desugaring (we keep static initialisers, vtable
instances, aggregate consts as first-class const values per
spec.md §9.7). So Charon's *terminal* shape is a poor model for stage
4; Charon's *construction-time* shape (the 13-variant
`ConstantExprKind`) is much closer to what stage 4 needs.

---

## 2. Type-tag placement for scalars

### 2.1 Inline-in-variant for scalars and floats

`ScalarValue::{Signed(IntTy, i128), Unsigned(UIntTy, u128)}` --
type tag is in the variant payload.

`FloatValue { value: String, ty: FloatTy }` -- type tag is a struct
field on the value.

### 2.2 Externally-attached for the `ConstantExpr` wrapper

`ConstantExpr { kind: ConstantExprKind, ty: Ty }` -- every
`ConstantExpr` carries an outer `ty` field independent of whatever the
inner kind tells us.

So Charon does **both** -- inline tags on scalar primitives AND a
separate wrapper-level `ty: Ty` field. The redundancy is intentional:

- The inline scalar tag (`UIntTy`/`IntTy`/`FloatTy`) carries enough to
  evaluate / serialise the literal in isolation.
- The wrapper `ty: Ty` carries the **full** type, which can be richer
  than the inline tag (e.g., for `ConstantExprKind::Adt`, the wrapper
  `ty` is `Ty::Adt(...)` -- the variant alone has no type info).

`LiteralTy` (`types.rs:797-803`):

```rust
pub enum LiteralTy {
    Int(IntTy),
    UInt(UIntTy),
    Float(FloatTy),
    Bool,
    Char,
}
```

The `LiteralTy` enum is the type-side counterpart of `Literal` (used in
`Ty` / `TyKind::Literal(LiteralTy)`).

**Implication for stage 4.** Charon shows that an inline-and-external
*both* approach is workable. Spec.md §9.7's "scalar primitives carry
type info inline" choice is consistent with Charon's `ScalarValue`. But
Charon ALSO adds an external `ty: Ty` -- if RegseqModel intends to
omit the external tag (so the `Const` carries identity-content only),
that diverges from Charon. Investigate whether the external tag earns
its place in our pipeline (e.g. for type-checking ConstantExpr against
a context type).

---

## 3. Aggregate constants: representation

### 3.1 Inline structural sub-children, not ID-referenced

`ConstantExprKind::Adt(Option<VariantId>, Vec<ConstantExpr>)` and
`ConstantExprKind::Array(Vec<ConstantExpr>)`
(`expressions.rs:614-615`) carry their child constants **inline** as
`Vec<ConstantExpr>`, NOT as `Vec<ConstId>`.

Translation (`translate_constants.rs:76-104`):

```rust
hax::ConstantExprKind::Adt { kind, fields } => {
    let fields: Vec<ConstantExpr> = fields
        .iter()
        .map(|f| self.translate_constant_expr(span, &f.value))
        .try_collect()?;
    use crate::hax::VariantKind;
    let vid = if let VariantKind::Enum { index, .. } = *kind {
        Some(translate_variant_id(index))
    } else {
        None
    };
    ConstantExprKind::Adt(vid, fields)
}
```

Tuples are translated as `Adt(None, fields)` (`translate_constants.rs:97-104`):

```rust
hax::ConstantExprKind::Tuple { fields } => {
    let fields: Vec<ConstantExpr> = fields
        .iter()
        .map(|f| self.translate_constant_expr(span, f))
        .try_collect()?;
    ConstantExprKind::Adt(None, fields)
}
```

Aggregates are recursive: `Adt(_, [Adt(_, [Literal(...)])])` is a
struct of struct of literal. Every layer carries its own `ty: Ty`
wrapper.

### 3.2 Sharing of equal subterms: hash-consing applies to `Ty`, NOT `ConstantExpr`

`charon/src/ast/types.rs:811`:

```rust
pub struct Ty(pub HashConsed<TyKind>);
```

`Ty` is hash-consed. But `ConstantExpr` itself is NOT
(`expressions.rs:680-683` -- `pub struct ConstantExpr { kind, ty }`,
no `HashConsed<...>` wrapper). So two structurally-equal
`ConstantExpr` values have NO sharing -- they are stored independently.

Sharing of equal subterms is therefore confined to:
- `ty: Ty` fields (hash-consed on the `Ty` side);
- `BoxedArgs` inside `GlobalDeclRef`, `FunDeclRef`, etc. (hash-consed
  via `Ty` and other interned components).

The const-value bytes themselves are not deduplicated.

### 3.3 The `RawMemory(Vec<Byte>)` fallback

`expressions.rs:669-672`:

```rust
/// Raw memory value obtained from constant evaluation. Used when a more
/// structured representation isn't possible (e.g. for unions) or just
/// isn't implemented yet.
RawMemory(Vec<Byte>),
```

This is the bytes-with-provenance escape for cases where the
structural `Adt`/`Array` shape doesn't apply. Used for:

- unions (no canonical field-sequence interpretation);
- partial / cast / non-structural results;
- `--unsized-strings` mode for `&str` (the `Str` literal becomes a
  `RawMemory` of bytes, see `translate_constants.rs:18-27`).

The `Byte` enum (next section) is what makes `RawMemory` capable of
expressing relocations + uninit.

**Implication for stage 4.** The aggregate inline-children pattern
(`Vec<ConstantExpr>` not `Vec<ConstId>`) is a legitimate design choice
but loses sharing across structurally-equal aggregates. Stage 4's
draft (`stage-4/status.md` open decisions, sub-decision 3) leans
`Vec<ConstId>` for sharing. Charon's choice does not refute that --
they accept the sharing loss because their post-simplify shape
removes most aggregates anyway. Stage 4 keeps aggregates as
first-class consts, so the sharing benefit is more relevant for us.

---

## 4. Relocation handling: function/static/vtable pointers

### 4.1 Symbol-typed references via `GlobalDeclRef` / `FnPtr`

`expressions.rs:617-647`:

- `Global(GlobalDeclRef)` -- references a top-level static / const.
- `TraitConst(TraitRef, TraitItemName)` -- references a trait-
  associated const.
- `VTableRef(TraitRef)` -- references a vtable static.
- `FnDef(FnPtr)` -- references a function definition (ZST const).
- `FnPtr(FnPtr)` -- pointer to a function item.

`GlobalDeclRef` (`gast.rs:277-280`):

```rust
pub struct GlobalDeclRef {
    pub id: GlobalDeclId,
    pub generics: BoxedArgs,
}
```

`FnPtr` (`expressions.rs:528-531`):

```rust
pub struct FnPtr {
    pub kind: Box<FnPtrKind>,
    pub generics: BoxedArgs,
}
```

`FnPtrKind` (`expressions.rs:506-514`):

```rust
pub enum FnPtrKind {
    Fun(FunId),
    Trait(TraitRef, TraitItemName, FunDeclId),
}
```

So every relocation is a tuple of `(local_id, generic_args)` --
`(FunDeclId | GlobalDeclId, BoxedArgs)`. The local IDs
(`FunDeclId` and `GlobalDeclId`) are **build-local indices**, NOT
cross-build stable identifiers.

This is a critical contrast point with RegseqModel. **Charon's
relocation scheme does NOT support cross-build identity.** A new
build of the same crate could produce different `FunDeclId` /
`GlobalDeclId` integers; the stored constants would be technically
"the same" only up to a re-keying of the IDs.

### 4.2 The `Provenance` enum for byte-level relocations

`expressions.rs:539-545`:

```rust
pub enum Provenance {
    Global(GlobalDeclRef),
    Function(FunDeclRef),
    Unknown,
}
```

This is used inside `Byte` (next section) for byte-level pointer
storage in `RawMemory`. So a `Byte::Provenance(Provenance::Function(
fdr), offset)` says "this byte is offset N within a pointer to the
function fdr" (where `fdr` includes generics). Same local-ID
exposure as in 4.1 -- relocations rest on `FunDeclId` /
`GlobalDeclId`.

### 4.3 The post-simplify desugaring of relocations

The `simplify_constants` pass demotes the `Global` and `FnPtr`
variants away (`simplify_constants.rs:31-33, 117-128`):

- `Global(global_ref)` becomes a `Place::new_global(global_ref, ty)`
  copied into a temporary local. The const is replaced by an
  `Operand::Copy(...)`.
- `FnPtr(fptr)` becomes a `FnDef -> FnPtr` cast statement.

So in the final LLBC, explicit relocations have been turned into a
sequence of statements that yield a place / value at runtime. The IR
no longer carries const-time relocations except inside `RawMemory`.

**Implication for stage 4.** RegseqModel keeps relocations as
first-class const values (vtable instances, static initialisers
referencing function shims). Charon's "demote to statements"
strategy is incompatible with that -- our codegen needs a const-side
const-value with embedded relocation tuples it can render to C.
Charon's local-ID-based `(FunDeclId, BoxedArgs)` is the wrong shape
for cross-build determinism but still informative as a structural
template: stage 4 needs an analogue with the local IDs replaced by a
stable cross-build encoding (per spec.md §9.7's
`(kind_rank, def_path_hash, args_key, shim_disambig)` tuple).

---

## 5. Null pointer typing

### 5.1 Charon's null is `PtrNoProvenance(u128)` with no type tag in the inner

`expressions.rs:664-668`:

```rust
/// A pointer with no provenance (e.g. 0 for the null pointer)
///
/// We eliminate this case in a micro-pass.
PtrNoProvenance(u128),
```

The wrapper-level `ty: Ty` carries the pointer's type; the inner
`PtrNoProvenance(u128)` carries only the address bits (typically
`0` for null but can be any provenance-less constant).

So Charon's "typed null" is the pair `(ty: Ty::Ptr(...), kind:
PtrNoProvenance(0))`. There is no per-pointee-type variant; the
type is in the external wrapper.

The `simplify_constants` pass demotes this further
(`simplify_constants.rs:34-48`):

```rust
ConstantExprKind::PtrNoProvenance(ptr) => {
    let usize_ty = TyKind::Literal(LiteralTy::UInt(UIntTy::Usize)).into_ty();
    let ptr_usize = ConstantExprKind::Literal(Literal::Scalar(ScalarValue::Unsigned(
        UIntTy::Usize,
        ptr,
    )));
    let cast = UnOp::Cast(CastKind::RawPtr(usize_ty.clone(), val.ty.clone()));
    Rvalue::UnaryOp(...)
}
```

So in the final LLBC, a typed null is `(usize 0) cast to *T` --
expressed as a usize literal followed by a cast statement.

**Implication for stage 4.** RegseqModel needs a typed null **as a
const value** (vacant vtable slots per spec.md §13.2 are typed-null
fn-pointer fields canonicalised against the trait's vacant signature).
Charon's external `ty: Ty` + null bits scheme would work but does not
canonicalise typed-null *equality* on its own -- two
`(Ty::Ptr<u8>, PtrNoProvenance(0))` and
`(Ty::Ptr<u32>, PtrNoProvenance(0))` are technically distinct
`ConstantExpr` instances at the wrapper level (the `ty` differs)
but identical at the inner-kind level. A stage-4 `NullPtr(TyId)`
variant fuses both layers and gives a canonical-equality story.

---

## 6. Uninit / padding distinguishability

### 6.1 The `Byte` enum -- direct prior art

`expressions.rs:547-559` (verbatim):

```rust
/// A byte, in the MiniRust sense: it can either be uninitialized, a
/// concrete u8 value, or part of a pointer with provenance (e.g. to a
/// global or a function)
pub enum Byte {
    /// An uninitialized byte
    Uninit,
    /// A concrete byte value
    Value(u8),
    /// A byte that is part of a pointer with provenance. The u8 is the
    /// offset within the pointer. Note that we do not have an actual
    /// value for this pointer byte, unlike MiniRust, as that is
    /// non-deterministic.
    Provenance(Provenance, u8),
}
```

Three states per byte:
1. `Uninit` -- the byte is uninitialised (padding, or a not-yet-set
   region).
2. `Value(u8)` -- the byte is a concrete u8.
3. `Provenance(Provenance, u8)` -- the byte is part of a pointer to a
   relocation target; the second `u8` is the offset within the pointer
   (so byte 0..=7 of a 64-bit pointer to `@foo` are
   `Provenance(@foo, 0..=7)`). Charon is explicit that there is no
   stored "address" value: the Rust comment says "we do not have an
   actual value for this pointer byte, unlike MiniRust, as that is
   non-deterministic."

The `Byte` enum lives in `RawMemory(Vec<Byte>)`
(`expressions.rs:669-672`).

**Implication for stage 4.** This is the cleanest direct precedent
for stage-4's uninit-byte handling. Three observations:

1. **First-class `Uninit`.** Padding bytes are not 0-bytes. They are
   their own variant. Matches the spec.md §9.7 line 1614-1616
   "canonicalisation must distinguish uninit from any specific byte
   pattern" requirement.
2. **Structural relocation embedding.** Pointers inside
   `RawMemory` are structurally embedded byte-wise as
   `Provenance(...)` rather than as a separate side-band. The "byte 3
   of pointer @foo" representation is unusual but cleanly serialises.
3. **No address materialisation.** Charon explicitly does not produce
   the actual address bits of a pointer constant (since they are not
   deterministic across builds / runs / ASLR). Stage 4 should
   borrow this discipline.

### 6.2 Aggregate uninit vs RawMemory uninit

For *structural* aggregates (`Adt`, `Array`), there is no padding
representation -- the field sequence is dense in `Vec<ConstantExpr>`,
and inter-field padding is implicit (recovered from layout when
needed). So Charon distinguishes:

- structural aggregates: no explicit padding bytes; layout-driven;
- `RawMemory` shape: explicit per-byte `Uninit` / `Value` /
  `Provenance` discriminator.

This duality is consistent with stage-4's framing where padding
distinguishability matters mostly for `RawMemory`-shaped value-level
consts (vtable instances, statics) but not for structural aggregates
where the field interpretation is canonical.

---

## 7. Validation / phase ordering

### 7.1 Charon does not have a dedicated "const validation phase"

There is no analogue of RegseqModel's §3.3 Phase B step 4 (validate
ConstInterner). Charon's pipeline is, broadly:

1. **Translation pass** (`bin/charon-driver/translate/`): rustc(hax) ->
   ULLBC `ConstantExpr`. Per-item; depends on rustc's MIR + hax
   already having const-evaluated. `translate_constants.rs:11-191`
   relies entirely on rustc/hax having pre-evaluated the const into a
   structural shape; the Charon side never re-evaluates.
2. **Normalisation passes** (`transform/normalize/`): partial
   monomorphization, dyn-trait-call rewrites, associated-type
   expansion, etc. Const handling here is mostly
   substitution-following.
3. **Simplification passes** (`transform/simplify_output/`):
   `inline_anon_consts` inlines anon-const initializer bodies;
   `simplify_constants` desugars rich const variants to statements.
4. **Finish translation** (`transform/finish_translation/`):
   ptr-metadata insertion, return-unit insertion, etc.
5. **Typecheck and unify** (`transform/typecheck_and_unify.rs`) --
   one pass, run last; not a per-table validation.

The const-on-translation contract is "rustc has already evaluated;
hax has already structured; we are extracting." There is no
construction-invariant table.

### 7.2 No type-level vs value-level const phase split

Stage 3's deferral expects a type-level / value-level split (spec.md
§9.7 lines 1618-1652) because `ConstId` participates in nominal-type
identity. Charon avoids the issue by not interning consts at all
(§3.2 above). `ConstGenericVarId` (`types/vars.rs:98`) is the const-
arg position's local handle; the `ConstantExpr` it resolves to is
just an inline subterm, not a separate interner.

`ConstGenericDbVar` (`types/vars.rs:173`) is `DeBruijnVar<
ConstGenericVarId>` -- so const-generic-position uses a de-Bruijn
indexing scheme over the parameter slots of the enclosing item's
`GenericParams`. Resolution is structural over the AST, not via an
interning table.

**Implication for stage 4.** Charon does NOT prove out a phase-split
discipline like RegseqModel's planned design. The reason it doesn't
need one is that consts are not interned and don't participate in
type identity (since Charon's `Ty` is hash-consed differently and
generic-arg positions hold inline `ConstantExpr`, not a `ConstId`).
RegseqModel's choice to intern consts and reach into them via
`GenericArg::Const(ConstId)` is what creates the validation cycle;
Charon's path of "do not intern, embed inline" sidesteps it.

This is a real fork in the road. Charon's evidence does not
contradict RegseqModel's planned approach, but it does demonstrate
that an "embed inline, no global interner" path is viable. Stage-4's
two-phase split needs justification grounded in OUR consumer needs
(F4-2 fingerprint determinism, snapshot deduplication), not in
prior-art necessity.

---

## 8. Cross-build determinism

### 8.1 Charon's hash-consing is build-local

`charon/src/ast/hash_cons.rs:51-55`:

```rust
fn fresh_id() -> HashConsId {
    static ID: AtomicU64 = AtomicU64::new(0);
    HashConsId(ID.fetch_add(1, Ordering::Relaxed))
}
```

`hash_cons.rs:135-144` doc comment:

> Identify this value uniquely amongst values of its type. The id
> depends on insertion order into the interning table which makes them
> in principle deterministic.

The `HashConsId` is a process-monotonic atomic counter. Two builds
of the same crate may insert types in a different order (due to query
caching, parallelism, hash-table iteration order) and produce
different IDs for the same `Ty` content. The `HashConsId` is a
**deduplication handle** -- used to short-circuit serialisation of
shared subterms (see the dedup-serialiser at
`hash_cons.rs:217-381`) -- not a cross-build identity.

### 8.2 Serialisation deduplication -- not cross-build content addressing

`hash_cons.rs:280-294` defines a serialisation envelope:

```rust
enum SerRepr<T> {
    HashConsedValue(HashConsId, T),  // first emission: id + value
    Deduplicated(HashConsId),         // subsequent: id only
    Untagged(T),                      // no dedup
}
```

This is *intra-document* dedup (the JSON output does not repeat the
same hash-consed value), not cross-build identity. The IDs reset per
document.

### 8.3 ConstantExpr equality

`ConstantExpr` derives `Hash, PartialEq, Eq` over its full structure
(`expressions.rs:589-594`). Two const values are equal iff they have
identical `kind` AND identical `ty` (the wrapper `ty: Ty` is
hash-consed, so its equality is an Arc-pointer comparison, which is
*content equality* via the interner -- but again, build-local).

So Charon's const equality is structural-and-deep. There is no
deterministic cross-build identity for consts.

**Implication for stage 4.** Charon does NOT solve the cross-build
determinism problem. RegseqModel cannot inherit Charon's scheme and
get the F4-2 fingerprint property "for free." The 4-tuple stable
encoding `(kind_rank, def_path_hash, args_key, shim_disambig)` from
spec.md §9.7 is novel relative to Charon (and consistent with
rustc's `HashStable for AllocId` approach documented in our sibling
file `rustc.md` finding 8). Stage 4 needs this encoding;
Charon does not provide a precedent we can borrow.

---

## Stage-4 applicability summary

| Stage-4 sub-decision | Charon precedent | Stage-4 stance suggested |
| --- | --- | --- |
| Variant set (D-4-1.*) | `Literal` (6) + `ConstantExprKind` (13) construction-time, narrowed to ~2 post-simplify | We do NOT desugar; Charon's *construction-time* shape is the relevant comparison. Variants we have analogues for: scalar primitives (Charon `Literal::Scalar`/`Float`/`Bool`/`Char`); aggregate (Charon `Adt`/`Array`); relocation (Charon `Global`/`FnDef`/`FnPtr`/`VTableRef`); typed null (Charon `PtrNoProvenance` + wrapper `ty`); raw bytes (Charon `RawMemory`); uninit (Charon `Byte::Uninit` inside `RawMemory`). Stage 4's draft variant set covers the same ground. |
| Inline scalar tag | `ScalarValue::{Signed(IntTy, _), Unsigned(UIntTy, _)}` + outer `ty: Ty` wrapper | Stage 4 inlines per spec.md §9.7; Charon does both inline-and-outer. We can drop the outer if we don't need it -- Charon needs it because of the wider `ConstantExpr` envelope; stage 4's `Const` is the envelope itself, so an outer `ty` field is more directly questioned. |
| Aggregate children: `Vec<ConstId>` vs inline | Charon uses inline `Vec<ConstantExpr>` | **Stage 4 should diverge.** Charon's inline-children loses cross-aggregate sharing, which is fine for them because they collapse aggregates post-simplify. Stage 4 keeps aggregates first-class; sharing of equal subterms across vtable instances + statics is non-trivial. `Vec<ConstId>` is the better fit. |
| Relocation encoding | `(FunDeclId, BoxedArgs)` / `(GlobalDeclId, BoxedArgs)` -- local-ID-based | **Stage 4 must diverge.** Charon's local IDs are not cross-build stable. Stage 4's planned 4-tuple stable encoding is novel relative to Charon. |
| Null pointer typing | Outer `ty: Ty` + inner `PtrNoProvenance(0)` | Stage 4 should fuse these into `NullPtr(TyId)` -- gives a single canonical-equality story without the wrapper-level type to dis-canonicalise. |
| Uninit / padding | `Byte::Uninit` first-class inside `RawMemory(Vec<Byte>)` | **Stage 4 should adopt.** Direct precedent. The MiniRust three-state `Byte` is a clean, well-motivated design. |
| Phase split (type-level vs value-level) | Not present; Charon does not intern consts | Charon does not validate the necessity of our phase split; stage 4's choice rests on F4-2 / fingerprint determinism, not on Charon precedent. |
| Cross-build deterministic identity | Not solved (build-local atomic counter) | **Stage 4 must solve this independently.** Closest precedent is rustc's `HashStable for AllocId` (sibling `rustc.md` finding 8). |
| `Byte` enum design | `Byte::{Uninit, Value(u8), Provenance(Provenance, u8)}` | Adopt the structural shape; replace `Provenance` with stage-4's stable-identity tuple instead of Charon's local-ID-based `Provenance` enum. |
| Float storage | `FloatValue { value: String, ty: FloatTy }` -- string repr | Hypothesis: not necessary if stage 4 uses `u128 bits` storage with a custom `Hash + Eq` impl that handles NaN canonicalisation deliberately. Charon's choice is a workaround for Rust's lack of `Eq` on float; we can choose differently. |

---

## Open questions / hypotheses

1. **Hypothesis -- the external `ty: Ty` wrapper field is not load-
   bearing for stage 4.** Charon needs it because `ConstantExpr` is a
   leaf type used in many positions (`Operand::Const`, generic-arg
   positions, some aggregate field positions). Stage 4's `Const` is
   the envelope, sits behind a `ConstId`, and is reached only from
   typed slots (`InstanceKind::Static.init: Option<ConstId>`,
   `GenericArg::Const(ConstId)`, etc.). The slot ALREADY knows the
   type (`Static` carries `Ty`; the generic-arg position knows the
   parameter's type from `ConstGenericParam.ty`). So the type tag can
   be elided from `Const` itself, with type-checking happening at the
   slot.

2. **Open -- what to do about union constants.** Charon punts to
   `RawMemory` for unions (`expressions.rs:670-672` doc comment).
   RegseqModel's stage 3 spec rejects no Rust-defined unions; the
   bridge translates them as `Ty::Union`. If a `union U { a: u32, b:
   f32 }` appears as a const value, what is its canonical
   representation? Plausibly `Const::RawMemory` (analogous to
   Charon). Worth surfacing in D-4-1 sub-decisions.

3. **Hypothesis -- function-pointer demotion is not relevant for
   stage 4.** Charon demotes `FnDef` -> `FnPtr` to a cast statement
   in `simplify_constants.rs:117-128`. RegseqModel keeps function
   pointers as first-class const values (vtable shim relocations).
   The demotion is a Charon-specific simplification for Aeneas
   consumers, not a general cleanup we should adopt.

4. **Open -- whether to mirror Charon's `Adt(Option<VariantId>,
   fields)` shape or split struct/tuple from enum.** Charon collapses
   struct/tuple/enum into a single `Adt` variant with `Option<
   VariantId>` distinguishing them. Cleaner: separate `Aggregate(
   Vec<ConstId>)` and `Enum { variant_idx, payload }` variants per
   stage-4/status.md draft. Charon's collapsed shape doesn't earn
   anything for us if struct/enum/tuple have different stable-
   encoding rules (stage 4 likely yes -- enum needs variant index,
   struct doesn't).

5. **Open -- whether `RawMemory(Vec<Byte>)` should be a stage-4
   variant or a follow-on (stage > 4).** Stage-4/expectations
   mentions byte arrays explicitly (item 2: "byte arrays") but does
   not specify whether they get a structured `Bytes(Vec<u8>)` variant
   or a structured `RawMemory(Vec<Byte>)` (with the three-state
   byte). Distinguishing factors: do v1 driver fixtures need the
   uninit-byte distinction in non-vtable contexts? If yes: include
   `RawMemory` from day one. If no: defer to a later stage. The
   pl011 fixture (mentioned in `../../status.md`) is the obvious
   first probe.

6. **Hypothesis -- the post-simplify "only Literal+Var survive" shape
   is anti-pattern for stage 4 and we should NOT borrow it.** The
   tradeoff: simpler downstream IR (Aeneas-style verification consumer
   benefits) at the cost of lots of statement insertion. Stage 4's
   downstream consumer is C codegen, which directly needs typed
   const values as initialisers; turning them into statements would
   force codegen to track local-init-vs-static-init separately. Worth
   cross-checking with the codegen team and stage-4/status.md
   "renderer style for `render_const`."
