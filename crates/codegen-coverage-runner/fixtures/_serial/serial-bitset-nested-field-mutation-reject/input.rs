#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// O2 body-provenance negative (Codex round-3 hole): a NESTED interior write
// `d.errors.bits = 1<<7` — the base local `d` is RxDatum (not the repr) but the
// projection prefix `d.errors` IS the RxError repr. Checking every projection
// prefix for repr type catches it; rejected (SER-ERR-BITSET-PROVENANCE).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serbitnested", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serbitnested"])]
#[rseq::device_class(serial)]
pub struct SerBitNested {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerBitNested {
    fn port_service_set(&mut self, _scope: PortScope, _on: bool) -> PrevState { PrevState::new(false) }
    fn data_ready_query(&self, _dir: ReadyDir) -> bool { false }
    fn data_unit_write(&mut self, _unit: u32) {}
    fn data_unit_read(&mut self) -> RxDatum {
        let mut d = RxDatum::new(0, RxError::empty());
        d.errors.bits = 1 << 7; // nested interior write: d.errors is the repr
        d
    }
}

pub fn make() -> SerBitNested { loop {} }
