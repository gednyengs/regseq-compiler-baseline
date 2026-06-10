#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The init-fail VARIANT fixture (backend-zephyr-c T10, GRILL D14): the
// authored-`Err` live witness. `on_init` logs an authored LOG_ERR line and
// then deterministically returns `Err(DeviceBusy)`; the generated Zephyr
// init function must cross the negated-errno adapter (-EBUSY), leaving the
// device with `device_is_ready() == false` — asserted live by the T16 L2
// expected-failure leg.
//
// Deliberately MINIMAL and Zephyr-only (D14): a fieldless driver, no MMIO
// region, no on_deinit. This fixture is OUTSIDE the C02 identity invariant,
// gets NO generic_c oracle bless, and has NO L3 leg. It matches the same
// eval DUT compatible (`regseq,pl011`) so the T16 variant image binds the
// same DT node the composed driver does — under a DISTINCT driver name.

// The shared device-lifecycle author surface (the `Lifecycle` trait + the
// inline `RegseqError`).
include!("../../_lifecycle_surface.rs");

// Minimal diagnostics-logging author surface: only the error level is
// authored here.
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

#[rseq::driver(name = "pl011_initfail",
    compatible = ["regseq,pl011"],
)]
pub struct Pl011InitFail {}

impl Lifecycle for Pl011InitFail {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        regseq::log::error!("pl011 init-fail witness: refusing to start");
        Err(RegseqError::DeviceBusy)
    }
}
