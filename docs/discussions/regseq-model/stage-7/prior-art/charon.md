# Stage 7 -- Prior Art: Charon primary source

Primary-source Charon (LLBC / ULLBC) evidence for `Rvalue`,
its sub-enums (`BorrowKind`, `CastKind`, `BinOp`, `UnOp`,
`NullOp`, `AggregateKind`), and the rustc-MIR -> Charon
translation behaviour governing the post-MIR shape of
rvalues. Scoped to what stage 7's candidate decision
namespace (D-7-1 variant set / payload shapes; D-7-2
BorrowKind subset; D-7-3 RawPtrKind subset; D-7-4 CastKind
subset; D-7-5 AggregateKind subset; D-7-6 BinOp/UnOp
curation; D-7-7 PtrMetadata retention; D-7-8 BinaryOp
operand-pair storage; D-7-9 Repeat count payload; D-7-10
reject CopyForDeref / ShallowInitBox; D-7-11 WrapUnsafeBinder
retention; D-7-K render_rvalue deferral) needs. Fresh pass
per `../../README.md` §2 methodology rule (do not reuse
stage-0 summaries).

Charon is INRIA's Rust front-end translator that feeds the
Aeneas verification pipeline by lowering rustc MIR (via the
`hax` extraction layer) into LLBC ("Low-Level Borrow
Calculus") and the more MIR-shaped intermediate ULLBC
("Unstructured LLBC"). LLBC is the headline IR named in the
ICFP 2022 paper "Aeneas: Rust Verification by Functional
Translation" (Ho & Protzenko, arxiv 2206.07185); ULLBC is
the basic-block-shaped predecessor produced before
control-flow reconstruction.

Repository: `https://github.com/AeneasVerif/charon` (main
branch).

Primary files inspected (web-fetched verbatim):

- `charon/src/ast/expressions.rs` -- IR type definitions
  (`Place`, `Operand`, `Rvalue`, `BorrowKind`, `CastKind`,
  `BinOp`, `UnOp`, `NullOp`, `AggregateKind`,
  `UnsizingMetadata`, `OverflowMode`).
- `charon/src/bin/charon-driver/translate/translate_bodies.rs`
  -- rustc MIR -> ULLBC translation, including the per-
  variant Rvalue/Cast/BorrowKind dispatch and the rejection
  sites for unsupported constructs.
- `charon/src/transform/simplify_output/simplify_constants.rs`
  -- micro-pass that normalises constant operand payloads.
- `charon/src/transform/simplify_output/ops_to_function_calls.rs`
  -- micro-pass that desugars some unops/binops/repeats to
  function calls.

Every factual claim cites a `[file:line]` in the Charon
source tree at `main`. Hypotheses are flagged. Inferences
across two sites are labelled INFERENCE.

---

## 1. `Rvalue` enum shape

`Rvalue` is an 11-variant closed enum
[`charon/src/ast/expressions.rs:702-771`]:

```rust
pub enum Rvalue {
    Use(Operand),
    #[charon::rename("RvRef")]
    Ref {
        place: Place,
        kind: BorrowKind,
        ptr_metadata: Operand,
    },
    RawPtr {
        place: Place,
        kind: RefKind,
        ptr_metadata: Operand,
    },
    BinaryOp(BinOp, Operand, Operand),
    UnaryOp(UnOp, Operand),
    NullaryOp(NullOp, Ty),
    Discriminant(Place),
    Aggregate(AggregateKind, Vec<Operand>),
    Len(Place, Ty, Option<Box<ConstantExpr>>),
    Repeat(Operand, Ty, Box<ConstantExpr>),
    ShallowInitBox(Operand, Ty),
}
```

Notable contrasts with rustc_middle's 13-variant `Rvalue<'tcx>`:

- **No standalone `Cast` variant.** Charon folds `Cast` into
  `UnOp::Cast(CastKind)`
  [`expressions.rs:179-181`]; the translator at
  [`translate_bodies.rs:766-767`] wraps the cast operand:

  ```rust
  let unop = UnOp::Cast(cast_kind);
  Ok(Rvalue::UnaryOp(unop, operand))
  ```

  HYPOTHESIS: this normalises one-operand operations into a
  single rvalue family, simplifying Aeneas's symbolic
  semantics. RegseqModel's stage 7 has not adopted this
  collapse -- rustc separates `Cast` and `UnaryOp`, and
  RegseqModel mirrors rustc on principle ("store rustc's
  resolved form").
- **No `ThreadLocalRef` variant.** Translation rejects:
  ```rust
  mir::Rvalue::ThreadLocalRef(_) => {
      raise_error!(self, span,
          "charon does not support thread local references");
  }
  ```
  [`translate_bodies.rs:903-909`]. Hard rejection at the
  bridge boundary.
- **No `CopyForDeref` variant.** Translation lowers to
  `Use(Copy(place))`:
  ```rust
  mir::Rvalue::CopyForDeref(place) => {
      let place = self.translate_place(span, place)?;
      Ok(Rvalue::Use(Operand::Copy(place)))
  }
  ```
  [`translate_bodies.rs:599-604`]. Direct precedent for an
  alternative D-7-10 strategy (lower-rather-than-reject).
- **No `WrapUnsafeBinder` variant.** Rejected:
  ```rust
  mir::Rvalue::WrapUnsafeBinder { .. } => {
      raise_error!(self, span,
          "charon does not support unsafe lifetime binders");
  }
  ```
  [`translate_bodies.rs:910-915`]. Direct precedent for
  D-7-11 reject-on-encounter.
- **`NullaryOp` survives at the rvalue level** -- this is
  the same variant Charon uses to absorb rustc's
  `Operand::RuntimeChecks` (per stage-6 charon.md §4) and
  also the natural home for `SizeOf` / `AlignOf` /
  `OffsetOf`. RegseqModel's stage 7 cannot follow this
  pattern: rustc_middle's pinned-nightly `Rvalue` has no
  `NullaryOp` variant at all; the bridge sees neither.
- **`Len(Place, Ty, Option<Box<ConstantExpr>>)`** -- Charon
  keeps it for slice-pattern matching (per the doc comment).
  rustc_middle's pinned-nightly `Rvalue` removed `Len`.
  RegseqModel cannot ship a variant the bridge will never
  see; treat as informational only.
- **`Ref` and `RawPtr` are struct-shaped variants**, not
  tuple variants. Each carries an inline `place: Place`,
  the kind discriminator, and an explicit
  `ptr_metadata: Operand`. The metadata operand is the
  metadata-init half of a wide pointer; thin pointers carry
  unit. Translation initially fills it with an "Opaque"
  placeholder
  [`translate_bodies.rs:619-622, 636-639`]:
  ```rust
  ptr_metadata: Operand::Const(Box::new(ConstantExpr {
      kind: ConstantExprKind::Opaque("Missing metadata".to_string()),
      ty: Ty::mk_unit(),
  })),
  ```
  with a `// Will be fixed by the cleanup pass
  insert_ptr_metadata.` comment. INFERENCE: Charon
  reconstructs the metadata explicitly so LLBC's reference-
  taking is uniformly typed at wide-pointer width;
  RegseqModel does not need this slot because stage 3 already
  encodes wide vs thin via `Ty::Ref` + the underlying
  pointee's layout (`Ty::Slice` / `Ty::Dynamic`).
- **`Aggregate` stores `Vec<Operand>`**, not
  rustc_middle's `IndexVec<FieldIdx, Operand>`. The
  translator preserves operand order
  [`translate_bodies.rs:824-828`]:
  ```rust
  let operands_t: Vec<Operand> = operands
      .iter()
      .map(|op| self.translate_operand(span, op))
      .try_collect()?;
  ```
  HYPOTHESIS: Charon does not need the typed-index because
  field-indexing uses the `AggregateKind`'s associated
  metadata at use sites; positional storage suffices.

`Rvalue` does not carry a size assert in the inspected
source. Charon's design is unconcerned with the byte
budget rustc enforces (`static_assert_size!(Rvalue<'_>, 40)`).

Maps to `D-7-1` (variant set + payload shapes), `D-7-8`
(operand-pair storage in `BinaryOp` -- Charon uses flat),
`D-7-10` (precedent for both lower-CopyForDeref and
reject-WrapUnsafeBinder).

---

## 2. `Use(Operand)`

Identity wrapper [`expressions.rs:703-704`]:

```rust
/// Lifts an operand as an rvalue.
Use(Operand),
```

Translation [`translate_bodies.rs:598`]:
```rust
mir::Rvalue::Use(operand) =>
    Ok(Rvalue::Use(self.translate_operand(span, operand)?)),
```

Direct one-to-one mapping. Maps to `D-7-1` (Use payload).

---

## 3. `Ref { place, kind, ptr_metadata }`

Reference to a place [`expressions.rs:705-713`]:

```rust
/// Takes a reference to the given place.
/// The `Operand` refers to the init value of the metadata,
/// it is `()` if no metadata
#[charon::rename("RvRef")]
Ref {
    place: Place,
    #[serde_state(stateless)]
    kind: BorrowKind,
    ptr_metadata: Operand,
},
```

`BorrowKind` is a 5-variant enum
[`expressions.rs:132-157`]:

```rust
#[charon::variants_prefix("B")]
pub enum BorrowKind {
    Shared,
    Mut,
    TwoPhaseMut,    // mir::MutBorrowKind::TwoPhaseBorrow
    Shallow,        // mir::FakeBorrowKind::Shallow only
    UniqueImmutable, // mir::MutBorrowKind::ClosureCapture
}
```

Translation collapses rustc's nested 3-variant
`BorrowKind { Shared, Fake(FakeBorrowKind),
Mut { kind: MutBorrowKind } }` into Charon's 5-variant flat
form [`translate_bodies.rs:124-136`]:

```rust
fn translate_borrow_kind(borrow_kind: mir::BorrowKind) -> BorrowKind {
    match borrow_kind {
        mir::BorrowKind::Shared => BorrowKind::Shared,
        mir::BorrowKind::Mut { kind } => match kind {
            mir::MutBorrowKind::Default => BorrowKind::Mut,
            mir::MutBorrowKind::TwoPhaseBorrow => BorrowKind::TwoPhaseMut,
            mir::MutBorrowKind::ClosureCapture => BorrowKind::UniqueImmutable,
        },
        mir::BorrowKind::Fake(mir::FakeBorrowKind::Shallow) => BorrowKind::Shallow,
        mir::BorrowKind::Fake(mir::FakeBorrowKind::Deep) => unimplemented!(),
    }
}
```

Key observations:

- **Region is dropped at translation.** The rustc-side
  pattern `mir::Rvalue::Ref(_region, borrow_kind, place)`
  ignores the region with `_region`
  [`translate_bodies.rs:612`]. Direct precedent for D-7-1's
  Ref variant carrying no `Region` slot -- Charon's
  Aeneas-targeted IR drops it just as RegseqModel's stage-3
  `Ty::Ref(t, SharedBorrow|UniqueBorrow)` does. The
  verification IR demonstrates that semantic preservation
  does not require the region tag in operand-position
  rvalues.
- **`BorrowKind::Fake(Shallow)` is preserved** as
  `BorrowKind::Shallow`, not collapsed. This contradicts
  RegseqModel's expectation that `CleanupPostBorrowck` NOPs
  every `Rvalue::Ref(_, BorrowKind::Fake(_), _)`. INFERENCE:
  Charon consumes a different pre-cleanup MIR form (likely
  via hax, which may interpose before rustc's cleanup
  passes), so its `Shallow` branch is reachable by design.
  RegseqModel reads `optimized_mir` which has
  `CleanupPostBorrowck` applied, so the Fake branch is
  unreachable -- the two designs target different MIR
  phases.
- **`BorrowKind::Fake(Deep)`** is `unimplemented!()` (a
  panic). HYPOTHESIS: Deep fake-borrows are deref-pattern
  artefacts that haven't required handling in Charon's test
  corpus. Stage 7 reads optimized_mir where this is also
  cleaned up.
- **`MutBorrowKind::TwoPhaseBorrow` and `ClosureCapture` are
  preserved** as distinct `BorrowKind` variants
  (`TwoPhaseMut` / `UniqueImmutable`). Aeneas's
  verification semantics need to distinguish them.
  RegseqModel's C-codegen target does not -- D-7-2 can
  collapse to a 2-variant subset (`SharedBorrow` /
  `UniqueBorrow`) parallel to stage 3's `Ty::Ref`
  discriminator.

`ptr_metadata: Operand` slot is filled with an
`Opaque("Missing metadata")` placeholder at translation time
[`translate_bodies.rs:619-622`] and reconstructed by the
`insert_ptr_metadata` cleanup pass.

Maps to `D-7-1` (Ref payload + drop of Region) +
`D-7-2` (BorrowKind subset; Charon's evidence supports a
narrower set than its 5-variant choice).

---

## 4. `RawPtr { place, kind, ptr_metadata }`

Raw pointer to a place [`expressions.rs:714-721`]:

```rust
/// Takes a raw pointer with the given mutability to the
/// given place.
RawPtr {
    place: Place,
    kind: RefKind,
    ptr_metadata: Operand,
},
```

`RefKind` (Charon's mutability discriminator -- searched in
the same file but not surfaced in the inspected lines; per
translation behaviour at [`translate_bodies.rs:625-630`]):

```rust
mir::Rvalue::RawPtr(mtbl, place) => {
    let mtbl = match mtbl {
        mir::RawPtrKind::Mut => RefKind::Mut,
        mir::RawPtrKind::Const => RefKind::Shared,
        mir::RawPtrKind::FakeForPtrMetadata => RefKind::Shared,
    };
    ...
}
```

Key observations:

- **`RawPtrKind::FakeForPtrMetadata` collapses to
  `RefKind::Shared`** at translation. Charon does not
  preserve the `FakeForPtrMetadata` distinction. Direct
  precedent for D-7-3 (RawPtrKind subset; collapse the
  borrowck-flavour to `Const` / `Shared`).
- **`RefKind` is binary (`Mut` / `Shared`)**. Same shape
  RegseqModel's D-7-3 candidate would land at -- 2-variant
  subset, no third "fake" arm.
- **`ptr_metadata` slot mirrors `Ref`'s** -- same
  Opaque-then-fixed-by-cleanup-pass pattern.

Maps to `D-7-1` (RawPtr payload) + `D-7-3` (RawPtrKind
subset; Charon collapses 3 -> 2 at the bridge).

---

## 5. `Cast` -- folded into `UnOp::Cast(CastKind)`

Charon does not have a top-level `Cast` rvalue. The
translator wraps every cast as
`Rvalue::UnaryOp(UnOp::Cast(cast_kind), operand)`
[`translate_bodies.rs:642-768`]. The `CastKind` enum is
6-variant [`expressions.rs:221-247`]:

```rust
#[charon::variants_prefix("Cast")]
pub enum CastKind {
    Scalar(LiteralTy, LiteralTy),
    RawPtr(Ty, Ty),
    FnPtr(Ty, Ty),
    Unsize(Ty, Ty, UnsizingMetadata),
    Transmute(Ty, Ty),
    Concretize(Ty, Ty),
}
```

Translation maps rustc_middle's 11-effective `CastKind`
arms (counting `PointerCoercion` sub-variants) into these 6
[`translate_bodies.rs:650-765`]. Notable collapses:

| rustc_middle | Charon `CastKind` |
|-|-|
| `IntToInt`, `IntToFloat`, `FloatToInt`, `FloatToFloat` | `Scalar(src_lit, tgt_lit)` |
| `PtrToPtr`, `PointerCoercion::MutToConstPointer`, `PointerCoercion::ArrayToPointer`, `FnPtrToPtr`, `PointerExposeProvenance`, `PointerWithExposedProvenance` | `RawPtr(src_ty, tgt_ty)` |
| `PointerCoercion::ClosureFnPointer(_)`, `PointerCoercion::UnsafeFnPointer`, `PointerCoercion::ReifyFnPointer(_)` | `FnPtr(src_ty, tgt_ty)` |
| `PointerCoercion::Unsize` | `Unsize(src_ty, tgt_ty, UnsizingMetadata)` |
| `Transmute` | `Transmute(src_ty, tgt_ty)` |
| `Subtype` | `Transmute(src_ty, tgt_ty)` (with `// TODO` comment) |

Key observations:

- **Charon collapses `Subtype` into `Transmute`**
  [`translate_bodies.rs:701`] with a `// TODO` comment --
  acknowledged hack rather than principled choice.
- **Charon includes `PointerCoercion::MutToConstPointer`
  and `ArrayToPointer` in the `RawPtr` arm.** This
  implies Charon's hax-side input has not had
  `CleanupPostBorrowck` applied (per §3 above). RegseqModel
  reads optimized_mir where these have been rewritten to
  `PtrToPtr` already, so the bridge sees only `PtrToPtr`
  on this path.
- **`CoercionSource` is dropped.** Charon's three
  `PointerCoercion` matches use `..` to ignore the
  diagnostic-only `CoercionSource` field
  [`translate_bodies.rs:660-666, 695-697, 702`]. Direct
  precedent for D-7-4's "drop CoercionSource" sub-decision.
- **`UnsizingMetadata` is reconstructed at translation**
  [`translate_bodies.rs:702-764`] -- Length / VTable /
  VTableUpcast / Unknown variants. RegseqModel's stage 3
  `Ty::Dynamic` already encodes the dynamic-Trait shape;
  the unsize cast's metadata reconstruction is a stage 14 /
  backend concern, not stage 7.

`UnsizingMetadata` is a 4-variant enum
[`expressions.rs:263-276`]: `Length(Box<ConstantExpr>)`,
`VTable(TraitRef, Box<ConstantExpr>)`,
`VTableUpcast(Vec<FieldId>)`, `Unknown`.

Maps to `D-7-4` (CastKind subset; Charon's 6-variant
collapse is more aggressive than stage-7's likely choice
of "preserve rustc's curated set after borrowck cleanup").

---

## 6. `BinaryOp(BinOp, Operand, Operand)` and `UnaryOp(UnOp, Operand)`

Both store operands flat (not boxed)
[`expressions.rs:723-725`]:

```rust
BinaryOp(BinOp, Operand, Operand),
UnaryOp(UnOp, Operand),
```

`BinOp` is 22-variant
[`expressions.rs:307-347`]:

```rust
#[charon::rename("Binop")]
pub enum BinOp {
    BitXor, BitAnd, BitOr,
    Eq, Lt, Le, Ne, Ge, Gt,
    Add(OverflowMode),
    Sub(OverflowMode),
    Mul(OverflowMode),
    Div(OverflowMode),
    Rem(OverflowMode),
    AddChecked, SubChecked, MulChecked,
    Shl(OverflowMode),
    Shr(OverflowMode),
    Offset,
    Cmp,
}
```

`OverflowMode` is 3-variant [`expressions.rs:280-291`]:
`Panic`, `UB`, `Wrap`. The arithmetic / shift variants
parameterise on it; the rationale is that rustc's
`Add` / `AddUnchecked` / `AddWithOverflow` triple collapses
into `Add(Panic)` / `Add(UB)` / `AddChecked` (where
`AddChecked` returns `(T, bool)`).

`UnOp` is 3-variant [`expressions.rs:173-181`]:

```rust
#[charon::rename("Unop")]
pub enum UnOp {
    Not,
    Neg(OverflowMode),
    Cast(CastKind),
}
```

`Neg(OverflowMode)` parameterises overflow ("can overflow,
for `-i::MIN`") -- different from rustc's bare `Neg`.

Notable absences from rustc's `UnOp`:

- **No `PtrMetadata`.** Charon's translator handles
  `mir::UnOp::PtrMetadata` by emitting a place-projection
  instead [`translate_bodies.rs:779-788`]:
  ```rust
  mir::UnOp::PtrMetadata => match operand {
      Operand::Copy(p) | Operand::Move(p) => {
          return Ok(Rvalue::Use(Operand::Copy(
              p.project(ProjectionElem::PtrMetadata,
                        tgt_ty.clone()),
          )));
      }
      Operand::Const(_) => panic!("unexpected metadata operand"),
  },
  ```
  PtrMetadata becomes a `ProjectionElem::PtrMetadata` on
  the place's projection chain
  [`expressions.rs:71-77`]. Direct precedent for an
  alternative D-7-7 strategy: lower `UnOp::PtrMetadata` to
  a place-side projection rather than carry it as a unary
  operator. RegseqModel's stage-5 `PlaceElem` variant set
  is closed at 6 variants without a `PtrMetadata` arm, so
  this Charon-style lowering is closed off; stage 7 can only
  preserve `UnOp::PtrMetadata` or reject it.

Maps to `D-7-1` (BinaryOp pair-storage choice -- Charon
uses flat, validating that Box is rustc's size-budget
artefact not a load-bearing semantic), `D-7-6` (BinOp /
UnOp curation -- Charon's `OverflowMode` parameterisation
diverges from rustc; stage 7 preserves rustc's flat
27-variant `BinOp`), `D-7-7` (PtrMetadata retention --
Charon lowers; stage 7 preserves).

---

## 7. `NullaryOp(NullOp, Ty)`

`NullaryOp` survives at the Charon rvalue level
[`expressions.rs:726-727`] with `NullOp` 6-variant
[`expressions.rs:197-204`]:

```rust
pub enum NullOp {
    SizeOf,
    AlignOf,
    OffsetOf(TypeDeclRef, Option<VariantId>, FieldId),
    UbChecks,
    OverflowChecks,
    ContractChecks,
}
```

Per stage-6 charon.md §4: rustc's `Operand::RuntimeChecks`
is lowered via translation to
`Rvalue::NullaryOp(NullOp::Ub|Overflow|ContractChecks, bool)`
plus `Operand::Move(local)`. SizeOf / AlignOf / OffsetOf
likewise survive at the rvalue level for verification.

**Stage-7 implication: rustc_middle has no `NullaryOp` rvalue
variant in the pinned nightly.** The bridge does not see
this construct on the rvalue side -- it sees `RuntimeChecks`
at the operand level (handled by stage 6's D-6-1.2 lowering)
and `SizeOf` / `AlignOf` / `OffsetOf` only as intrinsic
calls (handled at stage 9 / 14). RegseqModel has no parallel
rvalue variant to allocate.

Informational only for D-7-1 and D-7-K; no direct
mapping.

---

## 8. `Discriminant(Place)`

Identical to rustc_middle's variant
[`expressions.rs:728-732`]:

```rust
/// Discriminant read. Reads the discriminant value of an
/// enum.
Discriminant(Place),
```

Translation [`translate_bodies.rs:792-808`] additionally
checks that the place's type is an ADT -- a translation-
time well-formedness guard that rustc lacks.

Maps to `D-7-1` (Discriminant payload; trivial -- `Place`
inline).

---

## 9. `Aggregate(AggregateKind, Vec<Operand>)` and `AggregateKind`

`AggregateKind` is a 3-variant enum
[`expressions.rs:805-818`]:

```rust
#[charon::variants_prefix("Aggregated")]
pub enum AggregateKind {
    Adt(TypeDeclRef, Option<VariantId>, Option<FieldId>),
    Array(Ty, Box<ConstantExpr>),
    RawPtr(Ty, RefKind),
}
```

Translation collapses rustc_middle's 7-variant `AggregateKind`
into these 3 [`translate_bodies.rs:830-895`]:

| rustc_middle | Charon `AggregateKind` |
|-|-|
| `Array(Ty)` | `Array(t_ty, Box<ConstantExpr>)` (with computed length constant) |
| `Tuple` | `Adt(TypeDeclRef::new(TypeId::Tuple, _), None, None)` |
| `Adt(def_id, variant_idx, generics, _user_ty, field_index)` | `Adt(tref, variant_id_or_none, field_id_or_none)` |
| `Closure(def_id, generics)` | `Adt(closure_type_ref, None, None)` (closure as ADT) |
| `Coroutine(_, _)`, `CoroutineClosure(_, _)` | `raise_error!("Coroutines are not supported")` |
| `RawPtr(Ty, Mutability)` | `RawPtr(t_ty, RefKind)` |

Key observations:

- **`UserTypeAnnotationIndex` slot is dropped** at
  translation [`translate_bodies.rs:854`] (the `_` in the
  pattern). Direct precedent for D-7-5's "drop NLL slot"
  sub-decision.
- **Tuples are not a separate variant** -- collapsed into
  `Adt(TypeId::Tuple, ...)`. RegseqModel's stage-3 `Ty`
  enum has a separate `Ty::Tuple` shape; INFERENCE: stage 7
  should keep `AggregateKind::Tuple` distinct because
  RegseqModel's type-side already distinguishes them, and
  collapsing here would lose the symmetry.
- **`Closure` is not a separate variant** -- translated to
  `Adt(closure_type_ref, ...)` because Charon represents
  closures as captures-structs. Stage 3's RegseqModel has
  the same impulse: `Ty::Closure(item, args)` is the
  identity view, with a parallel `Ty::Struct(synthesised_item, args, layout)`
  for layout. INFERENCE for D-7-5: stage 7 may keep
  `Closure` distinct at the rvalue level (preserving the
  identity) or follow Charon's collapse (treating closures
  as ADTs at construction).
- **`Coroutine` and `CoroutineClosure` are rejected**
  outright. Direct precedent for an aggressive D-7-5
  subset that excludes the async / coroutine arms
  -- consistent with `#![no_std]` driver code's
  expected feature set.
- **Operand storage is `Vec<Operand>`**, positional. No
  `IndexVec<FieldIdx, _>` typed-index. INFERENCE: this
  works because Charon's Adt(field_id) and the underlying
  type's field declaration order canonicalise the
  field-to-position map.

Maps to `D-7-5` (AggregateKind subset; Charon collapses
3-variant target gives strong precedent for dropping
`UserTypeAnnotationIndex` and rejecting Coroutine /
CoroutineClosure; the Closure-as-Adt collapse is a stage-7
choice, not forced).

---

## 10. `Repeat(Operand, Ty, Box<ConstantExpr>)`, `ShallowInitBox`, `Len`

**`Repeat`** [`expressions.rs:763-766`]:

```rust
/// `Repeat(x, n)` creates an array where `x` is copied `n` times.
///
/// We translate this to a function call for LLBC.
Repeat(Operand, Ty, Box<ConstantExpr>),
```

- The count is a `Box<ConstantExpr>` -- a fully-resolved
  constant payload. Charon does not specialise to a `u64`
  even after monomorphisation; the constant slot is
  uniform. Translation
  [`translate_bodies.rs:605-611`] uses
  `translate_ty_constant_expr` to resolve.
- **Charon further lowers `Repeat` to a function call**
  in the LLBC layer (per the doc comment, and via the
  `ops_to_function_calls` pass at
  [`charon/src/transform/simplify_output/ops_to_function_calls.rs:1-12`]).
  This is a downstream lowering, not stage-7-relevant for
  RegseqModel which keeps `Repeat` as a structural rvalue.

Maps to `D-7-9` (Repeat count payload). Charon's choice
(`ConstantExpr` slot) parallels RegseqModel's stage-4
`ConstId` interning -- both are post-evaluation forms.
RegseqModel-specific: stage 4 + stage 3 `Ty::Array(elem,
n)` already pin the length as a post-mono `u64`; the
`Repeat` count is the same kind of value, so a direct `u64`
payload is well-typed. Charon's uniform-constant approach
is one valid choice; stage 7's `u64`-direct is another.

**`ShallowInitBox(Operand, Ty)`** [`expressions.rs:767-770`]:

```rust
/// Transmutes a `*mut u8` (obtained from `malloc`) into
/// shallow-initialized `Box<T>`. This only appears as part
/// of lowering `Box::new()` in some cases. We reconstruct
/// the original `Box::new()` call, but sometimes may fail
/// to do so, leaking the expression.
ShallowInitBox(Operand, Ty),
```

Charon **preserves** the variant -- it does not reject as
RegseqModel rustc.md §11 notes for the rustc codegen path.
INFERENCE: Charon consumes pre-runtime-lowering MIR (likely
hax intercepts before `ElaborateBoxDerefs`), so
`ShallowInitBox` is reachable for it. RegseqModel reads
post-runtime-lowering optimized_mir where it has been
eliminated -- bridge can reject without semantic loss.

Maps to `D-7-10` (ShallowInitBox handling). Charon's
preserve-the-variant choice is informational only; stage 7's
reject-on-encounter is consistent with what
optimized_mir actually delivers.

**`Len(Place, Ty, Option<Box<ConstantExpr>>)`**
[`expressions.rs:752-762`]: kept by Charon for slice-pattern
matching. rustc_middle's pinned-nightly removed `Len`. The
divergence is informational only -- rustc no longer
delivers this rvalue at all in the bridge's input.

---

## 11. Translation evidence: rejection vs lowering

Charon's translator establishes a clear rejection-vs-lowering
gradient for unsupported constructs:

| rustc construct | Charon behaviour | `[file:line]` |
|-|-|-|
| `mir::Rvalue::CopyForDeref(p)` | **Lower** to `Rvalue::Use(Copy(p))` | translate_bodies.rs:599-604 |
| `mir::Rvalue::ShallowInitBox(...)` | **Preserve** (variant kept) | translate_bodies.rs:898-902 |
| `mir::Rvalue::ThreadLocalRef(_)` | **Reject** with diagnostic | translate_bodies.rs:903-909 |
| `mir::Rvalue::WrapUnsafeBinder { .. }` | **Reject** with diagnostic | translate_bodies.rs:910-915 |
| `mir::AggregateKind::Coroutine(..)` / `CoroutineClosure(..)` | **Reject** with diagnostic | translate_bodies.rs:892-895 |
| `mir::BorrowKind::Fake(FakeBorrowKind::Deep)` | **Panic** (`unimplemented!()`) | translate_bodies.rs:134 |
| `mir::Operand::RuntimeChecks(check)` | **Lower** to `NullaryOp + Move(local)` | (per stage-6 charon.md §4) |

Patterns:

- **Lower when the construct has a trivially equivalent
  alternative form already in the IR.** `CopyForDeref` ->
  `Use(Copy(...))` (the docs say it's "equivalent to a read
  from a place"). `RuntimeChecks` -> `NullaryOp + Move`
  (preserves the runtime-flag query at a different level).
- **Reject when the construct is feature-gated and absent
  from Charon's target corpus.** Coroutines, thread-locals,
  unsafe binders -- features Charon does not aim to
  verify.
- **Preserve when the construct is well-formed at Charon's
  input layer but not at the bridge's downstream layers.**
  `ShallowInitBox` is preserved by Charon and then handled
  by a later reconstruction pass; not relevant to
  RegseqModel which reads post-runtime-lowering MIR.

For RegseqModel's D-7-10 (CopyForDeref / ShallowInitBox)
and D-7-11 (WrapUnsafeBinder), Charon's evidence supports
**reject-on-encounter** for ThreadLocalRef-style constructs
(no equivalent form available, no driver-corpus presence)
and **lower-or-reject** for elimination-pass-removed
constructs (CopyForDeref, ShallowInitBox).

Maps to `D-7-10`, `D-7-11`, `D-7-2` (Fake handling).

---

## 12. `simplify_constants` micro-pass -- operand-payload normalisation

The `simplify_constants` pass header
[`charon/src/transform/simplify_output/simplify_constants.rs:1-12`]:

```text
//! The MIR constant expressions lead to a lot of duplication: there are
//! for instance constant ADTs which duplicate the "regular" aggregated
//! ADTs in the operands, constant references, etc. This reduces the number
//! of cases to handle and eases the function translation in Aeneas.
//!
//! This pass removes all those occurrences so that only the
//! `ConstantExpression::Literal`. It does so by introducing intermediate statements.
```

The pass operates on operands inside rvalues -- when a
constant operand is an Adt / Array / Ref / Ptr / Global,
the pass emits intermediate let-bindings using
`Rvalue::Aggregate` / `Rvalue::Ref` / `Rvalue::Use` to build
the value, then replaces the constant operand with a
`Move(local)` of the synthesised local.

INFERENCE: this is the post-translation invariant
construction that lets Aeneas assume Operand::Const payloads
are always `Literal` at the verification entry point.
RegseqModel's stage-4 `Const` is already at the
post-evaluation layer with a fixed 11-variant set, so the
analogous invariant comes by construction rather than by a
normalisation pass. No direct stage-7 import.

Maps to `D-7-1` (Aggregate operand uniformity precedent --
Charon ensures aggregate operand operands are well-formed
literals or place-loads only).

---

## 13. `ops_to_function_calls` micro-pass

[`charon/src/transform/simplify_output/ops_to_function_calls.rs:1-12`]:

```text
//! Desugar some unary/binary operations and the array repeats to function calls.
//! For instance, we desugar ArrayToSlice from an unop to a function call.
//! This allows a more uniform treatment later on.
//! TODO: actually transform all the unops and binops to function calls?
```

The pass observed example
[`ops_to_function_calls.rs:11-19`]:
```rust
StatementKind::Assign(
    p,
    Rvalue::UnaryOp(
        UnOp::Cast(CastKind::Unsize(src_ty, tgt_ty,
                   UnsizingMetadata::Length(_))),
        op,
    ),
) => { ... transform to function call ... }
```

This is Charon's general approach: when an rvalue cannot be
uniformly handled in the verification semantics, lower to a
function call against a builtin / synthesised function.
`Repeat`, `RawPtr`, and slice-coercion all use this pattern.

INFERENCE for stage 7: RegseqModel's C-codegen target has
its own "lower to a helper" pattern (e.g., overflow checks
emit `__builtin_*` calls). The decision lives at stage 14
(domain-specific extensions / backend lowering), not stage
7. Stage 7's job is variant-set + payload shape; whether
each variant maps to direct C arithmetic vs a helper call
is downstream.

---

## 14. Summary mapping to `D-7-K` candidate decisions

| Tag | Decision | Charon evidence anchor |
|-|-|-|
| `D-7-1` | Rvalue variant set + payload shapes | §1, §2-§9; closer to 11 effective variants by collapsing Cast into UnaryOp |
| `D-7-2` | BorrowKind subset | §3; Charon has 5 variants; collapses MutBorrowKind 3-way; supports stage-7's narrower 2-variant target |
| `D-7-3` | RawPtrKind subset | §4; Charon collapses to binary `RefKind`; FakeForPtrMetadata folds to Shared at translation |
| `D-7-4` | CastKind subset (drop CoercionSource; drop ArrayToPointer / MutToConstPointer) | §5; Charon's 6-variant collapse is more aggressive than stage-7's likely choice |
| `D-7-5` | AggregateKind subset (drop UserTypeAnnotationIndex; reject Coroutine variants) | §9; Charon drops the NLL slot and rejects Coroutines |
| `D-7-6` | BinOp/UnOp curation | §6; Charon parameterises Add/Sub/Mul/etc. on OverflowMode rather than splitting variants -- different design choice; stage 7 preserves rustc's flat split |
| `D-7-7` | UnOp::PtrMetadata retention | §6; Charon lowers to ProjectionElem::PtrMetadata (closed off for stage 7 since PlaceElem is locked) |
| `D-7-8` | BinaryOp pair storage (Box vs flat) | §6; Charon stores flat -- validates that Box is a rustc-specific size-budget artefact |
| `D-7-9` | Repeat count payload | §10; Charon uses Box<ConstantExpr> uniformly; stage 7's u64-direct is also valid (Ty::Array length is post-mono u64) |
| `D-7-10` | Reject CopyForDeref / ShallowInitBox | §11; Charon lowers CopyForDeref (alternative pattern) but preserves ShallowInitBox; both are gone from optimized_mir for stage 7 -- reject-on-encounter is consistent |
| `D-7-11` | WrapUnsafeBinder retention | §11; Charon rejects with diagnostic (driver-feature-set absent) |
| `D-7-K` | render_rvalue defer to stage 11 | (no specific Charon precedent; deferral inherits from D-5-K / D-6-K rationale) |

This kickoff prior-art pass establishes the rejection /
lowering / preservation precedent. Mid-stage deeper reads
(e.g., into Aeneas's verification semantics for Repeat or
into Charon's `insert_ptr_metadata` cleanup pass) can land
additional citations as specific D-tags demand them.
