#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

//! Form-B all-ops coverage fixture: exercises every register op (read/write
//! whole-register raw, read_field/write_field field, modify field-RMW) across
//! two register offsets and both wrapper types — including `write_field`, which
//! the PL011/E1000 device fixtures do not use. A minimal synthetic block keeps
//! the expected offsets trivial to read.

pub mod regseq;

use regseq::{Field, FieldValue, Mmio};

pub struct Dev {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<regseq::Regs>,
}

impl Dev {
    fn go(&self) {
        // Form B: the register is the receiver (`regs.dr`), no token argument.
        // Each op's register offset is recovered from the block layout.
        self.regs.dr.write(0x41u32); // W32 @0x004 = 0x41
        let _x = self.regs.dr.read(); // R32 @0x004
        self.regs.dr.write_field(FieldValue::new(0xF0, 0x50)); // W32 @0x004 = 0x50
        self.regs.dr.modify(FieldValue::new(0x0F, 0x03)); // R32 @0x004, W32 @0x004 = 0x53
        let _f = self.regs.fr.read_field(Field::new(0x8, 3)); // R32 @0x008
        let _ = (_x, _f);
    }
}

#[no_mangle]
pub extern "C" fn regseq_main() {
    let dev = Dev {
        regs: Mmio::new(),
    };
    dev.go();
}
