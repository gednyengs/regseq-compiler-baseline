#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The OS-absent-slot witness fixture (os-capability-disposition T09, GRILL
// D13): authors `on_surprise_remove` with a LOG-ONLY body under a zephyr
// static-DT target. Zephyr's device model structurally cannot deliver a
// surprise removal (capability row CAP-ZEPHYR-DT-SURPRISE-1, AbsentDispatch),
// so zephyr_c must emit the TU successfully with exactly ONE
// `RSEQ-ABSENT-SLOT` audit block in the callback's place, a
// `conformance.zephyr.json` absent_dispatch row citing the same capability
// row, and NO `on_surprise_remove` symbol — while the body still runs the
// complete lowering pipeline in a throwaway context (D06).
//
// `on_init` is also authored (log-only) so the conformance report witnesses
// BOTH lists: delivered=[on_init] + absent_dispatch=[on_surprise_remove].
//
// Deliberately MINIMAL and zephyr-only (D13): a fieldless driver, no MMIO
// region; OUTSIDE the C02 identity invariant, NO generic_c oracle bless, NO
// L3 leg (the initfail-variant precedent). It matches the same eval DUT
// compatible (`regseq,pl011`) under a DISTINCT driver name.

// The shared device-lifecycle author surface (the `Lifecycle` trait + the
// inline `RegseqError`).
include!("../../_lifecycle_surface.rs");

// Minimal diagnostics-logging author surface: the info level for the
// delivered on_init body, the warn level for the absent surprise body.
pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                Str(&'a str),
            }
            #[inline(never)]
            #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)]
            #[rseq::intrinsic("log-warn")]
            pub fn log_warn(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }
        macro_rules! info { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_info($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        macro_rules! log_warn { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_warn($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use info;
        pub(crate) use log_warn as warn;
    }
}

#[rseq::driver(name = "pl011_absentslot",
    compatible = ["regseq,pl011"],
)]
pub struct Pl011AbsentSlot {}

impl Lifecycle for Pl011AbsentSlot {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        regseq::log::info!("pl011 absent-slot witness up");
        Ok(())
    }
    /// The OS-absent slot: legal log-only body — fully checked by the
    /// throwaway lowering, never delivered on this target.
    fn on_surprise_remove(&mut self) -> Result<(), RegseqError> {
        regseq::log::warn!("pl011 absent-slot witness: surprise removal");
        Ok(())
    }
}
