use crate::RegseqError;

/// Minimal opaque serial configuration type for stable callback signatures.
pub struct SerialConfig {
    _private: (),
}

/// Minimal opaque received-character type for stable callback signatures.
pub struct RxChar {
    _private: (),
}

/// Accepted serial device-class callback contract.
///
/// A driver declaring `#[rseq::device_class(serial)]` must implement this
/// trait. Callback entry context is `S` (subsystem-serialized): the OS serial
/// framework holds its subsystem lock before invoking these callbacks.
pub trait Serial {
    fn port_open(&mut self) -> Result<(), RegseqError>;
    fn port_close(&mut self);
    fn configure(&mut self, config: &SerialConfig) -> Result<(), RegseqError>;
    fn tx_byte(&mut self, byte: u8);
    fn rx_char(&mut self) -> Option<RxChar>;
    fn tx_ready(&mut self) -> bool;
}

/// Optional companion trait for serial console output.
pub trait SerialConsole {
    fn console_write(&mut self, text: &str);
}
