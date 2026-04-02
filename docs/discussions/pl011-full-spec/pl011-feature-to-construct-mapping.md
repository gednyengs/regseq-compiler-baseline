# PL011 Feature To Construct Mapping

This note maps the feature inventory in
[pl011-feature-matrix.md](/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/discussions/pl011-full-spec/pl011-feature-matrix.md)
onto the accepted Regseq construct set.

The goal is to answer three questions:

1. Which PL011 features are already represented by the current accepted
   construct taxonomy?
2. Which of those features are already covered by the current polling-first
   PL011 slice?
3. Which missing features should be pulled into a reopened final Phase B PL011
   expansion batch before Phase C model work begins?

This is a planning document for the full PL011 target, not a lowering design.

## Status labels

- `covered-now`: represented in the current B-08/B-09 PL011 slice
- `covered-taxonomy`: represented by accepted constructs, but not yet present in
  the current PL011 slice
- `needs-phase-b-expansion`: should be added before Phase C if we want the
  declaration model to reflect the fuller PL011 baseline
- `later-extension`: represented by accepted constructs or plausible future
  constructs, but does not need to block the final Phase B expansion

## Core PL011 baseline

These are the features evidenced across Linux, FreeBSD, and Zephyr in
[pl011-feature-matrix.md](/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/discussions/pl011-full-spec/pl011-feature-matrix.md).

| PL011 feature | Primary Regseq construct(s) | Current status | Notes |
| --- | --- | --- | --- |
| Polled TX/RX data path | Register Access, Serial/UART | `covered-now` | Present in the current polling slice through `Serial` plus MMIO register access. |
| Interrupt-driven TX/RX data path | Device Interrupt Setup and Handling, Register Access, Serial/UART | `needs-phase-b-expansion` | The accepted interrupt surface exists, but the real PL011 slice does not yet declare IRQ resources or ISR behavior. |
| Baud-rate programming | Register Access, Serial/UART, Peripheral Clock Management | `covered-now` | Present in the current slice, though clock-derived calculation is still simplified. |
| Line-format configuration | Register Access, Serial/UART | `covered-now` | Present through `LCR_H` programming. |
| FIFO control | Register Access, Serial/UART | `covered-now` | FIFO enable exists now; richer trigger control is not yet modeled in the slice. |
| UART enable/disable sequencing | Register Access, Driver Lifecycle, Serial/UART | `covered-now` | Present through `CR` writes in helper/callback paths. |
| RX-path error handling | Register Access, Serial/UART | `needs-phase-b-expansion` | Accepted taxonomy can express it, but the current slice does not yet model RX error inspection/propagation. |
| Interrupt mask lifecycle | Device Interrupt Setup and Handling, Register Access | `needs-phase-b-expansion` | Requires `IMSC` / status / clear-path register handling and a real interrupt path in the slice. |
| Variant / revision handling | Firmware Property Access, Bus Resource Management, compiler-managed fallback logic | `needs-phase-b-expansion` | This is part of the accepted PL011 baseline, but currently absent from the real slice. It may be partly compiler-managed and partly property/bus-driven. |

## Maximal feature set evidenced in the corpus

These features appear in at least one OS corpus driver and therefore define the
upper envelope for a fuller unified PL011 spec.

| PL011 feature | Primary Regseq construct(s) | Current status | Notes |
| --- | --- | --- | --- |
| Clock integration / clock-derived baud logic | Peripheral Clock Management | `covered-taxonomy` | The slice declares optional `Clock`, but does not yet use it as the primary baud source. |
| DMA TX | DMA Management | `covered-taxonomy` | Accepted in the construct set, but not yet in the PL011 slice. |
| DMA RX | DMA Management | `covered-taxonomy` | Same as TX DMA. |
| Hardware RTS/CTS flow control | Serial/UART, Register Access | `needs-phase-b-expansion` | This materially affects the fuller serial surface for PL011. |
| Modem status / modem control lines | Serial/UART, Register Access | `later-extension` | Important for Linux-rich PL011, but likely not the best first expansion target unless the paper target explicitly needs modem control. |
| RS-485 half-duplex support | Serial/UART | `later-extension` | Strong Linux-only feature; likely not needed before Phase C unless the full-spec target explicitly includes RS-485. |
| Break generation / break control | Serial/UART, Register Access | `later-extension` | Useful feature, but lower priority than IRQ, PM, and DMA. |
| Loopback exposure | Serial/UART, Register Access | `later-extension` | Mostly diagnostic/control-path surface. |
| IrDA / SIR mode | Serial/UART, Register Access | `later-extension` | Hardware exists, but current corpus evidence is weak/partial. |
| Pinctrl integration | Pin Controller and GPIO Configuration | `covered-taxonomy` | Optional resource is already present in the slice, but not yet used in fuller lifecycle/PM paths. |
| Reset-controller integration | Hardware Reset Sequencing and Post-Reset Polling | `covered-taxonomy` | Optional resource is already present in the slice, but no real reset sequencing yet. |
| Suspend/resume / PM-aware state preservation | Device Power State Management, Peripheral Clock Management, PinCtrl, Interrupt, Register Access | `covered-taxonomy` | Accepted constructs exist, but the slice does not yet use `#[rseq::pm]` or `PowerManagement`. |
| Console / early-console special paths | SerialConsole, Diagnostic Logging, compiler-managed console registration | `covered-taxonomy` | Accepted surface exists, but the slice currently omits `SerialConsole`. |
| Board / SoC-specific hooks or quirks | Firmware Property Access, Bus Resource Management, compiler-managed quirk handling | `covered-taxonomy` | The accepted baseline clearly leaves room for quirk handling, but the slice does not yet exercise it. |

## What this implies for a final Phase B expansion

If the goal is to avoid reshaping the declaration model later, the best Phase B
expansion candidates are the features that:

- materially change the declaration-model shape
- are already accepted in the construct taxonomy
- are clearly part of the fuller PL011 baseline

The strongest candidates are:

1. Interrupt-driven PL011 behavior
   - interrupt resource declaration
   - ISR binding
   - interrupt mask / status / clear register participation
   - this is the biggest missing pressure point in the current slice

2. PM-aware PL011 behavior
   - `#[rseq::pm(...)]`
   - `PowerManagement`
   - clock / pinctrl / interrupt interaction pressure
   - this exercises cross-construct lifecycle structure that the current slice
     does not stress

3. Real optional resource participation
   - clock-based baud logic
   - reset sequencing
   - pinctrl default/sleep application if the accepted PL011 baseline requires it
   - these are already partly declared in the slice and would make the slice
     much more representative if actually used

4. RX error handling
   - needed for a credible interrupt-capable serial path
   - likely interacts with `RxChar` / receive-data representation questions

5. DMA, if and only if the project wants the declaration model to account for
   DMA before E1000
   - DMA is clearly part of the maximal PL011 feature set
   - but it may substantially widen the remaining Phase B expansion

## Recommended split: mandatory vs optional full-PL011 target

### Mandatory before Phase C

These features are the strongest candidates to add before declaration-model
normalization:

- interrupt-driven TX/RX path
- interrupt mask lifecycle
- PM gate and `PowerManagement`
- real clock/reset/pinctrl participation where already accepted
- RX error handling
- variant/revision handling, at least in the authored/frontend-visible form

### Optional but desirable

These features improve the full PL011 target, but are not as critical for
declaration-model pressure:

- `SerialConsole`
- DMA TX/RX
- hardware flow control
- board/SoC-specific quirks if they can be modeled cleanly

### Likely later extensions

These are real PL011 features, but they do not need to block Phase C unless the
paper target explicitly commits to them:

- modem-control/status lines
- RS-485
- break control
- loopback exposure
- IrDA / SIR mode

## Bottom line

The feature matrix reinforces that the current polling-first slice is a good
first specimen, but not a strong enough final frontend target if the goal is to
shape the declaration model once and avoid revisiting it under E1000 pressure.

The best next move is:

1. define the intended **full PL011 target** in terms of mandatory and optional
   features
2. reopen Phase B with a final PL011 expansion batch for the highest-value
   missing constructs
3. only then begin Phase C normalized declaration-model work
