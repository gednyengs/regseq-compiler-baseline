# Task: regseq-facade-12 - Clock wrapper surface

## Description

Implement the shared typed `Clock` resource surface for the accepted PL011
frontend slice. This task owns only the facade-side wrapper declarations and
public exports; compiler-managed acquisition and Layer 2 clock binding remain
out of scope.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- crates/regseq/src/error.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/peripheral-clock-management.md

## Output Specification

Create:

- crates/regseq/src/clock.rs
- crates/regseq/tests/clock.rs

Modify:

- crates/regseq/src/lib.rs

## Implementation Notes

Add the stable typed resource shell and wrapper methods:

```rust
pub struct Clock {
    // opaque resource shell
}

impl Clock {
    pub fn enable(&self) -> Result<(), RegseqError> { unimplemented!() }
    pub fn disable(&self) { unimplemented!() }
    pub fn get_rate(&self) -> u32 { unimplemented!() }
}
```

Phase-B baseline:

- `Clock` is an opaque typed resource carrier
- `Clock` and `Option<Clock>` are the accepted Layer 1 declaration shapes
- wrapper receivers use `&self`
- `enable` is fallible on the stable facade surface
- `disable` and `get_rate` are infallible on the stable facade surface
- do not add trait impls, resource acquisition behavior, or Layer 2 binding
  logic in this task

Do not include:

- compiler-managed `acquire_clock`
- default-frequency fallback handling
- `#[rseq::clock(...)]` parsing
- backend clock-control behavior

## Test Plan

Create:

- crates/regseq/tests/clock.rs

Tests to implement:

1. Verify `Clock` is publicly exported from `regseq`.
2. Verify `Clock` and `Option<Clock>` compile as driver field shapes.
3. Verify the accepted wrapper signatures compile:
   - `enable(&self) -> Result<(), RegseqError>`
   - `disable(&self)`
   - `get_rate(&self) -> u32`
4. Verify the surface stays opaque and does not require runtime support to be
   usable in type positions.

Test command: `cargo test -p regseq --test clock`

## Acceptance Criteria

- `Clock` exists as an explicit shared facade resource shell
- the accepted wrapper method signatures are present and publicly exported
- the task stays limited to Layer 1 facade declarations
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
