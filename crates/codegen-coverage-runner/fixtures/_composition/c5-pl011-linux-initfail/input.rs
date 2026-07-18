#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");

pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                Str(&'a str),
            }
            #[inline(never)]
            #[rseq::intrinsic("log-error")]
            pub fn log_error(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }
        macro_rules! error { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_error($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use error;
    }
}

#[rseq::driver(name = "pl011_initfail", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,c5-pl011"])]
pub struct Pl011InitFail {}

impl Lifecycle for Pl011InitFail {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        regseq::log::error!("pl011 init-fail witness: refusing to start");
        Err(RegseqError::DeviceBusy)
    }
}
