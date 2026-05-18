# regseq-driver-54: Bridge `Ty::Enum` Layout Construction (`LayoutKind::Enum` + `EnumShape`)

## Description

The phase-1a bridge constructs `Ty::Enum` carrying a layout
produced by `nominal_layout`
(`crates/regseq-driver/src/bridge/ty.rs:905-942`), which only
emits `LayoutKind::Struct` (or `LayoutKind::Union` for unions).
The model spec at `docs/discussions/regseq-model/spec.md:3816-3819`
requires `Ty::Enum.layout.kind == LayoutKind::Enum { shape: EnumShape }`,
and the renderer + validator + place-validator + body-statement
invariant checker all enforce this -- some via clean errors and
some via `panic!`.

The bug is latent in the four checked-in phase-1a fixtures
(only `diag_logging`'s `LogArg` enum is bridged, and the
post-DiagLogPeel `dead_item_elim` residue pass at
`crates/regseq-driver/src/bridge/diag_log/dead_item_elim.rs:33-53,196-205`
removes it before validate runs). The moment any non-eliminated
user enum reaches `model.validate(...)`, rendering, or any place
walker, the compiler hard-fails -- in four sites with a
`panic!`, in others with a clean `LayoutMalformed` /
`StatementAssignMalformed` error:

- `crates/regseq-model/src/layout_validator.rs:529-533`
  (`validate_enum_layout`): clean error -- `Enum layout must use LayoutKind::Enum`.
- `crates/regseq-model/src/place_validator.rs:537-543, 585-589`:
  **panic** on `Ty::Enum` with non-`Enum` layout.
- `crates/regseq-model/src/pretty.rs:1730-1742, 2456-2470, 2692-2705, 2720-2729`:
  **panic** in enum instance / aggregate / field / variant-name rendering.
- `crates/regseq-model/src/body_ty.rs:359-361` +
  `crates/regseq-model/src/model.rs:2533-2563`: clean error path
  via `VariantRecovery::Malformed` -> `InvariantViolation::StatementAssignMalformed`.

Codex pre-task review at session
`019e38c1-0a57-7a92-9a78-2aa1b58ffa51` confirmed the diagnosis,
verified the rustc API surface at the pinned nightly, and
recommended S3 full-coverage scope (Empty + Single + Multiple
Direct + Multiple Niche) over partial-coverage alternatives.
Partial coverage was rejected because the spec contract is that
`Ty::Enum` MUST carry `LayoutKind::Enum`, niche is normative
spec (lines 3988-4014), and `Option<T>` is the most common
user-visible niche case -- punting it would not close the spec
gap, only shift the failure mode.

This task constructs the full enum layout family.

## Input Context

### Bridge site

- `crates/regseq-driver/src/bridge/ty.rs:325-348`
  -- `translate_adt`. Currently routes all ADT kinds (Struct /
  Enum / Union) through `nominal_layout`. After this task,
  the Enum arm dispatches to a new `enum_layout` builder.

- `crates/regseq-driver/src/bridge/ty.rs:905-942`
  -- `nominal_layout`. Unchanged by this task except for
  scope clarification: it remains the Struct/Union path
  only.

- `crates/regseq-driver/src/bridge/mod.rs:437-442`
  -- `BridgeError::Layout { ty_repr }`. The existing layout
  error carries only `ty_repr`; extend the error family with
  a new variant `UnsupportedEnumLayout { ty_repr, reason }`
  for upfront rejection of shapes outside the spec's admitted
  set.

### Spec contract

- `docs/discussions/regseq-model/spec.md:3816-3819` -- the
  `Layout.kind: LayoutKind::Enum { shape: EnumShape }` requirement
  for `Ty::Enum`.

- `docs/discussions/regseq-model/spec.md:3884-3938`
  -- §11.1 `EnumShape` + `VariantLayout`. Defines the three
  shapes (Empty / Single / Multiple) plus `VariantLayout` and
  the `tag_value` semantics table.

- `docs/discussions/regseq-model/spec.md:3926-3938`
  -- signed-discriminant interpretation. `discriminant: u128`
  carries the unsigned reinterpretation of the
  two's-complement encoding at the tag scalar's width.

- `docs/discussions/regseq-model/spec.md:3940-3987`
  -- §11.2 `TagScalar` / `TagEncoding` / `WrappingRange`.
  Includes the logical-scalar policy
  (`Bool/Char/Int/Uint/Ptr/Ref`; `Float` not admitted), the
  `Scalar::Union` rejection rule (lines 3971-3975), and the
  pointer/reference tag-scalar rule (lines 4016-4024).

- `docs/discussions/regseq-model/spec.md:3988-4014`
  -- normative niche tag-value formula:
  ```text
  For v.idx in niche_variants (inclusive range):
    raw = niche_start + (v.idx - niche_variants.start.idx)
    v.tag_value = Some(raw mod 2^scalar_width)
  For v.idx == untagged_variant:
    v.tag_value = None
  ```

- `docs/discussions/regseq-model/spec.md:4026-4033`
  -- width-fit invariant: `valid_range.start`,
  `valid_range.end`, `niche_start`, and per-`VariantLayout.tag_value`
  must all fit `scalar_ty`'s bit width.

### Model types consumed

- `crates/regseq-model/src/layout.rs:46-50,87-108,114-136,138-211`
  -- `LayoutKind::Enum`, `EnumShape`, `VariantLayout`,
  `TagScalar`, `TagEncoding`, `VariantRange`, `WrappingRange`
  schema definitions.

- `crates/regseq-model/src/layout.rs:115-136`
  -- `VariantLayout` fields:
  `{ name: StringId, fields: Vec<FieldLayout>, uninhabited: bool, discriminant: u128, tag_value: Option<u128> }`.

- `crates/regseq-model/src/layout.rs:58-67`
  -- `FieldLayout.offset` is "byte offset from the start of
  the containing aggregate. For enum variant payload fields,
  the containing aggregate is the whole enum object, not the
  variant payload start." -- normative rule for per-variant
  field offset frame.

### Rustc APIs (verified by Codex at pinned nightly)

- `rustc_abi::Variants::{Empty, Single, Multiple}`
  -- per Codex: `rustc_abi/src/lib.rs:1858-1879`. The bridge
  reads `tcx.layout_of(...)?.variants` and dispatches on
  these arms.

- `Variants::Multiple { tag, tag_encoding, tag_field, variants }`
  -- carries the tag `Scalar`, `tag_encoding`, the tag field
  index, and per-variant `LayoutData` whose `.fields` carries
  per-variant `FieldsShape::offset(i)` (Codex:
  `rustc_abi/src/lib.rs:2013-2024,1632-1638`).

- `rustc_abi::Scalar::Initialized { value: Primitive, valid_range: WrappingRange }`
  -- the admitted enum tag shape (spec rejection rule at
  spec lines 3971-3975 for `Scalar::Union`).

- `rustc_abi::Primitive::{Int, Float, Pointer}`
  -- per Codex: `rustc_abi/src/lib.rs:1351-1362`. The bridge
  maps `Int` -> logical `Int(_)` / `Uint(_)` (signedness
  from `Int.signed`), `Pointer` -> logical `Ptr(_, _)` /
  `Ref(_, _)`, and rejects `Float` per spec lines 3977-3986.

- `AdtDef::variants() -> &IndexVec<VariantIdx, VariantDef>`
  -- source-order iteration of enum variants.

- `AdtDef::variant(idx) -> &VariantDef`
  -- single-variant lookup; carries `.name: Symbol` and
  `.fields: IndexVec<FieldIdx, FieldDef>`.

- `AdtDef::discriminant_for_variant(tcx, idx) -> Discr<'_>`
  -- per Codex: `rustc_middle/src/ty/adt.rs:615-629`.
  `Discr.val: u128` is the bit representation per spec lines
  3926-3938.

### Downstream consumers (verified non-touching)

These sites already handle `LayoutKind::Enum` shapes correctly
and require NO modifications:

- `crates/regseq-model/src/layout_validator.rs:523-560+`
  -- `validate_enum_layout` already validates the
  three-shape EnumShape contract.

- `crates/regseq-model/src/place_validator.rs:537-543,585-589`
  -- panics on wrong-shape today; after this task, the bridge
  no longer produces the wrong shape, so the panics become
  unreachable in practice.

- `crates/regseq-model/src/pretty.rs:1730-1742,2456-2470,2692-2705,2720-2729`
  -- enum-shape pretty-printer paths consume `EnumShape`
  variants correctly.

- `crates/regseq-driver/src/bridge/fnsig.rs:480-496`
  -- `layout_field_tys` has correct `LayoutKind::Enum { shape }`
  arms; the bug today is that bridged enums fall through to
  `Struct/Union` instead of triggering this arm.

- `crates/regseq-model/src/ty_interner.rs:483-510`
  -- TyId validation under enum shapes is correct.

### Existing prior art (not reusable for this task)

- `crates/regseq-driver/src/bridge/fnsig.rs:347-353`
  -- `tag_scalar_shape` (C-ABI `repr(int)` substitution).
  Codex confirmed this is NOT a reusable rustc-to-model
  `TagScalar` builder for the layout path; it is a
  signature-side substitution helper. The enum_layout builder
  is greenfield.

### Test plumbing

- `crates/regseq-driver/tests/bridge_ty.rs`
  -- placement for new bridge-side enum-layout unit tests
  per the regseq-driver-53 pattern.

- `crates/regseq-model/tests/`
  -- new integration test directory for an end-to-end
  bridge -> model.validate test using a fixture enum.

## Implementation Notes

Six changes in dependency order.

### Change A: Split `translate_adt`'s layout dispatch

Currently at `crates/regseq-driver/src/bridge/ty.rs:325-348`:

```rust
let fields = adt.all_fields().map(...).collect()?;
let layout = nominal_layout(cx, ty, &fields)?;
let model_ty = match adt.adt_kind() {
    Struct => Ty::Struct { item, args, layout },
    Enum   => Ty::Enum   { item, args, layout },
    Union  => Ty::Union  { item, args, layout },
};
```

Refactor so the Enum arm calls a new `enum_layout` builder:

```rust
let model_ty = match adt.adt_kind() {
    ty::AdtKind::Struct => {
        let fields = ...nominal field (Symbol, TyId)...;
        let layout = nominal_layout(cx, ty, &fields)?;
        Ty::Struct { item, args: model_args, layout }
    }
    ty::AdtKind::Union => {
        let fields = ...nominal field (Symbol, TyId)...;
        let layout = nominal_layout(cx, ty, &fields)?;
        Ty::Union { item, args: model_args, layout }
    }
    ty::AdtKind::Enum => {
        let layout = enum_layout(cx, ty, adt, args)?;
        Ty::Enum { item, args: model_args, layout }
    }
};
```

`nominal_layout` is otherwise unchanged from regseq-driver-53;
its `Symbol::as_str()` field-name interning stays.

### Change B: `BridgeError::UnsupportedEnumLayout`

In `crates/regseq-driver/src/bridge/mod.rs` near
`BridgeError::Layout { ty_repr }` (lines 437-442), add a new
variant carrying both the type identity AND a human-readable
reason string:

```rust
#[error("unsupported enum layout for {ty_repr}: {reason}")]
UnsupportedEnumLayout { ty_repr: String, reason: String },
```

This is reserved for shapes outside the spec's admitted set:
`Scalar::Union` enum tags (spec 3971-3975) and `Float` tag
primitives (spec 3977-3986). Bridged enum layouts that the
bridge cannot translate into a spec-compliant `EnumShape`
trigger this clean error rather than fall through to wrong
layouts.

### Change C: `enum_layout(cx, ty, adt, args) -> Result<Layout, BridgeError>`

New function alongside `nominal_layout` in
`crates/regseq-driver/src/bridge/ty.rs`. Dispatches on
`rustc_layout.variants`.

**Important: rustc/model shape mismatch for fieldless `repr(int)`
single-variant enums.** Per Codex review at session
`019e38cc-29dd-7de2-bf2b-020ec35996d4`, rustc inhibits enum
layout optimisation when `repr.int` is set
(`rustc_abi/src/lib.rs:218-223`,
`rustc_abi/src/layout.rs:310-329`), so an enum like
`#[repr(u8)] enum E { A = 5 }` is lowered to
`Variants::Multiple` with a tag, NOT to `Variants::Single`. The
model contract at `docs/discussions/regseq-model/spec.md:3891-3896,
3920-3921` requires this exact case to map to
`EnumShape::Single { variant, scalar: Some(TagScalar) }`. So
the bridge MUST translate the shape:

| rustc `Variants` shape                                                | Model `EnumShape`                                            |
| --------------------------------------------------------------------- | ------------------------------------------------------------ |
| `Variants::Empty`                                                     | `EnumShape::Empty`                                           |
| `Variants::Single { index }` (one-variant, no `repr(int)`)            | `EnumShape::Single { variant: <index>, scalar: None }`       |
| `Variants::Multiple` with `len == 1` AND that variant is fieldless    | `EnumShape::Single { variant: <0>, scalar: Some(tag) }`      |
| `Variants::Multiple` with `len >= 2` OR len==1 with payload (unlikely)| `EnumShape::Multiple { tag, encoding, variants }`            |

**`Variants::Single { index }` uses `index`, not `VariantIdx::ZERO`.**
Per Codex: rustc `Variants::Single { index }` returns the full
layout for that index when queried for that variant
(`rustc_middle/src/ty/layout.rs:777-780`), and the live variant's
index need not be 0 (e.g., uninhabited variant elision). The
bridge must build the VariantLayout for the actual `index`, not
hard-code ZERO.

```rust
fn enum_layout<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
    adt: ty::AdtDef<'tcx>,
    args: ty::GenericArgsRef<'tcx>,
) -> Result<Layout, BridgeError> {
    let rustc_layout = cx.input.tcx.layout_of(...)?;
    let shape = match &rustc_layout.variants {
        rustc_abi::Variants::Empty => EnumShape::Empty,

        rustc_abi::Variants::Single { index } => {
            // One inhabited variant, no tag bytes. `index` may
            // not be VariantIdx::ZERO.
            let variant = build_variant_layout(
                cx, adt, args, &rustc_layout, *index,
            )?;
            // Tag value for ordinary Single (no scalar) per
            // spec table 3920: None.
            let variant = VariantLayout { tag_value: None, ..variant };
            EnumShape::Single { variant, scalar: None }
        }

        rustc_abi::Variants::Multiple { tag, tag_encoding, variants, .. }
            if variants.len() == 1 && variant_is_fieldless(adt, VariantIdx::ZERO) =>
        {
            // Fieldless `repr(int)` single-variant enum:
            // rustc emits Variants::Multiple (repr.int inhibits
            // optimisation) but the model contract requires
            // EnumShape::Single { scalar: Some(_) } per spec
            // 3891-3896.
            let model_tag = build_tag_scalar(cx, &rustc_layout, tag)?;
            let only_idx = VariantIdx::from_u32(0);
            let variant = build_variant_layout(
                cx, adt, args, &rustc_layout, only_idx,
            )?;
            // tag_value = Some(discriminant) per spec table
            // 3921.
            let discr = adt.discriminant_for_variant(cx.input.tcx, only_idx).val;
            let variant = VariantLayout { tag_value: Some(discr), ..variant };
            EnumShape::Single { variant, scalar: Some(model_tag) }
        }

        rustc_abi::Variants::Multiple { tag, tag_encoding, variants, .. } => {
            let model_tag = build_tag_scalar(cx, &rustc_layout, tag)?;
            let model_encoding = build_tag_encoding(tag_encoding)?;
            let mut model_variants = Vec::with_capacity(variants.len());
            for (idx, _variant_layout) in variants.iter_enumerated() {
                let variant = build_variant_layout(
                    cx, adt, args, &rustc_layout, idx,
                )?;
                let tag_value = compute_tag_value(
                    cx, idx, &model_encoding, &model_tag, adt,
                )?;
                model_variants.push(VariantLayout { tag_value, ..variant });
            }
            EnumShape::Multiple {
                tag: model_tag,
                encoding: model_encoding,
                variants: model_variants,
            }
        }
    };
    Ok(Layout {
        size: rustc_layout.size.bytes(),
        align: rustc_layout.align.abi.bytes(),
        pack: None,
        uninhabited: rustc_layout.uninhabited,
        kind: LayoutKind::Enum { shape },
    })
}
```

The exact split into helpers (`build_variant_layout`,
`build_tag_scalar`, `build_tag_encoding`, `compute_tag_value`,
`variant_is_fieldless`) is at the implementor's discretion;
the spec lists the conceptual steps, not a mandatory function
signature partition.

### Change D: `VariantLayout` construction

Per `crates/regseq-model/src/layout.rs:115-136` and spec
§11.1, each `VariantLayout` carries:

- `name: StringId` -- intern `adt.variant(idx).name.as_str()`.
- `fields: Vec<FieldLayout>` -- walk
  `adt.variant(idx).fields.iter_enumerated()` for source
  names + types. Field offsets come from
  `rustc_layout.variant(idx).fields.offset(i)` (Codex
  verified per
  `rustc_abi/src/lib.rs:1626-1631,2013-2024` and
  `rustc_abi/src/layout.rs:646-650,817-825`: rustc's
  per-variant field offsets are already enum-object-relative
  -- direct encoding prefixes variants with the tag, niche
  encoding shifts fitting variants by object-relative
  offsets). For `Variants::Single { index }`, rustc returns
  the full layout for `index` so `rustc_layout.fields` and
  `rustc_layout.variant(index).fields` are equivalent; the
  bridge should consistently read through
  `rustc_layout.variant(idx).fields.offset(i)` to keep one
  code path. Field name interning uses the same
  `Symbol::as_str()` pattern as regseq-driver-53's nominal
  struct fix.
- `uninhabited: bool` -- from
  `rustc_layout.variant(idx).uninhabited`.
- `discriminant: u128` -- from
  `adt.discriminant_for_variant(tcx, idx).val` per Codex's
  citation at `rustc_middle/src/ty/adt.rs:615-629`. The
  signed-discriminant interpretation per spec lines 3926-3938
  means signed `repr(i*)` discriminants are stored as their
  unsigned two's-complement reinterpretation at the tag
  scalar's width; rustc's `Discr.val` already gives this
  representation per Codex.
- `tag_value: Option<u128>` -- computed in Change E.

### Change E: `tag_value` computation per encoding

Per the spec's table at lines 3916-3924 and the normative
niche formula at lines 3988-4014:

| Enum shape                                                                  | `tag_value`                                  |
| --------------------------------------------------------------------------- | -------------------------------------------- |
| `EnumShape::Single { scalar: None }` (ordinary single-variant)              | `None`                                       |
| `EnumShape::Single { scalar: Some(_) }` (fieldless `repr(int)` one variant) | `Some(discriminant)`                         |
| `EnumShape::Multiple` + `TagEncoding::Direct`                               | `Some(discriminant)` for every variant       |
| `EnumShape::Multiple` + `TagEncoding::Niche`, niched variant                | `Some((niche_start + offset) mod 2^width)`   |
| `EnumShape::Multiple` + `TagEncoding::Niche`, untagged variant              | `None` (payload IS the tag)                  |

The Niche `raw mod 2^scalar_width` step implements wrapping
at the scalar boundary per spec lines 4007-4014.
`scalar_width` is the bit width of `TagScalar.scalar_ty`:
`Bool=8`, `Char=32`, `Int(Ix)`/`Uint(Ux)` natural width,
`Ptr`/`Ref` = `cx.model.target.pointer_width`.

**Out-of-range guard (REQUIRED).** Per spec lines 3988-4003
and rustc's own niche-write assertion at
`rustc_const_eval/src/interpret/discriminant.rs:290-297`, any
variant idx that is neither `untagged_variant` nor inside
`niche_variants` is unreachable: bridge construction must
enforce that. The `compute_tag_value` implementation MUST
include an explicit guard:

```rust
fn compute_tag_value(...) -> Result<Option<u128>, BridgeError> {
    match encoding {
        TagEncoding::Direct => Ok(Some(discr_for(idx))),
        TagEncoding::Niche { untagged_variant, niche_variants, niche_start } => {
            if idx == *untagged_variant {
                return Ok(None);  // payload IS the tag
            }
            if !niche_range_contains(niche_variants, idx) {
                return Err(BridgeError::UnsupportedEnumLayout {
                    ty_repr: ...,
                    reason: format!(
                        "variant {idx:?} is neither the untagged variant nor in niche_variants {niche_variants:?}",
                    ),
                });
            }
            let raw = niche_start
                .wrapping_add((idx.as_u32() - niche_variants.start.as_u32()) as u128);
            let scalar_width = tag_scalar_bit_width(model_tag, target);
            let masked = if scalar_width >= 128 { raw } else { raw & ((1u128 << scalar_width) - 1) };
            Ok(Some(masked))
        }
    }
}
```

The guard makes the bridge fail loudly with a clean
`UnsupportedEnumLayout` instead of silently computing a wrapped
tag value for an out-of-spec rustc shape.

### Change F: `TagScalar` construction

Per spec §11.2 lines 3940-3987:

1. From rustc `Scalar::Initialized { value, valid_range }`:
   - `offset`: from `Variants::Multiple.tag_field` and
     `rustc_layout.fields.offset(tag_field)`.
   - `scalar_ty`: map `Primitive` to logical model `Ty`:
     - `Primitive::Int { signed: true, ... }` -> `Ty::Int(IntKind::...)`.
     - `Primitive::Int { signed: false, ... }` -> `Ty::Uint(UintKind::...)`.
     - `Primitive::Pointer(_)` -> `Ty::Ptr(unit_ty, Mut)` per
       the existing closure-captures pattern at
       `crates/regseq-driver/src/bridge/ty.rs:741`.
     - `Primitive::Float(_)` -> reject via
       `BridgeError::UnsupportedEnumLayout`.

   **Bool/Char tag policy (DETERMINISTIC).** Per Codex review:
   the model spec at lines 3977-3984 admits `Bool` and `Char`
   as logical `TagScalar.scalar_ty` values, but the
   storage-lvalue rule (spec lines 4035-4051) is a codegen
   concern, not a bridge concern. Rustc represents bool tags
   via `Int { size: 1, signed: false }` and char tags via
   `Int { size: 4, signed: false }`; the bridge has no reliable
   way to recover the logical type from the primitive alone.
   This task therefore maps every `Primitive::Int` to
   `Ty::Int(_)` / `Ty::Uint(_)` via the signedness rule and
   does NOT actively produce `TagScalar.scalar_ty == Bool` or
   `Char`. The model spec admits this because validation only
   requires `scalar_ty` to be a valid value -- `Uint(U8)` for a
   logical-bool tag is spec-conformant. Future targeted bridge
   work can refine Bool/Char recovery from niche-payload
   context if/when codegen needs it; that is OUT OF SCOPE for
   regseq-driver-54.
   - `valid_range`: map rustc's `WrappingRange { start, end }`
     to model `WrappingRange { start: u128, end: u128 }`.

2. Reject `Scalar::Union` with `UnsupportedEnumLayout` per
   spec 3971-3975.

### Change G: Tests

#### G1. Bridge-side unit tests (new in `crates/regseq-driver/tests/bridge_ty.rs`)

Each test exercises a distinct enum shape via the in-process
`with_bridge` pattern (already used by
regseq-driver-53's tests at
`crates/regseq-driver/tests/bridge_ty.rs:847-921`):

1. **Empty enum** (`pub enum E {}`): assert layout
   `LayoutKind::Enum { shape: EnumShape::Empty }`.

2. **Single-variant ordinary** (`pub enum E { A }`): assert
   `EnumShape::Single { variant, scalar: None }` with
   `variant.name == "A"`, `variant.discriminant == 0`,
   `variant.tag_value == None`, `variant.fields == []`.

3. **Single-variant with payload** (`pub enum E { A(u32) }`):
   same as #2 but `variant.fields` contains one
   `FieldLayout` for the payload with source-derived name
   `"0"`.

4. **Single-variant fieldless `repr(int)`** (`#[repr(u8)] pub enum E { A = 5 }`):
   assert `EnumShape::Single { variant, scalar: Some(_) }`
   with `variant.discriminant == 5`,
   `variant.tag_value == Some(5)`, and `scalar.scalar_ty`
   resolves to `Ty::Uint(UintKind::U8)`.

5. **Multiple `repr(int)` direct** (`#[repr(u8)] pub enum E { A = 1, B = 2, C = 3 }`):
   assert `EnumShape::Multiple { tag, encoding: TagEncoding::Direct, variants }`
   with three variants, each carrying
   `tag_value == Some(discriminant)`. `tag.scalar_ty` is
   `Ty::Uint(UintKind::U8)`.

6. **Multiple with payload** (`pub enum E { A, B(u32), C { x: u32 } }`):
   exercises payload + named + tuple variants in one enum.
   Verify per-variant `fields[i].name` resolves correctly
   ("0" for tuple, "x" for struct, [] for unit), and
   per-variant field offsets are non-overlapping with the
   tag.

7. **Niche `Option<&u32>`** (the canonical niche case): assert
   `EnumShape::Multiple { tag, encoding: TagEncoding::Niche { untagged_variant, .. }, variants }`.
   `Option::Some(&u32)` is the untagged variant
   (`tag_value == None`); `Option::None` is niched
   (`tag_value == Some(0)` per the niche formula with
   `niche_start = 0`).

8. **Niche with `u32` payload** (`pub enum E { A, B(core::num::NonZeroU32) }`):
   exercises niche with explicit `NonZero` payload. The
   bridge must compute the niche tag_value correctly.

9. **Signed discriminant** (`#[repr(i8)] pub enum E { A = -1, B = 0 }`):
   assert `variant.discriminant == 0xFFu128` for `A` (u128
   bit pattern of two's-complement `-1` as `i8` per spec
   3926-3938) and `0` for `B`.

10. **Rejected: explicit `Scalar::Union` tag**: difficult to
    construct in stable Rust; mark as bridge-internal test
    with a hand-built rustc layout if the rustc API allows.
    If not, document as a code-review-only verification
    point ("rejection path is reachable via
    `Variants::Multiple` with `Scalar::Union` tag; no
    fixture-driven test").

#### G2. Integration test through `model.validate(...)` AND `render()`

Place a new test in `crates/regseq-model/tests/` (alongside
`e2e_diag_logging.rs` + `e2e_synthesis.rs`):

```rust
#[test]
fn bridged_enum_passes_model_validate_and_renders() {
    let mut model = bridge_fixture(test_ir_fixtures::fixture_names::ENUM_ROUND_TRIP);
    let bridge_input = regseq_model::BridgeInput::matching(&model);

    // Spec-contract lock: validate_layouts (model.rs:1590-1592)
    // is what enforces LayoutKind::Enum for Ty::Enum.
    model.validate(&bridge_input).expect("validate must pass");

    // Pretty-printer panic-coverage lock: the 4 panic sites
    // at pretty.rs:1730-1742, 2456-2470, 2692-2705,
    // 2720-2729 only fire on rendered output, not via
    // validate. Render to assert no panic. Mirrors the
    // pattern at e2e_diag_logging.rs:171 + e2e_synthesis.rs:269.
    let rendered = regseq_model::PrettyPrinter::new(&model).render();
    assert!(
        !rendered.is_empty(),
        "rendered model must be non-empty for the enum_round_trip fixture",
    );
}
```

Backed by a new fixture
`crates/test-ir-fixtures/fixtures/enum_round_trip.rs`
exercising at least one Empty / Single / Multiple Direct /
Multiple Niche enum each, plus uses that produce
`AggregateKind::Enum` and `SetDiscriminant` rvalues so the
full `validate_layouts` + body-invariant chain runs.

**Fixture registration (REQUIRED).** Existing shipped
fixtures are registered as constants in
`crates/test-ir-fixtures/src/lib.rs:34-44`
(`fixture_names::NO_OP`, `DIAG_LOGGING`, `TRAIT_OBJECT`,
`CLOSURE`) and enumerated in the fixture-presence test at
`crates/test-ir-fixtures/src/lib.rs:261-268`. This task MUST
add `fixture_names::ENUM_ROUND_TRIP = "enum_round_trip"` and
include it in the presence test. Reading the fixture file
without registering it would not surface in
`bridge_fixture(...)` lookups.

This integration test is the spec-contract lock per Codex's
V10 + S9: bridge-side unit tests on `EnumShape` fields alone
are not sufficient because the contract is enforced by
`validate_layouts` (`crates/regseq-model/src/model.rs:1590-1592`)
AND the pretty-printer panic sites only execute via render.

### Conventions

- Field-name interning uses the `Symbol::as_str()` pattern
  introduced by regseq-driver-53.
- Variant-name interning uses
  `cx.model.strings.intern(adt.variant(idx).name.as_str())`.
- All u128 storage uses rustc's `Discr.val: u128` directly --
  no re-encoding.
- The Niche tag-value formula is implemented in a single
  helper so the modular-width arithmetic is unit-testable.
- Unsupported shapes route through
  `BridgeError::UnsupportedEnumLayout` exclusively; do not
  reuse `BridgeError::Layout` for these (the latter carries
  only `ty_repr`).

## Non-Goals

- **Renderer changes.** `pretty.rs:1730-1742, 2456-2470,
  2692-2705, 2720-2729` already render enum shapes correctly;
  this task makes the panic paths unreachable, it does not
  modify them.
- **Validator changes.** `layout_validator.rs:523-560+`
  already enforces the EnumShape contract correctly.
- **`place_validator` panic recovery.** The panics at
  `place_validator.rs:537-543, 585-589` are correct given the
  spec contract; they become unreachable once bridged enums
  carry the right shape.
- **`body_ty.rs::place_enum_variant_count` changes.** The
  function correctly recovers variant counts from
  `EnumShape` today.
- **Coroutine variants / async enums.** Already rejected at
  the bridge level (per regseq-driver-50 Non-Goals).
- **Discriminant transmutes / `core::mem::discriminant`
  shortcuts.** Variant discriminant access at the MIR level
  already works through `Rvalue::Discriminant` and is out of
  scope for this task.
- **C-backend codegen of enum tag writes.** Phase-1b
  territory. This task closes the IR-level spec contract,
  not the codegen contract.
- **Bridging `Box<enum>` / `Rc<enum>` / `Arc<enum>`.**
  Inherits the regseq-driver-50 Non-Goals on indirection
  variants.

- **`Bool`/`Char` tag-scalar recovery.** Per the Change F
  policy, the bridge maps every rustc `Primitive::Int` to
  model `Int(_)`/`Uint(_)`. Future bridge work can refine
  Bool/Char recovery from niche-payload context if/when a
  downstream consumer requires it.

- **`#[repr(transparent)]` enums.** Rustc may lay these out
  differently from ordinary enums. If a `repr(transparent)`
  enum survives the bridge into `enum_layout`, the existing
  shape dispatch handles it via the standard
  `Variants::Single` arm (transparent enums are one-variant);
  no special transparent-aware handling is in scope.


## Test Plan

### A. Compilation gate

1. `cargo build --workspace` succeeds after Changes A-F.

### B. Bridge-side unit tests (per Change G1)

2. The ten unit tests from Change G1 above all pass under
   `cargo test -p regseq-driver --test bridge_ty`. Each test
   asserts the resulting `Layout.kind` and
   `EnumShape`/`VariantLayout` fields directly.

### C. Integration test through `model.validate`

3. The new `enum_round_trip` fixture
   (`crates/test-ir-fixtures/fixtures/enum_round_trip.rs`)
   bridges to a `RegseqModel` that passes
   `model.validate(...)`.

### D. No-regression on existing fixtures

4. `cargo test --workspace` passes including:
   - `crates/regseq-model/tests/e2e_diag_logging.rs` (the
     LogArg-elimination path -- verify the elim pass still
     removes the now-correct enum layout, not just the
     wrong one).
   - `crates/regseq-model/tests/e2e_synthesis.rs` (vtable
     and closure render).
   - `crates/regseq-driver/tests/bridge_body.rs:312-395`
     (the `enum E { A, B { v: u32 } }` aggregate / downcast
     / SetDiscriminant rvalue assertions -- now the body
     paths see a correctly-shaped enum, but the existing
     test only asserts on Rvalue shapes which are unchanged).

### E. Negative: reject unsupported shapes

5. Construct (in a bridge-side unit test) a `Variants::Multiple`
   with a `Scalar::Union` tag. Assert the bridge returns
   `BridgeError::UnsupportedEnumLayout { reason: ..., .. }`
   containing the string "union" (lowercase). If constructing
   a `Scalar::Union` tag is impossible in stable rustc, fall
   back to a code-review verification point with a
   doc-comment citation of spec 3971-3975.

6. Construct (similarly) a `Primitive::Float` tag. If
   impossible to construct via stable rust source, fall
   back to code-review.

### F. Workspace gates

7. `cargo fmt --all -- --check` -- clean.
8. `cargo clippy --workspace --tests -- -D warnings` -- no
   NEW lint sites in modified files beyond the pre-existing
   baseline (the baseline is 49 errors as of regseq-driver-53).

### G. End-to-end playground proof (verification-only)

9. Manual: invoke `regseq-driver` directly per the
   `crates/test-ir-fixtures/src/lib.rs:127-220` pattern on a
   source file containing
   `pub enum Color { Red, Green, Blue }` plus a `fn` that
   constructs and matches on it. Dump the rendered model.
   Confirm no panics and that the enum aggregate renders
   correctly (e.g., `Color::Red`, `Color::Green`, ...). This
   is verification-only, not a checked-in test; the B + C
   tests are the contract coverage.

## Review

Once tests pass, request a Codex review of the implementation
per `SESSION.md` Review section. Four-block template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-54 against its
task spec at
.harness/tasks/regseq-driver/54-bridge-enum-layout-construction.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/ty.rs (translate_adt
  dispatch, new enum_layout + helpers, nominal_layout
  unchanged)
- crates/regseq-driver/src/bridge/mod.rs
  (BridgeError::UnsupportedEnumLayout)
- crates/regseq-driver/tests/bridge_ty.rs (G1 unit tests)
- crates/test-ir-fixtures/fixtures/enum_round_trip.rs
  (new integration fixture)
- crates/regseq-model/tests/<new e2e test> (integration
  validate)

Focus on:
- enum_layout dispatches Empty/Single/Multiple via
  rustc Variants.
- VariantLayout.discriminant comes from rustc
  Discr.val (u128 bit representation).
- Niche tag_value follows spec 3988-4014 formula with
  mod 2^scalar_width.
- TagScalar.scalar_ty is the logical type
  (Bool/Char/Int/Uint/Ptr/Ref); Float and Scalar::Union
  rejected with BridgeError::UnsupportedEnumLayout.
- Per-variant field offsets are enum-object-relative
  per spec 3866-3870.
- Field/variant names use Symbol::as_str() interning.
- nominal_layout is unchanged from regseq-driver-53.
- No renderer/validator changes.

Do NOT flag anything explicitly marked out of scope
(renderer changes, validator changes, place_validator
panic recovery, coroutines).
</task>

<grounding_rules>
Cite file:line for every claim. No speculation. Mark
hypotheses as hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what
was verified and why any deliberate deviations are
acceptable.
Total response <= 500 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a
verdict, stop.
</default_follow_through_policy>
EOF
```

Iterate (fix -> re-invoke `codex_runner.py`) until
VERDICT: APPROVED.

## Quality Gate

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --tests -- -D warnings
```

Authoritative gate is `task.py done`, which runs cleanup +
workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements.

## Acceptance Criteria

- `crates/regseq-driver/src/bridge/ty.rs` `translate_adt`
  dispatches the Enum arm to a new `enum_layout` function;
  Struct and Union arms continue to use `nominal_layout`
  unchanged.

- `crates/regseq-driver/src/bridge/ty.rs::enum_layout`
  constructs `LayoutKind::Enum { shape }` for every
  `Ty::Enum`, dispatching on `rustc_abi::Variants` per the
  shape-mapping table in Change C:
  - `Variants::Empty` -> `EnumShape::Empty`.
  - `Variants::Single { index }` -> `EnumShape::Single { variant: <index>, scalar: None }`
    (uses the actual `index`, not hard-coded `VariantIdx::ZERO`).
  - `Variants::Multiple` with `variants.len() == 1` AND the
    only variant is fieldless ->
    `EnumShape::Single { variant: <0>, scalar: Some(tag) }`
    (the fieldless `repr(int)` single-variant case; rustc
    emits `Variants::Multiple` because `repr.int` inhibits
    layout optimisation).
  - Any other `Variants::Multiple` ->
    `EnumShape::Multiple { tag, encoding, variants }`.

- Each `VariantLayout` carries:
  - `name`: interned `adt.variant(idx).name.as_str()`.
  - `fields`: per-variant `FieldLayout`s with source names
    (`Symbol::as_str()`) and enum-object-relative offsets.
  - `uninhabited`: from rustc per-variant uninhabited flag.
  - `discriminant`: from
    `adt.discriminant_for_variant(tcx, idx).val: u128`.
  - `tag_value`: computed per the encoding (Direct =
    `Some(discriminant)`; Niche = formula per spec
    3988-4014; untagged = `None`; Single-no-scalar = `None`;
    Single-with-scalar = `Some(discriminant)`).

- `TagScalar.scalar_ty` resolves to a logical scalar from the
  admitted set (`Int(_)`/`Uint(_)`/`Ptr`/`Ref`); `Float` is
  rejected with `BridgeError::UnsupportedEnumLayout`. Bool/Char
  tag recovery is explicitly out of scope (see Non-Goals); the
  bridge maps every `Primitive::Int` to `Int(_)`/`Uint(_)`.

- `Scalar::Union` enum tags are rejected with
  `BridgeError::UnsupportedEnumLayout { ty_repr, reason }`.

- New `BridgeError::UnsupportedEnumLayout { ty_repr, reason }`
  exists in `crates/regseq-driver/src/bridge/mod.rs`.

- Niche `tag_value` computation guards out-of-range variants:
  if `idx != untagged_variant && !niche_variants.contains(idx)`,
  the bridge returns `BridgeError::UnsupportedEnumLayout`
  rather than computing a wrapped value.

- `Variants::Single { index }` builds the variant for `index`
  (not hard-coded `VariantIdx::ZERO`); single-variant
  fieldless `repr(int)` enums (which rustc emits as
  `Variants::Multiple` with `len == 1`) map to
  `EnumShape::Single { variant, scalar: Some(_) }` per spec
  table 3920-3921.

- Ten new bridge-side unit tests in
  `crates/regseq-driver/tests/bridge_ty.rs` cover G1.1-G1.10.

- New integration test bridges
  `crates/test-ir-fixtures/fixtures/enum_round_trip.rs` and
  passes BOTH `model.validate(...)` AND
  `PrettyPrinter::new(&model).render()` (the latter exercises
  the 4 pretty.rs panic sites that `validate` does not
  cover).

- `crates/test-ir-fixtures/src/lib.rs` carries a new
  `fixture_names::ENUM_ROUND_TRIP` constant alongside the
  existing `NO_OP`/`DIAG_LOGGING`/`TRAIT_OBJECT`/`CLOSURE` set
  and is included in the fixture-presence test at
  `:261-268`.

- `cargo test --workspace` regression-clean. Existing
  `e2e_diag_logging` and `e2e_synthesis` tests still pass.

- `cargo fmt --all -- --check` and
  `cargo clippy --workspace --tests -- -D warnings` clean
  (no NEW lint sites in modified files beyond the
  pre-existing baseline of 49).

- Codex Task Review returns VERDICT: APPROVED.
