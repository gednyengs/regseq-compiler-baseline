/// Typed MMIO resource declaration surface.
///
/// `Mmio<T>` is the opaque facade type that driver authors use to declare
/// MMIO-backed register-block fields. The compiler extracts these declarations
/// and joins them with Layer 2 binding data during lowering.
///
/// No trait bound is placed on `T`; validity of `T` as a register block is a
/// compiler-side extraction check, not a facade trait contract.
use core::ops::Deref;

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

impl<T> Deref for Mmio<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.inner
    }
}
