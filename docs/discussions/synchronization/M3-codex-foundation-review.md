# M3 Codex foundation review — outcome (REVISE-FIRST → design locked)

Before implementing M3 (the thesis component, T10–T13) the design memo was run past
Codex (GPT-5.5, high effort, cwd = this worktree) for an adversarial foundation review
against the actual code. Verdict **REVISE-FIRST**; both blocking findings folded into the
design *before* writing code. The locked design:

## T10 — produce `SpinlockHeldRegion` (the marking)
- Model: `SpinlockHeldRegion { lock_id, kind, body: InstanceRef, acquire_block, blocks }`
  + per-driver `held_regions: Vec<…>` (`#[serde(default)]`). Persist **only** the
  spinning kinds (SpinLock/IrqSpinLock) — the thesis "spinlock-held atomic-section"
  marking (PLAN T10). Mutex gets no persisted region (sleeping-while-holding-a-mutex is
  legal).
- Region = BFS from the acquire block's **successors** over `Terminator::successors()`,
  stopping at (but including) the release blocks; release-block successors are not
  traversed. Ends at `guard_drop_early` (it is one of `release_blocks`), not the syntactic
  scope. **Codex confirmed sound (no false negatives)**: acquire/release are both `Call`
  terminators, so only the normal target is post-acquire; release blocks bound the region.
- **Documented as a may-held, path-insensitive region** (SHOULD-FIX #2): the only
  false-positives are infeasible CFG paths, may-held joins, or a release the recognizer
  could not pair (out of slice). `Terminator::successors()` is the whole CFG API — there
  are no separate unwind edges to promise.

## T11 — §7.1 ISR-legality + §7.2 wait-context (consume `IsrBinding`)
- Gate on the body's `instance.item` carrying `AttrData::IsrBinding` (the exact mirror of
  `timed_delay::validate_isr_no_sleep`). **Codex confirmed correct + complete for the
  lexical slice**: `finalise_isrs` marks the handler's function item, and a lexical acquire
  in the ISR body is seen under that instance. Closures/helpers = the documented
  interprocedural gap (Open-4).
- In an ISR-context body reject `Mutex::lock` / `SpinLock::lock` acquires and
  `Completion::wait` (`forbidden-isr-blocking-op`); `IrqSpinLock::lock` / `signal` /
  `reset` pass. §7.2 collapses into the same rule (the only modeled non-thread context is
  `IsrBinding`). Never "ISR-safety verified" — incompleteness surfaced as a documented
  limitation (doc here + the T16 conformance note).

## T12 — may-sleep-while-held + recursive-acquire + the Rust-enforced rejections
- **may-sleep-while-held (analysis):** reject a may-sleep call whose `Call`-terminator block
  is inside a SpinLock/IrqSpinLock region (`sleep-in-atomic-section`). may-sleep predicate =
  { a `Sleep`-intrinsic callee (same `AttrData::Intrinsic(IntrinsicKind::Sleep)` predicate
  as timed-delay), a `Mutex::lock` acquire site, a `Completion::wait` site } in the body.
- **BLOCKING #2 (folded): recursive same-lock acquire is a STATIC bridge diagnostic, not
  runtime-only.** `lock(&self)` permits repeated shared borrows, so `let g1=self.m.lock();
  let g2=self.m.lock();` *compiles*, and the OS backends lower to real locks (deadlocking
  C); the generic-C oracle catches it only at runtime. So: reject an acquire of lock-id `L`
  whose acquire block lies inside an existing held region of the **same** `L`
  (`recursive-acquire`). A clean T10-region consumer; runs before backend emission. Covers
  all kinds (Mutex included), so the region is computed for every acquire site (the
  persisted `SpinlockHeldRegion` table keeps the spinning subset only).
- **guard-data-escape / guard-use-after-consume / forbidden-try-lock-or-unlock:**
  unrepresentable in the first-slice Rust surface (no `try_lock`/`unlock` method;
  `guard_drop_early(self)` makes post-consume use a move error; the guard borrows `&self`
  so escaping it is a lifetime error). Proven by **negative fixtures that fail at rustc**;
  the bridge variants are reserved catalog ids.
- **SHOULD-FIX #1 (folded): compute contexts after collecting all sites** so the may-sleep +
  recursive checks see every site in the body.

## T13 — oracle IRQ-reentry leg (M3 exit)
- **BLOCKING #1 (folded): coupling = c+b.** Emit a sync-aware combined helper variant
  **only when BOTH** `interrupt.drivers` and `synchronization.drivers` are non-empty;
  single-construct TUs stay byte-identical (no golden churn). Track per-lock-id IRQSPIN-ness
  set at `regseq_lock_init(id, "IRQSPIN")` (the init hook already has the kind), so
  acquire/release maintain a `regseq_local_irq_masked` depth **without changing the
  `regseq_lock_release(id)` call ABI**.
- Mask-aware `regseq_irq_fire`: while `regseq_local_irq_masked > 0`, trace the fire as
  **"masked"** and do not invoke the wrapper. **SHOULD-FIX #3 (folded): not "deferred"/
  "queued"** — the synchronous oracle drops the masked fire; pending replay is a separate
  model the oracle does not claim.
- Test: mainline holds the `IrqSpinLock`, fires the IRQ (must be masked → no re-entrant
  acquire → no double-acquire abort); fires while unheld (wrapper runs, the handler's own
  acquire/release is clean). Models `spin_lock_irqsave` / `k_spin_lock` local-IRQ-disable
  on a uniprocessor — **Codex confirmed sound, short of true SMP**.

Full `cargo test` was green (155 ok suites, 0 failed) at the start of M3.
