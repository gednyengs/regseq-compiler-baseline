#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// T04 negative (R10/C07/D07): `SerialBulkTransfer` is a v1-DEFERRED companion
// trait (signature un-frozen until a DMA-capable evaluation device).
// Implementing it on a serial-class driver is frontend-rejected
// (SER-ERR-DEFERRED-TRAIT).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serbulk", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serbulk"])]
#[rseq::device_class(serial)]
pub struct SerBulk {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerBulk {
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

// DEFERRED companion trait — rejected.
impl SerialBulkTransfer for SerBulk {
    fn bulk_engine_ctl(&mut self, _on: bool) {}
    fn bulk_xfer(&mut self) {}
}

pub fn make() -> SerBulk {
    loop {}
}
