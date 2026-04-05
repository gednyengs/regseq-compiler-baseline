# Task: regseq-analysis-04 - Analysis bootstrap exposure for bus identity and metadata

## Description

Expose explicit Layer 1 bus identity and Layer 2 bus metadata through
`AnalysisInput`, and add readiness-side visibility for bus consistency and
required bus facts.

This task should make bus information part of the stable lowering-input
contract that Phase D consumes.

## Input Context

Read before starting:

- `.harness/tasks/regseq-model/12-layer2-bus-section-parsing-and-binding-normalization.md`
- `docs/discussions/regseq-analysis/phase-c-03-analysis-bootstrap-plan.md`
- `docs/compiler-collateral/constructs/bus-resource-management.md`
- `crates/regseq-analysis/src/input.rs`
- `crates/regseq-analysis/src/readiness.rs`
- `tests/e2e/tests/pl011_lowering_input_stability.rs`

## Output Specification

Modify:

- `crates/regseq-analysis/src/`
- `crates/regseq-analysis/tests/`
- `tests/e2e/tests/` if reviewer-facing bus-output proofs belong there

Expose bus identity and metadata through the analysis/bootstrap layer and add
the corresponding readiness-side checks.

## Implementation Notes

This task should:

- expose authored Layer 1 bus identity through `AnalysisInput`
- expose normalized Layer 2 bus metadata through `AnalysisInput`
- add readiness/consistency checks for:
  - Layer 1 / Layer 2 bus-family mismatch
  - missing required bus metadata where the accepted bus family requires it
  - unsupported or malformed bus-family facts that survived earlier phases
- keep the output reviewer-facing and deterministic

Do not:

- add backend-specific planning here
- move bus validation into lowering
- flatten bus facts into generic property summaries

## Test Plan

Add tests to verify:

1. `AnalysisInput` exposes authored bus identity
2. `AnalysisInput` exposes normalized bus metadata
3. readiness output surfaces bus-family mismatches and required-fact gaps
4. PL011 lowering-input stability output now includes `bus = "amba"`

Test commands:

- `cargo test -p regseq-analysis`
- `cargo test -p regseq-e2e`
- `cargo clippy -p regseq-analysis -- -D warnings`

## Acceptance Criteria

- bus identity and bus metadata are part of the stable analysis/bootstrap
  contract
- readiness output can report bus-family conflicts and missing required bus
  facts
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
