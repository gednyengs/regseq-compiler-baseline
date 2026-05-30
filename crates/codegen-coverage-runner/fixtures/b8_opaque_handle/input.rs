#![no_std]
#![feature(register_tool)]
#![feature(core_intrinsics)]
#![register_tool(rseq)]

// B8 opaque-handle (regseq-backends-24, IT-14): a ZST aggregate used ONLY
// behind a pointer is emitted forward-decl-only (`struct Handle;` +
// `struct Handle *` at use sites) with NO full definition and NO layout
// `_Static_assert`s. The bridge classifier flags `Handle.opaque_handle_eligible`
// (a non-generic ZST struct reachable exclusively behind a `Ref`/`Ptr`, model
// invariant 19); the generic-C backend then suppresses its bank-3 definition.
//
// `Handle` is NEVER constructed by value: every reference to it is a thin
// `*const Handle` obtained by a pointer-to-pointer cast off an unrelated `u32`
// anchor. We observe the handle relationally (never a raw address): the value
// the handle function returns equals the anchor's address taken the same way.
// IT-14 is proven structurally by the golden.c forward-decl (no definition) and
// behaviorally by the round-trip below.

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

// The opaque handle: a ZST struct that is NEVER used by value. The only
// references to it are `*const Handle` thin pointers, so the bridge classifier
// flags it `opaque_handle_eligible` and the backend forward-declares it only.
struct Handle;

// Takes the opaque handle behind a pointer and returns its address as an
// integer. `p as usize` is a thin pointer-to-integer cast; `*const Handle` is
// never dereferenced (the C side would have no complete type to deref). The
// `#[inline(never)]` keeps it a distinct emitted function carrying the
// `struct Handle *` parameter in golden.c.
#[inline(never)]
fn handle_addr(p: *const Handle) -> usize {
    p as usize // proves: IT-14 (opaque handle passed behind `struct Handle *`)
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    // An unrelated anchor whose address we reinterpret as the opaque handle.
    // `&raw const anchor as *const Handle` is a pointer-to-pointer cast: no
    // `Handle` value is ever constructed (the by-value-use criterion holds).
    let anchor: u32 = 0xABCD;
    let h: *const Handle = &raw const anchor as *const Handle; // proves: IT-14
    let got: usize = handle_addr(h);
    // Observe relationally (never a raw address): the handle round-trips back to
    // the anchor's address taken the same way.
    let expected: usize = &raw const anchor as usize;
    regseq::log::info!("opaque_ok={}", (got == expected) as u8); // 1  proves: IT-14
    regseq::log::info!("opaque_nonnull={}", (got != 0) as u8);   // 1  proves: IT-14
}
