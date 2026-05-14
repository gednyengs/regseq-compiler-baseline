# Spec Batch: regseqmodel-phase-2 - Unified RegseqModel structure

## Description

This batch authors executable harness task specs for the unified
RegseqModel structure: the module tree, driver struct representation,
type definitions with classification, register descriptors/blocks,
functions, constants, impl blocks, and the RegseqModel root struct.

These types replace the old DeclarationModel/BindingModel/JoinedArtifact
split with a single progressively-populated model.

The concrete outcome is a set of executable harness tasks that deliver
the complete model structure needed for Phase 5 (IR construction) and
Phase 6 (normalization + join).

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-status.yaml`
- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 2)
- `docs/discussions/models/regseq-model/spec.md` (Sections 2-6, 10, 11)
- `crates/regseq-model/src/identity/` (Phase 0 types)
- `crates/regseq-model/src/ir/` (Phase 0 + 1 types)

## Output Specification

Create:

- `.harness/tasks/regseq-model/` task spec files

Update when done:

- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`

The output is executable harness tasks.

## Implementation Notes

- All types live under `crates/regseq-model/src/model/`.
- Progressive population: declaration fields populated in Phase 1
  normalization, binding slots (`Option<T>`) populated in Phase 2 join.
- Module tree mirrors source organization: `Module` with
  `kind: Driver|Normal`, nested submodules.
- Driver struct has three field categories: resources (with `Shape`),
  properties (with `bound_value: Option`), user data (with `inner_type`).
- Type definitions carry `classification: UserDefined | Abstraction |
  Framework`.
- `RegseqModel` root struct owns `SymbolInterner`, `source_locations`,
  `root_module`, `format_version`.

- **Task decomposition**: See execution plan steps 2.1-2.11 for the
  file-level split.

## Non-Goals

- No IR construction logic -- Phase 5.
- No normalization or join logic -- Phase 6.
- No changes to existing regseq-model modules.

## Acceptance Criteria

- All task spec files created.
- Each includes scope, non-goals, test plan, review section, quality gate.
- Tasks registered. Tracker updated.

## Update Instructions

1. Change `regseqmodel-phase-2` status to `tasked`.
2. Fill `generated_task_ids`.
3. Check if `regseqmodel-phase-5` dependency chain is advancing.
