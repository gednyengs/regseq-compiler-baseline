#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// Combined interrupt + synchronization fixture (T09 / G-SYN-17, and the M4
// PL011 ISR↔mainline seed): a PL011-shaped AMBA driver with ONE bound
// `#[rseq::isr]` AND ONE `IrqSpinLock` the handler acquires EXPLICITLY. The
// point T09 proves: `subsystem_locking` is a NULL surface — the framework ISR
// wrapper does NOT auto-acquire on the handler's behalf (G-SYN-17). The
// explicit `lock()` lowers into the HANDLER body (spin_lock_irqsave); the
// generated wrapper (`rseq_isr_*`) only dispatches. IrqSpinLock is the ISR-safe
// kind (§7.1), so acquiring it in IRQ context is legal.

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_irq_pl011_surface.rs");
include!("../../_sync_surface.rs");

#[rseq::driver(name = "sync-irq", bus = "amba",
    amba = [
    { id = 0x00041011, mask = 0x000fffff },
    ],
    compatible = ["regseq,sync-irq"]
)]
pub struct SyncIrq {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Regs>,
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
    // The ISR↔mainline lock (the corpus witness kind). Opaque payload this slice
    // (no Deref); it guards the critical section, not a dereferenced `T`.
    lock: IrqSpinLock<u32>,
}

impl SyncIrq {
    fn mask_all_interrupts(&mut self) {
        self.regs.imsc.write(0);
    }
    fn clear_all_interrupts(&mut self) {
        self.regs.icr.write(ALL_UART_INTERRUPTS_MASK);
    }

    /// The bound ISR (C04): it acquires the ISR↔mainline `IrqSpinLock`
    /// EXPLICITLY — the framework wrapper performs NO auto-acquire on its behalf
    /// (G-SYN-17 / `subsystem_locking` null surface). The guard releases on every
    /// exit (both the early `NotHandled` return and the normal `Handled`).
    #[rseq::isr]
    pub fn handle_irq(&mut self) -> IrqResult {
        let _g = self.lock.lock();
        let status = self.regs.mis.read();
        if status == 0 {
            return IrqResult::NotHandled;
        }
        self.regs.icr.write(status);
        IrqResult::Handled
    }
}

impl Lifecycle for SyncIrq {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        self.mask_all_interrupts();
        self.clear_all_interrupts();
        self.regs
            .cr
            .write_field(pl011::cr::UARTEN_ENABLED + pl011::cr::RXE_ENABLED);
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        self.mask_all_interrupts();
        self.regs.cr.write_field(pl011::cr::UARTEN_DISABLED);
        self.clear_all_interrupts();
        Ok(())
    }
}
