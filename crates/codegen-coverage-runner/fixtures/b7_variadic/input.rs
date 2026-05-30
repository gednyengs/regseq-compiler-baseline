#![no_std]
#![feature(register_tool)]
#![feature(core_intrinsics)]
#![register_tool(rseq)]

// B7-variadic - the C `...` boundary (FS-5: FnSig.c_variadic -> trailing C `...`).
//
// A `c_variadic` callee is unconstructible from an in-fixture Rust variadic body
// (an `extern "C" fn f(a, mut _rest: ...)` body trips `DslRejection { CVoid }`:
// the `...`/VaListImpl drags in core::ffi::c_void, which bridge type-translation
// rejects). The constructible surface is therefore a libc variadic foreign
// DECLARATION -- the same shape as b8_item_kinds's `abs`, but with a trailing
// `...`.
//
// snprintf (the FS-5 spec's first suggestion) is NOT usable: the backend's
// runtime prelude `#include <stdio.h>` AND its BoundsCheck panic-message code both
// rely on libc's real `snprintf(char *, ...)`, so a Rust `*mut u8` redeclaration
// emits `extern int32_t snprintf(uint8_t *, ...)` -> a hard C "conflicting types
// for snprintf" error. The spec explicitly permits "another that does not trip the
// c_void rejection", so the surface here is `open(2)` from libc: a genuinely
// variadic symbol NOT declared by any header the prelude includes (no conflict)
// and NOT used by the backend runtime.
//
// `open` returns the new fd or -1 on error. Calling it on a path whose PARENT
// directory does not exist fails with ENOENT and returns -1 *deterministically*,
// with zero filesystem side effects (no file is ever created, at any privilege
// level). The fixture makes two variadic-prototype calls -- one with a trailing
// variadic `mode` arg (O_CREAT) and one with none -- and logs the deterministic
// -1 from each, so the dual oracle proves the C `...` prototype compiled, linked
// against libc, and was called with varying variadic-arg counts. Raw pointer
// addresses are NEVER logged (LogArg rule); only the i32 return values are.

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

// --- FS-5 (c_variadic): a libc variadic foreign function. The bridge mints
// `open`'s `emission_name` from its real ABI link name (regseq-driver-84); the
// B-4 arity validators admit the trailing variadic arg (regseq-driver-86 +
// regseq-model-97); the backend emits the `...` prototype + variadic call. ---
extern "C" {
    fn open(path: *const u8, flags: i32, ...) -> i32; // proves: FS-5
}

// O_CREAT (0x40) drives the 3-arg variadic form (path, flags, mode); O_RDONLY (0)
// drives the 2-arg form (no trailing variadic arg). The path's parent directory
// does not exist, so each call returns -1 (ENOENT) with no side effect.
const O_CREAT: i32 = 0x40;
const O_RDONLY: i32 = 0x0;

// NUL-terminated path bytes as a `static` array, so the `*const u8` is `&raw`-
// derived (no .as_ptr() core method -> no undefined reference). Parent dir
// `/regseq_no_such_dir/` never exists -> deterministic ENOENT.
static PATH: [u8; 32] = *b"/regseq_no_such_dir/x\0\0\0\0\0\0\0\0\0\0\0";

#[no_mangle]
pub extern "C" fn regseq_main() {
    // --- FS-5: variadic call WITH a trailing variadic arg (mode 0o600). The
    // `...` machinery passes the i32 mode; `open` fails ENOENT -> -1. ---
    let fd_create: i32 = unsafe { open(&raw const PATH[0], O_CREAT, 0o600i32) };
    regseq::log::info!("open_create={}", fd_create);   // -1  proves: FS-5 (variadic-tail call)

    // --- FS-5: the same `...` prototype called with NO trailing variadic arg.
    // Exercises the variadic prototype across differing arg counts. ---
    let fd_ro: i32 = unsafe { open(&raw const PATH[0], O_RDONLY) };
    regseq::log::info!("open_ro={}", fd_ro);            // -1  proves: FS-5 (empty-tail call)
}
