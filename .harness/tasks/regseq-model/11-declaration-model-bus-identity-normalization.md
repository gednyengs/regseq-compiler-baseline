# Task: regseq-model-11 - Declaration-model bus identity normalization

## Description

Normalize authored Layer 1 bus identity into the declaration model.

This task should add a typed portable bus field to `DriverDecl`, normalize it
from raw frontend input, and update declaration-side fixtures/tests so bus is a
stable declaration-model fact.

## Input Context

Read before starting:

- `.harness/tasks/regseq-driver/21-driver-bus-attribute-extraction.md`
- `docs/compiler-collateral/constructs/bus-resource-management.md`
- `crates/regseq-model/src/declaration/driver.rs`
- `crates/regseq-model/src/declaration/raw_input.rs`
- `crates/regseq-model/src/declaration/normalize.rs`
- `crates/regseq-model/tests/declaration_normalize.rs`
- `crates/regseq-model/tests/declaration_reviewability.rs`

## Output Specification

Modify:

- `crates/regseq-model/src/declaration/`
- `crates/regseq-model/tests/`
- declaration-side expected raw frontend fixtures if needed

Add a typed portable bus identity to the declaration model and preserve it
through normalization.

## Implementation Notes

This task should:

- add a typed portable bus enum to declaration-side driver identity
- add a `bus` field to `RawIdentityInput`
- add a `bus` field to `DriverDecl`
- normalize the extracted raw bus into the typed declaration model
- update tests and fixtures to expect bus on real PL011 artifacts

Use the accepted portable taxonomy:

- `integrated`
- `amba`
- `pci`

Do not:

- add Layer 2 bus parsing here
- infer bus from `compatible`
- add backend-specific bus notions to the declaration model

## Test Plan

Add tests to verify:

1. raw frontend input with `bus = "amba"` normalizes successfully
2. declaration-model driver identity preserves the typed bus value
3. real PL011 declaration fixtures expose `bus = "amba"`
4. declaration-model output remains deterministic and reviewable

Test commands:

- `cargo test -p regseq-model`
- `cargo clippy -p regseq-model -- -D warnings`

## Acceptance Criteria

- declaration normalization preserves authored bus identity
- PL011 declaration artifacts expose `bus = "amba"`
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
