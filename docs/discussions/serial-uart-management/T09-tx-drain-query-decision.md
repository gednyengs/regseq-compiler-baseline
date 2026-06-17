# T09 / M1.5 — `tx_drain_query` include/defer decision (RESOLVED: INCLUDE)

**Status:** decided 2026-06-16. Gate per PLAN M1.5 (GRILL D11 / O3). Must precede M2/M3 emit.
**Process:** draft options → Codex critique (AGREE-WITH-CHANGES) → presented to Gedeon → **INCLUDE**.

## Decision

v1 **INCLUDES** `SerialQueueControl::tx_drain_query` as the **sole** member of an otherwise-deferred
`SerialQueueControl` impl. Its sibling `fifo_purge_hw` stays a deferred/capability-gated member
(T04 reject matrix). Lowering:

- **Linux** `uart_ops.tx_empty` ← `tx_drain_query`.
- **Zephyr** `uart_driver_api.irq_tx_complete` ← `tx_drain_query` (M3).

## Why (evidence, not assertion)

`tx_drain_query` is "shift-register-inclusive TX-drain completion" — on PL011 it reads **FR.BUSY**.
The base `Serial` op `data_ready_query(TxSpace)` reads **FR.TXFF** (FIFO has space). These are
**different device facts**; the drain fact is NOT derivable from the base ops.

- **Linux makes the slot structurally mandatory.** `serial_core.c` (6.x) dereferences
  `uport->ops->tx_empty(uport)` with **no NULL guard** at three sites: `uart_get_lsr_info`
  (TIOCSERGETLSR), `uart_wait_until_sent` (close/`tcdrain`: `while (!ops->tx_empty(port))`), and the
  console/suspend drain (`for (tries=3; !ops->tx_empty(uport) && tries; tries--)`). A NULL slot is a
  kernel NULL-deref on the first close. `pl011_tx_empty` reads `REG_FR & (fr_busy|TXFF)`.
- **Zephyr confirms the same semantic fact** (not Linux-only baggage): `pl011_irq_tx_complete`
  returns `(fr & PL011_FR_BUSY) == 0`, wired as `.irq_tx_complete` in `uart_driver_api`. (Zephyr
  NULL-guards the syscall → -ENOSYS, so not a hard crash, but needed for correct TX completion.)
- The only DEFER alternative is emitting a **behaviorally-lying constant `tx_empty` stub**
  (always-TEMT / always-busy) for a structurally-mandatory slot — incompatible with the construct's
  thesis claim of **faithful framework-pure lowering**. Codex: "a framework-satisfying lie, not
  faithful lowering."

## Scope guardrails (Codex AGREE-WITH-CHANGES, adopted)

1. **Required, not optional, where the drain slot is emitted.** For a serial driver targeting Linux
   (and Zephyr interrupt-driven), `tx_drain_query` must be authored — **no synthesized fallback**
   (the compiler cannot honestly invent FR.BUSY from `TxSpace`). Frontend diagnostic if a
   Linux/Zephyr-IRQ serial driver omits it. (For generic_c-only it may stay optional.)
2. **Do NOT pull in `fifo_purge_hw`.** This decision justifies only the read-only drain predicate.
3. **`drain_query_refinement` stays implicit-default `shift_register` in v1.** Do not surface the
   `{shift_register | fifo_empty_only}` L1 property as an author choice now; if `fifo_empty_only` is
   ever requested for a Linux/Zephyr drain callback, reject/defer (those slots want
   shift-register-inclusive completion).

## Implementation deltas (this milestone)

- **IR** (`regseq-model/src/serial.rs`): add `queue_control: Option<SerialQueueControlOps>` to
  `DriverSerial`, with `tx_drain_query: Option<InstanceRef>`. Mirrors `line_config`/`interrupt_driven`.
- **Fixture** (`_serial_surface.rs` + `_serial/serial-pl011/input.rs`): add the `SerialQueueControl`
  trait + `impl … { fn tx_drain_query(&self) -> bool { /* FR.BUSY read */ } }`.
- **Bridge** (`regseq-driver/src/bridge/serial.rs`): recognize `impl SerialQueueControl`, capture +
  force-materialize `tx_drain_query`.
- **linux_c** (T08): emit `uart_ops.tx_empty` ← `tx_drain_query`.
- **generic_c** (M1): unchanged unless re-bless needed; `tx_drain_query` is not exercised by the M1
  trace oracle (it's a Linux/Zephyr close-path slot).
