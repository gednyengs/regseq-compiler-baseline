#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// VTABLE — dyn trait-object dispatch: TY-14 (Ty::Dynamic fat pointer) + IN-6
// (FunctionInstanceData.virtual_idx, the vtable slot for the Virtual method
// shim). `&StaticByte as &dyn Reader` is an Unsize coercion that builds the
// section-3.2 fat pointer `{ void *data; const Reader_VTable *vtable }`; the
// `drive(&dyn Reader)` call dispatches through `d.vtable->read_byte(d.data)`
// (the bridge rewrote the virtual call to CallCallee::Indirect). The returned
// byte is logged, so the dual oracle proves "compiled + dispatched + returned
// the right value". `// proves:` maps each observed line to its matrix row(s).
//
// The vtable struct type + the `__vtable_StaticByte_for_Reader` instance static
// + the method shim are all bridge-synthesised; the backend emits them via the
// normal struct + static + body paths (07 plan D1/D2). The unsize site resolves
// the concrete impl's VTableInstance for the `.vtable` slot (07 plan D4).

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

// The trait + a concrete impl whose method body lives in the fixture (no
// undefined reference at link). `read_byte` returns an observable u8.
trait Reader {
    fn read_byte(&self) -> u8;
}

struct StaticByte {
    v: u8,
}

impl Reader for StaticByte {
    fn read_byte(&self) -> u8 {
        self.v
    }
}

// The dyn call site: `r` is a `&dyn Reader` fat pointer (TY-14); `r.read_byte()`
// dispatches through the vtable slot (IN-6 virtual_idx). `#[inline(never)]` keeps
// it a distinct instance so the indirect dispatch is genuinely emitted.
#[inline(never)]
fn drive(r: &dyn Reader) -> u8 {
    r.read_byte()
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let sb = StaticByte { v: 0x42 };
    let d: &dyn Reader = &sb;                 // Unsize: &StaticByte -> &dyn Reader (TY-14)
    regseq::log::info!("read={}", drive(d));  // 66  proves: TY-14 (dyn fat ptr), IN-6 (virtual dispatch)
}
