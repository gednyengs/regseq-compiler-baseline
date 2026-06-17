#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// T04 negative (R10/C05/D09): `fifo_purge_hw` is a v1-DEFERRED member of the
// in-scope `SerialQueueControl` trait (T09 INCLUDE kept only `tx_drain_query`).
// Overriding it is frontend-rejected (SER-ERR-DEFERRED-MEMBER).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serpurge", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serpurge"])]
#[rseq::device_class(serial)]
pub struct SerPurge {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerPurge {
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

impl SerialQueueControl for SerPurge {
    fn tx_drain_query(&self) -> bool {
        true
    }
    // DEFERRED override — rejected.
    fn fifo_purge_hw(&mut self, _dir: PurgeDir) {}
}

pub fn make() -> SerPurge {
    loop {}
}
