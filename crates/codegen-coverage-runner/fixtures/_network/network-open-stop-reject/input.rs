#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");

pub trait Network { fn open(&mut self) -> isize; fn stop(&mut self) -> isize; }

#[rseq::driver(name = "oldnet", compatible = ["regseq,oldnet"])]
#[rseq::device_class(network)]
pub struct OldNet;

impl Network for OldNet {
    fn open(&mut self) -> isize { 0 }
    fn stop(&mut self) -> isize { 0 }
}

impl Lifecycle for OldNet {}
