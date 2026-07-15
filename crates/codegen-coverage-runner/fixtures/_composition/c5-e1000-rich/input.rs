#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");
include!("../../_network_surface.rs");
include!("../../_transport_surface.rs");
include!("../../_composition_surface.rs");

mod e1000 {
    use super::{ReadOnly, ReadWrite, Reserved, WriteOnly};

    pub mod ctrl { pub enum Register {} }
    pub mod status { pub enum Register {} }
    pub mod icr { pub enum Register {} }
    pub mod imc { pub enum Register {} }
    pub mod tctl { pub enum Register {} }
    pub mod tdbal { pub enum Register {} }
    pub mod tdbah { pub enum Register {} }
    pub mod tdlen { pub enum Register {} }
    pub mod tdt { pub enum Register {} }

    #[repr(C)]
    pub struct Registers {
        pub ctrl: ReadWrite<u32, ctrl::Register>,
        pub _reserved0: Reserved<u32>,
        pub status: ReadOnly<u32, status::Register>,
        pub _reserved1: [Reserved<u32>; 45],
        pub icr: ReadOnly<u32, icr::Register>,
        pub _reserved2: [Reserved<u32>; 5],
        pub imc: WriteOnly<u32, imc::Register>,
        pub _reserved3: [Reserved<u32>; 201],
        pub tctl: ReadWrite<u32, tctl::Register>,
        pub _reserved4: [Reserved<u32>; 3327],
        pub tdbal: ReadWrite<u32, tdbal::Register>,
        pub tdbah: ReadWrite<u32, tdbah::Register>,
        pub tdlen: ReadWrite<u32, tdlen::Register>,
        pub _reserved5: [Reserved<u32>; 3],
        pub tdt: ReadWrite<u32, tdt::Register>,
    }
}

const CTRL_LINK_UP: FieldValue<u32, e1000::ctrl::Register> = FieldValue::new(1 << 6, 1 << 6);
const TCTL_ENABLE: FieldValue<u32, e1000::tctl::Register> = FieldValue::new(1 << 1, 1 << 1);
const CFG_CACHE_LINE_SIZE: u32 = 0x0c;

#[rseq::driver(name = "c5_e1000", bus = "pci",
    pci = [{ vendor = 0x8086, device = 0x100e }],
    compatible = ["regseq,c5-e1000"])]
#[rseq::device_class(network)]
pub struct C5E1000 {
    #[rseq::mmio(id = "regs", bar = 0)]
    regs: Mmio<e1000::Registers>,
    admin_enabled: bool,
    carrier: bool,
    tx_packets: u64,
    rx_packets: u64,
    tx_tail: u32,
}

impl NetworkInterface for C5E1000 {
    fn set_admin_enabled(&mut self, desired: bool) -> isize {
        self.admin_enabled = desired;
        if desired { self.regs.tctl.modify(TCTL_ENABLE); } else { self.regs.tctl.write(0); }
        0
    }

    fn transmit(&mut self, packet: OpaquePacketBuffer) -> Option<SubmissionVerdict> {
        if !self.admin_enabled || packet.native_identity() == 0 {
            return Some(SubmissionVerdict::BusyOrQueueFull);
        }
        match submit(1, 1) {
            SubmissionOutcome::Accepted => {
                self.tx_tail += 1;
                self.regs.tdt.write(self.tx_tail);
                self.tx_packets += 1;
                Some(SubmissionVerdict::Accepted)
            }
            SubmissionOutcome::Rejected(_) => Some(SubmissionVerdict::BusyOrQueueFull),
        }
    }

    fn receive(&mut self, packet: OpaquePacketBuffer) {
        if packet.native_identity() != 0 { self.rx_packets += 1; }
    }

    fn report_carrier(&mut self, present: bool) { self.carrier = present; }

    fn query_operational_state(&self) -> OperationalState {
        if self.admin_enabled && self.carrier { OperationalState::Up } else { OperationalState::Down }
    }

    fn get_property(&self, property: NetworkProperty) -> Option<NetworkPropertyValue> {
        match property {
            NetworkProperty::AdministrativelyEnabled => Some(NetworkPropertyValue::Bool(self.admin_enabled)),
            NetworkProperty::CarrierLinkState => Some(NetworkPropertyValue::Bool(self.carrier)),
            NetworkProperty::OperationalReadiness => Some(NetworkPropertyValue::OperationalState(self.query_operational_state())),
            NetworkProperty::Index => Some(NetworkPropertyValue::U32(1)),
            NetworkProperty::Mtu => Some(NetworkPropertyValue::U32(1500)),
            _ => None,
        }
    }

    fn read_counters(&self) -> InterfaceCounters {
        InterfaceCounters { tx_packets: self.tx_packets, rx_packets: self.rx_packets, ..InterfaceCounters::default() }
    }
}

impl Lifecycle for C5E1000 {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        let _vendor: u16 = regseq::bus::read_config::<u16>(0x00);
        let _device: u16 = regseq::bus::read_config::<u16>(0x02);
        regseq::bus::write_config::<u8>(CFG_CACHE_LINE_SIZE, 0x10);
        self.regs.ctrl.modify(CTRL_LINK_UP);
        self.regs.tdbah.write(0);
        self.regs.tdbal.write(0x2000);
        self.regs.tdlen.write(0x80);
        self.regs.tdt.write(0);
        self.report_carrier(true);
        if self.set_admin_enabled(true) != 0 { return Err(RegseqError::HardwareError); }
        let _cause = self.regs.icr.read();
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        self.regs.imc.write(0xffff_ffff);
        if self.set_admin_enabled(false) != 0 { return Err(RegseqError::HardwareError); }
        Ok(())
    }
}
