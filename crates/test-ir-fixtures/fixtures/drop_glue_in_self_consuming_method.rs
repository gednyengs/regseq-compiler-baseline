// regseq-driver-61 Test Plan B1 fixture: a self-consuming inherent
// method (`R::consume(self)`) on a `Drop`-implementing type. The
// fixture exercises the Drop-as-Call collapse at
// `crates/regseq-driver/src/bridge/body/terminator.rs:560+`. The
// `consume(self)` body keeps a `drop(_1) -> [return: bb_succ, unwind
// unreachable]` Drop terminator on the success path; the
// `panic=abort` cleanup-block pruning does NOT eliminate the
// success-path Drop, so the bridge takes the Drop-as-Call collapse
// path and interns a drop-glue `FnSig`
// (`unsafe fn(*mut R) -> ()`, `FnAbi::Rust`) via
// `lookup_or_create_direct_instance`. Pre-fix, that intern desyncs
// the `sig_c_abi_lowerings` parallel side table and trips the
// `permute_index_vec_with_keep` length check in
// `RegseqModel::remove_fn_sigs_and_remap`.
//
// The fixture avoids `Into::into(self)` because of the blanket
// `impl<T, U> Into<U> for T where U: From<T>` in `core::convert`:
// providing a parallel `impl Into<u32> for R` produces an
// `E0283` ambiguity at the call site. An inherent `consume(self)`
// method has the same Drop-on-success-path MIR shape without the
// ambiguity.

#![no_std]

pub struct R(u32);

impl Drop for R {
    fn drop(&mut self) {}
}

impl R {
    pub fn new(off: u32) -> R {
        R(off)
    }

    pub fn consume(self) -> u32 {
        self.0
    }
}

pub fn entry() -> u32 {
    R::new(0).consume() + R::new(1).consume()
}
