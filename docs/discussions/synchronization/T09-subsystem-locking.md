# T09 — `subsystem_locking` null surface (G-SYN-17) — as-built

`subsystem_locking` is **analysis-only**: the framework ISR wrapper does NOT
auto-acquire on the bound handler's behalf. There is **no model surface and no
emitted glue** — it is the *absence* of auto-acquire, by construction.

## Realization

The ISR wrapper emission (`linux_c` `rseq_isr_<drv>`, `zephyr_c` `rseq_isr_<drv>`)
is unchanged by the sync work: it dispatches `handler(&priv->inner)` / `(&data->inner)`
with no lock. The author's explicit `lock()` inside the handler body lowers (T07)
to the per-OS acquire **in the handler**, never the wrapper. So a bound handler
that needs the ISR↔mainline lock writes `let _g = self.lock.lock();` itself
(the PL011 §6.1 worked example). `S20` (auto-acquire for bound handlers) is
explicitly NOT emitted (inherited frozen).

## Verification

New combined fixture `crates/codegen-coverage-runner/fixtures/_sync_irq/sync-irq`
(a bound `#[rseq::isr]` that acquires an `IrqSpinLock` explicitly — also the M4
PL011 seam seed). `tests/sync_irq_e2e.rs::isr_wrapper_does_not_auto_acquire`
isolates the wrapper function and asserts:
- Linux wrapper `rseq_isr_sync_irq`: contains NO `spin_lock` (no auto-acquire);
  the handler body `sync_irq_handle_irq` carries `spin_lock_irqsave(&_1->lock, …)`
  + `spin_unlock_irqrestore` on every exit.
- Zephyr wrapper `rseq_isr_sync_irq`: contains NO `k_spin_lock`; the handler body
  carries `key = k_spin_lock(&_1->lock)` + `k_spin_unlock(&_1->lock, key)`.

The fixture also confirms the recognizer handles an `IrqSpinLock` acquire in an
`#[rseq::isr]` body (legal — IrqSpinLock is the ISR-safe kind, §7.1) with the
guard released on a branchy exit (early `NotHandled` return + normal `Handled`).

Full `cargo test` green (155 ok suites, 0 failed).
