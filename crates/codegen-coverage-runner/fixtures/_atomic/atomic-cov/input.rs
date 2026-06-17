#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// Atomicity construct M5/T18 coverage host oracle: element-type / width /
// signedness across i32 (two's-complement wrap at 32-bit), u64 and i64 (64-bit
// width, not truncated to 32). The host cell applies width-T masking, so the
// trace pins the wrapping + width behavior (C07/I04). Pointer-atomic LP64
// storage selection is covered by the per-target conformance test (no host
// behavioral consumer — O2).

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
    #[inline(never)]
    #[rseq::intrinsic("atomic_fetch_add")]
    pub fn fetch_add(&self, _v: T, _o: Ordering) -> T {
        self.value
    }
}

struct S {
    si: Atomic<i32>,
    wu: Atomic<u64>,
    wi: Atomic<i64>,
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let s = S {
        si: Atomic::new(0),
        wu: Atomic::new(0),
        wi: Atomic::new(0),
    };
    // i32 two's-complement wrap at width 32: i32::MAX + 1 -> 0x80000000.
    s.si.store(0x7fffffff, Ordering::Relaxed);
    let _ = s.si.fetch_add(1, Ordering::Relaxed);
    let _ = s.si.load(Ordering::Relaxed);
    // u64 64-bit width: 2^32 must not truncate to 0.
    s.wu.store(0x1_0000_0000, Ordering::Relaxed);
    let _ = s.wu.load(Ordering::Relaxed);
    // i64 signed: -1 is the all-ones 64-bit pattern.
    s.wi.store(-1, Ordering::Relaxed);
    let _ = s.wi.load(Ordering::Relaxed);
}
