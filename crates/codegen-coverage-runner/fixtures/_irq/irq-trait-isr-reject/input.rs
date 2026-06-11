#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 negative: an `#[rseq::isr]` on a trait impl method — the
// trait-provided ISR binding is in the taxonomy's excluded set
// (IRQ-ERR-EXCLUDED-PRIMITIVE).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");

#[rseq::driver(name = "traitisr", compatible = ["acme,traitisr"])]
pub struct TraitIsr {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

pub trait IrqOps {
    fn handle(&mut self) -> IrqResult;
}

impl IrqOps for TraitIsr {
    #[rseq::isr]
    fn handle(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> TraitIsr { loop {} }
