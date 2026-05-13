# Stage 7 -- Prior Art: rustc primary source

Primary-source rustc evidence for `mir::Rvalue<'tcx>` and its
consumer pipeline, scoped to what stage 7's candidate decision
namespace (`D-7-1` Rvalue variant set / per-variant payload
shapes; `D-7-2` BorrowKind subset; `D-7-3` RawPtrKind subset;
`D-7-4` CastKind subset; `D-7-5` AggregateKind subset; `D-7-6`
BinOp/UnOp curation; `D-7-7` `UnOp::PtrMetadata` retention;
`D-7-8` `BinaryOp` operand-pair storage; `D-7-9` `Repeat` count
payload; `D-7-10` reject `CopyForDeref` / `ShallowInitBox`;
`D-7-11` `WrapUnsafeBinder` retention; `D-7-K` `render_rvalue`
deferral) needs. Fresh pass per `../../README.md` §2 methodology
rule (do not reuse stage-0 summaries).

All paths below are relative to the pinned-toolchain root:

```
~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/
```

(See `../../rustc-references.md` for the canonical statement
of this root path.)

Every factual claim cites a `[file:line]` in that pinned
source. Hypotheses are flagged. Inferences from one site to
another are labelled INFERENCE.

---

## 1. `Rvalue<'tcx>` enum shape

`Rvalue<'tcx>` is a 13-variant closed enum
[`rustc_middle/src/mir/syntax.rs:1369-1482`]:

```rust
#[derive(Clone, TyEncodable, TyDecodable, Hash, HashStable,
         PartialEq, TypeFoldable, TypeVisitable)]
pub enum Rvalue<'tcx> {
    Use(Operand<'tcx>),
    Repeat(Operand<'tcx>, ty::Const<'tcx>),
    Ref(Region<'tcx>, BorrowKind, Place<'tcx>),
    ThreadLocalRef(DefId),
    RawPtr(RawPtrKind, Place<'tcx>),
    Cast(CastKind, Operand<'tcx>, Ty<'tcx>),
    BinaryOp(BinOp, Box<(Operand<'tcx>, Operand<'tcx>)>),
    UnaryOp(UnOp, Operand<'tcx>),
    Discriminant(Place<'tcx>),
    Aggregate(Box<AggregateKind<'tcx>>,
              IndexVec<FieldIdx, Operand<'tcx>>),
    ShallowInitBox(Operand<'tcx>, Ty<'tcx>),
    CopyForDeref(Place<'tcx>),
    WrapUnsafeBinder(Operand<'tcx>, Ty<'tcx>),
}
```

Key facts:

- **13 variants, closed.** No `Other` / `Opaque` arm.
- **No `Len` variant** in this nightly. (Compare
  `rustc_public/src/mir/body.rs:555` which still carries
  `Len(Place)`; that crate has not been updated to track
  rustc_middle's removal -- see §11 below.)
- **No `NullaryOp` variant** in this nightly. (Compare same
  drift in `rustc_public`.)
- **`AddressOf` was renamed to `RawPtr`.** rustc_public still
  spells the variant `AddressOf` (see §11 below). The
  rustc_middle name `RawPtr(RawPtrKind, Place)` is the
  current truth.
- **`RuntimeChecks` is NOT an `Rvalue` variant.** It is an
  `Operand` variant (`Operand::RuntimeChecks(RuntimeChecks)`,
  syntax.rs:1340) and is bridge-side lowered to
  `Operand::Constant(ConstId)` per stage-6 D-6-1.2. Stage 7
  inherits stage 6's surface: by the time stage-7 Rvalue
  variants reach the bridge consumer, all operand positions
  carry the closed 3-variant `Operand` (`Copy(Place)`,
  `Move(Place)`, `Constant(ConstId)`).
- **`Box` indirection appears at three sites:** `BinaryOp`
  (`Box<(Operand, Operand)>`), `Aggregate`
  (`Box<AggregateKind>`). rustc's size-asserts pin the enum at
  40 bytes [`syntax.rs:1752`]:

  ```rust
  static_assert_size!(Rvalue<'_>, 40);
  ```

  The Box on `BinaryOp` keeps the variant from inflating the
  enum to ~64 bytes (`Operand` is 24 bytes, syntax.rs:1749).
  The Box on `Aggregate` keeps the variant from inflating it
  beyond 32 bytes (AggregateKind is 32 bytes, syntax.rs:1748).

Maps to `D-7-1` (variant set + payload shapes).

---

## 2. `Use(Operand)`

Yields the operand unchanged [`syntax.rs:1370-1371`]:

```rust
/// Yields the operand unchanged
Use(Operand<'tcx>),
```

Codegen: `mir::Rvalue::Use(ref operand) =>
self.codegen_operand(bx, operand)` [`mir/rvalue.rs:653`]. In
the place-style codegen path
[`mir/rvalue.rs:26-46`], `Use` of a constant operand triggers
an "all-bytes-uninit" fast path that returns without storing.

Use-site C-shape: a value-load of the operand. No structural
distinction from a bare `Operand`.

Maps to `D-7-1` payload shape (trivial: `Operand`). No
"unbox" or restructuring decision -- direct embed.

---

## 3. `Repeat(Operand, ty::Const)`

Creates an array `[x; N]` [`syntax.rs:1373-1376`]:

```rust
/// Creates an array where each element is the value of the operand.
Repeat(Operand<'tcx>, ty::Const<'tcx>),
```

The count is a `ty::Const<'tcx>` (the "type-const" form, used
for generic array lengths and the like) -- not a
`mir::Const<'tcx>`. After monomorphization, the count is a
concrete `u64`, but the carrier type remains `ty::Const`.

Codegen path: `Repeat` for non-ZST elements is handled in the
place-style path [`mir/rvalue.rs:104-167`]; ZST arrays fall
through to the operand-style path [`mir/rvalue.rs:654-668`].
The non-ZST path materialises a per-element loop.

Cross-stage anchor: stage 4's `Const` enum closes 11
variants at top-level §9 with `ConstId` interning via
`ConstInterner`. Stage 3's `Ty::Array(elem_ty, n)` already
canonicalises the length as a post-mono `u64`
[`spec.md:4525` translation table for `Array(t, n)`]. The
`Repeat` count is the same kind of value -- post-mono `u64`
-- so a direct `u64` payload is well-typed for runtime MIR.

Maps to `D-7-1` payload (Repeat) + `D-7-9` (count payload:
`u64` direct vs `ConstId` interned).

---

## 4. `Ref(Region, BorrowKind, Place)`

Creates a reference of the indicated kind to the place
[`syntax.rs:1378-1385`]:

```rust
Ref(Region<'tcx>, BorrowKind, Place<'tcx>),
```

`BorrowKind` is a 3-variant enum [`syntax.rs:155-182`]:

```rust
pub enum BorrowKind {
    Shared,
    Fake(FakeBorrowKind),
    Mut { kind: MutBorrowKind },
}
```

`MutBorrowKind` has 3 sub-variants [`syntax.rs:202-248`]:
`Default`, `TwoPhaseBorrow`, `ClosureCapture`.

**Critical evidence: `BorrowKind::Fake` is gone from
optimized_mir.** The `CleanupPostBorrowck` pass
[`rustc_mir_transform/src/cleanup_post_borrowck.rs:33-43`]
NOPs every `Rvalue::Ref(_, BorrowKind::Fake(_), _)`
assignment statement. The pass reports
`is_required(): true` [`cleanup_post_borrowck.rs:88-90`],
meaning it always runs (independent of opt-level). It moves
the body to `MirPhase::Analysis(AnalysisPhase::PostCleanup)`
[`rustc_mir_transform/src/lib.rs:621,631`]. By the time we
read `optimized_mir`, every Fake borrow is a Nop statement
(no Rvalue at all) -- so `BorrowKind::Fake` cannot reach the
bridge in a well-formed body.

**Region: per stage-3, fully erased in `Ty::Ref(t,
SharedBorrow|UniqueBorrow)`.** Stage 3 closed the type-side
discriminator at `SharedBorrow`/`UniqueBorrow` and the
top-level §17 spec records "no region material survives in
`Ty::Ref` for any type that enters the IR" [`spec.md:4846`].
INFERENCE: the Rvalue-level `Region<'tcx>` is similarly not
load-bearing for stage 7's purposes -- the bridge can drop
it. Codegen confirms by pattern-matching `(_, bk, place)`
and ignoring the region [`mir/rvalue.rs:506-511`]:

```rust
mir::Rvalue::Ref(_, bk, place) => {
    let mk_ref = move |tcx: TyCtxt<'tcx>, ty: Ty<'tcx>| {
        Ty::new_ref(tcx, tcx.lifetimes.re_erased, ty,
                    bk.to_mutbl_lossy())
    };
    self.codegen_place_to_pointer(bx, place, mk_ref)
}
```

`tcx.lifetimes.re_erased` is the erased-region constant --
codegen synthesises the type with an erased region rather
than reading the rvalue's. Confirms the bridge can drop the
rvalue's `Region` slot without semantic loss.

**`MutBorrowKind::TwoPhaseBorrow`**: documented as arising
from method-call auto-ref (`Adjust::Borrow`). Survives into
optimized_mir but is operationally identical to
`MutBorrowKind::Default` for codegen
[`mir/rvalue.rs:506-511` -- `bk.to_mutbl_lossy()` collapses
all `Mut { kind: ... }` to `Mutability::Mut`].

**`MutBorrowKind::ClosureCapture`**: closure-only artefact
[`syntax.rs:202-248`]. INFERENCE: appears only in closure
bodies, which RegseqModel's `#![no_std]` driver corpus may
not exercise.

Maps to `D-7-1` (Ref payload) + `D-7-2` (BorrowKind subset).
Strong evidence supports a 2-variant subset
(`SharedBorrow` / `UniqueBorrow`) parallel to stage 3's
`Ty::Ref` discriminator, dropping the `Mut { kind: ... }`
sub-discriminator and the always-rejected `Fake`.

---

## 5. `ThreadLocalRef(DefId)`

Creates a pointer/reference to a thread local
[`syntax.rs:1387-1398`]:

```rust
/// The yielded type is a `*mut T` if the static is mutable,
/// otherwise if the static is extern a `*const T`, and if
/// neither of those apply a `&T`.
ThreadLocalRef(DefId),
```

Codegen [`mir/rvalue.rs:622-652`] either calls a
`ThreadLocalShim` (for non-local statics needing the shim) or
directly emits `bx.get_static(def_id)`.

Cross-stage anchor: thread-locals are referenced through
`InstanceKind::ThreadLocalShim(DefId)` per §9 (Instance and
InstanceKind). The bridge maps the rvalue's bare `DefId` to
RegseqModel's `InstanceId` via the same path stage 9
terminator-side `Call::func` resolution will use.

Driver-relevance question: `#![no_std]` Rust generally
disallows `#[thread_local]` (no thread-local storage in
embedded targets without OS support). HYPOTHESIS: the
RegseqModel target corpus does not exercise this variant.
Stage 7 must still spec it (to maintain closed-vocabulary
discipline) but may flag it as "rejected with a 'no TLS in
driver code' diagnostic" rather than carry the variant
through to the C backend.

Maps to `D-7-1` (ThreadLocalRef payload + reachability).

---

## 6. `RawPtr(RawPtrKind, Place)`

Creates a raw pointer to a place [`syntax.rs:1400-1407`]:

```rust
/// This is generated by pointer casts like `&v as *const _`
/// or raw borrow expressions like `&raw const v`.
RawPtr(RawPtrKind, Place<'tcx>),
```

`RawPtrKind` is a 3-variant enum [`syntax.rs:186-198`]:

```rust
pub enum RawPtrKind {
    Mut,
    Const,
    /// Creates a raw pointer to a place that will only be
    /// used to access its metadata, not the data behind the
    /// pointer.
    FakeForPtrMetadata,
}
```

`FakeForPtrMetadata` is documented as "limitation is *not*
enforced by the validator" [`syntax.rs:191-192`] and
operationally equivalent to `RawPtrKind::Mut` modulo retag
behaviour [`syntax.rs:194-197`]. INFERENCE: this is a
borrowck-internal annotation that may not survive into
runtime MIR; needs verification (no `CleanupPostBorrowck`-
style explicit removal seen for `FakeForPtrMetadata`, so it
may persist).

Maps to `D-7-1` (RawPtr payload) + `D-7-3` (RawPtrKind
subset). Likely 2-variant subset (`Mut` / `Const`) with
rejection of `FakeForPtrMetadata` if encountered (parallel
to stage-7's general "no flag-style markers" treatment).

---

## 7. `Cast(CastKind, Operand, Ty)`

Performs essentially all `as`-casts [`syntax.rs:1409-1414`].

`CastKind` is a 9-variant enum [`syntax.rs:1485-1526`]:

```rust
pub enum CastKind {
    PointerExposeProvenance,
    PointerWithExposedProvenance,
    PointerCoercion(PointerCoercion, CoercionSource),
    IntToInt,
    FloatToInt,
    FloatToFloat,
    IntToFloat,
    PtrToPtr,
    FnPtrToPtr,
    Transmute,
    Subtype,
}
```

`PointerCoercion` is a 6-variant enum
[`rustc_middle/src/ty/adjustment.rs:11-40`]:

```rust
pub enum PointerCoercion {
    ReifyFnPointer(hir::Safety),
    UnsafeFnPointer,
    ClosureFnPointer(hir::Safety),
    MutToConstPointer,
    ArrayToPointer,
    Unsize,
}
```

**Critical evidence: `PointerCoercion::ArrayToPointer` and
`MutToConstPointer` are gone from optimized_mir.** The same
`CleanupPostBorrowck` pass that NOPs `Fake` borrows
[`cleanup_post_borrowck.rs:44-60`] rewrites
`CastKind::PointerCoercion(ArrayToPointer | MutToConstPointer,
_)` casts to `CastKind::PtrToPtr`:

```rust
StatementKind::Assign(box (
    _,
    Rvalue::Cast(
        ref mut cast_kind @ CastKind::PointerCoercion(
            PointerCoercion::ArrayToPointer
            | PointerCoercion::MutToConstPointer,
            _,
        ),
        ..,
    ),
)) => {
    *cast_kind = CastKind::PtrToPtr;
}
```

The codegen consumer also rejects them with a `bug!`
[`mir/rvalue.rs:448-452`]:

```rust
mir::CastKind::PointerCoercion(
    PointerCoercion::MutToConstPointer
    | PointerCoercion::ArrayToPointer, _
) => {
    bug!("{kind:?} is for borrowck, and should never appear
          in codegen");
}
```

So both are double-confirmed gone.

**`CastKind::Subtype`** is *added* during runtime lowering by
the `Subtyper` pass [`rustc_mir_transform/src/lib.rs:644`].
It survives into optimized_mir.

`CoercionSource` is a 2-variant diagnostic-only enum
[`syntax.rs:1531-1536`] (`AsCast` vs `Implicit`). Codegen
ignores it [`mir/rvalue.rs:408-447`]; the spec can drop it.

Maps to `D-7-1` (Cast payload) + `D-7-4` (CastKind subset).
A reasonable subset post-cleanup is 9 effective `CastKind`
variants:
1. `PointerExposeProvenance`
2. `PointerWithExposedProvenance`
3. `PointerCoercion(ReifyFnPointer | UnsafeFnPointer |
   ClosureFnPointer | Unsize)` -- 4 sub-variants
4. `IntToInt`
5. `FloatToInt`
6. `FloatToFloat`
7. `IntToFloat`
8. `PtrToPtr`
9. `FnPtrToPtr`
10. `Transmute`
11. `Subtype`

Drop `CoercionSource` (diagnostic-only). Drop
`PointerCoercion::ArrayToPointer / MutToConstPointer`
(rewritten by required pass).

---

## 8. `BinaryOp(BinOp, Box<(Operand, Operand)>)`

Per-spec evidence: `BinOp` is a 27-variant enum
[`syntax.rs:1615-1703`]:

```rust
pub enum BinOp {
    Add, AddUnchecked, AddWithOverflow,
    Sub, SubUnchecked, SubWithOverflow,
    Mul, MulUnchecked, MulWithOverflow,
    Div, Rem,
    BitXor, BitAnd, BitOr,
    Shl, ShlUnchecked,
    Shr, ShrUnchecked,
    Eq, Lt, Le, Ne, Ge, Gt,
    Cmp,    // <=> three-way comparison (Ordering)
    Offset,
}
```

Operands are stored boxed: `Box<(Operand<'tcx>,
Operand<'tcx>)>` [`syntax.rs:1430`]. Each `Operand` is 24
bytes [`syntax.rs:1749`]; an unboxed pair would dominate the
40-byte size budget.

`BinOp::Cmp` returns `Ordering` (`-1_i8 / 0 / +1_i8` as
scalar) [`syntax.rs:1691-1700`].

`BinOp::Offset` is `ptr::offset` semantics
[`syntax.rs:1701-1702`].

`AddWithOverflow / SubWithOverflow / MulWithOverflow` return
`(T, bool)` -- the `bool` flags overflow
[`syntax.rs:1426-1428`]. These are emitted by overflow-check
lowering and survive into optimized_mir.

`AddUnchecked / SubUnchecked / MulUnchecked / ShlUnchecked /
ShrUnchecked` are the UB-on-overflow flavours
[`syntax.rs:1619-1633,1665-1678`]. These are emitted by
intrinsics like `unchecked_add`.

Maps to `D-7-1` (BinaryOp payload) + `D-7-6` (BinOp
curation) + `D-7-8` (operand-pair storage). Stage 7's spec
should preserve the 27-variant `BinOp` (no curation; each
maps directly to a C lowering or a runtime helper) and the
storage choice (Box vs flat) is a size-budget decision
specific to RegseqModel's `Operand` size after stage-6's
`ConstId`-based shrinkage.

---

## 9. `UnaryOp(UnOp, Operand)`

`UnOp` is a 3-variant enum [`syntax.rs:1597-1611`]:

```rust
pub enum UnOp {
    Not,
    Neg,
    PtrMetadata,
}
```

`UnOp::PtrMetadata`: "Allowed only in `MirPhase::Runtime`;
earlier it's an intrinsic." [`syntax.rs:1609`].
`optimized_mir` is `MirPhase::Runtime(RuntimePhase::PostCleanup)`
[`rustc_mir_transform/src/lib.rs:612`], so PtrMetadata is
allowed.

Codegen [`mir/rvalue.rs:592-601`]: extracts the metadata
field of a pointer/reference (returns `()` for thin
pointers).

Maps to `D-7-1` (UnaryOp payload) + `D-7-7` (PtrMetadata
retention). PtrMetadata is non-controversial -- preserved
verbatim, since it has a direct C lowering through the
data-pointer/metadata struct shape that stage-3 `Ty::Ref` /
`Ty::Ptr` already canonicalises.

---

## 10. `Discriminant(Place)` and `Aggregate(AggregateKind, ...)`

**`Discriminant(Place)`** [`syntax.rs:1439-1449`]: reads the
discriminant of an enum place, returning an integer of type
`discriminant_ty` (zero for non-enum types). Codegen
[`mir/rvalue.rs:610-620`] reads the place and computes the
discriminant integer.

`AggregateKind` is a 7-variant enum
[`syntax.rs:1538-1570`]:

```rust
pub enum AggregateKind<'tcx> {
    Array(Ty<'tcx>),
    Tuple,
    Adt(DefId, VariantIdx, GenericArgsRef<'tcx>,
        Option<UserTypeAnnotationIndex>, Option<FieldIdx>),
    Closure(DefId, GenericArgsRef<'tcx>),
    Coroutine(DefId, GenericArgsRef<'tcx>),
    CoroutineClosure(DefId, GenericArgsRef<'tcx>),
    RawPtr(Ty<'tcx>, Mutability),
}
```

The `Aggregate` rvalue carries an `IndexVec<FieldIdx,
Operand<'tcx>>` of operands [`syntax.rs:1459`].

**`Adt`'s `Option<UserTypeAnnotationIndex>`** is an NLL/borrowck
slot. INFERENCE: cleared by `CleanupPostBorrowck`
[`cleanup_post_borrowck.rs:81-85` -- `body.user_type_annotations.raw.clear()`,
`decl.user_ty = None`]. Slot survives but always `None`
post-cleanup; spec can drop it.

**`Adt`'s `Option<FieldIdx>`**: union active-field marker
[`syntax.rs:1547-1550`]. Required for unions; preserved in
optimized_mir.

**Aggregate phase qualification**: "Disallowed after
deaggregation for all aggregate kinds except `Array` and
`Coroutine`. After coroutine lowering, `Coroutine` aggregate
kinds are disallowed too." [`syntax.rs:1457-1458`]. So in
optimized_mir (post-coroutine-lowering), only `Array`
survives... but this is "after deaggregation", a separate
pass. The pass isn't in `run_runtime_lowering_passes` or
`run_runtime_cleanup_passes` [`rustc_mir_transform/src/lib.rs:637-687`],
so HYPOTHESIS: deaggregation does not run at
`mir_opt_level=0`, and all 7 `AggregateKind` variants can
survive. Verify before locking `D-7-5`.

Coroutine variants (`Closure`, `Coroutine`,
`CoroutineClosure`): driver-relevance question parallel to
ThreadLocalRef. `#![no_std]` driver code can use closures
(`Fn(...)` types compile fine without std), so `Closure` is
likely reachable. `Coroutine` / `CoroutineClosure` require
`async` machinery, generally absent from drivers.

Maps to `D-7-1` (Discriminant + Aggregate payloads) +
`D-7-5` (AggregateKind subset). Strong candidate to drop
`UserTypeAnnotationIndex` slot from `Adt`.

---

## 11. `ShallowInitBox` / `CopyForDeref` / `WrapUnsafeBinder`

**`ShallowInitBox(Operand, Ty)`** [`syntax.rs:1461-1466`]:
"Transmutes a `*mut u8` into shallow-initialized `Box<T>`."
Codegen rejects:

```rust
mir::Rvalue::ShallowInitBox(..) =>
    bug!("`ShallowInitBox` in codegen"),
[mir/rvalue.rs:713]
```

The `ElaborateBoxDerefs` pass runs in
`run_runtime_lowering_passes` [`lib.rs:659`] before
optimized_mir; INFERENCE: it eliminates `ShallowInitBox`
entirely.

**`CopyForDeref(Place)`** [`syntax.rs:1468-1478`]:
"Disallowed in runtime MIR and is replaced by normal
copies." Codegen rejects:

```rust
mir::Rvalue::CopyForDeref(_) =>
    bug!("`CopyForDeref` in codegen"),
[mir/rvalue.rs:712]
```

The `EraseDerefTemps` pass runs in
`run_runtime_lowering_passes` [`lib.rs:658`]; it eliminates
`CopyForDeref`.

Both: not reachable in well-formed optimized_mir bodies.
Bridge should reject if encountered (treat as an
`unsupported_construct!` -- consistent with the §17 rejection
strategy elsewhere in the spec).

**`WrapUnsafeBinder(Operand, Ty)`** [`syntax.rs:1480-1481`]:
recent feature (unsafe binders RFC). Codegen
[`mir/rvalue.rs:706-711`] passes operand value through
unchanged with the binder type as the layout. Driver-
relevance HYPOTHESIS: not exercised in current Linux/Zephyr
driver corpus; needs corpus check before locking.

Maps to `D-7-10` (reject `CopyForDeref` and `ShallowInitBox`)
+ `D-7-11` (WrapUnsafeBinder retention).

---

## 12. `rustc_public` Rvalue (peer evidence, drift)

`rustc_public/src/mir/body.rs:493-599` defines an owned
analog of `Rvalue` exposed by the rustc-public crate:

```rust
pub enum Rvalue {
    AddressOf(RawPtrKind, Place),
    Aggregate(AggregateKind, Vec<Operand>),
    BinaryOp(BinOp, Operand, Operand),
    Cast(CastKind, Operand, Ty),
    CheckedBinaryOp(BinOp, Operand, Operand),
    CopyForDeref(Place),
    Discriminant(Place),
    Len(Place),
    Ref(Region, BorrowKind, Place),
    Repeat(Operand, TyConst),
    ShallowInitBox(Operand, Ty),
    ThreadLocalRef(crate::CrateItem),
    UnaryOp(UnOp, Operand),
    Use(Operand),
}
```

Notable drift from rustc_middle (this nightly):

- **Still has `Len(Place)`** -- rustc_middle removed it.
- **Still has `AddressOf`** (renamed to `RawPtr` in
  rustc_middle).
- **Still has `CheckedBinaryOp`** (merged into `BinaryOp`
  with `BinOp::AddWithOverflow / SubWithOverflow /
  MulWithOverflow` in rustc_middle).
- **No `WrapUnsafeBinder`** (post-dates rustc_public's last
  refresh).
- **`ThreadLocalRef`** carries `CrateItem` (an opaque
  rustc_public ID) rather than `DefId`.
- **`BinaryOp(BinOp, Operand, Operand)`** stores the operands
  flat (not boxed). rustc_public's `Operand` (24 bytes
  estimated) and the absence of a size assert mean
  rustc_public can afford the wider variant.
- **`Aggregate(AggregateKind, Vec<Operand>)`** stores
  operands in a `Vec`, not an `IndexVec<FieldIdx, _>`.

This drift confirms two things:

1. **The owned-IR design space tolerates flatter shapes than
   rustc_middle.** rustc_middle's boxing is driven by its
   `static_assert_size!(Rvalue<'_>, 40)` budget; a
   downstream IR is free to pick a different size budget.
   This is the same evidence-shape that supported stage-6
   D-6-1.3's `Box<ConstOperand>` -> `ConstId` shrinkage.
2. **rustc_public is NOT a current-truth peer.** The
   stage-7 spec consumes `mir::Rvalue<'tcx>` from
   rustc_middle (per §15 bridge translation rules) -- not
   rustc_public's narrowed surface. Citations in stage 7's
   prior-art that point at rustc_middle take precedence
   over rustc_public when they conflict.

Maps to `D-7-8` (BinaryOp pair storage) + `D-7-1` (Aggregate
operand storage shape).

---

## 13. Cleanup-pass whitelist context

The pinned RegseqModel cleanup-pass whitelist (per top-level
`../../README.md`):

```
- RemoveStorageMarkers
- RemoveZsts
- SimplifyLocals
- ReorderBasicBlocks
- ReorderLocals
- CheckAlignment
- CheckNull
- CheckEnums
```

None of these alter `Rvalue` shape directly. The
load-bearing cleanup that affects Rvalue (`CleanupPostBorrowck`)
runs unconditionally at `is_required(): true` BEFORE the
opt-level-gated whitelist. So the bridge sees:

- `BorrowKind::Fake` -- gone (NOPed).
- `CastKind::PointerCoercion(ArrayToPointer | MutToConstPointer)`
  -- gone (rewritten to `PtrToPtr`).
- `Rvalue::CopyForDeref` -- gone (eliminated by
  `EraseDerefTemps` in runtime lowering).
- `Rvalue::ShallowInitBox` -- gone (eliminated by
  `ElaborateBoxDerefs` in runtime lowering).

These four are bridge-rejection candidates at the §17.x
"unsupported construct" sites -- they cannot reach a
well-formed optimized_mir.

Maps to `D-7-2`, `D-7-4`, `D-7-10`.

---

## 14. Codegen consumer pattern

The canonical Rvalue consumer is `codegen_rvalue` /
`codegen_rvalue_operand` in `mir/rvalue.rs`:

- `pub(crate) fn codegen_rvalue(&mut self, bx, dest, rvalue)`
  [`mir/rvalue.rs:19-388`]: writes the rvalue's value into a
  destination place. The "place-style" path. Used for
  rvalues whose result naturally lives in memory (large
  arrays, aggregates).
- `pub(crate) fn codegen_rvalue_operand(&mut self, bx,
  rvalue) -> OperandRef` [`mir/rvalue.rs:389-715`]: produces
  a single operand value. The "value-style" path. Used for
  scalar / scalar-pair rvalues.

Some variants appear in both (`Use`, `Repeat`, `Aggregate`):
the place-style path defers to the operand path for
ZST-or-immediate cases, and vice versa. The split is an
optimization, not a semantic distinction.

INFERENCE: the RegseqModel C backend can fold both paths
into a single `lower_rvalue(dest_or_temp, rvalue) -> CExpr`
function, since C doesn't have rustc's abstract-builder /
SSA-value distinction. The variant set is what stage 7 fixes;
the lowering split is a stage-14 backend decision.

---

## 15. Summary mapping to `D-7-K` candidate decisions

| Tag | Decision | Evidence anchor |
|-|-|-|
| `D-7-1` | Rvalue variant set + payload shapes | §1, §2-§11 |
| `D-7-2` | BorrowKind subset (drop Fake, drop MutBorrowKind sub-discriminator) | §4 + cleanup_post_borrowck.rs |
| `D-7-3` | RawPtrKind subset (drop FakeForPtrMetadata) | §6 |
| `D-7-4` | CastKind subset (drop ArrayToPointer / MutToConstPointer; drop CoercionSource) | §7 + cleanup_post_borrowck.rs |
| `D-7-5` | AggregateKind subset (Coroutine reachability; drop UserTypeAnnotationIndex) | §10 |
| `D-7-6` | BinOp/UnOp curation (preserve all 27 BinOp + 3 UnOp) | §8, §9 |
| `D-7-7` | UnOp::PtrMetadata retention | §9 |
| `D-7-8` | BinaryOp pair storage (Box vs flat) | §8 + §12 |
| `D-7-9` | Repeat count payload (u64 direct vs ConstId) | §3 |
| `D-7-10` | Reject CopyForDeref / ShallowInitBox | §11 + §13 |
| `D-7-11` | WrapUnsafeBinder retention (corpus check) | §11 |
| `D-7-K` | render_rvalue defer to stage 11 (D-5-K / D-6-K precedent) | (see ../memory.md §0) |

This rustc.md is the kickoff evidence pass; mid-stage
deeper reads on specific D-tags will land additional
citations (e.g., for `D-7-5` Coroutine reachability under
`mir_opt_level=0`, for `D-7-6` size-budget arithmetic with
the post-stage-6 `Operand` shape).
