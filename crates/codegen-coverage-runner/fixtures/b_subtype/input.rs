#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// CK-11 (CastKind::Subtype): a representation-identical subtype no-op cast.
// `coerce` returns its higher-ranked `for<'a> fn(&'a u32)` argument as a
// fixed-lifetime `fn(&'static u32)` -- a `Cast(Subtype, g, fn(&'static u32))`
// (the bridge-coverage RV-005-N witness shape). The backend lowers Subtype to
// the operand unchanged, so the coerced fn pointer still points at `observe`;
// calling it logs the pointee (42). `// proves:` maps the observed line.

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

// The observable callee: a `fn(&u32)` that logs its pointee.
#[inline(never)]
fn observe(x: &u32) {
    regseq::log::info!("subtype={}", *x);
}

static V: u32 = 42;

#[no_mangle]
pub extern "C" fn regseq_main() {
    // `observe` reified to a higher-ranked fn pointer, then coerced to a
    // fixed-lifetime fn pointer -- the coercion `g -> f` is the Subtype no-op
    // cast (CK-11). Done as a local assignment (not a fn return) to avoid the
    // separate fn-pointer-return declarator path.
    let g: for<'a> fn(&'a u32) = observe;
    let f: fn(&'static u32) = g; // proves: CK-11 (Cast(Subtype) representation-identical)
    f(&V); // 42 -> observe logs the pointee
}
