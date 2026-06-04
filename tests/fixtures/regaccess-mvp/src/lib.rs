#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

//! Minimal register-access lowering proof (thin slice).
//!
//! Inline facade + a one-register block + an entry that performs a single
//! `mmio_write`. The compiler is expected to recognize the `#[rseq::intrinsic
//! ("mmio_write")]` op and lower it to a `regseq_mmio_write(...)` host
//! device-model hook call in the generic-c backend.

use core::marker::PhantomData;

/// Typed MMIO resource (facade stub; the compiler lowers the ops).
pub struct Mmio<T> {
    inner: T,
}
impl<T> Mmio<T> {
    pub const fn new(inner: T) -> Self {
        Self { inner }
    }
}

/// Full-register identifier token (zero-sized; identity is in the type params).
pub struct RegisterId<W, R> {
    _w: PhantomData<W>,
    _r: PhantomData<R>,
}
impl<W, R> RegisterId<W, R> {
    pub const fn new() -> Self {
        Self {
            _w: PhantomData,
            _r: PhantomData,
        }
    }
}
impl<W, R> Clone for RegisterId<W, R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<W, R> Copy for RegisterId<W, R> {}

impl<Block> Mmio<Block> {
    /// Write a full register. Lowered by the compiler to a device-model hook.
    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write<W, R>(&self, _reg: RegisterId<W, R>, _val: W) {}
}

/// A tiny register block (one register, `foo`).
pub struct FooRegs;
/// Per-register marker type.
pub enum FooReg {}
pub mod foo {
    use super::{FooReg, RegisterId};
    /// Full-register token for `foo`.
    pub const REG: RegisterId<u32, FooReg> = RegisterId::new();
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let regs: Mmio<FooRegs> = Mmio::new(FooRegs);
    regs.write(foo::REG, 42u32);
}
