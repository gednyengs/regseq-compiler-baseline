# regseq-model-83: Place validator (P1a-VAL-01)

## Description

Implement the Phase B place validator that walks every `Place` reachable
through `Body` content and enforces the five `§19.7` rule categories:
type-coherence narrowing per `§19.4`, head-local existence, `FieldIdx`
range, `VariantIdx` range, and the two structural invariants
`I-Deref-first` and `I-Downcast-followed-by-Field` from `§19.3`. The
validator must run as a single traversal of body content per `§19.7`
"Walk implementation: deferred to stage 11" and report the first
violation it finds with enough location context (Instance, Body, Block,
Statement / Terminator index, projection index) for downstream
diagnostics.

The walk lives on `Body` (per the spec note in `§19.7` lines 6877-6886)
and is invoked by the joint validator from `RegseqModel::validate` over
every Function Instance whose `body: Some(b)` -- per `spec.md:797-799`.
Per-`PlaceElem` predecessor / successor types are derived using the
type-narrowing rules table at `spec.md:6764-6773`; per-variant rejection
errors must distinguish each row so callers can map the violation back
to the invariant it tripped.

## Input Context

- `docs/discussions/regseq-model/spec.md` `§19` (Place and projections),
  in particular `§19.3` (lines 6727-6755), `§19.4` (lines 6757-6804),
  `§19.7` (lines 6835-6886).
- `docs/discussions/regseq-model/spec.md` `§3.3` Phase B step 8 (lines
  788-799) for the joint-validator wiring (Function Instance -> Body
  walk).
- `docs/discussions/regseq-model/spec.md` `§11.1` (`EnumShape` /
  `VariantLayout`) for `VariantIdx` bounds.
- `crates/regseq-model/src/model.rs` -- existing
  `RegseqModel::validate` walk and the `InvariantViolation` enum
  (extend, do not replace).
- The `Place` / `PlaceElem` types from regseq-model-76 (P1a-DS-11), the
  `Body` / `LocalDecl` / `LocalId` / `BasicBlock` / `BlockId` types from
  regseq-model-82 (P1a-DS-14b).
- `Ty` / `Layout` / `EnumShape` / `VariantLayout` definitions from
  regseq-model-67/68 (P1a-DS-02 / P1a-DS-03).
- `archive/.harness/tasks/regseq-model/24-place-and-projection.md` as
  the SHAPE / SIZE reference.

## Implementation Notes

Add a new `place_validator` module under `crates/regseq-model/src/`
(re-exported from `lib.rs`) that exposes a single entry point:

```rust
/// Validates every `Place` reachable from the given `Body` against
/// the §19.7 rule set (type-coherence + structural invariants +
/// FieldIdx / VariantIdx range checks). Returns the first violation
/// in walk order or `Ok(())` if the body is well-formed.
pub fn validate_body_places(
    model: &RegseqModel,
    body: &Body,
) -> Result<(), PlaceValidationError>;
```

`RegseqModel::validate` invokes `validate_body_places` for every
Function Instance whose `kind = Function { body: Some(b), .. }` (the
joint-validation hook from `§3.3` step 8). The walk visits every
`Place` carried by every `Operand`, `Rvalue`, `Statement`, and
`Terminator` payload in every `BasicBlock` of the body (operand-side
recursion comes from regseq-model-77 / -78 / -79 / -80 / -81).

### Walk implementation per `§19.4`

Begin with the head local's `LocalDecl.ty` and active variant `None`.
For each `PlaceElem` in `place.projection`:

1. Apply the row in the `§19.4` table that matches the variant; reject
   if the predecessor `Ty` does not match the row's requirement. Each
   row produces a distinct `PlaceValidationError` variant naming the
   step (e.g., `DerefOnNonPointer`, `FieldOnNonAggregate`,
   `IndexOnNonIndexable`, `ConstantIndexFromEndOnArray`,
   `SubsliceFromEndOnArray`, `DowncastOnNonEnum`).
2. For `Field(idx, fty)`: dispatch on the predecessor type +
   active-variant pair per `spec.md:6845-6863`. Reject when:
   - `idx` is out-of-range for the resolved field list
     (`FieldIdxOutOfRange { actual, max }`).
   - The resolved field type does not equal `fty`
     (`FieldTypeMismatch { expected, actual }`).
3. For `Downcast(idx)`: reject when `idx` is out-of-range for
   `variant_count(predecessor.layout.kind.shape)` per
   `spec.md:6865-6872` (`VariantIdxOutOfRange { actual, max }`).
4. Update `(predecessor_ty, active_variant)` to the row's successor
   pair per the `§19.4` table.

### Structural invariants per `§19.3`

A pre-pass over `place.projection` enforces:

- `I-Deref-first` (per `spec.md:6738-6743`): if any `projection[i] ==
  Deref` and `i != 0`, reject with `DerefNotFirst { index: i }`.
- `I-Downcast-followed-by-Field` (per `spec.md:6745-6752`): if any
  `projection[i] == Downcast(_)` and either
  `i == projection.len() - 1` or
  `!matches!(projection[i + 1], PlaceElem::Field { .. })`, reject with
  `DowncastNotFollowedByField { index: i }`.

The structural pre-pass runs **before** the type-coherence walk so its
errors precede type-narrowing failures; this matches the `§19.7` rule
ordering (rules 1-5).

### Error type

```rust
/// First-violation report for §19.7 place validation.
/// `location` carries the (BlockId, statement_or_terminator,
/// projection-step) coordinates so consumers can pin the offending
/// Place inside the body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceValidationError {
    pub location: PlaceLocation,
    pub kind: PlaceValidationErrorKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceLocation {
    pub block: BlockId,
    pub site: PlaceSite,
    pub projection_step: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaceSite {
    Statement(usize),
    Terminator,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaceValidationErrorKind {
    DerefNotFirst { index: usize },
    DowncastNotFollowedByField { index: usize },
    HeadLocalOutOfRange { local: LocalId, locals_len: usize },
    DerefOnNonPointer { ty: TyId },
    FieldOnNonAggregate { ty: TyId, active_variant: Option<u32> },
    FieldIdxOutOfRange { actual: u32, max: u32 },
    FieldTypeMismatch { expected: TyId, actual: TyId },
    IndexOnNonIndexable { ty: TyId },
    ConstantIndexFromEndOnArray { ty: TyId },
    SubsliceFromEndOnArray { ty: TyId },
    SubsliceFromStartOnSlice { ty: TyId },
    DowncastOnNonEnum { ty: TyId },
    VariantIdxOutOfRange { actual: u32, max: u32 },
}
```

The `RegseqModel::validate` integration extends `InvariantViolation`
with a `PlaceWalk { instance: InstanceId, error: PlaceValidationError }`
variant; the joint validator wraps `validate_body_places` results and
returns on the first failure, preserving the per-instance walk order
already established by `validate_instance` at
`crates/regseq-model/src/model.rs:266-268`.

### Conventions

- Public types and functions carry doc comments; no unicode symbols
  (per `SESSION.md` "Coding Conventions").
- The walk is one pass; do not introduce per-rule infrastructure (per
  `§19.7` "all 5 rules are applied in a single traversal").
- Per the user's "no defensive fallbacks" rule, the validator does not
  guard against rustc-toolchain regressions -- it enforces the
  spec-pinned table verbatim.

## Non-Goals

- Statement / Terminator / BasicBlock structural validation outside
  Place coverage (operand-site invariants, terminator-site invariants,
  etc.) is owned by other phase-1a tasks; VAL-01 enforces only the
  `§19.7` Place rule set.
- ID referential integrity for Items / Ty / Const / FnSig is owned by
  P1a-VAL-02 (regseq-model-84).
- Layout consistency, `opaque_handle_eligible`, and
  `StableCrateId(0)` enforcement are owned by P1a-VAL-03
  (regseq-model-85).
- Bridge translation per `§19.5` (rustc `mir::PlaceElem<'tcx>` ->
  Regseq `PlaceElem`) is owned by P1a-BR-05.
- Pretty-printing of Places is owned by P1a-PP-03.

## Test Plan

Tests live in two tiers per the brief discipline (`unit-in-src` +
`integration-in-tests`). Each invariant the validator enforces gets
**both** a positive test (a model that passes) and a negative test (a
model that violates the invariant and is rejected with the right error
variant). Integration tests under
`crates/regseq-model/tests/place_validator.rs` build constructed-model
fixtures via the existing `with_primary` constructor + push helpers.

### Structural-invariant coverage (`§19.3`)

- `i_deref_first_pass`: a Place whose only `Deref` projection is at
  index 0 -- accepted.
- `i_deref_first_reject`: a Place with `[Field, Deref]` -- rejected
  with `DerefNotFirst { index: 1 }`.
- `i_downcast_followed_by_field_pass`: `[Downcast(0), Field { idx: 0,
  ty: <enum-variant-field-ty> }]` -- accepted.
- `i_downcast_followed_by_field_reject_last`: `[Downcast(0)]` (last
  projection) -- rejected with `DowncastNotFollowedByField { index: 0 }`.
- `i_downcast_followed_by_field_reject_non_field`:
  `[Downcast(0), Deref]` -- rejected with
  `DowncastNotFollowedByField { index: 0 }`.

### Type-narrowing coverage (`§19.4`, one positive + one negative per row)

Each row in the `spec.md:6764-6773` table is exercised with a
constructed `LocalDecl.ty` whose `Ty` matches (positive) or violates
(negative) the predecessor requirement:

- `narrow_local_to_deref_pass` / `_reject`: `Ty::Ref(inner, _)` and
  `Ty::Ptr(inner, _)` accepted; non-pointer rejected with
  `DerefOnNonPointer`.
- `narrow_local_to_field_struct_pass` / `_tuple_pass` /
  `_union_pass` / `_closure_pass` / `_reject_non_aggregate`:
  per-aggregate-variant acceptance; non-aggregate rejected with
  `FieldOnNonAggregate { active_variant: None }`.
- `narrow_local_to_field_after_downcast_pass` /
  `_reject_non_enum_active`: variant-active path accepted on
  `Ty::Enum`; rejected when active variant is `Some(_)` but
  predecessor is not `Ty::Enum`.
- `narrow_local_to_index_array_pass` / `_slice_pass` /
  `_reject_non_indexable`: `Ty::Array` and `Ty::Slice` accepted;
  non-indexable rejected with `IndexOnNonIndexable`.
- `narrow_local_to_constant_index_from_start_pass` /
  `_reject_non_indexable`: positive on `Array` and `Slice`; negative
  on non-indexable.
- `narrow_local_to_constant_index_from_end_pass_slice` /
  `_reject_array`: positive on `Slice`; negative on `Array`
  (`ConstantIndexFromEndOnArray`).
- `narrow_local_to_subslice_from_start_pass_array` /
  `_reject_slice`: positive on `Array`; negative on `Slice`
  (`SubsliceFromStartOnSlice`).
- `narrow_local_to_subslice_from_end_pass_slice` /
  `_reject_array`: positive on `Slice`; negative on `Array`
  (`SubsliceFromEndOnArray`).
- `narrow_local_to_downcast_pass` / `_reject_non_enum`: positive on
  `Ty::Enum`; negative on non-enum (`DowncastOnNonEnum`).

### `FieldIdx` range coverage (`§19.7` rule 3)

- `field_idx_in_range_struct` / `_tuple` / `_union` / `_closure` /
  `_enum_after_downcast`: each per-aggregate dispatch accepts
  in-range indices.
- `field_idx_out_of_range_struct`: `idx == LayoutKind::Struct.fields
  .len()` -- rejected with `FieldIdxOutOfRange { actual, max }`.
- `field_idx_out_of_range_tuple`, `_union`, `_closure`,
  `_enum_after_downcast`: each per-aggregate dispatch rejects with the
  correct (actual, max) pair.
- `field_type_mismatch`: in-range `idx` but wrong `fty` -- rejected
  with `FieldTypeMismatch { expected, actual }`.

### `VariantIdx` range coverage (`§19.7` rule 4)

- `variant_idx_in_range_multiple` / `_single`: accepts
  `Multiple { variants: [..] }` (in-range) and `Single { .. }`
  (`idx == 0`).
- `variant_idx_out_of_range_empty`: `EnumShape::Empty` rejects every
  `Downcast` with `VariantIdxOutOfRange { actual, max: 0 }`.
- `variant_idx_out_of_range_single`: `Single { .. }` with `idx == 1`
  -- rejected.
- `variant_idx_out_of_range_multiple`: `Multiple { variants:
  [v0, v1] }` with `idx == 2` -- rejected.

### Walk-level coverage (`§19.7` rule 2 + multi-block traversal)

- `multi_block_body_pass`: a `Body` with two `BasicBlock`s, each
  carrying a Statement::Assign whose Rvalue references a Place; both
  blocks pass.
- `head_local_out_of_range_reject`: a Place whose `LocalId` is
  beyond `body.local_decls.len()` -- rejected with
  `HeadLocalOutOfRange { local, locals_len }`.
- `bad_place_mid_block_reject`: a Body whose third Statement in the
  second block carries a malformed Place; the validator rejects with
  the violation pinned to
  `PlaceLocation { block: BlockId(1), site: PlaceSite::Statement(2),
  .. }`.
- `terminator_place_reject`: a Body whose Terminator payload (e.g.,
  `Call.destination`) carries a malformed Place; the location pins
  `site: PlaceSite::Terminator`.
- `joint_validator_integration_pass`: a `RegseqModel` containing a
  Function Instance with `body: Some(b)` whose `b` is well-formed;
  `RegseqModel::validate` returns `Ok(())`.
- `joint_validator_integration_reject`: same shape but with one bad
  Place in the body; `RegseqModel::validate` returns
  `Err(InvariantViolation::PlaceWalk { instance, error })` whose
  `error.location` matches the expected coordinates.

### Quality gates

- `cargo test -p regseq-model` passes (unit + integration tiers
  above).
- All public types and functions carry doc comments.

## Review

After tests pass, run a Codex review using the `codex_runner.py`
four-block template per `.harness/SESSION.md` Review section:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-83 (P1a-VAL-01: Place
validator) against its task spec at
.harness/tasks/regseq-model/83-place-validator.md. ALL CHECKS MUST BE
SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/place_validator.rs (or wherever the new
  validator module lands)
- crates/regseq-model/src/model.rs (joint-validator wiring + new
  InvariantViolation::PlaceWalk variant)
- crates/regseq-model/src/lib.rs (re-exports)
- crates/regseq-model/tests/place_validator.rs

Focus on:
- Spec deviations against spec.md §19.3, §19.4, §19.7 (every rule and
  every row of the type-narrowing table at spec.md:6764-6773).
- Single-traversal discipline (§19.7 lines 6877-6886): the walk must
  apply all 5 rule categories in one pass.
- Joint-validator wiring against spec.md:788-799 (Function Instance
  with body: Some(b) -> body walk).
- Per-variant error coverage: every rejection cited in the Test Plan
  has a distinct error variant.
- Test Plan coverage: each invariant has both a positive and a
  negative test.
Do NOT flag anything explicitly marked out of scope (Non-Goals, later
task IDs).
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what was verified
and why any deliberate deviations are acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict, stop.
</default_follow_through_policy>
EOF
```

Iterate `fix -> codex_runner.py review` until VERDICT: APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `crates/regseq-model/src/place_validator.rs` (or equivalent module)
  created with the public `validate_body_places` entry point and the
  `PlaceValidationError` / `PlaceLocation` / `PlaceSite` /
  `PlaceValidationErrorKind` types.
- `RegseqModel::validate` invokes the walk per Function Instance with
  `body: Some(b)` per `spec.md:788-799` and surfaces failures via a
  new `InvariantViolation::PlaceWalk { instance, error }` variant.
- All §19.3 / §19.4 / §19.7 rules are enforced in a single traversal
  per `spec.md:6877-6886`.
- Test Plan covers each invariant with both positive and negative
  cases against constructed-model fixtures.
- `cargo test -p regseq-model` passes; Codex Task Review verdict is
  APPROVED.
