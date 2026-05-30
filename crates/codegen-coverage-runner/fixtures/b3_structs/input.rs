#![no_std]
#![feature(register_tool)]
#![feature(core_intrinsics)]
#![register_tool(rseq)]

// B3a — aggregates / places / layout / struct+union consts. Every info! line
// computes a value THROUGH the construct under test and logs it (dual oracle:
// byte-exact stdout + golden.c snapshot). The `// proves:` comment maps each
// line to the Doc-1 matrix row(s) it drives. LAYOUT rows (LY-1/2/3/4/6) are
// proven by COMPILATION: the backend emits a sizeof/_Alignof/offsetof
// _Static_assert for every aggregate it emits, so merely CONTAINING the type
// (reachable from regseq_main) lets cc enforce the certified offsets. Carrier
// note: the diag LogArg holds u8..u64/i8..i64/bool/&str only, so usize is cast
// to u64 and pointer relations are observed as booleans (never raw addresses).

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

// --- aggregate item definitions (IT-4/IT-6, contained → emitted with layout
// _Static_assert guards). Mixed field sizes so the repr(Rust) struct gets
// rustc-reordered offsets (LY-3); a #[repr(C)] struct with declared order
// (LY-1/LY-6); a union (IT-6/TY-19/LY-4); a struct holding UnsafeCell<u32>
// (IT-11) and one holding PhantomData<u32> (IT-12); a #[repr(packed)] struct
// (LY-2). ---

#[repr(C)]
struct PointC {            // proves: IT-4, TY-17, IN-9, LY-1, LY-6 (declared order, no reorder)
    a: u8,
    b: u64,
    c: u16,
}

// --- #[repr(packed)] struct (LY-2). With repr(C) + packed, the fields keep
// declared order but every field is packed to alignment 1, so `b` sits at
// offset 1 (not 8 as in PointC). The bridge propagates `Layout.pack = Some(1)`
// from rustc's repr; the validator then accepts the unaligned offset and the
// backend emits `__attribute__((packed, aligned(1)))` + offsetof asserts that
// cc enforces at compile time. ---
#[repr(C, packed)]
struct PackedRegs {        // proves: LY-2 (pack = Some(1): packed offsets enforced by cc)
    tag: u8,
    word: u32,
}

struct PointRust {         // proves: LY-3 (repr(Rust): rustc reorders u8,u64,u16 → backend by-offset)
    a: u8,
    b: u64,
    c: u16,
}

union Bits {               // proves: IT-6, TY-19, IN-11, LY-4 (all fields offset 0)
    u: u32,
    bytes: [u8; 4],
}

struct WithCell {          // proves: IT-11 (UnsafeCell<u32> field)
    cell: core::cell::UnsafeCell<u32>,
}

struct WithPhantom {       // proves: IT-12 (PhantomData<u32> field, omitted from emitted C)
    val: u32,
    _marker: core::marker::PhantomData<u32>,
}

struct Unit;               // proves: CO-6 (ZST unit struct)

#[inline(never)]
fn consume_unit(_z: ()) {}        // takes the () ZST as an operand (Const::ZST)
#[inline(never)]
fn consume_zst(_u: Unit) {}       // takes the unit-struct ZST as an operand

// --- a `static [u8; N]` byte array is a Const::Bytes (CO-7); the static-init
// path serializes EVERY non-scalar static (incl. structs) to Const::Bytes
// (const_.rs:307), so APOINT (a PointC struct static) is ALSO a Const::Bytes,
// not a Const::Aggregate. CO-8 (Const::Aggregate) is driven instead by an
// inline `const` of tuple type, whose valtree translates to Const::Aggregate
// over its element consts (const_.rs:61-68). ---
static BYTES: [u8; 4] = [10, 20, 30, 40];   // proves: CO-7 (Const::Bytes)
static APOINT: PointC = PointC { a: 1, b: 2, c: 3 }; // proves: CO-7 (struct static → Const::Bytes)
const WORDS: (u16, u16, u16) = (100, 200, 300); // proves: CO-8 (Const::Aggregate over a tuple valtree)

#[no_mangle]
pub extern "C" fn regseq_main() {
    // --- struct literal + field place (AG-3 / RV-9 / PL-3 / PL-1 / TY-17) ---
    let pc = PointC { a: 9, b: 100, c: 7 };          // proves: AG-3, RV-9, IN-9, TY-17
    regseq::log::info!("pc.a={} pc.b={} pc.c={}", pc.a, pc.b, pc.c); // 9 100 7  proves: PL-3 (Field), PL-1 (local)

    let pr = PointRust { a: 5, b: 6, c: 8 };          // proves: LY-3 (reordered offsets enforced by cc)
    regseq::log::info!("pr.a={} pr.b={} pr.c={}", pr.a, pr.b, pr.c); // 5 6 8

    // --- #[repr(packed)] struct (LY-2): construct and read both fields by copy
    // (a packed field cannot be borrowed). The packed offset of `word` (1, not
    // 4) is enforced by the backend's offsetof _Static_assert. ---
    let pk = PackedRegs { tag: 0xAB, word: 0x11223344 }; // proves: LY-2
    let pk_tag = pk.tag;                                  // copy out (no borrow of packed field)
    let pk_word = pk.word;
    regseq::log::info!("pk.tag={} pk.word={}", pk_tag, pk_word); // 171 287454020

    // --- union: write one field, read another (AG-5 / IN-11 / TY-19 / LY-4) ---
    let bits = Bits { u: 0x04030201 };                // proves: AG-5, TY-19, IN-11
    let lo = unsafe { bits.bytes[0] };                // proves: PL-4 (Index into union field's array), LY-4
    regseq::log::info!("bits.u_lo={}", lo);           // 1 (little-endian low byte of 0x04030201)

    // --- UnsafeCell field (IT-11): `UnsafeCell::new` is a core fn (undefined
    // ref at link, no inlining at mir_opt_level=0), and its inner field is
    // private so no struct literal. UnsafeCell<u32> is repr(transparent) over
    // u32, so we transmute a u32 into it (transmute is a direct intrinsic that
    // LowerIntrinsics turns into a bit-cast in this fixture's MIR). Read it back
    // via a raw pointer into the field. ---
    let cell: core::cell::UnsafeCell<u32> =
        unsafe { core::intrinsics::transmute::<u32, core::cell::UnsafeCell<u32>>(77) };
    let wc = WithCell { cell };                        // proves: IT-11
    let cellp = &raw const wc.cell as *const u32;
    let cellv = unsafe { *cellp };                     // read the stored u32 back
    regseq::log::info!("cell={}", cellv);              // 77

    // --- PhantomData field (IT-12): construct + read the real field ---
    let wp = WithPhantom { val: 88, _marker: core::marker::PhantomData }; // proves: IT-12
    regseq::log::info!("phantom.val={}", wp.val);     // 88

    // --- array aggregate + index place (AG-1 / TY-8 / PL-4 / PL-1) ---
    let arr: [u8; 3] = [1, 2, 3];                     // proves: AG-1, TY-8, RV-9
    let idx: usize = 2;
    regseq::log::info!("arr[2]={}", arr[idx]);        // 3   proves: PL-4 (Index local)
    // NOTE: a literal `arr[0]` does NOT lower to PlaceElem::ConstantIndex — rustc
    // emits a bounds-checked Index(local) with the index in a local (`_68=0;
    // _57[_68]`), the SAME op as PL-4. PL-5 (ConstantIndex from_end:false) only
    // arises from a slice-pattern binding, owned by b3_refs_slices (`first`).
    regseq::log::info!("arr[0]={}", arr[0]);          // 1   (Index, not ConstantIndex)

    // --- repeat array [x; N] (RV-2) ---
    let rep = [7u8; 4];                               // proves: RV-2 (Rvalue::Repeat)
    regseq::log::info!("rep[3]={}", rep[3]);          // 7

    // --- tuple aggregate + field place (AG-2 / TY-16 / PL-3) ---
    let tup: (u8, u16, u32) = (1, 2, 3);              // proves: AG-2, TY-16, RV-9
    regseq::log::info!("tup.0={} tup.1={} tup.2={}", tup.0, tup.1, tup.2); // 1 2 3  proves: PL-3 (tuple field)

    // --- AG-6 (Closure aggregate) / TY-15 (Closure type) are DEFERRED. Any
    // closure present in the model makes the bridge synthesize its Fn-family
    // shims (closure.rs:133 — a `Fn` closure gets call/call_mut/call_once), and
    // the backend's `call_once`-by-value shim emits `_3 = (*_1)._0` treating the
    // by-value closure receiver `_1` as a pointer → cc rejects "invalid type
    // argument of unary '*' (have struct ...closure_0...)". The shim is emitted
    // even if the closure is never CALLED (all three Fn impls are reachable from
    // the closure type), so there is no clean surface — a backend bug in the
    // by-value-receiver closure shim lowering. ---

    // --- ZST const (CO-6): a unit value `()` and a unit struct. Constructing
    // `()` materializes a Const::ZST in MIR; a unit struct `Unit` is a ZST
    // aggregate too. CONTAINING them reaches CO-6 (no value to print — observe
    // with a marker boolean that the surrounding code ran). ---
    let z: () = ();                                   // proves: CO-6 (Const::ZST, unit)
    let zu = Unit;                                    // proves: CO-6 (ZST unit struct)
    consume_unit(z);                                  // pass the () ZST through a call (Const::ZST operand)
    consume_zst(zu);
    regseq::log::info!("zst_ok={}", 1u8);             // 1

    // --- byte/struct statics → Const::Bytes (CO-7) ---
    regseq::log::info!("bytes[1]={}", BYTES[1]);      // 20  proves: CO-7 (Const::Bytes static, [u8;4])
    regseq::log::info!("apoint.b={}", APOINT.b);      // 2   proves: CO-7 (struct static → Const::Bytes)

    // --- inline `const (u16,u16,u16)` → Const::Aggregate (CO-8). Reading WORDS.1
    // materializes the aggregate const; its tuple valtree is translated as
    // Const::Aggregate over the three element consts (emitted as a struct
    // compound literal `{ ._0=100, ._1=200, ._2=300 }`). ---
    regseq::log::info!("words1={}", WORDS.1);         // 200 proves: CO-8 (Const::Aggregate)
}
