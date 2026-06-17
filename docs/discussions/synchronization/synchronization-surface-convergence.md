# Synchronization (locks) — converged DSL surface + as-built (eval ⇄ compiler)

> **Home:** this is the **final spec contract** for the synchronization construct,
> co-located with the implementation in this repo
> (`docs/discussions/synchronization/`). The evidence-grounded **design analysis**
> (adequacy, model, DSL-surface, design) lives in the eval repo at
> `regseq-eval/docs/constructs/synchronization/`; filename references below to
> those analysis docs resolve there. The grill→PRD→plan decision record is
> `regseq-eval/.claude/sekekama/grill-to-plan/specs/synchronization/`
> (`GRILL-2026-06-16-204030.md` rev 2, `PRD-2026-06-16-222408.md`,
> `PLAN-2026-06-16-221216.md`). The milestone realization logs are the sibling
> `M1-architecture-proposal.md`, `T07-…`, `T08-lifecycle-slot.md`,
> `T09-subsystem-locking.md`, `M2-codex-review.md`, `M3-codex-foundation-review.md`,
> `M3-context-marking.md` in this folder; the eval-side M4 validation record is
> `regseq-eval/docs/constructs/synchronization/synchronization-m4-validation.md`.

**Date:** 2026-06-17 · **Status:** ✅ **FIRST SLICE IMPLEMENTED + Codex-SOUND** —
M1–M3 (author surface, per-OS lowering, lifecycle slot, the context-marking
analysis) are implemented in the compiler with the full suite green (**157 "test
result: ok", 0 failed**); each milestone passed an adversarial Codex review (M1
SOUND; M2 SOUND after 6 rounds; M3 SOUND foundation + SOUND diff). M4 is
construct-side complete with the **empirical Tier-2 SMP campaign left as a
documented open gate** (the live run is infra-blocked and not fabricable —
§9/§10). **Claim = feasibility + analytic generalization, NOT proven
completeness** (`regseq-eval/docs/methodology/`).

**Scope (GRILL `D03` / PRD).** The **first implementation slice**: the four
explicit lock kinds, the RAII guard, the decl-time cardinality, the four checks,
per-OS lowering to Linux + Zephyr + the generic-c oracle, and the lifecycle slot —
**all checks/markings intraprocedural/lexical** (`I04`). Deferred surfaces
(`RwLock`/`Semaphore`/`AllWaiters`/`Lock<T>` inference/interprocedural hardening)
are designed-in but NOT built.

---

## 0. The construct in one paragraph

The author declares a lock by placing a **typed field** on the `#[rseq::driver]`
struct — **the field's type IS the kind** (no separate "kind" attribute; `C01`/
`D01`, no inference). Acquire returns an RAII `Guard<'_,T>` that releases on drop
(or explicit `guard_drop_early(self)`); there is **no standalone `unlock`** (`I02`).
The construct is the first that is **both a consumer and a producer of context
markings**: it *consumes* device-interrupt's `IsrBinding` IRQ-context marking and
timed-delay's may-sleep predicate, and *produces* the **spinlock-held
atomic-section marking** over each spinning guard's (approximated) live range — the
thesis contribution, for the intraprocedural/lexical case.

---

## 1. The authored surface (`C01` — the first-slice set, frozen)

Declared in the per-fixture `regseq` facade (the regaccess-shim pattern;
`crates/codegen-coverage-runner/fixtures/_sync_surface.rs`):

- **Lock kinds (kind = the wrapper type):**
  - `Mutex<T>` — sleeping mutual exclusion (`may_sleep = Y`, `irq_safe = N`).
  - `SpinLock<T>` — spinning, preemption-off (`may_sleep = N`, `irq_safe = N`;
    **thread-only**).
  - `IrqSpinLock<T>` — spinning + IRQ save/restore (`may_sleep = N`,
    `irq_safe = Y`); the corpus witness kind (E1000 `stats_lock`, the PL011
    ISR↔mainline seam).
  - `Completion` — one-shot signaling; carries **no `T`**.
- **Guard.** `Guard<'_,T>` — move-only, non-`Copy`, **OPAQUE** (no `Deref`/
  `DerefMut` in the first slice, so the protected `T` cannot be read through it —
  keeping the slice payload-free *by the type system*); drop releases;
  `guard_drop_early(self)` releases early by value.
- **Ops** (wrapper methods, **NOT intrinsics** — no new `IntrinsicKind`):
  `lock() -> Guard` on the three lock kinds; `completion_wait(Duration) ->
  Result<(),RegseqError>` / `signal()` / `reset()` on `Completion`
  (`Duration` is the shared timed-delay `FrameworkType::Duration`).
- **Decl-time cardinality.** `#[rseq::sync(cardinality = SingleWaiter)]` on a
  `Completion` field; `SingleWaiter` is the only first-slice cardinality. An
  unknown value OR `AllWaiters` is **rejected loud** at recognition (never
  silently defaulted).
- **Null surface.** `init_primitives` / `destroy_primitives` / `subsystem_locking`
  are compiler-managed; the author never calls them. `subsystem_locking` is
  analysis-only — the framework ISR wrapper performs **no** auto-acquire
  (`G-SYN-17`); the authored ISR body acquires explicitly.

**L1/L2 split.** No concrete lock implementation lives in the `.rs` spec; the kind
selects the per-OS primitive at lowering. No L2 entries are required for the lock
fields (unlike `mmio`/`interrupt` resources).

---

## 2. The model IR (`regseq-model`)

A `synchronization` sibling-table module on `RegseqModel` (mirroring
`interrupt`/`device_lifecycle`), keyed by the driver-struct `ItemId`:

- `SyncResource { name, field, kind, protected_type: Option<TyId>, cardinality }`
  — declared lock/completion fields in declaration order; the index is the stable
  `lock_id`.
- `AcquireSite { lock_id, op: LockAcquire, body: InstanceRef, acquire_block,
  guard_local, release_blocks }` — recognized `lock()` acquires + their paired
  guard releases (drop-glue `Call` or `guard_drop_early`).
- `CompletionSite { lock_id, op: {Wait,Signal,Reset}, body, block }` — point ops.
- `SpinlockHeldRegion { lock_id, kind, body, acquire_block, blocks }` — **the
  produced marking** (M3); the spinning-kind subset of held regions (Mutex
  produces none). `blocks` = the may-held, path-insensitive block set where the
  guard is held at entry.
- `AttrData::SyncResource { kind }` — the field marker, appended at discriminant
  **5** (discriminant **4 reserved** for the atomicity construct's
  `AttrData::Atomic` — the two append-only enums stay disjoint, `C06`/Open-1).

All append-only with `#[serde(default)]`; pre-construct models still deserialize.
**No new `IntrinsicKind`.** Acquire-site bodies are content-addressed by
`InstanceRef` (survive the Phase-4 instance sort).

---

## 3. Recognition (`bridge/synchronization/`)

- **Phase-4 field recognition** (`recognition.rs`): name-based classification of
  the four kinds (matching the `Mmio`/`Interrupt` anchors) → `SyncResource` +
  `AttrData::SyncResource`; the decl-time cardinality parse; lock fields routed
  away from the device-lifecycle user-data classifier. **No inference** (`D01`).
- **Phase-5 op recognition** (`acquire.rs`): walk materialized bodies, anchor each
  `lock()`/`wait`/`signal`/`reset` on the receiver field's `SyncResource`, record
  the acquire/completion sites + the guard release blocks (the drop-glue `Call`
  chased back through the synthetic `&raw mut guard` temp). A consumed
  `Completion::wait` result is **rejected at recognition**
  (`CompletionWaitResultConsumed`) — the first slice discards the
  timeout→`Result`, so a body that reads it would emit wrong code; the rejection
  is a provably-complete 2-pass intraprocedural taint analysis (M2 review).

---

## 4. The context-marking analysis (`bridge/synchronization/context.rs`, M3 — the thesis component)

Runs after `recognise_acquire_sites` (the `validate_isr_no_sleep` placement
discipline). **All intraprocedural/lexical** (`I04`).

- **Produces (`C04`/T10).** Per `SpinLock`/`IrqSpinLock` acquire, a
  `SpinlockHeldRegion` = BFS from the acquire block's CFG successors, stopping at
  (incl) the `release_blocks` — ends at `guard_drop_early`, not the syntactic
  scope. May-held, path-insensitive; sound for its consumers (no false negative).
- **§7.1 ISR-legality + §7.2 wait-context (`C03`/T11).** Consume `IsrBinding`: in
  an ISR-context body, reject `Mutex::lock` / `SpinLock::lock` / `Completion::wait`
  (`ForbiddenIsrBlockingOp`); `IrqSpinLock` + `signal`/`reset` pass.
  **Sound-but-incomplete** — a transitively-IRQ-reachable illegal acquire deeper
  than the lexical body escapes until the Open-4 hardening; **never presented as
  "ISR-safety verified"** (the conformance/audit output carries the limitation,
  §6).
- **may-sleep-while-held (`C03`/T12).** Reject a may-sleep call (timed-delay
  `Sleep` intrinsic / `Mutex::lock` / `Completion::wait`) whose `Call`-terminator
  block is inside a spinning-kind region (`SleepInAtomicSection`). The may-sleep
  predicate agrees with timed-delay's.
- **recursive same-lock acquire (`C03`/T12).** Reject an acquire of lock-id `L`
  whose block lies inside an existing same-`L` held region
  (`ForbiddenTryLockOrUnlock` — recursive). `lock(&self)` permits repeated shared
  borrows, so this *compiles* and the OS backends would lower it to a real
  deadlocking lock — caught statically (a consumer of the T10 marking), not left
  to the oracle's runtime net.
- **Rust-enforced rejections.** `guard-data-escape` / `guard-use-after-consume` /
  `try_lock` / standalone `unlock` are **unrepresentable** in the first-slice
  surface (opaque guard → no borrowed-data escape; by-value `guard_drop_early` +
  move-only guard → use-after-move; no `try_lock`/`unlock` method) — proven by
  the subprocess-rustc compile-fail tests (`bridge_synchronization_guard_safety`).

Diagnostics are construct-local: `BridgeError::SynchronizationValidation(SyncDiagnostic)`.

---

## 5. Per-OS lowering (`C02`) + lifecycle slot (`C05`)

- **Field-type substitution** (the `emit_type_defs`→`field_override` seam): lock
  fields lower to `struct mutex`/`spinlock_t`/`struct completion` (Linux),
  `k_mutex`/`k_spinlock`/`k_sem` (Zephyr); the payload `T` is NOT stored (opaque
  guard). Per-OS guard hidden state round-trips through the guard: Linux `unsigned
  long flags` (`spin_lock_irqsave`/`spin_unlock_irqrestore`), Zephyr
  `k_spinlock_key_t key`. Zephyr `SpinLock ≡ IrqSpinLock` collapse, kinds kept
  distinct on the surface (`I06`). Primitive spellings verified against the pins
  (`T07-version-pin-recheck.md`).
- **Lifecycle slot (`C05`/T08).** `init_primitives` in the init preamble **before**
  the interrupt enable and `on_init`; `destroy_primitives` in the deinit postamble
  **after** the interrupt disable — so the lock-live window ⊇ the line-live window
  by construction. Linux drains (`disable_irq`); Zephyr `destroy` is a no-op
  (static primitives — safety by no-destroy). Defined relative to
  `interrupt_order.rs::IrqPhase` (no pre-existing sync slot — Codex finding #1).
- **generic-c oracle (`S11`).** `regseq_lock_init/acquire/release` +
  `regseq_completion_signal/wait/reset` model lock state and assert: balanced
  acquire/release, no double-acquire, LIFO guard-drop, release-only-the-held,
  acquire-after-init, no-held-at-exit. **T13 IRQ-reentry leg:** when a TU has BOTH
  an interrupt and a sync surface, a coupled bank (built by unique-anchor
  replacement on the untouched single-construct consts — so single-construct TUs,
  incl. the committed irq goldens, are byte-identical) makes a held `IrqSpinLock`
  **mask** a synchronous `regseq_irq_fire` (dropped, not replayed — modeling
  `spin_lock_irqsave` local-IRQ-disable). A negative control proves the masking is
  load-bearing (un-coupled → double-acquire abort).

---

## 6. Conformance / audit output (`C03`/`I03`, T16)

For an interrupt+sync TU (where the §7.1 check is live), the generic-C
both-construct artifact carries the `RSEQ-SYNC-AUDIT` note
(`generic_c/prelude.rs::SYNC_ISR_SAFETY_AUDIT`): the §7.1 check is
**SOUND-BUT-INCOMPLETE** and the artifact is **NOT "ISR-safety verified"**. The
deferred-surface normativity (`I01`: first-slice surface = exactly the C01 set) and
the empirical-pin residual (Open-2) are recorded in the spec + the eval M4 record.

---

## 7. Cross-construct seams

- **Atomicity (`C06`/Open-1).** Author-declared cut: a lock-kind field → sync; an
  atomic op → atomicity. Sync recognizes only its own surface and never infers a
  bare flag's intent (`__E1000_DOWN`/`__E1000_RESETTING` stay with atomicity).
  `AttrData` discriminant 4 reserved for `AttrData::Atomic`.
- **Device-lifecycle (`C05`/Open-3).** The init/destroy slot ordering is honored by
  the existing lifecycle preamble/postamble machinery (no device-lifecycle edit
  required); validated by `sync_os_e2e::c05_init_destroy_placement` (placement) +
  `sync_irq_e2e` (Linux `spin_lock_init` before `request_irq`).
- **Timed-delay.** The produced `SpinlockHeldRegion` is what timed-delay's
  `TCD-ERR-01` forward note will consume to close its sleep-in-atomic note for the
  intraprocedural case; T12 already owns the may-sleep-while-held check inside the
  sync pass (no timed-delay edit in this slice).

---

## 8. As-built — what was implemented + verified

| Milestone | Content | Codex |
|---|---|---|
| M1 (T01–T05) | `IrqSpinLock<T>` → recognition → IR → generic-c oracle (balanced/no-double/LIFO) | SOUND |
| M2 (T06–T09) | All four kinds + ops + cardinality; Linux+Zephyr lowering; lifecycle slot; `subsystem_locking` null; the `wait_result_consumed` taint analysis | SOUND (6 rounds) |
| M3 (T10–T13) | `SpinlockHeldRegion`; §7.1/§7.2; may-sleep-while-held; recursive-acquire; oracle IRQ-reentry masking | SOUND (foundation + diff) |
| M4 (T14–T16) | PL011 seam modeled (`IrqSpinLock<IsrState>`); cross-construct seams confirmed/recorded; §7.1 conformance note; version-pin recheck | SOUND (no blocking) |

Tests (compiler): model round-trip + back-compat (`regseq-model`); bridge
recognition + the M3 checks + loop/back-edge cases
(`bridge_synchronization_recognition.rs`, 21); the Rust-enforced rejections
(`bridge_synchronization_guard_safety.rs`, 5); per-OS C02 matrix + lifecycle + the
M2 negatives (`sync_os_e2e`, 6); the recursive-acquire static reject + oracle
discipline (`sync_oracle_e2e`, 4); G-SYN-17 + the combined irq+lock fixture
(`sync_irq_e2e`, 1); **the bilateral PL011 ISR↔mainline seam — a generated
`IrqSpinLock<IsrState>` driver where both the ISR and a mainline path acquire the
SAME lock, verified through the real Linux + Zephyr lowering**
(`sync_pl011_seam_e2e`, 2); the T13 masking + negative control + the T16 audit note
(`generic_c/mod_tests.rs`). Full suite: **157 ok, 0 failed**.

---

## 9. Honest scope & caveats (the thesis-defensibility line)

- **Intraprocedural/lexical only (`I04`).** Every check/marking is within one body.
  The interprocedural closure (transitive IRQ-context, guard-live-range callee
  summaries) is the **deferred Open-4 hardening**; the corpus is intraprocedural,
  so this blocks nothing first-slice but is honestly the completeness floor.
- **§7.1 sound-but-incomplete (`I03`).** Surfaced as a documented limitation
  (§6); **never** "ISR-safety verified".
- **The empirical Tier-2 SMP pin discharge is OPEN (`D04`/`I08`, Open-2).** The
  construct-side obligation (the `IrqSpinLock<IsrState>` seam + the §7.1 check) is
  satisfied, and the bilateral locked PL011 driver is **provably producible** — the
  `sync_pl011_seam_e2e` fixture generates it and verifies both the ISR and a
  mainline path acquire the same `isr_state` lock through the real Linux/Zephyr
  lowering. The **empirical** race-free SMP campaign was nonetheless **not run**
  (the live campaign needs a ~1–2 h substrate build + the generated driver wired
  into the DUT pipeline; it is not fabricable). `QEMU_SMP=1` **remains binding**
  until that run completes. Residual: the `IrqSpinLock<T>` payload type-covers the
  ISR-shared *data*, but in-guard *register* placement is author/codegen
  discipline — a finite SMP run corroborates but cannot prove completeness. See
  `pl011/TIER2-PIN-device-interrupt.md` + the eval M4 record.
- **Payload-free guards.** The opaque guard keeps the slice payload-free by the
  type system; `Deref` (payload access) lands with a later payload-lowering slice.

---

## 10. Open items / next

1. **Empirical Tier-2 SMP discharge** (the one operational gate to retire
   `pl011/TIER2-PIN-device-interrupt.md`): author a PL011 full spec with
   `isr_state: IrqSpinLock<IsrState>`, generate the locked Linux DUT, build the
   substrate, run `QEMU_SMP=2` race-free, then retire the pin + lift the whitelist.
2. **Open-4 interprocedural hardening** — transitive IRQ-context marking +
   guard-live-range callee summaries (§7.1 completeness; may-sleep across calls).
3. **Deferred surfaces** — `RwLock`/`Semaphore`/`wait_interruptible`/`AllWaiters`/
   `Lock<T>` inference + `#[rseq::default_lock]`; guard `Deref` (payload access).
4. **OS-backend conformance note** — emit the `RSEQ-SYNC-AUDIT` equivalent in the
   Linux/Zephyr shipped drivers (currently the generic-C reference artifact only).
