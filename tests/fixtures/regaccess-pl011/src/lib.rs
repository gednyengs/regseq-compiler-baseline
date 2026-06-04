#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

//! PL011 register-access lowering proof.
//!
//! Declares a PL011 register block via `register_block!`, then an entry that
//! performs write / read / read-modify-write / read_field. The compiler is
//! expected to lower each op to a `regseq_mmio_read/write(offset, width, ...)`
//! host device-model hook in the generic-c backend, with the offsets/values
//! flowing from the macro-generated descriptors.

pub mod regseq;

use regseq::{register_block, Mmio};

register_block! {
    #[endian = "little", access_width = 32]
    pub pl011 {
        // 0x000 Data Register
        dr: ReadWrite {
            DATA OFFSET(0) NUMBITS(8) [],
        },
        // 0x004 .. 0x018 reserved (5 words)
        _reserved0: Reserved<u32> [5],
        // 0x018 Flag Register (read-only, hardware-updated flags)
        fr: ReadOnly {
            BUSY OFFSET(3) NUMBITS(1) [],
            TXFF OFFSET(5) NUMBITS(1) [],
        },
        // 0x01C Integer Baud Rate Divisor
        ibrd: ReadWrite {
            BAUD_DIVINT OFFSET(0) NUMBITS(16) [],
        },
        // 0x020 Control Register
        cr: ReadWrite {
            UARTEN OFFSET(0) NUMBITS(1) [ DISABLED = 0, ENABLED = 1 ],
            TXE OFFSET(8) NUMBITS(1) [ DISABLED = 0, ENABLED = 1 ],
        },
    }
}

// Codex #5: explicit hardware-offset guards — compile-time `_Static_assert`-
// equivalents that the layout-derived offsets match the hardware map. A
// miscounted `Reserved` fails the build here rather than silently mis-lowering.
const _: () = assert!(pl011::dr::OFFSET == 0x00);
const _: () = assert!(pl011::fr::OFFSET == 0x18);
const _: () = assert!(pl011::ibrd::OFFSET == 0x1c);
const _: () = assert!(pl011::cr::OFFSET == 0x20);

#[no_mangle]
pub extern "C" fn regseq_main() {
    let regs: Mmio<pl011::Regs> = Mmio::new();

    // write a byte to DR  -> W32 @0x000 = 0x41
    regs.write(pl011::dr::REG, 0x41u32);
    // read DR back (round-trips through the device-model register file)
    let _dr = regs.read(pl011::dr::REG);
    // program the baud divisor -> W32 @0x01c = 0x1a
    regs.write(pl011::ibrd::REG, 0x1Au32);
    // enable UART + TX via RMW -> R32 @0x020, then W32 @0x020 = 0x101
    regs.modify(
        pl011::cr::REG,
        pl011::cr::UARTEN::ENABLED + pl011::cr::TXE::ENABLED,
    );
    // read the BUSY flag field -> R32 @0x018 (masked/shifted). `BUSY` is a
    // direct `Field` const (no named values), so there is no `::FIELD` submodule.
    let _busy = regs.read_field(pl011::fr::BUSY);
}
