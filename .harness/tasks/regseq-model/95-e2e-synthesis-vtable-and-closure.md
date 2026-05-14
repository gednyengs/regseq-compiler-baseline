# regseq-model-95: E2E -- synthesis fixture (vtable + closure) (P1a-E2E-03)

## Description

End-to-end integration test that exercises both synthesis families
landed by `regseq-driver-40` (P1a-BR-04) plus the cumulative
phase-1a bridge pipeline (specifically `regseq-driver-43`
(P1a-BR-07)'s vtable-indirect rewrite). One fixture file with two
test functions, per the post-draft Codex right-sizing review:

- **Test (a) -- vtable.** Bridge a function that calls a method
  through `&dyn Reader` and assert the post-bridge `RegseqModel`
  contains the synthesised `VTableType` + `VTableInstance` Items
  (per `spec.md` §13.2; `spec.md:4589+`); the `model.vtables`
  side table maps the `(PrincipalTraitKey, self_ty)` pair to the
  `VTableInstance` `ItemId`; the original `Call` Terminator's
  callee has been rewritten by the vtable-indirect pass (per
  `spec.md` §25.7.2; `spec.md:11642+`) to a
  `CallCallee::Indirect` form with the spec-mandated 4-5 fresh
  locals materialising the receiver-cast + vtable-load + fnptr-
  load chain (`spec.md:11725+`).
- **Test (b) -- closure.** Bridge a function returning `impl
  Fn(u32) -> u32` (the `|x| x + 1` closure expression) and
  assert the post-bridge model contains the synthesised
  `ClosureCaptures` discriminator Item, the parallel
  `Ty::Struct(ClosureCaptures, args, layout)` carrying the
  layout view per stage-3 D6.3, and the `ClosureCall` Function
  Item per `spec.md` §13.1 (`spec.md:4509+`). The `ClosureKind`
  for `|x| x + 1` is `Fn` (no captures, but the call-method
  arm exercised by the fixture is `call`), so a `ClosureCall`
  Item with `call_kind: Call` must be present.

Both tests assert the pretty-printed form against frozen `insta`
snapshots so the rendered output of the synthesised Items
remains stable across phase-1a closure.

## Input Context

- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent brief;
  P1a-E2E-03 row at axis 5)
- `.harness/SESSION.md` (project overview, Review section
  four-block template)
- `docs/discussions/regseq-model/spec.md` §13 (Synthesised Items;
  `spec.md:4503+`)
  - §13.1 (Closure family; `spec.md:4509+`) -- `ClosureCaptures`
    discriminator (Struct ItemKind), `ClosureCall` Function
    Item per `ClosureCallKind` arm, `ClosureDropGlue` only when
    captures need dropping (the fixture's `|x| x+1` has zero
    captures so `ClosureDropGlue` is NOT synthesised).
  - §13.2 (Vtable family; `spec.md:4589+`) -- `VTableType` +
    `VTableInstance` + `VTableShim` per the synthesised vtable
    layout algorithm.
  - §13.3 (`SyntheticItemKind` + `PrincipalTraitKey`;
    `spec.md:4815+`)
  - §13.4 (Synthetic `DefPath` segments + `DefPathHash`
    construction; `spec.md:5031+`) -- the vtable struct DefPath
    form `<regseq_synth>::vtable::<Trait>::for::<ImplTy>` per
    `spec.md:5067-5077`.
- `docs/discussions/regseq-model/spec.md` §25.7.2 (Vtable-indirect
  rewrite; `spec.md:11642+`) -- the post-construction Phase-5
  pass that consumes the vtable Items and rewrites every
  `Call(Direct(virtual_method))` into a `Call(Indirect(fnptr))`
  with 4-5 fresh locals.
- `docs/discussions/regseq-model/spec.md` §3 root struct's
  `vtables: HashMap<PrincipalTraitKey, ItemId>` field
  (`spec.md:347+`) -- the side table the test asserts against.
- `docs/discussions/regseq-model/pretty-printer.md` §6.4 (ADT
  rendering for the synthesised `VTableType` struct + the
  `ClosureCaptures` struct; `pretty-printer.md:662+`)
- `docs/discussions/regseq-model/pretty-printer.md` §10.2.4
  (ADT-definition Instance rendering; the synthesised structs'
  pretty form)
- `docs/discussions/regseq-model/pretty-printer.md` §10.3
  (top-level `render(&self)` the snapshot is taken against)
- `crates/test-ir-fixtures/src/lib.rs` and
  `crates/test-ir-fixtures/fixtures/{trait_object,closure}.rs`
  (the fixture files + helper API shipped by
  `test-ir-fixtures-01`)
- `crates/regseq-model/src/lib.rs` (the `RegseqModel`,
  `Item`, `ItemKind`, `SyntheticItemKind`, `ClosureCallKind`,
  `PrincipalTraitKey`, `Ty::Struct`, `Body`, `Terminator`,
  `TerminatorKind::Call`, `CallCallee::{Direct,Indirect}`
  types this task asserts against)
- `archive/.harness/tasks/regseq-model/24-place-and-projection.md`
  (the SHAPE / SIZE reference)

## Implementation Notes

This task lands a single integration test file at
`crates/regseq-model/tests/e2e_synthesis.rs` housing two `#[test]`
functions. Both tests consume helper-API output from
`test-ir-fixtures`. The file lives under `regseq-model` per the
brief's E2E placement rule (assertion surface = `RegseqModel` +
pretty-printer).

### `crates/regseq-model/Cargo.toml` dev-dependency check

If `regseq-model-94` (P1a-E2E-02) has already added the
`test-ir-fixtures` + `insta` dev-dependencies, no action needed.
Otherwise, add them per the same form documented in
`regseq-model-94`'s Implementation Notes:

```toml
test-ir-fixtures = { path = "../test-ir-fixtures" }
insta = { version = "1", features = ["yaml"] }
```

### `crates/regseq-model/tests/e2e_synthesis.rs`

Test file contents (assertion details enumerated in the Test
Plan below):

```rust
use regseq_model::{
    CallCallee, ClosureCallKind, ItemKind, PrincipalTraitKey,
    RegseqModel, SyntheticItemKind, TerminatorKind, Ty,
};
use test_ir_fixtures::{bridge_fixture, fixture_names, render_default};

#[test]
fn vtable_synthesis_and_indirect_rewrite() {
    let model = bridge_fixture(fixture_names::TRAIT_OBJECT);

    // A. Validation passes.
    model.validate().expect("validate must pass");

    // B. VTableType + VTableInstance Items present.
    let vtable_type = find_vtable_type_item(&model, "Reader");
    let vtable_instance = find_vtable_instance_item(&model, "Reader");

    // C. model.vtables side table maps the principal-trait key.
    assert_vtables_side_table(&model, vtable_instance);

    // D. The Call in `drive(...)` has been rewritten to Indirect.
    let call = find_indirect_call_in(&model, "drive");
    assert_indirect_callee_shape(&model, call);

    // E. 4-5 fresh locals materialised by the rewrite.
    let body = drive_body(&model);
    assert_fresh_local_count(body, 4..=5);

    // F. Snapshot.
    insta::assert_snapshot!("vtable_render", render_default(&model));
}

#[test]
fn closure_synthesis_and_struct_layout_view() {
    let model = bridge_fixture(fixture_names::CLOSURE);

    // A. Validation passes.
    model.validate().expect("validate must pass");

    // B. ClosureCaptures discriminator Item present.
    let captures_item = find_closure_captures_item(&model);
    assert!(matches!(captures_item.kind, ItemKind::Struct { .. }));

    // C. Parallel Ty::Struct(ClosureCaptures, args, layout) present.
    assert_struct_layout_view_present(&model, captures_item);

    // D. ClosureCall Function Item present with call_kind = Call.
    let call_item = find_closure_call_item(&model, ClosureCallKind::Call);
    assert!(matches!(call_item.kind, ItemKind::Function { .. }));

    // E. No ClosureDropGlue (zero captures -> no drop).
    assert_no_closure_drop_glue(&model);

    // F. Snapshot.
    insta::assert_snapshot!("closure_render", render_default(&model));
}
```

The helper functions (`find_vtable_type_item`,
`find_vtable_instance_item`, `assert_vtables_side_table`,
`find_indirect_call_in`, `assert_indirect_callee_shape`,
`drive_body`, `assert_fresh_local_count`,
`find_closure_captures_item`,
`assert_struct_layout_view_present`, `find_closure_call_item`,
`assert_no_closure_drop_glue`) live as private module-local
helpers in the same test file; their expected behaviour is
specified by the Test Plan below.

### Conventions

- Doc comments not required on integration-test private
  helpers; required on any `pub`-marked helper.
- Snapshot files land at
  `crates/regseq-model/tests/snapshots/e2e_synthesis__vtable_render.snap`
  and
  `crates/regseq-model/tests/snapshots/e2e_synthesis__closure_render.snap`.
- The `<regseq_synth>` reserved-prefix segment naming for
  synthesised vtable Items is per `spec.md:5055+`; helpers
  searching for synthesised Items must NOT pattern-match on
  rustc-derived `DefPath` segments for the vtable family
  (these are entirely synthetic per §13.4) but MAY pattern-
  match on the trait name embedded in the segment (e.g.,
  `Reader` extracted from
  `<regseq_synth>::vtable::Reader::for::<dyn Reader>`).
- The `ClosureCaptures` synthetic-kind is the **single
  source-derived** discriminator (per `regseq-model-72`'s
  invariant 2 carve-out at `spec.md:5200-5203`); its underlying
  `Item.def_path` is rustc's closure DefPath verbatim, NOT a
  `<regseq_synth>::...` synthetic path. Helpers searching for
  `ClosureCaptures` must look at the `item_synthetic_kinds`
  side table (per `regseq-driver-36` (P1a-BR-01)'s side-table
  population), NOT at the DefPath segments.
- The `vtable-indirect rewrite`'s "4-5 fresh locals" range per
  `spec.md:11725-11744`: 4 for raw-pointer receivers, 5 for
  `Box<dyn Trait>` / `Rc<dyn Trait>` / `Arc<dyn Trait>` style
  (with extra cast). The `dyn Reader` fixture uses a `&dyn
  Reader` reference receiver, so 4 fresh locals is the
  expected count; the assertion uses the inclusive range
  `4..=5` to tolerate either lowering shape rustc may emit.

## Non-Goals

- The bridge synthesis implementation itself (closure synthesis,
  vtable synthesis, Phase-2 working caches, Phase-5 final
  ItemId rekey) is `regseq-driver-40` (P1a-BR-04) territory.
- The vtable-indirect rewrite implementation is
  `regseq-driver-43` (P1a-BR-07) territory; this task only
  asserts the rewrite's output shape.
- The `ClosureDropGlue` Item synthesis (only fires when
  captures need dropping) is exercised by future fixtures, not
  this one. The fixture's `|x| x + 1` has zero captures and
  thus no drop glue.
- Coroutine / `CoroutineClosure` / `CoroutineWitness` paths
  (rejected at the bridge per `spec.md:4568-4569`) are out of
  scope; they have no fixture coverage in phase-1a.
- The `VTableShim` Item (per `spec.md` §13.2) is the
  per-virtual-method shim. The fixture's `Reader::read_byte`
  trait method generates one `VTableShim` Item; this task does
  not assert on shim Item presence individually -- the
  vtable-indirect Call rewrite assertion (Section D below) is
  the sufficient coverage for shim consumption. Per-shim
  presence is part of `regseq-driver-40`'s own unit tests.
- Backend C-emission of vtables / closures is phase-1b
  territory.
- Snapshot acceptance flow / CI integration of `cargo insta
  review` is out of scope (developer-side, per `regseq-model-94`
  precedent).

## Test Plan

**Test organization**: `integration-in-tests` -- the test file
lives at `crates/regseq-model/tests/e2e_synthesis.rs`. Per the
brief's axis 5 placement rule, E2E tests register under the
`regseq-model` group (assertion surface is the `RegseqModel`
+ pretty-printer).

The test file ships two `#[test]` functions; every assertion is
enumerated below.

### Test (a): `vtable_synthesis_and_indirect_rewrite`

#### A. Validation pass

1. `bridge_fixture(fixture_names::TRAIT_OBJECT)` returns a
   `RegseqModel` without panic.
2. `model.validate()` returns `Ok(())`.

#### B. VTableType + VTableInstance Items present

3. Walk `model.items` and `model.item_synthetic_kinds` jointly.
   Locate the `Item` whose `item_synthetic_kinds[item_id]` is
   `Some(SyntheticItemKind::VTableType { principal: PrincipalTraitKey { trait_def: <Reader's ItemId>, .. }, .. })`.
   Assert exactly one such Item exists.
4. Locate the `Item` whose `item_synthetic_kinds[item_id]` is
   `Some(SyntheticItemKind::VTableInstance { principal:
   PrincipalTraitKey { trait_def: <Reader's ItemId>, .. },
   self_ty: <ArgsKey for the impl Ty>, .. })`. Assert exactly
   one such Item exists.
5. Both Items' `def_path.segments` contain the
   `<regseq_synth>` reserved prefix segment per
   `spec.md:5055+`. The `VTableType` Item's `kind` is
   `ItemKind::Struct { .. }`; the `VTableInstance` Item's
   `kind` is `ItemKind::Static { .. }` (per `spec.md:987` --
   "VTableInstance Items are immutable statics").

#### C. `model.vtables` side-table mapping

6. `model.vtables` (the `HashMap<PrincipalTraitKey, ItemId>`
   from `spec.md:347+`) contains an entry whose key is the
   `PrincipalTraitKey` for `Reader` (with empty
   ProjectionBindings) and whose value is the `ItemId` of the
   `VTableInstance` Item located in step 4. Assert presence
   AND ItemId match.

#### D. Vtable-indirect rewrite output -- the `drive` function's Call

7. Locate the `Function Instance` for `drive(r: &dyn Reader)
   -> u8`. Walk its `Body.basic_blocks` to find the
   single `TerminatorKind::Call` (the `r.read_byte()` call).
8. Assert the Call's `callee` is
   `CallCallee::Indirect(<fnptr local>)`, NOT
   `CallCallee::Direct(_)`. The vtable-indirect rewrite per
   `spec.md` §25.7.2 must have transformed the original
   virtual-method call to an indirect callee form.
9. Assert the Call's `args.len() == 1` (the receiver `r`,
   passed as the loaded fnptr's first argument per the rewrite
   spec).

#### E. 4-5 fresh locals materialised

10. The `drive` function's pre-rewrite `Body` had two
    LocalDecls: `_0` (return) and `_1` (the `r: &dyn Reader`
    parameter). The vtable-indirect rewrite per `spec.md:11725-11744`
    introduces 4-5 fresh LocalDecls materialising the
    receiver-cast + vtable-load + fnptr-load chain:
    - The receiver cast (`*const dyn Trait` -> `*const ()`).
    - The vtable pointer load.
    - The vtable struct cast.
    - The fnptr field load.
    - (Optionally a final cast / reborrow for non-`&dyn`
      receivers; not present for `&dyn Reader`.)
11. Assert `body.local_decls.len() >= 6` (2 original + 4
    fresh; tolerate up to 7 if the bridge happens to emit the
    optional 5th fresh local). The Test Plan uses the
    inclusive `4..=5` fresh-local range per the spec.
12. Assert each fresh local has `debug_name = None` per
    `spec.md:11220-11222` (rewrite-pass-introduced locals carry
    `debug_name = None`).

#### F. Snapshot

13. `insta::assert_snapshot!("vtable_render",
    render_default(&model))` matches
    `crates/regseq-model/tests/snapshots/e2e_synthesis__vtable_render.snap`.
14. Snapshot baseline shape (verifiable by inspection):
    - The synthesised `VTableType` struct is rendered per
      `pretty-printer.md` §6.4 + §10.2.4 with the
      `<regseq_synth>::vtable::Reader::for::...` DefPath
      head.
    - The synthesised `VTableInstance` static is rendered as
      an `ItemKind::Static` per `pretty-printer.md` §10.2.2
      with its initializer composing the per-method shim
      function pointers.
    - The `drive` function's body shows the post-rewrite
      indirect-Call form with the 4-5 fresh-local chain
      visible (loads + casts) and the final `call (*<fnptr>)
      (...)` operand pair per `pretty-printer.md` §7.7
      `render_call_callee` Indirect arm.

### Test (b): `closure_synthesis_and_struct_layout_view`

#### A. Validation pass

15. `bridge_fixture(fixture_names::CLOSURE)` returns a
    `RegseqModel` without panic.
16. `model.validate()` returns `Ok(())`.

#### B. `ClosureCaptures` discriminator Item present

17. Walk `model.item_synthetic_kinds` and locate the entry
    whose value is `Some(SyntheticItemKind::ClosureCaptures
    { closure: <DefPathHash> })`. Assert exactly one such
    entry exists (the fixture has one closure expression
    `|x| x + 1`).
18. The corresponding `Item.kind` is `ItemKind::Struct { ..
    }` (per `spec.md:4513-4529`).
19. Per the §3.1 invariant 2 carve-out at `spec.md:5200-5203`,
    the `ClosureCaptures` Item's `def_path` is rustc's
    closure DefPath verbatim (NOT a `<regseq_synth>::...`
    synthetic path). Assert no `<regseq_synth>` segment in
    this Item's `def_path.segments`.

#### C. Parallel `Ty::Struct(ClosureCaptures, args, layout)` view

20. Walk every `Ty` in the type interner. Locate at least one
    `Ty::Struct(item, args, layout)` whose `item` matches the
    `ClosureCaptures` Item's `ItemId` from step 17. Assert
    exactly one such `Ty` exists (the canonical layout view
    per stage-3 D6.3).
21. Assert the `layout` carries a non-empty
    `Layout` (size + align resolved per `spec.md` §11). For
    the `|x| x + 1` zero-capture closure, the layout is the
    unit-sized struct (`size = 0`, `align = 1`), exercising
    the canonical UNIT_LAYOUT path per `spec.md:3849-3854`.

#### D. `ClosureCall` Function Item present (call_kind = Call)

22. Walk `model.item_synthetic_kinds` for the entry whose
    value is `Some(SyntheticItemKind::ClosureCall { closure:
    <same DefPathHash from step 17>, call_kind:
    ClosureCallKind::Call })`. Assert exactly one such entry
    exists. (The closure's `ClosureKind` is `Fn` per the
    `impl Fn(u32) -> u32` return type, so per
    `regseq-model-72`'s spec the synthesised arms are
    `Call` plus the inherited `CallMut` and `CallOnce`.)
23. Assert the corresponding `Item.kind` is
    `ItemKind::Function { .. }` (per `spec.md:4531-4566`).
24. Assert at least one of the inherited arms (`CallMut`,
    `CallOnce`) is also synthesised; assert presence of at
    least 1 additional `ClosureCall` entry beyond the `Call`
    arm. (Per `spec.md:4543` -- `Fn`'s synthesised set is
    `{Call, CallMut, CallOnce}`. The exact set fired is
    rustc-driven; the test asserts >=2 total `ClosureCall`
    entries to tolerate either rustc lowering.)
25. The `Item`'s `def_path` carries a `<regseq_synth>` segment
    per `spec.md:5067-5077` (synthetic call-method DefPath).

#### E. No `ClosureDropGlue`

26. Walk `model.item_synthetic_kinds` and assert NO entry has
    value `Some(SyntheticItemKind::ClosureDropGlue { .. })`.
    The `|x| x + 1` closure captures nothing; per
    `spec.md:4554` `ClosureDropGlue` is synthesised iff the
    captures need dropping, so the fixture must NOT produce
    one.

#### F. Snapshot

27. `insta::assert_snapshot!("closure_render",
    render_default(&model))` matches
    `crates/regseq-model/tests/snapshots/e2e_synthesis__closure_render.snap`.
28. Snapshot baseline shape (verifiable by inspection):
    - The `ClosureCaptures` struct is rendered per
      `pretty-printer.md` §6.4 + §10.2.4 with rustc's closure
      DefPath head (NOT a `<regseq_synth>` head per the
      invariant 2 carve-out).
    - At least one `ClosureCall` Function Instance is
      rendered with a `<regseq_synth>::...` DefPath head per
      `pretty-printer.md` §6.4.
    - The `make_inc` Function Instance is rendered with the
      closure-construction Rvalue (an `Aggregate(_)` over the
      `ClosureCaptures` struct -- empty in this case) per
      `pretty-printer.md` §7.3 `render_rvalue`.
    - No `ClosureDropGlue` Function Instance appears.

`cargo test -p regseq-model --test e2e_synthesis` passes (both
`vtable_synthesis_and_indirect_rewrite` and
`closure_synthesis_and_struct_layout_view`).
`cargo test --workspace` regression-clean.

## Review

Once tests pass, request a Codex review of the implementation by
invoking `scripts/python/codex_runner.py` via the Bash tool (per
`SESSION.md` Review section). Use the four-block template
verbatim:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-95 against its task spec
at .harness/tasks/regseq-model/95-e2e-synthesis-vtable-and-closure.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/Cargo.toml (dev-dependencies; idempotent if
  regseq-model-94 already added them)
- crates/regseq-model/tests/e2e_synthesis.rs
- crates/regseq-model/tests/snapshots/e2e_synthesis__vtable_render.snap
- crates/regseq-model/tests/snapshots/e2e_synthesis__closure_render.snap

Focus on:
- Spec deviations (every rule in the Implementation Notes section).
- Correctness issues and generality loss; in particular:
  - The vtable test must assert Indirect callee form (NOT Direct)
    AND the 4-5 fresh-local count from the vtable-indirect
    rewrite (spec.md §25.7.2; spec.md:11725+).
  - The closure test must assert the parallel
    Ty::Struct(ClosureCaptures, args, layout) view per D6.3.
  - The closure test must assert NO ClosureDropGlue (zero
    captures).
- Helpers must look up ClosureCaptures via item_synthetic_kinds
  side table (NOT via DefPath segments) per the invariant 2
  carve-out at spec.md:5200-5203.
- Test coverage vs. the spec's Test Plan -- every Test Plan
  enumeration item (A1-F28) must be present.
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs).
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as
hypotheses.
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

Iterate (fix -> re-invoke `codex_runner.py`) until VERDICT: APPROVED.

## Quality Gate

Workspace-wide because this task spans `regseq-model` (assertion
surface + new test file) + `test-ir-fixtures` (helper crate
consumed) + `regseq-driver` (bridge it exercises end-to-end):

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

The authoritative gate is `task.py done`, which runs cleanup plus
workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements. The above commands are dev-time guidance.

## Acceptance Criteria

- `crates/regseq-model/Cargo.toml` `[dev-dependencies]` includes
  `test-ir-fixtures` (path) + `insta` (idempotent if added by
  `regseq-model-94`).
- `crates/regseq-model/tests/e2e_synthesis.rs` created with both
  `#[test]` functions (`vtable_synthesis_and_indirect_rewrite`
  and `closure_synthesis_and_struct_layout_view`) covering every
  Test Plan enumeration item A1 through F28.
- `crates/regseq-model/tests/snapshots/e2e_synthesis__vtable_render.snap`
  and
  `crates/regseq-model/tests/snapshots/e2e_synthesis__closure_render.snap`
  committed; baseline shape per Test Plan F14 / F28.
- `cargo test -p regseq-model --test e2e_synthesis` passes (both
  test functions).
- `cargo test --workspace` regression-clean.
- `cargo fmt --all -- --check` and `cargo clippy --workspace --
  -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
