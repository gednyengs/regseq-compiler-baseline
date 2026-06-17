#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// T04 negative — EVASION (R07): an author tries to dodge the direct-call gate by
// reifying `irq_source_set` as a fn pointer and calling it indirectly. The
// `FnDef` ZST still appears as an operand, so the operand-level scan rejects it
// (SER-ERR-DYNAMIC-IRQ-SET). Codex T04 review hardening.

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serfnptr", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serfnptr"])]
#[rseq::device_class(serial)]
pub struct SerFnptr {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerFnptr {
    fn port_service_set(&mut self, _scope: PortScope, _on: bool) -> PrevState {
        // EVASION: reify the backend-only op as a fn pointer, then call it.
        let f: fn(&mut Self, SerialIrqSource, IrqSourceState) =
            <Self as SerialInterruptDriven>::irq_source_set;
        f(self, SerialIrqSource::TxReady, IrqSourceState::Armed);
        PrevState::new(false)
    }
    fn data_ready_query(&self, _dir: ReadyDir) -> bool {
        false
    }
    fn data_unit_write(&mut self, _unit: u32) {}
    fn data_unit_read(&mut self) -> RxDatum {
        RxDatum::new(0, RxError::empty())
    }
}

impl SerialInterruptDriven for SerFnptr {
    fn irq_source_set(&mut self, _source: SerialIrqSource, _state: IrqSourceState) {}
    fn irq_cause_decode(&mut self) -> CauseSet {
        CauseSet::empty()
    }
}

pub fn make() -> SerFnptr {
    loop {}
}
