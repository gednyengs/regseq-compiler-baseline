#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// T04 negative (R10/C06): `SerialModemControl` is a v1-DEFERRED companion trait.
// Implementing it on a serial-class driver is frontend-rejected
// (SER-ERR-DEFERRED-TRAIT).

include!("../../_lifecycle_surface.rs");
include!("../../_interrupt_surface.rs");
include!("../../_serial_deferred_surface.rs");

#[rseq::driver(name = "sermodem", bus = "amba",
    amba = [{ id = 0x00041011, mask = 0x000fffff }],
    compatible = ["regseq,sermodem"])]
#[rseq::device_class(serial)]
pub struct SerModem {
    #[rseq::interrupt(id = "uart_irq")]
    irq: Interrupt,
}

impl Serial for SerModem {
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
impl SerialModemControl for SerModem {
    fn modem_outputs_set(&mut self, _out: ModemLines) {}
    fn modem_inputs_get(&mut self) -> ModemLines {
        ModemLines { bits: 0 }
    }
}

pub fn make() -> SerModem {
    loop {}
}
