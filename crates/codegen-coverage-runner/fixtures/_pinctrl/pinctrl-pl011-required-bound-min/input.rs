#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The pin-controller-and-GPIO-configuration M1 behavioral-oracle fixture (PLAN
// T08): the smallest valid pinctrl spec on a PL011-shaped AMBA driver —
// construct-#1 register (DR) + ONE `#[rseq::pinctrl(id = "uart_pins")]` PinCtrl
// (the REQUIRED form — the unambiguous easy receiver, D13/D03) bound to a present
// `[pinctrl.uart_pins]`. The compiler-managed `acquire_pinctrl` rides the
// preamble (after the bus/map_region keystone, BEFORE `acquire_clock` —
// pins-before-clock, D08), applying the `"default"` state. The author body
// selects the `"sleep"` state via `apply::<SleepState>()` and anchors a register
// write; the compiler-managed `release_pinctrl` runs LIFO at teardown.
// `expected.stdout` is the PIN trace oracle (C05): `PIN acquire uart_pins default`
// before the author body, `PIN apply uart_pins sleep` before the first `W…`, then
// `PIN release uart_pins`. The distinct default/sleep states prove the
// generic-`S` -> `#[rseq::pin_state]` -> state-name resolution (D04/C03).

include!("../../_lifecycle_surface.rs");
include!("../../_pinctrl_surface.rs");

// AMBA-matched PL011-shaped driver with the bus-managed MMIO region joined to
// `[mmio.regs]` and the pinctrl joined to `[pinctrl.uart_pins]`.
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
        // The compiler-managed `acquire_pinctrl` already ran (preamble, applying
        // "default"). The author selects the "sleep" state, then anchors a
        // register write so the oracle can assert `PIN apply` precedes the `W…`.
        self.pins.apply::<SleepState>()?;
        self.regs.dr.write(0x42);
        Ok(())
    }

    // `on_deinit` left default (no-op). The compiler-managed `release_pinctrl`
    // runs at teardown (LIFO) regardless.
}

// No `regseq_main`: the generic-c backend synthesizes `main`; the manifest's
// `lifecycle_scenario = "orderly"` drives the run.
