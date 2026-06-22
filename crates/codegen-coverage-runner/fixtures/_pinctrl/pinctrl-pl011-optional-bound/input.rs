#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The pin-controller-and-GPIO-configuration M3 optional-PRESENT fixture (PLAN
// T16): a PL011-shaped AMBA driver with ONE `#[rseq::pinctrl(id = "uart_pins")]`
// `Option<PinCtrl>` (the OPTIONAL form, D02) bound to a PRESENT
// `[pinctrl.uart_pins]` section. Because the section is present, the resource is
// `Some` (D06): the compiler-managed `acquire_pinctrl` applies "default" in the
// preamble, and BOTH author call paths operate on the bound resource:
//   * the plain `if let Some(p) = &self.pinctrl { p.apply::<S>() }` path (D02 path
//     1, D13 local-tracing): `apply` on the unwrapped `&PinCtrl`;
//   * the `OptionalPinCtrl` extension-trait path (D02 path 2): `apply` directly on
//     `Option<PinCtrl>`.
// Present ⇒ both paths emit the `regseq_pinctrl_apply` hook (no fold). The trace
// proves both paths reach the bound resource: `PIN apply uart_pins sleep`
// (if-let) precedes the register write; `PIN apply uart_pins default` (trait)
// follows it. This uses the 3-variant `_pinctrl_lifecycle_surface.rs`
// (`RegseqError::PinctrlAbsent`) so the surface matches the optional-absent
// sibling (a single optional error surface across the optional corpus).

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
        // Path 1 — plain `if let Some`: recover the unwrapped `&PinCtrl` back to
        // the `Option<PinCtrl>` field through the match binding (D13/T15). Present
        // ⇒ the body runs and `apply::<SleepState>()` routes to the bound hook.
        if let Some(p) = &self.pinctrl {
            p.apply::<SleepState>()?;
        }
        // Anchor a register write so the oracle can order the two paths around it.
        self.regs.dr.write(0x42);
        // Path 2 — `OptionalPinCtrl` extension trait: `apply` directly on the
        // `Option<PinCtrl>`. Present ⇒ forwards to the bound hook (no fold, D06).
        self.pinctrl.apply::<DefaultState>()?;
        Ok(())
    }

    // `on_deinit` left default. The compiler-managed `release_pinctrl` runs at
    // teardown (LIFO) regardless.
}
