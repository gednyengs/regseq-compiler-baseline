# regseq-analysis-06: Migrate `regseq-analysis` Imports to `regseq_model::compat`

## Description

Phase-7 consumer migration task 1 of 4. This task rewrites every
`regseq_model::{declaration, binding, joined, semantic, item}` import
in `crates/regseq-analysis/src/` to `regseq_model::compat`. No
signature changes, no behavior changes, no `emit_driver.rs` changes.

`AnalysisInput::new` keeps its existing
`(DeclarationModel, JoinedArtifact, Option<BusBinding>)` signature;
the types simply resolve through `compat` re-exports. During this
task the compat view types are backed by the legacy old-model
instances (task `regseq-model-61`'s `new_legacy` arm). Task
`regseq-driver-35` swaps that backing to `RegseqModel` as part of
the runtime cutover.

Workspace must stay green after this task.

## Input Context

- `.harness/spec-batches/regseqmodel-phase-7.md` lines 93-103
  (per-consumer cutover discipline; "after a consumer's task
  completes, the consumer compiles against `RegseqModel`
  directly or via `compat` and never imports old model types");
  lines 138-142 (analysis grep acceptance, but with corrected
  paths per Codex fix 3 -- see Implementation Notes).
- `docs/discussions/models/regseq-model/execution-plan.md` Phase
  7 section (lines 358-372).
- `.harness/tasks/regseq-model/61-compat-scaffold.md` -- the
  compat surface this task consumes; the re-export and view-API
  list there is authoritative.
- `crates/regseq-analysis/src/input.rs` -- the primary migration
  surface; lines 16-39 carry the current import block that must
  be rewritten. `AnalysisInput` itself (lines 41-298) and its
  accessors stay structurally identical.
- `crates/regseq-analysis/src/readiness.rs` lines 15-39 --
  import block; lines 262-345 -- `DeclarationModel` method
  calls that must resolve through `compat`.
- `crates/regseq-analysis/src/body_readiness.rs` lines 16-79
  -- callback iteration and MIR body capture helpers whose
  types flow through the compat layer.
- `crates/regseq-analysis/Cargo.toml` -- confirm the crate
  already has `regseq-model` as a dep; no Cargo change should
  be needed.

## Implementation Notes

### Import-path-only migration

This is an import-only task. Consumer code BODIES are not
restructured. The compat surface landed by task `regseq-model-61`
re-exports every legacy type **by its original name** (`pub use`);
`compat::DeclarationModel` IS `declaration::DeclarationModel`,
not a new view type. This removes the only lifetime-shape
collision that would otherwise force `AnalysisInput` churn.

- Every `use regseq_model::declaration::...;` → `use
  regseq_model::compat::...;`.
- Every `use regseq_model::declaration::mir_body::...;` → `use
  regseq_model::compat::...;` (MIR body types are re-exported at
  the top level of compat; there is no nested
  `compat::mir_body` path -- simpler import).
- Every `use regseq_model::binding::...;` → `use
  regseq_model::compat::...;`.
- Every `use regseq_model::joined::...;` → `use
  regseq_model::compat::...;`.
- Every `use regseq_model::semantic::...;` → `use
  regseq_model::compat::...;`.
- Every `use regseq_model::item::...;` → `use
  regseq_model::compat::...;`.
- Any fully-qualified reference in the file body (e.g.,
  `regseq_model::declaration::CallbackFamily::Probe`) is rewritten
  to `regseq_model::compat::CallbackFamily::Probe`. Grep the file
  for `regseq_model::(declaration|binding|joined|semantic|item)`
  after editing imports; every residual match is an in-body
  qualified path to rewrite.

### Construction entry points stay on legacy types

`AnalysisInput::new(decl: DeclarationModel, joined:
JoinedArtifact, bus_binding: Option<BusBinding>)` **retains its
existing signature**. Because `compat::DeclarationModel` and
`compat::JoinedArtifact` are `pub use` re-exports of the legacy
owned types, the field types and ctor arguments remain the same
concrete types -- only the import path changes. No lifetime
parameter is introduced; no `'a` propagates through
`AnalysisInput`.

`emit_driver.rs` is untouched in this task -- it continues to
construct `DeclarationModel` and `JoinedArtifact` via the legacy
pipeline and pass them to the unchanged
`AnalysisInput::new(decl, joined, bus_binding)` signature. Task
`regseq-driver-35` owns the signature addition
(`AnalysisInput::from_regseq_model`) and the pipeline swap.

Do **not** introduce `AnalysisInput::from_regseq_model` in this
task. Do **not** change `AnalysisInput`'s field types. Do **not**
wrap legacy owned types in compat view types here -- the view
types exist but are not exercised on any hot path until task
`regseq-driver-35`.

### Grep rationale and residual-qualified-path rule

The grep gate on this task catches both `use` imports and
fully-qualified paths inside function bodies. Running:

```
rg -n -w 'regseq_model::(declaration|binding|joined|semantic|item)' crates/regseq-analysis/src/
```

must return zero matches. If it reports lines inside function
bodies (e.g., match-arm patterns spelling
`regseq_model::declaration::CallbackFamily::Probe`), rewrite
those to `regseq_model::compat::CallbackFamily::Probe`. The audit
command in task `regseq-model-61` Section A was designed to
anticipate exactly these spots; every qualified path the consumer
currently references resolves via a `pub use` in compat.

### Grep acceptance (Codex fix 3)

The brief's wording of the grep check uses `use crate::declaration::`,
which is the old-file-internal path. The real import paths in
`regseq-analysis` are rooted at `regseq_model::`. This task's
acceptance uses the real paths:

```
rg -n -w 'regseq_model::(declaration|binding|joined|semantic|item)' crates/regseq-analysis/src/
```

must return zero matches after this task lands.

A companion positive grep asserts the new path is in use:

```
rg -n 'regseq_model::compat' crates/regseq-analysis/src/
```

must return at least one match per migrated file
(`input.rs`, `readiness.rs`, `body_readiness.rs`).

### What this task does NOT touch

- `crates/regseq-driver/src/emit_driver.rs` -- no changes.
- `crates/regseq-backends/` -- no changes.
- `tests/e2e/` -- no changes.
- `crates/regseq-model/src/compat.rs` -- no changes (if the
  task surfaces a genuine gap, stop and fix the compat scaffold
  in task `regseq-model-61`; do not extend `compat.rs` in this
  task).
- `AnalysisInput::new` signature -- no changes.
- `AnalysisInput` field TYPES -- avoid changing unless option 2
  above is forced.

## Non-Goals

- No pipeline cutover (task `regseq-driver-35`).
- No lowering or Linux backend migration (tasks `regseq-lowering-10`,
  `regseq-linux-backend-03`).
- No E2E rewrites (task `track-a-validation-11`).
- No test additions beyond a small smoke test verifying imports
  resolve (see Test Plan).
- No fixture changes.
- **Do not modify `crates/regseq-driver/src/construction*`.**
- No semantic changes to `AnalysisInput`'s output (if an E2E
  regresses due to a behavior drift introduced here, stop and
  fix the compat view -- do not rebase an E2E golden).

## Test Plan

- `cargo test -p regseq-analysis` passes unchanged. Existing tests
  exercise `AnalysisInput` through
  `DeclarationModel`/`JoinedArtifact` inputs; after this task
  they exercise the same inputs through the compat re-exports
  (same underlying types, different import paths).
  No assertion rewrites expected.
- `cargo test --workspace` passes.
- Add one new unit test in
  `crates/regseq-analysis/tests/compat_import_surface.rs`:

  ```rust
  #[test]
  fn uses_compat_only() {
      // Compile-only smoke: demonstrates that
      // AnalysisInput's public surface resolves through
      // regseq_model::compat. Failing to compile proves the
      // migration is incomplete. The re-exported types are the
      // legacy owned structs (not lifetime-parameterized views),
      // so no `<'_>` appears here.
      use regseq_model::compat::{
          DeclarationModel, JoinedArtifact, BusBinding,
          CallbackFamily, JoinOutcome,
      };
      let _ = std::any::type_name::<DeclarationModel>();
      let _ = std::any::type_name::<JoinedArtifact>();
      let _ = std::any::type_name::<BusBinding>();
      let _ = std::any::type_name::<CallbackFamily>();
      let _ = std::any::type_name::<JoinOutcome>();
  }
  ```

- Grep gate (run locally before committing; treat as acceptance):

  ```
  rg -n -w 'regseq_model::(declaration|binding|joined|semantic|item)' crates/regseq-analysis/src/ && echo FAIL || echo OK
  rg -n 'regseq_model::compat' crates/regseq-analysis/src/
  ```

## Review

After tests pass, invoke Codex per `.harness/SESSION.md`:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-analysis-06 against its spec at
.harness/tasks/regseq-analysis/06-migrate-to-regseq-model-compat.md.

Files to audit:
- crates/regseq-analysis/src/input.rs
- crates/regseq-analysis/src/readiness.rs
- crates/regseq-analysis/src/body_readiness.rs
- crates/regseq-analysis/tests/compat_import_surface.rs

Focus on:
- Grep gate: zero `regseq_model::(declaration|binding|joined|semantic|item)`
  under crates/regseq-analysis/src/.
- AnalysisInput::new signature is unchanged from the pre-task state.
- No behavioral drift: consumer method calls still resolve to
  the same output on the same inputs.
- No edits to emit_driver.rs, compat.rs, or any backend crate.
- No edits to crates/regseq-driver/src/construction*.

Do NOT flag:
- Missing `from_regseq_model` ctor (regseq-driver-35's work).
- Compat view completeness gaps (regseq-model-61's surface was
  approved when it landed; if a gap is found, it blocks this
  task's progress but is NOT this task's fault).
</task>
... structured_output_contract per SESSION.md ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-analysis
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-analysis -- -D warnings
rg -n -w 'regseq_model::(declaration|binding|joined|semantic|item)' crates/regseq-analysis/src/
# The above must print NOTHING (or exit non-zero).
```

## Acceptance Criteria

- `crates/regseq-analysis/src/` contains zero references to
  `regseq_model::{declaration, binding, joined, semantic, item}::*`
  (grep verified).
- At least one `regseq_model::compat` import per migrated file.
- `AnalysisInput::new` signature is byte-identical to its pre-task
  shape.
- `cargo test -p regseq-analysis` passes with no assertion
  rewrites.
- `cargo test --workspace` passes.
- `crates/regseq-analysis/tests/compat_import_surface.rs` exists
  and compiles.
- `git diff --name-only main..HEAD` touches only
  `crates/regseq-analysis/src/{input,readiness,body_readiness}.rs`,
  `crates/regseq-analysis/tests/compat_import_surface.rs`, and
  (possibly) `Cargo.lock`.
- Codex review approved.
