#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// Minimal self-contained bus+mmio driver for a BRM compile-fail (reject) fixture.
// It need only BRIDGE: the rejection happens at the L2 Bind stage (BRM-005),
// before emit, so there is no register access / lifecycle / golden. The inline
// `ReadWrite`/`Mmio`/`#[repr(C)] Regs` mirror the keystone surface (no `--extern`
// in the harness → no `register_block!` proc-macro).
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

// A PCI driver with a bus-managed MMIO region. The sibling `platform.toml`
// supplies `base` WITHOUT `size` → a half-specified region → BRM-005, raised by
// `regseq_l2::validate_l2_surface` at the Bind stage. The `pci = [{…}]` is the
// device-match attribute every driver needs, orthogonal to the BRM-005 violation.
#[rseq::driver(name = "halfspec", bus = "pci", pci = [{ vendor = 0x8086, device = 0x100e }])]
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
