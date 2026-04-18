// MIR Register Access Verification
//
// Verify at mir_drops_elaborated_and_const_checked with mir_opt_level=0:
//
//   1. Place expression visibility: does the MIR preserve the full
//      projection chain from driver struct -> transport -> register block
//      -> register field?
//
//   2. Wrapper inlining: is ReadOnly::read inlined, or does it appear
//      as a separate Call? If separate, is the body (containing the
//      intrinsic call) available?
//
//   3. Monomorphized types: are generic type parameters resolved so
//      the compiler knows concrete types at each call site?

// ---------------------------------------------------------------------------
// Minimal register access facade (mirrors regseq::registers)
// ---------------------------------------------------------------------------

use core::marker::PhantomData;

/// Register width trait.
pub trait RegWidth: Copy {}
impl RegWidth for u32 {}
impl RegWidth for u8 {}

/// Access permission traits.
pub trait Readable {}
pub trait Writable {}

/// A bitfield descriptor: mask + shift for a specific field.
#[derive(Clone, Copy)]
pub struct Field<T: RegWidth, R> {
    pub mask: T,
    pub shift: T,
    _reg: PhantomData<R>,
}

impl<T: RegWidth, R> Field<T, R> {
    pub const fn new(mask: T, shift: T) -> Self {
        Self { mask, shift, _reg: PhantomData }
    }
}

/// A field value carrier for writes.
#[derive(Clone, Copy)]
pub struct FieldValue<T: RegWidth, R> {
    pub mask: T,
    pub value: T,
    _reg: PhantomData<R>,
}

impl<T: RegWidth, R> FieldValue<T, R> {
    pub const fn new(mask: T, value: T) -> Self {
        Self { mask, value, _reg: PhantomData }
    }
}

/// Read-only register wrapper.
#[repr(transparent)]
pub struct ReadOnly<T: RegWidth, R> {
    value: T,
    _reg: PhantomData<R>,
}

/// Read-write register wrapper.
#[repr(transparent)]
pub struct ReadWrite<T: RegWidth, R> {
    value: T,
    _reg: PhantomData<R>,
}

// Trait impls for access control
impl<T: RegWidth, R> Readable for ReadOnly<T, R> {}
impl<T: RegWidth, R> Readable for ReadWrite<T, R> {}
impl<T: RegWidth, R> Writable for ReadWrite<T, R> {}

/// Reserved gap in register block.
#[repr(transparent)]
pub struct Reserved<T>(T);

// ---------------------------------------------------------------------------
// Register intrinsics (mirrors regseq::intrinsics::register_access)
// ---------------------------------------------------------------------------

#[inline(never)]
pub fn register_read<T: RegWidth, Reg: Readable>(
    _reg: &Reg,
) -> T {
    unimplemented!("facade stub: lowered by compiler")
}

#[inline(never)]
pub fn register_read_field<T: RegWidth, R, Reg: Readable>(
    _reg: &Reg,
    _field: Field<T, R>,
) -> T {
    unimplemented!("facade stub: lowered by compiler")
}

#[inline(never)]
pub fn register_write_field<T: RegWidth, R, Reg: Writable>(
    _reg: &Reg,
    _values: FieldValue<T, R>,
) {
    unimplemented!("facade stub: lowered by compiler")
}

// ---------------------------------------------------------------------------
// Wrapper methods (sugar that calls intrinsics)
// ---------------------------------------------------------------------------

impl<T: RegWidth, R> ReadOnly<T, R> {
    #[inline(always)]
    pub fn read_raw(&self) -> T {
        register_read(self)
    }

    #[inline(always)]
    pub fn read(&self, field: Field<T, R>) -> T {
        register_read_field(self, field)
    }
}

impl<T: RegWidth, R> ReadWrite<T, R> {
    #[inline(always)]
    pub fn read_raw(&self) -> T {
        register_read(self)
    }

    #[inline(always)]
    pub fn read(&self, field: Field<T, R>) -> T {
        register_read_field(self, field)
    }

    #[inline(always)]
    pub fn write(&self, values: FieldValue<T, R>) {
        register_write_field(self, values)
    }
}

// ---------------------------------------------------------------------------
// Register descriptors (simulates register_bitfields! output)
// ---------------------------------------------------------------------------

pub mod STATUS_REG {
    use super::*;
    pub enum Register {}
    pub const READY: Field<u32, Register> = Field::new(0x1, 0);
    pub const BUSY: Field<u32, Register> = Field::new(0x2, 1);
}

pub mod CONTROL_REG {
    use super::*;
    pub enum Register {}
    pub const ENABLE: Field<u32, Register> = Field::new(0x1, 0);
    pub mod ENABLE_VAL {
        use super::*;
        pub const ENABLED: FieldValue<u32, super::Register> = FieldValue::new(0x1, 0x1);
        pub const DISABLED: FieldValue<u32, super::Register> = FieldValue::new(0x1, 0x0);
    }
}

pub mod CAL_REG {
    use super::*;
    pub enum Register {}
    pub const VALUE: Field<u8, Register> = Field::new(0xFF, 0);
}

// ---------------------------------------------------------------------------
// Register blocks
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct MainRegs {
    pub status: ReadOnly<u32, STATUS_REG::Register>,
    pub control: ReadWrite<u32, CONTROL_REG::Register>,
    _reserved: [Reserved<u32>; 2],
}

#[repr(C)]
pub struct CalRegs {
    pub offset_val: ReadWrite<u8, CAL_REG::Register>,
}

// ---------------------------------------------------------------------------
// Transport wrappers (minimal, just enough for place expression testing)
// ---------------------------------------------------------------------------

/// MMIO transport wrapper.
#[repr(transparent)]
pub struct Mmio<T> {
    inner: T,
}

impl<T> core::ops::Deref for Mmio<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

/// I2C transport wrapper.
#[repr(transparent)]
pub struct I2c<T> {
    inner: T,
}

impl<T> core::ops::Deref for I2c<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

// ---------------------------------------------------------------------------
// Driver struct
// ---------------------------------------------------------------------------

pub struct DualSensorDriver {
    pub main: Mmio<MainRegs>,
    pub cal: I2c<CalRegs>,
}

// ---------------------------------------------------------------------------
// Test functions: what does the MIR look like for each pattern?
// ---------------------------------------------------------------------------

/// Test 1: Read a field through MMIO transport.
/// Question: does MIR show the full projection chain through Mmio<T>?
pub fn test_mmio_read_field(driver: &DualSensorDriver) -> u32 {
    driver.main.status.read(STATUS_REG::READY)
}

/// Test 2: Write a field through MMIO transport.
pub fn test_mmio_write_field(driver: &DualSensorDriver) {
    driver.main.control.write(CONTROL_REG::ENABLE_VAL::ENABLED);
}

/// Test 3: Read through I2C transport.
/// Question: same projection chain, different transport type?
pub fn test_i2c_read_raw(driver: &DualSensorDriver) -> u8 {
    driver.cal.offset_val.read_raw()
}

/// Test 4: Both transports in one function.
/// Question: can the compiler distinguish the two resource paths?
pub fn test_both_transports(driver: &DualSensorDriver) -> u32 {
    let status = driver.main.status.read(STATUS_REG::READY);
    let cal = driver.cal.offset_val.read_raw();
    status + cal as u32
}
