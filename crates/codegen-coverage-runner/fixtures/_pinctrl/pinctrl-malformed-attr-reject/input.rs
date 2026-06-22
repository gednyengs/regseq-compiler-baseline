#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06: the #[rseq::pinctrl(id = "")] attribute has an empty id (PIN-ERR-MALFORMED-ATTR).
include!("../../_lifecycle_surface.rs");
include!("../../_pinctrl_surface.rs");

#[rseq::driver(name = "bad", compatible = ["acme,bad"])]
pub struct Bad {
    #[rseq::pinctrl(id = "")]
    pins: PinCtrl,
}

impl Lifecycle for Bad {}

pub fn make() -> Bad { loop {} }
