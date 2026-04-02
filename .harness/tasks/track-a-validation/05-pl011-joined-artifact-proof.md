# Task: track-a-validation-05 - PL011 joined-artifact proof

## Description

Prove that the normalized declaration model and normalized binding model join
cleanly for the supported PL011 frontend and accepted Layer 2 contract.

This is a validation-facing proof task, not a schema-design task. It should
exercise the joined artifact boundary with real PL011-shaped inputs and make
the outcome reviewer-facing.

## Input Context

Read before starting:

- `docs/discussions/regseq-model/phase-c-02-binding-and-join-plan.md`
- `.harness/tasks/regseq-model/08-binding-model-schema-and-boundaries.md`
- `.harness/tasks/regseq-model/09-parsed-platform-artifact-to-binding-model-normalization.md`
- `.harness/tasks/regseq-model/10-declaration-binding-join-and-joined-artifact-baseline.md`
- `tests/fixtures/pl011-spec-slice/`
- `tests/fixtures/pl011-full-spec/`

## Output Specification

Modify:

- `tests/e2e/tests/`
- `crates/regseq-model/tests/` only if small supporting fixtures/snapshots are
  needed

Add focused validation and snapshot-style proof that the joined PL011 artifact:

- is produced from real declaration and binding inputs
- is reviewable
- preserves joined and unjoined structure honestly

## Implementation Notes

This proof should cover the accepted join boundary only.

Use concrete PL011 inputs to show:

- property join
- mmio join where available
- interrupt join
- clock join
- reset join
- pinctrl join
- unmatched fact handling where relevant

The current checked-in PL011 `platform.toml` files do not yet contain the
resource construct sections needed to prove all of these joins. This task must
therefore own one of the following:

- enriching the PL011 Layer 2 fixture inputs with the accepted construct
  sections needed for the proof, or
- constructing equivalent PL011-shaped inline test inputs that include those
  sections

The proof should not rely only on `[platform]` + `[properties]` inputs.

Do not turn this into:

- declaration-model redesign
- binding-model redesign
- backend/lowering validation
- optional richer serial feature work
- DMA expansion

## Test Plan

Add validation tests to verify:

1. the PL011 declaration artifact and binding artifact join successfully
2. joined output is deterministic/reviewable
3. expected PL011 resources/properties join to the right Layer 2 entries
4. unmatched facts remain explicit rather than disappearing
5. partial join outcomes are reviewable when a binding section exists but is
   incomplete

Test commands:

- `cargo test -p regseq-model`
- `cargo test -p regseq-e2e`
- `cargo clippy --workspace -- -D warnings`

## Acceptance Criteria

- there is a reviewer-facing joined-artifact proof for PL011
- the proof covers both successful joins and unmatched-fact handling
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
