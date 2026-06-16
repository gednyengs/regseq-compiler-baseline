#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 negative: `#[rseq::isr(field = "nope")]` names a field that is not a
// recognized Interrupt resource (IRQ-ERR-UNKNOWN-FIELD).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");

#[rseq::driver(name = "unknown", compatible = ["acme,unknown"])]
pub struct Unknown {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

impl Unknown {
    #[rseq::isr(field = "nope")]
    pub fn handle(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> Unknown { loop {} }
