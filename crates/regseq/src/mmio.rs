/// Typed MMIO resource declaration surface.
///
/// `Mmio<T>` is the opaque facade type that driver authors use to declare
/// MMIO-backed register-block fields. The compiler extracts these declarations
/// and joins them with Layer 2 binding data during lowering.
///
/// No trait bound is placed on `T`; validity of `T` as a register block is a
/// compiler-side extraction check, not a facade trait contract.
use core::ops::Deref;

use crate::registers::{
    Field, FieldValue, ReadAccess, RegWidth, RegisterId, RegisterOf, WriteAccess,
};

#[rseq::abstraction("mmio")]
#[derive(Debug)]
pub struct Mmio<T> {
    inner: T,
}

impl<T> Mmio<T> {
    /// Create a new `Mmio<T>` wrapping the given value.
    ///
    /// This constructor exists for test and scaffolding use. Real driver
    /// instances are populated by compiler-generated init code.
    pub const fn new(inner: T) -> Self {
        Self { inner }
    }
}

// ---------------------------------------------------------------------------
// Register access methods
// ---------------------------------------------------------------------------

impl<Block> Mmio<Block> {
    /// Read the full raw register value.
    #[rseq::intrinsic("mmio_read")]
    pub fn read<W: RegWidth, R: RegisterOf<Block>>(&self, _reg: RegisterId<W, R>) -> W
    where
        R::Access: ReadAccess,
    {
        todo!("facade stub: lowered by the compiler backend")
    }

    /// Read a specific field (masked and shifted).
    #[rseq::intrinsic("mmio_read_field")]
    pub fn read_field<W: RegWidth, R: RegisterOf<Block>>(&self, _field: Field<W, R>) -> W
    where
        R::Access: ReadAccess,
    {
        todo!("facade stub: lowered by the compiler backend")
    }

    /// Write a full register value.
    #[rseq::intrinsic("mmio_write")]
    pub fn write<W: RegWidth, R: RegisterOf<Block>>(&self, _reg: RegisterId<W, R>, _value: W)
    where
        R::Access: WriteAccess,
    {
        todo!("facade stub: lowered by the compiler backend")
    }

    /// Write specific fields (masked).
    #[rseq::intrinsic("mmio_write_field")]
    pub fn write_field<W: RegWidth, R: RegisterOf<Block>>(&self, _values: FieldValue<W, R>)
    where
        R::Access: WriteAccess,
    {
        todo!("facade stub: lowered by the compiler backend")
    }

    /// Read-modify-write. NOT an intrinsic.
    ///
    /// Reads the current register value, applies the mask and value from the
    /// `FieldValue`, and writes back. Takes both a `RegisterId` (to identify
    /// which register to read/write) and a `FieldValue` (mask + value to apply).
    pub fn modify<W: RegWidth, R: RegisterOf<Block>>(
        &self,
        _reg: RegisterId<W, R>,
        _values: FieldValue<W, R>,
    ) where
        R::Access: ReadAccess + WriteAccess,
    {
        let _current: W = self.read(_reg);
        // real impl: self.write(_reg, (_current & !_values.mask) | _values.value)
        todo!("facade stub")
    }
}

impl<T> Deref for Mmio<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}
