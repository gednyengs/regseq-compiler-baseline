#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// Atomicity construct M3/T13 host oracle: the standalone `regseq::atomic::fence`
// (CPU memory ordering ONLY — never MMIO, D02/I03). The fence is interleaved
// with atomic store/load ops; the `ATM fence` line's position in the trace,
// relative to the surrounding store/load lines, fixes the fence PLACEMENT and
// its ordering tag (a Relaxed fence is rejected at the frontend, so only the
// effectful orderings appear here).

#[derive(Clone, Copy)]
pub enum Ordering {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

pub struct Atomic<T> {
    value: T,
}

impl<T: Copy> Atomic<T> {
    pub const fn new(value: T) -> Self {
        Self { value }
    }
    #[inline(never)]
    #[rseq::intrinsic("atomic_load")]
    pub fn load(&self, _o: Ordering) -> T {
        self.value
    }
    #[inline(never)]
    #[rseq::intrinsic("atomic_store")]
    pub fn store(&self, _v: T, _o: Ordering) {}
}

pub mod atomic {
    use super::Ordering;
    #[inline(never)]
    #[rseq::intrinsic("atomic_fence")]
    pub fn fence(_o: Ordering) {}
}

struct S {
    #[rseq::atomic(initial = 0)]
    flag: Atomic<u32>,
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let s = S {
        flag: Atomic::new(0),
    };
    // publish: write the flag, then a Release fence, then read it back; a final
    // SeqCst fence. The fence lines sit between the store and the load, and
    // after the load — pinning placement + ordering tag.
    s.flag.store(1, Ordering::Relaxed);
    atomic::fence(Ordering::Release);
    let _ = s.flag.load(Ordering::Relaxed);
    atomic::fence(Ordering::SeqCst);
}
