#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06: two #[rseq::pin_state(id = "…")] markers claim the same state name, both
// used in applies (PIN-ERR-DUPLICATE-STATE-MARKER).
include!("../../_lifecycle_surface.rs");
include!("../../_pinctrl_surface.rs");

#[rseq::pin_state(id = "dup")]
pub struct MarkerA;
impl PinState for MarkerA {}

#[rseq::pin_state(id = "dup")]
pub struct MarkerB;
impl PinState for MarkerB {}

#[rseq::driver(name = "bad", compatible = ["acme,bad"])]
pub struct Bad {
    #[rseq::pinctrl(id = "uart_pins")]
    pins: PinCtrl,
}

impl Lifecycle for Bad {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        self.pins.apply::<MarkerA>()?;
        self.pins.apply::<MarkerB>()?;
        Ok(())
    }
}

pub fn make() -> Bad { loop {} }
