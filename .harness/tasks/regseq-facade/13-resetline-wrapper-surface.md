# Task: regseq-facade-13 - ResetLine wrapper surface

## Description

Implement the shared typed `ResetLine` resource surface for the accepted PL011
frontend slice. This task owns only the facade-side wrapper declarations and
public exports.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- crates/regseq/src/error.rs
- crates/regseq/src/time.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/hardware-reset-sequencing-and-post-reset-polling.md

## Output Specification

Create:

- crates/regseq/src/reset.rs
- crates/regseq/tests/reset.rs

Modify:

- crates/regseq/src/lib.rs

## Implementation Notes

Add the stable typed resource shell and wrapper methods:

```rust
pub struct ResetLine {
    // opaque resource shell
}

impl ResetLine {
    pub fn assert(&self) -> Result<(), RegseqError> { unimplemented!() }
    pub fn deassert(&self) -> Result<(), RegseqError> { unimplemented!() }
    pub fn toggle(&self, delay: Duration) -> Result<(), RegseqError> {
        unimplemented!()
    }
}
```

Phase-B baseline:

- `ResetLine` is an opaque typed resource carrier
- `ResetLine` and `Option<ResetLine>` are the accepted Layer 1 declaration
  shapes
- wrapper receivers use `&self`
- all three stable reset operations are fallible on the facade surface
- `toggle` must consume the shared `regseq::Duration` type

Do not include:

- compiler-managed `acquire_reset`
- field-name join or Layer 2 reset resource handling
- post-reset register polling
- backend reset-controller behavior

## Test Plan

Create:

- crates/regseq/tests/reset.rs

Tests to implement:

1. Verify `ResetLine` is publicly exported from `regseq`.
2. Verify `ResetLine` and `Option<ResetLine>` compile as driver field shapes.
3. Verify the accepted wrapper signatures compile:
   - `assert(&self) -> Result<(), RegseqError>`
   - `deassert(&self) -> Result<(), RegseqError>`
   - `toggle(&self, Duration) -> Result<(), RegseqError>`
4. Verify `toggle` uses the shared `Duration` facade type.

Test command: `cargo test -p regseq --test reset`

## Acceptance Criteria

- `ResetLine` exists as an explicit shared facade resource shell
- the accepted reset wrapper method signatures are present and publicly
  exported
- the task stays limited to Layer 1 facade declarations
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
