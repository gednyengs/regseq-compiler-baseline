# Task: track-a-validation-01 - Layer 2 platform-file fixtures and boundary smoke

## Description

Create the validation fixture corpus and minimal boundary-smoke evidence for the
 initial Layer 2 platform-file contract. This task exists to prove the accepted
 Layer 2 input shape is reviewable, parsable, and capable of being related to
 the already-captured Layer 1 firmware-property surface without yet defining
 the final join model.

This is a validation/evidence task. It should stay narrow and fixture-driven.

## Input Context

Read before starting:

- docs/plans/asplos-implementation-plan.md
- docs/compiler-collateral/constructs/firmware-property-access.md
- docs/discussions/layer-2-format/layer-2-format-discussion.md
- tests/e2e/tests/driver_e2e.rs
- tests/fixtures/minimal-driver/platform.toml
- .harness/tasks/regseq-driver/17-firmware-property-declaration-capture.md
- .harness/tasks/regseq-model/02-layer2-platform-file-schema-and-parsed-artifact.md
- .harness/tasks/regseq-model/03-toml-platform-file-parser-and-section-preservation.md
- .harness/tasks/regseq-model/04-properties-section-capture-and-parser-side-validation.md

## Output Specification

Create:

- tests/fixtures/platform-file/minimal-valid.toml
- tests/fixtures/platform-file/with-properties.toml
- tests/fixtures/platform-file/with-future-sections.toml
- tests/fixtures/platform-file/missing-version.toml
- tests/fixtures/platform-file/missing-platform-key.toml
- tests/fixtures/platform-file/unsupported-property-value.toml
- tests/fixtures/property-driver/Cargo.toml
- tests/fixtures/property-driver/platform.toml
- tests/fixtures/property-driver/src/lib.rs
- tests/e2e/tests/platform_file_boundary.rs

Modify if needed:

- tests/e2e/Cargo.toml
- tests/e2e/src/lib.rs
- tests/fixtures/minimal-driver/platform.toml

## Implementation Notes

This task should provide two kinds of evidence:

### 1. Parser/validation fixture corpus

Create a small checked-in corpus of TOML platform files that exercise:

- minimal valid file
- valid `[properties]`
- valid future sections preserved without semantic validation
- missing required top-level contract
- unsupported property value kinds

These fixtures are meant to be:

- human-reviewable
- stable
- reusable by `regseq-model` parser tests and future validation work

### 2. Minimal Layer 1 / Layer 2 boundary smoke test

Create a small property-focused fixture driver and a smoke test that proves,
 without the final normalized join model, that:

- a Layer 1 driver with `#[rseq::property(...)]` declarations exists
- a Layer 2 platform file with `[properties]` entries exists
- the two can be loaded and compared at the level of property ids / presence
  as a boundary compatibility check

Important boundary:

- do not implement the final join model
- do not implement required-property completeness
- do not implement Rust-type compatibility checks
- do not implement lowering

The smoke test may stay lightweight and may use whatever narrow internal test
 scaffolding is already available, but it should prove something stronger than
 “the parser runs.” At minimum it should demonstrate that the property ids
 declared in the Layer 1 fixture and present in the parsed Layer 2 file can be
 related as a shared boundary vocabulary.

Preferred evidence path for this task:

- use the existing rustc/extraction test path where practical so the Layer 1
  side comes from real `#[rseq::property(...)]` extraction rather than a
  hand-simulated declaration list
- the test still stops at boundary comparability; it must not implement the
  final join model

If the existing `minimal-driver` fixture is too small or too generic for this,
 keep it unchanged and add a new dedicated property-focused fixture instead.

## Test Plan

Tests/fixtures to implement:

1. Fixture corpus covers:
   - valid minimal contract
   - valid `[properties]`
   - valid future sections
   - missing required keys
   - unsupported property value kinds
2. Boundary smoke test verifies:
   - property-focused Layer 1 driver fixture is valid
   - its platform file parses
   - property ids can be related across the boundary without a final join
3. The boundary smoke should remain lightweight and stable enough for repeated
   regression use.

Suggested commands:

- `cargo test -p regseq-model --test platform_parser --test platform_properties`
- `cargo test -p regseq-e2e platform_file_boundary -- --nocapture`
- `cargo test --workspace`

## Acceptance Criteria

- a checked-in fixture corpus exists for the initial Layer 2 platform-file
  contract
- the corpus exercises both valid and invalid representative inputs
- a minimal Layer 1 / Layer 2 boundary smoke test exists
- the task stays at the evidence/validation layer and does not implement the
  final join model
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
