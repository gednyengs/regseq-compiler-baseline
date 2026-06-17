#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// O2 body-provenance negative (Codex hole): irq_cause_decode builds the CauseSet
// via a same-module STRUCT LITERAL `CauseSet { bits: 1<<7 }` (include! places the
// driver in the surface's module, so the private `bits` field is reachable),
// bypassing the from_bits anchor. The aggregate's bits field is sliced and the
// out-of-domain bit is rejected (SER-ERR-BITSET-PROVENANCE).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serbitlitstruct", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serbitlitstruct"])]
#[rseq::device_class(serial)]
pub struct SerBitLitStruct {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerBitLitStruct {
    fn port_service_set(&mut self, _scope: PortScope, _on: bool) -> PrevState { PrevState::new(false) }
    fn data_ready_query(&self, _dir: ReadyDir) -> bool { false }
    fn data_unit_write(&mut self, _unit: u32) {}
    fn data_unit_read(&mut self) -> RxDatum { RxDatum::new(0, RxError::empty()) }
}

impl SerialInterruptDriven for SerBitLitStruct {
    fn irq_source_set(&mut self, _source: SerialIrqSource, _state: IrqSourceState) {}
    fn irq_cause_decode(&mut self) -> CauseSet {
        CauseSet { bits: 1 << 7 }
    }
}

pub fn make() -> SerBitLitStruct { loop {} }
