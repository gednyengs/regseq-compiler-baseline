# PL011 Linux Stock Driver Analysis

This document is the work product defined by
`docs/discussions/pl011-full-spec/linux-driver-analysis-task-brief.md`.
It describes the functional and architectural capabilities of the ARM
PrimeCell PL011 UART as stated in its datasheet and records which of those
capabilities the Linux stock driver exercises, and how. The analysis is
partitioned along four dimensions (Functional, Integration, Performance,
Correctness), each with a Variability subsection that records places where
the driver branches on PrimeCell revision, integration variant, or vendor
quirk.

## 0. Inputs and Reading Guide

### 0.1 Inputs

The analysis below draws exclusively on:

1. ARM PrimeCell UART (PL011) Technical Reference Manual, revision r1p5
   (ARM DDI 0183G). Cited by section and register mnemonic (e.g.
   "Section 3.3.8, UARTCR"). Page numbers are from the PDF's own
   pagination.
2. `reference-drivers/pl011/linux/serial_pl011.h` -- the register-offset
   and bitfield header.
3. `reference-drivers/pl011/linux/amba-pl011.c` -- the main driver source.

Linux driver references use the form `amba-pl011.c:<line>` or
`serial_pl011.h:<line>`, and name the enclosing function where that helps
the reader find the context.

### 0.2 Reading guide

- Each top-level section answers one question (Functional, Integration,
  Performance, Correctness). A feature that is relevant under more than
  one question (for example, the RX FIFO) appears under each relevant
  section with the section-specific question asked. This is by design per
  the task brief, not duplication.
- The Variability subsection at the end of each dimension lists the
  places where the driver branches on PL011 revision, integration variant
  (ARM, ST Micro, NVIDIA, SBSA, ZTE, Qualcomm QDF2400-E44), or vendor
  quirk, and points back to the feature being conditioned.
- Datasheet claims are cited inline; driver claims are cited inline. Any
  claim that cannot be grounded in these sources is either removed or
  marked as a hypothesis.

## 1. Functional Analysis

Question: which functional capabilities of the PL011 device does the
Linux driver capture, and how does it capture them?

### 1.1 Register map coverage and indirection

Datasheet: The programmer's model (Section 3.2, Table 3-1) lays out the
UART register file at fixed offsets within a 4KB window: UARTDR at 0x000,
UARTRSR/UARTECR at 0x004, UARTFR at 0x018, UARTILPR at 0x020, UARTIBRD at
0x024, UARTFBRD at 0x028, UARTLCR_H at 0x02C, UARTCR at 0x030, UARTIFLS
at 0x034, UARTIMSC at 0x038, UARTRIS at 0x03C, UARTMIS at 0x040, UARTICR
at 0x044, UARTDMACR at 0x048, and Peripheral/PrimeCell ID registers at
0xFE0-0xFFC.

Driver: The driver indirects every register access through a `reg_offset`
table indexed by a logical register id (`enum { REG_DR, REG_FR, ..., REG_ARRAY_SIZE }`
at `amba-pl011.c:56-84`). The standard mapping `pl011_std_offsets` at
`amba-pl011.c:86-100` matches the datasheet verbatim for the registers
the driver uses. The indirection lets the driver support non-standard
integrations (see Section 1.10 Variability). All MMIO goes through
`pl011_read` / `pl011_write` (`amba-pl011.c:319-337`), which dispatch to
either `readl_relaxed`/`writel_relaxed` or `readw_relaxed`/`writew_relaxed`
depending on `uap->port.iotype`. The 16-bit default matches the
datasheet's 16-bit-wide register layout (Table 3-1); the 32-bit path
handles SBSA and other variants where all accesses must be 32-bit.

Discrepancy / note: The datasheet notes that accesses to the Peripheral
Identification Registers "must be 32-bit, using the LDR and STR
instructions" (page 3-23). The stock driver never reads PeriphID / CellID
registers; `amba_rev(dev)` at `amba-pl011.c:125` gets the revision from
the AMBA bus subsystem, which in turn is responsible for reading those
registers. So this rule is honored by construction, not by the driver.

### 1.2 Port enable and the transmit / receive enable bits

Datasheet: UARTCR (Section 3.3.8, Table 3-12) has UARTEN at bit 0, TXE at
bit 8, RXE at bit 9. The datasheet states: "To enable transmission, the
TXE bit and UARTEN bit must be set to 1. Similarly, to enable reception,
the RXE bit and UARTEN bit, must be set to 1" (page 3-16). The datasheet
further mandates a fixed sequence for control-register reprogramming
(Section 3.3.8, note on page 3-16): disable UART, wait for current
character to complete, flush TX FIFO by clearing FEN, reprogram UARTCR,
re-enable UART.

Driver: `pl011_startup` at `amba-pl011.c:1927-1972` sets UARTEN | RXE (and
TXE unless RS485 is enabled) in UARTCR, preserving the current values of
RTS and DTR across the port open (`cr &= UART011_CR_RTS | UART011_CR_DTR`
at line 1947). Shutdown is split across `pl011_disable_uart`
(`amba-pl011.c:2010-2028`) and `pl011_shutdown_channel`
(`amba-pl011.c:1996-2003`). `pl011_disable_uart` reads CR, masks it
down to RTS | DTR, then ORs in `UARTEN | TXE` and writes it back
(lines 2016-2019); it then clears BRK | FEN in LCR_H via
`pl011_shutdown_channel`. UARTEN is therefore never cleared by any
register write during shutdown -- it stays set until the final
`clk_disable_unprepare` in `pl011_shutdown` (`amba-pl011.c:2061`)
stops the peripheral clock. This is a datasheet-vs-driver discrepancy
relative to the five-step procedure in Section 3.3.8, which prescribes
clearing UARTEN before reprogramming; see Section 4.4 for the
correctness record.

SBSA variant: The SBSA vendor is marked `.always_enabled = true`
(`amba-pl011.c:153`) and the driver never writes UARTCR on SBSA. The
vendor ops table `sbsa_uart_pops` (`amba-pl011.c:2397-2415`) reflects
this: no mctrl, no set_termios reprogramming of CR.

### 1.3 Baud rate: UARTIBRD / UARTFBRD / UARTLCR_H

Datasheet: Section 2.4.3 ("UART operation") and Sections 3.3.5 / 3.3.6
(UARTIBRD / UARTFBRD) define the 22-bit baud rate divisor composed of a
16-bit integer (UARTIBRD) and a 6-bit fractional (UARTFBRD) part, with
BAUDDIV = FUARTCLK / (16 x Baud rate) (page 3-10). The datasheet is
explicit that "The UARTLCR_H, UARTIBRD, and UARTFBRD registers must not
be changed when the UART is enabled" (page 3-14). It also prescribes the
sequence for updating them (page 3-14): write UARTIBRD, then UARTFBRD,
then UARTLCR_H (the LCR_H write is what latches the composite 30-bit
UARTLCR register).

Driver: `pl011_set_termios` (`amba-pl011.c:2122-2285`) implements the
baud computation. `quot = DIV_ROUND_CLOSEST(port->uartclk * 4, baud)`
(line 2249) when baud <= uartclk/16; the write order is FBRD (low 6
bits), IBRD, then LCR_H via `pl011_write_lcr_h` (lines 2264-2274), which
the comment at 2269-2273 explicitly flags as a hardware-mandated order.
The driver does not clear UARTEN before these writes; it holds only the
port lock across them. Datasheet page 3-14 forbids changing LCR_H /
IBRD / FBRD while the UART is enabled. The separate page 3-10 note
that a written divisor does not take effect at the baud-rate generator
until the current character completes concerns when the already-
written value is latched internally, not permission to write the
registers while UARTEN is set. This is a datasheet-vs-driver
discrepancy and is recorded against the correctness dimension in
Section 4.4.

ST variant: If `vendor->oversampling` is true, the driver uses clkdiv=8
instead of 16 (line 2135) and adjusts `quot` downward at high baud rates
to work around delayed start-bit sampling at 3.0 Mbps+ (lines 2257-2262).
The ST variant also sets the OVSFACT bit (bit 3 of UARTCR) when baud >
uartclk/16 (lines 2238-2243). This bit is an ST-specific use of an
otherwise-reserved UARTCR bit in the PL011 datasheet.

NVIDIA variant: `vendor_nvidia` sets `.skip_ibrd_fbrd = true`
(`amba-pl011.c:238`) and `.set_uartclk_rate = true` (line 239). The
driver reprograms `uap->clk` to `baud * 16` via `clk_set_rate`
(`amba-pl011.c:2160`) and skips IBRD/FBRD writes entirely (line 2245).
This is an integration choice that moves baud programming from the UART
into the clock tree.

Console reverse path: `pl011_console_get_options`
(`amba-pl011.c:2432-2469`) reads IBRD/FBRD back to reconstruct the
baud, handling NVIDIA's `skip_ibrd_fbrd` by assuming a fixed divisor of
64, and the ST OVSFACT bit by doubling the computed baud (lines
2466-2468).

### 1.4 Line format: UARTLCR_H (word length, stop bits, parity, FIFO enable, break)

Datasheet: UARTLCR_H (Section 3.3.7, Table 3-10) fields: BRK (bit 0),
PEN (bit 1), EPS (bit 2), STP2 (bit 3), FEN (bit 4), WLEN (bits 6:5),
SPS (bit 7). Parity truth table at Section 3.3.7 (Table 3-11).

Driver: `pl011_set_termios` (`amba-pl011.c:2175-2199`) builds the LCR_H
value from termios:

- CSIZE selects WLEN via `UART01x_LCRH_WLEN_5/6/7/8` (lines 2175-2188).
  The WLEN_{5..8} constants in `serial_pl011.h:127-130` are 0x00, 0x20,
  0x40, 0x60, matching datasheet bit 6:5 encoding.
- CSTOPB selects STP2 (line 2189).
- PARENB / PARODD / CMSPAR map to PEN / EPS / SPS (lines 2191-2197).
- FEN is set when `uap->fifosize > 1` (line 2198).

Break: `pl011_break_ctl` (`amba-pl011.c:1727-1742`) reads LCR_H, toggles
UART01x_LCRH_BRK, and writes back under the port lock. The driver does
not independently satisfy the datasheet advice that "the software must
set this bit for at least two complete frames" (page 3-13, UARTLCR_H
description of BRK); that is the responsibility of the caller.

FIFO enable / disable: The only path that clears FEN is
`pl011_shutdown_channel` (`amba-pl011.c:1996-2003`), which clears BRK and
FEN as part of the shutdown sequence. `pl011_set_termios` unconditionally
sets FEN when fifosize > 1; the fifosize-1 case corresponds to a PL010
(not used by this driver) or a degraded configuration.

### 1.5 TX data path (PIO)

Datasheet: UARTDR writes push into the 32-deep 8-bit TX FIFO (Section
2.2.4, Section 3.3.1, UARTDR). UARTFR.TXFF (bit 5) indicates FIFO full;
UARTFR.TXFE (bit 7) indicates FIFO empty; UARTFR.BUSY (bit 3) remains
asserted while any character is still in the shift register. Transmit
is initiated by writing UARTDR when TXE and UARTEN are set (Section
3.3.8).

Driver: `pl011_tx_chars` (`amba-pl011.c:1537-1580`), called from the
IRQ handler (`amba-pl011.c:1649`) and from `pl011_start_tx_pio`
(`amba-pl011.c:1369-1375`), drains up to `fifosize >> 1` characters per
IRQ (line 1540) to leave half-FIFO headroom. Each character goes through
`pl011_tx_char` (`amba-pl011.c:1523-1534`), which writes UARTDR; from IRQ
context it assumes space (TXIM fires as the FIFO drains through the
threshold), but from non-IRQ context it first reads UARTFR and bails if
TXFF is set (line 1527). X-char handling (software flow control) is
folded into `pl011_tx_chars` (lines 1542-1547): the X-char is written
before the ring-buffer pass and TX count is decremented accordingly.

TX-empty test: `pl011_tx_empty` (`amba-pl011.c:1664-1674`) reads UARTFR,
XORs with `vendor->inv_fr` to handle the QDF2400-E44 inversion (see
Variability), and returns TIOCSER_TEMT only when both BUSY and TXFF are
clear.

### 1.6 RX data path (PIO)

Datasheet: UARTDR reads pop from the 12-bit-wide, 32-deep RX FIFO
(Section 2.2.5, Section 3.3.1, UARTDR). The top 4 bits carry status
(OE at bit 11, BE at bit 10, PE at bit 9, FE at bit 8, Table 2-1,
page 2-13). UARTFR.RXFE (bit 4) is the FIFO-empty flag; UARTFR.RXFF (bit
6) is the FIFO-full flag. Error bits (per-character) are associated with
the character at the top of the FIFO; the overrun bit is not associated
with a character -- it is set at the time of overrun and cleared when an
empty FIFO location later becomes available (Section 2.4.3, page 2-13).

Driver: `pl011_fifo_to_tty` (`amba-pl011.c:344-391`) drains the RX FIFO
up to 256 characters. Per iteration it:

- Reads UARTFR and exits on RXFE (line 353).
- Reads UARTDR once per character and ORs in `UART_DUMMY_DR_RX = BIT(16)`
  (line 357) as a software marker that distinguishes a real byte from a
  synthetic zero used elsewhere.
- If the character carries any of the DR error bits
  (`UART_DR_ERROR = OE | BE | PE | FE`, `amba-pl011.c:53`), it updates
  the break / parity / frame / overrun counters (lines 361-373), then
  applies `port->read_status_mask` to suppress ignored bits before
  translating to a TTY flag (lines 375-382).

This directly consumes the DR status bits (Section 3.3.1, UARTDR, Table
3-2) and does not separately consult UARTRSR. The datasheet explicitly
states that "the received data character must be read first from the
Data Register, UARTDR ... before reading the error status associated
with that data character from the UARTRSR Register" (page 3-7). The
driver bypasses that sequencing concern by reading the combined
{data, error} word from UARTDR directly.

Throttle / unthrottle: `pl011_throttle_rx` (`amba-pl011.c:1472-1479`)
and `pl011_unthrottle_rx` (`amba-pl011.c:1904-1925`) disable / re-enable
RXIM / RTIM / error masks in UARTIMSC; unthrottle also re-enables
RXDMAE if DMA is in use (lines 1917-1922).

### 1.7 Interrupt management

Datasheet: Section 2.8 and Sections 3.3.10 / 3.3.11 / 3.3.12 / 3.3.13
define eleven maskable interrupt sources combined into a single UART
interrupt line (UARTINTR): OE, BE, PE, FE, RT (timeout), TX, RX, DSR,
DCD, CTS, RI. UARTIMSC (bit layout at Section 3.3.10) enables
individual interrupts; UARTRIS (Section 3.3.11) shows raw status;
UARTMIS (Section 3.3.12) shows masked status; UARTICR (Section 3.3.13)
is write-1-to-clear. The datasheet states "the transmit interrupt is
based on a transition through a level, rather than on the level itself"
(Section 2.8.3, page 2-23), and "the receive interrupt changes state
when the receive FIFO reaches the programmed trigger level" (Section
2.8.2).

Driver: A single hard IRQ, `pl011_int` (`amba-pl011.c:1624-1662`),
registered with IRQF_SHARED in `pl011_allocate_irq`
(`amba-pl011.c:1862-1867`). The handler:

1. Takes the port lock (line 1630).
2. Reads RIS & IM (line 1631). Note: it reads the raw interrupt
   register ANDed with the software-cached mask rather than reading
   UARTMIS. This is functionally equivalent for determining pending-and-
   enabled interrupts.
3. Within a bounded loop of AMBA_ISR_PASS_LIMIT=256 iterations
   (`amba-pl011.c:51`), clears modem and error interrupts via ICR
   (mask excludes TXIS, RTIS, RXIS because those are cleared by
   reading / writing UARTDR or draining the FIFO; line 1636-1637),
   then dispatches to `pl011_dma_rx_irq` / `pl011_rx_chars`,
   `pl011_modem_status`, and `pl011_tx_chars`.
4. Re-reads RIS & IM at the end of each iteration (line 1654).

Interrupt clear semantics honored: The driver clears the error bits
(OE, BE, PE, FE) and the modem-status bits via UARTICR
(line 1636-1637); these are write-1-clear per Section 3.3.13. TXIS,
RXIS, and RTIS are explicitly excluded from the ICR write mask. RXIS
and TXIS are level-transition interrupts that are cleared by reading /
writing UARTDR in the dispatched paths. RTIS is cleared by draining
the RX FIFO inside `pl011_rx_chars`; datasheet Section 2.8.4 lists
either "FIFO empty through reading all the data" or "a 1 is written to
the corresponding bit of UARTICR" as valid ways to clear the receive
timeout interrupt, and the driver uses the drain path.

Startup / shutdown of the IRQ state:

- `pl011_hwinit` (`amba-pl011.c:1801-1839`) pre-clears OEIS, BEIS, PEIS,
  FEIS, RTIS, RXIS via ICR (line 1820-1822) and saves / sets the IMSC
  mask to just {RTIM, RXIM}.
- `pl011_enable_interrupts` (`amba-pl011.c:1874-1902`) clears RTIS and
  RXIS, drains the RX FIFO up to 2*fifosize bytes to unstick RXIS (lines
  1890-1895 -- see Correctness for why this matters), then writes
  `im = RTIM | RXIM` to UARTIMSC.
- `pl011_disable_interrupts` (`amba-pl011.c:2030-2040`) writes 0 to IMSC
  and 0xFFFF to ICR (all bits; the top bits are reserved so this is
  effectively "clear all eleven").

Enable-modem-status: `pl011_enable_ms` (`amba-pl011.c:1481-1488`)
enables RI, CTS, DCD, DSR mask bits.

### 1.8 Modem status and control

Datasheet: UARTCR (Section 3.3.8) provides RTSEn/CTSEn (bits 14/15,
hardware flow control), RTS / DTR outputs (bits 11/10), Out1 / Out2 (bits
12/13, usable as DCD / RI in DCE mode per Section 2.5). UARTFR (Section
3.3.3, Table 3-4) provides the four modem status input flags: CTS (bit
0), DSR (bit 1), DCD (bit 2), RI (bit 8). Section 2.5 maps DTE vs DCE
semantics.

Driver: `pl011_get_mctrl` (`amba-pl011.c:1682-1695`) reads UARTFR and
maps CTS / DSR / CAR / RNG to TIOCM_* bits using vendor-parameterized
masks for DSR, CTS, and RI (the ZTE variant has different bit positions;
`serial_pl011.h:99-106`). `pl011_set_mctrl` (`amba-pl011.c:1705-1725`)
writes UARTCR to drive RTS, DTR, OUT1, OUT2, LBE; when UPSTAT_AUTORTS is
set it also toggles RTSEn to arm/disarm auto-RTS (lines 1719-1722).

Modem-status interrupt: `pl011_modem_status`
(`amba-pl011.c:1582-1605`) reads the current modem flags from UARTFR,
XORs against `uap->old_status` to detect edges, dispatches DCD and CTS
changes to the serial core, and bumps the DSR counter. RI is not
surfaced to the serial core; see the Discrepancy note below for the
mechanism.

Discrepancy: The datasheet exposes RI both as a UARTFR level and as a
UARTRIINTR modem interrupt (Section 2.8, page 2-22). The driver does
set RIMIM in `pl011_enable_ms` (line 1486), and `pl011_int` dispatches
to `pl011_modem_status` on RIMIS (`amba-pl011.c:1645-1647`). However,
`UART01x_FR_MODEM_ANY` is defined as `UART01x_FR_DCD | UART01x_FR_DSR
| UART01x_FR_CTS` (`serial_pl011.h:216`) and deliberately excludes RI.
Because `pl011_modem_status` reads only `REG_FR & UART01x_FR_MODEM_ANY`
(line 1586), an RI-only edge produces a zero delta and returns early at
line 1591-1592 without waking msr-waiters or bumping any counter. The
driver's own header states that the PL011 does not have a real RI input
on DTE-mode variants, and RTS / DTR / RI emulation is described as
"out of scope -- has to be supplied via GPIO" (`amba-pl011.c:13-17`),
which is consistent with the silent-drop behavior.

### 1.9 Hardware flow control

Datasheet: Section 2.6 describes RTS / CTS hardware flow control enabled
via UARTCR.RTSEn / UARTCR.CTSEn. Section 2.6.1 couples RTS to the
receive FIFO watermark programmed in UARTIFLS. Section 2.6.2 governs CTS
semantics: "If CTS flow control is enabled, then the transmitter checks
the nUARTCTS signal before transmitting the next byte".

Driver: `pl011_set_termios` (`amba-pl011.c:2227-2236`): if termios
carries CRTSCTS, the driver sets CTSEn and (conditionally) RTSEn, and
reflects the status in `port->status` (UPSTAT_AUTOCTS | UPSTAT_AUTORTS).
If RS485 is enabled, `termios->c_cflag &= ~CRTSCTS` (line 2223) because
RS485 reuses RTS as a direction-control line.

The datasheet footnote "When RTS flow control is enabled, the software
cannot use the RTSEn bit ... to control the status of nUARTRTS"
(page 2-17) is honored implicitly: when UPSTAT_AUTORTS is set, the
driver's `pl011_set_mctrl` toggles RTSEn to enable / disable auto-RTS
(lines 1719-1722) rather than driving the physical line with bit 11 of
UARTCR.

### 1.10 FIFO depth and trigger levels: UARTIFLS

Datasheet: Section 3.3.9, Table 3-13 defines UARTIFLS with RXIFLSEL
(bits 5:3) and TXIFLSEL (bits 2:0) selecting 1/8, 1/4, 1/2, 3/4, or 7/8.
Reset value is 0x12 (both RX and TX at 1/2; Section 3.2 Table 3-1,
page 3-3).

Driver: `pl011_startup` (`amba-pl011.c:1942`) writes `vendor->ifls`.
Default for ARM / NVIDIA is `RX4_8 | TX4_8` (i.e. both at 1/2)
(`amba-pl011.c:130, 228`), matching the datasheet reset. The ST variant
uses non-standard `RX_HALF | TX_HALF` (line 208), which are extended
RXIFLSEL=5 / TXIFLSEL=5 encodings that the datasheet marks reserved
for the reference PL011 (`serial_pl011.h:172-173`, commented "special
values for ST vendor with deeper fifo").

FIFO depth: Per-vendor `get_fifosize` (`amba-pl011.c:123-126, 201-204,
221-224`) returns 16 for PL011 rev<3 ARM, 32 for PL011 rev>=3 and
NVIDIA, 64 for ST. SBSA has a hard-coded 32 in `sbsa_uart_probe`
(`amba-pl011.c:3116`). The datasheet states the PL011 FIFO is 32-deep
(Section 2.1, page 2-2); the rev<3 case with 16 is an ARM-variant
choice, and the 64-deep ST is an implementation-variant.

### 1.11 Break detection and generation

Datasheet: Generation: UARTLCR_H.BRK (bit 0); the datasheet requires the
bit be set "for at least two complete frames" (Section 3.3.7). Detection:
BE error bit at UARTDR bit 10 (Table 3-2). The datasheet notes "when a
break occurs, only one 0 character is loaded into the FIFO" (page 3-6).

Driver: Generation handled by `pl011_break_ctl`
(`amba-pl011.c:1727-1742`), reading-modify-writing UARTLCR_H under the
port lock. Detection in `pl011_fifo_to_tty` sees BE, increments
`port->icount.brk`, and calls `uart_handle_break`
(`amba-pl011.c:362-366`), which also performs sysrq-break handling.

### 1.12 Loopback

Datasheet: UARTCR.LBE (bit 7) enables internal loopback: UARTTXD is fed
to UARTRXD (Section 3.3.8 / Section 2.4.3).

Driver: `pl011_set_mctrl` (`amba-pl011.c:1717`) drives LBE from
TIOCM_LOOP. Not used elsewhere.

### 1.13 IrDA

Datasheet: Sections 1.1, 2.1.1, 2.4.4, and 3.3.8 describe the IrDA SIR
ENDEC block, enabled via UARTCR.SIREN (bit 1) with SIRLP (bit 2) for
low-power mode, plus the UARTILPR register (Section 3.3.4) for the
low-power divisor.

Driver: The SIREN / SIRLP / UARTILPR features are not exercised. The
header defines `UART01x_CR_SIREN` and `UART01x_CR_IIRLP`
(`serial_pl011.h:122-123`) and `UART01x_ILPR` (line 38), but no code in
`amba-pl011.c` references them. The IrDA ENDEC block is a documented
device capability that the Linux stock driver does not exercise.

### 1.14 DMA (functional aspect)

Datasheet: Section 2.7 defines UARTRXDMASREQ / UARTRXDMABREQ /
UARTTXDMASREQ / UARTTXDMABREQ handshake signals, and UARTDMACR (Section
3.3.14, Table 3-18) gates them with RXDMAE (bit 0), TXDMAE (bit 1), and
DMAONERR (bit 2). Table 2-4 at page 2-21 lists DMA burst trigger points
per watermark level.

Driver: DMA setup lives under `#ifdef CONFIG_DMA_ENGINE`
(`amba-pl011.c:398-1302`). `pl011_dma_probe` (`amba-pl011.c:423-551`)
acquires TX and optionally RX channels, configuring `src_addr` /
`dst_addr` to the DR offset, and picks burst sizes
`dst_maxburst = fifosize >> 1`, `src_maxburst = fifosize >> 2`
(lines 433, 489). `pl011_dma_startup` (`amba-pl011.c:1127-1199`)
enables DMAONERR and, on ST variants, programs the
ST-specific DMA watermark register (REG_ST_DMAWM) to 16 / 16
(lines 1183-1185).

Performance implications of DMA are covered in Section 3; the point
here is only that the datasheet-defined DMA feature is captured.

### 1.15 Variability (Functional)

- ARM rev<3 vs rev>=3 FIFO depth. `get_fifosize_arm`
  (`amba-pl011.c:123-126`) returns 16 for rev<3 and 32 for rev>=3. The
  PL011 datasheet (r1p5, pages 2-5 / 2-6) describes the FIFO as
  32-deep; the rev<3 16-deep variant is an implementation choice.
- ST Micro vendor (`vendor_st`, `amba-pl011.c:206-219`). Deeper 64-byte
  FIFO, half-mark IFLS encoding beyond the datasheet's 1/8..7/8 range,
  oversampling, dma_threshold, cts_event_workaround (see Correctness),
  split LCR_H into LCRH_RX (0x1C) and LCRH_TX (0x2C)
  (`serial_pl011.h:37, 42`, `amba-pl011.c:179-180`).
- NVIDIA vendor (`vendor_nvidia`, `amba-pl011.c:226-241`). Skips
  IBRD / FBRD programming and sets the UART clock rate directly via
  `clk_set_rate`, which is an integration where baud programming is
  moved out of the UART itself.
- ZTE variant. Radically different register offsets (`serial_pl011.h:64-76`)
  and modem-flag bit positions (`serial_pl011.h:99-106`). Note: the
  `ZX_*` offsets are defined in the header but not wired through a
  dedicated `pl011_zte_offsets` table in `amba-pl011.c`. Without
  external glue, the stock driver does not dispatch to those offsets;
  the header's ZX_* entries are effectively dormant in this driver.
  (Hypothesis: there may exist out-of-tree or a different in-tree
  driver that wires them up; I cannot verify that from the provided
  sources.)
- SBSA (`vendor_sbsa`, `amba-pl011.c:143-155`). Always-enabled,
  fixed-options (8n1 with fixed baud), no modem lines, 32-bit accesses
  only. Uses a separate platform driver
  (`arm_sbsa_uart_platform_driver`, `amba-pl011.c:3155-3165`).
- Qualcomm QDF2400-E44 (`vendor_qdt_qdf2400_e44`,
  `amba-pl011.c:158-171`). Inherits from SBSA but sets
  `fr_busy = UART011_FR_TXFE` and `inv_fr = UART011_FR_TXFE`; see
  Correctness for the workaround.
- Split LCR_H. `pl011_split_lcrh` (`amba-pl011.c:1841-1845`) detects
  when LCRH_RX and LCRH_TX map to different offsets (ST only) and the
  driver writes LCRH twice with a 10-PCLK delay interleaved
  (`pl011_write_lcr_h`, `amba-pl011.c:1847-1860`).

## 2. Integration Analysis

Question: which Linux kernel subsystems does the stock driver integrate
the PL011 device into, and how?

### 2.1 tty / serial_core (primary integration)

The driver is a `uart_driver` / `uart_port` client. Declaration of the
driver: `amba_reg` at `amba-pl011.c:2838-2846` with `driver_name =
"ttyAMA"`, major 204 / minor 64 (`amba-pl011.c:47-48`), nr=UART_NR=14
(line 45). Per-port registration goes through
`uart_add_one_port(&amba_reg, &uap->port)` in `pl011_register_port`
(`amba-pl011.c:2957`).

The two `uart_ops` tables:

- `amba_pl011_pops` (`amba-pl011.c:2363-2386`) for PL011.
- `sbsa_uart_pops` (`amba-pl011.c:2397-2415`) for SBSA (minimal ops,
  no mctrl, no flush_buffer, no break_ctl, no enable_ms, no throttle).

All the low-level callbacks covered in Section 1 are wired here: start/
stop TX / RX, set / get mctrl, set_termios, startup, shutdown, config_port,
verify_port, tx_empty, break_ctl, throttle/unthrottle.

### 2.2 Console

Console integration is gated by `CONFIG_SERIAL_AMBA_PL011_CONSOLE`
(`amba-pl011.c:2419, 2836`). The console structure is `amba_console`
(`amba-pl011.c:2672-2684`) with name "ttyAMA".

Console write paths:

- `pl011_console_write_atomic` (`amba-pl011.c:2583-2613`) -- nbcon
  unsafe-region write used for atomic / emergency console output.
- `pl011_console_write_thread` (`amba-pl011.c:2615-2657`) -- nbcon
  thread write used for normal console output.

Both bracket output with: enable clock, save UARTCR (unless the vendor
is always_enabled, e.g. SBSA), write UARTCR with CTSEn cleared and
UARTEN | TXE set (this deliberately bypasses CTS flow control so a
stuck CTS cannot hang the console, lines 2596-2597 / 2628-2629),
drain each character via `pl011_console_putchar`
(`amba-pl011.c:2421-2430`), wait for BUSY to clear, restore UARTCR,
disable clock. Flags include `CON_PRINTBUFFER | CON_ANYTIME | CON_NBCON`
(line 2681).

Console options read-back. `pl011_console_get_options`
(`amba-pl011.c:2432-2469`) reconstructs the current baud / parity / bits
from UARTCR / LCR_H / IBRD / FBRD so that the console can join an
already-running hardware configuration (handling NVIDIA and ST variants
as described in Section 1.3).

Console match. `pl011_console_match` (`amba-pl011.c:2541-2581`) parses
`console=pl011,...` command-line entries, finds the matching port by
`mapbase`, and hands off to `pl011_console_setup`. It also recognizes
the `qdf2400_e44` name so that SPCR-driven earlycon with erratum 44
can be adopted by the runtime driver.

### 2.3 Earlycon

Two `OF_EARLYCON_DECLARE` entries:

- `pl011_early_console_setup` for `arm,pl011`
  (`amba-pl011.c:2766-2791`). Enables the port (UARTCR with UARTEN |
  RXE | TXE) and wires `pl011_early_write` + `pl011_early_read`.
- `sbsa_uart_early_console_setup` for `arm,sbsa-uart`
  (`amba-pl011.c:2797-2809`). Does not touch UARTCR (SBSA is assumed
  pre-enabled by firmware).

An `EARLYCON_DECLARE` entry for `qdf2400_e44`
(`amba-pl011.c:2821-2832`) registers a separate early-console setup
that uses `qdf2400_e44_putc` (`amba-pl011.c:2688-2695`), which after
every character polls until UARTFR.TXFE asserts rather than BUSY
deasserts -- see Correctness for why.

### 2.4 kgdb / console-poll

`CONFIG_CONSOLE_POLL` path (`amba-pl011.c:1744-1799`):

- `pl011_quiesce_irqs` (`amba-pl011.c:1746-1767`) clears pending IRQs
  via MIS->ICR copy, then masks TXIM so a polling user (e.g. KDB NMI
  entry) does not get preempted by a spurious TX interrupt.
- `pl011_get_poll_char` (`amba-pl011.c:1769-1786`) calls
  `pl011_quiesce_irqs`, polls UARTFR.RXFE, reads UARTDR.
- `pl011_put_poll_char` (`amba-pl011.c:1788-1797`) polls UARTFR.TXFF,
  writes UARTDR.

Both uart_ops tables install these via `poll_init = pl011_hwinit`,
`poll_get_char`, `poll_put_char` (`amba-pl011.c:2381-2385` and
`2410-2414`).

### 2.5 Sysrq

`uap->port.has_sysrq = IS_ENABLED(CONFIG_SERIAL_AMBA_PL011_CONSOLE)`
(`amba-pl011.c:2924`). Sysrq character handling hooks into the RX
fast-path via `uart_prepare_sysrq_char` in `pl011_fifo_to_tty`
(`amba-pl011.c:385`) and `uart_unlock_and_check_sysrq` in `pl011_int`
(`amba-pl011.c:1659`).

### 2.6 AMBA / platform bus, Device Tree, ACPI

PL011 integration is via AMBA bus. `pl011_probe`
(`amba-pl011.c:2971-3027`) matches against `pl011_ids`
(`amba-pl011.c:3167-3184`): ARM 0x00041011, ST 0x00380802, NVIDIA
0x0006b011. Probe reads `reg-io-width` from device properties
(lines 3001-3013) to override iotype.

SBSA integration is via platform bus because SBSA ports are not full
PrimeCells (they lack the standard PeriphID registers). `sbsa_uart_probe`
(`amba-pl011.c:3077-3132`) splits the baud-rate source by probe path:
on a DT probe (`pdev->dev.of_node` non-NULL), `current-speed` is
mandatory and probe returns the read error if the property is absent
(`amba-pl011.c:3088-3093`, comment at 3084-3087 calls it "mandatory");
on the non-DT (ACPI) path, `baudrate` defaults to 115200
(`amba-pl011.c:3094-3096`). ACPI match table is `sbsa_uart_acpi_match`
for ARMH0011 / ARMHB000 (`amba-pl011.c:3148-3153`).

DT alias. `pl011_probe_dt_alias` (`amba-pl011.c:2848-2878`) looks up
`serial<n>` aliases in the DT to assign stable port numbers.

RS485 DT properties. `uart_get_rs485_mode(&uap->port)` in
`pl011_setup_port` (`amba-pl011.c:2928`) parses the RS485 DT
properties into `port->rs485`.

### 2.7 Power management

`SIMPLE_DEV_PM_OPS(pl011_dev_pm_ops, pl011_suspend, pl011_resume)`
(`amba-pl011.c:3059`). `pl011_suspend` / `pl011_resume`
(`amba-pl011.c:3038-3057`) delegate to the serial core via
`uart_suspend_port` / `uart_resume_port`. Both the AMBA and the SBSA
drivers reuse the same `pl011_dev_pm_ops` (`amba-pl011.c:3191, 3160`).

### 2.8 Clock, pinctrl, DMA engine, slab, IRQ

- Clock: the driver looks up a named-less clock
  (`devm_clk_get(&dev->dev, NULL)`, `amba-pl011.c:2987`), enables it in
  `pl011_hwinit` (`amba-pl011.c:1813`), disables it on shutdown
  (`amba-pl011.c:2061`). The console atomic / thread paths wrap writes
  with `clk_enable` / `clk_disable` to handle the case where runtime
  PM has gated the clock.
- Pinctrl: `pinctrl_pm_select_default_state` /
  `pinctrl_pm_select_sleep_state` at `amba-pl011.c:1808, 2063, 2492`.
- DMA engine: `dma_request_chan(dev, "tx")` / `dma_request_chan(dev,
  "rx")` (`amba-pl011.c:440, 472`), with a platform-data fallback to
  `dma_request_channel` (lines 457, 475). The RX channel requires
  `DMA_RESIDUE_GRANULARITY_DESCRIPTOR` support or better
  (lines 499-507).
- slab: `devm_kzalloc` for the port, `kmalloc` for the TX DMA buffer
  (`amba-pl011.c:1137`), `dma_alloc_coherent` for the RX DMA ping-pong
  buffers (`pl011_dmabuf_init` at `amba-pl011.c:402-412`).
- IRQ: `request_irq(...,IRQF_SHARED, "uart-pl011", uap)` in
  `pl011_allocate_irq` (`amba-pl011.c:1866`); single shared IRQ per
  port.

### 2.9 RS485

`pl011_rs485_config` (`amba-pl011.c:2342-2361`) and `pl011_rs485_tx_start`
/ `pl011_rs485_tx_stop` (`amba-pl011.c:1304-1344, 1377-1414`) implement
half-duplex RS485 direction control by driving RTS manually around each
TX burst via UARTCR. Two hrtimers (`trigger_start_tx` and
`trigger_stop_tx`, declared at `amba-pl011.c:297-298` and set up in
`pl011_probe` at `amba-pl011.c:3015-3018`) cover the
`delay_rts_before_send` and `delay_rts_after_send` intervals, plus a
post-TX drain interval derived from the baud rate
(`rs485_tx_drain_interval`, `amba-pl011.c:2215`).
`pl011_rs485_supported` (`amba-pl011.c:2964-2969`) advertises the flag
bits SER_RS485_ENABLED | SER_RS485_RTS_ON_SEND | SER_RS485_RTS_AFTER_SEND
| SER_RS485_RX_DURING_TX.

### 2.10 Variability (Integration)

- SBSA vs PrimeCell PL011. SBSA ports do not have PeriphID / CellID
  registers, so the SBSA path uses the platform bus rather than the AMBA
  bus (`amba-pl011.c:3155-3165`). Its uart_ops
  (`sbsa_uart_pops`) is a reduced subset.
- ACPI. Only the SBSA platform driver has an `acpi_match_table`
  (`amba-pl011.c:3153`); the AMBA PL011 side has no ACPI bindings.
- Console match for the erratum-44 quirk name (`qdf2400_e44`,
  `amba-pl011.c:2554`) lives in the console match callback rather than
  in a vendor_data entry.
- ARM vs ARM-variant revision gating. `get_fifosize_arm` switches on
  `amba_rev(dev)` (`amba-pl011.c:125`), which is the PL011 PrimeCell
  revision field.
- The `suppress_bind_attrs = IS_BUILTIN(CONFIG_SERIAL_AMBA_PL011)`
  (`amba-pl011.c:3163, 3192`) prevents unbind when the driver is
  built-in, because an unbind of a console-providing UART would break
  kernel printk.

## 3. Performance Analysis

Question: which PL011 performance features does the Linux driver
implement, and which Linux-side performance facilities does the driver
use to improve the throughput, latency, or CPU cost of device operation?

### 3.1 FIFO depth and trigger levels (performance aspect)

Datasheet: 32-byte TX and 32x12 RX FIFOs (Section 2.1, page 2-2);
trigger levels programmable through UARTIFLS at 1/8, 1/4, 1/2, 3/4, 7/8
(Section 3.3.9). Deeper FIFOs reduce interrupt rate at a given line
rate.

Driver: Default trigger level is 1/2 on both TX and RX for ARM / NVIDIA
(`amba-pl011.c:130, 228`). This is the datasheet's reset value and a
middle ground between latency (small trigger, more IRQs per byte) and
throughput (large trigger, fewer IRQs but more buffering delay). The ST
variant uses `IFLS_RX_HALF | IFLS_TX_HALF` (value 5 in each field),
which is an ST-specific encoding for a 64-byte FIFO half-mark
(`serial_pl011.h:172-173`).

Per-IRQ work is bounded by `fifosize >> 1` characters in TX
(`pl011_tx_chars`, `amba-pl011.c:1540`) and 256 characters in RX
(`pl011_fifo_to_tty`, `amba-pl011.c:351`). The RX 256 limit is larger
than any PL011 FIFO and is intended to allow timeout-induced drains to
catch up after an overrun.

### 3.2 Receive timeout interrupt (RTIM)

Datasheet: Section 2.8.4: UARTRTINTR "is asserted when the receive FIFO
is not empty, and no more data is received during a 32-bit period". This
lets a driver pick up trailing characters that never reached the RX
trigger level, without having to poll.

Driver: RTIM is enabled in `pl011_enable_interrupts`
(`amba-pl011.c:1897`) and `pl011_unthrottle_rx`
(`amba-pl011.c:1911`), always. The IRQ handler treats RTIS identically
to RXIS (`pl011_int`, `amba-pl011.c:1639-1644`): both trigger a FIFO
drain. In DMA-RX mode, RTIS is what ends a bursty receive that did not
fill a DMA buffer (`pl011_dma_rx_irq`, `amba-pl011.c:964-1010`); it is
therefore the latency-bounding mechanism for DMA RX.

### 3.3 DMA TX

Datasheet: Section 2.7 and Table 2-4. TX DMA burst request fires when
the TX FIFO has more empty slots than the TXIFLSEL watermark.

Driver (performance aspect): `pl011_dma_tx_refill`
(`amba-pl011.c:620-707`) decides whether to use DMA TX for a given
pending chunk. The short-chunk heuristic: if `count < fifosize/2`, fall
back to PIO TX (line 636-639), because the full-FIFO IRQ path already
sinks half a FIFO without a DMA setup cost. Also, the last byte is
intentionally not DMA'd (line 645) so that XON / XOFF reception can pause
the stream via interrupt without racing a pending DMA. Alignment is
respected via `chan->device->copy_align` (line 658-659) so DMA
controllers with a word-alignment requirement (Tegra GPC is called out
in the comment) do not receive misaligned data.

On successful DMA submission, the driver masks TXIM (`uap->im &=
~UART011_TXIM`, line 740 in `pl011_dma_tx_irq`) so the CPU is not
interrupted until the DMA completes, at which point `pl011_dma_tx_callback`
decides whether to re-refill or switch to PIO
(`amba-pl011.c:569-610`). `port.fifosize` is rewritten to the DMA
buffer size (`PL011_DMA_BUFFER_SIZE = PAGE_SIZE`) in `pl011_dma_startup`
(`amba-pl011.c:1146`) so the serial core batches larger write requests
through the port.

### 3.4 DMA RX

Datasheet: Section 2.7 defines RX burst requests fired when the RX FIFO
has more characters than the watermark. DMAONERR (Section 3.3.14, bit 2)
masks RX DMA requests when UARTEINTR is asserted.

Driver: `pl011_dma_rx_trigger_dma` (`amba-pl011.c:847-888`) prepares a
slave descriptor into one of two ping-pong DMA buffers (`dbuf_a` /
`dbuf_b`, `amba-pl011.c:255-256`), enables RXDMAE, and masks RXIM
(line 884) so only RTIS (timeout) remains as an interrupt source for
RX. On either RTIS (timeout while DMA is running) or DMA-buffer-
complete, the driver pauses / terminates the DMA, reads the residue,
copies out the received bytes with `tty_insert_flip_string`, flips the
buffer, and retriggers (`pl011_dma_rx_irq`, `amba-pl011.c:964-1010`,
and `pl011_dma_rx_callback`, `amba-pl011.c:1012-1058`).

When the DMA engine cannot prepare a descriptor, the driver falls back
to IRQ-driven RX with a log line (`amba-pl011.c:1004-1008`, 1052-1056).

DMAONERR is set on startup (`pl011_dma_startup`, `amba-pl011.c:1175`),
so an overrun / break / parity / framing error immediately stops RX DMA,
draining through the FIFO error path instead. This is the datasheet
(page 3-22, DMAONERR description) behavior, and the driver comment at
`amba-pl011.c:944-954` explicitly depends on it ("error conditions will
only occur in the FIFO, these will trigger an immediate interrupt and
stop the DMA job").

### 3.5 DMA RX polling mode (Linux-side latency optimization)

Datasheet: The PL011 does not define a polling mode; this is a
Linux-side strategy.

Driver: `pl011_dma_rx_poll` (`amba-pl011.c:1080-1125`) runs on a
`timer_list` at `dmarx.poll_rate` (default 100 ms, optionally tuned to
`10_000_000 / baud` at `amba-pl011.c:2172` when `auto-poll` is set). The
poll reads the DMA residue and pushes any new bytes to the tty without
waiting for a timeout IRQ. This reduces the interrupt rate at moderate
line rates where RTIS would otherwise fire often. If no data is seen
for `poll_timeout` (default 3 s, `amba-pl011.c:531`), the driver falls
back to IRQ RX.

### 3.6 DMA TX buffer size

`PL011_DMA_BUFFER_SIZE = PAGE_SIZE` (`amba-pl011.c:400`). Larger
buffers amortize DMA setup cost across more characters. The driver
re-announces this via `uap->port.fifosize = PL011_DMA_BUFFER_SIZE`
(`amba-pl011.c:1146`) so that serial_core's `uart_write_wakeup` fires
correctly.

### 3.7 Burst sizes

`dst_maxburst = fifosize >> 1` (TX, `amba-pl011.c:433`) and
`src_maxburst = fifosize >> 2` (RX, `amba-pl011.c:489`). These are
chosen to match (TX) or be smaller than (RX) the datasheet burst trigger
counts at 1/2 watermark (Table 2-4), so the DMA controller can complete
a burst in one go without under-running.

ST DMA watermark. `pl011_dma_startup` also programs REG_ST_DMAWM with
`DMAWM_RX_16 | DMAWM_TX_16` on ST (`amba-pl011.c:1183-1185`). This
register is an ST addition not in the r1p5 datasheet; the comment at
`amba-pl011.c:1179-1182` frames it as an ST-specific burst threshold
compensation.

### 3.8 IRQ batching in `pl011_int`

`AMBA_ISR_PASS_LIMIT = 256` (`amba-pl011.c:51`). The handler loops as
long as `RIS & im` is non-zero; the bound is expressed as a
post-decrement `if (pass_counter-- == 0) break;` (`amba-pl011.c:1651`)
evaluated after the dispatch, so up to 257 dispatch passes can run
before the break fires. The purpose of the loop is to drain all
pending interrupts in one IRQ entry and avoid the overhead of
re-entering the handler repeatedly under a sustained IRQ storm
(`pl011_int`, `amba-pl011.c:1633-1655`).

### 3.9 RX-FIFO drain at enable time

`pl011_enable_interrupts` drains up to `fifosize * 2` RX characters
(`amba-pl011.c:1890-1895`) before unmasking RXIM. The comment at
`amba-pl011.c:1884-1889` explains the performance-relevant fault mode:
if the FIFO is already above the trigger level at unmask time, RXIS (a
level-to-above-threshold transition interrupt) will never re-fire and
the driver would see no RX until overrun. This is both a correctness
issue (Section 4) and a latency-restoring step.

### 3.10 Relaxed MMIO

`readl_relaxed` / `readw_relaxed` / `writel_relaxed` / `writew_relaxed`
are used throughout (`amba-pl011.c:325, 334, 336`). These avoid the
stronger DMB / DSB barriers of the non-relaxed variants. The driver
relies on the port spinlock and on the APB interface's own ordering for
cross-CPU visibility. (See Correctness for the exceptions: the console
atomic / thread paths and the errata-44 workaround use plain `readl` /
`writel` / `writeb`.)

### 3.11 Variability (Performance)

- ST variant sets `dma_threshold = true`
  (`vendor_st.dma_threshold`, `amba-pl011.c:214`). This toggles the ST
  DMAWM programming in `pl011_dma_startup` (`amba-pl011.c:1183-1185`),
  a performance tunable not present on ARM / NVIDIA / SBSA.
- ST variant enables `oversampling = true`
  (`amba-pl011.c:213`). When baud > uartclk/16 the driver activates
  OVSFACT (effectively x8 oversampling, doubling max baud) and trims
  the divisor at 3.0-3.25 Mbps to avoid start-bit sampling corruption
  (`amba-pl011.c:2238-2243, 2257-2262`). This is both a functional and
  a performance feature: higher top-end baud rate.
- ST variant's 64-byte FIFO raises the effective FIFO-to-trigger ratio.
- ARM rev<3 16-byte FIFO reduces the available batching headroom vs
  rev>=3.
- NVIDIA path sets `skip_ibrd_fbrd`; baud is taken from the clock rate,
  which typically means coarser granularity but no per-write baud
  reprogramming cost (`amba-pl011.c:2155-2166`).
- SBSA has no DMA (`sbsa_uart_pops` does not wire `flush_buffer`, and
  `sbsa_uart_startup` does not call `pl011_dma_startup`), so DMA-related
  batching does not apply.

## 4. Correctness Analysis

Question: what invariants, disciplines, and ordering constraints govern
how the Linux driver exercises the functional mechanisms safely?

### 4.1 Port lock discipline

The driver uses a single `uart_port` spinlock per port. Taken as:

- `uart_port_lock` / `uart_port_unlock`: IRQ-context entry
  (hard-IRQ-safe, see `pl011_int` at `amba-pl011.c:1630, 1659` via
  `uart_unlock_and_check_sysrq`).
- `uart_port_lock_irq` / `uart_port_lock_irqsave`: process / softirq
  context entry, disabling local IRQs while held. Examples:
  `pl011_break_ctl` at `amba-pl011.c:1734`, `pl011_dma_tx_callback` at
  `amba-pl011.c:577`, `pl011_set_termios` at `amba-pl011.c:2203`,
  `pl011_throttle_rx` at `amba-pl011.c:1476`, RS485 hrtimer callbacks
  at `amba-pl011.c:1438, 1452`, `pl011_dma_rx_poll` at
  `amba-pl011.c:1112`.

Locking annotations `__releases(&uap->port.lock)` /
`__acquires(&uap->port.lock)` on `pl011_dma_flush_buffer`
(`amba-pl011.c:824-826`) and `pl011_rx_chars` (`amba-pl011.c:1490-1492`)
declare that those routines drop the port lock internally. `pl011_rx_chars`
drops the lock around `tty_flip_buffer_push` and the RX DMA retrigger
path (`amba-pl011.c:1496-1520`), because the flip-buffer path can
acquire tty-core locks that conflict with the port lock.

Callbacks in DMA engine context: both `pl011_dma_tx_callback`
(`amba-pl011.c:569-610`) and `pl011_dma_rx_callback`
(`amba-pl011.c:1012-1058`) run in the DMA engine's callback context
and acquire the port lock. The TX callback uses
`uart_port_lock_irqsave` / `uart_port_unlock_irqrestore` around the
whole body (lines 577, 598, 609). The RX callback uses
`uart_port_lock_irq` at line 1031 and ends with
`uart_unlock_and_check_sysrq` at line 1047; it has a post-unlock path
at lines 1052-1057 that sets RXIM and writes REG_IMSC outside the
lock on DMA-retrigger failure (see Section 4.12 for the IMSC /
`uap->im` exception this creates).

### 4.2 Hardware-mandated ordering

The datasheet and the driver agree on several hard sequencing rules.

LCR_H / IBRD / FBRD write sequence: Datasheet Section 3.3.6 (page 3-14):
"To update the three registers there are two possible sequences: IBRD,
FBRD, LCR_H, or FBRD, IBRD, LCR_H". The driver writes FBRD, IBRD, then
LCR_H in `pl011_set_termios` (`amba-pl011.c:2264-2274`), with the
comment at 2269-2273 explicitly calling out the order as mandatory.

UARTDR-before-UARTRSR for error reading. Datasheet page 3-7: "The
received data character must be read first from the Data Register,
UARTDR ... before reading the error status associated with that data
character from the UARTRSR Register. This read sequence cannot be
reversed". The driver sidesteps the issue by reading the combined
{data, error} word from UARTDR and never consulting UARTRSR
(`pl011_fifo_to_tty`, `amba-pl011.c:357-382`).

Split LCR_H (ST only): When LCRH_RX and LCRH_TX are at different
offsets, the driver writes LCRH_RX first, then executes a 10-iteration
loop writing `0xff` to REG_MIS (lines 1856-1857). REG_MIS is read-
only, so each write is discarded by the hardware; the purpose of the
loop is the 10 APB bus cycles it consumes, which supplies at least 10
PCLK cycles of delay. The driver then writes LCRH_TX
(`pl011_write_lcr_h`, `amba-pl011.c:1847-1860`; the comment at
1852-1855 names the 10-PCLK delay requirement and literally says
"write read only register 10 times"). This is an ST Micro integration
constraint not present in the r1p5 datasheet.

UART enable ordering at shutdown (datasheet page 3-16, steps 1-5):
The datasheet mandates: disable UART, wait for current char to complete,
flush TX FIFO by clearing FEN, reprogram UARTCR, re-enable. The driver's
`pl011_shutdown` sequence is: disable IRQs (`pl011_disable_interrupts`)
-> shutdown DMA (`pl011_dma_shutdown`) -> free_irq -> `pl011_disable_uart`
-> clk_disable_unprepare. Inside `pl011_disable_uart`
(`amba-pl011.c:2010-2028`), the driver reads CR, preserves RTS | DTR,
sets UARTEN | TXE (keeping the UART enabled so the final character
drains), then calls `pl011_shutdown_channel` (`amba-pl011.c:1996-2003`)
which clears BRK | FEN. The driver does not issue an explicit wait after
clearing FEN; it relies on the hardware completing the current character
and on the subsequent clock disable being the true stop point. See 4.4
for the ordering guarantees around this.

DMA shutdown wait-for-busy: `pl011_dma_shutdown` spins on UARTFR.BUSY
before clearing DMACR (`amba-pl011.c:1207-1208`). This ensures the PL011
has finished draining the shift register before DMA is torn down, so no
in-flight byte is dropped. The wait uses `cpu_relax()` and `vendor->fr_busy`
for the variant-specific busy bit.

### 4.3 State-machine invariants

Port lifecycle: The stock driver follows the tty layer's open / close
state machine; `pl011_startup` is called once the port is opened, and
`pl011_shutdown` once the last reference is closed. Within
`pl011_startup`: hwinit -> allocate_irq -> program IFLS -> program CR
(UARTEN | RXE + TXE) -> dma_startup -> enable_interrupts. Within
`pl011_shutdown`: disable_interrupts -> dma_shutdown -> RS485 cleanup ->
free_irq -> disable_uart -> clk_disable -> pinctrl sleep.

TX state machine (RS485): `enum pl011_rs485_tx_state`
(`amba-pl011.c:275-280`) has four states: OFF, WAIT_AFTER_RTS, SEND,
WAIT_AFTER_SEND. Transitions happen only in `pl011_rs485_tx_start` and
`pl011_rs485_tx_stop` (`amba-pl011.c:1304-1344, 1377-1414`), and are
driven by hrtimer callbacks and TX-drain status. The invariant that the
physical RS485 direction (RTS level) matches the logical state is
maintained by the per-transition RTS writes to UARTCR.

Cannot-configure-while-enabled: Datasheet page 3-14 forbids changing
LCR_H / IBRD / FBRD while the UART is enabled. The driver's
`pl011_set_termios` reads CR at line 2225 (UARTEN still set from the
prior startup) and writes FBRD, IBRD, and LCR_H at lines 2264-2274
while holding only the port lock, without clearing UARTEN first. The
datasheet's separate note that "the contents of the UARTIBRD and
UARTFBRD registers are not updated until transmission or reception of
the current character is complete" (page 3-10) concerns when an
already-written divisor value takes effect at the baud-rate generator;
it does not grant permission to write those registers while the UART
is enabled. See Section 4.4 for the full discrepancy record.

### 4.4 UART enable around reconfiguration

Datasheet page 3-6 (UARTDR note) and page 3-16 (UARTCR programming
procedure) both say "you must disable the UART before any of the control
registers are reprogrammed" and require a five-step sequence. Driver
practice: `pl011_set_termios` does not clear UARTEN before the
IBRD/FBRD/LCR_H writes. The mitigations in the driver are:

- Holding the port spinlock, so user-level TX/RX cannot race the
  reconfiguration (`amba-pl011.c:2203, 2284`).
- The datasheet backstop (above) that delays the latch.
- The split-LCRH delay for ST (Section 4.2).

This is a datasheet-vs-driver discrepancy: the driver is less strict
than the five-step procedure the datasheet prescribes. The datasheet's
separate guarantee (page 3-10) that an already-written IBRD / FBRD
value does not take effect at the baud generator until the current
character completes narrows the window in which an inconsistent
divisor can be observed by the shift register, but it does not
authorize writing those registers while UARTEN is set. The same
discrepancy applies to shutdown: `pl011_disable_uart` leaves UARTEN
set (see Section 1.2). This is recorded here per the brief's mandate
to record such discrepancies.

### 4.5 RX unmask drain-for-level-correctness

When `pl011_enable_interrupts` unmasks RXIM, RXIS is a
transition-through-threshold interrupt (datasheet Section 2.8.2). If
the RX FIFO is already at / above the threshold at unmask time, RXIS
will never fire and the driver will miss RX until a timeout or an
overrun. The driver therefore drains up to `fifosize * 2` characters
before unmasking (`amba-pl011.c:1890-1895`). This is a correctness
invariant: "post-condition of pl011_enable_interrupts is that the RX
FIFO is below the threshold".

### 4.6 IRQ clear semantics

Datasheet Section 3.3.13: UARTICR is write-1-to-clear: Datasheet
Section 2.8.2 / 2.8.3: RXIS / TXIS are cleared by reading UARTDR until
below threshold, or by writing UARTDR until above threshold. Driver:

- `pl011_int` writes to ICR excluding TXIS | RTIS | RXIS
  (`amba-pl011.c:1636-1637`). The mask excludes those because clearing
  RXIS / TXIS via ICR is the wrong primitive (they are level-transition
  flags); RTIS is excluded because the driver wants the FIFO drain to
  be the clearing event. Modem and error bits are cleared here.
- `pl011_disable_interrupts` writes 0xFFFF to ICR
  (`amba-pl011.c:2037`). At shutdown all bits are cleared because no
  further dispatch will happen.
- `pl011_hwinit` (`amba-pl011.c:1820-1822`) clears the error bits plus
  RTIS and RXIS before unmasking, so pre-existing stale status does
  not immediately fire.

### 4.7 CTS event workaround (ST)

Datasheet: Not documented.

Driver: `check_apply_cts_event_workaround`
(`amba-pl011.c:1607-1622`) is invoked at the top of every IRQ-loop
iteration (`amba-pl011.c:1634`) when `vendor->cts_event_workaround` is
true (ST Micro only, `amba-pl011.c:215`). It writes 0 to ICR followed
by two dummy reads of ICR to introduce ~2 PCLK cycles of delay before
the actual W1C. The comment at 1615-1619 documents the need: "introduce
26 ns (1 uart clk) delay before W1C; single apb access will incur
2 pclk(133.12Mhz) delay". This is a driver workaround for an ST Micro
quirk absent from the r1p5 datasheet.

### 4.8 QDF2400 erratum 44 workaround

Datasheet: Not documented (this is a Qualcomm SoC erratum, not a PL011
feature).

Driver: `vendor_qdt_qdf2400_e44` (`amba-pl011.c:158-171`) sets
`fr_busy = UART011_FR_TXFE` and `inv_fr = UART011_FR_TXFE`. The effect
in `pl011_tx_empty` and in the console polling loop is that the BUSY
check becomes "(FR XOR TXFE) has TXFE set" = "TXFE is 0 after XOR" =
"TXFE was 1 before XOR". In other words: poll for TX-FIFO-empty instead
of NOT-busy, because on this SoC the BUSY bit is unreliable.
`qdf2400_e44_putc` (`amba-pl011.c:2688-2695`) similarly polls TXFE
after each written byte. `qpdf2400_erratum44_workaround`
(`amba-pl011.c:3062-3075`) swaps the vendor pointer if SPCR reported
the quirk. Driver-side workaround for undocumented silicon behavior.

### 4.9 Bounded work under the port lock

`pl011_int` bounds its outer loop with `AMBA_ISR_PASS_LIMIT=256`
(`amba-pl011.c:1627, 1651`). `pl011_fifo_to_tty` bounds its FIFO drain
at 256 characters (`amba-pl011.c:351`). `pl011_tx_chars` bounds its
drain at `fifosize >> 1` when called from IRQ (`amba-pl011.c:1540,
1560`). These limits prevent a misbehaving peer from holding the CPU in
the ISR under spinlock indefinitely.

Unbounded paths that are explicitly allowed:

- `pl011_console_putchar`, `pl011_put_poll_char`, `pl011_putc`,
  `qdf2400_e44_putc`: spin with `cpu_relax()` on TXFF / BUSY. These are
  only called from console / poll / early paths, which are expected to
  be allowed to spin.
- `pl011_dma_shutdown` spins on BUSY (`amba-pl011.c:1207-1208`) at port
  teardown, where the port lock is not held.

### 4.10 Error-path recovery

- `pl011_startup` on failure jumps to `clk_dis:` and calls
  `clk_disable_unprepare` (`amba-pl011.c:1969-1971`). If
  `pl011_allocate_irq` fails (IRQ could not be requested), the clock is
  released.
- `pl011_probe` failures: every `devm_*` allocation unwinds via the
  devres machinery. The RS485 hrtimers at `amba-pl011.c:3015-3018` are
  initialized via `hrtimer_setup`, which does not arm the timer: no
  kernel timer-subsystem state exists for an initialized-but-never-
  started hrtimer, so there is nothing to cancel on probe failure.
  `hrtimer_start` is invoked only in the RS485 TX paths
  (`amba-pl011.c:1315-1323, 1408-1410`), which are reachable only after
  port open, so any running timer is always associated with a normal
  shutdown path rather than a probe unwind.
- `pl011_dma_rx_trigger_dma` failure on retrigger: both
  `pl011_dma_rx_irq` and `pl011_dma_rx_callback` fall back to enabling
  RXIM and setting `im` accordingly (`amba-pl011.c:1004-1009,
  1052-1057`), so the driver survives transient DMA failures.

### 4.11 Data integrity and barriers

The driver uses `_relaxed` MMIO accessors throughout (Section 3.10).
For steady-state register accesses after the port is visible to other
threads, the port lock serves as the serialization point. Several
register accesses that run before the port is visible are not covered
by the port lock and do not need to be: the initial IMSC / ICR
clearing in `pl011_register_port` (`amba-pl011.c:2941-2943`); the
ICR / IMSC writes in `pl011_hwinit` (`amba-pl011.c:1820-1829`); the
IFLS write in `pl011_startup` (`amba-pl011.c:1942`, just before the
lock is taken at 1944); and the `old_status` read from REG_FR in
`pl011_startup` (`amba-pl011.c:1960`, after the lock is dropped at
1955). These run during probe, registration, or open-time setup when
no other thread can race. The console atomic path uses its own
mechanism (see Section 4.13). DMA coherency:
the TX DMA buffer is `kmalloc` + `__GFP_DMA` and wrapped by
`dma_map_single` / `dma_unmap_single` (`amba-pl011.c:1137, 662-663,
837-838`); the RX DMA buffers are `dma_alloc_coherent`
(`amba-pl011.c:405`), which is the stronger guarantee appropriate for
the ping-pong buffers that are read by CPU without a cache
invalidation.

### 4.12 Concurrency: hard-IRQ vs thread vs hrtimer vs DMA callback

Shared state:

- UARTIMSC cache `uap->im` (`amba-pl011.c:290`). Updated under the port
  lock in `pl011_stop_tx`, `pl011_enable_ms`, `pl011_stop_rx`,
  `pl011_unthrottle_rx`, `pl011_dma_rx_trigger_dma`, `pl011_int`,
  `pl011_start_tx_pio`, `pl011_enable_interrupts`, and
  `pl011_disable_interrupts`. Every UARTIMSC write is paired with the
  updated `uap->im`. Exception: `pl011_dma_rx_callback`
  (`amba-pl011.c:1012-1058`) releases the port lock at line 1047 via
  `uart_unlock_and_check_sysrq` and then, on the DMA-retrigger-failure
  fallback path at lines 1052-1057, sets `UART011_RXIM` in `uap->im`
  and writes REG_IMSC while the lock is no longer held. This is the
  one documented path where the "always under port lock" invariant on
  `uap->im` / REG_IMSC is relaxed; the comment at lines 1048-1051
  motivates the late write ("Do this check after we picked the DMA
  chars so we don't get some IRQ immediately from RX").
- `uap->dmacr` cache. Updated under port lock except in
  `pl011_dma_rx_trigger_dma`, which is called both from DMA callback
  (holding port lock) and from `pl011_dma_startup` (port not yet
  visible).
- `uap->old_status` for modem-status delta. Accessed only from
  `pl011_modem_status` under port lock (`amba-pl011.c:1582-1605`) and
  initialized in `pl011_startup` / `sbsa_uart_startup`
  (`amba-pl011.c:1960, 1989`).
- `uap->rs485_tx_state`. Updated under port lock in the RS485 start /
  stop routines and in hrtimer callbacks, which themselves take the
  port lock (`amba-pl011.c:1438-1443, 1452-1456`).

Sysrq-char handling: `pl011_fifo_to_tty` calls
`uart_prepare_sysrq_char` while the port lock is held
(`amba-pl011.c:385`), and `pl011_int` ends with
`uart_unlock_and_check_sysrq` (`amba-pl011.c:1659`), which drops the
port lock and invokes any pending sysrq handler outside the lock.

### 4.13 Console flow-control override

`pl011_console_write_atomic` and `_thread` clear CTSEn on the saved-and-
temporarily-rewritten UARTCR (`amba-pl011.c:2596-2597, 2628-2629`). This
is a deliberate correctness choice: the console must make progress
regardless of CTS state. The alternative (honor CTS) would hang the
console on a stuck peer. This overrides the datasheet-defined "CTS
gates TX" semantics for the duration of the console write and restores
the original UARTCR afterward.

### 4.14 Variability (Correctness)

- Split LCR_H delay (ST only): required 10-PCLK gap between LCRH_RX and
  LCRH_TX writes; satisfied by ten dummy writes of `0xff` to the
  read-only REG_MIS (the writes are discarded, but each APB transaction
  consumes a PCLK cycle) (`pl011_write_lcr_h`, `amba-pl011.c:1847-1860`).
- CTS event workaround (ST only): 26 ns / 2 PCLK delay before W1C on
  ICR, implemented as two dummy reads (`check_apply_cts_event_workaround`,
  `amba-pl011.c:1607-1622`).
- QDF2400 erratum 44: BUSY bit unreliable, substitute TXFE with
  inversion (`vendor_qdt_qdf2400_e44`, `amba-pl011.c:158-171`, and use
  sites `pl011_tx_empty` at `amba-pl011.c:1670`, console BUSY poll at
  `amba-pl011.c:2604, 2648`).
- SBSA always_enabled: console does not write UARTCR, the startup and
  set_termios paths do not write UARTCR (`amba-pl011.c:2594-2608,
  2626-2652` guarded by `!uap->vendor->always_enabled`; `sbsa_uart_startup`
  at `amba-pl011.c:1974-1994` omits the UARTCR write).
- SBSA fixed options: `sbsa_uart_set_termios`
  (`amba-pl011.c:2287-2306`) forces CS8 | CLOCAL, clears CSTOPB /
  PARENB / PARODD / CMSPAR / CRTSCTS, and leaves IBRD / FBRD /
  LCR_H / CR untouched. This is a correctness choice: the SBSA spec
  requires the UART to stay at the firmware-set configuration.
- NVIDIA `skip_ibrd_fbrd`: `pl011_set_termios` skips IBRD / FBRD writes
  entirely and only reprograms the clock
  (`amba-pl011.c:2245, 2157-2166`); `pl011_console_get_options`
  assumes a fixed divisor of 64 on read-back
  (`amba-pl011.c:2456-2458`).
- `inv_fr`: used to XOR the UARTFR read in `pl011_tx_empty`
  (`amba-pl011.c:1670`) and in the console BUSY-wait loops
  (`amba-pl011.c:2604, 2648`). Non-zero only for QDF2400-E44.
- ZTE offsets (`ZX_*` in `serial_pl011.h:64-76`): declared, not wired
  into the driver's offset dispatch in `amba-pl011.c`. Correctness
  implication: the stock driver does not handle the ZTE offset
  variant; users on that silicon require external glue. (Hypothesis:
  a separate driver or out-of-tree patch binds these; not verifiable
  from the given sources.)
