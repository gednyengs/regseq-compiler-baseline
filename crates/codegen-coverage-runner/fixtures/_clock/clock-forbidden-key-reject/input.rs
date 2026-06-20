#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 N21: `mode` is a forbidden L1 attribute key (it belongs in the
// `[clock.<id>]` platform section, not the L1 `#[rseq::clock(...)]`).
include!("../../_lifecycle_surface.rs");
include!("../../_clock_surface.rs");

#[rseq::driver(name = "forbidkey", compatible = ["acme,forbidkey"])]
pub struct ForbidKey {
    #[rseq::clock(id = "uartclk", mode = "managed")]
    clk: Clock,
}

impl Lifecycle for ForbidKey {}

pub fn make() -> ForbidKey { loop {} }
