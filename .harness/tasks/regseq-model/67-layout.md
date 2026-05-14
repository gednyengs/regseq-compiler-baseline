# regseq-model-67: Layout + EnumShape + TagScalar + `UNIT_LAYOUT` + `size_align_of`

## Description

Land the resolved-`Layout` machinery from spec.md §11
(spec.md:3800-4113): the `Layout` struct + `LayoutKind` enum +
`FieldLayout` + `EnumShape` (3 variants) + `VariantLayout` +
`TagScalar` + `TagEncoding` (Direct + Niche) + `WrappingRange` +
`VariantRange` + the canonical `UNIT_LAYOUT` constant + the
`size_align_of(ty: TyId) -> SizeAlign` query method on
`RegseqModel`. Layout values are inline on `Ty::Tuple` /
`Ty::Struct` / `Ty::Enum` / `Ty::Union` (per P1a-DS-02), and the
bridge does **not** recompute layout -- rustc owns the algorithm,
RegseqModel owns preserving the answer (spec.md:3802-3804).

This task carries the **field and variant indexing conventions**
(§11.0), the **tag-scalar policy** (§11.2: logical-scalar-type rule
+ niche tag-value formula + width-fit invariant +
storage-lvalue rule for invalid-bit-pattern niches), the
**signed-discriminant interpretation** (§11.1), and the
**`size_align_of` per-variant rules** (§11.5).

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (this task is
  P1a-DS-03)
- `docs/discussions/regseq-model/spec.md` §11 (spec.md:3800-4113)
- `crates/regseq-model/src/ty.rs` (P1a-DS-02 -- `Ty` enum +
  `IntKind` / `UintKind` / `FloatKind` + auxiliary enums)
- `crates/regseq-model/src/model.rs` (P1a-DS-01 -- `RegseqModel`
  root, `TargetInfo`, `ScalarLayouts`, `SizeAlign`)
- `crates/regseq-model/src/ids.rs` (`VariantIdx`, `TyId`, `StringId`)
- `.harness/tasks/regseq-model/65-root-regseq-model-and-foundation.md`
- `.harness/tasks/regseq-model/66-ty-enum-and-helpers.md`

## Implementation Notes

Create a new module `crates/regseq-model/src/layout.rs` (or
`layout/mod.rs` if splitting reads better). Re-export from `lib.rs`.

### Public API (verbatim from spec)

#### `Layout` core (spec.md:3807-3826)

```rust
pub struct Layout {
    pub size:        u64,
    pub align:       u64,
    pub pack:        Option<u32>,   // None = not packed; Some(N) = repr(packed(N))
    pub uninhabited: bool,          // Result<T, !>, enum E {}, struct S(!), etc.
    pub kind:        LayoutKind,
}

pub enum LayoutKind {
    Struct { fields: Vec<FieldLayout> },   // for Ty::Struct AND Ty::Tuple
    Union  { fields: Vec<FieldLayout> },   // for Ty::Union (offsets all 0)
    Enum   { shape:  EnumShape       },    // for Ty::Enum
}

pub struct FieldLayout {
    pub name:   StringId,    // interned via RegseqModel.strings
    pub ty:     TyId,
    pub offset: u64,         // see §11.0 for offset reference frame
}
```

#### `UNIT_LAYOUT` constant (spec.md:3841-3848)

```rust
pub const UNIT_LAYOUT: Layout = Layout {
    size:        0,
    align:       1,
    pack:        None,
    uninhabited: false,
    kind:        LayoutKind::Struct { fields: Vec::new() },
};
```

Note: `Vec::new()` is a `const fn`; the constant is constructible
in `const` context.

#### `EnumShape` and `VariantLayout` (spec.md:3886-3913)

```rust
pub enum EnumShape {
    /// Zero-variant uninhabited enum (`enum Never {}`).
    Empty,

    /// Single-variant enum.
    Single { variant: VariantLayout, scalar: Option<TagScalar> },

    Multiple {
        tag:      TagScalar,
        encoding: TagEncoding,
        variants: Vec<VariantLayout>,
    },
}

pub struct VariantLayout {
    pub name:         StringId,
    pub fields:       Vec<FieldLayout>,
    pub uninhabited:  bool,
    pub discriminant: u128,
    pub tag_value:    Option<u128>,
}
```

`tag_value` semantics per spec.md:3916-3925 (5-row table). Document
the table on the `tag_value` field.

#### `TagScalar`, `TagEncoding`, `VariantRange`, `WrappingRange` (spec.md:3942-3969)

```rust
pub struct TagScalar {
    pub offset:      u64,
    pub scalar_ty:   TyId,            // logical scalar type — see §11.2 policy
    pub valid_range: WrappingRange,   // always present
}

pub enum TagEncoding {
    Direct,
    Niche {
        untagged_variant: VariantIdx,
        niche_variants:   VariantRange,
        niche_start:      u128,
    },
}

pub struct VariantRange {
    pub start: VariantIdx,
    pub end:   VariantIdx,    // inclusive
}

pub struct WrappingRange {
    pub start: u128,
    pub end:   u128,                  // inclusive; wraps if start > end
}
```

#### `size_align_of` query (spec.md:4080-4083)

```rust
impl RegseqModel {
    pub fn size_align_of(&self, ty: TyId) -> SizeAlign;
}
```

Per-variant rules per the table at spec.md:4087-4101 (verbatim):

| Variant                                              | Rule                                                                                       |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `Struct`/`Enum`/`Union`/`Tuple`                      | read inline `Layout.size` and `Layout.align`                                               |
| `Array(elem, n)`                                     | `stride(elem) * n`, `align(elem)`. Stride: `round_up_to_multiple(size: size(elem), boundary: align(elem))`. Validation: `checked_mul(stride, n)` must not overflow `u64`; product must not exceed target-pointer-width `usize::MAX`. |
| `Ref(t, _)` / `Ptr(t, _)` to **sized** pointee       | read `target.scalar_layouts.data_ptr` -- thin pointer                                      |
| `Ref(t, _)` / `Ptr(t, _)` to **unsized** pointee     | `(2 * data_ptr.size, data_ptr.align)` -- fat pointer (two slots)                           |
| `FnPtr(_)`                                           | read `target.scalar_layouts.fn_ptr`                                                        |
| `FnDef(_, _)`                                        | `(0, 1)` -- ZST                                                                            |
| `Bool`                                               | read `target.scalar_layouts.bool_`                                                         |
| `Char`                                               | read `target.scalar_layouts.char_`                                                         |
| `Int(k)`                                             | read `target.scalar_layouts.int[k as usize]`                                               |
| `Uint(k)`                                            | read `target.scalar_layouts.uint[k as usize]`                                              |
| `Float(k)`                                           | read `target.scalar_layouts.float[k as usize]`                                             |
| `Never`                                              | `(0, 1)` -- uninhabited                                                                    |
| `Str`, `Slice(_)`, `Dynamic{..}` (queried directly)  | **panic / debug-assert** -- unsized markers must only be queried via the enclosing `Ref`/`Ptr` |

"Sized pointee" / "unsized pointee" classification per
spec.md:4103-4105: pointee is unsized iff it is `Str`, `Slice(_)`, or
`Dynamic{..}`. All other types (including `Array(_, _)` and any sized
ADT) are sized.

The implementation is straightforward dispatch per `Ty` variant.
Helper functions:

- `fn round_up_to_multiple(value: u64, boundary: u64) -> u64` --
  used by Array stride.
- For Array, perform `checked_mul` and overflow-bounds check
  (target pointer width); document panic conditions on the public
  API.

### Conventions

- **Serde derive**: `Layout` / `LayoutKind` / `FieldLayout` /
  `EnumShape` / `VariantLayout` / `TagScalar` / `TagEncoding` /
  `VariantRange` / `WrappingRange` derive `Serialize` /
  `Deserialize` per the cross-cutting note (these types contain
  `StringId` / `TyId` / `VariantIdx`; per the brief's
  `crates/regseq-model/src/placeholders.rs` precedent, types
  containing `Symbol` defer serde -- but `StringId` is *not*
  `Symbol`. The relevant rule is "types containing `Symbol`/`StringId`
  are deferred from serde until resolution-time pass lands"; here
  `Layout` carries `StringId` via `FieldLayout.name` and
  `VariantLayout.name`, so per the brief's cross-cutting note this
  task **defers serde derives on Layout** until the resolution-time
  pass. Document this with a `// SERDE: deferred per phase-1a brief
  cross-cutting note` comment).

  EXCEPTION: P1a-DS-02 + P1a-DS-04 both require `Ty` to derive
  serde, and `Ty` carries `Layout` inline. Since `Ty` ships serde
  in DS-02, `Layout` MUST also ship serde at this task -- otherwise
  the parent type doesn't compile. Reconcile by deriving serde here
  unconditionally and letting the resolution-time pass refine the
  on-wire shape later. Document that the derived serde is
  "construction shape" pending resolution-time refinement.

- **Derive set**: `Clone, Debug, PartialEq, Eq, Hash` on `Layout`,
  `LayoutKind`, `FieldLayout`, `EnumShape`, `VariantLayout`,
  `TagScalar`, `TagEncoding`, `VariantRange`, `WrappingRange` (full-
  value equality is what spec §10's full-value `Ty` `Eq` /
  `PartialEq` / `Hash` derives over, per spec.md:4127-4137); plus
  `Serialize, Deserialize` per the EXCEPTION above.
- **Doc comments on every public type / variant / function**.
- **No unicode in comments / docs**.
- **Closed enum discipline** on `LayoutKind` and `TagEncoding`.

### Variant invariants (documentation, validator-enforced later)

Per spec.md:3850-3854 the empty tuple's `UNIT_LAYOUT` is the only
admissible layout for `Ty::Tuple { fields: [] }`. Document this on
the constant. Enforcement at P1a-VAL-03.

Per spec.md:3971-3975 `TagScalar.valid_range` is always present
(bridge populates from rustc's `Scalar::Initialized`); rejection of
`Scalar::Union` is at the bridge (P1a-BR-02 / VAL-03).

Per spec.md:3977-3986 `TagScalar.scalar_ty` is the **logical**
rustc-resolved tag type (admissible variants: `Bool`, `Char`,
`Int(_)`, `Uint(_)`, `Ptr(_, _)`, `Ref(_, _)`; `Float` is **not**
admitted). Document this on the field.

Per spec.md:3988-4014 niche tag-value formula is deterministic;
document the formula in a doc-comment on `TagEncoding::Niche`.

Per spec.md:4026-4033 width-fit invariant: every
`valid_range.{start,end}`, `niche_start`, and per-`VariantLayout.tag_value`
fits the bit width of `TagScalar.scalar_ty`. Document on
`TagScalar` and `WrappingRange`. Validator enforcement at
P1a-VAL-03.

## Non-Goals

- **No `Layout` validator** -- P1a-VAL-03 owns invariant-14 / 19 / 20
  enforcement, the §11.5 layout-sanity walks, the
  `Ty::Tuple { fields: [] } -> UNIT_LAYOUT` check, the tag-scalar
  width-fit walk, and packed-alignment formula correctness.
- **No bridge translation** -- P1a-BR-02 owns layout extraction
  from rustc's `tcx.layout_of(ty)`.
- **No `BackendRepr` / `max_repr_align` fields** -- per spec.md:4058-4065
  these are dropped from rustc's `LayoutData`.
- **No per-target C struct rendering** -- backend territory; phase-1b.
- **No vtable layout algorithm** -- P1a-DS-08b / P1a-BR-04 own the
  synthesised vtable Layout per spec.md:4680-4742.
- **No `EnumShape::Multiple` validator** -- P1a-VAL-03.
- **No pretty-printer** -- P1a-PP-* axis.

## Test Plan

`unit-in-src` discipline. All tests live in
`crates/regseq-model/src/layout.rs::tests`. Per `SESSION.md` Testing
Requirements, every public type / function / constant gets at least
one test.

### Constructor tests

- `Layout { size: 4, align: 4, pack: None, uninhabited: false,
  kind: LayoutKind::Struct { fields: vec![] } }` constructs.
- Each `LayoutKind` variant constructs (`Struct` / `Union` / `Enum`).
- `FieldLayout { name, ty, offset }` constructs and round-trips
  through serde.
- Each `EnumShape` variant constructs:
  - `EnumShape::Empty` (zero-variant).
  - `EnumShape::Single { variant, scalar: Some(_) }` (fieldless
    `repr(int)` enum); `EnumShape::Single { variant, scalar: None }`
    (ordinary single-variant enum).
  - `EnumShape::Multiple { tag, encoding: TagEncoding::Direct,
    variants }`; same with `TagEncoding::Niche { ... }`.
- `VariantLayout { name, fields, uninhabited, discriminant, tag_value }`
  constructs with both `Some(v)` and `None` for `tag_value`.
- `TagScalar { offset, scalar_ty, valid_range }` constructs.
- `VariantRange { start, end }` constructs (verify inclusive `end`).
- `WrappingRange { start: 5, end: 2 }` constructs the wrapping case
  (start > end).

### `UNIT_LAYOUT` constant

- `UNIT_LAYOUT.size == 0`, `UNIT_LAYOUT.align == 1`,
  `UNIT_LAYOUT.pack == None`, `UNIT_LAYOUT.uninhabited == false`.
- `UNIT_LAYOUT.kind` matches `LayoutKind::Struct { fields }` with
  `fields.is_empty()`.
- The constant is usable in const context (e.g., as the layout of a
  hand-constructed `Ty::Tuple { fields: vec![], layout: UNIT_LAYOUT }`).
- Direct equality: `UNIT_LAYOUT == UNIT_LAYOUT`.

### Equality and hash

- Two `FieldLayout`s with identical content are equal and hash
  equal.
- Two `Layout`s with identical content are equal and hash equal.
- A `Layout` differing in `size` only is not equal to its sibling.
- A `Layout` differing in `pack` (`None` vs `Some(2)`) is not equal.

### Serde round-trip

- `Layout`, `LayoutKind` (each variant), `FieldLayout`, `EnumShape`
  (each variant), `VariantLayout`, `TagScalar`, `TagEncoding`
  (each variant), `VariantRange`, `WrappingRange` round-trip
  losslessly through `serde_json` per the construction-shape
  derive.

### `size_align_of` per-variant tests

Build a fixture `RegseqModel` whose `target.scalar_layouts` is
populated with deterministic per-scalar `SizeAlign` values, then
intern test `Ty`s and assert `size_align_of` matches:

- **Primitives**: every `IntKind` / `UintKind` / `FloatKind`
  variant returns `target.scalar_layouts.{int,uint,float}[k as usize]`.
- **`Bool`** returns `target.scalar_layouts.bool_`.
- **`Char`** returns `target.scalar_layouts.char_`.
- **`FnPtr(_)`** returns `target.scalar_layouts.fn_ptr`.
- **`FnDef(_, _)`** returns `(0, 1)`.
- **`Never`** returns `(0, 1)`.
- **`Ref(sized_ty, _)`** and **`Ptr(sized_ty, _)`** return
  `target.scalar_layouts.data_ptr` (test with both `Provenance`
  variants and both `Mutability` variants).
- **`Ref(unsized_ty, _)`** and **`Ptr(unsized_ty, _)`**: with
  pointee in `{Str, Slice(_), Dynamic{..}}`, return
  `(2 * data_ptr.size, data_ptr.align)`.
- **`Array(elem, n)`**: with `elem.size = 4, elem.align = 4, n = 10`,
  returns `(40, 4)`. With `elem.size = 1, elem.align = 1, n = 5`,
  returns `(5, 1)`. With non-power-of-two element size +
  alignment forcing stride padding, verify
  `stride = round_up_to_multiple(size, boundary)`.
- **`Array` overflow**: `Array(elem, u64::MAX)` with non-zero
  element stride should detect `checked_mul` overflow (panic or
  documented behavior); test with a fixture where
  `stride * n > u64::MAX` and assert the panic / documented error.
- **`Struct`/`Enum`/`Union`/`Tuple`**: return inline
  `Layout.size`, `Layout.align` from the variant's payload.
- **Direct query of unsized markers** (`size_align_of(model,
  str_ty)` / `size_align_of(model, slice_ty)` /
  `size_align_of(model, dynamic_ty)`) panics or debug-asserts
  per the rule (#[should_panic] tests).

### `round_up_to_multiple` helper

If exposed publicly, test:

- `round_up_to_multiple(0, 4) == 0`.
- `round_up_to_multiple(1, 4) == 4`.
- `round_up_to_multiple(4, 4) == 4`.
- `round_up_to_multiple(5, 4) == 8`.
- `round_up_to_multiple(7, 8) == 8`.
- `round_up_to_multiple(8, 8) == 8`.

If kept private, the per-`Array` stride tests above cover it
indirectly.

### Closed-enum exhaustiveness scaffold

`LayoutKind`, `EnumShape`, `TagEncoding` each get a hidden
`_exhaustiveness_check(_: &Foo)` function with an exhaustive
`match` (no wildcard arm), so a future variant addition surfaces
as a compile error.

## Review

After tests pass, request a Codex Task Review per `SESSION.md`
Review section. Use `scripts/python/codex_runner.py` directly via
the Bash tool with the four-block template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-67 against its task spec at
.harness/tasks/regseq-model/67-layout.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/layout.rs (or layout/*.rs)
- crates/regseq-model/src/lib.rs (re-exports)
- crates/regseq-model/src/model.rs (`size_align_of` impl
  attaches to RegseqModel)

Focus on:
- Spec deviations (every rule in the Implementation Notes section,
  including the spec-quoted public-API signatures: `Layout`,
  `LayoutKind`, `FieldLayout`, `EnumShape`, `VariantLayout`,
  `TagScalar`, `TagEncoding`, `VariantRange`, `WrappingRange`,
  `UNIT_LAYOUT`, `size_align_of`).
- `UNIT_LAYOUT` constant: every field matches spec.md:3841-3848
  exactly; constructible in const context.
- `size_align_of` per-variant rules match spec.md:4087-4101 verbatim
  including Array stride formula, fat-pointer two-slot rule, ZST
  rules for `FnDef` and `Never`, panic / debug-assert on direct
  unsized-marker query.
- Closed-enum discipline on `LayoutKind`, `EnumShape`, `TagEncoding`
  (no `#[non_exhaustive]`, no wildcard arms).
- Doc comments on every variant; `tag_value` 5-row semantics
  documented; `TagScalar.scalar_ty` admissible-variants rule
  documented (Bool / Char / Int / Uint / Ptr / Ref; not Float).
- Test coverage vs the task's Test Plan (constructors,
  `UNIT_LAYOUT`, equality / hash, serde round-trip, every
  `size_align_of` variant including overflow on Array, helper
  formula, exhaustiveness scaffolds).
Do NOT flag anything explicitly marked out of scope (Non-Goals:
P1a-VAL-03 owns Layout validators; P1a-BR-02 owns bridge extraction
from `tcx.layout_of`; P1a-DS-08b / BR-04 own vtable Layout
algorithm; phase-1b owns C struct rendering).
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

## Acceptance Criteria

- `Layout`, `LayoutKind`, `FieldLayout`, `EnumShape`,
  `VariantLayout`, `TagScalar`, `TagEncoding`, `VariantRange`,
  `WrappingRange` all land per spec.md:3807-3969.
- `UNIT_LAYOUT` constant lands per spec.md:3841-3848 and is
  constructible in `const` context.
- `RegseqModel::size_align_of(&self, ty: TyId) -> SizeAlign` lands
  with per-variant dispatch matching spec.md:4087-4101.
- All types ship doc comments; field-level invariants (tag_value
  semantics, scalar_ty admissibility, niche tag-value formula,
  width-fit) documented.
- Serde derive on every Layout type per the construction-shape
  EXCEPTION above.
- Closed-enum exhaustiveness scaffolds present for `LayoutKind`,
  `EnumShape`, `TagEncoding`.
- All tests pass (`cargo test -p regseq-model`).
- Codex Task Review verdict: APPROVED.
