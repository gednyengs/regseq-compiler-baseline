#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

#[rseq::driver(name = "minimal")]
pub struct MinimalDriver {
    pub base_addr: usize,
}
