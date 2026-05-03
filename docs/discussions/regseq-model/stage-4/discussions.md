# Stage 4 -- Discussion Record

Distilled record of the design dialogue producing the stage-4
const-vocabulary spec. Each entry captures the question raised,
key options considered, the decision (or current lean for open
decisions), the reasoning chain, and cross-references to
`spec-snapshot.md` (the spec) and `review-log.md` (the audit
trail).

Status of decisions in this file mirrors `spec-snapshot.md` §9.7
and adjacent sections, plus `status.md` "Open decisions" /
"Locked decisions" tables.

D-tag convention: prefixed `D-4-K` form per `../README.md` §5.2
(stage 4 starts day-one with the new convention; stages 1 and 3
use legacy un-prefixed tags).

Status tags:
- `[DRAFT]` -- audit-shape recorded; pending Codex review.
- `[LOCKED]` -- closed at APPROVE on a Codex review round.
- `[REOPENED]` -- a later sub-decision or sibling D-tag forced a
  revision; superseded entry kept for history.
- `[DEFERRED to stage N]` -- moved to a later stage's scope.

---

## 1. Decision discussions

### D-4-1 -- `Const` enum variant set [LOCKED]

**Question.** What is the closed list of `Const` variants that
the bridge fills from rustc-evaluated const values, given the
post-monomorphisation guarantee and the F4-2 cross-build
fingerprint requirement?

**Key options.**

1. **Option A -- flat enum, per-flavour variants.** One variant
   per value flavour (scalars per type, ZST, aggregate, enum-value,
   relocation, null, uninit-bearing bytes). Variant tag IS the
   equality class. Each variant carries the type info it needs
   internally (no shared `Const { ty, kind }` wrapper).
2. **Option B -- rustc-mirror.** Mirror `mir::ConstValue`'s
   `Scalar / ZeroSized / Slice / Indirect` shape. Minimum bridge
   translation effort.
3. **Option C -- two-tier physical split.** Separate
   `TypeLevelConst` and `ValueLevelConst` types interned in two
   tables, structurally enforcing the type-level vs value-level
   phase split.

**Decision.** Option A, refined to **11 variants** (the parent
shape; per-variant payload pinned in sub-decisions D-4-1.1
through D-4-1.9):

```rust
pub enum Const {
    Bool(bool),
    Char(u32),
    Int(IntKind, u128),
    Uint(UintKind, u128),
    Float(FloatKind, u128),
    ZST,
    Bytes(Vec<Byte>),
    Aggregate(Vec<ConstId>),
    EnumValue { variant_idx: VariantIdx, payload: Vec<ConstId> },
    NullPtr(TyId),
    Reloc(InstanceRef),
}
```

**Reasoning.**

*Why not Option B (rustc-mirror).* rustc's stratification --
`ty::Const` / `mir::Const` / `mir::ConstValue` plus the
`Indirect`-with-`Allocation` lump -- exists because rustc has both
pre-eval and post-eval surfaces and layered IRs. The post-mono
guarantee that motivated Q3/Q5/Q6 of `architecture.md` lets us
skip pre-eval shapes entirely (rustc has already evaluated
everything that reaches RegseqModel). The `Indirect` lump would
defeat per-variant local identity-key derivation: every kind of
value-level const (statics, vtable instances, aggregates, fn-
pointer relocations) would funnel through one variant whose
payload is opaque bytes-with-provenance, and the bridge would
have to re-introspect the same way the codegen does. See
`prior-art/rustc.md` Findings 1 and 7 for the layering, and
`prior-art/README.md` §4.3 for the cross-cluster confirmation
that stratified universes exist for pre-eval / post-eval reasons
that don't apply here.

*Why not Option C (two-tier physical split).* 0-of-5 non-rustc
projects have a phase split (`prior-art/README.md` §2.4). In
Option A the type-level subset is identifiable by variant
discriminant -- `Bool`, `Char`, `Int`, `Uint`, `Float` are type-
level; the rest are value-level. That's a `match` arm, not a
separate type. Option C buys structural enforcement at the cost
of two interner tables, double the public API, and divergence
from spec.md §9.7's "one `ConstInterner`" language. The
validation phase ordering already enforces the dependency rule
mechanically; physical separation is cosmetic.

*Why per-flavour variants over a tagged scalar `Scalar { ty,
value }`.* Three considerations. (a) Spec.md §9.7 line 1560-1561
already pins inline scalar tagging. (b) Per-variant identity-key
derivation is local: each variant has a clear canonical
representation rule (`Int(I32, 1)` ≠ `Int(I64, 1)`; the variant
tag itself is part of the identity). (c) Three voices in the
prior-art evidence agree on inline scalar tags: spec.md §9.7
already, `prior-art/mlir.md` (`TypedAttrInterface`),
`prior-art/mrustc.md` (`MIR::Constant` scalar variants). The
externalised dissenters (`prior-art/llvm.md`, `prior-art/rustc.md`)
diverge for class-system reasons (LLVM's `Constant` is a
`Value` subclass with type already on the parent;
rustc's `mir::Const` carries the type for two-layer reasons we
don't replicate).

**Methodology rules invoked.**

- *Store the const-eval result, not the const expression.* The
  variant set carries fully-evaluated bytes / scalar values; no
  variant represents `ConstKind::Unevaluated` or expression-
  level shapes.
- *Cross-build canonicalisation uses stable identity.* `Reloc`
  and `Bytes::Reloc` carry `InstanceRef` (the 4-tuple), not
  `InstanceId`.
- *No defensive fallbacks.* No "Other" / "Unknown" / "Opaque"
  variant. If rustc gives the bridge something that does not
  fit a listed variant, the bridge raises a diagnostic; it does
  not silently widen.
- *Compositional behavior beats special-case flags.* No
  flag-style "is_string_literal" / "is_vtable_instance" markers
  on `Const` — distinctions ride on variant tag and
  consumer-context type. (Once the deferred
  `SyntheticItemKind::ConstMemoryStatic` family lands per
  D-4-1.9, `&str` backing also falls out compositionally as
  `Aggregate([Reloc(memory_static_inst_ref), Uint(Usize, len)])`
  — no new variant; the same compositional shape that vtable
  instance fat pointers will use. Until then, `&str` /
  `&[T]` reference-typed const handling is stage-4-deferred per
  the §9.7 "Stage-4 deferred: reference-typed const handling"
  subsection.) `Option<ConstId>` niche preservation (the
  4-byte invariant) imposes `ConstId = u32` (`NonMaxU32`)
  which the variant choices honour.

**Prior-art evidence.**

- `prior-art/rustc.md` Findings 1, 7 -- the two-layer
  stratification and post-mono guarantee at codegen. Confirms
  Option B's stratification is unnecessary for us.
- `prior-art/charon.md` Findings 1, 6 -- 13-variant
  `ConstantExprKind` pre-simplification; the `Byte` three-state
  for uninit; closest peer's design.
- `prior-art/llvm.md` Findings 1, 5 -- `Constant` hierarchy
  including per-pointer-type `ConstantPointerNull`; gold-
  standard for typed null.
- `prior-art/mlir.md` Findings 1, 2 -- attribute-not-value model
  with inline `TypedAttrInterface` tagging.
- `prior-art/mrustc.md` Finding 1 -- 10-variant pre-eval
  `MIR::Constant`; cross-check on what's tractable without the
  post-mono guarantee (and Finding 7 -- the cost of doing
  without it).
- `prior-art/compcert.md` Finding 1 -- 4-universe stratification;
  reference for the formal-verification pressure.
- `prior-art/regseq-model-old.md` Findings 1-9 -- 7-of-8 prior
  decisions don't survive re-derivation under stage-4 framing;
  the design choice we deliberately do not lift wholesale.
- `prior-art/README.md` §1, §2.1, §3.1, §4.3, §6 -- cross-cluster
  synthesis grounding the choice.

**Worked examples.** Pinned in sub-decisions D-4-1.1 through
D-4-1.9; assembled into `spec-snapshot.md` §9.7 worked-example
appendix.

**Cross-refs.** `spec-snapshot.md` §9.7 (variant definitions);
`status.md` "Open decisions" (sub-decision catalogue);
`review-log.md` rounds (none yet).

---

### D-4-1.1 -- Scalar grouping: per-type variants [LOCKED]

**Question.** How are scalar const values shaped at the variant
level: per-type variants with inline tag, or a single
`Scalar { ty, bits }` shape?

**Key options.**

1. Per-type variants: `Bool(bool)`, `Char(u32)`, `Int(IntKind,
   u128)`, `Uint(UintKind, u128)`, `Float(FloatKind, u128)`.
2. Single tagged scalar: `Scalar { ty: TyId, bits: u128 }`.

**Decision.** Option 1 (per-type variants). All five scalar
variants store their value in a `u128` (signed/unsigned variants
use widening/zero-extension; smaller floats zero-pad into the
high bits of the bit-pattern field). `Bool` carries a bare `bool`;
`Char` carries the 32-bit Unicode scalar value as `u32`.

**Reasoning.** Per-type variants make the variant tag itself
part of the equality class: `Int(I32, 1)` is not equal to
`Int(I64, 1)` is not equal to `Uint(U32, 1)` is not equal to
`Float(F32, 0x3f800000)` (the bit-pattern of `1.0f32`). With
single tagged scalar the equality rule depends on the `ty`
field's interaction with the `bits` field, which complicates
canonicalisation rules and gives the canonicaliser more places
to be wrong.

The `u128` width is mandatory for `I128` / `U128` and sufficient
for `F128`; smaller types carry their canonical-zero-extended
form (e.g., `Int(I8, -1)` stores as `u128::MAX` -- the two's-
complement extension of `-1` to 128 bits). Canonicalisation rule
in `ConstIdentityKey`: scalar variants are keyed by
`(variant_tag, kind_discriminant, value_bits)` where smaller
types' extension bits are deterministic by bridge construction.

`Bool(bool)` and `Char(u32)` get bare payloads because their
domains are smaller and natural. `Char` storage as `u32` (rather
than `char`) keeps the canonical bytes deterministic without
relying on Rust's `char` validity invariant in serialisation /
hashing -- the bridge has already validated the value is a valid
Unicode scalar at construction.

**Methodology rules invoked.** *Store the result, not the input;*
*no defensive fallbacks;* *information preservation* (smaller
scalar widths preserve via deterministic extension rather than
narrowing).

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 2 -- rustc keeps the type tag on
  the enclosing pair (externalised), but `ScalarInt` carries
  `(data: u128, size: u8)` -- the same widening pattern.
  Stage-4 inlines the type, which is the divergence we
  deliberately make.
- `prior-art/charon.md` Finding 2 -- `ScalarValue::{Signed(IntTy,
  _), Unsigned(UIntTy, _)}` confirms inline tag.
- `prior-art/mrustc.md` Finding 2 -- `MIR::Constant` scalar
  variants confirm inline tag; flagged warning on lossy `double`
  storage for `f16/f128`. Stage-4 avoids this with per-`FloatKind`
  bit-pattern storage in `u128`.
- `prior-art/regseq-model-old.md` Finding 1 -- 11 scalar-only
  variants confirm the inline-tag pattern but lack `Char`,
  `Float`, and `U128/I128` coverage; stage-4 corrects all three.

**Cross-refs.** `spec-snapshot.md` §9.7 (scalar variants);
`spec-snapshot.md` §10.0 (`Ty` enum scalar primitives -- the
type-side pairing).

---

### D-4-1.2 -- ZST canonicalisation: single representative [LOCKED]

**Question.** How are zero-sized const values represented? One
canonical `ZST` variant with no payload, or per-zst-type
representatives?

**Key options.**

1. Single `ZST` variant, no payload. All Rust ZSTs (`()`,
   fieldless structs, `PhantomData<T>`, etc.) collapse to one
   canonical representative.
2. `ZST(TyId)`, one per zst-type.

**Decision.** Option 1 (single `ZST` variant, no payload). The
type-of-this-ZST is supplied by the consumer's context (the
`InstanceKind::Const { value }`'s associated Item carries `ty`;
`GenericArg::Const(_)` does not admit ZSTs in v1).

**Reasoning.** ZSTs have no value bits -- they contribute zero
bytes to a byte-level encoding. Carrying type identity inside
the variant would force `ZST(unit_ty_id)` and
`ZST(phantom_data_ty_id)` to intern as distinct `ConstId`s
despite being byte-level identical, which blocks sharing across
contexts that re-use the canonical ZST. The Item-level type
already disambiguates: codegen knows from the Item or
GenericArg position which type the ZST inhabits.

The type-level subset (`GenericArg::Const(_)`) is restricted to
scalar primitives in v1 (per `spec.md` §9.7 line 1639), so a ZST
appearing as a generic-const argument is bridge-rejected per the
no-defensive-fallbacks methodology rule -- if `feature(adt_const_params)`
or unit-typed const generics ever appear in the driver DSL,
they're out-of-scope.

**Methodology rules invoked.** *No defensive fallbacks;*
*compositional behavior beats special-case flags* (single ZST
representative; type comes from context).

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 1 -- `ConstValue::ZeroSized` is a
  payload-free variant.
- `prior-art/llvm.md` Finding 1 -- `ConstantAggregateZero` is the
  ZST analogue (slightly different scope; LLVM uses it for
  zero-initialised aggregates, not just ZSTs, but the
  no-payload pattern matches).
- `prior-art/charon.md` Finding 1 -- `Adt(VariantId, [])` for
  empty-aggregate ZSTs; less canonical than stage-4's choice
  because it ties to a specific empty-variant context.

**Cross-refs.** `spec-snapshot.md` §9.7 (ZST canonical
representative); `discussions.md` D-4-1.7 (type-tag placement
implication).

---

### D-4-1.3 -- Aggregate field elements: `Vec<ConstId>` (interned children) [LOCKED]

**Question.** How are the field children of an aggregate const
represented: as interned `ConstId`s for sharing, or as inline
`Const` values?

**Key options.**

1. `Aggregate(Vec<ConstId>)` -- interned children share across
   structurally-equal aggregates.
2. `Aggregate(Vec<Const>)` -- inline children, no sharing.

**Decision.** Option 1 (`Vec<ConstId>`).

**Reasoning.** Two arguments. (a) **Sharing matters at our
scale.** Vtable instances reuse the same `NullPtr(canonical_vacant_sig_ty_id)`
and the same `Reloc(drop_shim_inst_ref)` across many vacant
slots and across many vtable types. Generic struct const literals
(`const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 }`) reuse
their scalar children. Inline children would force duplicate
materialisation; interned children share for free with the
existing `ConstInterner` content-uniqueness rule. (b) **The
identity-key shape stays simple.** `Vec<ConstId>` is the
*storage shape* of the variant; the *cross-build canonical
identity key* for an `Aggregate` recursively uses each child's
canonical const representation (per §12.3's deep-canonicaliser
pattern: a child `ConstId` resolves via `canonical_const_repr`
to the same canonical content that interned the child's entry,
which is build-stable per the type-level / value-level
invariants). With inline children, the variant carries each
child's full content materialised; interner-mediated sharing
is lost. The two are operationally equivalent for canonical
equality but storage-shape-divergent — `Vec<ConstId>` is
chosen for the sharing benefit.

`prior-art/mlir.md` Finding 3 confirms: `ArrayAttr<Vec<Attribute>>`
and `DenseElementsAttr` both store children as
storage-pointer references (the MLIR attribute uniquer's
`Attribute` is a pointer-into-`AttributeStorage`). The MLIR
splat optimisation (single-element storage when all children
equal) is a candidate compression noted in
`prior-art/README.md` §5.1 -- defer to implementation-time.

The choice diverges from `prior-art/charon.md` (inline `Vec<ConstantExpr>`)
and `prior-art/mrustc.md` (flat byte buffer or eager-promotion
to fresh synthetic statics) and `prior-art/compcert.md`
(aggregates only as `init_data` lists for globals) -- the
divergences trade more interner activity for downstream code
clarity, which is the right trade-off for a verification-target
IR.

**Methodology rules invoked.** *Information preservation*
(structural shape preserved, not flattened to bytes);
*compositional behavior* (sharing falls out of the existing
interner mechanism; no new sharing primitive needed).

**Prior-art evidence.**

- `prior-art/mlir.md` Finding 3 -- `ArrayAttr<Vec<storage-ptr>>`
  confirms the indirected-children pattern.
- `prior-art/rustc.md` Finding 3 -- `ValTreeKind::Branch(Box<[Const]>)`
  uses inline children at `ty::Const` layer, but rustc has its
  own arena interning that gives functionally equivalent
  sharing; we'd need explicit IDs without the arena.
- `prior-art/charon.md` Finding 3 -- inline `Vec<ConstantExpr>`,
  no sharing; deliberately chosen divergence.
- `prior-art/mrustc.md` Finding 3 -- demote-to-static (flat byte
  buffers); deliberately chosen divergence.

**Cross-refs.** `spec-snapshot.md` §9.7 (`Aggregate` variant);
`discussions.md` D-4-1.9 (first-class aggregate vs demote-to-
static); `discussions.md` D-4-1.8 (the byte-level alternative
shape `Bytes` for unstructured aggregates).

---

### D-4-1.4 -- Enum payload: `EnumValue { variant_idx, payload }` [LOCKED]

**Question.** How are enum-typed const values represented? With
explicit variant index plus payload children, or via the byte
encoding alone?

**Key options.**

1. `EnumValue { variant_idx: VariantIdx, payload: Vec<ConstId> }`.
   Discriminant value is implicit via the consumer's Ty's
   `VariantLayout.discriminant` (per stage-3 D13).
2. `Bytes(Vec<Byte>)` containing the materialised tag bytes plus
   payload bytes.
3. `EnumValue { discriminant: u128, tag_value: Option<u128>,
   payload: Vec<ConstId> }` -- carry the layout-resolved
   discriminant inline.

**Decision.** Option 1 (`EnumValue { variant_idx, payload }`).
Discriminant value is **not** stored in `Const`; the consumer's
Ty (an `Ty::Enum` with a populated `Layout` per stage-3 D2 + D13)
carries it on `VariantLayout.discriminant`.

**Reasoning.** *Don't duplicate stage-3's work.* Stage-3 D13
explicitly added `VariantLayout.discriminant: u128` and
`tag_value: Option<u128>` so consumers can resolve discriminant
values from the type's layout. Carrying them on the const value
would force the bridge to compute (and store) the same
information twice, with the const-side and type-side values
required to match by an additional invariant. By keeping
`EnumValue` to `(variant_idx, payload)`, the const value is
keyed by which variant it inhabits and what data the payload
carries, and the discriminant byte representation is read from
the type at codegen / pretty-printing time.

`Bytes(Vec<Byte>)` is the fallback when an enum value carries
provenance/relocations that can't decompose structurally (a niche-
encoded enum where the niche is a function-pointer relocation,
hypothetically). Stage-4 prefers `EnumValue` when rustc's
const-eval gives a structured `ValTree::Branch` shape; the bridge
falls back to `Bytes` otherwise per the D-4-1.9 dispatch rule.

**Methodology rules invoked.** *Don't redo work the rustc
compiler has already done* (or, here, work stage-3 has already
done -- discriminant resolution lives on `VariantLayout`).
*Compositional behavior;* *information preservation* (the
variant identity is preserved structurally; the byte
representation is computable).

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 3 -- `ValTreeKind::Branch` for
  enum values is `(variant_idx, fields)` with discriminant
  resolved separately; structurally identical to stage-4's
  choice.
- `prior-art/charon.md` Finding 1 -- `Adt(VariantId, fields)`;
  same shape.
- `prior-art/llvm.md` Finding 3 -- LLVM has no enum concept at
  the IR level (enums are lowered to integer-tagged unions
  before reaching IR); their `ConstantStruct` over the lowered
  representation is the analogue.

**Cross-refs.** `spec-snapshot.md` §9.7 (`EnumValue` variant);
`spec-snapshot.md` §11.1 (`VariantLayout` -- consumer of the
variant index); stage-3 D13.

---

### D-4-1.5 -- Relocation reference: `InstanceRef` 4-tuple [LOCKED]

**Question.** How is "this const points to that function /
static / vtable" represented in a way that supports cross-build
identity?

**Key options.**

1. **`InstanceRef` 4-tuple**: `(regseq_kind_rank: u8,
   def_path_hash: DefPathHash, args_key: ArgsKey,
   fn_shim_disambig: u8)`. Mirrors §9.3's `RegseqInstanceKey`
   shape with the local `ItemId` projected to
   `def_path_hash`.
2. **Local `InstanceId`**: an `IndexVec<InstanceId, _>` index.
3. **`(DefPathHash, ArgsKey)` only**: drop the kind rank and
   shim disambiguator.

**Decision.** Option 1 (the 4-tuple).

**Reasoning.** *Cross-build canonicalisation requires
build-stable identity.* Two builds satisfying the build-
equivalence quad (equal source, equal pinned toolchain, equal
`RegseqModel.target`, equal bridge options) must produce
identical canonical const keys for relocations to the same
Instance. Local `InstanceId` is a counter assigned in F4-2 sort
order during construction; relying on it would propagate the
counter into the const key, which then propagates into the F4-2
fingerprint via `GenericArg::Const(ConstId)` inside `ArgsKey`,
forming a circularity (the counter depends on the sort which
depends on the fingerprint which depends on the counter). The
4-tuple is build-stable because each component is build-stable:
`regseq_kind_rank` is the §9.3-fixed `regseq_instance_kind_rank`
(`Function`=0, `Static`=1, `Const`=2, `Struct`=3, `Enum`=4,
`Union`=5); `def_path_hash` is rustc's stable cross-build hash
(or the deterministic bridge-synthesised hash per spec.md §13.4
for synthetic Items); `args_key` is the §12.3 deep canonicaliser;
`fn_shim_disambig` is the §9.3-fixed `rustc_instance_kind_sort_rank`
(e.g., `Item`=0, `DropGlue`=10, `VTableShim`=2, ...). The
identical shape — minus the local `ItemId` — appears in the
in-build `RegseqInstanceKey` (spec.md §9.3); reusing it for
`InstanceRef` keeps cross-build identity operationally
equivalent to in-build identity.

*Why `regseq_kind_rank` (not a "kind discriminant").* Stage-3
§9.3 pins exactly six Regseq InstanceKind ranks; nothing labelled
`DropGlue`, `VTableShim`, or `VTableInstance` is its own
Regseq kind. `DropGlue` is a *rustc shim flavor* of the
`Function` Regseq kind, captured in `fn_shim_disambig`; vtable
instances are bridge-synthesised `Static` Items (rank 1) marked
in `item_synthetic_kinds` per §13.2. So `regseq_kind_rank`
disambiguates Regseq emission kinds, and `fn_shim_disambig`
disambiguates rustc shim flavors within the `Function` kind.
Two different rustc shim flavors sharing a `(DefId, args)` —
which §9.3 explicitly calls out as the case `Item(foo)` and
`ReifyShim(foo)` — would otherwise collapse.

*Why no separate `shim_disambig: Option<ShimDisambig>` enum.*
Vtable method shims and vtable drop shims are bridge-
synthesised Function Items per §13.2. Their canonical identity
is the synthesis context (`PrincipalTraitKey`, `ConcreteImpl`,
`vtable_entry_index` or `"drop"`); spec.md §13.4 encodes this
in the synthesised DefPath segments (`vtable_method_shim` /
`vtable_drop_shim` segment, per-shim `vtable_entry_index`
segment, etc.), so the resulting synthesised `DefPathHash`
already disambiguates method-vs-drop and per-method-index. A
separate `shim_disambig` enum field would duplicate information
already carried by `def_path_hash` — failing the "no defensive
fallbacks" methodology rule (no v2 hedge for cases that cannot
arise under the documented spec surface).

`prior-art/README.md` §3.1 documents that 5-of-5 non-rustc
projects fail cross-build determinism. Only adjacent precedent
is rustc's `HashStable for AllocId` at
`compiler/rustc_middle/src/ty/impls_ty.rs:67-76` which resolves
the local AllocId to its `GlobalAlloc` target before hashing,
deliberately discarding the local handle. Stage-4's 4-tuple is
operationally equivalent: it captures the resolved target in
build-stable form.

**Methodology rules invoked.** *Cross-build canonicalisation
uses stable identity.* *No defensive fallbacks* (no fallback
to local `InstanceId`).

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 8 -- `HashStable for AllocId`
  precedent; the only direct prior art for cross-build
  content-addressing of relocations.
- `prior-art/charon.md` Finding 4 -- local-ID relocations; cited
  as the negative case stage-4 deliberately avoids.
- `prior-art/llvm.md` Finding 4 -- `GlobalValue`-as-Constant
  relocations; in-memory pointer; bitcode is the stable form
  but allocation-order-dependent.
- `prior-art/mlir.md` Finding 4 -- `SymbolRefAttr` is string-path-
  based; structurally simpler than 4-tuple but loses
  disambiguation between `Function` / `DropGlue` /
  `VTableMethodShim` instances at the same DefPath.
- `prior-art/mrustc.md` Finding 4 -- `HIR::Path` verbatim with
  `PTR_BASE = 0x1000` magic byte; serialisation hack stage-4
  avoids via structured 4-tuple.
- `prior-art/compcert.md` Finding 4 -- `Init_addrof`; identifier
  is local `positive`.
- `prior-art/regseq-model-old.md` Finding 4 -- no relocation
  shape at all in the old `LiteralValue` variant.
- `prior-art/README.md` §3.1 -- cross-cluster "novel territory"
  finding.

**Cross-refs.** `spec-snapshot.md` §9.7 (`InstanceRef`,
`Reloc(InstanceRef)` variant; the in-bytes `Bytes::Reloc(InstanceRef)`
case); `spec-snapshot.md` §13.4 (synthesised `DefPath`
segments and `DefPathHash` construction); `spec-snapshot.md`
§16 (F4 / F4-2 determinism).

---

### D-4-1.6 -- Null pointer typing: `NullPtr(TyId)` [LOCKED]

**Question.** How are typed null pointers represented? With
pointee-type inline (per-type variant), or as a single null
representative with type contextually supplied?

**Key options.**

1. `NullPtr(TyId)` -- pointee type inline; per-pointee-type
   canonicalisation.
2. `NullPtr` -- single representative; type from context.
3. Subsume into `Bytes`: a null pointer is `Bytes(Vec<Byte::Value(0); ptr_size>)`
   plus the consumer's pointer type.

**Decision.** Option 1 (`NullPtr(TyId)` with pointee type
inline).

**Reasoning.** *Vacant vtable slots and source-level typed
nulls force per-pointee-type canonicalisation.* Per stage-3
§13.2, vacant vtable slots are explicit fn-pointer fields with
`Ty::FnPtr(canonical_vacant_sig_id)` and a null value. The
vtable type Item carries a `Layout` with multiple vacant slots;
the same model also admits source-level
`core::ptr::null::<T>()` calls producing typed nulls
`NullPtr(T)`. If a single canonical-null `NullPtr` applied to
all of these, `ConstId`-level identity could not distinguish
"vacant slot of canonical-vacant-sig" from "source-level
`core::ptr::null::<u8>()`" — they are functionally distinct
constants (different downstream codegen lowerings, different
F4-2 fingerprints reachable through `GenericArg::Const(_)` if
ever exposed). A single `NullPtr` representative with type-
from-context would shift the canonicalisation burden onto the
consumer's identity scheme, fragmenting the F4-2 fingerprint
story.

`prior-art/llvm.md` Finding 5 confirms the choice: LLVM's
`ConstantPointerNull::get(PointerType*)` interns per-pointer-
type; the polymorphic factory `Constant::getNullValue(Type*)`
takes a type and dispatches. LLVM is the gold standard for
this design; charon's hybrid (`PtrNoProvenance(0)` + outer
`ty: Ty` -- `prior-art/charon.md` Finding 5) achieves the same
disambiguation through different mechanism.

The `Bytes` subsumption (Option 3) loses information -- byte-
level encoding cannot recover whether the source was a typed
null or just zero bytes that happen to be pointer-sized. Stage-
4's information-preservation methodology rule rules it out.

**Methodology rules invoked.** *Information preservation*
(distinguish typed null from zero bytes); *compositional
behavior* (typed null is its own variant; doesn't fold into a
shared "null thing" representative).

**Prior-art evidence.**

- `prior-art/llvm.md` Finding 5 -- `ConstantPointerNull::get(PointerType*)`;
  per-pointer-type interning. Direct gold-standard precedent.
- `prior-art/charon.md` Finding 5 -- outer `ty: Ty` + inner
  `PtrNoProvenance(0)`; functionally equivalent.
- `prior-art/mrustc.md` Finding 5 -- implicit (zero bytes,
  no reloc, type from context); fails vacant-vtable-slot
  canonicalisation.
- `prior-art/compcert.md` Finding 5 -- `Vnullptr` is untyped;
  same failure mode.
- `prior-art/mlir.md` Finding 5 -- no `NullAttr`; per-type
  materialised via `llvm.mlir.zero` op.

**Cross-refs.** `spec-snapshot.md` §9.7 (`NullPtr(TyId)` variant);
`spec-snapshot.md` §13.2 (vacant vtable slots; canonical vacant
signature `Ty::FnPtr(canonical_vacant_sig_id)`).

---

### D-4-1.7 -- Type tag placement: inline-only, no outer wrapper [LOCKED]

**Question.** Does `Const` have an outer type tag (a wrapper
`Const { ty: TyId, kind: ConstKind }`) or do per-variant
contents carry whatever type info they need internally?

**Key options.**

1. **Inline-only.** No wrapper. Each variant carries the type
   info it needs (scalars via `IntKind` / `UintKind` /
   `FloatKind`; nulls via `NullPtr(TyId)`; aggregates via
   children's types).
2. **Hybrid.** `Const { ty: TyId, kind: ConstKind }` where `kind`
   is the per-variant payload AND `ty` is an outer wrapper.
3. **Wrapper-only.** Variants carry only structural shape; type
   comes from the outer `ty` field.

**Decision.** Option 1 (inline-only).

**Reasoning.** *Each variant carries exactly the type info it
needs.* Scalars carry `IntKind` / `UintKind` / `FloatKind` --
the type-equivalent for primitive scalars without forcing a
TyId resolution. Nulls carry `NullPtr(TyId)` because the pointee
type is load-bearing for canonicalisation (D-4-1.6). Aggregates
do not carry type at the `Const` layer because the consumer's
context (a struct-typed Item, an enum's `EnumValue`) already
determines structure; codegen pattern-matches on the consumer's
`Ty`.

The hybrid (Option 2) would carry a redundant outer `TyId` for
scalars (the `IntKind` already determines the type) and a
duplicate-information outer `TyId` for nulls and aggregates
(matching the consumer's context). The wrapper-only option
(Option 3) loses the per-variant discrimination that makes
identity-key derivation local.

3 prior-art voices favor inline-tag for scalars (spec.md §9.7
already pins, `prior-art/mlir.md`, `prior-art/mrustc.md`); 2
favor externalised (`prior-art/llvm.md` Value-class
inheritance, `prior-art/rustc.md` `mir::Const::Val(ConstValue,
Ty)` two-layer); 2 use hybrid (`prior-art/charon.md`,
`prior-art/regseq-model-old.md`). The decision picks the
inline-tag side and additionally drops the outer wrapper that
the hybrid voices keep -- justified by the per-variant payload
analysis above.

A consequence: code paths that need the type of an arbitrary
`ConstId` (e.g., a debug renderer) walk per-variant: scalars
synthesise from `IntKind` / `UintKind` / `FloatKind`; `NullPtr`
reads its `TyId`; `ZST` returns "ZST" symbolically (the
consumer's context disambiguates); `Aggregate` and `EnumValue`
have no inherent type without consumer context. Renderers
should be invoked with the consumer's `Ty` already in scope --
which they are at every call site stage-4 has surveyed
(InstanceKind dispatch, GenericArg resolution, &str backing).

**Methodology rules invoked.** *No defensive fallbacks*
(redundant outer wrappers are a defensive fallback against
"what if the consumer doesn't supply context"); *information
preservation* (each variant carries what it needs, no more);
*compositional behavior* (type info comes from the consumer's
context for variants that don't carry it intrinsically).

**Prior-art evidence.**

- spec.md §9.7 line 1560-1561 -- already pins inline scalar tags.
- `prior-art/mlir.md` Finding 2 -- inline via
  `TypedAttrInterface`.
- `prior-art/mrustc.md` Finding 2 -- inline scalar tags.
- `prior-art/charon.md` Finding 2 -- hybrid (inline + outer);
  stage-4 deliberately drops outer.
- `prior-art/regseq-model-old.md` Finding 2 -- hybrid; stage-4
  drops outer per the re-derivation analysis at
  `prior-art/regseq-model-old.md` Finding 9.
- `prior-art/llvm.md` Finding 2 -- externalised (type on
  `Value::Type *`); stage-4 inlines for the per-variant identity-
  key locality argument.
- `prior-art/rustc.md` Finding 2 -- externalised
  (`mir::Const::Val(ConstValue, Ty)`); rustc has a two-layer
  reason (Ty-level Const vs MIR-level Const); stage-4 has one
  layer and inlines.
- `prior-art/README.md` §1.1, §2.1 -- cross-cluster split (3
  inline / 2 externalised / 2 hybrid); the synthesis flagged
  the inline-vs-hybrid question for explicit decision.

**Cross-refs.** `spec-snapshot.md` §9.7 (variant definitions
with inline payloads); D-4-1.1 (scalar tags inline);
D-4-1.6 (`NullPtr` carries `TyId` -- not a wrapper-style outer
field but a variant-intrinsic type carrier).

---

### D-4-1.8 -- Uninit / byte-level handling: `Bytes(Vec<Byte>)` three-state [LOCKED]

**Question.** How are byte-level const values (with potential
uninit bytes and potential pointer relocations) represented?

**Key options.**

1. **`Bytes(Vec<Byte>)` with three-state Byte:**
   ```
   Byte = Uninit | Value(u8) | Reloc(InstanceRef)
   ```
2. **`Bytes(Vec<u8>)` only**: lossy, collapse uninit-as-zero
   (mrustc's path).
3. **`Bytes(Vec<u8>)` + parallel `InitMask`**: rustc-style lazy
   bit-mask alongside the byte buffer.
4. **No Bytes variant**: every byte-level value goes through
   structured `Aggregate` / `EnumValue` / `Reloc`.

**Decision.** Option 1 (three-state `Byte`).

**Reasoning.** *Stage-4 must distinguish uninit from any byte
pattern* (spec.md §9.7 line 1614-1616). Option 2 (lossy) is
ruled out by the methodology's information-preservation rule.
Option 3 (parallel InitMask) is the rustc design and is sound,
but introduces dual-storage with a synchronisation invariant
(the byte buffer length and the InitMask length must match;
relocations live in a third side table); the three-state `Byte`
unifies all three concerns in one container at modest size cost
(one tag byte + one payload byte vs raw `u8`).

*Why three states (not four).* Uninit, materialised value, and
relocation are the only states the bridge encounters in rustc's
const-eval output. Padding bytes inside an aggregate ARE uninit
per Rust's semantics (rustc's `InitMask` marks them so).
Pointer-bearing bytes are the relocations. There is no fourth
state.

*Why per-byte tagging (not per-range).* rustc's `InitMask` is
range-based for storage efficiency. Per-byte is simpler in code
and matches `prior-art/charon.md`'s `Byte::{Uninit, Value(u8),
Provenance(Provenance, u8)}` design (MiniRust-derived). At
RegseqModel scale, individual const values are small (vtable
instances are dozens of bytes; string literals are dozens to
hundreds; aggregate consts rarely exceed kilobytes); per-byte
overhead is negligible. The MLIR splat optimisation
(`prior-art/README.md` §5.1) is available as a downstream
compression if it ever matters.

*Why `Reloc(InstanceRef)` per byte rather than `Reloc(InstanceRef,
u8 offset_in_pointer)`.* Each byte of a pointer-bearing slot
carries the same `InstanceRef`; the byte position within the
pointer is recoverable from the slot offset. Validation
invariant (pinned at D-4-2 with the identity-key shape): if
byte at offset `o` is `Reloc(r)`, then bytes
`[o..o + ptr_size]` are all `Reloc(r)`, `o` is aligned to
`ptr_align`, and no `Reloc(_)` byte straddles the alignment
boundary. Carrying offset-within-pointer in the byte payload
would let the bridge encode partial-pointer overlays (which
rustc's const-eval does not produce post-mono); per the no-
defensive-fallbacks rule, we don't pre-allocate for it.

**Methodology rules invoked.** *Information preservation*
(uninit ≠ zero; pointer ≠ scalar bytes); *no defensive
fallbacks* (no offset-within-pointer field; no "Other" Byte
state); *compositional behavior* (the same `InstanceRef` shape
appears in `Bytes::Reloc` and the top-level `Reloc(InstanceRef)`
variant).

**Prior-art evidence.**

- `prior-art/charon.md` Finding 6 -- `Byte::{Uninit, Value(u8),
  Provenance(Provenance, u8)}`; direct precedent. Stage-4 drops
  the `u8` payload in `Provenance` because we don't carry
  offset-within-pointer (post-mono guarantee).
- `prior-art/rustc.md` Finding 6 -- `InitMask` lazy bitmask +
  `Allocation::bytes` + `ProvenanceMap`; equivalent semantics,
  different storage. `prior-art/rustc.md` cites the
  rustc-source location for the lazy bitmask state machine.
- `prior-art/mrustc.md` Finding 6 -- `allocation_to_encoded`
  collapses uninit-as-zero (line 3089 NOTE); explicit lossy
  path stage-4 deliberately avoids.
- `prior-art/compcert.md` Finding 6 -- `Init_space n` becomes
  zero bytes at `Globalenvs.v:910-924`; same lossy path.
- `prior-art/mlir.md` Finding 6 -- no byte-level uninit;
  attribute-only at value level.
- `prior-art/llvm.md` Finding 6 -- `UndefValue` / `PoisonValue`
  per-Type singletons; value-level not byte-level.

**Cross-refs.** `spec-snapshot.md` §9.7 (`Bytes` variant; `Byte`
enum); `spec-snapshot.md` §13.2 (vtable instance values --
primary consumer of `Bytes::Reloc`); D-4-1.5 (`InstanceRef`
shape).

---

### D-4-1.9 -- Aggregate first-class vs demote-to-static [LOCKED]

**Question.** Should aggregate const values (struct / array /
tuple literals) be a first-class `Const` variant, or demoted to
synthesised `InstanceKind::Static` Items with a `Reloc(InstanceRef)`
in the const?

**Key options.**

1. **First-class:** `Aggregate(Vec<ConstId>)` and
   `EnumValue { variant_idx, payload: Vec<ConstId> }` are full
   `Const` variants. `Bytes(Vec<Byte>)` is the byte-level
   alternative when the aggregate carries provenance / mixed
   relocations.
2. **Demote-to-static:** drop `Aggregate` and `EnumValue` from
   `Const`. The bridge synthesises an immutable Static Item per
   aggregate value; the const value reduces to
   `Reloc(synthesised_static_inst_ref)`.

**Decision.** Option 1 (first-class).

**Reasoning.** *Addressability is the discriminating concern.*

True statics (vtable instances, source `static FOO`,
synthesised statics for vtable instance values per stage-3
§13.2) need C-level addresses -- they take program-image space
at fixed locations and downstream code can take pointers to
them. They live in `InstanceKind::Static` with stable storage
semantics.

Aggregate const values used by-value (`const ARR: [u32; 3] =
[1,2,3]; let x = ARR;`) do not need C-level addresses; they
lower to compound-literal expressions inline at use sites:
`int x[3] = {1, 2, 3};` or `(struct Foo){.a=1, .b=2}`.
Forcing every aggregate const into a synthesised Static would
over-allocate program-image memory, conflate two C-level
storage classes, and create thousands of named statics for
trivial array literals.

The asymmetry between `Aggregate` (first-class const) and
vtable-instance-Static (synthesised Item) is genuine, not a
coincidence -- they have different lifetimes in the C output.

`prior-art/mrustc.md` Finding 3 (eager-promote-to-static) and
`prior-art/compcert.md` Finding 2 (no value-level aggregate;
only `init_data` for globals) and `prior-art/regseq-model-old.md`
Finding 3 (no aggregate variant at all; only runtime
`Rvalue::Aggregate`) all take the demote path. Their motivations
differ:
- mrustc has no const-eval result form to canonicalise; demote-
  to-static lets the C codegen emit each aggregate as a named
  initialiser.
- CompCert's `init_data` is the operational semantics of "a
  global variable with this initial value"; aggregate at value
  level would force a value semantics for compile-time-only
  aggregates, complicating the proof.
- regseq-model-old had no const-eval integration; aggregates
  appeared only as runtime constructions.

None of those motivations apply to stage-4. We have rustc's
post-mono const-eval result; we have a use-by-value pattern in
the driver DSL (`const`-bound array/struct literals); we want
codegen to emit them inline.

**Methodology rules invoked.** *Compositional behavior over
special-case flags* (the bridge picks `Aggregate` vs `Bytes` vs
`Reloc` based on what rustc gives it; no special-case demotion
rule); *information preservation* (structural aggregate shape
preserved at IR level; codegen chooses storage class).

*Bridge construction rule (forward to D-4-2 / spec.md §15;
authoritative listing in `spec-snapshot.md` §9.7's "Bridge
construction" tables).* Given a rustc `mir::ConstValue` post-
mono, the bridge dispatches:
- `ConstValue::Scalar(Scalar::Int(_))` → `Bool` / `Char` /
  `Int(...)` / `Uint(...)` / `Float(...)` / `NullPtr(_)` per
  the scalar's type.
- `ConstValue::Scalar(Scalar::Ptr(_))` → `Reloc(InstanceRef)`
  per the pointer's `AllocId` resolution to a `GlobalAlloc`.
- `ConstValue::ZeroSized` → `ZST`.
- `ConstValue::Indirect { alloc, offset }`:
  - if `alloc` has no provenance entries and the value-side `Ty`
    has structural shape → walk via valtree-to-aggregate; emit
    `Aggregate(Vec<ConstId>)` or `EnumValue { variant_idx,
    payload }`.
  - otherwise → emit `Bytes(Vec<Byte>)` from `alloc.bytes` and
    `alloc.provenance`, with `Byte::Uninit` for ranges absent
    from `alloc.init_mask`.

`ConstValue::Slice { data, meta }` and any
`ConstValue::Indirect` arm targeting `GlobalAlloc::Memory` are
**deferred** — they require a new `SyntheticItemKind::ConstMemoryStatic`
family in §13.3 / §13.4 to back the resulting
`Aggregate([Reloc(memory_static_inst_ref), Uint(Usize, meta)])`
shape. See `spec-snapshot.md` §9.7 "Stage-4 deferred:
reference-typed const handling."

The bridge does NOT walk through `valtree_to_const_val` (per
`prior-art/rustc.md` open question (c)) -- that path materialises
fresh `AllocId`s for non-primitive aggregates, breaking
determinism. Stage-4's bridge walks `ValTree::Branch` directly
to `Aggregate` / `EnumValue`, and walks `mir::ConstValue::Indirect`
for already-encoded byte-level values.

**Prior-art evidence.**

- `prior-art/rustc.md` Findings 1, 3 -- `ValTreeKind::Branch` is
  the structured-aggregate source; `ConstValue::Indirect` is
  the byte-level source. Both available post-mono.
- `prior-art/mlir.md` Finding 3 -- `ArrayAttr<Vec<Attribute>>`;
  first-class structured aggregate at attribute level.
- `prior-art/charon.md` Findings 1, 3 -- 13 `ConstantExprKind`
  variants including `Adt` first-class; closest peer's choice
  matches stage-4.
- `prior-art/llvm.md` Finding 3 -- `ConstantArray`,
  `ConstantStruct` first-class.
- `prior-art/mrustc.md` Finding 3 -- demote-to-static; cited as
  cross-check, not adopted.
- `prior-art/compcert.md` Finding 2 -- demote-to-init_data;
  cited as cross-check, not adopted.
- `prior-art/regseq-model-old.md` Findings 3, 9 -- no aggregate
  variant; re-derivation analysis confirms the old choice does
  not survive stage-4 framing.
- `prior-art/README.md` §2.2 -- cross-cluster framing of the
  fork.

**Cross-refs.** `spec-snapshot.md` §9.7 (`Aggregate`,
`EnumValue`, `Bytes` variants); `spec-snapshot.md` §13.2
(vtable instances -- the synthesised-Static path that does not
demote); `spec-snapshot.md` §15 (bridge translation rules
extending with `mir::ConstValue` dispatch).

---

## 2. Reference primer

(To be filled as concepts surface during stage-4 work that
deserve concise per-stage explanation. Stage 3 used Section 2
for layout-primer / niche-encoding-primer / TyInterner-pattern-
primer / region-erasure-as-invariant material -- analogous
material may surface here for the const-eval result form,
the AllocId-to-GlobalAlloc resolution, the type-level vs
value-level partition, etc.)

---

## 3. Cross-references

- `../README.md` §5.1 (stage lifecycle), §5.2 (decision tags),
  §5.9 (spec.md update guidelines).
- `memory.md` §1 (workflow rhythm), §2 (stage-4-specific
  principles), §4 (Codex review prompt template).
- `status.md` "Open decisions" (live D-tag catalogue).
- `prior-art/README.md` (cross-cluster prior-art synthesis).
- `prior-art/<project>.md` (per-project primary-source
  research).
- Top-level `../spec.md` §9.7 (`ConstInterner` and the const
  vocabulary stub stage 4 fills); §3.3 (model validation order);
  §13.2 (vtable family -- primary consumer of `Bytes::Reloc`);
  §15.1 (rejections).
