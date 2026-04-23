# PL011 Windows Stock Driver Analysis

This document is the work product defined by
`docs/discussions/pl011-full-spec/windows-driver-analysis-task-brief.md`.
It describes the functional and architectural capabilities of the ARM
PrimeCell PL011 UART as stated in its datasheet, and records which of
those capabilities the Windows stock driver exercises, and how. The
analysis is partitioned along four dimensions (Functional, Integration,
Performance, Correctness), each with a Variability subsection that
records places where the driver branches on PrimeCell revision,
integration variant, or platform configuration.

## 0. Inputs and Reading Guide

### 0.1 Inputs

The analysis below draws exclusively on:

1. The condensed PL011 datasheet at
   `docs/discussions/pl011-full-spec/pl011-minimized-datasheet.md` and
   the requirements extraction at
   `docs/discussions/pl011-full-spec/pl011-spec-requirements.md`.
   Datasheet citations use `pl011-minimized-datasheet.md:<line>` or
   `pl011-spec-requirements.md:<line>`.
2. The Windows stock PL011 driver source in
   `reference-drivers/pl011/windows/`, a WDF/KMDF SerCx2 Serial
   Controller Driver (`SerPL011`) targeting the Raspberry Pi 2/3
   (BCM2836/BCM2837) PL011 instance. All files in that directory are
   in scope. Driver citations use `<file>:<line>` or
   `<file>:<line>-<line>`.

### 0.2 Reading guide

- Each top-level section answers one question (Functional,
  Integration, Performance, Correctness). A feature that is relevant
  under more than one question (for example, the RX FIFO) appears
  under each relevant section with the section-specific question
  asked. This is by design per the task brief.
- The Variability subsection at the end of each dimension lists the
  places where the driver branches on PL011 revision, integration
  variant, or platform/registry configuration, and points back to the
  feature being conditioned.
- Datasheet claims are cited inline; driver claims are cited inline.
  Any claim that cannot be grounded in these sources is either
  removed or marked `Hypothesis:`.
- The driver declines to use several device capabilities on this
  platform. In line with the task brief, these are recorded
  explicitly, and the distinction between "platform default does not
  expose or use this capability" and "driver has no implementation
  for this capability" is made where the source supports it.

## 1. Functional Analysis

Question: which functional capabilities of the PL011 device does the
Windows driver capture, and how does it capture them?

### 1.1 Register map coverage and MMIO indirection

Datasheet: The PL011 programmer's model exposes a fixed register file:
UARTDR at 0x000, UARTRSR/UARTECR at 0x004, UARTFR at 0x018, UARTILPR
at 0x020, UARTIBRD at 0x024, UARTFBRD at 0x028, UARTLCR_H at 0x02C,
UARTCR at 0x030, UARTIFLS at 0x034, UARTIMSC at 0x038, UARTRIS at
0x03C, UARTMIS at 0x040, UARTICR at 0x044, UARTDMACR at 0x048, and
Peripheral / PrimeCell ID registers at 0xFE0-0xFFC
(`pl011-minimized-datasheet.md:767-804`).

Driver: The driver declares these offsets as an enum `PL011_REG_FILE`
in `PL011hw.h:29-62`, including `UARTDR = 0x0000` at
`PL011hw.h:30`, `UARTRSR_ECR = 0x0004` at `PL011hw.h:31`, `UARTFR =
0x0018` at `PL011hw.h:33`, `UARTILPR = 0x0020` at `PL011hw.h:35`,
`UARTIBRD = 0x0024` at `PL011hw.h:36`, `UARTFBRD = 0x0028` at
`PL011hw.h:37`, `UARTLCR_H = 0x002C` at `PL011hw.h:38`, `UARTCR =
0x0030` at `PL011hw.h:39`, `UARTIFLS = 0x0034` at `PL011hw.h:40`,
`UARTIMSC = 0x0038` at `PL011hw.h:41`, `UARTRIS = 0x003C` at
`PL011hw.h:42`, `UARTMIS = 0x0040` at `PL011hw.h:43`, `UARTICR =
0x0044` at `PL011hw.h:44`, `UARTDMACR = 0x0048` at `PL011hw.h:45`,
the test-interface registers UARTITCR/UARTITIP/UARTITOP/UARTTDR at
`PL011hw.h:47-50`, the Peripheral ID registers at `PL011hw.h:52-55`,
the PrimeCell ID registers at `PL011hw.h:56-59`, and the total
register-file span `REG_FILE_SIZE = 0x1000` at `PL011hw.h:61` used
to validate the ACPI memory resource at `PL011device.cpp:728`.

Register access goes through forced-inline helpers in
`PL011hw.h:364-464`: `PL011HwReadRegisterUlong` wraps
`READ_REGISTER_ULONG`, `PL011HwReadRegisterUlongNoFence` wraps
`READ_REGISTER_NOFENCE_ULONG`, `PL011HwWriteRegisterUlong` wraps
`WRITE_REGISTER_ULONG`, and `PL011HwWriteRegisterUlongNoFence` wraps
`WRITE_REGISTER_NOFENCE_ULONG`. The two write helpers are guarded by
`#if IS_DONT_CHANGE_HW` so that a build variant can be produced in
which writes are no-ops while reads still occur
(`PL011hw.h:419-428`, `PL011hw.h:454-463`).

Discrepancy / note: The register-file enum contains offsets for
several registers the driver does not actually program, including
UARTILPR (IrDA low-power counter) and UARTDMACR (DMA control). Their
presence in the enum reflects datasheet coverage, not driver use.

The Peripheral ID and PrimeCell ID registers (0xFE0-0xFFC) are
declared in the enum but are not read by the driver; it does not
discover PrimeCell revision from the device. All revision-dependent
branches are driven externally (ACPI resources and registry
parameters; see Section 1.15).

### 1.2 Port enable and the transmit / receive enable bits

Datasheet: UARTCR has UARTEN at bit 0, TXE at bit 8, RXE at bit 9.
To enable transmission, both TXE and UARTEN must be set; likewise
RXE and UARTEN for reception. Control-register reprogramming
requires the UART to be disabled, current character completion
awaited, FIFO flushed by clearing FEN, the control register
reprogrammed, and the UART re-enabled
(`pl011-minimized-datasheet.md:1047-1079`; see also
`pl011-spec-requirements.md:62-71` for the ordered-write rule).

Driver: UARTCR is driven through `PL011HwUartControl`
(`PL011hw.cpp:257-315`), which serializes access under a `KSPIN_LOCK`
device register lock (`PL011device.h:108`) and supports four update
modes: `BITMASK_SET`, `BITMASK_CLEAR`, `OVERWRITE`, and `QUERY`. The
write-back masks the assembled value against `UARTCR_ALL` at
`PL011hw.cpp:306`.

`PL011HwInitController` (`PL011hw.cpp:50-118`) performs a
disable-first sequence: at `PL011hw.cpp:59-64` it clears UARTCR
fully (`UARTCR_ALL`, `REG_UPDATE_MODE::BITMASK_CLEAR`), then masks
all interrupts at `PL011hw.cpp:69-74`, clears all pending interrupts
at `PL011hw.cpp:79-82`, clears RX errors at `PL011hw.cpp:87`,
programs FIFO thresholds at `PL011hw.cpp:92-96`, discovers supported
baud rates at `PL011hw.cpp:102`, and only then writes UARTCR to
`UARTCR_UARTEN` in `OVERWRITE` mode at `PL011hw.cpp:108-113`. The
TXE and RXE bits are left clear at init; they are set later on
SerCx2 file open (Section 2.7), not at `PrepareHardware`.
`PL011HwStopController` (`PL011hw.cpp:134-161`) mirrors the sequence
in reverse: it clears UARTCR fully and masks all interrupts.

Discrepancy / note: The comment at `PL011hw.cpp:105-106` states
explicitly that "RX and TX are enabled when the device is opened."
This is consistent with the datasheet rule that the UART may be
enabled before TX/RX enable is asserted.

### 1.3 Data register read and write (RX / TX FIFO entries)

Datasheet: Writes to UARTDR place a byte in the TX FIFO. Reads from
UARTDR return a 12-bit receive-FIFO entry: bits 7:0 are the data
byte, bits 11:8 are the per-character error flags OE, BE, PE, FE
(`pl011-minimized-datasheet.md:810-841`).

Driver: The TX path writes UARTDR through
`PL011HwWriteRegisterUlongNoFence` from `PL011tx.cpp` (the
FIFO-fill copy routine `PL011TxPioFifoCopy` declared at
`PL011tx.h:151`). The RX path reads UARTDR through
`PL011HwReadRegisterUlongNoFence` from `pl011rx.cpp`; the read is
cast to `UCHAR` at `pl011rx.cpp:525-526` before being stored into
the RX software buffer, which discards the top-four error bits of
the FIFO entry. Per-character error reporting therefore does not
come from the UARTDR read; it comes from the error-class interrupt
bits (see Section 1.4).

The `_NOFENCE_` variants are chosen specifically for FIFO traffic;
the helpers' comment at `PL011hw.h:378-380` and `PL011hw.h:437-439`
explicitly justifies the choice as "faster and can be used for
read-only processes like reading from FIFOs".

### 1.4 Receive errors: interrupt-driven reporting (UARTRSR / UARTECR, error interrupts)

Datasheet: UARTRSR provides framing (FE), parity (PE), break (BE),
and overrun (OE) error bits for the character most recently read
from UARTDR. A write to UARTECR clears all error bits
(`pl011-minimized-datasheet.md:842-863`). The datasheet also exposes
each error as an independent interrupt source in UARTRIS / UARTIMSC:
FEIS, PEIS, BEIS, OEIS (`pl011-minimized-datasheet.md:722-732`,
`pl011-minimized-datasheet.md:1096-1148`).

Driver: The driver does not read UARTRSR. It clears UARTRSR/ECR
once, during controller init, at `PL011hw.cpp:87` via
`PL011HwClearRxErros`. At runtime, error classes are surfaced
entirely through the interrupt-status register. `PL011pInterruptIsr`
(`PL011interrupt.cpp:247-328`) reads UARTRIS at
`PL011interrupt.cpp:256-258`, accumulates the snapshot into
`DevExtPtr->IntEventsForDpc` (`PL011interrupt.cpp:270-273`), and
acknowledges the processed bits by writing them back to UARTICR at
`PL011interrupt.cpp:322-325`. In the DPC, `PL011DeviceRecordErrors`
(`PL011device.cpp:370-401`) translates the error-class bits
(OEIM/FEIM/PEIM/BEIM) into `SERIAL_ERROR_OVERRUN` /
`SERIAL_ERROR_FRAMING` / `SERIAL_ERROR_PARITY` / `SERIAL_ERROR_BREAK`
flags stored with `InterlockedOr` at `PL011device.cpp:397-400`. The
break-interrupt bit (UARTRIS_BEIS) additionally feeds the
wait-event path in `PL011DeviceNotifyEvents` at
`PL011device.cpp:438-441` (see Section 2.7).

### 1.5 Flag register and busy-state queries

Datasheet: UARTFR exposes real-time status bits CTS/DSR/DCD/BUSY/
RXFE/TXFF/RXFF/TXFE/RI (`pl011-minimized-datasheet.md:864-882`).

Driver: UARTFR is consulted through small forced-inline helpers in
`PL011hw.h`: `PL011HwIsRxFifoEmpty` at `PL011hw.h:534-543` checks
bit 4 (RXFE), `PL011HwIsTxFifoEmpty` at `PL011hw.h:587-596` checks
bit 7 (TXFE), `PL011HwIsTxFifoFull` at `PL011hw.h:612-621` checks
bit 5 (TXFF), and `PL011HwIsTxBusy` at `PL011hw.h:560-572` combines
TXFE and BUSY to produce a "TX is idle" predicate. All use the
fenced read helper.

### 1.6 Baud-rate generation (UARTIBRD / UARTFBRD)

Datasheet: Baud rate is generated as UARTCLK / (16 * BaudDiv),
where BaudDiv = IBRD + FBRD/64. UARTIBRD is a 16-bit integer
divisor (must be non-zero); UARTFBRD is a 6-bit fractional divisor
(`pl011-minimized-datasheet.md:909-998`). The register-specific
note at `pl011-minimized-datasheet.md:1039-1042` states that
UARTLCR_H, UARTIBRD, and UARTFBRD must not be changed while the
UART is enabled.

Driver: The baud clock source is bound at driver load:
`PL011pDriverReadConfig` (`PL011driver.cpp:185-312`) reads
`UartClockHz` (default `PL011_DEAFULT_UART_CLOCK`, 16 MHz; see
`PL011driver.cpp:225-231`) and `MaxBaudRateBPS` (default 921600;
see `PL011driver.cpp:217-223`) from the service parameters key via
a descriptor-table loop at `PL011driver.cpp:269-300` over
`WdfRegistryQueryULong`. `PL011HwGetSupportedBaudRates` is invoked
once per device during init (`PL011hw.cpp:102`) to enumerate
achievable rates given the configured clock. A per-baud-rate error
tolerance of 1% is checked against `PL011_MAX_BUAD_RATE_ERROR_PERCENT`
at `PL011hw.cpp:568-576`; rates whose computed divisors produce
more than 1% error are rejected with `STATUS_NOT_SUPPORTED`.

At runtime the baud-change path `PL011HwSetBaudRate` writes the
divisor registers at `PL011hw.cpp:581-605`, under the `RegsLock`
`KSPIN_LOCK` (`PL011hw.cpp:583`): UARTIBRD at `PL011hw.cpp:589-592`,
UARTFBRD at `PL011hw.cpp:593-596`, and UARTLCR_H at
`PL011hw.cpp:598-601`. The sequence does not bracket the writes
with `UARTEN=0` / `UARTEN=1`. Contrast this with
`PL011HwSetLineControl`, which does bracket its UARTLCR_H write:
it clears `UARTEN` via `PL011HwUartControl(... BITMASK_CLEAR ...)`
at `PL011hw.cpp:777-783`, performs the LCR_H modifications, and
restores the prior UARTCR value in `OVERWRITE` mode at
`PL011hw.cpp:917-922` (invoked unconditionally from the `done`
label at `PL011hw.cpp:913-922`).

Discrepancy / note: The `PL011HwSetBaudRate` path violates the
datasheet rule that UART must be disabled before IBRD/FBRD/LCR_H
are reprogrammed. The `PL011HwSetLineControl` path honors it. Both
paths are reached from the SerCx2 apply-config callback, so the
discipline is inconsistent across the two post-init reconfiguration
paths.

### 1.7 Line-control configuration (UARTLCR_H)

Datasheet: UARTLCR_H combines word length (WLEN, bits 6:5; values
5/6/7/8 bits), stop bits (STP2, bit 3; 1 or 2), parity enable
(PEN, bit 1), even-parity select (EPS, bit 2), stick-parity select
(SPS, bit 7), break transmit (BRK, bit 0), and FIFO enable (FEN,
bit 4) (`pl011-minimized-datasheet.md:999-1046`).

Driver: `PL011HwSetLineControl` at `PL011hw.cpp:760-925` is invoked
from `PL011pApplyConfig` (in `PL011uart.cpp:244-260`) after parsing
the SerCx2 line-control request. It disables the UART at
`PL011hw.cpp:777-783`, acquires the device register lock at
`PL011hw.cpp:785-786`, reads UARTLCR_H at `PL011hw.cpp:788`,
clears `UARTLCR_WLEN_MASK` at `PL011hw.cpp:793`, and selects one
of the four WLEN values in `switch (WordLength)` at
`PL011hw.cpp:795-817` (5, 6, 7, or 8 bits). Stop-bit selection is
at `PL011hw.cpp:822-839`. Parity selection is at
`PL011hw.cpp:844-884` and covers five cases: `NO_PARITY` (clear
PEN) at `PL011hw.cpp:847-850`, `ODD_PARITY` (set PEN, clear
EPS and SPS) at `PL011hw.cpp:852-857`, `EVEN_PARITY` (set PEN and
EPS, clear SPS) at `PL011hw.cpp:859-864`, `MARK_PARITY` (set PEN
and SPS, clear EPS) at `PL011hw.cpp:866-871`, and `SPACE_PARITY`
(set PEN, EPS, and SPS) at `PL011hw.cpp:873-878`. The new
UARTLCR_H value is written at `PL011hw.cpp:889`, and the prior
UARTCR is restored at `PL011hw.cpp:917-922`.

FIFO enable is driven separately through the `PL011HwEnableFifos`
helper; break transmission is driven through `PL011HwSetBreak`.
Both are declared alongside the other hardware helpers in
`PL011hw.h`.

### 1.8 FIFO configuration and interrupt thresholds (UARTIFLS)

Datasheet: UARTIFLS selects RX and TX trigger levels: RXIFLSEL
(bits 5:3) and TXIFLSEL (bits 2:0), with values 1/8, 1/4, 1/2,
3/4, 7/8 full (RX) / empty (TX)
(`pl011-minimized-datasheet.md:1080-1095`). The PL011 FIFO is 32
entries deep (32x8 TX, 32x12 RX; `pl011-minimized-datasheet.md:107-147`,
`pl011-minimized-datasheet.md:296-303`).

Driver: `PL011HwInitController` programs IFLS at
`PL011hw.cpp:92-96` with `UARTIFLS_RXIFLSEL::RXIFLSEL_1_4` (RX
trigger at one-quarter full) and
`UARTIFLS_TXIFLSEL::TXIFLSEL_1_8` (TX trigger at one-eighth empty),
via `PL011HwSetFifoThreshold` declared in `PL011hw.h`. The enum
values for both fields are defined alongside the register bit
layout in `PL011hw.h`.

The compile-time constant `PL011_FIFO_DEPTH = 16` at
`PL011hw.h:82` is used to size the software TX staging buffer:
`PL011_TX_BUFFER_SIZE_BYTES` is defined as
`1 * PL011_FIFO_DEPTH` at `PL011tx.h:27`. It is therefore not
dead code, but it is also not the hardware FIFO depth (which is
32 per the datasheet). It functions as a TX-staging-buffer sizing
constant whose name is misleading.

### 1.9 Interrupt source masking and clearing (UARTIMSC / UARTICR)

Datasheet: UARTIMSC is the mask-set/clear register that enables or
disables each interrupt source. UARTRIS holds the raw (pre-mask)
interrupt status; UARTMIS holds the masked (post-mask) status.
UARTICR is a write-one-to-clear register that acknowledges
interrupts (`pl011-minimized-datasheet.md:1096-1192`).

Driver: The raw-status bits, mask bits, and clear bits are defined
in `PL011hw.h:219-229` (UARTRIS fields RIMIS, CTSMIS, DCDMIS,
DSRMIS, RXIS, TXIS, RTIS, FEIS, PEIS, BEIS, OEIS). The ISR reads
UARTRIS (not UARTMIS) at `PL011interrupt.cpp:256-258` and compares
against `UART_INTERUPPTS_ALL` at `PL011interrupt.cpp:259` to decide
whether the interrupt belongs to the PL011. Masking is done via
`PL011HwMaskInterrupts` (`PL011hw.cpp:428-476`), called from
`PL011HwInitController` at `PL011hw.cpp:69-74` to mask all
interrupts during init. Acknowledging is done via
`PL011HwClearInterrupts` (declared in `PL011hw.h`), called at
`PL011hw.cpp:79-82` during init to clear any stale pending
interrupts. At runtime, the ISR acknowledges the bits it observed
by writing the UARTRIS snapshot directly to UARTICR at
`PL011interrupt.cpp:322-325`.

Discrepancy / note: The driver reads UARTRIS rather than UARTMIS.
`pl011-spec-requirements.md:171-187` REQ-IRQ-03 recommends
maintaining a software IMSC shadow to avoid read-modify-write on
the live register. The Windows driver does not maintain such a
shadow; every mask/unmask is an RMW (see Section 4.4).

### 1.10 Receive path

Datasheet: Received bytes and their error flags accumulate in the
32-deep RX FIFO. RX data interrupts (RXIS) fire at the
IFLS-configured threshold; a receive-timeout interrupt (RTIS /
RTINTR) fires when the FIFO is non-empty and no further character
has arrived for a 32-bit period
(`pl011-minimized-datasheet.md:296-303`,
`pl011-minimized-datasheet.md:718-721`).

Driver: The RX PIO path is defined in `PL011rx.h:28-369`. The
state machine `PL011_RX_PIO_STATE` (`PL011rx.h:46-59`) tracks
receive enable/drain state. A SerCx2 PIO receive object is created
at `PL011device.cpp:592` (`SerCx2PioReceiveCreate`), wired to
three callbacks: `PL011SerCx2EvtPioReceiveReadBuffer`,
`PL011SerCx2EvtPioReceiveEnableReadyNotification`, and
`PL011SerCx2EvtPioReceiveCancelReadyNotification`
(`PL011device.cpp:578-581`, callbacks declared at
`PL011rx.h:114-116`). Drained RX data is placed into an 8 KB
circular staging buffer (`RxBuffer[PL011_RX_BUFFER_SIZE_BYTES]`)
declared at `PL011rx.h:84`, with `volatile LONG RxBufferLock`,
`RxBufferIn`, `RxBufferOut`, `RxBufferCount` at `PL011rx.h:80-83`.

Both the ISR and the DPC drive the RX drain. The ISR at
`PL011interrupt.cpp:281-298`, when `regUARTRIS &
(UARTRIS_RXIS | UARTRIS_RTIS)` is non-zero, calls
`PL011RxPioFifoCopy` at `PL011interrupt.cpp:285` and transitions
the RX state to `RX_PIO_STATE__DATA_READY` via
`PL011RxPioStateSetCompare` at `PL011interrupt.cpp:292-296`. The
DPC at `PL011interrupt.cpp:140-170`, when the deferred event mask
carries the same bits, also calls `PL011RxPioFifoCopy` at
`PL011interrupt.cpp:148`, then (if new bytes are pending)
transitions the state via `PL011RxPioStateSetCompare` from
`RX_PIO_STATE__WAIT_READ_DATA` to `RX_PIO_STATE__WAIT_DATA` at
`PL011interrupt.cpp:156-160` and notifies SerCx2 via
`SerCx2PioReceiveReady` at `PL011interrupt.cpp:164`. Two drain
passes are therefore possible per interrupt (once in the ISR and
once in the DPC); `PL011RxPioFifoCopy` handles the "already
drained" case as a no-op by checking `PL011HwIsRxFifoEmpty`
before reading UARTDR.

### 1.11 Transmit path

Datasheet: TX FIFO bytes drain as the serializer shifts them out
at the configured baud rate. The TXIS interrupt fires when the TX
FIFO crosses the IFLS TX threshold on the way to empty
(`pl011-minimized-datasheet.md:296-299`,
`pl011-minimized-datasheet.md:704-717`). The BUSY bit in UARTFR
remains set until the shift register has finished emitting.

Driver: The TX PIO path is defined in `PL011tx.h:28-367` with a
state machine `PL011_TX_PIO_STATE` (`PL011tx.h:46-60`) and a
`TxBuffer` sized to `PL011_TX_BUFFER_SIZE_BYTES` (= 16) at
`PL011tx.h:27` and `PL011tx.h:85`. The TX context holds
`volatile LONG TxBufferLock` and `volatile LONG TxBufferCount` at
`PL011tx.h:81-84`. A SerCx2 PIO transmit object is created at
`PL011device.cpp:647` (`SerCx2PioTransmitCreate`), wired to the
primary send/enable/cancel callbacks at `PL011device.cpp:625-630`
plus drain / cancel-drain / purge callbacks at
`PL011device.cpp:631-636`. The drain-fifo callback
(`PL011SerCx2EvtPioTransmitDrainFifo`, declared at
`PL011tx.h:111`) waits for the hardware FIFO to empty and the
serializer to become idle via the `PL011HwIsTxBusy` /
`PL011HwIsTxFifoEmpty` predicates in `PL011hw.h`.

The TX refill is driven by both ISR and DPC. The ISR at
`PL011interrupt.cpp:305-317`, when `regUARTRIS & UARTRIS_TXIS` is
non-zero, calls `PL011TxPioFifoCopy` at `PL011interrupt.cpp:309`
and transitions the TX state to `TX_PIO_STATE__DATA_SENT` via
`PL011TxPioStateSetCompare` at `PL011interrupt.cpp:311-315`. The
DPC at `PL011interrupt.cpp:177-208`, when the deferred event mask
carries `UARTRIS_TXIS`, also calls `PL011TxPioFifoCopy` at
`PL011interrupt.cpp:185`, then (if software-buffer space has
opened up) transitions the state via `PL011TxPioStateSetCompare`
from `TX_PIO_STATE__WAIT_SEND_DATA` to
`TX_PIO_STATE__WAIT_DATA_SENT` at
`PL011interrupt.cpp:194-198` and notifies SerCx2 via
`SerCx2PioTransmitReady` at `PL011interrupt.cpp:202`.

### 1.12 Modem control and status signals

Datasheet: UARTCR drives the DTR (bit 10), RTS (bit 11), Out1
(bit 12), and Out2 (bit 13) output lines; UARTFR reflects the
CTS/DSR/DCD/RI input lines. Modem-status interrupts are maskable
through UARTIMSC (CTSMIM, DSRMIM, DCDMIM, RIMIM)
(`pl011-minimized-datasheet.md:534-549`,
`pl011-minimized-datasheet.md:1047-1079`).

Driver: Modem output control bits are enumerated in the UARTCR bit
definitions in `PL011hw.h`. The default-supported-controls constant
`PL011_DEFAULT_SUPPORTED_CONTROLS` at `PL011hw.h:279-284` includes
only `UARTCR_UARTEN`, `UARTCR_LBE`, `UARTCR_TXE`, and `UARTCR_RXE`
(not RTS or DTR). Exposure of RTS and DTR as software-controlled
outputs is conditional on the `UartControlLines` registry value:
in `PL011pDeviceGetSupportedFeatures`
(`PL011device.cpp:1314-1394`), the `FLOW_CTL_NONE` branch at
`PL011device.cpp:1332-1340` ORs `UARTCR_RTS` into
`UartSupportedControlsMask` only if
`UART_SERIAL_LINES_RTS` is set in `UartControlLines`, and
`UARTCR_DTR` only if `UART_SERIAL_LINES_DTR` is set.

Incoming modem-status interrupts are translated to wait-event
flags in `PL011DeviceNotifyEvents` at `PL011device.cpp:434-453`:
`UART_INTERUPPTS_ERRORS` produces `SERIAL_EV_ERR`, `UARTRIS_BEIS`
produces `SERIAL_EV_BREAK`, `UARTRIS_CTSMIS` produces
`SERIAL_EV_CTS`, and `UARTRIS_DSRMIS` produces `SERIAL_EV_DSR`.
`UARTRIS_DCDMIS` and `UARTRIS_RIMIS` have no translation branch
in this function; the corresponding `SERIAL_EV_RLSD` and
`SERIAL_EV_RING` event types are not populated by the driver.

Discrepancy / note: On RPi2/3 the CTS/RTS/DTR/DSR/DCD/RI pins are
not routed to user-accessible signals, and the default
`UartControlLines` registry value is 0
(`PL011driver.cpp:241-247`). This suppresses modem output exposure
by default. For modem *inputs*, the driver has a partial
implementation: CTS and DSR interrupts are translated to wait
events, but DCD and RI interrupts are not. That is a driver gap,
not a platform exposure issue.

### 1.13 Flow control (hardware RTS/CTS and software XON/XOFF)

Datasheet: UARTCR bits RTSEN (bit 14) and CTSEN (bit 15) enable
hardware RTS/CTS flow control. With RTSEN set, RTS deasserts when
the RX FIFO fills past the IFLS threshold and reasserts when it
drains below it. With CTSEN set, TX is gated on CTS assertion
(`pl011-minimized-datasheet.md:551-603`). The PL011 does not
implement software (XON/XOFF) flow control in hardware; any such
support must be driver-level.

Driver: The RTSEN / CTSEN bit definitions are in `PL011hw.h`
alongside the other UARTCR bits. Flow-control mode is bound from
`PL011driver.cpp:233-239` via the `UartFlowControl` registry
parameter (default `UART_SERIAL_FLAG_FLOW_CTL_NONE`). The
supported-features predicate `PL011pDeviceGetSupportedFeatures`
(`PL011device.cpp:1314-1394`) switches on this value at
`PL011device.cpp:1328`:

- `UART_SERIAL_FLAG_FLOW_CTL_NONE` at
  `PL011device.cpp:1330-1343`: no RTSEN / CTSEN; driver optionally
  exposes RTS and DTR as static output lines based on
  `UartControlLines` (see Section 1.12).
- `UART_SERIAL_FLAG_FLOW_CTL_XONXOFF` at
  `PL011device.cpp:1345-1353`: the driver logs an error and
  returns `STATUS_NOT_IMPLEMENTED`. Software flow control is
  explicitly unsupported by this driver.
- `UART_SERIAL_FLAG_FLOW_CTL_HW` at
  `PL011device.cpp:1355-1366`: ORs `UARTCR_RTSEn` into
  `UartSupportedControlsMask` when `UART_SERIAL_LINES_RTS` is set,
  and `UARTCR_CTSEn` when `UART_SERIAL_LINES_CTS` is set.

Discrepancy / note: Hardware flow control is a distinct case from
software flow control in this driver. Hardware flow control is a
valid device feature that the driver implements behind a registry
gate; it is simply not exposed by default on RPi2/3 because the
platform does not route CTS/RTS pins (README at
`reference-drivers/pl011/windows/README.md:4`). Software flow
control is not a datasheet feature of the PL011 hardware, and the
driver has no implementation for it; it rejects the mode at the
capability-check layer.

### 1.14 Loopback, break, IrDA SIR, DMA

Datasheet: UARTCR provides a Loopback Enable bit (LBE, bit 7)
that internally connects TXD to RXD for diagnostic use
(`pl011-minimized-datasheet.md:452-455`). Break transmission is
produced by setting BRK in UARTLCR_H (bit 0). IrDA SIR support is
optional and driven through SIREN (bit 1) / SIRLP (bit 2) in
UARTCR plus UARTILPR (`pl011-minimized-datasheet.md:212-219`,
`pl011-minimized-datasheet.md:456-491`). DMA request signaling is
configured through UARTDMACR (RXDMAE, TXDMAE, DMAONERR;
`pl011-minimized-datasheet.md:604-643`,
`pl011-minimized-datasheet.md:1193-1205`).

Driver:

- Loopback: `UARTCR_LBE` is present in
  `PL011_DEFAULT_SUPPORTED_CONTROLS` at `PL011hw.h:279-284`. The
  driver does not program LBE during normal operation.
- Break: break transmission is driven through `PL011HwSetBreak`
  declared in `PL011hw.h` and invoked from the IOCTL handlers
  declared in `PL011ioctl.h` (Set Break On / Off entry points).
- IrDA SIR: `UARTILPR` is declared in the register-file enum at
  `PL011hw.h:35`, but no code reads or writes it, and
  `PL011_DEFAULT_SUPPORTED_CONTROLS` does not include SIREN or
  SIRLP. The driver has no implementation for IrDA SIR mode.
- DMA: `UARTDMACR` is declared in the register-file enum at
  `PL011hw.h:45`, but no code reads or writes it.
  `PL011pDeviceParseResources` at `PL011device.cpp:779-787`
  counts `CmResourceTypeDma` entries but contains an explicit "To
  be implemented..." comment at `PL011device.cpp:783-785`. The
  count is validated (0 or 2) at `PL011device.cpp:857-864`. The
  README at `reference-drivers/pl011/windows/README.md:4`
  confirms the platform does not use DMA.

Discrepancy / note: IrDA and DMA are both cases where the
register-file enum covers the capability but the driver has no
runtime implementation. They differ in that DMA additionally has
resource-parse scaffolding (for counting channels) and an
explicit "To be implemented..." placeholder, while IrDA has
neither. Per the task brief's Section 4.1 rule, these are
recorded explicitly rather than collapsed into a single "not
used" note: IrDA is "platform does not expose, and driver has no
implementation"; DMA is "platform does not expose on this
platform, and driver has partial resource-parsing scaffolding
pending implementation."

### 1.15 Variability

Driver behavior branches on the following axes within the
Functional dimension:

- `UartClockHz` registry parameter (default 16 MHz) drives
  baud-rate divisor calculation. Branch sites:
  `PL011driver.cpp:225-231` (read), `PL011HwGetSupportedBaudRates`
  and `PL011HwSetBaudRate` (consume).
- `MaxBaudRateBPS` registry parameter (default 921600) bounds
  the accepted baud range. Branch site:
  `PL011driver.cpp:217-223`.
- `UartFlowControl` registry parameter gates exposure of
  hardware flow control and rejects software flow control.
  Branch sites: `PL011driver.cpp:233-239`
  (read); `PL011device.cpp:1328-1366` (switch).
- `UartControlLines` registry parameter gates exposure of RTS
  and DTR as software-controlled output lines (and of RTS/CTS
  under hardware flow control). Branch sites:
  `PL011driver.cpp:241-247` (read),
  `PL011device.cpp:1332-1340` and `PL011device.cpp:1357-1364`
  (consume).
- ACPI FunctionConfig connection resource (optional) toggles
  the kernel-debugger-conflict behavior between "fail load" and
  "load stale, reject opens". Branch sites:
  `PL011device.cpp:733-749`, `PL011device.cpp:810-826`.
- `IS_DONT_CHANGE_HW` build-time flag elides hardware writes
  while preserving reads. Branch sites: `PL011hw.h:419-428`,
  `PL011hw.h:454-463`, `PL011device.cpp:743-749`.
- PrimeCell revision is not read by the driver, so there is no
  revision-conditional functional branch in this driver.

## 2. Integration Analysis

Question: which Windows kernel subsystems does the stock driver
integrate the PL011 device into, and how?

### 2.1 Driver object and entry point

Driver: `DriverEntry` at `PL011driver.cpp:61-157` initializes
WPP tracing with `WPP_INIT_TRACING` at `PL011driver.cpp:74`,
creates a `WDFDRIVER` through `WdfDriverCreate`, and registers
`PL011EvtDriverUnload` at `PL011driver.cpp:158` as the unload
callback. Driver-wide state (`PL011_DRIVER_EXTENSION`) holds
the registry-bound parameters populated by
`PL011pDriverReadConfig` (`PL011driver.cpp:187-312`).

### 2.2 SerCx2 initialization and device creation

Driver: `PL011EvtDeviceAdd` at `PL011device.cpp:78-173` calls
`SerCx2InitializeDeviceInit(DeviceInitPtr)` at
`PL011device.cpp:108` before `WdfDeviceCreate`, with the
ordering asserted in a comment at `PL011device.cpp:101-106`
("MUST be called before calling WdfDeviceCreate()"). After
`WdfDeviceCreate`, `PL011EvtDeviceAdd` invokes
`PL011pDeviceSerCx2Init` at `PL011device.cpp:166`.

`PL011pDeviceSerCx2Init` (`PL011device.cpp:536-683`) calls
`SerCx2InitializeDevice` at `PL011device.cpp:560`, passing a
configuration struct that binds the SerCx2 uart-event
callbacks declared in `PL011uart.h`:
`PL011EvtSerCx2ApplyConfig` (`PL011uart.h:75`),
`PL011EvtSerCx2Control` (`PL011uart.h:155`),
`PL011EvtSerCx2PurgeFifos`, `PL011EvtSerCx2FileOpen`,
`PL011EvtSerCx2FileClose`, and `PL011EvtSerCx2SetWaitMask`.

Note on lifecycle: because `PL011pDeviceSerCx2Init` is called
from `PL011EvtDeviceAdd`, the SerCx2 PIO receive and transmit
objects (Section 2.3) are created during device add, not
during `PrepareHardware`. `PrepareHardware` is responsible
only for ACPI resource parsing, MMIO mapping, interrupt-object
creation, and hardware init (Section 2.5, 2.6).

### 2.3 SerCx2 PIO receive and transmit contracts

Driver: Only the PIO receive and PIO transmit SerCx2 paths are
registered. PIO receive is created at `PL011device.cpp:592`
(`SerCx2PioReceiveCreate`) with callbacks at
`PL011device.cpp:578-581`. PIO transmit is created at
`PL011device.cpp:647` (`SerCx2PioTransmitCreate`) with the
primary callbacks at `PL011device.cpp:625-630` and the drain /
cancel-drain / purge callbacks at `PL011device.cpp:631-636`.
Neither `SerCx2CustomReceiveTransmitCreate` nor
`SerCx2SystemDmaReceiveCreate` nor
`SerCx2SystemDmaTransmitCreate` is called; the driver is a
PIO-only integration.

### 2.4 WDF PnP / power callbacks

Driver: In `PL011EvtDeviceAdd`, only `EvtDevicePrepareHardware`
and `EvtDeviceReleaseHardware` are registered. The
initialization block at `PL011device.cpp:90-96` calls
`WDF_PNPPOWER_EVENT_CALLBACKS_INIT`, assigns
`PL011EvtDevicePrepareHardware` at `PL011device.cpp:93` and
`PL011EvtDeviceReleaseHardware` at `PL011device.cpp:94`, then
`WdfDeviceInitSetPnpPowerEventCallbacks` at
`PL011device.cpp:96`. `EvtDeviceD0Entry`, `EvtDeviceD0Exit`,
self-managed I/O callbacks, and arming/disarming callbacks are
not assigned in this code path.

Discrepancy / note: The `PL011_DEVICE_EXTENSION` struct
declares a `BOOLEAN IsDeviceActive` field at `PL011device.h:131`,
with a comment at `PL011device.h:124-130` that references
`EvtDeviceD0Enty` and `EvtDeviceD0ExitPreInterruptsDisabled`
and states that the flag is set to TRUE in D0Entry and cleared
before D0Exit returns. Those callbacks are never registered;
no other code in the provided sources reads or writes the
`IsDeviceActive` field. The field and its comment are stale
relative to the final PnP/Power callback wiring.

### 2.5 Interrupt object and ISR / DPC wiring

Driver: `PL011pDeviceMapResources` creates an interrupt spin
lock at `PL011device.cpp:945` (`WdfSpinLockCreate`), then uses
it in a `WDF_INTERRUPT_CONFIG` passed to `WdfInterruptCreate`.
The ISR callback is `PL011EvtInterruptIsr`
(`PL011interrupt.cpp:56-87`), which defers to
`PL011pInterruptIsr` at `PL011interrupt.cpp:247-328` for the
hardware work. The DPC callback is `PL011EvtInterruptDpc`
(`PL011interrupt.cpp:115-225`), dispatched by
`WdfInterruptQueueDpcForIsr` at `PL011interrupt.cpp:79`.

The ISR reads UARTRIS at `PL011interrupt.cpp:256-258`, ORs the
snapshot into `DevExtPtr->IntEventsForDpc` for the DPC at
`PL011interrupt.cpp:270-273`, performs FIFO data movement inline
(RX drain at `PL011interrupt.cpp:281-298`, TX refill at
`PL011interrupt.cpp:305-317`), and acknowledges all observed
interrupts by writing the UARTRIS snapshot to UARTICR at
`PL011interrupt.cpp:322-325`. RX and TX state-machine
transitions occur at `PL011interrupt.cpp:292-296` and
`PL011interrupt.cpp:311-315`.

The DPC also performs FIFO data movement and SerCx2 ready
notifications (the ISR cannot call into SerCx2 at DIRQL). It
atomically exchanges `IntEventsForDpc` to 0 at
`PL011interrupt.cpp:130-132`, then in the RX branch at
`PL011interrupt.cpp:140-170` it calls `PL011RxPioFifoCopy` at
`PL011interrupt.cpp:148`, transitions RX state at
`PL011interrupt.cpp:156-160`, and calls `SerCx2PioReceiveReady`
at `PL011interrupt.cpp:164`; in the TX branch at
`PL011interrupt.cpp:177-208` it calls `PL011TxPioFifoCopy` at
`PL011interrupt.cpp:185`, transitions TX state at
`PL011interrupt.cpp:194-198`, and calls `SerCx2PioTransmitReady`
at `PL011interrupt.cpp:202`. Finally it records error flags via
`PL011DeviceRecordErrors` at `PL011interrupt.cpp:213-216` and
notifies wait events via `PL011DeviceNotifyEvents` at
`PL011interrupt.cpp:221-224`. The DPC does not write UARTICR;
acknowledgement remains the ISR's responsibility.

### 2.6 Resource acquisition (ACPI / PnP)

Driver: `PL011EvtDevicePrepareHardware` at
`PL011device.cpp:197-291` receives raw and translated resource
lists from PnP. `PL011pDeviceParseResources` at
`PL011device.cpp:700-867` iterates the translated list:
`CmResourceTypeMemory` for MMIO base/span with validation
against `REG_FILE_SIZE` at `PL011device.cpp:728`,
`CmResourceTypeInterrupt` for IRQ vector/level/affinity at
`PL011device.cpp:769-777`, `CmResourceTypeDma` counted but
unused at `PL011device.cpp:779-787` ("To be implemented..."
comment), `CmResourceTypeConnection` for the UART serial
connection ID at `PL011device.cpp:789-808`, and the optional
FunctionConfig connection ID at `PL011device.cpp:810-826`.
`PL011pDeviceMapResources` (starting at
`PL011device.cpp:894`) then maps the MMIO range and creates
the interrupt object. The debugger-conflict check at
`PL011device.cpp:733-749` compares the MMIO base against
`*KdComPortInUse` and records `IsDebuggerConflict` if they
match.

`PL011EvtDevicePrepareHardware` ends at `PL011device.cpp:290`
by returning `PL011HwInitController(WdfDevice)` (Section 1.2),
so hardware init is part of `PrepareHardware`.

### 2.7 SerCx2 file open / close and wait-mask lifecycle

Driver: `PL011EvtSerCx2FileOpen` and `PL011EvtSerCx2FileClose`
(declared in `PL011uart.h` near the other SerCx2 callbacks)
are the points at which TXE and RXE in UARTCR are toggled; the
comment at `PL011hw.cpp:105-106` ("RX and TX are enabled when
the device is opened") states this contract.
`EvtSerCx2SetWaitMask` is also registered, providing the
subset of wait-event bits the client requests; DPC-side event
notification in `PL011DeviceNotifyEvents` at
`PL011device.cpp:423-503` consults the client's wait mask
under the `ConfigLock` `EX_SPIN_LOCK` before signaling SerCx2
at `PL011device.cpp:458-459`.

### 2.8 IOCTL / IRP surface

Driver: The IOCTL surface is implemented in `PL011ioctl.{h,cpp}`
and dispatched through the SerCx2 `PL011EvtSerCx2Control`
callback declared at `PL011uart.h:155`. The handler delegates
to per-IOCTL helpers declared in `PL011ioctl.h`:
`PL011IoctlSetBaudRate` / `GetBaudRate`, `PL011IoctlSetHandflow`
/ `GetHandflow`, `PL011IoctlSetModemControl` /
`GetModemControl`, `PL011IoctlSetLineControl` / `GetLineControl`,
`PL011IoctlGetModemStatus`, `PL011IoctlGetCommStatus`,
`PL011IoctlSetBreakOn` / `SetBreakOff`, `PL011IoctlGetProperties`,
`PL011IoctlSetClrRTS` / `SetClrDTR`, and FIFO-control entry
points. SerCx2 is responsible for the I/O queue wiring; the
driver provides only the per-IOCTL behavior.

### 2.9 Registry parameter binding

Driver: Registry reads happen once, in `PL011pDriverReadConfig`
(`PL011driver.cpp:187-312`), via a descriptor-table loop at
`PL011driver.cpp:269-300`. Values are written into the
`PL011_DRIVER_EXTENSION` and copied per-device during
`PrepareHardware` into each device's `CurrentConfiguration`.
Missing keys fall back to compile-time defaults
(`PL011driver.cpp:278-283`). The
`HKLM\System\CurrentControlSet\services\SerPl011\Parameters`
values are pre-populated by the INF:
`UartClockHz = 0x00F42400` (16 MHz),
`MaxBaudRateBPS = 0x000E1000` (921600),
`UartFlowControl = 0`, `UartControlLines = 0` (see
`SerPL011.inf:64-67`).

### 2.10 INF, device class, and service registration

Driver: `SerPL011.inf` declares service `SerPL011` as
`KERNEL_DRIVER` (type), `DEMAND_START` (start), with error
control `SERVICE_ERROR_NORMAL`, load-order group "Extended
Base", and a `DependOnService` on `SerCx2`
(`SerPL011.inf:50-60`). Device class is System
(`{4D36E97D-E325-11CE-BFC1-08002BE10318}`) on
`SerPL011.inf:14`. The hardware-ID match is `ACPI\BCM2837` on
`SerPL011.inf:40`. The `Parameters` subkey is populated via an
`AddReg` entry range at `SerPL011.inf:64-67`.

### 2.11 WPP / ETW tracing

Driver: `PL011logging.h:42-47` declares the WPP control GUID
`SERPL011` (`2EA62EE7-3DC8-401D-99DA-3F9379A231AD`) with flags
`TRACING_DEFAULT`, `TRACING_VERBOSE`, `TRACING_DEBUG`, and
defines macros `PL011_LOG_ERROR`, `PL011_LOG_WARNING`,
`PL011_LOG_INFORMATION`, `PL011_LOG_TRACE`, `PL011_ASSERT` at
`PL011logging.h:54-67`. `DriverEntry` initializes WPP via
`WPP_INIT_TRACING` at `PL011driver.cpp:74`. Representative
call sites include `PL011HwInitController` emitting
`PL011_LOG_INFORMATION("Controller initialization done!")` at
`PL011hw.cpp:115`, the ISR emitting `PL011_LOG_TRACE` at
`PL011interrupt.cpp:81-84`, and error paths throughout
`PL011pDeviceParseResources` and `PL011pDriverReadConfig`
emitting `PL011_LOG_ERROR`.

### 2.12 Device interface and SDDL

Driver: `PL011EvtDeviceAdd` at `PL011device.cpp:117-125`
declares an SDDL string granting `GA` (generic all) to system
and local administrators and `GRGW` to built-in users, paired
with a SerCx2 device interface created later through
`PL011pDeviceCreateDeviceInterface` at `PL011device.cpp:1024`.
This controls which user-mode callers can open the serial
device.

### 2.13 Variability

- `ACPI\BCM2837` hardware ID in `SerPL011.inf:40` binds this
  driver to RPi3 (and RPi2 by family); the driver is not
  installed on platforms exposing other PL011 hardware IDs
  through this INF.
- ACPI resources are dynamic (BIOS/UEFI-supplied): the driver
  accepts 1 memory, 1 interrupt, 0 or 2 DMA, 1 serial
  connection, and optionally 1 FunctionConfig connection.
  Branch sites: `PL011device.cpp:722-827`, with the DMA
  0-or-2 check at `PL011device.cpp:857-864`.
- `DependOnService SerCx2` in the INF requires the SerCx2
  class extension to be installed; this is a system-level
  integration variability but is static at install time.
- Kernel-debugger conflict handling: depending on whether a
  FunctionConfig pin-reservation resource is supplied, the
  driver either fails load or loads in a
  stale-reject-opens state (`PL011device.cpp:733-749`,
  `PL011device.cpp:810-826`).

## 3. Performance Analysis

Question: which PL011 performance features does the Windows
driver implement, and which Windows-side performance facilities
does the driver use to improve the throughput, latency, or CPU
cost of device operation? This section catalogs features whose
implementation affects performance in general; no external
workload or benchmark is used.

### 3.1 FIFO thresholds

Datasheet: UARTIFLS determines when RXIS / TXIS fire, trading
off batching against latency
(`pl011-minimized-datasheet.md:1080-1095`). RX threshold at
1/4 full means up to 24 characters can accumulate before
interrupting; TX threshold at 1/8 empty means the TX-refill
interrupt fires when only 4 characters remain in the FIFO,
giving the driver time to refill before the hardware drains.

Driver: Programmed at init in `PL011HwInitController` at
`PL011hw.cpp:92-96` to `RXIFLSEL_1_4` (RX trigger one-quarter
full) and `TXIFLSEL_1_8` (TX trigger one-eighth empty). The
default is a deliberate tradeoff: RX-side batches into
medium-sized bursts (for throughput), TX-side prioritizes
underrun-avoidance (for latency-sensitive line rates).

At runtime, IFLS is reprogrammable through an IOCTL surface:
`PL011EvtSerCx2Control` dispatches `IOCTL_SERIAL_SET_FIFO_CONTROL`
to `PL011IoctlSetFifoControl` at `PL011uart.cpp:246-248`. The
handler at `PL011ioctl.cpp:1280-1402` retrieves a single
`ULONG fifoControl` word from the request via
`WdfRequestRetrieveInputBuffer` at `PL011ioctl.cpp:1302-1308`,
rejects `SERIAL_FCR_DMA_MODE` with `STATUS_NOT_SUPPORTED` at
`PL011ioctl.cpp:1320-1330`, then decodes the TX trigger by
switching on `fifoControl & SERIAL_TX_FIFO_MASK` at
`PL011ioctl.cpp:1338-1356` (mapping `SERIAL_TX_1_BYTE_TRIG` to
`TXIFLSEL_1_8`, `SERIAL_TX_4_BYTE_TRIG` to `TXIFLSEL_1_4`,
`SERIAL_TX_8_BYTE_TRIG` to `TXIFLSEL_1_2`, and
`SERIAL_TX_14_BYTE_TRIG` to `TXIFLSEL_7_8`) and the RX trigger
by switching on `fifoControl & SERIAL_RX_FIFO_MASK` at
`PL011ioctl.cpp:1362-1380` (mapping `SERIAL_1_BYTE_HIGH_WATER`
through `SERIAL_14_BYTE_HIGH_WATER` to the four `RXIFLSEL` enum
values). `SERIAL_FCR_RCVR_RESET` and `SERIAL_FCR_TXMT_RESET`
flags trigger `PL011RxPurgeFifo` / `PL011TxPurgeFifo` at
`PL011ioctl.cpp:1382-1390`. The handler then calls
`PL011HwSetFifoThreshold` at `PL011ioctl.cpp:1392` and
`PL011HwEnableFifos(isFifoOn)` at `PL011ioctl.cpp:1394` where
`isFifoOn = (fifoControl & SERIAL_FCR_ENABLE) != 0`
(`PL011ioctl.cpp:1332`).
`PL011HwSetFifoThreshold` at `PL011hw.cpp:359-395` bookends the
IFLS write with `PL011HwEnableFifos(FALSE)` at `PL011hw.cpp:368`
and `PL011HwEnableFifos(TRUE)` at `PL011hw.cpp:388`, toggling
`UARTLCR_H.FEN` rather than clearing `UARTCR.UARTEN` (see
Section 4.3 for the correctness implications of this choice).

### 3.2 Receive timeout interrupt

Datasheet: RTINTR (RTIS in UARTRIS) fires when the RX FIFO is
non-empty and no new character has arrived for a 32-bit
period, delivering sub-FIFO-threshold data with low latency
(`pl011-minimized-datasheet.md:718-721`).

Driver: The ISR's RX branch combines RXIS and RTIS into a
single RX-drain trigger at `PL011interrupt.cpp:281`:
`regUARTRIS & (UARTRIS_RXIS | UARTRIS_RTIS)`. This means the
driver treats a receive-timeout the same as a
threshold-crossing: drain the FIFO into the RX staging buffer
at `PL011interrupt.cpp:285`. The combination gives good
latency for bursty and steady-state traffic without
duplicating drain logic.

### 3.3 ISR / DPC partitioning

Driver: FIFO data movement happens in both the ISR and the DPC.
The ISR (`PL011EvtInterruptIsr` at `PL011interrupt.cpp:56-87`
calling `PL011pInterruptIsr` at `PL011interrupt.cpp:247-328`)
reads UARTRIS, drains the RX FIFO when `UARTRIS_RXIS |
UARTRIS_RTIS` is asserted (`PL011interrupt.cpp:281-298`),
refills the TX FIFO when `UARTRIS_TXIS` is asserted
(`PL011interrupt.cpp:305-317`), and acknowledges the
interrupts by writing the UARTRIS snapshot to UARTICR at
`PL011interrupt.cpp:322-325`. The DPC
(`PL011EvtInterruptDpc` at `PL011interrupt.cpp:115-225`)
re-runs `PL011RxPioFifoCopy` and `PL011TxPioFifoCopy` at
`PL011interrupt.cpp:148` and `PL011interrupt.cpp:185` on the
bits the ISR staged, performs the SerCx2 ready notifications
that are illegal at DIRQL
(`SerCx2PioReceiveReady`, `SerCx2PioTransmitReady`), and
performs error / wait-event translation. The ISR reduces
worst-case interrupt latency by draining / refilling in the
same DIRQL window the interrupt occurred; the DPC handles the
SerCx2 upcalls and catches any bytes that arrived between the
ISR-side drain and the ICR acknowledge. `PL011RxPioFifoCopy`
and `PL011TxPioFifoCopy` handle the already-drained / already-
refilled case as a no-op, so the double-pass is not a
correctness concern, only a cost tradeoff.

### 3.4 Fenced vs. non-fenced MMIO primitives

Driver: `PL011hw.h:364-464` defines two families of MMIO
helpers: `PL011HwReadRegisterUlong` /
`PL011HwWriteRegisterUlong` use the fenced `READ_REGISTER_ULONG`
/ `WRITE_REGISTER_ULONG` primitives;
`PL011HwReadRegisterUlongNoFence` /
`PL011HwWriteRegisterUlongNoFence` use the `_NOFENCE_`
variants. The justifying comment at `PL011hw.h:378-380` and
`PL011hw.h:437-439` explicitly notes the non-fenced variants
are "faster and can be used for read-only processes like
reading from FIFOs, etc." FIFO access (`PL011TxPioFifoCopy`,
`PL011RxPioFifoCopy`, for example the UARTDR read cast at
`pl011rx.cpp:525-526`) uses the non-fenced helpers;
control-register configuration (baud, line control, IMSC, ICR,
CR) uses the fenced helpers.

### 3.5 RX staging buffer

Driver: `PL011rx.h:80-84` declares an 8 KB circular RX buffer
(`RxBuffer[PL011_RX_BUFFER_SIZE_BYTES]`) with a `volatile LONG
RxBufferLock` interlocked flag and `RxBufferIn` /
`RxBufferOut` / `RxBufferCount` cursors. The ISR drains the
hardware FIFO into this buffer (Section 1.10), and the DPC
then notifies SerCx2. The buffer decouples interrupt-driven
drain from user-mode read cadence; it absorbs bursts up to 8
KB without requiring the user-mode reader to run at
DPC-matching rates.

### 3.6 TX staging buffer

Driver: `PL011tx.h:27` defines
`PL011_TX_BUFFER_SIZE_BYTES = 1 * PL011_FIFO_DEPTH = 16`. The
staging buffer `TxBuffer[PL011_TX_BUFFER_SIZE_BYTES]` at
`PL011tx.h:85` is guarded by a `volatile LONG TxBufferLock`
interlocked flag at `PL011tx.h:81`. The write callback stages
client-supplied bytes into `TxBuffer`, and the
ISR's TX branch walks them into the hardware FIFO as space
becomes available (Section 1.11). Because the hardware FIFO
is itself 32 entries, a 16-byte software staging buffer is
sufficient to absorb a single callback's worth of bytes while
the FIFO refill runs.

### 3.7 DMA: not used

Datasheet: UARTDMACR (`pl011-minimized-datasheet.md:604-643`,
`pl011-minimized-datasheet.md:1193-1205`) enables RX and TX
DMA request signaling, with DMAONERR to mask RX requests on
error. DMA is a performance feature of the PL011 aimed at
reducing per-byte CPU cost and IRQ rate for high baud rates.

Driver: DMA is a recognized-but-unused capability. UARTDMACR
is declared in the register-file enum at `PL011hw.h:45` but is
not accessed by any code. Resource-parse scaffolding exists at
`PL011device.cpp:779-787` with an explicit "To be
implemented..." comment, and the count validator accepts 0 or
2 DMA channels at `PL011device.cpp:857-864`. The README
states the device does not use DMA on this platform. Per the
task brief, this is a descriptive finding recording non-use,
not a gap list or a prescription: the driver catalogs the
capability but does not implement a DMA path.

### 3.8 Variability

- `UartClockHz` and `MaxBaudRateBPS` registry parameters
  affect the baud rates achievable and therefore the
  per-second byte rate the driver must sustain. Branch sites:
  `PL011driver.cpp:217-231`.
- IFLS thresholds default to `RXIFLSEL_1_4` / `TXIFLSEL_1_8`
  at init (`PL011hw.cpp:92-96`) but are reprogrammable at
  runtime via `IOCTL_SERIAL_SET_FIFO_CONTROL`
  (`PL011uart.cpp:246-248`, `PL011ioctl.cpp:1280-1402`,
  `PL011hw.cpp:359-395`). The per-connection threshold is
  therefore a client-observable performance knob.
- Whether DMA is used: static "no" on this platform. Branch
  sites: `PL011device.cpp:779-787`, `PL011device.cpp:857-864`,
  `PL011hw.h:45` (UARTDMACR offset only, no access sites).

## 4. Correctness Analysis

Question: what invariants, disciplines, and ordering constraints
govern how the Windows driver exercises the functional mechanisms
safely?

### 4.1 IRQL discipline

Driver: DIRQL code paths run during ISR execution:
`PL011EvtInterruptIsr` at `PL011interrupt.cpp:56-87` and
`PL011pInterruptIsr` at `PL011interrupt.cpp:247-328`. This
code reads UARTRIS, drains the RX FIFO, refills the TX FIFO,
updates the two state machines with
`PL011RxPioStateSetCompare` / `PL011TxPioStateSetCompare`,
writes UARTICR, and uses `InterlockedOr` to stage an event
mask for the DPC at `PL011interrupt.cpp:270-273`. It does not
access paged memory, does not block, and does not acquire the
`RegsLock` `KSPIN_LOCK`; it only acquires the WDF interrupt
lock implicitly through the framework's ISR invocation
contract. DPC-level code
(`PL011EvtInterruptDpc` at `PL011interrupt.cpp:115-225`) runs
at DISPATCH_LEVEL; it too must not touch paged memory or
block. PASSIVE_LEVEL code includes
`PL011EvtDevicePrepareHardware` and
`PL011EvtDeviceReleaseHardware` (both marked `PAGED_CODE()`;
see for example `PL011device.cpp:319`), `DriverEntry` /
`PL011EvtDriverUnload`, and the `PL011pDriverReadConfig`
registry path (`PL011driver.cpp:189` asserts `PAGED_CODE()`).

### 4.2 Lock families in use

Driver: The driver uses several distinct synchronization
primitives, each with a well-defined scope.

- `KSPIN_LOCK RegsLock` (`PL011device.h:108`) serializes
  register read-modify-write operations performed outside the
  ISR. `PL011HwUartControl` acquires it via
  `KeAcquireInStackQueuedSpinLock` at `PL011hw.cpp:273`, and
  `PL011HwSetBaudRate` acquires it at `PL011hw.cpp:583`.
- WDF interrupt spin lock created at `PL011device.cpp:945`
  (`WdfSpinLockCreate`), passed into
  `WDF_INTERRUPT_CONFIG.SpinLock` and acquired implicitly by
  the framework around the ISR. `PL011HwMaskInterrupts`
  acquires this same lock explicitly via
  `WdfInterruptAcquireLock` at `PL011hw.cpp:446-448` when
  called with `IsIsrSafe=TRUE`, and releases it at
  `PL011hw.cpp:463-465`.
- `EX_SPIN_LOCK ConfigLock` (`PL011device.h:142`) serializes
  apply-config transitions; `PL011DeviceNotifyEvents`
  acquires it shared at `PL011device.cpp:459`, while
  configuration writers acquire it exclusive (for example,
  `PL011HwSetBaudRate` at `PL011hw.cpp:611-616`).
- `volatile LONG RxBufferLock` (`PL011rx.h:80`) and
  `volatile LONG TxBufferLock` (`PL011tx.h:81`) are
  interlocked flag fields, not kernel spin locks. They are
  operated on with `Interlocked*` primitives alongside the
  `volatile LONG RxBufferCount` and `volatile LONG
  TxBufferCount` fields at `PL011rx.h:83` and `PL011tx.h:84`.

No WDF wait-lock usage is present in the observed files. The
separation of `RegsLock` (thread-context RMW) from the WDF
interrupt spin lock (ISR) avoids nested-acquire concerns: the
ISR does not acquire `RegsLock`, so a thread-context RMW
under `RegsLock` cannot collide with ISR execution at the
lock level; ISR exclusion against thread-context IMSC changes
is provided by `PL011HwMaskInterrupts`'s
`WdfInterruptAcquireLock` path.

### 4.3 UARTEN gating around init, line control, baud change, and FIFO control

Datasheet: A general note at
`pl011-minimized-datasheet.md:839-840` states that the UART
must be disabled before any of the control registers are
reprogrammed. A register-specific note at
`pl011-minimized-datasheet.md:1039-1042` narrows this to
UARTLCR_H, UARTIBRD, and UARTFBRD: these three must not be
changed while the UART is enabled, or while it is completing a
transmission or reception after having been programmed to
become disabled. `pl011-minimized-datasheet.md:1043-1045` adds
that FIFO integrity is not guaranteed if the software disables
the UART mid-transmission with data in the FIFO and then
re-enables it.

Driver:

- Init: `PL011HwInitController` (`PL011hw.cpp:50-118`) performs
  the full disable-first sequence: UARTCR_ALL / BITMASK_CLEAR
  at `PL011hw.cpp:59-64`, UART_INTERUPPTS_ALL mask at
  `PL011hw.cpp:69-74`, UART_INTERUPPTS_ALL clear via UARTICR at
  `PL011hw.cpp:79-82`, UARTRSR/ECR clear at `PL011hw.cpp:87`,
  UARTIFLS write at `PL011hw.cpp:92-96`, baud-rate enumeration
  at `PL011hw.cpp:102`, and finally UARTCR_UARTEN / OVERWRITE
  at `PL011hw.cpp:108-113`.
- Shutdown: `PL011HwStopController` (`PL011hw.cpp:134-161`)
  inverts the enable step: UARTCR_ALL / BITMASK_CLEAR, then
  UART_INTERUPPTS_ALL mask.
- Line control: `PL011HwSetLineControl` brackets its UARTLCR_H
  write with an explicit UARTEN clear at `PL011hw.cpp:777-783`
  and a UARTCR restore via `OVERWRITE` at `PL011hw.cpp:917-922`
  on the `done` label.
- Baud change: `PL011HwSetBaudRate` at `PL011hw.cpp:581-605`
  writes UARTIBRD, UARTFBRD, and UARTLCR_H under `RegsLock` but
  does not clear UARTEN first or restore it after.
- FIFO-threshold change: `PL011HwSetFifoThreshold` at
  `PL011hw.cpp:359-395` is reached from
  `IOCTL_SERIAL_SET_FIFO_CONTROL` (Section 3.1). It calls
  `PL011HwEnableFifos(FALSE)` at `PL011hw.cpp:368` (which
  clears `UARTLCR_H.FEN` under `RegsLock` at
  `PL011hw.cpp:953-970`), writes UARTIFLS under `RegsLock` at
  `PL011hw.cpp:373-386`, then re-enables the FIFOs via
  `PL011HwEnableFifos(TRUE)` at `PL011hw.cpp:388`. UARTEN is
  not cleared around this sequence.

Discrepancy / note: The baud-change path (`PL011HwSetBaudRate`)
reprograms UARTIBRD, UARTFBRD, and UARTLCR_H while UARTEN
remains set. This violates the register-specific datasheet
note at `pl011-minimized-datasheet.md:1039-1042`. The
line-control path in the same file honors the rule. The
FIFO-threshold path writes UARTIFLS while UARTEN remains set;
UARTIFLS is not named in the register-specific note, so the
stricter general-note interpretation at
`pl011-minimized-datasheet.md:839-840` is the only one that
would require UARTEN to be cleared for IFLS changes. The three
post-init reconfiguration paths therefore apply inconsistent
disciplines: line control does the full disable/restore;
baud change and FIFO-threshold change do not.

### 4.4 IMSC manipulation: lock-covered RMW without shadow

Driver: `PL011HwMaskInterrupts` at `PL011hw.cpp:428-476` takes
a mask, a "mask vs. unmask" direction, and an `IsIsrSafe` flag.
When called outside the ISR (`IsIsrSafe=TRUE`) it acquires the
WDF interrupt spin lock via `WdfInterruptAcquireLock` at
`PL011hw.cpp:446-448`, reads UARTIMSC via the fenced read at
`PL011hw.cpp:450`, ORs or ANDs the mask at
`PL011hw.cpp:453-458`, masks against `UART_INTERUPPTS_ALL` at
`PL011hw.cpp:459`, writes back via the fenced write at
`PL011hw.cpp:461`, and releases the interrupt lock at
`PL011hw.cpp:463-465`.

Acquiring the WDF interrupt spin lock (rather than `RegsLock`)
synchronizes the thread-context RMW with the ISR itself, since
the ISR runs under that same lock. There is no software IMSC
shadow; `pl011-spec-requirements.md:171-187` REQ-IRQ-03
recommends one to avoid the device-side read.

### 4.5 ICR write-one-to-clear semantics

Datasheet: UARTICR is W1C; writing a 1 to a bit clears that
pending interrupt (`pl011-minimized-datasheet.md:1172-1192`).

Driver: Two paths write UARTICR:

- `PL011HwClearInterrupts` writes the supplied mask to UARTICR
  directly (no read-then-write, consistent with W1C). It is
  called from `PL011HwInitController` at `PL011hw.cpp:79-82`
  after interrupts are masked, to clear any stale pending
  state.
- The ISR, `PL011pInterruptIsr`, writes the observed UARTRIS
  snapshot to UARTICR at `PL011interrupt.cpp:322-325` to
  acknowledge every interrupt bit it saw this entry. This is
  the only runtime acknowledge path; the DPC does not touch
  UARTICR.

### 4.6 UARTDR / UARTRSR read pairing

Datasheet: The 12-bit FIFO-entry read from UARTDR carries
per-character error flags in bits 11:8; UARTRSR is an
alternate path for the same flags
(`pl011-minimized-datasheet.md:428-445`).

Driver: The driver does not read UARTRSR at any runtime path.
The RX FIFO-drain routine `PL011RxPioFifoCopy` reads UARTDR
with `PL011HwReadRegisterUlongNoFence` and immediately
down-casts to `UCHAR` at `pl011rx.cpp:525-526`, discarding
bits 11:8. Per-character error information therefore does not
propagate through the byte stream. Error reporting comes
instead from the interrupt-status channel: the ISR captures
UARTRIS (Section 4.5), and the DPC's
`PL011DeviceRecordErrors` at `PL011device.cpp:370-401` ORs
the `SERIAL_ERROR_*` flags into
`DevExtPtr->UartErrorTypes` for later retrieval via
`IOCTL_SERIAL_GET_COMMSTATUS`.

Discrepancy / note: Because the driver discards the in-band
error bits from UARTDR and instead relies on UARTRIS, the
datasheet's "read UARTDR, then read UARTRSR" discipline is
moot for this driver. The UARTRSR/ECR register is never
read; it is only cleared, once, during controller init
(`PL011hw.cpp:87`). The correctness of the driver's error
reporting therefore depends on the PL011 asserting the
corresponding error interrupts (FEIS / PEIS / BEIS / OEIS)
for every error, which the datasheet guarantees.

### 4.7 ISR and DPC boundedness

Driver: The ISR (`PL011pInterruptIsr`) performs a fixed set
of bounded operations per entry: a UARTRIS read, one RX
drain via `PL011RxPioFifoCopy`, one TX refill via
`PL011TxPioFifoCopy`, two state-machine
`InterlockedCompareExchange` transitions, and a single
UARTICR write. The DPC (`PL011EvtInterruptDpc` at
`PL011interrupt.cpp:115-225`) performs a second round of
RX drain / TX refill, a constant number of event-bit
checks, and calls into SerCx2 for RX and TX readiness.

The per-call worst case of the drain / refill loops is
bounded by the software staging buffers, not by the
hardware FIFO depth:

- `PL011RxPioFifoCopy` at `pl011rx.cpp:501-533` loops while
  `PL011RxPendingByteCount(rxPioPtr) <
  PL011_RX_BUFFER_SIZE_BYTES` (8 KB per `PL011rx.h:28`) and
  breaks only on `UARTFR_RXFE` at `pl011rx.cpp:505` or
  when the 8 KB software buffer fills. In practice the
  hardware FIFO (32 entries) empties first and the loop
  exits on RXFE; the code-level bound is the software RX
  buffer capacity.
- `PL011TxPioFifoCopy` at `PL011tx.cpp:685-722` loops while
  `PL011TxPendingByteCount(txPioPtr) > 0` (bounded by
  `PL011_TX_BUFFER_SIZE_BYTES` = 16 per `PL011tx.h:27`)
  and breaks on `UARTFR_TXFF` at `PL011tx.cpp:694`.
  The code-level bound is the software TX staging buffer
  capacity.

No ISR or DPC path takes an unbounded number of passes; both
loops are bounded in the small-constant range by the
software staging buffers above them.

### 4.8 Resource-safety and callback-pair symmetry

Driver: `EvtDevicePrepareHardware`
(`PL011device.cpp:199-291`) and `EvtDeviceReleaseHardware`
(`PL011device.cpp:312-349`) are registered as the only
PnP/power callback pair. `PrepareHardware` parses ACPI
resources at `PL011device.cpp:700-867`, maps MMIO and
creates the interrupt object at `PL011device.cpp:894+`,
optionally creates the SerCx2-device interface at
`PL011device.cpp:1024`, and ends by returning
`PL011HwInitController` at `PL011device.cpp:290`.
`ReleaseHardware` at `PL011device.cpp:312-349` unmaps the
MMIO range via `MmUnmapIoSpace`
(`PL011device.cpp:330-333`), zeroes the resource
structure at `PL011device.cpp:344`, and clears
`PL011RegsPtr` and `WdfUartInterrupt` at
`PL011device.cpp:345-346`. The comment at
`PL011device.cpp:336-339` notes that WDF
automatically disconnects the interrupt on release.

Discrepancy / note: `ReleaseHardware` does not call
`PL011HwStopController`, so UARTCR and UARTIMSC are not
explicitly cleared on release; the MMIO is simply unmapped
after the framework disables the interrupt. The device may
therefore be left in whatever UARTCR / UARTIMSC state was
last programmed (typically `UARTEN=1, TXE, RXE, IMSC
enabled`). The SerCx2 receive and transmit objects were
created in `DeviceAdd` at `PL011device.cpp:166` (via
`PL011pDeviceSerCx2Init`), not `PrepareHardware`, and are
tied to the WDFDEVICE lifetime rather than the
Prepare/Release cycle.

### 4.9 MMIO ordering and barriers

Driver: Ordering between register writes is enforced by the
choice of fenced vs. non-fenced MMIO primitive (Section
3.4). Writes that must be ordered with respect to other
writes (IBRD, FBRD, LCR_H) use `WRITE_REGISTER_ULONG` via
the fenced helpers; for example, `PL011HwSetBaudRate` at
`PL011hw.cpp:589-601` uses the fenced writer for all three
divisor/line-control updates. FIFO reads and writes, which
do not require inter-access ordering with unrelated
registers, use the `_NOFENCE_` variants.
`KeMemoryBarrier` is not used in the observed files;
barrier semantics come from the register-access primitives.

### 4.10 Kernel-debugger conflict and stale-load handling

Driver: When the kernel debugger is configured to the same
serial port, `PL011pDeviceParseResources` at
`PL011device.cpp:733-749` sets
`devExtPtr->IsDebuggerConflict = TRUE` (guarded by
`!IS_DONT_CHANGE_HW`). The device does not fail load in
this case; it loads but rejects user-mode Create requests.
The motivation is documented at `PL011device.h:87-93`:
failing load would block `RHPROXY` from loading, so a
stale device object is preferred. This keeps the pin
reservation (FunctionConfig) intact so other drivers
cannot claim the same pins.

### 4.11 State-machine discipline (RX and TX)

Driver: `PL011rx.h:46-59` defines `PL011_RX_PIO_STATE`, and
`PL011tx.h:46-60` defines `PL011_TX_PIO_STATE`. These gate
which callbacks are valid in which state, reflecting the
SerCx2 contract (for example, transmit callbacks must not
write to the hardware FIFO in the "cancelled" state). The
ISR performs atomic state transitions via
`PL011RxPioStateSetCompare` at
`PL011interrupt.cpp:292-296` and
`PL011TxPioStateSetCompare` at
`PL011interrupt.cpp:311-315`; SerCx2 callbacks in
`pl011rx.cpp` / `PL011tx.cpp` do their own transitions
under the `volatile LONG` buffer-lock interlocked
protocols.

### 4.12 "All-known-bits" masks on RMW writes

Datasheet: `pl011-spec-requirements.md:259-287` REQ-CORR-
RESERVED-01 requires that read-modify-write operations on
UARTCR, UARTLCR_H, UARTIFLS, UARTIMSC, UARTICR, and
UARTDMACR preserve reserved bits.

Driver: Two helpers mask the assembled value against an
"all known bits" constant before writing:

- `PL011HwUartControl` ANDs the new UARTCR value against
  `UARTCR_ALL` at `PL011hw.cpp:306` before writing UARTCR
  at `PL011hw.cpp:308-311`.
- `PL011HwMaskInterrupts` ANDs the new UARTIMSC value
  against `UART_INTERUPPTS_ALL` at `PL011hw.cpp:459`
  before writing UARTIMSC at `PL011hw.cpp:461`.

This is not reserved-bit preservation. The operation
forces any bit outside the driver-defined "all known"
mask to zero, rather than preserving whatever the
hardware returned in those positions. The correctness
properties that follow from this are that the driver
(a) will never spuriously set an undefined bit, but
(b) will clear any bit that the hardware or an external
agent may have left set in a reserved position, whether
that bit was architecturally "reserved as zero" or
"reserved for future use."

### 4.13 Variability

- ISR / DPC lock-acquisition patterns do not branch on
  platform or registry; they are structural.
- The `IS_DONT_CHANGE_HW` build flag elides every write
  (`PL011hw.h:419-428`, `PL011hw.h:454-463`), which
  changes the effective state machine: reads still
  happen but the device is not actually reconfigured.
  This is a build-time correctness-relevant
  variability.
- Debugger-conflict handling branches based on both
  debugger presence and FunctionConfig resource
  availability (`PL011device.cpp:733-749`,
  `PL011device.cpp:810-826`), producing two distinct
  load paths with different correctness invariants
  (normal-load vs. stale-load-reject-opens).
- SerCx2 callback behavior is conditioned on
  state-machine values in `PL011_RX_PIO_STATE` and
  `PL011_TX_PIO_STATE`, which themselves transition in
  response to registry-gated capabilities
  (`UartFlowControl`, `UartControlLines`).
