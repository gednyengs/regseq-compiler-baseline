#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");

// The locked line-error value is a named four-flag structure. A raw integer
// carrier would leak device bits into L1 and must be rejected by the bridge.
pub struct LineErrorFlags { pub bits: u32 }
pub struct SerialRxBuffer {
    pub data: *mut u8,
    pub len: usize,
    pub errors: *mut LineErrorFlags,
}
pub trait SerialPort {
    fn receive(&mut self, _buffer: SerialRxBuffer) -> usize { 0 }
}

#[rseq::driver(name = "raw_flags", compatible = ["regseq,raw-flags"])]
#[rseq::device_class(serial)]
pub struct RawFlags;
impl Lifecycle for RawFlags {}
impl SerialPort for RawFlags {
    fn receive(&mut self, buffer: SerialRxBuffer) -> usize { buffer.len }
}
