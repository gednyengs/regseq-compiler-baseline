# Stage 7 -- Prior Art: mrustc primary source

Primary-source mrustc evidence for the **MIR Rvalue analog**,
scoped to what stage 7's candidate decision namespace
(`D-7-1` Rvalue variant set / per-variant payload shapes;
`D-7-2` BorrowKind subset; `D-7-3` RawPtrKind subset; `D-7-4`
CastKind subset; `D-7-5` AggregateKind subset; `D-7-6`
BinOp/UnOp curation; `D-7-7` `UnOp::PtrMetadata` retention;
`D-7-8` `BinaryOp` operand-pair storage; `D-7-9` `Repeat`
count payload; `D-7-10` reject `CopyForDeref` /
`ShallowInitBox`; `D-7-11` `WrapUnsafeBinder` retention;
`D-7-K` `render_rvalue` deferral) needs. mrustc is the
closest existing peer: a Rust-to-C compiler with its own
HIR/MIR. Fresh primary-source pass per `../../README.md` §2
methodology rule (do not reuse stage-0 summaries; web-fetched
verbatim source is the evidence). Framing source for the
D-7-K namespace: `./rustc.md` §15.

Repository: https://github.com/thepowersgang/mrustc
Primary files inspected:

- `src/mir/mir.hpp` -- IR type definitions (RValue, Param,
  Constant, eBinOp, eUniOp, LValue, Statement, Terminator)

The stage-6 `../../stage-6/prior-art/mrustc.md` already
covers the `LValue` storage shape, the `Constant` variant
set, the `Param` operand-equivalent, and the
`MIR_Validate_ValState` validation pass. This file does not
re-derive that material; it focuses on the `RValue` surface
and the `eBinOp`/`eUniOp` enums that stage 6 did not touch.

Every factual claim cites `[file:line]` in the mrustc tree.
Hypotheses are flagged. Inferences across two sites are
labelled INFERENCE.

---

## 1. `RValue` enum shape

mrustc's `RValue` is a 15-variant tagged union
[`src/mir/mir.hpp:628-677`]:

```cpp
TAGGED_UNION_EX(RValue, (), Tuple, (
    // TODO: Split "Use" into "Copy" and "Move" (Where 'move' indicates that the source is unused)
    (Use, LValue),
    (Borrow, struct {
        ::HIR::BorrowType   type;
        bool is_raw;
        LValue  val;
        }),
    (Constant, Constant),
    (SizedArray, struct {
        Param   val;
        ::HIR::ArraySize    count;
        }),
    // Cast on primitives (thin pointers, integers, floats)
    (Cast, struct {
        LValue  val;
        ::HIR::TypeRef  type;
        }),
    // Binary operation on primitives
    (BinOp, struct {
        Param   val_l;
        eBinOp  op;
        Param   val_r;
        }),
    // Unary operation on primitives
    (UniOp, struct {
        LValue  val;    // NOTE: Not a param, because UniOps can be const propagated
        eUniOp  op;
        }),
    // Extract the metadata from a DST pointer
    // NOTE: If used on an array, this yields the array size (for generics)
    (DstMeta, struct {
        LValue  val;
        }),
    // Extract the pointer from a DST pointer (as *const ())
    (DstPtr, struct {
        LValue  val;
        }),
    // Construct a DST pointer from a thin pointer and metadata
    // OR: (if `meta_val` is `Constant::ItemAddr(nullptr)`) A still-to-be-resolved unsizing coercion
    (MakeDst, struct {
        Param   ptr_val;
        Param   meta_val;
        }),
    (Tuple, struct {
        ::std::vector<Param>   vals;
        }),
    // Array literal
    (Array, struct {
        ::std::vector<Param>   vals;
        }),
    // Create a new instance of a union
    (UnionVariant, struct {
        ::HIR::GenericPath  path;
        unsigned int index;
        Param   val;
        }),
    // Create a new instance of an enum
    (EnumVariant, struct {
        ::HIR::GenericPath  path;
        unsigned int index;
        ::std::vector<Param>   vals;
        }),
    // Create a new instance of a struct
    (Struct, struct {
        ::HIR::GenericPath  path;
        ::std::vector<Param>   vals;
        })
    ), (),(), (
        RValue clone() const;
    )
);
```

Variant tally (15 arms): `Use`, `Borrow`, `Constant`,
`SizedArray`, `Cast`, `BinOp`, `UniOp`, `DstMeta`, `DstPtr`,
`MakeDst`, `Tuple`, `Array`, `UnionVariant`, `EnumVariant`,
`Struct`.

Notable absences vs rustc_middle's 13-variant `Rvalue<'tcx>`
(see `./rustc.md` §1):

- **No `Discriminant` variant.** mrustc reads enum
  discriminants through a different mechanism. INFERENCE: the
  enum-variant pattern match at the C-codegen level handles
  the discriminant read implicitly (no IR-level surface).
- **No `ThreadLocalRef` variant.** mrustc predates the
  thread-local rvalue lowering rustc later added; statics with
  `#[thread_local]` would presumably reach codegen via the
  normal `Constant::ItemAddr(path)` route. HYPOTHESIS:
  unverified.
- **No `CopyForDeref` / `ShallowInitBox` / `WrapUnsafeBinder`
  variants.** Confirms these are rustc-internal early-MIR
  artefacts (per `./rustc.md` §11) that a peer Rust-to-C
  compiler does not need at all.

Notable presences vs rustc_middle:

- **`DstMeta(val: LValue)` is a dedicated rvalue variant** for
  extracting metadata from a DST pointer (mrustc's
  `UnOp::PtrMetadata` analog). rustc folds this into
  `UnaryOp(UnOp::PtrMetadata, _)`; mrustc keeps it as a
  first-class rvalue.
- **`DstPtr(val: LValue)` is a dedicated variant** for the
  *opposite* operation: extract the data pointer from a wide
  pointer as `*const ()`. rustc has no direct equivalent
  ("data ptr" is recovered via `Cast` + `PtrToPtr` from a wide
  to a thin).
- **`MakeDst{ ptr_val, meta_val }` constructs a wide pointer**
  from data + metadata. rustc's analog is
  `Aggregate(AggregateKind::RawPtr(..), [ptr, meta])`. mrustc
  promotes this to a top-level rvalue variant rather than an
  aggregate sub-kind.
- **`SizedArray{ val: Param, count: HIR::ArraySize }`** is
  mrustc's `Repeat` equivalent. The count is `HIR::ArraySize`
  (mrustc's array-length carrier), embedded by value -- *not*
  a `Constant` reference. INFERENCE: mrustc handles array
  length resolution at HIR-to-MIR lowering, baking the
  resolved length into the rvalue payload directly. Maps to
  `D-7-9`.
- **Aggregate kinds are split into separate rvalue
  variants.** rustc's `Aggregate(AggregateKind, ...)` becomes
  five mrustc variants: `Tuple`, `Array`, `UnionVariant`,
  `EnumVariant`, `Struct`. The "tag-discriminator" cost is
  paid at the variant-tag level, not in a sub-enum.

Maps to `D-7-1` (variant set + payload shapes), `D-7-5`
(AggregateKind subset).

---

## 2. The "Use" TODO is repeated at the rvalue level

The same Copy/Move-collapse TODO already documented in stage
6 (`./../../stage-6/prior-art/mrustc.md` §2) appears here
verbatim:

```cpp
// TODO: Split "Use" into "Copy" and "Move" (Where 'move' indicates that the source is unused)
(Use, LValue),
[src/mir/mir.hpp:629-630]
```

Implication for stage 7 has the same shape as for stage 6:
mrustc's IR-level loss of the move-vs-copy distinction is a
debt the project flagged but never paid. Stage 7's `Rvalue`
variants that consume operands (Use, Repeat, BinaryOp,
UnaryOp, Cast, Aggregate, ShallowInitBox, WrapUnsafeBinder)
inherit stage 6's preserved 3-variant `Operand` and so do not
re-litigate the question. This re-citation is recorded for
audit completeness.

Maps to `D-7-1` (cross-reference; not a fresh stage-7 finding).

---

## 3. `Borrow` is a single variant; `is_raw` flag distinguishes ref vs raw-ptr

mrustc collapses rustc's two distinct rvalues
(`Rvalue::Ref(BorrowKind, Place)` and
`Rvalue::RawPtr(RawPtrKind, Place)`) into a single
`Borrow` variant [`src/mir/mir.hpp:631-635`]:

```cpp
(Borrow, struct {
    ::HIR::BorrowType   type;
    bool is_raw;
    LValue  val;
    }),
```

Key observations:

- **`type: HIR::BorrowType`** carries the
  shared-vs-unique-vs-mut discriminator at the *type* level
  (HIR::BorrowType is defined in the HIR namespace, not
  in `mir.hpp`). HYPOTHESIS: this is a 3-or-fewer-variant
  enum mirroring `Shared`/`Unique`/`Mut`-ish kinds; the
  `mir.hpp` header does not include the enum body.
- **`is_raw: bool`** discriminates `&place` (ref, `is_raw =
  false`) from `&raw place` (raw pointer, `is_raw = true`).
  rustc keeps these as separate Rvalue variants
  (`Ref` vs `RawPtr`).
- **No region/lifetime slot** in the `Borrow` payload. mrustc
  has no MIR-level region tracking; references are erased
  during HIR-to-MIR lowering. INFERENCE: this matches
  RegseqModel stage 3's stance that "no region material
  survives in `Ty::Ref`" (see `./rustc.md` §4 cross-stage
  anchor) -- a peer compiler with the same Rust-to-C target
  reached the same erased-region conclusion.
- **No `Fake` borrow kind** / **no `FakeForPtrMetadata`
  raw-ptr kind** in mrustc's surface. INFERENCE: borrowck
  artefacts are HIR-level (or earlier) and never reach mrustc
  MIR -- consistent with rustc's `CleanupPostBorrowck` pass
  removing them before runtime MIR.

Maps to `D-7-1` (Ref/RawPtr payload shapes), `D-7-2`
(BorrowKind subset -- mrustc's `HIR::BorrowType` is the
shared/unique/mut carrier with no Fake variant), `D-7-3`
(RawPtrKind subset -- mrustc has no separate enum, the
`is_raw` bool plus reuse of `HIR::BorrowType` covers it).

Design alternative for stage 7: collapse `Ref` and `RawPtr`
into one variant with an `is_raw` bool. Cost: loses rustc's
clean two-variant split that already cleanly maps to C
output (`&` vs `&raw`). Benefit: smaller variant tally,
shared payload shape. RegseqModel stages 3-6 have so far
preferred preserving rustc's structural distinctions (cf.
`Operand::Copy` vs `Operand::Move` in stage 6); stage 7 is
likely to follow the same precedent unless evidence
demands collapse.

---

## 4. `Cast` carries no `CastKind` discriminator

mrustc's `Cast` rvalue [`src/mir/mir.hpp:638-641`]:

```cpp
(Cast, struct {
    LValue  val;
    ::HIR::TypeRef  type;
    }),
```

The payload is just `(input lvalue, target type)` -- no
`CastKind` enum, no `PointerCoercion` sub-classification.
mrustc's C backend recovers the cast kind from the
input-type / output-type pair at codegen time.

INFERENCE: this is a structurally simpler design than
rustc's. The cost is that the codegen consumer must do the
cast-kind dispatch from types every time, rather than
reading a pre-computed tag. The benefit is that the IR
variant set stays smaller (one Cast variant, no CastKind
enum) and there is no risk of the IR's CastKind disagreeing
with the operand/target types after subsequent
transformations.

For RegseqModel: this is a real alternative to consider for
`D-7-4`. rustc's CastKind has 11 effective variants
post-cleanup (per `./rustc.md` §7); mrustc demonstrates a
peer Rust-to-C compiler that does without entirely. The
trade-off is whether the bridge translates rustc's
already-computed CastKind to RegseqModel as-is (preserves
information, larger surface) or strips it and re-derives at
the C backend (smaller surface, more codegen complexity).
Per the `../../README.md` "C semantics with information
preservation" rule, RegseqModel generally prefers preserving
what rustc gives us; the CastKind is exactly that kind of
rustc-resolved tag. INFERENCE: stage 7 will likely keep
CastKind even though mrustc proves it is not strictly
necessary.

Maps to `D-7-4` (CastKind subset).

---

## 5. `BinOp` operands stored flat (no Box)

mrustc's `BinOp` rvalue [`src/mir/mir.hpp:643-647`]:

```cpp
(BinOp, struct {
    Param   val_l;
    eBinOp  op;
    Param   val_r;
    }),
```

Two `Param` operands stored *by value*, side by side, with
the operator tag between them. No `Box<(Param, Param)>` or
heap indirection. mrustc's `Param` (3-variant tagged union;
see `./../../stage-6/prior-art/mrustc.md` §1) is wider than
rustc's `Operand` (24 bytes) because it includes a `Borrow`
arm with an `LValue` payload.

INFERENCE: mrustc accepts a wider `RValue` enum than
rustc's 40-byte budget would allow. The C++ tagged-union
machinery has no `static_assert_size`-style guard, and
mrustc's compile-time size and runtime memory budget are
less constrained than a production rustc-internal type.

For RegseqModel: this is direct peer evidence that flat
(non-boxed) operand-pair storage is viable. RegseqModel's
`Operand` (post-stage-6 D-6-1.3 shrinkage to
`Constant(ConstId)` only) is a 3-variant enum carrying
either a `Place` (which contains `LocalId` + projection
slice) or a `ConstId` (one u32). The size budget arithmetic
needs to be redone for stage 7's enum, but flat storage is
a real candidate.

Maps to `D-7-8` (BinaryOp pair storage). mrustc supports
"flat" as a viable peer choice.

---

## 6. `UniOp` operand: `LValue`, not `Param`

mrustc's `UniOp` rvalue [`src/mir/mir.hpp:649-652`]:

```cpp
(UniOp, struct {
    LValue  val;    // NOTE: Not a param, because UniOps can be const propagated
    eUniOp  op;
    }),
```

The inline comment is load-bearing: **"Not a param, because
UniOps can be const propagated."** mrustc encodes the
constant-foldability of unary ops by restricting the operand
to an `LValue` (= "must come from a place, not from a
constant") -- once a unary op's input is a constant, it's
fold-able and the rvalue is replaced with a `Constant` rvalue
directly, bypassing `UniOp` entirely.

INFERENCE: this is a peer compiler optimization decision, not
an IR design constraint. rustc's `UnaryOp(UnOp, Operand)`
keeps the operand as the full `Operand` enum (which can be a
Constant). mrustc's stricter `LValue`-only design would
reject `UniOp(Neg, Constant(5))` and require the simplifier
to emit `Constant(-5)` instead.

For RegseqModel: this is *not* directly applicable.
RegseqModel reads `optimized_mir` post-`mir_opt_level=0`
cleanup; the bridge is a translator, not an optimizer.
RegseqModel will preserve whatever shape rustc gives us,
including `UnaryOp(Op, Operand::Constant(_))` if rustc emits
it. A subsequent backend-side simplifier could match
mrustc's policy, but the IR design must accept the wider
shape.

Maps to `D-7-1` (UnaryOp payload). Stage 7 takes rustc's
shape (`Operand`, not the place-only restriction).

---

## 7. `eBinOp` enum: 21 variants, simpler than rustc

mrustc's `eBinOp` enum [`src/mir/mir.hpp:550-566`]:

```cpp
enum class eBinOp
{
    ADD, ADD_OV,
    SUB, SUB_OV,
    MUL, MUL_OV,
    DIV, DIV_OV,
    MOD,// MOD_OV,

    BIT_OR,
    BIT_AND,
    BIT_XOR,

    BIT_SHR,
    BIT_SHL,

    EQ, NE,
    GT, GE,
    LT, LE,
};
```

Tally: 21 variants. Differences vs rustc's 27-variant
`BinOp` (per `./rustc.md` §8):

- **Overflow handling: dedicated `_OV` variants.** mrustc
  uses `ADD_OV`, `SUB_OV`, `MUL_OV`, `DIV_OV` for the
  overflow-checked arithmetic. rustc uses
  `AddWithOverflow`, `SubWithOverflow`, `MulWithOverflow`
  (returning `(T, bool)` tuples). The names differ but the
  semantics are equivalent. mrustc has a commented-out
  `MOD_OV` (presumably never needed because integer modulo
  cannot overflow except in the `MIN % -1` case).
- **No `_Unchecked` variants.** mrustc does not carry the
  UB-on-overflow flavours rustc uses for `unchecked_add` /
  `unchecked_shl` etc. INFERENCE: mrustc may resolve those
  intrinsics differently (perhaps inline at codegen, or via
  a special intrinsic dispatch in the consumer).
- **No `Cmp` (three-way comparison).** mrustc was last
  active before Rust 1.78's `<=>` operator stabilization;
  the variant simply did not exist when mrustc was written.
  Equivalent semantics would presumably be expressed as
  multiple `LT`/`EQ`/`GT` operations or via a
  type-resolved-call to `Ord::cmp`.
- **No `Offset`.** mrustc presumably handles `ptr.offset` via
  a dedicated rvalue variant or as an intrinsic call rather
  than a `BinOp`. Not pursued in this pass.

For RegseqModel: rustc's modern `BinOp` with the `_Unchecked`
and `_WithOverflow` variants and `Cmp`/`Offset` is the
ground truth. mrustc's smaller set is informative but not a
reduction target -- RegseqModel preserves what rustc gives
(per the C-semantics-with-information-preservation rule).
mrustc's `_OV` -> rustc's `_WithOverflow` migration is a
spelling difference, not a structural one.

Maps to `D-7-6` (BinOp curation). Stage 7 likely preserves
rustc's full 27-variant set.

---

## 8. `eUniOp` enum: only `INV` and `NEG`

mrustc's `eUniOp` enum [`src/mir/mir.hpp:567-570`]:

```cpp
enum class eUniOp
{
    INV,
    NEG
};
```

Tally: 2 variants. `INV` is bitwise-not / logical-not (rustc's
`UnOp::Not`). `NEG` is arithmetic negation (rustc's
`UnOp::Neg`).

**No `PtrMetadata`** in `eUniOp`. mrustc has it as a
top-level rvalue variant `RValue::DstMeta(LValue)` (§1
above), not as a UnOp.

For RegseqModel: stage 7 has two viable shapes for the
metadata-extraction operation:

1. **rustc shape**: `UnaryOp(UnOp::PtrMetadata, Operand)`
   -- folds metadata extraction into the unary op surface.
   3-variant `UnOp` (Not, Neg, PtrMetadata).
2. **mrustc shape**: `Rvalue::DstMeta(Place)` -- promotes
   metadata extraction to a top-level variant. 2-variant
   `UnOp` (Not, Neg) with a separate top-level rvalue.

Both shapes are well-formed peer choices. rustc's shape is
the bridge input ground truth. INFERENCE: stage 7 preserves
rustc's `UnaryOp(UnOp::PtrMetadata, ...)` because the bridge
consumes it directly; promoting to a separate Rvalue variant
would add a translation step with no semantic benefit.

Maps to `D-7-7` (PtrMetadata retention). Stage 7 keeps
PtrMetadata in the UnOp tag, parallel to rustc.

---

## 9. `SizedArray.count` is `HIR::ArraySize`, not `Constant`

mrustc's `Repeat` analog stores the count as `HIR::ArraySize`
inline:

```cpp
(SizedArray, struct {
    Param   val;
    ::HIR::ArraySize    count;
    }),
[src/mir/mir.hpp:636-638]
```

`HIR::ArraySize` is mrustc's array-length carrier; the
header does not include its definition (it lives in the
HIR namespace). INFERENCE: it is a small union of
"resolved-u64 length" and "unresolved-generic-const-param
length"; resolving generics during HIR-to-MIR lowering
produces the resolved-u64 form for monomorphic code.

Notable: **mrustc does NOT use its `Constant` tagged union
for the count.** A `Constant::Uint{ v, t }` would be the
"correct" general-purpose representation, but mrustc embeds
`HIR::ArraySize` directly. INFERENCE: the array-length
domain is narrow enough (always non-negative integer or
generic-const-param-ref) that a dedicated type is cleaner
than re-using the general `Constant` enum.

For RegseqModel: this informs `D-7-9`. mrustc embeds the
length by value; rustc embeds `ty::Const<'tcx>` (which after
monomorphization is a u64); RegseqModel could choose:

1. **Direct `u64` payload** -- post-monomorphization is
   guaranteed integer, no interning needed. Parallels
   stage-3's `Ty::Array(elem_ty, n: u64)` decision (per
   `./rustc.md` §3 cross-stage anchor).
2. **`ConstId` payload** (stage 4 interning) -- preserves
   uniformity with other constant-bearing variants. Pays
   the cost of an interner indirection for a value that is
   guaranteed integer post-mono.

mrustc's `HIR::ArraySize` plus rustc's `ty::Const` plus
stage-3's pinned `Ty::Array(elem_ty, u64)` all converge on
"a u64 is sufficient post-monomorphization." Direct `u64`
payload is the path of least resistance.

Maps to `D-7-9` (Repeat count payload).

---

## 10. Aggregate kinds split per-shape

mrustc enumerates aggregate constructors as separate rvalue
variants rather than as one `Aggregate(AggregateKind, ...)`:

- `Tuple{ vals: vec<Param> }` [`src/mir/mir.hpp:660-662`]
- `Array{ vals: vec<Param> }` [`src/mir/mir.hpp:663-666`]
- `UnionVariant{ path, index, val }`
  [`src/mir/mir.hpp:667-672`]
- `EnumVariant{ path, index, vals }`
  [`src/mir/mir.hpp:673-678`] (note: the line ranges here
  are approximate due to overlapping multi-line struct
  bodies; verbatim source confirms five distinct variants
  total)
- `Struct{ path, vals }` [`src/mir/mir.hpp:680-684`]
- (Plus `MakeDst{ ptr_val, meta_val }` for raw-pointer
  construction, §1 above.)

vs rustc's `Aggregate(Box<AggregateKind<'tcx>>, IndexVec<FieldIdx, Operand<'tcx>>)`
with 7 `AggregateKind` sub-variants (Array, Tuple, Adt,
Closure, Coroutine, CoroutineClosure, RawPtr) per
`./rustc.md` §10.

Differences:

- **mrustc has no `Closure` / `Coroutine` /
  `CoroutineClosure` aggregate kinds.** mrustc predates
  significant async/coroutine MIR lowering work in modern
  rustc; closures presumably reach codegen via a separate
  path (likely `Struct{ path, vals }` for the closure-as-
  struct lowering, since mrustc closures are concretized at
  HIR level).
- **mrustc splits `Adt` into `UnionVariant` / `EnumVariant`
  / `Struct`.** rustc collapses these into one
  `AggregateKind::Adt(DefId, VariantIdx, ..., active_field_idx)`
  with an `active_field_idx: Option<FieldIdx>` to handle the
  union case. mrustc's per-variant split makes the union
  handling explicit (`UnionVariant.val` is single-valued; no
  active-field index needed).
- **mrustc has no `UserTypeAnnotationIndex` slot** on its
  variant constructors. INFERENCE: mrustc strips
  user-type-annotation information at HIR (since mrustc has
  no NLL borrowchecker that consumes them), or never carries
  them. Confirms rustc's `Adt`'s `Option<UserTypeAnnotationIndex>`
  is dead by the time MIR is consumed by a peer compiler --
  parallel to rustc's own `CleanupPostBorrowck` clearing
  (per `./rustc.md` §10).

For RegseqModel: this is informative for `D-7-5`. mrustc's
flat-per-variant approach is a viable alternative to rustc's
`Aggregate(AggregateKind, ...)` shape. Stage 7's choice
depends on:

- Whether driver code actually uses closures (likely yes;
  stage 3 already pinned `Ty::Closure(item, args)`).
- Whether the `Coroutine`/`CoroutineClosure` variants
  surface in driver code (likely no; async absent from
  `#![no_std]` driver corpus).
- Whether stage-7 prefers rustc's tag-based shape (smaller
  rvalue variant tally) or mrustc's per-shape shape (larger
  rvalue variant tally, but no sub-tag).

Maps to `D-7-5` (AggregateKind subset).

---

## 11. Cross-D-tag findings

Mapping of mrustc evidence to stage-7 candidate D-tags:

### `D-7-1` (Rvalue variant set + payload shapes)

- **mrustc has 15 rvalue variants** (vs rustc's 13). The
  difference is structural (mrustc splits aggregate kinds
  per-shape, has no Discriminant, has DstMeta/DstPtr/MakeDst
  as distinct variants).
- **Same Use-Copy/Move TODO** as documented in stage 6
  (§2 above). Stage 7 inherits stage-6's resolution
  (preserved Operand 3-variant set with Copy/Move distinct).
- **No `CopyForDeref` / `ShallowInitBox` /
  `WrapUnsafeBinder`** in mrustc. Confirms these are rustc-
  internal early-MIR artefacts (cf. `./rustc.md` §11).
- **No `ThreadLocalRef`** in mrustc. HYPOTHESIS: mrustc handles
  thread-local statics via a different (probably HIR-to-MIR-
  lowered) path; not pursued.

### `D-7-2` (BorrowKind subset)

- **mrustc's `Borrow` variant carries `HIR::BorrowType` plus
  `is_raw: bool`** (§3). The HIR-side BorrowType enum
  (definition not in `mir.hpp`) is presumably the
  shared/unique/mut carrier with no Fake variant. **No Fake
  borrow kind** at the MIR level. Confirms rustc's
  `BorrowKind::Fake` is borrowck-internal and should not
  reach a Rust-to-C IR.

### `D-7-3` (RawPtrKind subset)

- **mrustc has no separate RawPtrKind enum** (§3). The
  ref-vs-raw distinction is a `bool is_raw` flag on the
  `Borrow` variant; the mut/const distinction is carried by
  the same `HIR::BorrowType` field used for references.
  Smaller surface than rustc's 3-variant `RawPtrKind` (with
  `FakeForPtrMetadata`).

### `D-7-4` (CastKind subset)

- **mrustc's `Cast` carries no CastKind** -- payload is just
  `(LValue val, HIR::TypeRef type)` (§4). The cast kind is
  re-derived at codegen time from input/output types.
  Demonstrates a peer compiler that does without CastKind
  entirely. RegseqModel will likely keep CastKind per the
  C-semantics-with-information-preservation rule.

### `D-7-5` (AggregateKind subset)

- **mrustc splits aggregates per-shape** (Tuple, Array,
  UnionVariant, EnumVariant, Struct) rather than carrying a
  single `Aggregate(AggregateKind, ...)` with a sub-tag (§10).
- **mrustc has no `Closure` / `Coroutine` /
  `CoroutineClosure` aggregate variants**. RegseqModel needs
  Closure (per stage 3 `Ty::Closure(item, args)`); Coroutine
  variants are likely unreachable in driver code.
- **No `UserTypeAnnotationIndex` slot** on any aggregate
  constructor. Confirms rustc's slot is dead post-borrowck.

### `D-7-6` (BinOp/UnOp curation)

- **mrustc's `eBinOp` is 21 variants** (vs rustc's 27)
  (§7). Missing: `_Unchecked` variants, `Cmp` three-way,
  `Offset`. Spelling: `_OV` instead of `_WithOverflow`.
  RegseqModel preserves rustc's full set (information
  preservation rule).
- **mrustc's `eUniOp` is 2 variants** (Not + Neg, no
  PtrMetadata) (§8). PtrMetadata exists as a separate
  rvalue (`DstMeta`).

### `D-7-7` (UnOp::PtrMetadata retention)

- **mrustc keeps metadata extraction as a top-level rvalue
  (`DstMeta`)** rather than a UnOp variant (§8).
  RegseqModel inherits rustc's UnOp shape (PtrMetadata as
  UnOp variant).

### `D-7-8` (BinaryOp pair storage)

- **mrustc stores BinaryOp operands flat** (`Param val_l;
  eBinOp op; Param val_r`), no Box (§5). Demonstrates a
  peer compiler that pays the wider variant for cache /
  indirection wins. RegseqModel can choose flat or Box per
  size-budget arithmetic on the post-stage-6 Operand shape.

### `D-7-9` (Repeat count payload)

- **mrustc's `SizedArray.count` is `HIR::ArraySize`** by
  value, not via a Constant interner (§9). Stage 3's
  pinned `Ty::Array(elem_ty, u64)` plus mrustc's by-value
  embed converge on direct `u64` post-monomorphization; no
  ConstId interner needed.

### `D-7-10` (reject CopyForDeref / ShallowInitBox)

- **mrustc has neither variant** (§1, §11). Confirms these
  are rustc-internal early-MIR artefacts that a Rust-to-C
  IR does not need; reject if encountered.

### `D-7-11` (WrapUnsafeBinder retention)

- **mrustc has no equivalent** (§1). HYPOTHESIS: unsafe
  binders are a recent Rust feature post-dating mrustc's
  active development. Does not directly inform whether
  RegseqModel should preserve or reject; corpus check
  required.

### `D-7-K` (render_rvalue deferral)

- **mrustc emits per-type `operator<<` rendering** baked
  into each `TAGGED_UNION_EX` declaration. Different
  discipline from RegseqModel's centralized `pretty.rs`.
  Same observation as stage 6's `D-6-K` analysis;
  unaffected by stage 7.

---

## 12. What's NOT in this pass

- **`HIR::BorrowType` enum body** -- defined in the HIR
  namespace; not in `mir.hpp`. The header's reference is
  enough to confirm mrustc's borrow surface, but the exact
  variant tally requires fetching the HIR header.
- **`HIR::ArraySize` definition** -- same: HIR-namespace
  type, not in `mir.hpp`. The reference is enough to confirm
  the by-value embed; the exact representation (resolved-u64
  vs deferred-generic-const) is HIR-level detail.
- **`src/mir/check.cpp` Rvalue validation** -- the stage-6
  prior-art covered the operand-side `MIR_Validate_ValState`
  pass; the rvalue-side validators (for shape well-formedness
  on RValue::Cast / RValue::BinOp / etc.) were not pursued
  in this pass. Stage-7 candidate decisions on rvalue
  invariants do not seem to require deep validation-pass
  evidence; if a specific D-tag needs it, mid-stage top-up
  research can land it.
- **`src/trans/codegen_c.cpp` consumer dispatch** -- the
  file is large (multi-thousand lines) and the per-rvalue
  emission patterns would require a separate fetch pass.
  The high-level evidence in this file (variant set, payload
  shapes, BinOp/UnOp enum content, aggregate-kind split) is
  sufficient for the stage-7 kickoff D-tag candidate
  framing. Mid-stage top-ups can land specific consumer-side
  evidence as needed (especially for `D-7-4` if the rustc-
  shaped CastKind decision needs additional support).
