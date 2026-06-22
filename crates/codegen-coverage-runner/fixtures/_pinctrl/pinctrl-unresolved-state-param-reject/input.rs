#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06/C03: apply::<S>() whose concrete S is NOT a #[rseq::pin_state] marker (a
// hand-rolled `impl PinState`) — PIN-ERR-UNRESOLVED-STATE-PARAM. Caught by the
// post-L2 semantic pass at the bind stage.
include!("../../_lifecycle_surface.rs");
include!("../../_pinctrl_surface.rs");

pub struct NotAMarker;
impl PinState for NotAMarker {}

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
        self.pins.apply::<NotAMarker>()?;
        Ok(())
    }
}
