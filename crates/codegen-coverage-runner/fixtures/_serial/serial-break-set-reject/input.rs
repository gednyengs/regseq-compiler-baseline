#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// T04 negative (R10/C03/D10): `break_set` is a v1-DEFERRED member of the
// in-scope `SerialRuntimeLineConfig` trait (gated `break_supported`). Overriding
// it is frontend-rejected (SER-ERR-DEFERRED-MEMBER).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serbreak", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serbreak"])]
#[rseq::device_class(serial)]
pub struct SerBreak {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerBreak {
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

impl SerialRuntimeLineConfig for SerBreak {
    fn line_format_apply(&mut self, _f: LineFormat) -> ConfigResult {
        ConfigResult::Ok
    }
    // DEFERRED override — rejected.
    fn break_set(&mut self, _state: BreakState) {}
}

pub fn make() -> SerBreak {
    loop {}
}
