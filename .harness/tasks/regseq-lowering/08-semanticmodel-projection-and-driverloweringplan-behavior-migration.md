# Task: regseq-lowering-08 - SemanticModel projection and DriverLoweringPlan behavior migration

## Description

Project the backend-facing `SemanticModel` from the populated item hierarchy and
migrate `DriverLoweringPlan.behavior` away from the old `BehaviorPlan`.

This task establishes the final backend-consumption boundary for behavioral
semantics before Linux emission is rewritten on top of it.

## Input Context

Read before starting:

- `docs/discussions/semantic-procedural-ir/plan.md`
- `docs/discussions/semantic-procedural-ir/semantic-procedural-ir-implementation-tracker.md`
- `docs/discussions/semantic-procedural-ir/backend-boundary-validation.md`
- `docs/discussions/semantic-procedural-ir/model-layer-coherence-validation.md`
- `docs/discussions/semantic-procedural-ir/paper-migration-validation.md`
- `.harness/tasks/regseq-lowering/07-supported-regseq-semantic-body-lowering-validated-on-pl011-representative-set.md`
- `.harness/tasks/regseq-lowering/01-lowering-module-scaffold-and-typed-plan-boundary.md`
- `crates/regseq-backends/src/lowering/`
- `crates/regseq-backends/tests/`

## Output Specification

Modify:

- `crates/regseq-backends/src/lowering/`
- `crates/regseq-backends/tests/`

Project and thread the new backend-facing behavioral boundary:

- `SemanticModel.functions`
- `SemanticModel.constants`
- `SemanticModel.interrupt_classes`

and migrate portable planning so `DriverLoweringPlan.behavior` uses
`SemanticModel` rather than `BehaviorPlan`.

## Implementation Notes

### Phase Boundary

This task owns the plan-facing migration boundary. It must not yet:

- rewrite Linux emission code
- reintroduce backend-specific semantic recovery
- keep `BehaviorPlan` as the authoritative behavioral payload for the new path

### Projection Requirements

The projection must be explicit and reviewable:

- what comes from the item hierarchy
- what comes from semantic-body lowering
- what is selected into `SemanticModel`
- what is intentionally not projected

The projection must not drop behavioral information the backend needs.

### DriverLoweringPlan Migration

`DriverLoweringPlan.behavior` must move to `SemanticModel`.

If temporary coexistence with legacy `BehaviorPlan` is needed for migration,
that coexistence must be clearly transitional and must not leave the new backend
path consuming the old plan by default.

### Boundary Rules

Backends must receive behavioral semantics through the new semantic model,
not by:

- re-reading raw MIR models
- re-walking declaration-side artifacts for body meaning
- reconstructing helper or register semantics from names

### Non-Goals / Negative Constraints

Do not in this task:

- rewrite Linux emitter implementation
- add Linux-specific semantic nodes
- hide partial migration behind backend-side adapters that keep `BehaviorPlan`
  authoritative

## Test Plan

Add tests to verify:

1. `SemanticModel` can be projected deterministically from the populated
   hierarchy/lowering output
2. projected `SemanticModel` contains the required:
   - functions
   - constants
   - interrupt classes
3. `DriverLoweringPlan.behavior` uses `SemanticModel`
4. the backend-facing plan no longer requires `BehaviorPlan` for the new path
5. projection does not lose required identity or body semantics
6. the new behavioral boundary remains OS-neutral and device-neutral

Test commands:

- `cargo test -p regseq-backends`
- `cargo clippy -p regseq-backends -- -D warnings`

## Acceptance Criteria

- `SemanticModel` is the backend-facing behavioral projection for the new path
- `DriverLoweringPlan.behavior` is migrated away from `BehaviorPlan`
- the projection is deterministic and reviewer-inspectable
- no backend phase needs to reconstruct body semantics outside the plan
- any legacy coexistence is explicitly transitional, not authoritative
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
