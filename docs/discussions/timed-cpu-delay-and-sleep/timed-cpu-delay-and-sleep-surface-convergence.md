# Timed CPU Delay & Sleep — converged DSL surface (eval ⇄ compiler) + as-built

**Construct:** `timed-cpu-delay-and-sleep` (TCD). **Disposition:** a *ride-along
intrinsic family + framework types* (GRILL `D01`) — not a typed resource, no L2
properties, no standalone fixture campaign. Its value is the vocabulary
(`busy_wait`/`sleep` over an opaque `Duration`) + the legality rule that *other*
constructs consume.

**Authoritative sources.**
- Eval design analysis: `~/regseq-eval/docs/constructs/timed-cpu-delay-and-sleep/`
  (README, adequacy, model, dsl-surface, design, macOS cross-OS, codex review).
- Grill → PRD → Plan (the implementation-blocking decision record):
  `~/regseq-eval/.../grill-to-plan/specs/timed-cpu-delay-and-sleep/`
  — `GRILL-2026-06-16-123919.md` (contracts C01–C06, decisions D01–D11),
  `PRD-2026-06-16-132509.md` (R01–R13, I01–I08), `PLAN-2026-06-16-133944.md`
  (T01–T10 / M0–M3).

This doc is the **compiler-side spec contract**; the §"As-built" section is the
record of what shipped, kept current with the implementation.

---

## 1. Converged author surface (C01/C02 — `design-intended`)

```rust
// regseq::delay (re-exported from regseq::intrinsics::timed_delay)
#[rseq::intrinsic("busy_wait")] pub fn busy_wait(d: Duration);  // spin; legal in ANY context
#[rseq::intrinsic("sleep")]     pub fn sleep(d: Duration);      // yield; SLEEPABLE context only

#[rseq::framework_type("duration")]
pub struct Duration { ns: u64 }                 // opaque; NANOSECOND storage
impl Duration {
    pub const fn from_us(us: u64) -> Self;       // saturating ×1_000
    pub const fn from_ms(ms: u64) -> Self;       // saturating ×1_000_000
    pub const fn from_s(s: u64)  -> Self;        // saturating ×1_000_000_000
    pub const MAX: Duration;                     // saturation target on overflow
}
```

The author expresses **intent** (busy vs sleep) + **amount** only; mechanism
selection is backend-internal. Both intrinsics return `()` and cannot fail
(minimum-wait guarantee). **Deferred, not demoted** (GRILL `D02`, normative
taxonomy surface for a later fold): `sleep_ex`, `sleep_until`, `Instant`,
`SleepOpts`/`SleepOutcome`, `Duration::from_ns`.

---

## 2. IR recognition (C03 — append-only)

- `IntrinsicKind::BusyWait = 4`, `IntrinsicKind::Sleep = 5`
  (`regseq-model/src/domain_attr.rs`) — appended after `Config = 3`;
  `Log`/`Mmio`/`PollUntil`/`Config` discriminants unmoved (discriminant-pinned
  test). `from_name`: `"busy_wait" => BusyWait`, `"sleep" => Sleep`.
- `FrameworkType::Duration = 2` — appended after `RegseqError = 0` / `IrqResult = 1`
  (device-interrupt construct #6 added `IrqResult`). Bridge maps
  `#[rseq::framework_type("duration")]`.
- Recognition is the construct-neutral call-driven path
  (`bridge/intrinsic/recognition.rs`): the `regseq::delay::*` re-export resolves
  to the attributed `intrinsics::timed_delay::*` original. The per-call `Duration`
  argument flows as an ordinary call operand (value-ref; see As-built §B).

---

## 3. Lowering table (C04 / R06 / D04 / D09)

| Intrinsic | generic_c (oracle) | linux_c | zephyr_c |
|---|---|---|---|
| `busy_wait(d)` | `regseq_delay_busy_wait_ns(u64)` extern hook | `regseq_linux_busy_wait_ns(ns)` helper: `ndelay`(<1µs) / `udelay`(<1ms) / `mdelay`(≥1ms), round **up** | `k_busy_wait(us)`, round **up** to µs |
| `sleep(d)` | `regseq_delay_sleep_ns(u64)` extern hook | `msleep(ms)`, round **up** — msleep-only (D09) | `k_msleep(ms)`, round **up** to ms |

- **Minimum-wait (R06):** every unit conversion rounds **up**, via the
  overflow-safe ceiling `ns == 0 ? 0 : 1 + (ns - 1)/scale` (the naive
  `ns + scale-1` overflows near `Duration::MAX = u64::MAX`).
- **Intent (I04):** a `busy_wait` only ever lowers to a busy primitive, a
  `sleep` only to a sleeping one — observable as hook identity in the oracle.
- The generic_c oracle proves **amount + order + intent**, NOT real timing or
  the real primitive (I08); per-OS primitive fidelity is the linux_c/zephyr_c
  conformance diff.

---

## 4. Diagnostics

- **`TCD-ERR-01`** (C05, verbatim) — a `sleep` call lexically within a marked
  `#[rseq::isr]` handler body is rejected at the frontend, no IR (I06):
  ```
  TCD / Timed CPU Delay And Sleep: 'regseq::delay::sleep' is not legal in IRQ context (call site reachable from rseq::isr per the callback-context model); use 'regseq::delay::busy_wait' instead
  ```
  `busy_wait` is never rejected on context grounds (I07). **Phase-1 is
  marking-based** (the handler body); full callgraph reachability is GRILL `O1`,
  NOT shipped — the thesis text must not claim full static sleep-in-ISR
  enforcement.
- **`TCD-ERR-02`** (R13/D11) — a recognized `busy_wait`/`sleep` facade whose
  signature is not exactly one by-value `regseq::Duration` argument (a
  mis-declared facade; the ordinary typed facade is subsumed by rustc's own
  type-check). Mirrors device-interrupt's `IRQ-ERR-ISR-SIGNATURE`.
- All `TCD-WARN-*` deferred (D11).

---

## 5. Host harness (C06/D08)

The generic_c hooks are emitted as extern **calls**; the codegen-coverage
harness links its **own** record-only implementation (`toolchain.rs`,
`DELAY_HARNESS_C`), gated on the TU referencing the hooks. Each call records an
ordered `{kind, ns, seq}` event by printing it at call time — so events
interleave with the `regseq_mmio_*`/log trace by execution order — and advances
a **virtual** clock by `ns`. No real time passes.

---

## As-built (shipped on `construct/timed-cpu-delay-and-sleep`)

Built on `main` (which by 2026-06-16 had merged `backend/zephyr-c-e1000` — the
real per-OS backends — and device-interrupt construct #6 — the `#[rseq::isr]`
marking). This resolved the two risks the PLAN carried as open (OR-1 greenfield
platform emission, OR-3 missing ISR marking): both prerequisites already existed.

### A. Milestones (PLAN T01–T10)
- **M0 — Foundations.** `Duration` µs→ns migration with saturating ctors + `MAX`
  (fixture `pl011-full-spec/src/regseq/time.rs`; a compile-time `const _`
  conformance assertion); `FrameworkType::Duration` + the bridge `"duration"`
  mapping, with the framework-type materialization broadened from enum-only to
  **enum-or-struct** (`Duration` is a struct) so the marker is actually recorded.
- **M1 — Host vertical slice.** `IntrinsicKind::{BusyWait,Sleep}`; bridge facade
  recognition; `TCD-ERR-02`; generic_c `emit_delay` (extern hook calls,
  demand-gated `DELAY_HOOKS`); the harness record-only fake clock; the
  behavioral fixture `fixtures/_delay/delay-settle` (stdout oracle proving
  amount + order + intent + virtual clock).
- **M2 — Per-OS fidelity.** linux_c + zephyr_c lowering (per-TU demand-injected
  helpers, `uses_delay_*` flags + `<linux/delay.h>` / `<zephyr/kernel.h>`);
  `delay_helpers` unit tests + the `delay_conformance_e2e` emit test.
- **M3 — Frontend legality.** `TimedCpuDelayDiagnostic::SleepInIsr` (verbatim
  C05) + the post-`finalise_isrs` pass `validate_isr_no_sleep` walking
  `#[rseq::isr]`-marked handler bodies for direct `Sleep` calls.

Each milestone closed with an external **Codex** review (foundation review
before M2; diff review after each); all four passed. Full workspace `cargo test`
green (2466 tests, 0 failed).

### B. Honest scope & deviations (Codex-reviewed)
1. **`Duration` is always a value-ref at lowering.** A `const Duration` folds to
   a single-field-struct *scalar* const that the bridge rejects
   (`UnsupportedConst`), so every `Duration` reaching a delay call is the
   value-ref result of an inline `Duration::from_*` constructor. Backends read
   `(<operand>).ns` off the operand, taking the `Duration` type from the
   **callee's first parameter** (known for any operand shape), not `operand_ty`
   (which is `None` for a constant).
2. **linux_c uses the runtime magnitude-dispatch helper UNIFORMLY** (not C04's
   "const → single static primitive"): because the ns magnitude is never a
   compile-time constant at lowering (point 1), C04's const form collapses into
   the runtime arm for all `busy_wait`s. Minimum-wait (R06) still holds via the
   helper's round-up. (Codex foundation review: approved; const-propagation to
   recover `from_us(literal)` magnitudes is scope creep.)
3. **Narrowing precondition.** `msleep`/`k_busy_wait`/`k_msleep` take 32-bit
   arguments; a `Duration` is assumed to fit (true for any real driver delay —
   the corpus tops out at a 4 s settle). The ceiling arithmetic itself is
   overflow-safe to `u64::MAX`.
4. **`TCD-ERR-01` is phase-1 marking-based.** It catches a `sleep` lexically in
   the handler body; a `sleep` one helper-call deeper escapes until O1.
5. **The host behavioral fixture interleaves delays with `[INFO]` log lines**
   (not MMIO writes): no codegen-coverage fixture exercises MMIO without the
   heavy composed-fixture machinery, and the log trace proves the same
   observable property (ordered interleaving + amount + intent + virtual clock).
   Per-OS register interaction is covered by the conformance emit test + the
   composed fixtures.

### C. Cross-construct hand-offs
- `Duration` µs→ns is **value-preserving** for the existing consumers
  (`poll_until` signature, `reset.toggle`, `sync.wait` timeout) — call sites
  unchanged; no golden/snapshot churned.
- The PL011 crosswalk row 72 (`serial.console_write` `cpu_relax`) is reclassified
  OUT of TCD (an untimed condition poll → register-access poll-lowering, GRILL
  `D06`); TCD's standalone corpus evidence is E1000-only.
