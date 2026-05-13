# rustc (internal) prior-art: pretty-printing

Primary-source pass on rustc's internal MIR + Ty
pretty-printers, framed by the stage-13 D-tag set.
Per README §2 this is a fresh pretty-printing-lens
read; the stage-3 prior-art note (`../stage-3/prior-art/`)
was framed through the architectural-decisions lens
(Q1-Q7) and is non-binding here.

All paths are relative to the pinned toolchain at
`~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/`.

## Sources

- `rustc_middle/src/mir/pretty.rs` (~2024 lines) --
  canonical MIR pretty-printer; the `--emit=mir`
  output's source.
- `rustc_middle/src/ty/print/pretty.rs` (~3533 lines)
  -- canonical TY pretty-printer.

## Per-D-tag relevance

### D-13-1 (master rendering style + method shape)

- **Style: hybrid Rust-source-style with sigil
  decorations.** Refs render `&{region }{mut }T`; raw
  pointers `*const T` / `*mut T`; arrays `[T; N]`;
  casts `place as Ty (CastKind)`; aggregates
  delegate to `fmt.debug_struct(name)` /
  `debug_tuple(name)` (a Rust-source surface but
  using stdlib formatter machinery).
  `rustc_middle/src/mir/pretty.rs:1083-1180` (Rvalue
  arms); `:1251-1260` (Operand);
  `rustc_middle/src/ty/print/pretty.rs:712`
  (`pretty_print_type` entry).
- **Method shape: per-IR-type `Debug` / `Display`
  impls.** One big match per type, no
  context-bearing printer. `Statement::fmt` lives at
  `mir/pretty.rs:805`; `TerminatorKind::fmt_head` at
  `:923` + `fmt_successor_labels` at `:1025`;
  `Place` / `PlaceRef` at `:1279` / `:1285`;
  `pre_fmt_projection` at `:1293` +
  `post_fmt_projection` at `:1316`.
- **Counter-fit for RegseqModel.** rustc's
  Debug-impl pattern works because `Local` and
  `BasicBlock` have self-contained Debug impls
  (sigils baked in). RegseqModel cannot match
  because its IDs need model lookup (Q5: context
  required); pattern (b) `PrettyPrinter` methods
  returning `String` is the natural carry-forward.

### D-13-2 (body envelope + indentation)

- **Envelope.**
  `fn $path(_1: ArgTy, _2: ArgTy) -> RetTy { $scope_tree $blocks }`
  with each block as
  `bbN: { stmt; stmt; terminator -> [edges]; }`.
  `mir/pretty.rs:539` (`write_mir_intro`);
  `:612` (`write_mir_sig`);
  `:714` (`write_basic_block`).
- **Indentation.** `INDENT = "    "` (4 spaces) at
  `mir/pretty.rs:19`. `ALIGN = 40` at `:21` reserves
  the column for trailing comments. Block-content
  indent steps to 8 spaces; nested constructs
  continue 4-space steps.
- **Direct adoption candidate.** RegseqModel's
  envelope is structurally identical (Body with
  locals + basic blocks per stage-11 §25; bbN
  blocks per stage-10 §24). 4-space INDENT is the
  rustc-converged default.

### D-13-3 (span-rendering mode)

- **Off-by-default with explicit toggle.** The
  `PrettyPrintMirOptions.include_extra_comments`
  flag (set by `-Z mir-include-spans`) gates trailing
  aligned `// span_to_diagnostic_string(...)`
  comments. `mir/pretty.rs:51-58` (option
  definition); `:736-751` (call sites guarded by
  the flag).
- **Format when on.** Trailing comment per
  statement / terminator, aligned to ALIGN=40.
- **Implication for D-13-3.** rustc's pattern is the
  most viable for RegseqModel: snapshot tests can
  toggle spans on/off without changing the
  per-renderer specs; default-off keeps insta
  baselines compact.

### D-13-4 (Item-level rendering shape)

- **rustc's MIR pretty-printer renders Body content,
  not Items.** Item rendering lives in HIR
  pretty-printer + `ty::print`. `write_mir_sig`
  (`mir/pretty.rs:612`) is the closest Function-Item
  analog -- emits the function header
  `fn $path(...) -> Ret`. Attributes (linkage,
  inline) are handled outside this file in HIR /
  attribute-printing infrastructure.
- **Implication for D-13-4.** rustc doesn't directly
  evidence the Item-level rendering shape RegseqModel
  needs (D-12-K bundle: `render_item`, ItemKind
  variant rendering, Linkage / InlineHint /
  Function-flag / lang-item-flag rendering,
  link_section, opaque_handle_eligible). Stage 13
  must synthesize from analogous patterns (HIR
  attribute prefix `#[link_section = "..."]`,
  modifier keywords `unsafe fn`, etc.).

### D-13-5 (`render_ty` per-arm specifications)

Representative arms from
`rustc_middle/src/ty/print/pretty.rs` (Rust-source-
style throughout):

- `Ref` -> `&{region }{mut }T` via
  `should_print_optional_region` for region (line
  near `:1100`-ish; `'_` and `'static` suppressed in
  MIR Refs at `:1123`); `Mutability::Not` /
  `Mutability::Mut` via `ptr_str()`.
- `Array` -> `[T; N]` (`:1037`).
- `Slice` -> `[T]` (`:1044`).
- `Tuple` -> `(T,)` for arity-1; `(T1, T2, ..)`
  otherwise (`:737`).
- `Adt` -> defers to `print_def_path` (`:796`).
  RegseqModel's analog: defers to
  `render_def_path(ItemId)` / `render_def_path_value`.
- `FnPtr` -> defers to `sig_tys.with(hdr).print(self)`
  (`:763`).
- `Dynamic` -> `dyn Trait` with optional
  `(dyn Trait + 'r)` parens (`:797`).
- `Never` -> `!` (`:736`).
- `Foreign` -> `print_def_path` (`:810`).
- Primitives (`bool`, `char`, `iN`, `uN`, `fN`,
  `str`) -> source form (`:714-717`, `:857`).
- **Default-suppression pattern.** Lifetimes
  `should_print_optional_region`-gated; `'static` in
  MIR-Refs always suppressed; `Mutability::Not` in
  RawPtr -> `*const` not `*const`-with-explicit-Not.

### D-13-6 (body-level renderer per-arm specifications)

- **Place projections via pre/post-wrap.**
  `pre_fmt_projection` (`:1293`) + `post_fmt_projection`
  (`:1316`) wrap projections around the head local.
  `Deref` -> `(*X)`. `Field` -> `(X.idx: ty)`.
  `Downcast` -> `(X as Variant)`. Per-projection
  wrapping accumulates for a chain.
  RegseqModel's adoption candidate: take the
  pre/post-wrap pattern as-is; the closed 6-variant
  `PlaceElem` enum (D-5-1) maps cleanly.
- **Aggregate / Adt rendering via stdlib
  formatter.** `:1155` -- `AggregateKind::Adt`
  defers to `fmt.debug_struct(name).field(...)
  .finish()` for struct variants; `debug_tuple` for
  tuple variants. RegseqModel can't reuse stdlib
  formatter because field/variant names need model
  lookup; the textual SHAPE (`Path::Variant {
  fname: op, ... }` / `Path::Variant(op, ...)`) is
  the carry-forward.
- **Per-variant idiom.** One big match per IR type,
  inside the type's Debug impl. RegseqModel cannot
  use this pattern (Q5 context-required); but the
  variant-by-variant arm structure is reusable as
  per-method dispatch within `PrettyPrinter`.

### D-13-7 (worked-example specimens)

- rustc has its own MIR test fixtures across the
  testsuite (in
  `~/.rustup/.../tests/mir-opt/` and similar). The
  `--emit=mir` output is implicitly stable: rustc
  pretty-printer changes break golden tests.
- No formal worked-example specimens cited within
  `mir/pretty.rs` itself; convention is well-
  established through external fixtures.
- **Implication for D-13-7.** Stage 13's
  worked-example specimens belong in
  `stage-13/spec-snapshot.md` (canonical fragments
  per public renderer) and project insta snapshots
  (per Q5).

### D-13-8 (sigil tables / ownership tables)

- **No formal documented sigil table.** Sigils
  (`_N`, `bbN`, `Local(N)`, etc.) are baked into
  per-type Debug impls. `Local::fmt` produces `_N`;
  `BasicBlock::fmt` produces `bbN`.
- **No analog to §18.2's per-method ownership
  table.** rustc's body of pretty-printer methods
  is monolithic per type; RegseqModel's per-method
  ownership table is a stage-management artifact
  (which stage owns which renderer) without a
  rustc analog.

## Key takeaway for stage 13

rustc's pretty-printer evidences (a) Rust-source-style
with sigil decorations as the converged textual choice,
(b) 4-space INDENT and the `fn ... { bbN: { ... } }`
envelope as the converged structural choice, (c)
off-by-default span rendering with toggle, and (d) the
pre/post-wrap pattern for place projections. Stage 13
can adopt all four directly. The Debug-impl method
pattern is RUST-fundamental to rustc's design (no
context required) and incompatible with RegseqModel
(Q5 + ID resolution); RegseqModel takes rustc's textual
choices but adopts rustc_public's structural pattern
(free helpers on a context-bearing printer).

## Cross-stage drift / counter-evidence

None substantive. rustc's evidence aligns with Charon's
Rust-source-style precedent (per stage-3 D14) and the
project's existing `render_def_path_value` precedent;
the only friction is the Debug-impl method pattern
which is incompatible with the project's Q5 context-
required printer shape, but this is structural not
textual.
