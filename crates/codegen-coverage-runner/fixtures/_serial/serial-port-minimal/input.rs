#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");
include!("../../_serial_port_surface.rs");

#[rseq::driver(name = "serial_port_minimal", compatible = ["regseq,serial-port-minimal"])]
#[rseq::device_class(serial)]
pub struct Minimal;

impl SerialPort for Minimal {
    fn transmit(&mut self, buffer: SerialTxBuffer) -> usize { buffer.len }
}

impl Lifecycle for Minimal {
    fn on_init(&mut self) -> Result<(), RegseqError> { Ok(()) }
}
