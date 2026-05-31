#![no_std]
#![feature(core_intrinsics)]

// B6 panic -- TM-4 (the Unreachable terminator). `core::intrinsics::unreachable()`
// is an intrinsic lowered to a MIR `Unreachable` terminator in place (not a Call
// to a separate function, so there is no undefined reference at link). The Phase-1
// host panic policy lowers `Unreachable` to a loud trap -- `regseq_unreachable()`,
// which calls `regseq_panic("internal error: entered unreachable code")` and
// aborts -- rather than a silent `__builtin_unreachable()`, so the oracle observes
// it. `expect = "panic"` (see manifest); expected.stderr carries the message.

#[no_mangle]
pub extern "C" fn regseq_main() {
    // The unsafe promise is deliberately violated at runtime: reaching the
    // intrinsic exercises the host trap. MIR: bb0 -> Unreachable (TM-4).
    unsafe { core::intrinsics::unreachable() }
}
