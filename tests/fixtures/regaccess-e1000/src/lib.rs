#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

//! E1000 (NIC) register-access lowering proof.
//!
//! Demonstrates the E1000 register-access patterns lowering through generic-c:
//! PCI BAR0 binding (L2 `bar = 0` in platform.toml), RMW on control registers,
//! a read-clear interrupt-cause register, a 64-bit TX-ring base programmed as
//! two 32-bit writes (high then low), and a doorbell write.
//!
//! Offsets here are COMPACT (consecutive) to keep the fixture small; the real
//! E1000 hardware offsets are noted per register. Absolute offsets do not change
//! what the lowering proves (the backend recovers each register's offset from the
//! `#[repr(C)]` block layout regardless); the compile-time guards below pin the
//! compact layout.

pub mod regseq;

use regseq::{register_block, Mmio};

register_block! {
    #[endian = "little", access_width = 32]
    pub e1000 {
        // CTRL (real 0x0000): device control — reset + set-link-up.
        ctrl: ReadWrite {
            SLU OFFSET(6) NUMBITS(1) [ DOWN = 0, UP = 1 ],
            RST OFFSET(26) NUMBITS(1) [ NORMAL = 0, RESET = 1 ],
        },
        // STATUS (real 0x0008): device status — link up.
        status: ReadOnly {
            LU OFFSET(1) NUMBITS(1) [],
        },
        // ICR (real 0x00C0): interrupt cause — READ-CLEAR (reading clears causes).
        icr: ReadOnly {
            TXDW OFFSET(0) NUMBITS(1) [],
            RXT0 OFFSET(7) NUMBITS(1) [],
        },
        // TCTL (real 0x0400): transmit control — enable TX.
        tctl: ReadWrite {
            EN OFFSET(1) NUMBITS(1) [ DISABLED = 0, ENABLED = 1 ],
        },
        // TX descriptor ring base — 64 bits split across two 32-bit registers.
        tdbal: ReadWrite {},                 // real 0x3800 (low 32)
        tdbah: ReadWrite {},                 // real 0x3804 (high 32)
        tdlen: ReadWrite {},                 // real 0x3808 (ring length bytes)
        tdt: ReadWrite {},                   // real 0x3818 (tail doorbell)

        // --- register ARRAYS (the A3 form; counts shrunk for the fixture) ---
        // MTA multicast hash table — contiguous u32 array (real MTA[128] @0x5200,
        // 4B stride). A driver clears it entry-by-entry at init.
        mta[8]: ReadWrite {},
        // VFTA VLAN filter table — contiguous u32 array (real VFTA[128] @0x5600).
        vfta[8]: ReadWrite {},
        // RA receive-address filter — an array of RAL+RAH *pairs* (real RA[15]
        // @0x5400, 8B stride). RAL = low 32 of the MAC; RAH = high 16 + the
        // Address-Valid bit. Exercises the array-of-struct chain (Field+Index+Field).
        ra[4]: {
            ral: ReadWrite {},
            rah: ReadWrite { AV OFFSET(31) NUMBITS(1) [ INVALID = 0, VALID = 1 ] },
        },
    }
}

// Codex #5: compile-time offset guards (compact layout).
const _: () = assert!(e1000::ctrl::OFFSET == 0x00);
const _: () = assert!(e1000::status::OFFSET == 0x04);
const _: () = assert!(e1000::icr::OFFSET == 0x08);
const _: () = assert!(e1000::tctl::OFFSET == 0x0c);
const _: () = assert!(e1000::tdbal::OFFSET == 0x10);
const _: () = assert!(e1000::tdbah::OFFSET == 0x14);
const _: () = assert!(e1000::tdlen::OFFSET == 0x18);
const _: () = assert!(e1000::tdt::OFFSET == 0x1c);
// Array bases (single registers 0x00..0x20, then the three arrays).
const _: () = assert!(e1000::mta::OFFSET == 0x20); //  8 * 4B  -> 0x20..0x40
const _: () = assert!(e1000::vfta::OFFSET == 0x40); //  8 * 4B  -> 0x40..0x60
const _: () = assert!(e1000::ra::OFFSET == 0x60); //  4 * 8B  -> 0x60..0x80

/// The driver: declares the typed MMIO resource (L1). The `id = "regs"` joins to
/// the platform file's `[mmio.regs]` section (L2: PCI BAR0).
pub struct E1000 {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<e1000::Regs>,
}

impl E1000 {
    fn bring_up(&self) {
        // Form B: the register is the receiver; the backend recovers each
        // register's byte offset from the `#[repr(C)]` block layout.
        // RMW CTRL: set link up.  -> R32 @0x000 ; W32 @0x000 = 0x40
        self.regs.ctrl.modify(e1000::ctrl::SLU::UP);
        // read STATUS (whole register). -> R32 @0x004
        let _st = self.regs.status.read();
        // program the 64-bit TX ring base: HIGH then LOW (program order; device
        // latches on the low write).  -> W32 @0x014 (high) ; W32 @0x010 (low)
        self.regs.tdbah.write(0x0000_0001u32);
        self.regs.tdbal.write(0x0000_2000u32);
        // ring length (whole register). -> W32 @0x018 = 0x80
        self.regs.tdlen.write(0x80u32);
        // enable TX via field RMW.      -> R32 @0x00c ; W32 @0x00c = 0x2
        self.regs.tctl.modify(e1000::tctl::EN::ENABLED);
        // doorbell: advance TX tail (whole register). -> W32 @0x01c = 0x1
        self.regs.tdt.write(0x1u32);
        // read-clear ICR (reading clears causes). -> R32 @0x008
        let _icr = self.regs.icr.read();
        let _ = (_st, _icr);
    }

    /// Exercise the register arrays with RUNTIME indices (the offset is recovered
    /// as `base + i*stride`, not a layout constant). `m`/`v`/`r` are parameters,
    /// so each `[i]` lowers to a runtime `Index` projection.
    fn setup_filters(&self, m: usize, v: usize, r: usize) {
        // clear a multicast-table entry  -> W32 @(0x20 + m*4)
        self.regs.mta[m].write(0u32);
        // clear a VLAN-filter entry      -> W32 @(0x40 + v*4)
        self.regs.vfta[v].write(0u32);
        // program receive-address entry r: low MAC word, then set Address-Valid.
        // array-of-struct: ral at (0x60 + r*8 + 0), rah at (0x60 + r*8 + 4).
        self.regs.ra[r].ral.write(0xAABB_CCDDu32);
        self.regs.ra[r].rah.write_field(e1000::ra::rah::AV::VALID);
    }
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let dev = E1000 {
        regs: Mmio::new(e1000::Regs::new()),
    };
    dev.bring_up();
    // runtime indices m=3, v=5, r=1
    dev.setup_filters(3, 5, 1);
}
