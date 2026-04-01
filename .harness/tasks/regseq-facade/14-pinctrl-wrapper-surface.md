# Task: regseq-facade-14 - PinCtrl wrapper surface

## Description

Implement the shared typed `PinCtrl` resource surface and the `PinState` marker
contract for the accepted PL011 frontend slice. This task owns only the
facade-side wrapper declarations and public exports.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- crates/regseq/src/error.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/pin-controller-and-gpio-configuration.md

## Output Specification

Create:

- crates/regseq/src/pinctrl.rs
- crates/regseq/tests/pinctrl.rs

Modify:

- crates/regseq/src/lib.rs

## Implementation Notes

Add the stable typed resource shell, marker trait, and wrapper method:

```rust
pub struct PinCtrl {
    // opaque resource shell
}

pub trait PinState {}

impl PinCtrl {
    pub fn apply<S: PinState>(&self) -> Result<(), RegseqError> {
        unimplemented!()
    }
}
```

Phase-B baseline:

- `PinCtrl` is an opaque typed resource carrier
- `PinCtrl` and `Option<PinCtrl>` are the accepted Layer 1 declaration shapes
- `PinState` is only the compile-time marker contract in this batch
- `apply::<S>()` is the stable state-selection wrapper surface
- keep `PinState` open so tests and later generated state markers can
  implement it

Do not include:

- concrete built-in pin-state marker types
- Layer 2 `pin_states` inventories or validation
- compiler-managed `acquire_pinctrl`
- PM-generated pin-state transitions
- backend pinctrl behavior

## Test Plan

Create:

- crates/regseq/tests/pinctrl.rs

Tests to implement:

1. Verify `PinCtrl` and `PinState` are publicly exported from `regseq`.
2. Verify `PinCtrl` and `Option<PinCtrl>` compile as driver field shapes.
3. Verify a local marker type implementing `PinState` can be used with
   `PinCtrl::apply::<State>()`.
4. Verify the accepted wrapper signature compiles:
   - `apply<S: PinState>(&self) -> Result<(), RegseqError>`

Test command: `cargo test -p regseq --test pinctrl`

## Acceptance Criteria

- `PinCtrl` exists as an explicit shared facade resource shell
- `PinState` exists as the accepted compile-time marker contract
- the accepted `apply::<S>()` wrapper method signature is present
- the task stays limited to Layer 1 facade declarations
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
