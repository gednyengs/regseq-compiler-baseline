# regseq-model-92: `render_instance` for ADT-definition variants + attribute-stream composition (P1a-PP-05b)

## Description

Complete the stage-14 composer `render_instance(InstanceId) -> String`
by implementing the **ADT-definition** `InstanceKind` variants
(`Struct`, `Enum`, `Union`) per `pretty-printer.md` §10.2.4 / §10.2.5
/ §10.2.6, and own the §10.2.7 two-stream attribute-block composition
helper (`render_domain_attribute_block`) consumed by all 6
`InstanceKind` variants for Stream 2.

Stream 2 walks `model.domain_attrs.item_attrs[item]` in Vec-index
order per `spec.md` §3.6.2's bridge-walk-order determinism rule
(D-14-3.3 LOCKED), dispatching per `AttrData` variant. Stage-14
ships exactly one `AttrData` variant rendering rule
(`Intrinsic(IntrinsicKind::Log(LogLevel))` ->
`#[rseq::intrinsic("log-<sev>")]`) per pretty-printer.md:4100-4106;
phase-2 stages append per-family rules to §10.2.7 (Mmio / Interrupt /
Atomic / etc.) but those are out of phase-1a scope.

This task closes out the §10.2 composer surface together with
P1a-PP-05a; both tasks share the same single `render_instance`
method.

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (per-task scope row
  P1a-PP-05b)
- `docs/discussions/regseq-model/pretty-printer.md` §1 (Mission), §2
  (Master rendering style), §5.6 (the shared
  `render_item_attribute_block` helper from P1a-PP-01), §6
  (`render_ty`), §6.7 (`render_generic_args(args, item)` from
  P1a-PP-02), §8 (Per-method ownership table), §10.2 dispatch
  overview (pretty-printer.md:3284-3417), §10.2.4 Struct variant
  rendering (pretty-printer.md:3813-3898), §10.2.5 Enum variant
  rendering (pretty-printer.md:3900-4003), §10.2.6 Union variant
  rendering (pretty-printer.md:4005-4067), §10.2.7 attribute-block
  composition (pretty-printer.md:4069-4199 -- canonical source for
  the two-stream interleaving and per-`AttrData` render rules), §9.11
  specimens 123 - 128 (ADT subset)
- `docs/discussions/regseq-model/spec.md` §3.1 (`RegseqModel.domain_attrs`),
  §3.6 (`DomainAttrTables` + `AttrData` closed-append-only union +
  `IntrinsicKind` + `LogLevel` + the dash-form NAME mapping at
  `spec.md:1542-1551`), §9.4 (`InstanceKind::Struct { layout }` /
  `Enum { layout }` / `Union { layout }`), §11 (`Layout` /
  `LayoutKind::Struct { fields }` / `LayoutKind::Enum { shape }` /
  `LayoutKind::Union { fields }` / `EnumShape` 3-variant /
  `VariantLayout` / `FieldLayout`)
- `crates/regseq-model/src/pretty.rs` (the existing `PrettyPrinter`
  shell amended by P1a-PP-01 through P1a-PP-05a)
- The `render_item_attribute_block` helper from P1a-PP-01 (Stream 1)
- The `render_def_path`, `render_ty`, `render_generic_args` helpers
  from earlier P1a-PP-* tasks
- The `Item` / `ItemKind` types from regseq-model-DS-07
- The `Instance` / `InstanceKind` types from regseq-model-DS-10
- The `DomainAttrTables` / `AttrData` / `IntrinsicKind` / `LogLevel`
  from regseq-model-DS-09 (P1a-DS-09)
- `.harness/tasks/regseq-model/24-place-and-projection.md` (size /
  shape reference)

## Implementation Notes

Edit `crates/regseq-model/src/pretty.rs`. This task fills in the
`Struct / Enum / Union` arms of the `render_instance` per-variant
match laid out by P1a-PP-05a, and adds the
`render_domain_attribute_block` internal helper.

### Internal helper -- §10.2.7 Stream 2

Per pretty-printer.md:4124-4139:

```rust
impl PrettyPrinter<'_> {
    /// Walks `model.domain_attrs.item_attrs[id]` in Vec-index order,
    /// dispatching per `AttrData` variant per the table at
    /// pretty-printer.md:4100-4106. Returns the multi-line block
    /// (with trailing `\n` when non-empty) or empty `String` when no
    /// entries. Per stage-14 D-14-5.6 LOCKED.
    fn render_domain_attribute_block(&self, id: ItemId) -> String;
}
```

Per-`AttrData` variant render rule (table at pretty-printer.md:4100-4106):

| AttrData variant | Output |
|---|---|
| `Intrinsic(IntrinsicKind::Log(LogLevel::Debug))` | `#[rseq::intrinsic("log-debug")]` |
| `Intrinsic(IntrinsicKind::Log(LogLevel::Info))` | `#[rseq::intrinsic("log-info")]` |
| `Intrinsic(IntrinsicKind::Log(LogLevel::Warn))` | `#[rseq::intrinsic("log-warn")]` |
| `Intrinsic(IntrinsicKind::Log(LogLevel::Error))` | `#[rseq::intrinsic("log-error")]` |

Walks `model.domain_attrs.item_attrs[id]` (a `Vec<AttrData>`) in
Vec-index order per `spec.md` §3.6.2's bridge-walk-order rule. The
dash-form NAME (`log-debug` / `log-info` / `log-warn` / `log-error`)
is the canonical intrinsic NAME from `spec.md` §3.6.3
`IntrinsicKind::from_name` reversed; the surface form
`#[rseq::intrinsic("<NAME>")]` mirrors the DSL-level recognition
shape per Q7 + stage-14 D-14-1 LOCKED.

The helper produces a multi-line block where each `#[...]` is on
its own line; ends with trailing `\n` when non-empty; empty
`String` otherwise. Same shape contract as `render_item_attribute_block`.

This helper is called from all 6 `InstanceKind` variant arms (the
3 Function / Static / Const arms in P1a-PP-05a and the 3 ADT arms
here). If P1a-PP-05a landed first with a placeholder helper that
returned the empty string, this task replaces that placeholder with
the full implementation.

### §10.2.4 Struct variant rendering (per D-14-5.5 LOCKED Option B; rules at pretty-printer.md:3813-3898)

`InstanceKind::Struct { layout }` resolves to `Ty::Struct { item,
args, layout = Layout { kind: LayoutKind::Struct { fields }, .. } }`
per `spec.md` §9.4 / §10. The renderer reads `item` and `args` from
the resolved Ty, and `fields` from `layout.kind.as_struct().fields`.

#### Dispatch on `(fields.len(), first-field-name-pattern per §3.4)`

| Case | Rendered form |
|---|---|
| `fields.is_empty()` | `[<attrs>\n]struct <path><args>;` (unit form -- collapses Rust's three syntactic empty-struct shapes) |
| All `fields[i].name` match `_<idx>` synthetic pattern | tuple form (one line) |
| `fields[i].name` carry real source names | named-field form (multi-line) |

Tuple form (one line; pretty-printer.md:3845-3849):

```
[<attribute-block>\n]struct <render_def_path(item)><render_generic_args(args, item)>(<render_ty(fields[0].ty)>, <render_ty(fields[1].ty)>, ...);
```

Named-field form (multi-line; pretty-printer.md:3851-3859):

```
[<attribute-block>\n]struct <render_def_path(item)><render_generic_args(args, item)> {
    <fields[0].name>: <render_ty(fields[0].ty)>,
    <fields[1].name>: <render_ty(fields[1].ty)>,
    ...
}
```

Field-name pattern check (per pretty-printer.md:3867): synthetic
`_<idx>` names (where `<idx>` is the field's position in `fields`)
indicate tuple-style. A Rust source struct never mixes tuple and
named fields, so a first-field check is sufficient (all fields share
the same style).

### §10.2.5 Enum variant rendering (per D-14-5.5 LOCKED Option B; rules at pretty-printer.md:3900-4003)

`InstanceKind::Enum { layout }` resolves to `Ty::Enum { item, args,
layout = Layout { kind: LayoutKind::Enum { shape }, .. } }`.

#### Dispatch on `shape: EnumShape` per `spec.md` §11.1

| EnumShape | Rendered form |
|---|---|
| `Empty` | `[<attrs>\n]enum <path><args> {}` (uninhabited; one-line empty body) |
| `Single { variant, .. }` | one variant arm inside `{ ... }` |
| `Multiple { variants, .. }` | per-variant arms inside `{ ... }`, comma-separated, trailing comma after last |

#### Per-variant arm (each `variant: VariantLayout`, dispatching on `(variant.fields.len(), first-field-name-pattern)`)

| Case | Rendered form |
|---|---|
| `variant.fields.is_empty()` | `<variant.name>` (unit variant) |
| All `variant.fields[i].name` synthetic | `<variant.name>(<render_ty(fields[0].ty)>, ...)` (tuple variant) |
| `variant.fields[i].name` real names | `<variant.name> { <fields[0].name>: <render_ty(fields[0].ty)>, ... }` (named-field variant) |

Multi-variant composition (multi-line; pretty-printer.md:3938-3945):

```
[<attribute-block>\n]enum <render_def_path(item)><render_generic_args(args, item)> {
    <variant_arm_0>,
    <variant_arm_1>,
    ...
    <variant_arm_N>,
}
```

Trailing comma after last variant arm matches Rust source idiom.

Single-variant composition (multi-line; pretty-printer.md:3949-3955):

```
[<attribute-block>\n]enum <render_def_path(item)><render_generic_args(args, item)> {
    <variant_arm>,
}
```

Variants may mix ctor forms (each variant renders per its own
field-name pattern; Rust source allows mixed shapes per
pretty-printer.md:3959-3967).

**v1 omits discriminant values** per pretty-printer.md:3969-3971:
`VariantLayout.discriminant` and `tag_value` are not surfaced as
`Variant = N,` syntax. Variants distinguished by name + body shape.

**v1 omits `EnumShape::Single { scalar: Some(_) }` tag-scalar surfacing**
per pretty-printer.md:3971: the single-variant fieldless `repr(int)`
enum case renders the variant arm normally; tag-scalar info is
IR-internal.

### §10.2.6 Union variant rendering (per D-14-5.5 LOCKED Option B; rules at pretty-printer.md:4005-4067)

`InstanceKind::Union { layout }` resolves to `Ty::Union { item, args,
layout = Layout { kind: LayoutKind::Union { fields }, .. } }`.

Unions carry only named-field bodies in Rust source (no tuple-union,
no unit-union); the renderer unconditionally produces the
named-field form (pretty-printer.md:4030-4036):

```
[<attribute-block>\n]union <render_def_path(item)><render_generic_args(args, item)> {
    <fields[0].name>: <render_ty(fields[0].ty)>,
    <fields[1].name>: <render_ty(fields[1].ty)>,
    ...
}
```

If `fields.is_empty()` (Rust source forbids it; the IR allows the
shape structurally), render `union <path><args> {}` -- no defensive
special-case at the renderer level. A model construction bug
surfaces at validation per `spec.md` §11 invariants, not at render
(per pretty-printer.md:4038-4040 and the brief's "no defensive
fallbacks" rule).

`LayoutKind::Union.fields` carries field offsets per `spec.md` §11
(all offsets are 0 since fields share the union's address per the
schema convention); offsets are NOT rendered in v1.

### §10.2.7 attribute-block composition for ADTs (rules at pretty-printer.md:4069-4199)

All three ADT-variant rendering forms above accept `[<attribute-block>\n]`
prefix per the §10.2.7 composition. Stage-14 ships no
`AttrData::Intrinsic` variant occupying Struct / Enum / Union Items
in v1 (intrinsic recognition targets Function Items per `spec.md`
§3.6.3 / D-14-1); the ADT-Instance attribute block reduces to
Stream 1 in v1. Common Stream 1 entries for ADTs: `#[lang = "phantom_data"]`,
`#[lang = "unsafe_cell"]`, `#[lang = "manually_drop"]`,
`#[opaque_handle]`, `#[link_section]`.

The composition rule (combined output layout per
pretty-printer.md:4141-4151):

```
<Stream 1 row 0>          <-- e.g., #[opaque_handle]
<Stream 1 row 1>          <-- e.g., #[lang = "manually_drop"]
...
<Stream 2 row 0>          <-- e.g., #[rseq::intrinsic("log-info")] (Function only in v1)
<Stream 2 row 1>
...
<kind-keyword line>       <-- starts at column 0
```

Empty cases:

- Both empty: no prefix; kind-keyword starts at column 0 (no leading
  `\n`).
- Stream 1 empty + Stream 2 non-empty: only Stream 2 rows; trailing
  `\n` after the last row.
- Stream 1 non-empty + Stream 2 empty: only Stream 1 rows; trailing
  `\n` after the last row.

### Cross-references

- §3.4 tuple-vs-named-field dispatch: synthetic `_<idx>` field names
  signal tuple-style; non-synthetic names signal named-field-style;
  empty fields signal unit-style. Same rule used by §10.1.3 / §10.1.4
  (P1a-PP-04b).
- `LayoutKind::Struct.fields[i]` is a `FieldLayout { name: StringId,
  ty: TyId, offset, .. }` per `spec.md` §11.0; field names resolve via
  `model.strings.resolve(name)`.
- `LayoutKind::Enum.shape.variants[i]` is a `VariantLayout { name:
  StringId, fields: Vec<FieldLayout>, .. }` per `spec.md` §11.1;
  variant names resolve via `model.strings.resolve(name)`.

## Non-Goals

- No re-implementation of Function / Static / Const variants (owned
  by P1a-PP-05a).
- No top-level `render(&self)` driver. Owned by P1a-PP-06.
- No PrettyPrintOptions struct definition or `with_options`
  constructor. Owned by P1a-PP-06.
- No future-stage `AttrData` variant render rules (Mmio / Interrupt /
  Atomic / etc.). Stage-14 commits only the `Intrinsic(Log(_))`
  variant per pretty-printer.md:4115-4122.
- No discriminant-value surfacing on Enum variants (`Variant = N,`).
  Per pretty-printer.md:3969-3971, v1 omits this.
- No `EnumShape::Single { scalar: Some(_) }` tag-scalar surfacing.
- No Union field offset rendering.
- No `render_ty` / `render_def_path` / `render_generic_args`
  reimplementation. Consumed via P1a-PP-02 / stage 12.
- No Item-level `render_item` or §5.6 helper reimplementation.
  Consumed via P1a-PP-01.
- No bridge-side ADT layout construction. Owned by axis 3
  (P1a-BR-*).

## Test Plan

Test discipline names `unit-in-src` (per-variant dispatch shape;
field-name-pattern-vs-synthetic detection; EnumShape dispatch;
Stream 2 helper per-`AttrData` rule) and `integration-in-tests`
(insta snapshots for §9.11 specimens 123 - 128).

### Unit tests (in `crates/regseq-model/src/pretty.rs`)

1. `render_domain_attribute_block` empty `item_attrs` -> empty
   `String`.
2. `render_domain_attribute_block` for each LogLevel: Debug / Info /
   Warn / Error -> `#[rseq::intrinsic("log-<sev>")]` with the
   correct dash-form name; trailing `\n` when non-empty.
3. `render_domain_attribute_block` Vec-index order: multiple
   AttrData entries render in declaration order.
4. `render_instance` Struct empty fields -> `struct <path><args>;`
   unit form (Specimen 123).
5. `render_instance` Struct tuple-style (synthetic `_0`, `_1`
   names) -> `struct <path><args>(T0, T1);` (Specimen 124).
6. `render_instance` Struct named-field -> multi-line
   `struct <path><args> {\n    name: T,\n    ...\n}` (Specimen 125).
7. `render_instance` Enum `EnumShape::Empty` -> `enum <path><args> {}`
   one-line (Specimen 127).
8. `render_instance` Enum `EnumShape::Single` with unit / tuple /
   named variant arms.
9. `render_instance` Enum `EnumShape::Multiple` with mixed-ctor
   variants (unit / tuple / named) -> trailing comma after last
   variant (Specimen 126).
10. `render_instance` Union named-field form (Specimen 128).
11. `render_instance` Union with empty fields -> `union <path><args> {}`
    (no defensive special-case; the structurally-allowed shape
    renders bare).
12. ADT attribute block: Stream 1 with lang-flag (`#[lang = "manually_drop"]`)
    on Struct (Specimen 125); Stream 1 with `#[lang = "unsafe_cell"]`
    on Union (the §10.2.6 example pattern).

### Integration tests (`crates/regseq-model/tests/pretty.rs`)

Add insta snapshot tests against the §9.11 specimens covered by this
task:

- Specimen 123: unit struct (empty fields).
- Specimen 124: tuple struct (synthetic `_N` field names).
- Specimen 125: named-field struct with lang-flag + generic args.
- Specimen 126: multi-variant enum with mixed ctor forms.
- Specimen 127: empty enum (uninhabited).
- Specimen 128: simple union.

Add `insta` to `crates/regseq-model/Cargo.toml` `[dev-dependencies]`
if not already present.

### Test-discipline tags

- `unit-in-src` -- per-variant dispatch shape + field-name-pattern
  detection + EnumShape dispatch + Stream 2 helper per-`AttrData`
  rule.
- `integration-in-tests` -- insta snapshots for §9.11 specimens
  123 - 128.

## Review

After tests pass, request a Codex review using `codex_runner.py`:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-92 (P1a-PP-05b) against
its task spec at .harness/tasks/regseq-model/92-render-instance-adt-and-attribute-streams.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/pretty.rs
- crates/regseq-model/tests/pretty.rs
- (any new insta snapshot files under crates/regseq-model/tests/snapshots/)

Focus on:
- Spec deviations: §10.2.4 Struct (3-row dispatch on
  fields.len()/first-field-name-pattern); §10.2.5 Enum (EnumShape
  dispatch + per-variant arm form + trailing comma); §10.2.6 Union
  (named-field unconditional form); §10.2.7 Stream 2 per-`AttrData`
  render table for the four LogLevel variants.
- §3.4 tuple-vs-named-field-vs-unit dispatch is consistent with
  P1a-PP-04b's ZST / Aggregate arms.
- v1 omits: discriminant values on Enum variants; tag-scalar
  surfacing for `EnumShape::Single { scalar: Some(_) }`; Union field
  offsets.
- No defensive fallbacks: empty Union fields render bare (not a
  special case); cannot-occur cases in `EnumShape::Empty` rendering
  do not exist (Empty has its own arm).
- `render_domain_attribute_block` walks Vec in index order; trailing
  `\n` when non-empty; empty when no entries.
- Test coverage vs spec Test Plan: every Specimen 123 - 128 has a
  matching insta snapshot; LogLevel dispatch covered.
Do NOT flag anything explicitly marked out of scope (Function /
Static / Const variants which belong to P1a-PP-05a; later task ID
P1a-PP-06; future-stage `AttrData` variants like Mmio / Interrupt /
Atomic).
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as hypotheses.
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

Iterate fix -> re-review until VERDICT: APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `Struct` / `Enum` / `Union` arms of `render_instance(InstanceId)`
  implemented per pretty-printer.md §10.2.4 / §10.2.5 / §10.2.6.
- `render_domain_attribute_block(ItemId) -> String` internal helper
  implemented per §10.2.7's Stream 2 contract; per-`AttrData`
  dispatch covers the four `IntrinsicKind::Log(LogLevel::*)`
  variants with the canonical dash-form NAMEs.
- All §9.11 specimens 123 - 128 have matching insta snapshot tests;
  outputs match verbatim.
- Unit tests cover Struct unit / tuple / named-field dispatch; Enum
  3-shape dispatch (Empty / Single / Multiple) with mixed-ctor
  variant arms; Union named-field unconditional form; the Stream 2
  helper per-LogLevel rendering and Vec-index walk order.
- `cargo test -p regseq-model`, `cargo fmt --all -- --check`, and
  `cargo clippy -p regseq-model -- -D warnings` all pass.
- Codex Task Review verdict APPROVED.
