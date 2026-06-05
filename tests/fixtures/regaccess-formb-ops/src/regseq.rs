//! Minimal self-contained Form-B facade: wrapper-receiver ops reached via
//! `Mmio<T>: Deref`. Hand-written (no `register_block!` macro) so the all-ops
//! coverage is independent of the macro path. Exercises all five register ops
//! (read/write whole-register raw, read_field/write_field field, modify
//! field-RMW); descriptors carry only bit `mask`/`shift`/`value` (NO register
//! offset — the offset comes from the receiver wrapper's `#[repr(C)]` position
//! in the block, recovered by the backend via Strategy C).

use core::marker::PhantomData;
use core::ops::Deref;

// -- bit-level descriptors (register-relative; no register byte offset) --------

/// Field (bitfield) descriptor: `mask` + `shift` within the register.
#[derive(Clone, Copy)]
pub struct Field<R> {
    pub mask: u32,
    pub shift: u32,
    _r: PhantomData<R>,
}
impl<R> Field<R> {
    pub const fn new(mask: u32, shift: u32) -> Self {
        Field {
            mask,
            shift,
            _r: PhantomData,
        }
    }
}

/// Field write value: `mask` + composed `value` within the register.
#[derive(Clone, Copy)]
pub struct FieldValue<R> {
    pub mask: u32,
    pub value: u32,
    _r: PhantomData<R>,
}
impl<R> FieldValue<R> {
    pub const fn new(mask: u32, value: u32) -> Self {
        FieldValue {
            mask,
            value,
            _r: PhantomData,
        }
    }
}

// -- register wrappers (non-ZST so `#[repr(C)]` gives distinct byte offsets) ---

/// Read-write register wrapper.
#[repr(transparent)]
pub struct ReadWrite<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
impl<R> ReadWrite<u32, R> {
    pub const fn new() -> Self {
        ReadWrite {
            _v: 0,
            _r: PhantomData,
        }
    }
    /// Whole-register raw read.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read")]
    pub fn read(&self) -> u32 {
        loop {}
    }
    /// Masked + shifted field read.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read_field")]
    pub fn read_field(&self, _f: Field<R>) -> u32 {
        loop {}
    }
    /// Whole-register raw write.
    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _val: u32) {}
    /// Masked field write (bits outside the mask = 0).
    #[inline(never)]
    #[rseq::intrinsic("mmio_write_field")]
    pub fn write_field(&self, _fv: FieldValue<R>) {}
    /// Read-modify-write a field (preserves unmentioned bits).
    #[inline(never)]
    #[rseq::intrinsic("mmio_modify")]
    pub fn modify(&self, _fv: FieldValue<R>) {}
}

/// Read-only register wrapper (no write/modify methods — legality by type).
#[repr(transparent)]
pub struct ReadOnly<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
impl<R> ReadOnly<u32, R> {
    pub const fn new() -> Self {
        ReadOnly {
            _v: 0,
            _r: PhantomData,
        }
    }
    #[inline(never)]
    #[rseq::intrinsic("mmio_read")]
    pub fn read(&self) -> u32 {
        loop {}
    }
    #[inline(never)]
    #[rseq::intrinsic("mmio_read_field")]
    pub fn read_field(&self, _f: Field<R>) -> u32 {
        loop {}
    }
}

pub enum AReg {}
pub enum DrReg {}
pub enum FrReg {}

/// `#[repr(C)]` register block: `a` @0x0, `dr` @0x4, `fr` @0x8.
#[repr(C)]
pub struct Regs {
    pub a: ReadWrite<u32, AReg>,
    pub dr: ReadWrite<u32, DrReg>,
    pub fr: ReadOnly<u32, FrReg>,
}
impl Regs {
    pub const fn new() -> Self {
        Regs {
            a: ReadWrite::new(),
            dr: ReadWrite::new(),
            fr: ReadOnly::new(),
        }
    }
}

/// Typed MMIO resource; derefs to the block for field-access.
pub struct Mmio<T> {
    inner: T,
}
impl<T> Deref for Mmio<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}
impl Mmio<Regs> {
    pub const fn new() -> Self {
        Mmio { inner: Regs::new() }
    }
}
