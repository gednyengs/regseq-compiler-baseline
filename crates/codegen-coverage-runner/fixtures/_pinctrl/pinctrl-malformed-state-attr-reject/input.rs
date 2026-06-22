#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06: a #[rseq::pin_state] marker with no id, used in an apply (PIN-ERR-MALFORMED-STATE-ATTR).
include!("../../_lifecycle_surface.rs");
include!("../../_pinctrl_surface.rs");

#[rseq::pin_state]
pub struct Nameless;
impl PinState for Nameless {}

#[rseq::driver(name = "bad", compatible = ["acme,bad"])]
pub struct Bad {
    #[rseq::pinctrl(id = "uart_pins")]
    pins: PinCtrl,
}

impl Lifecycle for Bad {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        self.pins.apply::<Nameless>()?;
        Ok(())
    }
}

pub fn make() -> Bad { loop {} }
