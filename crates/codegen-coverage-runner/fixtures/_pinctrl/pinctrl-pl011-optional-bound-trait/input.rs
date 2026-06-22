#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The pin-controller-and-GPIO-configuration optional-PRESENT fixture, TRAIT-PATH
// ONLY (PLAN M4/T23): a PL011-shaped AMBA driver with ONE
// `#[rseq::pinctrl(id = "uart_pins")]` `Option<PinCtrl>` (the OPTIONAL form, D02)
// bound to a PRESENT `[pinctrl.uart_pins]` section. Because the section is
// present, the resource is `Some` (D06). BOTH author applies go through the
// `OptionalPinCtrl` extension trait (`apply` directly on `Option<PinCtrl>`, D02
// path 2) — there is NO plain `if let Some` path here. The trait call is
// intercepted at the call site (it does not pattern-match the `Option`), so the
// real backends lower it directly to their pinctrl API without an `if let`
// switchInt/Downcast; the plain-`if let` real-backend lowering is the harder T15
// shape (covered behaviorally by the generic-c oracle, design-in on
// Linux/Zephyr). Uses the 3-variant `_pinctrl_lifecycle_surface.rs`
// (`RegseqError::PinctrlAbsent`) so the surface matches the optional corpus.

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
        // Path 2 — `OptionalPinCtrl` extension trait: `apply` directly on the
        // `Option<PinCtrl>`. Present ⇒ forwards to the bound resource (no fold,
        // D06). Apply "sleep" before the register write...
        self.pinctrl.apply::<SleepState>()?;
        // Anchor a register write so the oracle can order the two applies around it.
        self.regs.dr.write(0x42);
        // ...and "default" after.
        self.pinctrl.apply::<DefaultState>()?;
        Ok(())
    }

    // `on_deinit` left default. The compiler-managed `release_pinctrl` runs at
    // teardown (LIFO) regardless.
}
