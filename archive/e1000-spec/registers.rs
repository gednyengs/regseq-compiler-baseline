//! Intel 82540EM register-block layout.
//!
//! Coverage scope: registers required by the variant-pinned 82540EM
//! claim path. Includes:
//!
//! - Device control / status / extended control
//! - EEPROM (EECD, EERD) for MAC-from-EEPROM bring-up
//! - MDIC for PHY access (used by the compiler/backend for link
//!   management; the device-class spec keeps PHY out of Layer 1, so the
//!   driver body does not invoke MDIC directly)
//! - Flow-control identity (FCAL/FCAH/FCT/VET) and timer/threshold
//!   (FCTTV/FCRTL/FCRTH)
//! - Interrupt set (ICR/ITR/ICS/IMS/IMC)
//! - Receive engine (RCTL, RDBAL/RDBAH/RDLEN, RDH/RDT, RDTR, RXDCTL,
//!   RADV, RXCSUM)
//! - Transmit engine (TCTL, TIPG, TDBAL/TDBAH/TDLEN, TDH/TDT, TXDCTL,
//!   TADV)
//! - Receive-address register 0 (RAL/RAH) for the primary unicast MAC
//!
//! Out of scope by variant pin: TX/RX queue 1 shadow registers
//! (TDBAL1/RDBAL1/...), RAR entries 1..15 (deferred until multi-MAC
//! workloads are claimed), VLAN filter table (VFTA), full statistics
//! ring (CRCERRS .. PRC127). Statistics registers are read by the OS
//! through `get_stats`; the variant pin restricts the canonical stat
//! set, and the rest is deferred per `e1000-spec-requirements.md`.
//!
//! Open issue (DSL gap, not E1000-specific): the multicast hash table
//! `MTA[128]` and the receive-address table `RAR[16]` are
//! array-of-registers shapes that the current `register_block!` macro
//! does not express as first-class typed arrays. This draft models
//! `MTA` as a `Reserved<u32> [128]` filler and exposes only `RAR[0]`
//! (RAL/RAH) by name. A first-class `RegArray<W, R, N>` extension to
//! `register_block!` is tracked as a stage-N requirement; once landed,
//! `MTA` and the RAR will move to typed arrays.
//!
//! All offsets and bit definitions cross-checked against:
//! - `e1000_hw.h` (Linux corpus): /Users/nyengele/driverbase/os/linux/
//!   drivers/net/ethernet/intel/e1000/e1000_hw.h
//! - `eth_e1000_priv.h` (Zephyr corpus): /Users/nyengele/driverbase/os/
//!   zephyr/drivers/ethernet/eth_e1000_priv.h
//! - 82540EM SDM section 13 (register descriptions)

use crate::regseq::register_block;

register_block! {
    #[endian = "little", access_width = 32]
    pub e1000 {
        // ---------------------------------------------------------------
        // 0x0000 - General control, status, EEPROM, PHY
        // ---------------------------------------------------------------

        // 0x00000 - Device Control. SDM 13.4.1.
        ctrl: ReadWrite {
            FD OFFSET(0) NUMBITS(1) [
                HALF_DUPLEX = 0,
                FULL_DUPLEX = 1,
            ],
            GIO_MASTER_DISABLE OFFSET(2) NUMBITS(1) [
                ALLOW = 0,
                BLOCK = 1,
            ],
            LRST OFFSET(3) NUMBITS(1) [
                NORMAL = 0,
                RESET = 1,
            ],
            ASDE OFFSET(5) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            SLU OFFSET(6) NUMBITS(1) [
                NORMAL = 0,
                FORCE_LINK = 1,
            ],
            ILOS OFFSET(7) NUMBITS(1) [
                NORMAL = 0,
                INVERT = 1,
            ],
            SPEED OFFSET(8) NUMBITS(2) [
                FORCE_10 = 0,
                FORCE_100 = 1,
                FORCE_1000 = 2,
            ],
            FRCSPD OFFSET(11) NUMBITS(1) [
                AUTO = 0,
                FORCE = 1,
            ],
            FRCDPX OFFSET(12) NUMBITS(1) [
                AUTO = 0,
                FORCE = 1,
            ],
            RST OFFSET(26) NUMBITS(1) [
                NORMAL = 0,
                GLOBAL_RESET = 1,
            ],
            RFCE OFFSET(27) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            TFCE OFFSET(28) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            VME OFFSET(30) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            PHY_RST OFFSET(31) NUMBITS(1) [
                NORMAL = 0,
                RESET = 1,
            ],
        },

        _pad_ctrl: Reserved<u32> [1],

        // 0x00008 - Device Status (read-only). SDM 13.4.2.
        status: ReadOnly {
            FD OFFSET(0) NUMBITS(1) [],
            LU OFFSET(1) NUMBITS(1) [],
            FUNC OFFSET(2) NUMBITS(2) [],
            TXOFF OFFSET(4) NUMBITS(1) [],
            TBIMODE OFFSET(5) NUMBITS(1) [],
            SPEED OFFSET(6) NUMBITS(2) [
                SPEED_10 = 0,
                SPEED_100 = 1,
                SPEED_1000 = 2,
            ],
            ASDV OFFSET(8) NUMBITS(2) [],
            MTXCKOK OFFSET(10) NUMBITS(1) [],
            PCI66 OFFSET(11) NUMBITS(1) [],
            BUS64 OFFSET(12) NUMBITS(1) [],
            PCIX_MODE OFFSET(13) NUMBITS(1) [],
            PCIX_SPEED OFFSET(14) NUMBITS(2) [],
        },

        _pad_status: Reserved<u32> [1],

        // 0x00010 - EEPROM/Flash Control. SDM 13.4.3.
        // Driver reads EECD only to confirm EEPROM presence; the actual
        // EEPROM bring-up sequence is delegated to the compiler/backend
        // wrapper, which programs MAC-from-EEPROM during the
        // pre-`init` device-handle preparation phase.
        eecd: ReadWrite {
            SK OFFSET(0) NUMBITS(1) [],
            CS OFFSET(1) NUMBITS(1) [],
            DI OFFSET(2) NUMBITS(1) [],
            DO OFFSET(3) NUMBITS(1) [],
            FWE OFFSET(4) NUMBITS(2) [],
            REQ OFFSET(6) NUMBITS(1) [],
            GNT OFFSET(7) NUMBITS(1) [],
            PRES OFFSET(8) NUMBITS(1) [],
            SIZE OFFSET(9) NUMBITS(1) [],
            ADDR_BITS OFFSET(10) NUMBITS(1) [],
            TYPE OFFSET(13) NUMBITS(1) [],
        },

        // 0x00014 - EEPROM Read. SDM 13.4.4.
        eerd: ReadWrite {
            START OFFSET(0) NUMBITS(1) [],
            DONE OFFSET(4) NUMBITS(1) [],
            ADDR OFFSET(8) NUMBITS(8) [],
            DATA OFFSET(16) NUMBITS(16) [],
        },

        // 0x00018 - Extended Device Control. SDM 13.4.5.
        ctrl_ext: ReadWrite {
            PHY_INT OFFSET(5) NUMBITS(1) [],
            SDP6_DATA OFFSET(6) NUMBITS(1) [],
            SDP7_DATA OFFSET(7) NUMBITS(1) [],
            ASDCHK OFFSET(12) NUMBITS(1) [],
            EE_RST OFFSET(13) NUMBITS(1) [],
            SPD_BYPS OFFSET(15) NUMBITS(1) [],
            RO_DIS OFFSET(17) NUMBITS(1) [],
            LINK_MODE OFFSET(22) NUMBITS(2) [
                COPPER = 0,
                FIBER = 2,
                SERDES = 3,
            ],
        },

        // 0x0001C - Flash Access. (Out of scope: 82540EM has no SPI
        // flash; field stub kept for layout fidelity.)
        fla: ReadWrite {
            VALUE OFFSET(0) NUMBITS(32) [],
        },

        // 0x00020 - MDI Control. SDM 13.4.7.
        // PHY management is compiler/backend responsibility per
        // network-device-class-spec.md (`Link / PHY / media` rule); the
        // authored driver body does not invoke MDIC directly.
        mdic: ReadWrite {
            DATA OFFSET(0) NUMBITS(16) [],
            REGADD OFFSET(16) NUMBITS(5) [],
            PHYADD OFFSET(21) NUMBITS(5) [],
            OP OFFSET(26) NUMBITS(2) [
                WRITE = 1,
                READ = 2,
            ],
            READY OFFSET(28) NUMBITS(1) [],
            INT_EN OFFSET(29) NUMBITS(1) [],
            ERROR OFFSET(30) NUMBITS(1) [],
        },

        _pad_mdic: Reserved<u32> [1],

        // ---------------------------------------------------------------
        // 0x00028 - Flow control identity (legacy)
        // ---------------------------------------------------------------

        // 0x00028 - Flow Control Address Low.
        fcal: ReadWrite { VALUE OFFSET(0) NUMBITS(32) [] },
        // 0x0002C - Flow Control Address High.
        fcah: ReadWrite { VALUE OFFSET(0) NUMBITS(32) [] },
        // 0x00030 - Flow Control Type.
        fct: ReadWrite { VALUE OFFSET(0) NUMBITS(32) [] },

        _pad_fct: Reserved<u32> [1],

        // 0x00038 - VLAN Ether Type.
        vet: ReadWrite { VALUE OFFSET(0) NUMBITS(32) [] },

        _pad_vet: Reserved<u32> [33],

        // ---------------------------------------------------------------
        // 0x000C0 - Interrupts. SDM 13.4.17 - 13.4.21.
        // ---------------------------------------------------------------

        // 0x000C0 - Interrupt Cause Read.
        //
        // Read is non-destructive on the source bits but writes back
        // implicit clear via the W1C convention used by the driver.
        // The `INT_ASSERTED` bit (bit 31) is set when the device has
        // raised the interrupt line; legacy INTx lowering must check
        // this bit before claiming the interrupt to avoid stealing a
        // spurious shared-interrupt assertion.
        icr: ReadWrite {
            TXDW OFFSET(0) NUMBITS(1) [],
            TXQE OFFSET(1) NUMBITS(1) [],
            LSC OFFSET(2) NUMBITS(1) [],
            RXSEQ OFFSET(3) NUMBITS(1) [],
            RXDMT0 OFFSET(4) NUMBITS(1) [],
            RXO OFFSET(6) NUMBITS(1) [],
            RXT0 OFFSET(7) NUMBITS(1) [],
            MDAC OFFSET(9) NUMBITS(1) [],
            RXCFG OFFSET(10) NUMBITS(1) [],
            TXD_LOW OFFSET(15) NUMBITS(1) [],
            SRPD OFFSET(16) NUMBITS(1) [],
            INT_ASSERTED OFFSET(31) NUMBITS(1) [],
        },

        // 0x000C4 - Interrupt Throttling Rate. Units: 256 ns; 0 = no
        // throttle. The variant pin uses ITR for the canonical
        // moderation knob; per-queue (RDTR/RADV) provides finer control
        // and is also exposed.
        itr: ReadWrite {
            INTERVAL OFFSET(0) NUMBITS(16) [],
        },

        // 0x000C8 - Interrupt Cause Set. Same field layout as ICR;
        // writing 1 sets the corresponding bit. Used by the compiler
        // for self-test; the driver body does not invoke ICS.
        ics: ReadWrite {
            VALUE OFFSET(0) NUMBITS(32) [],
        },

        _pad_ics: Reserved<u32> [1],

        // 0x000D0 - Interrupt Mask Set/Read. Writing 1 unmasks the
        // corresponding source. Reads return the current mask.
        ims: ReadWrite {
            TXDW OFFSET(0) NUMBITS(1) [],
            TXQE OFFSET(1) NUMBITS(1) [],
            LSC OFFSET(2) NUMBITS(1) [],
            RXSEQ OFFSET(3) NUMBITS(1) [],
            RXDMT0 OFFSET(4) NUMBITS(1) [],
            RXO OFFSET(6) NUMBITS(1) [],
            RXT0 OFFSET(7) NUMBITS(1) [],
            MDAC OFFSET(9) NUMBITS(1) [],
            RXCFG OFFSET(10) NUMBITS(1) [],
            TXD_LOW OFFSET(15) NUMBITS(1) [],
            SRPD OFFSET(16) NUMBITS(1) [],
        },

        _pad_ims: Reserved<u32> [1],

        // 0x000D8 - Interrupt Mask Clear. Writing 1 clears (masks) the
        // corresponding source. Field layout identical to IMS.
        imc: WriteOnly {
            VALUE OFFSET(0) NUMBITS(32) [],
        },

        _pad_imc: Reserved<u32> [9],

        // ---------------------------------------------------------------
        // 0x00100 - Receive control. SDM 13.4.22.
        // ---------------------------------------------------------------

        rctl: ReadWrite {
            EN OFFSET(1) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            SBP OFFSET(2) NUMBITS(1) [
                DISCARD = 0,
                STORE = 1,
            ],
            UPE OFFSET(3) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            MPE OFFSET(4) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            LPE OFFSET(5) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            LBM OFFSET(6) NUMBITS(2) [
                NORMAL = 0,
                MAC = 1,
                TCVR = 3,
            ],
            RDMTS OFFSET(8) NUMBITS(2) [
                HALF = 0,
                QUARTER = 1,
                EIGHTH = 2,
            ],
            MO OFFSET(12) NUMBITS(2) [
                MO_47_36 = 0,
                MO_46_35 = 1,
                MO_45_34 = 2,
                MO_43_32 = 3,
            ],
            BAM OFFSET(15) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            BSIZE OFFSET(16) NUMBITS(2) [
                SZ_2048 = 0,
                SZ_1024 = 1,
                SZ_512 = 2,
                SZ_256 = 3,
            ],
            VFE OFFSET(18) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            CFIEN OFFSET(19) NUMBITS(1) [],
            CFI OFFSET(20) NUMBITS(1) [],
            DPF OFFSET(22) NUMBITS(1) [
                FORWARD = 0,
                DISCARD = 1,
            ],
            PMCF OFFSET(23) NUMBITS(1) [],
            BSEX OFFSET(25) NUMBITS(1) [
                NORMAL = 0,
                EXTENDED = 1,
            ],
            SECRC OFFSET(26) NUMBITS(1) [
                KEEP = 0,
                STRIP = 1,
            ],
        },

        _pad_rctl: Reserved<u32> [27],

        // 0x00170 - Flow Control Transmit Timer Value.
        fcttv: ReadWrite { TTV OFFSET(0) NUMBITS(16) [] },

        _pad_fcttv: Reserved<u32> [163],

        // ---------------------------------------------------------------
        // 0x00400 - Transmit control. SDM 13.4.33.
        // ---------------------------------------------------------------

        tctl: ReadWrite {
            EN OFFSET(1) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            PSP OFFSET(3) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            CT OFFSET(4) NUMBITS(8) [],
            COLD OFFSET(12) NUMBITS(10) [],
            SWXOFF OFFSET(22) NUMBITS(1) [],
            RTLC OFFSET(24) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            NRTU OFFSET(25) NUMBITS(1) [],
        },

        _pad_tctl: Reserved<u32> [3],

        // 0x00410 - Transmit Inter-Packet Gap. SDM 13.4.34.
        tipg: ReadWrite {
            IPGT OFFSET(0) NUMBITS(10) [],
            IPGR1 OFFSET(10) NUMBITS(10) [],
            IPGR2 OFFSET(20) NUMBITS(10) [],
        },

        _pad_tipg: Reserved<u32> [1875],

        // 0x02160 - Flow Control Receive Threshold Low.
        fcrtl: ReadWrite {
            RTL OFFSET(3) NUMBITS(13) [],
            XONE OFFSET(31) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
        },

        _pad_fcrtl: Reserved<u32> [1],

        // 0x02168 - Flow Control Receive Threshold High.
        fcrth: ReadWrite {
            RTH OFFSET(3) NUMBITS(13) [],
            XFCE OFFSET(31) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
        },

        _pad_fcrth: Reserved<u32> [421],

        // ---------------------------------------------------------------
        // 0x02800 - RX descriptor ring 0. SDM 13.4.27 - 13.4.32.
        // ---------------------------------------------------------------

        // 0x02800 - RX Descriptor Base Address Low.
        rdbal: ReadWrite { VALUE OFFSET(0) NUMBITS(32) [] },
        // 0x02804 - RX Descriptor Base Address High.
        rdbah: ReadWrite { VALUE OFFSET(0) NUMBITS(32) [] },
        // 0x02808 - RX Descriptor Length. Programmed in bytes; must be
        // a multiple of 128 (1024-bit cache-line constraint).
        rdlen: ReadWrite { VALUE OFFSET(0) NUMBITS(20) [] },

        _pad_rdlen: Reserved<u32> [1],

        // 0x02810 - RX Descriptor Head. Hardware-owned advance.
        rdh: ReadWrite { VALUE OFFSET(0) NUMBITS(16) [] },

        _pad_rdh: Reserved<u32> [1],

        // 0x02818 - RX Descriptor Tail. Driver-owned; writes here are
        // the receive doorbell. Tail must trail head by at least 1
        // entry (no wrap-completion).
        rdt: ReadWrite { VALUE OFFSET(0) NUMBITS(16) [] },

        _pad_rdt: Reserved<u32> [1],

        // 0x02820 - RX Delay Timer. Units: 1.024 us. 0 = no delay.
        // Receive interrupt is held for up to RDTR units after a
        // packet is queued, allowing batching.
        rdtr: ReadWrite {
            DELAY OFFSET(0) NUMBITS(16) [],
            FPD OFFSET(31) NUMBITS(1) [],
        },

        _pad_rdtr: Reserved<u32> [1],

        // 0x02828 - RX Descriptor Control. Per-queue prefetch/host
        // thresholds; we leave at hardware defaults for the variant
        // pin.
        rxdctl: ReadWrite {
            PTHRESH OFFSET(0) NUMBITS(6) [],
            HTHRESH OFFSET(8) NUMBITS(6) [],
            WTHRESH OFFSET(16) NUMBITS(6) [],
            GRAN OFFSET(24) NUMBITS(1) [],
        },

        // 0x0282C - RX Interrupt Absolute Delay Timer. Units: 1.024 us.
        // Hard cap on the RDTR coalescing window; 0 = no absolute cap.
        radv: ReadWrite { VALUE OFFSET(0) NUMBITS(16) [] },

        _pad_radv: Reserved<u32> [1012],

        // ---------------------------------------------------------------
        // 0x03800 - TX descriptor ring 0. SDM 13.4.36 - 13.4.41.
        // ---------------------------------------------------------------

        tdbal: ReadWrite { VALUE OFFSET(0) NUMBITS(32) [] },
        tdbah: ReadWrite { VALUE OFFSET(0) NUMBITS(32) [] },
        tdlen: ReadWrite { VALUE OFFSET(0) NUMBITS(20) [] },

        _pad_tdlen: Reserved<u32> [1],

        tdh: ReadWrite { VALUE OFFSET(0) NUMBITS(16) [] },

        _pad_tdh: Reserved<u32> [1],

        // 0x03818 - TX Descriptor Tail. Driver-owned; writes here are
        // the transmit doorbell.
        tdt: ReadWrite { VALUE OFFSET(0) NUMBITS(16) [] },

        _pad_tdt: Reserved<u32> [3],

        // 0x03828 - TX Descriptor Control.
        txdctl: ReadWrite {
            PTHRESH OFFSET(0) NUMBITS(6) [],
            HTHRESH OFFSET(8) NUMBITS(6) [],
            WTHRESH OFFSET(16) NUMBITS(6) [],
            GRAN OFFSET(24) NUMBITS(1) [],
            LWTHRESH OFFSET(25) NUMBITS(7) [],
        },

        // 0x0382C - TX Interrupt Absolute Delay Timer.
        tadv: ReadWrite { VALUE OFFSET(0) NUMBITS(16) [] },

        _pad_tadv: Reserved<u32> [1524],

        // ---------------------------------------------------------------
        // 0x05000 - RX checksum control + multicast hash + RAR0.
        // ---------------------------------------------------------------

        // 0x05000 - Receive Checksum Control. SDM 13.4.46.
        rxcsum: ReadWrite {
            PCSS OFFSET(0) NUMBITS(8) [],
            IPOFL OFFSET(8) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
            TUOFL OFFSET(9) NUMBITS(1) [
                DISABLED = 0,
                ENABLED = 1,
            ],
        },

        // 0x05004..0x051FF - reserved.
        _pad_rxcsum: Reserved<u32> [127],

        // 0x05200..0x053FF - Multicast Table Array. 128 entries x u32.
        // Treated as a contiguous reserved span here; the driver body
        // will access individual MTA[k] entries through compiler-side
        // helpers once the array-of-registers DSL extension lands. See
        // module-level "Open issue (DSL gap)" note. Until then, a
        // stage-2 multicast workload is gated behind the DSL extension.
        _mta_placeholder: Reserved<u32> [128],

        // 0x05400 - Receive Address Low (RAR0). SDM 13.4.47. Bits 31:0
        // are the lower 4 bytes of the 6-byte MAC.
        ral0: ReadWrite { VALUE OFFSET(0) NUMBITS(32) [] },

        // 0x05404 - Receive Address High (RAR0). SDM 13.4.48.
        // - bits 15:0 are the upper 2 bytes of the MAC
        // - bit 31 is AV (Address Valid); set to 1 to enable matching.
        rah0: ReadWrite {
            ADDR_HIGH OFFSET(0) NUMBITS(16) [],
            AS OFFSET(16) NUMBITS(2) [
                DEST_ADDR = 0,
                SRC_ADDR = 3,
            ],
            AV OFFSET(31) NUMBITS(1) [
                INVALID = 0,
                VALID = 1,
            ],
        },
    }
}

// ---------------------------------------------------------------------------
// Driver-facing constants
// ---------------------------------------------------------------------------

/// PCI vendor identifier for Intel.
pub const E1000_PCI_VENDOR_INTEL: u16 = 0x8086;

/// PCI device identifier for the 82540EM-A. Cross-checked against
/// `e1000_hw.h` `E1000_DEV_ID_82540EM` and the QEMU `e1000` device
/// model default.
pub const E1000_PCI_DEVICE_82540EM: u16 = 0x100E;

/// All-IRQ mask for the 82540EM claim path. Composed exactly as in the
/// Linux corpus `IMS_ENABLE_MASK`
/// (e1000_hw.h:476-481): RXT0 + TXDW + RXDMT0 + RXSEQ + LSC.
pub const E1000_IMS_ENABLE_MASK: u32 = (1 << 0)   // TXDW
    | (1 << 2)                                    // LSC
    | (1 << 3)                                    // RXSEQ
    | (1 << 4)                                    // RXDMT0
    | (1 << 7);                                   // RXT0

/// Mask covering every claim-path IRQ source that the driver needs to
/// disable in `mask_all_interrupts`. Superset of `IMS_ENABLE_MASK`
/// because IMC also clears RXO and TXQE that the variant pin keeps
/// available for fault diagnosis.
pub const E1000_IMS_ALL_MASK: u32 = (1 << 0)      // TXDW
    | (1 << 1)                                    // TXQE
    | (1 << 2)                                    // LSC
    | (1 << 3)                                    // RXSEQ
    | (1 << 4)                                    // RXDMT0
    | (1 << 6)                                    // RXO
    | (1 << 7)                                    // RXT0
    | (1 << 9)                                    // MDAC
    | (1 << 10)                                   // RXCFG
    | (1 << 15)                                   // TXD_LOW
    | (1 << 16);                                  // SRPD

/// Default IPG values for copper PHY at 1 Gb/s (SDM 13.4.34 Table
/// 13-77; Linux corpus `DEFAULT_82543_TIPG_IPGT_COPPER` /
/// `DEFAULT_82543_TIPG_IPGR1` / `DEFAULT_82543_TIPG_IPGR2` reused
/// verbatim because the 82540EM uses the same gigabit IPG defaults).
pub const E1000_TIPG_DEFAULT_COPPER: u32 = 8        // IPGT
    | (8 << 10)                                     // IPGR1
    | (6 << 20);                                    // IPGR2

/// Variant-pinned descriptor ring sizes, in entries. Sized to match the
/// QEMU emulated 82540EM evaluation envelope and the Linux corpus
/// defaults (`E1000_DEFAULT_TXD = 256`, `E1000_DEFAULT_RXD = 256`).
pub const E1000_TX_RING_SIZE: u32 = 256;
pub const E1000_RX_RING_SIZE: u32 = 256;
