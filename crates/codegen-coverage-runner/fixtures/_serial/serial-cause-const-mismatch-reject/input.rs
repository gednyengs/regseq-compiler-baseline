#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// O2-hardening negative: the author `CAUSE_RX_DATA` bit-position const is at the
// WRONG bit (1<<5, should be 1<<1 = the SerialCause::RxData discriminant). The
// backends decode irq_cause_decode's CauseSet against the pinned CauseSetRepr
// convention, so this would silently misclassify rx_data — rejected at the
// frontend (SER-ERR-REPR-CONST).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

// The offending convention const (WRONG bit position).
pub const CAUSE_RX_DATA: u32 = 1 << 5;

#[rseq::driver(name = "sercauseconst", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,sercauseconst"])]
#[rseq::device_class(serial)]
pub struct SerCauseConst {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerCauseConst {
    fn port_service_set(&mut self, _scope: PortScope, _on: bool) -> PrevState {
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

impl SerialInterruptDriven for SerCauseConst {
    fn irq_source_set(&mut self, _source: SerialIrqSource, _state: IrqSourceState) {}
    fn irq_cause_decode(&mut self) -> CauseSet {
        CauseSet::empty()
    }
}

pub fn make() -> SerCauseConst {
    loop {}
}
