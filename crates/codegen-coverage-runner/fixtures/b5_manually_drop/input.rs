#![no_std]
#![feature(register_tool)]
#![feature(core_intrinsics)]
#![register_tool(rseq)]

// B5 — IT-13 (is_manually_drop): a `core::mem::ManuallyDrop<D>` field is NOT
// dropped by the containing type's drop glue. This fixture OBSERVES the
// suppression: a `Hold { md: ManuallyDrop<D> }` is constructed + dropped in an
// inner scope and D::drop does NOT run; a plain `D` CONTROL in a sibling scope
// DOES run its Drop. The two sentinels prove the difference. Each info! line
// drives a value THROUGH the construct under test and logs the result (the dual
// oracle then proves "compiled + printed the right value"). `// proves:` maps
// each line to the Doc-1 matrix row it drives.
//
// TUPLE-STRUCT FIELD NAMING (gap 1, the compile prerequisite): `ManuallyDrop<T>`
// is `{ value: MaybeDangling<T> }` and `MaybeDangling<P>(P)` is a TUPLE STRUCT.
// The bridge interns the tuple-struct field with rustc's raw numeric index name
// (`0`), which is not a valid C identifier; the backend now emits it as `_0`
// (the same spelling tuples use) in the member declaration, `.field`
// projection, and the offsetof `_Static_assert` guard (regseq-backends-22).
// Without that fix any `ManuallyDrop` use fails to compile.

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

// A named-field type with a Drop impl whose body IS in the fixture. Its drop
// records the payload into a static, so "did D::drop run" is observable.
struct D { v: u32 }

// Two sentinels, distinguishable so a stray drop is visible. SUPPRESSED stays
// at its sentinel (0xFF) when D::drop does NOT run for the ManuallyDrop field;
// CONTROL is overwritten by the payload of the plain-D drop.
static mut SUPPRESSED: u32 = 0xFF;
static mut CONTROL: u32 = 0;

impl Drop for D {
    fn drop(&mut self) {
        // Record into whichever cell the active scope selects. The dispatch is
        // by payload value so we do not need a second Drop type.
        unsafe {
            if self.v == 0xAA {
                SUPPRESSED = self.v; // would only fire if the ManuallyDrop field WAS dropped
            } else {
                CONTROL = self.v; // the plain-D control DOES drop
            }
        }
    }
}

// Hold wraps a D in `core::mem::ManuallyDrop`, so Hold's drop glue must NOT run
// D::drop for the `md` field. `MaybeDangling<D>` (ManuallyDrop's inner tuple
// struct) is what exercises gap 1's tuple-struct field naming. A second scalar
// field (`marker`) keeps Hold's ABI a real aggregate, so the const below
// materialises as an aggregate `Const::Bytes` (the path the backend serializes)
// rather than a scalar-int valtree the bridge does not lower for an aggregate.
struct Hold {
    md: core::mem::ManuallyDrop<D>,
    marker: u32,
}

// `ManuallyDrop::new` is a `const fn`; constructing the Hold in a `const`
// const-evaluates it (rustc materialises a `Const`, so the emitted body needs
// no call to the upstream `core` inline-always `::new` -- which the backend does
// not have a body for). Binding the const to a runtime local still gives Hold's
// drop glue a runtime drop site to (not) run at scope exit, so the suppression
// is observed by the SAME path as a normally-constructed value.
const HOLD_INIT: Hold = Hold {
    md: core::mem::ManuallyDrop::new(D { v: 0xAA }),
    marker: 7,
};

#[no_mangle]
pub extern "C" fn regseq_main() {
    // --- IT-13 ManuallyDrop suppression: bind + drop a Hold whose D has payload
    // 0xAA. If the field were dropped, D::drop would write 0xAA into SUPPRESSED;
    // because ManuallyDrop suppresses it (rustc's drop elaboration omits the
    // md-field drop, so Hold has no drop glue reaching D::drop), SUPPRESSED stays
    // at its 0xFF sentinel. ---
    let marker;
    {
        let h: Hold = HOLD_INIT;
        marker = h.marker; // read the aggregate field (drives the Hold place)
    } // h leaves scope here -> Hold's glue does NOT drop md -> SUPPRESSED unchanged
    regseq::log::info!("marker={}", marker); // 7   proves: aggregate field read
    regseq::log::info!("suppressed={}", unsafe { SUPPRESSED }); // 255 proves: IT-13

    // --- CONTROL: a plain D (payload 0x7B) in a sibling scope DOES drop, so its
    // Drop runs and writes 0x7B into CONTROL. This proves the Drop impl is live
    // (the suppression above is real, not a dead/never-emitted Drop). ---
    {
        let _d = D { v: 0x7B };
    } // _d dropped here -> D::drop runs -> CONTROL = 123
    regseq::log::info!("control={}", unsafe { CONTROL }); // 123 proves: TM-10 control
}
