#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 negative: two `#[rseq::isr(field = "a")]` handlers on one resource —
// exactly one per resource (IRQ-ERR-DUPLICATE-HANDLER, D13).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");

#[rseq::driver(name = "dup", compatible = ["acme,dup"])]
pub struct Dup {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

impl Dup {
    #[rseq::isr(field = "a")]
    pub fn one(&mut self) -> IrqResult { IrqResult::Handled }
    #[rseq::isr(field = "a")]
    pub fn two(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> Dup { loop {} }
