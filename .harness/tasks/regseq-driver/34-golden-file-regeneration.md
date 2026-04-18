# regseq-driver-34: Golden file regeneration

## Description

Regenerate golden JSON files after extraction output changes from the
new registry surfaces and updated test specimens. Update E2E assertions
to match the new output format.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 4.6)
- `tests/fixtures/pl011-full-spec/expected/raw-frontend.json` (main golden file)
- `crates/regseq-driver/tests/` (test files with expected outputs)
- `tests/e2e/tests/` (E2E test files)

## Implementation Notes

### Golden file regeneration

1. Run the extraction pipeline on PL011 full-spec with all new
   extraction surfaces enabled.
2. Capture the new raw-frontend.json output.
3. Compare with the existing golden file to verify:
   - All existing surfaces are preserved.
   - New surfaces (intrinsic_registry, framework_types,
     abstraction_types, source_locations) appear correctly.
4. Replace the golden file with the new output.

### E2E test updates

First audit which of the 9 E2E tests in `tests/e2e/tests/` compare
raw artifact output vs. normalized model output. Only
raw-artifact-comparing tests need updates for the new registry fields.

Review those E2E tests and update assertions to account for:
- New fields in the artifact JSON.
- Any changed field values from updated test specimens.
- Pin state information in pin control surfaces.

### Unit test expected outputs

For each of the 33+ unit tests, if the test compares extraction output
against expected values, update those expected values to match the new
extraction output.

### Verification approach

The goal is NOT to change extraction logic -- that was done in tasks
27-32. This task only regenerates expected outputs to match the now-
correct extraction. All diffs should be additive (new fields) or
mechanical (updated attribute values).

## Non-Goals

- No extraction logic changes.
- No new test specimens.
- No normalization or IR changes.

## Test Plan

- `cargo test -p regseq-driver` passes with updated golden files.
- `cargo test --workspace` passes (including E2E tests).
- New golden file contains intrinsic_registry, framework_types,
  abstraction_types, and source_locations sections.
- Diff between old and new golden files is reviewable and consists
  only of expected additions.

## Review

After tests pass, use `codex:rescue` to review. Verify golden file
diffs are additive and correct. Verify no unexpected regressions in
existing surfaces. Iterate until approved.

## Quality Gate

```
cargo test --workspace
cargo fmt --all -- --check
```

## Acceptance Criteria

- All golden files regenerated.
- All E2E assertions updated.
- `cargo test --workspace` passes. Codex review approved.
