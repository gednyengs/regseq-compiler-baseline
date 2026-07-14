// Locked device-class-serial author surface. One trait denotes one serial port;
// each override is an independently capability-gated operation.

#[derive(Clone, Copy)]
pub enum SerialDataBits { Five, Six, Seven, Eight, Nine }
#[derive(Clone, Copy)]
pub enum SerialStopBits { Half, One, OneAndHalf, Two }
#[derive(Clone, Copy)]
pub enum SerialParity { None, Odd, Even, Mark, Space }
#[derive(Clone, Copy)]
pub enum SerialFlowControl { None, RtsCts, DtrDsr, Rs485 }
#[derive(Clone, Copy)]
pub enum SerialDirection { Receive, Transmit, Both }
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SerialStatus {
    Ok,
    Unsupported,
    Busy,
    NoActiveTransfer,
    InvalidParameter,
    BufferTooSmall,
}

#[derive(Clone, Copy)]
pub struct SerialParams {
    pub baud_rate: u32,
    pub data_bits: SerialDataBits,
    pub stop_bits: SerialStopBits,
    pub parity: SerialParity,
    pub flow_control_mode: SerialFlowControl,
}

#[derive(Clone, Copy)]
pub struct ModemSignalSet {
    pub dtr: bool,
    pub rts: bool,
    pub cts: bool,
    pub dsr: bool,
    pub dcd: bool,
    pub ri: bool,
    pub changed_cts: bool,
    pub changed_dsr: bool,
    pub changed_dcd: bool,
    pub changed_ri: bool,
}

#[derive(Clone, Copy)]
pub struct LineErrorFlags {
    pub break_detected: bool,
    pub framing: bool,
    pub parity: bool,
    pub overrun: bool,
}
#[derive(Clone, Copy)]
pub struct SerialEventMask {
    pub rx_data_ready: bool,
    pub tx_progress_or_idle: bool,
    pub line_error: bool,
    pub signal_change: bool,
}
#[derive(Clone, Copy)]
pub struct SerialTxBuffer {
    pub data: *const u8,
    pub len: usize,
}
#[derive(Clone, Copy)]
pub struct SerialRxBuffer {
    pub data: *mut u8,
    pub len: usize,
    pub errors: *mut LineErrorFlags,
}

pub trait SerialPort {
    fn configure(&mut self, _params: SerialParams) -> SerialStatus { SerialStatus::Unsupported }
    fn get_config(&self) -> SerialParams {
        SerialParams {
            baud_rate: 0,
            data_bits: SerialDataBits::Eight,
            stop_bits: SerialStopBits::One,
            parity: SerialParity::None,
            flow_control_mode: SerialFlowControl::None,
        }
    }
    fn transmit(&mut self, _buffer: SerialTxBuffer) -> usize { 0 }
    fn receive(&mut self, _buffer: SerialRxBuffer) -> usize { 0 }
    fn set_signals(&mut self, _signals: ModemSignalSet) -> SerialStatus { SerialStatus::Unsupported }
    fn get_signals(&mut self) -> ModemSignalSet {
        ModemSignalSet {
            dtr: false, rts: false, cts: false, dsr: false, dcd: false, ri: false,
            changed_cts: false, changed_dsr: false, changed_dcd: false, changed_ri: false,
        }
    }
    fn subscribe_events(&mut self, _mask: SerialEventMask) -> SerialStatus { SerialStatus::Unsupported }
    fn open(&mut self) -> SerialStatus { SerialStatus::Unsupported }
    fn close(&mut self) -> SerialStatus { SerialStatus::Unsupported }
    fn purge(&mut self, _direction: SerialDirection) -> SerialStatus { SerialStatus::Unsupported }
    fn await_drain(&self) -> bool { false }
    fn abort(&mut self, _direction: SerialDirection) -> SerialStatus { SerialStatus::Unsupported }
}
