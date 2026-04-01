# Task: track-a-validation-02 - Caution-focused PL011 frontend QC

## Description

Add a narrow, caution-focused frontend quality-control pass over the accepted
PL011 spec slice using the raw frontend artifact as the main evidence source.

This task exists to close the “it compiles, but did we really capture the
important semantics?” gap before Phase C begins.

## Input Context

Read before starting:

- docs/plans/asplos-implementation-plan.md
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/compiler-constraints.md
- archive/guide/open-issues.md
- .harness/tasks/regseq-driver/19-raw-frontend-artifact-emission.md
- .harness/tasks/track-a-pl011/04-pl011-raw-frontend-artifact-snapshot.md
- tests/fixtures/pl011-spec-slice/expected/raw-frontend.json
- tests/fixtures/pl011-spec-slice/src/driver.rs
- tests/e2e/tests/pl011_spec_slice.rs

## Output Specification

Create or modify:

- tests/e2e/tests/pl011_frontend_qc.rs

Modify if needed:

- tests/e2e/Cargo.toml
- tests/e2e/src/lib.rs

## Implementation Notes

This task should use the **real PL011 slice** as the main evidence source.
Only add tiny extra fixtures if a frontend edge cannot be isolated from the real
slice itself.

### What to QC

Focus on frontend elements that are accepted and present in the current slice,
especially where the collateral flags caution around compiler-managed meaning.

For the accepted PL011 polling slice, that means:

- firmware-property declarations are preserved with the right raw distinctions:
  - `reg-io-width` is required and carries a declared default
  - `current-speed` is optional
- optional resource declarations remain visibly optional in raw evidence:
  - clock
  - reset line
  - pinctrl
- callback-family capture remains coherent:
  - `Lifecycle`
  - `Serial`
- callback-context classification remains coherent for the accepted callback
  families present in the slice
- register-access raw evidence remains present for the real polling path:
  - wrapper call-site evidence involving `IBRD` / `FBRD`
  - wrapper call-site evidence involving `LCR_H`
  - wrapper call-site evidence involving `DR`
  - wrapper call-site or poll evidence involving `FR`

### Evidence source

Prefer reading the emitted/checked-in raw artifact plus the real fixture source
where helpful.

Do not rely on stderr-only debug strings for the QC proof if the raw artifact is
available.

The QC should intentionally stay at the raw frontend level:

- no final joined-model assertions
- no backend lowering assumptions
- no lifecycle-generated preamble assumptions beyond what the accepted baseline
  already declares as compiler-managed

### Caution note

The open issues specifically call out declarative firmware-property handling as
accepted with caution. This QC task should therefore make the property shape in
the PL011 slice especially visible and reviewer-friendly.

If helper functions contain the direct register accesses while trait callbacks
delegate to them, treat that as acceptable raw frontend evidence so long as the
artifact preserves both the callback family and the actual containing function
where the wrapper call was seen.

## Non-Goals

Do not implement:

- new extraction families
- normalized model checks
- backend-specific logging/property generation checks
- full-device PL011 widening beyond the accepted slice

## Test Plan

Tests/commands to support:

1. QC test loads the real PL011 raw artifact
2. QC assertions cover the accepted caution areas present in the slice
3. QC remains stable enough for repeated regression use

Suggested commands:

- `cargo test -p regseq-e2e --test pl011_frontend_qc -- --nocapture`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

## Acceptance Criteria

- a dedicated caution-focused QC test exists for the real PL011 slice
- the QC proves more than “artifact file exists”
- firmware-property and optional-resource evidence are explicitly reviewed
- callback-family/callback-context evidence is explicitly reviewed
- the task stays at raw frontend QC level
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
