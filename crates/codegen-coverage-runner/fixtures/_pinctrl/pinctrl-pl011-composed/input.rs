#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The pin-controller-and-GPIO-configuration COMPOSED-interaction fixture (pinctrl
// construct, PLAN T24): one driver carrying ALL FOUR preamble-ordered resources —
// an MMIO region (`regs`), a REQUIRED `PinCtrl` (`pins`), a MANAGED clock (`clk`),
// and an interrupt line (`irq`) with a bound ISR — to witness the C07 phase
// ordering end-to-end, with pinctrl threaded into the existing region/clock/
// interrupt preamble:
//   preamble : map_region -> acquire_pinctrl -> acquire_clock -> interrupt-register
//   teardown : interrupt-deregister -> clock-release -> pinctrl-release -> region-unmap
//              (exact reverse — pins-before-clock, so clock releases before pins, D08/C07).
// The generic-c oracle proves the runtime trace order; the linux_c/zephyr_c
// witnesses (`pinctrl_composed_witness_e2e`) prove the same static placement
// (devm_pinctrl_get -> devm_clk_get -> request_irq, and the Zephyr DT analog).
//
// The REQUIRED `PinCtrl` form (D03) is used (not the optional `if let`/trait path):
// the required form's handle path lowers cleanly on the real backends (the optional
// `if let` switchInt/Downcast is a generic-c-only shape, T15 design-in), giving an
// unambiguous `devm_pinctrl_get`-before-`devm_clk_get` placement witness. It binds
// a present `[pinctrl.uart_pins]` and uses the shared 2-variant `RegseqError` (a
// required resource is never absent, so no `PinctrlAbsent`).
//
// Surfaces: `_irq_pl011_surface.rs` supplies the full PL011 block (dr + the
// interrupt-path registers) + the register-access + log surface; the `Clock` and
// `PinCtrl` facades are declared INLINE (the `_clock_surface.rs` / `_pinctrl_surface.rs`
// copies minus their trimmed pl011/Mmio, which would collide with the irq surface).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_irq_pl011_surface.rs");

// -- The typed `Clock` resource facade (peripheral-clock-management construct),
//    inline (identical to `_clock_surface.rs`'s, sans the colliding pl011/Mmio).
#[rseq::abstraction("clock")]
pub struct Clock {
    _opaque: (),
}

impl Clock {
    #[inline(never)]
    #[rseq::intrinsic("clock_enable")]
    pub fn enable(&self) -> Result<(), RegseqError> {
        Ok(())
    }
    #[inline(never)]
    #[rseq::intrinsic("clock_disable")]
    pub fn disable(&self) {}
    #[inline(never)]
    #[rseq::intrinsic("clock_get_rate")]
    pub fn get_rate(&self) -> u32 {
        0
    }
}

// -- The typed `PinCtrl` resource facade (pin-controller-and-GPIO-configuration
//    construct), inline (identical to `_pinctrl_surface.rs`'s, sans the colliding
//    register-access / pl011 / Mmio surface). REQUIRED form only: the inherent
//    `apply::<S>()` on `&PinCtrl`. The pin-state markers carry the state names.
#[rseq::abstraction("pinctrl")]
pub struct PinCtrl {
    _opaque: (),
}

/// Empty marker trait — a pin-state marker type implements it; its `id` is carried
/// by the `#[rseq::pin_state(id = "…")]` attribute (D04).
pub trait PinState {}

impl PinCtrl {
    /// Select a named pin state. The resolved state name is read at lowering from
    /// the generic arg `S`'s `#[rseq::pin_state]` marker (D04/C03).
    #[inline(never)]
    #[rseq::intrinsic("pinctrl_apply")]
    pub fn apply<S: PinState>(&self) -> Result<(), RegseqError> {
        Ok(())
    }
}

#[rseq::pin_state(id = "default")]
pub struct DefaultState;
impl PinState for DefaultState {}

#[rseq::pin_state(id = "sleep")]
pub struct SleepState;
impl PinState for SleepState {}

// AMBA-matched PL011 carrying region + pinctrl + clock + interrupt. Field order is
// irrelevant to the preamble order (C07 batches by construct phase, not by field):
// map_region -> acquire_pinctrl -> acquire_clock -> interrupt-register regardless.
#[rseq::driver(name = "pl011pincomposed", bus = "amba",
    amba = [
    { id = 0x00041011, mask = 0x000fffff },
    ],
    compatible = ["regseq,pl011pincomposed"]
)]
pub struct Pl011PinComposed {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Regs>,
    #[rseq::pinctrl(id = "uart_pins")]
    pins: PinCtrl,
    #[rseq::clock(id = "uartclk")]
    clk: Clock,
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Pl011PinComposed {
    /// Mask all interrupt sources (bring-up/teardown quiesce).
    fn mask_all_interrupts(&mut self) {
        self.regs.imsc.write(0);
    }

    /// Clear all latched sources; W1C semantics.
    fn clear_all_interrupts(&mut self) {
        self.regs.icr.write(ALL_UART_INTERRUPTS_MASK);
    }

    /// The bound ISR (C04): read masked status, ack what fired. Total over the
    /// quiescent device (R08).
    #[rseq::isr]
    pub fn handle_irq(&mut self) -> IrqResult {
        let status = self.regs.mis.read();
        if status == 0 {
            return IrqResult::NotHandled;
        }
        self.regs.icr.write(status);
        IrqResult::Handled
    }
}

impl Lifecycle for Pl011PinComposed {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // The compiler-managed acquire_pinctrl (default applied) + acquire_clock +
        // interrupt-register already ran (preamble). Transition pins sleep ->
        // default (two direct &PinCtrl apply::<S>() calls), enable the clock, read
        // its rate, program DR, quiesce, then enable the UART with RX on.
        self.pins.apply::<SleepState>()?;
        self.pins.apply::<DefaultState>()?;
        self.clk.enable()?;
        let rate = self.clk.get_rate();
        self.regs.dr.write(rate);
        self.mask_all_interrupts();
        self.clear_all_interrupts();
        self.regs
            .cr
            .write_field(pl011::cr::UARTEN_ENABLED + pl011::cr::RXE_ENABLED);
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        // Author quiesce; the compiler-managed disable->deregister (interrupt),
        // release (clock), then release (pinctrl) run AFTER this callback, in
        // exact-reverse order (clock before pins — pins were acquired first).
        self.mask_all_interrupts();
        self.regs.cr.write_field(pl011::cr::UARTEN_DISABLED);
        self.clear_all_interrupts();
        self.clk.disable();
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main`; the manifest's
// `lifecycle_scenario = "orderly"` drives the run.
