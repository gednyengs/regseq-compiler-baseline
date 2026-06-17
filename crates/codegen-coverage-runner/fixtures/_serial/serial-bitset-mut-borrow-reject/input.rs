#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// O2 body-provenance negative (Codex hole): the bitset accumulator is mutated
// through a `&mut` alias (`*p |= 1 << 7`) the bare-local slice cannot see. The
// traced local is mutably borrowed, so it is treated as foreign and rejected
// (SER-ERR-BITSET-PROVENANCE).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serbitborrow", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serbitborrow"])]
#[rseq::device_class(serial)]
pub struct SerBitBorrow {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerBitBorrow {
    fn port_service_set(&mut self, _scope: PortScope, _on: bool) -> PrevState { PrevState::new(false) }
    fn data_ready_query(&self, _dir: ReadyDir) -> bool { false }
    fn data_unit_write(&mut self, _unit: u32) {}
    fn data_unit_read(&mut self) -> RxDatum { RxDatum::new(0, RxError::empty()) }
}

impl SerialInterruptDriven for SerBitBorrow {
    fn irq_source_set(&mut self, _source: SerialIrqSource, _state: IrqSourceState) {}
    fn irq_cause_decode(&mut self) -> CauseSet {
        let mut bits = 0u32;
        let p = &mut bits;
        *p |= 1 << 7; // mutation through an alias the slice cannot follow
        CauseSet::from_bits(bits)
    }
}

pub fn make() -> SerBitBorrow { loop {} }
