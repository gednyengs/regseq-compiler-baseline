#![no_std]
#![feature(register_tool)]
#![feature(core_intrinsics)]
#![register_tool(rseq)]

// B5-reify — direct fn-item -> fn-pointer reify + fn-ptr-to-thin-ptr cast.
//
// PC-1 (ReifyFnPointer): a *direct* coercion `let f: fn(i32)->i32 = dbl;`
// lowers to `Cast(PointerCoercion(ReifyFnPointer), Const::ZST, FnPtr)` — the
// operand is the zero-sized `FnDef` constant. The bridge (regseq-backends-20
// fix) recovers the `FnDef` identity at the cast site and lowers the operand to
// a `Const::Reloc` to the resolved function instance, which the backend's
// `reify_fn_pointer` Reloc path already emits as `&dbl`. (A backend-only fix is
// impossible: the model `Const::ZST` is a single canonical representative for
// all zero-sized types and carries no `FnDef` item/args, and the cast's dest
// type is only the *signature* — ambiguous across distinct same-signature fns.)
//
// CK-9 (FnPtrToPtr): `f as *const ()` casts the reified fn-pointer to a thin
// data pointer (a `CastKind::FnPtrToPtr` rvalue). The result is observed
// RELATIONALLY (`(p as usize != 0) as u8`) — never logging a raw address, so
// the byte-exact stdout oracle holds.
//
// Each info! line drives a value THROUGH the construct under test; `// proves:`
// maps it to the Doc-1 matrix row(s).

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

// The reify target. Its body lives in the fixture, so `&dbl` is a defined ref.
#[inline(never)]
fn dbl(x: i32) -> i32 { x * 2 }

#[no_mangle]
pub extern "C" fn regseq_main() {
    // --- PC-1 ReifyFnPointer: a DIRECT fn-item -> fn-pointer coercion. This is
    // the `Cast(ReifyFnPointer, Const::ZST FnDef, FnPtr)` surface. Call it
    // indirectly so the reified pointer is observably the real `dbl`. ---
    let f: fn(i32) -> i32 = dbl;              // proves: PC-1 (ReifyFnPointer)
    regseq::log::info!("reify={}", f(21));    // 42  proves: PC-1, TM-9 (indirect call)

    // --- CK-9 FnPtrToPtr: cast the reified fn-pointer to a thin data pointer.
    // Observed RELATIONALLY (a reified fn pointer is never null) to keep stdout
    // byte-exact and address-independent: the `f as *const ()` is the
    // `FnPtrToPtr` cast under test; the follow-on `as usize` (an expose-
    // provenance cast, already covered by CK-1) only lets us compare `!= 0`
    // without logging the address or calling a libcore method. ---
    let p = f as *const ();                   // proves: CK-9 (FnPtrToPtr)
    let n = p as usize;
    regseq::log::info!("fnptr_nonnull={}", (n != 0) as u8);  // 1  proves: CK-9
}
