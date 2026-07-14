#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]
include!("../../_lifecycle_surface.rs");
include!("../../_serial_port_surface.rs");
#[rseq::driver(name = "missing", compatible = ["regseq,missing"])]
#[rseq::device_class(serial)]
pub struct Missing;
impl Lifecycle for Missing {}
