#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06: a #[rseq::pin_state] marker that is not a unit struct (carries a field),
// used in an apply (PIN-ERR-NON-ZST-MARKER).
include!("../../_lifecycle_surface.rs");
include!("../../_pinctrl_surface.rs");

#[rseq::pin_state(id = "fat")]
pub struct Fat(u32);
impl PinState for Fat {}

#[rseq::driver(name = "bad", compatible = ["acme,bad"])]
pub struct Bad {
    #[rseq::pinctrl(id = "uart_pins")]
    pins: PinCtrl,
}

impl Lifecycle for Bad {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        self.pins.apply::<Fat>()?;
        Ok(())
    }
}

pub fn make() -> Bad { loop {} }
