# Atomic Memory Access & Ordering — converged DSL surface (eval ⇄ compiler) + as-built

The spec contract for the **atomicity** construct (`atomic-memory-access-and-ordering`):
a typed driver-local `Atomic<T>` + the lock-free op surface + a standalone CPU memory
`fence`, each op carrying an explicit **compile-time-constant** memory `Ordering`. CPU
memory ordering ONLY — MMIO/bus ordering belongs to register-access (D02).

- **Eval design analysis:** `~/regseq-eval/docs/constructs/atomic-memory-access-and-ordering/`
  (adequacy, model, dsl-surface, design, codex-plan-review-summary).
- **Decision chain (eval):** GRILL `GRILL-2026-06-16-221252` (rev 3, D01–D15, C01–C07) →
  PRD `PRD-2026-06-16-221601` (R01–R13, I01–I08) → PLAN `PLAN-2026-06-16-221737` (T01–T19).
  The GRILL is the authoritative contract; this doc is the as-built realization + the
  contract feedback the implementation surfaced.
- **Claim scope (D04/I06):** faithful per-target *primitive selection* + correct
  placement + value correctness — NOT weak-memory soundness (that needs a litmus/herd
  checker, a different instrument).

## 1. Converged author surface (C01)

```rust
pub enum Ordering { Relaxed, Acquire, Release, AcqRel, SeqCst }   // every op: a COMPILE-TIME CONSTANT (D14)
pub struct Atomic<T> { /* T ∈ {u32,i32,u64,i64} + permissive raw pointer *mut T / *const T (G-ATM-07) */ }
impl<T: AtomicElement> Atomic<T> {
    pub fn load (&self, order: Ordering) -> T;
    pub fn store(&self, value: T, order: Ordering);
    pub fn fetch_add/sub/or/and/xor(&self, v: T, order: Ordering) -> T;   // prior
    pub fn swap(&self, v: T, order: Ordering) -> T;                       // prior
    pub fn compare_exchange(&self, current: T, new: T, success: Ordering, failure: Ordering) -> Result<T, T>;
    pub fn bit_test_set/bit_test_clear(&self, bit: u32, order: Ordering) -> bool;  // prior bit; UNSIGNED T
}
pub mod atomic { pub fn fence(order: Ordering); }   // CPU memory fence; never MMIO
// optional field attr: #[rseq::atomic(initial = <literal>)]
```

**Per-op valid `Ordering` (C01, frontend-enforced over the const discriminant):**

| Op | Valid | Invalid ⇒ diagnostic |
|---|---|---|
| `load` | Relaxed, Acquire, SeqCst | Release/AcqRel ⇒ DEGENERATE-ACQREL |
| `store` | Relaxed, Release, SeqCst | Acquire/AcqRel ⇒ DEGENERATE-ACQREL |
| `fetch_*`/`swap` | all 5 | — |
| CAS `success` | all 5 | — |
| CAS `failure` | Relaxed, Acquire, SeqCst **and** `failure ⊑ success` | else DEGENERATE-ACQREL |
| `bit_test_*` | all 5 | (unsigned `T`, `bit < width` — C07) |
| `atomic::fence` | Acquire, Release, AcqRel, SeqCst | Relaxed ⇒ FENCE-RELAXED |
| **all** | the `Ordering` arg is a **compile-time constant** | non-const ⇒ ORDERING-NONCONST |

Lattice for `⊑`: `Relaxed ⊏ {Acquire, Release} ⊏ AcqRel ⊏ SeqCst` (Acquire/Release incomparable).

## 2. IR recognition (C06 — append-only)

- `IntrinsicKind::Atomic(AtomicOp) = 6` (after `Sleep = 5`), with
  `AtomicOp { Load=0, Store=1, FetchModify(FetchOp)=2, CompareExchange=3, BitTestSet=4,
  BitTestClear=5, Fence=6 }` and `FetchOp { Add=0..Swap=5 }`.
- `AttrData::Atomic { element: AtomicElement, initial_value: Option<i128> } = 4` (field-attr).
- `from_name`: `atomic_load`/`atomic_store`/`atomic_fetch_*`/`atomic_swap`/
  `atomic_compare_exchange`/`atomic_bit_test_{set,clear}`/`atomic_fence`.
- The const `Ordering` rides in the `Call.args` operand (the model `Call` terminator is
  UNCHANGED — no metadata slot, I08); `RegseqModel::resolve_operand_enum_variant` is the
  ONE shared read (frontend legality + every backend selector use it, chasing the
  reaching-def temp for `Const::EnumValue` / fieldless `Aggregate(Enum)`).
- Recognition (`bridge/intrinsic/recognition.rs`) maps the tags; the atomicity bridge
  (`bridge/atomic.rs`) records the field-attr + runs the legality passes.

## 3. Per-target lowering table (C03/C04/C07)

Storage is **uniform by width** (no bit-tested-vs-not storage split — Codex M4 review §B):

| Op | generic-c (oracle, C05) | Linux | Zephyr |
|---|---|---|---|
| storage | host cell table (per-`FieldPath` cell id) | `atomic_t`(32)/`atomic64_t`(64)/`atomic_long_t`(ptr) | plain width-`T` cell (`uint32_t`/`int64_t`/`uintptr_t`/…) |
| `load` | `regseq_atomic_load(cell,w,ord)` | `atomic_read[_acquire]` (SeqCst→`_acquire`) | `__atomic_load_n(&f, mo)` |
| `store` | `regseq_atomic_store(...)` | `atomic_set[_release]` (SeqCst→`_release`) | `__atomic_store_n(...)` |
| `fetch_*`/`swap` | `regseq_atomic_rmw(...)→prior` | `atomic_fetch_*{suffix}` / `atomic_xchg{suffix}` | `__atomic_fetch_*` / `__atomic_exchange_n` |
| `bit_test_*` | `regseq_atomic_bittest(...)→prior` | `atomic_fetch_or`/`_and{suffix}` (prior bit extracted) | `__atomic_fetch_or`/`_and` |
| `compare_exchange` | `regseq_atomic_cmpxchg(...,&obs)→int` | `atomic_try_cmpxchg{suffix}` (collapse = LUB(succ,fail)) | `__atomic_compare_exchange_n` (DUAL native) |
| `fence` | `regseq_atomic_fence(ord)` | `smp_mb()` (conservative) | `__atomic_thread_fence(mo)` |

Suffix: Relaxed→`_relaxed`, Acquire→`_acquire`, Release→`_release`, AcqRel/SeqCst→`""`
(full barrier). `Result<T,T>` is built into the opaque enum carrier (Ok=variant 0,
Err=variant 1, observed prior as payload) — ordinary control flow (OR-7/I07). Bit-op
shift amounts are masked to width-1 (UB-free). `#[rseq::atomic(initial=N)]` is seeded at
field-init (host: a `constructor`-attributed cell init; Linux: `atomic_set` in the probe;
Zephyr: `__atomic_store_n` in the init fn) before any callback.

## 4. Diagnostics (C02)

`AtomicDiagnostic` (`BridgeError::AtomicValidation`, `thiserror`, each message leads with
its `ATOMIC-ERR-*` ID):

| ID | Trigger | Built |
|---|---|---|
| `ATOMIC-ERR-ELEMENT-TYPE` | `Atomic<T>`, `T` outside `{u32,i32,u64,i64}`+ptr | ✅ (in `materialise_atomics`) |
| `ATOMIC-ERR-MISSING-ORDERING` | a `#[rseq::intrinsic("atomic_*")]` facade whose signature omits the `Ordering` at the canonical position (D15 facade-signature residual) | ✅ |
| `ATOMIC-ERR-BIT-TEST-TYPE` | `bit_test_*` on non-unsigned `T`, or a const out-of-range bit | ✅ |
| `ATOMIC-ERR-DEGENERATE-ACQREL` | any invalid ordering for an op per C01 (generalized, O3) | ✅ |
| `ATOMIC-ERR-FENCE-RELAXED` | `atomic::fence(Relaxed)` | ✅ |
| `ATOMIC-ERR-ORDERING-NONCONST` | a non-compile-time-constant `Ordering` arg (D14, O4) | ✅ |
| `ATOMIC-ERR-MMIO-FENCE-COLLAPSE` | an MMIO-ordering op smuggled onto the atomic surface | **designed-in** (D02/O1; no live trigger until register-access lands `io_fence`) |
| `ATOMIC-ERR-NESTED-ATOMIC` | an atomic op whose receiver is not a DIRECT driver-struct field (nested struct field / array index) — S01 (O6) | ✅ (frontend, target-agnostic) |
| `ATOMIC-ERR-ORDERING-ENUM` | a recognized facade whose `Ordering` enum is not exactly `Relaxed, Acquire, Release, AcqRel, SeqCst` in that order (the const-discriminant ABI is positional) — O7 | ✅ (facade-signature) |

The canonical-facade call-site missing-ordering is type-system-subsumed (a rustc error
before the bridge runs — D15); the reachable check is the facade-signature obligation.
The `NESTED-ATOMIC` check (`receiver_borrowed_place` + field-step count) runs in the
shared frontend pass so the generic-c oracle and the OS backends reject identically — the
frontend resolver mirrors the backends' `resolve_receiver_src` (it follows `Rvalue::Use`
chains with the same depth/ambiguity rules) so it resolves exactly the receiver shapes the
oracle would emit. The `ORDERING-ENUM` check makes the positional const-`Ordering` read
sound, and runs at EVERY ordering position (not just the first) — `compare_exchange`
declares two (success, failure), so a mixed facade with a canonical success enum and a
same-named reordered failure enum (e.g. `core::sync::atomic::Ordering`, which swaps
Release/Acquire) is rejected rather than positionally misreading the failure ordering.

## 5. Host harness (C05 — the emission/value oracle)

A self-contained static `ATOMIC_HELPERS` prelude — the `regseq_mmio_*` DEFINE model (the
hooks are defined inline, NOT harness-linked like the timed-delay `DELAY_HOOKS`), emitted
for any TU that uses an atomic OP (gated on `model_has_atomic_op` — a field-less
`atomic::fence`-only TU still emits `regseq_atomic_fence` and so needs the prelude; #1):
a cell table (sized per-TU to the `Atomic`-field count, `max(_, 1)` so a fence-only TU is
not a zero-length array — no fixed cap), the six hooks applying value
semantics + printing an `ATM <op>` line (the ordering NAME printed, so tag preservation
is observable), and a runtime `bit < width` assert. The printed trace IS the behavioral
oracle (value correctness, ordering-tag preservation, bit-test prior bit, CAS commit/
contention branch). NOT weak-memory soundness (I06).

## As-built (shipped on `construct/atomic-memory-access-and-ordering`)

### A. Milestones (PLAN T01–T19)

- **M1 (T01–T06):** model IR + recognition + the `bridge::atomic` field-recording + the
  const-`Ordering` reaching-def read + generic-c `emit_atomic` (load/store/bit) + the
  always-on host prelude + the `atomic-m1` six-stage fixture.
- **M2 (T07–T09):** `AtomicDiagnostic` + the per-op ordering matrix + const reject +
  element/bit-test/facade-signature checks; `bridge_atomic_legality` (12 cases).
- **M3 (T10–T13):** generic-c `fetch_*`/`swap`/`compare_exchange`(`Result<T,T>`)/`fence`
  + the `atomic-rmw` / `atomic-cas` / `atomic-fence` oracle fixtures.
- **M4 (T14–T17):** linux_c + zephyr_c per-target lowering (storage override + ops +
  init), `Atomic` driver fields routed past the device-lifecycle D3 classifier;
  `atomic_conformance_e2e` (string-assertion both targets).
- **M5 (T18–T19):** the `atomic-cov` coverage fixture (i32 wrap, u64/i64 width) + the
  per-target i32/i64/pointer storage assertions; the designed-in MMIO-FENCE-COLLAPSE
  assertion.

### B. Honest scope & deviations (Codex-reviewed)

The Codex M4 **foundation review** corrected the design-time GRILL/eval lowering rows;
the as-built follows the corrected mapping (recorded here as taxonomy/spec feedback):

1. **Bit ops via `atomic_fetch_or`/`_and`**, NOT `unsigned long` + `test_and_set_bit`
   (GRILL C07). The latter needs an `atomic_long_t`→`unsigned long*` aliasing cast and a
   per-field bit-tested-vs-not storage split; `fetch_or`/`_and` is idiomatic, width-clean,
   and makes the "value-op width invariant" automatic (storage is the exact width-`T`
   atomic type). Bit-op shift amounts are masked to width-1 (no signed-shift UB).
2. **Uniform storage by width** (`atomic_t`/`atomic64_t`/`atomic_long_t`; plain Zephyr
   cell) — supersedes the GRILL's bit-tested→`unsigned long` rule.
3. **Linux SeqCst load/store → `atomic_read_acquire`/`atomic_set_release`** (Linux has no
   seqcst non-RMW primitive; plain `atomic_read`/`atomic_set` would be a silent downgrade).
4. **Linux `fence` → `smp_mb()`** for every valid ordering (conservative full barrier;
   `smp_rmb`/`smp_wmb` are not faithful acquire/release fences).
5. **Zephyr via the GCC `__atomic_*` builtins** uniformly (width-parametric — no 32-bit
   `atomic_t` ceiling; native dual-ordering CAS; the exact `__ATOMIC_*` order per op).
6. **Pointer → `atomic_long_t` (Linux) / `uintptr_t` (Zephyr)**, never `atomic64_t`.
7. **CAS collapse = LUB(success, failure)** for the single Linux `try_cmpxchg` suffix
   (over-strengthens the success path, correctness-safe); Linux `try_cmpxchg` orders on
   success only — the failure ordering is best-effort, a documented per-target limitation.
8. **Atomic ops are restricted to DIRECT driver-struct fields (S01)** — a nested-atomic
   receiver is rejected loud (the per-target init seeds only the driver struct).
9. **Verification is string-assertion** for the per-target leg (the dev env has no
   `RSEQ_LINUX_KDIR` kernel tree nor a Zephyr/west workspace); the claim is faithful
   primitive selection, which the emitted-C assertions prove. The generic-c host oracle
   carries the behavioral (value/ordering-tag/branch) proof.
10. **Non-const `Ordering` rejected** (D14) — runtime orderings unsupported. Non-const
    bit indices are allowed (runtime-asserted at the host hook; shift UB-free per target).

### C. Cross-construct hand-offs

- **register-access (O1):** the baud-latch + `dma_wmb` MMIO ordering is register-access's
  deferred `io_fence`/`ordered!`; the `ATOMIC-ERR-MMIO-FENCE-COLLAPSE` trigger lands with
  that surface. `atomic::fence` is CPU-only and co-exists with `io_fence` (D02).
- **synchronization:** the flag-vs-lock edge is resolved on atomicity's side
  (`__E1000_DOWN` = atomicity flag; `__E1000_RESETTING` = synchronization lock).
- **DMA/data-path:** consumes `atomic::fence` for cross-CPU publication.

### D. Taxonomy feedback (O2/O3/O4 + the M4 corrections)

- O2: `fetch_modify`/`compare_exchange`/`fence` have no real-driver corpus consumer;
  covered by host-oracle + per-target conformance + constructed fixtures.
- O3: `ATOMIC-ERR-DEGENERATE-ACQREL` generalized to "invalid ordering for operation."
- O4: add `ATOMIC-ERR-ORDERING-NONCONST`; reclassify `ATOMIC-ERR-MISSING-ORDERING` as
  type-system-subsumed with a facade-signature residual.
- §B 1–7: the Linux/Zephyr atomic-API rows in the taxonomy organized-spec should adopt
  the `fetch_or`/`_and` bit idiom + uniform width-keyed storage + the SeqCst/fence
  collapses + the `__atomic` Zephyr leg.
- O5 (naming/scope, surfaced by the D02 fence-boundary call; recorded, NOT applied to
  the frozen pages): assigning device-access ordering (`io_fence`/`ordered!`, §C O1) to
  **register-access** makes that construct an access-*and-ordering* construct — the exact
  structural parallel of `atomic-memory-access-**and-ordering**`. The name `register-access`
  understates this; the precise label would be **`register-access-and-ordering`**. NOT
  `device-access` — that would collide with `dma-management` / `device-interrupt-setup-and-handling`
  / `bus-resource-management` (the name's "register" is load-bearing: the construct abstracts
  MMIO *and* PIO register access, not all device interaction). Deciding principle (consistent
  across the three "ordering" needs): ordering lives with the construct that models the
  accesses being ordered — CPU↔CPU memory → atomicity; context↔context exclusion →
  synchronization; CPU↔device accesses → register-access. **Mirror this note (and the O1
  obligation) into register-access's own surface-convergence when its `io_fence`/`ordered!`
  surface is built**, so a register-access reader finds it there.
- O6 (new diagnostic, surfaced by a post-completion review): `ATOMIC-ERR-NESTED-ATOMIC` —
  an atomic op's receiver must be a DIRECT driver-struct field (S01). Enforced in the
  shared frontend pass so the generic-c oracle and the OS backends reject identically
  (the oracle must not accept what the backends refuse).
- O7 (new diagnostic, same review): `ATOMIC-ERR-ORDERING-ENUM` — the `Ordering` enum a
  facade uses must declare exactly `Relaxed, Acquire, Release, AcqRel, SeqCst` in that
  order, because the compile-time-constant `Ordering` is read by POSITIONAL discriminant
  (the C05 ABI). A same-named reordered enum (notably `core::sync::atomic::Ordering`, which
  swaps Release/Acquire) would otherwise silently remap orderings — a silent miscompile.
  The check runs at EVERY ordering position, so `compare_exchange`'s failure ordering (a
  second position) is covered as well as the success ordering. (The taxonomy could instead
  mandate the canonical variant order in the surface spec.)
