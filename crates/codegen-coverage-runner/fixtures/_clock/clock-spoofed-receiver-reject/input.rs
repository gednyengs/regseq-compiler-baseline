#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06 N22: a `clock_enable` intrinsic tag on a non-`Clock` receiver (`Bogus`).
include!("../../_lifecycle_surface.rs");
include!("../../_clock_surface.rs");

pub struct Bogus {
    _x: u32,
}

impl Bogus {
    #[inline(never)]
    #[rseq::intrinsic("clock_enable")]
    pub fn enable(&self) -> Result<(), RegseqError> {
        Ok(())
    }
}

#[rseq::driver(name = "spoof", compatible = ["acme,spoof"])]
pub struct Spoof {
    bogus: Bogus,
}

impl Lifecycle for Spoof {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        self.bogus.enable()?;
        Ok(())
    }
}

pub fn make() -> Spoof { loop {} }
