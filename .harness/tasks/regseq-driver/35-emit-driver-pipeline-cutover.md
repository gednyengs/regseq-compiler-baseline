# regseq-driver-35: Pipeline Cutover in `emit_driver.rs` -- Swap to `normalize()` + `populate_bindings()`

## Description

Phase-7 pipeline cutover. This task is the integration choke point
for the RegseqModel refactor: after it lands, the runtime pipeline
is

    normalize(raw_artifact) -> populate_bindings(&mut model, platform) -> consumers

with the legacy pipeline (`declaration::normalize_from_json`,
`binding::normalize`, `perform_join`) removed from the hot path.

Three independent changes ship together because their correctness
depends on each other:

1. **`emit_driver.rs`**: swap the legacy three-step pipeline for
   the Phase-6 two-step pipeline.
2. **`AnalysisInput`**: add `AnalysisInput::from_regseq_model(&RegseqModel)
   -> AnalysisInput`. The legacy `AnalysisInput::new(decl, joined,
   bus_binding)` ctor stays alive (Phase 9 deletes it) so tests
   that construct `AnalysisInput` directly are not broken
   mid-migration; `emit_driver.rs` stops calling it.
3. **`compat.rs`**: fill in every `new_from_regseq_model` arm and
   every `RegseqBacking` match arm that task `regseq-model-61`
   stubbed with `unimplemented!()`.

The `normalize()` function internally verifies Phase-1 invariants
and `populate_bindings()` internally verifies Phase-2 invariants on
commit (per `.harness/tasks/regseq-model/58-normalize.md` and
`.harness/tasks/regseq-model/59-populate-bindings.md`), so
`emit_driver.rs` does not emit an explicit verifier call. Writing
one here would be incorrect (Codex fix 2).

## Input Context

- `.harness/spec-batches/regseqmodel-phase-7.md` lines 101-103
  (integration choke point); line 20-22 ("pipeline glue -- swap
  to normalize() + populate_bindings() from Phase 6"); lines
  148-150 (pipeline-glue grep gate).
- `docs/discussions/models/regseq-model/execution-plan.md` Phase 7
  block.
- `.harness/tasks/regseq-model/58-normalize.md` -- `normalize()`
  signature and internal Phase-1 verify contract.
- `.harness/tasks/regseq-model/59-populate-bindings.md` --
  `populate_bindings()` signature and internal Phase-2 verify on
  commit.
- `.harness/tasks/regseq-model/60-pl011-pipeline-end-to-end.md` --
  the end-to-end pipeline template.
- `.harness/tasks/regseq-model/61-compat-scaffold.md` -- the view
  surface this task finishes.
- `.harness/tasks/regseq-analysis/06-migrate-to-regseq-model-compat.md`
  -- the existing `AnalysisInput::new` signature stays, but a new
  ctor is added here.
- `crates/regseq-driver/src/emit_driver.rs` lines 69-120 --
  the current pipeline; specifically:
    - Line 78: `declaration::normalize_from_json(&json)` (remove).
    - Line 88: `binding::normalize(&pf, ...)` (remove).
    - Line 94: `perform_join(&decl, &binding_model)` (remove).
    - Line 97: `AnalysisInput::new(decl, joined, bus_binding)`
      (swap to the new ctor).
- `crates/regseq-model/src/compat.rs` (landed by task
  `regseq-model-61`) -- all `new_from_regseq_model` arms need to
  become concrete.
- `crates/regseq-model/src/{normalize,join}/` -- the Phase-6
  entry points.

## Implementation Notes

### Change 1: `emit_driver.rs` pipeline swap

Delete the three legacy calls; insert the Phase-6 two-step:

```rust
// Before (lines 78, 88, 94 today):
let decl = declaration::normalize_from_json(&json)?;
let binding_model = binding::normalize(&pf, Some(&platform_source),
                                       declaration_bus)?;
let joined = perform_join(&decl, &binding_model)?;
let input = AnalysisInput::new(decl, joined, bus_binding);

// After:
let mut model = regseq_model::normalize(&raw_artifact)?;
regseq_model::populate_bindings(&mut model, &platform_file)?;
let input = AnalysisInput::from_regseq_model(&model);
```

Callers of `emit_driver` pass `raw_artifact` and `platform_file` as
they do today (the existing plumbing that produces a
`RawFrontendArtifact` and a `PlatformFile` is untouched; confirm
before editing).

`normalize()` returning `Err(NormalizeError)` and `populate_bindings()`
returning `Err(JoinError)` both get propagated; `emit_driver.rs`'s
existing error type must absorb them either via `From` impls or a
sum-type conversion. Pick the option that preserves existing error
diagnostics (user-facing messages must not regress). Document the
conversion policy inline as a one-line comment explaining why.

Remove all imports of `declaration::normalize_from_json`,
`binding::normalize`, `perform_join`, `DeclarationModel`,
`BindingModel`, `JoinedArtifact`, and any associated helpers that
are now dead. Grep check:

```
rg -n 'declaration::normalize_from_json|binding::normalize|perform_join' crates/regseq-driver/src/emit_driver.rs
```

must return zero matches.

### Change 2: `AnalysisInput::from_regseq_model`

Add to `crates/regseq-analysis/src/input.rs`:

```rust
impl AnalysisInput {
    pub fn from_regseq_model(model: &RegseqModel) -> AnalysisInput {
        // Build the same internal state the legacy ctor builds,
        // but project everything from `model` via `compat` views.
    }
}
```

The legacy `AnalysisInput::new(decl, joined, bus_binding)` ctor
stays in place -- task `regseq-analysis-06` kept the signature
alive on purpose. Phase 9 deletes the legacy ctor.

**Internal representation**: pick one of the two storage shapes
based on what the crate's borrow-checker accepts cleanly.

1. Preferred: store `RegseqModel` (or `Arc<RegseqModel>`) plus
   derived cached structures. Every `AnalysisInput::declaration()`,
   `joined()`, etc. returns a `compat::*View<'_>` borrowing from
   the stored model.
2. Alternative: materialize legacy-shaped owned types from the
   model on the fly inside `from_regseq_model`. Accept the one-time
   copy cost; `emit_driver.rs` calls this once per driver.

Option 1 is ideal; option 2 is acceptable if option 1 causes
lifetime cascades. Pick and document. Do **not** introduce a
third option that changes `AnalysisInput`'s public method
signatures.

### Change 3: `compat.rs` `new_from_regseq_model` implementations

Every facade view in `crates/regseq-model/src/compat.rs` currently
panics in `new_from_regseq_model`. Implement each:

- `DeclarationView::new_from_regseq_model(&RegseqModel)`:
  projects declarations from `model.root_module` and returns a
  view whose accessors read the correct subset.
- `BindingView::new_from_regseq_model(&RegseqModel)`: projects
  Phase-2 binding state (the per-resource `binding` slots and
  the property `bound_value` slots) into the `BindingView`
  accessor surface.
- `JoinedView::new_from_regseq_model(&RegseqModel)`: projects
  the joined "both declared and bound" state, plus the
  decl-only / binding-only asymmetry. Because task
  `regseq-model-59` rejects unmatched platform ids at join time,
  `binding_only` is always empty post-Phase-2 -- document this
  in the implementation (a `debug_assert!(binding_only.is_empty())`
  is appropriate).
- `SemanticView::new_from_regseq_model(&RegseqModel)`: projects
  function bodies from the `model.root_module` module tree.
- `ItemView::new_from_regseq_model(&RegseqModel)`: projects the
  `Item` tree from `model.root_module`.

Delete the `#[should_panic]` tests that task `regseq-model-61`
added for the `new_from_regseq_model` stubs -- they were guarding
the Phase-7 handoff contract and this task is the handoff.

### Equivalence test (Codex fix recommendation)

Add a single end-to-end equivalence test under
`crates/regseq-driver/tests/pipeline_cutover_equivalence.rs`:

```rust
#[test]
fn pl011_legacy_and_regseq_model_inputs_are_equivalent() {
    // Build legacy AnalysisInput via the retained legacy ctor.
    let legacy_input = build_legacy_input_for_pl011();

    // Build new AnalysisInput via the Phase-6 pipeline.
    let raw = load_raw_artifact("tests/fixtures/pl011-full-spec/expected/raw-frontend.json");
    let platform = load_platform_file("tests/fixtures/pl011-full-spec/platform.toml");
    let mut model = regseq_model::normalize(&raw).unwrap();
    regseq_model::populate_bindings(&mut model, &platform).unwrap();
    let new_input = regseq_analysis::AnalysisInput::from_regseq_model(&model);

    assert_analysis_inputs_equivalent(&legacy_input, &new_input);
}
```

The helper `assert_analysis_inputs_equivalent` compares **every
accessor currently exercised by downstream consumers**. The
enumerated list below is the complete set derived from the Phase-7
consumer scan (the brief's audit command in task
`regseq-model-61` Section A); any accessor added to `AnalysisInput`
after this task lands must be added here in the same commit:

- Top-level handles: `declaration`, `joined`, `mir_bodies`.
  (These are the primary handles read by
  `regseq-backends/src/lowering/planner.rs` and
  `regseq-analysis/src/{readiness,body_readiness}.rs`. Compare
  via serialized-bytes equality because the returned references
  do not implement `PartialEq`.)
- Identity / resource topology: `bus_identity`, `bus_binding`,
  `callback_families_present`, `all_callback_impls`,
  `callbacks`.
- Join summaries: `all_join_summaries`, `property_join_summary`,
  `mmio_join_summary`, `interrupt_join_summary`,
  `clock_join_summary`, `reset_join_summary`,
  `pinctrl_join_summary`.
- Joined entries: `properties_joined`, `mmio_joined`,
  `interrupts_joined`, `clocks_joined`, `resets_joined`,
  `pinctrl_joined`.
- Register + sync surface: `register_blocks`,
  `register_descriptors`, `sync_fields`, `guard_scopes`,
  `completion_operations`.
- Logging + timing: `timed_delay_call_sites`,
  `duration_ctor_sites`, `log_call_sites`.
- Wrapper + polling: `wrapper_call_sites`, `poll_call_sites`.
- Property lints: `property_lints`.
- MIR body query surface: `mir_query_flavor`,
  `mir_callback_bodies`, `mir_helper_bodies`, `mir_isr_bodies`,
  `mir_body_by_name`, `mir_reachable_helper_names`.

Helper shape:

```rust
fn assert_analysis_inputs_equivalent(
    a: &AnalysisInput,
    b: &AnalysisInput,
) {
    assert_eq!(a.bus_identity(), b.bus_identity(), "bus_identity drift");
    assert_eq!(a.bus_binding(), b.bus_binding(), "bus_binding drift");
    assert_eq!(a.callback_families_present(),
               b.callback_families_present(),
               "callback_families_present drift");
    assert_eq!(a.all_callback_impls(),
               b.all_callback_impls(),
               "all_callback_impls drift");
    // ... one assertion per accessor in the enumerated list.
    // For non-PartialEq accessor outputs, serialize each side
    // with serde_json::to_string_pretty and assert_eq! the
    // strings.
}
```

This test catches semantic drift between legacy-bootstrapped and
Phase-6-bootstrapped `AnalysisInput` on the one fixture we have.
The PL011 E2E tests (`pl011_lowering_input_stability.rs`,
`pl011_bus_identity_proof.rs`) transitively cover deeper state;
this scoped test isolates any regression to the cutover.

### What this task does NOT touch

- No emitter / emit_*.rs / c_ast.rs changes (Phase 8).
- No deletion of `declaration::normalize_from_json`,
  `binding::normalize`, `perform_join`, or any old module
  (Phase 9).
- No E2E rewrites (task `track-a-validation-11`).
- No `lowering/planner.rs` or `linux/planner.rs` changes
  (already migrated in tasks 3, 4).
- No `regseq-analysis/src/input.rs` method-signature changes
  on existing accessors (`declaration()`, `joined()`, etc.).
- **Do not modify `crates/regseq-driver/src/construction*`.**

## Non-Goals

- No Phase-8 emission rewrite.
- No Phase-9 cleanup (legacy modules stay alive).
- No changes to `normalize()` or `populate_bindings()` internals
  (their contracts are final per Phase 6).
- No new verifier calls in `emit_driver.rs` -- Phase 6 verifies
  internally.
- No backend-specific legality checks.
- No platform-file parsing changes.

## Test Plan

- `crates/regseq-driver/tests/pipeline_cutover_equivalence.rs`
  passes (legacy-vs-new parity on every enumerated `AnalysisInput`
  accessor).
- `cargo test -p regseq-driver` passes.
- `cargo test -p regseq-analysis` passes (including the ctor
  addition and any new helper tests).
- `cargo test -p regseq-model` passes (the compat
  `new_from_regseq_model` implementations have unit tests added
  here, mirroring the structure of task `regseq-model-61`'s
  legacy-arm tests):

  ```rust
  #[test]
  fn declaration_view_regseq_model_round_trip() {
      let model = build_minimal_pl011_model();
      let view = compat::DeclarationView::new_from_regseq_model(&model);
      assert_eq!(view.all_resource_field_names().len(), 5);
      // ... one assert per accessor on the enumerated list.
  }
  ```

- `cargo test --workspace` passes -- including all 9 E2E tests
  (task `track-a-validation-11` rewrites the legacy-bootstrap
  ones afterward; before that, those E2Es still exercise the
  legacy ctor path, which stays alive).
- Grep gate on the cutover:

  ```
  rg -n 'declaration::normalize_from_json|binding::normalize|perform_join' crates/regseq-driver/src/emit_driver.rs && echo FAIL || echo OK
  rg -n 'regseq_model::(normalize|populate_bindings)' crates/regseq-driver/src/emit_driver.rs
  ```

- Determinism sanity (small, in-task): run the new cutover
  pipeline twice in a unit test; serialize the resulting
  `RegseqModel`; assert byte-equal. Task 60 covers the full
  determinism golden; this is just a smoke check that the cutover
  did not introduce nondeterminism.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-driver-35 against
.harness/tasks/regseq-driver/35-emit-driver-pipeline-cutover.md.

Files to audit:
- crates/regseq-driver/src/emit_driver.rs
- crates/regseq-analysis/src/input.rs (the from_regseq_model ctor
  addition; existing accessors must be unchanged).
- crates/regseq-model/src/compat.rs (new_from_regseq_model
  implementations; stub panics removed).
- crates/regseq-driver/tests/pipeline_cutover_equivalence.rs.
- crates/regseq-model/tests/compat_scaffold.rs (the
  #[should_panic] stubs from regseq-model-61 must be deleted
  here).

Focus on:
- emit_driver.rs no longer calls declaration::normalize_from_json,
  binding::normalize, perform_join, or AnalysisInput::new (the
  legacy ctor).
- emit_driver.rs does NOT emit an explicit verifier call;
  normalize() and populate_bindings() verify internally per
  tasks 58 and 59.
- AnalysisInput::from_regseq_model is the only new ctor; the
  legacy ctor stays alive; no existing accessor signature
  changes.
- Every new_from_regseq_model arm is implemented (none
  unimplemented!()). RegseqBacking match arms all concrete.
- The equivalence test covers every AnalysisInput accessor that
  downstream consumers (regseq-backends) read.
- No edits to crates/regseq-driver/src/construction*.

Do NOT flag:
- Old modules still present under crates/regseq-model/src/
  (Phase 9).
- E2E tests still using the legacy ctor (task
  track-a-validation-11).
</task>
... structured_output_contract per SESSION.md ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-driver --test pipeline_cutover_equivalence
cargo test -p regseq-driver
cargo test -p regseq-analysis
cargo test -p regseq-model
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
cargo clippy -p regseq-analysis -- -D warnings
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `emit_driver.rs` contains zero calls to
  `declaration::normalize_from_json`, `binding::normalize`,
  `perform_join`, or `AnalysisInput::new` (the legacy ctor).
- `emit_driver.rs` calls `regseq_model::normalize(&raw)` and
  `regseq_model::populate_bindings(&mut model, &platform)` exactly
  once each per driver emission.
- `emit_driver.rs` emits no explicit verifier call -- the Phase-6
  API verifies internally.
- `AnalysisInput::from_regseq_model(&RegseqModel)` exists with
  tests; legacy `AnalysisInput::new(decl, joined, bus_binding)`
  retained for Phase-9 deletion.
- Every `compat::*View::new_from_regseq_model` is implemented
  (no `unimplemented!()` anywhere in `compat.rs`).
- `pipeline_cutover_equivalence.rs` passes, asserting legacy-vs-new
  parity on every enumerated `AnalysisInput` accessor.
- `cargo test --workspace` passes.
- User-facing error diagnostics from `emit_driver.rs` are
  equivalent to pre-task shape (no regression in
  `NormalizeError` / `JoinError` rendering).
- `git diff --name-only main..HEAD` touches only
  `crates/regseq-driver/src/emit_driver.rs`,
  `crates/regseq-analysis/src/input.rs` (and possibly
  `tests/` under that crate),
  `crates/regseq-model/src/compat.rs`,
  `crates/regseq-model/tests/compat_scaffold.rs`,
  `crates/regseq-driver/tests/pipeline_cutover_equivalence.rs`,
  and (possibly) `Cargo.lock`.
- Codex review approved.
