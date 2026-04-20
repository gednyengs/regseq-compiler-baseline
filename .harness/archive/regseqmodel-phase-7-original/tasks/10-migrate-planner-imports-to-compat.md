# regseq-lowering-10: Migrate `lowering/planner.rs` Imports to `regseq_model::compat`

## Description

Phase-7 consumer migration task 2 of 4. Rewrites every
`regseq_model::{declaration, item, semantic}` import in
`crates/regseq-backends/src/lowering/planner.rs` to
`regseq_model::compat`. No behavior changes; workspace stays green.

**Scope is exactly one file**: `lowering/planner.rs`. Sibling files
in `crates/regseq-backends/src/lowering/` (`plan.rs`,
`behavior_planner.rs`, `hierarchy.rs`, `projection.rs`,
`mir_interpreter/`, `semantic_lowering/`) still import old-model
types and are explicitly deferred to Phase 8 (per Codex review of
the Phase-7 decomposition). This avoids mis-scoping the grep
gate to the entire `lowering/` directory.

## Input Context

- `.harness/spec-batches/regseqmodel-phase-7.md` lines 12-15
  ("lowering planner -- model inputs only"); lines 143-144
  (lowering grep gate; this task corrects the scope to the file).
- `docs/discussions/models/regseq-model/execution-plan.md` Phase 7
  and Phase 8 sections (Phase 8 owns the emission rewrite and
  semantic/mir interpreter removal).
- `.harness/tasks/regseq-analysis/06-migrate-to-regseq-model-compat.md`
  -- upstream task; this task's template mirrors its shape.
- `.harness/tasks/regseq-model/61-compat-scaffold.md` -- the
  re-exports and view surface this task consumes.
- `crates/regseq-backends/src/lowering/planner.rs` -- the
  migration target. Lines 11-28 carry the import block. Lines
  195-205, 236-249, 259-264, 678-728 are representative usage
  sites that continue to work unchanged once the imports are
  rerouted.

## Implementation Notes

### Import-path-only migration

**Top-of-file import block** (lines 11-28 in current `planner.rs`):

- `use regseq_model::declaration::CallbackFamily;` → `use
  regseq_model::compat::CallbackFamily;`.
- `use regseq_model::item::identity::ItemId;` → `use
  regseq_model::compat::ItemId;`.
- `use regseq_model::semantic::SemanticModel;` → `use
  regseq_model::compat::SemanticModel;`.
- Any other `regseq_model::{declaration, item, semantic}::*` import
  in the top block gets the same treatment.

**In-body fully-qualified paths** (NOT just the import block --
per Codex review of the decomposition):

`planner.rs` contains fully-qualified references in function
bodies and match patterns. Rewrite every occurrence:

- Around `crates/regseq-backends/src/lowering/planner.rs:239` and
  `crates/regseq-backends/src/lowering/planner.rs:437-439`, and
  any other in-body site where
  `regseq_model::{declaration, item, semantic}::...` appears,
  swap the path prefix to `regseq_model::compat::...`.
- After editing, run `rg -n 'regseq_model::(declaration|binding|joined|semantic|item)' crates/regseq-backends/src/lowering/planner.rs`
  and confirm zero matches. Any residual match is a missed
  in-body path -- rewrite it.

`AnalysisInput` references stay the same (imported from
`regseq_analysis::AnalysisInput`); the crate-level import path in
`regseq-analysis` was already migrated by task
`regseq-analysis-06`, so `.declaration()`, `.joined()`, and other
`AnalysisInput` accessors return the same types they did before
(the compat re-exports ARE the legacy types).

### Sibling files explicitly deferred

Do **not** touch these files -- they migrate in Phase 8:

- `crates/regseq-backends/src/lowering/plan.rs`
- `crates/regseq-backends/src/lowering/behavior_planner.rs`
- `crates/regseq-backends/src/lowering/hierarchy.rs`
- `crates/regseq-backends/src/lowering/projection.rs`
- `crates/regseq-backends/src/lowering/mir_interpreter/` (entire
  subtree; deleted in Phase 8)
- `crates/regseq-backends/src/lowering/semantic_lowering/` (entire
  subtree; deleted in Phase 8)

If `planner.rs` imports a type that turns out to live **only** on
one of the deferred files, stop and flag it. The compat scaffold
(task `regseq-model-61`) should have captured every type this
task needs; a gap there is a blocker for this task.

### Grep acceptance (scoped to one file)

```
rg -n -w 'regseq_model::(declaration|binding|joined|semantic|item)' crates/regseq-backends/src/lowering/planner.rs
```

must return zero matches.

The broader `crates/regseq-backends/src/lowering/` directory will
still show matches from the deferred files above; that is expected
and OK.

### What this task does NOT touch

- No changes outside `planner.rs`.
- No signature changes (`build_driver_lowering_plan` keeps its
  existing shape).
- No changes to `AnalysisInput` or any upstream crate.
- **Do not modify `crates/regseq-driver/src/construction*`.**

## Non-Goals

- No Phase-8 work (emission rewrite, mir_interpreter /
  semantic_lowering removal).
- No changes to sibling lowering files.
- No Zephyr backend work (still a stub).
- No fixture / golden file changes.
- No lowering plan struct-shape changes.

## Test Plan

- `cargo test -p regseq-backends` passes unchanged.
- `cargo test --workspace` passes.
- Grep gate:

  ```
  rg -n -w 'regseq_model::(declaration|binding|joined|semantic|item)' crates/regseq-backends/src/lowering/planner.rs && echo FAIL || echo OK
  rg -n 'regseq_model::compat' crates/regseq-backends/src/lowering/planner.rs
  ```

  First must print nothing; second must print at least one hit.

- No new unit tests are required for this task.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-lowering-10 against
.harness/tasks/regseq-lowering/10-migrate-planner-imports-to-compat.md.

Files to audit:
- crates/regseq-backends/src/lowering/planner.rs

Focus on:
- Grep gate: zero `regseq_model::(declaration|binding|joined|semantic|item)`
  matches in crates/regseq-backends/src/lowering/planner.rs.
- No edits to sibling lowering files.
- No signature changes to `build_driver_lowering_plan`.
- No edits to crates/regseq-driver/src/construction*.

Do NOT flag:
- Sibling lowering files still importing old types (Phase 8).
- Missing `from_regseq_model` ctor on compat views (Phase-7 task
  regseq-driver-35).
</task>
... structured_output_contract per SESSION.md ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-backends
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-backends -- -D warnings
```

## Acceptance Criteria

- `crates/regseq-backends/src/lowering/planner.rs` contains zero
  `regseq_model::{declaration, binding, joined, semantic, item}`
  imports.
- The file imports at least one `regseq_model::compat::*` symbol.
- `build_driver_lowering_plan` signature is byte-identical to its
  pre-task shape.
- `cargo test --workspace` passes.
- `git diff --name-only main..HEAD` touches only
  `crates/regseq-backends/src/lowering/planner.rs` and (possibly)
  `Cargo.lock`.
- Codex review approved.
