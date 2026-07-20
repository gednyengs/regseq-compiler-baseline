#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]
include!("../../_lifecycle_surface.rs");
pub struct DmaBufferRequest { pub size: usize }
pub struct DmaBufferHandle { pub identity: usize }
pub trait DmaBuffer {
    fn allocate(&mut self, request: DmaBufferRequest) -> DmaBufferHandle;
}
#[rseq::driver(name = "dma_buffer_incomplete", compatible = ["regseq,dma-buffer-incomplete"])]
pub struct Bad;
impl DmaBuffer for Bad {
    fn allocate(&mut self, _request: DmaBufferRequest) -> DmaBufferHandle { DmaBufferHandle { identity: 1 } }
}
impl Lifecycle for Bad { fn on_init(&mut self) -> Result<(), RegseqError> { Ok(()) } }
