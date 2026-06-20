#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 N12: an unknown `#[rseq::clock(...)]` key (`foo`) — clock rejects unknown
// keys rather than ignoring them (GRILL D13).
include!("../../_lifecycle_surface.rs");
include!("../../_clock_surface.rs");

#[rseq::driver(name = "badattr", compatible = ["acme,badattr"])]
pub struct BadAttr {
    #[rseq::clock(id = "uartclk", foo = "bar")]
    clk: Clock,
}

impl Lifecycle for BadAttr {}

pub fn make() -> BadAttr { loop {} }
