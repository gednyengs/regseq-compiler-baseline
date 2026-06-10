#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The linux_c smoke fixture (backend-linux-c T05): the SMALLEST input that
// flows the C02 linux pipeline row end-to-end — bridge → `emit_linux_c` →
// byte-exact `golden.linux.c`. An AMBA-matched fieldless lifecycle driver
// with trivial Ok(()) callbacks; no MMIO region, no platform.toml (the BRM
// preamble is exercised by the composed PL011 fixture, T10).

// The shared device-lifecycle author surface (the `Lifecycle` trait + the
// inline `RegseqError`).
include!("../../_lifecycle_surface.rs");

// A dummy AMBA periphid (NOT a real PrimeCell part): this fixture proves the
// scaffold, not a device.
#[rseq::driver(name = "linux-minimal", bus = "amba", amba = [{ id = 0x00099001, mask = 0x000fffff }])]
pub struct LinuxMinimal;

impl Lifecycle for LinuxMinimal {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        Ok(())
    }
}
