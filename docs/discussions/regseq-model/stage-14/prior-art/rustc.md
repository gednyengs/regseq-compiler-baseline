# Rustc prior art for D-14-1 (Q7 intrinsic recognition)

Evidence pass against rustc-src at the pinned toolchain
`nightly-2026-02-01-aarch64-apple-darwin`. Citations are
`file:line` into
`~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/`.

## Q1 -- Concrete rustc API for walking callee attributes

**Recommended: `tcx.get_attrs_by_path(did, &[sym::rseq, sym::intrinsic])`.**

This is the multi-segment attribute walker on `TyCtxt`,
defined at `rustc_middle/src/ty/mod.rs:1728-1739`. It calls
`Attribute::path_matches` per attribute, which for
`Attribute::Unparsed` compares the full path segment
sequence (`rustc_hir/src/hir.rs:1351-1356`). Custom
tool-namespace attributes ALWAYS land in
`Attribute::Unparsed` -- see the enum doc comment at
`rustc_hir/src/hir.rs:1265-1267` ("An attribute that could
not be parsed, out of a token-like representation. This is
the case for custom tool attributes"). For
`#[rseq::intrinsic("NAME")]`, segments are
`[sym::rseq, sym::intrinsic]`, so `get_attrs_by_path` is
the exact API.

**Ranked alternatives:**

- `tcx.get_attrs(did, sym::foo)` at
  `rustc_middle/src/ty/mod.rs:1707-1713`: single-segment
  only -- `has_name` at `rustc_hir/src/hir.rs:1502` only
  matches when the attribute path has exactly one segment.
  WRONG hook for two-segment tool paths.

- Manual `tcx.get_all_attrs(did).iter()` walk + match on
  `Attribute::Unparsed` + manual `path_matches` call. This
  is what `get_attrs_by_path` does internally; using it
  directly buys nothing.

- `find_attr!` macro at `rustc_hir/src/attrs/mod.rs:34-54`:
  ONLY matches `Attribute::Parsed(...)` arms (see line 44).
  Skips `Attribute::Unparsed` entirely. UNUSABLE for tool
  attributes -- this is the macro `get_all_attrs`' doc
  comment recommends, but it is implicitly scoped to
  built-in attributes.

- `rustc_resolve` tool-namespace machinery: relevant only
  for resolving the tool prefix at parse / expansion time
  (see Q3). Not directly callable from a query consumer;
  the bridge consumes the **result** via
  `get_attrs_by_path` after resolution has stamped the
  attribute into HIR as `Attribute::Unparsed` with the
  parsed `AttrPath`.

**Trade-off summary:** `get_attrs_by_path` is the natural
high-level API for this case; alternatives are either the
wrong hook (`get_attrs`, `find_attr!`) or strictly more
work (manual walks).

## Q2 -- Verifying Finding #1 about `intrinsic_raw`

**Confirmed: `tcx.intrinsic_raw(did)` matches
`#[rustc_intrinsic]`-marked items ONLY, not tool-namespace
attributes.**

Query declaration:
`rustc_middle/src/query/mod.rs:2244-2248`
(`query intrinsic_raw(def_id: DefId) -> Option<rustc_middle::ty::IntrinsicDef>`).

Provider:
`rustc_middle/src/ty/util.rs:1685-1701`. The function body
gates everything on
`tcx.has_attr(def_id, sym::rustc_intrinsic)` (line 1686).
`sym::rustc_intrinsic` is a single-segment built-in
attribute name; `has_attr` delegates to `get_attrs` (single
segment) per `rustc_middle/src/ty/mod.rs:1751-1753`.

`IntrinsicDef.name` is populated by `tcx.item_name(def_id)`
(line 1694) -- this is the Rust item's *name*, e.g.,
`size_of` for `core::intrinsics::size_of`. It is NOT the
argument of a `#[rseq::intrinsic("NAME")]` attribute. There
is no codepath in this provider that inspects custom
tool-namespace attributes.

Provider registration:
`rustc_middle/src/ty/util.rs:1703-1711`.

External-review Finding #1 stands as written: stage 14's
recognition mechanism MUST NOT use `intrinsic_raw`; that
API is scoped to rustc-internal compiler intrinsics.

## Q3 -- `#![register_tool(NAME)]` handling

**Registry construction.** Provider at
`rustc_resolve/src/macros.rs:123-160`
(`registered_tools_ast`). Walks crate-level attributes
matching `sym::register_tool` (line 133), extracts
identifier arguments (line 134-152), and adds them to a
`RegisteredTools` set. Predefined tools (`clippy`,
`rustfmt`, `diagnostic`, `miri`, `rust_analyzer`) are
implicitly registered (lines 155-158).

**Registry type.** `pub type RegisteredTools =
FxIndexSet<Ident>` at
`rustc_lint_defs/src/lib.rs:831`.

**Query access.** `query registered_tools(_: ()) -> &'tcx
ty::RegisteredTools` at
`rustc_middle/src/query/mod.rs:182-186`. Available to the
bridge via `tcx.registered_tools(())`.

**How unregistered tool paths are handled.** Name
resolution treats a registered tool name as
`Res::ToolMod` (`rustc_resolve/src/lib.rs:1676-1684`). When
a multi-segment attribute path's leading segment resolves
to a registered tool name, rustc rewrites the resolution
to `Res::NonMacroAttr(NonMacroAttrKind::Tool)` and stops
resolving subsequent segments
(`rustc_resolve/src/ident.rs:1896-1904`). This is why
`#[rseq::intrinsic(...)]` does NOT require `intrinsic` to
exist as a real path inside `rseq` -- the tool prefix
short-circuits the rest of the path.

If the leading segment is NOT registered (no
`#![register_tool(rseq)]` in scope), the tool prefix lookup
in `ToolPrelude` fails
(`rustc_resolve/src/ident.rs:691-694`, returns
`Determinacy::Determined`), and the attribute's path
resolution fails as a normal unresolved path -- rustc
emits an unresolved-import / cannot-find-attribute error.
**The bridge does NOT need to verify registration
explicitly**: any `#[rseq::intrinsic(...)]` attribute that
survives to MIR has already passed rustc's resolution
machinery, which guarantees `rseq` was registered. The
bridge can trust that `Attribute::Unparsed` instances with
the matching path are well-formed at the resolution level.

This matches the project's "no defensive fallbacks" rule
(README §2): no need to re-check `tcx.registered_tools(())`
for the `rseq` ident before walking attributes.

## Q4 -- Parsing the string-literal argument

**Parsed shape.** `Attribute::Unparsed(Box<AttrItem>)` at
`rustc_hir/src/hir.rs:1257-1268`. `AttrItem` fields at
`rustc_hir/src/hir.rs:1237-1247`: `path: AttrPath`,
`args: AttrArgs`, plus diagnostic fields.

**`AttrArgs`** at `rustc_hir/src/hir.rs:1186-1198`:
- `Empty` -- `#[attr]` (no args).
- `Delimited(DelimArgs)` -- `#[attr(...)]` /
  `#[attr[...]]` / `#[attr{...}]`. **This is the shape
  for `#[rseq::intrinsic("NAME")]`.**
- `Eq { eq_span, expr: MetaItemLit }` --
  `#[attr = "value"]`.

**Extraction path -- recommended idiom.** Use
`attr.meta_item_list() -> Option<ThinVec<MetaItemInner>>`
at `rustc_hir/src/hir.rs:1312-1323` (returns `Some` only
for `Delimited` args; calls
`MetaItemKind::list_from_tokens` on the inner token
stream). Then from each `MetaItemInner`:

- `.lit() -> Option<&MetaItemLit>` at
  `rustc_ast/src/attr/mod.rs:672-677` -- returns `Some`
  for the bare-literal arm
  `MetaItemInner::Lit(MetaItemLit)`
  (`rustc_ast/src/ast.rs:597-605`).
- Then match on `lit.kind` against
  `LitKind::Str(symbol, _style)` from
  `rustc_ast/src/ast.rs:2250-2253` to extract the
  `Symbol`.

For `#[rseq::intrinsic("NAME")]`: `meta_item_list()`
returns `Some(vec![MetaItemInner::Lit(MetaItemLit { kind:
LitKind::Str(sym, _), .. })])`; the bridge takes the
single element, calls `.lit()`, matches `LitKind::Str`.

**Error shapes for malformed payloads.**

- `#[rseq::intrinsic]` (no args, `AttrArgs::Empty`):
  `meta_item_list()` returns `None`. Bridge rejects.
- `#[rseq::intrinsic = "NAME"]`
  (`AttrArgs::Eq { .. }`): `meta_item_list()` returns
  `None`. Bridge rejects.
- `#[rseq::intrinsic(42)]` or `#[rseq::intrinsic(foo)]`
  (non-string literal or non-literal inner item): the
  outer list parses fine, but the `MetaItemInner` is not
  `Lit(MetaItemLit { kind: LitKind::Str(...), .. })`.
  Bridge rejects with diagnostic on the unexpected shape.
- `#[rseq::intrinsic("NAME", extra)]` (multiple args):
  `meta_item_list()` returns the full
  `ThinVec` (length > 1). Bridge inspects length and
  rejects if not exactly 1.

All four error shapes converge on a single check: "exactly
one inner item, and that item is `Lit` with
`LitKind::Str`." Rustc itself does NOT validate this for
tool attributes -- the attribute is `Unparsed` precisely
because rustc has no schema for custom tool payloads.
Validation is wholly the bridge's responsibility. This is
consistent with the "no defensive fallbacks" rule: the
bridge produces precise diagnostics; it does not silently
accept malformed forms.

## Q5 -- Optional prior-art notes

Skipped. Clippy / rustfmt / rust-analyzer source is not
included in the `rust-src` bundle; per-tool evidence
belongs in dedicated `prior-art/<project>.md` files when
those passes are scoped. The four primary questions are
answered above without needing the optional comparison.
