#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The INDUCED-FAILURE E1000 variant (backend-linux-c-e1000 T08, GRILL D07):
// the authored-Err witness for the error machinery's first runtime exercise.
// `on_init` reads STATUS — a register the QEMU 82540EM models
// deterministically (hw/net/e1000.c mac_reg_init: nonzero reset value;
// the host oracle's regfile reads 0) — and returns `Err(HardwareError)` on a
// DELIBERATELY wrong expectation (0xDEADBEEF, true in neither world), so the
// probe fails with an unambiguous negated errno in dmesg:
//   authored Err -> on_init_failure hook -> -EIO (C02 row).
// The driver is named `e1000fail` so its module can coexist with the
// composed `e1000` module in the campaign VM (S17 is handled operationally,
// but two loaded modules may not share one module name).
//
// Native-preamble fault injection stays out of scope (D07): the failure is
// authored, deterministic by construction.

include!("../../_lifecycle_surface.rs");

// ---------------------------------------------------------------------------
// Diagnostics-logging author surface (the canonical intrinsic anchors + the
// namespaced macros) — mirrors `e1000-composed`.
// ---------------------------------------------------------------------------
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
            #[inline(never)] #[rseq::intrinsic("log-error")]
            pub fn log_error(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }
        macro_rules! debug { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_debug($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use debug;
        macro_rules! error { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_error($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use error;
    }
}

// ---------------------------------------------------------------------------
// Register-access author surface (Form B), trimmed to the one read this
// variant performs.
// ---------------------------------------------------------------------------
use core::marker::PhantomData;
use core::ops::Deref;

/// Read-only register wrapper. `#[repr(transparent)]` over the storage width
/// so `#[repr(C)]` gives byte offsets.
#[repr(transparent)]
pub struct ReadOnly<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
impl<W, R> ReadOnly<W, R> {
    /// Whole-register raw read.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read")]
    pub fn read(&self) -> W {
        loop {}
    }
}
impl<R> ReadOnly<u32, R> {
    pub const fn new() -> Self {
        Self { _v: 0, _r: PhantomData }
    }
}

/// Reserved gap in a register block (`#[repr(C)]` spacer).
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Reserved<W> {
    _v: W,
}
impl Reserved<u32> {
    pub const fn new() -> Self {
        Self { _v: 0 }
    }
}

/// Typed MMIO resource carrier (see `e1000-composed`).
pub struct Mmio<T> {
    inner: T,
}
impl<T> Mmio<T> {
    pub const fn new(inner: T) -> Self {
        Mmio { inner }
    }
}
impl<T> Deref for Mmio<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

/// The block: STATUS only, at its real 82540EM offset (0x00008).
pub mod e1000 {
    use super::{ReadOnly, Reserved};

    pub mod status {
        pub enum Register {}
    }

    #[repr(C)]
    pub struct Regs {
        /// 0x00000..0x00008 — reserved (CTRL unused here).
        pub _reserved0: [Reserved<u32>; 2],
        /// 0x00008 — Device Status (RO).
        pub status: ReadOnly<u32, status::Register>,
    }
}

const _: () = assert!(core::mem::offset_of!(e1000::Regs, status) == 0x00008);

/// The deliberately wrong STATUS expectation (GRILL D07): QEMU's 82540EM
/// resets STATUS to a nonzero link-up word (hw/net/e1000.c mac_reg_init) and
/// the host oracle's regfile reads 0 — NEITHER equals this, so `on_init`
/// returns the authored Err deterministically in both worlds.
const IMPOSSIBLE_STATUS: u32 = 0xDEAD_BEEF;

// The CONVERGED match set (backend-zephyr-c-e1000 D01: the Pci row first,
// DT last): `regseq,e1000fail` mirrors the module name so both DUTs stay
// bindable side by side; each backend consumes its native entry and audits
// the foreign one (D03(a)).
#[rseq::device_class(network)]
#[rseq::driver(name = "e1000fail", bus = "pci", pci = [
    { vendor = 0x8086, device = 0x100e },
    ],
    compatible = ["regseq,e1000fail"]
)]
pub struct E1000InitFail {
    #[rseq::mmio(id = "regs", bar = 0)]
    regs: Mmio<e1000::Regs>,
}

impl Lifecycle for E1000InitFail {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        regseq::log::debug!("e1000fail probe start");
        // R32 @0x0008 — then the deliberately wrong expectation: the Err arm
        // is the path under test (authored Err -> on_init_failure -> -EIO).
        let status = self.regs.status.read();
        if status != IMPOSSIBLE_STATUS {
            regseq::log::error!("e1000fail: status {:#x} fails the impossible expectation", status);
            return Err(RegseqError::HardwareError);
        }
        Ok(())
    }

    fn on_init_failure(&mut self) -> Result<(), RegseqError> {
        // The D07 hook: visible in dmesg so T14 can witness it ran before
        // the negated errno surfaced.
        regseq::log::error!("e1000fail: init failed, cleaning up partial state");
        Ok(())
    }
}

// No `regseq_main`: the backend synthesizes the driving scaffold. On Linux
// the probe fails with -EIO (HardwareError C02 row, negated at the adapter)
// after the on_init_failure hook runs; devres unwinds the preamble.
