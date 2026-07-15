#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");
include!("../../_serial_port_surface.rs");
include!("../../_dma_surface.rs");

#[rseq::driver(name = "dma_channel_pl011", compatible = ["regseq,dma-channel-pl011"])]
#[rseq::device_class(serial)]
pub struct Pl011 { state: u32 }

impl SerialPort for Pl011 {}

impl AsyncTransferChannel for Pl011 {
    fn acquire(&mut self) -> DmaChannelHandle { self.state = 1; DmaChannelHandle { identity: 7 } }
    fn configure(&mut self, channel: DmaChannelHandle, config: DmaTransferConfig) -> DmaStatus {
        if channel.identity != 7 || config.length == 0 { return DmaStatus::InvalidParameter; }
        self.state = 2; DmaStatus::Ok
    }
    fn prepare(&mut self, channel: DmaChannelHandle) -> DmaRequestHandle {
        if channel.identity == 7 && self.state == 2 { self.state = 3; DmaRequestHandle { identity: 9 } }
        else { DmaRequestHandle { identity: 0 } }
    }
    fn submit(&mut self, request: DmaRequestHandle) -> DmaStatus {
        if request.identity != 9 || self.state != 3 { return DmaStatus::InvalidParameter; }
        self.state = 4; DmaStatus::Ok
    }
    fn activate(&mut self, channel: DmaChannelHandle) -> DmaStatus {
        if channel.identity != 7 || self.state != 4 { return DmaStatus::InvalidParameter; }
        self.state = 5; DmaStatus::Ok
    }
    fn status(&self, channel: DmaChannelHandle) -> Option<DmaRuntimeStatus> {
        if channel.identity == 7 && self.state == 5 {
            Some(DmaRuntimeStatus { busy: true, residue: 16, bytes_transferred: 0, pending_length: 16 })
        } else {
            None
        }
    }
    fn terminate(&mut self, channel: DmaChannelHandle) -> DmaStatus {
        if channel.identity != 7 || self.state != 5 { return DmaStatus::InvalidParameter; }
        self.state = 6; DmaStatus::Ok
    }
    fn synchronize(&mut self, channel: DmaChannelHandle) -> DmaStatus {
        if channel.identity != 7 || self.state != 6 { return DmaStatus::InvalidParameter; }
        self.state = 7; DmaStatus::Ok
    }
    fn release(&mut self, channel: DmaChannelHandle) {
        if channel.identity == 7 && self.state == 7 { self.state = 8; }
    }
}

impl Lifecycle for Pl011 {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        let channel = self.acquire();
        let config = DmaTransferConfig { direction: DmaDirection::MemoryToDevice, source_address: 0x2000, destination_address: 0x3000, length: 16 };
        if AsyncTransferChannel::configure(self, channel, config) != DmaStatus::Ok { return Err(RegseqError::HardwareError); }
        let request = self.prepare(channel);
        if self.submit(request) != DmaStatus::Ok { return Err(RegseqError::HardwareError); }
        if self.activate(channel) != DmaStatus::Ok { return Err(RegseqError::HardwareError); }
        match self.status(channel) {
            Some(DmaRuntimeStatus { busy: true, residue: 16, .. }) => {}
            _ => return Err(RegseqError::HardwareError),
        }
        self.release(channel);
        if self.state != 5 { return Err(RegseqError::HardwareError); }
        // Termination has no completion callback. Synchronization is the only
        // release barrier and must precede channel release.
        if self.terminate(channel) != DmaStatus::Ok { return Err(RegseqError::HardwareError); }
        self.release(channel);
        if self.state != 6 { return Err(RegseqError::HardwareError); }
        if self.synchronize(channel) != DmaStatus::Ok { return Err(RegseqError::HardwareError); }
        self.release(channel);
        if self.state != 8 { return Err(RegseqError::HardwareError); }
        Ok(())
    }
}
