#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The device-interrupt M1 vertical-slice fixture (construct #6, PLAN T07):
// the smallest valid interrupt spec on a PL011-shaped driver — construct-#1
// registers + ONE `Interrupt` + ONE bare `#[rseq::isr]` reading MIS and
// acking via ICR. The manifest's fire script exercises the four C01 lifecycle
// points: (a) pre-init fire → inert (registered, line DISABLED — D02);
// (b) post-init fire with sources masked → `IRQ result=NotHandled` (the
// handler is total over the quiescent device, R08); (c) cause poke
// (MIS=0x010, the RX source) + fire → `IRQ result=Handled` with the ICR W1C
// ack visible in the MMIO trace (I03); (d) post-deinit fire → inert
// (unregistered). `expected.stdout` asserts the FULL C01 interleaving —
// the trace ORDER is the placement contract made observable (I07/R14).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_irq_pl011_surface.rs");

/// The RX interrupt source bit (MIS/ICR bit 4) — the cause the harness pokes.
const RXIS: u32 = 1 << 4;

// AMBA-matched PL011 with the bus-managed MMIO region joined to
// `[mmio.regs]` and the interrupt line joined to `[interrupt.uart_irq]`
// (QEMU virt: SPI 33 → INTID 37).
#[rseq::driver(name = "pl011irq", bus = "amba",
    amba = [
    { id = 0x00041011, mask = 0x000fffff },
    ]
)]
pub struct Pl011Irq {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Regs>,
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Pl011Irq {
    /// Mask all interrupt sources (bring-up/teardown quiesce).
    fn mask_all_interrupts(&mut self) {
        self.regs.imsc.write(0);
    }

    /// Clear all latched sources; W1C semantics.
    fn clear_all_interrupts(&mut self) {
        self.regs.icr.write(ALL_UART_INTERRUPTS_MASK);
    }

    /// The bound ISR (C04): read the masked status, ack what fired. Total
    /// over the quiescent device — MIS=0 returns `NotHandled` verbatim (R08);
    /// a latched cause is acked through the ordinary register-access W1C
    /// write (I03: device-source ops stay MMIO traffic) and returns
    /// `Handled`. No `DeferWork` (emit-rejected this slice, R09).
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

impl Lifecycle for Pl011Irq {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        regseq::log::debug!("pl011irq probe start");
        // Quiesce-then-enable: mask + clear every source, then enable the
        // UART with RX on. The line itself is compiler-managed: registered
        // (disabled) BEFORE this callback, enabled only after its Ok (D02).
        self.mask_all_interrupts();
        self.clear_all_interrupts();
        self.regs
            .cr
            .write_field(pl011::cr::UARTEN_ENABLED + pl011::cr::RXE_ENABLED);
        regseq::log::info!("pl011irq up");
        Ok(())
    }

    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        // Mask + quiesce + clear; the compiler-managed disable→deregister
        // runs AFTER this callback (D09).
        regseq::log::warn!("pl011irq deinit: masking interrupts");
        self.mask_all_interrupts();
        self.regs.cr.write_field(pl011::cr::UARTEN_DISABLED);
        self.clear_all_interrupts();
        Ok(())
    }
}

// No `regseq_main`: the generic-c backend synthesizes `main`; the manifest's
// `lifecycle_scenario = "orderly"` + `irq_fire` script drive the run.
