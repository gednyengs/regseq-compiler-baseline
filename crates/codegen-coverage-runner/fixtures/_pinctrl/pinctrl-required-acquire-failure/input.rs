#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The pin-controller-and-GPIO-configuration M3 required-ACQUIRE-FAILURE fixture
// (PLAN T18): the REQUIRED `PinCtrl` form (D03) whose compiler-managed
// `acquire_pinctrl` is forced to FAIL by the harness (the manifest's
// `pinctrl_fail_acquire = "uart_pins"` → `backend.pinctrl.fail-acquire`). The
// required form CONSUMES the acquire rc and HARD-FAILS (C04/D03): init aborts in
// the preamble — the trace shows `PIN acquire uart_pins default` (the attempt)
// then `LC acquire-fail pinctrl uart_pins`, and `on_init` is NEVER reached (no
// `LC call on_init`, no `PIN apply`, no register write). This is the generic-c
// half of the required-acquire-failure assertion; the Linux/Zephyr halves
// (`IS_ERR`/`PTR_ERR`, the `pinctrl_apply_state` rc) are T22 (M4). Contrast: the
// OPTIONAL form ignores the rc (best-effort, structural in the acquire block).

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
        // NEVER reached: the required acquire hard-failed in the preamble, so init
        // aborted before `on_init`. (The body is well-formed so the driver still
        // compiles; the oracle proves it does not run.)
        self.pins.apply::<SleepState>()?;
        self.regs.dr.write(0x42);
        Ok(())
    }
}
