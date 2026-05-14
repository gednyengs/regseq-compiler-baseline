# regseq-model-91: `render_instance` for Function / Static / Const variants (P1a-PP-05a)

## Description

Implement the stage-14 composer `render_instance(InstanceId) ->
String` on `PrettyPrinter<'_>` for the **Function**, **Static**, and
**Const** variants of the closed 6-variant `InstanceKind` enum.
Compose the function-body envelope per `pretty-printer.md` §3.1
(locals declaration block + arg-locals + optional debug-name lines +
per-block content via `render_basic_block`). Own the §3 conventions
for the function-body envelope (header line, indentation, sigils for
arg-locals and the RETURN_PLACE).

This task lays the per-variant `match inst.kind` dispatch frame on
`render_instance`; the ADT-definition variants (Struct / Enum /
Union) and the §10.2.7 attribute-stream composition are owned by
P1a-PP-05b (regseq-model-92). Both tasks share the same single
`render_instance` method.

Per stage-14 D-14-5.7 LOCKED, `render_instance` does NOT call
`render_item`; it composes the full per-variant envelope from
scratch, sharing only the §5.6 internal helper
`render_item_attribute_block` (from P1a-PP-01) for the Stream 1
attribute block.

## Input Context

- `.harness/SESSION.md`
- `.harness/spec-batches/regseqmodel-phase-1a.md` (per-task scope row
  P1a-PP-05a)
- `docs/discussions/regseq-model/pretty-printer.md` §1 (Mission), §2
  (Master rendering style), §3.1 (Body envelope), §3.2 (Indentation),
  §3.3 (Sigil conventions; `LocalId` -> `_<N>`), §4 (Span / debuginfo
  modes -- particularly §4.4 debug-name rendering), §5.6 (the shared
  `render_item_attribute_block` helper from P1a-PP-01), §6
  (`render_ty`), §7 (body-level renderers from P1a-PP-03), §8
  (Per-method ownership table at `pretty-printer.md:1148-1170` --
  canonical signature for `render_instance`), §10.2 dispatch overview
  (pretty-printer.md:3284-3417), §10.2.1 Function variant envelope
  (pretty-printer.md:3419-3636), §10.2.2 Static variant rendering
  (pretty-printer.md:3638-3741), §10.2.3 Const variant rendering
  (pretty-printer.md:3743-3811), §10.2.7 attribute-block composition
  (pretty-printer.md:4069-4199 -- shared with P1a-PP-05b; this task
  consumes Stream 1 via §5.6 helper and Stream 2 via the
  `render_domain_attribute_block` helper from P1a-PP-05b), §9.11
  specimens 110-122 (Function / Static / Const subset)
- `docs/discussions/regseq-model/spec.md` §3.1 (RegseqModel root,
  including `domain_attrs`), §3.6 (`DomainAttr` sibling tables),
  §8 (Item schema), §9 (Instance + InstanceKind; particularly §9.4
  `InstanceKind::Function { fn_sig, body, flavor, virtual_idx }` /
  `Static { ty, init }` / `Const { ty, value }`), §11 (`Layout` for
  arg-local types referenced by FnSig), §14 (FnAbi / FnSig schema --
  `c_variadic`, `safety`, `abi`, `inputs`, `output`), §25 (Body /
  LocalDecl / LocalId; the locals declaration block ordering and the
  RETURN_PLACE convention `_0`)
- `crates/regseq-model/src/pretty.rs` (the existing `PrettyPrinter`
  shell amended by P1a-PP-01, P1a-PP-02, P1a-PP-03, P1a-PP-04a/b)
- `render_item_attribute_block` (private helper from P1a-PP-01;
  consumed for Stream 1)
- `render_def_path`, `render_ty`, `render_const`, body-level
  renderers (`render_basic_block` etc.) from earlier P1a-PP-* tasks
- The Item / ItemKind types from regseq-model-DS-07 (P1a-DS-07)
- The Instance / InstanceKind / FunctionInstanceFlavor from
  regseq-model-DS-10 (P1a-DS-10)
- The DomainAttrTables / AttrData / IntrinsicKind / LogLevel from
  regseq-model-DS-09 (P1a-DS-09) -- consumed by Stream 2 of the
  attribute-block composition (the Stream 2 helper itself is owned
  by P1a-PP-05b; this task may stub it via a placeholder if
  P1a-PP-05b lands later, but the call site is here)
- `.harness/tasks/regseq-model/24-place-and-projection.md` (size /
  shape reference)

## Implementation Notes

Edit `crates/regseq-model/src/pretty.rs`. The §8 ownership table at
`pretty-printer.md:1148-1170` is the canonical reference for the
`render_instance` signature.

### Public API (per §8 ownership table)

```rust
impl PrettyPrinter<'_> {
    /// Composes `render_def_path` + `render_ty` + body-level
    /// renderers + `render_const` + the §5.6 shared
    /// `render_item_attribute_block` helper per pretty-printer.md
    /// §10.2. Per stage-14 D-14-5.7 LOCKED, does NOT call
    /// `render_item`.
    pub fn render_instance(&self, id: InstanceId) -> String;
}
```

The single `render_instance` method dispatches on `inst.kind` (the 6
closed `InstanceKind` variants):

- `Function { fn_sig, body, flavor, virtual_idx }` -- THIS TASK
  (§10.2.1).
- `Static { ty, init }` -- THIS TASK (§10.2.2).
- `Const { ty, value }` -- THIS TASK (§10.2.3).
- `Struct { layout }` / `Enum { layout }` / `Union { layout }` --
  P1a-PP-05b (§10.2.4 / §10.2.5 / §10.2.6); may stay `todo!()` until
  P1a-PP-05b lands.

`flavor` and `virtual_idx` on `Function` are identity metadata only
per pretty-printer.md:3342-3356; the rendered envelope reads only the
DefPath via `render_def_path(inst.item)` (which already encodes
synthesised families per `spec.md` §13.4). No per-flavor rendering
branch.

### §10.2.7 attribute-block composition (shared with P1a-PP-05b)

Per D-14-5.6 LOCKED. The attribute-block prefix is composed by
interleaving two deterministic streams:

```
<Stream 1 -- Item-flag stream, §5.3 table-row order, via render_item_attribute_block>
<Stream 2 -- domain_attrs.item_attrs[item], Vec-index order>
```

- **Stream 1**: produced by the §5.6 shared helper
  `render_item_attribute_block(inst.item)` from P1a-PP-01. Returns a
  multi-line block ending in `\n` when non-empty; empty `String`
  otherwise.
- **Stream 2**: walks `model.domain_attrs.item_attrs[inst.item]` in
  Vec-index order; per-`AttrData` variant render rule per the table
  at pretty-printer.md:4100-4106. Stage-14 ships only the
  `Intrinsic(IntrinsicKind::Log(LogLevel))` variant rendering
  (`#[rseq::intrinsic("log-<sev>")]`).

The Stream 2 helper (`render_domain_attribute_block`) is an internal
method on `PrettyPrinter<'_>` per pretty-printer.md:4124-4139. The
helper is owned by P1a-PP-05b (since the §10.2.7 composition rule is
shared across all 6 InstanceKind variants); this task calls it from
the Function / Static / Const envelopes. If P1a-PP-05b has not
landed at this task's start, this task may add a minimal placeholder
helper that returns the empty string for empty `item_attrs`; the
full per-`AttrData` dispatch table lands when P1a-PP-05b closes.

Empty-cases (pretty-printer.md:4153-4161):

- Both streams empty: no prefix.
- Stream 1 empty + Stream 2 non-empty: only Stream 2 rows; trailing
  `\n` after the last row.
- Stream 1 non-empty + Stream 2 empty: only Stream 1 rows; trailing
  `\n` after the last row.

### §10.2.1 Function variant envelope (per D-14-5.2 LOCKED; rules at pretty-printer.md:3419-3636)

The full envelope per pretty-printer.md:3425-3433:

```
[<attribute-block>\n][unsafe ][extern "ABI" ]fn <render_def_path(item)>(<arg-list>[, ...]) -> <render_ty(output)>[ { <body-envelope> }];
```

The header line is followed by **exactly one** of `{ <body-envelope>
}` (definition form when `body == Some(b)`) or `;` (declaration form
when `body == None`).

#### Aspect 1 -- Attribute block

Per §10.2.7. Two-stream interleaving with empty-prefix collapse.

#### Aspect 2 -- Header keyword decoration

Order matches Rust source grammar:

| Token | Condition | Source |
|---|---|---|
| `unsafe ` | `model.items[inst.item].is_unsafe == true` | `Item.is_unsafe` |
| `extern "ABI" ` | `fn_sig.abi != FnAbi::Rust` | `model.sigs[fn_sig].abi` |
| `fn ` | invariant | -- |
| `<path>` | invariant | `render_def_path(inst.item)` |

`Item.is_unsafe` (NOT `fn_sig.safety`) is the authoritative source
per pretty-printer.md:3472-3477. The bridge populates both from
`tcx.fn_sig(did).safety` so they agree by construction; reading
`Item.is_unsafe` keeps `render_item`'s standalone view (§5.3) and
the composer output consistent.

ABI: `FnAbi::C` -> `extern "C"`. `FnAbi::Rust` is the default and
suppressed.

#### Aspect 3 -- Arg-list grammar (table at pretty-printer.md:3486-3491)

| Case | Form |
|---|---|
| `fn_sig.inputs.is_empty()` | `()` |
| `fn_sig.inputs.len() == N >= 1` (non-variadic) | `(_1: <render_ty(inputs[0])>, _2: <render_ty(inputs[1])>, ..., _N: <render_ty(inputs[N-1])>)` |
| `fn_sig.c_variadic == true` | append `, ...` after the last fixed input |

Per-arg local sigils `_1` through `_N` match the §3.1 / `spec.md`
§25.1 arg-locals convention (locals indexed `1..=arg_count`). Args
separated by `, ` (comma + space).

#### Aspect 4 -- Return type

`-> <render_ty(fn_sig.output)>` always rendered, including unit
(`-> ()`) and divergent (`-> !` per §6.1) cases. Header-vs-locals
consistency: the body envelope's locals declaration block always
declares `let mut _0: <render_ty(ret)>;`, so the header omitting
`-> ()` would create drift.

#### Aspect 5 -- Body-vs-declaration trailer

| `body` field | Trailer |
|---|---|
| `Some(b)` | ` {\n` + body envelope per §3.1 + `\n}` |
| `None` (foreign decl) | `;` immediately after the `-> <ret>` segment |

Foreign-declaration form (`body == None`) surfaces foreign Function
Instances (`Item.linkage == Linkage::Foreign`); rustc's MIR
pretty-printer skips foreign items but RegseqModel renders them as
emission units (the C backend emits `extern int foo(int);` for them).

#### Body envelope per §3.1 (definition form only)

The function-body envelope this task owns:

```
[<attribute-block>\n][unsafe ][extern "ABI" ]fn <path>(<args>[, ...]) -> <ret> {
    let mut _0: <ret>;       // RETURN_PLACE
    let mut _M: T;           // first non-arg local (M == arg_count + 1)
    let _M+1: T;             // immutable local
    ...
    debug NAME => _N;        // when opts.include_debug_names
    debug OTHER_NAME => _M;
    ...

    bb0: { ... }             // via render_basic_block (P1a-PP-03)
    bb1: { ... }
    ...
}
```

Composition rules (per pretty-printer.md:3534-3563):

- Header line at column 0.
- Locals declaration block: one declaration per line at 4-space
  indent (`INDENT = "    "`). Form:
  - `let mut _N: <render_ty(local_decl.ty)>;` for mutable locals.
  - `let _N: <render_ty(local_decl.ty)>;` for immutable.
  - `let mut _0: <render_ty(fn_sig.output)>;` always for
    RETURN_PLACE.
- Arg-locals (`_1` through `_N`) are declared in the header arg-list
  -- NOT redeclared in the locals block (per pretty-printer.md:3539-3540
  comment "arg-locals _1..=_N are declared in the header arg-list
  above, not redeclared here").
- LocalDecl span emission per §4.2 / §4.3: when
  `opts.include_spans == true`, emit a trailing `// <render_span(ld.span)>`
  comment aligned to column 40; suppress when `ld.span.file ==
  SourceFileId::SYNTHESIZED` per §4.3. The body-level span gating
  helper (`render_local_decl` style) is owned by P1a-PP-03; this task
  composes it.
- Debug-name lines per §4.4 (when `opts.include_debug_names == true`):
  blank line after locals declarations, then one `debug NAME => _N;`
  line per LocalDecl with `debug_name = Some(name)` in `LocalId`
  order. Names render via `model.strings.resolve(name)`. Locals
  without `debug_name` are silent.
- Per-block content: each basic block via
  `render_basic_block(block, body)` from P1a-PP-03 (4-space envelope
  indent + 8-space inner indent). Blocks separated by a blank line
  between consecutive blocks.
- Closing `}` at column 0.

### §10.2.2 Static variant rendering (per D-14-5.3 LOCKED; rules at pretty-printer.md:3638-3741)

Form (single line with optional multi-line attribute block above):

```
[<attribute-block>\n]static [mut ]<render_def_path(item)>: <render_ty(ty)>[ = <render_const(init.unwrap(), ty)>];
```

Mutability prefix from `model.items[inst.item].kind`:

| ItemKind | Prefix |
|---|---|
| `Static { mutability: Mutability::Not }` | `static ` |
| `Static { mutability: Mutability::Mut }` | `static mut ` |

Statics can't be generic so the 1:1 Item:Instance mapping always
holds (`spec.md` §8.0 + §9.4); the Item-side mutability is the
authoritative source.

Trailer:

| `init` | Form |
|---|---|
| `Some(c)` | ` = <render_const(c, ty)>;` |
| `None` (foreign extern) | `;` immediately after the `: <ty>` |

The biconditional `init.is_none() iff Item.linkage == Linkage::Foreign`
per `spec.md` §3.4 step 8 / D-12-2.2 is asserted by validation, not
re-checked at render time.

Stage-14 ships no `AttrData::Intrinsic` variant occupying Static
Items per `spec.md` §3.6.3 / D-14-1; Stream 2 is empty for Statics
in v1.

### §10.2.3 Const variant rendering (per D-14-5.4 LOCKED; rules at pretty-printer.md:3743-3811)

Form (single line with optional multi-line attribute block above):

```
[<attribute-block>\n]const <render_def_path(item)>: <render_ty(ty)> = <render_const(value, ty)>;
```

No mutability prefix (consts are immutable; `const mut` is invalid
Rust). No foreign-declaration form (`extern "C" { const FOO: u32; }`
is invalid). `value: ConstId` is always populated per stage-12
D-12-2 -- no Option wrap.

Stage-14 ships no `AttrData::Intrinsic` variant occupying Const
Items; Stream 2 is empty for Consts in v1.

### Pair-form D-14-4.1 composition

Per pretty-printer.md:3691-3696 / 3776-3781 -- both Static and Const
compose `render_const(c, ty)` with the Instance's `ty` as
consumer-context type, satisfying D-14-4.1's API contract.

### §3 conventions for the function-body envelope (this task owns)

- Header line at column 0.
- Inner content at `INDENT = "    "` (4 spaces) per §3.2.
- `bb<N>: {` envelope content already at 8-space indent inside
  `render_basic_block` (P1a-PP-03).
- Trailing `;` per-statement (P1a-PP-03's domain); terminator's
  `-> [...]` followed by `;` appended by `render_basic_block`.

## Non-Goals

- No ADT-definition rendering (`Struct` / `Enum` / `Union` Instance
  variants). Owned by P1a-PP-05b; the per-variant match arms in
  `render_instance` may stay `todo!()` until P1a-PP-05b lands.
- No top-level `render(&self)` driver. Owned by P1a-PP-06.
- No PrettyPrintOptions struct definition or `with_options`
  constructor. Owned by P1a-PP-06.
- No Item-level rendering (`render_item` / Stream 1 helper
  implementation). Owned by P1a-PP-01; this task consumes them.
- No `render_const` / `render_ty` / body-level renderer
  reimplementation. Consumed via P1a-PP-02 / P1a-PP-03 / P1a-PP-04a/b.
- No bridge-side construction of Instance / Item / Body shape.
  Owned by axis 3 (P1a-BR-*).
- No per-flavor rendering (Source / DropGlue / VTableShim / Closure
  / etc.). Per pretty-printer.md:3342-3356 / 3557-3563, `flavor` and
  `virtual_idx` are identity metadata only; the DefPath encodes the
  family.
- No Item-header span gating. Owned by P1a-PP-01. The body-envelope
  spans (LocalDecl span trailing comments) are composed here from
  the helper exposed by P1a-PP-03.

## Test Plan

Test discipline names `unit-in-src` (per-variant dispatch shape;
attribute-block composition empty-cases; body envelope locals-block
ordering; arg-list variadic appending) and `integration-in-tests`
(insta snapshots for §9.11 specimens 110-122; §3.1 envelope golden
shapes).

Per the brief: define the API first with `todo!()` body for the full
`match inst.kind`, then write tests, then implement.

### Unit tests (in `crates/regseq-model/src/pretty.rs`)

1. `render_instance` Function with empty arg-list -> `(...) -> ()`.
2. `render_instance` Function with non-empty arg-list -> `(_1: T,
   _2: T, ...) -> Ret`.
3. `render_instance` Function with `c_variadic == true` appends `,
   ...` after the last fixed input (Specimen 114).
4. `render_instance` Function with `Item.is_unsafe == true` emits
   `unsafe ` prefix on `fn` (Specimen 111).
5. `render_instance` Function with `fn_sig.abi == FnAbi::C` emits
   `extern "C" ` prefix.
6. `render_instance` Function `body: None` emits trailer `;`
   immediately after `-> <ret>` (Specimen 113); `body: Some(_)`
   emits ` {\n` + envelope + `\n}` (Specimen 110).
7. Function body envelope: locals declaration block declares
   `let mut _0: <ret>;` for RETURN_PLACE; arg-locals NOT redeclared;
   non-arg locals follow in LocalId order with mut/non-mut
   discrimination from `LocalDecl.mutability`.
8. Function body envelope: debug-name lines emitted when
   `include_debug_names == true`; suppressed when default-off; in
   LocalId order; only locals with `debug_name = Some(_)`.
9. Function body envelope: LocalDecl span trailing comment when
   `include_spans == true`; suppressed for synthesized spans.
10. `render_instance` Static immutable -> `static <path>: <ty>[ = <const>];`.
11. `render_instance` Static mutable -> `static mut <path>: <ty>[ = <const>];`.
12. `render_instance` Static `init: None` (foreign) -> `static <path>:
    <ty>;` plus `#[foreign]` from Stream 1 (Specimen 120).
13. `render_instance` Const single-line -> `const <path>: <ty> = <const>;`
    (Specimens 121, 122).
14. Attribute block composition: Stream 1 only / Stream 2 only / both
    streams / neither stream cases (Specimens 110, 112, 113, 115).
15. Specimen 117: divergent return `-> !`.

### Integration tests (`crates/regseq-model/tests/pretty.rs`)

Add insta snapshot tests against the §9.11 specimens covered by
this task (Function / Static / Const subset):

- Specimen 110: plain function, zero attributes, unit return.
- Specimen 111: unsafe function with raw-pointer arg.
- Specimen 112: function with full Stream-1 attribute block (no_mangle
  + inline_always + link_section).
- Specimen 113: foreign C function declaration (body: None).
- Specimen 114: foreign C variadic declaration.
- Specimen 115: diagnostic-logging intrinsic (Stream 1 + Stream 2).
- Specimen 116: bridge-synthesised drop glue (DefPath encodes family).
- Specimen 117: divergent return (`Ty::Never`).
- Specimen 118: defined immutable static.
- Specimen 119: defined mutable static with struct initializer.
- Specimen 120: foreign extern static (init: None).
- Specimen 121: scalar const.
- Specimen 122: reference-typed const (string literal).

(Specimens 123 - 128 ADT-definition variants belong to P1a-PP-05b.)

Add `insta` to `crates/regseq-model/Cargo.toml` `[dev-dependencies]`
if not already present.

### Test-discipline tags

- `unit-in-src` -- per-variant dispatch shape; attribute-block
  composition empty-case logic; body envelope ordering; arg-list
  variadic appending; debug-name + LocalDecl span gating.
- `integration-in-tests` -- insta snapshots for §9.11 specimens
  110 - 122.

## Review

After tests pass, request a Codex review using `codex_runner.py`:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-91 (P1a-PP-05a) against
its task spec at .harness/tasks/regseq-model/91-render-instance-function-static-const.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/pretty.rs
- crates/regseq-model/tests/pretty.rs
- (any new insta snapshot files under crates/regseq-model/tests/snapshots/)

Focus on:
- Spec deviations: §10.2.1 Function envelope all five aspects
  (attribute block, header keyword decoration, arg-list grammar,
  return type, body-vs-declaration trailer); §10.2.2 Static all
  four aspects; §10.2.3 Const all four aspects.
- D-14-5.7 layering boundary: `render_instance` does NOT call
  `render_item`; instead composes from scratch and shares only the
  §5.6 helper for Stream 1.
- §10.2.7 attribute-block composition empty-case behavior matches
  pretty-printer.md:4153-4161.
- §3.1 body envelope rules: locals declaration block ordering
  (RETURN_PLACE first, arg-locals NOT redeclared, non-arg locals
  in LocalId order); INDENT discipline (4 spaces); debug-name
  emission gated by include_debug_names; LocalDecl span gating
  honored.
- `Item.is_unsafe` (NOT `fn_sig.safety`) is the authoritative source
  for the `unsafe ` prefix per pretty-printer.md:3472-3477.
- `flavor` / `virtual_idx` are NOT rendering inputs.
- Pair-form D-14-4.1 composition for Static/Const value rendering
  threads the Instance's `ty` as consumer Ty.
- Test coverage vs spec Test Plan: every Specimen 110 - 122 has a
  matching insta snapshot.
Do NOT flag anything explicitly marked out of scope (ADT-definition
variants which belong to P1a-PP-05b; later task ID P1a-PP-06; the
Stream 2 helper's per-AttrData dispatch table -- placeholder
acceptable here).
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

- `render_instance(InstanceId) -> String` implemented with a
  per-`InstanceKind` match covering all 6 variants (Function /
  Static / Const filled per this task; Struct / Enum / Union may
  stay `todo!()` until P1a-PP-05b lands).
- §10.2.1 Function envelope rendered per all five aspects (attribute
  block; header keyword decoration; arg-list grammar including
  variadic; return type; body-vs-declaration trailer); §3.1 body
  envelope composed correctly (RETURN_PLACE first, arg-locals not
  redeclared, debug-name + span gating).
- §10.2.2 Static rendered per all four aspects (attribute block;
  mutability prefix; path + type; initializer-vs-declaration
  trailer).
- §10.2.3 Const rendered as single-line `const <path>: <ty> = <const>;`
  with attribute block.
- §10.2.7 attribute-block composition (Stream 1 via P1a-PP-01's §5.6
  helper; Stream 2 via the helper owned by P1a-PP-05b -- placeholder
  empty-string behavior acceptable until P1a-PP-05b lands).
- All §9.11 specimens 110 - 122 have matching insta snapshot tests;
  outputs match verbatim.
- Unit tests cover per-aspect dispatch and the empty / Stream-1-only /
  Stream-2-only / both-streams attribute composition cases.
- `cargo test -p regseq-model`, `cargo fmt --all -- --check`, and
  `cargo clippy -p regseq-model -- -D warnings` all pass.
- Codex Task Review verdict APPROVED.
