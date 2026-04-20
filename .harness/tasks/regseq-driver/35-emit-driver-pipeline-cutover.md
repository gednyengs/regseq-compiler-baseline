# regseq-driver-35: `emit_driver.rs` Pipeline Cutover

**Depends on**: `track-a-validation-12` (blessed reference must
exist before cutover proof test can assert against it).

## Description

Phase-7D pipeline cutover. Replaces the legacy three-step emission
pipeline in `emit_driver.rs` (`declaration::normalize_from_json` +
`binding::normalize` + `perform_join` + `AnalysisInput::new` +
legacy `build_driver_lowering_plan`) with the new pipeline:
`normalize() -> populate_bindings() ->
build_driver_lowering_plan_from_model() -> build_linux_backend_plan()
-> emit_linux_artifacts()`.

After this task, `emit_driver.rs` is the ONLY production caller of
the new pipeline, and the legacy path has no in-production consumer.
Legacy lowering entry + `declaration`/`binding`/`joined`/`semantic`/
`item` modules remain compilable (deletions land in tasks 14, 15),
but nothing in the production pipeline touches them.

## Input Context

- `docs/discussions/models/regseq-model/migration.md` Q6
  (`emit_driver` is the LAST cut, not the first).
- `.harness/tasks/regseq-model/58-normalize.md` +
  `59-populate-bindings.md` -- Phase-6 pipeline entry points.
- `.harness/tasks/regseq-lowering/10-new-lowering-entry-from-regseq-model.md`
  -- upstream new lowering entry.
- `.harness/tasks/regseq-linux-backend/03-linux-plan-self-sufficiency.md`
  -- upstream Linux-plan self-sufficiency.
- `.harness/tasks/regseq-linux-backend/05-emit-module-cfg-rewrite-and-emit-behavior-delete.md`
  -- upstream emitter.
- `.harness/tasks/track-a-validation/12-pl011-reference-blessing-gate.md`
  -- upstream blessing must have passed before this task runs.
- `crates/regseq-driver/src/emit_driver.rs` lines 69-120 -- the
  function being cut over.
- `crates/regseq-driver/src/raw_artifact.rs` (or equivalent) --
  the `RawFrontendArtifact` shape `normalize()` consumes.

## Implementation Notes

### The swap

```rust
// Before (current emit_driver.rs lines 73-118):
let decl = declaration::normalize_from_json(&json)?;
let pf = platform::parse(&platform_source)?;
let binding_model = binding::normalize(&pf, Some(&platform_source),
                                       declaration_bus)?;
let joined = perform_join(&decl, &binding_model)?;
let input = AnalysisInput::new(decl, joined, bus_binding);
let plan = build_driver_lowering_plan(&input, ...)?;
let linux_plan = build_linux_backend_plan(&plan, ...)?;
let artifacts = emit_linux_artifacts(&linux_plan, &plan, out_dir)?;

// After:
let raw: RawFrontendArtifact = load_raw_artifact(&json)?;
let pf = platform::parse(&platform_source)?;
let mut model = regseq_model::normalize(&raw)?;
regseq_model::populate_bindings(&mut model, &pf)?;
let plan = build_driver_lowering_plan_from_model(&model, &config)?;
let linux_plan = build_linux_backend_plan(&plan, ...)?;
let bodies = FunctionBodyIndex::from_regseq_model(&model);
let artifacts = emit_linux_artifacts(&linux_plan, &bodies, out_dir)?;
```

### Error-type absorption

`normalize()` returns `Result<RegseqModel, NormalizeError>`.
`populate_bindings()` returns `Result<(), JoinError>`. These
must flow into `emit_driver`'s existing `EmitDriverError` variant.
Add `From<NormalizeError>` + `From<JoinError>` impls or wrap
inline. Preserve user-facing error messages (no regression in
diagnostic quality).

### Legacy calls removed from emit_driver

After this task, `emit_driver.rs` contains NO references to:

- `declaration::normalize_from_json`
- `binding::normalize`
- `perform_join`
- `AnalysisInput::new`
- legacy `build_driver_lowering_plan`

Grep gate:

```
rg -n 'normalize_from_json|binding::normalize|perform_join|AnalysisInput::new|build_driver_lowering_plan[^_]' crates/regseq-driver/src/emit_driver.rs
# zero matches (note: build_driver_lowering_plan_from_model is OK;
# the [^_] in the pattern excludes the new entry from the gate)
```

### Phase-6 auto-verify semantics preserved

`normalize()` runs `verify_phase_1` internally on the returned
model; `populate_bindings()` runs `verify_phase_2` on
`MutationGuard::commit`. No explicit verifier call in
`emit_driver.rs` is needed (or correct -- doing so would be
redundant work that could double-fire rollback).

Per task 5 (`regseq-model-62`), both verifiers return
`Result<VerifyReport, VerifierError>`. `emit_driver.rs` either:

- Ignores `VerifyReport.lints` (current behavior equivalent).
- Surfaces them through `EmitDriverResult` to a caller that
  wants reviewer diagnostics. Pick whichever matches Phase-6
  caller expectations.

### Legacy path does not disappear yet

Although `emit_driver` no longer calls the legacy path, the
legacy functions + modules still exist:

- `declaration::normalize_from_json`, `binding::normalize`,
  `perform_join` still compile (they're still referenced by
  tests that were not yet migrated, e.g., some of the test
  fixtures; task 13 handles those).
- Legacy `build_driver_lowering_plan` entry still exists (deleted
  in task 14).
- `declaration/`, `binding/`, `joined/`, `semantic/`, `item/`
  modules still exist (deleted in task 15).

Task 13 (E2E retarget) migrates the remaining test-side callers of
the legacy APIs. Task 14 deletes the legacy lowering entry +
downstream submodules. Task 15 deletes the old model modules.

### Post-cut regression test

Add `crates/regseq-driver/tests/pipeline_cutover_proof.rs`:

```rust
#[test]
fn emit_driver_runs_new_pipeline_on_pl011() {
    // Run emit_driver on the PL011 fixture.
    // Assert output matches the blessed normalized_reference.c.
    // Asserts the cutover is complete and equivalent.
}
```

This catches the case where emit_driver silently still calls the
old path; if it did, the output would not match the blessed
reference.

### What this task does NOT do

- Does not delete anything; deletions are tasks 14, 15.
- Does not rewrite E2E tests (task 13).
- **`crates/regseq-driver/src/construction*`** strict non-goal.

## Non-Goals

- No emitter semantic changes.
- No legacy deletion.
- No Phase-8 work.

## Test Plan

- **Cutover proof**: `emit_driver` output byte-equals the blessed
  `normalized_reference.c`.
- **Error paths**: malformed `RawFrontendArtifact` produces
  `EmitDriverError::Normalize(_)`; malformed platform file
  produces `EmitDriverError::Join(_)`.
- **Grep gate**: `emit_driver.rs` has zero references to the
  five legacy call names.
- `cargo test -p regseq-driver` green.
- `cargo test --workspace` green.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-driver-35 against .harness/tasks/regseq-driver/35-emit-driver-pipeline-cutover.md.

Files:
- crates/regseq-driver/src/emit_driver.rs
- crates/regseq-driver/tests/pipeline_cutover_proof.rs

Focus on:
- Zero references to normalize_from_json / binding::normalize /
  perform_join / AnalysisInput::new / build_driver_lowering_plan
  (without _from_model suffix) in emit_driver.rs.
- emit_driver produces byte-equal output to the blessed
  normalized_reference.c (the cutover proof test).
- Error diagnostics preserved (NormalizeError + JoinError flow
  through cleanly).
- Phase-6 verifiers not re-invoked explicitly in emit_driver.

Do NOT flag:
- Legacy modules / entries still existing (tasks 14, 15).
- Test files that still use legacy paths (task 13).
</task>
... (standard grounding + structured output) ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-driver --test pipeline_cutover_proof
cargo test -p regseq-driver
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
rg -n 'normalize_from_json|binding::normalize|perform_join|AnalysisInput::new|build_driver_lowering_plan[^_]' crates/regseq-driver/src/emit_driver.rs
# must print nothing
```

## Acceptance Criteria

- `emit_driver.rs` swapped per the Implementation Notes.
- Zero references to the five legacy call names in
  `emit_driver.rs`.
- Cutover proof test passes (byte-equal to blessed reference).
- User-facing error diagnostics preserved.
- `cargo test --workspace` green.
- Codex review APPROVED.
