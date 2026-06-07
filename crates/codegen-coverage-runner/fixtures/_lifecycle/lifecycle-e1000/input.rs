#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The shared device-lifecycle author surface (the `Lifecycle` trait + an inline
// `RegseqError`). The harness verbs are gone (RD-1): the backend synthesizes the
// driving `main` from the model.
include!("../../_lifecycle_surface.rs");

// M5 shutdown-PRESENT driver: PCI-matched (Intel NIC), device_class network.
// Overrides `on_init` + `on_system_shutdown`, so the synthesized `shutdown`
// scenario emits `LC call on_system_shutdown` + a call to E1000::on_system_shutdown
// (the slot is PRESENT because the driver overrides it).
#[rseq::device_class(network)]
#[rseq::driver(name = "e1000", pci = [{ vendor = 0x8086, device = 0x100e }])]
pub struct E1000;

impl Lifecycle for E1000 {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
    fn on_system_shutdown(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main` (RD-4/RD-6). The
// manifest's `lifecycle_scenario = "shutdown"` selects the shutdown scenario:
// register (match=pci:0x8086:0x100e) -> create -> associate -> `LC call on_init`
// -> `LC call on_system_shutdown` + a call to E1000::on_system_shutdown.
