# PL011 Full Spec Status

## REFRAMING (2026-04-20)

Status tracking has been realigned against the requirements defined
in `pl011-spec-requirements.md`. The roadmap items 1-10 below remain
correct as a fixture-growth history, but the current authoritative
question is: which `REQ-*` entries in
`pl011-spec-requirements.md` are `captured` vs `pending` vs
`deferred`?

Status-to-requirements mapping summary (see requirements doc for
IDs):

- **captured** (Stage 1 / F+I internal): REQ-TX-*, REQ-RX-*,
  REQ-CFG-*, REQ-FIFO-01/02/03, REQ-LIFE-*, REQ-IRQ-01/02/03/04,
  REQ-BRK-01, REQ-RES-CLK-01/02, REQ-RES-RST-01, REQ-RES-PIN-01,
  REQ-RES-IRQ-01, REQ-RES-MMIO-01, REQ-ID-01/02, REQ-CORR-*,
  REQ-BC-*.
- **pending** (Stage 2 / P internal): REQ-PERF-BATCH-01,
  REQ-PERF-IFLS-01, REQ-PERF-DMA-01, REQ-PERF-DMA-02,
  REQ-PERF-LATENCY-01, REQ-FLOW-01, REQ-FLOW-02, REQ-INT-LIN-01
  (hot-path batching), REQ-INT-ZEP-01 (interrupt-driven API).
- **deferred** (not in current claim): REQ-DEF-*, REQ-INT-CONSOLE-*,
  REQ-INT-PM, REQ-INT-SYSFS-ADMIN.

Internal staging is a tactical choice for the compiler dev team
(F+I first to pipe-clean the compiler, then P including DMA). The
paper claim is the full F+I+P result.

---

# Historical content (below)

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
