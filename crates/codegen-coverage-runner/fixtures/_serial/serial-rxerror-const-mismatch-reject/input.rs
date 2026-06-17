#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// O2-hardening negative: the author `RX_ERR_PARITY` bit-position const is at the
// WRONG bit (1<<7, should be 1<<1 = the SerialRxErrorKind::Parity discriminant).
// The backends decode data_unit_read's inline RxError against the pinned
// RxErrorRepr convention, so this would silently misclassify parity errors —
// rejected at the frontend (SER-ERR-REPR-CONST).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

// The offending convention const (WRONG bit position).
pub const RX_ERR_PARITY: u32 = 1 << 7;

#[rseq::driver(name = "serrxerrconst", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serrxerrconst"])]
#[rseq::device_class(serial)]
pub struct SerRxErrConst {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerRxErrConst {
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

pub fn make() -> SerRxErrConst {
    loop {}
}
