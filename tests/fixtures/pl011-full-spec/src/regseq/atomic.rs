//! Atomic memory access & ordering facade (atomicity construct).
//!
//! A typed driver-local `Atomic<T>` + lock-free operations, each carrying an
//! explicit **compile-time-constant** memory `Ordering`, plus a standalone CPU
//! memory `fence`. The compiler recognizes the `#[rseq::intrinsic("atomic_*")]`
//! method tags + `atomic::fence` by path-match and lowers each to the target's
//! kernel-atomic primitives (CPU memory ordering ONLY — never MMIO). The bodies
//! are compiler-managed; they never execute.

/// Memory ordering for an atomic operation. Every op takes a `Ordering` that
/// must be a compile-time constant (a literal `Ordering::X` or a `const`).
#[derive(Clone, Copy)]
pub enum Ordering {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

/// A typed driver-local atomic cell. `T ∈ {u32, i32, u64, i64}` plus permissive
/// pointer atomics; `bit_test_*` requires an unsigned `T`. Zero L2, no lifecycle.
pub struct Atomic<T> {
    value: T,
}

impl<T: Copy> Atomic<T> {
    /// Construct an atomic cell with an initial value. (A driver field may also
    /// carry `#[rseq::atomic(initial = <literal>)]` for compiler-seeded init.)
    pub const fn new(value: T) -> Self {
        Self { value }
    }

    #[inline(never)]
    #[rseq::intrinsic("atomic_load")]
    pub fn load(&self, _order: Ordering) -> T {
        self.value
    }

    #[inline(never)]
    #[rseq::intrinsic("atomic_store")]
    pub fn store(&self, _value: T, _order: Ordering) {
        let _ = &self.value;
    }

    #[inline(never)]
    #[rseq::intrinsic("atomic_fetch_add")]
    pub fn fetch_add(&self, _value: T, _order: Ordering) -> T {
        self.value
    }

    #[inline(never)]
    #[rseq::intrinsic("atomic_fetch_sub")]
    pub fn fetch_sub(&self, _value: T, _order: Ordering) -> T {
        self.value
    }

    #[inline(never)]
    #[rseq::intrinsic("atomic_fetch_or")]
    pub fn fetch_or(&self, _value: T, _order: Ordering) -> T {
        self.value
    }

    #[inline(never)]
    #[rseq::intrinsic("atomic_fetch_and")]
    pub fn fetch_and(&self, _value: T, _order: Ordering) -> T {
        self.value
    }

    #[inline(never)]
    #[rseq::intrinsic("atomic_fetch_xor")]
    pub fn fetch_xor(&self, _value: T, _order: Ordering) -> T {
        self.value
    }

    #[inline(never)]
    #[rseq::intrinsic("atomic_swap")]
    pub fn swap(&self, _value: T, _order: Ordering) -> T {
        self.value
    }

    #[inline(never)]
    #[rseq::intrinsic("atomic_compare_exchange")]
    pub fn compare_exchange(
        &self,
        _current: T,
        _new: T,
        _success: Ordering,
        _failure: Ordering,
    ) -> Result<T, T> {
        Ok(self.value)
    }

    #[inline(never)]
    #[rseq::intrinsic("atomic_bit_test_set")]
    pub fn bit_test_set(&self, _bit: u32, _order: Ordering) -> bool {
        false
    }

    #[inline(never)]
    #[rseq::intrinsic("atomic_bit_test_clear")]
    pub fn bit_test_clear(&self, _bit: u32, _order: Ordering) -> bool {
        false
    }
}

/// Standalone CPU memory fence (CPU ordering only; never MMIO). `Relaxed` has no
/// cross-CPU effect and is rejected.
#[inline(never)]
#[rseq::intrinsic("atomic_fence")]
pub fn fence(_order: Ordering) {
    unimplemented!("compiler-managed: atomic::fence")
}
