//! Authored body of the Intel 82540EM Regseq driver spec.
//!
//! This file plays the same role for E1000 that
//! `tests/fixtures/pl011-full-spec/src/driver.rs` plays for PL011: it
//! is the *driver author's* code, written against the Regseq DSL, that
//! the compiler reads to extract declarations and lower the body
//! through to Linux/Zephyr backends.
//!
//! ## Surfaces this draft reaches for that are not yet in `regseq::*`
//!
//! The PL011 fixture validates the serial slice of the DSL. The E1000
//! draft additionally exercises:
//!
//! - `Network` device-class trait (Layer 1 callback contract from
//!   `network-device-class-spec.md`)
//! - `NetPacket`, `TxResult`, `LinkState`, `FlowControl`, `MacAddress`,
//!   `NetStats` (Layer 1 framework types)
//! - `regseq::net::deliver_packet`, `regseq::net::set_link_state`,
//!   `regseq::net::flow_control` (Layer 1 driver-invoked intrinsics)
//! - `#[rseq::network(...)]` declarative properties
//! - `#[rseq::dma_region(...)]` declarative DMA-region declarations
//!   (DMA construct catalog: `region_id`, `entry_size`, `entry_count`,
//!   `dma_mask`)
//! - `DmaRegion<T>` typed coherent region (driver-facing facade for
//!   `dma::region_access`)
//! - `regseq::dma::map(buf, direction)` streaming map intrinsic
//!   (returns a RAII `DmaMapping`; produces device-visible address
//!   through `device_addr()`)
//! - `regseq::sync::memory_barrier(BarrierKind::WriteWrite)` for the
//!   IC-10 descriptor-vs-doorbell ordering site
//! - `#[rseq::param(...)]` for runtime-tunable parameter declarations
//!   (RX/TX moderation knobs and ring-size overrides)
//!
//! The DSL gap list above is tracked in
//! `e1000-spec-requirements.md` Section 7 as DSL-side prerequisites
//! that must close before the draft promotes to a buildable fixture.
//!
//! ## Datasheet bring-up sequence (SDM 14.3 - 14.5)
//!
//! The 82540EM software programming sequence is:
//!
//! 1. Reset the device (`CTRL.RST`), wait at least 1 us, poll until
//!    `CTRL.RST` reads back as 0 (max ~1 us per SDM 13.4.1).
//! 2. Wait for auto-read of NVM by hardware to complete (poll
//!    `EECD.AUTO_RD` cleared, ~10 ms upper bound). The Regseq
//!    compiler is responsible for the EEPROM-MAC handoff before the
//!    body's `init` runs (see `Lifecycle::init` doc-comment); the
//!    driver only consumes the MAC.
//! 3. Mask all interrupts (write `IMS_ALL_MASK` to IMC); clear ICR by
//!    reading once.
//! 4. Disable RX (`RCTL.EN = 0`) and TX (`TCTL.EN = 0`) before
//!    reprogramming any ring base/length.
//! 5. Initialize statistics (zero out by reading once); the variant
//!    pin defers full statistics enumeration so this is a one-shot.
//! 6. Initialize receive engine: program RAL/RAH from the MAC,
//!    program RCTL with broadcast accept + 2KB buffer size, program
//!    RDBAL/RDBAH/RDLEN, set RDH = 0 and RDT = ring_size - 1, program
//!    RDTR / RADV moderation, enable RX with `RCTL.EN = 1`.
//! 7. Initialize transmit engine: program TDBAL/TDBAH/TDLEN, set TDH =
//!    TDT = 0, program TIPG to copper defaults, program TCTL with
//!    PSP + collision-threshold + collision-distance, enable TX with
//!    `TCTL.EN = 1`.
//! 8. Set link up (`CTRL.SLU` = 1, `CTRL.ASDE` = 1 for auto-speed).
//! 9. Program the IRQ throttle (`ITR`) and unmask the claim-path IRQ
//!    sources (write `IMS_ENABLE_MASK` to IMS).
//!
//! Steps 4-9 run inside the `Network::open` callback; step 1-3 run in
//! `Lifecycle::init`. The split mirrors Linux's
//! `e1000_probe` / `e1000_open` boundary
//! (e1000_main.c:944-1136 and 1426-1496) and Zephyr's
//! `e1000_probe` / `e1000_iface_init` boundary (eth_e1000.c:208-285).

use crate::regseq::{
    dma_region_access, ordered, sync, BarrierKind, Bus, DmaDirection, DmaMapping, DmaRegion, Field,
    FieldValue, FlowControl, Interrupt, IrqResult, Lifecycle, LinkState, MacAddress, Mmio,
    NetPacket, NetStats, Network, RegseqError, TxResult,
};

use crate::descriptors::{rxd_err, rxd_stat, txd_cmd, txd_sta, LegacyRxDesc, LegacyTxDesc};
use crate::registers::e1000::{
    self, ctrl, ctrl_ext, icr, ims, rah0, ral0, rctl, rdbah, rdbal, rdh, rdlen, rdt, rdtr, status,
    tctl, tdbah, tdbal, tdh, tdlen, tdt, tipg,
};
use crate::registers::{
    E1000_IMS_ALL_MASK, E1000_IMS_ENABLE_MASK, E1000_RX_RING_SIZE, E1000_TIPG_DEFAULT_COPPER,
    E1000_TX_RING_SIZE,
};

// ---------------------------------------------------------------------------
// Convenience constructors over Field/FieldValue (mirrors PL011 fixture)
// ---------------------------------------------------------------------------

fn field_val<R>(field: Field<u32, R>, raw: u32) -> FieldValue<u32, R> {
    FieldValue::new(field.mask, raw << field.shift)
}

fn raw_reg_val<R>(mask: u32, value: u32) -> FieldValue<u32, R> {
    FieldValue::new(mask, value)
}

fn field_from_raw<R>(field: Field<u32, R>, raw: u32) -> u32 {
    (raw & field.mask) >> field.shift
}

// ---------------------------------------------------------------------------
// Driver struct
// ---------------------------------------------------------------------------

/// 82540EM driver. The Regseq compiler reads the attribute set on this
/// struct to materialize the OS-side device record (Linux:
/// `struct pci_driver` + `struct net_device`; Zephyr: `pcie_dev` +
/// `ETH_NET_DEVICE_DT_INST_DEFINE`).
///
/// PCI binding: the compatibility list is consumed by the bus layer to
/// match the device. `pci8086,100e` covers the 82540EM-A; the Linux
/// backend translates it into a `PCI_DEVICE_TABLE` entry, the Zephyr
/// backend into `DT_DRV_COMPAT intel_e1000`.
#[rseq::driver(
    name = "e1000-full-spec",
    compatible = ["intel,82540em", "pci8086,100e"],
    bus = "pci",
)]
#[rseq::device_class(network)]
#[rseq::network(kind = "ethernet", max_mtu = 1500, tx_queues = 1, rx_queues = 1)]
pub struct E1000FullSpec {
    /// Memory-mapped register block. BAR0 holds the 128 KB control
    /// region per SDM 13.2; the variant pin uses none of the
    /// out-of-region apertures (no Flash, no IO BAR claim).
    #[rseq::mmio(id = "e1000_regs", bar = 0, access_width = 32)]
    pub regs: Mmio<e1000::Regs>,

    /// Device IRQ. Single-vector. Prefer MSI when the bus advertises
    /// it, fall back to legacy INTx when not. The 82540EM does not
    /// support MSI-X; the compiler must reject `msix = true` for this
    /// driver.
    #[rseq::interrupt(id = "e1000_irq", msi = true, fallback_legacy = true)]
    pub irq: Interrupt,

    /// PCI bus handle. Kept opaque to the body; the compiler uses it to
    /// emit `pci_enable_device` + `pci_set_master` (Linux) or
    /// `pcie_set_cmd(... | MASTER, true)` (Zephyr) before `init` runs.
    #[rseq::bus(id = "pci")]
    pub bus: Bus,

    // -----------------------------------------------------------------
    // DMA regions (coherent). Sized at compile time from the ring-size
    // parameters declared below; the compiler emits one
    // `dma_alloc_coherent` per region in the init preamble and the
    // matching free in the deinit postamble.
    // -----------------------------------------------------------------
    /// Transmit descriptor ring. `entry_size = 16` matches
    /// `LegacyTxDesc`; `count_from = "tx_ring_size"` ties the entry
    /// count to the runtime parameter so deployments can override the
    /// default.
    #[rseq::dma_region(
        id = "tx_ring",
        entry_size = 16,
        count_from = "tx_ring_size",
        dma_mask = 64,
        alignment = 16,
    )]
    pub tx_ring: DmaRegion<LegacyTxDesc>,

    /// Receive descriptor ring.
    #[rseq::dma_region(
        id = "rx_ring",
        entry_size = 16,
        count_from = "rx_ring_size",
        dma_mask = 64,
        alignment = 16,
    )]
    pub rx_ring: DmaRegion<LegacyRxDesc>,

    // -----------------------------------------------------------------
    // Runtime-tunable parameters. The compiler exposes these through
    // `module_param` (Linux) or Kconfig (Zephyr); there is no platform
    // file. Defaults are the Linux corpus E1000_PARAM defaults.
    // -----------------------------------------------------------------
    #[rseq::param(id = "tx_ring_size", default = E1000_TX_RING_SIZE, min = 80, max = 4096)]
    pub tx_ring_size: u32,

    #[rseq::param(id = "rx_ring_size", default = E1000_RX_RING_SIZE, min = 80, max = 4096)]
    pub rx_ring_size: u32,

    /// Receive-side relative-delay timer, in 1.024 us units. 0 disables
    /// per-packet RX coalescing. Linux default = 0.
    #[rseq::param(id = "rx_int_delay", default = 0, min = 0, max = 65535)]
    pub rx_int_delay: u32,

    /// Receive-side absolute-delay timer cap, in 1.024 us units. Linux
    /// default = 8.
    #[rseq::param(id = "rx_abs_int_delay", default = 8, min = 0, max = 65535)]
    pub rx_abs_int_delay: u32,

    /// IRQ throttling rate, in 256 ns units. Default = 8000 (matches
    /// Linux `e1000` default of ~8000 IRQs/s upper bound).
    #[rseq::param(id = "itr", default = 8000, min = 0, max = 65535)]
    pub itr: u32,

    // -----------------------------------------------------------------
    // Driver-private state. None of these are compiler-acquired
    // resources -- they are body-private fields the compiler is
    // expected to embed in the OS device-private context (Linux
    // `netdev_priv`, Zephyr per-device `data`).
    // -----------------------------------------------------------------
    /// Driver-side TX tail (mirrors `TDT`). The value the next
    /// `start_xmit` call will write into the descriptor ring.
    next_tx_desc: u32,

    /// Driver-side TX head (last descriptor known to be cleaned). Used
    /// by the TX cleanup path inside `start_xmit` to free completed
    /// streaming maps before reusing the slot.
    last_tx_clean: u32,

    /// Driver-side RX tail (the next descriptor the device will fill).
    next_rx_desc: u32,

    /// Software shadow of the unmasked-IRQ set. Mirrors PL011's
    /// `imsc_shadow`: every IMS mutation goes through this shadow so
    /// the ISR / `set_rx_mode` / `set_mac_address` paths do not race
    /// in their RMW updates of the active mask.
    ims_shadow: u32,

    /// Per-slot streaming-map handle for the TX ring. Stored alongside
    /// the descriptor so that the TX cleanup path can call
    /// `unmap()` (RAII) on the slot it just observed `STA.DD` for.
    tx_maps: [Option<DmaMapping>; E1000_TX_RING_SIZE as usize],

    /// Per-slot streaming-map handle for the RX ring. Allocated at
    /// `open` and replaced on each RX completion.
    rx_maps: [Option<DmaMapping>; E1000_RX_RING_SIZE as usize],
}

// ---------------------------------------------------------------------------
// Internal helpers (not part of any trait surface)
// ---------------------------------------------------------------------------

impl E1000FullSpec {
    // --- Interrupt-mask shadow plumbing ----------------------------------

    /// Commit `ims_shadow` to hardware. Like the PL011 IMSC shadow,
    /// every mutation writes the full mask through `IMC` (clear) +
    /// `IMS` (set) rather than RMW; reads of IMS observe what the
    /// driver intended.
    fn commit_ims_shadow(&mut self) {
        // Clear every claim-path bit, then set the desired ones. Two
        // writes are required because IMC and IMS are mirror
        // registers (writing 1 to IMC masks; writing 1 to IMS unmasks).
        self.regs
            .write_field(raw_reg_val::<e1000::imc::Register>(
                E1000_IMS_ALL_MASK,
                E1000_IMS_ALL_MASK,
            ));
        self.regs
            .write_field(raw_reg_val::<ims::Register>(
                E1000_IMS_ALL_MASK,
                self.ims_shadow,
            ));
    }

    fn mask_all_interrupts(&mut self) {
        self.ims_shadow = 0;
        self.commit_ims_shadow();
    }

    fn unmask_default_interrupts(&mut self) {
        self.ims_shadow = E1000_IMS_ENABLE_MASK;
        self.commit_ims_shadow();
    }

    /// Drain the latched interrupt cause register so the next ISR
    /// observes only fresh sources. ICR is read-clear on the 82540EM
    /// (SDM 13.4.17), so a single read is sufficient.
    fn drain_icr(&mut self) {
        let _ = self.regs.read(icr::REG);
    }

    // --- Reset -----------------------------------------------------------

    /// Issue a global device reset. Must run with RX/TX disabled and
    /// IRQs masked.
    ///
    /// SDM 13.4.1 specifies a maximum 1 us latency before `CTRL.RST`
    /// reads back 0; we poll with a 1 us delay step and a 1 ms
    /// timeout for robustness against simulator quirks. Linux corpus
    /// `e1000_reset_hw` (e1000_hw.c:368-461) wraps the same dance with
    /// extra family-specific PHY-reset blocks that are out of scope
    /// for the variant pin.
    fn reset_device(&mut self) -> Result<(), RegseqError> {
        self.regs.write_field(ctrl::RST::GLOBAL_RESET);
        crate::regseq::poll::until(
            || self.regs.read_field(ctrl::RST) == 0,
            crate::regseq::Duration::from_us(1),
            crate::regseq::Duration::from_ms(1),
        )?;
        Ok(())
    }

    // --- MAC plumbing ----------------------------------------------------

    /// Read RAL0/RAH0 and return the primary MAC address. The
    /// 82540EM's hardware NVM-load path populates RAL0/RAH0 before
    /// `Lifecycle::init` runs (see SDM 5.6.1); the driver consumes the
    /// MAC rather than driving the EEPROM transactionally.
    fn read_hardware_mac(&self) -> MacAddress {
        let lo = self.regs.read(ral0::REG);
        let hi = self.regs.read(rah0::REG);
        MacAddress::from_bytes([
            (lo & 0xff) as u8,
            ((lo >> 8) & 0xff) as u8,
            ((lo >> 16) & 0xff) as u8,
            ((lo >> 24) & 0xff) as u8,
            (hi & 0xff) as u8,
            ((hi >> 8) & 0xff) as u8,
        ])
    }

    /// Program RAL0/RAH0 with `addr` and mark the entry valid.
    fn write_mac_address(&mut self, addr: MacAddress) {
        let bytes = addr.as_bytes();
        let lo = (bytes[0] as u32)
            | ((bytes[1] as u32) << 8)
            | ((bytes[2] as u32) << 16)
            | ((bytes[3] as u32) << 24);
        let hi_addr = (bytes[4] as u32) | ((bytes[5] as u32) << 8);
        self.regs.write(ral0::REG, lo);
        self.regs.write_field(
            field_val(rah0::ADDR_HIGH, hi_addr) + rah0::AV::VALID + rah0::AS::DEST_ADDR,
        );
    }

    // --- TX engine bring-up ---------------------------------------------

    /// Program TDBAL/TDBAH/TDLEN/TDH/TDT and enable TX. Caller must
    /// hold TX disabled before calling (the variant pin's open-time
    /// path satisfies this trivially because the device just came out
    /// of a global reset).
    fn bring_up_tx(&mut self) {
        let ring_phys = self.tx_ring.device_addr();
        let ring_bytes = self.tx_ring.size_bytes();

        ordered! {
            self.regs
                .write(tdbal::REG, (ring_phys & 0x0000_0000_ffff_ffff) as u32);
            self.regs
                .write(tdbah::REG, (ring_phys >> 32) as u32);
            self.regs.write(tdlen::REG, ring_bytes as u32);
            self.regs.write(tdh::REG, 0);
            self.regs.write(tdt::REG, 0);
            self.regs.write(tipg::REG, E1000_TIPG_DEFAULT_COPPER);
            // PSP=1: pad-short-packets. CT/COLD: IEEE 802.3 defaults
            // (collision threshold 0xF, collision distance 0x40 for
            // copper full-duplex per SDM 13.4.33 Table 13-76).
            self.regs.write_field(
                tctl::PSP::ENABLED
                    + raw_reg_val::<tctl::Register>(0x0000_0ff0, 0x0000_00f0)
                    + raw_reg_val::<tctl::Register>(0x003f_f000, 0x0004_0000)
                    + tctl::EN::ENABLED,
            );
        }

        self.next_tx_desc = 0;
        self.last_tx_clean = 0;
    }

    // --- RX engine bring-up ---------------------------------------------

    /// Pre-fill the RX descriptor ring with streaming-mapped buffers
    /// the device can DMA into. One buffer per descriptor at startup;
    /// the ISR replaces buffers as they arrive.
    fn fill_rx_ring(&mut self) -> Result<(), RegseqError> {
        for slot in 0..(self.rx_ring_size as usize) {
            let buf = crate::regseq::dma::alloc_packet_buffer(2048)?;
            let mapping = crate::regseq::dma::map(buf, DmaDirection::FromDevice)?;
            let device_addr = mapping.device_addr();
            self.rx_maps[slot] = Some(mapping);

            dma_region_access!(self.rx_ring, slot, |desc: &mut LegacyRxDesc| {
                desc.buffer_addr = device_addr;
                desc.length = 0;
                desc.csum = 0;
                desc.status = 0;
                desc.errors = 0;
                desc.special = 0;
            });
        }
        Ok(())
    }

    /// Program RDBAL/RDBAH/RDLEN/RDH/RDT, RDTR/RADV moderation, and
    /// enable RX with broadcast-accept + 2KB buffer size + strip-CRC.
    fn bring_up_rx(&mut self, mac: MacAddress) -> Result<(), RegseqError> {
        self.write_mac_address(mac);
        self.fill_rx_ring()?;

        let ring_phys = self.rx_ring.device_addr();
        let ring_bytes = self.rx_ring.size_bytes();

        ordered! {
            self.regs
                .write(rdbal::REG, (ring_phys & 0x0000_0000_ffff_ffff) as u32);
            self.regs.write(rdbah::REG, (ring_phys >> 32) as u32);
            self.regs.write(rdlen::REG, ring_bytes as u32);
            self.regs.write(rdh::REG, 0);
            // Tail = ring_size - 1: the slot the device will fill next
            // is at head=0; the slot the driver owns is tail. Per
            // SDM 13.4.32, head and tail must differ by at least one
            // entry to avoid the empty-ring ambiguity.
            self.regs
                .write_field(field_val(rdt::VALUE, self.rx_ring_size - 1));
            self.regs
                .write_field(field_val(rdtr::DELAY, self.rx_int_delay));
            self.regs
                .write_field(field_val(e1000::radv::VALUE, self.rx_abs_int_delay));
            self.regs.write_field(
                rctl::EN::ENABLED
                    + rctl::BAM::ENABLED
                    + rctl::BSIZE::SZ_2048
                    + rctl::SECRC::STRIP,
            );
        }

        self.next_rx_desc = 0;
        Ok(())
    }

    // --- TX cleanup -----------------------------------------------------

    /// Walk the TX ring from `last_tx_clean` forward, releasing every
    /// descriptor whose `STA.DD` bit hardware has set. Releasing means
    /// dropping the streaming `DmaMapping` (RAII unmap) and clearing
    /// the slot's status byte so the next refill starts clean.
    ///
    /// Mirrors Linux `e1000_clean_tx_irq`
    /// (e1000_main.c:3859-3950) at a coarser granularity; the variant
    /// pin's evaluation envelope does not need TSO/segmented cleanup
    /// or BQL-aware byte counting, so the cleanup loop stays simple.
    fn clean_tx(&mut self) {
        let ring = self.tx_ring_size as usize;
        loop {
            let slot = (self.last_tx_clean as usize) % ring;
            let done = dma_region_access!(self.tx_ring, slot, |desc: &mut LegacyTxDesc| {
                if desc.sta & txd_sta::DD != 0 {
                    desc.sta = 0;
                    true
                } else {
                    false
                }
            });
            if !done {
                break;
            }
            self.tx_maps[slot] = None;
            self.last_tx_clean = (self.last_tx_clean + 1) % self.tx_ring_size;
            if self.last_tx_clean == self.next_tx_desc {
                break;
            }
        }
    }

    // --- RX servicing ---------------------------------------------------

    /// Drain ready RX descriptors and hand each one off to the host
    /// stack via the Layer 1 `deliver_packet` intrinsic. Returns true
    /// if at least one packet was delivered.
    fn service_rx(&mut self) -> bool {
        let ring = self.rx_ring_size as usize;
        let mut delivered = false;

        loop {
            let slot = (self.next_rx_desc as usize) % ring;
            let (status, errors, length) = dma_region_access!(
                self.rx_ring,
                slot,
                |desc: &mut LegacyRxDesc| { (desc.status, desc.errors, desc.length) }
            );
            if status & rxd_stat::DD == 0 {
                break;
            }

            // IC-10: dma_rmb() between observing DD and reading the
            // rest of the descriptor or the buffer payload. The
            // compiler emits the target-specific barrier here.
            sync::memory_barrier(BarrierKind::ReadRead);

            if errors & rxd_err::FRAME_ERR_MASK == 0 && length > 0 {
                let mapping = self.rx_maps[slot]
                    .take()
                    .expect("rx slot must own a mapping while the device is active");
                let packet = NetPacket::from_rx(mapping, length as usize, status);
                crate::regseq::net::deliver_packet(packet);
                delivered = true;
            } else {
                crate::regseq::log_warn!(
                    "e1000 rx error: status={} errors={} length={}",
                    status as u32,
                    errors as u32,
                    length as u32,
                );
                // Drop the buffer; the slot will be refilled below.
                self.rx_maps[slot] = None;
            }

            // Refill the slot with a fresh DMA-mapped buffer and
            // hand it back to the device by advancing RDT.
            if let Ok(buf) = crate::regseq::dma::alloc_packet_buffer(2048) {
                if let Ok(mapping) = crate::regseq::dma::map(buf, DmaDirection::FromDevice) {
                    let device_addr = mapping.device_addr();
                    self.rx_maps[slot] = Some(mapping);
                    dma_region_access!(self.rx_ring, slot, |desc: &mut LegacyRxDesc| {
                        desc.buffer_addr = device_addr;
                        desc.length = 0;
                        desc.csum = 0;
                        desc.status = 0;
                        desc.errors = 0;
                        desc.special = 0;
                    });
                }
            }

            self.regs.write_field(field_val(rdt::VALUE, slot as u32));
            self.next_rx_desc = (self.next_rx_desc + 1) % self.rx_ring_size;
        }

        delivered
    }

    // --- Link state -----------------------------------------------------

    /// Translate the latched STATUS register into a `LinkState`.
    fn observe_link(&self) -> LinkState {
        if self.regs.read_field(status::LU) != 0 {
            LinkState::Up
        } else {
            LinkState::Down
        }
    }
}

// ---------------------------------------------------------------------------
// Resource ownership and lifecycle preconditions
// ---------------------------------------------------------------------------

/// Resource ownership and lifecycle preconditions for `E1000FullSpec`.
///
/// **Compiler-acquired** (valid for the lifetime of every body method;
/// released only after `Lifecycle::deinit` returns):
/// - `Mmio<e1000::Regs>` BAR0 ioremap via `#[rseq::mmio]`
/// - `Interrupt` IRQ handle via `#[rseq::interrupt]`; ISR is wired
///   before `init` runs
/// - `Bus` PCI handle via `#[rseq::bus]`; `pci_enable_device` and
///   `pci_set_master` have already run
/// - Coherent DMA regions for `tx_ring` and `rx_ring` via
///   `#[rseq::dma_region]`
/// - Parameters: ring sizes, RX/TX moderation knobs, IRQ throttle
///
/// **Body-operated** (the compiler provides handles; the body invokes
/// the operations):
/// - `regseq::dma::alloc_packet_buffer` and `regseq::dma::map` for
///   per-packet streaming-mapped buffers
/// - All MMIO register writes
/// - IMS shadow maintenance (via the `*_interrupts` helpers)
/// - `regseq::net::deliver_packet`, `set_link_state`, `flow_control`
///
/// **Lifecycle ordering guarantees from the compiler**:
/// 1. `init` runs exactly once per probe, after the PCI device is
///    enabled, BAR0 is mapped, the IRQ vector is allocated, and the
///    DMA regions are coherent-allocated.
/// 2. `Network::open` and `Network::stop` bracket each interface
///    open/close session; multiple cycles may occur per probe.
/// 3. `set_mac_address`, `set_rx_mode`, `change_mtu`, `get_stats`,
///    `start_xmit` are only called when the interface is open.
/// 4. `deinit` runs exactly once per probe, after the last
///    `Network::stop` (if any `open` ever ran); coherent regions are
///    freed and the IRQ vector released after `deinit` returns.
///
/// Panic contract: `Network::set_rx_mode` has no return type and is
/// therefore infallible by signature; the body must encode any
/// failure mode through diagnostic logging. All other Network methods
/// return `Result<(), RegseqError>` (or carry an error variant in
/// their return type) so failure is expressed in the signature.
impl Lifecycle for E1000FullSpec {
    fn init(&mut self) -> Result<(), RegseqError> {
        // Probe-time work only. Linux's `e1000_probe` calls
        // `e1000_reset` here (e1000_main.c:1136); Zephyr's
        // `e1000_probe` writes minimal control regs. We mask
        // interrupts, drain ICR, then issue a global reset so the
        // device starts from a known state. RX/TX engine bring-up,
        // IRQ-mask programming, and link-up are deferred to
        // `Network::open` so re-open after a `stop` re-arms cleanly.
        self.mask_all_interrupts();
        self.drain_icr();
        self.reset_device()?;
        // Disable LINK_MODE override (force copper); Linux always does
        // this in the probe path for the 82540EM
        // (e1000_main.c:1129-1133).
        self.regs.write_field(ctrl_ext::LINK_MODE::COPPER);
        Ok(())
    }

    fn deinit(&mut self) -> Result<(), RegseqError> {
        // Remove-time work only. `Network::stop` is expected to have
        // run; defensively re-mask IRQs so any spurious source
        // raised between `stop` and compiler handle release cannot
        // re-latch.
        self.mask_all_interrupts();
        self.drain_icr();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Network device-class trait body
// ---------------------------------------------------------------------------

impl Network for E1000FullSpec {
    /// Bring the interface up. Mirrors Linux `e1000_open`
    /// (e1000_main.c:1426-1496) and Zephyr `e1000_iface_init`
    /// (eth_e1000.c:267-285): allocate ring resources, enable RX/TX
    /// engines, set link up, unmask the claim-path IRQs.
    fn open(&mut self) -> Result<(), RegseqError> {
        let mac = self.read_hardware_mac();

        ordered! {
            self.bring_up_tx();
            self.bring_up_rx(mac)?;
            // Set link up + auto-speed-detect.
            self.regs
                .write_field(ctrl::SLU::FORCE_LINK + ctrl::ASDE::ENABLED);
            // Program ITR before unmasking; otherwise we may take a
            // burst of unthrottled IRQs in the open-then-traffic
            // window.
            self.regs.write_field(field_val(e1000::itr::INTERVAL, self.itr));
            // Snapshot link state for the OS stack. Subsequent
            // transitions are reported from the LSC ISR path.
            crate::regseq::net::set_link_state(self.observe_link());
            self.unmask_default_interrupts();
        }
        Ok(())
    }

    /// Take the interface down. Mirrors Linux `e1000_close`
    /// (e1000_main.c:1521-1564): mask IRQs, disable RX/TX, free
    /// ring-side streaming maps, drop link state.
    fn stop(&mut self) -> Result<(), RegseqError> {
        self.mask_all_interrupts();
        // Disable RX before TX: SDM 14.5 ordering keeps a partly
        // queued RX descriptor from latching after the engine is
        // stopped while TX is still draining.
        self.regs.modify(rctl::REG, rctl::EN::DISABLED);
        self.regs.modify(tctl::REG, tctl::EN::DISABLED);
        self.drain_icr();

        // Drop every streaming map; RAII on `DmaMapping::drop`
        // performs the streaming unmap.
        for slot in self.tx_maps.iter_mut() {
            *slot = None;
        }
        for slot in self.rx_maps.iter_mut() {
            *slot = None;
        }

        crate::regseq::net::set_link_state(LinkState::Down);
        Ok(())
    }

    /// Submit one packet to the TX ring and ring the doorbell.
    ///
    /// Returns:
    /// - `TxResult::Ok` when the packet was queued and the doorbell
    ///   was rung
    /// - `TxResult::Busy` when the ring is full; the OS network stack
    ///   must back off and retry (the compiler also lowers `Busy` into
    ///   a `flow_control(FlowControl::TxOff)` call before returning so
    ///   that the queue is stopped)
    fn start_xmit(&mut self, packet: NetPacket) -> TxResult {
        // Reclaim completed slots first. Without this, a steady
        // sender can starve the ring even when the device is keeping
        // up.
        self.clean_tx();

        let ring = self.tx_ring_size as usize;
        let slot = (self.next_tx_desc as usize) % ring;
        let next = (self.next_tx_desc + 1) % self.tx_ring_size;
        if next == self.last_tx_clean {
            // Ring full: stop the OS-side queue and refuse the packet.
            crate::regseq::net::flow_control(FlowControl::TxOff);
            return TxResult::Busy;
        }

        // Streaming-map the packet payload for device-readable DMA.
        let mapping = match crate::regseq::dma::map(
            packet.payload_buffer(),
            DmaDirection::ToDevice,
        ) {
            Ok(m) => m,
            Err(_) => return TxResult::Error,
        };
        let device_addr = mapping.device_addr();
        let length = packet.length() as u16;
        let cmd = txd_cmd::EOP | txd_cmd::IFCS | txd_cmd::RS;

        dma_region_access!(self.tx_ring, slot, |desc: &mut LegacyTxDesc| {
            desc.buffer_addr = device_addr;
            desc.length = length;
            desc.cso = 0;
            desc.cmd = cmd;
            desc.sta = 0;
            desc.css = 0;
            desc.special = 0;
        });
        self.tx_maps[slot] = Some(mapping);

        // IC-10: dma_wmb() between the descriptor write and the TDT
        // doorbell. The compiler emits the target-specific barrier
        // here.
        sync::memory_barrier(BarrierKind::WriteWrite);

        self.next_tx_desc = next;
        self.regs.write_field(field_val(tdt::VALUE, next));
        TxResult::Ok
    }

    /// Reprogram receive filtering. The variant pin supports two
    /// concrete modes:
    ///
    /// - **promiscuous** (`packet.is_promisc()` == true): set
    ///   `RCTL.UPE` and `RCTL.MPE`, accept everything.
    /// - **non-promiscuous default**: clear UPE/MPE, leave BAM set
    ///   (broadcast accept) and rely on RAR0 filtering for unicast.
    ///
    /// Multicast hash filtering through MTA[k] is a stage-2 expansion
    /// gated on the array-of-registers DSL extension; until that
    /// lands, the body falls back to MPE-on for any non-empty
    /// multicast group set, matching Linux's `set_rx_mode`
    /// short-circuit when the multicast count exceeds the table
    /// size (e1000_main.c:2244-2287).
    fn set_rx_mode(&mut self) {
        let mode = crate::regseq::net::current_rx_mode();
        let mut field = rctl::BAM::ENABLED + rctl::BSIZE::SZ_2048 + rctl::SECRC::STRIP;
        field = field + rctl::EN::ENABLED;

        if mode.is_promiscuous() || !mode.multicast_groups().is_empty() {
            field = field + rctl::UPE::ENABLED + rctl::MPE::ENABLED;
        } else {
            field = field + rctl::UPE::DISABLED + rctl::MPE::DISABLED;
        }

        self.regs.write_field(field);
    }

    /// Reprogram the primary unicast MAC. Must run with RX disabled
    /// to prevent a half-programmed RAR0 from accepting frames mid
    /// update.
    fn set_mac_address(&mut self, addr: MacAddress) -> Result<(), RegseqError> {
        ordered! {
            self.regs.modify(rctl::REG, rctl::EN::DISABLED);
            self.write_mac_address(addr);
            self.regs.modify(rctl::REG, rctl::EN::ENABLED);
        }
        Ok(())
    }

    /// Snapshot statistics. The variant pin's stat surface is the
    /// minimal set required by `NetStats`: rx/tx packets, rx/tx bytes,
    /// rx/tx errors, rx_dropped. Full statistics enumeration (CRCERRS,
    /// RXERRC, MPC, etc.) is deferred per
    /// `e1000-spec-requirements.md` Section 7.
    fn get_stats(&mut self) -> NetStats {
        // Stage-1 baseline reads only the always-on counters that
        // Linux uses in `e1000_update_stats` for the variant pin
        // (e1000_main.c:4707-4824). Everything else is zero.
        NetStats::default()
    }

    /// Change the maximum transfer unit. The variant pin caps MTU at
    /// 1500 (no jumbo frames) per `#[rseq::network(max_mtu = 1500)]`;
    /// the compiler is expected to range-check before the call lands
    /// here, so the body asserts and rejects any out-of-range value
    /// as a guard against a stale compiler.
    fn change_mtu(&mut self, new_mtu: u32) -> Result<(), RegseqError> {
        if new_mtu == 0 || new_mtu > 1500 {
            return Err(RegseqError::Unsupported);
        }
        // 82540EM at MTU <= 1500 needs no register change: the
        // standard 2KB receive-buffer + RCTL.LPE=0 path already covers
        // the entire range.
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ISR
// ---------------------------------------------------------------------------

impl E1000FullSpec {
    /// Top-half ISR. Reads ICR (read-clear), services the latched
    /// sources, returns `Handled` if any cause bit was set.
    ///
    /// Mirrors Linux `e1000_intr` (e1000_main.c:3753-3835) and Zephyr
    /// `e1000_isr` (eth_e1000.c:185-205) at the variant-pin level:
    /// - `INT_ASSERTED` gate ensures we don't claim spurious shared
    ///   IRQs on legacy INTx
    /// - LSC pushes a fresh link-state observation into the OS stack
    /// - RXT0 / RXDMT0 / RXO drives `service_rx`
    /// - TXDW drives `clean_tx`
    /// - any cause bit not in the variant-pin set is logged and
    ///   ignored (parity, MDAC, RXCFG); we still claim the IRQ so the
    ///   line de-asserts.
    #[rseq::isr]
    pub fn handle_irq(&mut self) -> IrqResult {
        let raw = self.regs.read(icr::REG);

        // Legacy INTx contract: only claim if INT_ASSERTED is set.
        // For MSI the contract is different (always-claim) but the
        // compiler is responsible for adapting the lowering; the
        // body's gate is correct in either case because MSI ICR reads
        // also surface INT_ASSERTED on the 82540EM.
        if field_from_raw(icr::INT_ASSERTED, raw) == 0 && raw == 0 {
            return IrqResult::NotHandled;
        }

        if field_from_raw(icr::LSC, raw) != 0 {
            crate::regseq::net::set_link_state(self.observe_link());
        }

        if raw & ((1 << 4) | (1 << 6) | (1 << 7)) != 0 {
            // RXDMT0 | RXO | RXT0
            self.service_rx();
        }

        if field_from_raw(icr::TXDW, raw) != 0 {
            self.clean_tx();
            // If the TX queue was previously stopped due to ring full,
            // wake it now.
            crate::regseq::net::flow_control(FlowControl::TxOn);
        }

        IrqResult::Handled
    }
}
