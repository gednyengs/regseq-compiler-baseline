// C5 composition-only author surface for register access and PCI config I/O.
// Device specs include this neutral surface; target frameworks remain backend-owned.

use core::marker::PhantomData;
use core::ops::Deref;

#[derive(Clone, Copy)]
pub struct FieldValue<W, R> {
    pub mask: W,
    pub value: W,
    _register: PhantomData<R>,
}

impl<W, R> FieldValue<W, R> {
    pub const fn new(mask: W, value: W) -> Self {
        Self { mask, value, _register: PhantomData }
    }
}

#[repr(transparent)]
pub struct ReadOnly<W, R> {
    _value: W,
    _register: PhantomData<R>,
}

impl<W, R> ReadOnly<W, R> {
    #[inline(never)]
    #[rseq::intrinsic("mmio_read")]
    pub fn read(&self) -> W { loop {} }
}

#[repr(transparent)]
pub struct WriteOnly<W, R> {
    _value: W,
    _register: PhantomData<R>,
}

impl<W, R> WriteOnly<W, R> {
    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _value: W) {}
}

#[repr(transparent)]
pub struct ReadWrite<W, R> {
    _value: W,
    _register: PhantomData<R>,
}

impl<W, R> ReadWrite<W, R> {
    #[inline(never)]
    #[rseq::intrinsic("mmio_read")]
    pub fn read(&self) -> W { loop {} }

    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _value: W) {}

    #[inline(never)]
    #[rseq::intrinsic("mmio_modify")]
    pub fn modify(&self, _value: FieldValue<W, R>) {}
}

#[repr(transparent)]
pub struct Reserved<W> { _value: W }

pub struct Mmio<T> { inner: T }

impl<T> Deref for Mmio<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target { &self.inner }
}

pub mod regseq {
    pub mod bus {
        #[inline(never)]
        #[rseq::intrinsic("read_config")]
        pub fn read_config<T>(_offset: u32) -> T { loop {} }

        #[inline(never)]
        #[rseq::intrinsic("write_config")]
        pub fn write_config<T>(_offset: u32, _value: T) {}
    }
}
