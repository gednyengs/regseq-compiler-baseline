#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// Pin-controller-and-GPIO-configuration TIER-2 VEHICLE (eval QEMU behavioral
// ground truth): a PLAIN-AMBA PL011 driver (NOT `#[rseq::device_class(serial)]`,
// because the serial fast-path fail-loud-rejects a resolved pinctrl — the M4
// serial guard) carrying ONE `#[rseq::pinctrl(id = "uart_pins")]` `Option<PinCtrl>`
// (the OPTIONAL form, D02) bound to a PRESENT `[pinctrl.uart_pins]` section. The
// OPTIONAL form is used deliberately: on the QEMU `virt` target the PL011 node has
// NO pin controller, so the required form's `pinctrl_lookup_state("default")` would
// fail probe; the optional form lowers to `pinctrl_pm_select_default_state(dev)`,
// which returns 0 (a no-op) when `dev->pins` is NULL — so the driver still probes,
// and the pinctrl scaffold is exercised-but-inert. The Tier-2 question this answers:
// does adding the (optional) pinctrl construct to a real, bootable PL011 driver
// still let it bind the reserved test UART, run its bring-up, and tear down cleanly
// on real Linux? (It cannot witness real pin muxing — QEMU virt has no pinctrl
// device — see the surface-convergence doc's phase limitations.)
//
// Surfaces: `_pinctrl_lifecycle_surface.rs` (the 3-variant `RegseqError` with
// `PinctrlAbsent`, the optional corpus's lifecycle) + `_irq_pl011_surface.rs` (the
// full PL011 register block + `Mmio` + the `regseq::log` surface the smoke gate
// greps). The `Option<PinCtrl>` facade is declared INLINE (the optional trait
// `OptionalPinCtrl` path — the real-backend-lowerable path, NOT plain `if let`).

include!("../../_pinctrl_lifecycle_surface.rs");
include!("../../_irq_pl011_surface.rs");

// -- The typed `PinCtrl` resource facade + the optional forwarding trait, inline
//    (identical to `_pinctrl_surface.rs`'s, sans the colliding register-access /
//    pl011 / Mmio surface, which `_irq_pl011_surface.rs` already supplies).
#[rseq::abstraction("pinctrl")]
pub struct PinCtrl {
    _opaque: (),
}

/// Empty marker trait — a pin-state marker type implements it; its `id` is carried
/// by the `#[rseq::pin_state(id = "…")]` attribute (D04).
pub trait PinState {}

impl PinCtrl {
    #[inline(never)]
    #[rseq::intrinsic("pinctrl_apply")]
    pub fn apply<S: PinState>(&self) -> Result<(), RegseqError> {
        Ok(())
    }
}

/// Optional-form forwarding (D02/C08): the `apply` lives on an EXTENSION TRAIT
/// (`Option` is foreign). The concrete `impl … for Option<PinCtrl>` method carries
/// the intrinsic tag; the call site is intercepted (no `if let` switchInt), so the
/// real backends lower it directly to `pinctrl_pm_select_<state>_state(dev)`.
pub trait OptionalPinCtrl {
    fn apply<S: PinState>(&self) -> Result<(), RegseqError>;
}

impl OptionalPinCtrl for Option<PinCtrl> {
    #[inline(never)]
    #[rseq::intrinsic("pinctrl_apply")]
    fn apply<S: PinState>(&self) -> Result<(), RegseqError> {
        Ok(())
    }
}

#[rseq::pin_state(id = "default")]
pub struct DefaultState;
impl PinState for DefaultState {}

#[rseq::pin_state(id = "sleep")]
pub struct SleepState;
impl PinState for SleepState {}

// Plain-AMBA PL011. The match table carries BOTH the real PL011 primecell id
// (0x00041011) AND the reserved Tier-2 test-UART id (0x00abc123, mask 0x00ffffff) —
// the latter lets the generated driver bind the QEMU `ttyAMA1` that the harness
// reserves from stock amba-pl011 via a DT periphid override (the serial-pl011
// convention).
#[rseq::driver(name = "pl011pin", bus = "amba",
    amba = [
    { id = 0x00041011, mask = 0x000fffff },
    { id = 0x00abc123, mask = 0x00ffffff },
    ],
    compatible = ["regseq,pl011pin"]
)]
pub struct Pl011Pin {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Regs>,
    #[rseq::pinctrl(id = "uart_pins")]
    pinctrl: Option<PinCtrl>,
}

impl Pl011Pin {
    /// Mask all interrupt sources (bring-up/teardown quiesce).
    fn mask_all_interrupts(&mut self) {
        self.regs.imsc.write(0);
    }
    /// Clear all latched sources (W1C).
    fn clear_all_interrupts(&mut self) {
        self.regs.icr.write(ALL_UART_INTERRUPTS_MASK);
    }
}

impl Lifecycle for Pl011Pin {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // Optional-trait pinctrl apply: best-effort, lowers to
        // `(void)pinctrl_pm_select_default_state(dev)` on Linux (inert on QEMU virt,
        // which has no pin controller — returns 0 when dev->pins is NULL).
        self.pinctrl.apply::<DefaultState>()?;
        // A real probe: quiesce interrupts (one register write each), then announce.
        self.mask_all_interrupts();
        self.clear_all_interrupts();
        regseq::log::info!("pl011pin up");
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        regseq::log::warn!("pl011pin deinit: masking interrupts");
        self.mask_all_interrupts();
        self.clear_all_interrupts();
        Ok(())
    }
}
