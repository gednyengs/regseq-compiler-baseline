# Task: regseq-facade-10 - Serial trait surfaces

## Description

Implement the accepted serial device-class callback-family surfaces in the
public facade. This task owns `Serial`, the supporting `SerialConfig` and
`RxChar` placeholder types needed for the stable signatures, and the optional
`SerialConsole` companion trait.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- crates/regseq/src/error.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/serial-uart-management.md
- docs/discussions/regseq-error/regseq-error-spec.md

## Output Specification

Create:

- crates/regseq/src/serial.rs
- crates/regseq/tests/serial.rs

Modify:

- crates/regseq/src/lib.rs

## Implementation Notes

Add the accepted serial callback contracts:

```rust
pub struct SerialConfig {
    _private: (),
}

pub struct RxChar {
    _private: (),
}

pub trait Serial {
    fn port_open(&mut self) -> Result<(), RegseqError>;
    fn port_close(&mut self);
    fn configure(&mut self, config: &SerialConfig) -> Result<(), RegseqError>;
    fn tx_byte(&mut self, byte: u8);
    fn rx_char(&mut self) -> Option<RxChar>;
    fn tx_ready(&mut self) -> bool;
}

pub trait SerialConsole {
    fn console_write(&mut self, text: &str);
}
```

Phase-B baseline:

- keep the accepted trait family names `Serial` and `SerialConsole`
- `SerialConsole` is optional, but the facade should still expose it now
- `SerialConfig` and `RxChar` are minimal public support types for signature
  stability; keep them intentionally opaque for now
- `SerialConfig` and `RxChar` are intentionally minimal for this batch; their
  internal fields and construction APIs can wait for later Layer 2 serial
  property and real-PL011 work
- preserve the callback return-shape distinctions exactly:
  - `Result<(), RegseqError>`
  - `()`
  - `Option<RxChar>`
  - `bool`

Do not include:

- `push_rx_data` intrinsic work
- Layer 2 serial property types
- backend behavior or subsystem registration glue
- extraction or validation logic

## Test Plan

Create:

- crates/regseq/tests/serial.rs

Tests to implement:

1. Verify `Serial`, `SerialConsole`, `SerialConfig`, and `RxChar` are publicly
   exported from `regseq`.
2. Verify a small type can implement `Serial` with the accepted method
   signatures.
3. Verify `SerialConsole` can be implemented independently as an optional
   companion trait.
4. Verify the distinct return shapes compile as accepted.

Test command: `cargo test -p regseq --test serial`

## Acceptance Criteria

- `Serial` and `SerialConsole` exist as public facade traits
- `SerialConfig` and `RxChar` stabilize the accepted method signatures
- the accepted return-shape distinctions are preserved
- the task stays limited to facade trait/type shape
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
