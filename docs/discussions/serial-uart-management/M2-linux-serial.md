# serial-uart-management — M2 (linux_c serial lowering) — COMPLETE

**Status:** done 2026-06-16. PLAN T08. Codex M2 review loop **converged → SOUND / M2 PASS**.
Snapshot-validated (the live Tier-2 boot+IO campaign is T13-gated; see "Validation boundary").

## What landed

A `#[rseq::device_class(serial)]` AMBA driver now lowers to a Linux serial-core driver
(`uart_driver` + `uart_port` + `uart_ops` vtable) on the linux_c backend, replacing the
deferred/NULL-api treatment of `ClassTag::Serial` **additively** (a bare/deferred serial tag still
falls through to the class-neutral scaffold).

- **IR (`regseq-model/src/serial.rs`):** `DriverSerial.queue_control: Option<SerialQueueControlOps>`
  with `tx_drain_query` (T09 INCLUDE). Re-exported in `lib.rs`. Round-trip tests updated.
- **Fixture (`_serial_surface.rs` + `_serial/serial-pl011/input.rs`):** the `SerialQueueControl`
  trait + `impl … { tx_drain_query() = (FR.BUSY|FR.TXFF)==0 }`.
- **Bridge (`regseq-driver/src/bridge/serial.rs`):** recognizes `impl SerialQueueControl`,
  force-materializes `tx_drain_query`.
- **linux_c (`linux_c/serial.rs`, NEW + dispatch in `linux_c/mod.rs`):** the serial scaffold —
  `uart_ops` vtable, the scatter-bearing wrappers (startup/shutdown/start_tx/stop_tx/stop_rx via
  `serial_scatter::linux_scatter`), `tx_empty`←`tx_drain_query`, `set_termios`←`line_format_apply`
  (surfacing `ConfigResult::Unsupported`, §3.3.8 disable→format→enable), the synthesized serial ISR
  (decode→dispatch: RX drain to tty / kfifo TX refill), the SBSA-style framework-mandatory stubs
  (set_mctrl/get_mctrl/type/config_port/verify_port — the modem plane is the deferred
  `SerialModemControl`), and the `uart_register_driver`/`uart_add_one_port` registration. The vtable
  slot set mirrors the in-tree minimal `sbsa_uart_pops`.
- **Shared linux_c emitter extensions (higher blast radius — regression-checked):**
  - `body.rs` `Rvalue::Discriminant` → read the fieldless-enum carrier tag byte (the first author
    body to `match` a bare fieldless enum is the construct's `irq_source_set`).
  - `types.rs` `IntKind::Isize → "ssize_t"` (the discriminant-read local's type).
- **Test:** `codegen-coverage-runner/tests/linux_serial_pl011_e2e.rs` — byte-exact `golden.linux.c`
  + `conformance.linux.json` + the M2 structural suite (vtable wiring, per-source scatter distinct
  from the line `request_irq`, `tx_empty`←drain, `set_termios` Unsupported-surfacing, the ISR, the
  registration order, the A-10 negative leg).

## Codex M2 review loop (converged)

Round 1 → **UNSOUND**, 5 blocking; all fixed against the in-tree 6.x `amba-pl011.c`:
1. **TX double-skip** — the loop did `kfifo_skip` AND `uart_xmit_advance` → dropped every other
   byte. Fixed: `kfifo_peek` + `uart_xmit_advance` only.
2. **`port->uartclk` uninitialised** → div-by-zero in `uart_get_baud_rate`/`uart_update_timeout`.
   Fixed: probe sets it (placeholder, see follow-up F2).
3. **Missing `x_char` + `uart_write_wakeup`** in the TX pump. Fixed (modelled on `pl011_tx_chars`).
4. **RX line-error not consumed** — drain trigger widened `6u`→`14u` (incl. line_error) so the
   errored datum is drained, not stranded after the W1C-ack (see follow-up F1).
5. **`on_deinit` unreachable** — `.remove` now runs it; round 2 added the probe **failure-path**
   unwind (`uart_add_one_port` failing after `on_init` Ok now runs `on_deinit`, R16).
Round 3 → **SOUND / M2 PASS**, no findings.

## Validation boundary (T13) — UPDATED: the live QEMU campaign WAS run

The earlier T13 verdict ("Linux campaign not runnable here") was **wrong** — it probed the construct
*worktree*'s `pl011/linux/` and missed that the **root eval checkout** (`~/regseq-eval/pl011/linux/`)
has the campaign artifacts prebuilt: the patched QEMU v8.2.2 (two-PL011 topology), the arm64 kernel
`Image` (6.18.21), `rootfs.ext4`, the DTBs, and the exerciser. So the **R05 boot+IO done-bar was
actually executed** (see below). Zephyr (M3) remains genuinely blocked (no SDK).

## ✅ R05 ACHIEVED — live amba-pl011 QEMU run (2026-06-16)

The generated `serialpl011` driver, built from the **compiler's actual emission** (golden.linux.c,
no hand-edits), was taken through the eval Tier-2 campaign (`scripts/15-build-generated-module.sh` →
`30-launch-vm.sh` → exerciser loopback over ttyAMA1):

- **Compiles against the real 6.18.21 arm64 kernel headers** — clean, 0 warnings (`vermagic 6.18.21
  SMP aarch64`).
- **Boots + binds the reserved test UART** (`9040000.pl011`), registers the uart port:
  `serialpl011 up` + `9040000.pl011: ttyserialpl0110 at MMIO 0x9040000 (irq = 25 …) is a serialpl011`.
- **`line_format_apply` ran on real HW** — dmesg `serialpl011 line: baud=115200`.
- **Interrupt-driven RX** — host seeds N bytes → driver IRQ line 25 (`serialpl011`) delta **+5** across
  a 96-byte transfer → synthesized ISR (`irq_cause_decode`→`data_unit_read`→`tty_insert_flip_char`)
  drains → guest reads **96/96 and 80/80, match=true**.
- **Interrupt-driven TX** — guest transmits 80 bytes through the 16-deep FIFO (needs ISR refill) →
  host reads **80/80, match=true**.
- **Per-source IMSC scatter observable on silicon** (MMIO trace `off 0x038`): `0x50` (RxData armed) →
  `0x7d0` (+LineError, startup) → `0x7f0` (+TxReady, start_tx) → `0x7d0` (TxReady disarmed on drain) →
  `0x0` (mask-all teardown) — each a distinct write via the author `irq_source_set` at the compiler's
  WHEN sites (the construct's confirmed contribution, I01).
- **Clean lifecycle** — unbind → `serialpl011 deinit: masking interrupts` (on_deinit), rmmod, **no
  Oops/BUG**, device UNBOUND.

### 4 bugs the LIVE run caught that the snapshot + 3 Codex passes did NOT

1. **Compile (ABI):** `uart_port_unlock_irqrestore(port, &flags)` — passes a `unsigned long *` where
   the by-value `unsigned long` is expected (`-Wint-conversion`). Fixed: `flags` (the lock takes
   `&flags`, the unlock takes the value).
2. **`dev_name` collision:** emitted `.dev_name = "ttyAMA"` (copied from amba-pl011) collides with the
   already-loaded stock console driver's global tty namespace → `uart_add_one_port` fails "Cannot
   register tty device on line 0". Fixed: derive a unique `tty<drv>` prefix (a generated driver names
   its own nodes; major/minor are dynamic). `ttyAMA` was an overfit.
3. **Remove-Oops (downstream of #2):** with the half-registered port, `rmmod` → `uart_remove_one_port`
   → `cdev_del` on a never-registered tty → NULL-deref Oops. Resolved by #2 (the port now fully
   registers).
4. **TX deadlock — the interrupt-driven "kick":** `start_tx` armed TxReady only; a level-triggered
   PL011 TX IRQ does **not** fire for an already-empty FIFO, so the ISR never ran — an IMSC-rearm
   **storm** (trace: endless `0x7f0` writes, zero DR writes), host received 0 TX bytes. Fixed:
   `start_tx` = arm **+ pump** (prime the FIFO via the author data path), factored into a shared
   `<drv>_tx_pump` the ISR also calls — mirrors the in-tree `pl011_start_tx`→`pl011_tx_chars`. The
   generic_c M1 oracle hid this (its harness did the byte write); the real OS exposed it. This is a
   **construct-level finding**: the synthesized OS `start_tx` is arm+pump, not arm-only.

### Post-QEMU Codex review (round 4) — 2 MORE blocking bugs the run did NOT catch

After the live run, the 4 QEMU-driven fixes were Codex-reviewed (they post-dated the round-1–3
SOUND verdict). Codex found **2 blocking flow-control bugs in `tx_pump`** that neither the snapshot,
rounds 1–3, NOR the happy-path QEMU run (one baud, instant-drain FIFO) exercised:

1. **`x_char` strand** — if `x_char` is pending, the FIFO is full, and the kfifo is empty, the pump
   couldn't write x_char but still hit `kfifo_is_empty → disarm TxReady` → the XON/XOFF char is stuck
   forever (no TX IRQ armed to retry).
2. **`uart_tx_stopped` storm** — breaking the drain on `tx_stopped` with a non-empty kfifo skipped the
   disarm → TxReady stays armed → the level TX IRQ re-enters / breaks / repeats (a storm).

Fix: `tx_pump` rewritten to mirror the in-tree `pl011_tx_chars` — the x_char path returns WITHOUT
disarming when it can't write (TX IRQ stays armed to retry), and a single
`if (kfifo_is_empty || uart_tx_stopped) { disarm; return; }` gate disarms in BOTH the drained and the
stopped cases. **Re-validated on QEMU** (interrupt-driven TX+RX 64/64 match each way) and
**Codex-confirmed SOUND** (round 5). The Codex loop on the post-QEMU fixes is closed.

### Residual non-blocking gaps observed on the run (follow-ups)

- **`port->icount.rx` not incremented** in the ISR RX drain (so `TIOCGICOUNT` rx=0); data flow is
  unaffected. The reference does `icount.rx++`. (Same class as the icount/`RxError`-vocab gaps.)
- The exerciser's own `irq_delta` reads 0 because it greps `/proc/interrupts` for the literal
  `"uart-pl011"`, but the generated driver names its IRQ `"serialpl011"` (the `request_irq` name) —
  a harness-side string mismatch, NOT a driver issue (proven by the direct line-25 delta).

## Validation boundary (original, pre-run note retained)

The snapshot test remains the in-repo gate; the live QEMU run above is the R05 done-bar and was
executed in this environment from the root eval checkout.

## Recorded follow-ups (non-blocking, NOT M2 blockers)

- **F1 — `RxDatum.errors` → tty flag mapping.** The synthesized ISR inserts RX bytes as
  `TTY_NORMAL`, ignoring the inline parity/frame/break/overrun errors, because the **`RxError` bit
  vocabulary is not surfaced in the IR** (the author `RX_ERR_*` are plain `u32` consts). Surfacing it
  (an `RxErrorRepr`, analogous to `CauseSetRepr`/O2) would let the backend map errors to
  `TTY_PARITY`/`TTY_FRAME`/`TTY_BREAK`/`TTY_OVERRUN`. Codex accepted this as a documented v1 gap.
- **F2 — `uart_port.uartclk` clock binding.** Set to a placeholder (QEMU virt PL011 ref clock,
  24 MHz). It should bind from the (future) clock-management construct; the author
  `line_format_apply` owns the actual divisor math, so `uartclk` here only feeds
  `uart_get_baud_rate`/`uart_update_timeout`.

## Test commands

```
cargo test -p codegen-coverage-runner --test linux_serial_pl011_e2e   # M2 snapshot + structure
BLESS=1 cargo test -p codegen-coverage-runner --test linux_serial_pl011_e2e   # re-bless golden + conformance
cargo test -p regseq-backends --lib                                  # 411 backend tests (shared-emitter regression)
cargo test -p codegen-coverage-runner --test linux_pl011_e2e --test linux_irq_pl011_e2e  # linux regression
```
