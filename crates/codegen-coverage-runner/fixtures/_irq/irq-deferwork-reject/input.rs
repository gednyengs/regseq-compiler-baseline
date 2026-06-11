#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// R09/D04 negative: the handler returns `IrqResult::DeferWork` — deferring
// work from an ISR requires the deferred-work-item-scheduling construct
// (not built); the emit guard rejects loudly, naming it.

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");

#[rseq::driver(name = "deferer", compatible = ["acme,deferer"])]
pub struct Deferer {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

impl Deferer {
    #[rseq::isr]
    pub fn handle(&mut self) -> IrqResult { IrqResult::DeferWork }
}

pub fn make() -> Deferer { loop {} }
