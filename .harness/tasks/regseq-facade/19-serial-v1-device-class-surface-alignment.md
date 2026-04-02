# Task: regseq-facade-19 - Serial v1 device-class surface alignment

## Description

Align the public serial device-class facade with the authoritative v1 Serial
device-class spec.

This task supersedes the earlier narrow six-method serial baseline where the
new v1 spec differs. The goal is to make `regseq` expose the stable v1 Serial
contract before later model or lowering work begins.

## Input Context

Read before starting:

- `crates/regseq/src/serial.rs`
- `crates/regseq/src/lib.rs`
- `crates/regseq/tests/serial.rs`
- `docs/discussions/device-class-serial/serial-device-class-spec.md`
- `docs/discussions/device-class-serial/serial-os-support-matrix.md`
- `.harness/tasks/regseq-facade/10-serial-trait-surfaces.md`

## Output Specification

Modify:

- `crates/regseq/src/serial.rs`
- `crates/regseq/src/lib.rs`
- `crates/regseq/tests/serial.rs`

## Implementation Notes

Align the facade to the stable v1 decisions:

- `Serial` remains the core trait.
- `SerialConsole` remains the optional companion trait.
- Add the missing stable v1 methods to `Serial`:
  - `tx_empty(&mut self) -> bool`
  - `flush_rx_fifo(&mut self)`
  - `flush_tx_fifo(&mut self)`
  - `send_break(&mut self, on: bool)`
- Change `SerialConsole::console_write` to:
  - `fn console_write(&mut self, bytes: &[u8]);`
- Keep the byte-primitive data-path model.
- Keep `port_close` infallible.

Supporting types:

- Reconcile the current `ErrorKind` naming with the stable v1 `SerialErrors`
  terminology.
- A compatibility bridge such as:
  - renaming the public type, or
  - making `SerialErrors` the public name and keeping `ErrorKind` as an alias
  is acceptable.
- Do not widen the public error set beyond:
  - overrun
  - parity
  - framing
  - break
- `noise` remains out of the stable v1 public surface.
- Align `RxChar` with the stable v1 shape:
  - rename the stored error carrier from `error` to `errors`, or provide an
    equally clear compatibility bridge if an immediate field rename would be
    too disruptive
  - ensure the public API clearly exposes `SerialErrors` semantics rather than
    the older singular `ErrorKind` terminology

`SerialConfig`:

- Make only the bounded changes needed to align with the stable v1 spec.
- The target v1 shape includes these supporting enums, even if rollout is
  staged:
  - `DataBits`
  - `Parity`
  - `StopBits`
  - `FlowControl`
- If fully concretizing all `SerialConfig` fields would create unnecessary
  churn, a staged approach is acceptable as long as:
  - the public type clearly moves toward the v1 shape
  - the chosen fields/enums are documented in code comments or tests
- Do not reopen broader serial taxonomy in this task.

Do not include:

- `SerialFlowControl` trait
- `SerialModemControl` trait
- RS-485 surfaces
- serial-specific DMA traits

## Test Plan

Update `crates/regseq/tests/serial.rs` to verify:

1. `Serial` now includes the stable v1 method set.
2. `SerialConsole` compiles with `console_write(&[u8])`.
3. A small type can implement the revised `Serial` trait.
4. `RxChar` carries the stable v1 serial error vocabulary.
5. The chosen `SerialErrors` / compatibility naming strategy is publicly usable.
6. The target `SerialConfig` support enums are publicly available or their
   staged status is explicitly documented and tested.

Test commands:

- `cargo test -p regseq --test serial`
- `cargo clippy -p regseq --test serial -- -D warnings`

## Acceptance Criteria

- the public serial facade matches the stable v1 core trait shape
- `SerialConsole` uses the byte-slice console signature
- the public error carrier aligns with the stable v1 `SerialErrors` model
- the public `RxChar` surface reflects the stable v1 plural error semantics
- the target `SerialConfig` support-enum shape is explicit enough for later
  compiler and fixture work
- facade tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
