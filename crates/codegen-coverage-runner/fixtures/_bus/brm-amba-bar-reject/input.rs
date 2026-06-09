#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// Minimal self-contained bus+mmio driver for a BRM compile-fail (reject) fixture.
// It need only BRIDGE: the rejection happens at the L2 Bind stage (BRM-002),
// before emit. No sibling `platform.toml` is needed — the violation is purely L1
// (a BAR declared on a non-PCI bus).
use core::marker::PhantomData;
use core::ops::Deref;

#[repr(transparent)]
pub struct ReadWrite<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
impl<R> ReadWrite<u32, R> {
    pub const fn new() -> Self {
        Self {
            _v: 0,
            _r: PhantomData,
        }
    }
}

pub struct Mmio<T> {
    inner: T,
}
impl<T> Mmio<T> {
    pub const fn new(inner: T) -> Self {
        Mmio { inner }
    }
}
impl<T> Deref for Mmio<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

#[repr(C)]
pub struct Regs {
    pub ctrl: ReadWrite<u32, ()>,
}
impl Regs {
    pub const fn new() -> Self {
        Self {
            ctrl: ReadWrite::new(),
        }
    }
}

// An AMBA driver that ILLEGALLY declares a PCI BAR on its MMIO region. A BAR is
// PCI-only, so `bus = "amba"` + `bar = 0` trips BRM-002 at the Bind stage,
// raised by `regseq_l2::validate_l2_surface` from the L1 `bar` alone (no platform
// file involved). The `amba = [{ id, mask }]` is the device-match attribute every
// driver needs (PL011 primecell id), orthogonal to the BRM-002 violation.
#[rseq::driver(name = "ambadev", bus = "amba", amba = [{ id = 0x00041011, mask = 0x000fffff }])]
pub struct Dev {
    #[rseq::mmio(id = "regs", bar = 0)]
    regs: Mmio<Regs>,
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let _dev = Dev {
        regs: Mmio::new(Regs::new()),
    };
}
