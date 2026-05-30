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
// IT-8 (link_section function) and IT-9 (inline_hint) are now driven here:
// regseq-driver-83 made the bridge read rustc's codegen_fn_attrs, so the
// link_section / inline classification reaches the backend's fn_qualifiers.
// IN-3 (foreign fn body:None) and IT-14 (opaque_handle) stay DEFERRED (BRIDGE
// drops/never-sets the construct — not a recipe gap). The manifest's DEFERRED
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

// --- IT-8 (link_section FUNCTION): the bridge reads rustc's codegen_fn_attrs
// link_section and the backend emits `__attribute__((section(".driver_data")))`
// on the function prototype + definition. Called from regseq_main so the
// instance is emitted. (Static link_section is a separate backend follow-up,
// regseq-backends-24; not driven here.) ---
#[inline(never)]
#[link_section = ".driver_data"]
fn sectioned(x: u32) -> u32 {     // proves: IT-8 (link_section attr emitted)
    x + 1                          // plain operator (no core method -> no undefined ref)
}

// --- IT-9 (inline_hint): #[inline(always)] -> `inline __attribute__((always_inline))`
// and #[inline(never)] -> `__attribute__((noinline))`. The bridge reads the
// codegen_fn_attrs inline classification; the backend's fn_qualifiers spells it.
// At mir_opt_level=0 these are NOT actually inlined, so both functions survive as
// real C definitions carrying their attribute. Bodies use plain operators only
// (a core method like wrapping_* would be an undefined ref AND, under
// always_inline, cc would reject the missing body). ---
#[inline(always)]
fn always_hot(x: u32) -> u32 {    // proves: IT-9 (inline(always) -> always_inline)
    x * 2
}

#[inline(never)]
fn never_cold(x: u32) -> u32 {    // proves: IT-9 (inline(never) -> noinline)
    x - 1
}

// NOTE: IN-3 (foreign fn body:None) and IT-14 (opaque_handle) are NOT driven
// here — each is dropped/never-set by the BRIDGE (not the backend), so the
// construct never reaches the emitter. See the manifest's DEFERRED block for the
// exact repro + file:line for each.

#[no_mangle]
pub extern "C" fn regseq_main() {
    // --- IT-2 / IN-7: read the immutable static, and the static mut under unsafe ---
    regseq::log::info!("stat={}", S);                 // 42  proves: IT-2, IN-7
    regseq::log::info!("statmut={}", unsafe { M });   // 7   proves: IT-2, IN-7

    // --- IT-3 / IN-8: use the const item in a computation (Const materialises at use) ---
    regseq::log::info!("constc={}", C + 1);           // 10  proves: IT-3, IN-8

    // --- IT-8: call the #[link_section] function (its definition carries the
    // section attr in golden.c; the call also forces it to be emitted) ---
    regseq::log::info!("sect={}", sectioned(40));     // 41  proves: IT-8

    // --- IT-9: call the inline-hinted functions (always_inline / noinline attrs
    // appear on their definitions in golden.c) ---
    regseq::log::info!("hot={}", always_hot(21));     // 42  proves: IT-9 (always)
    regseq::log::info!("cold={}", never_cold(10));    // 9   proves: IT-9 (never)
}
