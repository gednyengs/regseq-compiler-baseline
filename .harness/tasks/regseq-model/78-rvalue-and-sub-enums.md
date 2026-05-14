# regseq-model-78: Rvalue + sub-enums (CastKind, BorrowKind, AggregateKind, BinOp 26-variant, UnOp) (P1a-DS-12b)

## Description

Implement the `Rvalue` IR data shape per `spec.md` §21: the
closed 9-variant `Rvalue` enum plus the supporting sub-enums
(`CastKind` 11-variant + `PointerCoercion` 4-variant
sub-enum, the `Provenance` reuse of §10.1's borrow-flavor enum
in lieu of a parallel `BorrowKind`, `Mutability` reuse of
§10.1's pointer-mutability enum, `AggregateKind` 7-variant,
`BinOp` 26-variant, `UnOp` 3-variant).

`Rvalue` is the RHS-expression surface of MIR -- the typed
expression of "how a value is computed." It is body-local
content; no top-level interner; equality is structural. The
9 variants are exhaustive: `Use`, `Repeat`, `Ref`, `RawPtr`,
`Cast`, `BinaryOp`, `UnaryOp`, `Discriminant`, `Aggregate`.

## Input Context

- `docs/discussions/regseq-model/spec.md` §21.1 (`Rvalue` enum
  definition; `spec.md:7150-7256`)
- `docs/discussions/regseq-model/spec.md` §21.1.1 (Variants
  dropped vs rustc; `spec.md:7218-7256`)
- `docs/discussions/regseq-model/spec.md` §21.2 (Per-variant
  payload shapes; `spec.md:7258-7380`)
- `docs/discussions/regseq-model/spec.md` §21.3 (Sub-enums:
  Provenance reuse, Mutability reuse, CastKind +
  PointerCoercion, BinOp 26-variant, UnOp, AggregateKind;
  `spec.md:7381-7628`)
- `docs/discussions/regseq-model/spec.md` §21.4 (Identity and
  storage; `spec.md:7629-7649`)
- `docs/discussions/regseq-model/spec.md` §10.1 (auxiliary
  closed enums on `Ty` -- `Provenance`, `Mutability` --
  reused at the Rvalue surface)
- `crates/regseq-model/src/operand.rs` (`Operand` from
  regseq-model-77)
- `crates/regseq-model/src/place.rs` (`Place` from
  regseq-model-76)
- `crates/regseq-model/src/ids.rs` (`TyId`, `FieldIdx`,
  `VariantIdx`)
- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent brief;
  P1a-DS-12b row)

## Implementation Notes

Create a new module `crates/regseq-model/src/rvalue.rs`
(re-exported from `lib.rs`). The sub-enums `CastKind`,
`PointerCoercion`, `BinOp`, `UnOp`, `AggregateKind` live in the
same module; `Provenance` and `Mutability` are **reused**
from §10.1 (do not redefine).

### Public API -- Rvalue (closed 9-variant)

Per `spec.md:7157-7204`:

```rust
/// A typed RHS expression: a value-producing computation.
/// Body-local content; appears inside a body's statements,
/// not in any top-level interner table. Equality is structural.
pub enum Rvalue {
    /// Identity: yields the operand's value unchanged.
    Use(Operand),

    /// Array constructor: `[x; N]`. The `u64` count is the
    /// post-monomorphisation array length, mirroring stage-3
    /// `Ty::Array(elem_ty, n)` canonical form (§10.0).
    Repeat(Operand, u64),

    /// Borrow construction: `&p` / `&mut p`. The `Provenance`
    /// discriminator reuses the §10.1 type-system enum
    /// (`SharedBorrow` / `UniqueBorrow`); rustc's
    /// `Region<'tcx>` slot is dropped per the region-erasure
    /// rule of §17.1.
    Ref(Provenance, Place),

    /// Raw pointer construction: `&raw const p` / `&raw mut p`.
    /// The `Mutability` discriminator reuses the §10.1
    /// type-system enum (`Not` / `Mut`).
    RawPtr(Mutability, Place),

    /// `as`-style cast (or compiler-inserted coercion). The
    /// destination `TyId` resolves through the §12 type
    /// interner.
    Cast(CastKind, Operand, TyId),

    /// Binary arithmetic / comparison / pointer offset.
    /// Operands inline (no `Box<(Operand, Operand)>`
    /// indirection); see §21.2.6 for the size-budget
    /// rationale.
    BinaryOp(BinOp, Operand, Operand),

    /// Unary operation: `!x`, `-x`, or pointer-metadata
    /// extraction.
    UnaryOp(UnOp, Operand),

    /// Read the discriminant of an enum at the place. For
    /// non-enum predecessor types yields zero (rustc's
    /// documented semantics).
    Discriminant(Place),

    /// Aggregate value construction (tuple / struct / enum
    /// variant / union / closure environment / fixed-length
    /// array literal / fat raw pointer). Operand storage is
    /// `Box<[Operand]>` (boxed slice; matches §19.2's
    /// `Box<[PlaceElem]>` projection-storage convention).
    Aggregate(AggregateKind, Box<[Operand]>),
}
```

### Public API -- CastKind and PointerCoercion

Per `spec.md:7400-7421`:

```rust
pub enum CastKind {
    PointerExposeProvenance,
    PointerWithExposedProvenance,
    PointerCoercion(PointerCoercion),
    IntToInt,
    FloatToInt,
    FloatToFloat,
    IntToFloat,
    PtrToPtr,
    FnPtrToPtr,
    Transmute,
    Subtype,
}

pub enum PointerCoercion {
    ReifyFnPointer,           // hir::Safety dropped (§21.2.5)
    UnsafeFnPointer,
    ClosureFnPointer,         // hir::Safety dropped (§21.2.5)
    Unsize,
}
```

11 top-level `CastKind` variants; 4-variant `PointerCoercion`
sub-enum.

### Public API -- BinOp (26-variant) and UnOp (3-variant)

Per `spec.md:7453-7479`:

```rust
pub enum BinOp {
    // Wrapping arithmetic
    Add, Sub, Mul, Div, Rem,

    // UB-on-overflow arithmetic (from `unchecked_*` intrinsics)
    AddUnchecked, SubUnchecked, MulUnchecked,

    // Overflow-detecting arithmetic (returns `(T, bool)`)
    AddWithOverflow, SubWithOverflow, MulWithOverflow,

    // Bitwise
    BitXor, BitAnd, BitOr,

    // Shifts (wrapping and UB-on-overflow flavours)
    Shl, ShlUnchecked,
    Shr, ShrUnchecked,

    // Comparisons (return `bool`)
    Eq, Lt, Le, Ne, Ge, Gt,

    // Three-way comparison (returns `core::cmp::Ordering`)
    Cmp,

    // Pointer offset (`<*const T>::offset` semantics)
    Offset,
}
```

26 variants, preserved verbatim from rustc; see
`spec.md:7480-7503` for the family rationale.

Per `spec.md:7508-7513`:

```rust
pub enum UnOp {
    Not,           // logical inversion (`!`)
    Neg,           // arithmetic negation (`-`)
    PtrMetadata,   // metadata extraction from a fat pointer
}
```

3 variants, preserved verbatim from rustc.

### Public API -- AggregateKind (7-variant)

Per `spec.md:7526-7567`:

```rust
pub enum AggregateKind {
    /// Fixed-length array literal: `[a, b, c]`.
    /// `TyId` resolves to `Ty::Array(elem_ty, n)`.
    Array(TyId),

    /// Tuple constructor: `(a, b, c)`.
    /// `TyId` resolves to `Ty::Tuple { fields, layout }`.
    Tuple(TyId),

    /// Struct constructor: `MyStruct { field: a, ... }`.
    /// `TyId` resolves to `Ty::Struct { item, args, layout }`.
    Struct(TyId),

    /// Enum variant constructor: `MyEnum::Variant(a)`.
    /// `TyId` resolves to `Ty::Enum { item, args, layout }`;
    /// `VariantIdx` selects the active variant.
    Enum(TyId, VariantIdx),

    /// Union constructor: `MyUnion { active_field: a }`.
    /// `TyId` resolves to `Ty::Union { item, args, layout }`;
    /// `FieldIdx` selects the active field.
    Union(TyId, FieldIdx),

    /// Closure environment construction: `|...| { ... }`.
    /// `TyId` resolves to `Ty::Closure(item, args)` -- the
    /// closure-type identity view (§13.1).
    Closure(TyId),

    /// Fat raw pointer construction from `(data_ptr, metadata)`.
    /// `TyId` is the *pointee* type (mirroring rustc's
    /// `RawPtr(Ty<'tcx>, Mutability)` shape).
    RawPtr(TyId, Mutability),
}
```

7 variants. The "three-states-in-five-slots" rustc `Adt`
encoding is split into three RegseqModel variants
(`Struct(TyId)`, `Enum(TyId, VariantIdx)`, `Union(TyId,
FieldIdx)`) so the type system enforces "active_field is
Union-only" and "VariantIdx is Enum-only" without a runtime
invariant per `spec.md:7602-7612`.

### Conventions

- **Closed enum exhaustiveness.** All 9 `Rvalue` variants and
  all sub-enum variants are exhaustive; no `Other` / `Opaque`
  arm. The rejected rustc variants (`CopyForDeref`,
  `ShallowInitBox`, `ThreadLocalRef`, `WrapUnsafeBinder`,
  `BorrowKind::Fake`, `MutToConstPointer`, `ArrayToPointer`,
  `Coroutine` / `CoroutineClosure`) are bridge-side rejections
  per `spec.md` §21.5.1; they do not appear in the data
  shape.
- **Reuse `Provenance` and `Mutability` from §10.1.** Per
  `spec.md:7383-7396`, do not introduce parallel enums at the
  Rvalue surface. Import the existing types from the project's
  type-system module (e.g.,
  `crate::ty::{Provenance, Mutability}`); add the imports to
  the module preamble.
- **No `BorrowKind` enum at the data shape.** The §21.6.1
  bridge collapse maps rustc's 3-variant `BorrowKind` plus
  3-variant `MutBorrowKind` sub-discriminator to
  `Provenance`'s 2 variants (with reject-on-encounter of
  `BorrowKind::Fake`); the result is `Rvalue::Ref(Provenance,
  Place)` per `spec.md:7167-7170`. The "BorrowKind" name in
  the brief's task title is shorthand for the `Provenance`
  surface.
- **Flat operand storage in `BinaryOp`.** Operands are inline,
  not boxed (`spec.md:7332-7344`). RegseqModel does not
  inherit rustc's 40-byte `Rvalue<'_>` size budget.
- **`Aggregate(AggregateKind, Box<[Operand]>)`** uses
  `Box<[Operand]>` per `spec.md:7368-7379` (matches `Place`'s
  `Box<[PlaceElem]>` slab convention).
- **`CastKind::PointerCoercion(PointerCoercion)`** drops
  rustc's `CoercionSource` and `hir::Safety` slots per
  `spec.md:7424-7441`. The bridge maps
  `mir::CastKind::PointerCoercion(coerc, source)` to
  `CastKind::PointerCoercion(coerc)` discarding `source`.
- **`CastKind::Subtype` and `CastKind::Transmute` survive into
  optimized_mir** per `spec.md:7443-7448`; preserved.
- **`UnOp::PtrMetadata` is reachable** in `optimized_mir`
  (post-cleanup runtime MIR) per `spec.md:7515-7521`.
- **`BinOp::Cmp` returns `i8`** (`core::cmp::Ordering`) and
  `BinOp::Offset` mirrors `<*const T>::offset` semantics per
  `spec.md:7500-7503`.
- **`BinOp` family inheritance.** The 26-variant verbatim
  preservation per `spec.md:7480-7484` -- no curation. Each
  variant maps to a distinct C lowering at codegen; the spec
  commits only to the variant set.
- **Serde**: `Rvalue` and all sub-enums carry `Operand`,
  `Place`, `TyId`, `FieldIdx`, `VariantIdx`, `u64`, plus
  the §10.1 `Provenance` / `Mutability` reused from the type
  module. All serde-friendly. Use
  `#[derive(Serialize, Deserialize)]`.
- **Derive `Clone, Debug, PartialEq, Eq, Hash`** on `Rvalue`
  and all sub-enums.
- **Doc comments mandatory** on every public type, variant,
  field. Reference the per-variant payload-shape sections
  (§21.2.1 - §21.2.9) in the variant-level doc comments.

## Non-Goals

- **Bridge translation from rustc `mir::Rvalue<'tcx>`** (the
  §21.6 per-variant table) is P1a-BR-05.
- **Rejection rules** (`spec.md:7657-7672`) are bridge-side
  (P1a-BR-05 / P1a-BR-02 for the type-translation cascade
  rejections).
- **Positive Rvalue-site invariants** (I-Rvalue-Operand-
  Wellformed, I-Rvalue-TyId-Interned, I-Rvalue-Aggregate-
  Operand-Count, I-Rvalue-Cast-Source-Type, etc. per
  `spec.md` §21.5.2) are body-walk territory:
  P1a-VAL-01 (Place validation propagation) + P1a-VAL-02 (ID
  referential integrity).
- **C lowering** for `Rvalue` (§21 C-lowering rows) is
  phase-1b.
- **Rendering** (`render_rvalue`) lives at
  `pretty-printer.md` §7.3 and is implemented in P1a-PP-03.
- **`Provenance` / `Mutability` definitions** -- reused from
  §10.1 (regseq-model-67 / P1a-DS-02 surface). This task
  imports; it does not redefine.
- **`BorrowKind` collapse semantics** (§21.6.1) is bridge-side
  (P1a-BR-05).

## Test Plan

**Test organization**: `unit-in-src` -- tests live in
`crates/regseq-model/src/rvalue.rs` `#[cfg(test)] mod tests`.

Coverage requirements (every variant exercised at least once
per `SESSION.md` Testing Requirements):

1. **`Rvalue::Use` construction.** Build with
   `Operand::Constant(ConstId(0))`; assert pattern match.
2. **`Rvalue::Repeat` construction.** Build with operand +
   `u64` count; assert both fields read back.
3. **`Rvalue::Ref` construction (each Provenance variant).**
   Build `Ref(Provenance::SharedBorrow, place)` and
   `Ref(Provenance::UniqueBorrow, place)`; assert distinctness
   over identical place.
4. **`Rvalue::RawPtr` construction (each Mutability variant).**
   Build `RawPtr(Mutability::Not, place)` and
   `RawPtr(Mutability::Mut, place)`; assert distinctness.
5. **`Rvalue::Cast` construction.** Build with each
   `CastKind` top-level variant (10 cases including the inner
   `PointerCoercion(PointerCoercion::Unsize)` case to cover
   the nested arm); assert pattern match.
6. **`Rvalue::BinaryOp` construction.** Build with at least
   one operator from each family (e.g.,
   `Add`, `AddUnchecked`, `AddWithOverflow`, `BitXor`, `Shl`,
   `ShlUnchecked`, `Eq`, `Cmp`, `Offset`); assert structural
   reads.
7. **`Rvalue::BinaryOp` -- all 26 BinOp variants reachable.**
   Build a `BinaryOp` value for each of the 26 `BinOp`
   variants; assert pairwise distinctness on the BinOp slot
   (with identical operands).
8. **`Rvalue::UnaryOp` construction (each UnOp variant).**
   Build `UnaryOp(UnOp::Not, op)`, `UnaryOp(UnOp::Neg, op)`,
   `UnaryOp(UnOp::PtrMetadata, op)`; assert distinctness.
9. **`Rvalue::Discriminant` construction.** Build with a Place;
   assert.
10. **`Rvalue::Aggregate` construction (each AggregateKind
    variant).** Build one of each:
    - `Array(TyId(0))`
    - `Tuple(TyId(0))`
    - `Struct(TyId(0))`
    - `Enum(TyId(0), VariantIdx(0))`
    - `Union(TyId(0), FieldIdx(0))`
    - `Closure(TyId(0))`
    - `RawPtr(TyId(0), Mutability::Not)`
    Assert all 7 are pairwise distinct.
11. **`Rvalue::Aggregate` -- empty operand slab.** Build with
    `Box::new([])`; assert.
12. **`Rvalue::Aggregate` -- non-empty operand slab.** Build
    with `Box::new([Operand::Constant(c1),
    Operand::Constant(c2)])`; assert length / iteration.
13. **All 9 Rvalue variants pairwise distinct.** Build one of
    each (using whatever supporting payload makes sense);
    assert all `(9 choose 2) = 36` pairs are inequal.
14. **`CastKind` -- 11 top-level variants reachable.** Build
    each of the 11 variants (including
    `PointerCoercion(_)`); assert pairwise inequality (using
    `PointerCoercion::Unsize` as the wrapped value).
15. **`PointerCoercion` -- 4 variants reachable.** Build each;
    assert pairwise distinctness.
16. **`AggregateKind` -- 7 variants reachable.** Already covered
    at item 10.
17. **`AggregateKind::Enum` vs `AggregateKind::Union` -- type-
    system enforced split.** Verify that the variant-tag is
    part of equality: `Enum(TyId(0), VariantIdx(0))` is not
    `==` to `Union(TyId(0), FieldIdx(0))` even when the inner
    `u32` payloads happen to coincide.
18. **`UnOp` -- 3 variants reachable.** Already covered at
    item 8.
19. **`BinOp` -- exhaustive variant coverage via match.** A
    test function with a `match` over `BinOp` covering all 26
    arms (no `_` catch-all) compiles. (Verifies the closed-
    enum discipline at compile time.)
20. **`Rvalue` Hash discrimination.** Insert one of each of
    the 9 variants into a `HashSet`; assert size 9.
21. **Serde round-trip -- `Rvalue` (each variant).** Cover all
    9 variants; assert `==` after round-trip.
22. **Serde round-trip -- `CastKind` (each variant).**
23. **Serde round-trip -- `PointerCoercion` (each variant).**
24. **Serde round-trip -- `BinOp` (each of 26 variants).**
25. **Serde round-trip -- `UnOp` (each variant).**
26. **Serde round-trip -- `AggregateKind` (each variant).**
27. **Reuse confirmation -- `Provenance` / `Mutability` are
    imported from the §10.1 module, not redefined.** A test
    that compares the `Provenance` value used at
    `Rvalue::Ref` construction to a value imported directly
    from the type module's path; assert `==` and the same
    type path. (Mostly a compile-time test; a redefinition
    would type-error at the call site.)

`cargo test -p regseq-model` passes.

## Review

After tests pass, request a Codex review using the four-block
template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-78 against its task spec at
.harness/tasks/regseq-model/78-rvalue-and-sub-enums.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/rvalue.rs
- crates/regseq-model/src/lib.rs

Focus on:
- Spec deviations (every rule in the Implementation Notes section).
- Correctness issues and generality loss.
- Variant counts (Rvalue=9, CastKind=11, PointerCoercion=4,
  BinOp=26, UnOp=3, AggregateKind=7) against the locked spec.
- Whether `Provenance` and `Mutability` are reused (not
  redefined) per spec.md:7383-7396.
- Test coverage vs. the spec's Test Plan.
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs).
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as
hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what was
verified and why any deliberate deviations are acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict,
stop.
</default_follow_through_policy>
EOF
```

Iterate until VERDICT: APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

The authoritative gate is `task.py done` per `SESSION.md`.

## Acceptance Criteria

- New module `crates/regseq-model/src/rvalue.rs` created and
  re-exported from `crates/regseq-model/src/lib.rs`.
- `Rvalue` closed 9-variant enum defined per `spec.md:7157-7204`.
- `CastKind` closed 11-variant enum + `PointerCoercion` closed
  4-variant sub-enum defined per `spec.md:7400-7421`.
- `BinOp` closed 26-variant enum defined per `spec.md:7453-7479`.
- `UnOp` closed 3-variant enum defined per `spec.md:7508-7513`.
- `AggregateKind` closed 7-variant enum defined per
  `spec.md:7526-7567`.
- `Provenance` and `Mutability` are imported from the §10.1
  module (P1a-DS-02 surface), not redefined.
- All public types carry doc comments; per-variant doc
  comments cite the §21.2 payload-shape subsections where
  appropriate.
- Test Plan items 1-27 implemented; all pass under
  `cargo test -p regseq-model`.
- `cargo fmt --all -- --check` and
  `cargo clippy -p regseq-model -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
