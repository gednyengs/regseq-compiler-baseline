#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");
include!("../../_dma_surface.rs");

#[rseq::driver(name = "dma_independent", compatible = ["regseq,dma-independent"])]
pub struct Independent { state: u32, byte: u8 }

impl DmaBuffer for Independent {
    fn allocate(&mut self, request: DmaBufferRequest) -> DmaBufferHandle {
        if request.size == 8 { self.state = 1; DmaBufferHandle { identity: 11 } } else { DmaBufferHandle { identity: 0 } }
    }
    fn free(&mut self, buffer: DmaBufferHandle) { if buffer.identity == 11 && self.state == 8 { self.state = 9; } }
    fn cpu_address(&self, buffer: DmaBufferHandle) -> *mut u8 {
        if buffer.identity == 11 { core::ptr::addr_of!(self.byte) as *mut u8 } else { 0 as *mut u8 }
    }
    fn length(&self, buffer: DmaBufferHandle) -> usize { if buffer.identity == 11 { 8 } else { 0 } }
}

impl AsyncTransferChannel for Independent {
    fn acquire(&mut self) -> DmaChannelHandle { if self.state == 1 { self.state = 2; } DmaChannelHandle { identity: 12 } }
    fn configure(&mut self, _channel: DmaChannelHandle, _config: DmaTransferConfig) -> DmaStatus { self.state = 3; DmaStatus::Ok }
    fn prepare(&mut self, _channel: DmaChannelHandle) -> DmaRequestHandle { self.state = 4; DmaRequestHandle { identity: 13 } }
    fn submit(&mut self, _request: DmaRequestHandle) -> DmaStatus { self.state = 5; DmaStatus::Ok }
    fn activate(&mut self, _channel: DmaChannelHandle) -> DmaStatus { self.state = 6; DmaStatus::Ok }
    fn release(&mut self, channel: DmaChannelHandle) {
        if channel.identity == 12 && self.state == 2 { self.state = 7; }
    }
}

impl Lifecycle for Independent {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        let buffer = self.allocate(DmaBufferRequest { size: 8, alignment: 8, boundary: 0, zero: false });
        let channel = self.acquire();
        // No transfer is active: this fixture isolates ownership duration,
        // while dma-channel-pl011 proves the terminated transfer lifecycle.
        self.release(channel);
        // The channel is gone while the independently owned buffer remains usable.
        let _ = self.cpu_address(buffer);
        if self.length(buffer) != 8 || self.state != 7 { return Err(RegseqError::HardwareError); }
        self.state = 8;
        self.free(buffer);
        if self.state != 9 { return Err(RegseqError::HardwareError); }
        Ok(())
    }
}
