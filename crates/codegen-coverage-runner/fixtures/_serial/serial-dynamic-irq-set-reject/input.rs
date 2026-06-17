#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// T04 negative (R07/D03/I01): `irq_source_set` is BACKEND-CALLED only — the
// compiler emits its calls at the per-OS scatter sites (WHEN), the author owns
// only the mask bits (WHICH BITS). A dynamic author call breaks the portable
// placement contract and is frontend-rejected (SER-ERR-DYNAMIC-IRQ-SET). Here
// the base `port_service_set` illegally arms a source itself.

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serdynirq", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serdynirq"])]
#[rseq::device_class(serial)]
pub struct SerDynIrq {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerDynIrq {
    fn port_service_set(&mut self, _scope: PortScope, _on: bool) -> PrevState {
        // ILLEGAL: a dynamic author call to the backend-only op.
        self.irq_source_set(SerialIrqSource::TxReady, IrqSourceState::Armed);
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

impl SerialInterruptDriven for SerDynIrq {
    fn irq_source_set(&mut self, _source: SerialIrqSource, _state: IrqSourceState) {}
    fn irq_cause_decode(&mut self) -> CauseSet {
        CauseSet::empty()
    }
}

pub fn make() -> SerDynIrq {
    loop {}
}
