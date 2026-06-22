#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// A recognition-clean REQUIRED PinCtrl driver that applies a state (so the bridge
// fully instantiates the resource); the reject is a bind-stage error (L2 CONTENT
// or parser SHAPE) in platform.toml, which fires before any post-L2 membership
// check (so the applied state is immaterial).
include!("../../_lifecycle_surface.rs");
include!("../../_pinctrl_surface.rs");

#[rseq::driver(name = "pl011pin", bus = "amba",
    amba = [
    { id = 0x00041011, mask = 0x000fffff },
    ],
    compatible = ["regseq,pl011pin"]
)]
pub struct Pl011Pin {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Regs>,
    #[rseq::pinctrl(id = "uart_pins")]
    pins: PinCtrl,
}

impl Lifecycle for Pl011Pin {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        self.pins.apply::<SleepState>()?;
        Ok(())
    }
}
