#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]
include!("../../_lifecycle_surface.rs");
include!("../../_serial_port_surface.rs");
#[rseq::driver(name = "wrong", compatible = ["regseq,wrong"])]
pub struct Wrong;
impl Lifecycle for Wrong {}
impl SerialPort for Wrong {
    fn transmit(&mut self, buffer: SerialTxBuffer) -> usize { buffer.len }
}
