#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 negative: `#[rseq::interrupt]` without the mandatory `id` (R01/D08 —
// no field-name fallback). Rejected at Acquire with IRQ-ERR-MISSING-ID.

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");

#[rseq::driver(name = "noid", compatible = ["acme,noid"])]
pub struct NoId {
    #[rseq::interrupt]
    pub irq: Interrupt,
}

impl NoId {
    #[rseq::isr]
    pub fn handle_irq(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> NoId { loop {} }
