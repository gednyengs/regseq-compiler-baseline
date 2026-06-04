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
//! what the lowering proves (the offsets flow from the descriptors regardless);
//! the compile-time guards below pin the compact layout.

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

#[no_mangle]
pub extern "C" fn regseq_main() {
    let regs: Mmio<e1000::Regs> = Mmio::new();

    // RMW CTRL: set link up.  -> R32 @0x000 ; W32 @0x000 = 0x40
    regs.modify(e1000::ctrl::REG, e1000::ctrl::SLU::UP);
    // read STATUS.            -> R32 @0x004
    let _st = regs.read(e1000::status::REG);
    // program the 64-bit TX ring base: HIGH then LOW (program order; device
    // latches on the low write).  -> W32 @0x014 (high) ; W32 @0x010 (low)
    regs.write(e1000::tdbah::REG, 0x0000_0001u32);
    regs.write(e1000::tdbal::REG, 0x0000_2000u32);
    // ring length.            -> W32 @0x018 = 0x80
    regs.write(e1000::tdlen::REG, 0x80u32);
    // enable TX via RMW.      -> R32 @0x00c ; W32 @0x00c = 0x2
    regs.modify(e1000::tctl::REG, e1000::tctl::EN::ENABLED);
    // doorbell: advance TX tail. -> W32 @0x01c = 0x1
    regs.write(e1000::tdt::REG, 0x1u32);
    // read-clear ICR (reading clears causes). -> R32 @0x008
    let _icr = regs.read(e1000::icr::REG);
}
