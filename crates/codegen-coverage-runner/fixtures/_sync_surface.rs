// Synchronization AUTHOR surface — shared across the `sync-*` fixtures.
//
// The first-slice author surface (GRILL C01): the four lock/signal kinds and
// their RAII `Guard`. The author declares a lock by placing a typed field on the
// `#[rseq::driver]` struct (the field's *type is the kind* — C01/D01, no separate
// "kind" attribute) and operates it through the wrapper methods:
//   * `Mutex<T>` / `SpinLock<T>` / `IrqSpinLock<T>` : `lock() -> Guard` (release
//     on guard drop, or explicit `guard_drop_early(self)`; NO standalone
//     `unlock` — GRILL I02);
//   * `Completion` (no payload): `wait(Duration) -> Result<(), RegseqError>` /
//     `signal()` / `reset()`, with decl-time `#[rseq::sync(cardinality =
//     SingleWaiter)]`.
//
// Recognition keys: the kind is detected by the field's wrapper type name
// (matching the name-based `Mmio`/`Interrupt` anchors); `.lock()` /
// `guard_drop_early` / `wait`/`signal`/`reset` are recognized as sync ops on a
// recognized sync-resource receiver (the recognizer anchors on the marked
// field). Sync ops are wrapper methods, NOT intrinsics — there is NO
// `#[rseq::intrinsic]` here and NO new `IntrinsicKind` (GRILL C04 / design §6).
//
// FIRST SLICE is acquire/release + signal only: the guard is OPAQUE (no
// `Deref`/`DerefMut`), so the protected `T` cannot be read through it yet —
// keeping the slice payload-free *by the type system*. The guard's per-OS
// hidden state (Linux `flags` / Zephyr `key`) is backend codegen, not a surface
// concern; payload access (`Deref`) lands with the per-OS payload lowering.
//
// Declared inline per fixture (the CR-2 discipline); each fixture pulls it in
// with `include!("../_sync_surface.rs");`. A fixture that uses `Completion::wait`
// also pulls in the lifecycle surface (for `RegseqError`). The including
// `input.rs` owns the crate attrs (`#![feature(register_tool)]`,
// `#![register_tool(rseq)]`).

use core::cell::UnsafeCell;

// -- Duration ------------------------------------------------------------------
// The opaque nanosecond value type `Completion::wait` consumes (the shared
// timed-delay framework type — GRILL reuses `FrameworkType::Duration`). A sync
// fixture that uses `Completion::wait` must NOT also include the delay surface
// (which declares its own `Duration`).
#[rseq::framework_type("duration")]
#[derive(Clone, Copy)]
pub struct Duration {
    #[allow(dead_code)]
    ns: u64,
}

impl Duration {
    pub const fn from_us(us: u64) -> Self {
        Self { ns: us * 1_000 }
    }
    pub const fn from_ms(ms: u64) -> Self {
        Self { ns: ms * 1_000_000 }
    }
}

// -- the lock kinds ------------------------------------------------------------
// Three spinning/sleeping mutual-exclusion kinds, each detected by its type
// name. They share one RAII `Guard`; the backend recovers the kind (and so the
// per-OS primitive) from the recognized field's `SyncResource`, not from the
// guard type.
//
//   Mutex<T>       — sleeping mutual exclusion (may_sleep = Y, irq_safe = N)
//   SpinLock<T>    — spinning, preemption-off (may_sleep = N, irq_safe = N)
//   IrqSpinLock<T> — spinning + IRQ save/restore (may_sleep = N, irq_safe = Y)

pub struct Mutex<T> {
    data: UnsafeCell<T>,
}
unsafe impl<T: Send> Sync for Mutex<T> {}
impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            data: UnsafeCell::new(value),
        }
    }
    /// Acquire (sleeping). Recognized as a `LockAcquire` op; the guard releases.
    pub fn lock(&self) -> Guard<'_, T> {
        Guard { data: &self.data }
    }
}

pub struct SpinLock<T> {
    data: UnsafeCell<T>,
}
unsafe impl<T: Send> Sync for SpinLock<T> {}
impl<T> SpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            data: UnsafeCell::new(value),
        }
    }
    /// Acquire (spinning, thread-only). Recognized as a `LockAcquire` op.
    pub fn lock(&self) -> Guard<'_, T> {
        Guard { data: &self.data }
    }
}

pub struct IrqSpinLock<T> {
    data: UnsafeCell<T>,
}
unsafe impl<T: Send> Sync for IrqSpinLock<T> {}
impl<T> IrqSpinLock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            data: UnsafeCell::new(value),
        }
    }
    /// Acquire (spinning + IRQ save/restore; ISR-safe). Recognized as a
    /// `LockAcquire` op; the witness kind (E1000 `stats_lock`, PL011 seam).
    pub fn lock(&self) -> Guard<'_, T> {
        Guard { data: &self.data }
    }
}

// -- Guard ---------------------------------------------------------------------
// Kind-agnostic, move-only, OPAQUE RAII release handle (no `Copy`/`Clone`, no
// `Deref`/`DerefMut`): dropping it releases the lock, or `guard_drop_early(self)`
// releases it early (consume by value — post-consume use fails the move check,
// no standalone `unlock`). It borrows the protected cell so the borrow cannot
// outlive the critical section; the kind/primitive is recovered by the backend
// from the recognized acquire's field.
pub struct Guard<'a, T> {
    data: &'a UnsafeCell<T>,
}

impl<'a, T> Guard<'a, T> {
    /// Release the lock early (before scope end), consuming the guard.
    /// Recognized as the release site for this guard's acquire.
    pub fn guard_drop_early(self) {
        // Consume by value; the recognizer keys on this call as the release.
    }

    // Keep the `data` field live without exposing payload access.
    #[allow(dead_code)]
    fn _anchor(&self) -> *const UnsafeCell<T> {
        self.data as *const _
    }
}

impl<'a, T> Drop for Guard<'a, T> {
    fn drop(&mut self) {
        // Release-on-drop. Spec-side inert: the recognizer keys on the guard's
        // drop-glue `Call`, and the backend emits the per-OS release there.
    }
}

// -- Completion ----------------------------------------------------------------
// One-shot signaling (no payload `T`). `wait` is a sleeping op (thread context
// only); `signal` is ISR-safe; the decl-time `#[rseq::sync(cardinality =
// SingleWaiter)]` selects the per-OS signal primitive at `init_primitives` time.
// `wait` returns `Result<(), RegseqError>` — `Err` is a defined timeout outcome,
// not a fault. `RegseqError` comes from the included lifecycle surface.
// NOTE: `Completion::new` is concrete (non-generic), so the backend lowers it as
// a root; its body must therefore avoid any unlowered extern (e.g.
// `UnsafeCell::new`). A plain inert field suffices — the facade is spec-side
// only (the lowering emits the per-OS primitive). (The lock kinds keep
// `UnsafeCell<T>` because their `new` is generic — lowered only if a spec calls
// it, which the corpus never does.)
pub struct Completion {
    #[allow(dead_code)]
    _private: u32,
}
impl Completion {
    pub const fn new() -> Self {
        Self { _private: 0 }
    }
    /// Wait for the one-shot signal up to `timeout` (sleeping; thread-only).
    pub fn wait(&self, _timeout: Duration) -> Result<(), RegseqError> {
        Ok(())
    }
    /// Signal the completion (ISR-safe on all targets).
    pub fn signal(&self) {}
    /// Re-arm the one-shot completion.
    pub fn reset(&self) {}
}
