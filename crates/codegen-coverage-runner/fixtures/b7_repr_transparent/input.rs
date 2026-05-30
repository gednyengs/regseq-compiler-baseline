#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// B7 (cont.) -- the C-ABI `Substituted` boundary (regseq-backends-23). A
// `#[repr(transparent)] struct W(u32)` across an `extern "C"` boundary computes
// `CAbiShape::Substituted { emit_as: u32, reason: TransparentChain }` for the
// transparent-newtype position. The backend emits `emit_as` (the inner u32) at
// the C boundary -- prototype, definition, call site -- and bridges the gap to
// the original-typed body local by a representation-identical copy (marshalling)
// at the prologue / return / call site. The logged results prove the marshalled
// values round-trip correctly.
//
// id_w  exercises a Substituted INPUT  (W -> u32 at the boundary): proves the
//       prologue copy + the call-site argument reinterpret -- FS-8, FS-9.
// make_w exercises a Substituted OUTPUT (W <- u32 at the boundary): proves the
//       return marshalling + the call-site destination reinterpret -- FS-8, FS-9.

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

// FS-8 (CAbiShape::Substituted) + FS-9 (SubstReason::TransparentChain): a
// `#[repr(transparent)] struct W(u32)`. Its C-ABI shape across `extern "C"` is
// `Substituted { emit_as: u32, reason: TransparentChain }`; the backend emits
// the inner u32 (`emit_as`) at every C position and marshals to/from the
// original W local.
#[repr(transparent)]
struct W(u32);

// Substituted INPUT: the prototype/definition take `uint32_t`; the prologue
// copies the ABI u32 into the body's W local, and the call site reinterprets the
// W argument to u32. Reading `w.0` returns the inner value.
#[inline(never)]
extern "C" fn id_w(w: W) -> u32 { w.0 }      // proves: FS-8, FS-9

// Substituted OUTPUT: the prototype/definition return `uint32_t`; each `return`
// marshals the body's W return place to the ABI u32, and the call site
// reinterprets the u32 result back into the W destination.
#[inline(never)]
extern "C" fn make_w(x: u32) -> W { W(x) }   // proves: FS-8, FS-9

#[no_mangle]
pub extern "C" fn regseq_main() {
    // Substituted-input call: pass a W, get its inner u32 back. The 7 proves the
    // argument reinterpret + prologue marshalling round-tripped.
    regseq::log::info!("id_w={}", id_w(W(7)));         // 7    proves: FS-8, FS-9
    // Substituted-output call: get a W back, read its inner u32. The 9 proves the
    // return marshalling + destination reinterpret round-tripped.
    let made = make_w(9);
    regseq::log::info!("make_w={}", made.0);           // 9    proves: FS-8, FS-9
}
