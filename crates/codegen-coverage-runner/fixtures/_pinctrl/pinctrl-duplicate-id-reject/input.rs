#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06: two pinctrl fields on one driver declare the same id (PIN-ERR-DUPLICATE-ID).
include!("../../_lifecycle_surface.rs");
include!("../../_pinctrl_surface.rs");

#[rseq::driver(name = "bad", compatible = ["acme,bad"])]
pub struct Bad {
    #[rseq::pinctrl(id = "dup")]
    a: PinCtrl,
    #[rseq::pinctrl(id = "dup")]
    b: PinCtrl,
}

impl Lifecycle for Bad {}

pub fn make() -> Bad { loop {} }
