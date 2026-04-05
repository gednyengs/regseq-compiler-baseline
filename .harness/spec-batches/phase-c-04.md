# Spec Batch: phase-c-04 - explicit bus identity and Layer 2 bus metadata prerequisite

## Description

Author the prerequisite task batch needed to make bus identity a real compiler
fact before Phase D lowering continues.

This batch exists because the accepted architecture now requires:

- explicit authored Layer 1 bus identity
- typed Layer 2 bus metadata
- analysis/bootstrap visibility for both

The current pipeline does not yet carry those facts end-to-end. Continuing
Phase D lowering with heuristic bus inference would violate the accepted bus
resource management design and weaken the PL011 `bus = "amba"` story.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`
- `docs/plans/asplos-implementation-plan.md`
- `docs/compiler-collateral/constructs/bus-resource-management.md`
- `docs/discussions/pl011-full-spec/pl011-unified-spec-boundary.md`
- `docs/discussions/linux-codegen/lowering-plan-interface-boundary.md`
- `crates/regseq-driver/src/extract/driver_identity.rs`
- `crates/regseq-driver/src/raw_artifact.rs`
- `crates/regseq-model/src/declaration/driver.rs`
- `crates/regseq-model/src/declaration/raw_input.rs`
- `crates/regseq-model/src/declaration/normalize.rs`
- `crates/regseq-model/src/platform.rs`
- `crates/regseq-model/src/platform_parser.rs`
- `crates/regseq-model/src/binding/mod.rs`
- `crates/regseq-analysis/src/input.rs`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-driver/`
- `.harness/tasks/regseq-model/`
- `.harness/tasks/regseq-analysis/`
- `.harness/tasks/track-a-validation/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs plus the
required harness metadata updates.

## Implementation Notes

Follow these fixed decisions:

- `bus_type` is an explicit authored Layer 1 property, not a lowering-time
  inference.
- PL011 should be modeled as `bus = "amba"` while retaining
  `compatible = ["arm,pl011"]`.
- Layer 2 bus metadata belongs in a dedicated `[bus]` section rather than in
  generic `[properties]`.
- The accepted portable taxonomy is:
  - `integrated`
  - `amba`
  - `pci`
- Layer 2 should support at least the AMBA-shaped metadata needed by current
  planning and evaluation:
  - `type`
  - `amba_id`
  - `revision`
- This batch is upstream of lowering. Do not solve it with backend heuristics.
- The end-to-end validation proof should make `bus = "amba"` visible from raw
  frontend artifact through analysis/bootstrap so Phase D lowering can consume
  it as trusted input.

## Non-Goals

Do not include:

- Linux code emission
- Zephyr work
- evaluation-environment redesign
- heuristic bus inference in lowering
- PCI or E1000 expansion beyond the minimal typed bus support needed for the
  accepted taxonomy
- reopening unrelated Phase B or Phase C model work

## Acceptance Criteria

This batch is complete when:

- executable harness task specs exist for:
  - Layer 1 bus extraction
  - declaration-model bus normalization
  - Layer 2 bus parsing and binding normalization
  - analysis/bootstrap bus exposure
  - PL011 end-to-end bus identity proof
- `tasks.json` references the new task IDs under the appropriate groups with
  correct dependency order
- the new task batch is recorded in `.harness/task-spec-status.yaml`
- the existing Phase D lowering tasks can be made to depend on the new bus
  proof task instead of relying on heuristic inference

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - add `phase-c-04`
   - mark it `tasked`
   - populate `generated_task_ids`
2. Update `.harness/tasks.json` with the new tasks and dependencies.
3. Make `regseq-lowering-01` depend on the new end-to-end bus proof task.
4. Do not modify unrelated tracker items.
