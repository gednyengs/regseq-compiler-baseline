#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// The device-interrupt L2 NEGATIVE (construct #6, the T04 reject witness): a
// valid interrupt spec whose platform file lacks the `[interrupt.uart_irq]`
// section. The bind stage MUST reject it with IRQ-L2-003 (C03: a non-Option
// `Interrupt` requires its section; `Option<Interrupt>` absence is the
// supported case, D06).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_irq_pl011_surface.rs");

#[rseq::driver(name = "pl011irqnosec", bus = "amba",
    amba = [
    { id = 0x00041011, mask = 0x000fffff },
    ]
)]
pub struct Pl011IrqNoSec {
    #[rseq::mmio(id = "regs")]
    regs: Mmio<pl011::Regs>,
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Pl011IrqNoSec {
    /// The mandatory handler (D13) — the fixture is frontend-valid; only the
    /// platform join is broken.
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

impl Lifecycle for Pl011IrqNoSec {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        self.regs.imsc.write(0);
        Ok(())
    }
}
