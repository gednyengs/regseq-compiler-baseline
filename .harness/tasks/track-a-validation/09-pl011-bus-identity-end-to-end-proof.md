# Task: track-a-validation-09 - PL011 bus identity end-to-end proof

## Description

Add the reviewer-facing proof that PL011 carries explicit AMBA bus identity
end-to-end from authored source through the analysis/bootstrap contract.

This task is the structural gate that should land before Phase D lowering
resumes.

## Input Context

Read before starting:

- `.harness/tasks/regseq-analysis/04-analysis-bootstrap-exposure-for-bus-identity-and-metadata.md`
- `docs/compiler-collateral/constructs/bus-resource-management.md`
- `docs/discussions/pl011-full-spec/pl011-unified-spec-boundary.md`
- `docs/discussions/linux-codegen/lowering-plan-interface-boundary.md`
- `tests/fixtures/pl011-full-spec/`
- `tests/e2e/tests/pl011_lowering_input_stability.rs`

## Output Specification

Modify:

- `tests/e2e/tests/`
- `crates/regseq-analysis/tests/` only if narrow shared helpers or snapshots
  are needed

Add reviewer-facing validation that demonstrates `bus = "amba"` survives the
full model/bootstrap pipeline for PL011.

## Implementation Notes

This proof should cover:

- authored Layer 1 bus identity in the frontend artifact
- declaration-model bus identity
- Layer 2 `[bus]` metadata where the PL011 fixture uses it
- analysis/bootstrap visibility for both
- deterministic reviewer-facing output that later lowering tasks can trust

The proof should explicitly demonstrate:

- PL011 is not falling back to generic `integrated` or `platform`
- `compatible = ["arm,pl011"]` and `bus = "amba"` remain complementary facts
- Layer 2 AMBA metadata remains visible without being collapsed into generic
  properties

Do not turn this task into:

- lowering validation
- Linux backend validation
- QEMU smoke validation

## Test Plan

Add tests to verify:

1. PL011 frontend/declaration artifacts carry `bus = "amba"`
2. PL011 Layer 2 bus metadata is visible and deterministic
3. analysis/bootstrap output exposes both Layer 1 and Layer 2 bus facts
4. reviewer-facing proofs fail if PL011 regresses to non-AMBA bus identity

Test commands:

- `cargo test -p regseq-analysis`
- `cargo test -p regseq-e2e`
- `cargo clippy --workspace -- -D warnings`

## Acceptance Criteria

- there is a reviewer-facing end-to-end proof for PL011 bus identity
- the proof establishes `bus = "amba"` before lowering resumes
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
