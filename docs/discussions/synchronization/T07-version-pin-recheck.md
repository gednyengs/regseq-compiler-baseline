# T07 precondition — C02 primitive-spelling version-pin recheck

**Gate (PLAN review-D1 / T07 precondition):** verify every `C02` per-OS primitive
spelling against the pinned **Linux 6.18.21** / **Zephyr v4.4.0** headers *before*
authoring the per-OS golden-C, so a stale spelling is not baked into the goldens.

**Performed:** 2026-06-17, ahead of T07 golden authoring.

## Headers consulted (on-disk)

| Pin | Target | On-disk source | Version | Match |
|---|---|---|---|---|
| Linux | 6.18.21 | `~/driverbase/os/linux/include/linux/` (corpus base) | **6.18.0** | minor match; SUBLEVEL 0 vs 21 (patch-level only) |
| Zephyr | v4.4.0 | `~/regseq-eval/pl011/zephyr/zephyrproject/zephyr/include/zephyr/` | **4.4.0** | **exact** |

Linux residual: the on-disk tree is 6.18.0, the pin is 6.18.21 — a patch-level
delta on the same `6.18` minor line. The verified APIs (`spin_lock*`, `mutex_*`,
`completion`, `disable_irq`) are long-stable kernel locking primitives with no
signature change across the `6.18.x` patch series. Residual risk: nil for the
C02 set. Zephyr needs no residual (exact pin match).

## Verified spellings (all CONFIRMED)

### Linux (`spinlock.h`, `mutex.h`, `completion.h`, `interrupt.h`)
| C02 op | Spelling | Header:line | Notes |
|---|---|---|---|
| `Mutex::lock` | `mutex_lock(m)` / `mutex_unlock(m)` | mutex.h:168/189, 224 | `mutex_lock` → `mutex_lock_nested(m,0)` or extern; canonical name `mutex_lock` |
| `SpinLock::lock` | `spin_lock(l)` / `spin_unlock(l)` | spinlock.h:349/389 | `spinlock_t *` |
| `IrqSpinLock::lock` | `spin_lock_irqsave(l, flags)` / `spin_unlock_irqrestore(l, flags)` | spinlock.h:379/404 | **`irqsave` is a MACRO writing into `flags` (lvalue); `irqrestore(spinlock_t*, unsigned long flags)` takes `flags` by value** |
| `completion_wait` | `wait_for_completion_timeout(c, t)` | completion.h:107 | returns `unsigned long` (jiffies remaining); `t` in jiffies |
| `completion_signal` | `complete(c)` | completion.h:118 | SingleWaiter |
| `completion_reset` | `reinit_completion(c)` | completion.h:97 | |
| `init_primitives` | `mutex_init(m)` / `spin_lock_init(l)` / `init_completion(c)` | mutex.h:60, spinlock.h:331/341, completion.h:84 | macros |
| `destroy_primitives` | mostly no-op / `devm` | — | no per-primitive destroy call needed (devm/no-op) |
| drain (C05/D07) | `disable_irq(irq)` | interrupt.h:232 | synchronizes (drains) in-flight handler |

### Zephyr v4.4.0 (`spinlock.h`, `kernel.h`)
| C02 op | Spelling | Header:line | Notes |
|---|---|---|---|
| `Mutex::lock` | `k_mutex_lock(m, K_FOREVER)` / `k_mutex_unlock(m)` | kernel.h:3525/3547; K_FOREVER 1664 | |
| `SpinLock::lock` | `k_spin_lock(l)` / `k_spin_unlock(l, key)` | spinlock.h:181 | **≡ IrqSpinLock collapse** (always IRQ-off) |
| `IrqSpinLock::lock` | `k_spin_lock(l)` / `k_spin_unlock(l, key)` | spinlock.h:181 | same primitive; surface keeps kinds distinct (anti-narrowing, I06) |
| `completion_wait` | `k_sem_take(s, t)` | kernel.h:3743 | `t` is `k_timeout_t` |
| `completion_signal` | `k_sem_give(s)` | kernel.h:3755 | SingleWaiter |
| `completion_reset` | `k_sem_reset(s)` | kernel.h:3766 | |
| `init_primitives` | `k_mutex_init` / (spin: no init / `K_*_DEFINE`) / `k_sem_init` | kernel.h:3501, 3722; K_MUTEX_DEFINE 3485, K_SEM_DEFINE 3798 | |
| `destroy_primitives` | **NO-OP** (static primitives) | — | C05 safety-by-no-destroy |

### Guard hidden-state contract (T07 codegen)
- **Linux** `IrqSpinLock`: guard carries `unsigned long flags`, **declared before** the
  acquire; `spin_lock_irqsave(l, flags)` writes it; drop emits `spin_unlock_irqrestore(l, flags)`.
- **Zephyr** `SpinLock`/`IrqSpinLock`: guard carries `k_spinlock_key_t key`, **assigned from**
  the acquire (`key = k_spin_lock(l)`); drop emits `k_spin_unlock(l, key)`.
- (Windows `KIRQL OldIrql`, FreeBSD none — designed-in matrix rows, **not built** this slice.)

**Outcome:** no stale spellings; goldens may be authored against the C02 matrix as written.
Linux patch-level residual (6.18.0 vs 6.18.21) noted, nil impact on the C02 set.
