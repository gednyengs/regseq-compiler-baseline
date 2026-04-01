# Task: track-a-pl011-03 - Polling driver slice and Layer 2 property fixture

## Description

Complete the first real PL011 frontend specimen by adding the polling-first
 driver declarations, property-backed Layer 2 fixture usage, and callback
 implementations for the accepted narrow slice.

This is the task that turns the scaffold plus register surface into a real
 PL011-facing Regseq driver fragment.

## Input Context

Read before starting:

- docs/plans/asplos-implementation-plan.md
- docs/discussions/pl011-spec-slice/pl011-spec-slice-discussion.md
- docs/discussions/layer-2-format/layer-2-format-discussion.md
- .harness/tasks/regseq-facade/09-lifecycle-trait-surface.md
- .harness/tasks/regseq-facade/10-serial-trait-surfaces.md
- .harness/tasks/regseq-facade/12-clock-wrapper-surface.md
- .harness/tasks/regseq-facade/13-resetline-wrapper-surface.md
- .harness/tasks/regseq-facade/14-pinctrl-wrapper-surface.md
- .harness/tasks/regseq-facade/17-firmware-property-declaration-surface.md
- .harness/tasks/regseq-driver/10-lifecycle-and-serial-trait-capture.md
- .harness/tasks/regseq-driver/12-clock-resource-and-wrapper-capture.md
- .harness/tasks/regseq-driver/13-resetline-resource-and-wrapper-capture.md
- .harness/tasks/regseq-driver/14-pinctrl-resource-and-wrapper-capture.md
- .harness/tasks/regseq-driver/17-firmware-property-declaration-capture.md
- tests/fixtures/pl011-spec-slice/platform.toml
- tests/fixtures/pl011-spec-slice/src/lib.rs
- tests/fixtures/pl011-spec-slice/src/registers.rs
- tests/e2e/tests/pl011_spec_slice.rs

## Output Specification

Create or modify:

- tests/fixtures/pl011-spec-slice/src/lib.rs
- tests/fixtures/pl011-spec-slice/src/driver.rs
- tests/fixtures/pl011-spec-slice/platform.toml
- tests/e2e/tests/pl011_spec_slice.rs

## Implementation Notes

This task should realize the **polling-first PL011 bring-up slice** from the
 discussion note.

Required driver identity/declarations:

- `#[rseq::driver(name = "pl011-spec-slice", compatible = ["arm,pl011"])]`
- `#[rseq::device_class(serial)]`
- a real driver type, for example `Pl011SpecSlice`

Required fields:

- `#[rseq::mmio(id = "uart_regs")] regs: Mmio<Pl011Registers>`
- `#[rseq::property(id = "reg-io-width", default = 4)] reg_io_width: u32`
- `#[rseq::property(id = "current-speed")] current_speed: Option<u32>`

Optional-but-recommended resource declarations for this first slice:

- `#[rseq::clock(id = "uart_clk")] clock: Option<Clock>`
- `reset: Option<ResetLine>`
- `#[rseq::pinctrl(id = "uart_pins")] pinctrl: Option<PinCtrl>`

If included, these optional resources should stay frontend-only and should not
 force unsupported Layer 2 join work.

If `PinCtrl` is included, define a local zero-sized marker such as
 `DefaultState` implementing `PinState`.

Required callback-family implementations:

- `Lifecycle`
  - `init`
  - `deinit`
- `Serial`
  - `port_open`
  - `port_close`
  - `configure`
  - `tx_byte`
  - `rx_char`
  - `tx_ready`

Required authored behavior for the slice:

- real UART configuration writes using the accepted register wrappers
- a believable baud-rate programming sequence using `IBRD` / `FBRD`
- FIFO / line-control setup through `LCR_H`
- UART enable / disable through `CR`
- polling-based TX readiness through `FR::TXFF`
- polling-based RX empty check through `FR::RXFE`
- data-register access through `DR`

Use these canonical Layer 2 property values in the checked-in fixture unless
there is a strong documented reason to differ:

- `reg-io-width = 4`
- `current-speed = 115200`

Important realism rule:

- `tx_byte` should not be a pure no-op; it should show a real authored data-path
  write shape over `DR`
- `rx_char` may remain minimal if the current opaque `RxChar` facade surface
  does not yet let the fixture materialize a real value, but it should still
  exercise a believable polling/read path over `FR` and `DR`

Current facade-opacity guidance for this task:

- `RxChar` has no public constructor today, so `rx_char` may return `None`
  after exercising the polling/read path
- `SerialConfig` is currently opaque, so `configure` may use plausible
  hardcoded bring-up defaults rather than reading fields from `config`
- if baud-rate programming needs a clock input, use a plausible documented
  assumption such as a 48 MHz UART clock unless an optional `Clock` resource is
  present and you intentionally call `get_rate()`

Guidance for `port_open` / `port_close` in this first slice:

- `port_open` may delegate to the same enable/configuration path used by `init`
- `port_close` may delegate to the same disable path used by `deinit`
- the important requirement is that these methods look coherent for a polling
  bring-up specimen; they do not need final subsystem-accurate semantics yet

Layer 2 boundary for this task:

- keep `platform.toml` inside the B-07-supported subset
- use only the required header plus `[properties]`
- do not add full Layer 2 sections for mmio / interrupt / clock / reset /
  pinctrl joins yet

What to leave out of this first slice:

- ISR handlers
- interrupt-driven RX/TX
- `PowerManagement`
- `SerialConsole`
- DMA
- modem-control features
- full console integration
- raw-artifact snapshot/closure work

The e2e test should now prove that the real fixture:

- compiles through the wrapper path
- still reaches `after_analysis`
- exposes the expected driver identity
- exposes register-surface evidence for the PL011 subset
- contains real wrapper call-site evidence from the callback bodies

Use the current debug extraction output as the assertion source where practical.
 This task does not need to introduce final raw-artifact snapshots.

The e2e assertions should check for concrete evidence patterns, such as:

- the expected driver identity `pl011-spec-slice`
- register-access extraction includes wrapper call-site evidence from:
  - `init`
  - `configure`
  - `tx_byte`
  - `rx_char`
  - `tx_ready`
- the register block name `Pl011Registers` and MMIO field `regs` remain
  visible in the extraction output

Do not widen this task by wiring new extraction families into the main
`callbacks.rs` debug stream unless that becomes strictly necessary. The e2e
assertions may rely on the extraction evidence that is already emitted today,
primarily driver identity and register-access facts.

## Test Plan

Tests/commands to support:

1. Wrapper compile of the real PL011 slice fixture succeeds
2. E2E assertions cover driver identity plus representative extraction evidence
   from the polling slice callback bodies
3. The checked-in `platform.toml` remains valid under the current B-07 parser
4. Workspace tests and clippy remain green

Suggested commands:

- `cargo test -p regseq-e2e --test pl011_spec_slice -- --nocapture`
- `cargo test -p regseq-model --test platform_parser --test platform_properties`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

## Acceptance Criteria

- the checked-in PL011 fixture now looks like a real polling-first Regseq
  driver fragment rather than a synthetic placeholder
- the fixture implements `Lifecycle` and `Serial` for the accepted narrow slice
- the fixture uses the required PL011 properties in `platform.toml`
- the fixture remains within current Phase B frontend and Layer 2 boundaries
- the e2e test shows successful wrapper compile and representative extraction
  evidence from the real fixture
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
