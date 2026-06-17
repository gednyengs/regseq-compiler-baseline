#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// O2 body-provenance negative (Codex hole): irq_cause_decode delegates the bitset
// construction to a HELPER that returns a CauseSet — a single-from_bits anchor
// would skip it. The repr-producing call in irq_cause_decode is not from_bits/
// empty, so it is rejected (SER-ERR-BITSET-PROVENANCE).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serbithelp", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serbithelp"])]
#[rseq::device_class(serial)]
pub struct SerBitHelp {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl SerBitHelp {
    fn bad_causes(&self) -> CauseSet {
        CauseSet::from_bits(1 << 7)
    }
}

impl Serial for SerBitHelp {
    fn port_service_set(&mut self, _scope: PortScope, _on: bool) -> PrevState { PrevState::new(false) }
    fn data_ready_query(&self, _dir: ReadyDir) -> bool { false }
    fn data_unit_write(&mut self, _unit: u32) {}
    fn data_unit_read(&mut self) -> RxDatum { RxDatum::new(0, RxError::empty()) }
}

impl SerialInterruptDriven for SerBitHelp {
    fn irq_source_set(&mut self, _source: SerialIrqSource, _state: IrqSourceState) {}
    fn irq_cause_decode(&mut self) -> CauseSet {
        self.bad_causes()
    }
}

pub fn make() -> SerBitHelp { loop {} }
