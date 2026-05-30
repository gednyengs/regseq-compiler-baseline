#![no_std]
#![feature(register_tool)]
#![feature(core_intrinsics)]
#![register_tool(rseq)]

// B4 — enums (runtime). Defines four enums covering every D9 layout shape
// (fieldless Direct, multi-variant payload, Option-like Niche, and an empty
// enum referenced structurally), constructs variants (Aggregate::Enum),
// `match`es on them (Downcast + Discriminant read), mutates a discriminant
// (SetDiscriminant), and references an EnumValue const. Each enum is both
// CONTAINED (so the backend emits its `_Static_assert(sizeof/_Alignof/
// offsetof)` layout guards — proving the LY-* shape rows by COMPILATION) and
// exercised at runtime (a logged payload/discriminant proves the op rows).
// Carrier note: the diag LogArg holds u8..u64/i8..i64/bool/&str only, so
// payloads/discriminants are observed as ints.

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

// --- Enum definitions (the IT-5 Enum item / TY-18 Enum type universe) ---

// (0) single-variant enum → LY-8 (EnumShape::Single). One payload field so it
// is not a ZST and emits a size-faithful struct + its layout guards.
enum One { X(u16) }               // proves: LY-8 (Single)

// (1) fieldless C-like enum → LY-9 (Multiple, fieldless), LY-11 (Direct tag
// encoding), LY-10 (TagScalar — the fieldless repr(int) tag cell).
enum Color { Red, Green, Blue }   // proves: TY-18, IT-5, LY-11, LY-10, LY-9

// (2) multi-variant PAYLOAD enum → LY-9 (Multiple variants), LY-13
// (VariantLayout — per-variant field offsets), LY-14 (VariantRange — the
// variants span a discriminant range). The shifted-alignment payloads
// (u32 vs (u8,u8) vs unit) exercise the union-of-views D9 form.
enum E { A(u32), B(u8, u8), C }   // proves: TY-18, LY-9, LY-13, LY-14

// (3) Option-like NICHE enum → LY-12 (Niche encoding: the `None` variant is
// represented by a niche value of the `&u32` field, no separate tag word).
enum N { Some(&'static u32), None }   // proves: LY-12 (Niche)

// (4) empty enum, referenced ONLY behind a `*const Void` field → LY-7
// (Empty enum). Structural: the backend names every nominal ADT reachable
// from the model, so `Void` emits its (degenerate) form into golden.c.
enum Void {}                      // proves: LY-7 (Empty enum)
struct VoidRef { p: *const Void } // contains Void so it is reachable

// CO-9 (EnumValue const): a const enum value referenced at runtime.
const KONST: E = E::A(7);         // proves: CO-9 (EnumValue const)

static GUARD: u32 = 0xABCD;       // pointee for the niche enum's Some arm

// Read the payload of an E::A variant via `match` → PL-8 (Downcast) + RV-8
// (the match scrutinee lowers to a Discriminant read of the enum).
#[inline(never)]
fn e_tag_and_payload(e: &E) -> (u8, u32) {
    match e {                     // proves: RV-8 (Discriminant), PL-8 (Downcast)
        E::A(x) => (0, *x),
        E::B(p, q) => (1, (*p as u32) << 8 | (*q as u32)),
        E::C => (2, 0),
    }
}

// Read a fieldless enum's discriminant via `match`.
#[inline(never)]
fn color_tag(c: &Color) -> u8 {
    match c {                     // proves: RV-8, PL-8
        Color::Red => 0,
        Color::Green => 1,
        Color::Blue => 2,
    }
}

// Read the niche enum: 1 if Some (and deref the pointee), 0 if None.
#[inline(never)]
fn n_probe(n: &N) -> (u8, u32) {
    match n {                     // proves: PL-8 (Downcast on niche), RV-8
        N::Some(r) => (1, **r),
        N::None => (0, 0),
    }
}

// Read the single-variant enum's only payload (Single shape).
#[inline(never)]
fn one_payload(o: &One) -> u16 {
    match o {                     // proves: PL-8 (Downcast on Single), LY-8
        One::X(v) => *v,
    }
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    // --- (0) single-variant enum (LY-8 Single): construct + read payload ---
    let one = One::X(4242);       // proves: AG-4, LY-8, IN-10
    regseq::log::info!("one={}", one_payload(&one));       // 4242   proves: LY-8, PL-8

    // --- (1) fieldless Direct enum: construct (AG-4) + discriminant read ---
    let c0 = Color::Red;          // proves: AG-4 (Enum aggregate), IN-10 (Enum inst)
    let c1 = Color::Green;
    let c2 = Color::Blue;
    regseq::log::info!("color={} {} {}",
        color_tag(&c0), color_tag(&c1), color_tag(&c2));   // 0 1 2  proves: RV-8, PL-8, LY-11

    // --- (2) payload enum: construct each variant (AG-4) + Downcast read ---
    let ea = E::A(0xDEAD);        // proves: AG-4 (payload aggregate)
    let eb = E::B(0x12, 0x34);
    let ec = E::C;
    let (ta, pa) = e_tag_and_payload(&ea);
    let (tb, pb) = e_tag_and_payload(&eb);
    let (tc, pc) = e_tag_and_payload(&ec);
    regseq::log::info!("ea_tag={} ea_pl={}", ta, pa);      // 0 57005   proves: PL-8, RV-8, LY-13
    regseq::log::info!("eb_tag={} eb_pl={}", tb, pb);      // 1 4660    proves: PL-8, LY-14
    regseq::log::info!("ec_tag={} ec_pl={}", tc, pc);      // 2 0       proves: PL-8

    // --- ST-2 SetDiscriminant: reassign an enum local to a new variant in a
    // branch (the `e = E::C` store lowers to a SetDiscriminant) ---
    let mut em = E::A(1);
    if pa == 0xDEAD {
        em = E::C;                // proves: ST-2 (SetDiscriminant), AG-4
    }
    let (tm, _pm) = e_tag_and_payload(&em);
    regseq::log::info!("em_tag={}", tm);                   // 2   proves: ST-2

    // --- CO-9 EnumValue const: read the const enum value's payload ---
    let (tk, pk) = e_tag_and_payload(&KONST);
    regseq::log::info!("k_tag={} k_pl={}", tk, pk);        // 0 7   proves: CO-9

    // --- (3) niche enum: construct both variants (AG-4) + probe (PL-8) ---
    let ns: N = N::Some(&GUARD);  // proves: LY-12, AG-4
    let nn: N = N::None;
    let (s_is, s_val) = n_probe(&ns);
    let (n_is, n_val) = n_probe(&nn);
    regseq::log::info!("ns_is={} ns_val={}", s_is, s_val); // 1 43981  proves: LY-12, PL-8
    regseq::log::info!("nn_is={} nn_val={}", n_is, n_val); // 0 0      proves: LY-12

    // --- (4) empty enum LY-7: contain a `*const Void` (structural; emits the
    // Void form + its guards into golden.c). Observe the null-pointer field
    // relationally (never log a raw address). ---
    let vr = VoidRef { p: 0usize as *const Void };
    regseq::log::info!("void_null={}", (vr.p as usize == 0) as u8);  // 1  proves: LY-7 (structural)
}
