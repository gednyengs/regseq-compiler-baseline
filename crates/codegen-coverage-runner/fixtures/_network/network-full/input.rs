#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");
include!("../../_network_surface.rs");
include!("../../_dma_surface.rs");
include!("../../_transport_surface.rs");
include!("../../_deferred_work_surface.rs");
include!("../../_interrupt_surface.rs");

#[rseq::driver(name = "netfull", bus = "pci",
    pci = [{ vendor = 0x8086, device = 0x100e }],
    compatible = ["regseq,netfull"])]
#[rseq::device_class(network)]
pub struct NetFull {
    work: Work,
    #[rseq::interrupt(id = "net_irq")]
    irq: Interrupt,
    admin: bool,
    carrier: bool,
    dormant: bool,
    published: bool,
    tx_owned: u64,
    rx_owned: u64,
    observer: u64,
    storage: u8,
}

impl NetworkInterface for NetFull {
    fn set_published(&mut self, desired: bool) -> isize { self.published = desired; 0 }
    fn set_admin_enabled(&mut self, desired: bool) -> isize { self.admin = desired; 0 }
    fn transmit(&mut self, packet: OpaquePacketBuffer) -> Option<SubmissionVerdict> {
        if packet.native_identity() == 0 { return Some(SubmissionVerdict::BusyOrQueueFull); }
        self.tx_owned += 1;
        Some(SubmissionVerdict::Accepted)
    }
    fn receive(&mut self, packet: OpaquePacketBuffer) {
        if packet.native_identity() != 0 { self.rx_owned += 1; }
    }
    fn report_carrier(&mut self, present: bool) { self.carrier = present; }
    fn report_dormant(&mut self, dormant: bool) -> isize { self.dormant = dormant; 0 }
    fn query_operational_state(&self) -> OperationalState {
        if self.admin && self.carrier && !self.dormant { OperationalState::Up } else { OperationalState::Down }
    }
    fn get_property(&self, property: NetworkProperty) -> Option<NetworkPropertyValue> {
        match property {
            NetworkProperty::Published => Some(NetworkPropertyValue::Bool(self.published)),
            NetworkProperty::AdministrativelyEnabled => Some(NetworkPropertyValue::Bool(self.admin)),
            NetworkProperty::OperationalReadiness => Some(NetworkPropertyValue::OperationalState(self.query_operational_state())),
            NetworkProperty::CarrierLinkState => Some(NetworkPropertyValue::Bool(self.carrier)),
            NetworkProperty::Index => Some(NetworkPropertyValue::U32(7)),
            NetworkProperty::Mtu => Some(NetworkPropertyValue::U32(1500)),
            _ => None,
        }
    }
    fn set_property(&mut self, property: NetworkProperty, value: NetworkPropertyValue) -> isize {
        match (property, value) {
            (NetworkProperty::ReceiveFilterMode, NetworkPropertyValue::ReceiveFilter(_)) => 0,
            _ => -95,
        }
    }
    fn read_counters(&self) -> InterfaceCounters {
        InterfaceCounters { tx_packets: self.tx_owned, rx_packets: self.rx_owned, ..InterfaceCounters::default() }
    }
    fn register_observer(&mut self, observer: usize) -> isize { self.observer = observer as u64; 0 }
    fn unregister_observer(&mut self, observer: usize) -> isize {
        if self.observer == observer as u64 { self.observer = 0; 0 } else { -22 }
    }
}

impl DmaBuffer for NetFull {
    fn allocate(&mut self, request: DmaBufferRequest) -> DmaBufferHandle {
        DmaBufferHandle { identity: if request.size != 0 { 1 } else { 0 } }
    }
    fn free(&mut self, _buffer: DmaBufferHandle) {}
    fn cpu_address(&self, buffer: DmaBufferHandle) -> *mut u8 {
        if buffer.identity == 1 { core::ptr::addr_of!(self.storage) as *mut u8 } else { core::ptr::null_mut() }
    }
    fn length(&self, buffer: DmaBufferHandle) -> usize { if buffer.identity == 1 { 64 } else { 0 } }
    fn transfer_address(&self, buffer: DmaBufferHandle) -> Option<u64> {
        if buffer.identity == 1 { Some(0x1000) } else { None }
    }
}

impl NetFull {
    #[rseq::isr]
    fn handle_irq(&mut self) -> IrqResult {
        self.work.submit();
        IrqResult::Handled
    }

    #[rseq::work_handler]
    fn poll_packets(&mut self) { self.receive(OpaquePacketBuffer::from_native(2)); }
}

impl Lifecycle for NetFull {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        if self.set_published(true) != 0 || self.set_admin_enabled(true) != 0 { return Err(RegseqError::HardwareError); }
        self.report_carrier(true);
        if self.query_operational_state() != OperationalState::Up { return Err(RegseqError::HardwareError); }
        match self.transmit(OpaquePacketBuffer::from_native(1)) {
            Some(SubmissionVerdict::Accepted) => {}
            _ => return Err(RegseqError::HardwareError),
        }
        match self.transmit(OpaquePacketBuffer::from_native(0)) {
            Some(SubmissionVerdict::BusyOrQueueFull) => {}
            _ => return Err(RegseqError::HardwareError),
        }
        match self.get_property(NetworkProperty::Index) {
            Some(NetworkPropertyValue::U32(7)) => {}
            _ => return Err(RegseqError::HardwareError),
        }
        let ring = self.allocate(DmaBufferRequest { size: 64, alignment: 16, boundary: 0, zero: true });
        match self.transfer_address(ring) {
            Some(0x1000) => {}
            _ => return Err(RegseqError::HardwareError),
        }
        match submit(1, ring.identity) {
            SubmissionOutcome::Accepted => {}
            SubmissionOutcome::Rejected(_) => return Err(RegseqError::HardwareError),
        }
        self.work.submit();
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        self.set_admin_enabled(false);
        self.set_published(false);
        Ok(())
    }
}
