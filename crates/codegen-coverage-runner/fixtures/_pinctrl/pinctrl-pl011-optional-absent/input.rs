#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The pin-controller-and-GPIO-configuration M3 optional-ABSENT fixture (PLAN T16):
// a PL011-shaped AMBA driver with ONE `#[rseq::pinctrl(id = "uart_pins")]`
// `Option<PinCtrl>` (the OPTIONAL form) whose platform.toml has NO
// `[pinctrl.uart_pins]` section. Absence ⇒ the resource binds to `None`
// (`resolved = false`, D06): there is no `acquire_pinctrl`/`release_pinctrl`
// preamble, the prelude gate (`model_has_resolved_pinctrl_resource`) does NOT
// fire (no `PINCTRL_HELPERS`/`regseq_pinctrl_*` emitted), and the author's
// `OptionalPinCtrl::apply::<SleepState>()` compile-time-folds to a direct
// `Err(RegseqError::PinctrlAbsent)` — NO hook call, NO `PIN apply` trace (D06/C04).
// The author DISCARDS the result (`let _ =`), so `on_init` still returns `Ok`.
// This is the POSITIVE-not-vacuous absent oracle (S12): the runner test asserts
// the model carries `required=false`/`resolved=false`/empty `pin_states`, the
// emitted source has no `regseq_pinctrl_*`, the `ENODEV`/`PinctrlAbsent` fold IS
// present, and the run exits cleanly with no `PIN` lines.

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
        // Optional-ABSENT: the `OptionalPinCtrl` extension-trait `apply` on the
        // `None` resource compile-time-folds to `Err(PinctrlAbsent)` (D06). The
        // author ignores it (`let _ =`), so `on_init` succeeds and the fold leaves
        // NO `PIN apply` trace and NO `regseq_pinctrl_apply` call.
        let _ = self.pinctrl.apply::<SleepState>();
        // A register write anchors the (still-present) MMIO path so the run is not
        // vacuous: the trace must show the `W…` with NO surrounding `PIN` lines.
        self.regs.dr.write(0x42);
        Ok(())
    }
}
