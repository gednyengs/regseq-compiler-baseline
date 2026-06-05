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

/// The driver: declares the typed MMIO resource (L1). The `id = "regs"` joins to
/// the platform file's `[mmio.regs]` section (L2).
pub struct Pl011 {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Regs>,
}

impl Pl011 {
    fn configure(&self) {
        // Form B: the register is the receiver (`regs.dr`), reached via
        // `Mmio<T>: Deref`; the backend recovers each register's byte offset
        // from the `#[repr(C)]` block layout.
        // write a byte to DR (whole-register raw) -> W32 @0x000 = 0x41
        self.regs.dr.write(0x41u32);
        // read DR back (round-trips through the device-model register file)
        let _dr = self.regs.dr.read();
        // program the baud divisor -> W32 @0x01c = 0x1a
        self.regs.ibrd.write(0x1Au32);
        // enable UART + TX via field RMW -> R32 @0x020, then W32 @0x020 = 0x101
        self.regs
            .cr
            .modify(pl011::cr::UARTEN::ENABLED + pl011::cr::TXE::ENABLED);
        // read the BUSY flag field -> R32 @0x018 (masked/shifted). `BUSY` is a
        // direct `Field` const (no named values), so there is no `::FIELD` submodule.
        let _busy = self.regs.fr.read_field(pl011::fr::BUSY);
        let _ = (_dr, _busy);
    }
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    // The resource is normally bound by the compiler at init (map_region); for
    // this host generic-c proof the device model is a global register file, so
    // the `Mmio` block instance is just a layout carrier.
    let dev = Pl011 {
        regs: Mmio::new(pl011::Regs::new()),
    };
    dev.configure();
}
