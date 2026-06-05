//! Form-B register-access facade: wrapper-receiver ops via `Mmio<T>: Deref`.
//!
//! The register is the *receiver* (`regs.dr`); the backend recovers its byte
//! offset + access width from the `#[repr(C)]` block layout (Strategy C), so the
//! `Field`/`FieldValue` descriptors carry only register-relative bit
//! `mask`/`shift`/`value`. Access legality is by wrapper type: a `ReadOnly`
//! register simply has no `write`/`write_field`/`modify` method.
//!
//! The wrappers/descriptors are generic over the storage width `W` (u8/u16/u32)
//! to match every `access_width` the `register_block!` macro accepts. The block
//! instance is a layout carrier only (its storage is never read — every access
//! lowers to a device-model hook). `new()` uses a concrete-width literal `0`
//! (one impl per width) rather than `W::default()`: a generic `Default::default`
//! / `BitOr::bitor` lowers to a core-trait method the generic-c backend declares
//! but does not define (the same reason `modify` is a C-emitted intrinsic), so
//! the construction + `+`-compose paths stay on native, body-available ops.

use core::marker::PhantomData;
use core::ops::{Add, Deref};

pub use regseq_macros::register_block;

// -- bit-level descriptors (no register byte offset) ---------------------------

/// Field (bitfield) descriptor: register-relative `mask` + `shift`.
#[derive(Clone, Copy)]
pub struct Field<W, R> {
    pub mask: W,
    pub shift: W,
    _r: PhantomData<R>,
}
impl<W, R> Field<W, R> {
    pub const fn new(mask: W, shift: W) -> Self {
        Self {
            mask,
            shift,
            _r: PhantomData,
        }
    }
}

/// Field write value: register-relative `mask` + composed `value`. Composes via `+`.
#[derive(Clone, Copy)]
pub struct FieldValue<W, R> {
    pub mask: W,
    pub value: W,
    _r: PhantomData<R>,
}
impl<W, R> FieldValue<W, R> {
    pub const fn new(mask: W, value: W) -> Self {
        Self {
            mask,
            value,
            _r: PhantomData,
        }
    }
}
// Concrete-`u32` so the `|` lowers to a native C bit-or (a generic `W: BitOr`
// would lower to an undefined core-trait method). u8/u16 compose can add their
// own impls when a fixture needs them.
impl<R> Add for FieldValue<u32, R> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        // Same register; OR the masks and the values.
        Self {
            mask: self.mask | rhs.mask,
            value: self.value | rhs.value,
            _r: PhantomData,
        }
    }
}

// -- register wrappers: non-ZST storage so `#[repr(C)]` gives byte offsets ------
//    Access legality = method availability per wrapper type.

/// Read-only register wrapper.
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
    /// Masked + shifted field read.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read_field")]
    pub fn read_field(&self, _f: Field<W, R>) -> W {
        loop {}
    }
}

/// Write-only register wrapper.
#[repr(transparent)]
pub struct WriteOnly<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
impl<W, R> WriteOnly<W, R> {
    /// Whole-register raw write.
    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _val: W) {}
    /// Masked field write (bits outside the mask = 0).
    #[inline(never)]
    #[rseq::intrinsic("mmio_write_field")]
    pub fn write_field(&self, _fv: FieldValue<W, R>) {}
}

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
    /// Masked + shifted field read.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read_field")]
    pub fn read_field(&self, _f: Field<W, R>) -> W {
        loop {}
    }
    /// Whole-register raw write.
    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _val: W) {}
    /// Masked field write (bits outside the mask = 0).
    #[inline(never)]
    #[rseq::intrinsic("mmio_write_field")]
    pub fn write_field(&self, _fv: FieldValue<W, R>) {}
    /// Read-modify-write a field (preserves unmentioned bits).
    #[inline(never)]
    #[rseq::intrinsic("mmio_modify")]
    pub fn modify(&self, _fv: FieldValue<W, R>) {}
}

/// Reserved gap in a register block (`#[repr(C)]` spacer).
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Reserved<W> {
    _v: W,
}

// Manual Clone/Copy bounded on `W: Copy` only — a `#[derive]` would wrongly
// require the marker `R: Copy` (it appears only in PhantomData). Copy lets a
// register array initialize as `[Wrapper::new(); N]` (a runtime Repeat, not a
// const) and lets a struct-array element derive Copy.
impl<W: Copy, R> Clone for ReadOnly<W, R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<W: Copy, R> Copy for ReadOnly<W, R> {}
impl<W: Copy, R> Clone for WriteOnly<W, R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<W: Copy, R> Copy for WriteOnly<W, R> {}
impl<W: Copy, R> Clone for ReadWrite<W, R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<W: Copy, R> Copy for ReadWrite<W, R> {}

// Per-width `new()` constructors (literal `0` → native, body-available; a
// generic `W::default()` would lower to an undefined core-trait method). Covers
// every width the `register_block!` macro accepts.
macro_rules! impl_layout_carrier_new {
    ($($w:ty),+ $(,)?) => {
        $(
            impl<R> ReadOnly<$w, R> {
                pub const fn new() -> Self { Self { _v: 0, _r: PhantomData } }
            }
            impl<R> WriteOnly<$w, R> {
                pub const fn new() -> Self { Self { _v: 0, _r: PhantomData } }
            }
            impl<R> ReadWrite<$w, R> {
                pub const fn new() -> Self { Self { _v: 0, _r: PhantomData } }
            }
            impl Reserved<$w> {
                pub const fn new() -> Self { Self { _v: 0 } }
            }
        )+
    };
}
impl_layout_carrier_new!(u8, u16, u32);

// -- the typed MMIO resource ---------------------------------------------------

/// Typed MMIO resource. Holds the register-block layout carrier and derefs to it
/// so `regs.<reg>` selects a register by field access. (A real target binds a
/// base at `map_region`; this host proof uses a global device-model register
/// file, so the carrier's storage is never read.)
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
