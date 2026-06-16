#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// D06/R02 negative: `Option<Interrupt>` is RECOGNIZED whole (the frontend
// accepts it) but its lowering is emit-stage rejected this slice — loud,
// never partial (I05). No platform file: the form guard fires before the
// irq_number requirement.

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");

#[rseq::driver(name = "optlower", compatible = ["acme,optlower"])]
pub struct OptLower {
    #[rseq::interrupt(id = "maybe")]
    pub maybe: Option<Interrupt>,
}

impl OptLower {
    #[rseq::isr(field = "maybe")]
    pub fn handle(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> OptLower { loop {} }
