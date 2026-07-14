#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");

// The historical byte-at-a-time primitive is below the locked buffered port
// plane. The bridge must demand SerialTxBuffer and a progress count.
pub trait SerialPort {
    fn transmit(&mut self, _byte: u8) -> usize { 0 }
}

#[rseq::driver(name = "byte_transfer", compatible = ["regseq,byte-transfer"])]
#[rseq::device_class(serial)]
pub struct ByteTransfer;
impl Lifecycle for ByteTransfer {}
impl SerialPort for ByteTransfer {
    fn transmit(&mut self, _byte: u8) -> usize { 1 }
}
