#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 negative: a bare `#[rseq::isr]` with TWO Interrupt resources — the
// bare form binds against exactly one (IRQ-ERR-BARE-ISR-AMBIGUOUS).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");

#[rseq::driver(name = "twoirq", compatible = ["acme,twoirq"])]
pub struct TwoIrq {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
    #[rseq::interrupt(id = "b")]
    pub b: Interrupt,
}

impl TwoIrq {
    #[rseq::isr]
    pub fn handle(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> TwoIrq { loop {} }
