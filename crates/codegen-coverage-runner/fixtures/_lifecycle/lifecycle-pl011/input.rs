#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The shared device-lifecycle author surface (the `Lifecycle` trait + an inline
// `RegseqError`). The harness verbs are gone (RD-1): the backend synthesizes the
// driving `main` from the model.
include!("../../_lifecycle_surface.rs");

// M5 shutdown-OMIT driver: AMBA-matched (ARM PrimeCell UART), device_class serial.
// Overrides `on_init` only — NOT `on_system_shutdown` — so the synthesized
// `shutdown` scenario emits ZERO shutdown glue (omit-when-absent, D10/I02): no
// marker, no slot, ABSENT in golden.c. The 5-negative.
#[rseq::device_class(serial)]
#[rseq::driver(name = "pl011", amba = [{ id = 0x00041011, mask = 0x000fffff }])]
pub struct Pl011;

impl Lifecycle for Pl011 {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main` (RD-4/RD-6). The
// manifest's `lifecycle_scenario = "shutdown"` selects the shutdown scenario:
// register (match=amba:0x41011/0xfffff) -> create -> associate -> `LC call
// on_init`, with NO on_system_shutdown glue (the driver does not override it).
