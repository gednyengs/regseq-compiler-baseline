#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 negative: a handler bound to `[Interrupt; 2]` without the
// `index: usize` parameter (IRQ-ERR-ARRAY-BIND-NO-INDEX).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");

#[rseq::driver(name = "arrnoidx", compatible = ["acme,arrnoidx"])]
pub struct ArrNoIdx {
    #[rseq::interrupt(id = "lines")]
    pub lines: [Interrupt; 2],
}

impl ArrNoIdx {
    #[rseq::isr(field = "lines")]
    pub fn handle(&mut self) -> IrqResult { IrqResult::Handled }
}

pub fn make() -> ArrNoIdx { loop {} }
