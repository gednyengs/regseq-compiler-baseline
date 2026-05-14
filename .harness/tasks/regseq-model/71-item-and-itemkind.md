# regseq-model-71: `Item` + `ItemKind` + Item Attribute Flags

## Description

Land the source-level item alphabet from spec.md §8 (spec.md:2112-2386):
the `Item` struct (spec.md:2128-2203 -- 11 fields covering `def_path`,
`kind`, `span`, plus the cross-variant Item-level metadata
`linkage` / `link_section` / `inline_hint` / `is_unsafe` / lang-item
flags `is_phantom_data` / `is_unsafe_cell` / `is_manually_drop` /
bridge-computed `opaque_handle_eligible`), the 6-variant closed
`ItemKind` enum (spec.md:2205-2212), the 4-variant closed `Linkage`
enum (spec.md:2223-2233), and the 4-variant closed `InlineHint` enum
(spec.md:2238-2248). This task replaces the placeholder `Item` in
`crates/regseq-model/src/placeholders.rs` with the spec-mandated
body.

`Item` is **one Item per rustc DefId** (1:1 with source-level
definitions). For generic items, the Item carries source-level
traceback (DefPath, kind, attributes, span); per-monomorphisation
data (substituted FnSig, Body, layout) lives on `Instance` per Q2
(P1a-DS-10). `Items` table is the **pre-mono source-level alphabet**;
`Instances` is the **per-mono emission unit**.

The `opaque_handle_eligible: bool` field is the bridge-computed
global-use outcome for the opaque-handle C lowering convention
per spec construction invariant 19; v1 restricts to **non-generic**
Items. Per-condition consistency is enforced by P1a-VAL-03.

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (this task is
  P1a-DS-07)
- `docs/discussions/regseq-model/spec.md` §8 (spec.md:2112-2386);
  §3.1 invariant 19 (`opaque_handle_eligible` consistency,
  spec.md:556-595); §15.1 (rejection rows, e.g., `#[naked]` per
  D-14-7 -- documentation-only here)
- `crates/regseq-model/src/def_path.rs` (`DefPath` -- §7)
- `crates/regseq-model/src/source.rs` (`Span` -- §5)
- `crates/regseq-model/src/ty.rs` (P1a-DS-02 -- `Mutability`)
- `crates/regseq-model/src/ids.rs` (`StringId`, `ItemId`)
- `crates/regseq-model/src/model.rs` (P1a-DS-01 -- `RegseqModel`
  hosts `items: IndexVec<ItemId, Item>`)
- `crates/regseq-model/src/placeholders.rs` (existing `Item`
  placeholder to replace)
- `.harness/tasks/regseq-model/65-root-regseq-model-and-foundation.md`
- `.harness/tasks/regseq-model/66-ty-enum-and-helpers.md`
- `.harness/tasks/regseq-model/68-ty-interner-and-args-key.md`

## Implementation Notes

Create new module `crates/regseq-model/src/item.rs` (or
`item/mod.rs`). Re-export from `lib.rs`. Replace the `Item`
placeholder in `placeholders.rs` with a re-export from the new
module (or remove the placeholder entirely if no other module
references it).

### Public API (verbatim from spec)

#### `Item` struct (spec.md:2128-2203)

```rust
pub struct Item {
    pub def_path: DefPath,
    pub kind:     ItemKind,
    pub span:     Span,

    // Cross-variant Item-level metadata (D-12-1.3).

    /// Symbol-emission classifier. Populated from §3.4 D-12-2.3
    /// per-DefId mapping.
    pub linkage:        Linkage,

    /// ELF section placement (e.g., ".init_array",
    /// ".driver_data"). Bridge populates from rustc's
    /// `CodegenFnAttrs.link_section`.
    pub link_section:   Option<StringId>,

    /// `#[inline]` / `#[inline(always)]` / `#[inline(never)]`
    /// per rustc's `CodegenFnAttrs.inline: InlineAttr`.
    /// Defaults to `None` for non-Function ItemKind variants.
    pub inline_hint:    InlineHint,

    /// `unsafe fn`. Consumed by analyses; not load-bearing for C
    /// codegen.
    pub is_unsafe:      bool,

    // No `is_naked` field. Per stage-14 D-14-7, `#[naked]` is
    // rejected at Item translation (§15.1 row); no representable
    // IR shape exists for naked functions in v1.

    // Lang-item flags (§8.2). Load-bearing for codegen lowering
    // decisions; populated from rustc's `AdtFlags`.
    pub is_phantom_data:  bool,
    pub is_unsafe_cell:   bool,
    pub is_manually_drop: bool,

    /// Bridge-computed global-use outcome for the opaque-handle
    /// C lowering convention (§14.5). v1 restricts eligibility
    /// to **non-generic** Items: when `true`, the Item is
    /// `ItemKind::Struct`, `repr(C)`, has no generic parameters,
    /// its monomorphic `Ty::Struct.layout` is ZST, and every
    /// reachable use of that monomorphic `TyId` is behind
    /// `Ref` / `Ptr`. False otherwise.
    /// Generic opaque-handle candidates (`Handle<T>`-style) are
    /// rejected at the bridge with
    /// `BridgeError::GenericOpaqueHandle`.
    pub opaque_handle_eligible: bool,
}
```

#### `ItemKind` (spec.md:2205-2212)

```rust
pub enum ItemKind {
    Function,
    Static { mutability: Mutability },
    Const,
    Struct,
    Enum,
    Union,
}
```

The 6 variants close per spec.md:2356-2362 (no `Impl`, `Trait`, or
`AssocType` -- those lift to top-level Function / Const Items per
§8.1). `Static.mutability` lives here per stage-12 D-12-1.2 (c)
(source-level keyword choice; statics can't be generic so 1:1
Item:Instance always; mutability moved off InstanceKind to ItemKind).

#### `Linkage` (spec.md:2223-2233)

```rust
#[repr(u8)]
pub enum Linkage {
    /// Ordinary defined fn/static; mangled name.
    Default,
    /// `#[no_mangle]` or `#[export_name]`; symbol_name overrides.
    NoMangle,
    /// `extern "C" { ... }` declaration; never defined locally.
    Foreign,
    /// `#[linkage = "weak"]`; tentative definition.
    Weak,
}
```

Closed 4-variant set per spec.md:2214-2222 (other rustc Linkage
variants are LLVM-specific and rejected at the bridge with
`BridgeError::UnsupportedLinkage`).

#### `InlineHint` (spec.md:2238-2248)

```rust
#[repr(u8)]
pub enum InlineHint {
    /// No `#[inline]` annotation.
    None,
    /// `#[inline]`.
    Hint,
    /// `#[inline(always)]`.
    Always,
    /// `#[inline(never)]`.
    Never,
}
```

Closed 4-variant mirror of rustc's `InlineAttr` per spec.md:2236-2237.
Defaults to `InlineHint::None` for non-Function ItemKind variants.

### Per-`ItemKind` data placement (documentation)

Per spec.md:2251-2273 per-monomorphisation rich data lives on
`Instance` per Q2 / Option B:

- `InstanceKind::Function { fn_sig: FnSigId, body: Option<Body>,
  flavor: FunctionInstanceFlavor, virtual_idx: Option<u32> }`
  (P1a-DS-10).
- `InstanceKind::Static { ty: TyId, init: Option<ConstId> }` --
  mutability lives on `ItemKind::Static { mutability }` per D-12-1.2 (c).
- `InstanceKind::Const { ty: TyId, value: ConstId }`.
- `InstanceKind::{Struct, Enum, Union} { layout: TyId }`.

This task ships the **Item-side metadata only**; Instance shape
lands at P1a-DS-10.

### Storage rule for attribute-derived metadata (documentation)

Per spec.md:2275-2330 the typed-Item-field surface (`linkage`,
`link_section`, `inline_hint`, `is_unsafe` + 3 lang-item flags +
`opaque_handle_eligible`) holds the typed source-level metadata.
Declarative DSL property attributes (`#[rseq::*]`) live in
`DomainAttrTables` (P1a-DS-09); recognised Rust attributes
(`#[inline]`, `#[link_section]`, `#[no_mangle]`, ...) and language
built-ins (lang-items) land in typed Item fields per D-12-1.3.

The `#[rseq::intrinsic("NAME")]` operations attribute is recognized
per stage-14 D-14-1; its resolved identity lives on `RegseqModel`
as an `AttrData::Intrinsic(IntrinsicKind)` entry in
`domain_attrs.item_attrs[item_id]` per §3.6 (P1a-DS-09).

### §8.1 lifting `impl` items (documentation)

Per spec.md:2334-2362 `impl` blocks are source-level scopes, not
emittable units. RegseqModel does not represent them as Items.
Their contents lift:

- **Inherent impl methods** -> top-level Function Items.
- **Trait impl methods** -> top-level Function Items + (for dynamic
  dispatch only) vtables synthesised as Static Instances.
- **Associated constants** -> top-level Const Items.
- **Associated types** -> erased; uses substituted at
  monomorphisation.
- **Trait declarations** -> erased; static dispatch resolves to
  the implementing function's Instance, dynamic dispatch goes
  through a vtable.
- **Default trait method bodies** -> owned by the trait, lives at
  the trait's DefPath.

Document on the `ItemKind` enum's doc-comment block.

### §8.2 lang-item flags (documentation)

Per spec.md:2363-2384 the bridge populates flags via
`adt_def.is_*()` predicates:

| Flag               | Recognition                  | rustc cache                  |
| ------------------ | ---------------------------- | ---------------------------- |
| `is_phantom_data`  | `adt_def.is_phantom_data()`  | `AdtFlags::IS_PHANTOM_DATA`  |
| `is_unsafe_cell`   | `adt_def.is_unsafe_cell()`   | `AdtFlags::IS_UNSAFE_CELL`   |
| `is_manually_drop` | `adt_def.is_manually_drop()` | `AdtFlags::IS_MANUALLY_DROP` |

Lang-items NOT flagged on `Item` (recognised at call/use time, or
not in v1 scope): `MaybeUninit`, `CVoid`, `Pin`, `OwnedBox`,
`VaList`, `GlobalAlloc`, `UnsafePinned`. Document on the
respective `is_*` field doc-comments.

### Conventions

- **Closed-enum discipline**: `ItemKind`, `Linkage`, `InlineHint`
  are closed; no `#[non_exhaustive]`, no wildcard arms.
- **`Linkage` and `InlineHint`** are `#[repr(u8)]` for stable
  serialised discriminants.
- **`Item` derives**: `Clone, Debug, PartialEq, Eq, Hash` for
  full-value structural equality; `Serialize, Deserialize`. Per
  the brief's cross-cutting note, `Item` carries `StringId` (via
  `link_section: Option<StringId>` and via `def_path.rendered:
  StringId`) so per the rule "types containing `Symbol`/`StringId`
  are deferred from serde until resolution-time pass lands", serde
  is technically deferred. EXCEPTION (same as P1a-DS-03's Layout
  reasoning): `Item` is referenced by `RegseqModel.items:
  IndexVec<ItemId, Item>` which derives serde at the root, so this
  task ships construction-shape serde here and lets the resolution
  pass refine.
- **Doc comments on every public type / variant / field**.
- **No unicode in comments / docs**.

### `opaque_handle_eligible` consistency (invariant 19, documentation)

Per spec.md:556-595 the invariant requires (when
`opaque_handle_eligible == true`):

- `Item.kind == ItemKind::Struct` (model-checkable).
- Source-side `adt_def.repr()` is `C` (bridge certificate).
- Item is non-generic (model-checkable).
- Monomorphic `Ty::Struct.layout` is ZST (`size: 0, align: 1`)
  (model-checkable).
- Every reachable use of the monomorphic `TyId` is behind
  `Ref`/`Ptr` (model-checkable).

This task ships the `opaque_handle_eligible: bool` field with a
doc-comment listing the 5 conditions; **enforcement** lives at
P1a-VAL-03 (the model-checkable subset; the source-side `repr(C)`
is bridge-certified per the §3.3 trust model).

## Non-Goals

- **No `Instance` body shape** -- P1a-DS-10 owns
  `InstanceKind::Function`'s `fn_sig: FnSigId, body: Option<Body>,
  flavor: FunctionInstanceFlavor, virtual_idx: Option<u32>` etc.
- **No `DomainAttrTables` / `IntrinsicKind`** -- P1a-DS-09 owns
  the sibling tables architecture.
- **No `SyntheticItemKind`** -- P1a-DS-08b. `Item.def_path` may
  carry a synthetic shape per §13.4, but the `SyntheticItemKind`
  side table is separate from `Item` and lands at DS-08b.
- **No bridge translation rules** -- P1a-BR-* axis (BR-01 covers
  Phase 4 ItemId assignment + side-table population).
- **No `opaque_handle_eligible` validator** -- P1a-VAL-03 enforces
  invariant 19.
- **No `linkage` / `inline_hint` / lang-item flag population
  rules** at the bridge -- P1a-BR-01 / BR-02 / BR-03b.
- **No `BridgeError::*` enum** -- bridge crate (P1a-BR-* axis).
- **No `is_naked` field** -- per D-14-7 `#[naked]` is rejected at
  §15.1 Item translation; no IR-side representation exists.
- **No pretty-printer** -- P1a-PP-01 owns `render_item` /
  `render_item_attribute_block` per pretty-printer.md §5.

## Test Plan

`unit-in-src` discipline. Tests live in
`crates/regseq-model/src/item.rs::tests`. Per `SESSION.md` Testing
Requirements, every public type / function / field gets at least one
test.

### `ItemKind` constructor + variant tests

6 tests, one per variant:

- `ItemKind::Function` constructs (no payload).
- `ItemKind::Static { mutability: Mutability::Not }` constructs;
  same with `Mutability::Mut`.
- `ItemKind::Const` constructs.
- `ItemKind::Struct` constructs.
- `ItemKind::Enum` constructs.
- `ItemKind::Union` constructs.

### `Linkage` constructor + round-trip tests

- All 4 variants construct: `Default`, `NoMangle`, `Foreign`, `Weak`.
- `#[repr(u8)]` discriminants are stable: `Default == 0`,
  `NoMangle == 1`, `Foreign == 2`, `Weak == 3` (verify via `as u8`).
- Round-trip through serde preserves the discriminant.

### `InlineHint` constructor + round-trip tests

- All 4 variants construct: `None`, `Hint`, `Always`, `Never`.
- `#[repr(u8)]` discriminants: `None == 0`, `Hint == 1`, `Always
  == 2`, `Never == 3`.
- Round-trip through serde preserves the discriminant.
- Default for non-Function ItemKind: when constructing an `Item`
  with `kind: ItemKind::Static { ... }`, the test convention is
  that callers pass `inline_hint: InlineHint::None` (no auto-
  default; the field is mandatory). Document this convention; if a
  builder helper `Item::default_for(kind)` is provided, it returns
  `InlineHint::None` for non-Function variants.

### `Item` constructor tests (all 11 fields)

Construct an `Item` exercising every field with both the default
and a non-default value:

- A function Item: `Item { def_path: <fixture>, kind:
  ItemKind::Function, span: <fixture>, linkage: Linkage::Default,
  link_section: None, inline_hint: InlineHint::None, is_unsafe:
  false, is_phantom_data: false, is_unsafe_cell: false,
  is_manually_drop: false, opaque_handle_eligible: false }`.
- An `unsafe fn`: `is_unsafe: true`.
- An `#[inline(always)]` function: `inline_hint: InlineHint::Always`.
- A static with a section: `kind: ItemKind::Static { mutability:
  Mutability::Not }, link_section: Some(strings.intern(".rodata"))`.
- An `#[no_mangle]` function: `linkage: Linkage::NoMangle`.
- A `PhantomData<T>` struct Item: `kind: ItemKind::Struct,
  is_phantom_data: true`.
- An `UnsafeCell<T>` struct Item: `is_unsafe_cell: true`.
- A `ManuallyDrop<T>` struct Item: `is_manually_drop: true`.
- An opaque-handle struct Item: `kind: ItemKind::Struct,
  opaque_handle_eligible: true`.

### `Item` equality / hash

- Two `Item`s with identical content are equal and hash equal.
- Two `Item`s differing only in `linkage` are not equal.
- Two `Item`s differing only in `inline_hint` are not equal.
- Two `Item`s differing only in `is_unsafe` are not equal.
- Two `Item`s differing only in `opaque_handle_eligible` are not
  equal.
- Two `Item`s differing only in `link_section` (`Some` vs `None`)
  are not equal.

### `Item` field destructure exhaustiveness

```rust
fn _exhaustiveness_check(item: &Item) {
    let Item {
        def_path: _,
        kind: _,
        span: _,
        linkage: _,
        link_section: _,
        inline_hint: _,
        is_unsafe: _,
        is_phantom_data: _,
        is_unsafe_cell: _,
        is_manually_drop: _,
        opaque_handle_eligible: _,
    } = item;
}
```

A future field addition without updating this scaffold breaks the
build.

### Serde round-trip

- A constructed `Item` round-trips through `serde_json` losslessly
  (every field preserved).
- An `Option<StringId>` `link_section` round-trips both `Some(_)`
  and `None`.

### `ItemKind` exhaustiveness scaffold

```rust
#[allow(dead_code)]
fn _itemkind_exhaustiveness(k: &ItemKind) {
    match k {
        ItemKind::Function
        | ItemKind::Static { .. }
        | ItemKind::Const
        | ItemKind::Struct
        | ItemKind::Enum
        | ItemKind::Union => (),
    }
}
```

### `Linkage` and `InlineHint` exhaustiveness scaffolds

```rust
#[allow(dead_code)]
fn _linkage_exhaustiveness(l: &Linkage) {
    match l {
        Linkage::Default
        | Linkage::NoMangle
        | Linkage::Foreign
        | Linkage::Weak => (),
    }
}

#[allow(dead_code)]
fn _inline_hint_exhaustiveness(h: &InlineHint) {
    match h {
        InlineHint::None
        | InlineHint::Hint
        | InlineHint::Always
        | InlineHint::Never => (),
    }
}
```

### Integration with `RegseqModel.items`

- Construct a `RegseqModel` (per P1a-DS-01); push 3 `Item`s into
  `model.items`; verify each round-trips through `model.items[id]`.
- Verify `RegseqModel::validate(&model, &BridgeInput::matching(&model))`
  passes when `items` has matching `item_def_paths` length (the
  existing legacy validator coverage continues to hold).

### `opaque_handle_eligible` documentation test

Hand-construct an `Item { kind: ItemKind::Struct,
opaque_handle_eligible: true, .. }`. The `Item` constructs and
round-trips through serde without rejection (data-shape only;
enforcement lives at P1a-VAL-03). Document in the test that the
five conditions per spec.md:556-595 are validator territory.

### Forward stub removal

Verify the `Item` placeholder in
`crates/regseq-model/src/placeholders.rs` is removed (or replaced
with a re-export). The new `Item` body integrates with the existing
`pub items: IndexVec<ItemId, Item>` field on `RegseqModel`.

## Review

After tests pass, request a Codex Task Review per `SESSION.md`
Review section. Use `scripts/python/codex_runner.py` directly via
the Bash tool with the four-block template:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-71 against its task spec at
.harness/tasks/regseq-model/71-item-and-itemkind.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/item.rs (or item/*.rs)
- crates/regseq-model/src/lib.rs (re-exports)
- crates/regseq-model/src/placeholders.rs (Item stub removed
  or replaced)
- crates/regseq-model/src/model.rs (items field type-checks
  against the new Item)

Focus on:
- Spec deviations (every rule in the Implementation Notes section,
  including the spec-quoted public-API signatures: 11-field Item,
  6-variant ItemKind, 4-variant Linkage with #[repr(u8)],
  4-variant InlineHint with #[repr(u8)]).
- ABSENCE of `is_naked` field per stage-14 D-14-7.
- `Static.mutability` lives on `ItemKind::Static`, NOT on
  `InstanceKind::Static` (per stage-12 D-12-1.2 (c)).
- Linkage discriminants stable: Default=0, NoMangle=1, Foreign=2,
  Weak=3.
- InlineHint discriminants stable: None=0, Hint=1, Always=2, Never=3.
- Lang-item flags (`is_phantom_data`, `is_unsafe_cell`,
  `is_manually_drop`) are bool fields with doc-comments naming the
  rustc `AdtFlags` source per spec.md:2370-2384.
- `opaque_handle_eligible: bool` field doc-comment lists the 5
  conditions per spec.md:556-595 with explicit non-generic +
  ZST + reachable-use-behind-Ref/Ptr requirements.
- Closed-enum discipline on ItemKind / Linkage / InlineHint;
  exhaustiveness scaffolds present.
- Test coverage vs the task's Test Plan (per-variant constructors,
  Linkage/InlineHint discriminant stability, Item full-field
  equality, destructure exhaustiveness, serde round-trip,
  RegseqModel integration, exhaustiveness scaffolds).
Do NOT flag anything explicitly marked out of scope (Non-Goals:
P1a-DS-10 owns InstanceKind body; P1a-DS-09 owns DomainAttrTables;
P1a-DS-08b owns SyntheticItemKind; P1a-VAL-03 owns invariant 19
enforcement; P1a-BR-* owns bridge population).
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

- 11-field `Item` lands per spec.md:2128-2203, including
  `linkage`, `link_section`, `inline_hint`, `is_unsafe`, the 3
  lang-item flags, and `opaque_handle_eligible`.
- 6-variant closed `ItemKind` lands per spec.md:2205-2212 with
  `Static.mutability` on the variant payload.
- 4-variant closed `Linkage` lands per spec.md:2223-2233 with
  `#[repr(u8)]` discriminants 0..=3.
- 4-variant closed `InlineHint` lands per spec.md:2238-2248 with
  `#[repr(u8)]` discriminants 0..=3.
- The `Item` placeholder in `placeholders.rs` is removed (or
  replaced with a re-export).
- `Item` derives `Clone, Debug, PartialEq, Eq, Hash, Serialize,
  Deserialize`; full-field destructure exhaustiveness scaffold
  present.
- Doc comments on every variant / field; lang-item recognition
  table per spec.md:2370-2384 documented; opaque-handle
  consistency conditions per spec.md:556-595 documented; absence
  of `is_naked` per D-14-7 documented.
- Closed-enum exhaustiveness scaffolds present for ItemKind /
  Linkage / InlineHint.
- All tests pass (`cargo test -p regseq-model`).
- Codex Task Review verdict: APPROVED.
