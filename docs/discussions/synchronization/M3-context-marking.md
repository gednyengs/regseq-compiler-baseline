# M3 — the context-marking analysis (the thesis component) — as-built

M3 (PLAN T10–T13) lands the construct's load-bearing pass: the first that is
**both a consumer and a producer of context markings**. Implemented after a
Codex foundation review (`M3-codex-foundation-review.md`, REVISE-FIRST → design
locked) whose two blocking findings are folded in. New module:
`regseq-driver/src/bridge/synchronization/context.rs`, run from `bridge/mod.rs`
right after `recognise_acquire_sites` (the `validate_isr_no_sleep` placement
discipline).

## T10 — the produced `SpinlockHeldRegion` marking

`regseq-model/src/synchronization.rs` gains `SpinlockHeldRegion { lock_id, kind,
body, acquire_block, blocks }` + a per-driver `held_regions: Vec<…>`
(`#[serde(default)]`; round-trip + back-compat tests added). The region is
computed by `context::held_region_blocks`: a BFS from the acquire block's CFG
**successors** (`Terminator::successors()`), stopping at — but **including** —
the acquire-site's `release_blocks` (the guard is held through a release block's
statements, dropped at its terminator; its successors are not traversed). So the
region ends at `guard_drop_early` (it is one of `release_blocks`), **not** the
syntactic scope. `acquire_block` is excluded (the guard is not held while the
acquire `Call` runs) unless a CFG back-edge re-reaches it while still held — a
genuine loop re-acquire, correctly included.

Only `SpinLock`/`IrqSpinLock` regions are **persisted** (the thesis
"spinlock-held atomic-section" marking); `Mutex` produces none (sleeping while
holding a sleeping mutex is legal). Regions are computed transiently for **every**
kind (the recursive-acquire check needs Mutex regions too).

It is a **may-held, path-insensitive** over-approximation. Soundness for its
consumers = *no false negative* (it never misses a block where the guard is
genuinely held). Residual false-positives are infeasible CFG paths / may-held
joins / a release the recognizer could not pair — all intraprocedural/lexical
(`I04`). Bridge test: `irqspinlock_acquire_produces_held_region_over_live_range`
(acquire excluded, release blocks ⊆ region) + `mutex_acquire_produces_no_region`.

## T11 — §7.1 ISR-legality + §7.2 wait-context (consume `IsrBinding`)

`check_isr_legality`: in a body whose `instance.item` carries
`AttrData::IsrBinding` (the exact `validate_isr_no_sleep` gate — Codex confirmed
correct + complete for the lexical slice), reject `Mutex::lock` / `SpinLock::lock`
acquires and `Completion::wait` (`SyncDiagnostic::ForbiddenIsrBlockingOp`);
`IrqSpinLock::lock` and `Completion::signal`/`reset` pass. §7.2 collapses into the
same rule (the only modeled non-thread context is `IsrBinding`). The §7.1
incompleteness — a transitively-IRQ-reachable illegal acquisition one helper-call
deeper — is the documented Open-4 gap: the conformance/audit output (T16) carries
it, and the construct **never** claims "ISR-safety verified". Compile-fail bridge
tests for Mutex/SpinLock/`wait` in `#[rseq::isr]`; positive tests for IrqSpinLock
+ signal in ISR.

## T12 — may-sleep-while-held + recursive-acquire + the Rust-enforced rejections

- **`sleep-in-atomic-section` (analysis):** `check_sleep_in_atomic` rejects a
  may-sleep call whose `Call`-terminator block is inside a spinning-kind region.
  The may-sleep predicate = { a `Sleep`-intrinsic callee (the same
  `AttrData::Intrinsic(IntrinsicKind::Sleep)` predicate timed-delay uses), a
  `Mutex::lock` acquire, a `Completion::wait` } — it agrees with timed-delay's.
  Bridge tests cover all three arms; `sleep_after_guard_drop_early_is_accepted`
  pins that the region ends at the early release.

- **`forbidden-try-lock-or-unlock` — recursive acquire (Codex BLOCKING #2,
  folded):** `check_recursive_acquire` rejects an acquire of lock-id `L` whose
  acquire block lies inside an existing held region of the **same** `L`.
  `lock(&self)` permits repeated shared borrows, so `let g1 = self.m.lock(); let
  g2 = self.m.lock();` *compiles* and the OS backends lower it to a real
  deadlocking lock — so this MUST be a static bridge reject, not left to the
  generic-c oracle's runtime net. A clean consumer of the T10 region; runs before
  backend emission. `sequential_same_lock_acquire_is_accepted` /
  `nested_distinct_spinlocks_are_accepted` guard against false positives. The
  existing M1 `double_acquire_is_caught_by_the_oracle` test was repurposed to
  `recursive_double_acquire_is_rejected_at_the_bridge` (caught earlier than the
  oracle now).

- **`guard-data-escape` / `guard-use-after-consume` / `try_lock` / `unlock`:**
  unrepresentable in the first-slice Rust surface (opaque guard → no `Deref` → no
  borrowed-data escape; by-value `guard_drop_early` + move-only guard →
  use-after-move; no `try_lock`/`unlock` method). They need **no** bridge
  diagnostic — `bridge_synchronization_guard_safety.rs` proves each is rejected by
  a subprocess `rustc` type-check (the rseq tool attrs are inert to rustc, so only
  the Rust legality of the body is under test), with a clean positive control.

`SHOULD-FIX #1 (folded):` contexts are computed after all acquire/completion
sites are recorded, so the may-sleep + recursive checks see every site in the
body.

## T13 — the oracle IRQ-reentry leg (M3 exit)

The generic-c oracle is extended so a held `IrqSpinLock` masks a **synchronous**
`regseq_irq_fire` — modeling the local-IRQ-disable of `spin_lock_irqsave` /
`k_spin_lock` (the IRQ-reentry behavioral check, short of true SMP). Per Codex
BLOCKING #1 (c+b, folded), the coupling is emitted **only** when a TU has BOTH
constructs (so single-construct TUs — and the committed interrupt-only goldens —
are byte-identical); it is built by **targeted unique-anchor string replacement**
on the existing `IRQ_HELPERS`/`SYNC_HELPERS` consts (which stay whole and
untouched), so there is no duplicated helper text to drift:

- `MASK_STATE` (both-only): `regseq_local_irq_masked` (the local-IRQ-disable
  nesting depth) + `regseq_lock_held_is_irqspin[]` (per held-stack slot, so
  `regseq_lock_release(id)` lowers the mask without a kind arg — the ABI is
  preserved).
- `irq_helpers_masked()`: `regseq_irq_fire` traces `IRQ fire line=<n> masked` and
  does not invoke the wrapper while masked. **Masked = dropped, NOT
  queued/replayed** (SHOULD-FIX #3, folded — the synchronous oracle models no
  pending delivery).
- `sync_helpers_masked()`: `regseq_lock_acquire` of an `IrqSpinLock` raises the
  mask (and records the slot kind); `regseq_lock_release` of one lowers it.

Tests (`generic_c/mod_tests.rs`, the `prelude::PRELUDE + bank + hand-authored
main` compile-and-run pattern — one TU, so `main` can call the `static` hooks):
- `t13_irqspinlock_masks_synchronous_irq_reentry`: mainline holds the IrqSpinLock
  across a fire (masked → no reentry → exit 0), fires again unheld (the ISR runs
  and acquires/releases cleanly — exactly 2 acquires total).
- `t13_without_masking_the_held_fire_double_acquires`: the **negative control** —
  the un-coupled banks re-enter on the held fire and trip the oracle's runtime
  double-acquire abort, proving the masking is load-bearing, not vacuous.

## M3 Codex diff review — SOUND (no blocking)

The full M3 delta was run past Codex (GPT-5.5, high effort, cwd = worktree) as an
adversarial diff review. **Verdict SOUND, zero blocking findings.** Codex verified
with file:line: the region BFS (successors, include releases, exclude acquire
unless re-reached by a back-edge), the may-sleep predicate over call-terminator
blocks, the all-kinds recursive check / spinning-only persistence, the `IsrBinding`
consumption + the preserved "incomplete, not verified" caveat, the both-construct-only
coupled emission, the mask bookkeeping (slot written after push, read before
`depth--`), the still-live runtime double-acquire net (via the negative control),
and append-only serde / no new intrinsic. Two should-fixes folded:
1. `prelude::replace_once` asserts each T13 coupling anchor occurs exactly once
   (fails loud on future helper-text drift instead of silently emitting an
   un-coupled bank); pinned by `t13_coupling_weaves_the_mask_logic_into_both_banks`.
2. Loop/back-edge regression tests: `sleep_in_a_loop_inside_a_spinlock_region_is_rejected`
   (region BFS reaches loop-body blocks) + `lock_acquired_and_released_each_loop_iteration_is_accepted`
   (no false-positive recursive reject across a back-edge).

## Status

M3 COMPLETE. Full `cargo test` green: **156 "test result: ok", 0 failed**.
Codex-SOUND. M4 (Tier-2 SMP discharge + cross-construct seams, eval repo) follows.
