# regseq-lowering-10: New `build_driver_lowering_plan_from_model` + Trim `DriverLoweringPlan`


**Depends on**: `regseq-model-61` (query helpers).
**Does NOT depend on**: `regseq-analysis-06` -- the new lowering
entry point consumes `&RegseqModel` + `query::*`, not
`AnalysisInput`. Task 6 and this task run in parallel.

## Description

Phase-7B lowering. Adds `build_driver_lowering_plan_from_model(
&RegseqModel, &LoweringConfig) -> Result<DriverLoweringPlan,
LoweringError>` alongside the existing legacy entry. Trims
`DriverLoweringPlan` of its three vestigial fields (`behavior:
SemanticModel`, `legacy_behavior: BehaviorPlan`,
`callback_actions: Vec<CallbackBodyPlan>`). Relocates
`LinuxResourceSequencing` / `LinuxDeploymentMetadata` (verified
reporting-only per Codex round-3 findings): either folds them into
`DriverLoweringPlan` as portable obligations or moves them to a
diagnostics module.

The existing legacy entry (`build_driver_lowering_plan`) **stays
alive** so the old-pipeline call chain continues to compile until
task 12 (`emit_driver` cutover). Both entries coexist during
Phases 7C/7D; task 14 deletes the legacy entry.

Per migration.md Q7: the portable-lowering contract is the right
abstraction level (MLIR-style dialect conversion between portable
and target dialects), but the current representation is impure --
the three deleted fields duplicate behavioral authority that
`RegseqModel`'s CFG IR already carries.

This task also explicitly **supersedes
`docs/discussions/linux-codegen/lowering-plan-interface-boundary.md`**
where that older design note conflicts with migration.md. See
Codex review round 1 MANDATORY fix.

## Input Context

- `docs/discussions/models/regseq-model/migration.md` Q7 (the
  trim recommendation + the relocation guidance for
  `LinuxResourceSequencing` / `LinuxDeploymentMetadata`).
- `docs/discussions/linux-codegen/lowering-plan-interface-boundary.md`
  (the pre-existing design note; superseded by migration.md where
  they conflict -- specifically where the old note still assumes
  `AnalysisInput` is the stable boundary and that
  `callback_actions` belongs in the portable plan).
- `.harness/tasks/regseq-model/61-query-module.md` -- the query
  helpers this task's new entry point consumes.
- `crates/regseq-backends/src/lowering/plan.rs` lines 32-582 --
  the `DriverLoweringPlan` struct being trimmed.
- `crates/regseq-backends/src/lowering/planner.rs` lines 79-175
  -- the existing legacy entry (kept alive).
- `crates/regseq-backends/tests/{lowering_boundary,pl011_full_spec_lowering,pl011_lowering_completeness_proof}.rs`
  -- test files that currently assert on the fields being
  trimmed / relocated. This task OWNS their rewrite per Codex
  round-2 MANDATORY fix.
- `crates/regseq-backends/src/linux/plan.rs` lines 33, 41, 205,
  378 -- `LinuxResourceSequencing` / `LinuxDeploymentMetadata`
  current location.
- `crates/regseq-backends/src/linux/planner.rs` lines 44-48,
  232-273, 437-472 -- their current population.
- `crates/regseq-backends/tests/pl011_full_spec_lowering.rs`
  lines 2218-2326 + `pl011_lowering_completeness_proof.rs` lines
  2172-2186 + `lowering_boundary.rs` lines 662-667 -- their
  assertions (reporting-only confirmed).

## Implementation Notes

### New entry point

```rust
//! In crates/regseq-backends/src/lowering/planner.rs (or a new
//! lowering/from_model.rs module).

use regseq_model::RegseqModel;
use regseq_model::query;

pub struct LoweringConfig { /* backend-neutral config knobs */ }

#[derive(Debug, thiserror::Error)]
pub enum LoweringError {
    // Derived from structural invariants that verify_phase_1/2
    // should already have caught -- if an error slips through it
    // is a regressed invariant, not a driver-author bug.
    #[error("lowering invariant violated: {0}")]
    InvariantViolated(String),
}

/// Build a portable lowering plan directly from a verified
/// `RegseqModel`. Does NOT go through `AnalysisInput` or the
/// legacy declaration / joined / semantic projections.
///
/// This is the `RegseqModel`-era entry; the legacy
/// `build_driver_lowering_plan(&AnalysisInput, ...)` remains
/// alongside until `regseq-driver-35` cuts `emit_driver` over.
pub fn build_driver_lowering_plan_from_model(
    model: &RegseqModel,
    config: &LoweringConfig,
) -> Result<DriverLoweringPlan, LoweringError> {
    Ok(DriverLoweringPlan {
        identity: driver_identity_from_model(model),
        callbacks: callback_inventory_from_model(model),
        resources: resource_obligations_from_model(model),
        acquisition_order: acquisition_order_from_model(model),
        teardown_order: teardown_order_from_model(model),
        device_class_roles: device_class_roles_from_model(model),
        register_obligations: register_obligations_from_model(model),
        synchronization: synchronization_from_model(model),
        // behavior / legacy_behavior / callback_actions are GONE.
        // A new `function_bodies` handle gives backends access to
        // CFG bodies by FunctionId (see DriverLoweringPlan trim below).
    })
}
```

Internal helpers derive each portable obligation by walking
`model.root_module` + calling `regseq_model::query::*` helpers
from task 4. No string matching; no CanonicalPath lookups; typed
`DeclId`s throughout.

### `DriverLoweringPlan` trim

Delete fields:

- `behavior: SemanticModel` -- replaced by a FunctionId -> &Body
  index (see below).
- `legacy_behavior: BehaviorPlan` -- replaced by same.
- `callback_actions: Vec<CallbackBodyPlan>` -- redundant with CFG
  body info.

`DriverLoweringPlan` does NOT gain a `function_bodies` field.
Body access is routed through a separate `FunctionBodyIndex<'_>`
type defined by task `regseq-linux-backend-03` (task 8) and passed
alongside the plan to the emitter. `DriverLoweringPlan` remains
non-lifetime-parameterized; it holds `FunctionId`s where it needs
to refer to bodies. This is the single canonical body-access
contract across tasks 7/8/9/10; the "store bodies on the plan"
alternative is explicitly rejected.

Delete:

- `use regseq_model::semantic::SemanticModel;` from the top of
  `plan.rs`.
- `use super::behavior::BehaviorPlan;` from the top of `plan.rs`.
- Every struct / enum transitively needed only by the three
  removed fields (e.g., `CallbackBodyPlan`, `LoweringAction`,
  unless they're reused by downstream consumers -- check via
  grep).

### `LinuxResourceSequencing` / `LinuxDeploymentMetadata` relocation

Codex round-3 verified these are reporting-only (populated +
test-asserted, not emitter-consumed). Two options:

1. **Fold into `DriverLoweringPlan`** as portable obligations
   (since they're backend-neutral data derived from model +
   platform).
2. **Move to a new `DriverLoweringDiagnostics`** type in a
   `diagnostics` module; have `build_driver_lowering_plan_from_model`
   also produce a diagnostics report for reviewers.

Pick option 1 (folding in) unless it creates naming collisions
with existing `DriverLoweringPlan` fields. Document the choice
inline.

### Test rewrites (explicit ownership per Codex round-2)

This task owns the test-assertion updates for the three test
files flagged:

- `crates/regseq-backends/tests/lowering_boundary.rs`:
  - Lines 500-519, 572-590 assert on `callback_actions` and
    `LoweringAction::*`. Rewrite to assert on the equivalent
    CFG-body / query-helper output, OR delete the assertions if
    the query helpers already cover them.
  - Lines 658-675 assert on `resource_acquisition.acquisition_order`
    / `release_order`. If the relocation folds these into
    `DriverLoweringPlan`, update to new field path.
- `crates/regseq-backends/tests/pl011_full_spec_lowering.rs`:
  - Lines 2218-2326 assert on `deployment_metadata` clock /
    reset / pinctrl fields. Update to new field path.
- `crates/regseq-backends/tests/pl011_lowering_completeness_proof.rs`:
  - Lines 2164-2188 assert on `deployment_metadata` lengths /
    content. Update.

Every trimmed field that had a test also gets its test updated in
the same commit as the trim.

### Grep gate

After this task:

```
rg -n 'behavior:|legacy_behavior|callback_actions|SemanticModel|BehaviorPlan' \
    crates/regseq-backends/src/lowering/plan.rs
# must return zero matches (all removed)
```

### What this task does NOT touch

- `linux/planner.rs` (task 8 fixes the leaky boundary).
- `linux/emitter.rs`, `emit_module.rs`, `emit_semantic.rs` --
  still string-format bodies; their rewrite is tasks 9/10.
- Legacy `build_driver_lowering_plan` entry stays alive.
- `lowering/{behavior_planner, body_interpreter, mir_interpreter,
  hierarchy, projection, semantic_lowering}` -- deleted in task 14.
- **`crates/regseq-driver/src/construction*`**.

## Non-Goals

- No deletion of legacy lowering entry (task 14).
- No consumer-side migration (task 12 cuts emit_driver over).
- No Linux-plan self-sufficiency fix (task 8).
- No Zephyr work.

## Test Plan

- **New entry unit tests**: under
  `crates/regseq-backends/tests/lowering_from_model.rs`, test
  `build_driver_lowering_plan_from_model` against hand-built
  minimal models. One test per output-field family.
- **PL011 integration test**: load Phase-6 PL011 golden, call new
  entry, assert output shape matches what the legacy entry
  produces on the equivalent input (where they overlap).
  Divergences are acceptable for fields like `callback_actions`
  (deleted) but must be documented.
- **Trim test rewrites** land in the same commit (see
  Implementation Notes).
- `cargo test -p regseq-backends` green.
- `cargo test --workspace` green.

## Review

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540 --message-only
<task>
Review regseq-lowering-10 against .harness/tasks/regseq-lowering/10-new-lowering-entry-from-regseq-model.md.

Files to audit:
- crates/regseq-backends/src/lowering/planner.rs (new entry)
- crates/regseq-backends/src/lowering/plan.rs (fields trimmed)
- crates/regseq-backends/tests/{lowering_boundary,pl011_full_spec_lowering,pl011_lowering_completeness_proof}.rs
- crates/regseq-backends/tests/lowering_from_model.rs (new)

Focus on:
- DriverLoweringPlan no longer references SemanticModel or BehaviorPlan.
- `behavior`, `legacy_behavior`, `callback_actions` fields are GONE.
- New entry point consumes only &RegseqModel + &LoweringConfig --
  no AnalysisInput in the signature.
- The new LinuxResourceSequencing / LinuxDeploymentMetadata location
  is documented (folded into DriverLoweringPlan, or moved to
  diagnostics).
- Test rewrites land in the same commit; no leftover assertions
  on removed fields.
- Legacy `build_driver_lowering_plan` still exists; deletion is
  task 14.

Do NOT flag:
- Leaky LinuxBackendPlan boundary (task 8).
- Emitter rewrites (tasks 9/10).
</task>
... (standard grounding + structured output) ...
EOF
```

Iterate until APPROVED.

## Quality Gate

```
cargo test -p regseq-backends --test lowering_from_model
cargo test -p regseq-backends
cargo test --workspace
cargo fmt --all -- --check
cargo clippy -p regseq-backends -- -D warnings
```

## Acceptance Criteria

- `build_driver_lowering_plan_from_model(&RegseqModel,
  &LoweringConfig) -> Result<DriverLoweringPlan, LoweringError>`
  implemented.
- `DriverLoweringPlan` no longer has `behavior`, `legacy_behavior`,
  or `callback_actions` fields.
- `LinuxResourceSequencing` / `LinuxDeploymentMetadata` relocated
  per one of the two options in Implementation Notes; choice
  documented.
- Three flagged test files rewritten in the same commit.
- Grep gate passes (no `behavior:|legacy_behavior|callback_actions|
  SemanticModel|BehaviorPlan` in `lowering/plan.rs`).
- Legacy entry still exists and compiles (deleted in task 14).
- `cargo test --workspace` green.
- Codex review APPROVED.
