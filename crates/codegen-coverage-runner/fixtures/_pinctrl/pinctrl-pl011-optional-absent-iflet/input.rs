#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The pin-controller-and-GPIO-configuration M3 optional-ABSENT-via-IF-LET fixture
// (PLAN T16 hardening, Codex diff-review rec #3): the optional `Option<PinCtrl>`
// is ABSENT (no `[pinctrl.<id>]` section), AND the author selects the state via
// the plain `if let Some(p) = &self.pinctrl { p.apply::<S>() }` path (D02 path 1)
// rather than the `OptionalPinCtrl` trait. Absent ⇒ the field is statically `None`
// (no Some-materialization), so the runtime `if let` switch skips the `Some`
// branch — its `p.apply()` is statically DEAD. The dead branch must still compile
// (the apply folds to a discarded `Err(PinctrlAbsent)`, no hook) and the absent-
// only TU must remain helper-free (no `PINCTRL_HELPERS`/`regseq_pinctrl_*`). This
// is the if-let twin of `pinctrl-pl011-optional-absent` (which uses the trait
// path); together they prove BOTH absent call paths stay hook-free + trace-free.

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
        // Absent ⇒ the field is None at runtime (no Some-materialization), so this
        // `if let` Some branch is statically dead: it never runs (no `PIN apply`),
        // and its `p.apply()` folds to a discarded `Err(PinctrlAbsent)` — emitting
        // NO `regseq_pinctrl_apply` so the absent-only TU stays helper-free.
        if let Some(p) = &self.pinctrl {
            let _ = p.apply::<SleepState>();
        }
        self.regs.dr.write(0x42);
        Ok(())
    }
}
