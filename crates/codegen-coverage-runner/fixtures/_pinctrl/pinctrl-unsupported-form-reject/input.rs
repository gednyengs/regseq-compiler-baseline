#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06: a recognized pinctrl field in the array form [PinCtrl; N] (PIN-ERR-UNSUPPORTED-FORM).
include!("../../_lifecycle_surface.rs");
include!("../../_pinctrl_surface.rs");

#[rseq::driver(name = "bad", compatible = ["acme,bad"])]
pub struct Bad {
    #[rseq::pinctrl(id = "x")]
    pins: [PinCtrl; 2],
}

impl Lifecycle for Bad {}

pub fn make() -> Bad { loop {} }
