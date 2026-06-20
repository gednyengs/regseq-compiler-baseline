#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 N15: a duplicate L1 clock id within one driver (the id must be unique).
include!("../../_lifecycle_surface.rs");
include!("../../_clock_surface.rs");

#[rseq::driver(name = "dupid", compatible = ["acme,dupid"])]
pub struct DupId {
    #[rseq::clock(id = "uartclk")]
    a: Clock,
    #[rseq::clock(id = "uartclk")]
    b: Clock,
}

impl Lifecycle for DupId {}

pub fn make() -> DupId { loop {} }
