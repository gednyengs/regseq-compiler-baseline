// Peripheral-clock-management AUTHOR surface — shared across the `_clock/*`
// fixtures. Pulls together the minimal register-access Form-B surface (so a
// clock fixture can do one anchoring register write) and the typed `Clock`
// resource facade. Declared **inline per fixture** (the CR-2 discipline the
// lifecycle/interrupt surfaces established): identity is the attribute marker.
// Each `_clock/*` fixture pulls this in with
//   include!("../../_clock_surface.rs");
// alongside `_lifecycle_surface.rs` (which provides `RegseqError` + `Lifecycle`).
// The including `input.rs` owns the crate attrs (`#![no_std]`,
// `#![feature(register_tool)]`, `#![register_tool(rseq)]`).

// ---------------------------------------------------------------------------
// Register-access author surface (Form B), trimmed to the raw `write`/`read`
// used by the clock fixtures (mirrors the proven `_irq_pl011_surface.rs`
// primitives). One anchoring register write lets the oracle assert that
// `CLK enable` precedes the first `W…` (a fixture/oracle assertion, C05/D11).
// ---------------------------------------------------------------------------
use core::marker::PhantomData;
use core::ops::Deref;

/// Read-write register wrapper.
#[repr(transparent)]
pub struct ReadWrite<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
impl<W, R> ReadWrite<W, R> {
    /// Whole-register raw read.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read")]
    pub fn read(&self) -> W {
        loop {}
    }
    /// Whole-register raw write.
    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _val: W) {}
}
impl<R> ReadWrite<u32, R> {
    pub const fn new() -> Self {
        Self {
            _v: 0,
            _r: PhantomData,
        }
    }
}

/// Typed MMIO resource: the register-block layout carrier.
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

// ---------------------------------------------------------------------------
// A minimal PL011-shaped register block: just the Data Register at 0x000, the
// single write the clock fixtures anchor against. (The full PL011 block lives
// in `_irq_pl011_surface.rs`; clock needs only one register.)
// ---------------------------------------------------------------------------
pub mod pl011 {
    use super::ReadWrite;

    pub mod dr {
        pub enum Register {}
    }

    /// The block: the Data Register at the real PL011 offset 0x000.
    #[repr(C)]
    pub struct Regs {
        /// 0x000 — Data Register.
        pub dr: ReadWrite<u32, dr::Register>,
    }
}

const _: () = assert!(core::mem::offset_of!(pl011::Regs, dr) == 0x000);

// ---------------------------------------------------------------------------
// The typed `Clock` resource facade (peripheral-clock-management construct).
// `Clock` is detected by type NAME (matching the `Mmio`/`Interrupt` anchors);
// its `#[rseq::abstraction("clock")]` tag is documentation. The author declares
// the field with `#[rseq::clock(id = "…")]` (the `id` joins the platform file's
// `[clock.<id>]` section, C01) and calls `enable`/`disable`/`get_rate`; the
// compiler-managed `acquire_clock`/`release` are synthesized into the lifecycle
// preamble/postamble, never author-called (D05).
// ---------------------------------------------------------------------------
#[rseq::abstraction("clock")]
pub struct Clock {
    _opaque: (),
}

impl Clock {
    /// Enable the peripheral clock. Fallible (`clk_prepare_enable` can fail on a
    /// real backend; the generic-c host clock never fails).
    #[inline(never)]
    #[rseq::intrinsic("clock_enable")]
    pub fn enable(&self) -> Result<(), RegseqError> {
        Ok(())
    }

    /// Disable the peripheral clock. Infallible on the stable facade surface.
    #[inline(never)]
    #[rseq::intrinsic("clock_disable")]
    pub fn disable(&self) {}

    /// Query the clock rate in Hz.
    #[inline(never)]
    #[rseq::intrinsic("clock_get_rate")]
    pub fn get_rate(&self) -> u32 {
        0
    }
}
