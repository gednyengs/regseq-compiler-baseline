# Task: track-a-validation-03 - Minimal PL011 Layer 1 / Layer 2 compatibility proof

## Description

Add the final Phase B boundary-compatibility proof for the real PL011 slice by
showing that the emitted raw Layer 1 frontend artifact and the parsed
`platform.toml` can be compared coherently at the boundary without yet
performing the final Phase C join.

This task should be the last frontend-boundary check before normalized model
work begins.

## Input Context

Read before starting:

- docs/plans/asplos-implementation-plan.md
- docs/discussions/layer-2-format/layer-2-format-discussion.md
- docs/discussions/pl011-spec-slice/pl011-spec-slice-discussion.md
- .harness/tasks/regseq-driver/19-raw-frontend-artifact-emission.md
- .harness/tasks/track-a-pl011/04-pl011-raw-frontend-artifact-snapshot.md
- .harness/tasks/track-a-validation/01-layer2-platform-file-fixtures-and-boundary-smoke.md
- tests/fixtures/pl011-spec-slice/platform.toml
- tests/fixtures/pl011-spec-slice/expected/raw-frontend.json
- tests/e2e/tests/platform_file_boundary.rs

## Output Specification

Create or modify:

- tests/e2e/tests/pl011_layer_boundary.rs

Modify if needed:

- tests/e2e/Cargo.toml
- tests/e2e/src/lib.rs

## Implementation Notes

This task should use the **real PL011 slice**, not a synthetic mini-driver, as
the main evidence source.

The proof should stay intentionally minimal:

- parse the checked-in `platform.toml`
- load the raw frontend artifact emitted from the real PL011 slice
- compare the two at the boundary vocabulary level

### What to prove

At minimum, the boundary proof should show that:

- the platform file satisfies the current B-07 header contract:
  - `version`
  - `platform.os`
  - `platform.board`
  - `platform.arch`
- the Layer 2 property ids in `[properties]` correspond to property ids declared
  in the real PL011 Layer 1 slice
- the selected target OS and the parsed `platform.os` stay coherent for the
  closure proof

### Important boundary

Do not turn this into Phase C.

Specifically, do not implement:

- final join logic
- required-property completeness
- Rust-field type compatibility checks
- default-value interpretation semantics
- normalized binding models

The comparison should stay at the level of:

- shared ids / names
- presence / absence
- basic header compatibility

The test should only compare:

- parsed Layer 2 header fields
- the set of property ids present in `[properties]`

It should not assert or interpret any future-section semantics.

### Relationship to earlier boundary smoke

`track-a-validation-01` used a dedicated property-focused fixture to prove the
boundary in the small.

This task should now prove the same boundary discipline against the **real PL011
slice**, using the raw artifact instead of source-text heuristics.

## Non-Goals

Do not implement:

- final joined PL011 binding artifact
- property completeness validation
- platform-to-Rust type compatibility
- backend lowering assumptions

## Test Plan

Tests/commands to support:

1. Real PL011 platform file parses
2. Real PL011 raw artifact loads
3. Boundary compatibility holds at the shared-vocabulary level
4. Workspace tests and clippy remain green

Suggested commands:

- `cargo test -p regseq-e2e --test pl011_layer_boundary -- --nocapture`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

## Acceptance Criteria

- a dedicated real-PL011 Layer 1 / Layer 2 compatibility test exists
- the test uses the raw frontend artifact rather than source-text scanning
- the test stays at boundary-comparison level and does not implement Phase C
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
