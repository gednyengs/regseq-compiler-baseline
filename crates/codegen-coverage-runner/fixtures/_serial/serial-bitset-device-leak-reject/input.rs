#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// O2 body-provenance negative: data_unit_read hands a NON-CONSTANT value (a
// struct field — standing in for a masked device read) to RxError::from_bits,
// rather than a pure OR of convention constants. Raw/computed device bits would
// leak into the inline error and the backend would misclassify; the
// body-provenance slice rejects the non-constant contribution
// (SER-ERR-BITSET-PROVENANCE).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serbitleak", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serbitleak"])]
#[rseq::device_class(serial)]
pub struct SerBitLeak {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
    /// A non-constant the body leaks into the error bitset (device-read analog).
    leak: u32,
}

impl Serial for SerBitLeak {
    fn port_service_set(&mut self, _scope: PortScope, _on: bool) -> PrevState {
        PrevState::new(false)
    }
    fn data_ready_query(&self, _dir: ReadyDir) -> bool {
        false
    }
    fn data_unit_write(&mut self, _unit: u32) {}
    fn data_unit_read(&mut self) -> RxDatum {
        // FOREIGN: a non-constant (the device-leak analog) reaches from_bits.
        RxDatum::new(0, RxError::from_bits(self.leak))
    }
}

pub fn make() -> SerBitLeak {
    loop {}
}
