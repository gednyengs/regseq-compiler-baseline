//! Minimal register-access facade for the generic-c lowering proof.
//!
//! Form A: register operations are wrapper methods on `Mmio<Block>` taking a
//! `RegisterId` / `Field` / `FieldValue` token. Each token carries the
//! register `offset` + access `width` (bytes) — and, for fields, `mask`/`shift`
//! / `value` — as ordinary runtime fields the `register_block!` macro
//! populates, so the backend lowers an op to a host device-model hook by
//! reading those fields off the token (no type-level offset recovery).
//!
//! `Mmio<T>` is a zero-sized marker here: the host device model is a single
//! global register file, so the resource carries no base for this proof. (A
//! real target binds a base via the lifecycle/`map_region` seam; out of scope.)

use core::marker::PhantomData;
use core::ops::Add;

pub use regseq_macros::register_block;

// -- access-kind marker traits + types (access legality is by method bound) ---
pub trait ReadAccess {}
pub trait WriteAccess {}
pub struct ReadOnlyAccess;
impl ReadAccess for ReadOnlyAccess {}
pub struct WriteOnlyAccess;
impl WriteAccess for WriteOnlyAccess {}
pub struct ReadWriteAccess;
impl ReadAccess for ReadWriteAccess {}
impl WriteAccess for ReadWriteAccess {}

/// Register width marker (the integer types a register may use). Bit-ops are
/// done by the backend (in C) for `modify`, so no bit-op bounds are needed here.
pub trait RegWidth: Copy {}
impl RegWidth for u8 {}
impl RegWidth for u16 {}
impl RegWidth for u32 {}

/// Associates a register marker type with its block and access kind.
pub trait RegisterOf<Block> {
    type Access;
}

// -- descriptors (carry offset/width so lowering reads them off the value) ----

/// Full-register token.
pub struct RegisterId<W, R> {
    pub offset: u32,
    pub width: u32,
    _w: PhantomData<W>,
    _r: PhantomData<R>,
}
impl<W, R> RegisterId<W, R> {
    pub const fn new(offset: u32, width: u32) -> Self {
        Self {
            offset,
            width,
            _w: PhantomData,
            _r: PhantomData,
        }
    }
}
// Manual Clone/Copy so the marker `R` (a fieldless enum) need not be `Copy`.
impl<W, R> Clone for RegisterId<W, R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<W, R> Copy for RegisterId<W, R> {}

/// Field (bitfield) descriptor: register `offset`, field `mask`/`shift`, `width`.
#[derive(Clone, Copy)]
pub struct Field<W, R> {
    pub offset: u32,
    pub mask: W,
    pub shift: W,
    pub width: u32,
    _r: PhantomData<R>,
}
impl<W, R> Field<W, R> {
    pub const fn new(offset: u32, mask: W, shift: W, width: u32) -> Self {
        Self {
            offset,
            mask,
            shift,
            width,
            _r: PhantomData,
        }
    }
}

/// Field write value: register `offset`, `mask`+`value`, `width`. Composes via `+`.
#[derive(Clone, Copy)]
pub struct FieldValue<W, R> {
    pub offset: u32,
    pub mask: W,
    pub value: W,
    pub width: u32,
    _r: PhantomData<R>,
}
impl<W, R> FieldValue<W, R> {
    pub const fn new(offset: u32, mask: W, value: W, width: u32) -> Self {
        Self {
            offset,
            mask,
            value,
            width,
            _r: PhantomData,
        }
    }
}
impl<R> Add for FieldValue<u32, R> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        // Same register (same offset/width); OR the masks and values.
        Self {
            offset: self.offset,
            mask: self.mask | rhs.mask,
            value: self.value | rhs.value,
            width: self.width,
            _r: PhantomData,
        }
    }
}

// -- layout-only wrappers (the macro types the block-struct fields with these) -
#[repr(transparent)]
pub struct ReadOnly<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
#[repr(transparent)]
pub struct WriteOnly<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
#[repr(transparent)]
pub struct ReadWrite<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
#[repr(transparent)]
pub struct Reserved<W> {
    _v: W,
}

// -- the typed MMIO resource + its operations (compiler-lowered intrinsics) ---

/// Typed MMIO resource. Holds the bound region base (set by the compiler at
/// `map_region`). Unused in this host proof — the device model is a single
/// global register file — but a real (non-ZST) field is required so the resource
/// can live as a driver-struct field whose address is taken at a call site.
pub struct Mmio<T> {
    #[allow(dead_code)]
    base: usize,
    _t: PhantomData<T>,
}
impl<T> Mmio<T> {
    pub const fn new() -> Self {
        Self {
            base: 0,
            _t: PhantomData,
        }
    }
}

impl<Block> Mmio<Block> {
    /// Read a full register. Lowered to `regseq_mmio_read(offset, width)`.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read")]
    pub fn read<W: RegWidth, R: RegisterOf<Block>>(&self, _reg: RegisterId<W, R>) -> W
    where
        R::Access: ReadAccess,
    {
        loop {}
    }

    /// Read a masked/shifted field. Lowered to `(read(off,w) & mask) >> shift`.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read_field")]
    pub fn read_field<W: RegWidth, R: RegisterOf<Block>>(&self, _field: Field<W, R>) -> W
    where
        R::Access: ReadAccess,
    {
        loop {}
    }

    /// Write a full register value. Lowered to `regseq_mmio_write(off, w, val)`.
    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write<W: RegWidth, R: RegisterOf<Block>>(&self, _reg: RegisterId<W, R>, _val: W)
    where
        R::Access: WriteAccess,
    {
    }

    /// Write masked field(s). Lowered to `regseq_mmio_write(off, w, value)`.
    #[inline(never)]
    #[rseq::intrinsic("mmio_write_field")]
    pub fn write_field<W: RegWidth, R: RegisterOf<Block>>(&self, _values: FieldValue<W, R>)
    where
        R::Access: WriteAccess,
    {
    }

    /// Read-modify-write. Lowered to `read` + (clear mask, OR value) + `write`
    /// in the backend (C), so the masking needs no generic Rust bit-op lowering.
    #[inline(never)]
    #[rseq::intrinsic("mmio_modify")]
    pub fn modify<W: RegWidth, R: RegisterOf<Block>>(
        &self,
        _reg: RegisterId<W, R>,
        _values: FieldValue<W, R>,
    ) where
        R::Access: ReadAccess + WriteAccess,
    {
    }
}
