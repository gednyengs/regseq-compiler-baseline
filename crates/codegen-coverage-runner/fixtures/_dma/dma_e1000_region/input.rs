#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The dma-management M1 E1000 coherent-region golden (PLAN T23). An E1000-style
// TX driver declares a coherent `DmaRegion<LegacyTxDesc>` ring, writes one
// descriptor through the REAL closure-borrow element-access surface
// (`region.write(i, |d: &mut V| { d.field = … })`, T13), programs the ring base
// register from the region's device address (`device_addr()`, A2), and rings the
// TX-tail doorbell (a plain register-access MMIO write).
//
// The golden asserts BOTH oracles:
//   * behavioral (bytes): the descriptor bytes in the region's backing buffer ==
//     the expected LegacyTxDesc layout (the closure stores landed in real memory);
//   * codegen-shape (order): the emitted C sequences
//       …region stores… → regseq_dma_wmb() → regseq_mmio_w32(<doorbell TDT>)
//     in that order — FAILS if the store→doorbell fence (T16/C07/D09) is absent or
//     misordered. register-access emits the plain doorbell; dma-management inserts
//     ONLY the fence between the publishing stores and the doorbell.
//
// generic-c only (Tier-1, candidate-prototype). The compiler-managed
// alloc_coherent/free_coherent lifecycle is synthesized into the device-lifetime
// preamble/postamble (T12), so the author never calls them.

include!("../../_lifecycle_surface.rs");

// ---------------------------------------------------------------------------
// Register-access author surface (Form B), trimmed to the whole-register `write`
// the TX ring needs (the hand-written mirror of the `register_block!` expansion,
// per the e1000-composed fixture).
// ---------------------------------------------------------------------------
use core::marker::PhantomData;
use core::ops::Deref;

#[repr(transparent)]
pub struct ReadWrite<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
impl<W, R> ReadWrite<W, R> {
    #[inline(never)]
    #[rseq::intrinsic("mmio_read")]
    pub fn read(&self) -> W {
        loop {}
    }
    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _val: W) {}
}
impl<R> ReadWrite<u32, R> {
    pub const fn new() -> Self {
        Self { _v: 0, _r: PhantomData }
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

// A trimmed E1000 register block: the TX ring base (low/high) + the TX tail
// doorbell, at hardware-faithful 82540EM offsets (qemu hw/net/e1000_regs.h).
pub mod e1000 {
    use super::ReadWrite;

    pub mod tdbal { pub enum Register {} }
    pub mod tdbah { pub enum Register {} }
    pub mod resv { pub enum Register {} }
    pub mod tdt { pub enum Register {} }

    #[repr(C)]
    pub struct Regs {
        /// 0x03800 — TX Descriptor Base Address Low (RW).
        pub tdbal: ReadWrite<u32, tdbal::Register>,
        /// 0x03804 — TX Descriptor Base Address High (RW).
        pub tdbah: ReadWrite<u32, tdbah::Register>,
        /// 0x03808..0x03818 — reserved (TDLEN/TDH unused here).
        pub _reserved: [ReadWrite<u32, resv::Register>; 4],
        /// 0x03818 — TX Descriptor Tail (RW, doorbell).
        pub tdt: ReadWrite<u32, tdt::Register>,
    }
}

const _: () = assert!(core::mem::offset_of!(e1000::Regs, tdbal) == 0x00);
const _: () = assert!(core::mem::offset_of!(e1000::Regs, tdt) == 0x18);

// ---------------------------------------------------------------------------
// DMA-management author surface (dma-management construct, M1 coherent-region).
// The typed `DmaRegion<T>` resource (detected by type NAME, the Mmio/Clock
// anchor) + the A2 element-access closure verb + the addressing ops, each tagged
// with its namespaced `#[rseq::intrinsic("dma_*")]` facade. The element type `T`
// is an E1000 legacy TX descriptor (16 bytes).
// ---------------------------------------------------------------------------
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LegacyTxDesc {
    /// Buffer address (the DMA-visible address of the packet payload).
    pub buffer_addr: u64,
    /// Length + CSO + CMD packed low word (programmed as one u32 here).
    pub lower: u32,
    /// Status + CSS + special packed high word.
    pub upper: u32,
}

const _: () = assert!(core::mem::size_of::<LegacyTxDesc>() == 16);

pub struct DmaRegion<T> {
    _t: core::marker::PhantomData<T>,
}
impl<T> DmaRegion<T> {
    pub const fn new() -> Self {
        Self { _t: core::marker::PhantomData }
    }
    /// Scoped typed read borrow of element `i` (A2, hands the closure `&V`).
    #[inline(never)]
    #[rseq::intrinsic("dma_element_read")]
    pub fn read<V, R>(&self, _i: usize, _f: impl FnOnce(&V) -> R) -> R {
        loop {}
    }
    /// Scoped typed write borrow of element `i` (A2, hands the closure `&mut V`).
    #[inline(never)]
    #[rseq::intrinsic("dma_element_write")]
    pub fn write<V>(&self, _i: usize, _f: impl FnOnce(&mut V)) {}
    /// Scoped typed read-modify borrow of element `i` (A2, `&mut V`).
    #[inline(never)]
    #[rseq::intrinsic("dma_element_update")]
    pub fn update<V>(&self, _i: usize, _f: impl FnOnce(&mut V)) {}
    /// The region's DMA-visible base address (A2 addressing).
    #[inline(never)]
    #[rseq::intrinsic("dma_device_addr")]
    pub fn device_addr(&self) -> u64 {
        loop {}
    }
    /// The DMA-visible address of element `i` (`base + i*element_size`, A2).
    #[inline(never)]
    #[rseq::intrinsic("dma_device_addr_of")]
    pub fn device_addr_of(&self, _i: usize) -> u64 {
        loop {}
    }
    /// The declared element cardinality (A2 introspection).
    #[inline(never)]
    #[rseq::intrinsic("dma_element_count")]
    pub fn element_count(&self) -> usize {
        loop {}
    }
}

// ---------------------------------------------------------------------------
// The TX driver: PCI-matched (E1000 82540EM), an MMIO register block + a coherent
// TX-descriptor ring. The bus-master coherent region sets `dma_present` (T05), so
// the bus acquisition emits `set_bus_master`; the region's alloc_coherent/
// free_coherent lifecycle frames the device lifetime (T12).
// ---------------------------------------------------------------------------

/// The DMA-visible address of the (host-side, never-read) packet buffer the
/// descriptor points at — a donor-inert literal (the device never processes the
/// ring in this Tier-1 oracle).
const PKT_BUF_DMA_ADDR: u64 = 0x0000_0001_0000_2000;
/// The legacy TX descriptor's packed low word: length 0x40, EOP+RS cmd bits.
const TXDESC_LOWER: u32 = 0x0B00_0040;
/// The packed high word: status cleared, no checksum offload.
const TXDESC_UPPER: u32 = 0x0000_0000;

#[rseq::device_class(network)]
#[rseq::driver(name = "e1000dma", bus = "pci", pci = [
    { vendor = 0x8086, device = 0x100e },
    ],
    compatible = ["regseq,e1000dma"]
)]
pub struct E1000Dma {
    #[rseq::mmio(id = "regs", bar = 0)]
    regs: Mmio<e1000::Regs>,
    #[rseq::dma_region(id = "tx_ring", element_size = 16, element_count = 256)]
    tx_ring: DmaRegion<LegacyTxDesc>,
}

impl Lifecycle for E1000Dma {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // 1) Write TX descriptor 0 through the REAL closure-borrow element-access
        //    surface (T13): the closure's typed field stores land in the region's
        //    coherent backing buffer (the behavioral byte oracle reads them).
        self.tx_ring.write::<LegacyTxDesc>(0, |d: &mut LegacyTxDesc| {
            d.buffer_addr = PKT_BUF_DMA_ADDR;
            d.lower = TXDESC_LOWER;
            d.upper = TXDESC_UPPER;
        });

        // 2) Program the ring base register from the region's DMA-visible address
        //    (A2 device_addr): high-then-low (the donor program order; the device
        //    latches on the low write).
        let base = self.tx_ring.device_addr();
        self.regs.tdbah.write((base >> 32) as u32);
        self.regs.tdbal.write(base as u32);

        // 3) Ring the TX-tail doorbell (a plain register-access MMIO write). The
        //    publishing region stores in (1) MUST be fenced before this doorbell
        //    (the dma-management store→doorbell fence, T16): the golden asserts
        //    …region stores… → regseq_dma_wmb() → regseq_mmio_w32(TDT). The ring
        //    stays empty (TDT = 0) so the device model never touches guest RAM.
        self.regs.tdt.write(0);
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        // Quiesce: reset the tail. The compiler-managed free_coherent postamble
        // runs after this (LIFO with alloc_coherent), trace-only in the oracle.
        self.regs.tdt.write(0);
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main`; the manifest's
// `lifecycle_scenario = "orderly"` drives the run (register → create → associate
// → resource preamble → PCI acquisition (incl. set_bus_master, dma_present) →
// map_region → alloc_coherent → on_init → on_deinit → free_coherent → teardown).
