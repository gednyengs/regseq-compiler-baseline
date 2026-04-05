# Task: regseq-model-12 - Layer 2 bus section parsing and binding normalization

## Description

Add typed Layer 2 bus-section support to the platform parser and normalized
binding model.

This task should turn `[bus]` from a generic unknown section into an accepted
typed Layer 2 family and add normalized binding-model support for bus metadata
needed by current PL011 planning.

## Input Context

Read before starting:

- `.harness/tasks/regseq-model/11-declaration-model-bus-identity-normalization.md`
- `docs/compiler-collateral/constructs/bus-resource-management.md`
- `crates/regseq-model/src/platform.rs`
- `crates/regseq-model/src/platform_parser.rs`
- `crates/regseq-model/src/binding/mod.rs`
- `crates/regseq-model/src/binding/normalize.rs`
- `crates/regseq-model/tests/platform_parser.rs`
- `crates/regseq-model/tests/binding_normalize.rs`

## Output Specification

Modify:

- `crates/regseq-model/src/platform.rs`
- `crates/regseq-model/src/platform_parser.rs`
- `crates/regseq-model/src/binding/`
- `crates/regseq-model/tests/`
- PL011 Layer 2 fixtures if needed

Add typed `[bus]` parsing and normalized binding-model support for Layer 2 bus
metadata.

## Implementation Notes

This task should support at least:

- `type`
- `amba_id`
- `revision`

The normalized binding model should gain a typed bus binding entry family
rather than preserving `[bus]` as an untyped unknown section.

The task should also define normalization-time consistency checks for:

- Layer 1 / Layer 2 bus-type mismatches
- invalid family-specific keys for the selected bus family

Do not:

- overload generic `[properties]` with bus metadata
- add Linux-only concepts to Layer 2 bus normalization
- defer all validation to lowering

## Test Plan

Add tests to verify:

1. `[bus]` parses as a typed section rather than an unknown section
2. AMBA-shaped bus metadata normalizes successfully
3. Layer 1 / Layer 2 bus mismatches are rejected or surfaced explicitly per
   the accepted normalization stance
4. unsupported family-specific keys are rejected
5. PL011 Layer 2 fixtures can carry AMBA bus metadata deterministically

Test commands:

- `cargo test -p regseq-model`
- `cargo clippy -p regseq-model -- -D warnings`

## Acceptance Criteria

- `[bus]` is a first-class parsed and normalized Layer 2 family
- AMBA metadata is available in the normalized binding model
- Layer 1 / Layer 2 bus consistency is enforced
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
