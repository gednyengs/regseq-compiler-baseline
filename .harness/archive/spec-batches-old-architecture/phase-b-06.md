# Spec Batch: phase-b-06 - declarative compiler-managed frontend surfaces

## Description

Author the sixth Phase B batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the declarative compiler-managed frontend surfaces used
by the accepted PL011 slice. The output should cover accepted property and
power-management metadata, along with any closely related compiler-facing
surfaces that carry intent without introducing new callback or wrapper APIs.

The output should make these declarations authorable and capturable without
drifting into Layer 2 parsing, normalized models, or final join logic.

Concretely, the center of gravity for this batch is:

- firmware property declarations such as:
  - `#[rseq::property(id = "...")]`
  - `default = ...`
  - `vendor_prefix = "..."`
  - required vs optional semantics from field type `T` vs `Option<T>`
- power-management gate metadata such as:
  - `#[rseq::pm(idle = ...)]`
  - `#[rseq::pm(wakeup_capable = ...)]`

This batch should make those declarations:

- available at the authored Layer 1 surface
- visible to `regseq-driver` extraction
- structurally validated where the accepted collateral already requires
  declaration-side checks

This batch is intentionally the declarative half of a later split:

- Phase B-06: Layer 1 declaration surfaces and declaration-side capture
- Phase B-07: Layer 2 binding format, parser, and property capture
- Phase C: normalized model and Layer 1 + Layer 2 join logic

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`
- `docs/plans/asplos-implementation-plan.md`
- `docs/compiler-collateral/compiler-baseline-overview.md`
- `docs/compiler-collateral/compiler-constraints.md`
- `docs/compiler-collateral/constructs/device-power-state-management.md`
- `docs/compiler-collateral/constructs/firmware-property-access.md`
- `archive/guide/open-issues.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-facade/`
- `.harness/tasks/regseq-driver/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs for the
declarative compiler-managed surfaces across the appropriate groups.

## Implementation Notes

Follow these fixed decisions:

- keep this batch focused on declarative metadata whose meaning is primarily
  compiler-managed
- include accepted property and power-management declaration forms only where
  they are part of the supported PL011 slice
- reuse earlier shared foundations and wrapper carriers where these declarations
  attach to items or fields
- do not introduce Layer 2 parser work, normalized models, or join logic here
- do not reopen the `PowerManagement` trait surface from Phase B-04; this batch
  owns the declarative PM gate metadata, not the callback trait itself
- do not introduce a driver-invoked firmware-property API; property reads remain
  compiler-managed

The likely declaration families in this batch are:

- field-level firmware-property declarations
  - property id
  - optional default
  - optional vendor prefix
  - required vs optional field semantics
- item-level PM gate declarations
  - idle policy
  - wakeup capability

Expected driver-side work in this batch should stay narrow:

- capture the accepted declaration shapes from HIR
- preserve enough structured metadata for later Layer 2/property work
- enforce declaration-side invariants that do not require the full Layer 2
  parser or final join

Decompose into narrow tasks such as:

- firmware-property declaration surface
- firmware-property declaration capture and declaration-side validation
- PM gate metadata surface
- PM gate capture and structural validation

Use this batch to make the declarations authorable and capturable, not to solve
their eventual generated behavior. In particular:

- `read_property` generation belongs later
- PM registration and PM ordering lowering belong later
- platform-file `[properties]` parsing belongs in Phase B-07
- final required-property completeness checks that depend on parsed Layer 2 data
  belong after the Layer 2 frontier is in place

## Non-Goals

Do not include:

- Layer 2 parser and property-binding work
- normalized model and join work
- raw-artifact closure work
- backend lowering
- E1000 expansion
- redefinition of callback traits or typed wrapper APIs that were already owned
  by earlier Phase B batches

## Acceptance Criteria

This batch is complete when:

- one or more executable task specs exist under the declarative-surface target groups
- firmware-property and power-management declaration surfaces are explicitly represented
- the task specs clearly separate declaration-side capture from later Layer 2
  parsing and later join/lowering work
- the task specs keep this work declarative and compiler-managed
- `tasks.json` references the new task IDs under the appropriate groups
- `phase-b-06` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-b-06` to `tasked`
   - populate `generated_task_ids` with the new task IDs
2. Move the next dependency-satisfied item to `ready_to_elaborate`.
3. Do not modify unrelated tracker items.
