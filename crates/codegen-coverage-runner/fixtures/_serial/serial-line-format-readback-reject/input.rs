#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// T04 negative (R10/C03/D04): `line_format_readback` is a v1-DEFERRED member of
// the in-scope `SerialRuntimeLineConfig` trait (gated `has_live_readback`).
// Overriding it reaches for a not-yet-built capability and is frontend-rejected
// (SER-ERR-DEFERRED-MEMBER).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "serreadback", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,serreadback"])]
#[rseq::device_class(serial)]
pub struct SerReadback {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerReadback {
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

impl SerialRuntimeLineConfig for SerReadback {
    fn line_format_apply(&mut self, _f: LineFormat) -> ConfigResult {
        ConfigResult::Ok
    }
    // DEFERRED override — rejected.
    fn line_format_readback(&mut self) -> LineFormat {
        LineFormat {
            baud: 115_200,
            data_bits: DataBits::Eight,
            parity: Parity::None,
            stop_bits: StopBits::One,
        }
    }
}

pub fn make() -> SerReadback {
    loop {}
}
