# PL011 Feature To Construct Mapping

This note maps the feature inventory in
[pl011-feature-matrix.md](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/pl011-full-spec/pl011-feature-matrix.md)
onto the accepted Regseq construct set.

The goal is to answer three questions in light of the current concrete
`pl011-full-spec` fixture:

1. Which PL011 features are already represented by the current accepted
   construct taxonomy?
2. Which of those features are already covered by the concrete target fixture?
3. Which features remain intentionally deferred from that target fixture?

This is a planning document for the full PL011 target, not a lowering design.

## Status labels

- `covered-now`: represented in the concrete target fixture
- `covered-taxonomy`: represented by accepted constructs, but not yet present in
  the concrete target fixture
- `later-extension`: represented by accepted constructs or plausible future
  constructs, but intentionally outside the current target fixture

## Core PL011 baseline

These are the features evidenced across Linux, FreeBSD, and Zephyr in
[pl011-feature-matrix.md](/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/pl011-full-spec/pl011-feature-matrix.md).

| PL011 feature | Primary Regseq construct(s) | Current status | Notes |
| --- | --- | --- | --- |
| Polled TX/RX data path | Register Access, Serial/UART | `covered-now` | Present in the concrete target fixture through `Serial` plus MMIO register access. |
| Interrupt-driven TX/RX data path | Device Interrupt Setup and Handling, Register Access, Serial/UART | `covered-now` | Present in the concrete full-spec fixture through declared IRQ resource, interrupt-mask lifecycle, and `#[rseq::isr]` handling. |
| Baud-rate programming | Register Access, Serial/UART, Peripheral Clock Management | `covered-now` | Present in the concrete target fixture, including clock-derived calculation when an explicit clock is modeled. |
| Line-format configuration | Register Access, Serial/UART | `covered-now` | Present through `LCR_H` programming. |
| FIFO control | Register Access, Serial/UART | `covered-now` | FIFO enable/disable and interrupt-threshold configuration are present in the concrete target fixture. |
| UART enable/disable sequencing | Register Access, Driver Lifecycle, Serial/UART | `covered-now` | Present through `CR` writes in helper/callback paths. |
| RX-path error handling | Register Access, Serial/UART | `covered-now` | Present in the concrete full-spec fixture through `DR` status-bit inspection and `RxChar` error propagation. |
| Interrupt mask lifecycle | Device Interrupt Setup and Handling, Register Access | `covered-now` | Present in the concrete full-spec fixture through `IMSC` / `MIS` / `ICR` handling and staged TX interrupt control. |
| Variant / revision handling | Firmware Property Access, Bus Resource Management, compiler-managed fallback logic | `later-extension` | Intentionally deferred for the current single-variant ARM PL011 target. It should not shape the declaration model in this version unless a mandatory feature forces it back in. |

## Maximal feature set evidenced in the corpus

These features appear in at least one OS corpus driver and therefore define the
upper envelope for a fuller unified PL011 spec.

| PL011 feature | Primary Regseq construct(s) | Current status | Notes |
| --- | --- | --- | --- |
| Clock integration / clock-derived baud logic | Peripheral Clock Management | `covered-now` | The concrete full-spec fixture declares optional `Clock` and uses it as the primary baud source when present. |
| DMA TX | DMA Management | `covered-taxonomy` | Accepted in the construct set, but not yet in the PL011 slice. |
| DMA RX | DMA Management | `covered-taxonomy` | Same as TX DMA. |
| Hardware RTS/CTS flow control | Serial/UART, Register Access | `later-extension` | The current concrete full-spec fixture does not include flow-control handling. |
| Modem status / modem control lines | Serial/UART, Register Access | `later-extension` | Important for Linux-rich PL011, but likely not the best first expansion target unless the paper target explicitly needs modem control. |
| RS-485 half-duplex support | Serial/UART | `later-extension` | Strong Linux-only feature; likely not needed before Phase C unless the full-spec target explicitly includes RS-485. |
| Break generation / break control | Serial/UART, Register Access | `later-extension` | Useful feature, but lower priority than IRQ, PM, and DMA. |
| Loopback exposure | Serial/UART, Register Access | `later-extension` | Mostly diagnostic/control-path surface. |
| IrDA / SIR mode | Serial/UART, Register Access | `later-extension` | Hardware exists, but current corpus evidence is weak/partial. |
| Pinctrl integration | Pin Controller and GPIO Configuration | `covered-now` | The concrete full-spec fixture declares optional `PinCtrl` and applies default/sleep states in lifecycle paths. |
| Reset-controller integration | Hardware Reset Sequencing and Post-Reset Polling | `covered-now` | The concrete full-spec fixture declares optional `ResetLine` and toggles it during bring-up. |
| Suspend/resume / PM-aware state preservation | Device Power State Management, Peripheral Clock Management, PinCtrl, Interrupt, Register Access | `later-extension` | Intentionally deferred for the current concrete target fixture. |
| Console / early-console special paths | SerialConsole, Diagnostic Logging, compiler-managed console registration | `later-extension` | The current concrete full-spec fixture omits `SerialConsole`. |
| Board / SoC-specific hooks or quirks | Firmware Property Access, Bus Resource Management, compiler-managed quirk handling | `covered-taxonomy` | The accepted baseline clearly leaves room for quirk handling, but the slice does not yet exercise it. |

## What this implies for the current full-spec target

The concrete full-spec fixture already absorbed several of the high-value
features that were previously discussed as pre-Phase-C expansion candidates.
What remains intentionally outside the current target should therefore be read
as target-boundary guidance for lowering and later phases, not as an immediate
Phase B backlog.

The remaining high-value gaps are therefore mostly in the deferred/optional
space rather than in the core full-spec fixture itself.

## Recommended split: current target vs later extension

### Captured in the current target fixture

- interrupt-driven TX/RX path
- interrupt mask lifecycle
- real clock/reset/pinctrl participation where already accepted
- RX error handling
- break control and FIFO flush hooks

### Deferred from the current target fixture

- `SerialConsole`
- DMA TX/RX
- hardware flow control
- PM gate / `PowerManagement`
- board/SoC-specific quirks and variant handling

### Likely later extensions

These are real PL011 features, but they do not need to block Phase C unless the
paper target explicitly commits to them:

- modem-control/status lines
- RS-485
- break control
- loopback exposure
- IrDA / SIR mode

## Bottom line

The concrete `pl011-full-spec` fixture already reflects a substantially richer
PL011 target than the old polling-first slice. This document should therefore
be used to understand what is already represented in the target fixture and
what remains intentionally deferred, rather than to imply that the same
features are still missing from the live target.
