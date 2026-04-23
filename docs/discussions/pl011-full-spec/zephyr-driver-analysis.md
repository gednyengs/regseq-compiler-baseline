# PL011 Zephyr Stock Driver Analysis

This document is a descriptive reference for the ARM PrimeCell PL011 UART
device and the Zephyr stock PL011 driver. It records (a) which PL011
functional and architectural capabilities are defined by the datasheet, and
(b) which of those capabilities the Zephyr stock driver exercises, and how.
It is partitioned along four dimensions (Functional, Integration,
Performance, Correctness), each with a Variability subsection. A single
device feature may appear in more than one dimension; each occurrence asks
that dimension's question about the feature.

This analysis is bound to `docs/discussions/pl011-full-spec/zephyr-driver-analysis-task-brief.md`
and to the three driver source files `reference-drivers/pl011/zephyr/arm_pl011.yaml`,
`reference-drivers/pl011/zephyr/uart_pl011_registers.h`, and
`reference-drivers/pl011/zephyr/uart_pl011.c`, together with
`docs/discussions/pl011-full-spec/pl011-minimized-datasheet.md` and
`docs/discussions/pl011-full-spec/pl011-registers.md`.

---

## 1. Functional Analysis

This section asks: which functional capabilities of the PL011 does the
Zephyr driver capture, and how does it capture them?

### 1.1 Device enable (UARTEN, TXE, RXE)

- Datasheet. `UARTCR.UARTEN` (bit 0) globally enables the UART; `UARTCR.TXE`
  (bit 8) enables the transmit section; `UARTCR.RXE` (bit 9) enables the
  receive section. Reset value of UARTCR is `0x0300`, so TXE and RXE come
  out of reset as 1 but UARTEN as 0 (datasheet section 3.3.8).
- Driver. Enable and disable are wrapped as helpers `pl011_enable` /
  `pl011_disable`, doing a read-modify-write of `UARTEN` on `cr`
  (`uart_pl011.c:119-127`). During `pl011_init`, after programming
  line/baud/FIFO, the driver explicitly re-asserts TX and RX enables and
  clears SIREN in one sequence (`uart_pl011.c:645-647`), bracketed by
  `barrier_isync_fence_full()` calls (`uart_pl011.c:644, 647`). Final
  `pl011_enable` is called at the end of init on non-SBSA instances
  (`uart_pl011.c:657-659`).

### 1.2 Baud-rate programming (IBRD / FBRD)

- Datasheet. The baud divisor is a 22-bit value split across `UARTIBRD`
  (16-bit integer) and `UARTFBRD` (6-bit fractional), formed as
  `F_UARTCLK / (16 * baud)` (datasheet sections 2.4.3, 3.3.5, 3.3.6). The
  integer part must be in `[1, 65535]`; with `IBRD = 65535`, `FBRD` must be
  zero. The registers are updated internally only by a subsequent write to
  `UARTLCR_H` (sections 3.3.6, 3.3.7).
- Driver. `pl011_set_baudrate` computes
  `bauddiv = ((uint64_t)clk << 6) / (baud * 16)` with a 64-bit-cast
  numerator and a 32-bit `baud * 16` denominator, avoiding float
  (`uart_pl011.c:159-167`); it then validates the fixed-point range
  `[1 << 6, 65535 << 6]` (`uart_pl011.c:171-174`), writes
  `ibrd = bauddiv >> 6` and `fbrd = bauddiv & 0x3F`
  (`uart_pl011.c:176-177`), issues `barrier_dmem_fence_full()`
  (`uart_pl011.c:179`), and commits with a `uart->lcr_h = uart->lcr_h`
  read-modify-write against the line-control register to honour the
  datasheet's "LCR_H write commits IBRD/FBRD" rule (`uart_pl011.c:181-185`,
  citing ARM DDI 0183F p3-13 in comment).
- Datasheet-vs-driver finding: the datasheet's worked example computes
  the 6-bit fractional divisor with round-to-nearest
  (`m = integer((frac * 64) + 0.5)`, see
  `pl011-minimized-datasheet.md:944-962`), while the driver computes
  `bauddiv = floor((clk * 64) / (16 * baud))` and splits into
  `IBRD = bauddiv >> 6`, `FBRD = bauddiv & 0x3F`, which is
  round-toward-zero on the fractional component. For clock / baud
  combinations where the datasheet example would round FBRD up, this
  driver selects the lower FBRD, yielding a slightly different
  generated baud rate than the datasheet's illustrative procedure. The
  rule-bound upper limit (`IBRD = 0xFFFF` with `FBRD > 0`) is NOT
  separately trapped; the upper-bound check at `uart_pl011.c:171-173`
  rejects any fixed-point divisor `bauddiv > (65535 << 6)`, which
  excludes both `IBRD >= 65536` and the datasheet-forbidden case
  `IBRD == 0xFFFF && FBRD > 0` (any nonzero FBRD with IBRD=0xFFFF
  puts `bauddiv` strictly above `65535 * 64`). At the boundary,
  `bauddiv == (65535 << 6)` yields `IBRD == 0xFFFF, FBRD == 0`
  because the low six bits are zero by construction. The datasheet
  rule "`FBRD` must be zero when `IBRD` is `65535`" is therefore
  enforced indirectly by the existing range check.
- Datasheet-vs-header finding: `UARTIBRD` is a 16-bit integer register
  per the datasheet (`pl011-registers.md:176-184`), but the driver
  header defines `PL011_IBRD_BAUD_DIVINT_MASK` as `0xff` with a
  comment claiming "16 bits of divider"
  (`uart_pl011_registers.h:58-60`). The constant is never used by
  `pl011_set_baudrate`, which writes `ibrd = bauddiv >> 6` directly
  (`uart_pl011.c:176`), so baud programming is not affected; the mask
  is an inconsistent / stale header constant.

### 1.3 Line format: word length, parity, stop bits, break (LCR_H)

- Datasheet. `UARTLCR_H` encodes `BRK` (bit 0), `PEN` (bit 1), `EPS`
  (bit 2), `STP2` (bit 3), `FEN` (bit 4), `WLEN[1:0]` (bits 6:5), and
  `SPS` (bit 7). `WLEN` is two bits selecting 5/6/7/8 bits
  (section 3.3.7). `LCR_H` must not be rewritten while the UART is
  enabled, and a `LCR_H` write is what commits IBRD/FBRD.
- Driver. Bit layout is reproduced in `uart_pl011_registers.h:75-83`, with
  a helper `PL011_LCRH_WLEN_SIZE(x) = x - 5` at
  `uart_pl011_registers.h:84`. In `pl011_runtime_configure_internal` the
  driver (a) clears the format field plus STP2 with a mask
  `PL011_LCRH_FORMAT_MASK | PL011_LCRH_STP2` (`uart_pl011.c:275`),
  (b) rebuilds the parity, stop-bits and word-length fields from
  `struct uart_config` (`uart_pl011.c:277-317`), (c) programs flow control
  via a helper (`uart_pl011.c:319-328`), (d) sets the baud rate
  (`uart_pl011.c:331`), and (e) writes the new `LCR_H` in a single
  register write (`uart_pl011.c:337`).
- The `BRK` bit (break transmission) is NOT driven by the driver. It is
  named in the register header (`uart_pl011_registers.h:75`) but no code
  path writes it, so break generation is not a feature the driver
  exercises.

### 1.4 FIFO enable and depth (LCR_H.FEN)

- Datasheet. When `FEN = 1` the transmit FIFO is 32x8 and the receive
  FIFO is 32x12; when `FEN = 0` both collapse to 1-byte holding registers
  (sections 2.2.4, 2.2.5, 2.4.3 Disabling the FIFOs, 3.3.7).
- Driver. FEN is set / cleared by `pl011_enable_fifo` / `pl011_disable_fifo`
  (`uart_pl011.c:129-137`). FIFO mode is selectable per instance via the
  DT boolean `fifo-disable` (`arm_pl011.yaml:11-13`), captured into
  `pl011_config.fifo_disable` (`uart_pl011.c:57, 763`). `pl011_init`
  enables FIFO at the end when `!config->fifo_disable`
  (`uart_pl011.c:634-636`). The runtime reconfigure path re-enables FIFO
  after reprogramming unless `fifo_disable` is set
  (`uart_pl011.c:343-345`).

### 1.5 Transmit path (polling)

- Datasheet. Writing `UARTDR` pushes onto the TX FIFO (or into the holding
  register when FEN=0); `UARTFR.TXFF` indicates no room; `UARTFR.BUSY`
  indicates the shifter is active (sections 3.3.1, 3.3.3).
- Driver. `pl011_poll_out` spin-waits on `fr & TXFF` and then writes `dr`
  (`uart_pl011.c:218-230`). No explicit barrier surrounds the MMIO write;
  `dr` and `fr` are reached through a `volatile struct pl011_regs *`
  obtained from `DEVICE_MMIO_GET(dev)` (`uart_pl011_registers.h:39-43`).

### 1.6 Receive path (polling)

- Datasheet. Reading `UARTDR` pops the RX FIFO and returns the 8 data bits
  together with the per-character error bits (OE, BE, PE, FE) in
  `UARTDR[11:8]` (section 3.3.1). `UARTFR.RXFE` indicates empty.
- Driver. `pl011_poll_in` calls `pl011_is_readable(dev)` and then reads
  `uart->dr` (`uart_pl011.c:204-216`). `pl011_is_readable` additionally
  gates on `UARTEN && RXE` for non-SBSA instances (`uart_pl011.c:190-202`).
- Discrepancy / finding. The poll-in path reads `dr` as
  `(unsigned char)uart->dr` (`uart_pl011.c:213`), which discards the four
  per-character error bits. Error information must be retrieved
  separately via `pl011_err_check`.

### 1.7 Error detection and clearing (DR / RSR / ECR)

- Datasheet. The per-character error bits are surfaced both in `UARTDR[11:8]`
  and in `UARTRSR[3:0]`. The overrun bit in `UARTRSR` is set immediately,
  not on the character in the FIFO. Writing to `UARTECR` (same offset as
  `UARTRSR`) clears FE/PE/BE/OE. The required read order is
  `DR-before-RSR`, since `UARTRSR` is only updated on a `UARTDR` read
  (section 3.3.2 and its note).
- Driver. `pl011_err_check` reads `rsr` and folds the four bits into
  Zephyr's error mask (`uart_pl011.c:232-253`). It does NOT issue a
  write-to-`ecr` to clear the errors; the returned error bits remain set
  in the hardware after `pl011_err_check` returns.
- Discrepancy. The datasheet-mandated DR-before-RSR order is not enforced
  by the driver in either direction; `pl011_poll_in` may be called without
  ever calling `pl011_err_check` after, and `pl011_err_check` reads `rsr`
  without requiring a prior `dr` read. For overrun the RSR bit is set
  immediately and does not depend on the read order. Hypothesis: for
  framing/parity/break, callers that invoke `err_check` before reading the
  corresponding `dr` will see error state tied to the next FIFO top, not
  to the character they are about to consume. Confirmation would require
  a test with intentionally corrupted frames.
- The driver also emits `uart->icr = PL011_IMSC_MASK_ALL` during
  `pl011_init` (`uart_pl011.c:640`), which clears all 11 raw interrupt
  sources, including the four error interrupts.

### 1.8 Interrupt management (IMSC / RIS / MIS / ICR)

- Datasheet. Eleven maskable sources: OE, BE, PE, FE, RT, TX, RX, DSR, DCD,
  CTS, RI, collected in `UARTIMSC`, `UARTRIS`, `UARTMIS`, and
  `UARTICR` (sections 2.8, 3.3.10-3.3.13).
- Driver. Full bit layout in `uart_pl011_registers.h:126-147`. `pl011_init`
  zeros `imsc` and writes `PL011_IMSC_MASK_ALL` to `icr` to clear every
  raw source (`uart_pl011.c:639-640`). Per-direction enables set or clear
  groups of `IMSC` bits:
  - `pl011_irq_tx_enable`: sets `TXIM` (`uart_pl011.c:403`) and may also
    set `CTSMIM` if CTS is blocking (`uart_pl011.c:440`).
  - `pl011_irq_tx_disable`: clears both `TXIM` and `CTSMIM`
    (`uart_pl011.c:456`).
  - `pl011_irq_rx_enable` / `pl011_irq_rx_disable`: set or clear
    `RXIM | RTIM` in `imsc` (`uart_pl011.c:479-487`).
  - `pl011_irq_err_enable` / `pl011_irq_err_disable`: set or clear
    `PL011_IMSC_ERROR_MASK = FEIM | PEIM | BEIM | OEIM`
    (`uart_pl011.c:502-511`, mask at `uart_pl011_registers.h:138-140`).
- In the ISR, only the CTS modem status source is explicitly
  acknowledged via a write to `icr` (`uart_pl011.c:716-719`). Per the
  datasheet (`pl011-minimized-datasheet.md:697-720`), RX raw status
  deasserts when the FIFO drops below RXIFLSEL, RT raw status
  deasserts only when the RX FIFO becomes empty, and TX raw status
  deasserts when the FIFO rises above TXIFLSEL -- so RX/TX/RT clear
  implicitly as the consumer moves data through `dr`, provided the
  FIFO reaches the required level. Error raw status (FE / PE / BE /
  OE), however, has no `icr`-clear path in this driver:
  `pl011_err_check` reads `rsr` only and never writes `ecr`
  (`uart_pl011.c:232-253`), and no other code path clears error raw
  status after the blanket init clear at `uart_pl011.c:639-640`. See
  section 4.12 for the correctness angle.

### 1.9 FIFO trigger levels (IFLS)

- Datasheet. `UARTIFLS` programs TXIFLSEL and RXIFLSEL trigger levels at
  1/8, 1/4, 1/2, 3/4, 7/8 of the FIFO; reset value `0x12` yields 1/2 full
  on both (sections 2.8.2, 2.8.3, 3.3.9).
- Driver. `pl011_init` explicitly programs
  `TXIFLSEL = TXIFLSEL_1_8_FULL (0)` and `RXIFLSEL = RXIFLSEL_1_2_FULL (2)`
  via `FIELD_PREP` into `ifls` (`uart_pl011.c:629-631`,
  constants at `uart_pl011_registers.h:120-123`). So TX fires on
  "<= 1/8 full" and RX on ">= 1/2 full". This is driver-selected; the
  reset value of IFLS would have given 1/2 on TX.

### 1.10 RX timeout interrupt (RTIM)

- Datasheet. `UARTRTINTR` fires when the RX FIFO is non-empty and no
  further character arrives within a 32-bit period; its mask acts as an
  enable (sections 2.8.4, 3.3.11 note a).
- Driver. `pl011_irq_rx_enable` sets `RTIM` alongside `RXIM`
  (`uart_pl011.c:481`); `pl011_irq_rx_disable` clears both
  (`uart_pl011.c:486`). There is no separate API for RT alone, and no
  timeout-only use outside the RX-enable bundle.

### 1.11 Transmit interrupt / FIFO fill (TXIM, UARTDR, TXFF)

- Datasheet. `UARTTXINTR` is asserted on a transition through TXIFLSEL;
  crucially, it is a level-transition interrupt, so enabling TX without
  ever writing data does NOT trigger it (datasheet section 2.8.3 note).
- Driver. `pl011_fifo_fill` copies up to `len` bytes into `dr` while
  `!TXFF` (`uart_pl011.c:373-383`). `pl011_irq_tx_enable`
  (`uart_pl011.c:398-447`) sets `TXIM`, and if the software flag
  `sw_call_txdrdy` is set, drives the first interrupt by calling the
  registered callback in a loop until either CTS blocks further
  transmission or the TX mask is cleared. See section 3.5 for the
  performance angle and section 4.4 for the correctness angle.

### 1.12 Receive interrupt / FIFO read (RXIM, UARTDR, RXFE)

- Datasheet. `UARTRXINTR` transitions high when the RX FIFO reaches the
  RXIFLSEL trigger level, or (with FIFO disabled) when the 1-byte holder
  fills (section 2.8.2).
- Driver. `pl011_fifo_read` reads `dr` while `!RXFE` and `len - num_rx > 0`
  (`uart_pl011.c:385-396`). `pl011_irq_rx_ready` returns true only when
  `RXIM` is set and `!RXFE` (`uart_pl011.c:489-500`). The driver does not
  consult `mis` when deciding readiness, only `fr` and `imsc`.

### 1.13 Hardware flow control (CTSEn, RTSEn)

- Datasheet. `UARTCR.CTSEn` (bit 15) gates TX on `nUARTCTS`;
  `UARTCR.RTSEn` (bit 14) makes the UART manage `nUARTRTS` based on the
  RX FIFO watermark (section 2.6). These are distinct from the
  software-driven `RTS` bit at `UARTCR[11]`.
- Driver. `pl011_set_flow_control(rts, cts)` does a read-modify-write of
  `cr` to set or clear the two hardware-flow-control enables
  (`uart_pl011.c:139-157`). The reconfigure path uses it with both on
  (`UART_CFG_FLOW_CTRL_RTS_CTS`) or both off (`UART_CFG_FLOW_CTRL_NONE`);
  single-direction flow control is not exposed through
  `pl011_runtime_configure_internal` (`uart_pl011.c:319-328`). Initial DT
  selection is via the `hw-flow-control` property
  (`uart_pl011.c:780-782`). Runtime CTS handling during TX drain is
  described in 1.11 and 4.4.

### 1.14 Modem control outputs (DTR, RTS, Out1, Out2)

- Datasheet. `UARTCR[10]=DTR`, `UARTCR[11]=RTS`, `UARTCR[12]=Out1`,
  `UARTCR[13]=Out2`: the driver can assert each modem output
  independently of the hardware-flow-control engine (section 3.3.8).
- Driver. Bits are named in `uart_pl011_registers.h:101-104`, but the
  driver exposes no API to set or clear them, nor does it drive them in
  init. Only the `CTSEn` / `RTSEn` hardware-flow enables and `UARTEN`,
  `TXE`, `RXE`, `SIREN` bits of `cr` are ever written. `PL011_CR_LBE`
  is defined at `uart_pl011_registers.h:98` but is never written by
  the driver.

### 1.15 Modem status inputs (CTS, DSR, DCD, RI in UARTFR)

- Datasheet. `UARTFR[0]=CTS`, `UARTFR[1]=DSR`, `UARTFR[2]=DCD`,
  `UARTFR[8]=RI` reflect the inverted modem inputs
  (section 3.3.3). Per-input modem status interrupts live in
  `IMSC[3:0]`.
- Driver. The CTS flag in `UARTFR` is consulted in `pl011_irq_tx_enable`
  to detect CTS-blocked state (`uart_pl011.c:436`). DSR, DCD, RI flags
  are defined in the header (`uart_pl011_registers.h:49-56`) but never
  referenced in `uart_pl011.c`. The CTS interrupt source (`CTSMIM`) is
  managed (set when CTS blocks TX, cleared on ISR entry) but the other
  modem interrupts (`RIMIM`, `DCDMIM`, `DSRMIM`) are only touched via the
  collective `PL011_IMSC_MASK_ALL` clear at
  `uart_pl011.c:640`.

### 1.16 IrDA SIR (SIREN / SIRLP / ILPR)

- Datasheet. `UARTCR[1]=SIREN` enables the IrDA SIR ENDEC;
  `UARTCR[2]=SIRLP` selects low-power IrDA mode; `UARTILPR` holds the
  low-power divisor (sections 2.4.4, 3.3.4, 3.3.8).
- Driver. SIREN is explicitly cleared at the end of `pl011_init`
  (`uart_pl011.c:645`). SIRLP and ILPR are never written. The driver
  therefore does not support IrDA SIR; it only ensures SIREN is off for a
  non-SBSA instance.

### 1.17 DMA control (DMACR)

- Datasheet. `UARTDMACR` has three meaningful bits: `RXDMAE`, `TXDMAE`,
  `DMAONERR` (section 3.3.14).
- Driver. `dmacr` is written exactly once, to zero, on non-SBSA init
  (`uart_pl011.c:643`), disabling both DMA request paths. No UART DMA is
  exercised. See section 3.6 for the performance implication.

### 1.18 Loopback (LBE)

- Datasheet. `UARTCR[7]=LBE` feeds TXD back into RXD for diagnostics
  (section 3.3.8).
- Driver. `PL011_CR_LBE` is defined (`uart_pl011_registers.h:98`) but
  never set or cleared in C; no loopback mode is exposed.

### 1.19 Identification registers (PeriphID, PCellID)

- Datasheet. Bytes at offsets `0xFE0-0xFFC` identify the peripheral and
  its revision (sections 3.3.15, 3.3.16). PCellID reads
  `0xB105F00D`.
- Driver. The `struct pl011_regs` in `uart_pl011_registers.h:17-37`
  stops at `dmacr` (offset `0x048`). The ID registers are not mapped
  into the struct and never read. Discrepancy: the driver declines to
  use the hardware's self-identification, so it cannot distinguish PL010
  from PL011, nor between PL011 revisions, at runtime.

### 1.x Variability (Functional)

- `CONFIG_UART_USE_RUNTIME_CONFIGURE` (`uart_pl011.c:353-370`,
  `uart_pl011.c:538-541`, `uart_pl011.c:549-552`) gates the
  `.configure` / `.config_get` entries on both API tables, and therefore
  whether line format, parity, stop bits, data bits and baud rate can
  change after init.
- `CONFIG_UART_PL011_SBSA` (`uart_pl011.c:795-831`) selects an
  instantiation block whose data struct presets `.sbsa = true`
  (`uart_pl011.c:817-819`). Throughout the driver, `data->sbsa`
  short-circuits functional configuration: `pl011_is_readable`
  skips the `UARTEN && RXE` gate (`uart_pl011.c:196-199`),
  `pl011_runtime_configure_internal` returns `-ENOTSUP` without
  touching any register (`uart_pl011.c:266-268`), and `pl011_init`
  bypasses the `if (!data->sbsa)` bodies for pinctrl, the vendor
  `pwr_on_func`, the initial disable, `clk_enable_func`, the
  internal runtime-configure call, IFLS programming and FIFO enable
  (`uart_pl011.c:603-637`), as well as the later `if (!data->sbsa)`
  body covering DMACR clear and the SIREN/RXE/TXE rewrite
  (`uart_pl011.c:642-648`), and the final `pl011_enable`
  (`uart_pl011.c:657-659`). `pl011_irq_tx_ready` /
  `pl011_irq_rx_ready` skip the `TXE` / `RXE` gates
  (`uart_pl011.c:470-472, 494-496`). Note: the reset call at
  `uart_pl011.c:582-589` and the generic `clock_control_on` /
  `clock_control_get_rate` calls at `uart_pl011.c:591-595` are
  outside the `if (!data->sbsa)` guards and therefore not
  SBSA-gated; reset is unreachable for an independent reason (see 2.5
  and 2.9: generated configs never populate `.reset`), and clock
  control runs for both SBSA and non-SBSA if CONFIG_CLOCK_CONTROL is
  enabled and a `clock_dev` is populated.
- `CONFIG_UART_INTERRUPT_DRIVEN` + per-DT `interrupts` presence is folded
  into `PL011_USE_IRQ` (`uart_pl011.c:37-40`) and `PL011_NODE_USE_IRQ(n)`
  (`uart_pl011.c:740-741`). It controls the presence of the interrupt-driven
  API, the FIFO fill/read entries, and the TX-enable drain loop and
  ISR.
- The devicetree boolean `fifo-disable` (`arm_pl011.yaml:11-13`,
  `uart_pl011.c:57, 634, 763`) causes the init path and the runtime
  reconfigure re-enable path to skip `pl011_enable_fifo`, collapsing both
  directions to 1-byte holders.
- Vendor specialisations `ambiq_pl011_uart`
  (`uart_pl011.c:79-81`) and `raspberrypi_pico_uart`
  (`uart_pl011.c:83-85`) pull in additional headers that define per-compat
  `pwr_on_*` and `clk_enable_*` functions, hooked via the
  `COMPAT_SPECIFIC_*` macros (`uart_pl011.c:664-683`). These can change
  what happens before the common disable / clock-enable / configure
  sequence in `pl011_init`. The Ambiq variant also maps vendor clock-select
  fields into bits 3 and 6:4 of `cr` (`uart_pl011_registers.h:108-117`).

---

## 2. Integration Analysis

This section asks: which Zephyr kernel subsystems does the stock driver
integrate the PL011 device into, and how?

### 2.1 UART driver API surface (polling)

- Driver. Two `struct uart_driver_api` tables are populated:
  - `pl011_driver_api_noirq` (`uart_pl011.c:534-542`), marked
    `__maybe_unused`, contains `.poll_in`, `.poll_out`, `.err_check`, plus
    `.configure` / `.config_get` gated by
    `CONFIG_UART_USE_RUNTIME_CONFIGURE`.
  - `pl011_driver_api` (`uart_pl011.c:545-567`), compiled only under
    `PL011_USE_IRQ`, adds `.fifo_fill`, `.fifo_read`, the `.irq_tx_*`,
    `.irq_rx_*`, `.irq_err_*`, `.irq_is_pending`, `.irq_update`, and
    `.irq_callback_set` entries.
- The chosen table per instance is `PL011_DEVICE_API(n)`
  (`uart_pl011.c:743-747`): with `CONFIG_UART_INTERRUPT_DRIVEN` and a
  DT node that has an `interrupts` property, `pl011_driver_api` wins;
  otherwise `pl011_driver_api_noirq` is used. This is passed as the final
  argument to `DEVICE_DT_INST_DEFINE` (`uart_pl011.c:789-791, 821-827`).

### 2.2 UART driver API surface (interrupt-driven)

- Driver. Populated as above, under `PL011_USE_IRQ`. Each function is
  defined between `uart_pl011.c:373` and `uart_pl011.c:531`. There is no
  `.callback_set`, no `.tx`, no `.rx_enable`, no `.tx_abort`, no
  `.rx_disable`, no `.rx_buf_rsp`. Zephyr's async UART API is therefore
  not exposed by this driver; this is itself an integration-dimension
  finding.

### 2.3 Console integration

- Driver. The provided C source contains no console-registration code
  and no early-console glue. The only surfaces the driver exposes are
  its UART API tables (`uart_pl011.c:534-567`) and device definition
  at `PRE_KERNEL_1` priority (`uart_pl011.c:790, 826`). Any binding
  through Zephyr's generic console subsystem is outside the provided
  source set and therefore outside this analysis's audit scope.

### 2.4 Logging and shell backends

- Driver. The provided C source contains no logging registration (no
  `LOG_MODULE_REGISTER`, no include of any Zephyr logging header) and
  no shell-backend registration. Any binding through Zephyr's generic
  logging or shell subsystems is outside the provided source set.

### 2.5 Devicetree binding and source-level compatibles

- Binding. `arm_pl011.yaml` declares a single compatible
  (`arm_pl011.yaml:3`), includes `uart-controller.yaml`,
  `pinctrl-device.yaml`, and `reset-device.yaml`
  (`arm_pl011.yaml:5`), requires the `reg` property
  (`arm_pl011.yaml:8-9`), and adds the `fifo-disable` boolean
  (`arm_pl011.yaml:11-13`). Standard `current-speed`,
  `hw-flow-control`, `interrupts`, `interrupt-names`, `clocks`, and
  `pinctrl-0` properties come from `uart-controller.yaml` /
  `pinctrl-device.yaml` / `reset-device.yaml` and are consumed at
  `uart_pl011.c:776` (current_speed), `:780` (hw_flow_control),
  `:702-704` (clocks), `:687` (pinctrl), and `:753` (interrupt_names).
  The `resets` property is declared by the included
  `reset-device.yaml` but is not wired into generated configs: the
  `RESET_INIT(n)` macro (`uart_pl011.c:693-695`) is defined yet never
  expanded inside `PL011_CONFIG_PORT(n)` or `PL011_SBSA_CONFIG_PORT(n)`
  (`uart_pl011.c:749-766, 800-812`), so `.reset.dev` remains zero-
  initialized for every generated instance.
- Source-level compatibles. `arm,sbsa-uart` is handled only at source
  level via an `#undef DT_DRV_COMPAT` / redefine sequence
  (`uart_pl011.c:9-10, 797-798`) inside
  `#ifdef CONFIG_UART_PL011_SBSA` (`uart_pl011.c:795, 831`). SBSA
  instances take the `PL011_SBSA_INIT` path and get a minimal
  `pl011_cfg_sbsa_##n` (`uart_pl011.c:800-812`) plus a
  `pl011_data_sbsa_##n` with `.sbsa = true`
  (`uart_pl011.c:817-819`). Both paths register through
  `DEVICE_DT_INST_DEFINE` with identical init function, init level,
  priority, and API selector; only their config / data are different.

### 2.6 Device initialization lifecycle

- Driver. Every instance is defined with `DEVICE_DT_INST_DEFINE(n,
  pl011_init, PM_INST_GET(n), ..., PRE_KERNEL_1,
  CONFIG_SERIAL_INIT_PRIORITY, PL011_DEVICE_API(n))`
  (`uart_pl011.c:789-791`). The SBSA path differs only in passing NULL
  for the PM device (`uart_pl011.c:821-827`). The init function is
  `pl011_init` (`uart_pl011.c:570-662`).
- There is no `.remove` or teardown callback; Zephyr has no device-remove
  lifecycle for this API, and the driver does not provide one. Absence is
  itself an integration finding.

### 2.7 Power management

- Driver. For Ambiq SoCs (`CONFIG_SOC_SERIES_APOLLO3X` or
  `CONFIG_SOC_SERIES_APOLLO5X`), `PM_INST_DEFINE(n)` expands to
  `PM_DEVICE_DT_INST_DEFINE(n, uart_ambiq_pm_action)` and
  `PM_INST_GET(n)` returns the PM device
  (`uart_pl011.c:106-111`). Otherwise both macros collapse to empty /
  `NULL` (`uart_pl011.c:113-117`), so `DEVICE_DT_INST_DEFINE` is called
  with a null PM device (`uart_pl011.c:789, 821`). The
  `uart_ambiq_pm_action` symbol is referenced from the vendor header
  `uart_pl011_ambiq.h` that `uart_pl011.c:79-81` conditionally
  includes; that header is not among the provided audit inputs, so the
  symbol's definition is outside the files this analysis is bound to.
  There is no generic PL011 PM handler.

### 2.8 Interrupt controller integration

- Driver. Per-instance, a compile-time generated function
  `pl011_irq_config_func_##n` is emitted only when
  `PL011_NODE_USE_IRQ(n)` is true (`uart_pl011.c:749-756`). Its body uses
  `DT_INST_FOREACH_PROP_ELEM(n, interrupt_names,
  PL011_IRQ_CONFIG_FUNC_BODY)` (`uart_pl011.c:753-754`), where
  `PL011_IRQ_CONFIG_FUNC_BODY` (`uart_pl011.c:730-738`) calls
  `IRQ_CONNECT(DT_IRQ_BY_IDX(...), DT_IRQ_BY_IDX(...), pl011_isr,
  DEVICE_DT_GET(n), 0)` and then `irq_enable(...)`. This means the driver
  connects each entry in the DT `interrupt-names` list to the same
  `pl011_isr` handler, with vector / priority sourced from
  devicetree. Multi-line IRQ descriptors collapse to one ISR that
  inspects `UARTMIS` only -- to acknowledge CTS if it fired -- and
  then invokes the registered callback (`uart_pl011.c:710-727`); the
  readiness helpers that inspect `UARTFR` / `UARTRIS` / `UARTIMSC`
  (`uart_pl011.c:465-500`) run from the callback, not from the ISR
  itself. The SBSA path declares its own
  `pl011_irq_config_func_sbsa_##n` (`uart_pl011.c:801-806`). Only the
  CTS modem source is explicitly acknowledged inside the ISR
  (`uart_pl011.c:716-719`).

### 2.9 Kconfig surface

- `CONFIG_UART_INTERRUPT_DRIVEN`: folded into `PL011_USE_IRQ`
  (`uart_pl011.c:37-40`) and `PL011_NODE_USE_IRQ(n)`
  (`uart_pl011.c:740-741`). Controls presence of the interrupt API, ISR,
  and IRQ config function.
- `CONFIG_UART_USE_RUNTIME_CONFIGURE`: gates `pl011_runtime_configure` and
  `pl011_runtime_config_get` (`uart_pl011.c:353-370`) and their entries
  in both API tables (`uart_pl011.c:538-541, 549-552`).
- `CONFIG_UART_PL011_SBSA`: enables the SBSA instantiation block
  (`uart_pl011.c:795-831`).
- `CONFIG_PINCTRL`: makes `pinctrl_dev_config` appear in the config
  struct (`uart_pl011.c:44-46`), enables the `pinctrl_apply_state` call
  in init (`uart_pl011.c:604-609`), and `PINCTRL_DEFINE` / `PINCTRL_INIT`
  macros (`uart_pl011.c:685-691`).
- `CONFIG_RESET`: makes the `reset_dt_spec reset` field appear in the
  config struct (`uart_pl011.c:47-49`), guards the
  `reset_line_toggle_dt` call in `pl011_init` behind
  `if (config->reset.dev)` (`uart_pl011.c:582-589`), and defines the
  `RESET_INIT(n)` macro (`uart_pl011.c:693-698`). Finding: the
  generated config structs in `PL011_CONFIG_PORT(n)`
  (`uart_pl011.c:749-766`) and `PL011_SBSA_CONFIG_PORT(n)`
  (`uart_pl011.c:800-812`) do not expand `RESET_INIT(n)`, so
  `.reset.dev` is always zero-initialized and
  `reset_line_toggle_dt` is never actually invoked in this source.
  The reset code path is present but unreachable for generated
  instances as written.
- `CONFIG_CLOCK_CONTROL`: makes `clock_dev`, `clock_id` appear in config
  (`uart_pl011.c:50-53`) and enables `clock_control_on` /
  `clock_control_get_rate` in init (`uart_pl011.c:591-596`).
- `CONFIG_SOC_SERIES_APOLLO3X`, `CONFIG_SOC_SERIES_APOLLO5X`: select the
  Ambiq PM handler definition (`uart_pl011.c:106-117`).
- `CONFIG_CPU_CORTEX_M`: conditionally pulls in `cmsis_compiler.h`
  (`uart_pl011.c:31-33`). `DEVICE_MMIO_*` and the barrier intrinsics
  are resolved through `zephyr/sys/device_mmio.h` and
  `zephyr/sys/barrier.h` (`uart_pl011.c:18-19`), independent of this
  gate.
- `CONFIG_SERIAL_INIT_PRIORITY`: passes through as the init priority for
  every instance (`uart_pl011.c:790, 826`).

### 2.x Variability (Integration)

- API table selection (polling-only vs. polling+IRQ) is decided per
  instance by `PL011_DEVICE_API(n)` (`uart_pl011.c:743-747`) and depends
  on `CONFIG_UART_INTERRUPT_DRIVEN` and the presence of `interrupts` in
  DT.
- PM device is either the Ambiq handler or NULL depending on
  `CONFIG_SOC_SERIES_APOLLO*` (`uart_pl011.c:106-117`), flowing into
  `DEVICE_DT_INST_DEFINE`'s second argument.
- The SBSA instantiation block redefines `DT_DRV_COMPAT` and uses a
  reduced config struct: it has no `fifo_disable`, no `pincfg`, no
  `reset`, no `clk_enable_func`, and no `pwr_on_func`
  (`uart_pl011.c:808-812`); these fields are zero-initialised or absent
  in the SBSA config, and the SBSA path in `pl011_init` ignores them via
  `data->sbsa` guards.
- Clocks-based baud rate: when the DT `clocks` phandle points at a
  `fixed-clock`, `clk_freq` is read from the `clock-frequency` property
  (`uart_pl011.c:784-786`) and no clock-control API is used. Otherwise
  `CLOCK_INIT(n)` populates `clock_dev` / `clock_id`
  (`uart_pl011.c:700-704`), and init calls `clock_control_on` and
  `clock_control_get_rate` (`uart_pl011.c:591-596`). This is a DT-driven
  branch, not a Kconfig branch.
- `COMPAT_SPECIFIC_PWR_ON_FUNC` / `COMPAT_SPECIFIC_CLK_ENABLE_FUNC`
  select a compat-prefixed function by taking the first compatible
  string of the DT node (`uart_pl011.c:671-683`). The generic
  `arm,pl011` compat resolves to the no-op
  `pwr_on_arm_pl011` / `clk_enable_arm_pl011`
  (`uart_pl011.c:91-101`); `ambiq,pl011-uart` and
  `raspberrypi,pico-uart` supply their own via included headers
  (`uart_pl011.c:79-85`).

---

## 3. Performance Analysis

This section asks: which PL011 performance features does the driver
implement, and which Zephyr-side performance facilities does the driver
use to change throughput, latency, or CPU cost?

### 3.1 FIFO enablement as amortization

- Datasheet. With `LCR_H.FEN=1`, TX and RX FIFOs are 32 entries deep,
  turning one IRQ / FIFO-threshold crossing into a batched handling of
  many bytes (sections 2.2.4, 2.2.5, 2.4.3).
- Driver. FIFO is enabled by default in `pl011_init`
  (`uart_pl011.c:634-636`) unless `fifo-disable` overrides it
  (`uart_pl011.c:57, 763`). The reconfigure path preserves this
  (`uart_pl011.c:343-345`). The polling path does not benefit from FIFO
  batching directly, since `pl011_poll_out` writes a single byte
  (`uart_pl011.c:229`); FIFO depth here serves only to absorb bursts
  between poll calls.

### 3.2 Trigger-level tuning (IFLS)

- Datasheet. TXIFLSEL / RXIFLSEL choose 1/8, 1/4, 1/2, 3/4, 7/8 for
  TX-empty and RX-fill trigger thresholds (section 3.3.9). The reset
  value is 1/2 on both.
- Driver. `pl011_init` explicitly programs `TXIFLSEL = 1/8` and
  `RXIFLSEL = 1/2` (`uart_pl011.c:629-631`). TXIFLSEL at 1/8 fires the
  refill IRQ only once the TX FIFO has drained to the "nearly empty"
  threshold; this reduces TX interrupt frequency and maximises the
  refill batch opportunity per IRQ, but it leaves less headroom against
  TX FIFO underrun than a higher threshold would offer. RXIFLSEL at
  1/2 coalesces roughly half the RX FIFO depth before raising
  `UARTRXINTR`, trading per-byte RX latency for fewer RX interrupts.
  Both values are hard-coded; the driver exposes no per-instance
  override.

### 3.3 RX timeout interrupt (RTIM)

- Datasheet. `UARTRTINTR` fires when RX FIFO is non-empty and no further
  character arrives for 32 bit-times (section 2.8.4). This is a
  performance feature because it allows bursty traffic to be delivered
  even when the IFLS watermark is not reached, without polling.
- Driver. `pl011_irq_rx_enable` sets both `RXIM` and `RTIM` together
  (`uart_pl011.c:481`). The two are armed and disarmed as a pair
  (`uart_pl011.c:486`), so using interrupt-driven RX always costs one
  extra source but yields bounded latency for short messages that don't
  fill to RXIFLSEL.

### 3.4 Direct FIFO copy loops

- Driver. `pl011_fifo_fill` and `pl011_fifo_read` loop directly on `dr`
  with `!TXFF` and `!RXFE` checks
  (`uart_pl011.c:373-396`), bounded by the caller-supplied `len`. There
  is no intermediate buffering and no per-byte overhead beyond the
  MMIO read and flag check. The loops are single-pass and do not release
  the CPU.

### 3.5 Software-triggered first TX interrupt

- Datasheet. `UARTTXINTR` is level-transition-based (section 2.8.3
  note). Asserting `TXIM` while the FIFO is already below the trigger
  level yields no interrupt until fresh data is put in and the level
  crosses again.
- Driver. To work around this, `pl011_irq_tx_enable` carries a
  `sw_call_txdrdy` flag on `pl011_data` (`uart_pl011.c:69`). On the
  first call after a disable or a CTS-block, the flag is set;
  `pl011_irq_tx_enable` then invokes the registered callback in a loop
  with `TXIM` held enabled (`uart_pl011.c:434-446`), draining as much
  data as possible until either the mask is cleared by the callback or
  CTS blocks transmission. The flag is reset on `irq_tx_disable`
  (`uart_pl011.c:454`) and re-armed when CTS blocks
  (`uart_pl011.c:438`). Performance effect: the TX engine starts without
  having to rely on a real hardware edge, and short transfers that would
  never cross the 1/8 threshold still complete.

### 3.6 API-mode selection: polling vs. interrupt-driven

- Driver. Polling path: `pl011_poll_out` busy-spins reading `fr` until
  `!TXFF`, then issues one `dr` write (`uart_pl011.c:218-230`), so its
  per-byte cost is N reads of `fr` plus one write of `dr`, where N
  depends on TX FIFO occupancy at call time. `pl011_poll_in` reads
  `cr` and `fr` through `pl011_is_readable` (`uart_pl011.c:190-202`),
  returns -1 if no data, and otherwise reads `dr`
  (`uart_pl011.c:204-216`). Latency is a function of caller reschedule
  behaviour; CPU cost per byte varies with FIFO state and caller
  polling cadence.
- Interrupt-driven path: `pl011_isr` calls the registered callback under
  `irq_cb_lock` (`uart_pl011.c:710-727`). Cost per IRQ entry includes
  the spinlock acquire, the ISR itself, and the callback body; cost per
  byte depends on how `fifo_fill` / `fifo_read` are used by the callback.
- The mode is selected per instance at compile time via the DT
  `interrupts` property and `CONFIG_UART_INTERRUPT_DRIVEN`
  (`uart_pl011.c:37-40, 743-747`). There is no runtime switch.

### 3.7 Absence of DMA

- Datasheet. PL011 exposes a DMA interface (section 2.7) with burst /
  single request pairs and a `DMAONERR` safety bit, plus per-direction
  enables in `UARTDMACR`.
- Driver. `pl011_init` writes `dmacr = 0` (`uart_pl011.c:643`) and never
  re-enables it. There are no DMA-related API entries. Recording this
  per the brief's Performance scope: DMA acceleration is not available
  through this driver.

### 3.8 Absence of ring / intermediate buffers

- Driver. There is no `ring_buf` used anywhere in
  `uart_pl011.c` and no internal per-instance TX / RX buffer in
  `struct pl011_data` (`uart_pl011.c:63-74`). Interrupt-driven
  callers are expected to own their buffers and to drive
  `pl011_fifo_fill` / `pl011_fifo_read` directly from them. This keeps
  the driver light but means RX backpressure is entirely on the
  consumer.

### 3.9 MMIO cacheability

- Driver. `DEVICE_MMIO_MAP(dev, K_MEM_CACHE_NONE)` is issued at the top
  of `pl011_init` (`uart_pl011.c:577`). The mapping is fixed at
  uncached; the driver does not expose a per-instance cacheability
  knob.

### 3.10 Barriers around reconfigure

- Driver. `pl011_set_baudrate` emits `barrier_dmem_fence_full()` after
  writing IBRD/FBRD (`uart_pl011.c:179`). `pl011_init` emits
  `barrier_isync_fence_full()` around the post-DMACR-clear CR writes
  (`uart_pl011.c:644, 647`). These fences are part of the reconfigure
  path and are recorded here because they bound how cheap a
  reconfigure can be.

### 3.x Variability (Performance)

- `fifo-disable` collapses both FIFOs to 1-byte holders, eliminating
  batching entirely (`uart_pl011.c:634-636, 343-345`,
  `arm_pl011.yaml:11-13`).
- The interrupt-driven TX drain loop in `pl011_irq_tx_enable`
  (`uart_pl011.c:434-446`) only exists when `PL011_USE_IRQ` is true.
  Polling-only builds therefore have no first-TX workaround because they
  do not use the TX interrupt at all.
- CTS-aware exit from the drain loop (`uart_pl011.c:436-441`) only
  matters when the flow-control configuration has set CTSEn. Without
  CTSEn, the loop condition is `while (uart->imsc & PL011_IMSC_TXIM)`
  (`uart_pl011.c:434-446`), so the only termination is the callback
  clearing `TXIM`.
- RXIFLSEL / TXIFLSEL values are constants in the driver. Any tuning
  requires a code change, not a DT / Kconfig option.

---

## 4. Correctness Analysis

This section asks: what invariants, disciplines, and ordering rules govern
how the Zephyr driver exercises the device safely?

### 4.1 Concurrency discipline: irq_cb_lock

- Driver. The registered UART IRQ callback is invoked from two places,
  and in both cases is wrapped in `K_SPINLOCK(&data->irq_cb_lock)`:
  - The ISR `pl011_isr` (`uart_pl011.c:710-727`), specifically at
    `uart_pl011.c:723-725`.
  - The TX drain loop inside `pl011_irq_tx_enable`
    (`uart_pl011.c:443-445`).
- Invariant. The callback body must tolerate re-entrant invocation
  patterns from both ISR context and thread context (the drain loop runs
  under whoever called `uart_irq_tx_enable`, typically a cooperative or
  preemptible thread). No unbounded work, no blocking primitives, no
  `k_sleep` in the callback; the ISR rule applies transitively.
- `irq_cb` and `irq_cb_data` are set in `pl011_irq_callback_set`
  without holding `irq_cb_lock` (`uart_pl011.c:523-531`). Hypothesis:
  the driver assumes `irq_callback_set` is called before any call to
  `irq_tx_enable` or `irq_rx_enable`, i.e. during device setup, not
  concurrently with an active IRQ. A confirmation would require checking
  the Zephyr `uart_irq_callback_set` contract.
- `sw_call_txdrdy` is declared `volatile bool` (`uart_pl011.c:69`) and is
  touched from both the ISR-adjacent enable/disable paths
  (`uart_pl011.c:404, 407, 438, 454, 653`) without being covered by
  `irq_cb_lock`. Hypothesis: the flag is treated as a single-word
  observation that is monotonic under the intended usage (disable clears,
  enable reads/sets); simultaneous in-flight enable and disable is
  assumed not to occur. Finding: there is no explicit synchronisation
  between `sw_call_txdrdy` writes and reads across contexts.

### 4.2 Hardware-mandated ordering: UARTEN = 0 before reprogramming

- Datasheet. `UARTLCR_H`, `UARTIBRD`, and `UARTFBRD` must not be changed
  while the UART is enabled (section 3.3.7 note). The recommended
  programming sequence in section 3.3.8 is: disable, wait for completion,
  flush FIFO, reprogram, re-enable.
- Driver. `pl011_runtime_configure_internal` with `disable = true`:
  calls `pl011_disable` and `pl011_disable_fifo`
  (`uart_pl011.c:270-273`), reprograms LCR_H / IBRD / FBRD via
  `pl011_set_baudrate` and the `lcrh = uart->lcr_h = lcrh` sequence
  (`uart_pl011.c:275-337`), and re-enables with `pl011_enable_fifo`
  (if configured) and `pl011_enable` (`uart_pl011.c:342-347`). The
  enabling post-step happens on both the success path and most
  error-returns: failure in the middle of parity / stop / data bits /
  flow / baud configuration goto-jumps to the `enable` label
  (`uart_pl011.c:288, 299, 316, 327, 333, 341-347`). Correctness
  consequence: LCR_H and IBRD / FBRD are NOT half-updated on these
  error paths -- LCR_H is only written at `uart_pl011.c:337` which is
  past every `goto enable`, and `pl011_set_baudrate` rejects an invalid
  divisor at `uart_pl011.c:171-177` before touching `ibrd` / `fbrd`.
  The one register that can be left with new-config state after a
  later error is `UARTCR`: `pl011_set_flow_control` writes CR at
  `uart_pl011.c:139-157` before the baud check runs, so a valid
  flow-control value followed by an invalid baud value results in the
  re-enable running with the new flow-control bits but the old baud.
- Datasheet-vs-driver finding (see also 4.10): the datasheet's
  recommended sequence in `pl011-minimized-datasheet.md:1072-1078` is
  disable, then wait for end of transmission / reception, then flush
  the TX FIFO (FEN=0), then reprogram, then re-enable. The driver
  disables UART and clears FEN immediately, with no `UARTFR.BUSY`
  poll between disable and reprogram
  (`uart_pl011.c:270-273, 337`), meaning a character still in flight
  when `uart_configure` is called may be corrupted.
- `pl011_init` performs a similar sequence: `pl011_disable` +
  `pl011_disable_fifo` (`uart_pl011.c:616-617`), clock-enable,
  reconfigure-internal with `disable = false` (`uart_pl011.c:627`),
  IFLS, optional `pl011_enable_fifo` (`uart_pl011.c:629-636`), IMSC /
  ICR (`uart_pl011.c:639-640`), DMACR clear (`uart_pl011.c:643`),
  SIREN clear + RXE/TXE set (`uart_pl011.c:645-646`), and finally
  `pl011_enable` (`uart_pl011.c:658`). The first-time call uses
  `disable = false` because the UART is already disabled.

### 4.3 LCR_H commit for IBRD/FBRD updates

- Datasheet. IBRD and FBRD are latched internally only on the next write
  to LCR_H (section 3.3.7).
- Driver. `pl011_set_baudrate` ends with `uart->lcr_h = uart->lcr_h;`
  (`uart_pl011.c:185`), a read-modify-write that preserves the current
  LCR_H value and issues the required write strobe. The preceding
  `barrier_dmem_fence_full()` (`uart_pl011.c:179`) orders the IBRD /
  FBRD stores against the LCR_H commit.
- In the runtime reconfigure path, LCR_H is written twice in sequence:
  first inside `pl011_set_baudrate` as `uart->lcr_h = uart->lcr_h`
  (`uart_pl011.c:185`), which commits IBRD/FBRD while preserving the
  existing line-format bits; then again in
  `pl011_runtime_configure_internal` as `uart->lcr_h = lcrh`
  (`uart_pl011.c:337`), which installs the freshly rebuilt line format.
  The first write commits the new baud divisor with the stale format;
  the second replaces the format. Both satisfy the datasheet rule that
  a LCR_H write is what commits IBRD/FBRD. The surrounding
  runtime-configure path disables UART before this pair of writes and
  re-enables it afterwards (`uart_pl011.c:270-273, 342-347`), so the
  two-write intermediate state is harmless once the UART is truly
  idle. Since the driver does not wait for `UARTFR.BUSY` to clear
  between disable and the LCR_H writes, however, reprogramming can
  still overlap a completing character
  (`pl011-minimized-datasheet.md:1040-1042`). The correctness risk
  tied to this pair is therefore the missing BUSY wait (see 4.2), not
  the two-step update itself.

### 4.4 State-machine invariant: sw_call_txdrdy

- Invariant (as stated in the driver comment at
  `uart_pl011.c:408-420`). PL011 TX interrupt is level-transition-based;
  enabling `TXIM` without prior data does not fire an interrupt. The
  driver therefore guarantees that the first `uart_irq_tx_enable` after
  a `uart_irq_tx_disable` or CTS-block will invoke the callback
  manually.
- Mechanism. `sw_call_txdrdy` is set to true in `pl011_irq_tx_disable`
  (`uart_pl011.c:454`), is set on init when IRQ is configured
  (`uart_pl011.c:653`), is cleared on entry to
  `pl011_irq_tx_enable` once the flag is observed
  (`uart_pl011.c:404-407`), and is re-set when the drain loop exits due
  to CTS blocking transmission (`uart_pl011.c:438`). The drain loop
  itself is `while (uart->imsc & PL011_IMSC_TXIM)` with an inner
  spinlock (`uart_pl011.c:434-446`), so the loop terminates as soon as
  the callback clears `TXIM` (typical Zephyr pattern) or CTS blocks.

### 4.5 CTS recovery via CTSMIM

- Invariant. If CTS blocks transmission during the drain loop, the
  driver must (a) stop the drain (to avoid unbounded looping while TX
  cannot proceed), (b) arrange to resume when CTS clears, and
  (c) re-arm the sw-callback flag.
- Mechanism. All three happen at `uart_pl011.c:436-442`: it sets
  `sw_call_txdrdy = true` (re-arm), ORs `CTSMIM` into `imsc` (so that
  a CTS-edge raises the combined UART interrupt), and breaks the loop.
  The ISR handles the resulting CTS interrupt by clearing the raw source
  and masking CTS again (`uart_pl011.c:715-719`), and then invokes the
  registered callback, which has the opportunity to re-arm TX.
  Hypothesis: the intended contract is that the callback calls
  `uart_irq_tx_enable` again once CTS permits; the driver does not
  verify or enforce this.

### 4.6 Bounded ISR work

- Invariant. ISRs must terminate in bounded work.
- Driver. `pl011_isr` performs one conditional `icr` write, one
  conditional `imsc` mask clear, and one bounded callback invocation
  under spinlock (`uart_pl011.c:710-727`). No loops. The callback is
  whatever the consumer registered; the driver assumes the consumer
  respects the ISR-safety contract.

### 4.7 Volatile MMIO and barriers

- Invariant. All device register accesses must be visible to hardware
  and not reordered past subsequent dependent accesses; explicit
  ordering is required around reprogramming sequences.
- Mechanism. The register struct pointer is `volatile struct pl011_regs *`
  (`uart_pl011_registers.h:39-43`), obtained through
  `DEVICE_MMIO_GET(dev)` which returns the mapped MMIO base. Explicit
  barriers:
  - `barrier_dmem_fence_full()` after IBRD / FBRD writes, before the
    LCR_H commit (`uart_pl011.c:179`).
  - `barrier_isync_fence_full()` before and after the CR SIREN /
    RXE / TXE sequence (`uart_pl011.c:644, 647`).
- There are no barriers around the poll-in / poll-out path or around the
  fifo-fill / fifo-read loops. Hypothesis: on the supported targets,
  volatile MMIO plus device-memory semantics at the mapping stage
  (`K_MEM_CACHE_NONE` at `uart_pl011.c:577`) are deemed sufficient for
  ordering between consecutive register accesses in the same thread.

### 4.8 Error-path recovery in pl011_init

- Driver. `pl011_init` contains three early-return paths:
  - `reset_line_toggle_dt` failure (`uart_pl011.c:582-588`), guarded
    by `if (config->reset.dev)`. Because no generated config in this
    source populates `.reset` (see 2.9 CONFIG_RESET bullet), this
    path is unreachable for every generated instance as written; the
    failure-handling code is dead.
  - `pinctrl_apply_state` failure (`uart_pl011.c:604-608`).
  - `config->clk_enable_func` failure (`uart_pl011.c:620-624`).
- On each reachable early-return, `pl011_init` returns the error and
  Zephyr's device framework marks the instance unready. No partial
  cleanup is attempted. The state established before each reachable
  early-return differs:
  - `pinctrl_apply_state` failure (`uart_pl011.c:604-608`) happens
    after the ignored `clock_control_on` / `clock_control_get_rate`
    calls (`uart_pl011.c:591-595`), so the clock provider may already
    have been turned on and `data->clk_freq` may already have been
    mutated before the early-return.
  - `clk_enable_func` failure (`uart_pl011.c:620-624`) happens after
    `pwr_on_func` may have executed with its return value ignored
    (`uart_pl011.c:611-613`) and after `pl011_disable` /
    `pl011_disable_fifo` (`uart_pl011.c:616-617`). Whether the
    ignored vendor `pwr_on_func` side effects require undoing cannot
    be determined from the provided audit inputs because the vendor
    headers (`uart_pl011_ambiq.h`,
    `uart_pl011_raspberrypi_pico.h`) that define those hooks are
    outside this analysis's source set.
- Finding. Return values of `clock_control_on` and
  `clock_control_get_rate` are NOT checked
  (`uart_pl011.c:591-595`). Init therefore proceeds with whatever
  value `data->clk_freq` holds after those ignored calls. For
  non-fixed-clock DT nodes, `pl011_data` is initialised with
  `clk_freq = 0` (`uart_pl011.c:784-786`); if no clock provider
  successfully populated it, `pl011_set_baudrate` sees `clk = 0` and
  returns `-EINVAL` (`uart_pl011.c:171-174`). That return is observed
  inside `pl011_runtime_configure_internal` only to `goto enable`
  (`uart_pl011.c:331-334`), which does nothing during init because
  `disable == false`; the `ret` value is discarded by `pl011_init`
  (`uart_pl011.c:627` does not assign `ret`). Result: the UART is
  brought up with whatever LCR_H / IBRD / FBRD state predated the
  aborted configure, and `pl011_init` returns 0. Hypothesis: the
  stock driver treats clock-control absence as benign, relying on
  DT `clock-frequency` or the vendor `clk_enable_func` to supply
  `clk_freq`; any other failure path is a latent bug. Confirmation
  would need intentional clock-control failure injection.
- Similarly, `config->pwr_on_func(dev)` return value is ignored
  (`uart_pl011.c:611-613`); only `clk_enable_func`'s return is
  honoured (`uart_pl011.c:620-624`).

### 4.9 IRQ enable / disable ordering at shutdown

- Invariant (datasheet-derived). IMSC should be cleared before any IRQ
  release, so that no residual interrupts fire into a torn-down handler.
- Driver. `pl011_init` sets `uart->imsc = 0` and
  `uart->icr = PL011_IMSC_MASK_ALL` (`uart_pl011.c:639-640`) BEFORE
  `IRQ_CONNECT` is issued from `config->irq_config_func` at
  `uart_pl011.c:651-654`. This means the driver masks all sources
  before enabling the controller-level line, which is the safe order.
  There is no shutdown path, and `irq_disable` is never called.

### 4.10 SBSA-mode invariant

- Invariant. SBSA-mode instances delegate device configuration to
  firmware / hypervisor; the driver must not write the line / baud
  registers and must reject runtime reconfiguration.
- Mechanism. `data->sbsa == true` is preset by the SBSA
  instantiation (`uart_pl011.c:817-819`). `pl011_runtime_configure_internal`
  returns `-ENOTSUP` on entry with `data->sbsa`
  (`uart_pl011.c:266-268`). `pl011_init` skips the entire
  configuration block inside `if (!data->sbsa)` at
  `uart_pl011.c:603-637` and again at
  `uart_pl011.c:642-648`, and skips the final `pl011_enable`
  (`uart_pl011.c:657-659`). `pl011_is_readable` and
  `pl011_irq_tx_ready` / `pl011_irq_rx_ready` skip the `UARTEN` / TXE
  / RXE gates in SBSA mode (`uart_pl011.c:196-199, 470-472, 494-496`),
  because the firmware owns those bits.

### 4.11 DR-before-RSR read order for error clearing

- Datasheet. Section 3.3.2 note: "The received data character must be
  read first from UARTDR before reading the error status associated
  with that data character from the UARTRSR Register. This read
  sequence cannot be reversed".
- Driver. `pl011_err_check` reads `rsr` only
  (`uart_pl011.c:234`). There is no bookkeeping to ensure a `dr` read
  happened first; the driver relies on callers to interleave
  `poll_in` / `fifo_read` and `err_check` correctly. This is a
  datasheet-vs-driver finding: the contract is not enforced in the
  driver layer, and for overrun (which is set immediately, not per
  FIFO top) it does not matter, but for framing / parity / break the
  reported bits may reflect a different character than the caller
  expects.

### 4.12 Interrupt acknowledge coverage

- Finding. Among the 11 interrupt sources, only the CTS modem status
  source is explicitly acknowledged by the ISR (`uart_pl011.c:716-719`).
  Deassertion semantics for the other sources differ by type (per
  `pl011-minimized-datasheet.md:697-720`): RX raw status deasserts
  when the callback reads enough data through `pl011_fifo_read` to
  drop the RX FIFO level below RXIFLSEL; RT raw status deasserts only
  when the RX FIFO becomes empty (reading below the trigger is not
  sufficient for RT); TX raw status deasserts when the callback writes
  enough data through `pl011_fifo_fill` to raise the TX FIFO level
  above TXIFLSEL. Error sources (FE / PE / BE /
  OE), however, have no `icr`-clear path exposed by this driver:
  `pl011_err_check` reads `rsr` only and never writes `ecr`
  (`uart_pl011.c:232-253`), the ISR clears `icr` only for CTS
  (`uart_pl011.c:716-719`), and no entry in `pl011_driver_api`
  (`uart_pl011.c:545-567`) gives the callback a way to ack the error
  raw status. If an error interrupt's mask bit is enabled and its raw
  status asserts, the interrupt line can therefore remain asserted
  until the next `pl011_init`. This is the correctness-side view of
  the same gap recorded in 4.11.
- Secondary gap. `pl011_irq_err_enable` / `pl011_irq_err_disable`
  expose enabling and disabling the error IRQ masks
  (`uart_pl011.c:502-511`), but `pl011_irq_is_pending` returns only
  `pl011_irq_rx_ready(dev) || pl011_irq_tx_ready(dev)`
  (`uart_pl011.c:513-515`); it does not test `ris` or `mis` for
  error sources. Both `irq_err_enable` and `irq_is_pending` are
  registered in the interrupt-driven API table
  (`uart_pl011.c:562-564`). An error-only pending interrupt therefore
  does not show up through `.irq_is_pending`, even though the ISR
  will still invoke the callback and `pl011_err_check` will still
  observe the error bits in `rsr`.

### 4.x Variability (Correctness)

- With `CONFIG_UART_USE_RUNTIME_CONFIGURE=n`, the
  disable/reprogram/enable sequence in
  `pl011_runtime_configure_internal` (`uart_pl011.c:270-347`) is still
  present (used by `pl011_init` with `disable = false`), but no
  runtime path can invoke it with `disable = true`. The correctness
  surface is therefore smaller.
- With `CONFIG_UART_INTERRUPT_DRIVEN=n` (or no `interrupts` in DT),
  `irq_cb_lock` is not defined in `pl011_data`
  (`uart_pl011.c:68-73`), the ISR is absent, and there is no drain
  loop. All correctness invariants about ISR-vs-thread interleaving
  vanish; only barrier and ordering invariants remain.
- SBSA mode short-circuits the correctness invariants tied to device
  programming: sections 4.2 (UARTEN=0 before reprogramming) and 4.3
  (LCR_H commit for IBRD/FBRD) do not apply because
  `pl011_runtime_configure_internal` returns early for SBSA
  (`uart_pl011.c:266-268`) and `pl011_init` skips the
  line / baud / FIFO / CR programming path
  (`uart_pl011.c:603-648`). The IMSC / ICR init at
  `uart_pl011.c:639-640` still runs, and the IRQ config function still
  runs (`uart_pl011.c:650-654`), so the ISR invariants (4.6) and CTS
  recovery (4.5) still apply to SBSA instances. `pl011_err_check` is
  shared with non-SBSA (`uart_pl011.c:232-253`), so the DR-before-RSR
  concern in 4.11 is unchanged for SBSA too.
- `fifo-disable` changes what the FIFO empty / full flags mean (see
  datasheet note in 2.8.2 / 2.8.3); the driver's use of `TXFF` /
  `RXFE` in `fifo_fill` / `fifo_read` and `poll_out` / `poll_in`
  still works because the flags retain their single-holder-register
  meaning, but the per-interrupt trigger logic degrades to
  "holding register full / empty". The TX level-transition quirk and
  its software workaround in `pl011_irq_tx_enable` still apply.

