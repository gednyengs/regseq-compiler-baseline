#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 negative: a non-optional `Interrupt` with NO bound handler — D13
// demands exactly one (IRQ-ERR-MISSING-HANDLER).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");

#[rseq::driver(name = "nohandler", compatible = ["acme,nohandler"])]
pub struct NoHandler {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

pub fn make() -> NoHandler { loop {} }
