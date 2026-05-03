# Prior Art: rustc -- Constant Value Representation

Primary-source research for stage-4 of the RegseqModel refactor
(D-4-1: `Const` enum variant set). All citations are to the pinned
nightly `nightly-2026-02-01-aarch64-apple-darwin` rustc-src checkout
under

```
~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/
```

(paths below are relative to that root).

This is the source-of-truth study: rustc is what the bridge translates
*from*, so the variant set, type-tag placement, relocation handling,
init-mask design, and post-mono guarantee here directly constrain
RegseqModel's `Const`.

This file deliberately does NOT consult `stage-0/prior-art/`. It cites
rustc source headers and module docs directly.

---

## Source map

| Topic                                      | File                                                          | Anchor lines |
| ------------------------------------------ | ------------------------------------------------------------- | ------------ |
| Generic `ConstKind` enum (type-system)     | `rustc_type_ir/src/const_kind.rs`                             | 21-49        |
| `ValTreeKind` (type-system value form)     | `rustc_type_ir/src/const_kind.rs`                             | 131-164      |
| `ty::Const<'tcx>` interned wrapper          | `rustc_middle/src/ty/consts.rs`                               | 27-29        |
| `ty::Const::new_value` constructor          | `rustc_middle/src/ty/consts.rs`                               | 124-127      |
| `ty::Value<'tcx>` (typed valtree pair)      | `rustc_middle/src/ty/consts/valtree.rs`                       | 95-100       |
| `ValTreeKind` doc-comment                   | `rustc_type_ir/src/const_kind.rs`                             | 131-148      |
| `mir::Const<'tcx>` (3-variant union)        | `rustc_middle/src/mir/consts.rs`                              | 232-253      |
| `mir::ConstValue` (4-variant CTFE result)   | `rustc_middle/src/mir/consts.rs`                              | 33-71        |
| `ConstAlloc<'tcx>` (alloc-id + ty)          | `rustc_middle/src/mir/consts.rs`                              | 21-27        |
| `mir::Const::eval` (entry to evaluation)    | `rustc_middle/src/mir/consts.rs`                              | 332-361      |
| `mir::Const::is_deterministic`              | `rustc_middle/src/mir/consts.rs`                              | 478-509      |
| `Scalar` enum                               | `rustc_middle/src/mir/interpret/value.rs`                     | 22-34        |
| `Scalar::null_ptr` constructor              | `rustc_middle/src/mir/interpret/value.rs`                     | 121-123      |
| `Scalar::from_pointer`                      | `rustc_middle/src/mir/interpret/value.rs`                     | 104-107      |
| `ScalarInt` packed struct                   | `rustc_middle/src/ty/consts/int.rs`                           | 152-159      |
| `AllocId(NonZero<u64>)`                     | `rustc_middle/src/mir/interpret/mod.rs`                       | 87-88        |
| `GlobalAlloc<'tcx>` (5 variants)            | `rustc_middle/src/mir/interpret/mod.rs`                       | 259-276      |
| `tcx.global_alloc` query                    | `rustc_middle/src/mir/interpret/mod.rs`                       | 549          |
| `Allocation<Prov, Extra, Bytes>`            | `rustc_middle/src/mir/interpret/allocation.rs`                | 92-116       |
| `ConstAllocation<'tcx>` (interned)          | `rustc_middle/src/mir/interpret/allocation.rs`                | 285-302      |
| `InitMask` (lazy bitmask)                   | `rustc_middle/src/mir/interpret/allocation/init_mask.rs`      | 14-34        |
| `InitMask::is_range_initialized` API        | `rustc_middle/src/mir/interpret/allocation/init_mask.rs`      | 39-64        |
| `ProvenanceMap<Prov>` (relocations)         | `rustc_middle/src/mir/interpret/allocation/provenance_map.rs` | 35-43        |
| `CtfeProvenance(NonZero<u64>)`              | `rustc_middle/src/mir/interpret/pointer.rs`                   | 87-167       |
| `Provenance` trait                          | `rustc_middle/src/mir/interpret/pointer.rs`                   | 59-89        |
| `GenericArgKind::Const(Const)`              | `rustc_middle/src/ty/generic_args.rs`                         | 178-180+     |
| Codegen post-mono panic                     | `rustc_codegen_ssa/src/mir/constant.rs`                       | 23-29        |
| `HashStable for AllocId` (resolves target!) | `rustc_middle/src/ty/impls_ty.rs`                             | 67-76        |
| `HashStable for CtfeProvenance`             | `rustc_middle/src/ty/impls_ty.rs`                             | 78-82        |
| `rustc_public::TyConst` / `TyConstKind`     | `rustc_public/src/ty.rs`                                      | 117-152      |
| `rustc_public::MirConst` / `ConstantKind`   | `rustc_public/src/ty.rs`                                      | 159-1365     |
| `rustc_public::Allocation`                  | `rustc_public/src/ty.rs`                                      | 1280-1354    |
| Const-eval entry / `valtree_to_const_val`   | `rustc_middle/src/mir/consts.rs`                              | 332-361      |

The relevant doc comments at `ValTreeKind` (lines 131-142, the
"`ValTree` does not have this problem with representation" paragraph)
and at `ConstValue` line 29 ("`Scalar` and `Slice` are optimizations")
are load-bearing for understanding the design and are quoted in detail
in Section 1 below.

---

## 1. Const-value types: variant lists, payload shapes, two-layer
relationship

Rust has **two distinct const-value representations** in `rustc_middle`,
serving different purposes. Stage 4's bridge consumes both surfaces
because both are reachable from `optimized_mir` post-mono.

### 1.1 Type-system layer: `ty::Const<'tcx>` over `ConstKind`

`ty::Const<'tcx>` is a 1-word interned wrapper
(`rustc_middle/src/ty/consts.rs:27-29`):

```rust
pub struct Const<'tcx>(pub(super) Interned<'tcx, WithCachedTypeInfo<ConstKind<'tcx>>>);
```

It defers to `ConstKind` (defined generically over `Interner` in
`rustc_type_ir/src/const_kind.rs:21-49`):

```rust
pub enum ConstKind<I: Interner> {
    Param(I::ParamConst),
    Infer(InferConst),
    Bound(BoundVarIndexKind, ty::BoundConst<I>),
    Placeholder(ty::PlaceholderConst<I>),
    Unevaluated(ty::UnevaluatedConst<I>),
    Value(I::ValueConst),
    Error(I::ErrorGuaranteed),
    Expr(I::ExprConst),
}
```

(`rustc_middle` instantiates `I = TyCtxt<'tcx>` so `ConstKind<'tcx>` is
the concrete type, see `rustc_middle/src/ty/consts.rs:21`.)

**Eight variants**, but the post-mono surface stage 4 consumes is
**`Value` and `Unevaluated` only** (see Section 7 for what survives).
The static-assert at line 25 fixes
`size_of::<ConstKind<'tcx>>() == 24` bytes.

The `Value` variant carries `ty::Value<'tcx>`
(`rustc_middle/src/ty/consts/valtree.rs:95-100`):

```rust
pub struct Value<'tcx> {
    pub ty: Ty<'tcx>,
    pub valtree: ValTree<'tcx>,
}
```

Note **the type travels with the value** at the type-system layer --
this is significant for stage-4's type-tag-placement question
(see Section 2).

### 1.2 The `ValTree` representation: padding-free aggregates

`ValTree<'tcx>` is itself an interned wrapper
(`rustc_middle/src/ty/consts/valtree.rs:32`) over `ValTreeKind`
(`rustc_type_ir/src/const_kind.rs:149-164`):

```rust
pub enum ValTreeKind<I: Interner> {
    /// integers, `bool`, `char` are represented as scalars.
    /// See the `ScalarInt` documentation for how `ScalarInt` guarantees that equal values
    /// of these types have the same representation.
    Leaf(I::ScalarInt),

    /// The fields of any kind of aggregate. Structs, tuples and arrays are represented by
    /// listing their fields' values in order.
    ///
    /// Enums are represented by storing their variant index as a u32 field, followed by all
    /// the fields of the variant.
    ///
    /// ZST types are represented as an empty slice.
    Branch(Box<[I::Const]>),
}
```

**Only two variants.** The doc comment at lines 131-142 of
`const_kind.rs` is critical -- it states the **design intent** that
gives valtrees their canonicalisation property:

> "`ValTree` does not have this problem with representation, as it only
> contains integers or lists of (nested) `ty::Const`s (which may
> indirectly contain more `ValTree`s)."

The "this problem" refers to:

> "Since CTFE uses `AllocId` to represent pointers, it often happens
> that two different `AllocId`s point to equal values. So we may end up
> with different representations for two constants whose value is
> `&42`. Furthermore any kind of struct that has padding will have
> arbitrary values within that padding, even if the values of the
> struct are the same."

**Direct interpretation:** valtrees deliberately strip allocation-
identity and padding-byte indeterminism so that `==` agrees with
semantic equality. This is **exactly the canonicalisation property
stage 4 needs** for `ConstId` content uniqueness via
`GenericArg::Const` -- see Section 8.

`ValTree::zst()` returns `Branch([])` (`valtree.rs:42-44`); ZST is
represented as the empty branch. Bytes literals like `&[u8]` are built
by `ValTree::from_raw_bytes` (`valtree.rs:50-55`) which folds each byte
into a leaf-wrapped `Const::new_value(tcx, ..., u8)`.

Enum variant index is a `u32` field stored as the **first** field of
the branch (the method `destructure_adt_const` at
`valtree.rs:200-217` extracts it: line 209
`(VariantIdx::from_u32(head.to_leaf().to_u32()), rest)`).

Note one limitation called out at line 92-94:

> "Note that this is also used by pattern elaboration to represent
> values which cannot occur in types, such as raw pointers and floats."

So `ty::Value` covers more than strict generic-const usage.

### 1.3 MIR layer: `mir::Const<'tcx>` over `ConstValue`

The MIR layer has its **own** const enum
(`rustc_middle/src/mir/consts.rs:232-253`):

```rust
pub enum Const<'tcx> {
    /// This constant came from the type system.
    Ty(Ty<'tcx>, ty::Const<'tcx>),

    /// An unevaluated mir constant which is not part of the type system.
    Unevaluated(UnevaluatedConst<'tcx>, Ty<'tcx>),

    /// This constant cannot go back into the type system, as it represents
    /// something the type system cannot handle (e.g. pointers).
    Val(ConstValue, Ty<'tcx>),
}
```

**Three variants.** Critical observation from the doc comments:

- `Ty(Ty, ty::Const)` -- value originates in the type system
  (`mir/consts.rs:233-241`). Comment at line 235-237:
  *"Any way of turning ty::Const into ConstValue should go through
  valtree_to_const_val; this ensures that we consistently produce
  'clean' values without data in the padding or anything like that."*
- `Unevaluated(UnevaluatedConst, Ty)` -- unevaluated MIR constant. Note
  that `Ty(ty::ConstKind::Unevaluated)` and `mir::Const::Unevaluated`
  are *not* identical (line 244-247):
  *"`Ty` will always flow through a valtree, so all data not captured
  in the valtree is lost. This variant directly uses the evaluated
  result of the given constant, including e.g. data stored in
  padding."*
- `Val(ConstValue, Ty)` -- evaluated CTFE result (line 250-252):
  *"This constant cannot go back into the type system, as it represents
  something the type system cannot handle (e.g. pointers)."*

The MIR layer **always pairs the value with its type** -- every variant
carries a `Ty<'tcx>`. The bridge will see `mir::Const` whenever it
reads operands, statements, terminators, and `var_debug_info` from a
`Body`.

`ConstValue` itself is the post-eval MIR result form
(`rustc_middle/src/mir/consts.rs:33-71`):

```rust
pub enum ConstValue {
    Scalar(Scalar),
    ZeroSized,
    Slice { alloc_id: AllocId, meta: u64 },
    Indirect { alloc_id: AllocId, offset: Size },
}
```

Static-assert at line 73-74 fixes
`size_of::<ConstValue>() == 24` bytes.

Comments at lines 29-30, 39, 42-47, 57-59 declare the design:

- `Scalar` is for "types with `layout::abi::Scalar` ABI" (line 34) and
  must not be `Uninit` (line 36).
- `ZeroSized` is "Only for ZSTs" (line 39).
- `Slice { alloc_id, meta }` is the **slice-tail optimisation**
  (lines 43-47): *"This is worth an optimized representation since Rust
  has literals of type `&str` and `&[u8]`. Not having to indirect those
  through an `AllocId` (or two, if we used Indirect) has shown
  measurable performance improvements on stress tests. We then reuse
  this optimization for slice-tail types more generally during
  valtree-to-constval conversion."*
- `Indirect { alloc_id, offset }` is the catch-all for in-memory
  constants (lines 57-70): *"A value not representable by the other
  variants; needs to be stored in-memory. Must *not* be used for
  scalars or ZST, but having `&str` or other slices in this variant
  is fine."*

### 1.4 The two-layer relationship

```
                   Source / HIR const expression
                              │
                              ▼
                   ┌──────────┴──────────┐
                   │                     │
              type system            MIR-only
              (generic args,    (operand constants,
               array lengths,    static initialisers,
               etc.)              vtable values)
                   │                     │
                   ▼                     ▼
            ty::Const<'tcx>        mir::Const<'tcx>
            (8-variant ConstKind)   ┌── Ty(Ty, ty::Const)        --- comes from ty layer
                   │                ├── Unevaluated(UnevaluatedConst, Ty)
                   ▼                └── Val(ConstValue, Ty)
            ConstKind::Value(Value)
                   │                       │
                   ▼                       ▼
            ValTreeKind                ConstValue
            ┌── Leaf(ScalarInt)        ├── Scalar(Scalar)
            └── Branch([Const])        ├── ZeroSized
                                       ├── Slice { alloc_id, meta }
                                       └── Indirect { alloc_id, offset }
                                                       │
                                                       ▼
                                             AllocId → GlobalAlloc → 5 variants
                                             (Function | VTable | Static | Memory | TypeId)
```

The `mir::Const::eval` method (`mir/consts.rs:332-361`) is the bridge
*from* the MIR layer to a fully-evaluated `ConstValue`:

```rust
pub fn eval(self, tcx, typing_env, span) -> Result<ConstValue, ErrorHandled> {
    match self {
        Const::Ty(_, c) => match c.kind() {
            ConstKind::Value(cv) => Ok(tcx.valtree_to_const_val(cv)),
            ConstKind::Expr(_) => bug!("Normalization of `ty::ConstKind::Expr` is unimplemented"),
            _ => Err(ReportedErrorInfo::non_const_eval_error(...)),
        },
        Const::Unevaluated(uneval, _) => tcx.const_eval_resolve(typing_env, uneval, span),
        Const::Val(val, _) => Ok(val),
    }
}
```

So `valtree_to_const_val` is the canonicaliser that converts type-
system valtrees into MIR `ConstValue`. Looking at the codegen-side
behaviour at `rustc_codegen_ssa/src/mir/constant.rs:23-29`:

```rust
pub fn eval_mir_constant(&self, constant: &mir::ConstOperand<'tcx>) -> mir::ConstValue {
    // `MirUsedCollector` visited all required_consts before codegen began, so if we got here
    // there can be no more constants that fail to evaluate.
    self.monomorphize(constant.const_)
        .eval(self.cx.tcx(), self.cx.typing_env(), constant.span)
        .expect("erroneous constant missed by mono item collection")
}
```

By the time codegen runs, **every required const has already been
evaluated** (panic message at line 28). RegseqModel construction runs
after this point, so the bridge can safely call `eval` on every
encountered `mir::Const` and expect a `ConstValue` back. See Section 7
for the full post-mono guarantee.

### 1.5 Comparison: rustc_public's collapsed shapes

`rustc_public` (the `'tcx`-erased SMIR replacement) provides a
flatter, owned IR that is closer to what RegseqModel will be. Its
const story is reported here as confirmation evidence (not as
authoritative -- rustc_middle is the source).

`TyConst` (`rustc_public/src/ty.rs:117-152`):

```rust
pub struct TyConst { pub(crate) kind: TyConstKind, pub id: TyConstId }

pub enum TyConstKind {
    Param(ParamConst),
    Bound(DebruijnIndex, BoundVar),
    Unevaluated(ConstDef, GenericArgs),
    // FIXME: These should be a valtree
    Value(Ty, Allocation),
    ZSTValue(Ty),
}
```

Note rustc_public **drops the valtree representation** and replaces it
with `Value(Ty, Allocation)` -- using the post-eval byte-buffer form
even at the type-system layer. The FIXME at line 149 acknowledges this
is a regression vs rustc-internal canonicalisation.

`MirConst` (`rustc_public/src/ty.rs:158-216`) carries
`ConstantKind` (lines 1356-1365):

```rust
pub enum ConstantKind {
    Ty(TyConst),
    Allocated(Allocation),
    Unevaluated(UnevaluatedConst),
    Param(ParamConst),
    /// Store ZST constants.
    ZeroSized,
}
```

`rustc_public::Allocation` (`ty.rs:1280-1286`) is a flat owned
struct:

```rust
pub struct Allocation {
    pub bytes: Bytes,
    pub provenance: ProvenanceMap,
    pub align: Align,
    pub mutability: Mutability,
}
```

Note no `init_mask`. Fields use `Bytes = Vec<Option<u8>>` per the
`raw_bytes()` method (line 1290-1296), where each byte is `None` if
uninitialised. **Per-byte init tracking moved into the byte-vector
itself.** The relocation map is preserved (`ProvenanceMap.ptrs:
Vec<(Size, Prov)>`, line 1276-1278).

The `is_null` check at lines 1346-1353 illustrates the canonicalisation
form for typed-null:

```rust
pub fn is_null(&self) -> Result<bool, Error> {
    let len = self.bytes.len();
    let ptr_len = MachineInfo::target_pointer_width().bytes();
    if len != ptr_len { return Err(error!(...)); }
    Ok(self.read_uint()? == 0 && self.provenance.ptrs.is_empty())
}
```

i.e., a typed null pointer is "all zero bytes of pointer width with
empty provenance map." Stage 4 will need a similar check.

---

## 2. Type-tag placement for scalars

**Rustc's posture: type tag is *external* to the value bits, not inline
in scalar variants.**

Evidence:

### 2.1 At the type-system layer

`ScalarInt` (`rustc_middle/src/ty/consts/int.rs:152-159`) is a packed
`(data: u128, size: NonZero<u8>)` with no Rust-type information:

```rust
#[repr(packed)]
pub struct ScalarInt {
    /// The first `size` bytes of `data` are the value.
    /// Do not try to read less or more bytes than that. The remaining bytes must be 0.
    data: u128,
    size: NonZero<u8>,
}
```

The doc-comment at lines 156-158 is significant: only `size` bytes are
the value; the rest must be 0. The `try_to_uint` family of methods
(`int.rs:191+`) takes a `Size` argument explicitly because they don't
know the type. The static-assert that `Bool=1u128` and `False=0u128`
(line 192-193) shows even bool packs as u128.

`ValTreeKind::Leaf(I::ScalarInt)` therefore carries no Rust type;
canonicalisation across `bool`/`char`/`u8`/`u32` of the same numeric
value would collide if you only had the leaf. The type tag lives on
the parent `ty::Value { ty: Ty<'tcx>, valtree: ValTree<'tcx> }`
(`valtree.rs:97-100`).

### 2.2 At the MIR layer

`ConstValue::Scalar(Scalar)` (`mir/consts.rs:37`) wraps a `Scalar`
which is `Int(ScalarInt) | Ptr(Pointer<Prov>, u8)`
(`interpret/value.rs:22-34`) -- still no Rust type. Type tag lives on
the parent `mir::Const::Val(ConstValue, Ty)` (line 252).

### 2.3 Practical consequence

Type identity for scalar constants comes from the *enclosing pair*,
not from the variant. Two `ScalarInt`s with the same `(data, size)`
are equal; the same `(data, size)` interpreted as `u32` vs `i32` vs
`bool` is identified only by the surrounding `Ty`.

**Stage-4 implication:**
spec.md §9.7 already pins the *opposite* posture for RegseqModel:
*"scalar primitives (Bool, Char, Int(IntKind, value), Uint(UintKind,
value), Float(FloatKind, bits))"* -- inline type tag in the variant.
This is a deliberate divergence from rustc's design. Two reasons that
make the divergence reasonable for our context:

1. Stage 1's F4-2 `ArgsKey` fingerprint reaches scalar consts via
   `GenericArg::Const(ConstId)`; if scalar variants don't carry their
   own type info, the `ArgsKey` canonicaliser must thread `Ty`
   alongside every `ConstId` reference -- an extra field per generic
   arg slot.
2. RegseqModel is post-mono and rustc-free; we don't need the
   `ScalarInt` packing trick (which exists for inline-in-enum size
   reasons inside a `'tcx`-arena-allocated structure).

But it is a divergence worth flagging; rustc's evidence supports the
opposite design. (Hypothesis: rustc's choice is driven by `'tcx`-
interning constraints we don't have.)

---

## 3. Aggregate constants

### 3.1 At the type-system layer: `ValTree::Branch`

For type-system constants, aggregates are nested `ValTree::Branch`:

> "The fields of any kind of aggregate. Structs, tuples and arrays are
> represented by listing their fields' values in order. Enums are
> represented by storing their variant index as a u32 field, followed
> by all the fields of the variant. ZST types are represented as an
> empty slice."
> -- `rustc_type_ir/src/const_kind.rs:155-161`

So:

- **Struct/tuple:** `Branch([field_0, field_1, ...])` -- one entry per
  field, in declaration order.
- **Array:** `Branch([elem_0, elem_1, ..., elem_n-1])` -- one entry per
  element.
- **Enum:** `Branch([variant_idx_const, payload_field_0, ...])` -- the
  variant index is folded in as a u32 ScalarInt leaf in slot 0,
  followed by the variant's fields.
- **ZST:** `Branch([])` -- empty.

Each child is itself a `ty::Const`, which is interned. So aggregate
sharing happens via `ty::Const` interning -- two structurally equal
aggregates share child `Const` IDs.

`Vec<u8>` literals (`&[u8; N]`, `&str`) flow through
`ValTree::from_raw_bytes` (`valtree.rs:50-55`): each byte becomes a
`u8`-typed `ScalarInt`-leaf `ty::Const`, and they're collected into a
branch. There's no specialised "byte buffer" variant at the type-
system layer; bytes are just an array of u8 leaves.

`destructure_adt_const` (`valtree.rs:200-217`) is the splitter that
extracts the variant index (line 209) and remaining fields based on
the parent `ty.kind()`.

### 3.2 At the MIR layer: `Indirect` / `Slice`

Once a const has crossed into MIR via `valtree_to_const_val`, the
representation flips: the canonicalised valtree is **flattened to bytes**
in an `Allocation`, with `AllocId`-indirected children for embedded
relocations.

For arrays/structs/tuples large enough not to fit a `Scalar`, the
result is `ConstValue::Indirect { alloc_id, offset }` -- a pointer
into a byte buffer in tcx's global allocation map. The `Allocation`
stores bytes plus an `init_mask` plus a `provenance` map for any
embedded pointers (see Section 4).

For the slice-tail optimisation (`&str`, `&[u8]`, `&[T]`),
`ConstValue::Slice { alloc_id, meta: u64 }` is used (line 48-55,
"This is worth an optimized representation since Rust has literals of
type `&str` and `&[u8]`"). `meta` is the slice length; `alloc_id`
points at the data.

For ZSTs, `ConstValue::ZeroSized` -- no payload (line 39-40).

### 3.3 What's NOT in the MIR layer

There is **no `Aggregate(Vec<ConstId>)` style variant in `ConstValue`.**
Once you cross into MIR via `valtree_to_const_val`, the structure is
flattened to bytes-with-relocations. Even if the const came from a
nested `ValTree::Branch`, the resulting `ConstValue::Indirect` points
at an `Allocation` whose bytes encode the aggregate per the layout
rules.

**Stage-4 implication:** rustc's evidence supports two distinct
designs depending on whether stage 4 wants to preserve the valtree
shape (interner-of-children) or the MIR shape (bytes-with-relocations).
The bridge will see *both* surfaces -- generic-const args go through
`ty::Const::Value` (valtree shape); operand/static initialiser
constants go through `mir::ConstValue` (bytes shape). Stage 4 will
need to canonicalise across both. A single-shape `Const` enum (e.g.,
"only valtree-style") forces conversion at the bridge boundary.

---

## 4. Relocation handling (`AllocId`, `GlobalAlloc`,
`ProvenanceMap`)

Rustc's relocation model is the most directly relevant prior art for
RegseqModel's **stable cross-build relocation encoding** (spec.md
§9.7).

### 4.1 The indirection: `AllocId`

`AllocId` (`rustc_middle/src/mir/interpret/mod.rs:87-88`):

```rust
pub struct AllocId(pub NonZero<u64>);
```

A 64-bit non-zero handle. **Build-local**: the Debug printing at lines
92-95 confirms it's printed as `alloc{N}` because IDs are not
user-visible. AllocIds are reserved-on-demand
(`reserve_alloc_id` at line 462) and stored in
`tcx.alloc_map.to_alloc: ShardedHashMap<AllocId, GlobalAlloc<'tcx>>`
(line 419-426).

**Crucially: AllocId is NOT cross-build stable on its own.** This is
exactly RegseqModel's problem with `InstanceId` integers in
`ConstIdentityKey`. Section 8 below covers how rustc stabilises this.

### 4.2 The targets: `GlobalAlloc<'tcx>`

`GlobalAlloc` (`mir/interpret/mod.rs:259-276`) has 5 variants:

```rust
pub enum GlobalAlloc<'tcx> {
    Function { instance: Instance<'tcx> },
    VTable(Ty<'tcx>, &'tcx ty::List<ty::PolyExistentialPredicate<'tcx>>),
    Static(DefId),
    Memory(ConstAllocation<'tcx>),
    TypeId { ty: Ty<'tcx> },
}
```

Each AllocId points at one of:

- **Function pointer** (line 261-262): the AllocId is a function-pointer
  relocation; resolving via `tcx.global_alloc(id).unwrap_fn()` returns
  the `Instance<'tcx>` it points at.
- **Vtable** (lines 263-267): "symbolic (not-reified) vtable"; we
  remember the *full* dyn type, not just the principal trait, "so that
  const-eval and Miri can detect UB due to invalid transmutes of `dyn
  Trait` types." Comparable in role to RegseqModel's `Ty::Dynamic`
  vtable instance reference.
- **Static** (line 268-270): `DefId` of a static item; the AllocId is a
  reference *to* the static. Note: "lazy" -- the actual initialiser
  may not have been computed yet at this point.
- **Memory** (line 272): a `ConstAllocation<'tcx>` -- this is the byte
  buffer storing actual constant data.
- **TypeId** (line 274-275): a synthetic `core::any::TypeId` value.

Reservation per kind has a separate API: `reserve_and_set_static_alloc`
(line 497-501), `reserve_and_set_fn_alloc` (line 502-506),
`reserve_and_set_vtable_alloc` (line 508+),
`reserve_and_set_type_id_alloc` (line 518+),
`reserve_and_set_memory_dedup` (line 489-493). Notable: function and
vtable kinds are deduplicated; multiple references to the same
function/vtable share an AllocId.

**Stage-4 implication:** GlobalAlloc's 5-variant union maps almost
1-1 to RegseqModel's relocation flavours:

| `GlobalAlloc<'tcx>` variant     | RegseqModel equivalent                                  |
| ------------------------------- | ------------------------------------------------------- |
| `Function { instance }`         | `InstanceRef` to `InstanceKind::Function`               |
| `VTable(ty, dyn_preds)`         | `InstanceRef` to vtable instance Item (§13.2)           |
| `Static(DefId)`                 | `InstanceRef` to `InstanceKind::Static`                 |
| `Memory(ConstAllocation)`       | embedded byte buffer (handled within `Const` itself)    |
| `TypeId { ty }`                 | not in v1 scope (no `core::any::TypeId` support)        |

The 4-tuple stable encoding RegseqModel uses
(`(kind_rank, def_path_hash, args, shim_disambig)`) per spec.md §9.7
mirrors how rustc resolves AllocIds across builds: the AllocId itself
is just a handle; the target's identity (the `Instance`'s DefId+args,
or the static's DefId, or the vtable's `(Ty, dyn_preds)`) is the
stable identity carrier.

### 4.3 The provenance map: how relocations live inside bytes

Inside a `ConstAllocation`'s bytes, the `ProvenanceMap`
(`mir/interpret/allocation/provenance_map.rs:35-43`) encodes which
byte ranges contain pointer values:

```rust
pub struct ProvenanceMap<Prov = CtfeProvenance> {
    /// `Provenance` in this map applies from the given offset for an entire pointer-size worth of
    /// bytes. Two entries in this map are always at least a pointer size apart.
    ptrs: SortedMap<Size, Prov>,
    /// This stores byte-sized provenance fragments.
    bytes: Option<Box<SortedMap<Size, PointerFrag<Prov>>>>,
}
```

- `ptrs: SortedMap<Size, Prov>` -- "Provenance in this map applies from
  the given offset for an entire pointer-size worth of bytes" (line 38-
  40). Each entry says "starting at byte offset X, the next
  pointer-size bytes are a pointer with provenance Prov."
- `bytes` -- per-byte provenance fragments for the unusual case where
  a pointer is written byte-by-byte without preserving alignment;
  encoded as `PointerFrag { idx: u8, prov: Prov, bytes: [u8; 8] }`
  (line 21-31).

For the interned case (`ConstAllocation`), `bytes` is always `None` --
see `ProvenanceMap::ptrs()` accessor at line 70-77:
*"Only use on interned allocations, as other allocations may have
per-byte provenance!"* (line 73). The encoder confirms this at line
54-58:
*"`bytes` is not in the serialized format. ... assert!(bytes.is_none());
// interning refuses allocations with pointer fragments"*.

**Provenance type for CTFE.** `CtfeProvenance(NonZero<u64>)`
(`mir/interpret/pointer.rs:91`) is a packed value holding both an
`AllocId` and two flag bits (immutable, shared-ref):

```rust
pub struct CtfeProvenance(NonZero<u64>);
impl From<AllocId> for CtfeProvenance {
    fn from(value: AllocId) -> Self {
        let prov = CtfeProvenance(value.0);
        // ... (sets flags as needed)
    }
}
```

### 4.4 End-to-end relocation example

A vtable instance with a function-pointer field at offset 16 looks like:

```
ConstAllocation {
    bytes:        [.., .., .., .., .., .., .., .., .., .., .., .., .., .., .., .., 0, 0, 0, 0, 0, 0, 0, 0, ..],
                                                                                        ^^^^^^^^^^^^^^^^^^^^^^
                                                                                        offset 16, pointer-sized
    provenance: { 16: CtfeProvenance(alloc_id for VTableMethodShim) }
    init_mask:    [16..24 are all initialised]
    align:        target.scalar_layouts.fn_ptr.abi_align
    mutability:   Mutability::Not
}
```

The byte content at offset 16 is **uninformative on its own** (it might
be zero; rustc doesn't "embed" the actual function address since the
build doesn't have that yet). The relocation is what carries the
identity. At codegen time, the linker fills in the bytes with the
function's actual address.

Stage 4 must preserve this exact discipline: the byte content of a
relocation slot is irrelevant for canonicalisation; only the relocated-
to identity matters.

---

## 5. Null pointer typing

**Rustc represents null pointers as `Scalar::Int(ScalarInt::null(...))`
-- a non-pointer scalar of pointer width, with provenance NOT in the
provenance map.**

Evidence at `rustc_middle/src/mir/interpret/value.rs:121-123`:

```rust
#[inline]
pub fn null_ptr(cx: &impl HasDataLayout) -> Self {
    Scalar::Int(ScalarInt::null(cx.pointer_size()))
}
```

So `Scalar::null_ptr` produces a `Scalar::Int` (variant 0 of Scalar,
not the `Ptr` variant). It's a scalar-int of `pointer_size` bytes,
all zero.

`Scalar::from_pointer` (line 104-107) is for **non-null** pointers:

```rust
pub fn from_pointer(ptr: Pointer<Prov>, cx: &impl HasDataLayout) -> Self {
    Scalar::Ptr(ptr, u8::try_from(cx.pointer_size().bytes()).unwrap())
}
```

`Scalar::from_maybe_pointer` (line 110-118) handles the union case:

```rust
pub fn from_maybe_pointer(ptr: Pointer<Option<Prov>>, cx: &impl HasDataLayout) -> Self {
    match ptr.into_raw_parts() {
        (Some(prov), offset) => Scalar::from_pointer(Pointer::new(prov, offset), cx),
        (None, offset) => {
            Scalar::Int(ScalarInt::try_from_uint(offset.bytes(), cx.pointer_size()).unwrap())
        }
    }
}
```

Note lines 113-118: when there's no provenance, the result is
`Scalar::Int(ScalarInt::try_from_uint(offset, ptr_size))` -- a typed
int of pointer width. So a "null pointer" is just an int of pointer
size with value 0; a "dangling integer pointer" (e.g., `0x1234 as
*const u8`) is the same form with non-zero value.

**Where is the type tag?** On the enclosing `mir::Const::Val(_, ty)`
(line 252) -- the pointee type lives in the surrounding `Ty`, not in
the Scalar.

**For `Indirect` representation** (e.g., a struct field that's a null
pointer), the same encoding applies inside an `Allocation`:

- bytes are zero-filled for `pointer_size` bytes
- `init_mask` says those bytes are initialised (it's a real null, not
  uninit)
- `provenance.ptrs` does NOT contain an entry for that offset (because
  there's no provenance)

This is exactly how `rustc_public::Allocation::is_null` (`ty.rs:1346-
1353`) tests it:

```rust
Ok(self.read_uint()? == 0 && self.provenance.ptrs.is_empty())
```

**Stage-4 implication:** rustc supports two equivalent shapes for
typed null:

1. **Inline typed null** -- a dedicated `Const::NullPtr(TyId)` variant
   at the variant level (mentioned in spec.md §9.7's "typed null
   pointers (one per pointee type)").
2. **Bytes-with-no-provenance** -- represent null as a byte buffer of
   pointer-width zeros, equivalent to any other constant-zero scalar
   of that width, distinguished only by the surrounding `Ty`.

Rustc takes shape (2). The choice between them affects:

- Identity-key derivation -- inline form has a local key
  `(NullPtr_tag, TyId)`; bytes form has the same key as any other
  zero-pointer-width allocation, requiring the surrounding `Ty` for
  disambiguation.
- Renderer style -- inline is easier to print as `null::<*const u8>()`.
- Validation -- inline can be validated without TyId resolution
  (only "is this TyId a Ref/Ptr?"); bytes form requires reading the
  init_mask and provenance map.

The vacant-vtable-slot use case in spec.md §13.2 ("Vacant slots are
explicit placeholder fn-pointer fields with a fixed null initialiser,
NOT omitted") strongly favours the inline form -- vacant slots are
**typed** placeholders and need to canonicalise with the slot's
fn-pointer type. Per-pointee-type inline nulls give that for free.

---

## 6. Uninit / padding bytes

### 6.1 At the MIR / CTFE layer: `InitMask`

`InitMask` (`rustc_middle/src/mir/interpret/allocation/init_mask.rs:14-
34`) is a per-byte initialisation tracker:

```rust
/// A bitmask where each bit refers to the byte with the same index. If the bit is `true`, the byte
/// is initialized. If it is `false` the byte is uninitialized.
/// The actual bits are only materialized when needed, and we try to keep this data lazy as long as
/// possible. Currently, if all the blocks have the same value, then the mask represents either a
/// fully initialized or fully uninitialized const allocation, so we can only store that single
/// value.
pub struct InitMask {
    blocks: InitMaskBlocks,
    len: Size,
}

enum InitMaskBlocks {
    Lazy { state: bool },
    Materialized(InitMaskMaterialized),
}
```

**Two-level representation:**

- `Lazy { state }` -- the entire mask is "all init" or "all uninit",
  no bit storage needed. This is the common case for fully-initialised
  scalar/aggregate allocations.
- `Materialized(blocks)` -- a real bitmask, one bit per byte, stored
  as `Vec<u64>` blocks. Only allocated when there's a mix of init and
  uninit bytes (e.g., a struct with padding).

The API is range-oriented (`init_mask.rs:39-64`):

```rust
pub fn is_range_initialized(&self, range: AllocRange) -> Result<(), AllocRange>
pub fn set_range(&mut self, range: AllocRange, new_state: bool)
```

`is_range_initialized` returns `Ok(())` if the range is fully init,
`Err(range)` if any uninit byte is in the range (it returns the first
contiguous span of uninit).

### 6.2 At the type-system layer: not represented

ValTrees deliberately have NO concept of uninit (per the doc comment
already cited at `const_kind.rs:131-142`):

> "any kind of struct that has padding will have arbitrary values
> within that padding, even if the values of the struct are the same"

This is why ValTrees can't be used for typed-null or for raw-byte
constants -- they would lose padding-byte information. The ValTree
form is **canonicalised through layout** rather than carrying padding
explicitly.

### 6.3 What gets serialised

When an `Allocation` is encoded (`allocation.rs:188-205`), `init_mask`
is encoded as part of the structure (line 202: `self.init_mask.encode
(encoder);`). The lazy form is preserved -- `InitMaskBlocks::Lazy` and
`Materialized` are both `Encodable_NoContext` (`init_mask.rs:21-23`).

Stage 4's RegseqModel pretty-printer / serialiser will need a similar
discipline: distinguish "all initialised" from "all uninitialised"
from "specific byte ranges init/uninit", and serialise the
materialised form only when needed.

### 6.4 Implication for `Const::all_bytes_uninit`

The check at `mir/consts.rs:205-224` shows the practical use:

```rust
pub fn all_bytes_uninit(&self, tcx: TyCtxt<'_>) -> bool {
    let ConstValue::Indirect { alloc_id, .. } = self else { return false; };
    let alloc = tcx.global_alloc(*alloc_id);
    let GlobalAlloc::Memory(alloc) = alloc else { return false; };
    let init_mask = alloc.0.init_mask();
    let init_range = init_mask.is_range_initialized(...);
    if let Err(range) = init_range {
        if range.size == alloc.0.size() {
            return true;
        }
    }
    false
}
```

i.e., a "fully uninit constant" is asking "is the entire byte range an
init-mask error?" -- the range API is doing the work.

**Stage-4 implication:** Two implementation choices for stage 4:

1. **Bit-mask-per-allocation** (mirror rustc's InitMask): Stage 4 has a
   bytes-and-init-mask pair on aggregate variants. Distinguishing
   "padding" from "data zero byte" is a query into the mask.
2. **Per-byte option** (rustc_public's choice -- `Vec<Option<u8>>`):
   each byte slot is `Option<u8>` -- `Some(b)` for an initialised byte
   with value `b`, `None` for uninit. Less compact but trivially
   stage-4-canonicalisable byte-by-byte.

Mask-form preserves rustc's lazy optimisation; option-form is simpler
to reason about. The size implication for `Option<ConstId>` 4-byte
niche (Stage-1 invariant) is unchanged either way -- both are inside
the variant payload, not affecting the discriminant size.

---

## 7. Validation phase: when do consts get evaluated, what survives?

This is the **critical question** for the bridge's no-defensive-
fallbacks rule: when does the bridge see a non-evaluated const, and
when is that a bug?

### 7.1 Evaluation triggers

Two main entry points:

- `mir::Const::eval` (`mir/consts.rs:332-361`) -- the per-const
  evaluator, called from codegen and consumers like the mono
  collector. Dispatches:

  - `Const::Ty(_, c)`: extracts the valtree via `valtree_to_const_val`
    if `c.kind() == ConstKind::Value(cv)`; bug for `ConstKind::Expr`;
    error for `Param/Infer/Bound/Placeholder/Unevaluated`.
  - `Const::Unevaluated(uneval, _)`: calls
    `tcx.const_eval_resolve(typing_env, uneval, span)`.
  - `Const::Val(val, _)`: returns as-is (already evaluated).

  Note: `Const::Ty(_, ConstKind::Param(_))` returns
  `ErrorHandled::TooGeneric` (line 340-342) -- stage 4 must **never**
  see this post-mono.

- `MirUsedCollector` -- runs during the mono collection pass. The
  comment at codegen `rustc_codegen_ssa/src/mir/constant.rs:24-25`
  states:

  > "`MirUsedCollector` visited all required_consts before codegen
  > began, so if we got here there can be no more constants that fail
  > to evaluate."

  And the panic message at line 28 is unconditional:

  > `.expect("erroneous constant missed by mono item collection")`

So: **once mono collection completes, all reachable
`required_consts` are evaluated.** Codegen panics on any survivor.

### 7.2 What `required_consts` excludes

`mir::Const::is_required_const` (`mir/consts.rs:288-300`):

```rust
pub fn is_required_const(&self) -> bool {
    match self {
        Const::Ty(_, c) => match c.kind() {
            ty::ConstKind::Value(_) => false, // already a value, cannot error
            _ => true,
        },
        Const::Val(..) => false, // already a value, cannot error
        Const::Unevaluated(..) => true,
    }
}
```

Translation: the only consts that *aren't* on the required list are
those already in `Value` or `Val` form (cannot error). Therefore every
path from a body's reachable code that produces an `Unevaluated` or
non-Value `Ty` const is required to evaluate before codegen.

### 7.3 What can therefore survive into stage 4

Given:
- We read `optimized_mir` post-mono.
- The bridge runs after mono collection (analogous to codegen's
  position).
- All reachable `required_consts` are pre-evaluated.

**Surviving forms:**

1. **`mir::Const::Ty(ty, ty::Const)` where the inner `ConstKind` is
   `Value(cv)`.** The valtree carries the canonicalised value. Bridge
   converts to `RegseqModel::Const` directly via the valtree
   structure.
2. **`mir::Const::Val(ConstValue, ty)`.** Already a CTFE result.
   Bridge converts each `ConstValue` arm to a RegseqModel variant.

**Bridge-bug forms (must reject):**

- `mir::Const::Unevaluated(_, _)` -- collector should have evaluated
  this. Reject with a diagnostic per the no-defensive-fallbacks rule.
- `mir::Const::Ty(_, c)` where `c.kind()` is `Param/Infer/Bound/
  Placeholder/Error/Expr/Unevaluated`. Param/Infer/Bound/Placeholder
  shouldn't survive monomorphisation; Error means the source had an
  earlier const-eval failure (no diagnostic budget); Expr is unstable
  `feature(generic_const_exprs)`; Unevaluated should have been hit by
  the collector.

`mir::Const::is_deterministic` (`mir/consts.rs:478-509`) is the
runtime check rustc uses for "is this const stable across re-eval?":

```rust
pub fn is_deterministic(&self) -> bool {
    // Some constants may generate fresh allocations for pointers they contain,
    // so using the same constant twice can yield two different results.
    // Notably, valtrees purposefully generate new allocations.
    match self {
        Const::Ty(_, c) => match c.kind() {
            ty::ConstKind::Param(..) => true,
            ty::ConstKind::Value(cv) => cv.ty.is_primitive(),
            ty::ConstKind::Unevaluated(..) | ty::ConstKind::Expr(..) => false,
            ty::ConstKind::Error(..) => false,
            ty::ConstKind::Infer(..) | ty::ConstKind::Bound(..) | ty::ConstKind::Placeholder(..) => bug!(),
        },
        Const::Unevaluated(..) => false,
        Const::Val(ConstValue::Slice { .. } | ConstValue::ZeroSized | ConstValue::Scalar(_) | ConstValue::Indirect { .. }, _) => true,
    }
}
```

Note **all `Const::Val` are deterministic** (line 502-507). This is
the cleanest guarantee for stage 4: once we have `Val`, we have a
canonical answer.

For `Const::Ty(_, ConstKind::Value(cv))`, deterministic only for
**primitive types** (line 491). Non-primitive valtrees may produce
fresh allocations on each `valtree_to_const_val` -- exactly the
indeterminism rustc warns against. Stage 4's bridge-side
canonicalisation must be careful: do not call `valtree_to_const_val`
twice on the same valtree expecting equal AllocIds.

### 7.4 The `is_trivially_wf` check

`ty::Const::is_trivially_wf` (`rustc_middle/src/ty/consts.rs:160-171`):

```rust
pub fn is_trivially_wf(self) -> bool {
    match self.kind() {
        ty::ConstKind::Param(_) | ty::ConstKind::Placeholder(_) | ty::ConstKind::Bound(..) => true,
        ty::ConstKind::Infer(_) | ty::ConstKind::Unevaluated(..) | ty::ConstKind::Value(_)
            | ty::ConstKind::Error(_) | ty::ConstKind::Expr(_) => false,
    }
}
```

Notable: `Value` is *not* trivially WF -- it requires checking that
the value matches its type. This validation is part of mono-time
checking (`rustc_const_eval`'s `validate_const_value` and similar).

**Stage-4 implication:** the bridge inherits rustc's mono-time
validation: any `ConstKind::Value(cv)` reaching us has been validated
to match its type. We do *not* re-run this check. (RegseqModel
validation per spec.md §3.3 only checks internal well-formedness, not
re-deriving rustc-side facts -- consistent with the trust model.)

### 7.5 Promoted constants

`UnevaluatedConst<'tcx>` in `mir/consts.rs:512-527` carries an optional
`promoted: Option<Promoted>` field:

```rust
pub struct UnevaluatedConst<'tcx> {
    pub def: DefId,
    pub args: GenericArgsRef<'tcx>,
    pub promoted: Option<Promoted>,
}
```

`Promoted` is rustc's body-local index for promoted (lifted-out)
constants; e.g., `&5u32` in a function body becomes a promoted
`Const::Unevaluated` referencing a synthetic body. The mono collector
evaluates these too (per the post-mono guarantee), so by the time
RegseqModel construction runs, they should be `Val(ConstValue, ty)`.
The bridge does not need to handle `promoted` explicitly.

---

## 8. Cross-build determinism: how rustc stabilises AllocIds

**The key finding for stage-4's relocation-encoding decision:** rustc's
`HashStable` for `AllocId` resolves the AllocId to its `GlobalAlloc`
*target* and hashes that, deliberately discarding the AllocId integer.

Evidence at `rustc_middle/src/ty/impls_ty.rs:67-76`:

```rust
// AllocIds get resolved to whatever they point to (to be stable)
impl<'a> HashStable<StableHashingContext<'a>> for mir::interpret::AllocId {
    fn hash_stable(&self, hcx: &mut StableHashingContext<'a>, hasher: &mut StableHasher) {
        ty::tls::with_opt(|tcx| {
            trace!("hashing {:?}", *self);
            let tcx = tcx.expect("can't hash AllocIds during hir lowering");
            tcx.try_get_global_alloc(*self).hash_stable(hcx, hasher);
        });
    }
}
```

The comment "AllocIds get resolved to whatever they point to (to be
stable)" is the **operational rule**. The cross-build identity carrier
is the `GlobalAlloc` target:

- For `GlobalAlloc::Function { instance }`, the hashed identity
  reduces to `instance.def_id` and `instance.args` (both have stable
  hashing via `DefPathHash` and through the type-system stable
  hashing).
- For `GlobalAlloc::Static(def_id)`, the identity is the `DefId`
  (which has `DefPathHash`).
- For `GlobalAlloc::VTable(ty, dyn_preds)`, the identity is the
  trait-object type (and the predicates list).
- For `GlobalAlloc::Memory(const_alloc)`, the identity is the
  byte-content + provenance + init_mask of the allocation (recursive --
  if the allocation contains AllocId-bearing pointers, those resolve
  too).
- For `GlobalAlloc::TypeId { ty }`, the identity is the type.

**Stage-4 implication:** spec.md §9.7's stable-encoding tuple
`(regseq_instance_kind_rank, item_def_path_hash,
canonical_generic_args, fn_shim_disambig_or_None)` is doing
exactly what rustc does, just made explicit and serialisable. The
4-tuple is a closed-form expression of "what would rustc's
`HashStable<for AllocId>` reduce to for this AllocId?"

`HashStable for CtfeProvenance` at line 78-82:

```rust
impl<'a> HashStable<StableHashingContext<'a>> for mir::interpret::CtfeProvenance {
    fn hash_stable(&self, hcx: &mut StableHashingContext<'a>, hasher: &mut StableHasher) {
        self.into_parts().hash_stable(hcx, hasher);
    }
}
```

`into_parts()` returns `(AllocId, flags)`. The AllocId is then resolved
per the impl above. So a provenance entry inside a `ProvenanceMap`
ultimately stable-hashes to the target's identity (DefPathHash + args
+ flags), not the local handle.

### 8.1 Recursion: nested constants

If a `Memory` allocation contains relocations to other allocations,
hashing the parent recursively hashes the children. The provenance map
encodes the AllocIds; each AllocId resolves via the stable-hash impl;
each resolved target may itself contain provenance-bearing memory.

**Stage 4 must mirror this recursion:** an aggregate const whose bytes
contain pointer relocations will canonicalise to a key that depends
recursively on the relocated targets' stable identities. Naive byte-
hashing of the bytes plus literal-AllocId encoding would NOT be cross-
build stable.

### 8.2 Two-build invariant

For two builds satisfying the build-equivalence quad (spec.md §12.4):

- **DefPathHash-based identity** stays equal: stable from rustc-source
  + crate metadata.
- **Generic-args canonicalisation** stays equal: spec §12.3's
  `ArgsKey` deep canonicaliser handles this.
- **Resolved const values** stay equal: the valtree representation is
  designed exactly so that equal values have equal valtrees (per the
  doc-comment at `const_kind.rs:131-142`).

So: stage-4-canonical `ConstId` values can be cross-build stable
*provided* (a) we hash through to the target identity for
relocations, (b) we use valtree-equivalent representations for
aggregates (i.e., padding-stripped), and (c) we don't expose
local-AllocId-equivalent integers in the canonical key.

---

## 9. Stage-4 applicability

Mapping rustc's evidence to RegseqModel D-4-1:

### 9.1 Variant-set design

rustc has two surface enums to consider:

- **`ValTreeKind`** (2 variants: Leaf, Branch) -- minimal,
  canonicalisation-strong, *but* loses padding info, can't represent
  raw pointers / typed nulls / float bits, and is type-system-only.
- **`ConstValue`** (4 variants: Scalar, ZeroSized, Slice, Indirect) --
  catches all post-eval shapes, but `Indirect` is a "see allocation
  for details" variant that hides aggregate structure behind a byte
  buffer.

**The rich GlobalAlloc + Allocation + Provenance + InitMask combo
behind `ConstValue::Indirect` is where most of the real work lives.**
Stage 4 cannot pick "just one" of these without losing the bridge
contract for some reachable shape:

- ValTree-only: vtable instances cannot be expressed (need
  relocations).
- ConstValue-only: aggregates lose their structural identity (they
  become opaque byte buffers indexed by Ty-derived layout).

The Option A flat-enum sketch in the framing message
(D-4-1.1 through .7) is closer to a hybrid of the two:

- Scalar variants (per-type) serve the role of `Scalar(Int)` /
  `ScalarInt::Leaf`.
- ZST variant matches `ConstValue::ZeroSized` and `ValTree::zst`.
- Aggregate-with-children variant matches `ValTreeKind::Branch` -- the
  preferred shape for canonicalisation.
- Bytes + Reloc + NullPtr + Uninit variants together cover what
  `ConstValue::Indirect` and `Slice` express.

This hybrid avoids both the valtree-only loss and the bytes-only
loss.

### 9.2 Type-tag placement

Rustc's evidence: type travels *with* the value via the enclosing
pair. RegseqModel diverges (inline in scalar variants) -- divergence
is acceptable but should be deliberately spec'd, not casual. The
hypothesis (Section 2.3) is that rustc's choice is `'tcx`-arena-
constrained; we don't have that constraint.

### 9.3 Aggregate shape

Rustc's `ValTreeKind::Branch(Box<[Const]>)` shows the design that
maximises canonicalisation: nested IDs to share child structure, with
the outer container interning across builds. Stage 4's `Aggregate(Vec<
ConstId>)` proposed shape (D-4-1.3 in the framing) directly mirrors
this. Padding is *not* represented at the valtree layer; if stage 4
wants padding-bytes-distinguishable canonicalisation (per spec.md
§9.7's "padding/uninitialised bytes" requirement), it needs the bytes-
form too -- hybrid enum stays justified.

### 9.4 Relocation encoding

The 4-tuple encoding spec.md §9.7 already pins is **operationally
equivalent** to what rustc's stable-hash AllocId resolution computes,
just made explicit. Stage 4 should:

- Define a `RelocationTarget` enum mirroring `GlobalAlloc`'s 5 variants
  (or 4 -- TypeId is out-of-v1-scope).
- For each variant, the 4-tuple's `kind_rank` selects the variant; the
  remaining fields are variant-specific (DefPathHash for Static/Function,
  PrincipalTraitKey for Vtable, byte-content for Memory).

For `GlobalAlloc::Memory` (a const with no provenance, e.g. `&[u8]`
literal): the canonical key reduces to byte content + alignment +
mutability (no further relocation chain). Stage 4 likely doesn't need
a separate `RelocationTarget::Memory` because such anonymous-byte-
buffer constants can be *inlined* into the parent `Const` rather than
relocated (RegseqModel doesn't have separate `&str` static items, just
the `Const` with byte payload).

### 9.5 Null pointer typing

Rustc's representation is "scalar of pointer-width zero, no
provenance" -- the type tag is external. Stage 4's proposed
`NullPtr(TyId)` is divergent (inline type). The vacant-vtable-slot use
case from spec.md §13.2 strongly favours inline; it gives us per-slot
canonical identity-key locality and easier renderer support.

If stage 4 picks bytes-form (matching rustc), every typed-null in a
vtable instance becomes `(8 zero bytes, no provenance entry, type
inferred from the slot's FnPtr ty)`. This works but pushes the type
information onto the consumer (the code reading the bytes).

### 9.6 Uninit / padding

Stage 4 needs the distinction (per spec.md §9.7). Two implementation
choices:

1. Mirror rustc's `InitMask` (lazy bit-mask) -- compact, supports
   "all init / all uninit" common cases without bit storage.
2. Per-byte option `Vec<Option<u8>>` (rustc_public's choice) -- simple
   but linear-size per allocation.

Either works for canonicalisation; stage 4's choice depends on
expected aggregate sizes in driver fixtures (likely small enough for
option-form to be acceptable).

### 9.7 Validation phase

Stage 3's `ConstInterner` validates **before** `TyInterner` (spec.md
§3.3 Phase B step 4). The doc-comment in `const_kind.rs:131-142` and
the `mir::Const::is_deterministic` analysis (§7.3 above) both confirm
that **the type-system valtree subset (Leaf-only) is sound to validate
without TyId resolution** -- the canonical form depends only on
`(scalar_int_data, scalar_int_size)`. This matches the proposed
"scalar primitives only in v1" type-level subset.

The value-level subset validation needs `TyId` (typed null
canonicalisation, bytes-encoded enum-discriminant tag types) and
`InstanceId` (relocation encoding). Stage 4's two-phase discipline is
spec.md §9.7's already-pinned answer.

### 9.8 Post-mono guarantee

Rustc's `MirUsedCollector` runs before codegen; all `required_consts`
get evaluated. RegseqModel construction runs after `optimized_mir` is
computed (which is after the collector). So by the time the bridge
sees a `mir::Const`, the post-mono guarantee has held. The bridge can
call `.eval()` on every encountered const with confidence.

If a non-evaluated const (`mir::Const::Unevaluated`, or `mir::Const::
Ty` with non-Value kind) reaches the bridge, **it's a bridge bug** --
some IR construction path is reading from somewhere the collector
didn't visit. Per the no-defensive-fallbacks rule, the bridge raises
a diagnostic; it does not gracefully evaluate.

---

## 10. Open questions / hypotheses

These surfaced during the research and warrant decision:

### 10.1 Hypothesis: aggregate shape -- "ValTree shape only", not
"bytes shape"

For the **type-level subset** (scalar primitives reachable from
GenericArg), stage 4's variant set can stay valtree-shape (no byte
buffers, no provenance maps, no init masks). The type-level phase
validates these without TyId resolution.

For the **value-level subset** (statics, vtables, aggregates with
relocations), the question is whether to keep them in valtree shape
(`Aggregate(Vec<ConstId>)` with relocations as separate `Reloc`
children) or to fall back to bytes-form (`Bytes(Vec<u8> + ProvenanceMap
+ InitMask)`).

**Hypothesis:** valtree shape is sufficient even for value-level if
the variants are augmented:

- `Bool`, `Char`, `Int`, `Uint`, `Float`, `ZST`: trivial
- `Aggregate(Vec<ConstId>)`: structs, tuples, arrays
- `Enum { variant_idx: VariantIdx, payload: Vec<ConstId> }`: enums
- `Reloc(InstanceRef)`: any function/static/vtable pointer
- `NullPtr(TyId)`: any typed null
- `Uninit`: a padding marker
- `Bytes(Vec<u8>)`: only for raw-byte literals (`&str`, `&[u8]`),
  flat byte sequence, no relocations possible inside

The key question: are there any reachable value-level consts that
need `Bytes + ProvenanceMap + InitMask` together (i.e., "byte buffer
with relocations and uninit ranges")?

**Likely answer:** `core::any::TypeId` constants would (rustc's
`GlobalAlloc::TypeId` exists for them), but TypeId is out-of-v1-scope
for the driver DSL. Reading `optimized_mir` for a v1 driver fixture
will probably never yield such a construct.

**Action item for prior-art synthesis:** check charon and
regseq-model-old to see if either project has chosen the all-valtree
or hybrid shape, and whether they encountered the byte-buffer-with-
relocations case.

### 10.2 Hypothesis: padding bytes never reach stage-4 canonicalisation
in practice

Per the doc comment at `const_kind.rs:131-142`, valtrees deliberately
strip padding. Per `valtree_to_const_val`, the conversion to MIR
`ConstValue` may *introduce* padding (by laying the valtree out per
the type's layout), but the result is then zero-filled in padding
ranges (the InitMask marks them uninit, but the bytes are typically
zero per the layout's zero-fill default).

**Hypothesis:** for any const value reaching `optimized_mir` in a v1
driver fixture, the canonicalisation key never depends on padding
bytes -- we can canonicalise valtree-shape and ignore the padding
distinction even when a `ConstValue::Indirect` arrives.

This is **NOT** what spec.md §9.7 says (which explicitly requires
"padding/uninitialised bytes (canonicalisation must distinguish 'uninit'
from any specific byte pattern)"). The spec is more conservative.

**Action item:** stage 4 should investigate whether the conservative
position (track uninit) is justifiable against actual fixture
evidence, or whether the strict-canonical-via-valtree position is
sufficient. If sufficient, drop the `Uninit` variant from the proposed
Option A.

### 10.3 Question: how does stage 4 handle `mir::Const::Ty(_, ty::Const)`
where the inner valtree is a non-primitive aggregate?

Per `Const::is_deterministic` (§7.3 above), non-primitive valtrees
return `false` -- "valtrees purposefully generate new allocations" per
the comment at `mir/consts.rs:481-484`. So calling
`valtree_to_const_val` twice produces non-equal results (different
AllocIds).

**For stage-4 canonicalisation,** this means:

- Stage 4's bridge must NOT call `valtree_to_const_val` for stage-3
  generic-arg-position consts. It should walk the valtree directly and
  produce a `Const` from the valtree shape (Leaf -> scalar variant,
  Branch -> Aggregate(Vec<ConstId>)). The valtree IS the canonical
  representation; calling `valtree_to_const_val` is what introduces
  the AllocId churn.
- For value-level consts (`mir::Const::Val`), the `ConstValue` is
  already evaluated and shouldn't churn. But: if the bridge encounters
  a `Const::Ty(_, ty::Const::Value(cv))` in a value-level position
  (e.g., as a static initialiser), it has the choice to walk the
  valtree directly (avoiding AllocId churn) or to call
  `valtree_to_const_val` (re-introducing churn).

**Hypothesis:** the bridge always walks valtrees directly; never calls
`valtree_to_const_val`. This avoids the deterministic-mismatch
problem.

### 10.4 Question: do `GlobalAlloc::TypeId` allocations appear in v1
driver MIR?

`TypeId` appears in MIR only when `core::any::TypeId::of::<T>()` is
called. Driver DSL doesn't permit reflection (no v1 motivation), so
the answer is likely "no" -- but this should be verified against the
pl011 fixture once stage 11/12 land.

If the answer is no, stage 4 can omit the `TypeId` reloc target from
its `RelocationTarget` enum entirely (closed vocabulary, rejection at
the bridge).

### 10.5 Question: how does stage 4 handle `&str` and `&[u8]`?

Rustc has a dedicated optimisation: `ConstValue::Slice { alloc_id, meta
}` (a fat-pointer-shaped pair). The pointee is byte-array-typed; meta
is the slice length.

Stage 4 has options:

1. **Mirror the optimisation:** add a `Const::SliceRef { bytes_const:
   ConstId, len: u64 }` variant.
2. **Subsume into Aggregate + Reloc:** the slice is a 2-field tuple of
   `(*const u8, usize)`; the first field is `Reloc(<bytes-const>)`,
   second is `Uint(Usize, len)`.

Option 2 is simpler (fewer variants) but requires the consumer to
recognise the pattern. Option 1 makes the codegen-side rendering of
`&"foo"` trivial.

The pl011 fixture is unlikely to use string literals (no_std driver),
so the practical pressure is low. Stage 4 can defer this to a sub-
decision.

### 10.6 Hypothesis: vtable instance values are recursive aggregates
of relocations + scalars + (maybe) typed-nulls for vacant slots

Per spec.md §13.2 and the algorithm at §13.2.1, a vtable instance is
a `repr(C)`-like aggregate with fields:

- drop-glue ptr: `Reloc(VTableDropShim)` or `NullPtr(canonical_vacant_fnptr_ty)`
- size: `Uint(Usize, size_value)`
- align: `Uint(Usize, align_value)`
- per-method fn-ptrs: `Reloc(VTableMethodShim)` or `NullPtr(...)` for vacant
- supertrait vtable ptrs: `Reloc(VTableInstance)`

So a vtable instance value's Const shape is exactly:

```
Aggregate(Vec<ConstId>) where each ConstId resolves to one of:
  - Reloc(InstanceRef)  -- function or vtable pointer
  - Uint(Usize, value)  -- size or align
  - NullPtr(canonical_vacant_fnptr_ty)  -- vacant slot
```

This is a **load-bearing test case** for the variant set. If this
shape is expressible in the proposed variant list, we have working
v1 vtable lowering. If something is missing (e.g., padding between
fields breaks the pure-aggregate shape), the variant set is
incomplete.

**Action item:** when D-4-1 is being framed, walk through the
vtable-instance shape end-to-end and confirm each field expressible
in the chosen variant set without falling back to bytes-form.

---

## 11. Summary table: rustc evidence -> stage-4 design pressure

| Stage-4 question                  | Rustc evidence                                              | Direction                                                                    |
| --------------------------------- | ----------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Variant count                     | 8 ConstKind / 2 ValTreeKind / 3 mir::Const / 4 ConstValue   | Hybrid: valtree-shape for type-level, augmented for value-level (~10 variants) |
| Type-tag placement (scalars)      | External (in enclosing Ty pair)                             | Stage 4 diverges to inline; reasonable but flag the divergence               |
| Aggregate shape                   | Branch-of-children at ty layer; bytes+provenance at MIR     | Use Branch-of-children (`Vec<ConstId>`); avoid byte-buffer for v1 reachable  |
| Enum encoding                     | First field = variant_idx, then payload                     | Store `(variant_idx, Vec<ConstId>)` directly; don't re-encode bytes          |
| Relocation encoding               | AllocId resolves to GlobalAlloc target; stable hash via target | 4-tuple stable encoding (already in spec.md) is operationally equivalent     |
| Null pointer typing               | Inline ScalarInt zero of pointer-width, no provenance       | Stage-4 NullPtr(TyId) inline form is divergent but better for vtable shims   |
| Uninit / padding                  | InitMask (lazy bit-mask); rustc_public uses Vec<Option<u8>> | Per-byte option in Bytes variant; or skip Uninit variant if not reached     |
| Type-level vs value-level         | Type-level only `Value` survives; valtree primitive-only deterministic | Phase split is variant-driven (scalar primitives = type-level)              |
| Cross-build determinism mechanism | HashStable for AllocId resolves to target identity recursively | Stable encoding tuple in spec.md does this explicitly                       |
| Post-mono guarantee               | MirUsedCollector evaluates required_consts; codegen panics   | Bridge inherits guarantee; non-Value/Val survivors are bridge bugs          |

The strongest single piece of evidence is the
`HashStable for AllocId` impl at `rustc_middle/src/ty/impls_ty.rs:67-76`
plus its comment "AllocIds get resolved to whatever they point to (to
be stable)". This *is* the cross-build relocation discipline RegseqModel
needs, made explicit.

---

## 12. References (file:line)

Primary citations:

- `rustc_type_ir/src/const_kind.rs:21-49` -- `ConstKind` 8-variant enum
- `rustc_type_ir/src/const_kind.rs:131-164` -- `ValTreeKind` doc
  comment + 2-variant enum
- `rustc_middle/src/ty/consts.rs:24-29` -- `ty::Const<'tcx>` interned
  wrapper, 24-byte size assert
- `rustc_middle/src/ty/consts.rs:124-127` -- `ty::Const::new_value`
- `rustc_middle/src/ty/consts.rs:160-171` -- `is_trivially_wf`
- `rustc_middle/src/ty/consts/valtree.rs:32` -- `ValTree<'tcx>`
  interned
- `rustc_middle/src/ty/consts/valtree.rs:50-55` -- `from_raw_bytes`
- `rustc_middle/src/ty/consts/valtree.rs:95-100` -- `ty::Value`
  (ty + valtree pair)
- `rustc_middle/src/ty/consts/valtree.rs:200-217` --
  `destructure_adt_const`
- `rustc_middle/src/ty/consts/int.rs:152-159` -- `ScalarInt` packed
  struct
- `rustc_middle/src/mir/consts.rs:21-27` -- `ConstAlloc<'tcx>`
- `rustc_middle/src/mir/consts.rs:33-71` -- `ConstValue` 4-variant
  enum
- `rustc_middle/src/mir/consts.rs:73-74` -- 24-byte size assert
- `rustc_middle/src/mir/consts.rs:205-224` -- `all_bytes_uninit`
- `rustc_middle/src/mir/consts.rs:232-253` -- `mir::Const<'tcx>`
  3-variant
- `rustc_middle/src/mir/consts.rs:288-300` -- `is_required_const`
- `rustc_middle/src/mir/consts.rs:332-361` -- `mir::Const::eval`
- `rustc_middle/src/mir/consts.rs:478-509` -- `is_deterministic`
- `rustc_middle/src/mir/consts.rs:512-527` -- `UnevaluatedConst`
  (with `promoted` field)
- `rustc_middle/src/mir/interpret/value.rs:22-34` -- `Scalar`
  enum
- `rustc_middle/src/mir/interpret/value.rs:104-118` --
  `from_pointer` / `from_maybe_pointer`
- `rustc_middle/src/mir/interpret/value.rs:121-123` -- `null_ptr`
- `rustc_middle/src/mir/interpret/mod.rs:87-88` -- `AllocId`
- `rustc_middle/src/mir/interpret/mod.rs:259-276` -- `GlobalAlloc<'tcx>`
- `rustc_middle/src/mir/interpret/mod.rs:489-520` --
  `reserve_and_set_*` API per kind
- `rustc_middle/src/mir/interpret/allocation.rs:92-116` -- `Allocation`
- `rustc_middle/src/mir/interpret/allocation.rs:285-302` --
  `ConstAllocation`
- `rustc_middle/src/mir/interpret/allocation/init_mask.rs:14-34` --
  `InitMask` doc + struct
- `rustc_middle/src/mir/interpret/allocation/init_mask.rs:39-64` --
  `is_range_initialized` / `set_range`
- `rustc_middle/src/mir/interpret/allocation/provenance_map.rs:35-43` --
  `ProvenanceMap`
- `rustc_middle/src/mir/interpret/allocation/provenance_map.rs:70-77` --
  `ptrs()` accessor (interned-only)
- `rustc_middle/src/mir/interpret/pointer.rs:91-167` --
  `CtfeProvenance` + flag bits
- `rustc_middle/src/ty/generic_args.rs:30-39` -- `GenericArg<'tcx>`
  (tagged-pointer to Ty/Region/Const)
- `rustc_middle/src/ty/generic_args.rs:178-180+` -- `GenericArgKind::Const(Const)`
- `rustc_middle/src/ty/impls_ty.rs:67-76` -- `HashStable for AllocId`
  (the **load-bearing** stable-resolution rule)
- `rustc_middle/src/ty/impls_ty.rs:78-82` -- `HashStable for
  CtfeProvenance`
- `rustc_codegen_ssa/src/mir/constant.rs:23-29` -- post-mono panic
  message ("erroneous constant missed by mono item collection")

Comparative (rustc_public):

- `rustc_public/src/ty.rs:117-152` -- `TyConst` / `TyConstKind`
  (5-variant)
- `rustc_public/src/ty.rs:159-216` -- `MirConst` / `MirConstId`
- `rustc_public/src/ty.rs:1280-1354` -- `Allocation` (flat, owned)
- `rustc_public/src/ty.rs:1346-1353` -- `is_null` (canonical
  null-test)
- `rustc_public/src/ty.rs:1357-1365` -- `ConstantKind` (5-variant)
- `rustc_public/src/mir/body.rs:568` -- `Repeat(Operand, TyConst)`
  (one of the consumer sites)
