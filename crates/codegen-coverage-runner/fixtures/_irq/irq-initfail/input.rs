#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The device-interrupt init-failure variant (construct #6, PLAN T07): the
// same PL011-shaped interrupt spec whose `on_init` FAILS. The init-fail
// scenario's golden asserts the C01 Err branch: the line was registered
// DISABLED in the preamble, NO `IRQ enable` ever traces (R05/I01), and the
// reverse unwind includes the `IRQ unregister` (R16) after the rollback
// callback. No fire script — the placement is the witness.

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_irq_pl011_surface.rs");

#[rseq::driver(name = "pl011irqfail", bus = "amba",
    amba = [
    { id = 0x00041011, mask = 0x000fffff },
    ]
)]
pub struct Pl011IrqFail {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Regs>,
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Pl011IrqFail {
    /// The bound ISR (mandatory on the non-optional form, D13); never invoked
    /// in this scenario — the line is never enabled.
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

impl Lifecycle for Pl011IrqFail {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        // Quiesce, then fail bring-up: the device reports busy. The MMIO
        // writes BEFORE the Err prove the callback ran with the line already
        // registered (but disabled).
        regseq::log::debug!("pl011irqfail probe start");
        self.regs.imsc.write(0);
        self.regs.icr.write(ALL_UART_INTERRUPTS_MASK);
        regseq::log::error!("pl011irqfail device busy");
        Err(RegseqError::DeviceBusy)
    }

    fn on_init_failure(&mut self) -> Result<(), RegseqError> {
        // Partial-state cleanup; the compiler-managed unwind (IRQ unregister)
        // runs AFTER this callback (R16).
        regseq::log::warn!("pl011irqfail rollback");
        Ok(())
    }
}

// No `regseq_main`: the backend synthesizes `main`; the manifest selects the
// init-fail scenario.
