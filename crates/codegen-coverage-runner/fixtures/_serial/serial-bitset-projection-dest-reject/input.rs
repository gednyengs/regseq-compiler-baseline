#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// O2 body-provenance negative (Codex round-2 hole): RxError::from_bits writes
// into a PROJECTED destination (`d.errors = ...`), not a bare local — a scan
// keyed on `as_local()` would skip the call. Keying on the destination place
// TYPE (RxError) catches it; the out-of-domain bit is rejected
// (SER-ERR-BITSET-PROVENANCE).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serbitproj", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serbitproj"])]
#[rseq::device_class(serial)]
pub struct SerBitProj {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerBitProj {
    fn port_service_set(&mut self, _scope: PortScope, _on: bool) -> PrevState { PrevState::new(false) }
    fn data_ready_query(&self, _dir: ReadyDir) -> bool { false }
    fn data_unit_write(&mut self, _unit: u32) {}
    fn data_unit_read(&mut self) -> RxDatum {
        let mut d = RxDatum::new(0, RxError::empty());
        d.errors = RxError::from_bits(1 << 7); // projected call destination
        d
    }
}

pub fn make() -> SerBitProj { loop {} }
