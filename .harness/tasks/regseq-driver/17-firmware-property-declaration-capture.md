# Task: regseq-driver-17 - Firmware property declaration capture

## Description

Implement the narrow compiler-facing extraction scaffolding for accepted
 firmware-property declarations. This task should prove `regseq-driver` can
 recover declaration-side property metadata from HIR and preserve enough
 structure for the later Layer 2 property frontier, without introducing Layer 2
 parsing or generated-property behavior.

## Input Context

Read before starting:

- crates/regseq-driver/src/extract/mod.rs
- crates/regseq-driver/src/callbacks.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/firmware-property-access.md
- docs/compiler-collateral/compiler-constraints.md

## Output Specification

Create:

- crates/regseq-driver/src/extract/property.rs
- crates/regseq-driver/tests/property.rs

Modify:

- crates/regseq-driver/src/extract/mod.rs

## Implementation Notes

Recognize and extract only the declaration-side property forms:

- `#[rseq::property(id = "...")]` on driver fields
- optional `default = ...`
- optional `vendor_prefix = "..."`
- required vs optional semantics from field type:
  - `T`
  - `Option<T>`

The extraction output should remain internal to `regseq-driver`. Narrow records
 such as `PropertySurface`, `PropertyField`, and `PropertyLint` are
 appropriate.

Required capture details:

- driver type name containing the declaration
- field name
- field Rust type in a recoverable form sufficient for later property/type
  validation work; for this batch, a simple recoverable representation such as
  a type path string, canonical path, or equivalent is sufficient
- `property_id`
- optional `default` literal payload in a recoverable raw form
- optional `vendor_prefix`
- required vs optional boolean derived from `T` vs `Option<T>`

Literal-handling rule for this batch:

- preserve `default = ...` as a raw recoverable literal/token representation
- do not interpret it as a typed Rust value
- do not validate the literal against the field Rust type
- do not attempt integer-range validation here

Required declaration-side checks for this batch:

- reject malformed `#[rseq::property(...)]` attributes deterministically
- reject duplicate property ids within a single driver type
- preserve a warning/lint fact when a required field (`T`) also declares a
  `default`

Declaration-side checks intentionally deferred to Phase B-07 or later:

- `[properties]` parsing
- required-property completeness against compiled-in fallback data
- platform/type mismatch checks
- enum-variant validation against platform-file values
- integer-range validation against parsed Layer 2 property values
- generated `read_property`

Important boundary:

- this task should capture declaration metadata only
- it must not try to read firmware values
- it must not join against Layer 2 yet
- it must not populate driver fields

## Test Plan

Create:

- crates/regseq-driver/tests/property.rs

Tests to implement:

1. Verify representative property declarations are recognized and preserve:
   - field name
   - property id
   - required vs optional status
   - default literal when present
   - vendor prefix when present
2. Verify duplicate property ids within one driver are rejected
   deterministically.
3. Verify malformed property attributes are rejected deterministically.
4. Verify `required + default` is surfaced as a lint/warning fact rather than
   a hard extraction failure.
5. Verify unrelated field attributes are ignored.
6. Verify the extracted field-type information is recoverable enough for later
   Layer 2/property validation work.

Test command: `cargo test -p regseq-driver --test property`

## Acceptance Criteria

- `regseq-driver` can recover the accepted declaration-side firmware-property
  surface from HIR
- duplicate ids and malformed attributes are rejected deterministically
- `required + default` is preserved as a non-fatal declaration-side lint fact
- the task stays strictly on the declaration side and does not drift into
  Layer 2 parsing or generated-property behavior
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
