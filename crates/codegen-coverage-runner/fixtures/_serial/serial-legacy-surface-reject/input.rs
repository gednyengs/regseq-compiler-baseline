#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]
include!("../../_lifecycle_surface.rs");
include!("../../_serial_port_surface.rs");
pub trait SerialShutdownOps { fn shutdown_quiesce(&mut self); }
#[rseq::driver(name = "legacy", compatible = ["regseq,legacy"])]
#[rseq::device_class(serial)]
pub struct Legacy;
impl Lifecycle for Legacy {}
impl SerialPort for Legacy { fn close(&mut self) -> SerialStatus { SerialStatus::Ok } }
impl SerialShutdownOps for Legacy { fn shutdown_quiesce(&mut self) {} }
