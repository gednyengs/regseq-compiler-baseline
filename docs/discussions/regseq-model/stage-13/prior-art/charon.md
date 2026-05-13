# Charon prior-art: pretty-printing

Primary-source pass framed by stage-13's eight D-tags. Charon is
the load-bearing precedent for Rust-source-style rendering of an
owned post-MIR IR.

## Sources

- `charon/src/pretty/mod.rs:1-5` -- thin re-export.
- `charon/src/pretty/formatter.rs:1-227` -- `AstFormatter` trait
  and `FmtCtx<'a>` struct; the context that carries the
  `TranslatedCrate`, generics binding stack, locals, and indent
  level.
- `charon/src/pretty/fmt_with_ctx.rs:1-1874` -- the per-construct
  `FmtWithCtx` impls. Largest of the three.
- `charon/src/common.rs:3` -- `pub static TAB_INCR: &str = "    "`
  (4 spaces).
- `charon/src/ids/mod.rs:33-34` -- `to_pretty_string` macro:
  `"@{pretty_name}{idx}"`.
- `charon/src/ast/values.rs:15` -- `LocalId` registered with empty
  pretty-name, so locals render `@N`.
- `charon/src/ast/ullbc_ast.rs:9` -- `BlockId` registered with
  pretty-name `"Block"` (so its `to_pretty_string` is `@BlockN`),
  but body-rendering hard-codes `bb{N}`.
- Specimen: `charon/tests/ui/comments.out:1-524` (LLBC final
  output of the `comments.rs` UI test).

## Per-D-tag relevance

### D-13-1 (master rendering style + method shape)

Rust-source-style with sigil decorations, confirming the D14
lean. Method shape: a `FmtWithCtx<C>` trait + a `WithCtx<'a, C, T>`
wrapper struct that implements `Display`
(`fmt_with_ctx.rs:18-47`). Callers write `x.with_ctx(ctx)` to get
a `Display`able. Context is `FmtCtx { translated, generics,
locals, indent_level }` (`formatter.rs:101-109`). This is
**different** from RegseqModel's stage-1-shipped `PrettyPrinter`-
methods-returning-`String` shape, and different from rustc's
Display-impl-on-IR-types. Strongest candidate-(b) evidence: the
existing project shape can be retained while still producing
Charon-equivalent textual output.

### D-13-2 (body envelope + indentation)

INDENT = 4 spaces (`common.rs:3`). The ULLBC body envelope
(`fmt_with_ctx.rs:709-728`) is exactly the rustc-converged shape:
locals printed first as `let v: T; // {return | arg #N | local |
anonymous local}` lines, then a blank line, then `bb{N}: {\n
stmts;\n  terminator;\n}` blocks. The function-decl wrapper
(`fmt_with_ctx.rs:412-457`) emits `fn $path<generics>(args) ->
Ret\n{\n  body\n}`. Block sigil `bb{N}` matches RegseqModel.

### D-13-3 (span-rendering mode)

**Charon does not pretty-print spans.** No `Display`/`FmtWithCtx`
impl for `Span` in the pretty module; the only span surface
`Display` is `Loc` at `fmt_with_ctx.rs:887-891` (renders
`{line}:{col}`), but it is not invoked from any body or item arm.
This matches rustc_public's omission and contradicts the rustc-
internal opt-in. Strong evidence for candidate (a) "off by
default" -- two of three peers omit entirely.

### D-13-4 (Item-level rendering shape)

`ItemMeta::fmt_item_intro` (`fmt_with_ctx.rs:833-861`) emits the
header sequence: optional `// Full name: $path` comment when
short_name differs, optional `#[lang_item("name")]` annotation,
indent, optional `pub` prefix, then `keyword name`.
`FunDecl::fmt_with_ctx` (`fmt_with_ctx.rs:412-458`) selects
`unsafe fn` vs `fn` from `signature.is_unsafe` and lays out
generics + args + return + body. **Charon does not surface
linkage / inline / link_section / no_mangle / naked / opaque-
handle markers** -- those fields exist on `attr_info` but are not
rendered. Specimen `comments.out:1-50` shows the actual shape:
`#[lang_item("Default")]\npub trait Default<Self>` and `pub fn
core::default::Default::default<Self>() -> Self`. For RegseqModel's
broader Item-level metadata (D-12-1.3), Charon evidence covers
only `unsafe`, `pub`, and the lang-item annotation -- partial
support for candidate (a) Rust-source attribute syntax (`#[...]`).

### D-13-5 (`render_ty` per-arm specs)

`Ty::fmt_with_ctx` at `fmt_with_ctx.rs:1711-1758`:
- `Adt(tuple)` → `(T, U, V)`; `Adt(non-tuple)` → `Path<args>`.
- `TypeVar(id)` → bound-var name.
- `Literal` → `bool` / `u32` / `f32` / `char` / etc.
- `Never` → `!`.
- `Ref(r, ty, Shared)` → `&{r} ({ty})`; `Ref(r, ty, Mut)` →
  `&{r} mut ({ty})`. **Note parens around the referent.**
- `RawPtr(ty, Shared)` → `*const {ty}`; `Mut` → `*mut {ty}`.
- `TraitType(tref, name)` → `{tref}::{name}`.
- `DynTrait(pred)` → `dyn ({pred})`.
- `FnPtr(io)` → binder-form `(inputs) -> output`.
- `FnDef(binder)` → `for<regions> {value}` when regions present.
- `Error(msg)` → `type_error("{msg}")`.

### D-13-6 (body-level renderer per-arm specs)

- **Place** (`fmt_with_ctx.rs:972-1023`): Local → `@N` or
  `name@N`; Deref → `*({sub})`; Field(Adt) → `({sub} as variant
  V).f`; Field(Tuple) → `({sub}).{idx}`; Index → `({sub})[off]`;
  Subslice → `({sub})[from..to]`. **Always parens around
  subplace** -- different from rustc's pre/post pattern.
- **Operand** (`:931-939`): `copy ({p})`, `move ({p})`,
  `const ({c})`. Explicit `const` keyword wrapper -- different
  from rustc's bare literal.
- **Rvalue** (`:1146-1245`): `Use` delegates; `Ref` →
  `&p`/`&mut p`/`&two-phase-mut p`/`&uniq p`/`&shallow p`;
  `RawPtr` → `&raw const p`/`&raw mut p`; `BinaryOp` → infix
  `x op y`; `Aggregate(Adt)` → Rust-struct-literal `Path { f: op,
  ... }` (or `Path::Variant { f: op, ... }` for enum variants);
  `Aggregate(Tuple)` → `(op, op, ...)`; `Aggregate(Array)` →
  `[op, op, ...]`; `Discriminant(p)` → `@discriminant(p)`;
  `Repeat(op, _, n)` → `[op; n]`.
- **Statement** (`:1267-1311` ULLBC, `:1314-1437` LLBC):
  `place := rvalue`, `@discriminant(p) := variant_id`, `drop p`,
  `storage_live(p)` / `storage_dead(p)`, `nop`, `assert(...)`.
  Comments before a statement render as preceding `// ...` lines.
- **Terminator** (`:1439-1478`): `goto bb{N}`,
  `if discr -> bb{T} else -> bb{F}`, `switch discr -> {v: bbN,
  ..., otherwise: bbN}`, `return`, `unwind_continue`, plus call.

### D-13-7 (worked-example specimens)

`charon/tests/ui/*.out` is a UI-test snapshot framework: each
`<file>.rs` produces `<file>.out` containing the LLBC pretty-
print, and the test-runner diffs against the committed `.out`
file. `comments.out:1-524` is a 200+-line specimen exercising
every body-level construct. Per `CONTRIBUTING.md:84` this is the
primary mode of regression testing in Charon. Strong precedent
for candidate "snapshot every public renderer with a worked
example".

### D-13-8 (sigil tables / ownership tables)

Charon does not maintain a centralized sigil table or per-method
ownership table. Sigils are hard-coded in the relevant arms
(`bb{N}` literal in `:721`, `:1447`, `:1471`; `@N` via
`to_pretty_string`); ownership is implicit in the `FmtWithCtx`
impl distribution. Charon offers no positive evidence for the
RegseqModel §18.1 / §18.2 review work.

## Key takeaway for stage 13

Charon strongly supports D-13-1 candidate (a) Rust-source-style
and D-13-3 candidate (a) span-omission. It supports the rustc-
converged 4-space INDENT and the `bb{N}` block sigil. Aggregate-
Adt rendering as `Path { f: op, ... }` is a clean Rust-source-
style precedent worth adopting. Charon's local-sigil choice
(`@N`) **differs** from RegseqModel's pinned `_N`; the project
should keep `_N` (rustc-converged + already in `spec.md` §18.1)
and treat Charon's `@N` as evidence-of-divergence rather than
something to import. Charon's two structural choices RegseqModel
should not import: (1) Display-via-wrapper trait (vs the project's
already-shipped `PrettyPrinter` methods); (2) parens-around-every-
subplace -- rustc's pre/post pattern is more readable for nested
projections.

## Cross-stage drift / counter-evidence

- Charon's `Operand` renders the `Const` arm as `const ({c})`
  with an explicit `const` keyword wrapper. Rustc and rustc_public
  render constants bare (no `const` prefix). Stage 13 should make
  this call explicitly under D-13-6 -- it's a small choice with
  noticeable impact on snapshot density.
- Charon does not surface `linkage` / `inline_hint` /
  `link_section`. RegseqModel's D-12-1.3 typed Item fields require
  rendering decisions that have no direct Charon precedent;
  `#[link_section = "..."]` / `#[inline(always)]` are natural
  Rust-source-style extensions but stage 13 must invent them
  rather than inherit.
- Charon's `LocalId` pretty-name is **empty** (sigil `@N`). The
  in-project sigil-conventions doc-comment at
  `crates/regseq-model/src/pretty.rs:22-34` already pins `_N`
  (matching rustc and stage-1 shipped). No drift; recording the
  divergence so D-13-2's locals-block rendering does not silently
  follow Charon.
