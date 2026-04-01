#![no_std]

use regseq::Mmio;

/// PL011 register block placeholder for the initial spec-slice scaffold.
pub struct Pl011Registers;

#[rseq::driver(name = "pl011", compatible = "arm,pl011")]
#[rseq::device_class(serial)]
pub struct Pl011Driver {
    pub regs: Mmio<Pl011Registers>,
}
