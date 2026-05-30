#![no_std]
#![feature(register_tool)]
#![feature(core_intrinsics)]
#![register_tool(rseq)]

// B5 — control flow + calls: a loop back-edge (Goto, TM-1), a >=3-arm match
// (SwitchTargets, TM-7), an indirect call through a fn-pointer value (TM-9), the
// FnPtr type (TY-13), the UnsafeFnPointer pointer-coercion (PC-2 / CK-3), and Drop
// glue run at scope exit (TM-10). Each info! line drives a value THROUGH the construct
// under test and logs the computed result (the dual oracle then proves "compiled +
// printed the right value"). `// proves:` maps each line to the Doc-1 matrix row(s) it
// drives. (IT-13 ManuallyDrop, PC-1 ReifyFnPointer, and IN-5 ReifyShim are DEFERRED on
// backend bugs — see the manifest.)
//
// FN-POINTER CONSTRUCTION NOTE (load-bearing): the fn pointer is sourced from a
// `static F: fn(i32)->i32 = dbl;`. rustc const-evaluates that initializer into a
// `Const::Reloc` to `dbl`, which the backend emits cleanly. A *direct* coercion
// (`let f: fn(i32)->i32 = dbl;`) instead emits a `Cast(ReifyFnPointer, Const::ZST,
// FnPtr)` that the backend cannot lower (it can recover the target only from a
// `Const::Reloc` operand or an `FnDef`-typed *place*, never from the ZST FnDef
// constant). That is why PC-1 (ReifyFnPointer) and IN-5 (ReifyShim) are DEFERRED —
// see manifest. The static route keeps TM-9/TY-13/PC-2/CK-3 fully constructible.

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

// The indirect-call target. Its body lives in the fixture, so no undefined ref.
#[inline(never)]
fn dbl(x: i32) -> i32 { x * 2 }

// A `static` of fn-pointer type: rustc const-evaluates the `dbl` coercion into a
// `Const::Reloc`, so loading F yields a clean fn-pointer value (no ZST reify cast).
static F: fn(i32) -> i32 = dbl;

// A type with a Drop impl whose body IS in the fixture; constructing it in a scope
// makes drop glue (TM-10) run at scope exit. The drop body records the payload in a
// static so the side effect is observable. A NAMED-field struct (not a tuple struct)
// is used deliberately: the backend emits a tuple field with its raw rustc index name
// (`.0`) — invalid C — in the struct def, offsetof static-assert, aggregate init, and
// field access. A named field sanitizes to a legal C identifier. (Backend bug; see the
// manifest deferral note.)
struct D { v: u32 }

static mut DROPPED: u32 = 0;

impl Drop for D {
    fn drop(&mut self) {
        unsafe { DROPPED = self.v; }
    }
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    // --- TM-1 Goto: a `while` loop's back-edge is a Goto terminator. Sum 0..4. ---
    let mut i: i32 = 0;
    let mut s: i32 = 0;
    while i < 4 {
        s += i;
        i += 1;
    }
    regseq::log::info!("loopsum={}", s);          // 6   proves: TM-1 (Goto)

    // --- TM-7 SwitchTargets: a match with >=3 value arms on an int lowers to a
    // SwitchInt with multiple non-default targets. Select & log the matched arm. ---
    let sel: i32 = 2;
    let m = match sel {
        0 => 100i32,
        1 => 200i32,
        2 => 300i32,
        3 => 400i32,
        _ => 999i32,
    };
    regseq::log::info!("match={}", m);            // 300 proves: TM-7 (SwitchTargets)

    // --- TY-13 FnPtr + TM-9 Indirect call: load the fn-pointer value from the
    // static F (a clean Const::Reloc to `dbl`) into a `fn(i32)->i32` local, then
    // call it indirectly. The call terminator's callee is Indirect(Move(fnptr)). ---
    let f: fn(i32) -> i32 = F;                     // proves: TY-13 (FnPtr)
    regseq::log::info!("indirect={}", f(21));      // 42  proves: TM-9 (Indirect call)

    // --- PC-2 UnsafeFnPointer + CK-3 PointerCoercion: coerce the loaded safe
    // fn-pointer to an `unsafe fn(...)` pointer (a PointerCoercion::UnsafeFnPointer
    // cast), then call it indirectly in an unsafe block. ---
    let g: unsafe fn(i32) -> i32 = f;              // proves: PC-2 (UnsafeFnPointer), CK-3 (PointerCoercion)
    let gv = unsafe { g(20) };
    regseq::log::info!("unsafe_fp={}", gv);        // 40  proves: PC-2, TM-9

    // --- TM-10 Drop glue: construct a real D in an inner scope; at scope exit its
    // Drop::drop runs (drop glue), writing its payload into DROPPED. ---
    {
        let _d = D { v: 123 };
    } // _d dropped here -> D::drop runs -> DROPPED = 123
    regseq::log::info!("dropped={}", unsafe { DROPPED });     // 123 proves: TM-10 (Drop glue)
}
