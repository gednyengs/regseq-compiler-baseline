// Locked DMA author surface. Buffer ownership and asynchronous channel
// ownership are independent; optional overrides are capabilities, never
// implicit success or fabricated completion.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DmaStatus {
    Ok,
    Unsupported,
    Busy,
    NoResource,
    InvalidParameter,
}

#[derive(Clone, Copy)]
pub struct DmaBufferHandle {
    pub identity: usize,
}

#[derive(Clone, Copy)]
pub struct DmaBufferRequest {
    pub size: usize,
    pub alignment: usize,
    pub boundary: usize,
    pub zero: bool,
}

pub trait DmaBuffer {
    fn allocate(&mut self, request: DmaBufferRequest) -> DmaBufferHandle;
    fn free(&mut self, buffer: DmaBufferHandle);
    fn cpu_address(&self, buffer: DmaBufferHandle) -> *mut u8;
    fn length(&self, buffer: DmaBufferHandle) -> usize;
    fn transfer_address(&self, _buffer: DmaBufferHandle) -> Option<u64> { None }
    fn set_valid_length(&mut self, _buffer: DmaBufferHandle, _length: usize) -> DmaStatus {
        DmaStatus::Unsupported
    }
}

#[derive(Clone, Copy)]
pub struct DmaChannelHandle {
    pub identity: usize,
}

#[derive(Clone, Copy)]
pub struct DmaRequestHandle {
    pub identity: usize,
}

#[derive(Clone, Copy)]
pub enum DmaDirection {
    MemoryToDevice,
    DeviceToMemory,
    MemoryToMemory,
}

#[derive(Clone, Copy)]
pub struct DmaTransferConfig {
    pub direction: DmaDirection,
    pub source_address: u64,
    pub destination_address: u64,
    pub length: usize,
}

#[derive(Clone, Copy)]
pub struct DmaRuntimeStatus {
    pub busy: bool,
    pub residue: usize,
    pub bytes_transferred: usize,
    pub pending_length: usize,
}

pub trait AsyncTransferChannel {
    fn acquire(&mut self) -> DmaChannelHandle;
    fn configure(&mut self, channel: DmaChannelHandle, config: DmaTransferConfig) -> DmaStatus;
    fn prepare(&mut self, channel: DmaChannelHandle) -> DmaRequestHandle;
    fn submit(&mut self, request: DmaRequestHandle) -> DmaStatus;
    fn activate(&mut self, channel: DmaChannelHandle) -> DmaStatus;
    fn release(&mut self, channel: DmaChannelHandle);

    fn status(&self, _channel: DmaChannelHandle) -> Option<DmaRuntimeStatus> { None }
    fn pause(&mut self, _channel: DmaChannelHandle) -> DmaStatus { DmaStatus::Unsupported }
    fn resume(&mut self, _channel: DmaChannelHandle) -> DmaStatus { DmaStatus::Unsupported }
    fn suspend(&mut self, _channel: DmaChannelHandle) -> DmaStatus { DmaStatus::Unsupported }
    fn reload(&mut self, _channel: DmaChannelHandle, _config: DmaTransferConfig) -> DmaStatus {
        DmaStatus::Unsupported
    }
    fn terminate(&mut self, _channel: DmaChannelHandle) -> DmaStatus { DmaStatus::Unsupported }
    fn synchronize(&mut self, _channel: DmaChannelHandle) -> DmaStatus { DmaStatus::Unsupported }
    fn reclaim(&mut self, _request: DmaRequestHandle) -> DmaStatus { DmaStatus::Unsupported }
    fn on_completion(&mut self, _status: DmaStatus, _residue: usize) {}
}
