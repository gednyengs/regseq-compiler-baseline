# Expectations on stage 14 (incoming)

Stage 14 covers domain-specific extensions: intrinsics, ordered
regions, register access, ISR classification, error-cleanup
analysis, callback context inference.

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

## From stage 1

- **Pin the intrinsic vocabulary (Q7).** Per Q7, the bridge
  identifies domain intrinsics by the `NAME` argument of
  `#[rseq::intrinsic("NAME")]` on the callee, not by the
  callee's path. Names form a closed vocabulary maintained in
  the bridge; unknown names are rejected. Stage 14 ships the
  closed vocabulary list and the bridge-side recognition.

- **Single-level IR (Q3).** Domain extensions live as additional
  variants or tags on existing IR variants -- not as a separate
  IR layer. For example, an ISR subkind of `Function` can be
  added to `InstanceKind::Function` (e.g., as an `is_isr: bool`
  field, or as a new `InstanceKind::Isr` variant). Stage 1's
  closed six-variant `InstanceKind` is the closed baseline; any
  stage-14 extension appends to it (with a corresponding
  append-only entry in `regseq_instance_kind_rank`).

  **`Ty` is closed at stage 3.** Per `stage-3/spec-snapshot.md`
  Section 1.1's "Forward reference: `Ty` enum closure (Q3)",
  the 19-variant `Ty` enum is closed -- stage 14 may **not**
  add new `Ty` variants. Domain extensions land on
  `InstanceKind` (per the bullet above), on `Item` flags
  (mirroring D1.5's lang-item flags `is_phantom_data` /
  `is_unsafe_cell` / `is_manually_drop`), or on tags attached
  to existing IR nodes -- never as new type shapes. The set
  of type shapes RegseqModel can represent is fixed at
  stage 3.

- **Promote items into the `regseq` facade (Q6).** The `regseq`
  crate (`crates/regseq/`) is currently empty per Q6's lazy
  promotion plan. Stage 14 promotes items from each spec's
  `crate::regseq` submodule into the facade incrementally as
  redesign work touches them. The crate's symmetric
  `register_tool` declarations and `[package.metadata.regseq]`
  marker are already in place.

- **Fill `pretty.rs::render_instance` stub.** Stage-1 stub at
  `crates/regseq-model/src/pretty.rs` panics with
  `unimplemented!("stage 14 fills render_instance")`. Stage 14
  replaces the body with a renderer producing
  `Item::<args>` per the spec sigil for `InstanceId`.

- **Domain-specific IDs (e.g., `RegisterId`, `IsrId`).** Per the
  spec's "IDs added at later stages" note, stage 14 may
  introduce additional typed IDs. They follow the same
  conventions (`newtype_index!` macro, niche-encoded u32, hand-
  written serde, single namespace per kind).

## Bridge concerns

- **Domain attribute parsing.** `#[rseq::*]` attributes become
  structured `DomainAttr` on `Item` / `Instance` per the
  C-semantics methodology rule (don't pass annotations through
  as opaque source-side metadata; structure them per the
  consumer's needs).

- **Recognition surface.** Bridge code identifies intrinsics by
  attribute-name lookup against the closed vocabulary. Mirror
  the "no defensive fallbacks" rule: unknown names are
  rejected.

## From stage 3

- [ ] **MMIO intrinsic vocabulary entries** (from stage 3 D9).

  *Stage-3 commitment.* D9 locks volatility on the intrinsic
  `InstanceId` (no `Ty` bit, no Statement flag); MMIO access in
  the regseq DSL flows through `#[rseq::intrinsic("mmio_*")]`
  exclusively. See `stage-3/spec-snapshot.md` Section 1.13 (D9 audit
  shape) and Section 4.11 (worked example).

  *Stage-13 deliverable.* Add the MMIO intrinsic family to the
  closed vocabulary list. Concrete names from the pl011 fixture
  (`tests/fixtures/pl011-full-spec/src/regseq/mmio.rs:35-71`):
  - `mmio_read`
  - `mmio_write`
  - `mmio_read_field`
  - `mmio_write_field`
  Stage-13 codegen for these `InstanceId`s emits volatile-marked
  C accesses (`*(volatile uint32_t*)addr` and the write
  counterpart). The volatility is a property of the InstanceId's
  emission, not an IR-level flag.

- [ ] **Bridge-side rejection of `core::ptr::{read,write}_volatile`**
  (from stage 3 D9).

  Enforce D9's Call-translation-phase rejection: raw
  `core::ptr::read_volatile` and `core::ptr::write_volatile`
  calls are out-of-DSL-scope and must be rejected at the bridge
  with a diagnostic pointing to the `#[rseq::intrinsic("mmio_*")]`
  API. Same posture as D1.4 Category A rejections, applied to
  function calls instead of types.

- [ ] **Atomic intrinsic vocabulary entries + ordering encoding
  choice** (from stage 3 D10).

  *Stage-3 commitment.* D10 locks atomicity on the intrinsic
  `InstanceId` (no `Ty` bit, no Statement flag, no
  `is_atomic` flag on `Item`); atomic types are ordinary
  `Ty::Struct` instances with the inner `UnsafeCell<T>`
  carrying the `is_unsafe_cell` flag (D1.5). The regseq DSL
  provides its own `Atomic<T>` facade with
  `#[rseq::intrinsic("atomic_*")]`-flagged methods. See
  `stage-3/spec-snapshot.md` Section 1.14 (D10 audit shape) and
  Section 4.12 (worked example).

  *Stage-13 deliverables.*

  A. *Atomic intrinsic vocabulary entries.* Add the atomic
  operation family to the closed vocabulary. Initial set
  (illustrative; stage 14 finalises the precise names):
  - `atomic_load`, `atomic_store`
  - `atomic_swap`
  - `atomic_compare_exchange`, `atomic_compare_exchange_weak`
  - `atomic_fetch_add`, `atomic_fetch_sub`
  - `atomic_fetch_and`, `atomic_fetch_or`, `atomic_fetch_xor`

  B. *Ordering encoding choice.* Memory ordering
  (`Relaxed`/`Acquire`/`Release`/`AcqRel`/`SeqCst`) survives in
  the IR at the operation/InstanceId level. Stage 14 picks
  between two encodings:
  - *Operand encoding*: a single `atomic_<op>(_, order)`
    intrinsic per op, with ordering as a value-level operand
    (e.g., `Atomic<T>::load(&self, order: Ordering)`); the
    bridge requires the ordering operand to be const-evaluable
    pre-bridge.
  - *Per-(op × ordering) InstanceId encoding*: distinct
    intrinsic names per ordering (`atomic_load_relaxed`,
    `atomic_load_acquire`, ...); the bridge selects the
    InstanceId by inspecting the const-evaluated ordering at
    the source.

  C. *Per-target codegen mapping.* Stage 14 also commits
  per-target codegen for each
  `(operation, ordering)` pair:
  - *Linux*: prefer the `atomic_*` family suffix mapping
    (`_relaxed`/`_acquire`/`_release` plus unsuffixed for
    SeqCst); AcqRel must be conservatively strengthened to
    unsuffixed (Linux has no dedicated AcqRel suffix). Non-RMW
    `_relaxed`/`_acquire`/`_release` loads/stores on plain
    primitive types use the `READ_ONCE`/`WRITE_ONCE`/
    `smp_load_acquire`/`smp_store_release` family
    (`include/asm-generic/rwonce.h`,
    `include/asm-generic/barrier.h`); `Documentation/atomic_t.txt`
    is the canonical reference.
  - *Zephyr*: codegen falls through to GCC `__atomic_*`
    builtins to preserve any non-SeqCst ordering. The public
    Zephyr API (`include/zephyr/sys/atomic.h`) is SeqCst-only
    and would pessimise non-SeqCst orderings.

- [ ] **Bridge-side rejection of `core::sync::atomic` operational
  API** (from stage 3 D10).

  Enforce D10's two-phase rejection:

  *Type-translation rejection.* Reject the
  `core::sync::atomic::Atomic*` type family at the point the
  bridge translates an atomic-typed Rust binding. Members (per
  `library/core/src/sync/atomic.rs` in the pinned toolchain):
  `AtomicU8`, `AtomicI8`, `AtomicU16`, `AtomicI16`, `AtomicU32`,
  `AtomicI32`, `AtomicU64`, `AtomicI64`, `AtomicU128`,
  `AtomicI128` (cfg-gated on `target_has_atomic_load_store =
  "128"` per `library/core/src/sync/atomic.rs:3884-3920`;
  rejection scope is the family, not gated by the cfg),
  `AtomicUsize`, `AtomicIsize`, `AtomicBool`, `AtomicPtr<T>`,
  plus `core::sync::atomic::Atomic<T>` -- the unstable generic
  alias declared with `#[unstable(feature = "generic_atomic",
  issue = "130539")]` at
  `library/core/src/sync/atomic.rs:340-345`. Rejection scope
  is the family, not conditional on the unstable feature gate.

  *Naming clarification.* `core::sync::atomic::Atomic<T>` (the
  unstable generic alias just listed) and the regseq-DSL
  `regseq::Atomic<T>` facade are distinct types living in
  distinct namespaces. The rejection target is *the core
  alias*; driver code that refers to `regseq::Atomic<T>` via
  the regseq DSL imports is the accept path.

  *Call-translation rejection.* Reject calls to the methods on
  the above types (`.load()`, `.store()`, `.fetch_add()`,
  `.fetch_sub()`, `.fetch_and()`, `.fetch_or()`, `.fetch_xor()`,
  `.swap()`, `.compare_exchange()`, `.compare_exchange_weak()`,
  ...) and to the rustc-internal `core::intrinsics::atomic_*`
  family.

  Explicitly *not* in the rejection scope:
  `core::sync::atomic::Ordering` (the value-level enum -- a
  plain Rust enum that `regseq::Atomic<T>` may re-export or
  shadow with its own ordering enum, per stage-14's choice).

  *Memory barriers / fences (v1 policy: bridge rejects).*
  `core::sync::atomic::fence(Ordering)`,
  `core::sync::atomic::compiler_fence(Ordering)`, and related
  barrier/fence free functions are *not* atomic intrinsic
  vocabulary entries. v1 policy (per stage-3 D10 Section 1.14):
  the bridge **rejects** these calls at Call translation,
  mirroring D9/D10's other rejection patterns. Stage 14 enforces
  the rejection. Future admission via a barrier-specific
  intrinsic or a construct-level decision remains open per
  `docs/compiler-collateral/constructs/atomic-memory-access-and-ordering.md`.
  D10 adds no accepted vocabulary entry for these. Source-level
  use in driver code is governed by a separate mechanism
  (future construct decision or stage-14 unsupported-call
  policy), not by Q7 -- Q7's recognition mechanism applies to
  `#[rseq::intrinsic("NAME")]`-attributed callees, not to raw
  `core::*` free-function calls.

  All driver atomic access flows through `regseq::Atomic<T>`
  and its `#[rseq::intrinsic("atomic_*")]`-flagged methods.
  Same posture as D9's `*_volatile` rejection.

- [ ] **Bridge-side rejection of `core::ffi::c_void`** (from stage 3
  D16).

  *Stage-3 commitment.* D16 hardens D1.5's "recognition
  deferred" into "actively rejected pending recognition." The
  bridge rejects `core::ffi::c_void` at type translation with a
  diagnostic recommending the opaque-struct pattern. See
  `stage-3/spec-snapshot.md` Section 1.15 (D16 audit shape) and
  Section 4.15 (worked example).

  *Stage-13 deliverable.* Enforce the type-translation-phase
  rejection at the bridge:
  - Recognition: `tcx.is_lang_item(did, LangItem::CVoid)`
    (per D1.5's locked recognition-path note in
    `stage-3/spec-snapshot.md` Section 1.6 lines 402-406; not in
    `AdtFlags`).
  - Scope: the c_void type itself; pointers to it
    (`*mut c_void`, `*const c_void`) reject transitively when
    the inner type is encountered during pointer translation
    -- not via a separate raw-pointer rule.
  - Diagnostic: recommends the opaque-struct pattern
    (`#[repr(C)] struct Opaque { _private: [u8; 0] }`) per
    D1.4's existing precedent for opaque C-side handles.

  *Trigger conditions for re-opening D16 (deferred to
  post-v1).* A v1 driver fixture or stage-14 intrinsic surfaces
  a stable need for FFI-canonical pointer-to-c_void lowering
  -- specifically `*mut c_void` -> C `void *` and/or
  `*const c_void` -> C `const void *` (the two pointer forms
  have distinct C spellings per locked D7/D8's `Ty::Ptr(TyId,
  Mutability)` and Section 1.12's raw-pointer
  `const`-derivation rule). When that materialises, D16
  reopens to add an `is_c_void` flag on `Item` and lift the
  rejection.
