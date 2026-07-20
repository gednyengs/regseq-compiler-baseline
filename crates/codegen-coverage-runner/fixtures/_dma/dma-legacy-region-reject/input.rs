#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]
include!("../../_lifecycle_surface.rs");
pub trait DmaRegion { fn address(&self) -> usize; }
#[rseq::driver(name = "dma_legacy_region", compatible = ["regseq,dma-legacy-region"])]
pub struct Bad;
impl DmaRegion for Bad { fn address(&self) -> usize { 0 } }
impl Lifecycle for Bad { fn on_init(&mut self) -> Result<(), RegseqError> { Ok(()) } }
