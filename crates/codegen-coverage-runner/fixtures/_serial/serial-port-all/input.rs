#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

include!("../../_lifecycle_surface.rs");
include!("../../_serial_port_surface.rs");

#[rseq::driver(name = "serial_port_all", compatible = ["regseq,serial-port-all"])]
#[rseq::device_class(serial)]
pub struct Port {
    state: u32,
    tx_byte: u8,
    rx_byte: u8,
}

impl SerialPort for Port {
    fn configure(&mut self, params: SerialParams) -> SerialStatus {
        self.state = params.baud_rate;
        SerialStatus::Ok
    }
    fn get_config(&self) -> SerialParams {
        SerialParams {
            baud_rate: self.state,
            data_bits: SerialDataBits::Eight,
            stop_bits: SerialStopBits::One,
            parity: SerialParity::None,
            flow_control_mode: SerialFlowControl::None,
        }
    }
    fn transmit(&mut self, buffer: SerialTxBuffer) -> usize { buffer.len }
    fn receive(&mut self, buffer: SerialRxBuffer) -> usize { buffer.len }
    fn set_signals(&mut self, signals: ModemSignalSet) -> SerialStatus {
        self.state = signals.rts as u32;
        SerialStatus::Ok
    }
    fn get_signals(&mut self) -> ModemSignalSet {
        ModemSignalSet {
            dtr: false, rts: self.state != 0, cts: true, dsr: false, dcd: false, ri: false,
            changed_cts: false, changed_dsr: false, changed_dcd: false, changed_ri: false,
        }
    }
    fn subscribe_events(&mut self, _mask: SerialEventMask) -> SerialStatus { SerialStatus::Ok }
    fn open(&mut self) -> SerialStatus { SerialStatus::Ok }
    fn close(&mut self) -> SerialStatus { SerialStatus::Ok }
    fn purge(&mut self, _direction: SerialDirection) -> SerialStatus {
        self.state = 1;
        SerialStatus::Ok
    }
    fn await_drain(&self) -> bool { true }
    fn abort(&mut self, _direction: SerialDirection) -> SerialStatus {
        if self.state == 1 { self.state = 2; }
        SerialStatus::Ok
    }
}

impl Lifecycle for Port {
    fn on_init(&mut self) -> Result<(), RegseqError> {
        let _ = self.open();
        let _ = self.configure(SerialParams {
            baud_rate: 115200,
            data_bits: SerialDataBits::Eight,
            stop_bits: SerialStopBits::One,
            parity: SerialParity::None,
            flow_control_mode: SerialFlowControl::None,
        });
        let tx = SerialTxBuffer {
            data: core::ptr::addr_of!(self.tx_byte),
            len: 3,
        };
        if self.transmit(tx) != 3 { return Err(RegseqError::HardwareError); }
        let mut rx_errors = LineErrorFlags {
            break_detected: false,
            framing: false,
            parity: false,
            overrun: false,
        };
        let rx = SerialRxBuffer {
            data: core::ptr::addr_of_mut!(self.rx_byte),
            len: 2,
            errors: core::ptr::addr_of_mut!(rx_errors),
        };
        if self.receive(rx) != 2 { return Err(RegseqError::HardwareError); }
        let _ = self.set_signals(ModemSignalSet {
            dtr: false, rts: true, cts: false, dsr: false, dcd: false, ri: false,
            changed_cts: false, changed_dsr: false, changed_dcd: false, changed_ri: false,
        });
        let _ = self.get_signals();
        let _ = self.subscribe_events(SerialEventMask {
            rx_data_ready: true,
            tx_progress_or_idle: false,
            line_error: true,
            signal_change: false,
        });
        let _ = self.get_config();
        let _ = self.await_drain();
        let _ = self.purge(SerialDirection::Both);
        let _ = self.abort(SerialDirection::Both);
        if self.state != 2 { return Err(RegseqError::HardwareError); }
        Ok(())
    }
    fn on_deinit(&mut self) -> Result<(), RegseqError> {
        let _ = self.close();
        Ok(())
    }
}
