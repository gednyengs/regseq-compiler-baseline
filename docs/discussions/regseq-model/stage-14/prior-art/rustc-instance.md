# Rustc prior art -- instance rendering

Evidence collected for **D-14-5** (`render_instance(InstanceId) -> String`
composer renderer; per-variant specs across the closed 6-variant
`InstanceKind` enum).

Toolchain: `nightly-2026-02-01-aarch64-apple-darwin`. Paths are
relative to:
`~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/`

The 6 variants under design (per `spec-snapshot.md` §9.4):
`Function { fn_sig: FnSigId, body: Option<Body>, flavor:
FunctionInstanceFlavor, virtual_idx: Option<u32> }` /
`Static { ty: TyId, init: Option<ConstId> }` /
`Const { ty: TyId, value: ConstId }` /
`Struct { layout: TyId }` / `Enum { layout: TyId }` /
`Union { layout: TyId }`.

## Q1 -- Per-Instance dispatch surface: does rustc render Instances or Items?

**Finding.** Rustc has **two distinct per-emission-unit dispatch
surfaces** with different rendering coverage:

1. **MIR pretty-printer** (`rustc_middle/src/mir/pretty.rs`) renders
   **body-bearing** entities only. The top-level entry is
   `write_mir_pretty(tcx, single, w)` at `pretty.rs:317`. It walks
   `dump_mir_def_ids(tcx, single)` = `tcx.mir_keys(())` at
   `pretty.rs:701`-`707` -- i.e., only `DefId`s that have MIR. For
   each, it either renders a trivial-const fast path
   (`const PATH: TY = const VAL;`, one-liner at `pretty.rs:356-361`)
   or fetches a body via `tcx.instance_mir(InstanceKind::Item(did))`
   and dispatches to `write_mir_fn`.

   `write_mir_fn` at `pretty.rs:384-399` is the canonical body
   envelope: `write_mir_intro` (`pretty.rs:539`) emits sig + `{` +
   scope tree, then per-block `write_basic_block`, then `}`, then
   `write_allocations`.

   The signature emitter `write_mir_sig` at `pretty.rs:612` dispatches
   on `DefKind`:
   - `DefKind::Const | DefKind::AssocConst` -> `const ` prefix.
   - `DefKind::Static { mutability: Not }` -> `static ` prefix.
   - `DefKind::Static { mutability: Mut }` -> `static mut ` prefix.
   - Promoteds -> `const ` prefix.
   - Function-like (`Fn / AssocFn / Ctor / SyntheticCoroutineBody` or
     `is_closure_like`) -> `fn ` prefix.
   - `AnonConst | InlineConst` -> no prefix.
   - `GlobalAsm` -> no prefix (fake body).
   - Anything else -> `bug!` rejection.

   For function-like the form is `fn PATH(_1: ArgTy, _2: ArgTy) -> RetTy {`;
   for static / const it is `static PATH: Ty =` or `const PATH: Ty =`
   (the body that follows constructs the value).

2. **`MonoItem::define`** (`rustc_codegen_ssa/src/mono_item.rs:29-55`)
   is the codegen-side per-emission-unit dispatch. Three variants:
   - `MonoItem::Fn(instance)` -> `base::codegen_instance::<Bx>(cx, instance)`
     (the `codegen_mir` path at `mir/mod.rs:173`); or
     `naked_asm::codegen_naked_asm` for `CodegenFnAttrFlags::NAKED`.
   - `MonoItem::Static(def_id)` -> `cx.codegen_static(def_id)` (emit
     a global with the evaluated initializer; no MIR body walk).
   - `MonoItem::GlobalAsm(item_id)` -> `base::codegen_global_asm`.

   `predefine` (lines `57-87`) is the symbol-declaration pass; it
   skips `GlobalAsm` and skips Function bodies that are `NAKED`.

**Stable-IR analog** (`rustc_public/src/mir/mono.rs:14`):

```rust
pub enum MonoItem { Fn(Instance), Static(StaticDef), GlobalAsm(Opaque) }
```

`Instance::emit_mir` at `rustc_public/src/mir/mono.rs:178-180`:

```rust
pub fn emit_mir<W: io::Write>(&self, w: &mut W) -> io::Result<()> {
    if let Some(body) = self.body() { body.dump(w, &self.name()) } else { Ok(()) }
}
```

Body-bearing-only: if `Instance::body()` returns `None` (foreign,
extern statics, etc.), the renderer emits **nothing**. The
`body.dump(w, name)` call delegates to `function_body(w, &body, name)`
at `rustc_public/src/mir/pretty.rs:38` which renders
`fn name(_1: T, ...) -> Ret { let mut _N: T; debug X => _N; bb0: {...}; }`
verbatim.

**Implication for D-14-5.** Rustc dispatches on **DefKind** at the
signature emitter level (4 keyword-prefix shapes: `fn` / `const` /
`static` / `static mut`) and on **MonoItem variant** at the codegen
walker (3 shapes: Fn / Static / GlobalAsm). RegseqModel's
`InstanceKind` is a **6-variant superset**: it adds Struct / Enum /
Union as Instance variants because RegseqModel emits typedefs for
nominal ADTs, whereas rustc/LLVM emit them implicitly inside the
function-pointer / global-value layouts.

D-14-5 must therefore commit per-variant rendering rules across all
6 variants -- the **3 body-bearing-or-value-bearing** variants
(Function / Static / Const) plus the **3 type-def-only** variants
(Struct / Enum / Union). The latter has **no rustc precedent**;
prior art is consulted only for the first 3.

## Q2 -- Function-variant envelope: what does `render_instance` produce for `InstanceKind::Function`?

**Finding.** Per `write_mir_intro` (`pretty.rs:539-572`) +
`write_mir_sig` (`pretty.rs:612-674`), rustc's function-Body
envelope is:

```text
[promoted-or-keyword-prefix]<def_path_str>(<arg_decls>) -> <ret_ty> {
    [scope-tree: var_debug_info + let-decls + nested scopes]
    [coverage_info_hi / function_coverage_info -- skip for RegseqModel]

    bb0: { ... }
    bb1: { ... }
    ...
}
```

Where:
- The `fn` keyword decoration comes from `DefKind` dispatch (Q1)
  -- rustc does NOT thread `Safety` or `FnAbi` keywords into the
  signature emitter; the `unsafe` / `extern "C"` keyword decoration
  is omitted from rustc's MIR-pretty output entirely (rustc's MIR is
  purely a body-level IR; it has no need to surface ABI in the
  envelope).
- The arg-list iterates `body.args_iter()` (locals indexed `1..=arg_count`)
  with form `<Place::from(arg):?>: <local_decls[arg].ty>`. The
  return type comes from `body.return_ty()` (= `local_decls[0].ty`).
- The scope tree emits var-debug-info + let-decls per `write_scope_tree`
  (`pretty.rs:403-521`); RegseqModel's existing `pretty-printer-snapshot.md`
  §3.1 already pins the equivalent locals declaration block plus
  optional `debug NAME => _N;` lines per stage-13 D-13-2 + D-13-3.
- The body content is a sequence of `bbN: { ... }` blocks; rustc
  prints them in `body.basic_blocks.indices()` order (post-cleanup
  natural ordering). Empty lines separate consecutive blocks. The
  final `}` closes the envelope on its own line.

**Stable-IR analog** `function_body(writer, body, name)` at
`rustc_public/src/mir/pretty.rs:38-88`:

```rust
write!(writer, "fn {name}(")?;
// arg_locals: "_<i+1>: <ty>" comma-separated
write!(writer, ")")?;
writeln!(writer, " -> {} {{", return_local.ty)?;
// locals: skip args; "let mut _N: T;" / "let _N: T;"
// var_debug_info: "debug NAME => _N;"
// basic_blocks: "bb<i>: { stmts; terminator }"
writeln!(writer, "}}")?;
```

Notes:
- The **caller passes the `name`** -- the body itself does not know
  its identifier. The composer (in RegseqModel terms,
  `render_instance`) is responsible for resolving the path.
- rustc_public's `Instance::emit_mir` always uses the `fn` keyword;
  there is no `static` / `const` envelope path because rustc_public's
  `Static` variant doesn't have a `body()`.

**Implication for D-14-5.** The Function envelope shape is well-pinned
by precedent. RegseqModel's existing `pretty-printer-snapshot.md` §3.1
already captures the body-internal envelope (locals + debug + bbs);
D-14-5's contribution is to **compose** that with:
- The Item-level attribute block (the `#[no_mangle]`, `#[inline]`, ...
  stack per §5.3) + domain-attribute prefix (see Q4 below).
- The kind-keyword + path + fn_sig-derived signature decoration:
  `[unsafe ][extern "ABI" ]fn <render_def_path(item)>(arg-list) -> <ret_ty>`.
- The body envelope `{ ... }` produced from `body: Some(b)` via
  stage-13 D-13-2's locals-declaration block + body-level renderers.
- For `body: None` (foreign declarations), the envelope is a
  **declaration form** -- header line terminated by `;` instead of
  ` { ... }`. This is the natural Rust-source shape for
  `extern "C" { fn foo(); }` post-flattening; rustc's MIR pretty-printer
  has no analog because foreign items are absent from `mir_keys`.
  RegseqModel surfaces them because they're emission units.

The load-bearing sub-questions D-14-5 must lock are listed under
"D-tag candidates" below.

## Q3 -- `unsafe` and `extern "ABI"` keyword decoration: where do they come from?

**Finding.** Rustc's MIR pretty-printer DOES NOT emit `unsafe` /
`extern "ABI"` on the function signature line (`write_mir_sig`
emits `fn ` unconditionally for function-like items). The Safety +
FnAbi data is in `body.source.instance.def`'s `tcx.fn_sig(def_id)`
but the MIR pretty-printer doesn't surface it.

In rustc's HIR/AST pretty-printers (different files; e.g.,
`rustc_ast_pretty/src/pprust/state/item.rs`) the order is:

```text
[<attrs>]
[pub ][const ][async ][unsafe ][extern "ABI" ]fn <name>(<args>) -> <ret>
```

(Visibility -> `const` -> `async` -> `unsafe` -> `extern "ABI"` ->
`fn`, mirroring the Rust grammar in the Reference.)

Locally in RegseqModel:
- `Item.is_unsafe: bool` (§8.0) carries the safety bit. Existing
  `pretty-printer-snapshot.md` §5.3 already renders this as a
  keyword prefix on the kind-keyword (`unsafe fn`).
- `FnSig.abi: FnAbi` (per stage-11 / stage-13 spec) carries the ABI.
  The §6.5 `FnPtr` arm renders `[unsafe ][extern "ABI" ]fn(<args>) -> <ret>`
  for function-pointer types using the same `unsafe` / `extern "ABI"`
  keyword decorators -- but the Item-level §5.3 / §5.4 surface DOES
  NOT carry ABI decoration (it cannot, because `render_item` has
  no Instance-context).

**Implication for D-14-5.** Either:
- (a) `render_instance` produces the Function-variant envelope
  **from scratch** (does not call `render_item`), composing
  attribute block + kind-keyword + path + signature, with full
  freedom to inject `extern "ABI"` between `unsafe` and `fn`.
  `render_item` remains the standalone Item-level view for
  diagnostics / Item-table dumps; both share the §5.3 per-flag
  rendering rules via a factored helper.
- (b) `render_item` extends to accept an optional ABI decoration
  parameter, and `render_instance` calls
  `render_item(item, Some(fn_sig.abi))` for Function instances.
  Couples §5.4 to Instance-context indirectly.

Option (a) is the cleaner layering: `render_item`'s `String`
return signature stays stable; `render_instance` owns the full
composition of the envelope; the §5.3 per-flag rule table becomes
a shared helper consumed by both.

## Q4 -- Domain-attribute weaving: where does `#[rseq::intrinsic("log-info")]` render?

**Finding.** RegseqModel's `domain_attrs.item_attrs[item_id]: Vec<AttrData>`
(§3.6.1) carries `AttrData::Intrinsic(IntrinsicKind::Log(LogLevel))`
entries for Function Items recognised as diagnostic-logging
intrinsics per stage-14 D-14-1 + D-14-2. The dash-form NAME mapping
is locked at §3.6.3 / §26.2: `log-debug` / `log-info` / `log-warn` /
`log-error`. Rust-source-style rendering is
`#[rseq::intrinsic("log-info")]` (rseq-namespace + intrinsic
selector + dash-form NAME string).

Rustc's analog: tool-namespace attributes (e.g., `#[clippy::lint]`)
render as `#[<tool>::<segment>(...)]` in the source. The rustc
internals don't have a unified per-DefId attribute-render pass --
each consumer (MIR pretty-printer, HIR pretty-printer, diagnostic
emitters) walks `tcx.get_all_attrs(did)` and renders attribute by
attribute. No precedent for ordering domain attributes vs codegen
flags.

**Implication for D-14-5.** D-14-5 must commit:

- **Ordering rule** for the attribute block. Two streams:
  (i) Item-flag-derived attributes (§5.3 hard-coded:
  `#[no_mangle]`, `#[inline]`, `#[link_section]`, `#[naked]`,
  lang-item flags, `#[opaque_handle]`), and
  (ii) `domain_attrs.item_attrs[item]` entries (each `AttrData`
  variant has its own rendered form). Rust source has no canonical
  ordering rule (the author writes them in whatever order); for a
  deterministic snapshot-stable rendering we must pin one.
  Candidates: (1) Item-flags first, domain second
  (matches rustc's CodegenFnAttrs-first historical ordering);
  (2) domain first, Item-flags second (groups Regseq-DSL
  metadata together); (3) lexical by attribute path (Rust idiomatic).

- **Per-`AttrData` variant render rule.** For stage 14 only the
  `Intrinsic(IntrinsicKind::Log(LogLevel))` arm is occupied. Render
  form: `#[rseq::intrinsic("log-<sev>")]` where `<sev>` is the
  dash-form NAME from §3.6.3 `IntrinsicKind::from_name` reversed.
  Phase-2 stages will append per-family arms (Mmio / Interrupt /
  Atomic) with their own NAME schemata.

- **Spec home.** The attribute block stays in `pretty-printer.md`
  per §5.x or a new sub-section dedicated to domain-attribute
  rendering inside §10.2 (`render_instance`).

## Q5 -- `body: None` foreign declarations: render or skip?

**Finding.** Rustc's MIR pretty-printer **skips** foreign items
entirely (they're absent from `tcx.mir_keys`). rustc_public's
`Instance::emit_mir` returns `Ok(())` for body-less instances.
The codegen-side `predefine` does emit the symbol declaration
(`cx.predefine_fn` for non-naked Fn instances) but produces no
textual MIR output.

RegseqModel cannot mirror "skip entirely" -- foreign declarations
are emission units (the C codegen produces `extern "C" int foo(int);`
declarations for them; downstream consumers need them in the model
listing). They must appear in the rendered output with the
declaration form distinct from the definition form.

**Implication for D-14-5.** Function-variant rendering must
dispatch on `body: Option<Body>`:
- `Some(b)` -> definition form: header + `{` + body envelope + `}`.
- `None` -> declaration form: header + `;`.

The `Item.linkage == Linkage::Foreign` attribute (§5.3 renders as
`#[foreign]`) is already in the attribute block; the absence of a
body further confirms the declaration shape. Per §3.1 invariant 10
(`spec-snapshot.md`), `body: None` <-> Item is foreign (a model
biconditional), so the two signals are redundant -- but the
rendering rule should dispatch on `body` (the post-bridge structural
data) rather than re-deriving from `Item.linkage`.

## Q6 -- Static / Const variants: composer-side value-rendering shape

**Finding.** Rustc's MIR pretty-printer treats Static / Const
bodies as **Body**s that construct the value -- the MIR is
literally a program that returns the const value. The envelope is
`static PATH: Ty = { let _0: Ty; ... bb0: { ... return; } }`.
This is correct for rustc (CTFE runs the body to get the value)
but lossy for RegseqModel because:
- RegseqModel has already evaluated the constant at the bridge --
  per Q2 of `architecture.md`, `'tcx` does not survive past the
  bridge call, and per stage-4 the result is a `ConstId` referring
  to a `Const` value in `model.consts`.
- The composer-side rendering should surface the **evaluated**
  value, not a body that re-computes it.

The natural form for RegseqModel:

```text
[<attrs>]
static [mut ]PATH: <render_ty(ty)> = <render_const(init.unwrap(), ty)>;
```

```text
[<attrs>]
const PATH: <render_ty(ty)> = <render_const(value, ty)>;
```

For `Static { init: None }` (foreign extern statics, e.g.,
`extern "C" { static FOO: u32; }`):

```text
[<attrs>]
static [mut ]PATH: <render_ty(ty)>;
```

`render_const` is the D-14-4 composer renderer, recently locked at
Phase B R4 APPROVE. The pair-form `render_const(id, ty)` is exactly
what Static / Const variant rendering needs.

**Implication for D-14-5.** Static / Const variants are
**single-line composers** -- attribute block + kind-keyword + path
+ `: <ty>` + (` = <const>` for definition, nothing for foreign) +
`;`. No body envelope. The mutability prefix for `Static` comes
from `Item.kind == ItemKind::Static { mutability }` per stage-12
D-12-1.2 (c) -- §5.4 already renders this as `static $path` or
`static mut $path`; D-14-5 extends the form with `: <ty> = <const>;`.

## Q7 -- Struct / Enum / Union variants: full definition or header-only?

**Finding.** Rustc has **no precedent** for MIR-side rendering of
ADT type-defs:
- MIR is body-IR only; ADT defs are HIR/AST-level and rendered by
  HIR / AST pretty-printers (`rustc_ast_pretty`, `rustc_hir_pretty`)
  which sit outside the post-mono rendering surface.
- rustc_public's `MonoItem` enum excludes ADT defs entirely (only
  `Fn / Static / GlobalAsm`); the codegen-side `MonoItem::define`
  similarly excludes them (LLVM doesn't emit type-defs as separate
  entities; nominal types lower inline at use sites).

RegseqModel **does** include ADT defs as Instances because the C
codegen produces explicit `typedef struct PL011 { ... }` /
`typedef enum E { ... }` / `typedef union U { ... }` declarations
for them. The pretty-printer surface must therefore commit a
shape with no rustc analog.

**Two design choices:**

- **Option A: Header-only** (= §5.4 wrapper, unmodified).
  Renders just `[<attrs>]struct $path`, `[<attrs>]enum $path`,
  `[<attrs>]union $path`. Layout body suppressed. Pros: simple;
  matches what `render_item` already gives. Cons: doesn't surface
  field/variant info; layout info is opaque in the rendered output.

- **Option B: Full definition body from layout.**
  Renders the §11 Layout-derived definition body:

  ```text
  [<attrs>]
  struct pl011::PL011<u32> {
      base: u32,
      flags: pl011::PL011Flags,
  }

  [<attrs>]
  enum pl011::Status {
      Ready,
      Busy(u32),
      Error { code: i32 },
  }

  [<attrs>]
  union pl011::Reg<u32> {
      byte: u8,
      word: u32,
  }
  ```

  Field shape resolved via §11 `LayoutKind::Struct { fields }` /
  `LayoutKind::Union { fields }`; enum variant dispatch via §11
  `EnumShape` (`Empty` / `Single` / `Multiple`) with per-variant
  field rendering reusing the §3.4 tuple-vs-named convention.
  Pros: surfaces layout structure consistently with §11 schema;
  natural Rust-source style; supports downstream diagnostics that
  need field-level visibility. Cons: significantly larger spec
  surface; need to commit discriminant rendering (explicit
  `#[repr(...)]` per the existing Item-attr stream vs implicit) +
  `EnumShape::Empty` rendering (probably `enum Foo {}` no body) +
  packed/aligned `repr` decorators if present.

**Implication for D-14-5.** This is a **load-bearing architectural
decision** with no prior-art tiebreaker. The pretty-printer's
overall design philosophy (§2.1 "Rust-source-style with sigil
decorations" per stage-13 D-13-1) leans toward Option B: rendering
a struct/enum/union as a header-only path string would feel
incongruous next to fn / static / const definitions that render
their full content. Worked-example specimens in §9 implicitly
favor Option B as well (any test snapshot for a driver fixture
would want to compare ADT layouts).

Recommend Option B as the working default with sub-questions to
lock during Phase A drafting.

## Q8 -- Synthesised Function instances: any per-flavor rendering rules?

**Finding.** Per `spec-snapshot.md` §9.4 + §13 (synthesised Item
families), bridge-synthesised Function instances cover:
- **Drop-glue** (`tcx.instance_mir(InstanceKind::DropGlue(...))`):
  rustc's MIR pretty-printer renders these inline using
  `def_path_str(did)` which yields the source-level type's path
  (e.g., `core::ptr::drop_in_place<pl011::PL011>`). The body is
  generated MIR (post-monomorphization) and renders normally.
- **VTable method shims** (`InstanceKind::VTableShim(...)`):
  similar -- rustc's pretty-printer uses `def_path_str`.
- **Closure call methods / drop glue** (`tcx.is_closure_like`):
  rustc dispatches through the same `fn ` keyword branch in
  `write_mir_sig`; closure DefPaths resolve like normal items
  (`closure#0` / `coroutine_closure_body#0` suffix segments).
- **ReifyShim / FnPtrShim / ClosureOnceShim / CloneShim /
  FnPtrAddrShim** -- rendered identically via `def_path_str`;
  rustc's MIR pretty-printer treats them as Function-bodied
  instances with their flavor encoded into the DefPath.

RegseqModel's per-Item `def_path` (stage 12 D-12-6) already encodes
the synthesised family per §13.4
(e.g., `<vtable for Trait for SelfTy>`,
`<closure_call for crate::path::closure#0>`, etc.). So
`render_def_path(item)` produces the right path string with no
per-flavor branching required.

**`flavor: FunctionInstanceFlavor`** is metadata for **instance
identity disambiguation** (per §9.3 `fn_shim_disambig`), not a
rendering decoration. Two distinct rustc-shim flavors sharing the
same `(item, args)` get distinct rendered paths because their
synthetic Items have distinct DefPaths.

**Implication for D-14-5.** No per-flavor rendering branch needed
in `render_instance`. `FunctionInstanceFlavor` is opaque to the
renderer; the DefPath is the rendering source of truth. (A future
revision could surface flavor as a trailing comment if diagnostic
need surfaces; v1 doesn't.)

## Q9 -- `virtual_idx` rendering

**Finding.** `InstanceKind::Function.virtual_idx: Option<u32>` is
populated for `flavor = FunctionInstanceFlavor::Virtual` instances
(per §9.4 construction invariant) -- it carries rustc's vtable-slot
index recovered from `ty::InstanceKind::Virtual(_, idx)`. Per the
stage-11 vtable-indirect rewrite pass (§23.6.2 B-8), virtual
instances are **rewritten** at the bridge into vtable-shim Function
Items by Phase 5 body construction. The `Virtual` flavor survives
on `InstanceKind.flavor` for identity but the renderable form is
already the rewritten shim Item.

**Implication for D-14-5.** `virtual_idx` is metadata, not a
rendering input -- the rewrite produces a synthesised shim Item
whose DefPath encodes the trait + impl + method position. The
renderer sees the shim Item via `instance.item` and renders it
normally per the synthesised-Item DefPath form. No special
rendering branch.

(Edge case: if validation surfaces a Virtual-flavor Instance with
`body: None` and a non-rewritten virtual_idx, that's a model
construction bug; render-time has no fallback per *No defensive
fallbacks*.)

## Q10 -- Per-Instance ordering when iterating `model.instances`

**Finding.** Rustc's `write_mir_pretty` walks `tcx.mir_keys` in
arbitrary order (HashMap iteration), inserting blank-line
separators between items (`pretty.rs:329-335`). codegen iteration
(`MonoItem::define` callers in `codegen_crate`) walks CGU contents
in sort-stabilised order (see `rustc_monomorphize/src/partitioning.rs`).

RegseqModel commits per Q2 to **codegen-keyed** ordering across
`model.instances` (the top-level F4 sort key). D-14-5 inherits
this -- `render_instance` is called once per InstanceId and
produces a self-contained block; **top-level ordering is D-14-6's
problem**, not D-14-5's.

**Implication for D-14-5.** No ordering rules required at D-14-5;
the composer renders one Instance at a time. The separator between
adjacent `render_instance` outputs (blank line / blank line + comment
header / etc.) is D-14-6 territory.

---

## D-tag candidates for D-14-5

Based on the evidence above, the following sub-decisions need
locking in Phase A. Allocation matches the §1.3 / §10.2 working
sketch in the existing pretty-printer-snapshot.md plus the
evidence-pass-surfaced extensions:

- **D-14-5.1 (API signature, load-bearing).**
  `render_instance(InstanceId) -> String`. Single-argument form
  -- no pair-form needed because `InstanceKind` already carries
  enough per-variant context (fn_sig, body, ty, init, value,
  layout). Mirrors rustc_public `Instance::emit_mir`'s
  `(self, w)` form.

- **D-14-5.2 (Function variant envelope, load-bearing).**
  Per-arm composition for `InstanceKind::Function { fn_sig, body,
  flavor, virtual_idx }`. Sub-questions:
  - Attribute block: composed from `Item` per-flag rules (§5.3)
    + `domain_attrs.item_attrs[item]` entries; rendered as
    multi-line `#[...]` block before the kind-keyword.
  - Signature line: `[unsafe ][extern "ABI" ]fn <render_def_path(item)>(
    _1: <render_ty(arg_0)>, ..., _N: <render_ty(arg_N-1)>) -> <render_ty(ret)>`.
    `unsafe` from `Item.is_unsafe`; ABI from `fn_sig.abi`
    (`render_ty`-style suppression for `FnAbi::Rust` default).
  - Body: dispatch on `body: Option<Body>`.
    - `Some(b)` -> ` {<newline><body envelope per §3.1 +
      stage-13 D-13-2><newline>}`.
    - `None` -> `;` (declaration form).
  - `flavor` / `virtual_idx`: not rendered (identity metadata
    only, per Q8 / Q9).

- **D-14-5.3 (Static variant).**
  `InstanceKind::Static { ty, init }`. Form:
  `[<attrs>]static [mut ]<render_def_path(item)>: <render_ty(ty)>[ = <render_const(init.unwrap(), ty)>];`.
  `init = None` (foreign extern static) -> drop the ` = ...` segment.
  Mutability from `Item.kind == ItemKind::Static { mutability }`.

- **D-14-5.4 (Const variant).**
  `InstanceKind::Const { ty, value }`. Form:
  `[<attrs>]const <render_def_path(item)>: <render_ty(ty)> = <render_const(value, ty)>;`.
  `value` is always `Some` for Const (stage-12 D-12-2 invariant);
  no None branch.

- **D-14-5.5 (ADT variants: Struct / Enum / Union, load-bearing).**
  Recommended Option B per Q7: full definition body from `layout`
  (which resolves to `Ty::Struct { layout, .. }` /
  `Ty::Enum { layout, .. }` / `Ty::Union { layout, .. }`).
  Sub-questions:
  - Struct: walk `LayoutKind::Struct { fields }`; per-field
    `<field.name>: <render_ty(field.ty)>,` per §3.4
    tuple-vs-named convention. Generic-args via
    `<render_generic_args(args, item)>` (resolved from the
    parent `Ty::Struct.args`).
  - Enum: dispatch on `EnumShape`:
    - `Empty` -> `enum <path> {}` no body.
    - `Single { variant, .. }` -> single-variant body.
    - `Multiple { variants, .. }` -> per-variant body.
    Per-variant rendering reuses §3.4 ctor conventions: unit
    `<name>`, tuple `<name>(<f.ty>, ...)`, named-field
    `<name> { <f.name>: <f.ty>, ... }`.
  - Union: walk `LayoutKind::Union { fields }`; per-field
    same shape as struct named-field.
  - `repr` decoration: surface via the Item-attr stream (the
    Item already carries lang-item / `is_phantom_data` /
    `is_unsafe_cell` / etc. flags; explicit `#[repr(C)]` /
    `#[repr(transparent)]` / `#[repr(packed)]` are not
    currently in `Item`'s typed-field set per §8.0; D-14-5.5
    must decide whether to thread a repr field through Item
    or render from layout flags). **Likely deferred** -- v1
    can omit `#[repr(...)]` entirely if no test fixture
    requires it.
  - Discriminant value rendering for Enum variants:
    rustc's source-style is `Variant = 5,`. Probably out
    of scope for v1 (discriminants live on `EnumShape`
    layout details). **Likely deferred** -- v1 renders just
    `Variant,` without discriminant.

- **D-14-5.6 (Domain-attribute weaving).**
  Per Q4. Sub-questions:
  - Ordering rule: Item-flag-derived attributes first
    (§5.3 stack), then `domain_attrs.item_attrs[item]`
    entries in `Vec<AttrData>` order per §3.6.2 deterministic
    ordering rule. (Alternative orderings rejected unless
    deemed important.)
  - Per-`AttrData::Intrinsic(IntrinsicKind::Log(LogLevel))`
    rendering: `#[rseq::intrinsic("log-<sev>")]` with dash-form
    NAME reversed from `IntrinsicKind` per §3.6.3.
  - Per-`AttrData::*` phase-2 family entries: not in
    stage-14 scope; phase-2 stages append their own
    per-variant rendering rules to §10.2.x.
  - Spec home: subsection of §10.2 (e.g., §10.2.x
    "Attribute block composition").

- **D-14-5.7 (`render_item` vs `render_instance` boundary,
  load-bearing).** Per Q3. Choice:
  - Option (a) per Q3: `render_instance` for Function variant
    produces the full envelope from scratch (does NOT call
    `render_item`); shares the §5.3 per-flag rule helper.
  - For non-Function variants (Static / Const / Struct /
    Enum / Union), `render_instance` MAY call `render_item`
    for the wrapper since no per-Instance keyword decoration
    overrides the §5.4 surface (mutability for Static is
    already on `Item.kind`; structs/enums/unions need no
    Instance-context decoration).
  - **Stage-14 D-14-5 recommendation**: composer-from-scratch
    for **all** variants for consistency; §5.3 per-flag rule
    is factored into a shared helper consumed by both
    `render_item` (standalone) and `render_instance`
    (composer). `render_item` is **not** in the call graph of
    `render_instance`.

- **D-14-5.8 (cross-stage friction).**
  No anticipated REOPEN of stage-13 D-tags from current
  evidence. The body envelope (§3.1 / D-13-2) and per-flag
  rendering table (§5.3 / D-13-4.3) are consumed as-is.
  `render_item` (§5.4 / D-13-4.5) stands -- if D-14-5.7
  factors the §5.3 helper out, that's a stage-13 spec
  refactor; the rule itself is unchanged. Monitor at Phase B
  review.

---

## Cross-references

- `pretty-printer-snapshot.md` §1.3 -- existing working sketch
  for the composer trio (renderer-ownership table).
- `pretty-printer-snapshot.md` §3.1 -- body envelope (stage-13
  D-13-2; D-14-5 consumes for Function `body: Some(_)` arm).
- `pretty-printer-snapshot.md` §5.3 -- per-flag rendering table
  (stage-13 D-13-4.3; D-14-5 consumes via shared helper).
- `pretty-printer-snapshot.md` §5.4 -- per-`ItemKind` variant
  rendering (stage-13 D-13-4.5; mutability prefix for Static
  consumed; otherwise `render_item` is parallel to
  `render_instance`, not in its call graph).
- `pretty-printer-snapshot.md` §6 -- `render_ty` per-arm specs
  (stage-13 D-13-5; D-14-5 consumes for fn_sig args/ret +
  Static/Const ty + ADT field types).
- `pretty-printer-snapshot.md` §10.1 -- `render_const` (stage-14
  D-14-4; D-14-5 consumes for Static `init` + Const `value`).
- `spec-snapshot.md` §3.6 -- DomainAttrTables (stage-14 D-14-1 +
  D-14-3; D-14-5 consumes `item_attrs` for attribute weaving).
- `spec-snapshot.md` §8 -- Item schema (D-14-5 consumes Item
  fields for attribute block + kind-keyword).
- `spec-snapshot.md` §9.4 -- InstanceKind (D-14-5 dispatches
  per-variant).
- `spec-snapshot.md` §11 -- Layout (D-14-5.5 consumes for ADT
  variant bodies).
- `spec-snapshot.md` §26 -- Diagnostic logging (D-14-5.6
  consumes intrinsic NAME mapping for AttrData::Intrinsic
  rendering).

---

## Step-6 prior-art consultation

Per stage-14 memory.md D-14-4 precedent, **Charon / mrustc /
rustc_public** prior art is **deferrable** for D-14-5: the
evidence above grounds all anticipated sub-decisions adequately
without secondary-project consultation. Two exceptions where
inline consultation may surface during Phase A:

- **D-14-5.5 ADT body rendering** -- if discriminant rendering
  or repr decoration becomes ambiguous, consult Charon's
  `charon-ml` or mrustc's HIR pretty-printer for an external
  precedent on how nominal-type defs render in post-mono IR
  dumps. Per-project findings would land at
  `stage-14/prior-art/{charon,mrustc}-instance.md` if consulted.

- **D-14-5.6 Domain-attribute ordering** -- no obvious
  precedent in any system surveyed; this is a Regseq-specific
  composition. Stage 14 commits an ordering rule de novo.
