#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");

pub struct OpaquePacketBuffer;
pub enum SubmissionVerdict { Accepted, BusyOrQueueFull }
pub enum OperationalState { Unknown, Down, Up }
pub enum NetworkProperty { Index }
pub enum NetworkPropertyValue { U32(u32) }
pub struct InterfaceCounters;

pub trait NetworkInterface {
    fn set_published(&mut self, _desired: bool) -> isize { -95 }
    fn set_admin_enabled(&mut self, desired: bool) -> isize;
    fn transmit(&mut self, packet: OpaquePacketBuffer) -> isize;
    fn receive(&mut self, _packet: OpaquePacketBuffer) {}
    fn report_carrier(&mut self, present: bool);
    fn report_dormant(&mut self, _dormant: bool) -> isize { -95 }
    fn query_operational_state(&self) -> OperationalState;
    fn get_property(&self, property: NetworkProperty) -> Option<NetworkPropertyValue>;
    fn set_property(&mut self, _property: NetworkProperty, _value: NetworkPropertyValue) -> isize { -95 }
    fn read_counters(&self) -> InterfaceCounters;
    fn register_observer(&mut self, _observer: usize) -> isize { -95 }
    fn unregister_observer(&mut self, _observer: usize) -> isize { -95 }
}

#[rseq::driver(name = "bad-network-signature", compatible = ["regseq,bad-network-signature"])]
#[rseq::device_class(network)]
pub struct BadNetworkSignature;

impl NetworkInterface for BadNetworkSignature {
    fn set_admin_enabled(&mut self, _desired: bool) -> isize { 0 }
    fn transmit(&mut self, _packet: OpaquePacketBuffer) -> isize { 0 }
    fn report_carrier(&mut self, _present: bool) {}
    fn query_operational_state(&self) -> OperationalState { OperationalState::Unknown }
    fn get_property(&self, _property: NetworkProperty) -> Option<NetworkPropertyValue> { None }
    fn read_counters(&self) -> InterfaceCounters { InterfaceCounters }
}

impl Lifecycle for BadNetworkSignature {}
