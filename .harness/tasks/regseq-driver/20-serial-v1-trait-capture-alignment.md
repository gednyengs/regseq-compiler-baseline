# Task: regseq-driver-20 - Serial v1 trait capture alignment

## Description

Align `regseq-driver` trait extraction and structural validation with the
stable v1 Serial device-class contract.

This task updates the earlier lifecycle/serial trait-capture baseline so the
compiler validates and extracts the revised trait shape rather than the older
six-method contract.

## Input Context

Read before starting:

- `crates/regseq-driver/src/extract/traits.rs`
- `crates/regseq-driver/src/extract/mod.rs`
- `crates/regseq-driver/tests/traits.rs`
- `docs/discussions/device-class-serial/serial-device-class-spec.md`
- `docs/discussions/device-class-serial/serial-os-support-matrix.md`
- `.harness/tasks/regseq-driver/10-lifecycle-and-serial-trait-capture.md`

## Output Specification

Modify:

- `crates/regseq-driver/src/extract/traits.rs`
- `crates/regseq-driver/src/extract/mod.rs`
- `crates/regseq-driver/tests/traits.rs`

## Implementation Notes

Update structural recognition for:

- `impl Serial for DriverType`
- optional `impl SerialConsole for DriverType`

Required Serial method set now includes:

- `port_open`
- `port_close`
- `configure`
- `tx_byte`
- `rx_char`
- `tx_ready`
- `tx_empty`
- `flush_rx_fifo`
- `flush_tx_fifo`
- `send_break`

Required validation updates:

- enforce the revised method presence set
- enforce the revised method signatures
- preserve callback context classification:
  - `Serial` -> `S`
  - `SerialConsole` -> `S`
- enforce `SerialConsole::console_write(&mut self, bytes: &[u8])`
- the current signature matcher is string-oriented; expect to extend the
  parameter-shape matching logic so `&[u8]` can be recognized explicitly in HIR
  rather than weakening the validation contract

Keep this task bounded:

- do not analyze callback bodies
- do not lower callback families
- do not enforce cross-trait joins beyond the already established structural
  capture scope
- do not add `SerialFlowControl`, RS-485, or DMA-specific trait handling

## Test Plan

Update `crates/regseq-driver/tests/traits.rs` to verify:

1. `Serial` impls with the revised stable v1 method set are recognized.
2. Missing new methods are rejected deterministically.
3. Wrong signatures for `tx_empty`, FIFO flush hooks, or `send_break` are
   rejected deterministically.
4. `SerialConsole` now requires `console_write(&mut self, bytes: &[u8])`.
5. Callback contexts remain `S`.

Test commands:

- `cargo test -p regseq-driver --test traits`
- `cargo clippy -p regseq-driver --test traits -- -D warnings`

## Acceptance Criteria

- `regseq-driver` structurally captures the stable v1 serial trait surface
- the revised `SerialConsole` signature is enforced
- trait extraction tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
