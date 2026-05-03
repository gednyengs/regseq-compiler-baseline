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
pattern: a child `ConstId` resolves via `canonical_const`
(D-4-2's per-`Const`-variant byte-encoding) to the same
canonical content that interned the child's entry,
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

### D-4-2 -- `ConstIdentityKey` per-variant byte-form schema [LOCKED]

**Question.** Given the locked 11-variant `Const` enum (D-4-1),
the locked `Byte` three-state, and the locked `InstanceRef`
4-tuple, what is the byte-level canonical form of
`ConstIdentityKey` per variant — such that two `Const` values
with semantically identical content produce byte-identical
identity-key bytes across builds, and so that
`canonical_args.recurse(ConstId)` (the §12.3 `ConstId`
walk-rule row) has a concrete encoding to call?

**Key options.**

1. **Option A -- mirror §12.3's per-`Ty` table directly.** A
   parallel per-`Const`-variant table with `u8` variant tags,
   tag-specific payloads, `u32` LE length prefixes, recursive
   `canonical_const(ConstId)` for nested children. Same
   tagged-tree convention; same append-only policy; same LE
   default for multi-byte integers.
2. **Option B -- value-bit-only key (skip variant tags).**
   Drop variant tags and key only on payload bits; rely on
   the consumer's `Ty` to disambiguate.
3. **Option C -- per-`Const` ad-hoc encoding.** Each variant
   gets a bespoke encoding optimized for its own shape (e.g.,
   variable-width integers, sentinel-based `Byte` encoding,
   compact aggregate forms).

**Decision.** Option A — closed across nine sub-decisions
D-4-2.1 through D-4-2.9 pinning the byte form of each variant
class and the recursion / acyclicity rules. The result extends
§9.7 with a new "Per-`Const`-variant byte-encoding tags"
subsection mirroring §12.3's per-`Ty` byte-encoding-tags table,
and replaces the §12.3 `ConstId` walk-rule stub with a concrete
recursion rule pointing at the new subsection.

**Reasoning.**

*Why not Option B (value-bit-only key).* Skipping variant tags
collapses canonically-distinct values into one key (e.g.,
`Bool(false)` and `ZST` would both reduce to "no payload";
`Int(I32, 0)` and `Uint(U32, 0)` would both reduce to "16
zero bytes"). D-4-1.1 made the variant tag itself part of the
equality class; the byte-form must preserve that — variant tag
is non-negotiable.

*Why not Option C (per-variant ad-hoc).* Two costs: (a) every
deviation from §12.3's conventions creates a special case the
canonicaliser must handle (variable-width integers need a
decoding rule; sentinel-based `Byte` encoding needs an escape
mechanism because `Value(0xFE)` is a legal byte). (b) Symmetry
with §12.3 reduces validation surface — the same byte-stream
walker handles both the per-`Ty` and per-`Const` trees with
shared rules for tags, lengths, recursion, and endianness.

*Why mirror §12.3 directly.* Three voices: (i) §12.3's per-`Ty`
table is the load-bearing precedent — the same conventions
(`u8` tag, LE multi-byte ints, `u32` LE length prefixes,
recursive walks) work for every encoded IR identifier. (ii) The
`ConstId` walk-rule row in §12.3 already calls into
`canonical_const(ConstId)` from the `GenericArgs` container's
`GenericArg::Const(ConstId)` arm (the only `ConstId` call site
in the per-`Ty` walk; `Ty::Array(elem, n: u64)` carries a
literal `u64`, not a `ConstId`); the symmetric form makes the
cross-walk uniform. (iii) The `GenericArgs` container encoding
in §12.3 is
itself the precedent for the `u32` LE length prefix — D-4-2
reuses the same prefix width for `Vec<Byte>`, `Vec<ConstId>`,
and `EnumValue.payload`.

**Methodology rules invoked.**

- *Store the result, not the input.* Each variant's encoding
  pins a fixed byte form, never a derivation procedure.
- *Cross-build canonicalisation uses stable identity.* Every
  recursive sub-component (`canonical_ty`, `def_path_hash`,
  `args_key` raw bytes) is already cross-build-stable;
  recursion preserves that property.
- *No defensive fallbacks.* No "Other" / "Opaque" escape
  encoding; the 11 variants are exhaustive (per D-4-1).
- *No forward-looking placeholders.* The deferred reference-
  typed `ConstMemoryStatic` family takes tag value `11+` when
  D-4-3+ lands; no tag values are reserved.
- *Compositional behavior beats special-case flags.* Each
  variant's encoding follows from its declared shape; no
  special-case rules.

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 6 -- rustc's
  `ScalarInt::data: u128` widening pattern is the precedent
  for D-4-2.2's `u128` LE value-bit storage.
- `prior-art/charon.md` Finding 6 -- `Byte` three-state from
  D-4-1.8 has a direct sub-tag analogue here (D-4-2.5).
- `prior-art/llvm.md` Finding 5 -- per-pointer-type
  `ConstantPointerNull` confirms `NullPtr(TyId)` recursion
  into `canonical_ty` (D-4-2.7).
- `prior-art/mlir.md` Finding 2 -- `TypedAttrInterface`'s
  inline-tag pattern matches D-4-2.1's variant-tag-as-
  equality-class.
- `prior-art/README.md` §3.1 -- documents the negative
  cross-cluster result for cross-build content-addressed const
  identity (5-of-5 non-rustc projects fail; rustc's
  `HashStable for AllocId` is the only adjacent precedent and
  the inspiration D-4-1.5's `InstanceRef` 4-tuple already
  inherits). Cited here as the framing for D-4-2's recursive
  `canonical_const` byte form: every recursive sub-component
  of the per-`Const` byte form (`canonical_ty`,
  `def_path_hash`, `args_key.to_be_bytes()`) inherits the same
  build-stable-projection discipline pinned in D-4-1.5.

**Worked examples.** Pinned in sub-decisions D-4-2.1 through
D-4-2.9; assembled into `spec-snapshot.md` §9.7's
"Worked examples" block within the new Per-`Const`-variant
byte-encoding tags subsection.

**Cross-refs.** `spec-snapshot.md` §9.7 (per-`Const`-variant
byte-encoding tags subsection); `spec-snapshot.md` §12.3
(`ConstId` walk-rule row); `spec-snapshot.md` §3.3 Phase B
step 9 (acyclicity sub-check); `status.md` "Open decisions";
`review-log.md` rounds (none yet).

---

### D-4-2.1 -- Variant tag width + numeric assignment + append-only [LOCKED]

**Question.** What `u8` discriminator value gets assigned to
each of the 11 variants, and how is future drift prevented?

**Key options.**

1. `u8` disc, contiguous `0..=10`, append-only.
2. `u16` disc, more headroom but breaks symmetry with §12.3.
3. `u8` disc with reserved gaps for future variants.

**Decision.** Option 1. Numeric assignment in source-declaration
order:

```text
0  = Bool        1 = Char        2 = Int        3 = Uint
4  = Float       5 = ZST         6 = Bytes      7 = Aggregate
8  = EnumValue   9 = NullPtr    10 = Reloc
```

Append-only: future variants (e.g., the deferred
`ConstMemoryStatic`-backed reference-typed family per
§13.3 / §13.4) take tag values `11+`; existing tags are
immutable so cross-build hashes for already-admitted consts
never change. No tag values reserved.

**Reasoning.** Mirrors §12.3's per-`Ty` table directly — same
`u8` width, same contiguous-from-zero assignment, same
append-only stability promise. 11 variants in `u8` leave 245
currently-unassigned tag values available for append-only
future variants (the no-forward-looking-placeholders rule means
those values are not pre-reserved for any specific extension —
they're simply the unused range of the `u8` discriminant
space). `u16` would create asymmetry between the per-`Ty` and
per-`Const` walks. Reserved gaps would formalise "where future
variants land," which violates the no-forward-looking-
placeholders rule.

**Methodology rules invoked.** *No forward-looking placeholders;*
*compositional behavior beats special-case flags;* *cross-build
canonicalisation via stable identity.*

**Prior-art evidence.**

- `prior-art/llvm.md` Findings 1, 5 -- LLVM's `Constant`
  hierarchy uses a per-class type-id (variant-tag-equivalent).
- `prior-art/mlir.md` Finding 2 -- `TypedAttrInterface`
  uses inline-tagged attributes.
- `prior-art/charon.md` Finding 1 -- 13-variant
  `ConstantExprKind` uses per-variant tags.
- `prior-art/mrustc.md` Finding 1 -- 10-variant
  `MIR::Constant` uses per-variant tags.
- `prior-art/regseq-model-old.md` Finding 1 -- 11-variant
  scalar-only enum uses per-variant tags.
  Cross-project: 5-of-5 projects use a tag-then-payload
  variant-tag convention; D-4-2.1's `u8` width and
  contiguous-from-zero assignment is the local materialisation.

**Cross-refs.** `spec-snapshot.md` §9.7 (per-`Const`-variant
byte-encoding tags); `spec-snapshot.md` §12.3 (per-`Ty`
parallel).

---

### D-4-2.2 -- Scalar payload byte forms [LOCKED]

**Question.** How does each scalar variant's payload (`Bool`,
`Char`, `Int(k, _)`, `Uint(k, _)`, `Float(k, _)`) serialise
byte by byte?

**Key options.**

1. **Per-`Ty`-mirroring** with `u128` LE for value-bit storage,
   `u8` discriminants for `IntKind` / `UintKind` / `FloatKind`
   reusing the §10 `#[repr(u8)]` enum-definition assignments
   (the same values §12.3's per-`Ty` table reuses for `Ty::Int(k)`,
   `Ty::Uint(k)`, `Ty::Float(k)`).
2. **Per-variant minimal width** (e.g., `Int(I8, _)` emits 1
   byte; `Int(I32, _)` emits 4 bytes). Saves 12-15 bytes per
   small-width scalar but requires a per-`IntKind` decoding
   rule.
3. **`u64` LE compromise** with `(low_u64, high_u64)` pair for
   `I128`/`U128`/`F128` only.

**Decision.** Option 1.

```text
Bool(b)        : u8 (0x00 | 0x01)
Char(cp)       : u32 LE Unicode scalar value
Int(k, v)      : u8 IntKind disc + u128 LE value (sign-extended for narrow)
Uint(k, v)     : u8 UintKind disc + u128 LE value (zero-extended for narrow)
Float(k, v)    : u8 FloatKind disc + u128 LE bit-pattern (low-order
                 holds IEEE bits; high-order zero for narrow widths)
```

`IntKind` / `UintKind` / `FloatKind` discriminants reuse the
`#[repr(u8)]` numeric assignments pinned in §10's enum
definitions (the normative source: `IntKind { I8=0, ...,
Isize=5 }`, `UintKind { U8=0, ..., Usize=5 }`, `FloatKind
{ F16=0, F32=1, F64=2, F128=3 }`). §12.3's per-`Ty` table reuses
these; D-4-2's per-`Const` table reuses them again.

**Reasoning.** D-4-1.1 pinned `u128` storage on the value side
("smaller widths sign-extend to `u128`") for canonicalisation
determinism — the byte form follows directly. Option 2's
per-width minimal-byte encoding violates "store the result, not
the input" because it requires a decoding rule keyed on the
`IntKind` discriminant; the canonical form should be readable
without conditional logic on width. Option 3 is half-uniform
which is worse than fully-uniform.

The float padding rule was load-bearing in Codex review R5/R6
of D-4-1: bit-pattern occupies low-order bits, high-order bits
are zero for narrow widths (`1.0f32` stores as
`0x3f800000_u128` with the upper 96 bits zero). The byte form
mirrors this directly — first 4 bytes of the `u128` LE encoding
hold the IEEE-754 binary32 bit-pattern; bytes 4..16 are `0x00`.

**Methodology rules invoked.** *Store the result, not the
input;* *information preservation* (every scalar's full bit
content survives unchanged into the byte form); *cross-build
canonicalisation via stable identity*.

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 2 -- `ScalarInt::data: u128`
  is the same widening pattern; the byte form follows.
- `prior-art/mrustc.md` Finding 2 -- `MIR::Constant`'s
  per-scalar variants demonstrate the inline-tag-then-bits
  pattern.
- `prior-art/charon.md` Finding 2 -- `ScalarValue` per-flavor
  variants use the same width-extending discipline.

**Cross-refs.** `spec-snapshot.md` §9.7 (scalar variants
declarations); `spec-snapshot.md` §10 (`IntKind` / `UintKind` /
`FloatKind` `#[repr(u8)]` enum definitions — normative source
for the disc values); `spec-snapshot.md` §12.3 (per-`Ty` table
reuses the same disc values for `Ty::Int(k)`, `Ty::Uint(k)`,
`Ty::Float(k)`).

---

### D-4-2.3 -- Length-prefix width [LOCKED]

**Question.** What integer width encodes the lengths of
`Vec<Byte>`, `Vec<ConstId>`, and `EnumValue.payload`?

**Key options.**

1. `u32` LE (matches §12.3's `GenericArgs` container).
2. `u16` LE — saves 2 bytes per variable-length payload but
   caps at 65 535 entries.
3. varint — saves bytes for small payloads but adds a per-call
   decoding rule.

**Decision.** Option 1 (`u32` LE for all three positions).

**Reasoning.** Matches §12.3's existing precedent ("`u32` LE
arg_count" in the `GenericArgs` container encoding) — uniformity
across the per-`Ty` and per-`Const` walks. `u16` is too tight:
a 256 KB const blob has 262 144 byte entries, exceeding `u16`'s
65 535 cap. `varint` saves marginal bytes for typical payloads
but introduces a decoding rule that breaks §12.3's
"lengths emit as little-endian `u32` prefixes" invariant and
complicates canonical-form auditing.

**Methodology rules invoked.** *Compositional behavior beats
special-case flags* (one length-prefix rule, not three);
*cross-build canonicalisation via stable identity*.

**Prior-art evidence.**

- `prior-art/llvm.md` Finding 7 -- `ConstantDataArray` length
  is fixed-width-encoded in bitcode.
- `prior-art/charon.md` Finding 5 -- `ConstantExprKind::Adt`
  fields use a fixed-width length prefix.
- `prior-art/mrustc.md` Finding 7 -- bincode-style
  serialisation of variable-length payloads uses `u32`-equivalent
  prefixes.
  Cross-project: no project surveyed used varint for canonical-
  key purposes; `u32` LE matches the §12.3 `GenericArgs`
  precedent already pinned in stage-1.

**Cross-refs.** `spec-snapshot.md` §9.7 (per-`Const`-variant
byte-encoding tags `Bytes` / `Aggregate` / `EnumValue` rows);
`spec-snapshot.md` §12.3 (`GenericArgs` container precedent).

---

### D-4-2.4 -- `EnumValue.variant_idx` encoding [LOCKED]

**Question.** How does `variant_idx: VariantIdx` (a `u32`
layout-local index per `spec-snapshot.md` §3 `VariantIdx`
type-alias declaration) lay out in the canonical key, and where
is it placed relative to the `payload` length prefix?

**Key options.**

1. `u32` LE, placed before the `payload` length prefix.
2. `u32` LE, placed after the `payload` length prefix (or
   appended at the end).
3. `u16` LE (variant indices unlikely to exceed 65 535 in
   practice, but no enforced cap).

**Decision.** Option 1 (`u32` LE, before `payload` length).

```text
EnumValue { variant_idx, payload } :
  tag(8)
  + u32 LE variant_idx
  + u32 LE payload_count
  + payload_count * canonical_const(ConstId)
```

**Reasoning.** `VariantIdx` is post-mono-stable (set at HIR
resolution; preserved through monomorphisation). The
placement-before-payload mirrors how `EnumValue` reads as
"first identify the variant, then the payload" — same lexical
order as the field declaration in D-4-1.4 and §9.7's
`Const::EnumValue` declaration (`variant_idx` declared first,
`payload` second). `u32` width
uses the same rationale as D-4-2.3; `u16` would create a
special case (discriminant-on-`Ty::Enum` is `u32`-stored per
§11.1, so the byte form should match).

**Methodology rules invoked.** *Store the result, not the
input;* *cross-build canonicalisation via stable identity*.

**Prior-art evidence.**

- `prior-art/charon.md` Finding 4 -- `Aggregate` carries
  `VariantId(usize)` analogue; the byte-form discipline mirrors
  there.
- `prior-art/regseq-model-old.md` Finding 3 -- `Variant(idx,
  payload)` shape predates this in regseq-model-old; preserves
  the first-identify-variant-then-payload reading.

**Cross-refs.** `spec-snapshot.md` §9.7 (`EnumValue` declaration);
`spec-snapshot.md` §11.1 (`VariantLayout.discriminant` and
`EnumShape.variants` — the consumer-context table that
`variant_idx` indexes into); `spec-snapshot.md` §3.3 step 9
(in-bounds validation).

---

### D-4-2.5 -- `Byte` sub-discriminator + payload layout [LOCKED]

**Question.** Within `Bytes(Vec<Byte>)`'s payload, how is each
`Byte` (the three-state `Uninit | Value(u8) | Reloc(InstanceRef)`)
encoded?

**Key options.**

1. **`u8` sub-disc + payload.** `Uninit = 0` (no payload);
   `Value(b) = 1` + 1 byte; `Reloc(r) = 2` + InstanceRef bytes
   (per D-4-2.8).
2. **Sentinel-based compact.** `0xFE = Uninit`, `0xFF = Reloc`
   (followed by InstanceRef), all other byte values =
   `Value(byte_value_itself)`. Saves 1 byte per `Value`.
3. **Run-length encoded.** Compress runs of `Uninit` and
   `Value` into `(count, value)` pairs.

**Decision.** Option 1.

```text
byte_sub_tag = u8
  0  = Uninit             (no payload)
  1  = Value(u8)          + 1 byte
  2  = Reloc(InstanceRef) + 34 bytes (per D-4-2.8)
```

**Reasoning.** Matches §12.3's tagged-tree convention (1-byte
sub-disc + tag-specific payload). Option 2 saves bytes in
typical-case `Value` runs but `Value(0xFE)` and `Value(0xFF)`
are legal byte values, so the sentinel scheme requires escape
encoding — and the escape encoding rule complicates
canonical-form auditing more than the saved bytes are worth.
Option 3 introduces stateful encoding (run boundaries depend on
neighbouring values), which violates the per-element
independence that the rest of the canonical scheme maintains.

The well-formedness invariant (maximal-relocation-runs are
pointer-shaped, per §9.7's `Byte` enum
"Byte-sequence well-formedness invariants" subsection) is
enforced in §3.3 Phase B step 9, not at encoding time — the
byte form encodes whatever sequence is in `Vec<Byte>`
faithfully; validation rejects ill-formed sequences.

**Methodology rules invoked.** *Store the result, not the
input;* *compositional behavior beats special-case flags;*
*no defensive fallbacks*.

**Prior-art evidence.**

- `prior-art/charon.md` Finding 6 -- `Byte` three-state
  precedent (D-4-1.8); the sub-tag pattern follows from the
  three-state shape directly.
- `prior-art/llvm.md` Finding 7 -- `ConstantDataArray` /
  `ConstantArray` use a tagged constant-element approach,
  cross-checking the per-element-tag pattern.

**Cross-refs.** `spec-snapshot.md` §9.7 (`Byte` enum,
`Bytes(Vec<Byte>)` variant, byte-sequence well-formedness
invariant); `spec-snapshot.md` §3.3 step 9 (validation rules).

---

### D-4-2.6 -- Aggregate / EnumValue recursion + acyclicity [LOCKED]

**Question.** How does `canonical_const(ConstId)` recurse into
nested `ConstId` children, and is recursion termination
validated or assumed?

**Key options.**

1. **Depth-first inline + three-state-DFS-validated acyclicity.**
   Recursion inlines child bytes into parent's stream; Phase B
   step 9 walks the const-graph with three-state DFS marks
   (`Unseen` / `Visiting` / `Done`) and rejects back-edges to
   `Visiting` nodes.
2. **Depth-first inline + assumed acyclicity.** Recursion
   inlines; no validation pass — termination assumed by const-
   eval semantics.
3. **Depth-bounded recursion** (e.g., limit to a configured
   max depth) with the depth bound counted as an invariant.

**Decision.** Option 1.

Recursion shape: for each child `ConstId`, look up the `Const`,
recurse to compute its canonical key, append the resulting
bytes to the parent's stream. The recursion edges are exactly
`Aggregate.0` and `EnumValue.payload`; `Bytes::Reloc` and
`Reloc` carry `InstanceRef` (not `ConstId`), so they do not
extend the const-recursion graph.

Acyclicity validation: extends §3.3 Phase B step 9 with a
**three-state DFS** (`Unseen` / `Visiting` / `Done`) over
`Aggregate` / `EnumValue` payload reachability. Entering a node
sets it to `Visiting`; leaving sets it to `Done`. An edge to a
`Visiting` node is a back-edge (cycle) and is a
model-corruption error; an edge to a `Done` node is skipped
(shared DAG children are legitimate — same canonical-content
child can appear under multiple parents). A single-bit
visited-set is insufficient: it cannot distinguish "currently
on the recursion stack" from "already validated."

Validation-order refinement: §3.3 step 4's `ConstInterner`
content-uniqueness pass is restricted to the type-level subset
(scalar variants `Bool` / `Char` / `Int` / `Uint` / `Float`,
whose canonical-content keys do not recurse through `ConstId`).
Value-level lookup injectivity and canonical-form
well-formedness recompute the canonical keys for value-level
entries — which DOES recurse — and therefore must run AFTER
acyclicity is established. Step 9 is ordered:
(1) `NullPtr` / `Reloc` / `Bytes::Reloc` sub-reference resolution;
(2) `Bytes` / `Aggregate` / `EnumValue` content checks;
(3) const-graph acyclicity via three-state DFS;
(4) value-level canonical-content key recomputation +
    lookup-map injectivity over the value-level subset.
This bounds recursive key computation to the verified DAG,
matching the same defence posture used for §3.3 Phase C's
vtable rebuild duplicate-check.

**Reasoning.** Well-typed Rust + const-eval termination
guarantee acyclicity at construction time — the bridge cannot
produce a cyclic const-graph from any legitimate input. But the
artefact is serialised; downstream stages re-validate the model
without the construction trace. A tampered serialised
`ConstInterner` could embed a cycle that wasn't present at
bridge time, and canonical-key computation would not terminate.
Option 2's "assumed acyclicity" requires trust in the upstream
artefact source; option 1 verifies. The cost is one DFS pass
linear in const-graph size — same defence posture used for
§3.3 Phase C's vtable rebuild duplicate-check.

Option 3 (depth-bounded) introduces a configurable invariant
(what's the depth bound?) that doesn't actually defend against
cycles, only catches them late.

**Methodology rules invoked.** *No defensive fallbacks*
(reject corruption, don't paper over);
*cross-build canonicalisation via stable identity* (acyclicity
is part of the canonicality guarantee).

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 4 -- rustc's const-eval
  query system relies on memoization with cycle detection;
  same fundamental discipline.
- `prior-art/charon.md` Finding 5 -- charon's
  `ConstantExprKind::Adt(_, _, fields)` recurses into
  `Box<ConstantExpr>` children; cycle detection is implicit
  via the construction trace, but charon's construction
  context is closer to ours so the analogue holds.

**Cross-refs.** `spec-snapshot.md` §9.7 (`Aggregate` /
`EnumValue` declarations; `canonical_const(ConstId)` recursion
rule); `spec-snapshot.md` §3.3 Phase B step 9 (acyclicity
sub-check addition).

---

### D-4-2.7 -- `NullPtr(TyId)` type encoding [LOCKED]

**Question.** How does the `TyId` field in `NullPtr(TyId)`
serialise into the canonical key?

**Key options.**

1. **Recursive `canonical_ty(TyId)`.** Resolve via
   `model.types.resolve(...)` and recurse on the resolved `Ty`
   per §12.3's per-IR-identifier termination rule for `TyId`.
2. **Raw `TyId` integer.** Emit the local `u32` index directly.
3. **`def_path_hash`-only** (for nominal types) with structural
   recursion for non-nominal pointee types.

**Decision.** Option 1.

```text
NullPtr(t) : tag(9) + canonical_ty(t)
```

**Reasoning.** `TyId` is a build-local index — emitting it
directly would defeat cross-build stability (two builds with
different `TyId` allocation orders would produce different
keys for `null::<i32>()`). `canonical_ty(t)` produces
cross-build-stable bytes for any `Ty` content. This is the only
`Const` variant whose canonical key directly recurses into a
`Ty` — and it must, because `null::<T>()` ≠ `null::<U>()` is
constitutive of value identity (per D-4-1.6). Option 3 splits
the recursion rule conditionally on `Ty` shape, complicating
the byte form for no benefit since `canonical_ty` already
handles every `Ty` variant uniformly.

**Methodology rules invoked.** *Cross-build canonicalisation
via stable identity;* *compositional behavior beats
special-case flags*.

**Prior-art evidence.**

- `prior-art/llvm.md` Finding 5 -- `ConstantPointerNull` is
  per-pointer-type, confirming type-as-part-of-identity for
  null pointers.
- `prior-art/charon.md` Finding 3 -- `ConstantExprKind`'s
  raw-pointer / null cases similarly carry the pointee type.

**Cross-refs.** `spec-snapshot.md` §9.7 (`NullPtr` variant
declaration; D-4-1.6 rationale); `spec-snapshot.md` §12.3
(per-IR-identifier `TyId` walk-rule row).

---

### D-4-2.8 -- `Reloc(InstanceRef)` byte layout [LOCKED]

**Question.** How does the `InstanceRef` 4-tuple (used by
`Reloc(InstanceRef)` and `Byte::Reloc(InstanceRef)`) lay out
byte by byte?

**Key options.**

1. **Field-order, fixed-width.** Emit each component in §9.7's
   `InstanceRef` struct-declaration order: `regseq_kind_rank`
   (`u8`) + `def_path_hash` (16 bytes) + `args_key` (16 bytes
   via `to_be_bytes()`) + `fn_shim_disambig` (`u8`) = 34 bytes
   total.
2. **Length-prefixed** (in case future `InstanceRef` variants
   widen the field set).
3. **Reordered for alignment** (e.g., 16-byte fields first,
   then `u8` fields).

**Decision.** Option 1.

```text
InstanceRef encoding (34 bytes):
  u8       regseq_kind_rank
  16 bytes def_path_hash       (StableCrateId u64 LE + local_hash u64 LE
                                per §12.3 per-IR-identifier rule)
  16 bytes args_key            (`ArgsKey.0.to_be_bytes()` — the canonical
                                big-endian `u128` serialisation; native
                                memory layout / `transmute` MUST NOT be
                                used)
  u8       fn_shim_disambig
```

The `args_key` field is emitted as `ArgsKey.0.to_be_bytes()`
(the standard big-endian `u128` serialisation). This is the
documented BE exception to §12.3's general
"multi-byte integers are little-endian" rule, anchored on the
BE-load convention in §12.3 (`ArgsKey::EMPTY.0` is "loaded as
a big-endian u128"). Since the serialised model stores
`ArgsKey` only as a `u128`, the canonical byte form must be
specified by operation on the `u128` value (not by appeal to
"underlying bytes," which leaves a native-endian /
`transmute` interpretation open). `to_be_bytes()` reproduces
the original SHA-256-leading-16-bytes sequence that the bridge
computed before storing into `ArgsKey.0`.

**Reasoning.** Field-order matches the struct declaration —
auditors can verify the byte form by reading the declaration
top-to-bottom. Length-prefixing (Option 2) is a no-forward-
looking-placeholder violation: there is no plan for
`InstanceRef` to widen, and the spec is closed at 4 fields.
Reordering for alignment (Option 3) produces a byte form
inconsistent with the source-level field order, which makes
auditing harder.

The `args_key`-as-`to_be_bytes()` choice resolves the
tension between §12.3's two endianness statements (the BE-load
convention for `ArgsKey.0` vs the LE-default for multi-byte
integers): the byte stream emits `ArgsKey.0.to_be_bytes()` as
the documented BE exception, anchored on the same BE-load
convention §12.3 already pins for `ArgsKey::EMPTY`.

**Methodology rules invoked.** *Store the result, not the
input;* *no forward-looking placeholders* (no length prefix
for unbounded extensibility); *cross-build canonicalisation
via stable identity*.

**Prior-art evidence.**

- `prior-art/rustc.md` Finding 8 -- rustc's `DefPathHash`
  StableCrateId + local_hash decomposition is the precedent
  for the 16-byte `def_path_hash` layout; reused here.
- `prior-art/README.md` §3.1 -- 5-of-5 non-rustc projects fail
  cross-build determinism; rustc's `HashStable for AllocId`
  (the resolve-then-hash discipline) is the only adjacent
  precedent. D-4-1.5's `InstanceRef` 4-tuple already embeds
  this discipline at the value-shape level; D-4-2.8 pins the
  byte-form serialisation that preserves it (each component
  emits its own cross-build-stable byte form;
  `args_key.to_be_bytes()` is the resolve-then-hash result).

**Cross-refs.** `spec-snapshot.md` §9.7 (`InstanceRef` struct
declaration; `Reloc` and `Byte::Reloc` variants);
`spec-snapshot.md` §12.3 (`def_path_hash` byte form;
`ArgsKey::EMPTY` definition; per-IR-identifier termination
rules).

---

### D-4-2.9 -- `ZST` identity (tag-only) [LOCKED]

**Question.** Does `ZST` carry any payload bytes in the
identity key?

**Key options.**

1. **Tag-only** (no payload bytes).
2. **Tag + sentinel byte** (e.g., `0x00` to make every variant
   carry at least one payload byte).
3. **Tag + consumer-`TyId`** (would force ZST to behave like
   `NullPtr(TyId)`).

**Decision.** Option 1.

```text
ZST : tag(5)         (single byte, no payload)
```

**Reasoning.** D-4-1.2 pinned single canonical `ZST` (no
payload — type from consumer context); D-4-1.7 pinned
inline-only with no outer wrapper. The byte form follows
directly: variant tag is the entire encoding.

Option 2 (sentinel byte) buys nothing — the variant tag
already disambiguates `ZST` from every other variant. Option
3 (consumer-`TyId`) reintroduces the type-tag wrapper that
D-4-1.7 explicitly rejected, undoes the dedup-explosion
mitigation from D-4-1.2 (one canonical `ZST` for all
zero-sized types), and would break the niche-preservation
discipline (D-4-1's contract that no variant payload widens
`ConstId`).

**Methodology rules invoked.** *Compositional behavior beats
special-case flags;* *cross-build canonicalisation via stable
identity*.

**Prior-art evidence.**

- `prior-art/charon.md` Finding 7 -- charon collapses ZSTs
  similarly; the byte-level analogue is tag-only.

**Cross-refs.** `spec-snapshot.md` §9.7 (`ZST` variant
declaration; D-4-1.2 single-canonical rationale; D-4-1.7
inline-only rationale).

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
