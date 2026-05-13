# Pretty-printer

> **Snapshot at close of stage 13 -- current pretty-printer
> spec lives in top-level `../pretty-printer.md`.** This file
> is a frozen historical artifact: it preserves what stage 13
> contributed at its close as the new top-level
> pretty-printer.md companion file's seed draft. Subsequent
> tier-3 post-merge Codex review (R1-R4) refinements landed
> in the top-level `../pretty-printer.md` only; this snapshot
> retains the per-stage-Phase-B form. See
> `review-log.md` for the round-by-round trail.

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

- `render_const(ConstId) -> String` -- composes
  `render_ty` for type-context arms.
- `render_instance(InstanceId) -> String` -- composes
  `render_item` (wrapper) + per-Instance content
  (`InstanceKind::Function.fn_sig` rendered via
  `render_ty`; `InstanceKind::Function.body` rendered
  via the body envelope + body-level renderers;
  `InstanceKind::Static.{ty, init}`; `InstanceKind::Const.value`;
  `InstanceKind::{Struct, Enum, Union}.layout`).
- `render(&self) -> String` -- top-level model dump;
  iterates `model.instances` (Q2 codegen-keyed) and
  composes per-Instance renderings.

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
| `ConstId`      | rendered via `render_const(ConstId)` (stage 14)   | per-variant value form; consumes `render_ty` for type-context arms     |
| `InstanceId`   | rendered via `render_instance(InstanceId)` (stage 14) | composes `render_item` wrapper + per-Instance content                  |
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

- Atomic: `#[no_mangle]`, `#[naked]`,
  `#[opaque_handle]`.
- Key-value: `#[link_section = "..."]`,
  `#[linkage = "weak"]`, `#[lang = "phantom_data"]`.
- Argumented: `#[inline]`, `#[inline(always)]`,
  `#[inline(never)]`.

### 5.2 Default-suppression

All default-valued / false-valued fields suppress.
A Function Item with no explicit attributes (default
linkage, no inline hint, default link section, not
unsafe, not naked, no lang-item flags, not eligible
for opaque-handle) renders as bare `fn $path` with
zero attribute prefixes.

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
| `is_naked = true`                | `#[naked]`                          |
| `is_naked = false`               | suppressed                          |
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

| Variant                             | Rendered form                                                      |
|-------------------------------------|--------------------------------------------------------------------|
| `Struct { item, args, .. }`         | `<render_def_path(item)><render_generic_args(args)>`               |
| `Enum   { item, args, .. }`         | `<render_def_path(item)><render_generic_args(args)>`               |
| `Union  { item, args, .. }`         | `<render_def_path(item)><render_generic_args(args)>`               |
| `Closure(item, args)`               | `<render_def_path(item)><render_generic_args(args)>`               |
| `FnDef(item, args)`                 | `fn(<arg_tys>) -> <ret_ty> {<render_def_path(item)><render_generic_args(args)>}` |

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
fn render_generic_args(&self, args: &GenericArgs) -> String;
```

Form: `<T1, T2, K1>` non-empty; empty string when
`args.is_empty()`. `GenericArg` is the closed 2-variant
enum `{ Type(TyId), Const(ConstId) }` per `spec.md`
§9.1 (lifetimes are erased post-monomorphisation and
not represented). Per-arg dispatch:

- `GenericArg::Type(ty)` -> `render_ty(ty)`.
- `GenericArg::Const(c)` -> `render_const(c)` (stage-14
  composer).

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

### 7.2 `render_operand(&Operand, &Body) -> String`

```rust
enum Operand { Copy(Place), Move(Place), Constant(ConstId) }
```

| Variant         | Rendered form                                |
|-----------------|----------------------------------------------|
| `Copy(p)`       | `copy <render_place(p, body)>`               |
| `Move(p)`       | `move <render_place(p, body)>`               |
| `Constant(id)`  | `<render_const(id)>` (stage-14 composer)     |

`Constant` is rendered bare; the const value's own
rendering encodes its const-ness (`42i32`, `true`,
`&"..."`, etc.).

### 7.3 `render_rvalue(&Rvalue, &Body) -> String`

The closed 9-variant `Rvalue` enum (`spec.md` §21).

| Variant                                       | Rendered form                                                                  |
|-----------------------------------------------|--------------------------------------------------------------------------------|
| `Use(operand)`                                | `<render_operand(operand, body)>` (transparent)                                |
| `Repeat(operand, count)`                      | `[<render_operand(operand, body)>; <count>]`                                   |
| `Ref(Provenance::SharedBorrow, place)`        | `&<render_place(place, body)>`                                                 |
| `Ref(Provenance::UniqueBorrow, place)`        | `&mut <render_place(place, body)>`                                             |
| `RawPtr(Mutability::Not, place)`              | `&raw const <render_place(place, body)>`                                       |
| `RawPtr(Mutability::Mut, place)`              | `&raw mut <render_place(place, body)>`                                         |
| `Cast(kind, operand, ty)`                     | `<render_operand(operand, body)> as <render_ty(ty)>` (CastKind shown only when ambiguous; see 7.3.1) |
| `BinaryOp(op, lhs, rhs)`                      | `<render_operand(lhs, body)> <op_glyph> <render_operand(rhs, body)>`           |
| `UnaryOp(op, operand)`                        | `<op_glyph><render_operand(operand, body)>`                                    |
| `Discriminant(place)`                         | `discriminant(<render_place(place, body)>)`                                    |
| `Aggregate(kind, operands)`                   | per `AggregateKind` variant; see 7.3.2                                         |

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

| `AggregateKind` variant                | Resolved destination type                  | Rendered form                                                                                                  |
|----------------------------------------|--------------------------------------------|----------------------------------------------------------------------------------------------------------------|
| `Array(elem_ty)`                       | `Ty::Array(elem_ty, n)`                    | `[<op0>, <op1>, ...]`                                                                                          |
| `Tuple(tuple_ty)` arity 0              | `Ty::Tuple { fields: [], .. }`             | `()`                                                                                                           |
| `Tuple(tuple_ty)` arity 1              | `Ty::Tuple { fields: [_], .. }`            | `(<op0>,)`                                                                                                     |
| `Tuple(tuple_ty)` arity 2+             | `Ty::Tuple { fields: [..], .. }`           | `(<op0>, <op1>, ...)`                                                                                          |
| `Struct(struct_ty)` tuple-variant      | `Ty::Struct { item, args, layout }`        | `<render_def_path(item)><render_generic_args(args)>(<op0>, <op1>, ...)`                                        |
| `Struct(struct_ty)` struct-variant     | `Ty::Struct { item, args, layout }`        | `<render_def_path(item)><render_generic_args(args)> { <field0_name>: <op0>, <field1_name>: <op1>, ... }`       |
| `Enum(enum_ty, variant_idx)` C-style   | `Ty::Enum { item, args, layout }`          | `<render_def_path(item)><render_generic_args(args)>::<variant_name>`                                           |
| `Enum(enum_ty, variant_idx)` tuple-variant | `Ty::Enum { item, args, layout }`      | `<render_def_path(item)><render_generic_args(args)>::<variant_name>(<op0>, <op1>, ...)`                         |
| `Enum(enum_ty, variant_idx)` struct-variant | `Ty::Enum { item, args, layout }`     | `<render_def_path(item)><render_generic_args(args)>::<variant_name> { <field0_name>: <op0>, ... }`              |
| `Union(union_ty, field_idx)`           | `Ty::Union { item, args, layout }`         | `<render_def_path(item)><render_generic_args(args)> { <field_name>: <op0> }`                                   |
| `Closure(closure_ty)`                  | `Ty::Closure(item, args)`                  | `<render_ty(closure_ty)> { <upvar0_name>: <op0>, ... }` (closure-captures struct's upvar names via parallel `find_nominal(NominalKind::Struct, item, args)` per `spec.md` §13.1) |
| `RawPtr(pointee_ty, Mutability::Not)`  | constructed type is `*const pointee_ty`    | `(<op_addr>, <op_meta>) as *const <render_ty(pointee_ty)>`                                                      |
| `RawPtr(pointee_ty, Mutability::Mut)`  | constructed type is `*mut pointee_ty`      | `(<op_addr>, <op_meta>) as *mut <render_ty(pointee_ty)>`                                                        |

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

| Variant                                                      | Rendered form                                                                                  |
|--------------------------------------------------------------|------------------------------------------------------------------------------------------------|
| `Assign(place, rvalue)`                                      | `<render_place(place, body)> = <render_rvalue(rvalue, body)>;`                                 |
| `SetDiscriminant(place, variant_idx)`                        | `discriminant(<render_place(place, body)>) = <variant_name>;`                                  |
| `Intrinsic(NonDivergingIntrinsic::Assume(operand))`          | `assume(<render_operand(operand, body)>);`                                                     |
| `Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping{src,dst,count})` | `copy_nonoverlapping(<render_operand(src, body)>, <render_operand(dst, body)>, <render_operand(count, body)>);` |

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

| Variant                                                  | Head text                                                                                  |
|----------------------------------------------------------|--------------------------------------------------------------------------------------------|
| `Goto { target }`                                        | `goto`                                                                                     |
| `SwitchInt { discr, targets }`                           | `switchInt(<render_operand(discr, body)>)`                                                 |
| `Return`                                                 | `return`                                                                                   |
| `Unreachable`                                            | `unreachable`                                                                              |
| `Call { callee, args, destination, .. }`                 | `<render_place(destination, body)> = <render_call_callee(callee, body)>(<arg0>, <arg1>, ...)` |
| `Assert { cond, expected, msg, .. }`                     | `assert(<expected>, <render_operand(cond, body)>, <render_assert_kind(msg, body)>)`         |

`<expected>` renders as `true` or `false` literal
matching the `Assert.expected: bool` field.

### 7.7 `render_call_callee(&CallCallee, &Body) -> String`

```rust
enum CallCallee { Direct(InstanceId), Indirect(Operand) }
```

| Variant            | Rendered form                                                                  |
|--------------------|--------------------------------------------------------------------------------|
| `Direct(id)`       | `<render_def_path(instance.item)><render_generic_args(instance.generic_args)>` |
| `Indirect(opand)`  | `<render_operand(opand, body)>`                                                |

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
`render_operand` (for Operand-typed fields) or as
decimal / value literal (for non-Operand fields per
the variant schema).

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
| `render(&self)`                     | stage 14          | -                            | String  | top-level model dump; iterates `model.instances`           |
| `render_instance(InstanceId)`       | stage 14          | InstanceId                   | String  | composes `render_item` + per-Instance content              |
| `render_const(ConstId)`             | stage 14          | ConstId                      | String  | per-variant Const value (composes `render_ty`)             |
| `render_item(ItemId)`               | stage 13          | ItemId                       | String  | Item-level wrapper (attribute block + kind-keyword + path) |
| `render_def_path(ItemId)`           | stage 12          | ItemId                       | String  | one-line cache lookup on `Item.def_path.rendered`          |
| `render_def_path_value(&DefPath)`   | stage 1           | &DefPath                     | String  | value-level helper for stage-1-shipped uses                |
| `render_ty(TyId)`                   | stage 13          | TyId                         | String  | 19-variant `Ty` enum dispatch                              |
| `render_generic_args(&GenericArgs)` | stage 13          | &GenericArgs                 | String  | `<T1, T2, K1>` form; helper                                |
| `render_place(&Place, &Body)`       | stage 13          | &Place, &Body                | String  | head local + projection chain                              |
| `render_operand(&Operand, &Body)`   | stage 13          | &Operand, &Body              | String  | Copy / Move / Constant dispatch                            |
| `render_rvalue(&Rvalue, &Body)`     | stage 13          | &Rvalue, &Body               | String  | 9-variant Rvalue dispatch                                  |
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
[Operand::Constant(<ConstId for 1u8>),
Operand::Constant(<ConstId for 2u8>),
Operand::Constant(<ConstId for 3u8>)])`.
Output: `[1_u8, 2_u8, 3_u8]`.

**Specimen 42: Aggregate Tuple arity-2.**

Input: `Rvalue::Aggregate(AggregateKind::Tuple(<tuple TyId>),
[Operand::Constant(<ConstId for 42i32>), Operand::Constant(<ConstId for true>)])`.
Output: `(42_i32, true)`.

**Specimen 43: Aggregate Struct (struct-variant).**

Input: `Rvalue::Aggregate(AggregateKind::Struct(<Point TyId>),
[Operand::Constant(<ConstId for 0i32>), Operand::Constant(<ConstId for 1i32>)])`
where `<Point TyId>` resolves to `Ty::Struct { item: <Point ItemId>,
args: GenericArgs::empty(), layout: ... }` with fields named `x`, `y`.
Output: `geom::Point { x: 0_i32, y: 1_i32 }`.

**Specimen 44: Aggregate Enum tuple-variant.**

Input: `Rvalue::Aggregate(AggregateKind::Enum(<Option<u32> TyId>, VariantIdx(1)),
[Operand::Constant(<ConstId for 42u32>)])` where `<Option<u32> TyId>`
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
[Operand::Constant(<ConstId for 0xFFu8>)])` where `<MyUnion TyId>`
resolves to `Ty::Union { item: <MyUnion ItemId>, args: GenericArgs::empty(),
layout: ... }`; field 1 is `byte`.
Output: `pl011::regs::MyUnion { byte: 0xFF_u8 }`.

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

Input: `Statement { kind: Assign(Place { local: LocalId(3), projection: [] }, Rvalue::Use(Operand::Constant(<ConstId for 42i32>))), span }`.
Output (default opts): `_3 = 42_i32;`.

**Specimen 49: SetDiscriminant.**

Input: `Statement { kind: SetDiscriminant(Place { local: LocalId(3), projection: [] }, VariantIdx(1)), span }`
where _3's type is Option<i32> (variant 1 named "Some").
Output: `discriminant(_3) = Some;`.

**Specimen 50: Intrinsic Assume.**

Input: `Statement { kind: Intrinsic(NonDivergingIntrinsic::Assume(Operand::Copy(<_5>))), span }`.
Output: `assume(copy _5);`.

**Specimen 51: Intrinsic CopyNonOverlapping.**

Input: `Statement { kind: Intrinsic(NonDivergingIntrinsic::CopyNonOverlapping {
    src: Operand::Move(<_3>), dst: Operand::Move(<_4>), count: Operand::Constant(<ConstId for 16usize>) }), span }`.
Output: `copy_nonoverlapping(move _3, move _4, 16_usize);`.

### 9.6 `render_terminator` specimens

Specimens 52-58 show the output of `render_terminator`
itself (no trailing `;`). The trailing `;` visible in
`render_basic_block` specimens (§9.8) is appended by
the composer.

**Specimen 52: Goto.**

Input: `Terminator { kind: Goto { target: BlockId(3) }, span }`.
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
    unwind: ...
}, span }`.
Output: `_0 = pl011::write_byte<u8>(move _3, copy _4) -> [return: bb2]`.

**Specimen 57: Call (Indirect).**

Input: `Terminator { kind: Call {
    callee: CallCallee::Indirect(Operand::Copy(<_5>)),
    args: [Operand::Move(<_3>)],
    destination: Place { local: LocalId(0), projection: [] },
    target: Some(BlockId(2)),
    unwind: ...
}, span }`.
Output: `_0 = copy _5(move _3) -> [return: bb2]`.

**Specimen 58: Assert.**

Input: `Terminator { kind: Assert {
    cond: Operand::Copy(<_3>), expected: true,
    msg: AssertKind::BoundsCheck { len: Operand::Copy(<_4>), index: Operand::Copy(<_5>) },
    target: BlockId(2),
    unwind: ...
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
        Statement { kind: Assign(<_3>, Use(Constant(42i32))), span },
        Statement { kind: Assign(<_4>, BinaryOp(Add, Copy(<_3>), Constant(1i32))), span },
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

Input: BlockId(1), `BasicBlock { statements: [], terminator: Terminator { kind: Goto { target: BlockId(2) }, span } }`.
Output:
```text
bb1: {
    goto -> [bb2];
}
```

### 9.9 `render_item` specimens

**Specimen 69: Bare function (zero attributes).**

Input: `Item { kind: ItemKind::Function, ..., linkage: Default,
inline_hint: None, link_section: None, is_unsafe: false, is_naked: false,
is_phantom_data: false, is_unsafe_cell: false, is_manually_drop: false,
opaque_handle_eligible: false, def_path: <pl011::init> }`.
Output: `fn pl011::init`.

**Specimen 70: Function with full attribute stress.**

Input: `Item { kind: ItemKind::Function, ..., linkage: NoMangle,
inline_hint: Always, link_section: Some(".text.boot"),
is_unsafe: true, is_naked: true,
is_phantom_data: false, is_unsafe_cell: false, is_manually_drop: false,
opaque_handle_eligible: true, def_path: <bsp::reset> }`.
Output:
```text
#[no_mangle]
#[inline(always)]
#[naked]
#[link_section = ".text.boot"]
#[opaque_handle]
unsafe fn bsp::reset
```

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
linkage: Default, inline_hint: None, link_section: None, is_unsafe: false, is_naked: false,
is_phantom_data: false, is_unsafe_cell: true, is_manually_drop: false,
opaque_handle_eligible: false }`.
Output:
```text
#[lang = "unsafe_cell"]
union core::cell::UnsafeCell
```

## 10. Cross-references to `spec.md`

Per-construct rendering pointers from `spec.md`'s
structural sections:

| `spec.md` section                              | Pointer                                          |
|------------------------------------------------|--------------------------------------------------|
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
| §9 Instance / InstanceKind                     | Rendering: see `pretty-printer.md` §1.3 (composer ownership) + stage-14's `render_instance` once shipped. |
