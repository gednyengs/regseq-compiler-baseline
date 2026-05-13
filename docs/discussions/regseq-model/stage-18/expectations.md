# Expectations on stage 18 (incoming)

Stage 18 covers the **atomic family**: `atomic_*` intrinsic
vocabulary entries, ordering encoding choice, per-target
Linux-backend codegen mapping, bridge-side rejection of
`core::sync::atomic` operational API, and memory-barrier /
fence policy.

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

**Provenance note.** All items below were originally filed
against the monolithic "Stage 14: Domain-specific
extensions" plan; the 2026-05-12 strategy revision
relocated them to this phase-2 stage. See
`../stage-14/expectations.md` for the foundation items that
stayed at stage 14 and the strategy-revision context.

## From stage 3 (relocated 2026-05-12)

- [ ] **Atomic intrinsic vocabulary entries + ordering
  encoding choice** (from stage 3 D10).

  *Stage-3 commitment.* D10 locks atomicity on the
  intrinsic `InstanceId` (no `Ty` bit, no Statement flag,
  no `is_atomic` flag on `Item`); atomic types are
  ordinary `Ty::Struct` instances with the inner
  `UnsafeCell<T>` carrying the `is_unsafe_cell` flag
  (D1.5). The regseq DSL provides its own `Atomic<T>`
  facade with `#[rseq::intrinsic("atomic_*")]`-flagged
  methods. See `../stage-3/spec-snapshot.md` Section 1.14
  (D10 audit shape) and Section 4.12 (worked example).

  *Stage-18 deliverables.*

  A. *Atomic intrinsic vocabulary entries.* Add the
  atomic operation family to the closed vocabulary
  maintained per stage-14's Q7 mechanism. Initial set
  (illustrative; stage 18 finalises the precise names):
  - `atomic_load`, `atomic_store`
  - `atomic_swap`
  - `atomic_compare_exchange`,
    `atomic_compare_exchange_weak`
  - `atomic_fetch_add`, `atomic_fetch_sub`
  - `atomic_fetch_and`, `atomic_fetch_or`,
    `atomic_fetch_xor`

  B. *Ordering encoding choice.* Memory ordering
  (`Relaxed`/`Acquire`/`Release`/`AcqRel`/`SeqCst`)
  survives in the IR at the operation/InstanceId level.
  Stage 18 picks between two encodings:
  - *Operand encoding*: a single `atomic_<op>(_, order)`
    intrinsic per op, with ordering as a value-level
    operand (e.g., `Atomic<T>::load(&self, order:
    Ordering)`); the bridge requires the ordering operand
    to be const-evaluable pre-bridge.
  - *Per-(op × ordering) InstanceId encoding*: distinct
    intrinsic names per ordering
    (`atomic_load_relaxed`, `atomic_load_acquire`, ...);
    the bridge selects the InstanceId by inspecting the
    const-evaluated ordering at the source.

  C. *Per-target codegen mapping.* Stage 18 also commits
  per-target codegen for each `(operation, ordering)`
  pair:
  - *Linux*: prefer the `atomic_*` family suffix mapping
    (`_relaxed`/`_acquire`/`_release` plus unsuffixed
    for SeqCst); AcqRel must be conservatively
    strengthened to unsuffixed (Linux has no dedicated
    AcqRel suffix). Non-RMW
    `_relaxed`/`_acquire`/`_release` loads/stores on
    plain primitive types use the
    `READ_ONCE`/`WRITE_ONCE`/
    `smp_load_acquire`/`smp_store_release` family
    (`include/asm-generic/rwonce.h`,
    `include/asm-generic/barrier.h`);
    `Documentation/atomic_t.txt` is the canonical
    reference.
  - *Zephyr*: codegen falls through to GCC `__atomic_*`
    builtins to preserve any non-SeqCst ordering. The
    public Zephyr API
    (`include/zephyr/sys/atomic.h`) is SeqCst-only and
    would pessimise non-SeqCst orderings. (Zephyr
    target lands when the Zephyr backend is added; not
    in stage-18 scope unless a Zephyr fixture surfaces
    before then.)

- [ ] **Bridge-side rejection of `core::sync::atomic`
  operational API** (from stage 3 D10).

  Enforce D10's two-phase rejection:

  *Type-translation rejection.* Reject the
  `core::sync::atomic::Atomic*` type family at the point
  the bridge translates an atomic-typed Rust binding.
  Members (per `library/core/src/sync/atomic.rs` in the
  pinned toolchain): `AtomicU8`, `AtomicI8`,
  `AtomicU16`, `AtomicI16`, `AtomicU32`, `AtomicI32`,
  `AtomicU64`, `AtomicI64`, `AtomicU128`, `AtomicI128`
  (cfg-gated on
  `target_has_atomic_load_store = "128"` per
  `library/core/src/sync/atomic.rs:3884-3920`;
  rejection scope is the family, not gated by the cfg),
  `AtomicUsize`, `AtomicIsize`, `AtomicBool`,
  `AtomicPtr<T>`, plus
  `core::sync::atomic::Atomic<T>` -- the unstable
  generic alias declared with `#[unstable(feature =
  "generic_atomic", issue = "130539")]` at
  `library/core/src/sync/atomic.rs:340-345`. Rejection
  scope is the family, not conditional on the unstable
  feature gate.

  *Naming clarification.*
  `core::sync::atomic::Atomic<T>` (the unstable generic
  alias just listed) and the regseq-DSL
  `regseq::Atomic<T>` facade are distinct types living
  in distinct namespaces. The rejection target is *the
  core alias*; driver code that refers to
  `regseq::Atomic<T>` via the regseq DSL imports is the
  accept path.

  *Call-translation rejection.* Reject calls to the
  methods on the above types (`.load()`, `.store()`,
  `.fetch_add()`, `.fetch_sub()`, `.fetch_and()`,
  `.fetch_or()`, `.fetch_xor()`, `.swap()`,
  `.compare_exchange()`, `.compare_exchange_weak()`,
  ...) and to the rustc-internal
  `core::intrinsics::atomic_*` family.

  Explicitly *not* in the rejection scope:
  `core::sync::atomic::Ordering` (the value-level enum
  -- a plain Rust enum that `regseq::Atomic<T>` may
  re-export or shadow with its own ordering enum, per
  stage-18's choice).

  *Memory barriers / fences (v1 policy: bridge
  rejects).* `core::sync::atomic::fence(Ordering)`,
  `core::sync::atomic::compiler_fence(Ordering)`, and
  related barrier/fence free functions are *not* atomic
  intrinsic vocabulary entries. v1 policy (per stage-3
  D10 Section 1.14): the bridge **rejects** these calls
  at Call translation, mirroring D9/D10's other
  rejection patterns. Stage 18 enforces the rejection.
  Future admission via a barrier-specific intrinsic or
  a construct-level decision remains open per
  `docs/compiler-collateral/constructs/atomic-memory-access-and-ordering.md`.
  D10 adds no accepted vocabulary entry for these.
  Source-level use in driver code is governed by a
  separate mechanism (future construct decision or
  stage-18 unsupported-call policy), not by Q7 -- Q7's
  recognition mechanism applies to
  `#[rseq::intrinsic("NAME")]`-attributed callees, not
  to raw `core::*` free-function calls.

  All driver atomic access flows through
  `regseq::Atomic<T>` and its
  `#[rseq::intrinsic("atomic_*")]`-flagged methods.
  Same posture as D9's `*_volatile` rejection.

## New (introduced by 2026-05-12 strategy revision)

- [ ] **`regseq::Atomic<T>` facade type specification.**
  Stage 18 specifies the regseq-provided `Atomic<T>`
  facade:
  - Rust shape: ordinary `Ty::Struct` instance per D10's
    architecture commitment; the inner `UnsafeCell<T>`
    carries the `is_unsafe_cell` flag (D1.5).
  - Method surface: `load(order: Ordering) -> T`,
    `store(value: T, order: Ordering)`, `swap`,
    `compare_exchange`, `compare_exchange_weak`,
    `fetch_add`, ..., each flagged with the
    `#[rseq::intrinsic("atomic_*")]` attribute per Q7.
  - Bridge translation: `Atomic<T>` types translate as
    ordinary `Ty::Struct`; method calls translate as
    `TerminatorKind::Call` with the callee Instance
    resolved through Q7's intrinsic-recognition path; no
    `Ty` extensions needed.

- [ ] **`regseq::Ordering` enum specification.** Stage
  18 specifies the regseq-provided memory-ordering enum:
  - Whether to re-export `core::sync::atomic::Ordering`
    (since it is explicitly NOT in the rejection scope
    per D10) or define a fresh
    `regseq::sync::atomic::Ordering` for cleaner
    namespace control.
  - Either way: closed 5-variant enum (`Relaxed`,
    `Acquire`, `Release`, `AcqRel`, `SeqCst`);
    `repr(int)` per `spec.md` §13.2 C-ABI-admissibility
    rule.
  - Composition with the ordering-encoding choice from
    Deliverable B above.

## Cross-refs

- `../stage-14/expectations.md` -- foundation items (Q7
  intrinsic recognition mechanism + DomainAttr
  sibling-table architecture) that this stage builds on.
- `../stage-15/expectations.md` -- `Lifecycle` trait +
  driver-struct discovery + DT-binding + `RegseqError`
  -- drivers using atomics compose with these.
- `../stage-3/spec-snapshot.md` Section 1.14 (D10) --
  atomicity-on-InstanceId commitment.
- `docs/compiler-collateral/constructs/atomic-memory-access-and-ordering.md`
  -- fences / barriers construct decision (post-v1).
