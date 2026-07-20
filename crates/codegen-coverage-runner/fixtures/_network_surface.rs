// Locked network-interface author surface. The packet value is opaque to the
// compiler. Packet layout, descriptor rings/indices, DMA mappings, interrupt
// lines, and deferred execution are supplied by their existing owners.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OperationalState {
    Unknown,
    Down,
    Up,
    NotPresent,
    LowerLayerDown,
    Testing,
    Dormant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SubmissionVerdict {
    Accepted,
    BusyOrQueueFull,
}

#[derive(Clone, Copy)]
pub struct OpaquePacketBuffer {
    identity: usize,
}

impl OpaquePacketBuffer {
    pub fn from_native(identity: usize) -> Self { Self { identity } }
    pub fn native_identity(self) -> usize { self.identity }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReceiveFilterMode { Ordinary, AllMulticast, Promiscuous }

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NetworkProperty {
    Published,
    AdministrativelyEnabled,
    OperationalReadiness,
    CarrierLinkState,
    Name,
    Index,
    Mtu,
    LinkLayerAddress,
    ReceiveFilterMode,
    LastStateChangeTime,
    EventSource,
    PerPacketCompletionSource,
}

#[derive(Clone, Copy)]
pub struct LinkLayerAddress {
    pub bytes: [u8; 32],
    pub length: usize,
}

#[derive(Clone, Copy)]
pub enum NetworkPropertyValue {
    Bool(bool),
    U32(u32),
    U64(u64),
    Name(*const u8),
    Address(LinkLayerAddress),
    OperationalState(OperationalState),
    ReceiveFilter(ReceiveFilterMode),
    NativeHandle(usize),
}

#[derive(Clone, Copy, Default)]
pub struct InterfaceCounters {
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub rx_errors: u64,
    pub tx_errors: u64,
    pub rx_dropped: u64,
    pub tx_dropped: u64,
}

pub trait NetworkInterface {
    // Optional runtime-publication capability.
    fn set_published(&mut self, _desired: bool) -> isize { -95 }

    // Required portable core.
    fn set_admin_enabled(&mut self, _desired: bool) -> isize { -95 }
    fn transmit(&mut self, _packet: OpaquePacketBuffer) -> Option<SubmissionVerdict> { None }
    fn report_carrier(&mut self, _present: bool) {}
    fn query_operational_state(&self) -> OperationalState { OperationalState::Unknown }
    fn get_property(&self, _property: NetworkProperty) -> Option<NetworkPropertyValue> { None }
    fn read_counters(&self) -> InterfaceCounters { InterfaceCounters::default() }

    // Independently optional capabilities. Missing overrides remain absent.
    fn receive(&mut self, _packet: OpaquePacketBuffer) {}
    fn report_dormant(&mut self, _dormant: bool) -> isize { -95 }
    fn set_property(&mut self, _property: NetworkProperty, _value: NetworkPropertyValue) -> isize { -95 }
    fn register_observer(&mut self, _observer: usize) -> isize { -95 }
    fn unregister_observer(&mut self, _observer: usize) -> isize { -95 }
}
