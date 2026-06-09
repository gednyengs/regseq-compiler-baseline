#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The shared device-lifecycle author surface (the `Lifecycle` trait + an inline
// `RegseqError`). The backend SYNTHESIZES the driving `main` from the model; the
// manifest's `lifecycle_scenario = "orderly"` selects the bring-up scenario.
include!("../../_lifecycle_surface.rs");

// ---------------------------------------------------------------------------
// Minimal inline register-access surface (Form B). The codegen-coverage-runner
// bridges a single self-contained `input.rs` (no `--extern`), so the
// `register_block!` proc-macro is unavailable: the `Mmio<T>` wrapper, the
// `ReadWrite` register wrapper (mmio intrinsics), and the `#[repr(C)]` block are
// hand-written here. Register access is recognized by the
// `#[rseq::intrinsic("mmio_*")]` attrs + the `Mmio` wrapper name + the
// `#[repr(C)]` block layout (Strategy C). Mirrors
// `_lifecycle/lifecycle-handle/input.rs`, trimmed to read/write.
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

/// The register block (hand-written, no macro): CTRL / STATUS / ICR at
/// `#[repr(C)]` byte offsets 0x00 / 0x04 / 0x08.
#[repr(C)]
pub struct Regs {
    pub ctrl: ReadWrite<u32, ()>,   // 0x00 — device control
    pub status: ReadWrite<u32, ()>, // 0x04 — device status
    pub icr: ReadWrite<u32, ()>,    // 0x08 — interrupt cause (read-clear)
}
impl Regs {
    pub const fn new() -> Self {
        Self {
            ctrl: ReadWrite::new(),
            status: ReadWrite::new(),
            icr: ReadWrite::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// The KEYSTONE driver: a PCI-matched (Intel NIC) device with a bus-managed MMIO
// region. The L1 `#[rseq::mmio(id = "regs", bar = 0)]` joins to the platform
// file's `[mmio.regs]` (base = 0xf000_0000, size = 0x2_0000) → a RESOLVED bus
// binding. The device-lifecycle preamble therefore emits the construct-#3
// acquisition (enable_device → claim_regions → get_handle → `map_region`), and
// `on_init`'s register access routes through the BASED accessor (tracing the real
// mapped address). There is NO hand-set `MmioBinding`: the base/size flow
// platform.toml → bound model → emitted C end-to-end.
// ---------------------------------------------------------------------------
// `bus = "pci"` is the construct-#3 bus selector (L1) that sets the model's
// `bus_type` — the fact that gates the PCI-only acquisition ops (enable_device,
// set_master). `pci = [{…}]` is the orthogonal device-lifecycle match table (the
// registration `match=pci:0x8086:0x100e`). A real PCI driver carries both.
#[rseq::device_class(network)]
#[rseq::driver(name = "e1000", bus = "pci", pci = [{ vendor = 0x8086, device = 0x100e }])]
pub struct E1000 {
    #[rseq::mmio(id = "regs", bar = 0)]
    regs: Mmio<Regs>,
}

impl Lifecycle for E1000 {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // Set link-up on CTRL (W32 @0xf0000000), then read read-clear ICR
        // (R32 @0xf0000008) — both through the based accessor, so the behavioral
        // trace shows the real mapped absolute addresses.
        self.regs.ctrl.write(0x40);
        let _icr = self.regs.icr.read();
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main` (RD-4/RD-6). The
// manifest's `lifecycle_scenario = "orderly"` lowers to:
//   register (match=pci:0x8086:0x100e) → create → associate → resource preamble
//   → bus acquisition (enable_device, claim_regions(0), get_handle,
//     map_region(0xf0000000, 0x20000, 0)) → LC call on_init (CTRL write +
//     ICR read through the based accessor) → LC call on_deinit → teardown.
