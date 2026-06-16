#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C04 negative: a handler returning `Result<(), RegseqError>` — an ISR
// returns the IrqResult dispatch result, never an error type
// (IRQ-ERR-ISR-SIGNATURE).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");

#[rseq::driver(name = "resultish", compatible = ["acme,resultish"])]
pub struct Resultish {
    #[rseq::interrupt(id = "a")]
    pub a: Interrupt,
}

impl Resultish {
    #[rseq::isr]
    pub fn handle(&mut self) -> Result<(), RegseqError> { Ok(()) }
}

pub fn make() -> Resultish { loop {} }
