# regseq-model-63: Delete Old Model Modules + Phase-7 Tracker Gate

**Depends on**: `regseq-lowering-11`.

## Description

Phase-7F cleanup (part 2). Deletes the legacy model modules:

- `crates/regseq-model/src/declaration/`
- `crates/regseq-model/src/binding/`
- `crates/regseq-model/src/joined/`
- `crates/regseq-model/src/semantic/`
- `crates/regseq-model/src/item/`

Per migration.md decision 3: deletion lands in Phase-7F (absorbing
what was originally execution-plan Phase 9.1). By this point,
task 14 removed every lowering-side consumer, task 12 cut over
`emit_driver`, and task 13 retargeted every E2E test. Nothing
remains that reads these modules.

This is also the **Phase-7 tracker gate**: flip
`regseqmodel-phase-7` to `tasked` with the 15 generated task IDs,
and preserve Phase-8 notes (Phase 8 was already
`ready_to_elaborate` when Phase 7 was re-elaborated; do NOT
re-flip its status).

## Input Context

- `docs/discussions/models/regseq-model/migration.md` decision 3
  (delete in Phase-7F final step).
- `docs/discussions/models/regseq-model/execution-plan.md` Phase
  9.1 (revised: absorbed by Phase 7F; this task executes it).
- `.harness/tasks/regseq-lowering/11-delete-dead-lowering-submodules.md`
  (upstream: lowering-side consumers gone).
- `.harness/tasks/regseq-analysis/06-readiness-rewrite-against-regseq-model.md`
  (upstream: analysis-side consumers gone).
- `.harness/tasks/regseq-driver/35-emit-driver-pipeline-cutover.md`
  (upstream: emit_driver uses only new pipeline).
- `.harness/task-spec-status.yaml` (to be updated).
- `.harness/tasks.json` (to be updated).

## Implementation Notes

### Pre-delete sanity check

Run before any deletion:

```
rg -n 'regseq_model::(declaration|binding|joined|semantic|item)::' crates/ tests/
```

Every hit MUST be in one of:

- The modules being deleted themselves (internal references).
- Tests that the migration plan documented as retired but haven't
  been cleaned up yet (task stops and escalates to originator
  instead of silently patching).
- Docs / comments (acceptable to leave; grep by `rg --type rust`
  if the signal is dominated by docs).

### Deletion steps

```
git rm -r crates/regseq-model/src/declaration/
git rm -r crates/regseq-model/src/binding/
git rm -r crates/regseq-model/src/joined/
git rm -r crates/regseq-model/src/semantic/
git rm -r crates/regseq-model/src/item/
```

Update `crates/regseq-model/src/lib.rs`: remove the five
`pub mod ...;` declarations plus any re-exports from these modules
at crate root. Any remaining `pub use declaration::*;` etc. gets
deleted.

### Post-delete compilation

`cargo check -p regseq-model` must pass immediately. `cargo test
--workspace` must pass. If either fails, a migration step missed
an import site; fix the import site (or escalate to the task that
should have removed it) rather than reintroducing the old module.

### Archive references

Any code comment / documentation inside `crates/regseq-model/`
referring to the deleted modules gets a scrubbing pass. Grep:

```
rg -n 'declaration::|binding::|joined::|semantic::|item::' crates/regseq-model/src/ --type rust
```

Each hit is either fixed or left as a doc-historical reference
with a note pointing at migration.md.

### Tracker flip

Update `.harness/task-spec-status.yaml`:

- `regseqmodel-phase-7.status`: `ready_to_elaborate` /
  `in_elaboration` -> `tasked`.
- `regseqmodel-phase-7.generated_task_ids`: populate with all 15
  task IDs produced by this batch:
  ```yaml
  generated_task_ids:
    - regseq-backends-02
    - regseq-backends-03
    - track-a-validation-11
    - regseq-model-61
    - regseq-model-62
    - regseq-analysis-06
    - regseq-lowering-10
    - regseq-linux-backend-03
    - regseq-linux-backend-04
    - regseq-linux-backend-05
    - track-a-validation-12
    - regseq-driver-35
    - track-a-validation-13
    - regseq-lowering-11
    - regseq-model-63
  ```
- `regseqmodel-phase-8.status`: **do NOT change**. It was already
  `ready_to_elaborate` when Phase 7 was re-elaborated. Codex
  round-3 review MANDATORY fix 7: "flip phase-8" is unreachable
  -- just preserve. Update the `spec_authoring_notes` under
  `regseqmodel-phase-8` to acknowledge that Phase-8.1 was
  absorbed by Phase-7C and Phase-8.3 was absorbed by Phase-7F.

### Tracker: `.harness/tasks.json`

Confirm every one of the 15 Phase-7 task entries is present in
tasks.json with correct dep edges. The final state is recorded at
task close via `task.py done regseq-model-63` (which checks all
upstream deps are `done`).

### What this task does NOT do

- Does not delete anything outside the old model modules.
- Does not refresh goldens (Phase-9.2 concern).
- Does not touch Phase-8 spec-batch state beyond preserving
  `ready_to_elaborate`.
- **`crates/regseq-driver/src/construction*`** strict non-goal.

## Non-Goals

- No Phase-8 work.
- No Phase-9 golden refresh.
- No new code; this task is deletion + tracker.

## Test Plan

- Pre-delete sanity grep passes.
- Post-delete `cargo build -p regseq-model` passes.
- `cargo test --workspace` green (byte-equal regression, host-build,
  CFG-invariants, all surviving E2E tests).
- `task.py next-step` after task close returns
  `NEXT_ACTION: TASK_SPEC_BATCH` for `regseqmodel-phase-8`.
- YAML tracker shows phase-7 `tasked` with 15 `generated_task_ids`;
  phase-8 preserved at `ready_to_elaborate`.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-model-63 against .harness/tasks/regseq-model/63-delete-old-model-modules-and-tracker-gate.md.

Files:
- git diff of deleted declaration/ binding/ joined/ semantic/ item/
- crates/regseq-model/src/lib.rs (module decls + re-exports removed)
- .harness/task-spec-status.yaml (phase-7 tasked, phase-8
  preserved)
- .harness/tasks.json (confirm final state)

Focus on:
- Zero references to the five module paths anywhere in crates/
  outside comments / docs.
- generated_task_ids list is complete (all 15 IDs).
- phase-8 status is preserved, not flipped (Codex round-3 MANDATORY).
- phase-8 spec_authoring_notes updated to reflect 7C/7F absorption.
- `cargo test --workspace` green.
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
rg -n 'regseq_model::(declaration|binding|joined|semantic|item)::' crates/ tests/
# must print nothing (or only docs-historical references)
uv run python scripts/python/task.py next-step
# must return TASK_SPEC_BATCH for regseqmodel-phase-8
```

## Acceptance Criteria

- Five old model modules deleted (`git rm -r`).
- `crates/regseq-model/src/lib.rs` purged of declarations +
  re-exports of deleted modules.
- Grep gate passes.
- `.harness/task-spec-status.yaml` shows `regseqmodel-phase-7` as
  `tasked` with 15 `generated_task_ids` matching the
  Implementation Notes list.
- `regseqmodel-phase-8.status` preserved (not flipped) + its
  `spec_authoring_notes` updated to note 7C/7F absorption.
- `.harness/tasks.json` has all 15 entries in `done` state
  (handled by `task.py done`).
- `cargo test --workspace` green.
- Codex review APPROVED.
- After this task: Phase 7 operationally complete; Phase 8 work
  can begin (starting at 8.2, since 8.1 absorbed).
