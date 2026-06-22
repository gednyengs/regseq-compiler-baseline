#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The pin-controller-and-GPIO-configuration M3 required-PRESENT (full-sequence)
// fixture (PLAN T16): a PL011-shaped AMBA driver with ONE
// `#[rseq::pinctrl(id = "uart_pins")]` `PinCtrl` (the REQUIRED form, D03) bound to
// a present `[pinctrl.uart_pins]`. The required form has NO optional call path —
// `apply::<S>()` is the direct `&PinCtrl` intrinsic. Unlike the M1
// `-required-bound-min` (a single `apply`), this exercises the FULL state
// sequence: the compiler-managed `acquire_pinctrl` applies "default", then the
// author transitions sleep → default → (write), proving multiple `apply::<S>()`
// state resolutions on the required form. The required form uses the shared
// 2-variant `_lifecycle_surface.rs` (no `PinctrlAbsent` — a required resource is
// never absent, D03/C06), so it is golden-isolated from the optional corpus.

include!("../../_lifecycle_surface.rs");
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
    pins: PinCtrl,
}

impl Lifecycle for Pl011Pin {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // Required form: direct `&PinCtrl` calls. Two transitions prove repeated
        // state resolution (sleep, then back to default) before the anchoring write.
        self.pins.apply::<SleepState>()?;
        self.pins.apply::<DefaultState>()?;
        self.regs.dr.write(0x42);
        Ok(())
    }
}
