# PL011 Full Spec Status

This note tracks the build-out of the concrete full-PL011
fixture at:

- [pl011-full-spec](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec)

It is intentionally lightweight. Its purpose is to prevent drift between the
target fixture and the surrounding discussion docs.

The methodology note remains useful historical context for how the fixture was
grown, but this status file should reflect the current target fixture as it
exists now.

## Scope boundary

- This version targets a single concrete ARM PL011 configuration.
- Family-wide variant/revision/quirk machinery is intentionally deferred.
- PM-aware behavior that only appears in deferred PL011 variants is also out of
  scope for this version.

## Status labels

- `captured`: represented in the fixture and compile-checked through the real
  `regseq-driver` wrapper path
- `pending`: on the planned roadmap, but not yet added
- `deferred`: intentionally out of scope for this version

## Current captured frontend surface

### Base declarations

- `captured` PL011 register descriptors in
  [registers.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/registers.rs)
- `captured` `Pl011Registers` register block in
  [registers.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/registers.rs)
- `captured` driver identity for `Pl011FullSpec` in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` `Mmio<Pl011Registers>` field in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` `current-speed` property with default `115200` in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` `fifo-size` property with default `32` in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` `Interrupt` resource in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

### Polling serial bring-up

- `captured` `Lifecycle` implementation in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` polling `Serial` implementation in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` helper sequencing for:
  - UART disable/enable
  - FIFO disable
  - baud-divisor programming
  - `LCR_H` commit after divisor programming
  - interrupt mask/clear hygiene
  - explicit ordered sequencing for the divisor/latch/enable sub-sequence
  in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` polling TX via `FR.TXFF` and `DR` in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` polling RX emptiness check via `FR.RXFE` and `DR` read in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

### Interrupt-driven and richer bring-up participation

- `captured` RX-path error inspection and `RxChar` error propagation in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` interrupt-driven service path via `#[rseq::isr]` in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` interrupt mask / status / clear register participation through
  `IMSC`, `MIS`, and `ICR` in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` optional clock participation in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` optional reset participation in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` optional pinctrl participation in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)
- `captured` richer serial hooks `flush_rx_fifo`, `flush_tx_fifo`, and
  `send_break` in
  [driver.rs](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

## Roadmap status

1. Base declarations
- `captured`

2. Polling serial bring-up
- `captured`

3. RX error handling
- `captured`

4. Interrupt-driven path
- `captured`

5. Clock participation
- `captured`

6. Reset participation
- `captured`

7. PinCtrl participation
- `captured`

8. PM-aware path
- `deferred`

9. Optional richer serial features
- `captured` in part
  - break control is captured
  - FIFO flush hooks are captured
  - flow control, modem-control/status, and `SerialConsole` remain outside the
    current target fixture

10. DMA
- `pending`

## Explicit deferrals

- `deferred` PM-aware path for variant-specific PL011 integrations
- `deferred` modem-control/status lines
- `deferred` RS-485
- `deferred` loopback
- `deferred` IrDA / SIR
- `deferred` family-wide variant/revision/quirk machinery

## Verification discipline

Each status change to `captured` should be backed by a successful compile check
of the fixture through the real compiler path, not just plain Cargo syntax
checking.
