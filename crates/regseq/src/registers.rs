/// Register support types and typed access wrappers for the Register Access surface.
///
/// These types form the compile-time facade for authored register operations.
/// Wrapper methods have placeholder bodies; the compiler replaces them with
/// backend-specific MMIO reads/writes during lowering.
use core::marker::PhantomData;
use core::ops::Add;

// ---------------------------------------------------------------------------
// Field descriptor
// ---------------------------------------------------------------------------

/// Const-constructible field descriptor identifying a bitfield within a register.
///
/// `T` is the register width type (e.g. `u32`), `R` is the register marker type.
#[derive(Debug, Clone, Copy)]
pub struct Field<T, R> {
    pub mask: T,
    pub shift: T,
    _reg: PhantomData<R>,
}

impl<T, R> Field<T, R> {
    /// Create a new field descriptor with the given mask and shift.
    pub const fn new(mask: T, shift: T) -> Self {
        Self {
            mask,
            shift,
            _reg: PhantomData,
        }
    }
}

// ---------------------------------------------------------------------------
// Field value carrier
// ---------------------------------------------------------------------------

/// Const-constructible write-value carrier for register operations.
///
/// `T` is the register width type (e.g. `u32`), `R` is the register marker type.
/// Supports composition via `Add` so authored code like
/// `LCRH_REG::WLEN::WLEN_8 + LCRH_REG::FEN::SET` type-checks.
#[derive(Debug, Clone, Copy)]
pub struct FieldValue<T, R> {
    pub mask: T,
    pub value: T,
    _reg: PhantomData<R>,
}

impl<T, R> FieldValue<T, R> {
    /// Create a new field value with the given mask and value.
    pub const fn new(mask: T, value: T) -> Self {
        Self {
            mask,
            value,
            _reg: PhantomData,
        }
    }
}

/// Composition for `FieldValue`: combines two field values by ORing their masks and values.
impl<R> Add for FieldValue<u32, R> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            mask: self.mask | rhs.mask,
            value: self.value | rhs.value,
            _reg: PhantomData,
        }
    }
}

// ---------------------------------------------------------------------------
// Register wrapper types
// ---------------------------------------------------------------------------

/// Read-only register wrapper.
///
/// Layout-sized so `#[repr(C)]` register blocks stay mechanically coherent.
#[repr(transparent)]
#[derive(Debug)]
pub struct ReadOnly<T, R> {
    value: T,
    _reg: PhantomData<R>,
}

impl<T: Copy, R> ReadOnly<T, R> {
    /// Read the full raw register value in one hardware operation.
    pub fn read_raw(&self) -> T {
        unimplemented!("facade stub: lowered by the compiler backend")
    }

    /// Read the value of the given field from this register.
    pub fn read(&self, _field: Field<T, R>) -> T {
        unimplemented!("facade stub: lowered by the compiler backend")
    }
}

/// Write-only register wrapper.
///
/// Layout-sized so `#[repr(C)]` register blocks stay mechanically coherent.
#[repr(transparent)]
#[derive(Debug)]
pub struct WriteOnly<T, R> {
    value: T,
    _reg: PhantomData<R>,
}

impl<T: Copy, R> WriteOnly<T, R> {
    /// Write the given field values to this register.
    pub fn write(&self, _values: FieldValue<T, R>) {
        unimplemented!("facade stub: lowered by the compiler backend")
    }

    /// Set the given field values in this register (alias for write).
    pub fn set(&self, _values: FieldValue<T, R>) {
        unimplemented!("facade stub: lowered by the compiler backend")
    }
}

/// Read-write register wrapper.
///
/// Layout-sized so `#[repr(C)]` register blocks stay mechanically coherent.
#[repr(transparent)]
#[derive(Debug)]
pub struct ReadWrite<T, R> {
    value: T,
    _reg: PhantomData<R>,
}

impl<T: Copy, R> ReadWrite<T, R> {
    /// Read the full raw register value in one hardware operation.
    pub fn read_raw(&self) -> T {
        unimplemented!("facade stub: lowered by the compiler backend")
    }

    /// Read the value of the given field from this register.
    pub fn read(&self, _field: Field<T, R>) -> T {
        unimplemented!("facade stub: lowered by the compiler backend")
    }

    /// Write the given field values to this register.
    pub fn write(&self, _values: FieldValue<T, R>) {
        unimplemented!("facade stub: lowered by the compiler backend")
    }

    /// Modify: read-modify-write with the given field values.
    pub fn modify(&self, _values: FieldValue<T, R>) {
        unimplemented!("facade stub: lowered by the compiler backend")
    }

    /// Set the given field values in this register (alias for write).
    pub fn set(&self, _values: FieldValue<T, R>) {
        unimplemented!("facade stub: lowered by the compiler backend")
    }
}

// ---------------------------------------------------------------------------
// Reserved placeholder
// ---------------------------------------------------------------------------

/// Reserved register slot placeholder.
///
/// Layout-sized so it occupies the correct space in `#[repr(C)]` register blocks.
#[repr(transparent)]
#[derive(Debug)]
pub struct Reserved<T> {
    _value: T,
}
