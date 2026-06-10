#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The COMPOSED E1000 fixture (backend-linux-c-e1000 T01, GRILL D01): the five
// refined construct surfaces — register-access, bus-resource-management (PCI),
// device-lifecycle, diagnostics-logging, RegseqError — PLUS the config-space
// ops (read_config/write_config, ratified A-11) in one PCI spec. Its generic_c
// run blesses the canonical E1000 oracle trace (T02); the linux_c backend
// lowers the SAME bound model to the PCI kernel module (T04–T09).
//
// This is an MMIO BRING-UP SLICE, explicitly not a functioning NIC (GRILL
// D01): no DMA/IRQ/netdev constructs exist. Device facts are transcribed from
// the `tests/fixtures/regaccess-e1000` donor surface (CTRL RMW, ICR
// read-clear, TDBAH-then-TDBAL high-then-low split, register arrays with
// runtime indices), at HARDWARE-FAITHFUL byte offsets (qemu
// hw/net/e1000_regs.h / e1000x_regs.h): this fixture's trace is the L3 oracle
// against QEMU's real 82540EM device model, so the offsets must hit the real
// registers. The donor fixture itself is untouched (S13).
//
// L2-safety invariant (QEMU hw/net/e1000.c `set_tctl`/`start_xmit`): every
// TCTL/TDT write kicks `start_xmit`, which DMA-reads TX descriptors from
// guest memory whenever TCTL.EN is set and TDH != TDT. The bring-up therefore
// keeps the ring EMPTY — the doorbell writes TDT = 0 (== TDH) — so enabling
// TX never makes the device model touch guest RAM.

// The shared device-lifecycle author surface (the `Lifecycle` trait + the
// inline `RegseqError`). The backend synthesizes the driving `main`; the
// manifest's `lifecycle_scenario = "orderly"` selects the bring-up scenario.
include!("../../_lifecycle_surface.rs");

// ---------------------------------------------------------------------------
// Diagnostics-logging author surface: the canonical intrinsic anchors + the
// namespaced `regseq::log::{debug,info,warn,error}!` macros, and the
// bus-resource-management config-space intrinsics `regseq::bus::{read_config,
// write_config}` (construct #3 M5; the first fixture to exercise them).
// Mirrors the `pl011-composed` inline surface.
// ---------------------------------------------------------------------------
pub mod regseq {
    pub mod log {
        pub mod intrinsics {
            pub enum LogArg<'a> {
                U8(u8), U16(u16), U32(u32), U64(u64),
                I8(i8), I16(i16), I32(i32), I64(i64),
                Bool(bool), Str(&'a str),
            }
            impl From<u8> for LogArg<'_> { fn from(v: u8) -> Self { LogArg::U8(v) } }
            impl From<u16> for LogArg<'_> { fn from(v: u16) -> Self { LogArg::U16(v) } }
            impl From<u32> for LogArg<'_> { fn from(v: u32) -> Self { LogArg::U32(v) } }
            impl From<u64> for LogArg<'_> { fn from(v: u64) -> Self { LogArg::U64(v) } }
            impl From<i8> for LogArg<'_> { fn from(v: i8) -> Self { LogArg::I8(v) } }
            impl From<i16> for LogArg<'_> { fn from(v: i16) -> Self { LogArg::I16(v) } }
            impl From<i32> for LogArg<'_> { fn from(v: i32) -> Self { LogArg::I32(v) } }
            impl From<i64> for LogArg<'_> { fn from(v: i64) -> Self { LogArg::I64(v) } }
            impl From<bool> for LogArg<'_> { fn from(v: bool) -> Self { LogArg::Bool(v) } }
            impl<'a> From<&'a str> for LogArg<'a> { fn from(v: &'a str) -> Self { LogArg::Str(v) } }

            #[inline(never)] #[rseq::intrinsic("log-debug")]
            pub fn log_debug(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-info")]
            pub fn log_info(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-warn")]
            pub fn log_warn(_fmt: &'static str, _args: &[LogArg<'_>]) {}
            #[inline(never)] #[rseq::intrinsic("log-error")]
            pub fn log_error(_fmt: &'static str, _args: &[LogArg<'_>]) {}
        }
        macro_rules! debug { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_debug($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use debug;
        macro_rules! info { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_info($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use info;
        macro_rules! log_warn { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_warn($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use log_warn as warn;
        macro_rules! error { ($fmt:literal $(, $arg:expr)* $(,)?) => {
            $crate::regseq::log::intrinsics::log_error($fmt, &[$($crate::regseq::log::intrinsics::LogArg::from($arg)),*]) }; }
        pub(crate) use error;
    }

    /// Config-space intrinsics (bus-resource-management construct #3 M5). The
    /// access width is `size_of::<T>()` — the destination type for a read,
    /// the value type for a write; the config offset is the first argument.
    pub mod bus {
        #[inline(never)]
        #[rseq::intrinsic("read_config")]
        pub fn read_config<T>(_off: u32) -> T {
            loop {}
        }
        #[inline(never)]
        #[rseq::intrinsic("write_config")]
        pub fn write_config<T>(_off: u32, _val: T) {}
    }
}

// ---------------------------------------------------------------------------
// Register-access author surface (Form B). The codegen-coverage-runner bridges
// a single self-contained `input.rs` (no `--extern`), so the `register_block!`
// proc-macro is unavailable: this is the hand-written mirror of its expansion,
// per `tests/fixtures/regaccess-e1000/src/regseq.rs` (trimmed to u32, the only
// width the E1000 CSR space uses). Register access is recognized by the
// `#[rseq::intrinsic("mmio_*")]` attrs + the `Mmio` wrapper name + the
// `#[repr(C)]` block layout (Strategy C).
// ---------------------------------------------------------------------------
use core::marker::PhantomData;
use core::ops::Deref;

/// Field (bitfield) descriptor: register-relative `mask` + `shift`.
#[derive(Clone, Copy)]
pub struct Field<W, R> {
    pub mask: W,
    pub shift: W,
    _r: PhantomData<R>,
}
impl<W, R> Field<W, R> {
    pub const fn new(mask: W, shift: W) -> Self {
        Self {
            mask,
            shift,
            _r: PhantomData,
        }
    }
}

/// Field write value: register-relative `mask` + composed `value`.
#[derive(Clone, Copy)]
pub struct FieldValue<W, R> {
    pub mask: W,
    pub value: W,
    _r: PhantomData<R>,
}
impl<W, R> FieldValue<W, R> {
    pub const fn new(mask: W, value: W) -> Self {
        Self {
            mask,
            value,
            _r: PhantomData,
        }
    }
}

/// Read-only register wrapper. `#[repr(transparent)]` over the storage width so
/// `#[repr(C)]` gives byte offsets; access legality = method availability.
#[repr(transparent)]
pub struct ReadOnly<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
impl<W, R> ReadOnly<W, R> {
    /// Whole-register raw read.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read")]
    pub fn read(&self) -> W {
        loop {}
    }
}

/// Write-only register wrapper.
#[repr(transparent)]
pub struct WriteOnly<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
impl<W, R> WriteOnly<W, R> {
    /// Whole-register raw write.
    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _val: W) {}
}

/// Read-write register wrapper.
#[repr(transparent)]
pub struct ReadWrite<W, R> {
    _v: W,
    _r: PhantomData<R>,
}
impl<W, R> ReadWrite<W, R> {
    /// Whole-register raw read.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read")]
    pub fn read(&self) -> W {
        loop {}
    }
    /// Masked + shifted field read.
    #[inline(never)]
    #[rseq::intrinsic("mmio_read_field")]
    pub fn read_field(&self, _f: Field<W, R>) -> W {
        loop {}
    }
    /// Whole-register raw write.
    #[inline(never)]
    #[rseq::intrinsic("mmio_write")]
    pub fn write(&self, _val: W) {}
    /// Masked field write (bits outside the mask = 0).
    #[inline(never)]
    #[rseq::intrinsic("mmio_write_field")]
    pub fn write_field(&self, _fv: FieldValue<W, R>) {}
    /// Read-modify-write a field (preserves unmentioned bits).
    #[inline(never)]
    #[rseq::intrinsic("mmio_modify")]
    pub fn modify(&self, _fv: FieldValue<W, R>) {}
}

/// Reserved gap in a register block (`#[repr(C)]` spacer).
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Reserved<W> {
    _v: W,
}

// Per-width `new()` constructors (literal `0` → native, body-available).
impl<R> ReadOnly<u32, R> {
    pub const fn new() -> Self {
        Self { _v: 0, _r: PhantomData }
    }
}
impl<R> WriteOnly<u32, R> {
    pub const fn new() -> Self {
        Self { _v: 0, _r: PhantomData }
    }
}
impl<R> ReadWrite<u32, R> {
    pub const fn new() -> Self {
        Self { _v: 0, _r: PhantomData }
    }
}
impl Reserved<u32> {
    pub const fn new() -> Self {
        Self { _v: 0 }
    }
}

/// Typed MMIO resource: the register-block layout carrier, deref'd so
/// `regs.<reg>` selects a register by field access (the carrier storage is
/// never read — every access lowers to a device-model hook).
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

// ---------------------------------------------------------------------------
// The E1000 register block — the hand-written mirror of the donor's
// `register_block!` declaration, HARDWARE-FAITHFUL byte offsets (qemu
// hw/net/e1000_regs.h, e1000x_regs.h; the donor fixture deliberately used
// compact offsets — this one cannot, its trace meets the real device model).
// ---------------------------------------------------------------------------
pub mod e1000 {
    use super::{Field, FieldValue, ReadOnly, ReadWrite, Reserved, WriteOnly};

    /// `FieldValue<u32, R>` shorthand for the per-register const tables.
    pub type FieldValue32<R> = FieldValue<u32, R>;

    pub mod ctrl {
        pub enum Register {}
        /// CTRL.SLU (bit 6): set link up (force link).
        pub const SLU_UP: super::FieldValue32<Register> = super::FieldValue32::new(1 << 6, 1 << 6);
    }
    pub mod status {
        pub enum Register {}
    }
    pub mod icr {
        pub enum Register {}
    }
    pub mod imc {
        pub enum Register {}
    }
    pub mod tctl {
        pub enum Register {}
        /// TCTL.EN (bit 1): transmit enable.
        pub const EN: super::Field<u32, Register> = super::Field::new(1 << 1, 1);
        pub const EN_ENABLED: super::FieldValue32<Register> =
            super::FieldValue32::new(1 << 1, 1 << 1);
    }
    pub mod tdbal {
        pub enum Register {}
    }
    pub mod tdbah {
        pub enum Register {}
    }
    pub mod tdlen {
        pub enum Register {}
    }
    pub mod tdt {
        pub enum Register {}
    }
    pub mod mta {
        pub enum Register {}
    }
    pub mod ra {
        pub mod ral {
            pub enum Register {}
        }
        pub mod rah {
            /// RAH.AV (bit 31): receive-address entry valid.
            pub const AV_VALID: crate::e1000::FieldValue32<Register> =
                crate::e1000::FieldValue32::new(1 << 31, 1 << 31);
            pub enum Register {}
        }
    }
    pub mod vfta {
        pub enum Register {}
    }

    /// One receive-address filter entry: RAL = low 32 of the MAC; RAH = high
    /// 16 + the Address-Valid bit (8-byte stride; the donor's array-of-struct
    /// `ra[N]: { ral, rah }` form).
    #[repr(C)]
    pub struct RaEntry {
        pub ral: ReadWrite<u32, ra::ral::Register>,
        pub rah: ReadWrite<u32, ra::rah::Register>,
    }

    /// The block: real 82540EM byte offsets (qemu hw/net/e1000_regs.h).
    #[repr(C)]
    pub struct Regs {
        /// 0x00000 — Device Control (RW).
        pub ctrl: ReadWrite<u32, ctrl::Register>,
        /// 0x00004 — reserved.
        pub _reserved0: Reserved<u32>,
        /// 0x00008 — Device Status (RO).
        pub status: ReadOnly<u32, status::Register>,
        /// 0x0000C..0x000C0 — reserved here (EECD/EERD/CTRL_EXT/… unused).
        pub _reserved1: [Reserved<u32>; 45],
        /// 0x000C0 — Interrupt Cause Read (RO, read-clear).
        pub icr: ReadOnly<u32, icr::Register>,
        /// 0x000C4..0x000D8 — reserved (ITR/ICS/IMS unused).
        pub _reserved2: [Reserved<u32>; 5],
        /// 0x000D8 — Interrupt Mask Clear (WO).
        pub imc: WriteOnly<u32, imc::Register>,
        /// 0x000DC..0x00400 — reserved (RCTL/… unused).
        pub _reserved3: [Reserved<u32>; 201],
        /// 0x00400 — Transmit Control (RW).
        pub tctl: ReadWrite<u32, tctl::Register>,
        /// 0x00404..0x03800 — reserved.
        pub _reserved4: [Reserved<u32>; 3327],
        /// 0x03800 — TX Descriptor Base Address Low (RW).
        pub tdbal: ReadWrite<u32, tdbal::Register>,
        /// 0x03804 — TX Descriptor Base Address High (RW).
        pub tdbah: ReadWrite<u32, tdbah::Register>,
        /// 0x03808 — TX Descriptor Length (RW).
        pub tdlen: ReadWrite<u32, tdlen::Register>,
        /// 0x0380C..0x03818 — reserved (TDH unused).
        pub _reserved5: [Reserved<u32>; 3],
        /// 0x03818 — TX Descriptor Tail (RW, doorbell).
        pub tdt: ReadWrite<u32, tdt::Register>,
        /// 0x0381C..0x05200 — reserved.
        pub _reserved6: [Reserved<u32>; 1657],
        /// 0x05200..0x05400 — Multicast Table Array (128 × u32).
        pub mta: [ReadWrite<u32, mta::Register>; 128],
        /// 0x05400..0x05480 — Receive Address filter (16 RAL/RAH pairs).
        pub ra: [RaEntry; 16],
        /// 0x05480..0x05600 — reserved.
        pub _reserved7: [Reserved<u32>; 96],
        /// 0x05600..0x05800 — VLAN Filter Table Array (128 × u32).
        pub vfta: [ReadWrite<u32, vfta::Register>; 128],
    }
}

// Hardware-offset guards: compile-time proof the layout-derived offsets match
// the 82540EM map. A miscounted `Reserved` gap fails the build here rather
// than silently mis-lowering (the regaccess donors' Codex-#5 convention).
const _: () = assert!(core::mem::offset_of!(e1000::Regs, ctrl) == 0x00000);
const _: () = assert!(core::mem::offset_of!(e1000::Regs, status) == 0x00008);
const _: () = assert!(core::mem::offset_of!(e1000::Regs, icr) == 0x000C0);
const _: () = assert!(core::mem::offset_of!(e1000::Regs, imc) == 0x000D8);
const _: () = assert!(core::mem::offset_of!(e1000::Regs, tctl) == 0x00400);
const _: () = assert!(core::mem::offset_of!(e1000::Regs, tdbal) == 0x03800);
const _: () = assert!(core::mem::offset_of!(e1000::Regs, tdbah) == 0x03804);
const _: () = assert!(core::mem::offset_of!(e1000::Regs, tdlen) == 0x03808);
const _: () = assert!(core::mem::offset_of!(e1000::Regs, tdt) == 0x03818);
const _: () = assert!(core::mem::offset_of!(e1000::Regs, mta) == 0x05200);
const _: () = assert!(core::mem::offset_of!(e1000::Regs, ra) == 0x05400);
const _: () = assert!(core::mem::offset_of!(e1000::Regs, vfta) == 0x05600);

// ---------------------------------------------------------------------------
// The composed driver: PCI-matched (Intel 82540EM + 82545EM), bus-managed MMIO
// region joined to the platform file's `[mmio.regs]` → a RESOLVED binding
// (A-10: the L2 literals are required as the trigger; on Linux their VALUES
// are dead — probe-time enumeration supplies the real extent, ledger B-11).
// ---------------------------------------------------------------------------

/// 64-bit TX ring base, programmed high-then-low (donor program order; the
/// device latches on the low write). The ring is NEVER processed: TDT stays
/// equal to TDH (see the L2-safety invariant in the header comment).
const TX_RING_BASE_HIGH: u32 = 0x0000_0001;
const TX_RING_BASE_LOW: u32 = 0x0000_2000;
/// Ring length in bytes (8 descriptors × 16 B — the donor's inert value).
const TX_RING_LEN_BYTES: u32 = 0x80;
/// IMC mask-all (every interrupt source cleared on deinit).
const ALL_INTERRUPTS_MASK: u32 = 0xFFFF_FFFF;
/// Low 32 bits of the receive-filter MAC address (donor value, inert).
const MAC_ADDR_LOW: u32 = 0xAABB_CCDD;

/// PCI config-space offsets (standard header) + the benign write value.
/// Width coverage for the A-11 read surface: word (vendor/device), byte
/// (revision), dword (BAR0).
const CFG_VENDOR_ID: u32 = 0x00;
const CFG_DEVICE_ID: u32 = 0x02;
const CFG_REVISION_ID: u32 = 0x08;
const CFG_BAR0: u32 = 0x10;
/// Cache Line Size @0x0C — the one benign config WRITE (GRILL D10, O2
/// discharged against the QEMU 82540EM model): qemu hw/pci/pci.c:863 makes
/// CLS guest-writable (`wmask[PCI_CACHE_LINE_SIZE] = 0xff`), and
/// hw/net/e1000.c `e1000_write_config` (:1624) only reacts to PCI_COMMAND —
/// a CLS write is stored by `pci_default_write_config` with no functional
/// side effect. The value 0x10 (64-byte lines, in dwords) is exactly what
/// e1000's realize already set (e1000.c:1649), so the write is idempotent.
const CFG_CACHE_LINE_SIZE: u32 = 0x0C;
const CACHE_LINE_SIZE_DWORDS: u8 = 0x10;

// Two-row match table: the campaign DUT id (82540EM, 8086:100e — the QEMU
// `-device e1000` default) PLUS the 82545EM sibling (8086:100f), exercising
// the FULL match_set → multi-row pci_device_id table lowering (the kernel
// tables are inherently multi-row + zero-sentinel; B-6 never-truncate).
#[rseq::device_class(network)]
#[rseq::driver(name = "e1000", bus = "pci", pci = [
    { vendor = 0x8086, device = 0x100e },
    { vendor = 0x8086, device = 0x100f },
])]
pub struct E1000Composed {
    #[rseq::mmio(id = "regs", bar = 0)]
    regs: Mmio<e1000::Regs>,
}

impl E1000Composed {
    /// Program the (never-processed) TX descriptor ring: 64-bit base split
    /// across two 32-bit registers HIGH-then-LOW (the donor's program order),
    /// then the ring length.
    fn program_tx_ring(&mut self) {
        self.regs.tdbah.write(TX_RING_BASE_HIGH);
        self.regs.tdbal.write(TX_RING_BASE_LOW);
        self.regs.tdlen.write(TX_RING_LEN_BYTES);
    }

    /// Exercise the register arrays with RUNTIME indices (the offset is
    /// recovered as `base + i*stride`, not a layout constant — the donor's
    /// `setup_filters` form): clear one multicast-table entry, clear one
    /// VLAN-filter entry, program receive-address entry `r` (low MAC word,
    /// then the Address-Valid bit — the array-of-struct Field+Index+Field
    /// chain).
    fn setup_filters(&mut self, m: usize, v: usize, r: usize) {
        self.regs.mta[m].write(0);
        self.regs.vfta[v].write(0);
        self.regs.ra[r].ral.write(MAC_ADDR_LOW);
        self.regs.ra[r].rah.write_field(e1000::ra::rah::AV_VALID);
    }
}

impl Lifecycle for E1000Composed {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        regseq::log::debug!("e1000 probe start");
        // Config-space surface (ratified A-11, both directions): standard-
        // header reads across widths — vendor/device (word), revision (byte),
        // BAR0 (dword) — plus the one benign CLS write (D10/O2, see the
        // const's comment). Values are logged, never branched on (the oracle's
        // config space is zero-seeded; QEMU returns the real header).
        let vendor: u16 = regseq::bus::read_config::<u16>(CFG_VENDOR_ID);
        let device: u16 = regseq::bus::read_config::<u16>(CFG_DEVICE_ID);
        let revision: u8 = regseq::bus::read_config::<u8>(CFG_REVISION_ID);
        let bar0: u32 = regseq::bus::read_config::<u32>(CFG_BAR0);
        regseq::log::info!("e1000 cfg {:#x}:{:#x} rev {} bar0 {:#x}", vendor, device, revision, bar0);
        regseq::bus::write_config::<u8>(CFG_CACHE_LINE_SIZE, CACHE_LINE_SIZE_DWORDS);

        // MMIO bring-up (donor patterns at real offsets):
        // CTRL RMW: set link up.            -> R32 @0x0000 ; W32 @0x0000
        self.regs.ctrl.modify(e1000::ctrl::SLU_UP);
        // TX ring base HIGH-then-LOW + len. -> W32 @0x3804; W32 @0x3800; W32 @0x3808
        self.program_tx_ring();
        // Doorbell while the ring is EMPTY (TDT = TDH = 0): the write itself
        // is the donor's doorbell pattern; 0 keeps `start_xmit` a no-op when
        // TX is enabled next (L2-safety invariant).  -> W32 @0x3818 = 0
        self.regs.tdt.write(0);
        // Enable TX via field RMW.          -> R32 @0x0400 ; W32 @0x0400 = 0x2
        self.regs.tctl.modify(e1000::tctl::EN_ENABLED);
        // Enable read-back sanity check (field read): TCTL.EN must be
        // observable — value-aligned in both worlds (the oracle's regfile and
        // QEMU's set_tctl both return the just-written value).
        if self.regs.tctl.read_field(e1000::tctl::EN) == 0 {
            regseq::log::error!("e1000 tx enable readback failed");
            return Err(RegseqError::HardwareError);
        }
        // Receive filters via runtime indices (donor call shape m=3 v=5 r=1).
        self.setup_filters(3, 5, 1);
        // Read-clear ICR (reading clears latched causes). -> R32 @0x00C0
        let _icr = self.regs.icr.read();
        regseq::log::info!("e1000 up");
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        // Orderly teardown: defensively mask every interrupt source, then
        // quiesce TX. Plain whole-register writes — value-aligned in both
        // worlds.
        regseq::log::warn!("e1000 deinit: masking interrupts");
        self.regs.imc.write(ALL_INTERRUPTS_MASK);
        self.regs.tctl.write(0);
        Ok(())
    }

    fn on_surprise_remove(&mut self) -> Result<(), RegseqError> {
        // The device is GONE: no MMIO is legal here (the linux_c surprise arm
        // runs no MMIO-touching releases — GRILL D06(3)); log-only body.
        regseq::log::error!("e1000 surprise remove: device gone, skipping hw teardown");
        Ok(())
    }

    fn on_init_failure(&mut self) -> Result<(), RegseqError> {
        // `on_init` returned Err: clean up partial state. Log-only — the
        // devres unwind owns resource release (ratified A-2).
        regseq::log::error!("e1000 init failed: cleaning up partial state");
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main` (RD-4/RD-6). The
// manifest's `lifecycle_scenario = "orderly"` lowers to: register
// (match=pci:0x8086:0x100e) → create → associate → resource preamble → bus
// acquisition (enable_device, claim_regions(0), get_handle,
// map_region(0xf0000000, 0x20000, 0)) → `LC call on_init` (config ops + the
// bring-up above through the based accessor) → `LC call on_deinit` (quiesce)
// → teardown.
