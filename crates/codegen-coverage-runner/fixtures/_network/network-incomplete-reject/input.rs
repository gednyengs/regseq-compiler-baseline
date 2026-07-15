#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");
include!("../../_network_surface.rs");

#[rseq::driver(name = "incomplete", compatible = ["regseq,incomplete"])]
#[rseq::device_class(network)]
pub struct Incomplete;

impl NetworkInterface for Incomplete {
    fn set_admin_enabled(&mut self, _desired: bool) -> isize { 0 }
    fn transmit(&mut self, _packet: OpaquePacketBuffer) -> Option<SubmissionVerdict> { None }
    fn report_carrier(&mut self, _present: bool) {}
    fn query_operational_state(&self) -> OperationalState { OperationalState::Unknown }
    fn get_property(&self, _property: NetworkProperty) -> Option<NetworkPropertyValue> { None }
}

impl Lifecycle for Incomplete {}
