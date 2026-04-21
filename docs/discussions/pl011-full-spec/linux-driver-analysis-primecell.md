# PL011 Linux Driver -- PrimeCell-Filtered Derivative

**Role**: PrimeCell-scope-pinned view of the Linux PL011 driver
feature inventory. This is a filtered derivative of
`linux-driver-analysis.md` which contains the full Codex-approved
inventory (AMBA PrimeCell + ST + SBSA + QDF2400).

**Purpose**: per the round-4 strategy, the Regseq PL011 scope is
pinned to the generic ARM PrimeCell PL011 (`vendor_arm` code path
in Linux). Non-PrimeCell material is out of scope for the paper
claim and must not appear as evidence in the comparison matrix.
This derivative records what gets filtered out so the filtering is
transparent and auditable.

**Source driver**:
`/Users/nyengele/driverbase/os/linux/drivers/tty/serial/amba-pl011.c`
(3124 lines) and
`reference-drivers/pl011/linux/amba-pl011.c` (cached verbatim).

## 1. Filtering rule

Keep:

- Every code path reached when `id->data` is `vendor_arm` and the
  device is bound via the AMBA driver registration path
  (`amba_driver` + `amba_id` table entry `0x00041011 /
  0x000fffff`).
- All shared code reached from both the AMBA path and the SBSA
  platform_driver path, qualified such that the filter preserves
  only the behavior relevant to the AMBA path (e.g., the shared
  `pl011_hwinit` is kept, but its `plat->init` hook reached via
  AMBA platform_data is kept while SBSA-specific branches are
  stripped).

Remove:

- **vendor_st** code paths (ST-Ericsson PL011 variant). Affects
  `pl011_st_offsets`, extended register set (DMAWM, XFCR, XON1/2,
  XOFF1/2, ITCR, ITIP, ABCR, ABIMSC), oversampling + CR_OVSFACT,
  10-PCLK split-LCRH delay, DMA burst watermark, vendor-specific
  fifosize=64.
- **vendor_sbsa** code paths (SBSA UART bound via `platform_driver`
  with `compatible = "arm,sbsa-uart"` or ACPI `ARMH0011` /
  `ARMHB000`).
- **vendor_qdt_qdf2400_e44** code paths (QDF2400 erratum 44:
  `fr_busy = TXFE` + `inv_fr = TXFE` inversion) and the
  `qpdf2400_erratum44_workaround` probe-time dispatch.
- The `sbsa_uart_startup` / `sbsa_uart_shutdown` /
  `sbsa_uart_set_termios` / `sbsa_uart_probe` / `sbsa_uart_remove`
  functions and the `sbsa_uart_pops` uart_ops table.
- The `cts_event_workaround` path gated by
  `vendor->cts_event_workaround`, which is only relevant on
  vendor_st (vendor_arm has the flag unset).
- The `always_enabled` / `fixed_options` vendor-flag code paths,
  which matter only for SBSA and QDF variants.
- The `inv_fr` bit-inversion path (QDF only).
- The `access_32b` forced-32-bit path (SBSA / QDF only; vendor_arm
  honors `reg-io-width` DT property for runtime dispatch).
- The `qdf2400_e44` earlycon name / console match path.

## 2. Section-by-section filter outcomes

All references are section numbers and bullets from
`linux-driver-analysis.md`.

### 1. Device Binding and Enumeration

- Keep: AMBA binding via `amba_id` for `vendor_arm` at
  `0x00041011 / 0x000fffff`.
- Remove: `vendor_st` `amba_id` entry at `0x00380802 / 0x00ffffff`
  and the entire `arm_sbsa_uart_platform_driver` registration.
- Keep: `arch_initcall` ordering, DT alias-based numbering,
  global `amba_ports[]` slot allocation.
- Remove: OF `current-speed` for SBSA; non-OF SBSA 115200
  default.
- Keep: `id->data = vendor_arm` dispatch.

### 2. MMIO Access

- Keep: all items. Relaxed reads/writes, width dispatch (under
  vendor_arm, runtime choice via `reg-io-width`), register-offset
  indirection (`pl011_std_offsets`), busy-wait FR polling, write
  ordering constraints.
- Remove: `pl011_st_offsets` array and references. Remove split
  LCRH_RX/LCRH_TX 10-PCLK delay (vendor_st only; vendor_arm has
  `REG_LCRH_RX == REG_LCRH_TX`, so the split-LCRH path's
  `pl011_split_lcrh` predicate returns false and the 10-dummy-MIS
  delay never runs).

### 3. Interrupt Handling

- Keep: all items. Shared IRQ, combined ISR, pass limit, IM shadow,
  named IRQ sources, RX enable policy, hwinit unconditional setup,
  ICR W1C, RIS & uap->im pending, spurious-RX drain, SysRq,
  mask-all at shutdown, console-poll quiesce.
- Remove: `check_apply_cts_event_workaround` path (vendor_st only).

### 4. FIFO and Framing

- Keep: vendor_arm `get_fifosize_arm` (16 or 32 based on
  `amba_rev`), `UART011_IFLS_RX4_8 | UART011_IFLS_TX4_8` threshold.
- Remove: `get_fifosize_st` (fifosize=64), ST IFLS defaults
  `RX_HALF | TX_HALF`.
- Keep: all remaining items (FIFO enable, refill threshold, RX
  error flags, break, wire-time estimation).

### 5. UART Configuration (termios -> hardware)

- Keep: baud divisor with `clkdiv = 16` (vendor_arm does not set
  `oversampling`). Keep word length, stop bits, parity, status
  masks, timeout, split LCRH rule (no-op on vendor_arm), per-char
  wire-time.
- Remove: oversampling `clkdiv = 8` and CR_OVSFACT branch,
  high-baud quot adjustment branch (vendor_st only); SBSA
  `fixed_options` fixed-CS8|CLOCAL termios path.

### 6. Modem Control and Flow Control

- Keep: all items. FR reads (DCD/DSR/CTS/RI), CR bits (RTS/DTR/
  OUT1/OUT2/LOOP), CRTSCTS auto flow, auto-RTS-disable-before-drop,
  modem-status IRQ, delta detection, `delta_msr_wait` wake.
- Note: `vendor->fr_ri` is the vendor_arm `UART011_FR_RI` bit;
  `vendor->fr_dsr` and `vendor->fr_cts` are vendor_arm
  `UART01x_FR_DSR` / `UART01x_FR_CTS`. Use those directly; do not
  parameterize against the vendor struct in the Regseq spec.

### 7. DMA Engine Support

- Keep: all items EXCEPT ST-variant DMA burst watermark
  (`ST_UART011_DMAWM`, register `REG_ST_DMAWM`), which is
  vendor_st only. TX/RX DMA paths, channel acquisition, capability
  probe, coherent + streaming mapping, polling fallback, PIO
  fallback cascade, `flush_buffer`, DMAONERR, XOFF x_char path
  are all generic PrimeCell.

### 8. Clock and Pinctrl

- Keep: all items. `devm_clk_get` in AMBA probe, `pl011_hwinit`
  clock enable, console path `clk_prepare_enable` variants,
  pinctrl default/sleep.
- Note: `sbsa_uart_probe` is stripped, so the "AMBA-only clock
  acquisition" caveat from the original inventory's Section 8
  becomes trivially true in this filtered view (there is no
  non-AMBA probe path).

### 9. Console / printk Integration

- Keep: nbcon console, console setup (parse options + read-back),
  console-match for `console=pl011`, newline synthesis, earlycon
  `pl011_putc` / `pl011_early_write`, `OF_EARLYCON_DECLARE` for
  `arm,pl011` only, console flags.
- Remove: `OF_EARLYCON_DECLARE` for `arm,sbsa-uart` (SBSA-only;
  uses the same `pl011_early_console_setup` but the binding
  target is SBSA and is stripped with the rest of the SBSA
  path).
- Remove: `qdf2400_e44` earlycon dedicated putc + EARLYCON_DECLARE,
  `qdf2400_e44` console name match, `always_enabled` save/restore
  bypass (vendor_sbsa / vendor_qdt only).

### 10. Power Management

- Keep: all items. `uart_suspend_port` / `uart_resume_port`,
  `SIMPLE_DEV_PM_OPS` attached to AMBA driver (`pl011_driver`).
- Note: SBSA SIMPLE_DEV_PM_OPS attachment to
  `arm_sbsa_uart_platform_driver` is stripped with the SBSA
  driver.

### 11. RS-485 Half-Duplex

- Keep: all items. 4-state FSM, hrtimers, direction control,
  `pl011_rs485_config`, `uart_get_rs485_mode` at setup,
  drain-interval, CRTSCTS mutual exclusion. RS-485 is installed
  only in the AMBA probe path (see Section 15a of the original
  inventory: "AMBA path only, 2902-2904, 2921-2924").
- Note: `sbsa_uart_probe` explicitly does not install RS-485;
  that non-installation is preserved (it's a negative fact about
  the SBSA path, which we're stripping).

### 12. Vendor Variants and Errata

- **Entire section filtered out** except the `vendor_arm` struct
  itself. This is the biggest section-level reduction.
- Remove: `vendor_st`, `vendor_sbsa`, `vendor_qdt_qdf2400_e44`
  structs and all associated flag-gated behavior. Remove QDF44
  workaround, ST delays/watermarks/oversampling, SBSA
  always_enabled / fixed_options, `inv_fr`, `cts_event_workaround`.
- The filtered view of this section is essentially: "vendor_arm is
  the only variant in scope; its flags are 0 for every
  variant-gated capability the driver provides."

### 13. TTY / serial_core Integration Contract

- Keep: AMBA `uart_ops` vtable (20 entries under
  CONFIG_CONSOLE_POLL).
- Remove: Separate SBSA `uart_ops` (stubbed mctrl, reduced ops).
  The SBSA ops table does not exist in the filtered view.
- Keep: all remaining items.

### 14. Locking and Concurrency

- Keep: all items (the locking protocol is vendor-agnostic).

### 15. Resource Ownership Model

- Keep: 15a devm-managed items (in AMBA probe only, which is the
  only probe in the filtered view), DMA channel lazy acquisition,
  RS-485 hrtimer setup at AMBA probe, `amba_ports[]` slot, last-
  port-teardown uart_driver unregister.
- Keep: 15b per-open/close items. Clock handling note simplifies
  (no SBSA branch to distinguish).

### 16. Module Metadata and Boot Ordering

- Keep: `MODULE_DEVICE_TABLE` for `amba` (3087). Keep MODULE_*
  metadata, `arch_initcall`, `suppress_bind_attrs`.
- Remove: `MODULE_DEVICE_TABLE` for `of` + `acpi` (SBSA-side only).

### 17. Driver Lifecycle Functions

- Keep: `pl011_probe` (AMBA). Keep `pl011_remove`.
- Remove: `sbsa_uart_probe`, `sbsa_uart_remove`,
  `sbsa_uart_startup`, `sbsa_uart_shutdown`.
- Keep: all shared functions (`pl011_setup_port`,
  `pl011_register_port`, `pl011_startup`, `pl011_shutdown`,
  `pl011_hwinit`, `pl011_disable_uart`).
- Keep: `plat->init` / `plat->exit` platform-data hooks (these
  are AMBA-side platform-data callbacks, distinct from SBSA
  platform_device).

### 18. Driver Policy / Runtime Heuristics

- Keep: all PrimeCell-applicable items (DMA-PIO cascade,
  RX-DMA-to-IRQ fallback, TX threshold, spurious-RX drain,
  UART_NR limit).
- Remove: SBSA fixed-baud policy (`sbsa_uart_set_termios` +
  non-OF 115200 default).
- Keep: platform-data hooks (`plat->dma_filter`,
  `plat->dma_*_param`, `plat->dma_rx_poll_*`).

### 19. Cross-cutting: Interrupt and DMA Mask Policy

- Keep: all items except `quiesce_irqs` subtleties that reference
  vendor flags. The IMSC-shadow + DMACR-shadow pattern is
  vendor-agnostic.

### 20. Cross-cutting: Vendor-Data Dispatch Matrix

- **Entire section filtered out.** In the PrimeCell-pinned view,
  `struct vendor_data` is a single static struct (`vendor_arm`)
  whose flags are all zero for variant-gated capabilities. Vendor
  dispatch is trivial. The matrix collapses.

## 3. Net effect on the comparison matrix

The comparison matrix in `pl011-comparison-matrix.md` will cite
only items retained above. Dropped items (vendor_st / vendor_sbsa /
vendor_qdt_qdf2400_e44 behavior) do not contribute evidence to any
matrix row, by design of the PrimeCell scope pin.

If a reviewer asks "why not include ST/SBSA/QDF features," the
answer is on record: scope was pinned to generic PrimeCell PL011
for single-device-focus reasons (see `pl011-comparison-axes.md`
Section 1). The filter is documented; nothing is being silently
omitted.

## 4. Change history

- 2026-04-20: initial version, derived from the Codex-approved
  `linux-driver-analysis.md` plus the round-4 strategy scope pin.
