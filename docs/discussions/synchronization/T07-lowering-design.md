# T07 — Linux + Zephyr per-OS lowering design (locked)

Resolves the PLAN's `[Open]` "guard-state codegen shape (design §5, harness-owned)"
for the first slice. Grounded in the C02 matrix (GRILL, spec-derived) + the
[version-pin recheck](T07-version-pin-recheck.md). Mirrors the done generic-c
`SyncAction` interception (`generic_c/body_emit.rs:112-176, 280-330`). The post-M2
Codex review checks the codegen shape against this note.

## D1 — Lock field type (struct `field_override`)

A driver-struct field recognized as a `SyncResource` (model
`synchronization.drivers[drv].resources`) substitutes its C declaration by kind,
via the existing `emit_aggregate_def(..., field_override)` seam (linux_c/mod.rs
`emit_type_defs`; zephyr_c/mod.rs:1265). The same seam register-access uses for
MMIO fields.

| `SyncKind` | Linux | Zephyr |
|---|---|---|
| `Mutex` | `struct mutex <f>` | `struct k_mutex <f>` |
| `SpinLock` | `spinlock_t <f>` | `struct k_spinlock <f>` |
| `IrqSpinLock` | `spinlock_t <f>` | `struct k_spinlock <f>` |
| `Completion` | `struct completion <f>` | `struct k_sem <f>` |

**Payload `T` is NOT stored** as a struct member this slice. M1/M2/M3 guards are
opaque (no `Deref`/`DerefMut`) → the payload is never accessed in generated code,
so the field lowers to *just* the OS lock object. **Residual (deferred):** payload
storage + `Deref` lowering; the M4 SMP campaign protects the critical *section*
(register placement inside the guard), with the `IrqSpinLock<T>` payload modelling
the shared state conceptually (Open-2 register-placement residual). Includes
pulled per kind: Linux `<linux/spinlock.h>` / `<linux/mutex.h>` /
`<linux/completion.h>`; Zephyr `<zephyr/kernel.h>` (k_mutex/k_sem) +
`<zephyr/spinlock.h>` (k_spinlock).

## D2 — Receiver recovery (lock object expression)

Mirrors `config_recovery` (linux_c/body.rs:750): the acquire/completion site's
body must have **arg-1 = `Ty::Ref(driver_ty)`** (`&self`/`&mut self`); the lock
object is `&<arg1_c_name>-><field_name>` (field name from `SyncResource.name`; the
sync field is a *direct* driver-struct field, FieldIdx on the driver). Any other
receiver shape → loud `EmitError::Unsupported` (never wrong code, I06). First-slice
corpus: every sync site is a driver method (PL011 seam, E1000 `stats_lock`).

The lock object is recovered **once at the acquire site** and **shared into all of
its release blocks** (same body, same direct field, valid in every block since the
receiver is a function parameter). Branchy CFG (multiple `release_blocks`) → all
reuse the one expression + the one guard-state local.

## D3 — acquire / release primitives + guard hidden state

Disambiguator `<n>` = the acquire `BlockId` (unique per site within a body).

**Linux**
| kind | local | acquire | release |
|---|---|---|---|
| Mutex | — | `mutex_lock(<obj>);` | `mutex_unlock(<obj>);` |
| SpinLock | — | `spin_lock(<obj>);` | `spin_unlock(<obj>);` |
| IrqSpinLock | `unsigned long _sync_flags_<n>;` | `spin_lock_irqsave(<obj>, _sync_flags_<n>);` | `spin_unlock_irqrestore(<obj>, _sync_flags_<n>);` |

**Zephyr** (`SpinLock ≡ IrqSpinLock` collapse, I06 — both use the key-carrying
primitive; surface keeps the kinds distinct, anti-narrowing)
| kind | local | acquire | release |
|---|---|---|---|
| Mutex | — | `k_mutex_lock(<obj>, K_FOREVER);` | `k_mutex_unlock(<obj>);` |
| SpinLock | `k_spinlock_key_t _sync_key_<n>;` | `_sync_key_<n> = k_spin_lock(<obj>);` | `k_spin_unlock(<obj>, _sync_key_<n>);` |
| IrqSpinLock | `k_spinlock_key_t _sync_key_<n>;` | `_sync_key_<n> = k_spin_lock(<obj>);` | `k_spin_unlock(<obj>, _sync_key_<n>);` |

The guard-state local is declared in the body's locals section (one per
state-carrying acquire site). **Linux** `spin_lock_irqsave` writes `flags`
(declared *before* the acquire); **Zephyr** assigns `key` *from* `k_spin_lock`'s
return. This is the per-OS guard hidden-state round-trip (review D2; a distinct
golden assertion verifies each).

## D4 — Completion ops (point events)

ns timeout extracted via the `emit_delay` pattern (linux_c/body.rs:808-834: callee
first param = `Duration` ty → sole `ns` field → `operand_expr_typed`).

| op | Linux | Zephyr |
|---|---|---|
| `wait(Duration)` | `(void)wait_for_completion_timeout(<obj>, msecs_to_jiffies((<ns>+999999ULL)/1000000ULL));` | `(void)k_sem_take(<obj>, K_NSEC(<ns>));` |
| `signal()` | `complete(<obj>);` | `k_sem_give(<obj>);` |
| `reset()` | `reinit_completion(<obj>);` | `k_sem_reset(<obj>);` |

**Result discarded** (`(void)`), exactly as the generic-c oracle discards it
(`regseq_completion_wait(id)` is void). **First-slice constraint, ENFORCED (M2
Codex blocking #1):** the timeout-expiry → `Err` outcome is not modeled, so a body
that *consumes* the `completion_wait` `Result` is **rejected loud at recognition**
(`SyncDiagnostic::CompletionWaitResultConsumed`) rather than emitting C that reads
an unset destination (I06) — bind it to `_` until the timeout→`Result` lowering
lands. The "consumed" test is the intraprocedural taint analysis
`acquire.rs::wait_result_consumed` (Codex-verified SOUND over the full model
statement+terminator surface, round 6): dead = the result's only uses are
whole-local aliases + the synthetic drop-glue. Linux ns→ms rounds *up* (min-wait
guarantee, as timed-delay). `signal` is ISR-safe (I07); SingleWaiter only.

## D5 — Map construction + interception (mirror generic-c)

In `emit_fn`, after elided-locals: compute `body_ref = model.instance_ref_of(instance)`,
build `BTreeMap<BlockId, SyncAction>` from the driver's `acquire_sites` +
`completion_sites` whose `body == body_ref` (lower the receiver/Duration via
`cx`/`operand_expr` at construction). Collisions → loud (two sites → one block).
Declare the guard-state locals. In the block loop, a block in the map suppresses
its `Call` terminator and emits the per-OS primitive + `goto bb_<target>` (recover
`target` from the `Call`, like generic-c `emit_sync_action`). Gating: emit the
prelude includes only when `!model.synchronization.drivers.is_empty()` and a
resource of the relevant kind is present.

## Tests (per-OS golden-C diffs)

Per kind/op golden assertion in `linux_c/mod_tests.rs` + `zephyr_c/mod_tests.rs`
(reuse `_sync_surface.rs` via the `emit_linux_c`/`emit_zephyr_c` seam), **plus a
distinct guard-state round-trip assertion** (review D2): Linux `unsigned long
_sync_flags_*` declared + paired `irqsave`/`irqrestore`; Zephyr `k_spinlock_key_t
_sync_key_*` assigned-from-acquire + paired `k_spin_lock`/`k_spin_unlock`. Zephyr
`SpinLock`≡`IrqSpinLock` collapse golden (both emit the key primitive; surface
distinct).

## As-built (2026-06-17)

- **Test seam chosen = e2e substring assertions, not byte goldens.** A new AMBA
  fixture `crates/codegen-coverage-runner/fixtures/_sync_os/sync-os` declares all
  four kinds (+ `compatible` so Zephyr's DT-match accepts it); `tests/sync_os_e2e.rs`
  bridges once and asserts the exact C02 primitive strings + the per-OS guard-state
  round-trip + the Zephyr collapse + no facade/`UnsafeCell` leak, over BOTH backends
  from one model. Preferred over hand-built synthetic models (`*/mod_tests.rs`) +
  byte goldens because it exercises the REAL bridge recognition → backend path and
  the assertions name the actual emitted locals. No committed `golden.*.c` for this
  fixture (substring assertions are the contract).
- **NEW finding — the suppressed-call RECEIVER temps must be elided, not just the
  guard.** `_t = &self.lock` (the `lock()`/`signal()` receiver) is dead once the
  call is replaced, AND is typed to the FACADE (`struct IrqSpinLock *`), which no
  longer matches the substituted field (`spinlock_t`) — leaving it would emit
  uncompilable C (`struct IrqSpinLock * _2 = &_1->mtx;` against a `spinlock_t`
  field) and re-introduce the facade struct definitions. `build_for_body` elides
  the first bare-local arg of every sync `Call` (preserving the `completion_wait`
  Duration arg, a later arg); the taint loop removes the derived borrow temps. This
  is why no `struct Mutex`/`struct UnsafeCell`/`struct Guard` leaks into the TU.
- **Receiver spelling** is `_1` (the driver `&self` param's C name), not `self` —
  the debug name is not preserved through the bridge; `&_1->mtx` is correct.
- **`completion_wait` dest** (the discarded `Result<(),RegseqError>`) stays a
  declared-but-unused local (`struct Result _16;`), matching the established
  set-but-unused backend posture + the generic-c oracle's void discard.
- Both backends + full `cargo test` green (154 ok suites, 0 failed); no golden churn.
