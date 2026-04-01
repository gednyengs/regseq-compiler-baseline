#![no_std]

mod registers;

use regseq::Mmio;
pub use registers::Pl011Registers;

#[rseq::driver(name = "pl011", compatible = "arm,pl011")]
#[rseq::device_class(serial)]
pub struct Pl011Driver {
    #[rseq::mmio(id = "uart_regs")]
    pub regs: Mmio<Pl011Registers>,
}
