#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06/C03: an apply::<IdleState>() selecting state "idle" NOT in the joined
// inventory ["default","sleep"] — PIN-ERR-INVALID-STATE-MARKER. Asserted through
// the plain `if let Some(p) = &self.pinctrl` path: the membership check is the
// SHARED receiver recovery the backend also uses, so it validates BOTH call paths
// (no split-brain). Caught by the post-L2 semantic pass at the bind stage.
include!("../../_pinctrl_lifecycle_surface.rs");
include!("../../_pinctrl_surface.rs");

#[rseq::pin_state(id = "idle")]
pub struct IdleState;
impl PinState for IdleState {}

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
        if let Some(p) = &self.pinctrl {
            p.apply::<IdleState>()?;
        }
        Ok(())
    }
}
