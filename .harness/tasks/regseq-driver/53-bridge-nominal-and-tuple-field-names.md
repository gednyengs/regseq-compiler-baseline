# regseq-driver-53: Bridge Source Field Names on Nominal and Tuple `FieldLayout`

## Description

The phase-1a bridge drops source-defined field names when
constructing `FieldLayout` entries for user-defined structs,
unions, and tuples. Every `FieldLayout.name` ends up as the
empty interned string, which the pretty-printer dutifully
renders as `Counter { : 0_u32 }` (empty name before the colon)
instead of `Counter { value: 0_u32 }`.

Codex pre-task review at session
`019e388b-b5e8-7fa3-a8af-a9a4adc899d4` confirmed the diagnosis
and identified two adjacent issues in the same bridge file. This
task scope covers only the nominal-struct/union and tuple
gaps; the enum-layout shape gap (which Codex flagged as
`nominal_layout` returning `LayoutKind::Struct` for `Ty::Enum`
when the renderer and validator both require
`LayoutKind::Enum { shape }`) is explicitly OUT of scope and
deferred to a separate task.

### What is broken

1. **Nominal struct/union field names.**
   `translate_adt` at `crates/regseq-driver/src/bridge/ty.rs:325-329`
   walks `adt.all_fields()` (each `FieldDef` carries
   `.name: rustc_span::Symbol`) but collects only
   `field.ty(cx.input.tcx, args)` into `field_tys`. The
   downstream `nominal_layout` at
   `crates/regseq-driver/src/bridge/ty.rs:912-919` interns
   `""` once and assigns it to every `FieldLayout.name` in
   the struct or union branch.

2. **Tuple field names.**
   `tuple_layout` at
   `crates/regseq-driver/src/bridge/ty.rs:864-898` interns
   `""` for every tuple field. Per Codex's review, this is a
   latent validator-contract violation: the model validator
   at `crates/regseq-model/src/layout_validator.rs:419-431`
   requires tuple field names `_0`, `_1`, ..., `_n-1`. The
   gap does not fire today because no current fixture
   routes a bridged tuple through validation in a way that
   reaches that check, but the contract is violated.

### Downstream-visible symptoms

- Rendered output of any user-defined struct aggregate
  shows `Counter { : value }` instead of
  `Counter { value: value }`. Reproducible against any
  playground crate containing
  `pub struct Counter { pub value: u32 } impl Counter { pub fn new() -> Counter { Counter { value: 0 } } }`.
- The vtable end-to-end snapshot at
  `crates/regseq-model/tests/snapshots/e2e_synthesis__vtable_render.snap:60`
  currently pins the broken form
  `_1 = trait_object::StaticByte { : 66_u8 };` -- the snapshot
  has been recording the bug as the expected output. Note
  `trait_object::StaticByte` is a TUPLE struct
  (`pub struct StaticByte(pub u8);` at
  `crates/test-ir-fixtures/fixtures/trait_object.rs:8`), so the
  post-fix render uses tuple-form parens, not named-struct
  braces: `_1 = trait_object::StaticByte(66_u8);` (the empty
  string for the field name today causes
  `layout_fields_are_tuple_style` at
  `crates/regseq-model/src/pretty.rs:2785-2796` to return
  `false`, so the BROKEN output uses the wrong delimiters;
  after the fix, the field name `"0"` triggers tuple-style
  detection and the delimiters change from `{ ... }` to
  `( ... )`).
- The tuple-vs-named-variant dispatch at
  `crates/regseq-model/src/pretty.rs:2785-2796`
  (`layout_fields_are_tuple_style`) returns `false` for any
  bridged tuple struct (because empty strings don't match
  `i.to_string()`), so tuple structs like `pub struct Pair(u32, u32)`
  would render as `Pair { : a, : b }` rather than
  `Pair(a, b)`. (No current snapshot exercises a tuple
  struct, so this is latent.)

### What is NOT broken

The pretty-printer is correct.
`crates/regseq-model/src/pretty_tests.rs:1511-1530` constructs
a model directly with named `FieldLayout` entries
(`make_named_struct(m, "geom::Point", [("x", i32_ty), ("y", i32_ty)])`)
and asserts the output is
`"geom::Point { x: copy _0, y: copy _1 }"`. The fix is
strictly bridge-side; no renderer changes are required.

## Input Context

- `crates/regseq-driver/src/bridge/ty.rs:325-329`
  -- the caller in `translate_adt`. Currently:
  ```rust
  let field_tys = adt
      .all_fields()
      .map(|field| translate_ty(cx, field.ty(cx.input.tcx, args)))
      .collect::<Result<Vec<_>, _>>()?;
  let layout = nominal_layout(cx, ty, &field_tys)?;
  ```
  The `field.name: rustc_span::Symbol` is discarded here.

- `crates/regseq-driver/src/bridge/ty.rs:864-898`
  -- `tuple_layout`. Currently interns `let empty = cx.model.strings.intern("");`
  at line 879 and uses it for every tuple field at line 891.

- `crates/regseq-driver/src/bridge/ty.rs:900-938`
  -- `nominal_layout`. Currently interns `let empty = cx.model.strings.intern("");`
  at line 912 and uses it for every nominal field at line 918.

- `crates/regseq-model/src/layout.rs:58-67`
  -- `FieldLayout` schema. The doc-comment at line 59 reads
  "Interned source or synthetic field name" -- source field
  names are the documented expectation for user-defined ADTs.

- `crates/regseq-model/src/pretty.rs:2422-2454`
  -- `render_aggregate` named-struct branch. Reads
  `field_layouts[i].name` and resolves via
  `self.model.strings.resolve(name_id)`. Unchanged by this
  task.

- `crates/regseq-model/src/pretty.rs:2785-2796`
  -- `layout_fields_are_tuple_style`. Tuple-style detection
  requires names `"0"`, `"1"`, ... -- numeric strings, not
  the `_N` form `tuple_layout` will emit. Note the
  asymmetry: rustc tuple-struct field names are `"0"`, `"1"`,
  ... (numeric without underscore), while bare-tuple type
  field names per validator spec are `"_0"`, `"_1"`, ...
  (underscore-prefixed). See V4-2 below.

- `crates/regseq-model/src/layout_validator.rs:419-431`
  -- tuple-field validator. Requires `_0`, `_1`, ..., `_n-1`.

- `crates/regseq-model/tests/snapshots/e2e_synthesis__vtable_render.snap:60`
  -- the ONE existing snapshot that pins the broken form.
  This snapshot must be regenerated after the fix lands. The
  diff is two bounded changes on a single line: the empty
  slot before `:` is filled, AND the brace delimiters change
  to parens (because `StaticByte` is a tuple struct -- see
  Change C for the mechanic). No other lines in the snapshot
  change.

- `crates/regseq-model/src/pretty_tests.rs:1511-1530`
  -- existing renderer test that proves the renderer renders
  named fields correctly. No change.

- `crates/regseq-driver/tests/bridge_ty.rs`
  -- the natural placement for the new
  `FieldLayout.name`-assertion regression test (uses the
  in-process `bridge_fixture` pattern).

- `crates/test-ir-fixtures/src/lib.rs:127-220`
  -- in-process bridge invocation pattern for new
  fixture-driven tests.

### Existing bridge `FieldLayout` constructors (out-of-scope sites)

Codex enumerated every `FieldLayout` constructor in the
bridge crate. These are correct as-is and MUST NOT be touched
by this task:

- `crates/regseq-driver/src/bridge/ty.rs:745-749`
  -- vtable struct slot fields (synthetic; correctly named
  `empty` because vtable slots are accessed by index, not
  name).
- `crates/regseq-driver/src/bridge/ty.rs:840-848`
  -- closure-captures struct. Intentionally interns
  positional names `_0`, `_1`, ... per spec.md
  section 13.1 lines 4527-4529 (audit-finding MEDIUM-40-3).
- `crates/regseq-driver/src/bridge/synth/vtable.rs:609-653`
  -- vtable-type synthesis. Already populates synthetic
  names like `__drop_in_place`, method names, and
  `__vacant_N`.
- `crates/regseq-driver/src/bridge/body/vtable_indirect.rs:447-533`
  -- `#[cfg(test)]`-only test scaffolding with synthetic
  name `"call"`. The production vtable-indirect rewrite
  path at `crates/regseq-driver/src/bridge/body/vtable_indirect.rs:51-107`
  works with locals/projections and does not construct
  `FieldLayout`s.

## Implementation Notes

Two changes in dependency order so each is testable in
isolation.

### Change A: Thread source field names through `nominal_layout`

Change `nominal_layout`'s signature so the caller passes
`(Symbol, TyId)` pairs rather than bare `TyId`s.

**Step A1.** Update the caller at
`crates/regseq-driver/src/bridge/ty.rs:325-329` from:

```rust
let field_tys = adt
    .all_fields()
    .map(|field| translate_ty(cx, field.ty(cx.input.tcx, args)))
    .collect::<Result<Vec<_>, _>>()?;
let layout = nominal_layout(cx, ty, &field_tys)?;
```

to (sketch -- exact shape per `cargo fmt`):

```rust
let fields = adt
    .all_fields()
    .map(|field| {
        let ty = translate_ty(cx, field.ty(cx.input.tcx, args))?;
        Ok((field.name, ty))
    })
    .collect::<Result<Vec<(rustc_span::Symbol, TyId)>, BridgeError>>()?;
let layout = nominal_layout(cx, ty, &fields)?;
```

**Step A2.** Update `nominal_layout`'s signature at
`crates/regseq-driver/src/bridge/ty.rs:900-938` from:

```rust
fn nominal_layout<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
    field_tys: &[TyId],
) -> Result<Layout, BridgeError> {
```

to:

```rust
fn nominal_layout<'tcx>(
    cx: &mut BridgeCtxt<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
    fields: &[(rustc_span::Symbol, TyId)],
) -> Result<Layout, BridgeError> {
```

**Step A3.** In the body, replace the `let empty = cx.model.strings.intern("");`
line and the `FieldLayout { name: empty, ty, ... }` constructor
with per-field name interning. The current body at lines 912-925
becomes:

```rust
let field_layouts = fields
    .iter()
    .copied()
    .enumerate()
    .map(|(idx, (name, ty))| {
        let name_id = cx.model.strings.intern(name.as_str());
        FieldLayout {
            name: name_id,
            ty,
            offset: if idx < rustc_layout.fields.count() {
                rustc_layout.fields.offset(idx).bytes()
            } else {
                0
            },
        }
    })
    .collect();
```

The union branch at `bridge/ty.rs:927-930` (which routes the
same `field_layouts` through `LayoutKind::Union { fields }`)
is unchanged.

### Change B: Populate tuple field names

`tuple_layout` at `crates/regseq-driver/src/bridge/ty.rs:864-898`
currently interns `""` for every field. Per
`crates/regseq-model/src/layout_validator.rs:419-431`, tuple
field names must be `_0`, `_1`, ..., `_n-1`.

Replace the `let empty = cx.model.strings.intern("");` line at
`crates/regseq-driver/src/bridge/ty.rs:879` and the
`FieldLayout { name: empty, ... }` constructor at line 891 with
per-index `_N` interning:

```rust
let fields = fields
    .iter()
    .copied()
    .enumerate()
    .map(|(idx, ty)| {
        let name_id = cx.model.strings.intern(&format!("_{idx}"));
        FieldLayout {
            name: name_id,
            ty,
            offset: rustc_layout.fields.offset(idx).bytes(),
        }
    })
    .collect()
```

### Change C: Regenerate the affected snapshot

The lone existing snapshot site is
`crates/regseq-model/tests/snapshots/e2e_synthesis__vtable_render.snap:60`,
which currently reads:

```text
_1 = trait_object::StaticByte { : 66_u8 };
```

`trait_object::StaticByte` is a tuple struct
(`pub struct StaticByte(pub u8);` at
`crates/test-ir-fixtures/fixtures/trait_object.rs:8`).
After the fix:

1. `nominal_layout` (Change A) interns the rustc tuple-struct
   field name `"0"` into `FieldLayout.name`.
2. `layout_fields_are_tuple_style` at
   `crates/regseq-model/src/pretty.rs:2785-2796` returns
   `true` (the field name `"0"` matches `i.to_string()` for
   `i=0`).
3. The renderer at `crates/regseq-model/src/pretty.rs:2450-2453`
   emits tuple form (parens), not named-struct form (braces).

The regenerated line must therefore read:

```text
_1 = trait_object::StaticByte(66_u8);
```

The diff is TWO bounded changes on a single line: the empty
slot before `:` is filled, AND the brace delimiters change
to parens. There are no structural changes elsewhere in the
snapshot.

Regenerate via `cargo insta accept` (or the project's
snapshot-update mechanism per `SESSION.md` Testing
Requirements) after Changes A and B land.

### Conventions

- `Symbol::as_str()` returns `&str`; the existing pattern at
  `crates/regseq-driver/src/bridge/ty.rs:841` for closure
  captures uses `cx.model.strings.intern(&format!("_{idx}"))`,
  so adopt the same `intern(name.as_str())` pattern for
  per-field naming.
- Do not introduce a new `FieldMeta` helper struct; the
  `(Symbol, TyId)` pair is local to the call site and the
  threading is minimal.
- Match the existing `field_tys` variable naming style if
  shadowing it (e.g. rename to `field_meta` or `fields`).

## Non-Goals

- **Enum layout construction.** Codex review identified that
  `translate_adt` at `crates/regseq-driver/src/bridge/ty.rs:331-340`
  routes `Ty::Enum` through the same `nominal_layout` which
  returns `LayoutKind::Struct`, while the enum renderer at
  `crates/regseq-model/src/pretty.rs:2456-2495` and validator
  at `crates/regseq-model/src/layout_validator.rs:523-534`
  require `LayoutKind::Enum { shape }` with per-variant
  `VariantLayout`s. This is meaningfully larger bridge work
  (full `EnumShape::Single` / `EnumShape::Multiple` construction
  with per-variant fields) and is OUT of scope for this task.
  File as a separate `regseq-driver-NN` follow-up.
- **Renderer changes.** The pretty-printer is correct. No
  edits to `crates/regseq-model/src/pretty.rs` or any other
  renderer code are in scope.
- **`FieldLayout` schema changes.** `crates/regseq-model/src/layout.rs:58-67`
  is the canonical schema; the `name: StringId` field is
  already in place. No model-crate edits except the snapshot
  regeneration.
- **VTable / closure / synthesized FieldLayout sites.** Out
  of scope per the enumerated list above; each has a
  documented synthetic-name policy.
- **Tuple-style detection refactor.**
  `layout_fields_are_tuple_style` at
  `crates/regseq-model/src/pretty.rs:2785-2796` uses numeric
  string check (`"0"`, `"1"`, ...) for rustc tuple-struct
  detection. This is correct as-is: bridged tuple
  STRUCTS (`pub struct Pair(u32, u32)`) flow through
  `nominal_layout` (Change A) and inherit the rustc field
  names `"0"`, `"1"`, ... from `FieldDef.name`. Bridged BARE
  TUPLES (`(u32, u32)`) flow through `tuple_layout`
  (Change B) and get `"_0"`, `"_1"`, ... names per validator
  spec -- bare tuples are NOT struct-style aggregates and
  do not route through `AggregateKind::Struct` rendering,
  so the tuple-style asymmetry is benign.

## Test Plan

### A. Compilation gate

1. `cargo build --workspace` succeeds after Changes A and B.
   (Catches signature drift if `nominal_layout` callers exist
   outside the one identified site.)

### B. Bridge regression (new unit tests in `bridge_ty.rs`)

2. New unit test in
   `crates/regseq-driver/tests/bridge_ty.rs` exercising a
   single-struct fixture via the in-process `bridge_fixture`
   pattern. Asserts that after bridging
   `pub struct Counter { pub value: u32 }`, the resulting
   `Ty::Struct.layout.kind` is `LayoutKind::Struct` whose
   first `FieldLayout.name` resolves to `"value"`.

3. New unit test in
   `crates/regseq-driver/tests/bridge_ty.rs` exercising a
   tuple struct (`pub struct Pair(pub u32, pub u32)`).
   Asserts the bridged `FieldLayout.name`s resolve to `"0"`
   and `"1"` (rustc's source-level field names for tuple
   structs, NOT the `_0`/`_1` form which is only for bare
   tuples per Change B).

4. New unit test for a bare tuple
   (`pub fn f() -> (u32, u32) { (1, 2) }`): bridged tuple
   `FieldLayout.name`s must resolve to `"_0"` and `"_1"` per
   the validator contract.

### C. Snapshot regeneration

5. After Changes A and B land, the snapshot at
   `crates/regseq-model/tests/snapshots/e2e_synthesis__vtable_render.snap`
   must be regenerated. The diff is bounded to a single
   line (line 60): the broken `{ : 66_u8 }` form becomes the
   tuple-form `(66_u8)` per Change C (delimiters change from
   braces to parens because rustc emits tuple-struct field
   name `"0"`, which triggers
   `layout_fields_are_tuple_style` -> tuple-form rendering).
   No other lines in the snapshot change.

### D. Workspace regression

6. `cargo test --workspace` -- all existing tests pass with
   the regenerated snapshot.
7. `cargo fmt --all -- --check` -- clean.
8. `cargo clippy --workspace --tests -- -D warnings` --
   no NEW lint sites in any modified file beyond the
   pre-existing baseline.

### E. End-to-end render proof (playground sanity)

9. Manual: invoke `regseq-driver` directly on the test
   fixture per
   `crates/test-ir-fixtures/src/lib.rs:127-220`'s pattern
   on a hand-rolled source containing
   `pub struct Counter { pub value: u32 }`, dump the
   rendered model, confirm the line reads
   `Counter { value: 0_u32 }` (not `Counter { : 0_u32 }`).
   This is verification-only, not a checked-in test (the
   B3 unit test covers the contract assertion).

## Review

Once tests pass, request a Codex review of the implementation
by invoking `scripts/python/codex_runner.py` via the Bash
tool (per `SESSION.md` Review section). Use the four-block
template verbatim:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-driver-53 against its
task spec at
.harness/tasks/regseq-driver/53-bridge-nominal-and-tuple-field-names.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-driver/src/bridge/ty.rs (nominal_layout,
  tuple_layout, and the translate_adt caller at lines
  325-329)
- crates/regseq-driver/tests/bridge_ty.rs (new B2-B4
  regression tests)
- crates/regseq-model/tests/snapshots/e2e_synthesis__vtable_render.snap
  (regenerated; line 60 now reads
  `_1 = trait_object::StaticByte(66_u8);` and no
  `{ : 66_u8 }` substring remains; no other lines change)

Focus on:
- The (Symbol, TyId) threading must come from `field.name`
  on rustc's `FieldDef`, not synthesized.
- `nominal_layout` per-field name interning must use
  `cx.model.strings.intern(name.as_str())`.
- `tuple_layout` per-field name interning must use
  `format!("_{idx}")`.
- No changes to any other FieldLayout constructor (vtable
  slot, closure captures, vtable synth, vtable_indirect
  test-only).
- No renderer changes.
- Snapshot line 60 changes to tuple-form
  `_1 = trait_object::StaticByte(66_u8);`; no other
  snapshot lines change.

Do NOT flag anything explicitly marked out of scope (enum
layout, renderer changes, vtable / closure sites).
</task>

<grounding_rules>
Cite file:line for every claim. No speculation. Mark
hypotheses as hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what
was verified and why any deliberate deviations are
acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a
verdict, stop.
</default_follow_through_policy>
EOF
```

Iterate (fix -> re-invoke `codex_runner.py`) until
VERDICT: APPROVED.

## Quality Gate

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --tests -- -D warnings
```

The authoritative gate is `task.py done`, which runs cleanup
plus workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements.

## Acceptance Criteria

- `crates/regseq-driver/src/bridge/ty.rs:900-938`
  (`nominal_layout`) accepts `&[(rustc_span::Symbol, TyId)]`
  and interns each field's `Symbol::as_str()` into
  `FieldLayout.name`.
- `crates/regseq-driver/src/bridge/ty.rs:325-329`
  (`translate_adt` caller) collects `(field.name, translate_ty(...))`
  pairs and passes them to `nominal_layout`.
- `crates/regseq-driver/src/bridge/ty.rs:864-898`
  (`tuple_layout`) interns per-index `_N` names
  (`"_0"`, `"_1"`, ...) into `FieldLayout.name`.
- New B2 unit test (`crates/regseq-driver/tests/bridge_ty.rs`)
  asserts bridged `Counter`'s `FieldLayout.name` resolves to
  `"value"`.
- New B3 unit test asserts bridged tuple struct `Pair`'s
  `FieldLayout.name`s resolve to `"0"` and `"1"`.
- New B4 unit test asserts bridged bare-tuple `(u32, u32)`'s
  `FieldLayout.name`s resolve to `"_0"` and `"_1"`.
- `crates/regseq-model/tests/snapshots/e2e_synthesis__vtable_render.snap:60`
  no longer contains the substring `{ : 66_u8 }` and instead
  renders the `StaticByte` construction in tuple-struct form
  `_1 = trait_object::StaticByte(66_u8);` (the rest of the
  snapshot is byte-identical to its pre-fix content).
- All other bridge `FieldLayout` constructors (vtable slot at
  `ty.rs:745`, closure captures at `ty.rs:840`, vtable synth
  at `synth/vtable.rs:649`, vtable-indirect test-only at
  `body/vtable_indirect.rs:529`) are unchanged.
- `cargo test --workspace` regression-clean.
- `cargo fmt --all -- --check` and
  `cargo clippy --workspace --tests -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
