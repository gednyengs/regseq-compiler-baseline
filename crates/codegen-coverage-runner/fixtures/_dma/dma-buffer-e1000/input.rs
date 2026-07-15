#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");
include!("../../_dma_surface.rs");

#[rseq::driver(name = "dma_buffer_e1000", pci = [{ vendor = 0x8086, device = 0x100e }])]
#[rseq::device_class(network)]
pub struct E1000 {
    storage: u8,
    allocated: bool,
    valid_length: u64,
}

impl DmaBuffer for E1000 {
    fn allocate(&mut self, request: DmaBufferRequest) -> DmaBufferHandle {
        if request.size == 0 || self.allocated { return DmaBufferHandle { identity: 0 }; }
        self.allocated = true;
        self.valid_length = request.size as u64;
        DmaBufferHandle { identity: 1 }
    }
    fn free(&mut self, buffer: DmaBufferHandle) {
        if buffer.identity == 1 { self.allocated = false; }
    }
    fn cpu_address(&self, buffer: DmaBufferHandle) -> *mut u8 {
        if buffer.identity == 1 { core::ptr::addr_of!(self.storage) as *mut u8 } else { core::ptr::null_mut() }
    }
    fn transfer_address(&self, buffer: DmaBufferHandle) -> Option<u64> {
        if buffer.identity == 1 { Some(0x1000) } else { None }
    }
    fn length(&self, buffer: DmaBufferHandle) -> usize {
        if buffer.identity == 1 { self.valid_length as usize } else { 0 }
    }
    fn set_valid_length(&mut self, buffer: DmaBufferHandle, length: usize) -> DmaStatus {
        if buffer.identity != 1 { return DmaStatus::InvalidParameter; }
        self.valid_length = length as u64;
        DmaStatus::Ok
    }
}

impl Lifecycle for E1000 {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        let buffer = self.allocate(DmaBufferRequest { size: 64, alignment: 16, boundary: 0, zero: true });
        if buffer.identity == 0 { return Err(RegseqError::HardwareError); }
        let _ = self.cpu_address(buffer);
        match self.transfer_address(buffer) {
            Some(0x1000) => {}
            _ => return Err(RegseqError::HardwareError),
        }
        if self.length(buffer) != 64 { return Err(RegseqError::HardwareError); }
        if self.set_valid_length(buffer, 32) != DmaStatus::Ok || self.length(buffer) != 32 { return Err(RegseqError::HardwareError); }
        self.free(buffer);
        if self.allocated { return Err(RegseqError::HardwareError); }
        Ok(())
    }
}
