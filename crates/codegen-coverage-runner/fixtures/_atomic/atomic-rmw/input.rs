#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// Atomicity construct M3/T11 host oracle: the `fetch_*` / `swap` RMW family on
// an `Atomic<u32>`. Each op returns the PRIOR value; the `regseq_atomic_rmw`
// host hook applies the arithmetic/bitwise op to the cell and prints the op,
// the argument, and the prior value — so value correctness, the prior-value
// semantics, and ordering-tag preservation are all observable.

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
    #[rseq::intrinsic("atomic_fetch_add")]
    pub fn fetch_add(&self, _v: T, _o: Ordering) -> T {
        self.value
    }
    #[inline(never)]
    #[rseq::intrinsic("atomic_fetch_sub")]
    pub fn fetch_sub(&self, _v: T, _o: Ordering) -> T {
        self.value
    }
    #[inline(never)]
    #[rseq::intrinsic("atomic_fetch_or")]
    pub fn fetch_or(&self, _v: T, _o: Ordering) -> T {
        self.value
    }
    #[inline(never)]
    #[rseq::intrinsic("atomic_fetch_and")]
    pub fn fetch_and(&self, _v: T, _o: Ordering) -> T {
        self.value
    }
    #[inline(never)]
    #[rseq::intrinsic("atomic_fetch_xor")]
    pub fn fetch_xor(&self, _v: T, _o: Ordering) -> T {
        self.value
    }
    #[inline(never)]
    #[rseq::intrinsic("atomic_swap")]
    pub fn swap(&self, _v: T, _o: Ordering) -> T {
        self.value
    }
}

struct S {
    #[rseq::atomic(initial = 0)]
    counter: Atomic<u32>,
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let s = S {
        counter: Atomic::new(0),
    };
    // add 5 (prior 0 -> 5), add 3 (prior 5 -> 8), or 0xf0 (prior 8 -> 0xf8),
    // and 0x0f (prior 0xf8 -> 0x08), xor 0xff (prior 0x08 -> 0xf7),
    // swap 42 (prior 0xf7 -> 0x2a). Distinct orderings prove tag preservation.
    let _ = s.counter.fetch_add(5, Ordering::Relaxed);
    let _ = s.counter.fetch_add(3, Ordering::AcqRel);
    let _ = s.counter.fetch_or(0xf0, Ordering::Release);
    let _ = s.counter.fetch_and(0x0f, Ordering::Acquire);
    let _ = s.counter.fetch_xor(0xff, Ordering::SeqCst);
    let _ = s.counter.swap(42, Ordering::SeqCst);
}
