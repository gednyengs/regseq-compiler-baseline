#![no_std]
#![feature(core_intrinsics)]

// B6 panic -- TY-7 (the `Never` type). `diverge` returns `!`, so its return type
// lowers to the C `_Noreturn void` spelling (visible in golden.c; the snapshot
// oracle proves it). Its body diverges through `core::intrinsics::unreachable()`,
// so the call from `regseq_main` actually traps at runtime: the host policy lowers
// the body's Unreachable terminator to `regseq_unreachable()` -> abort. The run
// thus proves the divergence, not merely the type spelling. `expect = "panic"`.

// A diverging function: the `-> !` return is the Never type (TY-7). `#[inline(never)]`
// keeps it a distinct emitted instance so its `_Noreturn void` prototype appears in
// golden.c (the bridge runs at mir_opt_level=0, but the attribute documents intent).
#[inline(never)]
fn diverge() -> ! {
    unsafe { core::intrinsics::unreachable() }
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    // A diverging call (Never return -> the call terminator has no successor).
    diverge();
}
