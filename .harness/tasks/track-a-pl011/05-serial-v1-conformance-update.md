# Task: track-a-pl011-05 - PL011 serial v1 conformance update

## Description

Bring the checked-in PL011 fixtures into conformance with the stable v1 Serial
device-class contract.

This task does not add a new optional serial feature family. It only updates
the existing PL011 fixtures so they satisfy the revised serial trait shape
before later richer serial work resumes.

## Input Context

Read before starting:

- `docs/discussions/device-class-serial/serial-device-class-spec.md`
- `docs/discussions/device-class-serial/serial-os-support-matrix.md`
- `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `tests/fixtures/pl011-spec-slice/src/driver.rs`
- `tests/e2e/tests/pl011_spec_slice.rs`

## Output Specification

Modify:

- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `tests/fixtures/pl011-spec-slice/src/driver.rs`

Modify only additional tests or fixture docs if required by the revised serial
trait conformance.

## Implementation Notes

Implement only the missing stable v1 Serial methods in the PL011 fixtures:

- `tx_empty`
- `flush_rx_fifo`
- `flush_tx_fifo`
- `send_break`

Use the new serial v1 spec as the authority:

- `tx_empty` must reflect full transmit completion, not just FIFO space
- FIFO flush hooks are explicit device-authored mechanisms, not compiler
  inference from register metadata
- `send_break` is an explicit hook and may be implemented with real register
  behavior or a documented no-op only if the fixture cannot yet justify the
  hardware path

Keep this task bounded:

- do not add `SerialConsole` unless required only for conformance to another
  already-landed test
- do not add flow-control, RS-485, or DMA behavior
- do not widen the current PL011 roadmap beyond trait conformance
- update both:
  - the richer `pl011-full-spec` fixture
  - the earlier `pl011-spec-slice` fixture
  so the serial-alignment batch does not leave one checked-in PL011 artifact
  compiling against the superseded six-method trait

## Test Plan

At minimum:

- real wrapper-based compile check of `tests/fixtures/pl011-full-spec`
- real wrapper-based compile check of `tests/fixtures/pl011-spec-slice`
- any affected e2e tests
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

## Acceptance Criteria

- the checked-in PL011 fixtures implement the stable v1 Serial trait
  completely
- both PL011 fixtures compile through the real `regseq-driver` wrapper path
- no new richer serial feature family is pulled in prematurely
- workspace tests and clippy pass
