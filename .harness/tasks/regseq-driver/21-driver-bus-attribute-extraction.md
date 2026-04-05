# Task: regseq-driver-21 - Driver bus attribute extraction

## Description

Extend frontend extraction and raw frontend artifact capture so authored driver
identity includes `bus = "..."` from `#[rseq::driver(...)]`.

This task should make bus identity a real extracted Layer 1 fact rather than a
later heuristic.

## Input Context

Read before starting:

- `docs/compiler-collateral/constructs/bus-resource-management.md`
- `docs/discussions/pl011-full-spec/pl011-unified-spec-boundary.md`
- `crates/regseq-driver/src/extract/driver_identity.rs`
- `crates/regseq-driver/src/raw_artifact.rs`
- `crates/regseq-driver/tests/driver_identity.rs`
- `crates/regseq-driver/tests/raw_artifact.rs`

## Output Specification

Modify:

- `crates/regseq-driver/src/extract/`
- `crates/regseq-driver/src/raw_artifact.rs`
- `crates/regseq-driver/tests/`

Add authored bus capture to driver identity extraction and raw frontend
artifacts.

## Implementation Notes

This task should:

- extend `#[rseq::driver(...)]` parsing to accept `bus = "..."` alongside
  `name` and `compatible`
- reject duplicate `bus` keys
- reject unknown bus values outside the accepted portable taxonomy
- preserve the extracted bus value in `DriverIdentity`
- serialize it into the raw frontend artifact identity family

Accepted initial taxonomy:

- `integrated`
- `amba`
- `pci`

The task should preserve the current stance that:

- `compatible` remains concrete device identity
- `bus` is a separate portable attachment/fabric identity

Do not:

- infer bus from `compatible`
- add Layer 2 bus parsing here
- add backend/lowering logic here

## Test Plan

Add tests to verify:

1. `#[rseq::driver(..., bus = "...")]` extracts successfully
2. duplicate `bus` keys are rejected
3. unknown bus values are rejected
4. raw frontend artifact JSON includes the authored bus field
5. existing driver identity extraction behavior for `name` and `compatible`
   remains intact

Test commands:

- `cargo test -p regseq-driver`
- `cargo clippy -p regseq-driver -- -D warnings`

## Acceptance Criteria

- authored bus identity is captured by frontend extraction
- raw frontend artifacts preserve the bus field
- accepted bus taxonomy is enforced at extraction time
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
