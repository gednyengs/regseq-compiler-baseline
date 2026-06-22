#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06: a #[rseq::pinctrl(...)] attribute on a non-PinCtrl field (PIN-ERR-ATTR-ON-NON-PINCTRL).
include!("../../_lifecycle_surface.rs");
include!("../../_pinctrl_surface.rs");

#[rseq::driver(name = "bad", compatible = ["acme,bad"])]
pub struct Bad {
    #[rseq::pinctrl(id = "x")]
    foo: u32,
}

impl Lifecycle for Bad {}

pub fn make() -> Bad { loop {} }
