# regseq-model-86: Item-level rendering (P1a-PP-01)

## Description

Implement the stage-13 building-block `render_item(ItemId) -> String`
on `PrettyPrinter<'_>` plus the shared `render_item_attribute_block`
helper consumed by both `render_item` (this task) and
`render_instance` (P1a-PP-05a/b). Cover the §5.3 per-flag attribute
rendering table, the §5.4 per-`ItemKind` variant rendering table, the
§5.5 multi-line vs one-line layout rule, the §5.6 helper contract, and
the §4 span / debuginfo gating for the Item header source position
(`Item.span`).

The output is the standalone-Item view per `pretty-printer.md`
§5.1-§5.6: an attribute block (each `#[...]` on its own line) plus a
kind-keyword (with optional `unsafe` prefix on `fn`) plus the rendered
DefPath. No body content, no per-Instance signature -- those belong to
the composer in P1a-PP-05a/b.

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (per-task scope row
  P1a-PP-01)
- `docs/discussions/regseq-model/pretty-printer.md` §1 (Mission and
  consumer model), §2 (Master rendering style), §3.3 (Sigil
  conventions), §4 (Span-rendering and debuginfo modes; especially
  §4.2 / §4.3 for the Item-header span gating), §5.1 - §5.6
  (Item-level rendering shape), §8 (Per-method ownership table at
  `pretty-printer.md:1148-1170` -- the canonical method-signature
  source for `render_item` and `render_item_attribute_block`),
  §9.9 (`render_item` specimens 69 - 75)
- `docs/discussions/regseq-model/spec.md` §8 (Item schema),
  particularly §8.0 Item flags (`linkage`, `inline_hint`,
  `link_section`, `is_unsafe`, `is_phantom_data`, `is_unsafe_cell`,
  `is_manually_drop`, `opaque_handle_eligible`, `def_path`, `span`)
- `crates/regseq-model/src/pretty.rs` (existing `PrettyPrinter` shell;
  this task replaces the `render_item` stub and adds the §5.6 helper)
- `crates/regseq-model/src/def_path.rs` (`render_def_path` -- already
  shipped at stage 12 per the §8 ownership table; this task composes
  it via the cached `Item.def_path.rendered` lookup)
- The `Item`, `ItemKind`, and Item-flag types from regseq-model-DS-07
  (P1a-DS-07; landed earlier in phase-1a)
- `.harness/tasks/regseq-model/24-place-and-projection.md` (size /
  shape reference for task specs)

## Implementation Notes

Edit `crates/regseq-model/src/pretty.rs`. The §8 ownership table at
`pretty-printer.md:1148-1170` is the canonical reference for the two
methods this task owns.

### Public API

Per §8 ownership table (rows for `render_item` and
`render_item_attribute_block`):

```rust
impl PrettyPrinter<'_> {
    /// Item-level wrapper (attribute block + kind-keyword + path).
    /// Per pretty-printer.md §5. Owns the §4 span / debuginfo gating
    /// for the Item header source position.
    pub fn render_item(&self, id: ItemId) -> String;

    /// Shared internal helper. Walks the §5.3 per-flag table in
    /// table-row order, emits surviving `#[...]` rows per §5.2
    /// default-suppression, returns a multi-line block ending in a
    /// trailing `\n` when non-empty (empty `String` otherwise).
    /// Pinned per stage-14 D-14-5.7 LOCKED. Consumed by both
    /// `render_item` (this task) and `render_instance` (P1a-PP-05a/b
    /// Stream 1 of the §10.2.7 attribute-block composition).
    fn render_item_attribute_block(&self, id: ItemId) -> String;
}
```

### Per-flag rendering rules (§5.3 table at pretty-printer.md:422-443)

The §5.6 helper walks Item fields in the row order pinned at stage-13
D-13-4.3 LOCKED (per pretty-printer.md:556-575):

1. `Item.linkage`:
   - `NoMangle` -> `#[no_mangle]`
   - `Foreign` -> `#[foreign]`
   - `Weak` -> `#[linkage = "weak"]`
   - `Default` -> suppressed (per §5.2)
2. `Item.inline_hint`:
   - `Hint` -> `#[inline]`
   - `Always` -> `#[inline(always)]`
   - `Never` -> `#[inline(never)]`
   - `None` -> suppressed
3. Lang-item flags (each emits `#[lang = "..."]` when true; suppressed
   when false):
   - `is_phantom_data` -> `#[lang = "phantom_data"]`
   - `is_unsafe_cell` -> `#[lang = "unsafe_cell"]`
   - `is_manually_drop` -> `#[lang = "manually_drop"]`
4. `Item.link_section.is_some()` -> `#[link_section = "<resolved>"]`
   (the `link_section` payload is a `StringId` resolved via
   `model.strings.resolve(...)`).
5. `Item.opaque_handle_eligible == true` -> `#[opaque_handle]`
   (suppressed when false).

`Item.is_unsafe` is **NOT** surfaced by the helper per
pretty-printer.md:576-583. It renders as a keyword prefix on the
`fn` kind-keyword in `render_item` (and in `render_instance`'s
Function-variant envelope), not as an attribute.

### Per-`ItemKind` rendering (§5.4 table at pretty-printer.md:450-465)

`render_item` composes `attribute_block + kind_keyword + " " + path`
where:

- `path = self.render_def_path(id)` (cached lookup on
  `Item.def_path.rendered` per §8 ownership table).
- `kind_keyword` per `ItemKind`:
  - `Function` -> `[unsafe ]fn`
    (`unsafe ` prefix when `Item.is_unsafe == true`)
  - `Static { mutability: Mutability::Not }` -> `static`
  - `Static { mutability: Mutability::Mut }` -> `static mut`
  - `Const` -> `const`
  - `Struct` -> `struct`
  - `Enum` -> `enum`
  - `Union` -> `union`

`Item.is_unsafe` only takes effect for the `Function` arm; per §5.3
notes, `unsafe static` is non-syntactic in Rust source so the prefix
is not emitted on Static / Const / Struct / Enum / Union.

### Layout rule (§5.5 at pretty-printer.md:467-490)

- Multi-line when the attribute block is non-empty: each surviving
  `#[...]` on its own line, followed by the kind-keyword + path on
  the final line (no trailing newline at the end of the
  `render_item` output).
- One-line bare `kind $path` when zero attributes survive.
- The §5.6 helper produces a block ending in `\n` when non-empty so
  the calling renderer can concatenate `attr_block + kind + " " +
  path` directly (see the helper-call convention at
  pretty-printer.md:585-602).

### §4 span gating (Item-header source position)

Per `pretty-printer.md` §4.2 + §4.3 + §4.4 and the brief's per-task
scope ("Owns the §4 span / debuginfo gating for the Item header
source position"):

- When `PrettyPrintOptions::include_spans == false` (default mode per
  §4.1): no span comment is emitted.
- When `include_spans == true`: emit a trailing comment aligned to
  column `ALIGN = 40` on the kind-keyword + path line, formatted as
  `// <render_span(item.span)>`. Suppress the comment unconditionally
  when `item.span.file == SourceFileId::SYNTHESIZED` per §4.3.
- `include_debug_names` does not affect Item-level rendering (debug
  names attach to LocalDecls per §4.4; that gating is owned by
  P1a-PP-03).

The Item header is the only span source `render_item` consumes;
Statement / Terminator / LocalDecl span gating belongs to P1a-PP-03,
and the foreign-Item / function-body envelope spans belong to
P1a-PP-05a.

### Cross-references

- §3.3 sigil-conventions table: `ItemId` renders via
  `render_def_path` (cached one-line lookup per stage-12 D-12-6).
- §1.3 building-blocks-vs-composers: `render_item` is a stage-13
  building block; it does NOT call `render_instance` and does NOT
  emit Stream 2 (domain-attribute) rows. Per pretty-printer.md:540-551
  the helper's "Sources NOT consumed" list pins this boundary.

## Non-Goals

- No per-Instance content (FnSig args / return / body / Static value /
  Const value / ADT body). Owned by P1a-PP-05a (Function / Static /
  Const) and P1a-PP-05b (Struct / Enum / Union).
- No Stream 2 (`domain_attrs.item_attrs[item]`) emission. Owned by
  P1a-PP-05a/b's §10.2.7 attribute-block composition.
- No `render_ty` invocation. Type rendering for ADT field / variant
  shapes is owned by P1a-PP-02 (`render_ty`) and surfaces only at the
  composer level.
- No top-level `render(&self)` driver. Owned by P1a-PP-06.
- No body-level renderer span gating (Statement / Terminator /
  LocalDecl). Owned by P1a-PP-03.
- No PrettyPrintOptions struct definition or `with_options`
  constructor. Owned by P1a-PP-06; this task assumes the options field
  is already wired on `PrettyPrinter` and reads `include_spans` only.

## Test Plan

The test discipline names `unit-in-src` (per-method dispatch logic
where it touches private helpers) and `integration-in-tests` (insta
snapshots for each `render_item` specimen). Per the brief's
cross-cutting authoring rule: define the public API first with
`todo!()` bodies, then write tests, then implement.

### Unit tests (in `crates/regseq-model/src/pretty.rs`'s `#[cfg(test)] mod tests`)

Cover the private dispatch logic of `render_item_attribute_block`:

1. Empty attribute block: a fully default-flagged Item produces an
   empty `String` (no trailing `\n`).
2. Stream-1 row order: an Item with `linkage = NoMangle`,
   `inline_hint = Always`, `is_unsafe_cell = true`,
   `link_section = Some("...")`, `opaque_handle_eligible = true`
   produces all five attribute lines in the §5.3 row order
   (linkage -> inline_hint -> lang-flag -> link_section ->
   opaque_handle).
3. `is_unsafe == true` does NOT produce an attribute line via the
   helper (the keyword prefix is `render_item`'s job).
4. Each lang-flag row independently: each of `is_phantom_data`,
   `is_unsafe_cell`, `is_manually_drop` set in isolation produces the
   matching `#[lang = "..."]` line and nothing else.
5. Helper output ends with `\n` when non-empty (verifies the
   helper-call concatenation convention at pretty-printer.md:585-602).

### Integration tests (`crates/regseq-model/tests/pretty.rs`)

Add insta snapshot tests against the §9.9 worked-example specimens
(pretty-printer.md:1660-1741). One snapshot per specimen, named after
the specimen (e.g., `render_item_specimen_69_bare_function`). Each
test constructs the input Item per the specimen description, calls
`PrettyPrinter::new(&model).render_item(item_id)`, and asserts the
output via `insta::assert_snapshot!`.

Specimens to cover:

- Specimen 69: bare function (zero attributes) -> `fn pl011::init`.
- Specimen 70: function with full attribute stress
  (`#[no_mangle] / #[inline(always)] / #[link_section = ".text.boot"]
  / unsafe fn bsp::reset`).
- Specimen 70b: struct with `opaque_handle_eligible: true`
  (`#[opaque_handle] / struct pl011::Handle`).
- Specimen 71: static mut (`static mut CONFIG`).
- Specimen 72: const (`const MAX_BUF_SIZE`).
- Specimen 73: struct (`struct pl011::PL011`).
- Specimen 74: enum (`enum core::option::Option`).
- Specimen 75: union with lang-flag
  (`#[lang = "unsafe_cell"] / union core::cell::UnsafeCell`).

Add an additional integration test exercising the §4 span gating:

- Default mode (`include_spans == false`) -> no trailing comment.
- `include_spans == true` with a non-synthesized `Item.span` ->
  trailing `// <path>:<l>:<c>: <l>:<c>` comment aligned to column 40.
- `include_spans == true` with `item.span.file ==
  SourceFileId::SYNTHESIZED` -> no comment (per §4.3 suppression).

Add `insta` to `crates/regseq-model/Cargo.toml` `[dev-dependencies]`
if not already present.

### Test-discipline tags

- `unit-in-src` -- helper dispatch logic.
- `integration-in-tests` -- insta snapshots for the §9.9 specimens
  and span gating.

## Review

After tests pass, request a Codex review using `codex_runner.py` per
`.harness/SESSION.md` Review section:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-86 (P1a-PP-01) against its
task spec at .harness/tasks/regseq-model/86-render-item.md. ALL CHECKS
MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/pretty.rs
- crates/regseq-model/tests/pretty.rs
- (any new insta snapshot files under crates/regseq-model/tests/snapshots/)

Focus on:
- Spec deviations: every rule in the §5.3 per-flag table, the §5.4
  per-ItemKind table, the §5.5 layout rule, the §5.6 helper contract,
  and the §4 Item-header span gating (including §4.3 synthetic-span
  suppression).
- Per-method ownership per pretty-printer.md §8 (rows for `render_item`
  and `render_item_attribute_block`).
- Correctness issues and generality loss (helper must produce
  byte-identical Stream 1 output for both this task and the future
  P1a-PP-05a/b composer).
- Test coverage vs the spec's Test Plan: every §9.9 specimen has a
  matching insta snapshot; the §4 span gating cases are exercised.
Do NOT flag anything explicitly marked out of scope (Non-Goals, later
task IDs P1a-PP-02 through P1a-PP-06).
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

- `render_item(ItemId) -> String` and `render_item_attribute_block(ItemId)
  -> String` implemented per the §8 ownership table signatures and the
  §5.1 - §5.6 / §4 rules.
- All §9.9 worked-example specimens (Specimens 69, 70, 70b, 71, 72,
  73, 74, 75) have matching insta snapshot tests; snapshots match the
  expected outputs verbatim.
- Unit tests cover the helper's row-order walk, default-suppression,
  empty case, and the `is_unsafe`-not-via-helper rule.
- §4 Item-header span gating exercised: default-off, on with
  non-synthesized span, on with synthesized span (suppressed).
- `cargo test -p regseq-model`, `cargo fmt --all -- --check`, and
  `cargo clippy -p regseq-model -- -D warnings` all pass.
- Codex Task Review verdict APPROVED.
