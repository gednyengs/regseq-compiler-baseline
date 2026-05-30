#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// B7 — function signatures (C ABI): the `extern "C"` boundary with Direct-shape
// (natural-C-repr) positions. add_c below is defined in-fixture and CALLED from
// regseq_main; the logged result proves the call lowered + executed through the
// C-ABI boundary, while the backend's per-position C-ABI lowering side-table
// (Direct for each scalar) is read at emit time. The `// proves:` comment maps
// each line to the Doc-1 matrix row(s) it drives.
//
// DEFERRED here (see manifest + the B7 report): FS-8/FS-9 (CAbiShape::Substituted
// + SubstReason) hit a BACKEND BUG — the repr(transparent) `emit_as` substitution
// is applied to the C *signature spelling* but NOT to the function body's local
// nor the call-site argument, so cc rejects the type mismatch. FS-5 (c_variadic)
// hits a MODEL-side blocker — the B-4 Call-arity joint validator
// (`args.len() != inputs.len()`) has no variadic exception, so any real variadic
// call site is rejected before the backend ever runs.

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

// FS-4 (FnAbi::C): an `extern "C"` boundary. Its two i32 inputs + i32 output are
// each CAbiShape::Direct (FS-7) — the position's own TyId in natural C repr — and
// the per-position outcomes are stored in the sig_c_abi_lowerings[FnSigId]
// side-table the backend reads at the C boundary (FS-10). The emitted prototype
// `int32_t input_add_c(int32_t, int32_t)` and its call both go through that table.
#[inline(never)]
extern "C" fn add_c(a: i32, b: i32) -> i32 { a + b }   // proves: FS-4, FS-7, FS-10

#[no_mangle]
pub extern "C" fn regseq_main() {
    // Direct-shape C-ABI call: both args + return are scalar i32 (FS-4 the
    // `extern "C"` FnAbi, FS-7 each position Direct, FS-10 the side-table read
    // at the boundary). The logged 42 proves the call lowered + executed.
    regseq::log::info!("add_c={}", add_c(40, 2));      // 42   proves: FS-4, FS-7, FS-10
}
