pub use exp01_regseq_macros::register_bitfields;

use core::marker::PhantomData;
use core::ops::{Add, Deref};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field<T, R> {
    offset: u8,
    width: u8,
    _marker: PhantomData<(T, R)>,
}

impl<T, R> Field<T, R> {
    pub const fn new(offset: u8, width: u8) -> Self {
        Self {
            offset,
            width,
            _marker: PhantomData,
        }
    }

    pub const fn val(self, value: T) -> FieldValue<T, R> {
        FieldValue::new(self.offset, self.width, value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldValue<T, R> {
    offset: u8,
    width: u8,
    value: T,
    _marker: PhantomData<R>,
}

impl<T, R> FieldValue<T, R> {
    pub const fn new(offset: u8, width: u8, value: T) -> Self {
        Self {
            offset,
            width,
            value,
            _marker: PhantomData,
        }
    }
}

impl<R> Add for FieldValue<u32, R> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let lhs_mask = ((1u32 << self.width) - 1) << self.offset;
        let rhs_mask = ((1u32 << rhs.width) - 1) << rhs.offset;
        let lhs_bits = (self.value << self.offset) & lhs_mask;
        let rhs_bits = (rhs.value << rhs.offset) & rhs_mask;

        Self {
            offset: 0,
            width: 32,
            value: lhs_bits | rhs_bits,
            _marker: PhantomData,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadOnly<T, R> {
    _value: T,
    _marker: PhantomData<R>,
}

impl<T: Copy, R> ReadOnly<T, R> {
    pub fn read(&self, _field: Field<T, R>) -> T {
        unimplemented!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriteOnly<T, R> {
    _value: T,
    _marker: PhantomData<R>,
}

impl<T, R> WriteOnly<T, R> {
    pub fn write(&self, _values: FieldValue<T, R>) {
        unimplemented!()
    }

    pub fn set(&self, _values: FieldValue<T, R>) {
        unimplemented!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadWrite<T, R> {
    _value: T,
    _marker: PhantomData<R>,
}

impl<T: Copy, R> ReadWrite<T, R> {
    pub fn read(&self, _field: Field<T, R>) -> T {
        unimplemented!()
    }

    pub fn write(&self, _values: FieldValue<T, R>) {
        unimplemented!()
    }

    pub fn modify(&self, _values: FieldValue<T, R>) {
        unimplemented!()
    }

    pub fn set(&self, _values: FieldValue<T, R>) {
        unimplemented!()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reserved<T> {
    _value: T,
}

#[derive(Debug)]
pub struct Mmio<T> {
    inner: T,
}

impl<T> Mmio<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
}

impl<T> Deref for Mmio<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegseqError {
    Timeout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duration {
    us: u64,
}

impl Duration {
    pub const fn from_us(us: u64) -> Self {
        Self { us }
    }

    pub const fn from_ms(ms: u64) -> Self {
        Self { us: ms * 1_000 }
    }
}

pub mod poll {
    use super::{Duration, RegseqError};

    pub fn until<C>(_condition: C, _delay: Duration, _timeout: Duration) -> Result<(), RegseqError>
    where
        C: FnMut() -> bool,
    {
        unimplemented!()
    }
}

