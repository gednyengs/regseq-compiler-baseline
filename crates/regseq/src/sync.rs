use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use crate::{Duration, RegseqError};

/// Mutual exclusion lock wrapper.
///
/// Opaque resource shell. The compiler lowers this to the appropriate
/// OS-specific mutex implementation.
#[rseq::abstraction("mutex")]
pub struct Mutex<T> {
    _marker: PhantomData<T>,
}

/// Spin lock wrapper.
///
/// Opaque resource shell for spin-based locking.
#[rseq::abstraction("spinlock")]
pub struct SpinLock<T> {
    _marker: PhantomData<T>,
}

/// IRQ-safe spin lock wrapper.
///
/// Opaque resource shell for interrupt-disabling spin-based locking.
#[rseq::abstraction("irq-spinlock")]
pub struct IrqSpinLock<T> {
    _marker: PhantomData<T>,
}

/// Completion event wrapper.
///
/// Opaque resource shell for wait/signal completion primitives.
#[rseq::abstraction("completion")]
pub struct Completion {
    _private: (),
}

/// Generic lock guard providing access to the protected data.
pub struct Guard<'a, T> {
    _marker: PhantomData<&'a mut T>,
}

impl<T> Mutex<T> {
    pub fn lock(&self) -> Guard<'_, T> {
        unimplemented!("facade stub: lowered by the compiler backend")
    }
}

impl<T> SpinLock<T> {
    pub fn lock(&self) -> Guard<'_, T> {
        unimplemented!("facade stub: lowered by the compiler backend")
    }
}

impl<T> IrqSpinLock<T> {
    pub fn lock(&self) -> Guard<'_, T> {
        unimplemented!("facade stub: lowered by the compiler backend")
    }
}

impl Completion {
    pub fn wait(&self, _timeout: Duration) -> Result<(), RegseqError> {
        unimplemented!("facade stub: lowered by the compiler backend")
    }

    pub fn signal(&self) {
        unimplemented!("facade stub: lowered by the compiler backend")
    }

    pub fn reset(&self) {
        unimplemented!("facade stub: lowered by the compiler backend")
    }
}

impl<T> Deref for Guard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unimplemented!("facade stub: lowered by the compiler backend")
    }
}

impl<T> DerefMut for Guard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unimplemented!("facade stub: lowered by the compiler backend")
    }
}
