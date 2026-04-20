# regseq-lowering-11: Delete Dead Lowering Submodules + Legacy Lowering Entry

**Depends on**: `track-a-validation-13`.

## Description

Phase-7F cleanup (part 1). Deletes every lowering module that is
unreachable after the pipeline cutover (task 12) and E2E retarget
(task 13):

- `crates/regseq-backends/src/lowering/behavior_planner.rs`
- `crates/regseq-backends/src/lowering/body_interpreter.rs`
- `crates/regseq-backends/src/lowering/mir_interpreter/` (entire
  subtree)
- `crates/regseq-backends/src/lowering/hierarchy.rs`
- `crates/regseq-backends/src/lowering/projection.rs`
- `crates/regseq-backends/src/lowering/semantic_lowering/` (entire
  subtree)
- Legacy `build_driver_lowering_plan` + `build_driver_lowering_plan_with_constants`
  entries in `lowering/planner.rs`.
- Any transitional adapter / shim introduced by tasks 9/10 (task
  10 retires these; this task is the belt-and-suspenders check).
- Any `BehaviorPlan` / `LoweringAction` / `CallbackBodyPlan` type
  still referenced only by the above (trim the graph).

Per migration.md decision 3: deletion comes AFTER validation
(task 13). By this point, nothing in production or tests reaches
the deleted code.

## Input Context

- `docs/discussions/models/regseq-model/migration.md` Q3 +
  decision 3 (old modules stay through Test A; deleted in Phase-7F).
- `docs/discussions/models/regseq-model/execution-plan.md` Phase
  8.3 (this is the revised version: deprecated-code removal that
  was originally scheduled there migrates here).
- `.harness/tasks/track-a-validation/13-e2e-rewrites-and-cfg-invariants.md`
  (upstream: all tests pass against new pipeline).
- `crates/regseq-backends/src/lowering/mod.rs` (module declarations
  being removed).
- `crates/regseq-backends/src/lowering/planner.rs` (legacy entry
  fn removed).
- Every file slated for deletion (read to confirm no unique
  concepts survive that other modules need to adopt).

## Implementation Notes

### Pre-delete sanity check

Before deleting anything, run:

```
rg -n 'behavior_planner|body_interpreter|mir_interpreter|hierarchy|projection|semantic_lowering' crates/
rg -n 'BehaviorPlan|LoweringAction|CallbackBodyPlan|build_driver_lowering_plan[^_]' crates/
```

Every hit must be:

- Inside the deletion scope, OR
- A comment / doc reference acceptable to leave, OR
- A test that was supposed to be migrated in tasks 6 / 10 / 13
  but wasn't.

If the third case surfaces, this task STOPS and escalates to the
originating task (re-opens it or files a fix task). Do not
silently patch missed migrations here.

### Deletion steps

```
git rm crates/regseq-backends/src/lowering/behavior_planner.rs
git rm crates/regseq-backends/src/lowering/body_interpreter.rs
git rm -r crates/regseq-backends/src/lowering/mir_interpreter/
git rm crates/regseq-backends/src/lowering/hierarchy.rs
git rm crates/regseq-backends/src/lowering/projection.rs
git rm -r crates/regseq-backends/src/lowering/semantic_lowering/
```

Update `crates/regseq-backends/src/lowering/mod.rs`: remove
`pub mod ...;` declarations for each deleted module.

Update `crates/regseq-backends/src/lowering/planner.rs`: delete
`build_driver_lowering_plan` (legacy signature taking
`&AnalysisInput`) and any helper functions used only by it
(`validate_interrupt_class_masks`, `build_behavior_plan_with_constants`,
etc., scoped by grep).

Remove any re-export of the legacy entry from the crate's
top-level.

### Trim BehaviorPlan / LoweringAction / CallbackBodyPlan

If these types (or their submodules) are now unreferenced after
deletion, remove them too. Grep confirms orphaned status:

```
rg -n 'BehaviorPlan|LoweringAction|CallbackBodyPlan' crates/ tests/
# zero matches outside the modules being deleted
```

Their files (if separate from `behavior_planner.rs`) get `git rm`'d.

### Compile + test after each deletion

Cargo-check (`cargo check --workspace`) after every `git rm` so the
set of broken symbols stays small. If a deletion breaks something,
the task stops.

### `lowering-plan-interface-boundary.md` archive

The design doc `docs/discussions/linux-codegen/lowering-plan-interface-boundary.md`
has been superseded by migration.md. Move it to
`docs/archive/linux-codegen/` with a breadcrumb comment at the
top pointing at migration.md. Do NOT delete outright -- it's
historical context.

### What this task does NOT do

- Does not delete old model modules (`declaration/`, `binding/`,
  etc.) -- that is task 15.
- Does not modify `emit_semantic.rs` / `emit_module.rs` (they're
  already retargeted).
- Does not touch `crates/regseq-analysis/` (task 6 handled).
- **`crates/regseq-driver/src/construction*`** strict non-goal.

## Non-Goals

- No Phase-8 dominator work.
- No old-model-module deletion.
- No golden-file regeneration.
- No consumer migration (should be done by now).

## Test Plan

- **Grep gate**: after deletion, zero references to the six
  submodule names anywhere in `crates/` or `tests/`.
- `cargo build --workspace` passes.
- `cargo test --workspace` green (all tests that used to go
  through legacy paths should already be migrated or deleted by
  this point).
- Byte-equal regression from task 12 still passes (no emitter
  drift).
- Host-build proof still passes.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-lowering-11 against .harness/tasks/regseq-lowering/11-delete-dead-lowering-submodules.md.

Files:
- git diff of deleted submodules (behavior_planner, body_interpreter,
  mir_interpreter/, hierarchy, projection, semantic_lowering/)
- crates/regseq-backends/src/lowering/mod.rs (module decls removed)
- crates/regseq-backends/src/lowering/planner.rs (legacy entry removed)
- docs/archive/linux-codegen/lowering-plan-interface-boundary.md
  (relocated with breadcrumb)

Focus on:
- Nothing outside the deleted scope still references the deleted
  modules or the legacy entry function.
- Any type (BehaviorPlan / LoweringAction / CallbackBodyPlan) that
  is now orphaned is also deleted.
- `cargo test --workspace` passes.
- No production-path regression (byte-equal + host-build still pass).

Do NOT flag:
- Old model modules still existing (task 15).
</task>
... (standard grounding + structured output) ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
rg -n 'behavior_planner|body_interpreter|mir_interpreter|hierarchy|projection|semantic_lowering' crates/ tests/
# must print nothing (or only the docs/archive/ breadcrumb)
rg -n 'BehaviorPlan|LoweringAction|CallbackBodyPlan' crates/
# zero matches
```

## Acceptance Criteria

- Six submodules deleted (file / directory-level `git rm`).
- Legacy lowering entry function deleted.
- Orphaned types deleted.
- `lowering-plan-interface-boundary.md` archived with breadcrumb.
- Grep gates pass.
- `cargo test --workspace` green.
- Codex review APPROVED.
