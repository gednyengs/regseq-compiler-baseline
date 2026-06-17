#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// T04 negative (R10/C03/D05): `flow_auto_set` is a v1-DEFERRED member of the
// in-scope `SerialRuntimeLineConfig` trait (gated `flow_mechanisms`). Overriding
// it is frontend-rejected (SER-ERR-DEFERRED-MEMBER).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serflow", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serflow"])]
#[rseq::device_class(serial)]
pub struct SerFlow {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerFlow {
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

impl SerialRuntimeLineConfig for SerFlow {
    fn line_format_apply(&mut self, _f: LineFormat) -> ConfigResult {
        ConfigResult::Ok
    }
    // DEFERRED override — rejected.
    fn flow_auto_set(&mut self, _mechanism: FlowControl, _on: bool) {}
}

pub fn make() -> SerFlow {
    loop {}
}
