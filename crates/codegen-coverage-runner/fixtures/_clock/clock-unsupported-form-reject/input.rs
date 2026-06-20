#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 N2/N14: `Option<Clock>` is recognized-and-rejected (v1 builds only the
// required `Clock` form).
include!("../../_lifecycle_surface.rs");
include!("../../_clock_surface.rs");

#[rseq::driver(name = "optclk", compatible = ["acme,optclk"])]
pub struct OptClk {
    #[rseq::clock(id = "uartclk")]
    clk: Option<Clock>,
}

impl Lifecycle for OptClk {}

pub fn make() -> OptClk { loop {} }
