#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// B9 — closures: a closure that CAPTURES a value by reference and is CALLED.
// `let cap = 7; let c = |x| x + cap; c(2)` exercises:
//   * TY-15 (Closure type)            — the closure value `c` has `Ty::Closure`.
//   * AG-6 (AggregateKind::Closure)   — building `c` is a closure-captures
//                                       compound literal `(closure_..){ .cap = .. }`.
// Calling `c(2)` reaches the bridge-synthesized `Fn`-family shims. ALL THREE
// shims (`call` / `call_mut` / `call_once`) are reachable from the closure type
// and are emitted, so the fixture compiles only if the `call_once` shim's
// by-value receiver agrees with its body projection (regseq-driver-85): rustc
// reuses the closure's by-reference body for all three call kinds, so the bridge
// retypes the `call_once` receiver local to the by-value captures struct and
// strips the now-spurious leading `Deref` (`(*_1)._0` -> `_1._0`). The single
// `info!` line drives the computed result THROUGH the closure call and logs it
// (the dual oracle then proves "compiled + printed the right value").
// `// proves:` maps the line to the Doc-1 matrix row(s) it drives.

pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                U8(u8), U16(u16), U32(u32), U64(u64),
                I8(i8), I16(i16), I32(i32), I64(i64),
                Bool(bool), Str(&'a str),
            }
            impl From<u8> for LogArg<'_> { fn from(v: u8) -> Self { LogArg::U8(v) } }
            impl From<u16> for LogArg<'_> { fn from(v: u16) -> Self { LogArg::U16(v) } }
            impl From<u32> for LogArg<'_> { fn from(v: u32) -> Self { LogArg::U32(v) } }
            impl From<u64> for LogArg<'_> { fn from(v: u64) -> Self { LogArg::U64(v) } }
            impl From<i8> for LogArg<'_> { fn from(v: i8) -> Self { LogArg::I8(v) } }
            impl From<i16> for LogArg<'_> { fn from(v: i16) -> Self { LogArg::I16(v) } }
            impl From<i32> for LogArg<'_> { fn from(v: i32) -> Self { LogArg::I32(v) } }
            impl From<i64> for LogArg<'_> { fn from(v: i64) -> Self { LogArg::I64(v) } }
            impl From<bool> for LogArg<'_> { fn from(v: bool) -> Self { LogArg::Bool(v) } }
            impl<'a> From<&'a str> for LogArg<'a> { fn from(v: &'a str) -> Self { LogArg::Str(v) } }

            #[inline(never)] #[rseq::intrinsic("log-debug")]
            pub fn log_debug(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-warn")]
            pub fn log_warn(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-error")]
            pub fn log_error(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }
        macro_rules! debug { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_debug($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use debug;
        macro_rules! info { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_info($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use info;
        macro_rules! log_warn { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_warn($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use log_warn as warn;
        macro_rules! error { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_error($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use error;
    }
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    // --- TY-15 Closure + AG-6 AggregateKind::Closure: build a closure that
    // captures `cap` (by reference, since `c` is an `Fn`) and call it. The
    // closure value `c` has `Ty::Closure`; constructing it is the closure
    // captures-struct aggregate. The call reaches all three Fn-family shims. ---
    let cap: u32 = 7;
    let c = |x: u32| x + cap;
    regseq::log::info!("closure={}", c(2));   // 9  proves: TY-15 (Closure), AG-6 (AggregateKind::Closure)
}
