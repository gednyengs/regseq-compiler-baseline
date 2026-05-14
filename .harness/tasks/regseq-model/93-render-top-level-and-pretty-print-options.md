# regseq-model-93: Top-level `render(&self)` + `PrettyPrintOptions` (P1a-PP-06)

## Description

Implement the top-level stage-14 composer `render(&self) -> String`
on `PrettyPrinter<'_>` per `pretty-printer.md` §10.3, and add the
`PrettyPrintOptions` opt-in machinery per §4.1. The composer iterates
`model.instances.iter_enumerated()` in InstanceId order (canonical
F4-2 sort per `spec.md` §16.3; rank-grouped via §9.3
`regseq_instance_kind_rank`), composes each Instance via
`render_instance(InstanceId)` (P1a-PP-05a/b), inserts a single blank
line (the two-byte `"\n\n"` sequence) between consecutive Instances,
emits no file-level header / trailer, and returns the empty `String`
for an empty `model.instances`.

This task owns ONLY options propagation through `PrettyPrinter`'s
construction surface; per-construct span / debug-name emission
already lives in P1a-PP-01 (Item header) and P1a-PP-03 (Statement /
Terminator / LocalDecl). PrettyPrintOptions adds the
`with_options(model, opts)` constructor parallel to the existing
`new(model)` and threads the two flags (`include_spans`,
`include_debug_names`) into the renderer state.

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (per-task scope row
  P1a-PP-06)
- `docs/discussions/regseq-model/pretty-printer.md` §1 (Mission), §2
  (Master rendering style; especially §2.2 `PrettyPrinter` shape and
  §2.3 method shape), §4.1 default mode + `PrettyPrintOptions`
  struct (`pretty-printer.md:309-326`), §4.2 / §4.3 / §4.4 (the
  per-construct span / debug-name emission rules implemented by
  P1a-PP-01 and P1a-PP-03; this task only owns options propagation),
  §8 (Per-method ownership table at `pretty-printer.md:1148-1170` --
  `render(&self)` row), §10.3 (Top-level `render(&self)` -- the
  authoritative source for this task), §9.12 specimens 129 - 133
- `docs/discussions/regseq-model/spec.md` §3.1 (`RegseqModel.instances`
  IndexVec keyed by `InstanceId`), §9.3
  (`regseq_instance_kind_rank` -- rank 0 Function, 1 Static, 2 Const,
  3 Struct, 4 Enum, 5 Union), §16.1 (F4 sort comparator), §16.3 (F4-2
  `RegseqInstanceKey` sort), §3.6 (`domain_attrs` -- consumed
  transitively via `render_instance` Stream 2)
- `crates/regseq-model/src/pretty.rs` (the existing `PrettyPrinter`
  shell amended by P1a-PP-01 through P1a-PP-05b)
- `render_instance` from regseq-model-91 (P1a-PP-05a) and
  regseq-model-92 (P1a-PP-05b)
- The `RegseqModel` root (with the F4-2-sorted `instances: IndexVec<InstanceId,
  Instance>`) from regseq-model-DS-01 (P1a-DS-01)
- The `Instance` / `InstanceKind` types from regseq-model-DS-10
  (P1a-DS-10)
- `.harness/tasks/regseq-model/24-place-and-projection.md` (size /
  shape reference)

## Implementation Notes

Edit `crates/regseq-model/src/pretty.rs`. The §8 ownership table at
`pretty-printer.md:1148-1170` is the canonical reference for the
`render(&self)` signature.

### `PrettyPrintOptions` struct (per §4.1 at pretty-printer.md:317-322)

```rust
/// Two independent flags controlling extra annotations on rendered
/// output. Default-off (constructed via `Default`); the
/// `PrettyPrinter::with_options` constructor opts in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrettyPrintOptions {
    /// When true, emit `// <render_span(...)>` trailing comments
    /// aligned to column 40 on Statement / Terminator / LocalDecl
    /// (per body-level emission in P1a-PP-03) and on the Item
    /// header line (per P1a-PP-01). Suppressed unconditionally when
    /// `span.file == SourceFileId::SYNTHESIZED` per §4.3.
    pub include_spans: bool,

    /// When true, emit standalone `debug NAME => _N;` lines after
    /// the locals declarations block per §4.4. Composed by the
    /// function-body envelope in P1a-PP-05a; only locals with
    /// `LocalDecl.debug_name = Some(name)` contribute.
    pub include_debug_names: bool,
}
```

The flags are **independent**: a consumer can opt into spans only,
debug names only, both, or neither (per pretty-printer.md:323-325).
`PrettyPrintOptions::default()` produces all-off (the §4.1 default
mode).

### `PrettyPrinter` constructor surface

Per §4.1:

- `PrettyPrinter::new(model)` -- constructs with
  `PrettyPrintOptions::default()` (existing constructor; this task
  does not change its semantics; `include_spans` and
  `include_debug_names` both default to false).
- `PrettyPrinter::with_options(model, opts)` -- new opt-in
  constructor:

```rust
impl<'a> PrettyPrinter<'a> {
    pub fn with_options(model: &'a RegseqModel, opts: PrettyPrintOptions) -> Self;
}
```

This task adds the `opts: PrettyPrintOptions` field to the
`PrettyPrinter` struct (if not already present from the foundation
shell) and threads it through `new` (which sets
`opts: PrettyPrintOptions::default()`) and `with_options`. The
per-construct emission rules (P1a-PP-01 Item header span;
P1a-PP-03 body-level span / LocalDecl span; P1a-PP-05a
debug-name lines) read `self.opts.include_spans` and
`self.opts.include_debug_names` directly. This task owns only the
propagation surface, not the per-construct emission logic.

### Top-level `render(&self) -> String` (per §10.3)

#### §10.3.1 API signature (pretty-printer.md:4209-4226)

```rust
impl PrettyPrinter<'_> {
    /// Top-level model dump. Iterates `model.instances.iter_enumerated()`
    /// in InstanceId order (canonical F4-2 sort per spec.md §16.3;
    /// rank-grouped via §9.3 regseq_instance_kind_rank); composes
    /// per-Instance via `render_instance(InstanceId)`; single blank
    /// line between consecutive Instances; no file-level header /
    /// trailer; empty `model.instances` -> empty `String`. Per
    /// pretty-printer.md §10.3.
    pub fn render(&self) -> String;
}
```

Zero arguments beyond `&self`. No `single: Option<InstanceId>` filter
parameter (single-Instance dispatch is `render_instance(InstanceId)`).
No write-sink form (`&mut W: io::Write`). No `Option` / `Result`
return wrapping.

#### §10.3.2 Iteration order (pretty-printer.md:4228-4255)

Walks `self.model.instances.iter_enumerated()` in **`InstanceId`
order verbatim** -- no render-time re-sort. The `IndexVec` is
already in canonical F4-2 sorted order per `spec.md` §16.3 (the
key `(regseq_kind_rank, item_id, generic_args_key, fn_shim_disambig)`
is the sort comparator output applied by P1a-VAL-02's check); rank
dominates ItemId, so iteration naturally groups Functions (rank 0) ->
Statics (rank 1) -> Consts (rank 2) -> Structs (rank 3) -> Enums
(rank 4) -> Unions (rank 5).

The renderer does NOT iterate `model.items`, `model.types`, or any
other table -- only `model.instances`. Items and types surface
transitively via `render_instance` (which consumes `Item` /
`ItemKind` / `Layout` via the existing `spec.md`-defined access
paths).

#### §10.3.3 Per-Instance separator (pretty-printer.md:4257-4288)

`render_instance(id)` outputs do NOT carry a trailing newline (per
P1a-PP-05a/b's per-variant rendering rules: Static / Const /
Function-declaration end with `;`; Function-definition / ADT-definition
end with closing `}`).

Between consecutive `render_instance(id)` outputs, `render` emits
the two-byte sequence `"\n\n"`:

- The first `\n` terminates the prior Instance's last line.
- The second `\n` produces the single blank line between Instances.

No separator before the first Instance; no separator after the
last.

#### §10.3.4 File-level output envelope (pretty-printer.md:4290-4316)

`render` emits **no file-level header**, **no file-level trailer**,
**no metadata wrapper**:

- The output begins at column 0 of line 1 with the first rendered
  byte of the first Instance's `render_instance` output (which is
  the first byte of its attribute block when non-empty per §10.2.7,
  otherwise the first byte of the kind-keyword line).
- The output ends with the last rendered byte of the last Instance's
  `render_instance` output (closing `}` for body forms and ADT
  definitions; `;` for declaration forms and Static / Const).
- `render` does NOT inject any trailing newline after the last
  Instance.

No `// WARNING: ...` header; no `// Crate: foo, toolchain: bar`
metadata; no version tag.

#### §10.3.5 Empty-model case (pretty-printer.md:4318-4338)

When `self.model.instances.is_empty()`, `render` returns the empty
`String` `""`. No header (per §10.3.4); no separator (no consecutive
pairs); no placeholder comment. The 0-byte output is the identity
element of the per-Instance concatenation + separator composition.

#### §10.3.6 Composition outline (pretty-printer.md:4340-4357)

```
render(&self) -> String:
  buf = ""
  first = true
  for (id, _inst) in self.model.instances.iter_enumerated():
    if first:
      first = false
    else:
      buf.push_str("\n\n")
    buf.push_str(&self.render_instance(id))
  buf
```

The composer is **purely scaffolding** over `render_instance`: it
adds the iteration + separator + framing rules; all per-Instance
content comes from `render_instance` unchanged. No file-level
decoration layer; no per-Instance post-processing.

### Existing `render` stub

The existing `pretty.rs::render` stub at
`crates/regseq-model/src/pretty.rs:96` is a `#[should_panic]`
placeholder (the existing test
`render_top_level_is_unimplemented_at_stage_one` pins the panic
message). This task replaces the stub with the §10.3 implementation;
the corresponding `#[should_panic]` integration test is removed (or
replaced with a positive assertion -- see Test Plan).

## Non-Goals

- No per-construct span emission. Owned by P1a-PP-01 (Item header)
  and P1a-PP-03 (Statement / Terminator / LocalDecl). This task
  only adds options propagation; the per-construct emission reads
  `self.opts.include_spans` directly.
- No per-construct debug-name emission. Owned by P1a-PP-05a
  (function-body envelope). This task only adds options
  propagation.
- No `render_instance` reimplementation. Owned by P1a-PP-05a/b.
- No render-time re-sort of `model.instances`. The IndexVec is
  expected to be in F4-2 sorted order per `spec.md` §16.3; the
  P1a-VAL-02 validator verifies the order at construction time.
- No file-level header / trailer / metadata.
- No write-sink (`fmt::Write` / `io::Write`) variant.
- No `Option<InstanceId>` filter or per-rank-group section header.
- No build-info surfacing in the output (per §10.3.4: no
  `// Crate: foo, toolchain: bar` metadata).
- No NaN-bit-pattern rendering option, no `integer_render_style` hex
  toggle (future-extension boundaries per pretty-printer.md:2691-2702
  and pretty-printer.md:2865-2875; not pre-allocated).

## Test Plan

Test discipline names `unit-in-src` (PrettyPrintOptions construction
+ `with_options` constructor wiring; iteration + separator logic on
small fixture models) and `integration-in-tests` (insta snapshots
for §9.12 specimens 129 - 133; full-model golden).

### Unit tests (in `crates/regseq-model/src/pretty.rs`)

1. `PrettyPrintOptions::default()` -> both flags false.
2. `PrettyPrinter::new(model).opts == PrettyPrintOptions::default()`.
3. `PrettyPrinter::with_options(model, opts).opts == opts` for
   non-default opts.
4. `render` on an empty `model.instances` returns `""` (no header,
   no separator, no placeholder).
5. `render` on a single-Instance model emits one
   `render_instance(id)` output with no leading or trailing
   separator.
6. `render` on an N-Instance model (N >= 2) emits each
   `render_instance(id)` output joined by exactly the two-byte
   `"\n\n"` separator; first emits no leading separator; last emits
   no trailing newline.
7. `render` walks `model.instances` in `iter_enumerated()` order
   without re-sorting. (Construct a model where Instance 0 is rank 3
   Struct, Instance 1 is rank 0 Function; verify iteration order is
   `[Struct, Function]` -- the renderer trusts the IndexVec order
   even if it disagrees with F4-2; sort enforcement lives in
   P1a-VAL-02.)

### Integration tests (`crates/regseq-model/tests/pretty.rs`)

Replace the existing `#[should_panic]` test
`render_top_level_is_unimplemented_at_stage_one` with positive
assertions, then add insta snapshot tests against the §9.12
specimens 129 - 133:

- Specimen 129: empty model -> empty `String`.
- Specimen 130: single-Instance model (Function `pl011::write_byte`).
- Specimen 131: multi-Instance model with rank-grouped F4-2 order
  (Function -> Static -> Struct).
- Specimen 132: multi-Instance model with all Instances in the same
  rank group (three Functions, ItemId-ordered: log_info -> init ->
  printf foreign declaration).
- Specimen 133: multi-Instance model spanning all six rank groups
  (Function -> Static -> Const -> Struct -> Enum -> Union).

Each specimen uses constructed `RegseqModel` fixtures (the
specimens describe the InstanceKind / item / fn_sig / layout for each
Instance); the snapshot asserts the full `PrettyPrinter::new(&model).render()`
output verbatim.

### Options propagation tests

Add an integration test that constructs a model with one Function
having a non-synthesized `Statement.span` and a `LocalDecl.debug_name
= Some("counter")`, then asserts:

- `PrettyPrinter::new(&model).render()` (default opts) emits NO span
  comments and NO debug-name lines.
- `PrettyPrinter::with_options(&model, PrettyPrintOptions {
  include_spans: true, include_debug_names: false }).render()`
  emits the span comment but no debug-name line.
- `PrettyPrinter::with_options(&model, PrettyPrintOptions {
  include_spans: false, include_debug_names: true }).render()`
  emits no span comments but the `debug counter => _N;` line.
- `PrettyPrinter::with_options(&model, PrettyPrintOptions {
  include_spans: true, include_debug_names: true }).render()` emits
  both.

This test exercises the propagation path through `render` ->
`render_instance` (P1a-PP-05a) -> `render_basic_block` ->
`render_statement` (P1a-PP-03), confirming the flags propagate
end-to-end without per-construct overrides.

Add `insta` to `crates/regseq-model/Cargo.toml` `[dev-dependencies]`
if not already present.

### Test-discipline tags

- `unit-in-src` -- `PrettyPrintOptions` construction; `with_options`
  wiring; iteration + separator + empty-model logic.
- `integration-in-tests` -- insta snapshots for §9.12 specimens
  129 - 133; full-model golden; options-propagation end-to-end test.

## Review

After tests pass, request a Codex review using `codex_runner.py`:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-93 (P1a-PP-06) against
its task spec at .harness/tasks/regseq-model/93-render-top-level-and-pretty-print-options.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/pretty.rs
- crates/regseq-model/tests/pretty.rs
- (any new insta snapshot files under crates/regseq-model/tests/snapshots/)

Focus on:
- Spec deviations: §10.3.1 signature; §10.3.2 iteration order
  (`iter_enumerated()` over `model.instances` verbatim, no re-sort);
  §10.3.3 `"\n\n"` separator (first-flag mechanism, no leading or
  trailing separator); §10.3.4 no file-level header / trailer / no
  trailing newline; §10.3.5 empty-model -> empty `String`.
- §4.1 `PrettyPrintOptions` struct shape: two independent bool
  flags; `Default` -> all-off; `Clone, Copy, Debug, Default,
  PartialEq, Eq` derives.
- `with_options(model, opts)` constructor wires opts into
  PrettyPrinter; `new(model)` keeps default-off semantics.
- Options propagation reaches per-construct emission sites: span
  comments fire only when `include_spans == true` (and only on
  non-synthesized spans per §4.3); debug-name lines fire only when
  `include_debug_names == true`.
- Test coverage vs spec Test Plan: every Specimen 129 - 133 has a
  matching insta snapshot; options propagation exercised
  end-to-end.
Do NOT flag anything explicitly marked out of scope (per-construct
span / debug-name emission lives in P1a-PP-01 / P1a-PP-03 / P1a-PP-05a;
render-time re-sort or sort enforcement which lives in P1a-VAL-02;
file-level headers / trailers).
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

- `PrettyPrintOptions { include_spans: bool, include_debug_names:
  bool }` struct defined per §4.1 with `Clone, Copy, Debug, Default,
  PartialEq, Eq` derives; `Default` -> all-off.
- `PrettyPrinter::with_options(model, opts)` constructor implemented
  parallel to `PrettyPrinter::new(model)`; both wire the options
  field on `PrettyPrinter`.
- Top-level `render(&self) -> String` implemented per
  pretty-printer.md §10.3:
  - Iterates `self.model.instances.iter_enumerated()` verbatim.
  - Composes per-Instance via `render_instance(id)`.
  - Inserts the two-byte `"\n\n"` separator between consecutive
    Instances; no leading or trailing separator.
  - Emits no file-level header / trailer / no trailing newline.
  - Returns `""` for empty `model.instances`.
- Options propagation reaches per-construct emission sites
  end-to-end (verified by the options-propagation integration
  test).
- All §9.12 specimens (129, 130, 131, 132, 133) have matching insta
  snapshot tests; outputs match verbatim.
- Existing `#[should_panic]` test for the `render` stub is replaced
  with positive assertions.
- `cargo test -p regseq-model`, `cargo fmt --all -- --check`, and
  `cargo clippy -p regseq-model -- -D warnings` all pass.
- Codex Task Review verdict APPROVED.
