#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The peripheral-clock-management COMPOSED-interaction fixture (clock construct,
// PLAN T17): one driver carrying ALL THREE preamble-ordered constructs — an MMIO
// region (`regs`), a MANAGED clock (`clk`), and an interrupt line (`irq`) with a
// bound ISR — to witness the C07 phase ordering end-to-end:
//   preamble : map_region -> acquire_clock -> interrupt-register
//   teardown : interrupt-deregister -> clock-release -> region-unmap (exact reverse).
// The generic-c oracle proves the runtime trace order; the linux_c/zephyr_c
// witnesses prove the same static placement (devm_ioremap -> devm_clk_get ->
// request_irq / DEVICE_MMIO_MAP -> device_is_ready -> IRQ_CONNECT).
//
// Surfaces: `_irq_pl011_surface.rs` supplies the full PL011 block (dr + the
// interrupt-path registers) + the register-access + log surface; the `Clock`
// facade is declared INLINE (the `_clock_surface.rs` copy minus its trimmed
// pl011/Mmio, which would collide with the irq surface).

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

// AMBA-matched PL011 carrying region + clock + interrupt. Field order is
// irrelevant to the preamble order (C07 batches by construct phase, not by
// field): map_region -> acquire_clock -> interrupt-register regardless.
#[rseq::driver(name = "pl011composed", bus = "amba",
    amba = [
    { id = 0x00041011, mask = 0x000fffff },
    ],
    compatible = ["regseq,pl011composed"]
)]
pub struct Pl011Composed {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Regs>,
    #[rseq::clock(id = "uartclk")]
    clk: Clock,
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Pl011Composed {
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

impl Lifecycle for Pl011Composed {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // The compiler-managed acquire_clock + interrupt-register already ran
        // (preamble). Enable the clock, read its rate, program DR from it, then
        // quiesce + enable the UART with RX on.
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
        // Author quiesce; the compiler-managed disable->deregister (interrupt)
        // and release (clock) run AFTER this callback, in exact-reverse order.
        self.mask_all_interrupts();
        self.regs.cr.write_field(pl011::cr::UARTEN_DISABLED);
        self.clear_all_interrupts();
        self.clk.disable();
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main`; the manifest's
// `lifecycle_scenario = "orderly"` drives the run.
