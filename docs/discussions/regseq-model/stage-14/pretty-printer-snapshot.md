# Pretty-printer

> **Snapshot at close of stage 14 (2026-05-13).** This file is
> frozen as a historical artifact preserving the stage-14
> contribution. The current pretty-printer spec lives in
> top-level `pretty-printer.md`.

This file is the canonical specification for the
RegseqModel pretty-printer. It is a co-equal merger
artifact alongside `spec.md`: the pretty-printer's
rendering style, per-renderer specifications,
ownership table, sigil conventions, and worked-example
specimens live here; the IR's structural definitions
(`Ty` enum, Layout, `Item` table, etc.) live in
`spec.md`. Cross-references between the two files are
explicit.

For the rationale, prior-art evidence, and per-decision
audit trail behind the spec content here, see
`discussions.md` (top-level merged) and per-stage
`stage-N/discussions.md`.

## 1. Mission and consumer model

### 1.1 Mission

The pretty-printer renders `RegseqModel` values to
human-readable text. Per architectural decision Q5,
it is the primary serialization (the textual MIR-
inspired form is the human-readable contract; serde
JSON is the secondary structured-format).

The renderer is **context-bearing**: per-method
rendering requires `&RegseqModel` for ID resolution
(types, items, instances, strings, source files all
resolve through the model's tables). The shape is
methods on a `PrettyPrinter<'a>` struct returning
`String`.

### 1.2 Consumer model

Three primary consumers:

1. **Insta snapshot tests** in `crates/regseq-model/
   tests/pretty.rs`. The default consumer; default-off
   span / debuginfo gating keeps snapshot baselines
   compact. Worked-example specimens (§9) are 1:1
   mirrored to insta tests.
2. **Diagnostic / debug output** during bridge
   construction. Opt-in span and debuginfo gating
   surfaces per-construct source positions for
   diagnosis. Synthetic-construct spans are suppressed
   to keep bridge-synthesis (vtable shims, drop glue,
   ConstMemoryStatic) outputs clean.
3. **Phase-2 device-pressure golden output** for driver
   fixtures (pl011, e1000, etc.). Pretty-print output
   anchors the C-codegen-vs-RegseqModel trace; same
   default-off gating applies.

### 1.3 Building blocks vs composers

The pretty-printer's API surface splits into two
classes by ownership stage:

**Building blocks (consumed by composers).** Per-construct
renderers each rendering one schema construct. Per-method
ownership splits across stages 1, 12, and 13 (see §8 for
the canonical ownership table); the building-block tag
is about consumer relationship, not ownership.

Stage-13-owned (this stage's contribution):
- `render_ty(TyId) -> String` -- the closed 19-variant
  `Ty` enum (`spec.md` §10).
- `render_item(ItemId) -> String` -- `Item` wrapper
  (attribute block + kind-keyword + DefPath); does NOT
  carry per-Instance content.
- 9 public body-level renderers: `render_place`,
  `render_operand`, `render_rvalue`, `render_statement`,
  `render_terminator`, `render_call_callee`,
  `render_switch_targets`, `render_assert_kind`,
  `render_basic_block`.
- 1 internal helper: `render_terminator_kind`.

Stage-12-owned (inherited; not stage-13-touched):
- `render_def_path(ItemId) -> String` -- delegates to
  the eager cache `Item.def_path.rendered` (one-line
  cache lookup; D-12-6).

Stage-1-shipped (inherited; not stage-13-touched):
- `render_def_path_value(&DefPath) -> String` -- value-
  level helper.
- `render_span(Span) -> String` -- `<path>:<lo_line>:<lo_col>:
  <hi_line>:<hi_col>` form.

**Composers (stage-14-owned).** Higher-level renderers
that compose building blocks plus per-Instance content:

- `render_const(ConstId, TyId) -> String` -- pair-form
  takes the consumer-context `TyId`; composes
  `render_ty` for type-context arms. Per D-14-4.1
  (LOCKED), the `TyId` is the type of the position
  holding the `ConstId` (e.g.,
  `InstanceKind::Static.ty`, the per-field Ty of an
  Aggregate child resolved from the parent's layout).
- `render_instance(InstanceId) -> String` -- per
  D-14-5.7 LOCKED, composes per-Instance content
  from scratch (does NOT call `render_item`); shares
  only the §5.6 internal helper
  `render_item_attribute_block` for the Stream 1
  attribute prefix. Per-variant composition:
  `InstanceKind::Function.fn_sig` rendered via
  `render_ty`; `InstanceKind::Function.body` rendered
  via the body envelope + body-level renderers;
  `InstanceKind::Static.{ty, init}` via `render_ty` +
  `render_const`; `InstanceKind::Const.value` via
  `render_const`; `InstanceKind::{Struct, Enum,
  Union}.layout` via the §11 Layout walk per
  D-14-5.5 (Option B full definition body). See
  §10.2.1-§10.2.6 per-variant subsections + §10.2.7
  attribute-block composition rule.
- `render(&self) -> String` -- top-level model dump.
  Walks `self.model.instances.iter_enumerated()` in
  InstanceId order (canonical F4-2 sort per `spec.md`
  §16.3; rank-grouped via `regseq_instance_kind_rank`
  per §9.3). Composes per-Instance via the §10.2
  `render_instance(InstanceId)` composer. Single blank
  line between consecutive Instances; no file-level
  header / trailer; empty `model.instances` -> empty
  string. See §10.3 for the full per-arm spec.

The split reflects that Items are pre-mono (1:1 with
rustc DefIds) while Instances are post-mono emission
units; rendering an Item-level view standalone
(`render_item`) is useful for diagnostics and Item-table
summaries, while the headline consumer-facing renders
are Instance-keyed.

## 2. Master rendering style

### 2.1 Rust-source-style with sigil decorations

Pretty-print output follows Rust source language style
with sigil decorations for IR-internal identifiers.
Types render as `&mut T`, `[T; N]`, `*const T`,
`dyn Trait + Send`, `(T1, T2)`, etc. (RegseqModel does
not carry lifetime regions per `spec.md` §10.1; refs
render without `'a` tokens.) Bodies render as
`fn $path(args) -> Ret { let _N: T; bbN: { ... } }` --
mirroring rustc's `--emit=mir` baseline.

This style is the load-bearing master decision; every
renderer specification in §3 through §7 inherits it.
Per-renderer style dissent is rejected by construction.

### 2.2 PrettyPrinter shape

```rust
pub struct PrettyPrinter<'a> {
    model: &'a RegseqModel,
    opts:  PrettyPrintOptions,
}

#[derive(Default)]
pub struct PrettyPrintOptions {
    pub include_spans:        bool,
    pub include_debug_names:  bool,
}

impl<'a> PrettyPrinter<'a> {
    pub fn new(model: &'a RegseqModel) -> Self {
        Self::with_options(model, PrettyPrintOptions::default())
    }
    pub fn with_options(model: &'a RegseqModel, opts: PrettyPrintOptions) -> Self {
        Self { model, opts }
    }
    // ... render methods (§3 through §7)
}
```

`new(model)` keeps its existing signature (defaults all
options to off). `with_options(model, opts)` is the
opt-in constructor for diagnostic / debug consumers.

### 2.3 Method shape

Each renderer is a method on `PrettyPrinter<'a>`
returning `String`. Per-method dispatch internally is
a `match` over the input value's variants; per-arm
output composes via `format!` or a `String` builder.

The `String`-return shape is acceptable for the v1
consumers (snapshot tests + diagnostics); a future
`&mut String` / `&mut W: Write` overload is non-breaking
extension if a hot-path consumer surfaces.

## 3. Body envelope, indentation, sigil conventions

### 3.1 Body envelope

A function body renders as:

```text
fn $path(_1: ArgTy1, _2: ArgTy2, ...) -> RetTy {
    let mut _0: RetTy;
    let mut _3: T;
    let _4: T;
    ...
    debug NAME => _N;            <-- when include_debug_names=true
    debug OTHER_NAME => _M;

    bb0: {
        <stmt>;
        <stmt>;                  // <render_span>  <-- when include_spans=true
        <terminator> -> [<successor_labels>];
    }

    bb1: {
        ...
    }
}
```

Composition:
- `fn $path(args) -> Ret` is the **function header**
  rendered by `render_item` + per-Instance signature
  composition (stage 14 `render_instance` for
  `InstanceKind::Function`). Stage 13 owns the body
  content inside `{ ... }`.
- The locals declaration block is one declaration per
  line: `let mut _N: <render_ty(T)>;` for mutable
  locals, `let _N: <render_ty(T)>;` for immutable.
  RETURN_PLACE always emits as `let mut _0: <ret_ty>;`.
- `debug NAME => _N;` lines render only when
  `include_debug_names=true`, after the locals
  declarations block, one per local with
  `LocalDecl.debug_name = Some(name)`.
- Each basic block renders as `bbN: { stmt;
  ...; terminator -> [labels]; }`.
- Trailing `;` is per-statement; the terminator emits
  its own successor labels (no trailing `;` after
  the closing `]`).

### 3.2 Indentation

`INDENT = "    "` (4 spaces). Block contents inside
`bbN: {}` render at 8-space indent (envelope's 4 +
INDENT step 4). Function body opens at column 0.

### 3.3 Sigil conventions

| ID type        | Sigil                                             | Wiring                                                                |
|----------------|---------------------------------------------------|-----------------------------------------------------------------------|
| `ItemId`       | rendered via `render_def_path(ItemId)` -> path    | path string from `Item.def_path.rendered` cache                       |
| `TyId`         | rendered via `render_ty(TyId)` -> Rust-source-style | recursive on `Ty` variants per §6                                      |
| `ConstId`      | rendered via `render_const(ConstId, TyId)` (stage 14) | per-variant value form; pair-form takes the consumer-context `TyId`; consumes `render_ty` for type-context arms     |
| `InstanceId`   | rendered via `render_instance(InstanceId)` (stage 14) | per D-14-5.7 LOCKED, composes `render_def_path` + per-`InstanceKind` content + the §5.6 shared `render_item_attribute_block` helper for Stream 1; does NOT call `render_item` |
| `LocalId`      | `_N` (zero-based; `_0` is RETURN_PLACE)           | inline `format!("_{}", id.index())` at `render_place` call sites       |
| `BlockId`      | `bbN`                                             | inline `format!("bb{}", id.index())` at `render_basic_block` header   |
| `SourceFileId` | rendered as the source-file path                  | resolved via `model.source_files[id].path` -> string                   |
| `StringId`     | rendered as the resolved string                   | resolved via `model.strings.resolve(id)`                                |
| `FieldIdx`     | resolved to field name via `place_ty` walk        | layout name lookup; `FieldLayout.name: StringId` non-optional per §11  |
| `VariantIdx`   | resolved to variant name via `place_ty` walk      | enum-shape lookup; `VariantLayout.name: StringId` non-optional per §11 |
| `Span`         | `<path>:<lo_l>:<lo_c>: <hi_l>:<hi_c>` form        | `render_span(Span)`, gated by `opts.include_spans`                     |

Sigils for `LocalId` and `BlockId` wire inline at first
consumer (no dedicated `render_local_sigil` /
`render_block_sigil` methods).

### 3.4 Field / variant name resolution

Field names render by name through `place_ty` walk
(each projection step refines the type at that point
in the chain). The dispatch goes through `layout.kind`
per `spec.md` §11:

- Tuple field: `place_ty` resolves to `Ty::Tuple { fields, layout }`;
  `layout.kind` is `LayoutKind::Struct { fields }`
  (per `spec.md` §11 the tuple-as-struct layout
  encoding); `name = layout.kind.as_struct().fields[idx].name`.
  Per the §11 tuple convention, tuple synthetic names
  resolve to `_0`, `_1`, ..., `_{n-1}` in MIR-index
  order.
- Struct field: `place_ty` resolves to `Ty::Struct { layout, .. }`;
  `layout.kind` is `LayoutKind::Struct { fields }`;
  `name = layout.kind.as_struct().fields[idx].name`.
- Union field: `place_ty` resolves to `Ty::Union { layout, .. }`;
  `layout.kind` is `LayoutKind::Union { fields }`;
  `name = layout.kind.as_union().fields[idx].name`.
- Enum variant field: `place_ty` resolves to `Ty::Enum { layout, .. }`;
  `layout.kind` is `LayoutKind::Enum { shape }`;
  `shape` is one of:
    - `EnumShape::Empty`: zero variants; cannot reach
      a Field projection on `EnumShape::Empty` -- if
      the IR contains one, validation rejects.
    - `EnumShape::Single { variant, .. }`: one variant;
      `name = variant.fields[idx].name`.
    - `EnumShape::Multiple { variants, .. }`: active
      variant from preceding `Downcast` projection's
      `VariantIdx`; `name = variants[v_idx].fields[idx].name`.

Variant names render via `EnumShape.variant`
(`Single`) or `EnumShape.variants[idx]` (`Multiple`)
analogously. Both `FieldLayout.name: StringId` and
`VariantLayout.name: StringId` are non-optional in the
schema (`spec.md` §11), so missing-name is invalid IR
and surfaces through validation, not through a
rendering-side fallback.

## 4. Span-rendering and debuginfo modes

### 4.1 Default mode

Default-off. `PrettyPrinter::new(model)` constructs
with `PrettyPrintOptions::default()` (all-off);
`PrettyPrinter::with_options(model, opts)` opts in.

Two independent flags:

```rust
pub struct PrettyPrintOptions {
    pub include_spans:       bool,
    pub include_debug_names: bool,
}
```

The flags are independent: a consumer can opt into
spans only, debug names only, both, or neither.

### 4.2 Span rendering (when include_spans=true)

Trailing inline comment aligned to column 40:

```text
    <stmt>;                              // <render_span(span)>
    <terminator> -> [<labels>];          // <render_span(span)>
```

`ALIGN = 40` matches rustc's `mir/pretty.rs:21`. If
the statement / terminator content exceeds 40 columns,
the comment continues on the same line one space after
the content.

Per-construct sources of spans:
- `Statement.span` (per stage-8 D-8-3 wrapper struct).
- `Terminator.span` (per stage-9 D-9-8 wrapper struct).
- `LocalDecl.span` (per stage-11 D-11-2; rendered as
  trailing comment on the locals declaration line).
- `Item.span` (rendered as trailing comment on the
  function header line, composed by `render_instance`
  at stage 14).

### 4.3 Synthetic-span suppression

Spans on synthesised constructs (where
`span.file == SourceFileId::SYNTHESIZED`) suppress
unconditionally when `include_spans=true`. The
literal `<synthesized>:1:1: 1:1` carries no positional
information and would proliferate noise in fixtures
involving bridge synthesis (vtable shims, drop glue,
ConstMemoryStatic).

The decision is implemented as a guard in the
trailing-comment emission: render the comment only if
`span.file != SourceFileId::SYNTHESIZED`.

### 4.4 Debug-name rendering (when include_debug_names=true)

Standalone `debug NAME => _N;` lines after the locals
declarations block:

```text
    let mut _0: bool;
    let _1: i32;
    let mut _2: PL011;

    debug initialised => _0;
    debug count => _1;

    bb0: {
        ...
    }
```

Only locals with `LocalDecl.debug_name = Some(name)`
contribute. The `name` is rendered via
`model.strings.resolve(name)`. Locals without
`debug_name` (i.e., `None`) are silent.

Multiple `debug` lines render in `LocalId` order (the
same order locals are declared above).

## 5. Item-level rendering shape

`render_item(ItemId) -> String` produces the Item-level
wrapper: an attribute block plus a kind-keyword
plus the rendered DefPath. Per-Instance content
(`FnSig`, body, layout, init, value) is the composer's
domain (stage 14 `render_instance`).

### 5.1 Attribute-decorator syntax

All attribute prefixes render in Rust-source `#[...]`
form. Three idiomatic shapes:

- Atomic: `#[no_mangle]`, `#[opaque_handle]`.
- Key-value: `#[link_section = "..."]`,
  `#[linkage = "weak"]`, `#[lang = "phantom_data"]`.
- Argumented: `#[inline]`, `#[inline(always)]`,
  `#[inline(never)]`.

### 5.2 Default-suppression

All default-valued / false-valued fields suppress.
A Function Item with no explicit attributes (default
linkage, no inline hint, default link section, not
unsafe, no lang-item flags, not eligible for opaque-
handle) renders as bare `fn $path` with zero attribute
prefixes.

Suppression is the simple "render only if non-default"
rule applied per-field; no parallel `should_render`
flag.

### 5.3 Per-flag rendering table

| Field                            | Rendered form                       |
|----------------------------------|-------------------------------------|
| `linkage = NoMangle`             | `#[no_mangle]`                      |
| `linkage = Foreign`              | `#[foreign]`                        |
| `linkage = Weak`                 | `#[linkage = "weak"]`               |
| `linkage = Default`              | suppressed                          |
| `inline_hint = Hint`             | `#[inline]`                         |
| `inline_hint = Always`           | `#[inline(always)]`                 |
| `inline_hint = Never`            | `#[inline(never)]`                  |
| `inline_hint = None`             | suppressed                          |
| `is_unsafe = true`               | `unsafe` keyword on kind-keyword    |
| `is_unsafe = false`              | suppressed                          |
| `is_phantom_data = true`         | `#[lang = "phantom_data"]`          |
| `is_unsafe_cell = true`          | `#[lang = "unsafe_cell"]`           |
| `is_manually_drop = true`        | `#[lang = "manually_drop"]`         |
| any lang-flag = false            | suppressed                          |
| `link_section = Some("...")`     | `#[link_section = "..."]`           |
| `link_section = None`            | suppressed                          |
| `opaque_handle_eligible = true`  | `#[opaque_handle]`                  |
| `opaque_handle_eligible = false` | suppressed                          |

`is_unsafe = true` is special: it renders as a
**keyword prefix** on the kind-keyword (`unsafe fn`),
not as an attribute. Only `fn` accepts the keyword;
`unsafe static` is non-syntactic in Rust source.

### 5.4 Per-ItemKind variant rendering

`render_item` produces:

| `ItemKind` variant                       | Rendered form                                     |
|------------------------------------------|---------------------------------------------------|
| `Function`                               | `[#[attrs]\n][unsafe ]fn $path`                  |
| `Static { mutability: Mutability::Not }` | `[#[attrs]\n]static $path`                       |
| `Static { mutability: Mutability::Mut }` | `[#[attrs]\n]static mut $path`                   |
| `Const`                                  | `[#[attrs]\n]const $path`                        |
| `Struct`                                 | `[#[attrs]\n]struct $path`                       |
| `Enum`                                   | `[#[attrs]\n]enum $path`                         |
| `Union`                                  | `[#[attrs]\n]union $path`                        |

`$path` resolves via `render_def_path(ItemId)` (cached
at `Item.def_path.rendered`).

### 5.5 Layout

Multi-line when one or more attributes survive
suppression: each attribute on its own line, then the
kind-keyword + path on the final line. One-line bare
`kind $path` when zero attributes survive.

Examples:

```text
fn pl011::write_byte
```

```text
#[no_mangle]
#[inline(always)]
#[link_section = ".text.boot"]
unsafe fn pl011::init
```

```text
#[lang = "phantom_data"]
struct core::marker::PhantomData
```

### 5.6 Shared attribute-block helper (`render_item_attribute_block`)

Per stage-14 D-14-5.7 LOCKED.
A single internal helper composes the per-flag
`#[...]` attribute block consumed by both
`render_item` (§5.4 standalone-Item view) and
`render_instance` (§10.2 composer; via §10.2.7
attribute-block composition rule, Stream 1):

```rust
impl PrettyPrinter<'_> {
    fn render_item_attribute_block(&self, id: ItemId) -> String;
}
```

The helper walks the §5.3 per-flag table in
**table-row order** (pinned at stage-13 D-13-4.3
LOCKED), applies §5.2 default-suppression per
row, and produces a multi-line block where each
surviving `#[...]` is on its own line. The block
ends with a trailing `\n` when non-empty; empty
string when zero attributes survive (the calling
renderer then continues at column 0 with the
kind-keyword line).

The helper is an internal method on
`PrettyPrinter<'_>` (not part of the public
renderer family per §1.3). Its existence is
pinned in spec so:
- `render_item` and `render_instance` produce
  byte-identical Stream 1 (Item-flag) attribute
  streams for any given
  Item (no parallel composition logic, no drift
  risk).
- `pretty-printer.md` §10.2.7 (D-14-5.6 attribute
  block composition) can reference Stream 1 as
  "the §5.6 shared helper" instead of duplicating
  the §5.3 walk in `render_instance`'s spec.

**Sources consumed:**

- §5.3 per-flag table -- the row-ordered list of
  Item fields the helper inspects.
- §5.2 default-suppression rule -- per-row
  emission gate.
- §5.5 block layout -- one `#[...]` per line;
  trailing `\n` when non-empty.

**Sources NOT consumed:**

- `domain_attrs.item_attrs[item]` -- the Stream 2
  source per D-14-5.6 § /10.2.7 lives in
  `render_instance`'s composer surface, not in
  the §5.6 helper. `render_item`'s standalone-
  Item view does **not** emit domain-attribute
  rows (per §5.4 LOCKED: `render_item` produces
  just the per-`ItemKind` wrapper); the
  composer-from-scratch model in D-14-5.7 keeps
  domain attrs at the Instance-level renderer
  only.
- `fn_sig.abi` -- ABI keyword decoration is
  Instance-level per D-14-5.2 aspect 2; not
  in the helper.

**§5.3 row-order recap (the helper's walk
sequence; per stage-13 D-13-4.3 LOCKED, as revised
by stage-14 D-14-7 which dropped the `is_naked`
row alongside the `is_naked` field deletion at
`spec.md` §8.0):**

1. `Item.linkage` -> per the §5.3 rows for
   `linkage = NoMangle` / `Foreign` / `Weak`
   (`Default` suppressed).
2. `Item.inline_hint` -> per the §5.3 rows for
   `inline_hint = Hint` / `Always` / `Never`
   (`None` suppressed).
3. Lang-item flags (`is_phantom_data` /
   `is_unsafe_cell` / `is_manually_drop`) ->
   per-row `#[lang = "..."]`.
4. `Item.link_section.is_some()` ->
   `#[link_section = "..."]`.
5. `Item.opaque_handle_eligible == true` ->
   `#[opaque_handle]`.

The `is_unsafe == true` flag is **not** surfaced
by the helper (per §5.3 LOCKED): `unsafe` renders
as a keyword prefix on `fn`, not as an
attribute. Both `render_item` (for the
`ItemKind::Function` variant per §5.4) and
`render_instance` (for `InstanceKind::Function`
per §10.2.1) emit the `unsafe` keyword inline at
the kind-keyword position, not via the helper.

**Helper-call convention** (illustrative):

```text
render_item(id):
    attr_block = render_item_attribute_block(id)
    kind_keyword = match item.kind { ... }   // unsafe-prefix for Function
    path = render_def_path(id)
    -> format!("{attr_block}{kind_keyword} {path}")

render_instance(id) [Function variant per §10.2.1]:
    attr_block = render_item_attribute_block(inst.item)   // Stream 1
    domain_block = render_domain_attribute_block(inst.item) // Stream 2 per §10.2.7
    -> format!("{attr_block}{domain_block}[unsafe ][extern \"ABI\" ]fn {path}({args}) -> {ret} {body_or_decl}")
```

(The `render_domain_attribute_block` helper for
Stream 2 is described at §10.2.7; both helpers
are internal to the `PrettyPrinter` impl.)

## 6. Type rendering -- `render_ty` per-arm specs

`render_ty(TyId) -> String` dispatches on the closed
19-variant `Ty` enum (`spec.md` §10). All arms inherit
the master Rust-source-style.

### 6.1 Primitives

| Variant      | Rendered form |
|--------------|---------------|
| `Bool`       | `bool`        |
| `Char`       | `char`        |
| `Int(I8)`    | `i8`          |
| `Int(I16)`   | `i16`         |
| `Int(I32)`   | `i32`         |
| `Int(I64)`   | `i64`         |
| `Int(I128)`  | `i128`        |
| `Int(Isize)` | `isize`       |
| `Uint(U8)`   | `u8`          |
| `Uint(U16)`  | `u16`         |
| `Uint(U32)`  | `u32`         |
| `Uint(U64)`  | `u64`         |
| `Uint(U128)` | `u128`        |
| `Uint(Usize)`| `usize`       |
| `Float(F16)` | `f16`         |
| `Float(F32)` | `f32`         |
| `Float(F64)` | `f64`         |
| `Float(F128)`| `f128`        |
| `Str`        | `str`         |
| `Never`      | `!`           |

### 6.2 Compound non-recursive

| Variant                              | Rendered form                            |
|--------------------------------------|------------------------------------------|
| `Array(T, n)`                        | `[<render_ty(T)>; <n>]` (`n` decimal)   |
| `Slice(T)`                           | `[<render_ty(T)>]`                       |
| `Ref(T, Provenance::SharedBorrow)`   | `&<render_ty(T)>`                        |
| `Ref(T, Provenance::UniqueBorrow)`   | `&mut <render_ty(T)>`                    |
| `Ptr(T, Mutability::Not)`            | `*const <render_ty(T)>`                  |
| `Ptr(T, Mutability::Mut)`            | `*mut <render_ty(T)>`                    |

RegseqModel does not carry rustc-style lifetime
regions (per `spec.md` §10.1: `Provenance` is borrow
flavor, not pointer provenance / lifetime). So `Ref`
arms render without any `'a` token; the borrow flavor
is conveyed via the `mut` keyword for UniqueBorrow.

### 6.3 Tuple

| Variant                                | Rendered form                                 |
|----------------------------------------|-----------------------------------------------|
| `Tuple { fields: [] }`                 | `()`                                          |
| `Tuple { fields: [T] }`                | `(<render_ty(T)>,)`                           |
| `Tuple { fields: [T1, T2, ...] }`      | `(<render_ty(T1)>, <render_ty(T2)>, ...)`    |

The arity-1 explicit comma matches Rust source.

### 6.4 ADT (`Struct` / `Enum` / `Union`) and `FnDef` / `Closure`

| Variant                             | Rendered form                                                                          |
|-------------------------------------|----------------------------------------------------------------------------------------|
| `Struct { item, args, .. }`         | `<render_def_path(item)><render_generic_args(args, item)>`                             |
| `Enum   { item, args, .. }`         | `<render_def_path(item)><render_generic_args(args, item)>`                             |
| `Union  { item, args, .. }`         | `<render_def_path(item)><render_generic_args(args, item)>`                             |
| `Closure(item, args)`               | `<render_def_path(item)><render_generic_args(args, item)>`                             |
| `FnDef(item, args)`                 | `fn(<arg_tys>) -> <ret_ty> {<render_def_path(item)><render_generic_args(args, item)>}` |

`FnDef` rendering looks up the matching Instance by
locating an `Instance` with `instance.item == item` and
`instance.generic_args == args`, then resolves
`instance.kind` (which is `InstanceKind::Function {
fn_sig, .. }`) to render arg / return types via
`render_ty`. The curly-brace form is rustc-faithful
(FnDef is not expressible as a type expression in
stable Rust).

### 6.5 Function pointer

| Variant         | Rendered form                                                   |
|-----------------|-----------------------------------------------------------------|
| `FnPtr(fnsig)`  | `[unsafe ][extern "ABI" ]fn(<arg_tys>) -> <ret_ty>`             |

The `unsafe` keyword renders when `FnSig.safety =
Safety::Unsafe`. The `extern "ABI"` prefix renders
when `FnSig.abi != FnAbi::Rust` (default `Rust` is
suppressed; `C` renders as `extern "C"`).

### 6.6 Dynamic

`Dynamic { auto_traits: Vec<AutoTrait>, vtable_ty: TyId }`
renders as:

```text
dyn <render_def_path(vtable_item)> + <Auto1> + <Auto2>
```

where:
- `vtable_item` is the synthesised vtable struct's
  Item, recovered via `vtable_ty` -> `Ty::Struct { item, .. }`
  -> `item`. The DefPath encodes the principal trait
  + impl per `spec.md` §13.4
  (`<regseq_synth>::vtable::<Trait>::for::<ImplTy>`
  prefix form).
- Auto-traits render in `spec.md` §10.1 sorted order
  (Send < Sync < Unpin), separated by ` + `.
- Empty `auto_traits: []` omits the `+ ...` suffix
  entirely (renders just `dyn <vtable-DefPath>`).

### 6.7 `render_generic_args` helper

```rust
fn render_generic_args(&self, args: &GenericArgs, item: ItemId) -> String;
```

**Signature** per stage-14 D-14-4.9 (MINOR resolution
from R1): the helper takes the enclosing `ItemId`
alongside the args. The `item` parameter is required
to resolve the declared type of each
`GenericArg::Const(c)` arg from the enclosing Item's
generic-parameter list (per D-14-4.1's pair-form for
`render_const`). All callers of `render_generic_args`
(per §6.4 `Ty::Struct` / `Ty::Enum` / `Ty::Union` /
`Ty::Closure` / `Ty::FnDef` rendering arms; §10.1.3 -
§10.1.7 composer-renderer arms that emit
`<render_def_path(item)><render_generic_args(args,
item)>`) already have the `item` adjacent to the
`args` in the source structure (e.g.,
`Ty::Struct { item, args, .. }`).

Form: `<T1, T2, K1>` non-empty; empty string when
`args.is_empty()`. `GenericArg` is the closed 2-variant
enum `{ Type(TyId), Const(ConstId) }` per `spec.md`
§9.1 (lifetimes are erased post-monomorphisation and
not represented). Per-arg dispatch:

- `GenericArg::Type(ty)` -> `render_ty(ty)`.
- `GenericArg::Const(c)` -> `render_const(c, param_ty)`
  where `param_ty` is the declared type of this const
  generic parameter, looked up from
  `model.items[item]`'s generic-parameter list at the
  matching parameter position. The mapping
  arg-position -> parameter-position is positional
  (rustc-mirrored convention).

Multiple args separate with `, `. Composes into
`<...>` wrapping by the caller (FnDef, Closure,
Struct, Enum, Union arms).

## 7. Body-level rendering

Per-arm specifications for the 9 public renderers + 1
internal helper. All inherit master Rust-source-style
+ §3 envelope + §4 span / debuginfo gating.

### 7.1 `render_place(&Place, &Body) -> String`

```rust
struct Place { local: LocalId, projection: Box<[PlaceElem]> }
```

Composes head local sigil + projection chain via
pre/post-wrap pattern. Each `PlaceElem` wraps the
prior result.

| `PlaceElem` variant                                         | Wrap form                                              |
|-------------------------------------------------------------|--------------------------------------------------------|
| `Deref`                                                     | `(*X)`                                                 |
| `Field(idx, ty)`                                            | `(X.<field_name>: <render_ty(ty)>)`                   |
| `Index(local)`                                              | `X[_<local.index()>]`                                  |
| `ConstantIndex { offset, min_length, from_end: false }`     | `X[<offset> of <min_length>]`                          |
| `ConstantIndex { offset, min_length, from_end: true }`      | `X[-<offset> of <min_length>]`                         |
| `Subslice { from, to, from_end: false }`                    | `X[<from>..<to>]`                                      |
| `Subslice { from, to, from_end: true }`                     | `X[<from>..-<to>]`                                     |
| `Downcast(variant_idx)`                                     | `(X as <variant_name>)`                                |

Field / variant names resolve via `place_ty` walk
(§3.4). Both name fields are non-optional in the
layout schema (`spec.md` §11); missing-name is invalid
IR and rejected by validation, not papered over at
render time.

Head local renders as `_<local.index()>`. With zero
projections, output is just the head sigil. With one
projection, the wrap applies once. With chains, each
projection wraps the prior result preserving operator
precedence: `Deref` followed by `Field` produces
`((*X).<name>: <ty>)` (outer parens preserve
deref-then-field semantics; without them
`(*X.name: ty)` would parse as "dereference X.name").

### 7.2 `render_operand(&Operand, &Body, expected_ty: TyId) -> String`

```rust
enum Operand { Copy(Place), Move(Place), Constant(ConstId) }
```

**Signature** per stage-14 D-14-4.9 REOPEN of
stage-13 D-13-6: the renderer takes an additional
`expected_ty: TyId` parameter -- the consumer Ty at
the operand's use site. The parameter is uniformly
required by the signature for API symmetry with
`render_const(ConstId, TyId)` per D-14-4.1's
pair-form. The `Copy` / `Move` arms ignore the
parameter (their place type is resolvable via
`place_ty(place, body)`); the `Constant` arm
consumes it via `render_const(id, expected_ty)`.
Callers resolve `expected_ty` from their structural
context (per §10.1.8's caller table).

| Variant         | Rendered form                                                |
|-----------------|--------------------------------------------------------------|
| `Copy(p)`       | `copy <render_place(p, body)>` (expected_ty ignored)         |
| `Move(p)`       | `move <render_place(p, body)>` (expected_ty ignored)         |
| `Constant(id)`  | `<render_const(id, expected_ty)>` (stage-14 composer)        |

`Constant` is rendered bare; the const value's own
rendering encodes its const-ness (`42_i32`, `true`,
`&"..."`, etc.). The pair-form `render_const(id,
expected_ty)` per D-14-4.1 + D-14-4.9 ensures
type-context-dependent Const arms (Bytes / Aggregate /
ZST / NullPtr / Reloc) render correctly at operand
use sites.

### 7.3 `render_rvalue(&Rvalue, &Body, expected_ty: TyId) -> String`

The closed 9-variant `Rvalue` enum (`spec.md` §21).

**Signature** per stage-14 D-14-4.9 REOPEN of stage-13
D-13-6: `render_rvalue` takes an `expected_ty: TyId`
parameter -- the Rvalue's result type at its use site.
The caller (`render_statement` Assign arm) resolves
`expected_ty` from the LHS Place's type via
`place_ty(place, body)` and threads it in. Each per-arm
dispatch below derives per-operand expected Tys from
`expected_ty` plus the variant's structural payload,
then calls `render_operand(operand, body, operand_expected_ty)`
with the derived type.

| Variant                                       | Rendered form                                                                                                  | Per-operand `expected_ty` derivation                                                                  |
|-----------------------------------------------|----------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------|
| `Use(operand)`                                | `<render_operand(operand, body, expected_ty)>` (transparent)                                                  | Operand type = `expected_ty` (transparent Use semantics).                                              |
| `Repeat(operand, count)`                      | `[<render_operand(operand, body, elem_ty)>; <count>]`                                                          | `elem_ty` = the element type derived from `expected_ty = Ty::Array(elem_ty, count)`.                  |
| `Ref(Provenance::SharedBorrow, place)`        | `&<render_place(place, body)>`                                                                                 | No operand; place type via `place_ty`.                                                                |
| `Ref(Provenance::UniqueBorrow, place)`        | `&mut <render_place(place, body)>`                                                                             | No operand.                                                                                            |
| `RawPtr(Mutability::Not, place)`              | `&raw const <render_place(place, body)>`                                                                       | No operand.                                                                                            |
| `RawPtr(Mutability::Mut, place)`              | `&raw mut <render_place(place, body)>`                                                                         | No operand.                                                                                            |
| `Cast(kind, operand, ty)`                     | `<render_operand(operand, body, source_ty)> as <render_ty(ty)>` (CastKind shown only when ambiguous; see 7.3.1) | `source_ty` = the operand's source type per CastKind; for `IntToInt` / `FloatToInt` / etc. derived from the cast kind + target type. The renderer looks up `source_ty` by inspecting the operand's place type for `Operand::Copy/Move(place)` or by inspecting the const variant for `Operand::Constant`. |
| `BinaryOp(op, lhs, rhs)`                      | `<render_operand(lhs, body, op_input_ty)> <op_glyph> <render_operand(rhs, body, op_input_ty)>`                | `op_input_ty` = the BinOp's input type per `op`'s signature: `Add`/`Sub`/`Mul`/`Div`/`Rem`/`Shl`/`Shr` use `expected_ty` (result equals input for ordinary arithmetic); `Eq`/`Lt`/`Le`/`Ne`/`Ge`/`Gt` use a separate operand type derived from the source MIR (looked up via operand inspection -- both operands have the same type). |
| `UnaryOp(op, operand)`                        | `<op_glyph><render_operand(operand, body, expected_ty)>`                                                       | Operand type = `expected_ty` (UnaryOp preserves type).                                                |
| `Discriminant(place)`                         | `discriminant(<render_place(place, body)>)`                                                                    | No operand.                                                                                            |
| `Aggregate(kind, operands)`                   | per `AggregateKind` variant; see 7.3.2                                                                         | Per-operand expected Tys derived from `AggregateKind`'s structural payload; see 7.3.2.                |

#### 7.3.1 CastKind disambiguation

By default `Cast` renders bare `<op> as <ty>` without
the `CastKind` annotation. The kind annotation
`<op> as <ty> (CastKind)` renders only for variants
that admit ambiguity (because the source / target
types alone cannot tell the reader which coercion
fired):

- `PointerCoercion(ReifyFnPointer)` -> annotated as
  `(PointerCoercion(ReifyFnPointer))`. (Coerces a
  `Ty::FnDef` ZST into a `Ty::FnPtr` value.)
- `PointerCoercion(UnsafeFnPointer)` -> annotated.
  (Coerces a safe `fn` pointer into an `unsafe fn`
  pointer; both are `Ty::FnPtr` so the source / target
  types differ only in `FnSig.safety`.)
- `PointerCoercion(ClosureFnPointer)` -> annotated.
  (Coerces a non-capturing closure into a `Ty::FnPtr`.)
- `PointerCoercion(Unsize)` -> annotated. (Coerces a
  sized pointer into an unsized pointer / DST-bearing
  pointer.)
- `Subtype` -> annotated as `(Subtype)`. (Lifetime /
  variance subtype coercion injected by the
  `Subtyper` runtime-lowering pass; bare cast renders
  identical to a no-op since RegseqModel has no
  lifetime regions.)

Other variants render bare; the source + target types
disambiguate:

- `IntToInt`, `FloatToInt`, `IntToFloat`, `FloatToFloat`,
  `PtrToPtr`, `FnPtrToPtr`, `Transmute`,
  `PointerExposeProvenance`,
  `PointerWithExposedProvenance`.

Note: rustc's `PointerCoercion::MutToConstPointer` and
`PointerCoercion::ArrayToPointer` are dropped by the
required `CleanupPostBorrowck` MIR pass and cannot
reach `optimized_mir`; they are not renderable.

#### 7.3.2 AggregateKind per-arm rendering

The closed 7-variant `AggregateKind` enum (`spec.md`
§21.3.6). Each variant carries a `TyId` resolving to
the destination type via `spec.md` §10's stage-3 type
interner; per-arm rendering looks up the layout there.

For each `AggregateKind` variant below, `<opN>` is
shorthand for `<render_operand(operandN, body, opN_expected_ty)>`
where `opN_expected_ty` is derived per the
AggregateKind's structural payload (per-operand
derivation column).

| `AggregateKind` variant                | Resolved destination type                  | Rendered form                                                                                                  | Per-operand `expected_ty` derivation                                                              |
|----------------------------------------|--------------------------------------------|----------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------|
| `Array(elem_ty)`                       | `Ty::Array(elem_ty, n)`                    | `[<op0>, <op1>, ...]`                                                                                          | Every operand has `expected_ty = elem_ty` (the AggregateKind payload).                            |
| `Tuple(tuple_ty)` arity 0              | `Ty::Tuple { fields: [], .. }`             | `()`                                                                                                           | No operands.                                                                                       |
| `Tuple(tuple_ty)` arity 1              | `Ty::Tuple { fields: [_], .. }`            | `(<op0>,)`                                                                                                     | `op0` expected_ty = `fields[0]` from the resolved `Ty::Tuple` layout.                             |
| `Tuple(tuple_ty)` arity 2+             | `Ty::Tuple { fields: [..], .. }`           | `(<op0>, <op1>, ...)`                                                                                          | i-th operand expected_ty = `fields[i]` from the resolved `Ty::Tuple` layout.                       |
| `Struct(struct_ty)` tuple-variant      | `Ty::Struct { item, args, layout }`        | `<render_def_path(item)><render_generic_args(args, item)>(<op0>, <op1>, ...)`                                  | i-th operand expected_ty = `layout.kind.as_struct().fields[i].ty`.                                |
| `Struct(struct_ty)` struct-variant     | `Ty::Struct { item, args, layout }`        | `<render_def_path(item)><render_generic_args(args, item)> { <field0_name>: <op0>, <field1_name>: <op1>, ... }` | i-th operand expected_ty = `layout.kind.as_struct().fields[i].ty`.                                |
| `Enum(enum_ty, variant_idx)` C-style   | `Ty::Enum { item, args, layout }`          | `<render_def_path(item)><render_generic_args(args, item)>::<variant_name>`                                     | No operands (C-style variant has empty fields).                                                    |
| `Enum(enum_ty, variant_idx)` tuple-variant | `Ty::Enum { item, args, layout }`      | `<render_def_path(item)><render_generic_args(args, item)>::<variant_name>(<op0>, <op1>, ...)`                  | i-th operand expected_ty = the active variant's `fields[i].ty` (via §3.4 EnumShape lookup).        |
| `Enum(enum_ty, variant_idx)` struct-variant | `Ty::Enum { item, args, layout }`     | `<render_def_path(item)><render_generic_args(args, item)>::<variant_name> { <field0_name>: <op0>, ... }`       | i-th operand expected_ty = the active variant's `fields[i].ty`.                                    |
| `Union(union_ty, field_idx)`           | `Ty::Union { item, args, layout }`         | `<render_def_path(item)><render_generic_args(args, item)> { <field_name>: <op0> }`                             | `op0` expected_ty = `layout.kind.as_union().fields[field_idx].ty`.                                |
| `Closure(closure_ty)`                  | `Ty::Closure(item, args)`                  | `<render_ty(closure_ty)> { <upvar0_name>: <op0>, ... }` (closure-captures struct's upvar names via parallel `find_nominal(NominalKind::Struct, item, args)` per `spec.md` §13.1) | i-th operand expected_ty = the parallel closure-captures struct's `fields[i].ty`.                |
| `RawPtr(pointee_ty, Mutability::Not)`  | constructed type is `*const pointee_ty`    | `(<op_addr>, <op_meta>) as *const <render_ty(pointee_ty)>`                                                     | `op_addr` expected_ty = `Ty::Ptr(pointee_ty, Mutability::Not)`; `op_meta` expected_ty depends on pointee (unit `()` for thin, `Ty::Uint(Usize)` for fat-pointer to `Ty::Slice`). |
| `RawPtr(pointee_ty, Mutability::Mut)`  | constructed type is `*mut pointee_ty`      | `(<op_addr>, <op_meta>) as *mut <render_ty(pointee_ty)>`                                                       | `op_addr` expected_ty = `Ty::Ptr(pointee_ty, Mutability::Mut)`; `op_meta` expected_ty as above.    |

The `Struct(struct_ty)` resolves `struct_ty` to a
`Ty::Struct { item, args, layout }` via `spec.md`
§10; field names come from `layout.kind.as_struct().fields[idx].name`.
The variant style (tuple vs struct) is determined by
the field-name pattern: a tuple-variant struct stores
numeric synthetic field names per the rustc convention
for `struct Foo(u32, u32)`-shape Items; a struct-variant
struct stores source-level field names. Stage 13 uses
the field-name shape to dispatch the rendering form
(positional `(<op0>, <op1>)` for tuple-variant;
named `{ <name>: <op0>, ... }` for struct-variant).

The `Enum(enum_ty, variant_idx)` resolves `enum_ty` to
a `Ty::Enum { item, args, layout }`; `layout.kind` is
`LayoutKind::Enum { shape }`. The active variant is
read from `shape` per the §3.4 dispatch: `EnumShape::Single`'s
`variant`; `EnumShape::Multiple`'s `variants[variant_idx]`.
The C-style / tuple / struct distinction is determined
by the active variant's `VariantLayout.fields` shape
(zero fields -> C-style; tuple-style synthetic names
-> tuple-variant; source-level names -> struct-variant).

The `Union(union_ty, field_idx)` resolves `union_ty`
to a `Ty::Union { item, args, layout }`; the active
field name comes from `layout.kind.as_union().fields[field_idx].name`.
Unions construct exactly one active field (one operand
position; rendered as `{ field_name: op0 }`).

The `RawPtr(pointee_ty, mutability)` aggregate
constructs a fat or thin raw pointer from operand 0
(thin data pointer) and operand 1 (metadata; unit
value `()` for thin pointers). Both operand positions
are required by the schema (`spec.md` §21.3.6;
mirroring rustc's `RawPtr(Ty<'tcx>, Mutability)`
shape per `rustc_middle/src/mir/syntax.rs:1561-1566`).
The two-operand `(<op_addr>, <op_meta>) as *const/mut <pointee_ty>`
form preserves the schema shape uniformly; thin
pointers render with `<op_meta>` resolving to a unit
constant (`()`).

### 7.4 `render_statement(&Statement, &Body) -> String`

The closed 3-variant `StatementKind` enum (`spec.md`
§22).

| Variant                                                      | Rendered form                                                                                                                       | Per-operand / per-rvalue `expected_ty` derivation                                                                                                                                                |
|--------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `Assign(place, rvalue)`                                      | `<render_place(place, body)> = <render_rvalue(rvalue, body, place_ty)>;`                                                            | `place_ty` = the LHS Place's resolved type via `place_ty(place, body)`. `render_rvalue` threads `place_ty` as `expected_ty` per §7.3.                                                            |
| `SetDiscriminant(place, variant_idx)`                        | `discriminant(<render_place(place, body)>) = <variant_name>;`                                                                       | No operand.                                                                                                                                                                                       |
| `Intrinsic(NonDivergingIntrinsic::Assume(operand))`          | `assume(<render_operand(operand, body, <Ty::Bool>)>);`                                                                              | `operand` expected_ty = `Ty::Bool` (assume operand is a boolean condition).                                                                                                                       |
| `Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(CopyNonOverlapping { src, dst, count }))` | `copy_nonoverlapping(<render_operand(src, body, <Ty::Ptr(unit, Not)>)>, <render_operand(dst, body, <Ty::Ptr(unit, Mut)>)>, <render_operand(count, body, <Ty::Uint(Usize)>)>);` | `src` expected_ty = `Ty::Ptr(Ty::Tuple([]), Mutability::Not)` (raw const pointer; pointee is unit per rustc's intrinsic signature); `dst` expected_ty = `Ty::Ptr(Ty::Tuple([]), Mutability::Mut)`; `count` expected_ty = `Ty::Uint(UintKind::Usize)`. |

Trailing `;` is always present.

### 7.5 `render_terminator(&Terminator, &Body) -> String`

```text
<render_terminator_kind(terminator.kind, body)> -> [<successor_labels>]
```

The terminator's head text comes from `render_terminator_kind`;
the successor labels are computed per-variant. The
output ends with `]` (no trailing `;`); the trailing
`;` after a terminator is appended by `render_basic_block`
at composition time (§7.10), not by `render_terminator`
itself.

Sub-renderers compose as detailed below.

### 7.6 `render_terminator_kind(&TerminatorKind, &Body) -> String` (internal)

The closed 6-variant `TerminatorKind` enum (`spec.md`
§23).

| Variant                                                  | Head text                                                                                                                  | Per-operand / per-callee `expected_ty` derivation                                                                                                                |
|----------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `Goto(BlockId)`                                          | `goto`                                                                                                                     | No operand.                                                                                                                                                    |
| `SwitchInt { discr, targets }`                           | `switchInt(<render_operand(discr, body, discr_ty)>)`                                                                       | `discr_ty` = the discriminant operand's type, looked up via `place_ty` for `Operand::Copy/Move(place)` (the discriminant is read from a place per `Rvalue::Discriminant` lowering) or per the const variant tag for `Operand::Constant`. |
| `Return`                                                 | `return`                                                                                                                   | No operand.                                                                                                                                                    |
| `Unreachable`                                            | `unreachable`                                                                                                              | No operand.                                                                                                                                                    |
| `Call { callee, args, destination, .. }`                 | `<render_place(destination, body)> = <render_call_callee(callee, body)>(<arg0>, <arg1>, ...)`                              | i-th `arg` rendered via `render_operand(arg, body, callee_fn_sig.inputs[i])`. For `Direct(id)`, `callee_fn_sig = model.instances[id].kind.as_function().fn_sig`'s resolved `FnSig.inputs`. For `Indirect(_)`, callee's type is `Ty::FnPtr(sig_id)`; signature lookup via the FnSigInterner. |
| `Assert { cond, expected, msg, .. }`                     | `assert(<expected>, <render_operand(cond, body, <Ty::Bool>)>, <render_assert_kind(msg, body)>)`                            | `cond` expected_ty = `Ty::Bool`.                                                                                                                                |

`<expected>` renders as `true` or `false` literal
matching the `Assert.expected: bool` field.

### 7.7 `render_call_callee(&CallCallee, &Body) -> String`

```rust
enum CallCallee { Direct(InstanceId), Indirect(Operand) }
```

| Variant            | Rendered form                                                                                                  | Per-operand `expected_ty` derivation                                                                              |
|--------------------|----------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|
| `Direct(id)`       | `<render_def_path(instance.item)><render_generic_args(instance.generic_args, instance.item)>`                  | No operand.                                                                                                       |
| `Indirect(opand)`  | `<render_operand(opand, body, <Ty::FnPtr(sig_id)>)>`                                                           | `opand` expected_ty = the function-pointer type of the callee at this call site. The Call's callee Ty is determined by the Call's parent context (resolved per the Terminator's structural information). |

For `Direct(id)`, the printer looks up
`instance = model.instances[id]`, then renders the
DefPath of `instance.item` plus
`instance.generic_args` (Instance-keyed; carried at
the Instance struct level per `spec.md` §9.2, not on
`InstanceKind`). Consistent with `Ty::FnDef` rendering
(§6.4).

### 7.8 `render_switch_targets(&SwitchTargets, &Body) -> String`

```rust
struct SwitchTargets {
    cases:     Box<[(u128, BlockId)]>,
    otherwise: BlockId,
}
```

Renders as the successor-labels content for
`SwitchInt`:

```text
<value0>: bb<N0>, <value1>: bb<N1>, ..., otherwise: bb<N_otherwise>
```

Values render as decimal integers (per `u128`).

### 7.9 `render_assert_kind(&AssertKind, &Body) -> String`

The 8-variant subset of `AssertKind` (`spec.md` §23.3.3).
Each renders as a fixed-template message string,
direct adoption of rustc_public's `pretty_assert_message`
patterns. `&Body` is required because the operand
placeholders (`{l}`, `{r}`, `{operand}`, `{len}`,
`{index}`, `{required}`, `{found}`) render via
`render_operand`, which itself takes `&Body` for
projection-chain type resolution.

| Variant                                            | Template                                                                             |
|----------------------------------------------------|--------------------------------------------------------------------------------------|
| `BoundsCheck { len, index }`                       | `index out of bounds: the length is {len} but the index is {index}`                  |
| `Overflow(BinOp::Add, l, r)`                       | `attempt to compute `{l} + {r}`, which would overflow`                              |
| `Overflow(BinOp::Sub, l, r)`                       | `attempt to compute `{l} - {r}`, which would overflow`                              |
| `Overflow(BinOp::Mul, l, r)`                       | `attempt to compute `{l} * {r}`, which would overflow`                              |
| `Overflow(BinOp::Div, l, r)`                       | `attempt to compute `{l} / {r}`, which would overflow`                              |
| `Overflow(BinOp::Rem, l, r)`                       | `attempt to compute the remainder of `{l} % {r}`, which would overflow`             |
| `Overflow(BinOp::Shl, l, r)`                       | `attempt to shift left by `{r}`, which would overflow`                              |
| `Overflow(BinOp::Shr, l, r)`                       | `attempt to shift right by `{r}`, which would overflow`                             |
| `OverflowNeg(operand)`                             | `attempt to negate `{operand}`, which would overflow`                                |
| `DivisionByZero(operand)`                          | `attempt to divide `{operand}` by zero`                                              |
| `RemainderByZero(operand)`                         | `attempt to calculate the remainder of `{operand}` with a divisor of zero`           |
| `MisalignedPointerDereference { required, found }` | `misaligned pointer dereference: address must be a multiple of {required} but is {found}` |
| `NullPointerDereference`                           | `null pointer dereference occurred`                                                  |
| `InvalidEnumConstruction(found)`                   | `invalid enum discriminant value: {found}`                                           |

`{l}`, `{r}`, `{operand}`, `{len}`, `{index}`,
`{required}`, `{found}` placeholders render via
`render_operand(_, body, expected_ty)` (for
Operand-typed fields) or as decimal / value literal
(for non-Operand fields per the variant schema).
Per-placeholder `expected_ty` derivation (per
D-14-4.9):

- `BoundsCheck { len, index }`: both `len` and
  `index` are `Ty::Uint(UintKind::Usize)`.
- `Overflow(BinOp::{Add, Sub, Mul, Div, Rem}, l, r)`:
  both `l` and `r` have the same `expected_ty` --
  the BinOp's input type (a numeric primitive).
  Looked up via operand inspection: both operands
  share the same source-MIR type per `Rvalue::BinaryOp`
  invariants.
- `Overflow(BinOp::{Shl, Shr}, l, r)`: `l` has the
  shifted value's type (a numeric primitive); `r`
  has a `Ty::Uint(_)` shift-amount type. Looked up
  similarly via operand inspection.
- `OverflowNeg(operand)` / `DivisionByZero(operand)`
  / `RemainderByZero(operand)`: operand's type is
  the numeric primitive being checked.
- `MisalignedPointerDereference { required, found }`:
  both `required` and `found` are
  `Ty::Uint(UintKind::Usize)`.
- `InvalidEnumConstruction(found)`: `found` is the
  discriminant scalar type per the source enum's tag.

### 7.10 `render_basic_block(&BasicBlock, &Body) -> String`

Envelope:

```text
bb<N>: {
    <stmt_0>;
    <stmt_1>;
    ...
    <terminator> -> [<successor_labels>];
}
```

Composition:
- Header line: `bb<N.index()>: {` at envelope indent
  (4 spaces).
- Per-statement lines: 8-space indent, each rendered
  via `render_statement` (which provides trailing `;`).
- Terminator line: 8-space indent, rendered via
  `render_terminator` (which provides head text +
  `-> [labels]`); `;` appended here.
- Closing line: `}` at envelope indent (4 spaces).

A basic block with zero statements (only the
terminator, which is always present per the
B-NonEmpty invariant in `spec.md` §24) renders
compactly:

```text
bb0: {
    return -> [];
}
```

## 8. Per-method ownership table

Stage-13-owned building blocks, stage-14-owned
composers, and stage-1-shipped helpers are summarized
below. The table consolidates what was previously
distributed across `spec.md` §18.2 + per-construct
deferred-renderer rows.

| Method                              | Owner             | Inputs                       | Returns | Description                                                |
|-------------------------------------|-------------------|------------------------------|---------|------------------------------------------------------------|
| `render(&self)`                     | stage 14          | -                            | String  | top-level model dump; iterates `model.instances.iter_enumerated()` in InstanceId order (canonical F4-2 sort per `spec.md` §16.3; rank-grouped via §9.3 `regseq_instance_kind_rank`); composes per-Instance via `render_instance(InstanceId)` (§10.2); single blank line between consecutive Instances; no file-level header / trailer; empty `model.instances` -> empty `String`. See §10.3 for the full per-arm spec. |
| `render_instance(InstanceId)`       | stage 14          | InstanceId                   | String  | composes `render_def_path` + `render_ty` + body-level renderers + `render_const` + the §5.6 shared `render_item_attribute_block` helper per §10.2 (per D-14-5.7 LOCKED, does NOT call `render_item`) |
| `render_item_attribute_block(ItemId)` | stage 14 (internal helper per D-14-5.7) | ItemId         | String  | walks §5.3 per-flag table in row order; emits `#[...]` block per §5.5 layout. Shared composition source consumed by both `render_item` (§5.4) and `render_instance` (§10.2.7 Stream 1) |
| `render_const(ConstId, TyId)`       | stage 14          | ConstId, TyId                | String  | per-variant Const value (pair-form mirrors rustc per-variant dispatch; composes `render_ty` for type-context arms) |
| `render_item(ItemId)`               | stage 13          | ItemId                       | String  | Item-level wrapper (attribute block + kind-keyword + path); uses the §5.6 shared `render_item_attribute_block` helper internally (per D-14-5.7) for byte-identical Stream 1 (Item-flag) attribute streams with `render_instance`. Note: `render_item` does NOT emit Stream 2 (domain-attribute) rows; those live at the Instance-level composer only |
| `render_def_path(ItemId)`           | stage 12          | ItemId                       | String  | one-line cache lookup on `Item.def_path.rendered`          |
| `render_def_path_value(&DefPath)`   | stage 1           | &DefPath                     | String  | value-level helper for stage-1-shipped uses                |
| `render_ty(TyId)`                   | stage 13          | TyId                         | String  | 19-variant `Ty` enum dispatch                              |
| `render_generic_args(&GenericArgs, ItemId)` | stage 13 (signature revised per stage-14 D-14-4.9) | &GenericArgs, ItemId | String  | `<T1, T2, K1>` form; enclosing `ItemId` required to resolve declared types of `GenericArg::Const` args per D-14-4.1 pair-form |
| `render_place(&Place, &Body)`       | stage 13          | &Place, &Body                | String  | head local + projection chain                              |
| `render_operand(&Operand, &Body, TyId)` | stage 13 (signature revised per stage-14 D-14-4.9) | &Operand, &Body, TyId | String | Copy / Move / Constant dispatch; pair-form `expected_ty` threads consumer Ty to `render_const` |
| `render_rvalue(&Rvalue, &Body, TyId)` | stage 13 (signature revised per stage-14 D-14-4.9) | &Rvalue, &Body, TyId | String  | 9-variant Rvalue dispatch; pair-form `expected_ty` (the Rvalue result type at use site) threads to per-operand expected Tys per §7.3 derivation table |
| `render_statement(&Statement, &Body)` | stage 13        | &Statement, &Body            | String  | 3-variant StatementKind dispatch                           |
| `render_terminator(&Terminator, &Body)` | stage 13      | &Terminator, &Body           | String  | head + successor labels                                    |
| `render_terminator_kind(&TerminatorKind, &Body)` | stage 13 (internal) | &TerminatorKind, &Body | String  | per-TerminatorKind head dispatch helper                    |
| `render_call_callee(&CallCallee, &Body)` | stage 13     | &CallCallee, &Body           | String  | Direct / Indirect dispatch                                 |
| `render_switch_targets(&SwitchTargets, &Body)` | stage 13 | &SwitchTargets, &Body        | String  | explicit-split storage to `value: bb, ..., otherwise: bb`  |
| `render_assert_kind(&AssertKind, &Body)` | stage 13     | &AssertKind, &Body           | String  | 8-variant AssertKind dispatch (template messages); &Body needed for operand rendering |
| `render_basic_block(&BasicBlock, &Body)` | stage 13     | &BasicBlock, &Body           | String  | `bbN: { stmts; terminator; }` envelope                     |
| `render_span(Span)`                 | stage 1 (shipped) | Span                         | String  | `<path>:<lo_l>:<lo_c>: <hi_l>:<hi_c>` form                 |

Stage-1-shipped helpers (`render_span`,
`render_def_path_value`) and stage-12 (`render_def_path`)
remain stable; stage-13 fills the remaining stage-13-
owned methods; stage-14 fills `render_const`,
`render_instance`, and top-level `render`.

## 9. Worked-example specimens

Each specimen pairs an input value (constructed
ctor or value description) with the expected rendered
output. Specimens pin the textual contract for snapshot
stability; insta-snapshot tests in
`crates/regseq-model/tests/pretty.rs` mirror specimens
1:1 at end-of-phase-1 implementation sweep.

`<elided>` notation marks portions whose rendering is
delegated to other renderers and whose specific
content is illustrative (e.g., a path produced by
`render_def_path` or a const value by `render_const`).

### 9.1 `render_ty` specimens

**Specimen 1: Bool.**

Input: `Ty::Bool`.
Output: `bool`.

**Specimen 2: Int (representative).**

Input: `Ty::Int(IntKind::I32)`.
Output: `i32`.

**Specimen 3: Uint (representative).**

Input: `Ty::Uint(UintKind::U32)`.
Output: `u32`.

**Specimen 4: Float (representative).**

Input: `Ty::Float(FloatKind::F64)`.
Output: `f64`.

**Specimen 5: Char.**

Input: `Ty::Char`.
Output: `char`.

**Specimen 6: Str.**

Input: `Ty::Str`.
Output: `str`.

**Specimen 7: Never.**

Input: `Ty::Never`.
Output: `!`.

**Specimen 8: Array.**

Input: `Ty::Array(<u8 TyId>, 32)`.
Output: `[u8; 32]`.

**Specimen 9: Slice.**

Input: `Ty::Slice(<u8 TyId>)`.
Output: `[u8]`.

**Specimen 10: Ref shared.**

Input: `Ty::Ref(<i32 TyId>, Provenance::SharedBorrow)`.
Output: `&i32`.

**Specimen 11: Ref unique.**

Input: `Ty::Ref(<i32 TyId>, Provenance::UniqueBorrow)`.
Output: `&mut i32`.

**Specimen 12: Ptr const.**

Input: `Ty::Ptr(<u8 TyId>, Mutability::Not)`.
Output: `*const u8`.

**Specimen 13: Ptr mut.**

Input: `Ty::Ptr(<u8 TyId>, Mutability::Mut)`.
Output: `*mut u8`.

**Specimen 14: Tuple unit.**

Input: `Ty::Tuple { fields: [], layout: UNIT_LAYOUT }`.
Output: `()`.

**Specimen 15: Tuple arity-1.**

Input: `Ty::Tuple { fields: [<i32 TyId>], layout: ... }`.
Output: `(i32,)`.

**Specimen 16: Tuple arity-2+.**

Input: `Ty::Tuple { fields: [<i32 TyId>, <bool TyId>], layout: ... }`.
Output: `(i32, bool)`.

**Specimen 17: Struct.**

Input: `Ty::Struct { item: <PL011 ItemId>, args: GenericArgs::empty(), layout: ... }`.
Output: `pl011::PL011`.

**Specimen 18: Enum with generic args.**

Input: `Ty::Enum { item: <Option ItemId>, args: GenericArgs([Type(<u32 TyId>)]), layout: ... }`.
Output: `core::option::Option<u32>`.

**Specimen 19: Union.**

Input: `Ty::Union { item: <MyUnion ItemId>, args: GenericArgs::empty(), layout: ... }`.
Output: `pl011::regs::MyUnion`.

**Specimen 20: FnDef.**

Input: `Ty::FnDef(<pl011::init ItemId>, GenericArgs::empty())` whose Instance has
`fn_sig: (TyId(*mut PL011)) -> TyId(bool)`.
Output: `fn(*mut pl011::PL011) -> bool {pl011::init}`.

**Specimen 21: FnPtr safe Rust.**

Input: `Ty::FnPtr(<FnSigId for fn(i32) -> u32>)` with `safety = Safe`, `abi = Rust`.
Output: `fn(i32) -> u32`.

**Specimen 22: FnPtr unsafe extern "C".**

Input: `Ty::FnPtr(<FnSigId for unsafe extern "C" fn(*mut u8) -> i32>)` with `safety = Unsafe`, `abi = C`.
Output: `unsafe extern "C" fn(*mut u8) -> i32`.

**Specimen 23: Dynamic trait + auto-traits.**

Input: `Ty::Dynamic { auto_traits: [Send, Sync], vtable_ty: <vtable TyId for Reader::for::<File>> }`.
Output: `dyn <regseq_synth>::vtable::Reader::for::<std::fs::File> + Send + Sync`.

**Specimen 24: Dynamic without auto-traits.**

Input: `Ty::Dynamic { auto_traits: [], vtable_ty: <vtable TyId for Foo::for::<Bar>> }`.
Output: `dyn <regseq_synth>::vtable::Foo::for::<Bar>`.

**Specimen 25: Closure.**

Input: `Ty::Closure(<closure ItemId for {closure#0} in pl011::init>, GenericArgs([Type(<i32 TyId>)]))`.
Output: `pl011::init::{closure#0}<i32>`.

### 9.2 `render_place` specimens

**Specimen 26: Long projection chain.**

Input: `Place { local: LocalId(3), projection: [Deref, Field(FieldIdx(2), <u32 TyId>), Index(LocalId(7))] }`
(head local `_3` is `*mut PL011RegBlock`; field 2 is named `mask`; indexed by `_7`).
Output: `((*_3).mask: u32)[_7]`.

Pre/post-wrap walk: head `_3` -> `(*_3)` after Deref ->
`((*_3).mask: u32)` after Field (Field's `(X.<name>: <ty>)`
wrap applies AROUND the prior `(*_3)`, preserving the
deref-then-field precedence) -> `((*_3).mask: u32)[_7]`
after Index. Without the outer parens around `(*_3)`,
the rendering `(*_3.mask: u32)` would parse as
"dereference `_3.mask`," which inverts the projection
order.

### 9.3 `render_operand` specimens

**Specimen 27: Copy.**

Input: `Operand::Copy(Place { local: LocalId(2), projection: [] })`.
Output: `copy _2`.

**Specimen 28: Move.**

Input: `Operand::Move(Place { local: LocalId(4), projection: [] })`.
Output: `move _4`.

**Specimen 29: Constant.**

Input: `Operand::Constant(<ConstId for 42_i32>)`.
Output: `42_i32` (via `render_const`).

### 9.4 `render_rvalue` specimens

**Specimen 30: Use.**

Input: `Rvalue::Use(Operand::Copy(Place { local: LocalId(3), projection: [] }))`.
Output: `copy _3`.

**Specimen 31: Repeat.**

Input: `Rvalue::Repeat(Operand::Constant(<ConstId for 0_u8>), 16)`.
Output: `[0_u8; 16]`.

**Specimen 32: Ref shared.**

Input: `Rvalue::Ref(Provenance::SharedBorrow, Place { local: LocalId(2), projection: [] })`.
Output: `&_2`.

**Specimen 33: Ref unique.**

Input: `Rvalue::Ref(Provenance::UniqueBorrow, Place { local: LocalId(2), projection: [] })`.
Output: `&mut _2`.

**Specimen 34: RawPtr const.**

Input: `Rvalue::RawPtr(Mutability::Not, Place { local: LocalId(5), projection: [] })`.
Output: `&raw const _5`.

**Specimen 35: RawPtr mut.**

Input: `Rvalue::RawPtr(Mutability::Mut, Place { local: LocalId(5), projection: [] })`.
Output: `&raw mut _5`.

**Specimen 36: Cast (unambiguous).**

Input: `Rvalue::Cast(CastKind::IntToInt, Operand::Copy(<_3>), <i64 TyId>)`.
Output: `copy _3 as i64`.

**Specimen 37: Cast (PointerCoercion ambiguous).**

Input: `Rvalue::Cast(CastKind::PointerCoercion(ReifyFnPointer), Operand::Move(<_3>), <fn(i32) -> u32 TyId>)`
(coerces a `Ty::FnDef` ZST into a `Ty::FnPtr`).
Output: `move _3 as fn(i32) -> u32 (PointerCoercion(ReifyFnPointer))`.

**Specimen 38: BinaryOp.**

Input: `Rvalue::BinaryOp(BinOp::Add, Operand::Copy(<_3>), Operand::Copy(<_4>))`.
Output: `copy _3 + copy _4`.

**Specimen 39: UnaryOp.**

Input: `Rvalue::UnaryOp(UnOp::Neg, Operand::Copy(<_3>))`.
Output: `-copy _3`.

**Specimen 40: Discriminant.**

Input: `Rvalue::Discriminant(Place { local: LocalId(5), projection: [] })`.
Output: `discriminant(_5)`.

**Specimen 41: Aggregate Array.**

Input: `Rvalue::Aggregate(AggregateKind::Array(<u8 TyId>),
[Operand::Constant(<ConstId for 1_u8>),
Operand::Constant(<ConstId for 2_u8>),
Operand::Constant(<ConstId for 3_u8>)])`.
Output: `[1_u8, 2_u8, 3_u8]`.

**Specimen 42: Aggregate Tuple arity-2.**

Input: `Rvalue::Aggregate(AggregateKind::Tuple(<tuple TyId>),
[Operand::Constant(<ConstId for 42_i32>), Operand::Constant(<ConstId for true>)])`.
Output: `(42_i32, true)`.

**Specimen 43: Aggregate Struct (struct-variant).**

Input: `Rvalue::Aggregate(AggregateKind::Struct(<Point TyId>),
[Operand::Constant(<ConstId for 0_i32>), Operand::Constant(<ConstId for 1_i32>)])`
where `<Point TyId>` resolves to `Ty::Struct { item: <Point ItemId>,
args: GenericArgs::empty(), layout: ... }` with fields named `x`, `y`.
Output: `geom::Point { x: 0_i32, y: 1_i32 }`.

**Specimen 44: Aggregate Enum tuple-variant.**

Input: `Rvalue::Aggregate(AggregateKind::Enum(<Option<u32> TyId>, VariantIdx(1)),
[Operand::Constant(<ConstId for 42_u32>)])` where `<Option<u32> TyId>`
resolves to `Ty::Enum { item: <Option ItemId>, args: GenericArgs([Type(<u32 TyId>)]),
layout: ... }`; variant 1 is `Some` (tuple-variant).
Output: `core::option::Option<u32>::Some(42_u32)`.

**Specimen 45: Aggregate Enum C-style.**

Input: `Rvalue::Aggregate(AggregateKind::Enum(<Color TyId>, VariantIdx(0)), [])`
where `<Color TyId>` resolves to `Ty::Enum { item: <Color ItemId>,
args: GenericArgs::empty(), layout: ... }`; variant 0 is `Red` (C-style).
Output: `pl011::Color::Red`.

**Specimen 46: Aggregate Union.**

Input: `Rvalue::Aggregate(AggregateKind::Union(<MyUnion TyId>, FieldIdx(1)),
[Operand::Constant(<ConstId for u8::MAX>)])` where `<MyUnion TyId>`
resolves to `Ty::Union { item: <MyUnion ItemId>, args: GenericArgs::empty(),
layout: ... }`; field 1 is `byte`.
Output: `pl011::regs::MyUnion { byte: u8::MAX }` (D-14-4.10 sentinel-collapsing applied; the bridge-produced `Uint(U8, 0xFF)` decodes to `255` == `u8::MAX`).

**Specimen 47: Aggregate Closure.**

Input: `Rvalue::Aggregate(AggregateKind::Closure(<closure TyId>),
[Operand::Move(<_3>)])` where the closure has one upvar named `count`.
Output: `pl011::init::{closure#0} { count: move _3 }`.

**Specimen 47b: Aggregate RawPtr (thin).**

Input: `Rvalue::Aggregate(AggregateKind::RawPtr(<u8 TyId>, Mutability::Mut),
[Operand::Copy(<_3>), Operand::Constant(<ConstId for unit ()>)])`
(thin pointer construction; `_3` is the data pointer; metadata is the unit value).
Output: `(copy _3, ()) as *mut u8`.

**Specimen 47c: Aggregate RawPtr (fat).**

Input: `Rvalue::Aggregate(AggregateKind::RawPtr(<[u8] TyId>, Mutability::Not),
[Operand::Copy(<_3>), Operand::Copy(<_4>)])` (fat pointer construction
to a slice; `_3` is the data pointer; `_4` is the length metadata).
Output: `(copy _3, copy _4) as *const [u8]`.

### 9.5 `render_statement` specimens

**Specimen 48: Assign.**

Input: `Statement { kind: Assign(Place { local: LocalId(3), projection: [] }, Rvalue::Use(Operand::Constant(<ConstId for 42_i32>))), span }`.
Output (default opts): `_3 = 42_i32;`.

**Specimen 49: SetDiscriminant.**

Input: `Statement { kind: SetDiscriminant(Place { local: LocalId(3), projection: [] }, VariantIdx(1)), span }`
where _3's type is Option<i32> (variant 1 named "Some").
Output: `discriminant(_3) = Some;`.

**Specimen 50: Intrinsic Assume.**

Input: `Statement { kind: Intrinsic(NonDivergingIntrinsic::Assume(Operand::Copy(<_5>))), span }`.
Output: `assume(copy _5);`.

**Specimen 51: Intrinsic CopyNonOverlapping.**

Input: `Statement { kind: Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping(CopyNonOverlapping {
    src: Operand::Move(<_3>), dst: Operand::Move(<_4>), count: Operand::Constant(<ConstId for 16_usize>) })), span }`.
Output: `copy_nonoverlapping(move _3, move _4, 16_usize);`.

### 9.6 `render_terminator` specimens

Specimens 52-58 show the output of `render_terminator`
itself (no trailing `;`). The trailing `;` visible in
`render_basic_block` specimens (§9.8) is appended by
the composer.

**Specimen 52: Goto.**

Input: `Terminator { kind: Goto(BlockId(3)), span }`.
Output: `goto -> [bb3]`.

**Specimen 53: SwitchInt.**

Input: `Terminator { kind: SwitchInt {
    discr: Operand::Copy(<_3>),
    targets: SwitchTargets { cases: [(0, BlockId(1)), (1, BlockId(2))], otherwise: BlockId(3) }
}, span }`.
Output: `switchInt(copy _3) -> [0: bb1, 1: bb2, otherwise: bb3]`.

**Specimen 54: Return.**

Input: `Terminator { kind: Return, span }`.
Output: `return -> []`.

**Specimen 55: Unreachable.**

Input: `Terminator { kind: Unreachable, span }`.
Output: `unreachable -> []`.

**Specimen 56: Call (Direct).**

Input: `Terminator { kind: Call {
    callee: CallCallee::Direct(<InstanceId for pl011::write_byte::<u8>>),
    args: [Operand::Move(<_3>), Operand::Copy(<_4>)],
    destination: Place { local: LocalId(0), projection: [] },
    target: Some(BlockId(2)),
}, span }`.
Output: `_0 = pl011::write_byte<u8>(move _3, copy _4) -> [return: bb2]`.

**Specimen 57: Call (Indirect).**

Input: `Terminator { kind: Call {
    callee: CallCallee::Indirect(Operand::Copy(<_5>)),
    args: [Operand::Move(<_3>)],
    destination: Place { local: LocalId(0), projection: [] },
    target: Some(BlockId(2)),
}, span }`.
Output: `_0 = copy _5(move _3) -> [return: bb2]`.

**Specimen 58: Assert.**

Input: `Terminator { kind: Assert {
    cond: Operand::Copy(<_3>), expected: true,
    msg: AssertKind::BoundsCheck { len: Operand::Copy(<_4>), index: Operand::Copy(<_5>) },
    target: BlockId(2),
}, span }`.
Output: `assert(true, copy _3, index out of bounds: the length is copy _4 but the index is copy _5) -> [success: bb2]`.

**Specimen 58a: render_call_callee Direct (standalone).**

Input: `CallCallee::Direct(<InstanceId for pl011::write_byte::<u8>>)`
where the resolved Instance has `instance.item = <pl011::write_byte ItemId>`
and `instance.generic_args = GenericArgs([Type(<u8 TyId>)])`.
Output: `pl011::write_byte<u8>`.

**Specimen 58b: render_call_callee Indirect (standalone).**

Input: `CallCallee::Indirect(Operand::Copy(Place { local: LocalId(5), projection: [] }))`.
Output: `copy _5`.

**Specimen 58c: render_switch_targets (standalone).**

Input: `SwitchTargets {
    cases: [(0u128, BlockId(1)), (1u128, BlockId(2)), (5u128, BlockId(7))],
    otherwise: BlockId(3),
}`.
Output: `0: bb1, 1: bb2, 5: bb7, otherwise: bb3`.

### 9.7 `render_assert_kind` specimens

**Specimen 59: BoundsCheck.**

Input: `AssertKind::BoundsCheck { len: <op_len>, index: <op_idx> }`.
Output: `index out of bounds: the length is <op_len> but the index is <op_idx>`.

**Specimen 60: Overflow Add.**

Input: `AssertKind::Overflow(BinOp::Add, <op_l>, <op_r>)`.
Output: `attempt to compute `<op_l> + <op_r>`, which would overflow`.

**Specimen 61: Overflow Shl.**

Input: `AssertKind::Overflow(BinOp::Shl, <op_l>, <op_r>)`.
Output: `attempt to shift left by `<op_r>`, which would overflow`.

**Specimen 62: OverflowNeg.**

Input: `AssertKind::OverflowNeg(<op>)`.
Output: `attempt to negate `<op>`, which would overflow`.

**Specimen 63: DivisionByZero.**

Input: `AssertKind::DivisionByZero(<op>)`.
Output: `attempt to divide `<op>` by zero`.

**Specimen 64: RemainderByZero.**

Input: `AssertKind::RemainderByZero(<op>)`.
Output: `attempt to calculate the remainder of `<op>` with a divisor of zero`.

**Specimen 65: MisalignedPointerDereference.**

Input: `AssertKind::MisalignedPointerDereference { required: <op_req>, found: <op_found> }`.
Output: `misaligned pointer dereference: address must be a multiple of <op_req> but is <op_found>`.

**Specimen 66: NullPointerDereference.**

Input: `AssertKind::NullPointerDereference`.
Output: `null pointer dereference occurred`.

**Specimen 66b: InvalidEnumConstruction.**

Input: `AssertKind::InvalidEnumConstruction(<op_found>)`.
Output: `invalid enum discriminant value: <op_found>`.

### 9.8 `render_basic_block` specimens

**Specimen 67: Multi-statement block.**

Input:
```text
BasicBlock {
    statements: [
        Statement { kind: Assign(<_3>, Use(Constant(42_i32))), span },
        Statement { kind: Assign(<_4>, BinaryOp(Add, Copy(<_3>), Constant(1_i32))), span },
    ],
    terminator: Terminator { kind: Return, span },
}
```
indexed at BlockId(0).
Output:
```text
bb0: {
    _3 = 42_i32;
    _4 = copy _3 + 1_i32;
    return -> [];
}
```

**Specimen 68: Empty-statements block.**

Input: BlockId(1), `BasicBlock { statements: [], terminator: Terminator { kind: Goto(BlockId(2)), span } }`.
Output:
```text
bb1: {
    goto -> [bb2];
}
```

### 9.9 `render_item` specimens

**Specimen 69: Bare function (zero attributes).**

Input: `Item { kind: ItemKind::Function, ..., linkage: Default,
inline_hint: None, link_section: None, is_unsafe: false,
is_phantom_data: false, is_unsafe_cell: false, is_manually_drop: false,
opaque_handle_eligible: false, def_path: <pl011::init> }`.
Output: `fn pl011::init`.

**Specimen 70: Function with full attribute stress.**

Input: `Item { kind: ItemKind::Function, ..., linkage: NoMangle,
inline_hint: Always, link_section: Some(".text.boot"),
is_unsafe: true,
is_phantom_data: false, is_unsafe_cell: false, is_manually_drop: false,
opaque_handle_eligible: false, def_path: <bsp::reset> }`.
Output:
```text
#[no_mangle]
#[inline(always)]
#[link_section = ".text.boot"]
unsafe fn bsp::reset
```

**Specimen 70b: Struct with `opaque_handle_eligible: true`.**

Input: `Item { kind: ItemKind::Struct, def_path: <pl011::Handle>, ...,
linkage: Default, inline_hint: None, link_section: None, is_unsafe: false,
is_phantom_data: false, is_unsafe_cell: false, is_manually_drop: false,
opaque_handle_eligible: true }`.
Output:
```text
#[opaque_handle]
struct pl011::Handle
```

(Specimen 70b exercises the
`opaque_handle_eligible: true` bridge-outcome
rendering on the only `ItemKind` that can carry it
per `spec.md` §8.0 invariant 19 -- a non-generic
`#[repr(C)]` ZST Struct. The `#[opaque_handle]` line
is emitted per §5.3's per-flag table row.
Specimen 70's split + 70b's addition closes
external-review Finding #4 per stage-14 D-14-9;
specimen count drifts to ~76 with the gap-free
70 -> 70b -> 71 sequence acceptable per D-13-7.2's
policy-bucket-exception spirit.)

**Specimen 71: Static (mut).**

Input: `Item { kind: ItemKind::Static { mutability: Mutability::Mut },
def_path: <CONFIG>, ..., all flags default }`.
Output: `static mut CONFIG`.

**Specimen 72: Const.**

Input: `Item { kind: ItemKind::Const, def_path: <MAX_BUF_SIZE>, ..., all flags default }`.
Output: `const MAX_BUF_SIZE`.

**Specimen 73: Struct.**

Input: `Item { kind: ItemKind::Struct, def_path: <pl011::PL011>, ..., all flags default }`.
Output: `struct pl011::PL011`.

**Specimen 74: Enum.**

Input: `Item { kind: ItemKind::Enum, def_path: <core::option::Option>, ..., all flags default }`.
Output: `enum core::option::Option`.

**Specimen 75: Union with lang-flag.**

Input: `Item { kind: ItemKind::Union, def_path: <core::cell::UnsafeCell>, ...,
linkage: Default, inline_hint: None, link_section: None, is_unsafe: false,
is_phantom_data: false, is_unsafe_cell: true, is_manually_drop: false,
opaque_handle_eligible: false }`.
Output:
```text
#[lang = "unsafe_cell"]
union core::cell::UnsafeCell
```

### 9.10 `render_const` specimens

Specimens pinning the textual contract for the
stage-14 composer `render_const(ConstId, TyId)` per
D-14-4.* sub-decisions. Each specimen exercises one
locked arm. Calls are written as
`render_const(<const>, <ty>)`.

**Specimen 76: Bool true.**

Input: `render_const(Const::Bool(true), Ty::Bool)`.
Output: `true`. (D-14-4.2 scalar arm; no suffix.)

**Specimen 77: Bool false.**

Input: `render_const(Const::Bool(false), Ty::Bool)`.
Output: `false`. (D-14-4.2 scalar arm.)

**Specimen 78: Int decimal (non-sentinel).**

Input: `render_const(Const::Int(IntKind::I32, 42),
Ty::Int(IntKind::I32))`.
Output: `42_i32`. (D-14-4.2 underscore-decimal form.)

**Specimen 79: Int sentinel MAX.**

Input: `render_const(Const::Int(IntKind::I32, 0x7FFFFFFF),
Ty::Int(IntKind::I32))`.
Output: `i32::MAX`. (D-14-4.10 sentinel collapsing;
no `_i32` suffix on sentinel form.)

**Specimen 80: Int sentinel MIN.**

Input: `render_const(Const::Int(IntKind::I32, 0xFFFFFFFF80000000),
Ty::Int(IntKind::I32))`. (Sign-extended `i32::MIN`
stored as `u128`.)
Output: `i32::MIN`. (D-14-4.10 sentinel.)

**Specimen 81: Uint sentinel MAX (target-pointer-width-independent).**

Input: `render_const(Const::Uint(UintKind::Usize,
0xFFFFFFFF), Ty::Uint(UintKind::Usize))` on a 32-bit
target.
Output: `usize::MAX`. (D-14-4.10 sentinel; renders
symbolically regardless of target pointer width.)

**Specimen 82: Float finite.**

Input: `render_const(Const::Float(FloatKind::F32,
0x3f800000), Ty::Float(FloatKind::F32))`.
Output: `1f32`. (D-14-4.2 finite-precision rule;
integer-valued finite floats render without fractional
part.)

**Specimen 83: Float positive infinity.**

Input: `render_const(Const::Float(FloatKind::F32,
0x7f800000), Ty::Float(FloatKind::F32))`.
Output: `inf_f32`. (D-14-4.12 non-finite form;
underscore-broken.)

**Specimen 84: Float NaN.**

Input: `render_const(Const::Float(FloatKind::F32,
0x7fc00000), Ty::Float(FloatKind::F32))`.
Output: `NaN_f32`. (D-14-4.12 non-finite; quiet NaN
bit pattern. All NaN bit patterns collapse to single
form.)

**Specimen 85: Char ASCII.**

Input: `render_const(Const::Char(0x61), Ty::Char)`.
Output: `'a'`. (D-14-4.2 char Debug form.)

**Specimen 86: Char escape.**

Input: `render_const(Const::Char(0x0A), Ty::Char)`.
Output: `'\n'`. (D-14-4.2 char Debug form with
standard escape.)

**Specimen 87: ZST unit.**

Input: `render_const(Const::ZST, Ty::Tuple([]))`.
Output: `()`. (D-14-4.3 ZST-over-unit-tuple.)

**Specimen 88: ZST FnDef.**

Input: `render_const(Const::ZST, Ty::FnDef(<pl011::init ItemId>, []))`.
Output: `pl011::init`. (D-14-4.3 ZST-over-FnDef
arm; rustc-mirrored value-namespace path.)

**Specimen 89: ZST PhantomData<u32>.**

Input: `render_const(Const::ZST, Ty::Struct(<core::marker::PhantomData ItemId>,
[Type(<u32 TyId>)], <fieldless layout>))`.
Output: `core::marker::PhantomData::<u32>`. (D-14-4.3
fieldless-struct arm with generic args.)

**Specimen 90: ZST composite tuple (recursion).**

Input: `render_const(Const::ZST, Ty::Tuple([<()>, <()>, <()>]))`.
Output: `((), (), ())`. (D-14-4.3 composite-ZST
recursion; the same `Const::ZST` ConstId threads
through each sub-field.)

**Specimen 91: NullPtr raw const pointer.**

Input: `render_const(Const::NullPtr(<u32 TyId>),
Ty::Ptr(<u32 TyId>, Mutability::Not))`.
Output: `0x0 as *const u32`. (D-14-4.7 typed-cast
form.)

**Specimen 92: NullPtr shared reference.**

Input: `render_const(Const::NullPtr(<u32 TyId>),
Ty::Ref(<u32 TyId>, Provenance::SharedBorrow))`.
Output: `0x0 as &u32`. (D-14-4.7 typed-cast form.)

**Specimen 93: Reloc to FnDef (no cast).**

Input: `render_const(Const::Reloc(<inst:foo>),
Ty::FnDef(<foo ItemId>, [<u32 TyId>]))` where `inst:foo`
resolves to `InstanceKind::Function { .. }` with the
matching `(item, args)`.
Output: `pl011::foo::<u32>`. (D-14-4.8 FnDef arm;
matches D-14-4.3 ZST-over-FnDef intentionally.)

**Specimen 94: Reloc to FnPtr-coerced fn (typed cast).**

Input: `render_const(Const::Reloc(<inst:foo>),
Ty::FnPtr(<sig u32 -> bool>))` where `inst:foo`
resolves to a Function with matching signature.
Output: `pl011::foo as fn(u32) -> bool`. (D-14-4.8
FnDef -> FnPtr coercion via typed cast.)

**Specimen 95: Reloc to source-defined Static.**

Input: `render_const(Const::Reloc(<inst:MY_REG>),
Ty::Ptr(<u32 TyId>, Mutability::Mut))` where
`inst:MY_REG` resolves to a non-synth Static.
Output: `pl011::MY_REG as *mut u32`. (D-14-4.8
Static typed-cast.)

**Specimen 96: Reloc to ConstMemoryStatic, byte-string special case.**

Input: `render_const(Const::Reloc(<inst:cm_<hash>>),
Ty::Ref(<[u8; 5] TyId>, Provenance::SharedBorrow))`
where `inst:cm_<hash>` resolves to a Static with
`item_synthetic_kinds == ConstMemoryStatic { .. }` and
init `Const::Bytes([Value(b'h'), Value(b'e'),
Value(b'l'), Value(b'l'), Value(b'o')])`.
Output: `b"hello"`. (D-14-4.8 special case;
rustc-mirrored.)

**Specimen 97: Aggregate Array of u8.**

Input: `render_const(Const::Aggregate([<c1>, <c2>, <c3>]),
Ty::Array(<u8 TyId>, 3))` where c1=Uint(U8,1),
c2=Uint(U8,2), c3=Uint(U8,3).
Output: `[1_u8, 2_u8, 3_u8]`. (D-14-4.4 Array arm;
each child rendered with `elem` as consumer Ty per
D-14-4.9.)

**Specimen 98: Aggregate 2-Tuple.**

Input: `render_const(Const::Aggregate([<c1>, <c2>]),
Ty::Tuple([<i32 TyId>, <bool TyId>]))` where
c1=Int(I32,42), c2=Bool(true).
Output: `(42_i32, true)`. (D-14-4.4 Tuple arm.)

**Specimen 99: Aggregate 1-Tuple (trailing comma).**

Input: `render_const(Const::Aggregate([<c1>]),
Ty::Tuple([<i32 TyId>]))` where c1=Int(I32,42).
Output: `(42_i32,)`. (D-14-4.4 1-tuple trailing
comma.)

**Specimen 100: Aggregate Struct (named-field).**

Input: `render_const(Const::Aggregate([<c1>, <c2>]),
Ty::Struct(<Point ItemId>, [], <layout with fields
x: i32, y: i32>))` where c1=Int(I32,0), c2=Int(I32,1).
Output: `geom::Point { x: 0_i32, y: 1_i32 }`.
(D-14-4.4 Struct named-field arm.)

**Specimen 101: Aggregate &str literal (special case).**

Input: `render_const(Const::Aggregate([<reloc_cm>, <len>]),
Ty::Ref(<str TyId>, Provenance::SharedBorrow))`
where `reloc_cm` resolves to a ConstMemoryStatic
synth Static with init `Const::Bytes([Value(b'h'),
Value(b'e'), Value(b'l'), Value(b'l'), Value(b'o')])`
and `len = Const::Uint(UintKind::Usize, 5)`.
Output: `"hello"`. (D-14-4.4 &str special case;
UTF-8 lossy decode.)

**Specimen 102: Aggregate &[u8] literal (special case).**

Input: `render_const(Const::Aggregate([<reloc_cm>, <len>]),
Ty::Ref(<[u8] TyId>, Provenance::SharedBorrow))`
where `reloc_cm` resolves to a ConstMemoryStatic with
init `Const::Bytes([Value(0), Value(1), Value(2), Value(3)])`
and `len = Const::Uint(UintKind::Usize, 4)`.
Output: `b"\x00\x01\x02\x03"`. (D-14-4.4 &[u8]
special case; byte-string with escape-ascii.)

**Specimen 103: EnumValue Option::None.**

Input: `render_const(Const::EnumValue { variant_idx: 0,
payload: [] }, Ty::Enum(<core::option::Option ItemId>,
[Type(<u32 TyId>)], <layout with EnumShape::Multiple
{ variants: [VariantLayout { name: "None", fields:
[] }, VariantLayout { name: "Some", fields: [_0: u32] }] }>))`.
Output: `core::option::Option::<u32>::None`.
(D-14-4.5 multi-variant unit arm.)

**Specimen 104: EnumValue Option::Some(42_u32).**

Input: `render_const(Const::EnumValue { variant_idx: 1,
payload: [<c>] }, Ty::Enum(<core::option::Option>,
[Type(<u32>)], <same layout as 103>))` where
c=Uint(U32,42).
Output: `core::option::Option::<u32>::Some(42_u32)`.
(D-14-4.5 multi-variant tuple arm.)

**Specimen 105: EnumValue C-style.**

Input: `render_const(Const::EnumValue { variant_idx: 0,
payload: [] }, Ty::Enum(<pl011::Color>, [], <layout with
EnumShape::Multiple { variants: [{name:"Red", fields:[]},
{name:"Green", fields:[]}, {name:"Blue", fields:[]}] }>))`.
Output: `pl011::Color::Red`. (D-14-4.5 multi-variant
unit arm; C-style enum.)

**Specimen 106: EnumValue named-field variant.**

Input: `render_const(Const::EnumValue { variant_idx: 0,
payload: [<c>] }, Ty::Enum(<pl011::MyEnum>, [], <layout
with EnumShape::Multiple { variants: [{name:"Err",
fields:[reason: &str]}, ...] }>))` where c renders to
`"oops"`.
Output: `pl011::MyEnum::Err { reason: "oops" }`.
(D-14-4.5 multi-variant named-field arm.)

**Specimen 107: Bytes all-uninit shortcut.**

Input: `render_const(Const::Bytes([Uninit, Uninit,
Uninit, Uninit]), Ty::Array(<u8 TyId>, 4))`.
Output: `<uninit>`. (D-14-4.6 all-uninit shortcut.)

**Specimen 108: Bytes byte-array deref form.**

Input: `render_const(Const::Bytes([Value(b'h'),
Value(b'e'), Value(b'l'), Value(b'l'), Value(b'o')]),
Ty::Array(<u8 TyId>, 5))`.
Output: `*b"hello"`. (D-14-4.6 byte-array `*b"..."`
form; rustc-mirrored deref form.)

**Specimen 109: Bytes structural fallback.**

Input: `render_const(Const::Bytes([Value(0x00),
Uninit, Value(0xff), Reloc(<inst:some_item>)]),
Ty::Struct(<MyStruct ItemId>, [], <some struct
layout>))`.
Output: `bytes![0x00, <uninit>, 0xff, <reloc: pl011::some_item>]`.
(D-14-4.6 structural fallback; mixed three-state
bytes.)

### 9.11 `render_instance` specimens

Specimens pinning the textual contract for the
stage-14 composer `render_instance(InstanceId)` per
D-14-5.* sub-decisions. Each specimen exercises one
or more locked arms. Calls are written as
`render_instance(<inst>)`. Per D-14-5.7 LOCKED, the
composer does not call `render_item`; both renderers
share the §5.6 helper `render_item_attribute_block`
for the Stream 1 attribute prefix per D-14-5.6 /
§10.2.7.

**Specimen 110: Plain function (zero attributes, unit
return).**

Input: `render_instance(<inst:pl011::write_byte>)`
where the Instance has
`kind: InstanceKind::Function { fn_sig:
<FnSig{inputs: [Ty::Ref(<PL011>, SharedBorrow),
Ty::Uint(U8)], output: Ty::Tuple([]), abi: FnAbi::Rust,
c_variadic: false, safety: Safe}>, body: Some(<b>),
.. }`, `item: <pl011::write_byte ItemId>` with all
Item flags default and `is_unsafe == false`.
Output:
```text
fn pl011::write_byte(_1: &pl011::PL011, _2: u8) -> () {
    let mut _0: ();
    bb0: {
        ...
    }
}
```
(D-14-5.2 Function envelope; empty attribute block;
no keyword decoration; non-empty arg-list; unit
return rendered as `-> ()` per always-render rule;
body envelope per §3.1 unchanged.)

**Specimen 111: Unsafe function with raw-pointer
arg.**

Input: `render_instance(<inst:pl011::ptr_write>)`
with `Item.is_unsafe == true`,
`fn_sig: {inputs: [Ty::Ptr(<u8>, Mut),
Ty::Uint(U8)], output: Ty::Tuple([]),
abi: FnAbi::Rust, c_variadic: false,
safety: Unsafe}`.
Output:
```text
unsafe fn pl011::ptr_write(_1: *mut u8, _2: u8) -> () {
    let mut _0: ();
    bb0: {
        ...
    }
}
```
(D-14-5.2 aspect 2 `unsafe` keyword from
`Item.is_unsafe` per §5.3 convention; `*mut u8`
renders via §6.2.)

**Specimen 112: Function with full Stream-1
attribute block (no_mangle + inline_always +
link_section).**

Input: `render_instance(<inst:pl011::init>)` with
`Item.linkage = NoMangle`,
`Item.inline_hint = Always`,
`Item.link_section = Some(<StringId resolving to ".text.boot">)`,
`Item.is_unsafe = true`,
`fn_sig: {inputs: [Ty::Ptr(<PL011>, Mut)],
output: Ty::Int(I32), abi: FnAbi::Rust, c_variadic:
false, safety: Unsafe}`, `body: Some(<b>)`. No
`domain_attrs.item_attrs` entries (Stream 2 empty).
Output:
```text
#[no_mangle]
#[inline(always)]
#[link_section = ".text.boot"]
unsafe fn pl011::init(_1: *mut pl011::PL011) -> i32 {
    let mut _0: i32;
    bb0: {
        ...
    }
}
```
(D-14-5.6 Stream 1 only via §5.6 shared helper;
§5.3 row order: linkage -> inline_hint ->
link_section. Stream 2 empty.)

**Specimen 113: Foreign C function declaration
(body: None).**

Input: `render_instance(<inst:libc::printf>)` with
`Item.linkage = Linkage::Foreign`,
`fn_sig: {inputs: [Ty::Ptr(<u8>, Not)],
output: Ty::Int(I32), abi: FnAbi::C, c_variadic:
false, safety: Safe}`, `body: None`. No
`domain_attrs.item_attrs` entries.
Output:
```text
#[foreign]
extern "C" fn libc::printf(_1: *const u8) -> i32;
```
(D-14-5.2 aspect 5 declaration form: `body == None`
trailer `;`; aspect 2 `extern "C"` from
`fn_sig.abi = FnAbi::C`; Stream 1 `#[foreign]` from
`Item.linkage = Foreign` per §5.3. Per Q5: foreign
declarations render as emission units that rustc's
MIR pretty-printer skips entirely.)

**Specimen 114: Foreign C variadic declaration.**

Input: `render_instance(<inst:libc::printf_variadic>)`
with `Item.linkage = Foreign`,
`fn_sig: {inputs: [Ty::Ptr(<u8>, Not)],
output: Ty::Int(I32), abi: FnAbi::C,
c_variadic: true, safety: Safe}`, `body: None`.
Output:
```text
#[foreign]
extern "C" fn libc::printf_variadic(_1: *const u8, ...) -> i32;
```
(D-14-5.2 aspect 3 variadic arg-list: `c_variadic ==
true` appends `, ...` after the last fixed input.
`spec.md` §14.3 invariant `inputs.len() >= 1`
guarantees the leading comma.)

**Specimen 115: Diagnostic-logging intrinsic
(Stream 1 + Stream 2).**

Input: `render_instance(<inst:regseq::log::intrinsics::log_info>)`
with `Item.linkage = Default`,
`Item.inline_hint = Never`,
`fn_sig: {inputs: [Ty::Ref(<str>, SharedBorrow),
Ty::Ref(Ty::Slice(Ty::Struct(<LogArg>, [], <layout>)),
SharedBorrow)], output: Ty::Tuple([]),
abi: FnAbi::Rust, c_variadic: false, safety: Safe}`,
`body: Some(<b>)`. `domain_attrs.item_attrs[item] ==
[AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info))]`.
Output:
```text
#[inline(never)]
#[rseq::intrinsic("log-info")]
fn regseq::log::intrinsics::log_info(_1: &str, _2: &[regseq::log::LogArg<'_>]) -> () {
    let mut _0: ();
    bb0: {
        ...
    }
}
```
(D-14-5.6 two-stream interleaving: Stream 1 emits
`#[inline(never)]` from `Item.inline_hint = Never`;
Stream 2 emits `#[rseq::intrinsic("log-info")]` from
`AttrData::Intrinsic(IntrinsicKind::Log(Info))` per
the §10.2.7 per-`AttrData` render table.)

**Specimen 116: Bridge-synthesised drop glue.**

Input: `render_instance(<inst:drop_in_place_for_PL011>)`
with `item: <core::ptr::drop_in_place ItemId>`,
`generic_args: [GenericArg::Type(<pl011::PL011>)]`,
`fn_sig: {inputs: [Ty::Ptr(<PL011>, Mut)],
output: Ty::Tuple([]), abi: FnAbi::Rust,
c_variadic: false, safety: Safe}`,
`flavor: FunctionInstanceFlavor::DropGlue`,
`body: Some(<b>)`.
Output:
```text
fn core::ptr::drop_in_place::<pl011::PL011>(_1: *mut pl011::PL011) -> () {
    let mut _0: ();
    bb0: {
        ...
    }
}
```
(D-14-5.2 + Q8/Q9: `flavor: DropGlue` is identity
metadata only; DefPath encodes the synthesised
family via the `::<PL011>` generic-arg suffix per
stage-12 D-12-6. No per-flavor render branch.)

**Specimen 117: Divergent return (Ty::Never).**

Input: `render_instance(<inst:pl011::panic_handler>)`
with `fn_sig.output: Ty::Never`,
`fn_sig.inputs: [Ty::Ref(<PanicInfo>, SharedBorrow)]`,
`abi: FnAbi::Rust`, `c_variadic: false`,
`safety: Safe`, `body: Some(<b>)`.
Output:
```text
fn pl011::panic_handler(_1: &core::panic::PanicInfo) -> ! {
    let mut _0: !;
    bb0: {
        ...
    }
}
```
(D-14-5.2 aspect 4 always-render `-> Ret`;
`Ty::Never` renders as `!` per §6.1.)

**Specimen 118: Defined immutable static.**

Input: `render_instance(<inst:pl011::BASE_ADDR>)`
with `kind: InstanceKind::Static { ty:
Ty::Uint(U32), init: Some(<const:268472320_u32>) }`,
`item: <pl011::BASE_ADDR>` with
`ItemKind::Static { mutability: Mutability::Not }`,
all flags default.
Output: `static pl011::BASE_ADDR: u32 = 268472320_u32;`
(D-14-5.3 definition form; immutable prefix `static `
from `ItemKind::Static.mutability = Not`; value via
`render_const(<268472320_u32 ConstId>, <Ty::Uint(U32) TyId>)`
per D-14-4.1 pair-form.)

**Specimen 119: Defined mutable static with struct
initializer.**

Input: `render_instance(<inst:pl011::DRIVER_STATE>)`
with `kind: InstanceKind::Static { ty:
Ty::Struct(<State>, [], <layout>), init:
Some(<const:Aggregate>) }`,
`item: <pl011::DRIVER_STATE>` with
`ItemKind::Static { mutability: Mutability::Mut }`.
Output: `static mut pl011::DRIVER_STATE: pl011::State = pl011::State { initialised: false, count: 0_u32 };`
(D-14-5.3 mutable prefix `static mut `;
`render_const` Aggregate dispatch per D-14-4.4
produces the struct-initializer form on one line.)

**Specimen 120: Foreign extern static (init: None).**

Input: `render_instance(<inst:libc::stderr>)` with
`kind: InstanceKind::Static { ty: Ty::Ptr(<FILE>,
Mut), init: None }`,
`item: <libc::stderr>` with
`Item.linkage = Linkage::Foreign`,
`ItemKind::Static { mutability: Mutability::Not }`.
Output:
```text
#[foreign]
static libc::stderr: *mut libc::FILE;
```
(D-14-5.3 declaration form: `init == None` drops
the `= ...` segment; trailing `;`. `#[foreign]`
emitted by Stream 1 from `Item.linkage = Foreign`
per §5.3. Biconditional `init.is_none() iff
Item.linkage == Foreign` per `spec.md` §3.4 step 8
/ D-12-2.2.)

**Specimen 121: Scalar const.**

Input: `render_instance(<inst:pl011::TX_FIFO_SIZE>)`
with `kind: InstanceKind::Const { ty: Ty::Int(I32),
value: <const:16_i32> }`.
Output: `const pl011::TX_FIFO_SIZE: i32 = 16_i32;`
(D-14-5.4 always-populated `value` per stage-12
D-12-2; single-line composer.)

**Specimen 122: Reference-typed const (string
literal).**

Input: `render_instance(<inst:pl011::GREETING>)`
with `kind: InstanceKind::Const { ty:
Ty::Ref(<str>, SharedBorrow), value:
<const:Aggregate([Reloc, Uint])> }`.
Output: `const pl011::GREETING: &str = "hello";`
(D-14-5.4 value composes
`render_const(<const ConstId>, <&str TyId>)` per
D-14-4.1 pair-form; the &str fat-pointer special
case per D-14-4.4 produces `"hello"` form.)

**Specimen 123: Unit struct (empty fields,
collapses to unit form).**

Input: `render_instance(<inst:pl011::Marker>)` with
`kind: InstanceKind::Struct { layout: <Ty::Struct(<pl011::Marker>,
[], <Layout{kind: LayoutKind::Struct{fields: []}, ..}>) TyId> }`,
no `domain_attrs` entries.
Output: `struct pl011::Marker;`
(D-14-5.5 Struct empty-fields case; collapses three
Rust syntactic empty-struct shapes to unit form;
zero generic args -> no `<...>` decoration.)

**Specimen 124: Tuple struct (synthetic `_N`
field names).**

Input: `render_instance(<inst:pl011::Coord>)` with
`kind: InstanceKind::Struct { layout: <Ty::Struct(<pl011::Coord>,
[], <Layout{kind: LayoutKind::Struct{fields:
[FieldLayout{name:"_0", ty: Ty::Int(I32), offset: 0},
FieldLayout{name:"_1", ty: Ty::Int(I32), offset: 4}]}, ..}>) TyId> }`.
Output: `struct pl011::Coord(i32, i32);`
(D-14-5.5 Struct tuple-form: all field names match
`_<idx>` synthetic pattern per §3.4; renders as
`<path>(<T0>, <T1>);` one-line form.)

**Specimen 125: Named-field struct with lang-flag
+ generic args.**

Input: `render_instance(<inst:ManuallyDrop_PL011_Resource>)`
with `item: <core::mem::ManuallyDrop>`,
`Item.is_manually_drop = true`,
`kind: InstanceKind::Struct { layout: <Ty::Struct(<core::mem::ManuallyDrop>,
[GenericArg::Type(<pl011::Resource>)],
<Layout{kind: LayoutKind::Struct{fields:
[FieldLayout{name:"value", ty: <pl011::Resource TyId>, offset: 0}]}, ..}>) TyId> }`.
Output:
```text
#[lang = "manually_drop"]
struct core::mem::ManuallyDrop<pl011::Resource> {
    value: pl011::Resource,
}
```
(D-14-5.5 Struct named-field form with generic-args
decoration via `render_generic_args(args, item)` per
§6.7 / D-14-4.9; Stream 1 emits `#[lang = "manually_drop"]`
from `Item.is_manually_drop = true` per §5.3.)

**Specimen 126: Multi-variant enum with mixed
ctor forms.**

Input: `render_instance(<inst:pl011::Status>)` with
`kind: InstanceKind::Enum { layout: <Ty::Enum(<pl011::Status>,
[], <Layout{kind: LayoutKind::Enum{shape: EnumShape::Multiple{
tag: <TagScalar>, encoding: Direct, variants: [
  VariantLayout{name: "Ready", fields: []},
  VariantLayout{name: "Busy", fields: [FieldLayout{name: "_0", ty: Ty::Uint(U32), offset: ..}]},
  VariantLayout{name: "Error", fields: [
    FieldLayout{name: "code", ty: Ty::Int(I32), offset: ..},
    FieldLayout{name: "retry", ty: Ty::Bool, offset: ..},
  ]},
]}}, ..}>) TyId> }`.
Output:
```text
enum pl011::Status {
    Ready,
    Busy(u32),
    Error { code: i32, retry: bool },
}
```
(D-14-5.5 Enum EnumShape::Multiple dispatch +
per-variant ctor forms: variant 0 unit (empty
fields), variant 1 tuple (synthetic `_0` name),
variant 2 named-field (real names "code"+"retry").
Trailing comma after last variant.)

**Specimen 127: Empty enum (uninhabited).**

Input: `render_instance(<inst:core::convert::Infallible>)`
with `kind: InstanceKind::Enum { layout: <Ty::Enum(<core::convert::Infallible>,
[], <Layout{kind: LayoutKind::Enum{shape: EnumShape::Empty}, ..}>) TyId> }`.
Output: `enum core::convert::Infallible {}`
(D-14-5.5 Enum EnumShape::Empty form: zero-variant
empty body; one line.)

**Specimen 128: Simple union.**

Input: `render_instance(<inst:pl011::RegBytes>)`
with `kind: InstanceKind::Union { layout: <Ty::Union(<pl011::RegBytes>,
[], <Layout{kind: LayoutKind::Union{fields: [
FieldLayout{name: "word", ty: Ty::Uint(U32), offset: 0},
FieldLayout{name: "bytes", ty: Ty::Array(Ty::Uint(U8), 4), offset: 0},
]}, ..}>) TyId> }`.
Output:
```text
union pl011::RegBytes {
    word: u32,
    bytes: [u8; 4],
}
```
(D-14-5.5 Union named-field form unconditional;
fields share offset 0 per §11.0 union convention,
not rendered.)

### 9.12 `render(&self)` specimens

Specimens cover the top-level composer renderer
`render(&self)` (§10.3). Each specimen records Input
(structured `RegseqModel.instances` state) and Output
(rendered top-level dump), exercising the §10.3.2
iteration order (`iter_enumerated()` over the
canonical F4-2 sort), §10.3.3 inter-Instance
`"\n\n"` separator, §10.3.4 no-file-envelope rule,
and §10.3.5 empty-model identity. Per-Instance
rendering follows §10.2 (covered by §9.11 specimens
110-128); this section focuses on top-level
scaffolding.

**Specimen 129: Empty model.**

Input: `model.instances = IndexVec::new()` (zero
Instances).
Output:
```text
```
(Empty `model.instances` returns the empty `String`
per §10.3.5. 0-byte output; no header; no placeholder;
no separator.)

**Specimen 130: Single-Instance model (Function).**

Input: `model.instances = [InstanceId(0)]` with
`kind: InstanceKind::Function { fn_sig: <fn(u8) ->
()>, body: Some(<2-block body>), flavor:
FunctionInstanceFlavor::Source, virtual_idx: None }`,
item = `pl011::write_byte` (rank 0, ItemId = 7).
Output:
```text
fn pl011::write_byte(_1: u8) -> () {
    let mut _0: ();

    bb0: {
        _0 = ();
        return;
    }
}
```
(`iter_enumerated()` yields one entry per §10.3.2.
The §10.3.3 `"\n\n"` separator is never emitted -- the
first-flag does not flip in a one-element iteration.
Output begins at the `fn` keyword line per §10.3.4
(no Stream-1 / Stream-2 attribute block). Per-Instance
content matches §10.2.1 Function-body form.)

**Specimen 131: Multi-Instance model with
rank-grouped F4-2 order (mixed InstanceKind).**

Input: `model.instances = [InstanceId(0),
InstanceId(1), InstanceId(2)]` with:
- `InstanceId(0)`: `InstanceKind::Function`,
  item = `pl011::write_byte` (rank 0, ItemId = 7),
  `body: Some(_)`, `fn_sig: fn(u8) -> ()`.
- `InstanceId(1)`: `InstanceKind::Static`,
  item = `pl011::BASE_ADDR` (rank 1, ItemId = 12),
  `ty: u32`, `init: Some(<Const::Uint(268472320_u32)>)`.
- `InstanceId(2)`: `InstanceKind::Struct`,
  item = `pl011::Resource` (rank 3, ItemId = 3),
  `layout: <Ty::Struct(...) with [base: u32, state:
  u32]>`.
Output:
```text
fn pl011::write_byte(_1: u8) -> () {
    let mut _0: ();

    bb0: {
        _0 = ();
        return;
    }
}

static pl011::BASE_ADDR: u32 = 268472320_u32;

struct pl011::Resource {
    base: u32,
    state: u32,
}
```
(Iteration walks `[0, 1, 2]` in InstanceId order per
§10.3.2; F4-2 ordered them by
`regseq_instance_kind_rank` -- Function (0) before
Static (1) before Struct (3) -- regardless of ItemId
numerics (Struct's ItemId 3 < Function's ItemId 7, but
rank dominates). §10.3.3 `"\n\n"` separator between
Function body and Static, and between Static and
Struct body; no leading or trailing blank per §10.3.4.
Static initializer `268472320_u32` is the decimal form
required by §10.1.1.)

**Specimen 132: Multi-Instance model with all
Instances from the same rank group (Functions only).**

Input: `model.instances = [InstanceId(0),
InstanceId(1), InstanceId(2)]` with all three of
kind `InstanceKind::Function` and items
(rank 0 / ItemId ordered):
- `InstanceId(0)`: `regseq::log::intrinsics::log_info`
  (ItemId = 4), `body: Some(_)`, `fn_sig: fn(*const u8) -> ()`,
  with `domain_attrs.item_attrs[item] =
  [Intrinsic(IntrinsicKind::Log(LogLevel::Info))]`.
- `InstanceId(1)`: `pl011::init` (ItemId = 9),
  `body: Some(_)`, `fn_sig: fn() -> ()`, no
  attributes.
- `InstanceId(2)`: `libc::printf` (ItemId = 15),
  `body: None`, `fn_sig: fn(*const u8) -> i32`,
  `flavor: FunctionInstanceFlavor::Source`,
  `Item.linkage == Foreign`,
  Stream-1 attrs: `linkage = Foreign`,
  `abi = FnAbi::C`.
Output:
```text
#[rseq::intrinsic("log-info")]
fn regseq::log::intrinsics::log_info(_1: *const u8) -> () {
    let mut _0: ();

    bb0: {
        _0 = ();
        return;
    }
}

fn pl011::init() -> () {
    let mut _0: ();

    bb0: {
        _0 = ();
        return;
    }
}

#[foreign]
extern "C" fn libc::printf(_1: *const u8) -> i32;
```
(All three Functions share rank 0 per §10.3.2; F4-2
orders by ItemId within the group: 4 < 9 < 15. §10.3.3
`"\n\n"` separator between each consecutive pair;
§10.3.4 no file-level header / trailer. The third
Instance is a foreign declaration per §10.2.1:
declaration trailer `;` immediately after return-type
segment; no body block. The first Instance's first
rendered line is the Stream-2
`#[rseq::intrinsic("log-info")]` attribute (per §10.2.7's
attribute-block composition), not the `fn` keyword.
The third Instance's first rendered line is the Stream-1
`#[foreign]` attribute.)

**Specimen 133: Multi-Instance model spanning all
six rank groups.**

Input: `model.instances = [InstanceId(0) .. InstanceId(5)]`
with one Instance per `InstanceKind` rank (the
full per-rank coverage):
- `InstanceId(0)`: `InstanceKind::Function`,
  item = `pl011::ping` (rank 0, ItemId = 7),
  `body: Some(<1-block body>)`, `fn_sig: fn() -> ()`.
- `InstanceId(1)`: `InstanceKind::Static`,
  item = `pl011::CFG` (rank 1, ItemId = 11),
  `ty: u32`, `init: Some(<Const::Uint(42_u32)>)`.
- `InstanceId(2)`: `InstanceKind::Const`,
  item = `pl011::MAGIC` (rank 2, ItemId = 8),
  `ty: u32`, `value: <Const::Uint(170_u32)>`.
- `InstanceId(3)`: `InstanceKind::Struct`,
  item = `pl011::Resource` (rank 3, ItemId = 3),
  `layout` with `[handle: *mut u8]`.
- `InstanceId(4)`: `InstanceKind::Enum`,
  item = `pl011::Mode` (rank 4, ItemId = 5),
  `layout` with `EnumShape::Multiple(variants =
  [unit Off, unit Tx])`.
- `InstanceId(5)`: `InstanceKind::Union`,
  item = `pl011::RegBytes` (rank 5, ItemId = 14),
  `layout` with `[word: u32, bytes: [u8; 4]]`.
Output:
```text
fn pl011::ping() -> () {
    let mut _0: ();

    bb0: {
        _0 = ();
        return;
    }
}

static pl011::CFG: u32 = 42_u32;

const pl011::MAGIC: u32 = 170_u32;

struct pl011::Resource {
    handle: *mut u8,
}

enum pl011::Mode {
    Off,
    Tx,
}

union pl011::RegBytes {
    word: u32,
    bytes: [u8; 4],
}
```
(Full rank-grouping spectrum: Function (0) -> Static
(1) -> Const (2) -> Struct (3) -> Enum (4) -> Union (5)
per §10.3.2. Each rank transition is visually
distinguished by the first-line keyword; no rank-group
header per §10.3.3 (uniform `"\n\n"` separator).
§10.3.4 no file-level header / trailer. Static
initializer `42_u32` and Const value `170_u32` use the
decimal form required by §10.1.1. ItemId is not the
iteration controller -- rank dominates: Struct (rank 3,
ItemId 3) comes after Const (rank 2, ItemId 8) even
though 3 < 8.)

## 10. Composer renderers

The three composer renderers owned by stage 14 per
D-13-4.6's building-block-vs-composer architectural
division: `render_const`, `render_instance`, and the
top-level `render(&self)`. They consume the
stage-13-owned building blocks (`render_ty`,
`render_item`, body-level renderers) and stage-12's
`render_def_path` to produce the IR's value-level
rendering surface.

### 10.1 `render_const(ConstId, TyId) -> String`

Composer renderer for the closed 11-variant `Const`
enum (`spec.md` §9.7). Per D-14-4.1 (LOCKED), pair-form
takes `(ConstId, TyId)` where `TyId` is the
consumer-context type at the use site -- the type of
the position holding the `ConstId`:

- `InstanceKind::Static { ty, init }` -> `init.unwrap()`
  is rendered with the Static's `ty` as consumer type.
- `InstanceKind::Const { ty, value }` -> `value` is
  rendered with the Const Instance's `ty` as consumer
  type.
- `Aggregate(Vec<ConstId>)` / `EnumValue { payload:
  Vec<ConstId>, .. }` recursion -> each child is
  rendered with its per-field Ty resolved from the
  parent Ty's layout (per D-14-4.9 LOCKED; see
  §10.1.8).
- `Operand::Constant(id)` at a use site -> the
  surrounding Rvalue / Statement / Terminator caller
  threads the consumer Ty via `render_operand`'s
  extended signature `render_operand(&Operand, &Body,
  expected_ty: TyId)` per D-14-4.9 LOCKED (stage-13
  D-13-6 REOPEN; see §7.2 + §10.1.8).
- `GenericArg::Const(c)` -> the generic parameter's
  declared type, resolved from the enclosing Item's
  generic-parameter list.

**Dispatch overview.** Rustc-mirrored
`(const, ty.kind())` match per
`rustc_middle/src/mir/pretty.rs:1851`. Several arms
key solely on the consumer Ty (`Bytes` / `Aggregate` /
`ZST` / `NullPtr` / `Reloc`); the scalar arms
(`Bool` / `Char` / `Int` / `Uint` / `Float`) key on the
variant tag and ignore the consumer Ty.

Per-arm rules across the 11 `Const` variants are
specified in the subsections below; each subsection
corresponds to a D-14-4 sub-decision (all LOCKED at
Phase A on 2026-05-13).

#### 10.1.1 Scalar variants -- `Bool` / `Char` / `Int` / `Uint` / `Float`

Per D-14-4.2 LOCKED. The five scalar variants render
as Rust-source-style literal forms with a
rustc-mirrored underscore-decimal convention. Width is
self-determined by the variant's payload tag
(`IntKind` / `UintKind` / `FloatKind` per `spec.md`
§10.0); the consumer `TyId` is ignored in the scalar
dispatch path.

| Const variant         | Rendered form                       | Notes                                                                                                  |
|-----------------------|-------------------------------------|--------------------------------------------------------------------------------------------------------|
| `Bool(false)`         | `false`                             | No suffix.                                                                                             |
| `Bool(true)`          | `true`                              | No suffix.                                                                                             |
| `Char(u32)`           | `'<ch>'` (Rust char-literal Debug)  | Quoted; Rust escape rules apply (`'\n'`, `'\\''`, `'\u{1f600}'`, etc.). Bridge has validated the `u32` is a valid Unicode scalar (`spec.md` §9.7); invalid `u32` cannot reach the renderer. |
| `Int(I8, v)`          | `<dec>_i8`                          | `<dec>` is the sign-extended decimal value (e.g., `Int(I8, u128::MAX) -> -1_i8`).                      |
| `Int(I16, v)`         | `<dec>_i16`                         |                                                                                                        |
| `Int(I32, v)`         | `<dec>_i32`                         |                                                                                                        |
| `Int(I64, v)`         | `<dec>_i64`                         |                                                                                                        |
| `Int(I128, v)`        | `<dec>_i128`                        |                                                                                                        |
| `Int(Isize, v)`       | `<dec>_isize`                       | Pointer-sized integer; rendering does not depend on target pointer width (decimal is faithful).        |
| `Uint(U8, v)`         | `<dec>_u8`                          | `<dec>` is the zero-extended decimal value (the low `width` bits of the u128).                         |
| `Uint(U16, v)`        | `<dec>_u16`                         |                                                                                                        |
| `Uint(U32, v)`        | `<dec>_u32`                         |                                                                                                        |
| `Uint(U64, v)`        | `<dec>_u64`                         |                                                                                                        |
| `Uint(U128, v)`       | `<dec>_u128`                        |                                                                                                        |
| `Uint(Usize, v)`      | `<dec>_usize`                       |                                                                                                        |
| `Float(F16, bits)`    | `<val>f16`                          | `<val>` is the shortest round-trippable IEEE-754 binary16 decimal display of the bit-pattern; non-finite forms specified by D-14-4.12 (LOCKED; see "Float-rendering precision (non-finite values)" subsection below). |
| `Float(F32, bits)`    | `<val>f32`                          | Per IEEE-754 binary32.                                                                                 |
| `Float(F64, bits)`    | `<val>f64`                          | Per IEEE-754 binary64.                                                                                 |
| `Float(F128, bits)`   | `<val>f128`                         | Per IEEE-754 binary128.                                                                                |

Three policy-shaped sub-aspects split out to dedicated
sub-decisions:

- **D-14-4.10** -- `Int` / `Uint` sentinel collapsing
  (`i32::MAX` / `i32::MIN` / `u32::MAX` / etc.). LOCKED
  per the sentinel-collapsing rule below.
- **D-14-4.11** -- `Int` / `Uint` hex form admission.
  LOCKED at decimal-only for v1; no
  `PrettyPrintOptions::integer_hex` toggle. Future
  revision can add an `integer_render_style` toggle
  if phase-1 testing surfaces register-bit-pattern
  diagnostic friction (additive shape; would not
  break the locked `render_const(ConstId, TyId) ->
  String` signature). Not pre-allocated.
- **D-14-4.12** -- `Float` infinity / NaN
  representation. LOCKED per the non-finite handling
  rule below. The `<val>` column in the per-arm table
  above is finite-only; non-finite values use the
  underscore-broken `inf_f<width>` / `-inf_f<width>` /
  `NaN_f<width>` form.

**Sentinel-collapsing rule** for `Int` / `Uint`
(per D-14-4.10 LOCKED). Before applying the
underscore-decimal form, the renderer tests the
decoded value against the width-bounds sentinel
constants:

| Variant + decoded value             | Rendered form  | Sentinel constant   |
|-------------------------------------|----------------|---------------------|
| `Int(I8, _)` where decoded == MIN   | `i8::MIN`      | `-128`              |
| `Int(I8, _)` where decoded == MAX   | `i8::MAX`      | `127`               |
| `Int(I16, _)` where decoded == MIN  | `i16::MIN`     | `-32768`            |
| `Int(I16, _)` where decoded == MAX  | `i16::MAX`     | `32767`             |
| `Int(I32, _)` where decoded == MIN  | `i32::MIN`     | `-2147483648`       |
| `Int(I32, _)` where decoded == MAX  | `i32::MAX`     | `2147483647`        |
| `Int(I64, _)` where decoded == MIN  | `i64::MIN`     | `-9223372036854775808` |
| `Int(I64, _)` where decoded == MAX  | `i64::MAX`     | `9223372036854775807` |
| `Int(I128, _)` where decoded == MIN | `i128::MIN`    | `-(2^127)`          |
| `Int(I128, _)` where decoded == MAX | `i128::MAX`    | `2^127 - 1`         |
| `Int(Isize, _)` where decoded == MIN| `isize::MIN`   | target-pointer-width-dependent |
| `Int(Isize, _)` where decoded == MAX| `isize::MAX`   | target-pointer-width-dependent |
| `Uint(U8, _)` where decoded == MAX  | `u8::MAX`      | `255`               |
| `Uint(U16, _)` where decoded == MAX | `u16::MAX`     | `65535`             |
| `Uint(U32, _)` where decoded == MAX | `u32::MAX`     | `4294967295`        |
| `Uint(U64, _)` where decoded == MAX | `u64::MAX`     | `18446744073709551615` |
| `Uint(U128, _)` where decoded == MAX| `u128::MAX`    | `2^128 - 1`         |
| `Uint(Usize, _)` where decoded == MAX| `usize::MAX`  | target-pointer-width-dependent |

Notes:

- **No `_` width suffix on sentinel forms** (e.g.,
  `i32::MAX`, NOT `i32::MAX_i32`). The type-name
  prefix already carries the width information
  unambiguously.
- **No unsigned MIN collapsing.** `Uint(_, 0)` always
  renders as `0_u<width>` decimal (matches rustc;
  `0_u8` is concise enough).
- **`Isize` / `Usize` are target-pointer-width-
  independent at the rendering level.** The symbolic
  form `isize::MIN` / `usize::MAX` is rendered
  regardless of target; the comparison uses the
  target's `int[Isize].size` / `uint[Usize].size` from
  `spec.md` §11 `ScalarLayouts` to determine the
  sentinel constant. Cross-target snapshot stability
  requires the symbolic form, not the per-target
  width-resolved decimal (e.g., `usize::MAX` on a
  32-bit target stays `usize::MAX`, NOT
  `4294967295_usize`).
- **Sentinel form is round-trip safe**: `i32::MAX`,
  `u8::MAX`, etc., are valid Rust source-level
  expressions that evaluate to the same constant.

If the decoded value does not match any sentinel
constant of its declared width, the underscore-decimal
form from D-14-4.2 applies.

**Decimal-rendering precision** for `Int` / `Uint`
(non-sentinel values): the renderer prints the
canonical decimal representation of the value at its
declared width:

- For `Int(k, v)`: re-interpret the low `width(k)` bits
  of `v` as a signed two's-complement integer of that
  width, then print using Rust's `Display` for the
  resulting signed type. The `u128` storage holds the
  sign-extended form per `spec.md` §9.7; decoding is
  `(v as i128) >> (128 - width(k))` for width < 128,
  identity for `I128`, with `Isize` using the
  target-pointer-width column from `spec.md` §11
  `ScalarLayouts`.
- For `Uint(k, v)`: print the low `width(k)` bits of
  `v` as an unsigned decimal. `width(k)` is 8 / 16 /
  32 / 64 / 128 / target-pointer-width for U8 / U16 /
  U32 / U64 / U128 / Usize.

No leading-zero or digit-grouping conventions are
applied at the renderer level (rustc does not, either).
The output is a single decimal numeral followed by the
underscore-suffix.

**Char-rendering precision.** The renderer constructs a
`char` from the `u32` payload (bridge has validated
validity per `spec.md` §9.7) and emits Rust's standard
char-literal Debug form: ASCII printable characters
render literally inside single quotes (`'a'`, `'7'`,
`'?'`); the standard escapes apply (`'\n'`, `'\t'`,
`'\r'`, `'\\'`, `'\''`, `'\"'`, `'\0'`); non-printable /
non-ASCII characters render as Unicode escape
`'\u{XXXX}'` with the minimum-width hex form. This
matches rustc's `pretty_print_const_scalar_int` Char arm
at `rustc_middle/src/ty/print/pretty.rs:1812-1814` which
delegates to `char::Debug`.

**Float-rendering precision (finite values).** The
renderer reconstitutes the IEEE-754 value from the
`u128` bit-pattern at the variant's `FloatKind` width,
then emits the apfloat-equivalent decimal display:

- `Float(F16, bits)`: load the low 16 bits as a
  `binary16` and render the shortest round-trippable
  decimal (e.g., `1.5f16`, `0f16`, `-0f16`).
- `Float(F32, bits)`: load the low 32 bits as a
  `binary32` (e.g., `1.5f32`, `3.14f32`).
- `Float(F64, bits)`: load the low 64 bits as a
  `binary64`.
- `Float(F128, bits)`: load all 128 bits as a
  `binary128`.

Integer-valued finite floats render without a fractional
part (`0f64`, `-1f32`, `42f64`) to match rustc's
apfloat-Display convention. Non-integer finite floats
render with a fractional part (`1.5f32`, `3.14f64`).
The display chooses the shortest round-trippable
decimal per IEEE-754; the renderer does not impose a
fixed precision.

**Float-rendering precision (non-finite values)** per
D-14-4.12 LOCKED. Non-finite IEEE-754 values render in
a rustc-mirrored underscore-broken form
`<repr>_f<width>` where `<repr>` is one of
`inf` / `-inf` / `NaN`:

| Variant + classification               | Rendered form  |
|----------------------------------------|----------------|
| `Float(F16, +inf bit pattern)`         | `inf_f16`      |
| `Float(F16, -inf bit pattern)`         | `-inf_f16`     |
| `Float(F16, any NaN bit pattern)`      | `NaN_f16`      |
| `Float(F32, +inf bit pattern)`         | `inf_f32`      |
| `Float(F32, -inf bit pattern)`         | `-inf_f32`     |
| `Float(F32, any NaN bit pattern)`      | `NaN_f32`      |
| `Float(F64, +inf bit pattern)`         | `inf_f64`      |
| `Float(F64, -inf bit pattern)`         | `-inf_f64`     |
| `Float(F64, any NaN bit pattern)`      | `NaN_f64`      |
| `Float(F128, +inf bit pattern)`        | `inf_f128`     |
| `Float(F128, -inf bit pattern)`        | `-inf_f128`    |
| `Float(F128, any NaN bit pattern)`     | `NaN_f128`     |

Classification rule:

- The renderer reconstitutes the IEEE-754 value at the
  variant's `FloatKind` width per the finite-precision
  rule above, then invokes the apfloat classification:
  - `is_finite() == true` (includes both positive zero
    `0f<width>` and negative zero `-0f<width>`) ->
    finite form per D-14-4.2 (no underscore).
  - `is_finite() == false` && positive infinity ->
    `inf_f<width>`.
  - `is_finite() == false` && negative infinity ->
    `-inf_f<width>`.
  - NaN (regardless of quiet / signaling / payload /
    sign bit) -> `NaN_f<width>`.

Notes:

- **The underscore is load-bearing for non-finite
  values only.** `inff32` would parse as a single
  identifier under Rust lexer rules, whereas
  `inf_f<width>` reads unambiguously as "non-finite
  floating-point value of type `f<width>`" to a human
  reader. Finite values do NOT use the underscore
  (D-14-4.2 form): `1.5f32`, `0f64`, `-0f32`, `42f64`
  are parseable Rust float literals.
- **NaN bit-pattern distinction is out of v1 scope.**
  All NaN bit-pattern variations (quiet, signaling,
  payload-bearing, sign-bit-set) render as the single
  `NaN_f<width>` form. The `Const::Float(_, u128)`
  storage preserves the bit pattern; only the
  *rendered* form collapses. Matches rustc's MIR
  pretty-printer behaviour. A future revision can add
  a NaN-bit-pattern rendering option to
  `PrettyPrintOptions` parallel to D-14-4.11's
  hex-admission future-extension boundary (not
  pre-allocated per README §5.9 rule 4).
- **Negative zero is finite, not non-finite.**
  `Float(F32, 0x80000000)` renders as `-0f32` per the
  finite-precision rule (D-14-4.2 territory); the
  underscore form does NOT apply.

#### 10.1.2 `ZST`

Per D-14-4.3 LOCKED. `Const::ZST` is the canonical
single representative for all ZST values per
`spec.md` §9.7. The renderer dispatches on the
consumer `Ty.kind()` across 7 categories of ZST
consumer Ty. For composite-ZST cases (tuples / arrays
/ structs / enum variants whose fields are themselves
ZST), the renderer recurses on the SAME `ConstId`
(the unique `Const::ZST` representative) with the
per-sub-field `Ty` as the new consumer-context Ty.
The recursion is what produces the rich per-Ty value
form at the rendering surface; the storage shape
remains the §9.7 canonical-form single
representative.

**Dispatch table** (consumer `Ty.kind()` -> rendered
form):

| Consumer `Ty.kind()`                                              | Rendered form                                                                  | Notes                                                                                                                                                                |
|-------------------------------------------------------------------|--------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `Ty::Tuple { fields: [] }`                                        | `()`                                                                           | Unit type. Common case.                                                                                                                                              |
| `Ty::Tuple { fields: [T1] }` (single-field ZST tuple)             | `(<r(zst, T1)>,)`                                                              | Trailing comma per Rust syntax for 1-tuples. Composite-ZST recursion.                                                                                                |
| `Ty::Tuple { fields: [T1, T2, ..., Tn] }` (n >= 2 ZST tuple)      | `(<r(zst, T1)>, <r(zst, T2)>, ..., <r(zst, Tn)>)`                              | Composite-ZST recursion per sub-field Ty.                                                                                                                            |
| `Ty::Array(_, 0)`                                                 | `[]`                                                                           | Empty array. (Note: stage-3 / stage-10 admit this Ty shape; the bridge produces `Const::ZST` for any 0-length array regardless of element type.)                     |
| `Ty::Array(elem, N)` where `N > 0` and array is ZST               | `[<r(zst, elem)>; N]`                                                          | Rust repeat syntax. Composite-ZST recursion on element Ty. `N` renders as a decimal usize literal (matches `Ty::Array` type-rendering form at §6.2).                |
| `Ty::Struct { item, args, layout: { kind: Struct { fields: [] }, .. } }`  | `<render_def_path(item)><render_generic_args(args, item)>`           | Fieldless struct. Includes `core::marker::PhantomData<T>`, unit structs like `struct Foo;`, and any other fieldless struct.                                          |
| `Ty::Struct { item, args, layout: { kind: Struct { fields }, .. } }` (`fields.len() > 0`, all `fields[i].ty` ZST, names tuple-synthetic `_<index>`)   | `<render_def_path(item)><render_generic_args(args, item)>(<r(zst, fields[0].ty)>, <r(zst, fields[1].ty)>, ...)` | Tuple-style struct with all-ZST fields. Tuple-vs-named distinction per §3.4.                                                                                         |
| `Ty::Struct { item, args, layout: { kind: Struct { fields }, .. } }` (`fields.len() > 0`, all ZST, named fields)   | `<render_def_path(item)><render_generic_args(args, item)> { <fields[0].name>: <r(zst, fields[0].ty)>, ... }`   | Named-field struct with all-ZST fields.                                                                                                                              |
| `Ty::Enum { item, args, layout: { kind: Enum { shape: EnumShape::Single { variant: V, .. } }, .. } }` (`V.fields.is_empty()`)  | `<enum_path>::<V.name>` where `enum_path = <render_def_path(item)><render_generic_args(args, item)>` | Single-variant unit-variant enum (`enum E { V }`). Variant name lookup via `EnumShape::Single.variant.name` per §3.4.                                                |
| `Ty::Enum { ... EnumShape::Single { variant: V, .. } }` (`V.fields` non-empty, tuple-synthetic names, all ZST)  | `<enum_path>::<V.name>(<r(zst, V.fields[0].ty)>, ...)`                         | Single-variant tuple-variant enum with all-ZST fields.                                                                                                               |
| `Ty::Enum { ... EnumShape::Single { variant: V, .. } }` (`V.fields` non-empty, named, all ZST)  | `<enum_path>::<V.name> { <V.fields[0].name>: <r(zst, V.fields[0].ty)>, ... }`  | Single-variant named-field-variant enum with all-ZST fields.                                                                                                         |
| `Ty::Enum { ... EnumShape::Multiple { .. } }`                     | Cannot occur for ZST                                                           | Multi-variant enums carry a discriminant tag of nonzero size; cannot be ZST. Renderer panics (assertion).                                                            |
| `Ty::Enum { ... EnumShape::Empty }`                               | Cannot occur for ZST                                                           | Zero-variant uninhabited enum has no values. Renderer panics.                                                                                                        |
| `Ty::Union { item, args, .. }`                                    | `<render_def_path(item)><render_generic_args(args, item)>`                           | Union (with single-ZST-field; rare). Field-disambiguation is NOT preserved at the value level for unions; rendering produces just the path.                          |
| `Ty::FnDef(item, args)`                                           | `<render_def_path(item)><render_generic_args(args, item)>`                           | Value-namespace path with generic args. **Mirrors rustc's only special-case at `mir/pretty.rs:1976-1982`.** Distinct from the type-rendering form at §6.4 (which adds `fn(<arg_tys>) -> <ret_ty> {...}` wrapping). |
| `Ty::Closure(item, args)` (non-capturing)                         | `<render_def_path(item)><render_generic_args(args, item)>`                           | The synthesized closure name (`parent::module::{closure#N}`) per stage-12 / stage-13 `render_def_path` cache.                                                          |
| `Ty::Never`                                                       | Cannot occur (uninhabited)                                                     | `!` has no values. Renderer panics.                                                                                                                                  |
| Any non-ZST `Ty.kind()` (Bool / Int / Uint / Float / Char / Ref / RawPtr / Dynamic / etc.) | Cannot occur                                                                   | Non-ZST types cannot consume `Const::ZST`: the bridge produces a non-ZST Const variant. Renderer panics (bridge bug).                                                |

**Recursion shape**: `r(zst, T)` denotes the recursive
call `self.render_const(zst_const_id, T)` where
`zst_const_id` is the SAME `ConstId` representing the
unique `Const::ZST` canonical representative. Per
`spec.md` §9.7, there is exactly one `Const::ZST` in
`model.consts` (canonical-form rule); the recursion
uses this single ID for every composite-ZST
sub-field. Termination is guaranteed because each
recursive step descends into a structurally smaller
consumer Ty and eventually bottoms out at a leaf ZST
Ty (`Ty::Tuple([])`, fieldless `Ty::Struct`,
`Ty::FnDef`, `Ty::Closure`, etc.).

**Tuple-vs-named-field dispatch** for `Ty::Struct` and
`Ty::Enum` variant fields: follows the §3.4 rule.
Field names matching the synthetic pattern `_<index>`
(`_0`, `_1`, ...) signal tuple-style; non-synthetic
names signal named-field-style. Empty-fields layout
(`fields.len() == 0`) signals unit-style.

**Composite-ZST validity.** The bridge produces
`Const::ZST` only for `ConstValue::ZeroSized` per
`spec.md` §9.7's dispatch table. The bridge does NOT
decompose a composite ZST (e.g., `[(); 100]` or
`((), (), ())`) into an `Aggregate(...)`; per §9.7
the canonical form is `Const::ZST`. The renderer's
recursion produces the rich per-Ty value form at the
rendering surface, not at the storage level.

#### 10.1.3 `Aggregate(Vec<ConstId>)`

Per D-14-4.4 LOCKED. `Const::Aggregate(children)`
dispatches on consumer `Ty.kind()` across three
structural categories (Array / Tuple / Struct) plus
two fat-pointer special cases (`&str` and `&[u8]`).
Children render via recursive `render_const(c_i,
per_field_ty_i)` per D-14-4.9's per-sub-field Ty
resolution (§10.1.8).

| Consumer `Ty.kind()`                                                                                                                  | Rendered form                                                                                                                  | Notes                                                                                                                                                                                                          |
|---------------------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `Ty::Array(elem, N)`                                                                                                                  | `[<r(c_0, elem)>, <r(c_1, elem)>, ..., <r(c_{N-1}, elem)>]`                                                                    | Children count == N (bridge-validated).                                                                                                                                                                        |
| `Ty::Tuple { fields: [T_0] }`                                                                                                         | `(<r(c_0, T_0)>,)`                                                                                                             | 1-tuple trailing comma.                                                                                                                                                                                        |
| `Ty::Tuple { fields: [T_0, T_1, ..., T_{n-1}] }` (n >= 2)                                                                             | `(<r(c_0, T_0)>, <r(c_1, T_1)>, ..., <r(c_{n-1}, T_{n-1})>)`                                                                   |                                                                                                                                                                                                                |
| `Ty::Struct { item, args, layout: Struct { fields: [] } }`                                                                            | `<render_def_path(item)><render_generic_args(args, item)>`                                                                           | Fieldless. Bridge usually canonicalises to `Const::ZST`; if Aggregate arrives renders identically.                                                                                                              |
| `Ty::Struct { item, args, layout: Struct { fields } }` (tuple-synthetic `_<index>` names per §3.4)                                    | `<render_def_path(item)><render_generic_args(args, item)>(<r(c_0, fields[0].ty)>, ...)`                                              | Tuple-style struct.                                                                                                                                                                                            |
| `Ty::Struct { item, args, layout: Struct { fields } }` (named-field names)                                                            | `<render_def_path(item)><render_generic_args(args, item)> { fields[0].name: <r(c_0, fields[0].ty)>, ... }`                          | Named-field struct.                                                                                                                                                                                            |
| `Ty::Ref(_, Ty::Str, _)` AND fat-pointer pre-conditions hold (see below)                                                              | `"<utf8-lossy decoded>"`                                                                                                       | rustc-mirrored `&str` special case per `mir/pretty.rs:1874-1879`.                                                                                                                                              |
| `Ty::Ref(_, Ty::Slice(<u8>), _)` AND fat-pointer pre-conditions hold                                                                  | `b"<escape-ascii decoded>"`                                                                                                    | rustc-mirrored `&[u8]` special case per `mir/pretty.rs:1880-1888`.                                                                                                                                             |
| Other `Ty::Ref(_, Ty::Slice(<non-u8>), _)` / `Ty::Ref(_, Ty::Dynamic(_), _)` / fat-pointer pre-conditions fail                       | `(<r(c_0, *const inner)>, <r(c_1, usize)>) as <render_ty(consumer)>` -- per D-14-4.9 child-Ty resolution via bridge-interned TyIds | Structural fallback for non-special fat pointers. Per D-14-4.9 bridge interning invariant: `Ty::Ptr(inner, Mutability::Not)` and `Ty::Uint(UintKind::Usize)` are bridge-interned in `model.types` when the bridge produces `Aggregate([Reloc, Uint])` for fat-pointer consumer Tys. |
| `Ty::Enum { .. }` or `Ty::Union { .. }`                                                                                               | Cannot occur                                                                                                                   | Enum aggregates use `Const::EnumValue` (§10.1.4); union aggregates lack field disambiguation. Renderer panics.                                                                                                  |
| Any other `Ty.kind()` (scalars / FnDef / Closure / thin pointers / etc.)                                                              | Cannot occur                                                                                                                   | Renderer panics (bridge bug).                                                                                                                                                                                  |

**Fat-pointer special-case pre-conditions** (for both
`&str` and `&[u8]` arms):

1. Aggregate has exactly 2 children.
2. `children[0]` resolves to `Const::Reloc(InstanceRef)`.
3. The InstanceRef resolves to a Static Instance with
   `item_synthetic_kinds[item] == SyntheticItemKind::ConstMemoryStatic { .. }`.
4. The Static's `init` resolves to
   `Const::Bytes(vec)` of all `Byte::Value(b)` bytes
   (no Uninit / Reloc).
5. `children[1]` resolves to
   `Const::Uint(UintKind::Usize, len)` where
   `len == vec.len() as u128`.

For `&[u8]`: byte sequence renders as
`b"<escape-ascii>"`. For `&str`: byte sequence
renders as `"<utf8-lossy decoded>"` (invalid bytes
become `U+FFFD` replacement character).

If any pre-condition fails, structural fallback
applies via D-14-4.9.

**Children count invariant** (bridge-validated):

- `Ty::Array(_, N)`: children count == N.
- `Ty::Tuple(fields)`: children count == fields.len().
- `Ty::Struct(layout)`: children count ==
  layout.kind.as_struct().fields.len().
- Fat-pointer Ref cases: children count == 2.

**Tuple-vs-named-field dispatch** follows §3.4 (same
rule as D-14-4.3): synthetic `_<index>` field names
signal tuple-style; non-synthetic names signal
named-field-style; empty fields signal unit-style.

#### 10.1.4 `EnumValue { variant_idx, payload }`

Per D-14-4.5 LOCKED. `Const::EnumValue { variant_idx,
payload }` dispatches on the consumer `Ty::Enum`'s
`EnumShape` to locate the named variant, then on the
variant's `fields` to determine the ctor form (unit /
tuple / named-field). Payload children render via
`render_const(p_i, fields[i].ty)` per D-14-4.9.

| Consumer Ty + variant shape                                                                                                                                                   | Rendered form                                                                                                                  | Notes                                                                                                                                |
|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------|
| `Ty::Enum { ... EnumShape::Single { variant: V, .. } }` AND `V.fields.is_empty()` AND `payload.is_empty()`                                                                    | `<enum_path>::<V.name>`                                                                                                        | `enum_path = <render_def_path(item)><render_generic_args(args, item)>`.                                                                    |
| `Ty::Enum { ... EnumShape::Single { variant: V, .. } }` AND `V.fields` non-empty (tuple-synthetic names)                                                                      | `<enum_path>::<V.name>(<r(p_0, V.fields[0].ty)>, ...)`                                                                         | Single-variant tuple-variant.                                                                                                        |
| `Ty::Enum { ... EnumShape::Single { variant: V, .. } }` AND `V.fields` non-empty (named names)                                                                                | `<enum_path>::<V.name> { V.fields[0].name: <r(p_0, V.fields[0].ty)>, ... }`                                                    | Single-variant named-field-variant.                                                                                                  |
| `Ty::Enum { ... EnumShape::Multiple { variants, .. } }` AND `V = variants[variant_idx]` AND `V.fields.is_empty()` AND `payload.is_empty()`                                    | `<enum_path>::<V.name>`                                                                                                        | Multi-variant enum, unit variant (e.g., `Option::None`).                                                                             |
| `Ty::Enum { ... EnumShape::Multiple { variants, .. } }` AND `V = variants[variant_idx]` AND `V.fields` non-empty (tuple-synthetic names)                                      | `<enum_path>::<V.name>(<r(p_0, V.fields[0].ty)>, ...)`                                                                         | Multi-variant enum, tuple variant (e.g., `Option::Some(42_i32)`).                                                                    |
| `Ty::Enum { ... EnumShape::Multiple { variants, .. } }` AND `V = variants[variant_idx]` AND `V.fields` non-empty (named names)                                                | `<enum_path>::<V.name> { V.fields[0].name: <r(p_0, V.fields[0].ty)>, ... }`                                                    | Multi-variant enum, named-field variant (e.g., `Result::Err { reason: "oops" }`).                                                    |
| `Ty::Enum { ... EnumShape::Empty }`                                                                                                                                           | Cannot occur                                                                                                                   | Zero-variant uninhabited enum has no values. Renderer panics.                                                                        |
| `variant_idx` out of bounds for the consumer's `EnumShape.variants`                                                                                                           | Cannot occur                                                                                                                   | Bridge-validated per `spec.md` §3.3 Phase B step 9. Renderer panics.                                                                 |
| `payload.len()` != `V.fields.len()`                                                                                                                                           | Cannot occur                                                                                                                   | Bridge-validated. Renderer panics.                                                                                                   |
| Any non-`Ty::Enum` consumer Ty                                                                                                                                                | Cannot occur                                                                                                                   | `Const::EnumValue` is produced only for enum consumer Tys. Renderer panics (bridge bug).                                             |

**Variant name lookup** per §3.4:
- `EnumShape::Single { variant: V, .. }` -> `V.name`
  (a `StringId`).
- `EnumShape::Multiple { variants, .. }` ->
  `variants[variant_idx].name`.

`VariantLayout.name` is non-optional per `spec.md`
§11; missing names are bridge bugs.

**Tuple-vs-named-field dispatch** follows §3.4 (same
rule as D-14-4.3 and D-14-4.4): synthetic `_<index>`
names signal tuple-style; non-synthetic names signal
named-field-style; empty fields signal unit-style.

#### 10.1.5 `Bytes(Vec<Byte>)`

Per D-14-4.6 LOCKED. `Const::Bytes(vec)` dispatches
on consumer `Ty.kind()` with an all-uninit shortcut +
a byte-array case + a structural fallback.

| Bytes payload + consumer Ty                                                                                                                                                                                    | Rendered form                                                                                                                                  | Notes                                                                                                                                                                                          |
|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| All bytes are `Byte::Uninit`                                                                                                                                                                                   | `<uninit>`                                                                                                                                     | Rustc-mirrored shortcut per `mir/pretty.rs:1864-1869`. Applies for any consumer Ty.                                                                                                            |
| All-`Byte::Value(b)` AND consumer `Ty == Ty::Array(<u8>, N)` AND `vec.len() == N`                                                                                                                              | `*b"<escape-ascii>"`                                                                                                                           | Rustc-mirrored deref byte-array form per `mir/pretty.rs:1889-1898`. Deref-prefix `*` distinguishes from `&[u8]` form (`b"..."`).                                                                |
| Other consumer Tys (composite structs / tuples / arrays / unions / non-byte-array) with mixed Uninit / Value / Reloc bytes                                                                                     | `bytes![<byte_0>, <byte_1>, ..., <byte_{n-1}>]`                                                                                                | RegseqModel-richer structural fallback (rustc falls back to `{ct:?}: {ty}` debug). Per-byte forms: `0xHH` lowercase 2-digit hex for `Value(b)`; `<uninit>` for `Uninit`; `<reloc: <render_def_path(resolved_item)>>` for `Reloc(_)` (one token per maximal relocation run per §9.7 well-formedness). |
| `Const::Bytes` consumed at `Ty::Ref(_, Ty::Str, _)`, `Ty::Ref(_, Ty::Slice(_), _)`                                                                                                                             | Cannot occur                                                                                                                                   | Bridge produces `Aggregate([Reloc, Uint])` per `spec.md` §9.7's `Slice { data, meta }` dispatch row. Renderer panics.                                                                          |
| `Const::Bytes` consumed at scalar Ty (`Ty::Bool` / `Ty::Int(_)` / `Ty::Uint(_)` / `Ty::Float(_)` / `Ty::Char`)                                                                                                  | Cannot occur                                                                                                                                   | Bridge produces the scalar variant per `spec.md` §9.7's scalar dispatch rows. Renderer panics.                                                                                                  |

**All-uninit shortcut precedence**: the all-uninit
check is evaluated FIRST, before the byte-array case.
`Const::Bytes([Uninit; N])` consumed at `Ty::Array(<u8>, N)`
renders as `<uninit>`, not `*b"\xc0\xc0..."`. Matches
rustc's behaviour at `mir/pretty.rs:1864-1869`.

**Structural fallback `bytes![...]` form**:

- `Byte::Value(b)`: `0xHH` lowercase 2-digit hex.
- `Byte::Uninit`: `<uninit>`.
- `Byte::Reloc(instance_ref)`: `<reloc: <render_def_path(resolved_item)>>`
  with one token per maximal relocation run (per §9.7
  well-formedness: relocation runs are pointer-shaped
  with `ptr_size` consecutive `Reloc(r)` bytes for one
  pointer).

Byte ordering renders in `Vec<Byte>` order (target-
endian as the bridge captured from `Allocation.bytes`).
The renderer does NOT re-order or re-interpret per
consumer-Ty endianness.

#### 10.1.6 `NullPtr(TyId)`

Per D-14-4.7 LOCKED. `Const::NullPtr(pointee_ty)`
renders in the rustc-mirrored typed-cast form:

```text
0x0 as <render_ty(consumer_ty)>
```

where `consumer_ty` is the consumer-context pointer
Ty from D-14-4.1's pair-form (the pointer type at the
use site). The variant payload `pointee_ty` is for
canonical-equality only per `spec.md` §9.7
("`NullPtr(t1) != NullPtr(t2)` whenever `t1 != t2`");
the renderer uses the consumer Ty's full pointer-type
rendering (which carries mutability + borrow-flavor +
FnPtr signature info that the pointee-only payload
does not).

**Admissible consumer Ty kinds** (thin pointers only;
the bridge produces `Const::NullPtr` only for
single-scalar pointer layouts per `spec.md` §9.7
dispatch table):

| Consumer `Ty.kind()`                            | Rendered form                                                | Notes                                                                                       |
|-------------------------------------------------|--------------------------------------------------------------|---------------------------------------------------------------------------------------------|
| `Ty::Ref(T, Provenance::SharedBorrow)`          | `0x0 as &<render_ty(T)>`                                     | Thin shared reference (pointee `T` is sized).                                               |
| `Ty::Ref(T, Provenance::UniqueBorrow)`          | `0x0 as &mut <render_ty(T)>`                                 | Thin unique reference.                                                                      |
| `Ty::Ptr(T, Mutability::Not)`                   | `0x0 as *const <render_ty(T)>`                               | Thin raw const pointer.                                                                     |
| `Ty::Ptr(T, Mutability::Mut)`                   | `0x0 as *mut <render_ty(T)>`                                 | Thin raw mut pointer.                                                                       |
| `Ty::FnPtr(sig_id)`                             | `0x0 as <render_ty(sig)>` (yielding e.g. `fn(u32) -> bool`) | Function pointer type per §6.5. Function pointers are thin (one scalar).                    |
| Any non-thin / non-pointer Ty                   | Cannot occur                                                 | Fat pointers (`&dyn Trait`, `&[T]`, `&str`, raw fat pointers) are 2-scalar; the bridge would produce `Aggregate([NullPtr, NullPtr])` or similar, not a single `NullPtr`. Other consumer Tys are bridge bugs. Renderer panics. |

The renderer delegates the cast-type form to
`render_ty(consumer_ty)` per stage-13 D-13-5; the
form is `&<inner>` / `&mut <inner>` / `*const
<inner>` / `*mut <inner>` / `fn(<args>) -> <ret>`
per §6.2 / §6.5.

**Consistency invariant** (bridge-guaranteed; not
re-validated at render time): for an `Operand` /
position holding a `Const::NullPtr(pointee_ty)`, the
consumer Ty is a thin pointer type whose pointee
equals `pointee_ty`. The bridge validates this at
construction per `spec.md` §3.3 Phase B step 9
("`NullPtr` pointee resolves"); the renderer trusts
the invariant and uses the consumer Ty for rendering
(richer than the bare pointee).

#### 10.1.7 `Reloc(InstanceRef)`

Per D-14-4.8 LOCKED. `Const::Reloc(instance_ref)`
dispatches on the resolved Instance's `InstanceKind`
and synthetic-Item classification:

1. Resolves `instance_ref` to `InstanceId` (F4-2;
   bridge-validated).
2. Looks up `instance = model.instances[InstanceId]`.
3. For Static instances, looks up
   `item_synthetic_kinds[instance.item]` to
   distinguish source-level statics from synth Items.
4. Dispatches on (`instance.kind`, consumer Ty):

| Resolved instance.kind / synth classification                                                   | Consumer `Ty.kind()`                                       | Rendered form                                                                                              |
|-------------------------------------------------------------------------------------------------|------------------------------------------------------------|------------------------------------------------------------------------------------------------------------|
| `InstanceKind::Function { .. }`                                                                 | `Ty::FnDef(item, args)` matching `(instance.item, instance.generic_args)` | `<render_def_path(instance.item)><render_generic_args(instance.generic_args, instance.item)>` (no cast)                   |
| `InstanceKind::Function { .. }`                                                                 | `Ty::FnPtr(sig_id)`                                        | `<render_def_path(instance.item)><render_generic_args(instance.generic_args, instance.item)> as <render_ty(consumer)>`    |
| `InstanceKind::Static { .. }`, `item_synthetic_kinds[item] = None` (source-defined)             | Any `Ty::Ref(_, _, _)` or `Ty::Ptr(_, _)`                  | `<render_def_path(instance.item)> as <render_ty(consumer)>`                                                |
| `InstanceKind::Static { .. }`, `item_synthetic_kinds[item] = VTableInstance { .. }`             | Any pointer consumer Ty                                    | `<render_def_path(instance.item)> as <render_ty(consumer)>` (synth path per §13.4 vtable-family)           |
| `InstanceKind::Static { .. }`, `item_synthetic_kinds[item] = ConstMemoryStatic { .. }`          | `Ty::Ref(_, Ty::Array(<u8>, N), _)` AND b"..." pre-conditions hold (see below) | `b"<escape-ascii of init bytes>"` (rustc-mirrored per `ty/print/pretty.rs:1727-1753`)                      |
| `InstanceKind::Static { .. }`, `item_synthetic_kinds[item] = ConstMemoryStatic { .. }`          | Other thin-pointer consumer Tys OR b"..." pre-conditions fail | `<render_def_path(instance.item)> as <render_ty(consumer)>` (structural fallback)                          |
| `InstanceKind::Const / Struct / Enum / Union`                                                   | Any consumer Ty                                            | Cannot occur. Renderer panics.                                                                             |
| `InstanceKind::Function { .. }`                                                                 | Non-FnDef / non-FnPtr consumer Ty                          | Cannot occur. Renderer panics.                                                                             |
| `InstanceKind::Static { .. }`                                                                   | Non-pointer consumer Ty                                    | Cannot occur. Renderer panics.                                                                             |

**`b"..."` byte-string pre-conditions** (RegseqModel
matches rustc's only structural special case for
Reloc):

1. Consumer Ty is `Ty::Ref(_, Ty::Array(u8_ty, N), _)`
   where `u8_ty` resolves to `Ty::Uint(UintKind::U8)`.
2. `instance_ref` resolves to an Instance with
   `InstanceKind::Static { ty: byte_array_ty, init: Some(bytes_const_id) }`.
3. `item_synthetic_kinds[instance.item] ==
   SyntheticItemKind::ConstMemoryStatic { .. }`.
4. `model.consts[bytes_const_id] == Const::Bytes(vec)`.
5. Every byte in `vec` is `Byte::Value(b)` (no
   `Byte::Uninit` or `Byte::Reloc(_)`).
6. `vec.len() == N`.

If all hold, render as `b"<byte_string.escape_ascii()>"`
(per `mir/pretty.rs:1831-1833`'s
`pretty_print_byte_str` helper). If any fails,
structural fallback applies.

**FnDef-vs-FnPtr dispatch.** The bridge produces
`Const::Reloc(fn_ref)` for both direct function-value
references (consumer Ty `Ty::FnDef(item, args)`; no
coercion) and function-to-FnPtr coercion sites
(consumer Ty `Ty::FnPtr(sig)`). The renderer
distinguishes by consumer Ty: bare path for FnDef,
typed-cast for FnPtr. The FnDef rendering form
matches D-14-4.3's ZST-over-FnDef form by design
(bridge may produce either `Const::ZST` or
`Const::Reloc` for function-value references; both
render identically at the use site).

#### 10.1.8 Type-context threading (recursion + operand wrapper)

Per D-14-4.9 LOCKED. Specifies (a) the per-sub-field
Ty resolution mechanism for recursive children, and
(b) the operand-callsite wiring contract.

**Per-sub-field Ty resolution** (recursion shape).
The renderer walks the parent consumer Ty's
structural layout to extract per-child Tys. Resolution
rules:

| Parent consumer Ty                                                                                | Per-child Ty resolution                                                                                                |
|---------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------|
| `Ty::Array(elem, N)`                                                                              | Every child has Ty `elem`.                                                                                             |
| `Ty::Tuple { fields }`                                                                            | The i-th child has Ty `fields[i]`.                                                                                     |
| `Ty::Struct { layout: { kind: Struct { fields }, .. } }`                                          | The i-th child has Ty `fields[i].ty`.                                                                                  |
| `Ty::Enum { layout: { kind: Enum { shape: EnumShape::Single { variant: V, .. } }, .. } }`         | The i-th payload child has Ty `V.fields[i].ty`.                                                                        |
| `Ty::Enum { layout: { kind: Enum { shape: EnumShape::Multiple { variants, .. } }, .. } }`         | For the variant at `variant_idx`, the i-th payload child has Ty `variants[variant_idx].fields[i].ty`.                  |
| `Ty::Ref(_, pointee, _)` (fat-pointer fallback per §10.1.3 / D-14-4.4)                            | Per-child Tys are `interned` TyIds resolved via the renderer's `TyInterner` lookup: data-pointer child Ty = the interned `Ty::Ptr(pointee, Mutability::Not)`; metadata child Ty = the interned `Ty::Uint(UintKind::Usize)`. **Bridge interning invariant**: when the bridge produces a `Const::Aggregate([Reloc, Uint])` value for a fat-pointer consumer Ty per `spec.md` §9.7's `Slice { data, meta }` dispatch row OR `Ref(_, dyn Trait)` synthesis paths, it MUST ensure both `Ty::Ptr(pointee, Mutability::Not)` and `Ty::Uint(UintKind::Usize)` are present in `model.types` (interning them if absent). This invariant lives at `spec.md` §9.7's bridge-side validation; the renderer trusts the invariant and looks up the TyIds at render time without re-interning. |

Resolution is structural-data-driven: the
`PrettyPrinter` has `&model` and walks layouts
internally; no caller-side resolution is required.
The fat-pointer fallback case threads through the
TyInterner for both child types, preserving the
"all TyIds are interned" closed-system invariant
(`spec.md` §10's closed-`Ty`-enum rule); the
renderer does NOT construct non-interned TyIds.

**Operand-callsite wiring** (stage-13 D-13-6 REOPEN
with three-signature cascade per R1 finding). Three
stage-13 signatures cascade in the REOPEN:

```rust
// BEFORE (stage-13 D-13-6):
fn render_operand(&self, op: &Operand, body: &Body) -> String;
fn render_rvalue(&self, rv: &Rvalue, body: &Body) -> String;
fn render_generic_args(&self, args: &GenericArgs) -> String;

// AFTER (D-14-4.9):
fn render_operand(&self, op: &Operand, body: &Body, expected_ty: TyId) -> String;
fn render_rvalue(&self, rv: &Rvalue, body: &Body, expected_ty: TyId) -> String;
fn render_generic_args(&self, args: &GenericArgs, item: ItemId) -> String;
```

`render_rvalue` is extended because
`Rvalue::Use(operand)` needs to propagate the
consumer Ty to its operand (the existing two-arg
signature provides no source for that type).
`render_generic_args` is extended because
`GenericArg::Const(c)` needs the enclosing Item's
generic-parameter list to resolve the declared type
per D-14-4.1's pair-form.

`expected_ty` is the consumer Ty at the operand's /
Rvalue's use site. Caller-side resolution rules:

| Caller                                                  | `expected_ty` resolution                                                                                                                  |
|---------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------|
| `render_statement` Assign(place, rvalue)                | Threads `place_ty(place, body)` to `render_rvalue(rvalue, body, expected_ty)`. Other StatementKind operand callsites pass explicit Tys (Assume cond `Ty::Bool`; CopyNonOverlapping src/dst/count pointer + usize Tys). |
| `render_rvalue` (per-Rvalue-kind dispatch)              | Per `Rvalue::Use(_)`: operand expected_ty = `expected_ty` (transparent). Per `Rvalue::Repeat(op, _)`: operand expected_ty = element type. Per `Rvalue::BinaryOp / UnaryOp / Cast / Aggregate(kind, ops)`: derived per §7.3 derivation column. |
| `render_terminator_kind` Call args                      | Each Call arg's expected_ty = `callee_fn_sig.inputs[i]` (resolved via `instance.fn_sig` for `Direct(id)`; via `FnSigInterner` for `Indirect(_)`).                                                                            |
| `render_call_callee` Indirect callees                   | Indirect operand expected_ty = `Ty::FnPtr(sig_id)`.                                                                                                                                                                       |
| `render_assert_kind` message operands                   | Per AssertKind variant: `BoundsCheck { len, index }` -> both `Ty::Uint(Usize)`; `Overflow / OverflowNeg / DivisionByZero / RemainderByZero` -> matches BinOp input type; `MisalignedPointerDereference { required, found }` -> both `Ty::Uint(Usize)`. |

The renderer's per-arm dispatch for `Operand::Copy /
Move(place)` ignores `expected_ty` (the place's type
is already resolvable via `place_ty(place, body)`);
only `Operand::Constant(id)` consumes it (threading
to `render_const(id, expected_ty)` per D-14-4.1's
pair-form). `expected_ty` is uniformly required by
the signature for API symmetry.

**§6.4 / §6.7 / §7.2 / §7.3 / §7.4 / §7.5 / §7.6 /
§7.7 / §7.9 caller-side specs** updated to reflect
the three signature extensions: §7.2's per-arm
table's `Constant(id)` row renders via
`<render_const(id, expected_ty)>`; §7.3 +
§7.3.2 gain per-operand expected_ty derivation
columns; §7.4 Assign threads place_ty to
`render_rvalue`; §6.4 + §10.1.* ADT callsites pass
the adjacent `item` to `render_generic_args`; §6.7
helper docs updated. §8 ownership table's
`render_operand`, `render_rvalue`, and
`render_generic_args` rows all update to the new
signatures.

**Stage-13 D-13-6 REOPEN.** Flip in `decisions.md` at
merger close with annotation "(revised in stage 14
D-14-4.9: `render_operand` and `render_rvalue`
signatures extended with `expected_ty: TyId`;
`render_generic_args` signature extended with
`item: ItemId`)". Stage-13 D-13-6's deliberate
divergence from rustc's `Display for ConstOperand`
(no `const ` prefix at the operand layer) is
preserved.

### 10.2 `render_instance(InstanceId) -> String`

Composer renderer for the closed 6-variant
`InstanceKind` enum (`spec.md` §9.4). Per D-14-5.1
(LOCKED), the API takes a single `InstanceId`:

```rust
impl PrettyPrinter<'_> {
    pub fn render_instance(&self, id: InstanceId) -> String;
}
```

The `PrettyPrinter` has `&model` and resolves the
`InstanceId` internally to `&self.model.instances[id]`.
No consumer-context type is required at the call site:
`InstanceKind` already embeds the per-variant rich
data the renderer needs (per stage-12 D-12-1.4 +
stage-14 §9.4):

- `Function { fn_sig, body, flavor, virtual_idx }` --
  `fn_sig` (via `FnSigId`) supplies args + ret +
  safety + abi; `body` supplies the locals + basic
  blocks (or `None` for foreign declarations).
- `Static { ty, init }` -- `ty` is the type for the
  `: <render_ty(ty)>` decoration; `init: Option<ConstId>`
  is threaded through `render_const(init.unwrap(), ty)`
  per D-14-4.1 pair-form, or omitted (foreign extern
  static) when `init = None`.
- `Const { ty, value }` -- same shape as Static;
  `value: ConstId` is always populated per stage-12
  D-12-2 (no None branch).
- `Struct { layout }` / `Enum { layout }` /
  `Union { layout }` -- `layout: TyId` resolves to
  `Ty::Struct/Enum/Union { layout: Layout, args:
  GenericArgs, .. }` (§9.4 + §11) which carries the
  field / variant table for the per-variant body
  rendering.

**Dispatch overview.** Single `match` on `inst.kind`
producing a self-contained `String` covering the
Instance's full rendering (attribute block + kind
keyword + path + per-variant body / value / decl
trailer). The match arms cover all 6 variants per the
closed `InstanceKind` enum; per-variant rendering
rules are pinned by D-14-5.2 (Function) / D-14-5.3
(Static) / D-14-5.4 (Const) / D-14-5.5 (Struct + Enum +
Union ADT variants) in subsequent sub-decision locks.
The attribute-block composition rule (how the
per-flag Item fields enumerated by the §5.3 table
in its locked row order interleave with
`domain_attrs.item_attrs[item]` entries per `spec.md`
§3.6.1) is pinned at §10.2.7 per D-14-5.6 LOCKED.
See §5.3 + §5.6 for the authoritative Stream 1
field enumeration and walk order.
(`is_unsafe == true` is surfaced as the `unsafe`
keyword prefix on `fn`, not as an attribute -- see
§5.3 / §10.2.1 aspect 2.)

**`flavor` / `virtual_idx` are identity metadata, not
rendering inputs.** `InstanceKind::Function.flavor:
FunctionInstanceFlavor` and
`InstanceKind::Function.virtual_idx: Option<u32>` are
disambiguators for the §9.3 `RegseqInstanceKey`
(`fn_shim_disambig`) -- they ensure distinct rustc
shim flavors sharing `(item, args)` get distinct
identities. The rendered envelope reads the
Instance's path via `render_def_path(inst.item)`
which already encodes bridge-synthesised families
in the path string per stage-12 D-12-6 + §13.4 (vtable
shims, drop glue, closure call methods, etc.). No
per-flavor branch is required at the renderer.

**Cross-references.** Composes the following
building blocks:

- `render_def_path(ItemId)` (stage 12 D-12-6) -- path
  resolution; reads `inst.item` per §9.2.
- `render_ty(TyId)` (stage 13 D-13-5) -- type
  rendering for `fn_sig` args + ret, Static / Const
  `ty`, and ADT-variant field types (D-14-5.5 lock).
- Body-level renderers (stage 13 D-13-2 / D-13-6,
  with the D-14-4.9 signature extensions for
  `render_operand` / `render_rvalue` /
  `render_generic_args`) -- `Function.body: Some(b)`
  envelope per §3.1 (locals decl block + per-block
  content).
- `render_const(ConstId, TyId)` (stage 14 D-14-4) --
  `Static.init.unwrap()` and `Const.value` rendering
  with the Instance's `ty` as consumer type per
  D-14-4.1 pair-form.
- §5.3 per-flag attribute rendering table (stage 13
  D-13-4.3) -- attribute-block composition; the
  D-14-5.7 layering lock pins whether `render_item`
  is in the `render_instance` call graph (composing
  via wrapper) or factored into a shared helper
  callable from both.

Per-variant rules across the 6 `InstanceKind`
variants are specified in the subsections below;
each subsection corresponds to a D-14-5 sub-decision:

- **§10.2.1** -- Function variant envelope per
  D-14-5.2 LOCKED.
- **§10.2.2** -- Static variant rendering per
  D-14-5.3 LOCKED.
- **§10.2.3** -- Const variant rendering per
  D-14-5.4 LOCKED.
- **§10.2.4** -- Struct variant rendering per
  D-14-5.5 LOCKED (Option B full-definition body).
- **§10.2.5** -- Enum variant rendering per
  D-14-5.5 LOCKED (Option B; EnumShape dispatch).
- **§10.2.6** -- Union variant rendering per
  D-14-5.5 LOCKED (Option B; named-field form).
- **§10.2.7** -- Attribute block composition rule
  per D-14-5.6 LOCKED (two-stream interleaving;
  Stream 1 via §5.6 shared helper, Stream 2 via
  `domain_attrs.item_attrs[item]` Vec-index walk
  with per-`AttrData` variant render rule).

**Layering boundary.** Per D-14-5.7 LOCKED,
`render_instance` does **not** call `render_item`;
it composes the full per-variant envelope from
scratch, sharing only the §5.6 internal helper
`render_item_attribute_block(item: ItemId) -> String`
for the Stream 1 attribute block. `render_item`
(stage-13 D-13-4 / D-13-4.5) remains the
standalone-Item view (single-line per-`ItemKind`
form for diagnostics / Item-table dumps). Both
renderers consume the §5.6 helper for Stream 1
ensuring byte-identical Stream 1 (Item-flag)
attribute streams. (`render_item` does NOT emit
Stream 2 domain-attribute rows; those live at the
Instance-level composer per §10.2.7.)

#### 10.2.1 Function variant envelope

Per D-14-5.2 LOCKED.
`InstanceKind::Function { fn_sig, body, flavor,
virtual_idx }` renders to a self-contained block:

```text
[<attribute-block>\n][unsafe ][extern "ABI" ]fn <render_def_path(item)>(<arg-list>[, ...]) -> <render_ty(output)>[ { <body-envelope> }];
```

The header line is followed by **exactly one** of
` { <body-envelope> }` (definition form, when
`body == Some(b)`) or `;` (declaration form, when
`body == None`).

**Aspect 1 -- Attribute block.** Per §10.2.7
attribute-block composition rule (D-14-5.6 LOCKED):
two-stream interleaving where Stream 1 (per-flag
§5.3 table-row order, emitted via the §5.6 shared
helper `render_item_attribute_block`) precedes
Stream 2 (`domain_attrs.item_attrs[item]` Vec-index
order, with per-`AttrData` variant render rule per
§10.2.7). Each `#[...]` renders on its own line;
the block ends with a trailing `\n` when non-empty.
Default-suppression (§5.2) applies per Stream 1
row; zero surviving attributes yields an empty
prefix, and the function-header line starts at
column 0. Stage-14 ships one occupant for Stream 2
(`AttrData::Intrinsic(IntrinsicKind::Log(LogLevel))`
-> `#[rseq::intrinsic("log-<sev>")]`); phase-2
stages append per-family render rules to §10.2.7.

**Aspect 2 -- Function-header line decoration.**
Keyword order matches Rust source grammar:

| Token             | Condition                                    | Source                                    |
|-------------------|----------------------------------------------|-------------------------------------------|
| `unsafe `         | `Item.is_unsafe == true`                     | `Item.is_unsafe` (matches §5.3 convention) |
| `extern "ABI" `   | `fn_sig.abi != FnAbi::Rust`                  | `model.sigs[fn_sig].abi` (§14.3)          |
| `fn `             | invariant                                    | -                                         |
| `<path>`          | invariant                                    | `render_def_path(inst.item)` per stage-12 D-12-6 |

The keyword block reduces to `[unsafe ][extern "ABI" ]fn`
because RegseqModel's MIR-shaped IR has no `const fn` /
`async fn` (post-MIR flattened) and emits no
visibility (every Instance is module-public per Q2's
codegen-keyed monomorphisation). `FnAbi` is the
closed 2-variant enum `{ Rust, C }` per `spec.md`
§14.1; the only non-default form in v1 is
`extern "C"`. Future ABI extensions append to
`FnAbi` and inherit the same `extern "<name>"`
rendering convention.

`Item.is_unsafe` and `fn_sig.safety` agree by
construction (bridge populates both from the same
`tcx.fn_sig(did).safety`). `Item.is_unsafe` is the
authoritative source so `render_item`'s standalone-
Item view (§5.3) and `render_instance`'s composer
output stay consistent.

The DefPath encodes bridge-synthesised flavors
(vtable shims, drop glue, closure call methods)
in the path string per §13.4. No per-flavor
rendering branch.

**Aspect 3 -- Arg-list grammar.**

| Case                                   | Form                                                                   |
|----------------------------------------|------------------------------------------------------------------------|
| `fn_sig.inputs.is_empty()`             | `()`                                                                   |
| `fn_sig.inputs.len() == N >= 1` (non-variadic) | `(_1: <render_ty(inputs[0])>, _2: <render_ty(inputs[1])>, ..., _N: <render_ty(inputs[N-1])>)` |
| `fn_sig.c_variadic == true`            | append `, ...` after the last fixed input (per `spec.md` §14.3 invariant `inputs.len() >= 1` so the leading comma is guaranteed) |

Per-arg local sigils `_1` through `_N` match the
§3.1 / `spec.md` §25.1 arg-locals convention
(locals indexed `1..=arg_count`,
`arg_count == fn_sig.inputs.len()`). Args
separated by `, ` (comma + space).

**Aspect 4 -- Return type.** `-> <render_ty(fn_sig.output)>`
always rendered, including the unit case `-> ()`.
Rust source permits omitting `-> ()` but the body
envelope (§3.1) always declares
`let mut _0: <render_ty(ret)>;`, so header-vs-locals
consistency takes precedence. Divergent return
renders as `-> !` per §6.1.

**Aspect 5 -- Body-vs-declaration trailer.**

| `body` field          | Trailer                                            |
|------------------------|----------------------------------------------------|
| `Some(b)` (definition) | ` {\n` + body envelope per §3.1 + `\n}`            |
| `None` (foreign decl)  | `;` (immediately after the return-type segment `-> <ret>`) |

The body envelope per §3.1 / stage-13 D-13-2 is
**unchanged** by D-14-5.2: locals declaration block
(`let mut _0: <ret>; let mut _N: T; ...`),
optional `debug NAME => _N;` lines (per §4.4 /
D-13-3 gating), and per-block content via
`render_basic_block` per §7.10. D-14-5.2 composes
the wrapper around the existing envelope.

The `body == None` declaration form surfaces
foreign-declaration Function Instances
(`Item.linkage == Linkage::Foreign` per `spec.md`
§3.1 invariant 10; D-12-2.1). RegseqModel renders
them as emission units (the C backend emits
`extern int foo(int);` for them); rustc's MIR
pretty-printer skips foreign items entirely. The
foreign-declaration form is the natural Rust-source
shape parallel to `extern "C" { fn foo(); }` block
content with the `extern "C" {}` brace structure
flattened (the bridge has monomorphised away the
declaration block).

**Composition outline** (definition form):

```text
[<attribute-block>\n][unsafe ][extern "ABI" ]fn <path>(<args>[, ...]) -> <ret> {
    let mut _0: <ret>;         // RETURN_PLACE; arg-locals _1..=_N are
                                // declared in the header arg-list above,
                                // not redeclared here (per §3.1 / D-13-2)
    let mut _M: T;             // first non-arg local (M == arg_count + 1)
    ...
    [debug NAME => _N;]        // when opts.include_debug_names
    ...

    bb0: { ... }
    ...
}
```

Composition outline (declaration form):

```text
[<attribute-block>\n][unsafe ][extern "ABI" ]fn <path>(<args>[, ...]) -> <ret>;
```

`flavor` / `virtual_idx` on `InstanceKind::Function`
are **not rendering inputs**: per D-14-5.1
reasoning + the prior-art Q8 / Q9 evidence, they
disambiguate shim identity at the model-construction
layer (§9.3 `fn_shim_disambig`) but the rendered
output reads only the DefPath (which already
encodes the synthesised family per §13.4).

#### Examples

The following examples illustrate the per-aspect
rules; full specimens land in §9.11 at the end of
the D-14-5 cluster:

```text
// Plain function, no surviving attributes, unit return:
fn pl011::write_byte(_1: &pl011::PL011, _2: u8) -> () {
    let mut _0: ();
    bb0: {
        ...
    }
}

// Unsafe function:
unsafe fn pl011::ptr_write(_1: *mut u8, _2: u8) -> () {
    let mut _0: ();
    bb0: {
        ...
    }
}

// Function with codegen-attribute block:
#[no_mangle]
#[inline(always)]
unsafe fn pl011::init(_1: *mut pl011::PL011) -> i32 {
    let mut _0: i32;
    bb0: {
        ...
    }
}

// Foreign declaration (body: None) with C ABI:
#[foreign]
extern "C" fn libc::printf(_1: *const u8) -> i32;

// Foreign declaration with C variadic:
#[foreign]
extern "C" fn libc::printf_variadic(_1: *const u8, ...) -> i32;

// Diagnostic-logging intrinsic (post-bridge-recognition;
// attribute-block ordering pinned at §10.2.7 per
// D-14-5.6 LOCKED -- Stream 1 (`#[inline(never)]` from
// Item.inline_hint) emitted first via the §5.6 helper;
// Stream 2 (`#[rseq::intrinsic("log-info")]` from
// AttrData::Intrinsic(Log(Info))) emitted after):
#[inline(never)]
#[rseq::intrinsic("log-info")]
fn regseq::log::intrinsics::log_info(_1: &str, _2: &[regseq::log::LogArg<'_>]) -> () {
    let mut _0: ();
    bb0: {
        ...
    }
}

// Bridge-synthesised drop glue (DefPath encodes the family):
fn core::ptr::drop_in_place::<pl011::PL011>(_1: *mut pl011::PL011) -> () {
    let mut _0: ();
    bb0: {
        ...
    }
}

// Divergent return:
fn pl011::panic_handler(_1: &core::panic::PanicInfo) -> ! {
    let mut _0: !;
    bb0: {
        ...
    }
}
```

#### 10.2.2 Static variant rendering

Per D-14-5.3 LOCKED.
`InstanceKind::Static { ty, init }` renders as a
single-line composer (with optional multi-line
attribute block above):

```text
[<attribute-block>\n]static [mut ]<render_def_path(item)>: <render_ty(ty)>[ = <render_const(init.unwrap(), ty)>];
```

**Aspect 1 -- Attribute block.** Per §10.2.7
attribute-block composition rule (D-14-5.6 LOCKED):
two-stream interleaving (Stream 1 §5.3 table-row
order via the §5.6 shared helper, Stream 2
`domain_attrs.item_attrs[item]` Vec-index order).
Stage-14 ships no `AttrData::Intrinsic` variant
occupying Static Items (intrinsic recognition
targets Function Items per `spec.md` §3.6.3 /
D-14-1); the Static-Instance attribute block
reduces to Stream 1 in v1 (i.e., the §5.3 per-flag
stream).

**Aspect 2 -- Mutability prefix.** Read from
`model.items[inst.item].kind` per stage-12 D-12-1.2 (c):

| `ItemKind::Static` field           | Rendered prefix    |
|------------------------------------|--------------------|
| `mutability: Mutability::Not`      | `static `          |
| `mutability: Mutability::Mut`      | `static mut `      |

Statics can't be generic so the 1:1 Item:Instance
mapping always holds (`spec.md` §8.0 + §9.4); the
Item-side mutability is the authoritative source.
`render_item`'s §5.4 standalone-Item view reads
the same source.

**Aspect 3 -- Path + type.**
- Path: `<render_def_path(inst.item)>` via stage-12
  D-12-6. Bridge-synthesised statics (vtable
  instances per `spec.md` §13.2, `ConstMemoryStatic`
  per `spec.md` §13.3) carry their family-encoded
  paths unchanged.
- Type: `: <render_ty(ty)>` always rendered. Source:
  `InstanceKind::Static.ty`.

**Aspect 4 -- Initializer-vs-declaration trailer.**

| `init` field           | Trailer                                           |
|-------------------------|---------------------------------------------------|
| `Some(c)` (definition)  | ` = <render_const(c, ty)>;`                       |
| `None` (foreign decl)   | `;` (immediately after the type decoration)      |

Definition-form value composes the D-14-4 pair-form:
the Static's `ty` is the consumer-context type for
the const value, satisfying D-14-4.1's API contract.
No layout walking at the D-14-5.3 layer; the §9.7
canonical-form rule plus D-14-4's per-arm dispatch
table handles all per-variant value rendering.

Declaration-form (`init == None`) surfaces foreign
extern statics. Per `spec.md` §3.4 step 8 / D-12-2.2
the biconditional `init.is_none() iff Item.linkage
== Linkage::Foreign` holds by construction; the
`#[foreign]` attribute (§5.3) and trailing `;`
reinforce each other in the rendered output (mirrors
the Function `body == None` shape per §10.2.1
aspect 5).

**Composition outline** (definition form):

```text
[<attribute-block>\n]static [mut ]<path>: <ty> = <const>;
```

Composition outline (declaration form):

```text
[<attribute-block>\n]static [mut ]<path>: <ty>;
```

#### Examples

```text
// Defined static, immutable:
static pl011::BASE_ADDR: u32 = 268472320_u32;

// Defined static, mutable, with multi-field initializer
// (the renderer emits on one line; the linebreak below
// is illustrative for readability only):
static mut pl011::DRIVER_STATE: pl011::State = pl011::State { initialised: false, count: 0_u32 };

// Foreign extern static (init: None):
#[foreign]
static libc::stderr: *mut libc::FILE;

// Bridge-synthesised ConstMemoryStatic (per spec.md §13.3;
// DefPath encodes the synthetic family):
static <const_mem_static for "hello\0">: [u8; 7] = *b"hello\0\0";

// Bridge-synthesised vtable instance (per spec.md §13.2;
// DefPath encodes the synthetic family):
static <vtable for pl011::Writer for pl011::PL011>: pl011::__VTable<Writer, PL011> = pl011::__VTable { drop: ..., size: 24_usize, align: 8_usize, write: ... };
```

#### 10.2.3 Const variant rendering

Per D-14-5.4 LOCKED.
`InstanceKind::Const { ty, value }` renders as a
single-line composer (with optional multi-line
attribute block above):

```text
[<attribute-block>\n]const <render_def_path(item)>: <render_ty(ty)> = <render_const(value, ty)>;
```

No mutability prefix (consts are immutable by
definition; `const mut` is not valid Rust source).
No foreign-declaration form (consts cannot be
foreign in Rust; `extern "C" { const FOO: u32; }`
is invalid). `value: ConstId` is always populated
per stage-12 D-12-2 -- no `Option` wrap, no
declaration-vs-definition dispatch.

**Aspect 1 -- Attribute block.** Per §10.2.7
attribute-block composition rule (D-14-5.6 LOCKED).
Stage-14 ships no `AttrData::Intrinsic` variant
occupying Const Items; the Const-Instance attribute
block reduces to Stream 1 in v1 (i.e., the §5.3
per-flag stream via the §5.6 shared helper).

**Aspect 2 -- Path + type + value + trailer.**

| Token                         | Source                          |
|-------------------------------|---------------------------------|
| `const `                      | invariant keyword               |
| `<render_def_path(item)>`     | stage-12 D-12-6                 |
| `: <render_ty(ty)>`           | `InstanceKind::Const.ty`        |
| ` = <render_const(value, ty)>`| `InstanceKind::Const.value` + D-14-4.1 pair-form |
| `;`                           | invariant trailer               |

The value is always populated; no
declaration-vs-definition dispatch and no
foreign-declaration form (Rust source forbids
both).

**Composition outline:**

```text
[<attribute-block>\n]const <path>: <ty> = <const>;
```

#### Examples

```text
// Scalar const (i32):
const pl011::TX_FIFO_SIZE: i32 = 16_i32;

// Scalar const at usize:
const pl011::MAX_BAUD: usize = 4000000_usize;

// Struct const (renderer emits on one line; linebreak below illustrative):
const pl011::DEFAULT_CONFIG: pl011::Config = pl011::Config { baud_rate: 115200_u32, parity: pl011::Parity::None };

// Enum-variant const:
const pl011::DEFAULT_PARITY: pl011::Parity = pl011::Parity::None;

// ZST const (e.g., a PhantomData marker):
const pl011::MARKER: core::marker::PhantomData<u32> = core::marker::PhantomData;

// Reference-typed const (render_const Bytes / Aggregate
// dispatch per D-14-4.4 + D-14-4.6 fat-pointer special case):
const pl011::GREETING: &str = "hello";
```

#### 10.2.4 Struct variant rendering

Per D-14-5.5 LOCKED (Option B -- full definition body
from `spec.md` §11 Layout).
`InstanceKind::Struct { layout }` resolves to
`Ty::Struct { item, args, layout = Layout { kind:
LayoutKind::Struct { fields }, .. } }` per
`spec.md` §9.4 / §10.

**Attribute block.** Per §10.2.7 attribute-block
composition rule (D-14-5.6 LOCKED): Stream 1
(Item-flag, §5.3 table-row order via the §5.6
shared helper) precedes Stream 2
(`domain_attrs.item_attrs[item]` Vec-index order).
Stage-14 ships no `AttrData::Intrinsic` variant
occupying Struct Items in v1 (intrinsic recognition
targets Function Items per `spec.md` §3.6.3 /
D-14-1); the Struct-Instance attribute block
reduces to Stream 1 in v1. For ADTs, Stream 1
typically surfaces lang-item flags
(`#[lang = "phantom_data"]` / `#[lang = "unsafe_cell"]` /
`#[lang = "manually_drop"]`), `#[opaque_handle]`
eligibility, and `#[link_section]`.

**Dispatch on `(fields.len(), first-field-name-pattern per §3.4)`.**

| Case                                                          | Rendered form                                                                                                |
|---------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------|
| `fields.is_empty()`                                           | `[<attrs>\n]struct <path><args>;` (unit form -- collapses Rust's three syntactic empty-struct shapes)        |
| All `fields[i].name` match `_<idx>` synthetic pattern         | tuple form (see below)                                                                                       |
| `fields[i].name` carry real source names                      | named-field form (see below)                                                                                 |

Tuple form (one line):

```text
[<attribute-block>\n]struct <render_def_path(item)><render_generic_args(args, item)>(<render_ty(fields[0].ty)>, <render_ty(fields[1].ty)>, ...);
```

Named-field form (multi-line):

```text
[<attribute-block>\n]struct <render_def_path(item)><render_generic_args(args, item)> {
    <fields[0].name>: <render_ty(fields[0].ty)>,
    <fields[1].name>: <render_ty(fields[1].ty)>,
    ...
}
```

**Sources:**

- `inst.item` (== `Ty::Struct.item`) -- DefPath via stage-12 D-12-6.
- `Ty::Struct.args` (from the `layout: TyId` resolved to `Ty::Struct`) -- generic-args via §6.7 (with `item` parameter per D-14-4.9).
- `LayoutKind::Struct.fields` -- field iteration per `spec.md` §11.0 (indexed by source / MIR field index; offsets carried but not rendered in v1).

**Field-name pattern check.** Synthetic `_<idx>` names (where `<idx>` is the field's index in `fields`) indicate tuple-style per `spec.md` §11.0 + §3.4. A Rust source struct never mixes tuple and named fields, so a first-field check is sufficient (all fields share the same style).

#### Examples

```text
// Unit struct:
struct pl011::Marker;

// Tuple struct:
struct pl011::Coord(i32, i32);

// Generic tuple struct (post-mono):
struct pl011::Pair<u32, u8>(u32, u8);

// Named-field struct:
struct pl011::Config {
    baud_rate: u32,
    parity: pl011::Parity,
}

// Generic named-field struct with lang-item attribute:
#[lang = "manually_drop"]
struct core::mem::ManuallyDrop<pl011::Resource> {
    value: pl011::Resource,
}

// Bridge-synthesised closure-captures struct (per spec.md §13.1):
struct <closure_captures for crate::main::closure#0> {
    captured_x: u32,
    captured_ref: &pl011::PL011,
}
```

#### 10.2.5 Enum variant rendering

Per D-14-5.5 LOCKED (Option B).
`InstanceKind::Enum { layout }` resolves to
`Ty::Enum { item, args, layout = Layout { kind:
LayoutKind::Enum { shape }, .. } }` per `spec.md`
§9.4 / §10 / §11.1.

**Attribute block.** Per §10.2.7 attribute-block
composition rule (D-14-5.6 LOCKED): Stream 1
(Item-flag, §5.3 table-row order via the §5.6
shared helper) precedes Stream 2
(`domain_attrs.item_attrs[item]` Vec-index order).
Stage-14 ships no `AttrData::Intrinsic` variant
occupying Enum Items in v1 (intrinsic recognition
targets Function Items per `spec.md` §3.6.3 /
D-14-1); the Enum-Instance attribute block reduces
to Stream 1 in v1.

**Dispatch on `shape: EnumShape` per `spec.md` §11.1.**

| `EnumShape`                                          | Rendered form                                                              |
|------------------------------------------------------|----------------------------------------------------------------------------|
| `Empty`                                              | `[<attrs>\n]enum <path><args> {}` (uninhabited; zero-variant empty body)   |
| `Single { variant, .. }`                             | one variant arm inside `{ ... }`                                           |
| `Multiple { variants, .. }`                          | per-variant arms inside `{ ... }`, comma-separated, trailing comma after last |

**Per-variant arm form** (each `variant: VariantLayout`,
dispatching on `(variant.fields.len(), first-field-name-pattern)`):

| Case                                                              | Rendered form                                                                 |
|-------------------------------------------------------------------|-------------------------------------------------------------------------------|
| `variant.fields.is_empty()`                                       | `<variant.name>` (unit variant)                                               |
| All `variant.fields[i].name` match `_<idx>` synthetic pattern     | `<variant.name>(<render_ty(fields[0].ty)>, ...)` (tuple variant)              |
| `variant.fields[i].name` carry real source names                  | `<variant.name> { <fields[0].name>: <render_ty(fields[0].ty)>, ... }` (named-field variant) |

Multi-variant composition (multi-line):

```text
[<attribute-block>\n]enum <render_def_path(item)><render_generic_args(args, item)> {
    <variant_arm_0>,
    <variant_arm_1>,
    ...
    <variant_arm_N>,
}
```

Trailing comma after the last variant arm matches Rust source idiom.

**Single-variant composition (multi-line):**

```text
[<attribute-block>\n]enum <render_def_path(item)><render_generic_args(args, item)> {
    <variant_arm>,
}
```

**Variant name lookup** via `VariantLayout.name: StringId` per `spec.md` §11.1. Non-optional (validation rejects missing variant names per `spec.md` §11.x invariants).

**Variants may mix ctor forms.** Each variant renders per its own field-name pattern; Rust source allows mixed shapes per enum:

```text
enum pl011::Status {
    Ready,                       // unit
    Busy(u32),                   // tuple
    Error { code: i32 },         // named-field
}
```

**v1 omits discriminant values.** `VariantLayout.discriminant` (per `spec.md` §11.1) carries the resolved discriminant bit-pattern, and `tag_value` the layout-level tag. v1 rendered output does not surface explicit `Variant = N,` discriminant syntax; specimens distinguish variants by name + body shape. Future revision may add discriminant rendering when the deferred `#[repr(...)]` work surfaces; not pre-allocated per `../README.md` §5.9 rule 4.

**v1 omits `EnumShape::Single { scalar: Some(_) }` tag-scalar surfacing.** The single-variant fieldless `repr(int)` enum case (per `spec.md` §11.1: a tag scalar is allocated even with one variant) renders the variant arm normally; the tag-scalar info is IR-internal and not surfaced.

#### Examples

```text
// Empty enum (uninhabited):
enum core::convert::Infallible {}

// Single-variant enum:
enum pl011::Singleton {
    Only,
}

// Multi-variant enum, all-unit ctor forms (C-style enum):
enum pl011::Parity {
    None,
    Even,
    Odd,
}

// Multi-variant enum, mixed ctor forms:
enum pl011::Status {
    Ready,
    Busy(u32),
    Error { code: i32, retry: bool },
}

// Generic enum (post-mono):
enum core::option::Option<u32> {
    None,
    Some(u32),
}
```

#### 10.2.6 Union variant rendering

Per D-14-5.5 LOCKED (Option B).
`InstanceKind::Union { layout }` resolves to
`Ty::Union { item, args, layout = Layout { kind:
LayoutKind::Union { fields }, .. } }` per
`spec.md` §9.4 / §10 / §11.

**Attribute block.** Per §10.2.7 attribute-block
composition rule (D-14-5.6 LOCKED): Stream 1
(Item-flag, §5.3 table-row order via the §5.6
shared helper) precedes Stream 2
(`domain_attrs.item_attrs[item]` Vec-index order).
Stage-14 ships no `AttrData::Intrinsic` variant
occupying Union Items in v1 (intrinsic recognition
targets Function Items per `spec.md` §3.6.3 /
D-14-1); the Union-Instance attribute block reduces
to Stream 1 in v1. Lang-item flag
`#[lang = "unsafe_cell"]` is a common Stream 1 entry
for Union Items (e.g., `core::cell::UnsafeCell`).

**Form.** Unions carry only named-field bodies in
Rust source (no tuple-union, no unit-union); the
renderer unconditionally produces:

```text
[<attribute-block>\n]union <render_def_path(item)><render_generic_args(args, item)> {
    <fields[0].name>: <render_ty(fields[0].ty)>,
    <fields[1].name>: <render_ty(fields[1].ty)>,
    ...
}
```

If `fields.is_empty()` (Rust source forbids it; the IR allows the shape structurally), render `union <path><args> {}` -- no defensive special-case at the renderer level. A model construction bug surfaces at validation per `spec.md` §11.x invariants, not at render.

`LayoutKind::Union.fields` carries field offsets per `spec.md` §11.0 (all offsets are 0 for union fields, since they share the union's address). Offsets are not rendered in v1 output.

#### Examples

```text
// Simple union:
union pl011::RegBytes {
    word: u32,
    bytes: [u8; 4],
}

// Generic union (post-mono):
union pl011::Reg<u32> {
    value: u32,
    bytes: [u8; 4],
}

// Bridge-synthesised vtable struct (per spec.md §13.2;
// NOTE: vtables are Struct, not Union -- shown here for
// contrast):
//
// struct <vtable_ty for dyn pl011::Writer> {
//     drop:  *mut fn(*mut ()),
//     size:  usize,
//     align: usize,
//     write: *mut fn(*mut (), u8) -> (),
// }
```

#### 10.2.7 Attribute block composition

Per D-14-5.6 LOCKED.
The attribute-block prefix consumed by §10.2.1 /
§10.2.2 / §10.2.3 / §10.2.4 / §10.2.5 / §10.2.6 is
composed by **interleaving two deterministic streams
in fixed order**: Item-flag stream first, then
`domain_attrs.item_attrs[item]` stream. Each `#[...]`
renders on its own line; the block ends with a
trailing `\n` when non-empty, empty otherwise.

```text
<Stream 1 -- Item-flag stream, §5.3 table-row order>
<Stream 2 -- domain_attrs.item_attrs Vec-index order>
```

**Stream 1 -- Item-flag stream.** Produced by the
§5.6 shared helper `render_item_attribute_block(item:
ItemId) -> String`. Walks the §5.3 per-flag table
in table-row order; emits surviving `#[...]` rows
per §5.2 default-suppression. The same helper is
consumed by `render_item` per §5.4; both renderers
produce byte-identical Stream 1 output.

**Stream 2 -- `domain_attrs.item_attrs[item]` stream.**
Walks the `Vec<AttrData>` at
`model.domain_attrs.item_attrs[item]` in **Vec-index
order** per `spec.md` §3.6.2's bridge-walk-order
determinism rule (D-14-3.3 LOCKED). Per-`AttrData`
variant render rules:

| `AttrData` variant                                    | Rendered form                       |
|-------------------------------------------------------|-------------------------------------|
| `Intrinsic(IntrinsicKind::Log(LogLevel::Debug))`      | `#[rseq::intrinsic("log-debug")]`   |
| `Intrinsic(IntrinsicKind::Log(LogLevel::Info))`       | `#[rseq::intrinsic("log-info")]`    |
| `Intrinsic(IntrinsicKind::Log(LogLevel::Warn))`       | `#[rseq::intrinsic("log-warn")]`    |
| `Intrinsic(IntrinsicKind::Log(LogLevel::Error))`      | `#[rseq::intrinsic("log-error")]`   |

The dash-form NAME (`log-debug` / `log-info` /
`log-warn` / `log-error`) is the canonical
intrinsic NAME from `spec.md` §3.6.3
`IntrinsicKind::from_name` reversed; the surface
form `#[rseq::intrinsic("<NAME>")]` mirrors the
DSL-level recognition shape per Q7 + stage-14
D-14-1 LOCKED.

**Phase-2 family render rules append.** Future
`AttrData` variants (`Mmio` per stage 16,
`Interrupt` per stage 17, `Atomic` per stage 18,
...) carry their per-variant render rules per the
relevant stage's spec; the per-`AttrData` render
table extends append-only per `spec.md` §3.6.2's
closed-append-only-union rule. Stage-14 commits
only the `Intrinsic` variant rendering.

**Stream 2 helper convention.** Internal method on
`PrettyPrinter<'_>` (illustrative; not part of the
public renderer family):

```rust
impl PrettyPrinter<'_> {
    fn render_domain_attribute_block(&self, id: ItemId) -> String;
}
```

Walks `model.domain_attrs.item_attrs[id]` in
Vec-index order, dispatching per `AttrData` variant
per the table above. Returns the multi-line block
(with trailing `\n` when non-empty) or empty
string. The helper composes after Stream 1 in
`render_instance`'s per-variant envelope.

**Combined output layout.**

```text
<Stream 1 row 0>          <-- e.g., #[no_mangle]
<Stream 1 row 1>          <-- e.g., #[inline(always)]
...
<Stream 2 row 0>          <-- e.g., #[rseq::intrinsic("log-info")]
<Stream 2 row 1>
...
<kind-keyword line>       <-- starts at column 0
```

Empty cases:
- **Both streams empty**: no prefix; kind-keyword
  line starts at column 0 (no leading `\n`).
- **Stream 1 empty, Stream 2 non-empty**: only
  Stream 2 rows render; trailing `\n` after the
  last Stream 2 row.
- **Stream 1 non-empty, Stream 2 empty**: only
  Stream 1 rows render; trailing `\n` after the
  last Stream 1 row.

**Examples:**

```text
// Function Item with no surviving attributes (empty block):
fn pl011::ordinary_function(_1: u32) -> u32 {
    let mut _0: u32;
    bb0: { ... }
}

// Function Item with Stream 1 only:
#[no_mangle]
#[inline(always)]
#[link_section = ".text.boot"]
unsafe fn pl011::init(_1: *mut pl011::PL011) -> i32 {
    let mut _0: i32;
    bb0: { ... }
}

// Function Item with both streams (Stream 1 first, Stream 2 second):
#[no_mangle]                          <-- Stream 1: linkage = NoMangle
#[inline(never)]                      <-- Stream 1: inline_hint = Never
#[rseq::intrinsic("log-info")]        <-- Stream 2: AttrData::Intrinsic(Log(Info))
fn regseq::log::intrinsics::log_info(_1: &str, _2: &[regseq::log::LogArg<'_>]) -> () {
    let mut _0: ();
    bb0: { ... }
}

// Foreign declaration (Stream 1 only, Stream 2 empty):
#[foreign]                            <-- Stream 1: linkage = Foreign
extern "C" fn libc::printf(_1: *const u8) -> i32;

// Struct Item with lang-item flag (Stream 1 only):
#[lang = "manually_drop"]             <-- Stream 1: is_manually_drop = true
struct core::mem::ManuallyDrop<pl011::Resource> {
    value: pl011::Resource,
}
```

### 10.3 Top-level `render(&self) -> String`

The top-level composer renderer. Iterates
`model.instances` and composes per-Instance via
`render_instance` (§10.2) plus an inter-Instance
separator and the no-file-envelope rule defined
below.

#### 10.3.1 API signature

```rust
impl PrettyPrinter<'_> {
    pub fn render(&self) -> String;
}
```

Zero arguments beyond `&self`; no
`single: Option<InstanceId>` filter parameter (single-
Instance dispatch is `render_instance(InstanceId)` per
§10.2); no write-sink form
(`&mut W: io::Write`); no `Option` / `Result` return
wrapping. Inherits the §2.3 String-return method-shape
convention. The composer consumes
`self.model.instances` (the canonical IndexVec keyed
by `InstanceId`) and produces a single `String`
covering the rendering of every Instance in the model.

#### 10.3.2 Iteration order

`render` walks `self.model.instances.iter_enumerated()`
in **`InstanceId` order verbatim** -- no render-time
re-sort. The IndexVec is already in canonical F4-2
sorted order per `spec.md` §16.3
(`RegseqInstanceKey = (regseq_kind_rank, item_id,
generic_args_key, fn_shim_disambig)`), which provides:

- **Cross-build stability** under the build-equivalence
  quad (`spec.md` §12.4 / §16.1): two builds satisfying
  the quad produce identical iteration orders
  independent of source layout.
- **InstanceKind rank-grouped readability**: the F4-2
  key's first component is
  `regseq_instance_kind_rank` per `spec.md` §9.3, so
  the iteration naturally groups Functions (rank 0)
  before Statics (rank 1) before Consts (rank 2)
  before Structs (rank 3) before Enums (rank 4) before
  Unions (rank 5).

The renderer does NOT iterate `model.items`,
`model.types`, or any other table; only
`model.instances`. Items and types surface
transitively via the per-Instance dispatch through
`render_instance` (which consumes `Item` /
`ItemKind` / `Layout` via the existing
`spec.md`-defined access paths).

#### 10.3.3 Per-Instance separator

`render_instance(id)` outputs (§10.2) do NOT carry a
trailing newline -- they end with the last byte of the
per-variant rendering (`;` for declaration forms and
Static / Const; closing `}` for body forms and ADT
definitions per §10.2.1 - §10.2.6).

The top-level composer owns the separator bytes.
Between consecutive `render_instance(id)` outputs,
`render` emits the two-byte sequence `"\n\n"` -- the
first `\n` terminates the prior Instance's last line;
the second `\n` produces the single blank line between
Instances. No separator is emitted before the first
Instance; no separator is emitted after the last
Instance.

The composition rule:

```text
for each Instance id in model.instances.iter_enumerated():
    if not first:
        emit "\n\n"
    emit render_instance(id)
    first = false
```

The first-flag mechanism mirrors the upstream
pretty-printer convention of guarded inter-item
separation; the difference is that the top-level
composer owns the full separator bytes rather than
relying on per-Instance trailing newlines.

#### 10.3.4 File-level output envelope

`render` emits **no file-level header**, **no
file-level trailer**, **no metadata wrapper**. The
output begins at column 0 of line 1 with the first
rendered byte of the first Instance's
`render_instance` output -- which is the first byte
of its attribute block when the Stream-1 + Stream-2
attribute composition at §10.2.7 produces a non-empty
block (e.g., `#[rseq::intrinsic("log-info")]`,
`#[foreign]`, `#[lang = "..."]`), and otherwise the
first byte of the kind-keyword line (`fn` / `static` /
`static mut` / `const` / `struct` / `enum` / `union`
per §10.2.1 - §10.2.6).

The output ends with the last rendered byte of the
last Instance's `render_instance` output -- the closing
`}` for body forms and ADT definitions, or the `;` for
declaration forms and Static / Const. `render` does
not inject any trailing newline after the last
Instance.

No `// WARNING: ...` header (the snapshot test suite
enforces format stability); no
`// Crate: foo, toolchain: bar` metadata
(`model.build_info` carries the build provenance
structurally per `spec.md` §3.1); no version tag.

#### 10.3.5 Empty-model case

When `self.model.instances.is_empty()`, `render`
returns the empty `String` `""`. No header (per
§10.3.4); no separator (no consecutive pairs);
no placeholder comment. The 0-byte output is the
identity element of the per-Instance concatenation
+ separator composition.

Empty `model.instances` is the operational form for:
- Foundation-crate test fixtures exercising pure-Item
  surfaces (DefPath / IntrinsicKind / Layout schema)
  without monomorphizing any bodies.
- Future-stage validator round-trip baselines on
  `RegseqModel::new()`.
- Rejection-path fixtures where bridge construction
  errs out before populating any Instance.

In all three, the empty-`String` output is the correct
response; consumers reading the snapshot inherit a
0-byte file and know the model is intentionally empty.

#### 10.3.6 Composition outline

The full composition outline (consolidating §10.3.1 -
§10.3.5):

```text
render(&self) -> String:
  buf = ""
  first = true
  for (id, _inst) in self.model.instances.iter_enumerated():
    if first:
      first = false
    else:
      buf.push_str("\n\n")   // §10.3.3 separator
    buf.push_str(&self.render_instance(id))  // §10.2 composer
  buf                        // §10.3.4 no trailing decoration;
                              // §10.3.5 empty when instances is empty
```

The composer is **purely scaffolding** over
`render_instance(InstanceId)` (§10.2): it adds the
iteration + separator + framing rules; all per-Instance
content comes from `render_instance` unchanged. No
file-level decoration layer; no per-Instance
post-processing.

#### 10.3.7 Worked composition examples

Two illustrative model shapes (full per-Instance
rendering is exercised in §9.12 specimens).

**Three-Instance model (mixed InstanceKind).**

Suppose `model.instances = [I0, I1, I2]` where:
- `I0 (InstanceId(0))`: Function `pl011::write_byte`
  (rank 0).
- `I1 (InstanceId(1))`: Static `pl011::BASE_ADDR`
  (rank 1).
- `I2 (InstanceId(2))`: Struct `pl011::Resource`
  (rank 3).

Per §10.3.2, the iteration walks `[0, 1, 2]` in
order. Per §10.3.3, single blank lines separate
consecutive Instances. Per §10.3.4, no file-level
header / trailer. The full `render(&self)` output:

```text
fn pl011::write_byte(_1: u8) -> () {
    let mut _0: ();

    bb0: {
        _0 = ();
        return;
    }
}

static pl011::BASE_ADDR: u32 = 268472320_u32;

struct pl011::Resource {
    base: u32,
    state: u32,
}
```

The output begins at column 0 line 1 with
`fn pl011::write_byte(`; ends at the closing `}` byte
of `struct pl011::Resource`. `render` does not inject
any trailing newline after the last Instance per
§10.3.4. The blank line between `let mut _0: ();` and
`bb0:` is the function-body envelope per §3.1 (locals-
declaration block + scope-tree blank-line separator),
not a `render`-level separator.

**Empty model.**

For `model.instances = IndexVec::new()`:

```text

```

(0-byte output; no header, no placeholder.)

## 11. Cross-references to `spec.md`

Per-construct rendering pointers from `spec.md`'s
structural sections:

| `spec.md` section                              | Pointer                                          |
|------------------------------------------------|--------------------------------------------------|
| §9.7 `Const` vocabulary                        | Rendering: see `pretty-printer.md` §10.1 (`render_const` per-arm specs). |
| §10 Type alphabet (`Ty` enum)                  | Rendering: see `pretty-printer.md` §6.           |
| §11 Layout                                     | Rendering: see `pretty-printer.md` §6 (ADT layouts surface via `Ty::Struct/Enum/Union` + `place_ty` walk). |
| §15 Bridge translation rules                   | (No pretty-printer pointer; translation is producer-side.) |
| §18 Pretty-printer (now stub)                  | This whole file.                                 |
| §19 Place                                      | Rendering: see `pretty-printer.md` §7.1.         |
| §20 Operand                                    | Rendering: see `pretty-printer.md` §7.2.         |
| §21 Rvalue                                     | Rendering: see `pretty-printer.md` §7.3.         |
| §22 Statement                                  | Rendering: see `pretty-printer.md` §7.4.         |
| §23 Terminator                                 | Rendering: see `pretty-printer.md` §7.5-7.9.     |
| §24 BasicBlock                                 | Rendering: see `pretty-printer.md` §7.10.        |
| §25 Body                                       | Rendering: see `pretty-printer.md` §3 (envelope) + §7. |
| §8 Item                                        | Rendering: see `pretty-printer.md` §5 (`render_item`); `render_def_path` listed in §8 ownership table. |
| §9 Instance / InstanceKind                     | Rendering: see `pretty-printer.md` §10.2 (`render_instance` composer per stage-14 D-14-5, with §10.2.1-§10.2.7 per-variant + attribute-composition subsections); §1.3 carries the composer-ownership overview. |
