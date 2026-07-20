#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]
include!("../../_lifecycle_surface.rs");
pub enum SerialStatus { Ok, Unsupported }
pub trait SerialPort { fn open(&mut self) -> SerialStatus { SerialStatus::Unsupported } }
#[rseq::driver(name = "bad_status", compatible = ["regseq,bad-status"])]
#[rseq::device_class(serial)]
pub struct BadStatus;
impl Lifecycle for BadStatus {}
impl SerialPort for BadStatus { fn open(&mut self) -> SerialStatus { SerialStatus::Ok } }
