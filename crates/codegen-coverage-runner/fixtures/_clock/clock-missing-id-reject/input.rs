#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 N1: a `Clock` field with no `#[rseq::clock(id = "…")]` attribute.
include!("../../_lifecycle_surface.rs");
include!("../../_clock_surface.rs");

#[rseq::driver(name = "noid", compatible = ["acme,noid"])]
pub struct NoId {
    clk: Clock,
}

impl Lifecycle for NoId {}

pub fn make() -> NoId { loop {} }
