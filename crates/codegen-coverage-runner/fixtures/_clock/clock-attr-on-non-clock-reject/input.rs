#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 N13: `#[rseq::clock(...)]` on a `u32` field (not the `Clock` shell).
include!("../../_lifecycle_surface.rs");
include!("../../_clock_surface.rs");

#[rseq::driver(name = "wrongfield", compatible = ["acme,wrongfield"])]
pub struct WrongField {
    #[rseq::clock(id = "uartclk")]
    bad: u32,
}

impl Lifecycle for WrongField {}

pub fn make() -> WrongField { loop {} }
