#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06: author code writes the compiler-managed pinctrl field
// (`self.pinctrl = None;`) — PIN-ERR-AUTHOR-MANAGED-FIELD. The post-L2 semantic
// pass (validate_pinctrl_semantics) catches it at the bind stage.
include!("../../_pinctrl_lifecycle_surface.rs");
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
    pinctrl: Option<PinCtrl>,
}

impl Lifecycle for Pl011Pin {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        self.pinctrl = None;
        Ok(())
    }
}
