#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]
include!("../../_lifecycle_surface.rs");
include!("../../_dma_surface.rs");
#[rseq::driver(name = "dma_channel_incomplete", compatible = ["regseq,dma-channel-incomplete"])]
pub struct Bad;
impl AsyncTransferChannel for Bad {
    fn acquire(&mut self) -> DmaChannelHandle { DmaChannelHandle { identity: 1 } }
    fn configure(&mut self, _channel: DmaChannelHandle, _config: DmaTransferConfig) -> DmaStatus { DmaStatus::Ok }
    fn prepare(&mut self, _channel: DmaChannelHandle) -> DmaRequestHandle { DmaRequestHandle { identity: 1 } }
    fn submit(&mut self, _request: DmaRequestHandle) -> DmaStatus { DmaStatus::Ok }
    fn activate(&mut self, _channel: DmaChannelHandle) -> DmaStatus { DmaStatus::Ok }
    fn release(&mut self, _channel: DmaChannelHandle) {}
    fn terminate(&mut self, _channel: DmaChannelHandle) -> DmaStatus { DmaStatus::Ok }
}
impl Lifecycle for Bad { fn on_init(&mut self) -> Result<(), RegseqError> { Ok(()) } }
