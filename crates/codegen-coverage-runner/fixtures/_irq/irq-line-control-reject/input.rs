#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06/I02 negative: the `Interrupt` shell grew a driver-callable method —
// line control is compiler-managed, never an author call
// (IRQ-ERR-DRIVER-CALLABLE-LINE-CONTROL).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");

impl Interrupt {
    pub fn enable_line(&mut self) {}
}

#[rseq::driver(name = "linectl", compatible = ["acme,linectl"])]
pub struct LineCtl {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

impl LineCtl {
    #[rseq::isr]
    pub fn handle(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> LineCtl { loop {} }
