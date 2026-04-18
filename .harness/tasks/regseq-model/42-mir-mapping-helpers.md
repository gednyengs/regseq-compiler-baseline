# regseq-model-42: rustc MIR Mapping Helpers (Type, Operand, Place, Rvalue)

## Description

Implement the leaf rustc-MIR-to-Regseq-IR mapping helpers used by
Pass 2 body construction (task 43+). These helpers translate
**live rustc MIR** values into Regseq IR constructs, given a
`LocalIdMap` and the indexes built in task 41.

Inputs are rustc types directly (`ty::Ty<'tcx>`,
`mir::Operand<'tcx>`, `mir::Place<'tcx>`, `mir::Rvalue<'tcx>`,
`ty::Const`). The legacy owned `Mir*` types in
`crates/regseq-model/src/declaration/mir_body.rs` are NOT used --
they are scaffolding Phase 9 deletes.

### Index types

`TypeDefIndex` and `ConstantIndex` below resolve to the maps in
`construction::CrossRefMaps` (populated by task 41's
`build_pass1_output`):

- `TypeDefIndex` is `&HashMap<DefId, TypeDefId>` -- use
  `cross_refs.type_defs` directly or define a transparent newtype
  wrapper for clarity.
- `ConstantIndex` is `&HashMap<DefId, ConstantId>`. Task 41 does not
  populate this map; **this task extends `CrossRefMaps` with a
  `pub constants: HashMap<DefId, ConstantId>` field and has
  `walk::build_constant` / `walk::build_associated_constant`
  populate it.** Pass 2 callers then read it through
  `cross_refs.constants`.

## Input Context

- `docs/discussions/models/regseq-model/ir-reference.md` (Sections
  1-3 Operand/Place/Rvalue with full MIR mapping tables; Section 8
  type construction + symbol interning)
- `docs/discussions/models/regseq-model/spec.md` (Section 7
  identity scheme, Section 8 type system + `Type construction
  from MIR` table)
- `crates/regseq-model/src/ir/place.rs`, `operand.rs`, `rvalue.rs`,
  `types.rs`, `ops.rs` (Phase 0 + 1 leaf types)
- `crates/regseq-model/src/identity/symbol.rs`
- `crates/regseq-driver/src/construction.rs` (from task 41)
- rustc-side references (for `use` statements):
  `rustc_middle::ty`, `rustc_middle::mir`,
  `rustc_middle::ty::TyCtxt`.

## Implementation Notes

### Pass 2 sub-module scaffolding (owned by this task)

To unblock parallel execution of tasks 43-53, pre-create the
full Pass 2 sub-module layout under
`crates/regseq-driver/src/construction/` so each later task
owns a dedicated file with no cross-task merge surface.

Create these empty stub files, each carrying only a module-doc
comment naming its owning task id. Business logic lands in the
owning task; this task does not write non-`mir_mapping` logic.

| File                                       | Future owner |
|--------------------------------------------|---|
| `construction/mir_mapping.rs`              | this task 42 |
| `construction/pass2.rs`                    | task 43 (Pass 2 body construction) |
| `construction/switch_and_loops.rs`         | task 44 (SwitchInt + loops) |
| `construction/intrinsics.rs`               | task 45 (Rule 1) |
| `construction/try_folding.rs`              | task 46 (Rule 2 `?`) |
| `construction/cross_crate.rs`              | task 47 (Rule 3) |
| `construction/deref_collapse.rs`           | task 48 (Rule 4) |
| `construction/ordered_region.rs`           | task 49 (Rule 5) |
| `construction/log_metadata.rs`             | task 50 (Rule 6) |
| `construction/register_metadata.rs`        | task 51 (Rule 7 + modify body) |
| `construction/pinctrl_metadata.rs`         | task 52 (PinCtrl metadata) |
| `construction/module_deps.rs`              | task 53 (module dep-sets) |

Add the corresponding `pub(crate) mod <name>;` declarations to
`crates/regseq-driver/src/construction.rs` (mod root). A minimal
stub file looks like:

```rust
//! <one-line description>
//!
//! Implemented by task regseq-model-4N.
```

Each stub compiles clean with no warnings. No `pub` items are
exported yet; the owning task adds its public API.

### `mir_mapping` helpers (this task's real deliverable)

The live mapping helpers below land in
`construction/mir_mapping.rs`. Every helper input is a live
rustc value; no owned `Mir*` mirror types appear.

### Type mapping

```rust
pub fn map_ty<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: ty::Ty<'tcx>,
    type_def_index: &TypeDefIndex,
) -> Result<RegseqType, ConstructionError>;
```

`TypeDefIndex` (introduced in task 41) maps rustc `DefId` →
`TypeDefId` for every type registered by Pass 1.

Cover every `RegseqType` variant via `ty.kind()`:
- `ty::Uint(_)` / `ty::Int(_)` → `Scalar(ScalarType)` (incl.
  `Usize`/`Isize`).
- `ty::Bool` → `Bool`.
- `ty::Tuple(tys)` where `tys.is_empty()` → `Unit`.
- `ty::Never` → `Never`.
- `ty::Adt(adt_def, args)` where `adt_def` is the Option lang
  item → `Option(map(args[0]))`.
- `ty::Adt(adt_def, args)` where `adt_def` is the Result lang
  item → `Result(map(args[0]), map(args[1]))`.
- `ty::Array(elem, n)` → `Array(map(elem), n)`.
- `ty::Ref(_, inner, _)` where `inner.kind()` is `ty::Slice(t)`
  → `Slice(map(t))`.
- `ty::Ref(_, inner, mutability)` otherwise → `Ref(map(inner),
  mutability)`.
- `ty::Adt(adt_def, _)` otherwise → `Defined(type_def_index.lookup(adt_def.did())?)`.

Hard errors:
- `ConstructionError::StdTypeNotPermitted` when `adt_def.did()`
  resolves to a `std::*` path (use `tcx.crate_name(krate)` or
  `tcx.def_path_str(def_id)` to detect).
- `ConstructionError::UnsupportedRustType` when `ty.kind()` is
  something the table doesn't cover (e.g., `ty::Dynamic`,
  `ty::Closure`, `ty::Coroutine`, `ty::FnPtr`, `ty::RawPtr` --
  drivers don't use these).

### LiteralValue construction

```rust
pub fn map_const<'tcx>(
    tcx: TyCtxt<'tcx>,
    c: ty::Const<'tcx>,
    expected_ty: ty::Ty<'tcx>,
) -> Result<LiteralValue, ConstructionError>;
```

Evaluate `c` to a `ConstValue::Scalar` via
`tcx.const_eval_poly(...)` (or the appropriate query) and
dispatch by `expected_ty`:
`U8/U16/U32/U64`, `I8/I16/I32/I64`, `Usize`, `Isize`, `Bool`.
`Usize`/`Isize` use 64-bit backing storage.

Non-scalar constants → `ConstructionError::UnsupportedConstShape`.

### Operand mapping

```rust
pub fn map_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    op: &mir::Operand<'tcx>,
    locals: &LocalIdMap,
    constants: &ConstantIndex,
    type_def_index: &TypeDefIndex,
    interner: &mut SymbolInterner,
) -> Result<MapOperandResult, ConstructionError>;

pub enum MapOperandResult {
    Mapped(Operand),
    /// Caller drops this operand entirely (e.g., RuntimeChecks).
    Drop,
}
```

- `mir::Operand::Copy(place)` → `Operand::Copy(map_place(...))`.
- `mir::Operand::Move(place)` → `Operand::Move(map_place(...))`.
- `mir::Operand::Constant(c)` where `c` is `Const::Unevaluated` →
  `Operand::Constant(ConstantId)` via `ConstantIndex` lookup on
  the unevaluated constant's `DefId`.
- `mir::Operand::Constant(c)` where `c` evaluates to a scalar →
  `Operand::Literal(map_const(...))`.
- The rustc MIR operand has no `RuntimeChecks` variant in current
  nightly, but the mapping accounts for any future
  compilation-flag-query shape by routing it to
  `MapOperandResult::Drop`.

`ConstantIndex` (introduced in task 41) maps rustc `DefId` →
`ConstantId`.

### Place + Projection mapping

```rust
pub fn map_place<'tcx>(
    tcx: TyCtxt<'tcx>,
    place: &mir::Place<'tcx>,
    locals: &LocalIdMap,
    type_def_index: &TypeDefIndex,
    interner: &mut SymbolInterner,
) -> Result<Place, ConstructionError>;
```

For each `mir::ProjectionElem`:
- `Deref` → `Projection::Deref`.
- `Field(idx, ty)` → `Projection::Field { index, name, ty }`
  with `name` resolved from the parent type's field-name table
  (`ty::AdtDef::variant(..).fields[idx].name`) and interned.
  `None` for tuple-struct positional fields.
- `Index(local)` → `Projection::Index(local)`.
- `ConstantIndex { offset, min_length, from_end }` → direct.
- `Subslice { from, to, from_end }` → direct.
- `Downcast(name_opt, variant_idx)` → `Projection::Downcast
  { name, variant_index }` with name interned from
  `adt_def.variant(variant_idx).name`.
- `OpaqueCast(_)`, `UnwrapUnsafeBinder(_)` →
  `ConstructionError::UnsupportedProjection`.

### Rvalue mapping

```rust
pub fn map_rvalue<'tcx>(
    tcx: TyCtxt<'tcx>,
    rv: &mir::Rvalue<'tcx>,
    locals: &LocalIdMap,
    type_def_index: &TypeDefIndex,
    constants: &ConstantIndex,
    interner: &mut SymbolInterner,
) -> Result<Rvalue, ConstructionError>;
```

Cover every Regseq IR `Rvalue` variant:
- `mir::Rvalue::Use(op)` → `Use(Operand)`.
- `mir::Rvalue::Repeat(op, ct)` → `Repeat(Operand, usize)`
  (evaluate `ct` to a concrete `usize`).
- `mir::Rvalue::Ref(_, borrow_kind, place)` → `Ref(BorrowKind,
  Place)` (BorrowKind reduced to Shared/Mutable; drop `Fake` and
  two-phase variants).
- `mir::Rvalue::Cast(cast_kind, op, ty)` → `Cast(CastKind,
  Operand, RegseqType)` (CastKind reduced to the 6 variants in
  ir-reference.md Section 3; drop
  `PointerExposeProvenance`/`PointerWithExposedProvenance`/
  `Subtype`/`FnPtrToPtr`; map `PointerCoercion` → `PtrToPtr`).
- `mir::Rvalue::BinaryOp(op, ops)` → `BinaryOp(BinOp, Operand,
  Operand)`.
- `mir::Rvalue::UnaryOp(op, operand)` → `UnaryOp(UnOp, Operand)`
  (drop `PtrMetadata`).
- `mir::Rvalue::Discriminant(place)` → `Discriminant(Place)`.
- `mir::Rvalue::Aggregate(kind, ops)` → `Aggregate(AggregateKind,
  Vec<Operand>)` (AggregateKind reduced to Array/Tuple/Adt;
  drop Closure/Coroutine/RawPtr).
- `mir::Rvalue::CopyForDeref(place)` → `Use(Operand::Copy(place))`
  (semantically equivalent).
- `mir::Rvalue::ThreadLocalRef`, `RawPtr`, `ShallowInitBox`,
  `WrapUnsafeBinder` → `ConstructionError::UnsupportedRvalue`.

### Test surface

Helpers are pure over rustc MIR fragments + indexes. Tests invoke
them via a small rustc harness under
`crates/regseq-driver/tests/` that compiles tiny fixture crates
and asserts helper output for each variant. No hand-built
`Mir*` owned values.

## Non-Goals

- No statement, terminator, or body-level construction (task 43).
- No recognition rules (tasks 45-52).
- No body source map (task 43).
- No use of the legacy owned `Mir*` types.

## Test Plan

Under `crates/regseq-driver/tests/` add a mapping-helpers
integration test that compiles a small fixture crate and drives
each helper against its MIR/Ty shapes:

- Every `RegseqType` variant exercised (including
  `Option`/`Result`/array/slice/ref/Defined).
- Every literal width exercised (including `Usize`/`Isize`).
- Every `Operand` variant exercised (Copy, Move,
  unevaluated-Constant → `Operand::Constant`, evaluated-Constant
  → `Operand::Literal`).
- Every `Projection` variant exercised (including `Field` name
  resolution and `Downcast` variant name).
- Every `Rvalue` variant exercised (including `CopyForDeref` →
  `Use(Copy)` and dropped variants).
- Every error variant exercised: `UnsupportedRustType`,
  `StdTypeNotPermitted` (reference a `std::*` type in a fixture),
  `UnsupportedConstShape`, `UnsupportedProjection`,
  `UnsupportedRvalue`.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to verify the helpers cover
every row of the MIR mapping tables in ir-reference.md Sections
1-3 against **live rustc types**, the type-construction table in
spec.md Section 8, and `TypeDefIndex`/`ConstantIndex` lookup keys
are rustc `DefId`s (not strings). Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- All 12 Pass 2 sub-module stub files exist under
  `crates/regseq-driver/src/construction/` (list in
  Implementation Notes), each compiling clean with no
  warnings, all declared as `pub(crate) mod <name>;` in
  `construction.rs`.
- `map_ty`, `map_const`, `map_operand`, `map_place`, `map_rvalue`
  implemented in `construction/mir_mapping.rs` against live
  rustc types.
- `TypeDefIndex` and `ConstantIndex` keyed by rustc `DefId`.
- Every Regseq IR variant from `RegseqType`, `LiteralValue`,
  `Operand`, `Place`/`Projection`, and `Rvalue` produced by at
  least one passing test.
- `ConstructionError` extended with new variants and each
  exercised by a negative test.
- No import of `regseq_model::declaration::mir_body::*`.
- Codex review approved.
