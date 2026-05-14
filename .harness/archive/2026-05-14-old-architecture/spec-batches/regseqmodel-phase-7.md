# Spec Batch: regseqmodel-phase-7 - Consumer migration to RegseqModel

## Description

This batch authors executable harness task specs for migrating every
consumer from the old model types (`DeclarationModel`, `BindingModel`,
`JoinedArtifact`, `SemanticModel`, `Item` hierarchy) to the unified
`RegseqModel`. A temporary `compat.rs` adapter layer bridges old
consumer APIs to the new model so consumers migrate one at a time
without breaking the build.

Migration order (each is a separate task that lands independently):

1. Adapter scaffold (`compat.rs`).
2. `regseq-analysis`.
3. `regseq-backends/lowering/planner.rs` (lowering planner -- model
   inputs only).
4. `regseq-backends/linux/planner.rs` (Linux backend planner -- model
   inputs only).
5. `regseq-driver/emit_driver.rs` (pipeline glue -- swap to
   `normalize()` + `populate_bindings()` from Phase 6).
6. E2E test rewrites under `tests/e2e/tests/`.

This phase is **API and model-input migration only**. It does NOT
rewrite Linux backend emission logic (Phase 8), and it does NOT
delete any old modules (Phase 9). The Linux backend's emission code
(`emitter.rs`, `emit_*.rs`, `c_ast.rs`) keeps producing the same C
output -- only its input model type changes via the adapter, and
later, directly.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-status.yaml`
- `.harness/spec-batches/BRIEF_TEMPLATE.md`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 7
  and the consumer-migration cascade risk row)
- `docs/discussions/models/regseq-model/spec.md` (the contract every
  consumer must read against)
- `crates/regseq-model/src/model/regseq_model.rs` (the destination
  model)
- `crates/regseq-model/src/declaration/`, `binding/`, `joined/`,
  `semantic/`, `item/` (the old modules being bridged -- to identify
  which APIs each consumer actually uses)
- `crates/regseq-analysis/src/input.rs`
- `crates/regseq-backends/src/lowering/planner.rs`
- `crates/regseq-backends/src/linux/planner.rs`
- `crates/regseq-backends/src/linux/emitter.rs`
- `crates/regseq-backends/src/linux/emit_behavior.rs`
- `crates/regseq-backends/src/linux/emit_module.rs`
- `crates/regseq-backends/src/linux/emit_regs.rs`
- `crates/regseq-backends/src/linux/emit_semantic.rs`
- `crates/regseq-backends/src/linux/c_ast.rs`
- `crates/regseq-backends/src/linux/plan.rs`
- `crates/regseq-driver/src/emit_driver.rs`
- `tests/e2e/tests/` (all 9 files -- each is a migration unit)

## Output Specification

Create:

- `.harness/tasks/regseq-model/` (compat scaffold)
- `.harness/tasks/regseq-analysis/`
- `.harness/tasks/regseq-lowering/`
- `.harness/tasks/regseq-linux-backend/`
- `.harness/tasks/regseq-driver/`
- `.harness/tasks/track-a-validation/` (E2E rewrites)

One task per consumer in the migration order above.

Update when done:

- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`

## Implementation Notes

- **Adapter scope.** `crates/regseq-model/src/compat.rs` exposes the
  old consumer-facing APIs (`DeclarationModel`, `BindingModel`,
  `JoinedArtifact`, `SemanticModel`, `Item` projections) as
  read-only views over a `RegseqModel`. Adapter code is temporary
  and lives only in this file -- no consumer reaches into the
  unified model through compat shortcuts. Phase 9 deletes
  `compat.rs`.
- **Old modules stay alive.** `declaration/`, `binding/`, `joined/`,
  `semantic/`, `item/` remain in the workspace until Phase 9 -- the
  adapter must NOT delete or rename anything. The pipeline keeps
  producing them in parallel during migration; consumers stop
  reading them as they migrate.
- **Per-consumer cutover discipline:**
  - Each migration task changes exactly one consumer.
  - The adapter is the only allowed bridge to the new model. After
    a consumer's task completes, the consumer compiles against
    `RegseqModel` (directly or via `compat`) and never imports old
    model types.
  - The full `cargo test --workspace` suite passes after every
    individual migration task -- never "fix in the next PR".
  - Pipeline glue (`emit_driver.rs`) is the integration choke
    point; after task 5 lands, the runtime pipeline is
    `normalize() -> populate_bindings() -> verify -> consumers`.
- **Linux backend boundary.** Backend code does NOT change emission
  semantics in Phase 7. The Linux task migrates only inputs:
  `planner.rs` consumes `RegseqModel` (via compat) and produces
  the same `Plan` shape it does today; `emitter.rs` and the
  `emit_*.rs` files are untouched in Phase 7. Phase 8 owns the
  emission rewrite.
- **E2E migration.** Each E2E test file in `tests/e2e/tests/` is a
  separate migration unit; rewrite assertions to read
  `RegseqModel` (directly, not via compat) once the upstream
  consumer is migrated. Keep golden inputs unchanged in Phase 7;
  golden refresh is Phase 9.
- **Phase boundary.** Phase 7 ends with every consumer compiling
  against `RegseqModel` (via compat or directly), the full test
  suite green, and zero old-model imports outside `compat.rs`,
  the old module directories themselves, and any tests that
  explicitly assert against the legacy artifacts during
  migration. **Do not** modify Linux emission semantics (Phase
  8) and **do not** delete old modules (Phase 9).

## Non-Goals

- No old module deletion, no `compat.rs` deletion -- Phase 9.
- No new C emission logic, no goto-based emitter, no
  dominator-based cleanup -- Phase 8.
- No Zephyr backend.
- No new driver fixtures.

## Acceptance Criteria

- Six migration tasks land in dependency order (compat scaffold
  first, then analysis, lowering planner, Linux planner,
  pipeline glue, E2E rewrites).
- After every individual task: `cargo test --workspace` passes,
  including all E2E tests, with no expected failures or skips
  introduced by the migration.
- After the analysis task: `crates/regseq-analysis/` contains no
  `use crate::declaration::`, `use crate::binding::`,
  `use crate::joined::`, `use crate::semantic::`, or
  `use crate::item::` imports (verified by grep).
- After the lowering planner task: same grep assertion for
  `crates/regseq-backends/src/lowering/`.
- After the Linux planner task: same grep assertion for
  `crates/regseq-backends/src/linux/planner.rs`. Other files in
  `linux/` may still touch old types -- they migrate in Phase 8.
- After the pipeline glue task: `emit_driver.rs` calls
  `normalize()` and `populate_bindings()` from Phase 6 directly
  and contains no old-pipeline calls.
- After the E2E task: every test in `tests/e2e/tests/` reads
  `RegseqModel` (or compat views) and asserts against new-model
  invariants. Old `JoinedArtifact`/`SemanticModel` assertions
  are removed from migrated tests.
- Adapter boundary verified: no module outside `compat.rs`
  reaches into both old and new model types in the same
  function.
- Tracker updated; `regseqmodel-phase-8` advances to
  `ready_to_elaborate`.

## Update Instructions

1. Change `regseqmodel-phase-7` status to `tasked`.
2. Fill `generated_task_ids`.
3. Move `regseqmodel-phase-8` to `ready_to_elaborate`.
