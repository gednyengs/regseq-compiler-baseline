#![no_std]
#![feature(register_tool)]
#![feature(core_intrinsics)]
#![register_tool(rseq)]

// B2 — floats + casts: float type/const, the `as` Cast rvalue across every
// CastKind (IntToInt, FloatToInt, FloatToFloat, IntToFloat, PtrToPtr,
// FnPtrToPtr, PointerExposeProvenance, PointerWithExposedProvenance) plus the
// Transmute intrinsic. Each info! line drives a value THROUGH the construct and
// logs it under the dual oracle. Carrier note (same as B1): the diag LogArg
// holds u8..u64/i8..i64/bool/&str only, so floats are observed via a
// boolean-checksum `(expr == EXPECTED) as u8` or a bit-exact transmute→u32, and
// pointers are observed RELATIONALLY (never log a raw address — it would break
// the byte-exact stdout oracle).

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

#[no_mangle]
pub extern "C" fn regseq_main() {
    // --- TY-5 (Float type) + CO-5 (Float const): f64/f32 arithmetic, observed
    // by boolean-checksum (== is a BinOp, no method call) ---
    let f: f64 = 3.5;                                  // CO-5 (f64 const)
    regseq::log::info!("f64ck={}", ((f + 1.0) == 4.5) as u8);   // 1  proves: TY-5, CO-5, BO-1
    let g: f32 = 2.25;                                 // CO-5 (f32 const)
    regseq::log::info!("f32ck={}", ((g * 2.0) == 4.5) as u8);   // 1  proves: TY-5 (f32), CO-5

    // f64 bit-exact via Transmute (also CK-10/RV-5): 1.5f64 == 0x3FF8000000000000.
    let bits64: u64 = unsafe { core::intrinsics::transmute::<f64, u64>(1.5f64) };
    regseq::log::info!("f64bits={}", bits64);          // 4609434218613702656  proves: TY-5

    // --- CK-10 (Transmute) + RV-5: f32 -> u32 bit-reinterpret. 1.5f32 = 0x3FC00000.
    let t: u32 = unsafe { core::intrinsics::transmute::<f32, u32>(1.5f32) };
    regseq::log::info!("transmute={}", (t == 0x3FC0_0000u32) as u8); // 1  proves: CK-10, RV-5
    regseq::log::info!("transmute_raw={}", t);         // 1069547520  proves: CK-10

    // --- CK-4 (IntToInt) + RV-5: narrowing int cast. 300i32 as u8 = 44 (300-256).
    let c4: u8 = 300i32 as u8;
    regseq::log::info!("inttoint={}", c4);             // 44  proves: CK-4, RV-5

    // --- CK-5 (FloatToInt, saturating) + RV-5: truncates toward zero. 3.9 -> 3.
    let c5: i32 = 3.9f64 as i32;
    regseq::log::info!("floattoint={}", c5);           // 3  proves: CK-5, RV-5
    // saturation edge: 1e20f64 as i32 saturates to i32::MAX (2147483647).
    let c5sat: i32 = 1e20f64 as i32;
    regseq::log::info!("floattoint_sat={}", c5sat);    // 2147483647  proves: CK-5

    // --- CK-6 (FloatToFloat) + RV-5: f64 -> f32. 3.5 is exactly representable.
    let c6: f32 = 3.5f64 as f32;
    regseq::log::info!("floattofloat={}", (c6 == 3.5f32) as u8); // 1  proves: CK-6, RV-5

    // --- CK-7 (IntToFloat) + RV-5: 7i32 as f64 == 7.0.
    let c7: f64 = 7i32 as f64;
    regseq::log::info!("inttofloat={}", (c7 == 7.0f64) as u8);   // 1  proves: CK-7, RV-5

    // --- CK-8 (PtrToPtr) + RV-5: *const i32 -> *const u8, deref the low byte.
    // x = 0x44434241 little-endian => byte[0] = 0x41 = 65.
    let x: i32 = 0x4443_4241;
    let p_i32: *const i32 = &x as *const i32;
    let p_u8: *const u8 = p_i32 as *const u8;          // PtrToPtr
    regseq::log::info!("ptrtoptr={}", unsafe { *p_u8 }); // 65  proves: CK-8, RV-5

    // --- CK-9 (FnPtrToPtr) is DEFERRED: it requires first reifying a fn item to
    // a fn-pointer value, and the natural `kind=rust` ReifyFnPointer surface
    // (`marker_fn as fn()`) lowers to a Cast whose operand is a bare `Const::ZST`
    // FnDef value, which the backend does not resolve ("ReifyFnPointer of an
    // unresolved function (task d2)"). See manifest note. ---

    // --- CK-1 (PointerExposeProvenance) + RV-5: *const i32 -> usize. Observe
    // relationally (NEVER log the address). ---
    let n: usize = p_i32 as usize;                     // PointerExposeProvenance
    regseq::log::info!("expose={}", (n != 0) as u8);   // 1  proves: CK-1, RV-5

    // --- CK-2 (PointerWithExposedProvenance) + RV-5: usize -> *const i32, round
    // -trip and deref-compare to the original value. ---
    let p2: *const i32 = n as *const i32;              // PointerWithExposedProvenance
    regseq::log::info!("withexposed={}", (unsafe { *p2 } == x) as u8); // 1  proves: CK-2, RV-5
}
