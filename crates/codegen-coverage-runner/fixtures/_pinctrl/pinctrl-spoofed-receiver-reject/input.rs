#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06/D14: a #[rseq::intrinsic("pinctrl_apply")] tag on a method whose receiver is
// not the PinCtrl/Option<PinCtrl> shell — a spoofed pinctrl op (PIN-ERR-SPOOFED-RECEIVER).
include!("../../_lifecycle_surface.rs");
include!("../../_pinctrl_surface.rs");

pub struct Bogus {
    _x: u32,
}

impl Bogus {
    #[inline(never)]
    #[rseq::intrinsic("pinctrl_apply")]
    pub fn apply<S: PinState>(&self) -> Result<(), RegseqError> {
        Ok(())
    }
}

#[rseq::driver(name = "spoof", compatible = ["acme,spoof"])]
pub struct Spoof {
    bogus: Bogus,
}

impl Lifecycle for Spoof {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        self.bogus.apply::<DefaultState>()?;
        Ok(())
    }
}

pub fn make() -> Spoof { loop {} }
