#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]
include!("../../_lifecycle_surface.rs");
pub trait DmaStreamBuffer { fn map(&mut self); fn unmap(&mut self); }
#[rseq::driver(name = "dma_streaming_map", compatible = ["regseq,dma-streaming-map"])]
pub struct Bad;
impl DmaStreamBuffer for Bad { fn map(&mut self) {} fn unmap(&mut self) {} }
impl Lifecycle for Bad { fn on_init(&mut self) -> Result<(), RegseqError> { Ok(()) } }
