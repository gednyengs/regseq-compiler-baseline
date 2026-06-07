#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The shared device-lifecycle author surface (the `Lifecycle` trait + an inline
// `RegseqError`). The harness verbs are gone (RD-1): the backend synthesizes the
// driving `main` from the model.
include!("../../_lifecycle_surface.rs");

// ---------------------------------------------------------------------------
// Minimal inline register-access surface (Form B). The codegen-coverage-runner
// bridges a single self-contained `input.rs` (no `--extern`), so the
// `register_block!` proc-macro is unavailable: the `Mmio<T>` wrapper, the
// `ReadWrite` register wrapper (mmio intrinsics), and the `#[repr(C)]` block are
// hand-written here. This mirrors `tests/fixtures/regaccess-pl011/src/regseq.rs`
// trimmed to the read/write ops this fixture uses. Register access is recognized
// by the `#[rseq::intrinsic("mmio_*")]` attrs + the `Mmio` wrapper name + the
// `#[repr(C)]` block layout (Strategy C), independent of any platform binding.
// ---------------------------------------------------------------------------
use core::marker::PhantomData;
use core::ops::Deref;

/// Read-write register wrapper. `#[repr(transparent)]` over the storage width so
/// `#[repr(C)]` gives byte offsets; access legality = method availability.
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
    #[inline(never)]
    #[rseq::intrinsic("mmio_read")]
    pub fn read(&self) -> u32 {
        loop {}
    }
    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _val: u32) {}
}

/// Typed MMIO resource: the register-block layout carrier, deref'd so
/// `regs.<reg>` selects a register by field access (the carrier storage is never
/// read — every access lowers to a device-model hook).
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

/// The register block (hand-written, no macro): one read-write data register.
#[repr(C)]
pub struct Regs {
    pub dr: ReadWrite<u32, ()>,
}
impl Regs {
    pub const fn new() -> Self {
        Self {
            dr: ReadWrite::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// M7 driver: a non-fieldless driver combining user-data (Rule-1 `count`) with a
// register-access resource (`regs`). `on_init` writes the zero-initialized
// `count` to a register (observing the zero-init, C10), then sets it and writes
// again (observing the field read/write through `self`).
// ---------------------------------------------------------------------------
#[rseq::driver(name = "handle", compatible = ["acme,handle"])]
pub struct Handle {
    count: u32,
    #[rseq::mmio(id = "regs")]
    regs: Mmio<Regs>,
}

impl Lifecycle for Handle {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // `count` is zero-initialized by the synthesized create(): writing it
        // observes 0x0 (the C10 zero-init proof).
        self.regs.dr.write(self.count);
        self.count = 0x5;
        self.regs.dr.write(self.count);
        Ok(())
    }
    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
    fn on_surprise_remove(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
}
