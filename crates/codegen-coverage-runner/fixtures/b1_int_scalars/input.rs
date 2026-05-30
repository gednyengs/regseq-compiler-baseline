#![no_std]
#![feature(register_tool)]
#![feature(core_intrinsics)]
#![register_tool(rseq)]

// B1 — integer/bool/char scalars: arithmetic, bitwise, shift, compare, unops
// (+ the function/body/signature ENVELOPE every fixture reuses). Each info!
// line below is a genuine end-to-end test: regseq_main computes a value
// THROUGH the construct under test and logs it (the dual oracle then proves
// "compiled + printed the right value"). The `// proves:` comment on each line
// maps it to the Doc-1 matrix row(s) it drives (the covers integrity convention
// — declare only what is actually exercised). Carrier note: the diag LogArg
// holds u8..u64/i8..i64/bool/&str only, so char/usize/isize are cast to a
// carrier and u128/i128 use a boolean-checksum `(value == EXPECTED) as u8`
// (the int-cast itself is CK-4/RV-5, owned by B2 — incidental here).

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

// Rust-ABI helpers taking args → exercise FS-1/2/3 (sig + Rust ABI), BD-2/3
// (formals vs temps, arg_count>0), the Call terminator, Move/Copy operands.
#[inline(never)]
fn add_i32(a: i32, b: i32) -> i32 { a + b }   // proves: BO-1 (Add), RV-6, FS-1/2/3, BD-2/3, OP-2 (arg move)
#[inline(never)]
fn neg_i32(x: i32) -> i32 { -x }              // proves: UN-2 (Neg), RV-7

#[no_mangle]
pub extern "C" fn regseq_main() {
    // --- envelope + arithmetic over i32 (BO-1..5) ---
    regseq::log::info!("add={}", add_i32(7, 3));      // 10   proves: BO-1, RV-6, FS-*, BD-*, OP-2, TM-5
    regseq::log::info!("neg={}", neg_i32(7));         // -7   proves: UN-2, RV-7
    let a: i32 = 7;
    let b: i32 = 3;
    regseq::log::info!("sub={}", a - b);              // 4    proves: BO-2 (Sub), OP-1 (Copy), ST-1 (Assign)
    regseq::log::info!("mul={}", a * b);              // 21   proves: BO-3 (Mul)
    regseq::log::info!("div={}", a / b);              // 2    proves: BO-4 (Div)
    regseq::log::info!("rem={}", a % b);              // 1    proves: BO-5 (Rem)
    let u = a as i32;                                  //      proves: RV-1 (Use)
    regseq::log::info!("use={}", u);                  // 7    proves: RV-1

    // --- bitwise over u8 (BO-12..14) ---
    let p: u8 = 0b1100;
    let q: u8 = 0b1010;
    regseq::log::info!("band={}", p & q);             // 8    proves: BO-12 (BitAnd), TY-4 (u8)
    regseq::log::info!("bor={}", p | q);              // 14   proves: BO-13 (BitOr)
    regseq::log::info!("bxor={}", p ^ q);             // 6    proves: BO-14 (BitXor)

    // --- shifts over u8 (BO-15..18): `<<`/`>>` operators (Shl/Shr) + the
    // unchecked-shift intrinsics (ShlUnchecked/ShrUnchecked — no operator
    // surface, so via core::intrinsics, which LowerIntrinsics turns into the
    // model BinOp in this fixture's own MIR). All four members of the range. ---
    regseq::log::info!("shl={}", 1u8 << 3);           // 8    proves: BO-15..18 (Shl)
    regseq::log::info!("shr={}", 240u8 >> 2);         // 60   proves: BO-15..18 (Shr)
    regseq::log::info!("ushl={}", unsafe { core::intrinsics::unchecked_shl(1u8, 3u32) });   // 8  proves: BO-15..18 (ShlUnchecked)
    regseq::log::info!("ushr={}", unsafe { core::intrinsics::unchecked_shr(240u8, 2u32) }); // 60 proves: BO-15..18 (ShrUnchecked)

    // --- unchecked arithmetic over i32 (BO-6..8) via intrinsics ---
    regseq::log::info!("uca={}", unsafe { core::intrinsics::unchecked_add(7i32, 3i32) });   // 10 proves: BO-6..8 (AddUnchecked)
    regseq::log::info!("ucs={}", unsafe { core::intrinsics::unchecked_sub(7i32, 3i32) });   // 4  proves: BO-6..8 (SubUnchecked)
    regseq::log::info!("ucm={}", unsafe { core::intrinsics::unchecked_mul(7i32, 3i32) });   // 21 proves: BO-6..8 (MulUnchecked)

    // --- with-overflow over u8 (BO-9..11): intrinsic returns (value, overflowed) ---
    let (ova, ovo) = core::intrinsics::add_with_overflow(250u8, 10u8);  // (4, true)
    regseq::log::info!("ova={} ovo={}", ova, ovo);    // 4 1   proves: BO-9..11 (AddWithOverflow)
    let (ovs, ovso) = core::intrinsics::sub_with_overflow(5u8, 10u8);   // (251, true)
    regseq::log::info!("ovs={} ovso={}", ovs, ovso);  // 251 1 proves: BO-9..11 (SubWithOverflow)
    let (ovm, ovmo) = core::intrinsics::mul_with_overflow(200u8, 2u8);  // (144, true)
    regseq::log::info!("ovm={} ovmo={}", ovm, ovmo);  // 144 1 proves: BO-9..11 (MulWithOverflow)

    // NOTE: BO-25 (Cmp / three_way_compare) is DEFERRED — it lowers fine but hits
    // a backend type bug: the Cmp result is emitted as int8_t yet assigned to the
    // Ordering enum-struct destination ("incompatible types ... struct
    // core_cmp_Ordering from signed char"). Needs a backend fix, not a recipe change.

    // --- comparisons i32 (BO-19..24) → bool (TY-1) ---
    regseq::log::info!("eq={} lt={} le={} ne={} ge={} gt={}",
        a == b, a < b, a <= b, a != b, a >= b, a > b); // 0 0 0 1 1 1  proves: BO-19..24, TY-1

    // --- unary Not (UN-1): integer bit-not + bool not ---
    regseq::log::info!("not_u={}", !0u8);              // 255 proves: UN-1 (Not, integer)
    regseq::log::info!("not_b={}", !true);             // 0   proves: UN-1 (Not, bool), TY-1
    regseq::log::info!("boolt={}", true);              // 1   proves: TY-1, CO-1 (Bool const)

    // --- char (TY-2): cast to u32 carrier ---
    regseq::log::info!("char={}", 'A' as u32);         // 65  proves: TY-2 (Char), CO-2 (Char const)

    // --- signed widths (TY-3): i8 / i16 / i64 / isize ---
    regseq::log::info!("i8={}", 5i8 + 3);              // 8        proves: TY-3 (i8)
    regseq::log::info!("i16={}", 300i16 + 100);        // 400      proves: TY-3 (i16)
    regseq::log::info!("i64={}", 1_000_000_000_000i64 + 1); // 1000000000001  proves: TY-3 (i64)
    let isz: isize = -7;
    regseq::log::info!("isz={}", (isz * 6) as i64);    // -42      proves: TY-3 (isize)

    // --- unsigned widths (TY-4): usize ---
    let usz: usize = 7;
    regseq::log::info!("usz={}", (usz * 6) as u64);    // 42       proves: TY-4 (usize)

    // --- 128-bit (TY-3 i128 / TY-4 u128): boolean-checksum of a >64-bit result ---
    let wu: u128 = 10_000_000_000u128 * 10_000_000_000u128;     // 1e20
    regseq::log::info!("u128ck={}", (wu == 100_000_000_000_000_000_000u128) as u8); // 1 proves: TY-4 (u128), BO-3
    let wi: i128 = -10_000_000_000i128 * 10_000_000_000i128;    // -1e20
    regseq::log::info!("i128ck={}", (wi == -100_000_000_000_000_000_000i128) as u8); // 1 proves: TY-3 (i128)

    // --- explicit unsigned-literal const (CO-4) + int const (CO-3 via 7/3 above) ---
    regseq::log::info!("cuint={}", 42u32);             // 42  proves: CO-4 (Uint const), OP-3 (Constant)
}
