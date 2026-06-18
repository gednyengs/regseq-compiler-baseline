#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// M4/T14 — the PL011 ISR↔mainline seam as a BILATERAL `IrqSpinLock<IsrState>`.
//
// The bound `#[rseq::isr]` AND a mainline reconfigure path BOTH acquire the SAME
// `IrqSpinLock`, so the lock ORDERS the two contexts — the precise mechanism that
// discharges the device-interrupt Tier-2 pin (`pl011/TIER2-PIN-device-interrupt.md`):
// on Linux this lowers to `spin_lock_irqsave`, which disables local IRQs for the
// hold, so the ISR cannot preempt the mainline mid-critical-section. This fixture
// is the GENERATED locked PL011 driver in miniature — the construct-side artifact
// the empirical SMP campaign would run.
//
// `IsrState { im, dmacr }` is the ISR-shared device-state shadow (the im-mask +
// dmacr shadow). It is OPAQUE this slice (the guard has no `Deref`), so the lock
// provides the critical-section *ordering*; payload access THROUGH the guard is
// the deferred Deref slice. The in-guard `imsc` write in the mainline path is the
// Open-2 register-placement residual made concrete (a register touched while the
// lock is held — covered by the lock's ordering, not by a separate static check).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_irq_pl011_surface.rs");
include!("../../_sync_surface.rs");

/// The ISR-shared device state: the im-mask shadow + the dmacr shadow. Opaque
/// behind the guard this slice (no payload access yet).
pub struct IsrState {
    pub im: u32,
    pub dmacr: u32,
}

#[rseq::driver(name = "sync-pl011-seam", bus = "amba",
    amba = [
    { id = 0x00041011, mask = 0x000fffff },
    ],
    compatible = ["regseq,sync-pl011-seam"]
)]
pub struct Pl011Seam {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Regs>,
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
    // The ISR↔mainline seam (the corpus witness kind). BOTH contexts below
    // acquire THIS lock — that is the whole point of the seam.
    isr_state: IrqSpinLock<IsrState>,
}

impl Pl011Seam {
    /// The bound ISR: acquires the ISR↔mainline `IrqSpinLock` EXPLICITLY (legal —
    /// IrqSpinLock is the ISR-safe kind, §7.1), reads MIS under the lock, W1C-acks
    /// via ICR. The guard releases on every exit (early `NotHandled` + `Handled`).
    #[rseq::isr]
    pub fn handle_irq(&mut self) -> IrqResult {
        let _g = self.isr_state.lock();
        let status = self.regs.mis.read();
        if status == 0 {
            return IrqResult::NotHandled;
        }
        self.regs.icr.write(status);
        IrqResult::Handled
    }

    /// A MAINLINE toucher of the same shared state — acquires the SAME lock, so
    /// the ISR cannot preempt it mid-section. The in-guard `imsc` write is the
    /// Open-2 register-placement scenario (a shared register touched while held).
    fn mainline_reconfigure(&mut self) {
        let _g = self.isr_state.lock();
        self.regs.imsc.write(0);
    }
}

impl Lifecycle for Pl011Seam {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        self.mainline_reconfigure();
        self.regs
            .cr
            .write_field(pl011::cr::UARTEN_ENABLED + pl011::cr::RXE_ENABLED);
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        self.mainline_reconfigure();
        self.regs.cr.write_field(pl011::cr::UARTEN_DISABLED);
        Ok(())
    }
}
