//! E1000 legacy TX / RX descriptor formats.
//!
//! These structs occupy coherent DMA memory allocated by the compiler
//! through `#[rseq::dma_region]` declarations on the driver struct.
//! Per `dma-construct-spec.md`, the descriptor *format* is a
//! device-class concern (NIC-specific layout); the DMA construct
//! provides the coherent-region allocation and barrier-emission
//! contract underneath.
//!
//! The 82540EM supports both legacy and extended descriptor formats.
//! The variant pin uses **legacy** descriptors: legacy TX descriptors
//! cover non-TSO TX paths, legacy RX descriptors cover non-split
//! receive. TSO and packet-split RX are out of scope by the variant
//! pin (see `../README.md` Scope pin section).
//!
//! References:
//! - 82540EM SDM section 3.3.3 (legacy TX descriptor),
//!   3.2.3 (legacy RX descriptor)
//! - Linux corpus `e1000_hw.h:512-660` (legacy descriptor unions and
//!   STAT/ERR bit definitions)
//! - Zephyr corpus `eth_e1000_priv.h:64-82` (struct e1000_tx,
//!   struct e1000_rx)

/// Legacy transmit descriptor. SDM 3.3.3, Table 3-10.
///
/// Layout (16 bytes, aligned to 16):
///
/// ```text
///  0x00 .. 0x07   buffer_addr  (u64, little-endian)
///  0x08 .. 0x09   length       (u16)
///  0x0A           cso          (u8)  -- Checksum Offset
///  0x0B           cmd          (u8)  -- TXD_CMD_*
///  0x0C           sta          (u8)  -- low nibble: TXD_STAT_*; high nibble: rsv
///  0x0D           css          (u8)  -- Checksum Start
///  0x0E .. 0x0F   special      (u16) -- VLAN tag
/// ```
///
/// `cmd` bit values (low byte of CMD field, mirrored from SDM):
/// - `EOP   = 0x01`  End of Packet
/// - `IFCS  = 0x02`  Insert FCS (Ethernet CRC)
/// - `IC    = 0x04`  Insert Checksum (uses `cso`/`css`)
/// - `RS    = 0x08`  Report Status (writes back DD on completion)
/// - `RPS   = 0x10`  Report Packet Sent
/// - `DEXT  = 0x20`  Descriptor Extension (must be 0 for legacy)
/// - `VLE   = 0x40`  VLAN Enable (driver supplies tag in `special`)
/// - `IDE   = 0x80`  Interrupt Delay Enable (uses TIDV/TADV)
///
/// `sta` bit values (low nibble; written back by hardware on completion):
/// - `DD = 0x01`  Descriptor Done
/// - `EC = 0x02`  Excess Collisions
/// - `LC = 0x04`  Late Collision
/// - `TU = 0x08`  Transmit Underrun
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LegacyTxDesc {
    /// DMA-visible buffer address (low 64 bits). The 82540EM is a
    /// 64-bit DMA-capable device but only 64 bits of address are valid
    /// here; bus-physical mapping is the compiler/backend's job
    /// through the `dma::map` streaming surface.
    pub buffer_addr: u64,
    pub length: u16,
    pub cso: u8,
    pub cmd: u8,
    pub sta: u8,
    pub css: u8,
    pub special: u16,
}

/// Legacy receive descriptor. SDM 3.2.3, Table 3-1.
///
/// Layout (16 bytes, aligned to 16):
///
/// ```text
///  0x00 .. 0x07   buffer_addr  (u64, little-endian)
///  0x08 .. 0x09   length       (u16) -- bytes received DMAed
///  0x0A .. 0x0B   csum         (u16) -- Packet Checksum
///  0x0C           status       (u8)  -- RXD_STAT_*
///  0x0D           errors       (u8)  -- RXD_ERR_*
///  0x0E .. 0x0F   special      (u16) -- VLAN tag (when VP set)
/// ```
///
/// `status` bits (mirrored from SDM):
/// - `DD     = 0x01`  Descriptor Done
/// - `EOP    = 0x02`  End of Packet
/// - `IXSM   = 0x04`  Ignore Checksum (TCP/IP)
/// - `VP     = 0x08`  802.1q Packet (VLAN)
/// - `UDPCS  = 0x10`  UDP Checksum Calculated
/// - `TCPCS  = 0x20`  TCP Checksum Calculated
/// - `IPCS   = 0x40`  IP Checksum Calculated
/// - `PIF    = 0x80`  Passed In-exact Filter
///
/// `errors` bits (relevant subset; the rest are documented in SDM 3.2.3.2):
/// - `CE   = 0x01`  CRC error
/// - `SE   = 0x02`  Symbol error
/// - `SEQ  = 0x04`  Sequence error
/// - `CXE  = 0x10`  Carrier extension error
/// - `TCPE = 0x20`  TCP/UDP Checksum Error
/// - `IPE  = 0x40`  IP Checksum Error
/// - `RXE  = 0x80`  Generic RX Data Error
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LegacyRxDesc {
    pub buffer_addr: u64,
    pub length: u16,
    pub csum: u16,
    pub status: u8,
    pub errors: u8,
    pub special: u16,
}

// ---------------------------------------------------------------------------
// Compile-time layout invariants
// ---------------------------------------------------------------------------

pub const TX_DESC_SIZE: usize = core::mem::size_of::<LegacyTxDesc>();
pub const RX_DESC_SIZE: usize = core::mem::size_of::<LegacyRxDesc>();

const _: () = assert!(TX_DESC_SIZE == 16);
const _: () = assert!(RX_DESC_SIZE == 16);
const _: () = assert!(core::mem::align_of::<LegacyTxDesc>() <= 16);
const _: () = assert!(core::mem::align_of::<LegacyRxDesc>() <= 16);

// ---------------------------------------------------------------------------
// Status / command bit constants
// ---------------------------------------------------------------------------

pub mod txd_cmd {
    pub const EOP: u8 = 0x01;
    pub const IFCS: u8 = 0x02;
    pub const IC: u8 = 0x04;
    pub const RS: u8 = 0x08;
    pub const RPS: u8 = 0x10;
    pub const DEXT: u8 = 0x20;
    pub const VLE: u8 = 0x40;
    pub const IDE: u8 = 0x80;
}

pub mod txd_sta {
    pub const DD: u8 = 0x01;
    pub const EC: u8 = 0x02;
    pub const LC: u8 = 0x04;
    pub const TU: u8 = 0x08;
}

pub mod rxd_stat {
    pub const DD: u8 = 0x01;
    pub const EOP: u8 = 0x02;
    pub const IXSM: u8 = 0x04;
    pub const VP: u8 = 0x08;
    pub const UDPCS: u8 = 0x10;
    pub const TCPCS: u8 = 0x20;
    pub const IPCS: u8 = 0x40;
    pub const PIF: u8 = 0x80;
}

pub mod rxd_err {
    pub const CE: u8 = 0x01;
    pub const SE: u8 = 0x02;
    pub const SEQ: u8 = 0x04;
    pub const CXE: u8 = 0x10;
    pub const TCPE: u8 = 0x20;
    pub const IPE: u8 = 0x40;
    pub const RXE: u8 = 0x80;

    /// Frame-error mask: bit-or of the framing-relevant errors that
    /// must surface in `NetPacket` rx-error metadata. Mirrors Linux
    /// corpus `E1000_RXD_ERR_FRAME_ERR_MASK` (e1000_hw.h:596-602).
    pub const FRAME_ERR_MASK: u8 = CE | SE | SEQ | CXE | RXE;
}
