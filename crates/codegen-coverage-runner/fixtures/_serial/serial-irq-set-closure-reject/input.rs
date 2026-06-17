#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// T04 negative — EVASION (R07): an author tries to dodge the gate by hiding the
// `irq_source_set` call inside a closure. The body-tree scan recurses into
// closures the method constructs, so it is still rejected
// (SER-ERR-DYNAMIC-IRQ-SET). Codex T04 review hardening.

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "sercls", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,sercls"])]
#[rseq::device_class(serial)]
pub struct SerCls {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerCls {
    fn port_service_set(&mut self, _scope: PortScope, _on: bool) -> PrevState {
        // EVASION: bury the backend-only call inside a closure.
        let mut arm = || {
            self.irq_source_set(SerialIrqSource::TxReady, IrqSourceState::Armed);
        };
        arm();
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

impl SerialInterruptDriven for SerCls {
    fn irq_source_set(&mut self, _source: SerialIrqSource, _state: IrqSourceState) {}
    fn irq_cause_decode(&mut self) -> CauseSet {
        CauseSet::empty()
    }
}

pub fn make() -> SerCls {
    loop {}
}
