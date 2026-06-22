// Pin-controller-and-GPIO-configuration AUTHOR surface — shared across the
// `_pinctrl/*` fixtures. Pulls together the minimal register-access Form-B
// surface (so a pinctrl fixture can do one anchoring register write) and the
// typed `PinCtrl` resource facade + `#[rseq::pin_state]` markers. Declared
// **inline per fixture** (the CR-2 discipline): identity is the attribute
// marker. Each `_pinctrl/*` fixture pulls this in with
//   include!("../../_pinctrl_surface.rs");
// alongside `_lifecycle_surface.rs` (which provides `RegseqError` + `Lifecycle`).
// The including `input.rs` owns the crate attrs (`#![no_std]`,
// `#![feature(register_tool)]`, `#![register_tool(rseq)]`).
//
// M1 builds the REQUIRED `PinCtrl` form (the unambiguous easy receiver, D13/D03):
// `apply::<S>()` is the inherent intrinsic on `&PinCtrl`, the exact clock-`enable`
// analog. The optional `Option<PinCtrl>` form + the `OptionalPinCtrl` extension +
// `RegseqError::PinctrlAbsent` + the compile-time absent fold are M3 (T14/T15/T16),
// where a pinctrl-specific lifecycle surface carries the extra error variant
// (the shared 2-variant `RegseqError` here keeps the M1 fixture golden-isolated).

// ---------------------------------------------------------------------------
// Register-access author surface (Form B), trimmed to the raw `write`/`read`
// used by the pinctrl fixtures (mirrors `_clock_surface.rs`). One anchoring
// register write lets the oracle assert that `PIN apply` precedes the first
// `W…` (a fixture/oracle assertion, C05/D11).
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
// single write the pinctrl fixtures anchor against.
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
// The typed `PinCtrl` resource facade (pin-controller-and-GPIO-configuration
// construct). `PinCtrl` is detected by type NAME (matching the `Clock`/`Mmio`
// anchors); its `#[rseq::abstraction("pinctrl")]` tag is documentation. The
// author declares the field with `#[rseq::pinctrl(id = "…")]` (the `id` joins the
// platform file's `[pinctrl.<id>]` section, C01) and calls `apply::<S>()`; the
// compiler-managed `acquire_pinctrl`/`release_pinctrl` are synthesized into the
// lifecycle preamble/postamble (pins-before-clock), never author-called (D05/D08).
// ---------------------------------------------------------------------------
#[rseq::abstraction("pinctrl")]
pub struct PinCtrl {
    _opaque: (),
}

/// Empty marker trait — a pin-state marker type implements it; its `id` is
/// carried by the `#[rseq::pin_state(id = "…")]` attribute (D04).
pub trait PinState {}

impl PinCtrl {
    /// Select a named pin state. The resolved state name is read at lowering
    /// from the generic arg `S`'s `#[rseq::pin_state]` marker (D04/C03).
    #[inline(never)]
    #[rseq::intrinsic("pinctrl_apply")]
    pub fn apply<S: PinState>(&self) -> Result<(), RegseqError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Optional-form forwarding (D02/C08). `Option` is a foreign type, so the
// forwarding `apply` lives on an EXTENSION TRAIT, not an inherent impl. The
// `#[rseq::intrinsic("pinctrl_apply")]` tag is on the CONCRETE
// `impl OptionalPinCtrl for Option<PinCtrl>` method (NOT the trait declaration):
// post-monomorphization the bridge canonicalizes the static trait call
// `self.pinctrl.apply::<S>()` to this resolved-impl `DefId`, so the concrete
// method's `IntrinsicKind::Pinctrl` is what the backend intercepts. Its `&self`
// receiver is `&Option<PinCtrl>` — a legal `pinctrl_apply` receiver (D14), so the
// M2 spoofed-receiver validator admits it unchanged. When the resource is PRESENT
// the call lowers to the bound `apply` hook; when ABSENT it compile-time-folds to
// a direct `Err(RegseqError::PinctrlAbsent)` (no hook, no `PIN apply` trace,
// D06/C04). The stub body is never emitted (the call site is intercepted) — it
// exists only so the surface type-checks; the absent fold is synthesized in the
// backend, so the body need not (and does not) reference `PinctrlAbsent`.
// ---------------------------------------------------------------------------
pub trait OptionalPinCtrl {
    /// Select a named pin state on an `Option<PinCtrl>`; returns
    /// `Err(RegseqError::PinctrlAbsent)` when the optional resource is absent.
    fn apply<S: PinState>(&self) -> Result<(), RegseqError>;
}

impl OptionalPinCtrl for Option<PinCtrl> {
    #[inline(never)]
    #[rseq::intrinsic("pinctrl_apply")]
    fn apply<S: PinState>(&self) -> Result<(), RegseqError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Author-declared pin-state markers (D04): unit structs carrying the state name
// via `#[rseq::pin_state(id = "…")]`; the empty `PinState` trait gates `apply`.
// ---------------------------------------------------------------------------
#[rseq::pin_state(id = "default")]
pub struct DefaultState;
impl PinState for DefaultState {}

#[rseq::pin_state(id = "sleep")]
pub struct SleepState;
impl PinState for SleepState {}
