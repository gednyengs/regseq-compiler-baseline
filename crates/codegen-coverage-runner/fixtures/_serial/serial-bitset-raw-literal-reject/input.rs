#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// O2 body-provenance negative: irq_cause_decode builds the CauseSet by OR-ing a
// RAW LITERAL `1 << 7` — a bit OUTSIDE the pinned CauseSetRepr bit-domain (0x1f)
// — which the named-const check (only `CAUSE_*` consts) cannot catch. The
// backend decodes the CauseSet against the pinned convention, so this would
// silently misclassify; the body-provenance slice rejects it
// (SER-ERR-BITSET-PROVENANCE).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serbitlit", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serbitlit"])]
#[rseq::device_class(serial)]
pub struct SerBitLit {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerBitLit {
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

impl SerialInterruptDriven for SerBitLit {
    fn irq_source_set(&mut self, _source: SerialIrqSource, _state: IrqSourceState) {}
    fn irq_cause_decode(&mut self) -> CauseSet {
        let mut bits = 0u32;
        bits |= 1 << 7; // FOREIGN: bit 7 is outside the convention bit-domain.
        CauseSet::from_bits(bits)
    }
}

pub fn make() -> SerBitLit {
    loop {}
}
