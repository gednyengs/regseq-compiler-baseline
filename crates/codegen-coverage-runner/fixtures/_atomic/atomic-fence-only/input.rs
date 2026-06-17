#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// Atomicity M6/#1 regression: a FENCE-ONLY translation unit — `atomic::fence`
// with NO `Atomic<T>` field. The fence is a free function, so the prelude must
// be gated on "uses an atomic op" (not "has an atomic field"); otherwise the
// emitted `regseq_atomic_fence(...)` references an undefined hook and the TU
// won't compile. Realistic: E1000 CPU-publish fences around non-atomic ring
// state (no Atomic<T> field).

#[derive(Clone, Copy)]
pub enum Ordering {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

pub mod atomic {
    use super::Ordering;
    #[inline(never)]
    #[rseq::intrinsic("atomic_fence")]
    pub fn fence(_o: Ordering) {}
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    atomic::fence(Ordering::Release);
    atomic::fence(Ordering::SeqCst);
}
