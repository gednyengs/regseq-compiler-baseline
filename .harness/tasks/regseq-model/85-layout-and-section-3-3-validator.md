# regseq-model-85: Layout consistency + invariants 14 / 19 / 20 + UNIT_LAYOUT + target / build + FnSig / CAbiLowering + const-graph acyclicity + domain-attr + source-file validators (P1a-VAL-03)

## Description

Implement the remaining Phase B validators that were not absorbed by
P1a-VAL-01 (Place walk) or P1a-VAL-02 (referential integrity, F4 / F4-2,
DefPath / span / vtables). Specifically: layout consistency per `§11`
and `§3.1` invariant 14, the model-checkable subset of `§3.1` invariant
19 (`Item.opaque_handle_eligible`), `§3.1` invariant 20
(`StableCrateId(0)` synthetic-domain enforcement), the canonical
`UNIT_LAYOUT` check on every `Ty::Tuple { fields: [] }` per
`spec.md:3849-3854` and `spec.md:1050-1056`, target / build
well-formedness per invariants 15 / 17, FnSig / CAbiLowering structural
checks per `§3.3` step 5 and the Phase B continued block, const-graph
acyclicity per `§3.3` step 9 (the value-level Const recursion DAG with
three-state DFS marks), domain-attr validation per `§3.6.5`, and
source-file validation (line-table consistency for non-sentinel
entries).

These validators extend the same `RegseqModel::validate` walk amended
by P1a-VAL-02 and reuse its `InvariantViolation` enum. The Phase B
ordering pinned at `spec.md:676-1102` is binding -- VAL-03 lands its
checks in the spec's documented order (target / build inside step 3
sub-steps (f) / (g); FnSig / CAbiLowering at step 5; layout sanity at
step 7; invariant 14 at step 8; const-graph acyclicity at step 9;
domain-attr at step 10; the Phase B continued block enforces invariants
14, 19, 20, `UNIT_LAYOUT`, and source-file span bounds beyond
reachability).

## Input Context

- `docs/discussions/regseq-model/spec.md` `§3.1` invariants 14
  (lines 512-516), 15 (lines 517-527), 17 (lines 543-548), 19
  (lines 556-595), 20 (lines 596-604).
- `docs/discussions/regseq-model/spec.md` `§3.3` Phase B step 3
  sub-steps (f) and (g) (lines 719-722); step 5 (lines 742-758);
  step 7 (line 763); step 8 (lines 764-787); step 9 (lines 812-908);
  step 10 (lines 909-951); Phase B continued block (lines 953-1102).
- `docs/discussions/regseq-model/spec.md` `§3.6.5` Validation
  invariants (lines 1716-1768).
- `docs/discussions/regseq-model/spec.md` `§5` (Source files and
  spans) for line-table semantics; `§5.5` for the synthesised
  sentinel.
- `docs/discussions/regseq-model/spec.md` `§11` (Layout) and `§11.5`
  (size/align query) and `§11.2` (TagScalar / TagEncoding /
  WrappingRange).
- `docs/discussions/regseq-model/spec.md` `§14.3` (FnSig minimum
  canonical schema) and `§14.5` (C ABI admissibility, lines 5366+);
  CAbiLowering Direct / Substituted shape.
- `docs/discussions/regseq-model/spec.md` `§9.7` (Const vocabulary
  byte-encoding tags), particularly the value-level recursion
  positions (`Aggregate.0`, `EnumValue.payload`).
- `docs/discussions/regseq-model/spec.md` `§13.4` (synthetic
  `DefPathHash` construction; `stable_crate_id == 0` domain).
- `crates/regseq-model/src/model.rs` -- existing
  `RegseqModel::validate` (extended by P1a-VAL-02; VAL-03 lands
  on top).
- `Layout` / `LayoutKind` / `EnumShape` / `VariantLayout` /
  `TagScalar` / `TagEncoding` / `WrappingRange` / `UNIT_LAYOUT`
  from regseq-model-68 (P1a-DS-03).
- `Item` / `ItemKind` / `Item.opaque_handle_eligible` from
  regseq-model-72 (P1a-DS-07).
- `SyntheticItemKind` / `PrincipalTraitKey` from regseq-model-74
  (P1a-DS-08b).
- `Instance` / `InstanceKind` from regseq-model-75 (P1a-DS-10).
- `DomainAttrTables` / `AttrData` / `IntrinsicKind` / `LogLevel` /
  `FieldPath` from regseq-model-73 (P1a-DS-09).
- `archive/.harness/tasks/regseq-model/24-place-and-projection.md`
  as the SHAPE / SIZE reference.

## Implementation Notes

The validator extensions land in the same module hierarchy already
amended by P1a-VAL-02. Internal organisation: split per-invariant
helpers into a new private submodule (e.g.,
`crates/regseq-model/src/model/validate_layout.rs` or a sibling free
module `crates/regseq-model/src/layout_validator.rs`) to keep each
helper focused -- the exact split is an implementation choice. The
spec's Phase B ordering at `spec.md:676-1102` is the binding sequence;
new helpers slot in at the spec-documented step.

### Step 3 (f) -- target well-formedness (invariant 15)

Per `spec.md:517-527` and `spec.md:719-720`. For
`RegseqModel.target: TargetInfo`:

- `target.triple` and `target.data_layout` are non-empty interned
  strings (resolve through `model.strings`).
- `target.pointer_width in {16, 32, 64}`.
- `target.pointer_width` and `target.endian` agree with the parsed
  `target.data_layout`.
- `target.scalar_layouts` populated for every primitive scalar
  (`Bool`, `Char`, every `IntKind` / `UintKind` / `FloatKind`,
  data-pointer, fn-pointer) with sizes / alignments matching
  `target.data_layout`. All values are powers of two where ABI
  requires it.

Reject with
`InvariantViolation::TargetMalformed { reason: &'static str }`.

### Step 3 (g) -- build well-formedness (invariant 17)

Per `spec.md:543-548` and `spec.md:721-722`. For
`RegseqModel.build: BuildInfo`:

- `build.toolchain` is a non-empty interned string.
- `build.bridge_options_hash` is present (structural check; the
  determinism property is bridge-side and not re-derived).

Reject with
`InvariantViolation::BuildMalformed { reason: &'static str }`.

### Step 5 -- FnSig / CAbiLowering structural checks

Per `spec.md:742-758`. For each `FnSig` in `model.sigs`:

- `c_variadic == true` implies `abi == FnAbi::C` and
  `inputs.len() >= 1` (per `spec.md:1021-1023`).
- No `inputs[i] == Ty::Never` (per `spec.md:1023`).

For each entry in `sig_c_abi_lowerings`:

- `sig_c_abi_lowerings.len() == sigs.len()` (per `spec.md:540-542`).
- `Some(_)` iff the corresponding FnSig has `abi == FnAbi::C`;
  `None` otherwise.
- Arity: `lowering.inputs.len() == sigs[id].inputs.len()`.
- Shape-target reachability: every `Substituted.emit_as` TyId is
  in-bounds.
- Tag-scalar primitiveness: when `Substituted.reason == TagScalar` or
  `TransparentChainAndTagScalar`, the resolved `Substituted.emit_as`
  `Ty` must be `Ty::Int(_)` or `Ty::Uint(_)` (per
  `spec.md:752-758` + `spec.md:1024-1030`).
- `Direct` on output unit (`Ty::Tuple { fields: [] }`) and on
  `Never` is the codegen-recognised void / noreturn-void shape;
  validation accepts it.

Reject with
`InvariantViolation::FnSigMalformed { sig: FnSigId, reason: &'static
str }` and
`InvariantViolation::CAbiLoweringMalformed { sig: FnSigId, reason:
&'static str }`.

### Step 7 -- Layout sanity (`§11.5`, `§12.5`)

Per `spec.md:1031-1040`. For each `Layout` reachable through
`Ty::{Struct, Enum, Union, Tuple} { layout, .. }` and each
`VariantLayout`:

- Variant-vs-kind matching: `LayoutKind::Struct` carries a `fields`
  vec; `Union` carries a `fields` vec; `Enum` carries a `shape`;
  Tuple's layout uses `LayoutKind::Struct` per the §11 convention.
- Field reachability: every `FieldLayout.ty` resolves in
  `model.types`.
- Field-fits-in-layout: `field.offset + size_of(field.ty) <=
  layout.size`.
- Alignment respected: `field.offset % required_field_align == 0`
  where `required_field_align = match layout.pack { None =>
  align_of(field.ty), Some(n) => min(align_of(field.ty), n) }` (per
  `spec.md:1037-1040`).
- Non-overlap of non-ZST fields per `spec.md:1034-1037`.

Reject with
`InvariantViolation::LayoutMalformed { ty: TyId, reason: &'static str }`.

### `§11.2` Tag-scalar width-fit

Per `spec.md:1041-1045`: every `valid_range.{start, end}`,
`niche_start`, and per-`VariantLayout.tag_value` fits the bit width of
`TagScalar.scalar_ty` (`Bool=8`, `Char=32`, `Int(Ix)/Uint(Ux)`
natural, `Ptr/Ref` -> `target.pointer_width`).

Reject with
`InvariantViolation::TagScalarOutOfRange { ty: TyId, value: u128,
scalar_width_bits: u32 }`.

### Step 8 (continued) -- Invariant 14: InstanceKind layout consistency

Per `spec.md:512-516`. For each `Instance` in `model.instances` whose
kind is `InstanceKind::Struct { layout }` /
`InstanceKind::Enum { layout }` / `InstanceKind::Union { layout }`:

- The `layout: TyId` resolves to a `Ty` of the matching nominal
  variant (`Ty::Struct` / `Ty::Enum` / `Ty::Union`).
- That `Ty`'s `(item, args)` equals `(Instance.item,
  Instance.generic_args)`.

Reject with
`InvariantViolation::InstanceLayoutMismatch { instance: InstanceId,
expected_kind: &'static str, found_kind: &'static str }` and
`InvariantViolation::InstanceLayoutItemArgsMismatch { instance:
InstanceId }`.

### Step 9 -- const-graph acyclicity (`§3.3` step 9 / `spec.md:839-856`)

Three-state DFS (`Unseen` / `Visiting` / `Done`) over the recursion
edges in `Aggregate.0` and `EnumValue.payload`. Edges to a `Visiting`
node are back-edges and reject; edges to a `Done` node are skipped
(shared DAG children admissible). After the DAG check, the value-level
canonical-form recomputation can run safely (this completes
ConstInterner injectivity coverage that VAL-02 short-circuited).

Reject with
`InvariantViolation::ConstGraphCycle { entry_point: ConstId, cycle:
Vec<ConstId> }`. The `cycle` field carries the back-edge path for
diagnostic clarity (the spec mandates the check; the diagnostic shape
is implementation-defined).

### Step 10 -- domain-attr validation (`§3.6.5`)

Per `spec.md:1716-1768`. For `model.domain_attrs`:

- `domain_attrs.item_attrs.len() == model.items.len()` (per
  `spec.md:1722-1724`).
- For each `FieldPath` key in `field_attrs`: the embedded `ItemId`
  resolves to an Item whose `kind` is `Struct` / `Enum` / `Union`;
  the `VariantIdx` is in-bounds for that ADT's variant set
  (`0` for Struct / Union; in-range per `EnumShape.variants` for
  Enum); the `FieldIdx` is in-bounds for the indicated variant's
  `VariantLayout.fields`.
- For each `Span` key in `span_attrs`: structurally valid `Span`
  (per `§5.3`); span bounds enforced by VAL-02's source-span check
  (do not duplicate -- VAL-02 covers all reachable Spans across the
  model, including domain-attr Spans). VAL-03 only verifies the
  structural validity (file IDs in range, lo / hi present).
- Nested ID well-formedness: every `ItemId` / `FnSigId` /
  `StringId` / embedded `FieldPath` component inside an `AttrData`
  payload resolves.
- Per-family duplicate invariant `I-DomainAttr-Intrinsic-Singleton`
  (per `spec.md:1751-1757`): for any `ItemId id`, the count of
  `AttrData::Intrinsic(_)` entries in
  `domain_attrs.item_attrs[id]` is at most 1.
- Per `spec.md:1764-1768`: the validator does NOT re-derive or
  re-sort `Vec<AttrData>` entries; ordering is bridge-walk-order
  deterministic per D-14-3.3.

Reject with `InvariantViolation::DomainAttrMalformed { reason:
&'static str }` and per-payload-position variants for nested-ID
failures (or reuse `DanglingId` with a domain-attr-specific
`field` label).

### Phase B continued -- Invariant 19 (model-checkable subset)

Per `spec.md:995-1014`. For each Item with `opaque_handle_eligible
== true`:

- `Item.kind == ItemKind::Struct`.
- Exactly one `Ty::Struct { item: this, args, layout }` entry in
  `model.types` with `args.is_empty()` (non-generic criterion).
- That entry's `layout.size == 0 && layout.align == 1`.
- Reachable-use traversal: every reachable use of the Item's
  monomorphic `TyId` across the model is behind `Ty::Ref` /
  `Ty::Ptr` (per the closed traversal domain pinned in `§14.5`).

Reject with `InvariantViolation::OpaqueHandleInconsistent { item:
ItemId, reason: &'static str }`.

### Phase B continued -- Invariant 20 (`StableCrateId(0)` enforcement)

Per `spec.md:596-604`: no real source crate may carry
`StableCrateId(0)`. Walk `model.crates`; if any
`crate.stable_crate_id == StableCrateId(0)`, reject with
`InvariantViolation::StableCrateIdZeroCollision { crate_id: CrateId }`.
Synthetic-item DefPathHashes use `stable_crate_id == 0` per `§13.4`;
they live in `item_def_paths`, not in `crates`. The same rule also
implies: for every Item with `item_synthetic_kinds[id] != None`, the
synthetic `DefPathHash` carries `stable_crate_id == 0`; for every
source-derived Item (`item_synthetic_kinds[id] == None`), the
`DefPathHash` carries `stable_crate_id != 0`. Reject the second-form
violation with
`InvariantViolation::SyntheticDomainBoundary { item: ItemId, reason:
&'static str }`.

### Phase B continued -- canonical `UNIT_LAYOUT` check

Per `spec.md:1050-1056`: every `Ty::Tuple { fields: [], layout }` must
carry `layout == UNIT_LAYOUT` (the constant defined in
regseq-model-68). Reject with
`InvariantViolation::UnitLayoutMismatch { ty: TyId }`.

### Source-file validation

Per `spec.md:684-686`: synthesised sentinel at `SourceFileId(0)`
(already enforced by stage-1 walk; VAL-03 verifies it is preserved by
the extended walk); line-table consistency for non-sentinel entries:
`line_starts` is monotonic, starts at `0`, and every entry is `<=
content.len()` for `content: Some(_)` files. Reject with
`InvariantViolation::SourceFileMalformed { file: SourceFileId, reason:
&'static str }`.

Also enforce: no two `SourceFile`s share the same `path` StringId
(duplicate-file rejection per the prompt's directive). Reject with
`InvariantViolation::DuplicateSourceFilePath { first: SourceFileId,
second: SourceFileId, path: StringId }`.

### Conventions

- All public types and functions carry doc comments; no unicode
  symbols (per `SESSION.md` "Coding Conventions").
- Per the user's "no defensive fallbacks" rule, the validator does
  not silently tolerate corrupt serialised models -- every check
  produces a typed `InvariantViolation` variant.
- VAL-02's `validate` walk and VAL-03's checks share the
  `InvariantViolation` enum; new variants are added as needed.
- Phase B ordering is binding per `spec.md:676-1102`; do not reorder.

## Non-Goals

- Place / projection structural and type-coherence checks per
  `§19.3` / `§19.4` / `§19.7` are owned by P1a-VAL-01
  (regseq-model-83).
- ID referential integrity per `§3.1` invariants 1, 2, 6, 9, 11, 18;
  F4 / F4-2 sort comparator output check; `DefPath.rendered`
  canonical-rendering; reachable-Span source bounds; Phase C
  `vtables` rebuild and duplicate rejection are owned by P1a-VAL-02
  (regseq-model-84). VAL-03 reuses VAL-02's source-span check; it
  does NOT duplicate it.
- Bridge-side construction of CAbiLowering / opaque_handle_eligible /
  synthetic DefPathHash is owned by the P1a-BR-* axis. VAL-03 only
  checks the bridge's stored outcomes.
- Re-deriving rustc-side facts (source repr classification, the
  recursive C-ABI admissibility predicate's per-step decisions) is
  out of scope per the `§3.3` trust model (`spec.md:629-656`).
- Pretty-printing is owned by the P1a-PP-* axis.

## Test Plan

Tests live in two tiers per the brief discipline (`unit-in-src` +
`integration-in-tests`). Each invariant the validator enforces gets
**both** a positive test (a model that passes) and a negative test (a
model that violates the invariant and is rejected with the right error
variant). Integration tests under
`crates/regseq-model/tests/layout_validator.rs` build constructed-model
fixtures using `RegseqModel::with_primary` + push helpers.

### Invariant 14 -- InstanceKind layout consistency

- `invariant_14_struct_pass`: `InstanceKind::Struct { layout: ty }`
  where `ty` resolves to `Ty::Struct { item, args, .. }` whose
  `(item, args)` equals `(Instance.item, Instance.generic_args)` --
  accepted.
- `invariant_14_enum_pass` / `invariant_14_union_pass`: same for
  Enum and Union.
- `invariant_14_reject_kind_mismatch_struct_to_enum`:
  `InstanceKind::Struct { layout: ty }` whose `ty` resolves to
  `Ty::Enum { .. }` -- rejected with `InstanceLayoutMismatch`.
- `invariant_14_reject_item_args_mismatch`: kind matches but
  `(item, args)` differs -- rejected with
  `InstanceLayoutItemArgsMismatch`.

### Invariant 19 -- opaque_handle_eligible

- `invariant_19_pass_zst_non_generic_ref_only`: an Item with
  `opaque_handle_eligible == true`; `kind == Struct`; exactly one
  monomorphic `Ty::Struct` entry with `args.is_empty()`,
  `layout.size == 0`, `layout.align == 1`; every reachable use is
  behind `Ref` / `Ptr` -- accepted.
- `invariant_19_reject_non_struct_kind`: `opaque_handle_eligible ==
  true` on `kind == Function` -- rejected with
  `OpaqueHandleInconsistent`.
- `invariant_19_reject_generic`: two `Ty::Struct` entries (or one
  with `!args.is_empty()`) -- rejected.
- `invariant_19_reject_non_zst_layout`: `layout.size != 0` --
  rejected.
- `invariant_19_reject_by_value_use`: a reachable use that is NOT
  behind `Ref` / `Ptr` (e.g., a struct field of the opaque handle
  type) -- rejected.

### Invariant 20 -- StableCrateId(0) enforcement

- `invariant_20_pass`: every real crate carries
  `stable_crate_id != 0`; every synthetic `DefPathHash` carries
  `stable_crate_id == 0`; every source-derived `DefPathHash` carries
  `stable_crate_id != 0` -- accepted.
- `invariant_20_reject_real_crate_zero`: a `CrateInfo` with
  `stable_crate_id == StableCrateId(0)` -- rejected with
  `StableCrateIdZeroCollision`.
- `invariant_20_reject_source_derived_synthetic_domain`: a
  source-derived Item (`item_synthetic_kinds[id] == None`) whose
  `DefPathHash.stable_crate_id == 0` -- rejected with
  `SyntheticDomainBoundary`.
- `invariant_20_reject_synthetic_non_zero_domain`: a synthetic
  Item (`item_synthetic_kinds[id] != None`) whose
  `DefPathHash.stable_crate_id != 0` -- rejected with
  `SyntheticDomainBoundary`. (Carve-out per `spec.md:440-447`:
  `SyntheticItemKind::ClosureCaptures` uses rustc's
  `def_path_hash(closure_did)` and is therefore admitted with
  `stable_crate_id != 0`. The fixture exercises this carve-out as a
  positive case.)

### Canonical UNIT_LAYOUT check

- `unit_layout_pass`: every `Ty::Tuple { fields: [], layout }` has
  `layout == UNIT_LAYOUT` -- accepted.
- `unit_layout_reject_non_canonical`: a `Ty::Tuple { fields: [],
  layout }` with `layout.size != 0` (or any other field deviating
  from `UNIT_LAYOUT`) -- rejected with `UnitLayoutMismatch`.

### Target / build well-formedness (invariants 15 / 17)

- `target_pass`: triple non-empty, `data_layout` non-empty,
  `pointer_width in {16, 32, 64}`, scalar layouts populated --
  accepted.
- `target_reject_pointer_width_invalid`: `pointer_width == 24` --
  rejected with `TargetMalformed`.
- `target_reject_endian_disagrees`: `target.endian` disagrees with
  the parsed `data_layout` -- rejected.
- `target_reject_scalar_layouts_missing`: `scalar_layouts` missing
  `Char` -- rejected.
- `build_pass`: `toolchain` non-empty, `bridge_options_hash`
  present -- accepted.
- `build_reject_toolchain_empty`: `toolchain` is the empty interned
  string -- rejected with `BuildMalformed`.

### FnSig / CAbiLowering consistency

- `fn_sig_c_variadic_pass`: `c_variadic == true`, `abi == FnAbi::C`,
  `inputs.len() == 1` -- accepted.
- `fn_sig_c_variadic_reject_non_c_abi`: `c_variadic == true`, `abi
  == FnAbi::Rust` -- rejected with `FnSigMalformed`.
- `fn_sig_c_variadic_reject_zero_inputs`: `c_variadic == true`,
  `inputs.is_empty()` -- rejected.
- `fn_sig_reject_inputs_never`: `inputs[0] == Ty::Never` --
  rejected.
- `cabi_lowering_pass_arity`: `lowering.inputs.len() ==
  sig.inputs.len()` -- accepted.
- `cabi_lowering_reject_arity_mismatch`: `lowering.inputs.len() !=
  sig.inputs.len()` -- rejected with `CAbiLoweringMalformed`.
- `cabi_lowering_reject_substituted_emit_as_dangling`: `Substituted
  { emit_as: TyId(99999), .. }` out of range -- rejected.
- `cabi_lowering_reject_tag_scalar_non_int`:
  `Substituted.reason == TagScalar`, `emit_as` resolves to
  `Ty::Bool` -- rejected (`TagScalar` requires `Int` or `Uint`).
- `cabi_lowering_reject_some_for_rust_abi`:
  `sig.abi == FnAbi::Rust` but `sig_c_abi_lowerings[id] ==
  Some(_)` -- rejected.
- `cabi_lowering_reject_none_for_c_abi`:
  `sig.abi == FnAbi::C` but `sig_c_abi_lowerings[id] == None` --
  rejected.

### Layout sanity (`§11.5`, `§12.5`) + tag-scalar width-fit

- `layout_pass_struct`: well-formed struct layout with non-overlapping
  fields and respected alignment -- accepted.
- `layout_reject_field_overlap`: two non-ZST fields whose byte
  intervals overlap -- rejected with `LayoutMalformed`.
- `layout_reject_field_offset_misaligned`: `field.offset %
  required_field_align != 0` -- rejected.
- `layout_reject_field_overflows_layout_size`: `field.offset +
  size_of(field.ty) > layout.size` -- rejected.
- `layout_pack_min_rule_pass`: `layout.pack == Some(2)` and the
  field alignment formula computes correctly -- accepted.
- `tag_scalar_width_fit_pass`: `valid_range.start = 0`, `end = 255`,
  `scalar_ty = Uint(U8)` -- accepted.
- `tag_scalar_width_fit_reject_overflow`: `valid_range.end = 256`,
  `scalar_ty = Uint(U8)` -- rejected with `TagScalarOutOfRange`.

### Const-graph acyclicity

- `const_graph_acyclic_pass`: a `Const::Aggregate` whose children
  form a tree -- accepted.
- `const_graph_dag_with_sharing_pass`: a `Const::Aggregate` whose
  two children both reference the same shared `ConstId` (DAG, not
  cycle) -- accepted (per `spec.md:847-854` "shared DAG children
  admissible").
- `const_graph_cycle_reject_self_loop`: a `Const::Aggregate` whose
  child is itself -- rejected with `ConstGraphCycle`.
- `const_graph_cycle_reject_two_node`: two `Const::Aggregate`s
  referencing each other -- rejected with `ConstGraphCycle`.
- `const_graph_cycle_reject_enum_value_payload`: a
  `Const::EnumValue { payload }` cycle through `payload` -- rejected.

### Domain-attr validation (`§3.6.5`)

- `domain_attr_item_attrs_len_pass`:
  `domain_attrs.item_attrs.len() == model.items.len()` -- accepted.
- `domain_attr_item_attrs_len_reject`: length mismatch -- rejected
  with `DomainAttrMalformed`.
- `domain_attr_field_path_pass_struct`: `FieldPath { item,
  variant_idx: 0, field_idx: 0 }` for a Struct Item -- accepted.
- `domain_attr_field_path_reject_non_adt_item`: `FieldPath.item`
  resolves to a `Function` Item -- rejected.
- `domain_attr_field_path_reject_variant_idx_out_of_range`: a Struct
  with `variant_idx == 1` -- rejected.
- `domain_attr_field_path_reject_field_idx_out_of_range`: in-range
  variant but `field_idx >= variant.fields.len()` -- rejected.
- `domain_attr_nested_id_pass`: every nested `ItemId` / `FnSigId` /
  `StringId` in `AttrData` payloads resolves -- accepted.
- `domain_attr_nested_id_reject_dangling`: a dangling `ItemId`
  inside an `AttrData::Intrinsic(_)` payload -- rejected.
- `intrinsic_singleton_pass`: zero or one `AttrData::Intrinsic(_)`
  entry per Item -- accepted.
- `intrinsic_singleton_reject_two_intrinsics`: two
  `AttrData::Intrinsic(_)` entries on the same Item -- rejected with
  `DomainAttrMalformed`.

### Source-file validation

- `source_file_pass`: synthesised sentinel at index 0; non-sentinel
  files have monotonic `line_starts` starting at 0; every
  `line_start <= content.len()` -- accepted.
- `source_file_reject_line_starts_non_monotonic`: a non-sentinel
  file with `line_starts == [0, 5, 3]` -- rejected with
  `SourceFileMalformed`.
- `source_file_reject_line_starts_does_not_start_at_zero`:
  `line_starts == [1, 5]` -- rejected.
- `source_file_reject_line_start_exceeds_content_len`: `line_starts
  == [0, 999]` for a 10-byte content -- rejected.
- `source_file_reject_duplicate_path`: two non-sentinel files with
  the same `path: StringId` -- rejected with
  `DuplicateSourceFilePath`.
- `source_file_pass_distinct_paths`: two non-sentinel files with
  distinct paths -- accepted.

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
Review the implementation of regseq-model-85 (P1a-VAL-03: Layout
consistency + invariants 14 / 19 / 20 + UNIT_LAYOUT + target / build
+ FnSig / CAbiLowering + const-graph acyclicity + domain-attr +
source-file validators) against its task spec at
.harness/tasks/regseq-model/85-layout-and-section-3-3-validator.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/model.rs (extended walk + new
  InvariantViolation variants)
- crates/regseq-model/src/layout_validator.rs (or wherever the new
  helpers land)
- crates/regseq-model/src/lib.rs (re-exports if any new module
  surfaces here)
- crates/regseq-model/tests/layout_validator.rs

Focus on:
- Spec deviations against spec.md §3.1 invariants 14 (lines 512-516),
  15 (lines 517-527), 17 (lines 543-548), 19 (lines 556-595), 20
  (lines 596-604); §3.3 step 3 (f) / (g), step 5 (lines 742-758),
  step 7, step 8 (lines 764-787), step 9 (lines 812-908), step 10
  (lines 909-951), and the Phase B continued block (lines 953-1102);
  §3.6.5 (lines 1716-1768); §11.5 / §12.5 layout sanity; §11.2
  tag-scalar width-fit; §14.3 / §14.5 FnSig / CAbiLowering; §13.4
  synthetic-domain.
- Phase B ordering (binding per spec.md:676-680).
- Trust model: VAL-03 does NOT re-prove rustc-side facts (per
  spec.md:629-656); it checks stored bridge outcomes.
- UNIT_LAYOUT check covers every Ty::Tuple { fields: [] } per
  spec.md:1050-1056.
- Const-graph acyclicity uses three-state DFS marks per
  spec.md:846-854.
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

- `RegseqModel::validate` (and any private helpers) extended to
  cover: invariant 14 (InstanceKind layout matches Ty nominal
  variant); invariant 19 model-checkable subset
  (`Item.opaque_handle_eligible`); invariant 20 (`StableCrateId(0)`
  synthetic-domain enforcement, including the `ClosureCaptures`
  carve-out per `spec.md:440-447`); canonical `UNIT_LAYOUT` check on
  every `Ty::Tuple { fields: [] }` per `spec.md:1050-1056`; target /
  build well-formedness (invariants 15 / 17); FnSig / CAbiLowering
  structural checks per `§3.3` step 5 and `§14.5`; layout sanity per
  `§11.5` / `§12.5`; tag-scalar width-fit per `§11.2`; const-graph
  acyclicity per `§3.3` step 9; domain-attr validation per `§3.6.5`;
  source-file validation (line-table consistency + duplicate-path
  rejection).
- `InvariantViolation` enum extended with the variants named in
  Implementation Notes (or equivalent typed variants pinning the
  same diagnostic information).
- Phase B ordering preserved per `spec.md:676-1102`.
- Test Plan covers each invariant with both positive and negative
  cases against constructed-model fixtures.
- `cargo test -p regseq-model` passes; Codex Task Review verdict is
  APPROVED.
