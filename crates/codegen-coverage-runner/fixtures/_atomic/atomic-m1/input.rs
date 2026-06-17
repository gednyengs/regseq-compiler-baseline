#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// Atomic-memory-access-and-ordering (atomicity construct) host behavioral
// oracle — the M1 vertical slice (T06). A flat routine drives an `Atomic<u32>`
// field through the M1 op slice (`store`/`load`/`bit_test_set`/`bit_test_clear`)
// under explicit compile-time-constant `Ordering`s; the always-on
// `regseq_atomic_*` host hooks apply value semantics to a cell table and print
// an `ATM` trace, so value correctness, ordering-tag preservation, and the
// bit-test prior bit are all observable. A second field with
// `#[rseq::atomic(initial = 7)]` proves the field-init (R10/S10) seeds its cell
// before `main`. NOT a weak-memory-soundness model (I06).

// -- regseq::atomic facade (inlined per fixture; identity is the `#[rseq::*]`
//    attributes, not a facade crate — mirroring how the delay/diag fixtures
//    inline their surface). ---------------------------------------------------
#[derive(Clone, Copy)]
pub enum Ordering {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

pub struct Atomic<T> {
    // The host oracle keeps the live value in its cell table (keyed by the
    // field's cell id), so this in-struct value is only the constructor seed.
    value: T,
}

impl<T: Copy> Atomic<T> {
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
    pub fn store(&self, _value: T, _order: Ordering) {}

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

// Driver-local atomic state (zero L2, no lifecycle — D11). `flags` default-inits
// (no `#[rseq::atomic]` attribute → cell 0 = 0); `counter` carries an explicit
// initial (cell 1 = 7). Field order fixes the cell ids (FieldPath order, C07).
struct DriverState {
    flags: Atomic<u32>,
    #[rseq::atomic(initial = 7)]
    counter: Atomic<u32>,
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let st = DriverState {
        flags: Atomic::new(0),
        counter: Atomic::new(7),
    };

    // Field-init (R10/S10): the cell-init constructor seeded counter's cell to
    // 7, so this first load — before any store — observes 7.
    let _c0 = st.counter.load(Ordering::Acquire);

    // store/load round-trip with explicit orderings (value correctness +
    // ordering-tag preservation).
    st.flags.store(1, Ordering::Release);
    let _v = st.flags.load(Ordering::Acquire);

    // bit_test_set then bit_test_clear on the same bit: the prior bit is
    // observable (0 on the set, then 1 on the clear).
    let _p_set = st.flags.bit_test_set(3, Ordering::SeqCst);
    let _p_clear = st.flags.bit_test_clear(3, Ordering::SeqCst);
}
