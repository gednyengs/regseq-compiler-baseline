# PL011 FreeBSD Stock Driver Analysis

This document is the work product defined by
`docs/discussions/pl011-full-spec/freebsd-driver-analysis-task-brief.md`.
It describes the functional and architectural capabilities of the ARM
PrimeCell PL011 UART as stated in its datasheet and records which of
those capabilities the FreeBSD stock driver exercises, and how. The
analysis is partitioned along four dimensions (Functional, Integration,
Performance, Correctness), each with a Variability subsection that
records places where the driver branches on PrimeCell revision,
integration binding, or platform quirk.

## 0. Inputs and Reading Guide

### 0.1 Inputs

The analysis below draws exclusively on:

1. ARM PrimeCell UART (PL011) Technical Reference Manual, revision
   r1p5 (ARM DDI 0183G). Cited by section and register mnemonic (e.g.
   "Section 3.3.8, UARTCR"). Page numbers are from the PDF's own
   pagination.
2. `reference-drivers/pl011/freebsd/uart_dev_pl011.c` -- the entire
   FreeBSD stock PL011 device-class module (612 lines, single file).

FreeBSD driver references use the form `uart_dev_pl011.c:<line>` and
name the enclosing function where that helps the reader find the
context.

### 0.2 Reading guide

- Each top-level section answers one question (Functional, Integration,
  Performance, Correctness). A feature that is relevant under more than
  one question (for example, the RX FIFO) appears under each relevant
  section with the section-specific question asked. This is by design
  per the task brief, not duplication.
- The Variability subsection at the end of each dimension lists the
  places where the driver branches on PL011 revision, binding (FDT vs
  ACPI), the BCM2835/RPi compatible-string special case, the
  `arm,primecell-periphid` OF property override, and the ACPI DBG2
  subtype variants, and points back to the feature being conditioned.
- Datasheet claims are cited inline; driver claims are cited inline.

### 0.3 Word-indexed register-offset convention

A single convention governs every driver-side register citation in this
document and must be read before any specific register claim. The
`uart_class` declaration `uart_pl011_class` at `uart_dev_pl011.c:323-331`
sets `.uc_rshift = 2`. Every register access in this file goes through
`__uart_getreg` / `__uart_setreg` (`uart_dev_pl011.c:150-153`), which
routes the register id through `uart_regofs(bas, reg)`. The register
numbers defined at `uart_dev_pl011.c:70-130` -- `UART_DR == 0x00`,
`UART_FR == 0x06`, `UART_IBRD == 0x09`, `UART_FBRD == 0x0a`,
`UART_LCR_H == 0x0b`, `UART_CR == 0x0c`, `UART_IFLS == 0x0d`,
`UART_IMSC == 0x0e`, `UART_RIS == 0x0f`, `UART_MIS == 0x10`,
`UART_ICR == 0x11`, `UART_PIDREG_0 == 0x3f8` ... `UART_PIDREG_3 == 0x3fb`
-- are therefore word indices, not byte offsets. The byte offset to
the device is `reg << 2`: `UART_FR` (0x06) is byte offset 0x18,
matching datasheet Table 3-1's UARTFR at 0x018; `UART_LCR_H` (0x0b) is
0x2C (UARTLCR_H); `UART_CR` (0x0c) is 0x30 (UARTCR); `UART_IFLS` (0x0d)
is 0x34 (UARTIFLS); `UART_IMSC` (0x0e) is 0x38 (UARTIMSC); `UART_RIS`
(0x0f) is 0x3C (UARTRIS); `UART_MIS` (0x10) is 0x40 (UARTMIS);
`UART_ICR` (0x11) is 0x44 (UARTICR); `UART_PIDREG_0..3` (0x3f8..0x3fb)
are 0xFE0, 0xFE4, 0xFE8, 0xFEC (UARTPeriphID0..3). Any apparent offset
mismatch against the datasheet that evaporates after multiplying by 4
is not a discrepancy; it is this convention at work.

## 1. Functional Analysis

Question: which functional capabilities of the PL011 device does the
FreeBSD driver capture, and how does it capture them?

### 1.1 Register map coverage

Datasheet: The programmer's model (Section 3.2, Table 3-1) defines the
register layout: UARTDR at 0x000, UARTRSR/UARTECR at 0x004, UARTFR at
0x018, UARTILPR at 0x020, UARTIBRD at 0x024, UARTFBRD at 0x028,
UARTLCR_H at 0x02C, UARTCR at 0x030, UARTIFLS at 0x034, UARTIMSC at
0x038, UARTRIS at 0x03C, UARTMIS at 0x040, UARTICR at 0x044, UARTDMACR
at 0x048, and the PeriphID / PCellID registers at 0xFE0-0xFFC.

Driver: The driver defines word-indexed constants for the subset of
registers it uses at `uart_dev_pl011.c:70-130`: UARTDR, UARTFR,
UARTIBRD, UARTFBRD, UARTLCR_H, UARTCR, UARTIFLS, UARTIMSC, UARTRIS,
UARTMIS, UARTICR, and UARTPeriphID0-3. UARTRSR/UARTECR, UARTILPR, and
UARTDMACR are not defined and never accessed. `uc_range = 0x48` at
line 328 declares the MMIO window size to the framework (byte offset
0x48 reaches the last datasheet register UARTDMACR, even though the
driver never touches it). The declared window does not extend to the
ID registers; access to `UART_PIDREG_*` is through the same
`bus_space_read_4` path and is assumed to land within whatever mapping
the `bus_space` layer provides.

Discrepancy: The datasheet (page 3-23, Section 3.3.15) states that
accesses to the Peripheral Identification Registers "must be 32-bit,
using the LDR and STR instructions". The driver satisfies this rule by
construction: every MMIO access in the file is a `bus_space_read_4` /
`bus_space_write_4` (`uart_dev_pl011.c:150-153`). `UART_PIDREG_2` is
read through that same 32-bit path at `uart_dev_pl011.c:502`, so the
32-bit discipline is inherited from the MMIO helper rather than
asserted explicitly.

### 1.2 Baud-rate programming (IBRD / FBRD / LCR_H)

Datasheet: Section 3.3.5 (UARTIBRD), Section 3.3.6 (UARTFBRD), and
Section 2.4.3 ("UART operation", "Fractional baud rate divider") define
the 22-bit divisor as BAUDDIV = F_UARTCLK / (16 x Baud rate), split
between a 16-bit integer (UARTIBRD, bits 15:0) and a 6-bit fractional
part (UARTFBRD, bits 5:0). The UARTLCR_H write latches the composite
UARTLCR internally, so IBRD/FBRD updates require an LCR_H write
afterwards (page 3-14, note after Table 3-11).

Driver: `uart_pl011_param` at `uart_dev_pl011.c:182-237` is the
baud-programming helper. The integer/fractional split is computed as
`baud = bas->rclk * 4 / baudrate` at line 224, then
`UART_IBRD = (baud >> 6) & IBRD_BDIVINT` and
`UART_FBRD = baud & 0x3F` at lines 225-226. The factor of 4 folds the
16x oversampling into the divisor: `rclk * 4 / baudrate` computed in
baud16 units yields the integer and fractional 6-bit partition in one
shift. The LCR_H write that completes the sequence is at lines
230-231, where the existing LCR_H is read, its low 8 bits cleared, and
the computed `line` configuration ORed back in.

Call-site coverage: `uart_pl011_param` runs from `uart_pl011_init`
(the low-level `uart_ops.init` hook at lines 239-248) and from
`uart_pl011_bus_param` (the high-level kobj method at lines 447-457).

### 1.3 Line configuration: word length, parity, stop bits, FIFO enable

Datasheet: UARTLCR_H (Section 3.3.7, Table 3-10) carries WLEN at bits
6:5 (b00=5, b01=6, b10=7, b11=8), FEN at bit 4, STP2 at bit 3, EPS at
bit 2, PEN at bit 1, BRK at bit 0, SPS at bit 7. The parity truth
table is Table 3-11.

Driver: `uart_pl011_param` at `uart_dev_pl011.c:196-218` composes a
local `line` word for LCR_H:

- Data bits: switch on the `databits` argument at lines 196-207:
  case 7 -> `LCR_H_WLEN7`, case 6 -> `LCR_H_WLEN6`, default -> 8
  (`LCR_H_WLEN8`). 5-bit framing (`b00`) is never selected.
- Stop bits: at lines 209-212, `stopbits == 2` sets `LCR_H_STP2`,
  otherwise it is cleared (1 stop bit).
- Parity: at lines 214-217, nonzero `parity` sets `LCR_H_PEN`,
  zero clears it. The `LCR_H_EPS` constant is defined at
  `uart_dev_pl011.c:94` but never set by the driver; `LCR_H_SPS` is
  not defined at all. This means the driver treats all nonzero
  parity requests as odd parity (PEN=1, EPS=0, SPS=0 -> odd, per
  Table 3-11), regardless of what the caller intended.
- FIFO enable: `line |= LCR_H_FEN` at line 218. FIFO mode is always
  on after a successful param; the driver never drives FEN=0.
- BRK: not set. `LCR_H_BRK` is not defined in this file at all; the
  driver does not generate break conditions.

The final write at lines 230-231 preserves the upper 8 bits of
LCR_H (which are reserved per Table 3-10) by masking only the low 8
and ORing `line` in.

Discrepancy: The datasheet supports 5, 6, 7, and 8-bit framing (Table
3-10, WLEN field). The driver supports only 6, 7, and 8, with 8 as
the fallback default. Parity is constrained to "off" or "odd only":
even / stick-parity-0 / stick-parity-1 are unreachable because EPS
and SPS are never written. Neither limitation is a bug in the
sense of misprogramming the device; they are capabilities the driver
simply does not expose.

### 1.4 TX path: polled `putc` and interrupt-driven `bus_transmit`

Datasheet: Section 3.3.3 (UARTFR) defines TXFF (bit 5, TX FIFO full)
and TXFE (bit 7, TX FIFO empty). Section 3.3.1 (UARTDR) defines the
data register; a write pushes a byte into the TX FIFO (or the
holding register when FEN=0). Section 2.8.3 (UARTTXINTR) specifies
the TX interrupt as "transition through a level": asserted when the
TX FIFO fill level drops to or below the programmed TXIFLSEL
threshold.

Driver: Two TX paths exist.

- `uart_pl011_putc` at `uart_dev_pl011.c:255-263` is the polled
  low-level primitive exposed through `uart_ops.putc`. It busy-waits
  on `FR_TXFF` at lines 260-261 and then writes `c & 0xff` to
  `UART_DR` at line 262. The mask `0xff` truncates the argument to
  eight bits even though the device's TX path is fixed 8-bit anyway
  (UARTDR DATA field is bits 7:0, Table 3-2).
- `uart_pl011_bus_transmit` at `uart_dev_pl011.c:559-582` is the
  interrupt-driven bulk TX path. It iterates `i` from 0 to
  `sc->sc_txdatasz`, writing each byte with
  `__uart_setreg(bas, UART_DR, sc->sc_txbuf[i])` at line 571 and
  issuing `uart_barrier(bas)` at line 572 between writes. After the
  burst, it sets `sc->sc_txbusy = 1` at line 576 and restores the
  cached `psc->imsc` (which contains `UART_TXEMPTY`) at line 577 to
  re-enable the TX-empty interrupt. The entire body runs under
  `sc_hwmtx` (acquired at line 568, released at line 579).

The framework guarantees that `sc_txdatasz` is bounded by the TX FIFO
headroom the driver published at probe time via `sc_txfifosz` (see
Section 1.9), so the burst fits in the FIFO without FR_TXFF
polling. The driver therefore relies on the framework's buffering
contract rather than inspecting TXFF during the interrupt-driven
path.

### 1.5 RX path: polled `getc` / `rxready` and interrupt-driven `bus_receive`

Datasheet: UARTFR (Section 3.3.3) defines RXFE (bit 4, RX FIFO empty).
UARTDR (Section 3.3.1, Table 3-2) carries the received byte in bits
7:0 plus error flags FE (bit 8), PE (bit 9), BE (bit 10), OE (bit 11).
Section 2.8.2 (UARTRXINTR) specifies that the RX interrupt fires when
the RX FIFO level passes through RXIFLSEL. Section 2.8.4 (UARTRTINTR)
adds a receive-timeout interrupt that fires when the RX FIFO is
non-empty and no new data has arrived for 32 bit-periods.

Driver: Three RX entry points.

- `uart_pl011_rxready` at `uart_dev_pl011.c:265-270` returns
  `!(UART_FR & FR_RXFE)`: true if at least one byte is available.
- `uart_pl011_getc` at `uart_dev_pl011.c:272-282` busy-waits on
  `rxready` at line 277 and returns `UART_DR & 0xff` at line 279.
  The `&0xff` discards any FE/PE/BE/OE status bits; the polled path
  therefore does not surface UART error bits to callers. The
  `hwmtx` argument declared at line 273 is unused in the body (a
  finding -- see Section 4.1).
- `uart_pl011_bus_receive` at `uart_dev_pl011.c:517-550` is the
  interrupt-driven RX drain. The loop at lines 527-545 reads
  `UART_FR` at 528, breaks on `FR_RXFE` at 529, breaks on
  `uart_rx_full(sc)` at 531 after posting an overrun marker into
  `sc->sc_rxbuf[sc->sc_rxput]` at 532, and otherwise reads the data
  register at 536, extracting the byte at 537 and folding FE/PE
  into a combined status word at 539-542 before handing the result
  to `uart_rx_put(sc, rx)` at 544.

Error-bit handling in `bus_receive` consults only `DR_FE` and
`DR_PE`. The constants `DR_BE` (1 << 10) and `DR_OE` (1 << 11) are
defined at `uart_dev_pl011.c:73-74` but never tested on the DR path.
Break and overrun detection is instead routed through the masked
interrupt status register (Section 1.6). This is a deliberate split,
not an oversight; see Section 4.3 for the correctness consequence.

Discrepancy: The polled `uart_pl011_getc` discards error bits by
masking `& 0xff` at line 279; any FE/PE/BE/OE reported by the device
on a polled read is lost. The datasheet (page 3-7, Table 3-2) places
the errors at bits 8-11 of UARTDR and (page 3-7, "Note") warns that
reading UARTDR is what causes the associated UARTRSR bits to update.
In the polled path the driver therefore cannot distinguish a valid
byte from one received with a framing or parity error.

### 1.6 Interrupt management

Datasheet: UARTIMSC (Section 3.3.10, Table 3-14) is the mask
set/clear register; bits 0-10 correspond to the individual interrupt
sources, with 1 enabling the interrupt. UARTMIS (Section 3.3.12) is
the masked interrupt status (RIS & IMSC), and UARTICR (Section
3.3.13) clears interrupts on write-one. The RX (bit 4), TX (bit 5),
RT (bit 6), FE (bit 7), PE (bit 8), BE (bit 9), OE (bit 10) bits map
to the named events.

Driver: The driver reflects a subset of these bits through its own
constants at `uart_dev_pl011.c:112-125`: `UART_RXREADY` (1 << 4),
`UART_TXEMPTY` (1 << 5), `RIS_RTIM` (1 << 6), `RIS_FE` (1 << 7),
`RIS_PE` (1 << 8), `RIS_BE` (1 << 9), `RIS_OE` (1 << 10), and
`IMSC_MASK_ALL = 0x7ff` at line 113. The softc caches the written
mask in `psc->imsc` (declared at line 289 of
`struct uart_pl011_softc`).

- Baseline enable at attach: `uart_pl011_bus_attach` at
  `uart_dev_pl011.c:351-368` sets
  `psc->imsc = (UART_RXREADY | RIS_RTIM | UART_TXEMPTY)` at line
  361, writes that to UARTIMSC at 362, and clears all pending
  interrupts by writing `IMSC_MASK_ALL` to UARTICR at 365.
  Modem, framing, parity, break, and overrun interrupts are left
  masked in IMSC; break and overrun are still observable through
  UARTMIS because they are always present in RIS, and UARTMIS is
  RIS & IMSC -- except that only unmasked bits appear in MIS.
  That means break and overrun will *not* appear in UARTMIS
  either, which the driver inspects from `bus_ipend`. This is
  described below.
- Mask-all at `init`: `uart_pl011_init` at lines 239-248 clears
  every bit in IMSC (line 244-245) by reading IMSC and writing it
  back with `~IMSC_MASK_ALL`. This runs before `uart_pl011_param`
  configures the line parameters.
- Mask-all on `grab`: `uart_pl011_bus_grab` at lines 584-597
  writes `psc->imsc & ~IMSC_MASK_ALL` to UARTIMSC, disabling all
  interrupts so that kdb (or any polled-mode consumer) can drive
  `getc` / `rxready` without spurious wakeups. The cached
  `psc->imsc` is preserved for restoration by `ungrab`.
- Restore on `ungrab`: `uart_pl011_bus_ungrab` at lines 599-612
  writes `psc->imsc` back to UARTIMSC.
- `ipend` translation: `uart_pl011_bus_ipend` at lines 413-445
  reads UARTMIS into a local `ints`, and then maps the hardware
  bits into the framework's `SER_INT_*` flags:

    ```
    if (ints & (UART_RXREADY | RIS_RTIM))  ipend |= SER_INT_RXREADY;
    if (ints & RIS_BE)                      ipend |= SER_INT_BREAK;
    if (ints & RIS_OE)                      ipend |= SER_INT_OVERRUN;
    ```

  There are no `RIS_FE` / `RIS_PE` branches: framing and parity
  errors are not surfaced from `ipend` because the driver folds
  them into per-byte status in `bus_receive` (Section 1.5).

  Observation: because `psc->imsc` does not enable `RIS_BE` / `RIS_OE`
  at attach time (line 361 unmasks only `UART_RXREADY | RIS_RTIM |
  UART_TXEMPTY`), those bits will not normally appear in UARTMIS
  (UARTMIS = UARTRIS & UARTIMSC, Section 3.3.12). The `RIS_BE` /
  `RIS_OE` branches in `ipend` therefore execute only when another
  component of the system has enabled those mask bits (for example,
  a future code path that edits `psc->imsc`). In the shipped
  configuration, neither `bus_ipend` nor `bus_receive` surfaces
  hardware break or hardware overrun to the framework: the ipend
  branches are dormant, and the DR read path in `bus_receive` does
  not consume `DR_BE` / `DR_OE`. The `UART_STAT_OVERRUN` marker
  posted by `bus_receive` at line 532 is a distinct, software-level
  overrun signalling that the driver's own RX ring was full
  (`uart_rx_full`), not a hardware overrun.

- Lazy TX interrupt gating in `ipend`: when `UART_TXEMPTY` is
  observed at line 434-440, the driver sets `SER_INT_TXIDLE` only
  if `sc->sc_txbusy` is set, and unconditionally masks
  `UART_TXEMPTY` out of the written IMSC at line 439:
  `__uart_setreg(bas, UART_IMSC, psc->imsc & ~UART_TXEMPTY)`.
  Note that `psc->imsc` itself is not modified -- only the value
  written to the hardware register is. `bus_transmit` at line 577
  writes `psc->imsc` back, re-enabling `UART_TXEMPTY`. This is the
  three-part discipline described in the task brief: attach-time
  enable, `ipend`-time mask-out on fire, `bus_transmit`-time
  re-enable from cached mask. See Section 3.4 for why.

### 1.7 Error handling

Datasheet: UARTDR bits 8-11 (Table 3-2) carry FE, PE, BE, OE for the
character at the top of the FIFO; OE is a sideband flag that is set
immediately on FIFO overflow. UARTRIS / UARTMIS (Sections 3.3.11 /
3.3.12) also expose these conditions at bits 7 (FE), 8 (PE), 9 (BE),
10 (OE) as interrupt sources. UARTICR (Section 3.3.13) clears them
on write-one.

Driver:

- Inline from UARTDR in `bus_receive` at `uart_dev_pl011.c:539-542`:
  `DR_FE` produces `UART_STAT_FRAMERR` and `DR_PE` produces
  `UART_STAT_PARERR`, OR-ed into the per-byte `rx` word passed to
  `uart_rx_put`. FE and PE are therefore live per-byte handling.
- From UARTMIS in `bus_ipend` at `uart_dev_pl011.c:430-433`:
  `RIS_BE` -> `SER_INT_BREAK`, `RIS_OE` -> `SER_INT_OVERRUN`.
  However, BEIM and OEIM are never set in `psc->imsc` (line 361
  unmasks only `UART_RXREADY | RIS_RTIM | UART_TXEMPTY`), so under
  Section 3.3.12 (UARTMIS = UARTRIS & UARTIMSC) those two ipend
  branches are dormant in the shipped configuration.
- `DR_BE` (line 73) and `DR_OE` (line 74) are defined but never
  consulted on the DR read path in `bus_receive`. The net effect
  is that FE and PE are live per-byte DR handling, while BE and OE
  have defined-but-unused DR bits and dormant MIS branches. The
  driver source does not document a rationale for this asymmetry,
  and this analysis does not infer one.
- The driver never writes to UARTICR to clear FE/PE/BE/OE
  explicitly except for the attach-time blanket clear at line 365
  (write `IMSC_MASK_ALL` to UARTICR). The datasheet permits
  individual clears; the driver only ever does the all-bits clear.
- The driver also never reads UARTRSR/UARTECR; it does not define
  those mnemonics. Error status is always taken from UARTDR on
  the read path or from UARTMIS on the interrupt path.

Discrepancy: The datasheet's "note" on page 3-7 observes that the
received data character must be read from UARTDR before reading the
error status from UARTRSR. Because the driver never reads UARTRSR,
the ordering constraint is vacuously satisfied. The in-band FE/PE
path through UARTDR is the datasheet's other supported access mode
and is used correctly.

### 1.8 Flow control, modem control, break transmission

Datasheet: The PL011 supports RTS/CTS hardware flow control via
UARTCR.CTSEn (bit 15) and UARTCR.RTSEn (bit 14) (Section 3.3.8,
Table 3-12). It supports modem-status outputs DTR, RTS, Out1, Out2
at UARTCR bits 10-13, and modem-status inputs CTS, DSR, DCD, RI
readable from UARTFR bits 0, 1, 2, 8 (Section 3.3.3, Table 3-4).
Break generation is via UARTLCR_H.BRK (bit 0, Section 3.3.7,
Table 3-10); the datasheet requires the BRK bit be held set for at
least two complete frames.

Driver:

- Hardware flow control: never enabled. `uart_pl011_param` writes
  UARTCR as `CR_RXE | CR_TXE | CR_UARTEN` at line 221, with
  CTSEn and RTSEn both zero. The driver defines no constants for
  CTSEn / RTSEn and provides no ioctl to change them.
- Modem signal read: `uart_pl011_bus_getsig` at lines 384-389
  returns 0 unconditionally. The driver never reads UARTFR bits
  0, 1, 2, 8 to report modem status.
- Modem signal set: `uart_pl011_bus_setsig` at lines 552-557
  returns 0 unconditionally. The driver never writes UARTCR bits
  10-13. RTS / DTR / Out1 / Out2 are not driven.
- Break transmission: `uart_pl011_bus_ioctl` at lines 391-411
  handles `UART_IOCTL_BREAK` at line 399 with an empty case body
  (`case UART_IOCTL_BREAK: break;`). The datasheet's BRK bit is
  never set by this driver.

Discrepancy: All three are capabilities the device provides and the
driver declines to expose. This is not a datasheet disagreement
(the driver does not mis-program the device); it is a scope
limitation. See Section 2.6 for the integration-contract view of
the same facts.

### 1.9 Hardware-revision detection and FIFO sizing

Datasheet: UARTPeriphID2 (Section 3.3.15, Table 3-21) carries the
revision in bits 7:4, with r1p0 -> 0x0, r1p1 -> 0x1, r1p3/r1p4 ->
0x2, r1p5 -> 0x3. The datasheet defines no FIFO-size negotiation
protocol; revision-dependent FIFO depth (16 for r2 and earlier, 32
for r3+) is a platform convention the driver must apply externally.

Driver: `uart_pl011_bus_probe` at `uart_dev_pl011.c:491-515` resolves
the revision and publishes FIFO sizes. The resolution order is:

1. If `IS_FDT` is true, attempt `uart_pl011_bus_hwrev_fdt`
   (`uart_dev_pl011.c:459-489`). That helper first checks the
   compatible-string for BCM2835/RPi via `ofw_bus_is_compatible`
   for both `"brcm,bcm2835-pl011"` and `"broadcom,bcm2835-uart"`
   (lines 476-477); if matched, it returns `2`, forcing the r2
   FIFO sizes regardless of what PeriphID2 says. (The in-source
   comment at lines 466-475 explains that RPi reports rev 3 in
   hardware but actually has 16-byte FIFOs.) Otherwise, it reads
   the OF property `arm,primecell-periphid` via `OF_getencprop`
   at lines 481-482; if present, it extracts the revision nibble
   via `(periphid >> 20) & 0x0f` at line 483. If no OF override
   is present, it returns -1.
2. If the FDT path returned -1 (or the platform is not FDT), the
   driver falls back to reading UARTPeriphID2 directly via
   `__uart_getreg(&sc->sc_bas, UART_PIDREG_2) >> 4` at line 502.

The resolved revision determines FIFO sizes at lines 504-510:
hwrev <= 2 -> `FIFO_RX_SIZE_R2=16` / `FIFO_TX_SIZE_R2=12`;
hwrev >= 3 -> `FIFO_RX_SIZE_R3=32` / `FIFO_TX_SIZE_R3=24` (constants
at `uart_dev_pl011.c:141-144`). These are written to
`sc->sc_rxfifosz` and `sc->sc_txfifosz` so the framework can size
its ring buffers and bound `sc_txdatasz` accordingly. The probe
also sets the device description string to `"PrimeCell UART
(PL011)"` at line 512.

Note: the TX-size constants (12, 24) are deliberately smaller than
the hardware capacity (16, 32). The in-source comment at lines
132-140 explains that TX-size is published as the capacity the
driver can guarantee will be empty when the TX interrupt fires at
the 2/8 threshold (Section 3.4); see Section 3.3 for the
performance framing.

Discrepancy: The datasheet's PeriphID2 revision encoding is
per-revision (r1p0 = 0x0, r1p1 = 0x1, r1p3/r1p4 = 0x2, r1p5 = 0x3).
The driver collapses this to a two-way split at `hwrev <= 2`; it
does not distinguish r1p3 from r1p4, or r1p0 from r1p1 from r1p2.
For the published FIFO-size decision this is sufficient because
the FIFO-depth change happens at the r1p3 -> r1p5 transition; but
the code loses fidelity against the full datasheet encoding.

### 1.10 Stubbed / no-op functional hooks

The driver exposes several functional hooks that return immediately.
Each is recorded here for completeness.

- `uart_pl011_term` at `uart_dev_pl011.c:250-253`: empty body. The
  low-level `term` hook is the counterpart to `init`; the driver
  declines to undo anything on term.
- `uart_pl011_bus_detach` at lines 370-375: returns 0. See Section
  2.7 for the integration view.
- `uart_pl011_bus_flush` at lines 377-382: returns 0. The datasheet
  supports TX-FIFO flush via toggling UARTLCR_H.FEN (page 3-14, note
  on "FIFO integrity"); the driver does not implement this.
- `uart_pl011_bus_getsig` / `uart_pl011_bus_setsig` (already covered
  in Section 1.8): return 0.
- `UART_IOCTL_BREAK` in `uart_pl011_bus_ioctl` (Section 1.8): empty
  case body.

### 1.11 Variability (Functional)

- Hardware-revision branch. `uart_pl011_bus_probe` at lines 491-515
  branches on revision number (<=2 vs >=3) to select FIFO sizes
  (`FIFO_{RX,TX}_SIZE_{R2,R3}`). The branch is reached from either
  an FDT-override path (Section 1.9 step 1) or the in-register
  fallback (Section 1.9 step 2).
- FDT vs ACPI binding selection. `IS_FDT` at
  `uart_dev_pl011.c:61-67` resolves to `arm64_bus_method ==
  ARM64_BUS_FDT` on `__aarch64__`, to 1 when only `FDT` is defined,
  and triggers `#error Unsupported configuration` otherwise. Only
  the FDT path invokes `uart_pl011_bus_hwrev_fdt`; the ACPI path
  always uses the PIDREG-2 fallback.
- BCM2835/RPi special case. `uart_pl011_bus_hwrev_fdt` at lines
  476-478 forces the result to `2` whenever the compatible string
  matches `"brcm,bcm2835-pl011"` or `"broadcom,bcm2835-uart"`.
  Exists because RPi reports r3 in PeriphID2 but has 16-byte
  FIFOs (see in-source comment).
- `arm,primecell-periphid` OF property override. Same helper, at
  lines 481-484: if the OF node has that property, the driver
  extracts `(periphid >> 20) & 0x0f` and treats it as the revision.
  This lets a board override the in-register value when the
  hardware's PeriphID2 is wrong or unreadable.
- ACPI DBG2 subtype variants. `acpi_compat_data` at
  `uart_dev_pl011.c:342-347` lists three DBG2 subtypes --
  `ACPI_DBG2_ARM_PL011`, `ACPI_DBG2_ARM_SBSA_GENERIC`,
  `ACPI_DBG2_ARM_SBSA_32BIT` -- all bound to the same
  `uart_pl011_class`. The driver does not branch on the subtype
  at runtime; the binding is one-way and the three entries exist
  only to be matched by the ACPI enumerator.

## 2. Integration Analysis

Question: which FreeBSD kernel subsystems does the stock driver
integrate the PL011 device into, and how?

### 2.1 Two-layer `uart(4)` surface

The driver implements both of the `uart(4)` framework's contracts.

- Low-level `uart_ops` (console / early-console / polling /
  debugger). Declared as `struct uart_ops uart_pl011_ops` at
  `uart_dev_pl011.c:165-172` with six function pointers: `.probe`
  = `uart_pl011_probe` (lines 174-179, returns 0 unconditionally),
  `.init` = `uart_pl011_init` (lines 239-248), `.term` =
  `uart_pl011_term` (lines 250-253, empty), `.putc` =
  `uart_pl011_putc` (lines 255-263), `.rxready` =
  `uart_pl011_rxready` (lines 265-270), `.getc` =
  `uart_pl011_getc` (lines 272-282). These are the entry points
  the console and kernel debugger call directly, with no softc;
  they take `struct uart_bas *` and a mutex pointer.
- High-level kobj-based `uart_class` methods. Declared as
  `uart_pl011_methods[]` at `uart_dev_pl011.c:306-321` using
  `KOBJMETHOD`. The thirteen kobj contract names are: `uart_attach`,
  `uart_detach`, `uart_flush`, `uart_getsig`, `uart_ioctl`,
  `uart_ipend`, `uart_param`, `uart_probe`, `uart_receive`,
  `uart_setsig`, `uart_transmit`, `uart_grab`, `uart_ungrab`.
  Each is bound to the corresponding `uart_pl011_bus_*` C
  function. These are the kobj contract names, not the newbus
  `bus_probe` / `bus_attach` / `bus_detach` methods: the
  `bus_*` prefix in the C function names is a naming convention
  internal to this file.

### 2.2 `uart_class` declaration and softc

`struct uart_class uart_pl011_class` at `uart_dev_pl011.c:323-331`
wires both surfaces together and announces integration parameters:

- `"uart_pl011"` (line 324) is the class name.
- `uart_pl011_methods` (line 325) is the kobj method table.
- `sizeof(struct uart_pl011_softc)` (line 326) is the
  per-instance softc size.
- `.uc_ops = &uart_pl011_ops` (line 327) is the low-level ops
  table.
- `.uc_range = 0x48` (line 328) is the MMIO window size in bytes
  (byte offset 0x48 reaches UARTDMACR per Table 3-1; the driver
  never touches UARTDMACR).
- `.uc_rclk = 0` (line 329): the reference-clock rate is supplied
  externally (by the bus enumeration).
- `.uc_rshift = 2` (line 330) is the word-indexed offset shift
  described in Section 0.3.

`struct uart_pl011_softc` at `uart_dev_pl011.c:287-290` adds one
field on top of the framework's `struct uart_softc`:
`uint16_t imsc` (the cached interrupt mask; see Section 1.6).

### 2.3 FDT binding

`UART_FDT_CLASS_AND_DEVICE(fdt_compat_data)` at
`uart_dev_pl011.c:338` registers the class as an FDT driver. The
compatible table `fdt_compat_data[]` at lines 334-337 lists
`{"arm,pl011", &uart_pl011_class}` plus a sentinel. The
BCM2835/RPi compatibles (`"brcm,bcm2835-pl011"`,
`"broadcom,bcm2835-uart"`) are not in this table; they are
instead detected inside `uart_pl011_bus_hwrev_fdt` (Section 1.9)
and must be matched elsewhere in the platform-FDT stack.

The FDT include path (`<dev/uart/uart_cpu_fdt.h>` and
`<dev/ofw/ofw_bus.h>`) is conditionally included at
`uart_dev_pl011.c:45-48` under `#ifdef FDT`, and the entire FDT
path (`uart_pl011_bus_hwrev_fdt` at lines 459-489 and the
`UART_FDT_CLASS_AND_DEVICE` macro expansion at 338) is guarded by
`#ifdef FDT`. See the `IS_FDT` macro at lines 61-67.

### 2.4 ACPI binding

`UART_ACPI_CLASS_AND_DEVICE(acpi_compat_data)` at
`uart_dev_pl011.c:348` registers the class as an ACPI driver.
`acpi_compat_data[]` at lines 342-347 declares three entries:

- `{"ARMH0011", &uart_pl011_class, ACPI_DBG2_ARM_PL011, ...,
  UART_F_IGNORE_SPCR_REGSHFT, "uart pl011"}`
- `{"ARMH0011", &uart_pl011_class, ACPI_DBG2_ARM_SBSA_GENERIC,
  ..., UART_F_IGNORE_SPCR_REGSHFT, "uart pl011"}`
- `{"ARMH0011", &uart_pl011_class, ACPI_DBG2_ARM_SBSA_32BIT, ...,
  UART_F_IGNORE_SPCR_REGSHFT, "uart pl011"}`

The HID `"ARMH0011"` is the ACPI hardware ID for PL011-class
UARTs. The three DBG2 subtype enums let the ACPI enumerator match
any of the three standardized debug-port variants against this
class. The `UART_F_IGNORE_SPCR_REGSHFT` flag tells the `uart(4)`
framework to ignore the register-shift field in the ACPI SPCR
table (because this class already declares `uc_rshift = 2`, and
the SPCR-supplied value is redundant or wrong on some
platforms).

The ACPI include path (`<dev/uart/uart_cpu_acpi.h>` and the
ACPICA headers) is conditionally included at
`uart_dev_pl011.c:52-57` under `#ifdef DEV_ACPI`, and the
compatibility table plus the registration macro are guarded by
`#ifdef DEV_ACPI` at lines 341-349.

### 2.5 newbus kobj lifecycle (probe / attach / detach)

The three lifecycle contract points, in kobj naming: `uart_probe`,
`uart_attach`, `uart_detach`. The file implements each as a
`uart_pl011_bus_*` function bound through `KOBJMETHOD` in
`uart_pl011_methods`.

- `uart_probe` -> `uart_pl011_bus_probe` at
  `uart_dev_pl011.c:491-515`: resolves hardware revision (Section
  1.9), publishes FIFO sizes to `sc_rxfifosz` / `sc_txfifosz`,
  sets the device description, returns 0.
- `uart_attach` -> `uart_pl011_bus_attach` at lines 351-368:
  initializes `psc->imsc`, writes IMSC, clears pending interrupts,
  returns 0.
- `uart_detach` -> `uart_pl011_bus_detach` at lines 370-375:
  returns 0 with no hardware interaction. The framework is
  responsible for whatever quiescence is required.

### 2.6 Indirect tty integration

The driver does not touch the `tty(4)` layer directly. Received
bytes are posted to the framework's RX ring via `uart_rx_put(sc,
rx)` at `uart_dev_pl011.c:544` inside `bus_receive`; the ring-full
check uses `uart_rx_full(sc)` at line 531 and the overrun-marker
write accesses `sc->sc_rxbuf[sc->sc_rxput]` at line 532 directly.
TX bytes are drained from `sc->sc_txbuf[i]` for `i < sc->sc_txdatasz`
in `bus_transmit` at line 570-573. The framework translates between
these driver-level buffers and the tty subsystem.

### 2.7 Console, early-console, and kdb integration

The low-level `uart_ops` surface (Section 2.1) is the console /
early-console / kdb integration point. The `uart(4)` framework
invokes `.probe` / `.init` / `.term` / `.putc` / `.rxready` /
`.getc` directly through the `struct uart_bas *` handle, without
touching the softc. This is why `uart_pl011_getc` takes an
`hwmtx` argument (a generic per-instance mutex that the framework
*could* pass down) but the PL011 implementation ignores it: the
polled path is expected to be lock-free in kdb context.

The kdb integration further uses the `grab` / `ungrab` kobj
methods:

- `uart_pl011_bus_grab` at `uart_dev_pl011.c:584-597` writes
  `psc->imsc & ~IMSC_MASK_ALL` to UARTIMSC under `sc_hwmtx`,
  silencing all UART interrupts so that polled getc/rxready
  inside kdb are not interrupted by the UART's own IRQ.
- `uart_pl011_bus_ungrab` at lines 599-612 restores
  `psc->imsc` on kdb exit.

The file also includes `<sys/kdb.h>` at `uart_dev_pl011.c:59`.
The include is present, but its precise purpose is not determinable
from this file alone (`kdb_*` symbols from that header are not
referenced in this source); this analysis records the include
without inferring its rationale.

### 2.8 `bus_space` MMIO API

Every MMIO access goes through the `bus_space` helpers. The
wrappers are defined at `uart_dev_pl011.c:150-153`:

```
#define __uart_getreg(bas, reg)   bus_space_read_4((bas)->bst, (bas)->bsh, uart_regofs(bas, reg))
#define __uart_setreg(bas, reg, v) bus_space_write_4((bas)->bst, (bas)->bsh, uart_regofs(bas, reg), v)
```

`uart_regofs(bas, reg)` applies the `uc_rshift` declared on the
class (Section 0.3). The helpers are always 32-bit reads/writes
(`bus_space_read_4` / `bus_space_write_4`). The in-source comment
at lines 147-149 (`FIXME: actual register size is SoC-dependent,
we need to handle it`) flags that for future SoCs with narrower
register widths, this assumption would need to be revisited.

Ordering around `bus_space_write_4` is supplemented by
`uart_barrier(bas)` between successive UARTDR writes in
`bus_transmit` at line 572; see Section 4.4.

### 2.9 Stubbed / no-op kobj contract points

The driver implements but stubs out several kobj methods; each is
an integration-level finding about which contract points the
driver declines to honor.

- `uart_flush` -> `uart_pl011_bus_flush` at lines 377-382:
  returns 0, ignoring the `what` argument. FIFO flushing via
  UARTLCR_H.FEN toggling is not performed.
- `uart_getsig` -> `uart_pl011_bus_getsig` at lines 384-389:
  returns 0. Modem status is never reported.
- `uart_setsig` -> `uart_pl011_bus_setsig` at lines 552-557:
  returns 0. Modem control outputs are never driven.
- `uart_detach` -> `uart_pl011_bus_detach` at lines 370-375:
  returns 0. No hardware quiescence on detach.
- Low-level `term` -> `uart_pl011_term` at lines 250-253: empty.
- `UART_IOCTL_BREAK` inside `uart_pl011_bus_ioctl` at line 399:
  empty case body. Break transmission is not exposed.

All six contract points exist in the kobj method table (or the
`uart_ops` table) and the driver is bound to them, so the
framework will dispatch to these functions, get a successful no-op
return, and behave as if the feature is not present. Absent
contract points (power management, sysctl) are not discussed
here; they are outside the surface this file contracts to.

### 2.10 Variability (Integration)

- Conditional compilation of the FDT path. `#ifdef FDT` guards
  the FDT includes (lines 45-48), the `uart_pl011_bus_hwrev_fdt`
  helper (lines 459-489), the `#ifdef FDT` inside
  `uart_pl011_bus_probe` (lines 497-500), and the FDT
  registration macro (lines 333-339).
- Conditional compilation of the ACPI path. `#ifdef DEV_ACPI`
  guards the ACPI includes (lines 52-57) and the ACPI
  registration table plus macro (lines 341-349).
- `IS_FDT` runtime branch. On `__aarch64__`,
  `IS_FDT == (arm64_bus_method == ARM64_BUS_FDT)` (line 62); on
  non-aarch64 with `FDT` defined, it is 1 (line 64); otherwise
  the build fails (line 66). Used inside
  `uart_pl011_bus_probe` at line 498 to gate the FDT-override
  revision resolution at runtime even when both FDT and ACPI
  paths are compiled in.
- Three ACPI DBG2 subtype entries in `acpi_compat_data` (Section
  2.4). All three bind to the same class; the subtype axis is
  a registration-time enumeration, not a runtime branch.
- The `UART_F_IGNORE_SPCR_REGSHFT` flag (Section 2.4) is a
  variability-absorption mechanism: it tells the framework to
  trust `uc_rshift = 2` from the class rather than whatever SPCR
  says.

## 3. Performance Analysis

Question: which PL011 performance features does the FreeBSD driver
implement, and which FreeBSD-side performance facilities does the
driver use to improve throughput, latency, or CPU cost?

### 3.1 Hardware FIFO enable

Datasheet: UARTLCR_H.FEN (Section 3.3.7, Table 3-10, bit 4) enables
the 32-location TX and RX FIFOs. With FEN=0, each side becomes a
one-byte holding register. FIFOs reduce IRQ rate by amortizing
per-byte overhead across the trigger-level's worth of bytes.

Driver: `uart_pl011_param` sets `line |= LCR_H_FEN` at
`uart_dev_pl011.c:218` unconditionally before writing LCR_H at
lines 230-231. FIFO mode is enabled on every param call and the
driver never disables it.

### 3.2 RX and TX FIFO trigger levels

Datasheet: UARTIFLS (Section 3.3.9, Table 3-13) selects the trigger
levels. RXIFLSEL (bits 5:3) values are 1/8, 1/4, 1/2, 3/4, 7/8;
TXIFLSEL (bits 2:0) values are 1/8, 1/4, 1/2, 3/4, 7/8. RX
interrupt fires when fill level transitions through the trigger;
TX interrupt fires when fill drops through the trigger.

Driver: The driver selects RX = 6/8 (3/4 full) and TX = 2/8 (1/4
full) via the constant `FIFO_IFLS_BITS` at `uart_dev_pl011.c:145`:

```
#define FIFO_IFLS_BITS ((IFLS_LVL_6_8th << IFLS_RX_SHIFT) | (IFLS_LVL_2_8th))
```

with `IFLS_LVL_6_8th == 3` (line 109), `IFLS_LVL_2_8th == 1` (line
107), `IFLS_RX_SHIFT == 3` (line 103), `IFLS_TX_SHIFT == 0` (line
104). `uart_pl011_param` writes this constant to UARTIFLS at line
234. The choice is reasserted on every `param` call.

The asymmetric choice (RX 3/4, TX 1/4) is the canonical serial
trade-off: RX fires near-full to amortize ingress IRQs, TX fires
near-empty to refill before the line goes idle.

### 3.3 FIFO-size publication tied to hardware revision

Datasheet: The r1p5 datasheet (Section 1.1.1 "Features":
"Separate 32x8 transmit and 32x12 receive FIFO memory buffers to
reduce CPU interrupts") describes 32-deep FIFOs. It does not
document a 16-byte configuration for earlier revisions; the
16-vs-32 split is not grounded in the datasheet as given.

Driver: `FIFO_RX_SIZE_R2 = 16`, `FIFO_TX_SIZE_R2 = 12`,
`FIFO_RX_SIZE_R3 = 32`, `FIFO_TX_SIZE_R3 = 24` at
`uart_dev_pl011.c:141-144`. `uart_pl011_bus_probe` (Section 1.9)
publishes these to `sc->sc_rxfifosz` (line 505 or 508) and
`sc->sc_txfifosz` (line 506 or 509).

The published TX size is strictly less than the physical capacity:
12 out of 16 for r2, 24 out of 32 for r3. The in-source comment at
`uart_dev_pl011.c:132-140` states the intent: when the TX
interrupt fires at the 2/8 threshold, the FIFO contains at most
2/8 * capacity bytes, so the empty headroom is 6/8 * capacity --
that is, 12 bytes on 16-byte FIFOs and 24 on 32-byte FIFOs. The
published TX size is the maximum number of bytes `bus_transmit`
can safely write when it gets called from the TX-empty IRQ without
ever polling UARTFR.TXFF. The in-source comment explicitly says:
"uart_core will feed exactly that many bytes to
uart_pl011_bus_transmit() which must consume them all." Under-
publishing the RX side would discard buffer capacity unnecessarily,
so RX uses the full physical size.

### 3.4 Lazy TX interrupt gating

Datasheet: UARTTXINTR (Section 2.8.3) is a transition-through-level
interrupt, asserted when the TX FIFO drops to or below TXIFLSEL. A
common problem: if TXIM is left enabled when the TX FIFO is empty
and idle, the interrupt continues to assert (the level is already
below the threshold), flooding the CPU with spurious wake-ups.

Driver: The driver implements a three-part gating discipline.

1. Attach-time enable (`uart_pl011_bus_attach` at
   `uart_dev_pl011.c:361`): `psc->imsc = (UART_RXREADY | RIS_RTIM
   | UART_TXEMPTY)` with `UART_TXEMPTY` enabled, and the value is
   written to UARTIMSC at line 362.
2. `bus_ipend` mask-out on fire (`uart_pl011_bus_ipend` at lines
   434-440): when `UART_TXEMPTY` is observed in UARTMIS, the
   driver writes `psc->imsc & ~UART_TXEMPTY` to UARTIMSC at line
   439, silencing the TX-empty interrupt in hardware. The
   cached `psc->imsc` is *not* modified -- it still has
   `UART_TXEMPTY` set. The `SER_INT_TXIDLE` flag is returned
   only if `sc->sc_txbusy` was set (line 435-436), preserving
   the ability to observe a genuine TX-idle transition.
3. `bus_transmit` re-enable after pushing bytes
   (`uart_pl011_bus_transmit` at lines 576-577): after setting
   `sc->sc_txbusy = 1`, the driver writes `psc->imsc` to
   UARTIMSC, which contains `UART_TXEMPTY`, re-enabling the
   interrupt. Now the TX FIFO is (freshly) non-empty and will
   fire the interrupt again when it drains to 2/8.

The result: the TX-empty IRQ is enabled only while a transmission
is in flight, masked immediately once the FIFO drains, and
re-enabled on the next TX burst. No spurious interrupts while
idle.

### 3.5 RX timeout coalescing

Datasheet: UARTRTINTR (Section 2.8.4) asserts when the RX FIFO is
non-empty and no new data has arrived for 32 bit-periods. It
coalesces sub-trigger-level RX bursts into a single delivered
interrupt.

Driver: `RIS_RTIM` (bit 6) is enabled at attach time in
`psc->imsc` (line 361) and is combined with `UART_RXREADY` at the
`bus_ipend` translation (line 428): both map to
`SER_INT_RXREADY`. The driver does not distinguish RX-timeout from
RX-fifo-full above the interrupt boundary; both conditions funnel
into the same drain loop in `bus_receive`.

### 3.6 Batched RX drain

Datasheet: Reading UARTDR pops one byte from the RX FIFO (Section
3.3.1). UARTFR.RXFE (Section 3.3.3) indicates whether the FIFO is
empty.

Driver: `uart_pl011_bus_receive` at `uart_dev_pl011.c:517-550`
drains in a `for (;;)` loop at line 527 that checks `UART_FR &
FR_RXFE` at line 528-530 and exits when the FIFO is empty. In a
single IRQ, up to `FIFO_RX_SIZE_{R2,R3}` bytes can be popped.
Each iteration also checks `uart_rx_full(sc)` at line 531 and
stops draining when the framework's RX ring is full, posting an
overrun marker at line 532 before breaking out. The drain runs
the full loop inside a single `uart_lock(sc->sc_hwmtx)` /
`uart_unlock` window (acquired at line 525, released at line
547).

### 3.7 Absence of DMA

Datasheet: Section 2.7 ("UART DMA interface") and Section 3.3.14
(UARTDMACR, Table 3-18) describe the DMA path: TXDMAE (bit 1) and
RXDMAE (bit 0) of UARTDMACR enable the per-direction DMA
request/clear signals. With DMA enabled, bursts of up to the
watermark boundary are transferred without CPU intervention.

Driver: The driver never writes to UARTDMACR. It does not define
the mnemonic `UART_DMACR` at all and has no RXDMAE / TXDMAE
constants. All TX goes through per-byte UARTDR writes (Section
1.4) and all RX goes through per-byte UARTDR reads (Section 1.5).
This is a performance *choice*: the driver accepts per-byte IRQ
cost in exchange for not having to coordinate a separate DMA
controller integration. The datasheet describes a DMA path; the
driver deliberately does not use it.

### 3.8 `uart_barrier` between successive TX writes

The MMIO ordering primitive `uart_barrier(bas)` is called at
`uart_dev_pl011.c:572` between each UARTDR write in
`bus_transmit`. Functionally it ensures the write reaches the
device before the next write is issued (see Section 4.4 for the
correctness discussion). From the performance angle, it caps the
per-byte cost in the TX burst at "one MMIO write plus one barrier"
-- it does not provide any batching or write-combining beyond the
framework's `sc_txdatasz` accounting.

### 3.9 Variability (Performance)

- FIFO-size branch at `uart_pl011_bus_probe` (Section 1.9 / 3.3).
  The published TX size (`sc_txfifosz`) differs by revision: 12
  vs 24. This directly changes the per-IRQ TX throughput ceiling
  and the framework's batching decisions.
- BCM2835/RPi override (Section 1.9). Even when the hardware
  reports r3 in PeriphID2, RPi is forced to the r2 FIFO sizes
  (12 TX, 16 RX). A performance-visible workaround driven by a
  platform quirk.
- `arm,primecell-periphid` OF property override (Section 1.9).
  Lets the device tree correct the hardware's PeriphID2 value,
  which changes which FIFO-size branch is taken.
- The `IFLS` threshold constants `FIFO_IFLS_BITS` (Section 3.2)
  are not variable: RX 6/8 and TX 2/8 are hard-coded for all
  revisions, bindings, and platform quirks.

## 4. Correctness Analysis

Question: what invariants, disciplines, and ordering constraints
govern how the FreeBSD driver exercises the functional mechanisms
safely?

### 4.1 Concurrency discipline and `sc_hwmtx`

The framework supplies a per-instance hardware mutex at
`sc->sc_hwmtx`. `uart_lock` and `uart_unlock` are the framework's
wrappers. The driver's entry points partition into three groups.

Entry points that take `sc_hwmtx`:

- `uart_pl011_bus_ioctl` at `uart_dev_pl011.c:397-408`
  (`uart_lock` at 397, `uart_unlock` at 408).
- `uart_pl011_bus_ipend` at lines 424-442
  (`uart_lock` at 424, `uart_unlock` at 442).
- `uart_pl011_bus_param` at lines 452-454
  (`uart_lock` at 452, `uart_unlock` at 454).
- `uart_pl011_bus_receive` at lines 525-547
  (`uart_lock` at 525, `uart_unlock` at 547).
- `uart_pl011_bus_transmit` at lines 568-579
  (`uart_lock` at 568, `uart_unlock` at 579).
- `uart_pl011_bus_grab` at lines 594-596
  (`uart_lock` at 594, `uart_unlock` at 596).
- `uart_pl011_bus_ungrab` at lines 609-611
  (`uart_lock` at 609, `uart_unlock` at 611).

Entry points that do *not* take `sc_hwmtx`:

- `uart_pl011_bus_attach` at lines 351-368. Runs at device
  attach time; the framework guarantees exclusivity in this
  phase.
- `uart_pl011_bus_detach` at lines 370-375. Empty.
- `uart_pl011_bus_probe` at lines 491-515. Runs before the
  device is registered to receive interrupts; reads PIDREG-2 at
  line 502 lock-free.
- `uart_pl011_bus_flush` at lines 377-382. Empty.
- `uart_pl011_bus_getsig` at lines 384-389. Empty.
- `uart_pl011_bus_setsig` at lines 552-557. Empty.
- Low-level `uart_ops` callbacks:
  - `uart_pl011_probe` at 174-179 (returns 0).
  - `uart_pl011_init` at 239-248. No locking; the comment
    elsewhere is that `init` is called at console-probe time
    before interrupts are routed. Note that it does call
    `uart_pl011_param` (line 247), which writes UARTCR,
    UARTLCR_H, UARTIBRD, UARTFBRD, UARTIFLS -- all lock-free
    here.
  - `uart_pl011_term` at 250-253 (empty).
  - `uart_pl011_putc` at 255-263. No locking; polled busy-wait.
  - `uart_pl011_rxready` at 265-270. No locking; single
    register read.
  - `uart_pl011_getc` at 272-282. Takes an `hwmtx` argument
    (line 273) and does not use it. This is a finding: the
    declared parameter is ignored. The design intent is that
    polled getc in kdb context must not deadlock trying to
    acquire a mutex that the framework might already hold; the
    signature preserves contract compatibility while the body
    elides the mutex acquisition.

The `sc_txbusy` flag at `sc->sc_txbusy` is a framework-owned field
that this driver sets at line 576 in `bus_transmit` (under
`sc_hwmtx`) and consults at line 435 in `bus_ipend` (also under
`sc_hwmtx`). `sc_txbusy` is not cleared in this file. Clearing is
the framework's responsibility and is explicitly out of scope;
here it suffices to note that the set/consult pair is protected by
the mutex, so the driver-side transition from "not busy" to
"busy" is atomic with respect to the interrupt handler.

### 4.2 UARTEN=0 before reprogramming

Datasheet: Section 3.3.8 (UARTCR) on page 3-16 prescribes a
five-step sequence before reprogramming the control register: (1)
disable the UART, (2) wait for the current character to complete,
(3) flush TX via FEN=0, (4) reprogram UARTCR, (5) re-enable. The
same section (page 3-14, after Table 3-11) states that UARTLCR_H,
UARTIBRD, and UARTFBRD "must not be changed when the UART is
enabled".

Driver: `uart_pl011_param` at `uart_dev_pl011.c:182-237`
implements the UARTEN=0 discipline. At lines 192-193 it writes
`ctrl = 0` to UARTCR, clearing UARTEN along with every other
UARTCR bit. It then composes `line` (LCR_H configuration) at
lines 196-218 and `ctrl |= CR_RXE | CR_TXE | CR_UARTEN` at line
221 (preparing the re-enable value, not yet committing it). It
writes UARTIBRD / UARTFBRD at lines 225-226, UARTLCR_H at lines
230-231, UARTIFLS at line 234, and finally commits the prepared
`ctrl` to UARTCR at line 236, re-enabling the UART. Between lines
193 (UARTEN=0) and 236 (UARTEN=1), the UART is disabled, and all
line-configuration writes occur in that window.

Comparison to the datasheet's five-step sequence:

- Step 1 (disable UART): line 193.
- Step 2 (wait for current character): the driver does not
  explicitly poll UARTFR.BUSY. It relies on the semantics
  documented by the datasheet for UARTEN=0 itself -- "If the
  UART is disabled in the middle of transmission or reception,
  it completes the current character before stopping" (Section
  3.3.8, page 3-16). The driver does not observe the completion;
  it proceeds immediately to the reprogramming.
- Step 3 (flush TX via FEN=0): not performed. The driver's
  LCR_H write at lines 230-231 re-enables FIFOs rather than
  flushing them.
- Step 4 (reprogram UARTCR): the re-enable value in `ctrl` at
  line 221 implicitly "programs" UARTCR, but the actual write
  happens last at line 236.
- Step 5 (re-enable): line 236.

Discrepancy: The driver skips step 2 (waiting for the current
character) and step 3 (flushing the TX FIFO). The datasheet's
note after Table 3-11 (page 3-14) observes that "FIFO integrity
is not guaranteed ... if the software disables the UART in the
middle of a transmission with data in the FIFO, and then
re-enables it". The verified source facts are that
`uart_pl011_param` writes `UART_CR = 0` at line 193, does not
poll UARTFR BUSY, writes LCR_H with `LCR_H_FEN` set at line 218
(passed through the read-modify-write at lines 230-231), and
re-enables UARTCR at line 236. Whether a caller quiesces TX
before invoking this path is not determinable from this driver
file; whether the datasheet recommendation is practically violated
in the shipped system therefore cannot be decided from the driver
source alone and is out of scope for this analysis.

### 4.3 IMSC masking discipline

The driver maintains two disciplines around UARTIMSC.

- At `uart_pl011_init` (line 244-245): read UARTIMSC, AND with
  `~IMSC_MASK_ALL`, write back. This masks every interrupt bit
  before `uart_pl011_param` reprograms the device. It runs at
  console-probe time, before the softc exists.
- At `uart_pl011_bus_grab` (line 595): write `psc->imsc &
  ~IMSC_MASK_ALL` to UARTIMSC under `sc_hwmtx`. This silences
  interrupts without modifying the cached `psc->imsc`, so
  `ungrab` can restore by writing `psc->imsc` back.

The `bus_ipend` partial mask-out (line 439, masking only
`UART_TXEMPTY`) and the `bus_transmit` restore (line 577) are the
lazy-TX-gating primitives described in Section 3.4; they are a
restricted form of the same discipline.

### 4.4 `uart_barrier` and MMIO ordering

`uart_barrier(bas)` appears at `uart_dev_pl011.c:572` inside
`bus_transmit`, between successive UARTDR writes. The primitive's
job is to enforce that each write is observed by the device in
program order before the next write is issued. FreeBSD's
`bus_space_write_4` on its own provides a weaker ordering
guarantee than strict program-order visibility, because the
implementation is free to reorder accesses on some architectures.
The explicit barrier makes the write-order contract local to the
driver.

No barrier is issued on the RX drain path (`bus_receive`, lines
517-550): the reads are naturally serialized because each UARTDR
read's effect (popping a byte from the FIFO) must be observed
before the next UARTFR check at the top of the loop sees the
updated state. The data dependency order is sufficient.

### 4.5 Split error-handling paths

Correctness note arising from Section 1.7:

- FE (framing error) and PE (parity error) are consumed
  per-character from UARTDR in `bus_receive` at lines 539-542.
  They travel with the data byte, so the order is:
  UARTFR-check-non-empty -> UARTDR-read -> decode-errors-and-byte
  -> `uart_rx_put`. The reader retrieves byte and status atomically
  because both come out of the single UARTDR read at line 536.
- BE (break) and OE (overrun) have branches in `bus_ipend` at
  lines 430-433, but under Section 3.3.12 UARTMIS reports only
  sources whose mask bits are set in UARTIMSC. The driver never
  sets BEIM or OEIM in `psc->imsc` (line 361 unmasks only RX,
  RX-timeout, and TX-empty), so these `ipend` branches do not
  normally observe anything in the shipped configuration. The
  ordering requirement of the code-as-written is that `UARTMIS`
  is read before the interrupt is acknowledged upstream. The
  driver does not write UARTICR in `bus_ipend`, so if BE or OE
  ever did fire, the interrupt would remain asserted until some
  other path cleared it. For BE and OE specifically, the absence
  of an explicit clear is benign in the shipped configuration
  because those bits are not observable in UARTMIS at all.
- The `DR_BE` (line 73) and `DR_OE` (line 74) constants are
  defined but never read on the DR path in `bus_receive`. The
  analysis records the defined-but-unused state of these
  constants without inferring an intended design.

### 4.6 Observable state-machine invariants per entry point

Per the brief, scoped strictly to what `uart_dev_pl011.c` itself
does.

- `uart_pl011_bus_attach` (lines 351-368): precondition is that
  the device is mapped (`sc->sc_bas.bst` / `bsh` valid) and no
  other entry point is running concurrently (framework guarantee).
  Postcondition: `psc->imsc = (UART_RXREADY | RIS_RTIM |
  UART_TXEMPTY)`, UARTIMSC has the same value, UARTICR has been
  written with `IMSC_MASK_ALL` (so all pending bits cleared).
- `uart_pl011_bus_detach` (lines 370-375): precondition: none.
  Postcondition: none.
- `uart_pl011_bus_probe` (lines 491-515): precondition: bas
  valid, IS_FDT resolvable. Postcondition: `sc->sc_rxfifosz` and
  `sc->sc_txfifosz` set to the R2 or R3 pair; device description
  set.
- `uart_pl011_bus_param` (lines 447-457): precondition: bas
  valid; `sc_hwmtx` acquirable. Postcondition: UART disabled
  during the body but re-enabled before return; IBRD/FBRD/LCR_H/
  IFLS programmed per arguments; FIFO mode enabled;
  UARTCR = `CR_RXE | CR_TXE | CR_UARTEN`. Side effect: any
  in-flight TX beyond the current character may be lost (see
  Section 4.2).
- `uart_pl011_bus_ioctl` (lines 391-411): precondition: sc
  valid. For `UART_IOCTL_BAUD` (line 401-403), writes 115200 to
  `*(int *)data` (regardless of what baud the device was actually
  programmed to -- see Section 4.9). For `UART_IOCTL_BREAK` (line
  399), no-op. For any other request, returns `EINVAL`.
- `uart_pl011_bus_ipend` (lines 413-445): precondition:
  attach completed. Postcondition: when `UART_TXEMPTY` was
  asserted in UARTMIS, `UART_TXEMPTY` has been masked out of the
  value written to UARTIMSC (but `psc->imsc` unchanged); returns
  an OR of `SER_INT_RXREADY`, `SER_INT_BREAK`, `SER_INT_OVERRUN`,
  `SER_INT_TXIDLE` as appropriate.
- `uart_pl011_bus_receive` (lines 517-550): precondition:
  attach completed; `sc_hwmtx` acquirable. Postcondition: the RX
  FIFO has been drained until either `UART_FR & FR_RXFE` is set
  or `uart_rx_full(sc)` is true (with an overrun marker posted at
  line 532 in the latter case). Each drained byte has been
  handed to `uart_rx_put` with any FE/PE bits folded into the
  status word.
- `uart_pl011_bus_transmit` (lines 559-582): precondition:
  `sc_hwmtx` acquirable; `sc->sc_txbuf` contains `sc->sc_txdatasz`
  bytes ready; `sc_txdatasz <= sc_txfifosz` (framework invariant).
  Postcondition: all `sc_txdatasz` bytes have been written to
  UARTDR in order, each write followed by `uart_barrier`;
  `sc_txbusy = 1`; UARTIMSC restored to `psc->imsc` (TX-empty
  re-enabled).
- `uart_pl011_bus_grab` (lines 584-597): precondition: attach
  completed. Postcondition: UARTIMSC has every bit masked;
  `psc->imsc` unchanged.
- `uart_pl011_bus_ungrab` (lines 599-612): precondition:
  attach completed. Postcondition: UARTIMSC = `psc->imsc`.
- `uart_pl011_bus_flush` (lines 377-382): precondition: none.
  Postcondition: none (returns 0; no hardware side effect).
- `uart_pl011_bus_getsig` (lines 384-389): always returns 0.
- `uart_pl011_bus_setsig` (lines 552-557): always returns 0.

### 4.7 Bounded work and resource safety

- `uart_pl011_bus_ipend` (lines 413-445): reads UARTMIS (one
  MMIO read), performs constant-time translation, optionally
  writes UARTIMSC (one MMIO write). Total work is O(1) in the
  hardware state; no loops, no allocations.
- `uart_pl011_bus_receive` (lines 517-550): the drain loop is
  bounded by the RX FIFO depth (<= 32) plus the framework's
  `uart_rx_full` early-exit. Each iteration does two MMIO reads
  (UARTFR, UARTDR) and one function call (`uart_rx_put`); no
  allocations. Terminates on FIFO-empty or ring-full-with-marker.
- `uart_pl011_bus_transmit` (lines 559-582): the loop is
  bounded by `sc->sc_txdatasz`, which is bounded by
  `sc->sc_txfifosz` (<= 24 for r3, <= 12 for r2). Each iteration
  does one MMIO write and one barrier; no allocations.
- All entry points run without blocking calls under `sc_hwmtx`.
  There is no `sleep`, `cv_wait`, `malloc(M_WAITOK)`, or
  file-system call anywhere in the file.
- The handlers invoked in interrupt context (`bus_ipend`,
  `bus_receive`) do not allocate memory. The buffer they write
  into (`sc->sc_rxbuf`) is allocated by the framework outside
  the IRQ path.

### 4.8 Partial-reprogram and recovery

`uart_pl011_param` (lines 182-237) is a multi-step hardware
reprogram. The sequence of MMIO writes is:

1. UARTCR <- 0 (line 193).
2. UARTIBRD <- integer divisor (line 225).
3. UARTFBRD <- fractional divisor (line 226).
4. UARTLCR_H <- `line` (lines 230-231).
5. UARTIFLS <- `FIFO_IFLS_BITS` (line 234).
6. UARTCR <- `CR_RXE | CR_TXE | CR_UARTEN` (line 236).

If the helper is interrupted (for example, by a long stall between
steps) and the caller aborts, the device is left with UARTEN=0
and whatever partial configuration has been written so far. The
driver does not have an explicit rollback path. Because
`uart_pl011_param` runs either under `sc_hwmtx` (from
`uart_pl011_bus_param` at lines 452-454) or lock-free at
`uart_pl011_init` time (before interrupts are routed), the window
in which the helper can be preempted by another PL011-touching
thread is bounded. Nevertheless, if the helper returns before
step 6, the device will remain disabled.

Both call sites are documented:

- Under `sc_hwmtx`: `uart_pl011_bus_param` at lines 452-454
  wraps the call in `uart_lock` / `uart_unlock`. No other
  PL011-touching code in this file can run concurrently.
- Lock-free: `uart_pl011_init` at lines 239-248 calls
  `uart_pl011_param` without any lock, because it runs at
  console-probe time when the framework guarantees exclusivity.

### 4.9 Specific findings

Gathered for reviewer convenience:

- `uart_pl011_getc`'s `hwmtx` argument (line 273) is declared
  and never used.
- `DR_BE` and `DR_OE` constants (lines 73-74) are defined and
  never consulted.
- `RIS_FE` and `RIS_PE` constants (lines 119-120) are defined
  and never consulted (FE/PE flow exclusively through UARTDR).
- `UART_IOCTL_BAUD` (line 401) always reports 115200 regardless
  of what the UART has actually been programmed to. The
  datasheet does not specify this ioctl, so this is a
  driver-level behavior to record without asserting datasheet
  discrepancy.
- `uart_pl011_putc` truncates to 8 bits with `c & 0xff` at line
  262; `bus_receive` truncates to 8 bits with `xc & 0xff` at
  line 537. The datasheet's maximum data-bit count is 8 (Section
  3.3.7, Table 3-10, WLEN = b11), so the truncation is
  consistent; it is visible only if the caller passes a wider
  int (for `putc`) or expects status bits in the low byte (for
  `bus_receive`, which then re-adds FE/PE at 539-542).
- `sc_txbusy` is set at line 576, consulted at line 435, never
  cleared in this file. Clearing is the framework's job; for the
  purpose of this analysis the lack of a clear is noted and left.

### 4.10 Variability (Correctness)

- The `uart_pl011_bus_probe` revision resolution (Section 1.9)
  touches UARTPIDREG_2 (line 502) lock-free and before the
  IRQ is routed. There is no concurrency concern for the probe
  fallback.
- The FDT-vs-ACPI binding selection (`IS_FDT`) does not change
  the correctness invariants of the hot paths (`bus_ipend`,
  `bus_receive`, `bus_transmit`); all three run the same locked
  discipline regardless of how the class was enumerated.
- The BCM2835/RPi override in `uart_pl011_bus_hwrev_fdt` forces
  the R2 FIFO sizes. This is correctness-relevant because the
  framework's `sc_txdatasz` bound follows `sc_txfifosz`; a wrong
  override would let `bus_transmit` write past the TX headroom
  and lose bytes. The override is therefore a correctness
  preservation, not a correctness concern.
- The `UART_F_IGNORE_SPCR_REGSHFT` flag on the ACPI path
  (Section 2.4) is also correctness-preserving: without it, the
  framework would apply the SPCR-supplied register shift on top
  of `uc_rshift = 2`, and every register access would address the
  wrong byte offset.
- Neither the DBG2 subtype nor the OF `arm,primecell-periphid`
  override changes the correctness invariants. Both are
  resolution-time data that feeds into FIFO-size publication.
