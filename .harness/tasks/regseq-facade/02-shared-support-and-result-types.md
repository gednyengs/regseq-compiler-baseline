# Task: regseq-facade-02 - Shared support and result types

## Description

Implement the smallest shared public types needed before construct-local PL011
frontend work can proceed. This task owns the stable facade surface for:

- `RegseqError`
- `Duration`
- `IrqResult`

These types are shared prerequisites for later callback, interrupt, polling,
timer, and logging surfaces. This task must keep them intentionally small and
must not pull MMIO, synchronization wrappers, callback traits, or standalone
intrinsics into the facade yet.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- crates/regseq/README.md
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/driver-lifecycle.md
- docs/compiler-collateral/constructs/device-interrupt-setup-and-handling.md
- docs/compiler-collateral/constructs/timed-cpu-delay-and-sleep.md
- docs/discussions/regseq-error/regseq-error-spec.md

## Output Specification

Create:

- crates/regseq/src/error.rs
- crates/regseq/src/time.rs
- crates/regseq/src/irq.rs
- crates/regseq/tests/shared_foundation.rs

Modify:

- crates/regseq/src/lib.rs

## Implementation Notes

### `RegseqError`

Add a public shared error type:

- stable public name: `regseq::RegseqError`
- authored carrier shape for this phase: `core::result::Result<T, RegseqError>`
- mark the enum `#[non_exhaustive]`
- derive:
  - `Debug`
  - `Clone`
  - `Copy`
  - `PartialEq`
  - `Eq`
- do not implement `core::fmt::Display`
- do not implement `std::error::Error`

Use the current active baseline variant set from the RegseqError spec:

```rust
#[non_exhaustive]
pub enum RegseqError {
    InvalidParameter,
    OutOfMemory,
    HardwareError,
    Timeout,
    Unsupported,
    DeviceBusy,
    PermissionDenied,
    DeviceNotFound,
    GenericFailure,
}
```

Keep the implementation intentionally minimal. This task should establish the
shared callback error type and current Phase B baseline, not the deferred
intrinsic-local error strategy.

### `Duration`

Add the shared compile-time utility type as a small custom public surface:

```rust
pub struct Duration { /* private fields */ }

impl Duration {
    pub const fn from_us(us: u64) -> Self { /* ... */ }
    pub const fn from_ms(ms: u64) -> Self { /* ... */ }
    pub const fn from_s(s: u64) -> Self { /* ... */ }
}
```

Baseline shape for this phase:

- keep `Duration` as a small opaque facade support type
- derive:
  - `Debug`
  - `Clone`
  - `Copy`
  - `PartialEq`
  - `Eq`
- keep storage private
- expose the three stable constructors as `pub const fn`

This task should establish only the stable public constructor surface accepted
by the collateral:

- `Duration::from_us(u64)`
- `Duration::from_ms(u64)`
- `Duration::from_s(u64)`

Keep formatting helpers, arithmetic helpers, and delay APIs out of scope. Those
belong to later frontend batches.

### `IrqResult`

Add the stable IRQ dispatch result:

```rust
pub enum IrqResult {
    Handled,
    NotHandled,
    DeferWork,
}
```

This is a dispatch result for bound handlers, not an error type.

### Crate-root exports

Re-export the three surfaces from `crates/regseq/src/lib.rs` so later tasks can
refer to:

- `regseq::RegseqError`
- `regseq::Duration`
- `regseq::IrqResult`

Do not add any wrapper families, callback traits, or macro re-exports in this
task.

## Test Plan

Create:

- crates/regseq/tests/shared_foundation.rs

Tests to implement:

1. Import `RegseqError`, `Duration`, and `IrqResult` from `regseq` and verify
   the crate exports compile.
2. Verify `IrqResult` exposes the accepted variants `Handled`, `NotHandled`, and
   `DeferWork`.
3. Verify `RegseqError` exposes the current working variants:
   `InvalidParameter`, `OutOfMemory`, `HardwareError`, `Timeout`,
   `Unsupported`, `DeviceBusy`, `PermissionDenied`, `DeviceNotFound`, and
   `GenericFailure`.
4. Verify `RegseqError` supports the baseline derives needed by current work
   (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`) without requiring `Display` or
   `std::error::Error`.
5. Verify `Duration::from_us`, `Duration::from_ms`, and `Duration::from_s` are
   available through the public `regseq` surface.
6. Verify `Duration` supports the baseline derives needed by current work
   (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`) and that the three
   constructors are usable as `const fn`.

Test command: `cargo test -p regseq`

## Acceptance Criteria

- `regseq` exports `RegseqError`, `Duration`, and `IrqResult`
- the task stays limited to shared support and result types
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
