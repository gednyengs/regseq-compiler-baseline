#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// O2 body-provenance negative (Codex round-2 hole): a CauseSet is constructed
// clean via empty(), then its bits field is MUTATED IN PLACE (`c.bits = 1<<7`) —
// a projected destination a bare-local scan would skip. Interior mutation of a
// repr is non-canonical and rejected (SER-ERR-BITSET-PROVENANCE).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serbitfield", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serbitfield"])]
#[rseq::device_class(serial)]
pub struct SerBitField {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerBitField {
    fn port_service_set(&mut self, _scope: PortScope, _on: bool) -> PrevState { PrevState::new(false) }
    fn data_ready_query(&self, _dir: ReadyDir) -> bool { false }
    fn data_unit_write(&mut self, _unit: u32) {}
    fn data_unit_read(&mut self) -> RxDatum { RxDatum::new(0, RxError::empty()) }
}

impl SerialInterruptDriven for SerBitField {
    fn irq_source_set(&mut self, _source: SerialIrqSource, _state: IrqSourceState) {}
    fn irq_cause_decode(&mut self) -> CauseSet {
        let mut c = CauseSet::empty();
        c.bits = 1 << 7; // interior mutation of an accepted repr
        c
    }
}

pub fn make() -> SerBitField { loop {} }
