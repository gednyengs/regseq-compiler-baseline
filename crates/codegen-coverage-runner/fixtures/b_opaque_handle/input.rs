#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// IT-14 (opaque-handle pattern, gc section 2): a `#[repr(C)] struct OpaqueCtx
// { _private: [u8; 0] }` is the canonical opaque handle -- a non-generic ZST
// struct carrying the `[u8; 0]` opt-in marker, used ONLY behind a pointer
// (never by value). The bridge sets `opaque_handle_eligible`; the backend emits
// it forward-declaration-only (`struct OpaqueCtx;`, no body, no layout asserts)
// and `struct OpaqueCtx *` at use sites. Two opaque handles derived from the
// same backing address compare equal -> logged 1 (the dual oracle: golden.c
// shows the forward-decl-only emission; stdout proves the pointer flows).

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
        macro_rules! info { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_info($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use info;
    }
}

// The opaque handle: a ZST `#[repr(C)]` struct with the `[u8; 0]` marker. It is
// NEVER used by value (only as `*const OpaqueCtx`), so it is opaque-handle
// eligible (IT-14) and emits forward-declaration-only.
#[repr(C)]
struct OpaqueCtx {
    _private: [u8; 0],
}

// A real backing byte the handles point at (so the pointers are non-dangling).
static BACKING: u8 = 7;

#[no_mangle]
pub extern "C" fn regseq_main() {
    let base = &BACKING as *const u8;
    // Opaque handles obtained by casting the backing pointer -- no `OpaqueCtx`
    // value ever exists, so the struct is used only behind `*const OpaqueCtx`.
    let p: *const OpaqueCtx = base as *const OpaqueCtx;
    let q: *const OpaqueCtx = base as *const OpaqueCtx;
    regseq::log::info!("handle_eq={}", (p == q) as u8); // 1  proves: IT-14 (opaque handle)
}
