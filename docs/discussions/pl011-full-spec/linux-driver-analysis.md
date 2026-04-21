# PL011 Linux Driver -- Driver-Concern Feature Inventory

**Source analyzed**:
`/Users/nyengele/driverbase/os/linux/drivers/tty/serial/amba-pl011.c`
(3124 lines; covers AMBA-bus PL011 and SBSA-UART platform variant;
integrates with serial_core, tty, nbcon console, earlycon, DMA engine,
pinctrl, clk, OF/ACPI, RS-485, PM).

**Purpose**: research input for a coverage-comparison matrix between
the features a production Linux PL011 driver actually expresses and
the features the Regseq OS-agnostic DSL captures in its PL011 spec.
Only **driver-concern features** are in scope (hardware interaction,
OS-subsystem integration, resource acquisition / cleanup, device
binding, concurrency / locking, PM, errata workarounds, policy,
lifecycle). Items trivially expressible as host-language constructs
(struct definitions, control flow, integer math, container_of, static
qualifiers, sparse annotations, MODULE_* boilerplate that exists only
because of host constraints) are out of scope.

**Review status**: APPROVED by Codex adversarial review after 6
iterations (15 -> 6 -> 4 -> 2 -> 2 -> 0 mandatory fixes).

All line numbers below reference the source file above.

## 1. Device Binding and Enumeration
- AMBA-bus binding via `amba_id` table with id/mask (`vendor_arm` at
  0x00041011 mask 0x000fffff; `vendor_st` at 0x00380802 mask
  0x00ffffff) (3073-3086, 3087).
- Secondary driver variant: SBSA UART as `platform_driver` with OF
  `compatible = "arm,sbsa-uart"` and ACPI `ARMH0011` / `ARMHB000`
  (2983-3071, 3048-3059).
- Two driver registrations + registration ordering (platform first,
  then amba) (3100-3107).
- `arch_initcall` (earlier than module_init) so console comes up
  before normal init (3119).
- DT alias-based port numbering via `of_alias_get_id(np, "serial")`
  with fallback to auto-allocation; warns on mixed aliased/unaliased
  devices (2754-2784).
- Runtime enumeration: scan global `amba_ports[UART_NR=14]` for a free
  slot (45, 2358, 2803-2812).
- Device property `reg-io-width` at probe selects UPIO_MEM vs
  UPIO_MEM32 (2907-2920).
- OF property `current-speed` required for SBSA with 115200 fallback
  for non-OF probes (2994-3002).
- Per-vendor data carried by `id->data` and dispatched at probe time
  (103-217, 2880).

## 2. MMIO Access
- Relaxed MMIO reads/writes with width dispatch
  (`readl_relaxed`/`readw_relaxed`, `writel_relaxed`/`writew_relaxed`)
  (295-313).
- IO width runtime choice: UPIO_MEM (16-bit) vs UPIO_MEM32 (32-bit)
  selected per vendor or via device property (298-312, 2900,
  2907-2920).
- Per-vendor register-offset indirection (`reg_offset[]`) maps logical
  register names (`REG_DR`, `REG_FR`, ...) to physical offsets (56-84,
  86-100, 172-197, 263).
- Memory mapping via `devm_ioremap_resource` (2820).
- Busy-wait polling on FR (TXFF, RXFE, BUSY, TXFE) with `cpu_relax()`
  (1174-1175, 1760-1761, 2367-2368, 2540, 2584, 2625-2629,
  2641-2648).
- Write-read fencing for erratum (dummy ICR reads to introduce delay)
  (1574-1589).
- Split register-bank write protocol (LCRH_RX must be followed by
  LCRH_TX after a 10-PCLK delay implemented as 10 dummy MIS writes)
  (1808-1827).
- Register-write ordering constraint: LCRH writes MUST follow IBRD +
  FBRD writes (2209-2215).

## 3. Interrupt Handling
- Shared-IRQ registration with `IRQF_SHARED` via `request_irq`
  (1829-1834).
- Single combined ISR multiplexing RX, TX, modem status, and RX DMA
  (1591-1629).
- ISR bounded-work loop via `AMBA_ISR_PASS_LIMIT = 256` (51, 1618).
- Local shadow of interrupt-mask register (`uap->im`) to avoid RMW on
  IMSC in the dominant runtime path (266).
- TX interrupt enable/disable via `TXIM` (1324-1325, 1338-1341).
- RX / error / modem mask bits (RXIM, RTIM, FEIM, PEIM, BEIM, OEIM,
  RIMIM, CTSMIM, DCDMIM, DSRMIM) (1432-1454).
- Runtime RX-interrupt enable policy: `RTIM` always, `RXIM` only when
  not in RX DMA. Applies to `pl011_enable_interrupts` (1864-1867) and
  `pl011_unthrottle_rx` (1878-1887).
- Initial RX-interrupt setup in `pl011_hwinit` unconditionally writes
  `UART011_RTIM | UART011_RXIM` after clearing pending error+RX IRQs
  (1791-1796).
- Interrupt-clear register `ICR` writes (W1C semantics) (1603-1604,
  1789, 2004).
- ISR pending computation: `RIS & uap->im`, i.e. raw-status masked by
  the IMSC shadow (not by a live IMSC read) (1597-1598, 266).
- Drain spurious RX interrupts at enable time (2*fifosize reads of
  DR) to unstick edge-triggered RXIS (1851-1862).
- SysRq character probe via `uart_prepare_sysrq_char` +
  `uart_unlock_and_check_sysrq` (361, 1626).
- Mask-all + clear-all on shutdown (IMSC=0, ICR=0xffff) (1997-2007).
- CTS-event workaround (W1C unlock + 26ns delay via two dummy reads)
  (1574-1589).
- Console-poll "quiesce" path: clear pending, mask TXIM without
  disturbing the rest (1713-1734).

## 4. FIFO and Framing
- Vendor-specific FIFO size (ARM: 16 or 32 based on `amba_rev`;
  ST: 64) via function-pointer `get_fifosize` (118, 121-124,
  199-202, 2899).
- FIFO enable via LCRH_FEN when fifosize > 1 (2146-2147).
- TX refill threshold at fifosize/2 (1507).
- RX/TX interrupt FIFO-level selection (IFLS: RX4_8|TX4_8 ARM,
  RX_HALF|TX_HALF ST) (128, 206, 1909).
- RX-word error flags OE/BE/PE/FE combined mask `UART_DR_ERROR`
  (53, 337-359).
- Break detection via `uart_handle_break`; break injection via
  LCRH_BRK (338-342, 353-355, 1703-1708).
- Per-char wire-time estimation (`tty_get_frame_size`) later used for
  RS-485 drain interval (2149, 2163).

## 5. UART Configuration (termios -> hardware)
- Baud-rate calculation via IBRD/FBRD divisor from `uartclk / clkdiv`
  (2118-2207).
- Oversampling variant (clkdiv 8 vs 16) with quot adjustment at high
  baud (2100-2103, 2186-2204).
- Word length CS5 / CS6 / CS7 / CS8 -> LCRH WLEN (2123-2136).
- Stop bits (CSTOPB -> STP2) (2137-2138).
- Parity (PARENB, PARODD, CMSPAR -> PEN / EPS / SPS) (2139-2145).
- Read-status-mask and ignore-status-mask from c_iflag (INPCK,
  IGNBRK, BRKINT, PARMRK, IGNPAR) (2057-2087).
- CREAD=0 -> synthesize dummy "ignore RX" mask bit
  (`UART_DUMMY_DR_RX`) (54, 2085-2086).
- `UART_ENABLE_MS` -> enable modem-status interrupts (2167-2168).
- Per-port timeout updated via `uart_update_timeout` (2156).
- Fixed-option variant (SBSA) installs a separate `set_termios` that
  forces CS8|CLOCAL and strips user choices; SBSA `set_termios`
  writes the fixed baud back into termios via
  `tty_termios_encode_baud_rate` (2229-2247, specifically
  2236-2245).
- Split LCRH_RX / LCRH_TX register bank with 10-PCLK delay between
  writes (1808-1827).

## 6. Modem Control and Flow Control
- Hardware signals read from FR: DCD, DSR, CTS, RI (RI presence
  vendor-gated via `fr_ri`) (108-110, 1561-1571, 1649-1662).
- Modem-control outputs: RTS, DTR, OUT1, OUT2, LOOP via CR bits
  (1680-1691).
- Auto-RTS / CTS hardware flow control (CRTSCTS -> CTSEN / RTSEN;
  status UPSTAT_AUTOCTS / AUTORTS) (2175-2184).
- Auto-RTS disable path (disable RTSEN before dropping RTS)
  (1686-1689).
- Modem status interrupts (DSRMIS / DCDMIS / CTSMIS / RIMIS) and
  delta detection (1549-1572, 1612-1614).
- Wake `delta_msr_wait` on any modem-line change (1571).

## 7. DMA Engine Support (CONFIG_DMA_ENGINE)
- TX DMA single buffer, RX DMA double-buffer (dbuf_a / dbuf_b)
  ping-pong (221-249, 816-826).
- Channel acquisition: `dma_request_chan(dev, "tx"/"rx")` -> fallback
  `dma_request_channel(mask, plat->dma_filter, plat->dma_*_param)`
  (416-457).
- EPROBE_DEFER handling (418-421).
- Slave config (DMA_MEM_TO_DEV / DMA_DEV_TO_MEM, bus width, maxburst
  derived from fifosize) (404-411, 460-467).
- Capability-probe via `dma_get_slave_caps()`: RX DMA is disabled only
  when `caps.residue_granularity ==
  DMA_RESIDUE_GRANULARITY_DESCRIPTOR`; finer-grained residue
  granularities are accepted (475-482).
- Coherent buffer alloc / free (`dma_alloc_coherent` /
  `dma_free_coherent`) for RX (378-397).
- Streaming mapping for TX buffer (`dma_map_single` /
  `dma_unmap_single`, `dma_mapping_error`) (629-640, 555-556,
  1186-1190).
- Prep slave single / submit / issue pending (637-658).
- DMA completion callbacks (TX and RX) with lock discipline
  (545-586, 979-1025).
- Pause / terminate / residue read for RX cut-over between DMA and
  PIO (946-961, 1003-1007).
- RX DMA polling timer with auto-poll rate tuned to baud (1047-1092,
  2110-2116).
- Poll-rate / poll-timeout sourced from platform data or DT
  (`auto-poll`, `poll-rate-ms`, `poll-timeout-ms`) (488-523).
- DMA -> PIO fallback on busy / prep failure (639-648, 706-710,
  970-977).
- `flush_buffer` terminates async and unmaps pending TX (791-810).
- `DMAONERR` bit for hardware-assisted DMA error handling
  (1141-1143).
- ST-variant DMA burst watermark register (`ST_UART011_DMAWM`)
  (1150-1152).
- XOFF + x_char path: disable DMA, squeeze single char into FIFO
  (759-784).

## 8. Clock and Pinctrl Management
NOTE: clock handling is not uniformly split between AMBA and SBSA.
The shared startup path (`pl011_hwinit`, reached by both
`pl011_startup` at 1894-1903 and `sbsa_uart_startup` at 1941-1949)
runs `clk_prepare_enable(uap->clk)` + `clk_get_rate(uap->clk)` at
1780-1784. But clock ACQUISITION (`devm_clk_get`) is AMBA-probe-only
at 2893, and `clk_disable_unprepare` TEARDOWN is
`pl011_shutdown`-only (2025-2028); `sbsa_uart_shutdown` at 2044-2055
does no clock teardown, and `sbsa_uart_startup` has no startup-error
clock-unwind path (returns directly on `pl011_allocate_irq` failure
at 1947-1953), whereas `pl011_startup` falls through `clk_dis:` to
`clk_disable_unprepare` (1901-1907, 1936-1938).

- Clock acquisition (AMBA probe only) via `devm_clk_get` in
  `pl011_probe` (2893).
- Shared startup: `clk_prepare_enable` + `clk_get_rate` in
  `pl011_hwinit` (1780-1784), reached by both AMBA and SBSA startups
  (1894-1903, 1941-1949).
- Console path uses `clk_prepare` + per-write `clk_enable` /
  `clk_disable` (2429, 2527, 2545, 2559, 2589).
- Close-side `clk_disable_unprepare` in `pl011_shutdown` (2025-2028).
  `sbsa_uart_shutdown` omits this step (2044-2055).
- Startup error-unwind `clk_dis:` label in `pl011_startup` runs
  `clk_disable_unprepare` (1936-1938). No corresponding unwind in
  `sbsa_uart_startup` (1947-1953).
- Pinctrl "default" state on open
  (`pinctrl_pm_select_default_state`) (1775, 2427).
- Pinctrl "sleep" state on `pl011_shutdown` close path
  (`pinctrl_pm_select_sleep_state`) (2030).

## 9. Console / printk Integration
- nbcon console with `write_atomic`, `write_thread`, `device_lock`,
  `device_unlock` (2518-2619).
- Console setup parses `baud / parity / bits / flow` options;
  fallback to reading hardware state when no options (2406-2456,
  2373-2404).
- Console-match hook for `console=pl011,mmio|mmio32,<addr>` and
  `qdf2400_e44` names (2476-2516).
- Newline synthesis on first output (`console_line_ended`) (2370,
  2535-2536).
- "Always enabled" vendor bypasses console CR save/restore
  (2529-2533, 2542-2543).
- Earlycon write (`pl011_putc` / `pl011_early_write`) and read
  (`pl011_getc` / `pl011_early_read`) (2639-2687).
- OF earlycon declarations for `arm,pl011` and `arm,sbsa-uart`
  (2713-2715).
- Legacy earlycon declaration for `qdf2400_e44` variant (waits on
  TXFE instead of BUSY) (2623-2637, 2738).
- Console flags: CON_PRINTBUFFER | CON_ANYTIME | CON_NBCON (2616).

## 10. Power Management
- Suspend / resume via `uart_suspend_port` / `uart_resume_port`
  (2944-2962).
- `SIMPLE_DEV_PM_OPS` wrapper attached to both drivers (2965, 3066,
  3092).

## 11. RS-485 Half-Duplex
- 4-state FSM (OFF / WAIT_AFTER_RTS / SEND / WAIT_AFTER_SEND)
  (251-256, 272).
- Two hrtimers (`trigger_start_tx`, `trigger_stop_tx`) set up in the
  AMBA probe path (273-274, 2921-2924). The SBSA probe installs no
  RS-485 hook and no hrtimers (2983-3037).
- `pl011_rs485_tx_start` direction control: enables CR_TXE,
  optionally clears CR_RXE when not `SER_RS485_RX_DURING_TX`, drives
  RTS polarity per `SER_RS485_RTS_ON_SEND` (1359-1371). If
  `delay_rts_before_send > 0`, arms `trigger_start_tx` for the
  pre-assert delay (1373-1377).
- `pl011_rs485_tx_stop` direction control: clears CR_TXE, sets
  CR_RXE, and applies post-send RTS polarity per
  `SER_RS485_RTS_AFTER_SEND` (1298-1310). Pre-disable drain delay
  honored via `delay_rts_after_send` (1279-1290).
- TX-drain polling via hrtimer when xmit FIFO not yet empty at stop
  time (1281-1286).
- RS-485 supported-flags capability table `pl011_rs485_supported`
  (2870-2875).
- `pl011_rs485_config` callback: if RS-485 was already enabled it
  stops the in-flight transfer; when enabling it clears
  `UART011_CR_RTSEN` and `UPSTAT_AUTORTS` (2289-2299). (It does NOT
  restart the FSM.)
- Firmware / board RS-485 configuration imported via
  `uart_get_rs485_mode(&uap->port)` during port setup (2834-2836).
- Drain-interval computation from baud and frame-size bits (2163).
- Mutual exclusion with CRTSCTS (forced off when RS-485 enabled)
  (2170-2171).

## 12. Vendor Variants and Errata
- Vendor structs: `vendor_arm`, `vendor_st`, `vendor_sbsa`,
  `vendor_qdt_qdf2400_e44` (126-217).
- Variant flags: oversampling, dma_threshold,
  cts_event_workaround, always_enabled, fixed_options, access_32b,
  inv_fr, fr_ri (103-119).
- QDF2400 erratum 44: vendor struct sets `fr_busy = UART011_FR_TXFE`
  and `inv_fr = UART011_FR_TXFE` (156-169); dispatched at SBSA probe
  via `qpdf2400_erratum44_workaround` (2968-2975, 3018-3019).
- QDF2400 earlycon dedicated putc that waits on TXFE rather than
  BUSY (2623-2630).
- ST 10-PCLK split-LCRH delay (1817-1826).
- ST DMA burst watermark (1150-1152).
- ST oversampling CR_OVSFACT + quot adjust (2186-2204).
- CTS-event workaround (W1C unlock + dummy reads) gated by
  `vendor->cts_event_workaround` (1574-1589).
- `inv_fr` FR-bit inversion to normalize FR reads across variants
  (1637, 2539, 2583).

## 13. TTY / serial_core Integration Contract
- AMBA `uart_ops` vtable: 17 unconditional methods (`tx_empty`,
  `set_mctrl`, `get_mctrl`, `stop_tx`, `start_tx`, `stop_rx`,
  `throttle`, `unthrottle`, `enable_ms`, `break_ctl`, `startup`,
  `shutdown`, `flush_buffer`, `set_termios`, `type`, `config_port`,
  `verify_port`) plus 3 conditional hooks under CONFIG_CONSOLE_POLL
  (`poll_init`, `poll_get_char`, `poll_put_char`), for 20 entries
  when compiled in (2304-2327).
- Separate SBSA `uart_ops` with stubbed mctrl, no `.throttle`, no
  `.unthrottle`, no `.flush_buffer`, no `.enable_ms`, no
  `.break_ctl` (2338-2356).
- Lifecycle: `uart_add_one_port` / `uart_remove_one_port` (2863,
  2939).
- Global `uart_driver` struct with major / minor / nr / cons
  (2744-2752).
- xmit FIFO operations (`kfifo_peek`, `kfifo_out_peek`, `kfifo_skip`,
  `kfifo_is_empty`, `kfifo_len`) (530-627, 1515, 1524-1546).
- `uart_write_wakeup` at `WAKEUP_CHARS` threshold (670-671,
  1539-1540).
- PIO RX insertion via `uart_insert_char` (361-363).
- DMA RX insertion via `tty_insert_flip_string` (890, 1065-1066).
- `tty_flip_buffer_push` to drive the tty layer after RX (928, 1071,
  1463-1464).
- `uart_xmit_advance` for accounting (668).
- Helpers: `uart_handle_break`, `uart_handle_dcd_change`,
  `uart_handle_cts_change` (341, 1562, 1568).
- x_char priority send (XON / XOFF) (741-785, 1509-1514).
- Console-poll support (`poll_init` / `poll_get_char` /
  `poll_put_char`) behind CONFIG_CONSOLE_POLL (1711-1766).
- RX throttle / unthrottle controls hardware RX pipeline (AMBA ops
  only) (1427-1446, 1871-1892).
- Counters on `port.icount` (rx, tx, brk, parity, frame, overrun,
  dsr) (335, 340-349, 1498, 1565).
- `verify_port` checks baud-base, iomem match, IRQ range, port type
  (2268-2281).
- `config_port` populates `port.type = PORT_AMBA` on request
  (2259-2263).
- Port-setup populates `port.dev`, `port.mapbase`, `port.membase`,
  `port.fifosize`, `port.has_sysrq`,
  `port.flags = UPF_BOOT_AUTOCONF`, `port.line` (2826-2834).

## 14. Locking and Concurrency Protocol
- Port spinlock is the dominant pattern for state-mutating paths
  (`uart_port_lock` / `_irqsave` / `_irqrestore` / `_irq`), but is
  NOT used on every externally visible op: `pl011_tx_empty`,
  `pl011_get_mctrl`, and `pl011_set_mctrl` are lockless (1631-1691,
  2304-2327).
- Concrete held-lock call sites documented by the driver:
  - DMA TX completion callback and TX refill (553-585).
  - DMA RX completion callback (998-1014).
  - hrtimer callbacks re-acquire port lock before state transitions
    (1405-1422).
  - ISR locks, drains, and releases via
    `uart_unlock_and_check_sysrq` (1597, 1626).
  - Startup enable-interrupts path (1846-1868).
  - `set_termios` critical section (2151-2225).
- Lock-drop-then-reacquire protocol around TTY handoff in
  `pl011_rx_chars`: drops port lock for `tty_flip_buffer_push` and
  subsequent DMA retrigger, then reacquires (1457-1487).
- nbcon enter / exit unsafe to coordinate with oops / panic prints
  (2524-2548, 2567-2592).
- Console device_lock / device_unlock use
  `__uart_port_lock_irqsave` / `__uart_port_unlock_irqrestore`
  (2595-2604).

## 15. Resource Ownership Model

### 15a. Probe / unregister lifecycle (lives as long as the device)
- devm-managed (auto-released on device detach):
  - `devm_kzalloc(uap)` (2888, 3008).
  - `devm_clk_get(clk)` -- AMBA path only (2893).
  - `devm_ioremap_resource(base)` (2820).
- Manually acquired / released at device lifetime:
  - DMA channels: acquired lazily on the first `pl011_dma_startup()`
    via `pl011_dma_probe()` when `!uap->dma_probed` (399-457,
    1098-1102); released only at final unregister via
    `pl011_dma_remove()` / `pl011_unregister_port()` (529-535,
    2798-2800).
  - RS-485 hrtimer setup at probe (AMBA path only, 2902-2904,
    2921-2924). `sbsa_uart_probe` installs neither the RS-485
    config hook nor the hrtimers (2983-3037).
  - `amba_ports[]` slot ownership (2838, 2792-2797).
- Last-port-teardown unregisters the `uart_driver` (2787-2801).

### 15b. Open / close lifecycle (per startup / shutdown cycle)
- Manually acquired at startup and released at shutdown:
  - `request_irq` / `free_irq` (1833, 2021, 2051).
  - `kmalloc(dmatx.buf)` / `kfree` (1104, 1192).
  - `dma_alloc_coherent(dbuf_*)` / `dma_free_coherent` (381, 394,
    1199-1200).
  - `timer_setup(dmarx.timer)` / `timer_delete_sync` (1159, 1202).
  - Streaming DMA mapping lifecycle tied to TX transfer (629,
    1186-1190).
  - Clock usage:
    - `clk_prepare_enable` in the shared `pl011_hwinit` startup path
      reached from both AMBA and SBSA startups (1780, 1894-1903,
      1941-1949).
    - `clk_disable_unprepare` only in `pl011_shutdown` (2025-2028)
      and the `pl011_startup` error-unwind `clk_dis:` label
      (1936-1938). `sbsa_uart_shutdown` does no clock teardown
      (2044-2055), and `sbsa_uart_startup` has no analogous
      error-unwind path (returns directly on `pl011_allocate_irq`
      failure at 1947-1953).
    - Clock handle itself is acquired only in `pl011_probe` via
      `devm_clk_get` (2893).
  - Pinctrl default / sleep state transitions (1775, 2030).
- Ordered teardown in `pl011_shutdown`: disable interrupts -> DMA
  shutdown -> rs485 stop -> free_irq -> disable_uart -> clk off ->
  pinctrl sleep -> plat->exit -> flush_buffer (2009-2042).
- Drain TX (wait on busy bit) before disabling DMA (1174-1175).
- `pl011_startup` error path via `clk_dis:` label (1936-1938).
- Conditional `hrtimer_try_to_cancel` for RS-485 timers during TX
  state transitions (1295, 1354).

## 16. Module Metadata and Boot Ordering
- `MODULE_DEVICE_TABLE` for amba, of, acpi (3052, 3059, 3087).
- `MODULE_AUTHOR`, `MODULE_DESCRIPTION`, `MODULE_LICENSE`
  (3122-3124).
- `arch_initcall(pl011_init)` + `module_exit(pl011_exit)` (init
  earlier than normal module_init so the console comes up early)
  (3119-3120).
- `suppress_bind_attrs = IS_BUILTIN(...)` to block sysfs-driven
  unbind when built-in (3069, 3093).

## 17. Driver Lifecycle Functions
- `pl011_probe` (amba) and `sbsa_uart_probe` (platform) (2877,
  2983).
- `pl011_setup_port`: ioremap, DT-alias resolution, port field
  population, `uart_get_rs485_mode`, `amba_ports[]` claim
  (2814-2841).
- `pl011_register_port`: mask+clear device interrupts, lazily
  `uart_register_driver()` on the first port, `uart_add_one_port`,
  unwind via `pl011_unregister_port` on failure (2843-2868).
- `pl011_remove` / `sbsa_uart_remove`: `uart_remove_one_port`
  followed by `pl011_unregister_port` (2935, 3040).
- Open-side `pl011_startup` (1894-1938): `hwinit` ->
  `pl011_allocate_irq` -> set IFLS -> under port lock, set
  `UARTEN | RXE`, also set `TXE` only when RS-485 is not enabled
  (1913-1919) -> read initial modem status -> `pl011_dma_startup`
  -> `pl011_enable_interrupts`.
- `sbsa_uart_startup` (1941-1961): `hwinit` -> `pl011_allocate_irq`
  -> zero old modem status -> `pl011_enable_interrupts` (no DMA, no
  control-register initialization).
- Close-side `pl011_shutdown` (2009-2042):
  `pl011_disable_interrupts` -> `pl011_dma_shutdown` -> rs485 tx
  stop (if enabled) -> `free_irq` -> `pl011_disable_uart` ->
  `clk_disable_unprepare` (2025-2028) -> pinctrl sleep ->
  `plat->exit` -> (conditional `port->ops->flush_buffer`).
- `sbsa_uart_shutdown` (2044-2055): `pl011_disable_interrupts` ->
  `free_irq`. The conditional `port->ops->flush_buffer` call is a
  no-op for SBSA because `sbsa_uart_pops` does not install a
  `flush_buffer` hook (2338-2356); no DMA, rs485, UART-disable,
  clock, or pinctrl work.
- `hwinit` (1768-1806): pinctrl default -> clk prepare+enable ->
  `clk_get_rate` -> clear pending IRQs -> save IMSC -> enable
  initial RX+timeout interrupts (RTIM|RXIM unconditionally,
  1795-1796) -> optional `plat->init`.
- `pl011_disable_uart` preserves RTS / DTR for the next open; also
  disables break + FIFOs via LCRH (1977-1995).
- Platform-data `plat->init` / `plat->exit` hooks invoked at several
  lifecycle sites:
  - `hwinit` during startup (1798-1804).
  - `pl011_shutdown` (2032-2038).
  - `pl011_console_setup` after `clk_prepare()`, before baud /
    parity option selection (2429-2440).

## 18. Driver Policy / Runtime Heuristics
- DMA <-> PIO fallback cascade (PIO -> DMA on ample buffer;
  DMA -> PIO on small / busy / residue-granularity inadequate).
- RX-DMA-to-IRQ fallback after polling timeout (1077-1091).
- TX DMA minimum size threshold (fifosize/2) below which PIO is used
  (611-615).
- "Bodge" last-byte-held-for-XON policy (617-621).
- Spurious RX IRQ drain at enable (fifosize*2 reads) to unstick
  edge-triggered RXIS (1851-1862).
- Global static port table with hard limit `UART_NR=14` (45).
- SBSA fixed-baud policy: DT `current-speed` mandatory under OF,
  non-OF SBSA probe defaults to 115200; `sbsa_uart_set_termios`
  writes the fixed baud back into termios via
  `tty_termios_encode_baud_rate` and forces CS8|CLOCAL (2994-3002,
  2236-2245).
- Platform-data hooks (`plat->dma_filter`, `plat->dma_tx_param`,
  `plat->dma_rx_param`, `plat->dma_rx_poll_*`) (424-434, 488-507).

## 19. Cross-cutting: Interrupt and DMA Mask Policy
The dominant runtime pattern: most sites mutate `uap->im` (IMSC
shadow) or `uap->dmacr` (DMACR shadow), then write the shadow through
to the hardware register. Known exceptions: `pl011_hwinit()` writes
IMSC directly after saving the current value into `uap->im`
(1795-1796), and `pl011_quiesce_irqs()` masks TXIM via a live IMSC
read-modify-write that bypasses the shadow entirely (1718-1733).
Representative sites:
- DMA TX IRQ handling toggles TXIM when DMA absorbs the transfer
  (697-698, 747-748).
- RX DMA trigger disables RXIM and enables RXDMAE (851-852).
- RX DMA failure paths re-enable RXIM as fallback (974-975,
  1022-1023).
- `pl011_stop_tx` disables TXIM (1324-1325).
- `pl011_stop_rx` clears only `RXIM`, `RTIM`, and RX error mask bits
  (FEIM, PEIM, BEIM, OEIM); modem-status mask bits are managed
  separately by `pl011_enable_ms` (1432-1454).
- `pl011_enable_ms` sets the modem-status mask bits (RIMIM, CTSMIM,
  DCDMIM, DSRMIM) (1453).
- Console-poll quiesce clears pending + masks TXIM via live RMW
  (1713-1733).
- `pl011_enable_interrupts` sets RTIM (+ RXIM when not in RX DMA)
  (1864-1868).
- `pl011_unthrottle_rx` mirrors the enable-interrupts RX policy and
  re-asserts RXDMAE under CONFIG_DMA_ENGINE (1878-1887).
- `pl011_disable_interrupts` zeroes IMSC and W1C-clears all pending
  (1997-2005).

## 20. Cross-cutting: Vendor-Data Dispatch Matrix
`struct vendor_data` is the single fan-out point for per-silicon
behavior. Covered axes:
- `reg_offset[]`: register layout (per-variant arrays at 86-100,
  172-197).
- Flag bits: oversampling, dma_threshold, cts_event_workaround,
  always_enabled, fixed_options, access_32b (103-119).
- `ifls`: IRQ FIFO level defaults (128, 206).
- `get_fifosize()`: FIFO sizing function (118, 121-124, 199-202).
- `fr_busy`, `fr_dsr`, `fr_cts`, `fr_ri`, `inv_fr`: FR-bit
  interpretation and normalization (108-110, 1636-1640, 1657-1659,
  2539, 2583).
- Oversampling policy: clkdiv + CR_OVSFACT + quot adjust
  (2100-2103, 2186-2204).
- DMA burst watermark (1150-1152) and CTS-event workaround
  (1574-1589) both gated by vendor flags.
- Console save/restore bypass via `always_enabled` (2529-2543).
- `fixed_options`: console-path option handling in
  `pl011_console_setup` selects `uap->fixed_baud` instead of parsing
  options (2445-2447). The SBSA termios-shape restriction comes from
  installing `sbsa_uart_pops` (3023-3024) rather than from a runtime
  test of this flag.
- Erratum dispatch at probe via `qpdf2400_erratum44_workaround`
  (2968-2975).
