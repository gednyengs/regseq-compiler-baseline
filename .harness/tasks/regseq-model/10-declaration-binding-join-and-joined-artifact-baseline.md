# Task: regseq-model-10 - Declaration-binding join and joined-artifact baseline

## Description

Implement the first real join path between normalized declaration IR and
normalized binding IR, and define the first reviewer-facing joined artifact.

This task should establish the actual Layer 1 + Layer 2 join boundary for the
supported PL011 frontend without pulling in backend lowering structure.

## Input Context

Read before starting:

- `docs/discussions/regseq-model/phase-c-02-binding-and-join-plan.md`
- `crates/regseq-model/src/declaration/`
- `.harness/tasks/regseq-model/08-binding-model-schema-and-boundaries.md`
- `.harness/tasks/regseq-model/09-parsed-platform-artifact-to-binding-model-normalization.md`
- `docs/compiler-collateral/compiler-baseline-overview.md`
- relevant construct docs under `docs/compiler-collateral/constructs/`

## Output Specification

Modify:

- `crates/regseq-model/src/`
- `crates/regseq-model/tests/`

Add the join entry points, joined-artifact types, and snapshot/review helpers
needed to combine normalized declaration and binding models into a stable joined
artifact.

## Implementation Notes

The join should follow these fixed decisions:

- declaration IR and binding IR remain first-class inputs; the joined artifact
  is a separate IR family
- the join should make explicit what joined, what remained declaration-only,
  and what remained binding-only
- explicit attribute/id join and field-name join are the only accepted
  mechanisms in this task
- the joined artifact must preserve provenance back to both sides

For the supported PL011 frontend, the join should explicitly cover:

- property declarations against Layer 2 `[properties]`
- mmio resources against Layer 2 mmio bindings
- interrupt resources against Layer 2 interrupt bindings
- clock resources against Layer 2 clock bindings
- reset resources against Layer 2 reset bindings
- pinctrl resources against Layer 2 pin-state/binding sections

Per-resource join-key conventions for this batch are:

- properties: explicit property id
- mmio: explicit `mmio_id` where present
- interrupts: explicit `interrupt_id`
- clocks: explicit `clock_id`
- resets: field-name join
- pinctrl: explicit `pinctrl_id` where present, otherwise field-name join

The joined artifact should be reviewable and deterministic, but it is still not
backend lowering IR.

Do not:

- widen the accepted join mechanisms
- reinterpret declaration-side interrupt binding hints as authoritative Layer 2
  bindings
- perform backend/lowering shaping
- redesign declaration or binding schemas inside this task
- fold all joined facts into one opaque resolution map

This task should also define the join stance for unmatched inputs:

- unmatched declaration-side facts stay explicit
- unmatched binding-side facts stay explicit
- no silent drop of either side

The join should also define the stance for two important non-binary outcomes:

- matched-but-incomplete bindings are valid join outcomes and must be
  representable without pretending the resource/property is fully satisfied
- unknown binding-side sections that are preserved outside the normalized
  binding model should remain visible in the binding-only/unmatched side rather
  than disappearing

For interrupt joins specifically:

- binding-model interrupt values are authoritative Layer 2 facts
- declaration-side interrupt binding hints may be preserved only as
  diagnostic/provenance-side annotations
- they must not be merged into the authoritative joined interrupt binding

## Test Plan

Add join-focused tests to verify:

1. a representative declaration model and binding model join successfully
2. explicit-id joins work where accepted
3. field-name joins work where accepted
4. unmatched declaration-side and binding-side facts remain reviewable
5. joined output is deterministic and snapshot-friendly
6. joined provenance points back to both declaration and binding origins
7. matched-but-incomplete binding sections are representable as partial join
   outcomes
8. interrupt sidecar hints do not override authoritative binding-model values

Use real PL011-shaped declaration/binding inputs where possible.

Test commands:

- `cargo test -p regseq-model`
- `cargo clippy -p regseq-model -- -D warnings`

## Acceptance Criteria

- normalized declaration and binding models can be joined into a stable joined
  artifact
- the join boundary is explicit and reviewable
- joined output preserves both-side provenance and unmatched facts
- no backend/lowering shaping is introduced
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
