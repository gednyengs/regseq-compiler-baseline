# T08 — init/destroy lifecycle placement seam (C05) — as-built

Defines the sync `init_primitives`/`destroy_primitives` placement **relative to**
`regseq-backends/src/interrupt_order.rs::IrqPhase` (which carries only *interrupt*
phases — Codex finding #1). Absorbs the deferred Codex-M1 findings **B1**
(`regseq_lock_init` emission) and **SF1** (TU-global lock ids).

## Placement (I05: lock-live ⊇ line-live)

`init_primitives` is emitted in the init preamble **before** the interrupt
register-disabled (`IrqPhase::InitPreambleAfterResources`) and **before**
`on_init` (which acquires); the interrupt *enable* is later still
(`IrqPhase::AfterOnInitOk`). `destroy_primitives` is emitted in the deinit
postamble **after** the interrupt disable (`IrqPhase::DeinitPostambleBeforeResources`)
and before the resource postambles — the symmetric reverse. So the lock-live
window ⊇ the line-live window by construction.

| backend | init site | init primitives | destroy |
|---|---|---|---|
| generic-c | `emit_lifecycle_synthesis`, after the bus/map_region block, before the interrupt preamble | `regseq_lock_init(id, KIND)` per resource | `regseq_lock_destroy(id)` in `emit_teardown_dispatch`, after `irq_teardown`, reverse order |
| linux | probe, after `set_drvdata`, before region/IRQ/`on_init` | `mutex_init`/`spin_lock_init`/`init_completion(&priv->inner.<f>)` | **no-op** (spinlock/mutex need no teardown; devm frees `priv`) |
| zephyr | `{drv}_init`, after the `data` decl, before region/IRQ/`on_init` | `k_mutex_init`/`k_sem_init(&data->inner.<f>, 0, 1)`; **spinlock: no runtime init** (zero-init valid) | **no-op** (static primitives — C05 safety-by-no-destroy) |

## B1 — `regseq_lock_init` emission + oracle acquire-after-init

The generic-c oracle (`prelude.rs SYNC_HELPERS`) already tracked `regseq_lock_inited[]`;
T08 (a) emits `regseq_lock_init` in the lifecycle preamble, (b) makes
`regseq_lock_acquire` + the completion ops **assert** the slot is inited
(`SYNC error acquire-before-init` → abort), and (c) adds `regseq_lock_destroy`
(asserts not-held, marks un-inited → a later acquire fails the init check: the
use-after-destroy half of I05). `regseq_lock_destroy` is reserved in
`prelude_ordinary_idents`.

## SF1 — TU-global lock ids

Each driver's `lock_id` (its index into that driver's `resources`) is offset by
the cumulative resource count of the earlier drivers (BTreeMap ItemId order),
so the oracle's per-id init/held tracking never collides across drivers in a
multi-driver TU. The same base (`tu_lock_base`) is used by the `init`/`destroy`
emission and the `acquire`/`release`/`completion` hooks. Single-driver TUs
(every current fixture) are unaffected (base 0).

## Verification + scope

`sync_os_e2e::c05_init_destroy_placement` asserts: linux + zephyr init before
`on_init`; zephyr spinlock-no-init; generic-c `lock_init` before `lock_acquire`,
`lock_destroy` emitted, and the oracle's acquire-before-init guard present. The
generic-c oracle ABORTS at runtime on an acquire-before-init (proven by the
existing `sync_oracle_e2e` passing only because init now precedes acquire).

**Deferred to M4/T15 (the real witness):** the COMBINED interrupt+lock ordering
(init-before-`irq_enable`, destroy-after-`irq_disable` with an actual ISR) is the
PL011 `IrqSpinLock<IsrState>` seam (T14) — that fixture naturally exercises the
combined order; **T15** confirms device-lifecycle honors the C05/D07 slot
ordering (Open-3). T08 owns the seam *definition* + emission + structural
placement; it does not stand up a combined irq+lock fixture (that is T14's).

Full `cargo test` green (154 ok suites, 0 failed); no golden churn (all sync
emission is gated on `model.synchronization.drivers`).
