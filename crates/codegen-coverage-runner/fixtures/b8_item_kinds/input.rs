#![no_std]
#![feature(register_tool)]
#![feature(core_intrinsics)]
#![register_tool(rseq)]

// B8 — item kinds: the constructs a plain function fixture does NOT contain.
// This fixture drives the item-kind rows that survive the bridge as observable
// runtime behavior: a Static item (read), a `static mut` (read under unsafe), and
// a named `const` item (used in a computation). Each info! line drives the
// construct THROUGH regseq_main and logs the result (the dual oracle proves
// "compiled the item kind + printed the right value"); the `// proves:` comment
// maps each line to the Doc-1 matrix row(s) it drives.
//
// The other four assigned rows are DEFERRED (BRIDGE drops/never-sets the
// construct — not a recipe gap): IT-8 (link_section), IT-9 (inline_hint),
// IN-3 (foreign fn body:None), IT-14 (opaque_handle). The manifest's DEFERRED
// block carries the exact repro + file:line for each.

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

// --- Static item (immutable) + `static mut` (mutable). Reading them drives the
// Static item kind + Static instance. ---
static S: u32 = 42;          // proves: IT-2 (Static item), IN-7 (Static inst)
static mut M: u32 = 7;       // proves: IT-2 (Static item), IN-7 (Static inst)

// --- Const item: a named `const`, materialised at its use site (Const inst). ---
const C: u32 = 9;            // proves: IT-3 (Const item), IN-8 (Const inst)

// NOTE: IT-8 (link_section), IT-9 (inline_hint), IN-3 (foreign fn body:None) and
// IT-14 (opaque_handle) are NOT driven here — each is dropped/never-set by the
// BRIDGE (not the backend), so the construct never reaches the emitter. See the
// manifest's DEFERRED block for the exact repro + file:line for each. This fixture
// drives only the four item-kind rows that survive the bridge as runtime behavior.

#[no_mangle]
pub extern "C" fn regseq_main() {
    // --- IT-2 / IN-7: read the immutable static, and the static mut under unsafe ---
    regseq::log::info!("stat={}", S);                 // 42  proves: IT-2, IN-7
    regseq::log::info!("statmut={}", unsafe { M });   // 7   proves: IT-2, IN-7

    // --- IT-3 / IN-8: use the const item in a computation (Const materialises at use) ---
    regseq::log::info!("constc={}", C + 1);           // 10  proves: IT-3, IN-8
}
