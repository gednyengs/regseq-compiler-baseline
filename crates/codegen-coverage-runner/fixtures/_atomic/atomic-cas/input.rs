#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// Atomicity construct M3/T12 host oracle: `compare_exchange` (strong, dual
// success/failure orderings, `Result<T, T>` — I07/C04). The host CAS hook prints
// the outcome (Ok=commit / Err=contention) + the observed prior; the fixture
// MATCHES the returned `Result` and runs a distinct op in each branch, so the
// correctness of the lowered `Result<T, T>` (the right branch with the right
// value) is observable in the trace as ordinary control flow (OR-7).

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
    #[rseq::intrinsic("atomic_store")]
    pub fn store(&self, _v: T, _o: Ordering) {}
    #[inline(never)]
    #[rseq::intrinsic("atomic_fetch_add")]
    pub fn fetch_add(&self, _v: T, _o: Ordering) -> T {
        self.value
    }
    #[inline(never)]
    #[rseq::intrinsic("atomic_compare_exchange")]
    pub fn compare_exchange(&self, _cur: T, _new: T, _s: Ordering, _f: Ordering) -> Result<T, T> {
        Ok(self.value)
    }
}

struct S {
    #[rseq::atomic(initial = 10)]
    cell: Atomic<u32>,
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let s = S {
        cell: Atomic::new(10),
    };
    // Commit: cell is 10 == expected 10, so swap to 20 and return Ok(10). The
    // Ok branch stores the prior (10), proving the Ok arm ran with prev=10.
    match s.cell.compare_exchange(10, 20, Ordering::AcqRel, Ordering::Acquire) {
        Ok(prev) => {
            s.cell.store(prev, Ordering::Release);
        }
        Err(cur) => {
            s.cell.store(cur, Ordering::Relaxed);
        }
    }
    // Contention: cell is 10 != expected 99, so return Err(10), cell unchanged.
    // The Err branch fetch_adds the observed (10), proving the Err arm ran.
    match s.cell.compare_exchange(99, 30, Ordering::SeqCst, Ordering::Relaxed) {
        Ok(prev) => {
            s.cell.store(prev, Ordering::Release);
        }
        Err(cur) => {
            let _ = s.cell.fetch_add(cur, Ordering::Relaxed);
        }
    }
}
