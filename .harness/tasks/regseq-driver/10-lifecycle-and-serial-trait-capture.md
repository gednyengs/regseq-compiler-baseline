# Task: regseq-driver-10 - Lifecycle and serial trait capture

## Description

Implement the narrow compiler-facing extraction scaffolding for the accepted
`Lifecycle`, `Serial`, and optional `SerialConsole` trait surfaces. This task
should prove `regseq-driver` can recover callback-family implementations
without introducing callback-body analysis or lowering behavior.

## Input Context

Read before starting:

- crates/regseq-driver/src/extract/callback_context.rs
- crates/regseq-driver/src/extract/mod.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/driver-lifecycle.md
- docs/compiler-collateral/constructs/serial-uart-management.md

## Output Specification

Create:

- crates/regseq-driver/src/extract/traits.rs
- crates/regseq-driver/tests/traits.rs

Modify:

- crates/regseq-driver/src/extract/mod.rs

## Implementation Notes

Recognize and extract only the accepted trait callback-family forms:

- `impl Lifecycle for DriverType { ... }`
- `impl Serial for DriverType { ... }`
- optional `impl SerialConsole for DriverType { ... }`

The extraction output should remain internal to `regseq-driver`. A narrow
record such as `TraitSurface`, `LifecycleImpl`, `SerialImpl`, and
`SerialConsoleImpl` is appropriate.

Required capture details:

- driver type name implementing each trait
- which accepted methods are present
- callback context classification:
  - `Lifecycle` -> `F`
  - `Serial` / `SerialConsole` -> `S`
- `SerialConsole` should be classified as `S` by extension from the serial
  subsystem's shared serialized callback scope

Required structural validations for this task:

- `Lifecycle` impls must provide both `init` and `deinit` with the accepted
  signatures
- `Serial` impls must provide the accepted six methods with the accepted
  signatures
- `SerialConsole` is optional but, when present, must provide
  `console_write(&mut self, text: &str)`
- reject wrong receiver types, wrong method names, and wrong return shapes
  deterministically

Keep this task bounded:

- do not consume `#[rseq::device_class(serial)]` yet
- do not enforce cross-trait requirements like “serial drivers must implement
  `Serial`” here; that join remains for later work
- do not require the implementing type to be the `#[rseq::driver]` type yet;
  that join remains for later work
- do not analyze callback bodies
- do not lower callback families

Known deferred validations for later work:

- every driver must implement `Lifecycle`
- `device_class = serial` requires a `Serial` impl
- device-class / trait-presence consistency checks
- trait impl / `#[rseq::driver]` type joining

## Test Plan

Create:

- crates/regseq-driver/tests/traits.rs

Tests to implement:

1. Verify `Lifecycle` impls with the accepted method set are recognized.
2. Verify `Serial` impls with the accepted method set are recognized.
3. Verify optional `SerialConsole` impls are recognized independently.
4. Verify wrong signatures are rejected deterministically for both trait
   families.
5. Verify extracted callback contexts are `F` for `Lifecycle` and `S` for
   `Serial` / `SerialConsole`.
6. Verify per-method Serial return-shape distinctions are enforced:
   reject cross-shape swaps such as `rx_char` returning `bool` or `tx_ready`
   returning `Option<RxChar>`.

Test command: `cargo test -p regseq-driver --test traits`

## Acceptance Criteria

- `regseq-driver` can reach the accepted lifecycle and serial trait impl forms
- the accepted callback contexts are preserved
- the task stays limited to trait-surface capture and structural validation
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
