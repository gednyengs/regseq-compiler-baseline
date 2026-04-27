use crate::regseq::RegseqError;
use core::ops::BitOr;

// ---------------------------------------------------------------------------
// SerialConfig support enums
// ---------------------------------------------------------------------------

/// Number of data bits per serial frame.
#[rseq::framework_type("data-bits")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataBits {
    Five,
    Six,
    Seven,
    Eight,
    Nine,
}

/// Parity mode for serial framing.
#[rseq::framework_type("parity")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    None,
    Even,
    Odd,
}

/// Number of stop bits per serial frame.
#[rseq::framework_type("stop-bits")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopBits {
    One,
    Two,
}

/// Hardware flow-control mode.
#[rseq::framework_type("flow-control")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowControl {
    None,
    RtsCts,
    DtrDsr,
}

// ---------------------------------------------------------------------------
// SerialConfig
// ---------------------------------------------------------------------------

/// Serial port configuration covering baud rate, framing, and flow control.
#[rseq::framework_type("serial-config")]
pub struct SerialConfig {
    pub baud_rate: u32,
    pub data_bits: DataBits,
    pub parity: Parity,
    pub stop_bits: StopBits,
    pub flow_control: FlowControl,
}

// ---------------------------------------------------------------------------
// SerialErrors (v1 error carrier)
// ---------------------------------------------------------------------------

/// UART-oriented received-character error carrier.
///
/// This is a small Regseq-owned semantic carrier rather than a device-local
/// raw bitfield. It is intentionally limited to the stable v1 UART RX error
/// set: overrun, parity, framing, and break.
#[rseq::framework_type("serial-errors")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SerialErrors {
    bits: u8,
}

impl SerialErrors {
    pub const NONE: Self = Self { bits: 0 };
    pub const OVERRUN: Self = Self { bits: 1 << 0 };
    pub const PARITY: Self = Self { bits: 1 << 1 };
    pub const FRAMING: Self = Self { bits: 1 << 2 };
    pub const BREAK: Self = Self { bits: 1 << 3 };

    pub const fn empty() -> Self {
        Self::NONE
    }

    pub const fn bits(self) -> u8 {
        self.bits
    }

    pub const fn contains(self, other: Self) -> bool {
        (self.bits & other.bits) == other.bits
    }

    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }
}

impl BitOr for SerialErrors {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self {
            bits: self.bits | rhs.bits,
        }
    }
}

// ---------------------------------------------------------------------------
// RxChar
// ---------------------------------------------------------------------------

/// Received UART character plus semantic RX error status.
#[rseq::framework_type("rx-char")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RxChar {
    byte: u8,
    errors: SerialErrors,
}

impl RxChar {
    pub const fn new(byte: u8, errors: SerialErrors) -> Self {
        Self { byte, errors }
    }

    pub const fn byte(self) -> u8 {
        self.byte
    }

    /// Return the per-character error status using the stable v1 name.
    pub const fn errors(self) -> SerialErrors {
        self.errors
    }

    /// Backward-compatible accessor for the per-character error status.
    pub const fn error(self) -> SerialErrors {
        self.errors
    }
}

// ---------------------------------------------------------------------------
// Serial trait (stable v1)
// ---------------------------------------------------------------------------

/// Accepted serial device-class callback contract.
///
/// A driver declaring `#[rseq::device_class(serial)]` must implement this
/// trait. Callback entry context is `S` (subsystem-serialized): the OS serial
/// framework holds its subsystem lock before invoking these callbacks.
pub trait Serial {
    /// Bring the serial port into service after device probe / initialization.
    fn port_open(&mut self) -> Result<(), RegseqError>;

    /// Take the port out of service before device teardown or file/session close.
    fn port_close(&mut self);

    /// Apply baud rate, framing, and flow-control configuration.
    fn configure(&mut self, config: &SerialConfig) -> Result<(), RegseqError>;

    /// Emit one byte to the transmit data path.
    ///
    /// Precondition: `tx_ready()` returned true.
    fn tx_byte(&mut self, byte: u8);

    /// Receive one character with inline per-character status.
    ///
    /// Returns `None` when no character is available.
    fn rx_char(&mut self) -> Option<RxChar>;

    /// True when the transmitter can accept at least one more byte.
    fn tx_ready(&mut self) -> bool;

    /// True when transmission is fully complete, including the shift register.
    fn tx_empty(&mut self) -> bool;

    /// Purge or reset the receive FIFO / queue state for this device.
    ///
    /// Devices that do not exercise this behavior may treat it as a no-op.
    fn flush_rx_fifo(&mut self);

    /// Purge or reset the transmit FIFO / queue state for this device.
    ///
    /// Devices that do not exercise this behavior may treat it as a no-op.
    fn flush_tx_fifo(&mut self);

    /// Enable or disable break transmission.
    ///
    /// Devices that do not support break control may treat it as a no-op.
    fn send_break(&mut self, on: bool);
}

// ---------------------------------------------------------------------------
// SerialConsole trait (stable v1)
// ---------------------------------------------------------------------------

/// Optional companion trait for serial console output.
///
/// `console_write` takes a byte slice rather than `&str` because it does not
/// assume text encoding and matches the common compiler responsibility of
/// writing an already-formed byte stream through the console-safe path.
pub trait SerialConsole {
    fn console_write(&mut self, bytes: &[u8]);
}
