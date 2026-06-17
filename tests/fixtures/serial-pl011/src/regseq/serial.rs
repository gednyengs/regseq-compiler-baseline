//! Public DSL facade — the framework-pure serial surface (construct #7).
//!
//! This is the **v1 in-scope subset** of the 6-trait framework-pure serial
//! surface (GRILL D02/D03): base `Serial` (C04), `SerialRuntimeLineConfig` with
//! only `line_format_apply` (C03), and `SerialInterruptDriven` (C01). The
//! construct's contribution is `irq_source_set` — a **backend-called** per-source
//! interrupt mask: the compiler emits calls with `(SerialIrqSource,
//! IrqSourceState)` enum literals at each per-OS site (WHEN), while the author
//! owns only the mask-register bits (the match arms, WHICH BITS).
//!
//! The deferred members/traits (`line_format_readback`, `break_set`,
//! `flow_auto_set`, `SerialQueueControl`, `SerialModemControl`,
//! `SerialBulkTransfer`, `SerialConsole`) are NOT part of this in-scope facade;
//! the frontend rejects any attempt to implement them (T04).

use core::ops::BitOr;

// ---------------------------------------------------------------------------
// Base data-path support types
// ---------------------------------------------------------------------------

/// Port-service scope for `Serial::port_service_set` (C04).
#[rseq::framework_type("serial-port-scope")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortScope {
    /// The whole port (UART enable, both directions).
    Block,
    /// The transmit path only.
    Tx,
    /// The receive path only.
    Rx,
}

/// Readiness direction for `Serial::data_ready_query` (C04).
#[rseq::framework_type("serial-ready-dir")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadyDir {
    /// TX path can accept at least one more unit.
    TxSpace,
    /// RX path has at least one unit available.
    RxData,
}

/// Previous on/off state returned by `port_service_set` (C04). Not a recognized
/// framework type — the compiler discards the return at scatter sites in v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrevState {
    /// Whether the scope was on before this call.
    pub was_on: bool,
}

impl PrevState {
    /// Construct a previous-state marker.
    pub const fn new(was_on: bool) -> Self {
        Self { was_on }
    }
}

/// Inline per-unit RX error vocabulary (C04 / I04). `error_kinds_reported =
/// {framing, parity, hw_overrun, sw_overrun, break}`.
#[rseq::framework_type("serial-rx-error")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RxError {
    bits: u32,
}

impl RxError {
    /// No error.
    pub const NONE: Self = Self { bits: 0 };
    /// Framing error.
    pub const FRAMING: Self = Self { bits: 1 << 0 };
    /// Parity error.
    pub const PARITY: Self = Self { bits: 1 << 1 };
    /// Hardware overrun.
    pub const HW_OVERRUN: Self = Self { bits: 1 << 2 };
    /// Software overrun.
    pub const SW_OVERRUN: Self = Self { bits: 1 << 3 };
    /// Break condition.
    pub const BREAK: Self = Self { bits: 1 << 4 };

    /// The empty error set.
    pub const fn empty() -> Self {
        Self::NONE
    }
    /// The raw bits.
    pub const fn bits(self) -> u32 {
        self.bits
    }
    /// Whether `other` is a subset.
    pub const fn contains(self, other: Self) -> bool {
        (self.bits & other.bits) == other.bits
    }
    /// Whether no error bits are set.
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }
}

impl BitOr for RxError {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self {
            bits: self.bits | rhs.bits,
        }
    }
}

/// One received data unit with inline per-unit errors (C04). The `unit` is
/// width-parameterized; PL011 carries an 8-bit datum in the low byte.
#[rseq::framework_type("serial-rx-datum")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RxDatum {
    /// The received data unit.
    pub unit: u32,
    /// The inline per-unit error status.
    pub errors: RxError,
}

impl RxDatum {
    /// Construct an RX datum.
    pub const fn new(unit: u32, errors: RxError) -> Self {
        Self { unit, errors }
    }
}

// ---------------------------------------------------------------------------
// Line-format support types (SerialRuntimeLineConfig::line_format_apply, C03)
// ---------------------------------------------------------------------------

/// Number of data bits per frame.
#[rseq::framework_type("serial-data-bits")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataBits {
    /// 5 data bits.
    Five,
    /// 6 data bits.
    Six,
    /// 7 data bits.
    Seven,
    /// 8 data bits.
    Eight,
    /// 9 data bits (unsupported on PL011 hardware).
    Nine,
}

/// Parity mode.
#[rseq::framework_type("serial-parity")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    /// No parity.
    None,
    /// Even parity.
    Even,
    /// Odd parity.
    Odd,
}

/// Number of stop bits.
#[rseq::framework_type("serial-stop-bits")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopBits {
    /// One stop bit.
    One,
    /// Two stop bits.
    Two,
}

/// Requested line format (C03). `line_format_apply` attempts to commit this;
/// it does NOT read back (I03).
#[rseq::framework_type("serial-line-format")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineFormat {
    /// Baud rate in bits/second.
    pub baud: u32,
    /// Data bits per frame.
    pub data_bits: DataBits,
    /// Parity mode.
    pub parity: Parity,
    /// Stop bits per frame.
    pub stop_bits: StopBits,
}

/// Result of committing a line format (C03). `Unsupported` must be surfaced, not
/// silently defaulted (I03).
#[rseq::framework_type("serial-config-result")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigResult {
    /// The requested format was committed.
    Ok,
    /// The requested format is not supported by this device.
    Unsupported,
}

// ---------------------------------------------------------------------------
// Interrupt-driven support types (SerialInterruptDriven, C01)
// ---------------------------------------------------------------------------

/// A portable interrupt source the compiler arms/disarms (C01/D03). Backends
/// couple `rx_timeout` into `RxData` (Zephyr Z11). `modem_delta` arming lives in
/// `SerialModemControl` (deferred), not here.
#[rseq::framework_type("serial-irq-source")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialIrqSource {
    /// TX-ready source.
    TxReady,
    /// RX-data source (couples rx_timeout).
    RxData,
    /// Line-error source (framing/parity/overrun/break).
    LineError,
}

/// Arm/disarm state for a `SerialIrqSource` (C01/D03).
#[rseq::framework_type("serial-irq-state")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrqSourceState {
    /// Source armed (unmasked).
    Armed,
    /// Source disarmed (masked).
    Disarmed,
}

/// The decoded set of pending serviceable causes (C01). A bitset over
/// `{tx_ready, rx_data, rx_timeout, line_error, modem_delta}`; `modem_delta` is
/// present-but-unarmed in v1.
#[rseq::framework_type("serial-cause-set")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CauseSet {
    bits: u32,
}

impl CauseSet {
    /// No causes pending.
    pub const NONE: Self = Self { bits: 0 };
    /// TX path can accept more data.
    pub const TX_READY: Self = Self { bits: 1 << 0 };
    /// RX data available.
    pub const RX_DATA: Self = Self { bits: 1 << 1 };
    /// RX idle-timeout with residual data.
    pub const RX_TIMEOUT: Self = Self { bits: 1 << 2 };
    /// Line error.
    pub const LINE_ERROR: Self = Self { bits: 1 << 3 };
    /// Modem-status change (present-but-unarmed in v1).
    pub const MODEM_DELTA: Self = Self { bits: 1 << 4 };

    /// The empty cause set.
    pub const fn empty() -> Self {
        Self::NONE
    }
    /// The raw bits.
    pub const fn bits(self) -> u32 {
        self.bits
    }
    /// Whether `other` is a subset.
    pub const fn contains(self, other: Self) -> bool {
        (self.bits & other.bits) == other.bits
    }
    /// Whether no cause bits are set.
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }
}

impl BitOr for CauseSet {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Self {
            bits: self.bits | rhs.bits,
        }
    }
}

// ---------------------------------------------------------------------------
// The framework-pure serial traits (recognized by name)
// ---------------------------------------------------------------------------

/// Base serial data path (C04) — always implemented by a serial-class driver.
/// Enable/service the port at a scope, test readiness, and move data units with
/// inline per-unit RX errors.
pub trait Serial {
    /// Enable/service the port at `scope`; returns the previous on-state.
    fn port_service_set(&mut self, scope: PortScope, on: bool) -> PrevState;
    /// True iff the path in `dir` is ready (TX space / RX data).
    fn data_ready_query(&self, dir: ReadyDir) -> bool;
    /// Emit one data unit to the transmit path. Precondition:
    /// `data_ready_query(TxSpace)` returned true.
    fn data_unit_write(&mut self, unit: u32);
    /// Read one data unit with inline per-unit error status.
    fn data_unit_read(&mut self) -> RxDatum;
}

/// Runtime line-format control (C03). v1 in-scope: only the procedural
/// `line_format_apply` (D04). The deferred members (`line_format_readback`,
/// `break_set`, `flow_auto_set`) are not part of this in-scope trait.
pub trait SerialRuntimeLineConfig {
    /// Attempt to commit the requested line format; returns `Unsupported` for an
    /// unrepresentable format. Procedural (baud→divisor + register encode); does
    /// not read back (I03).
    fn line_format_apply(&mut self, f: LineFormat) -> ConfigResult;
}

/// Interrupt-driven operation (C01) — the construct's confirmed contribution.
pub trait SerialInterruptDriven {
    /// **Backend-called** per-source interrupt mask. The compiler emits calls
    /// with `(SerialIrqSource, IrqSourceState)` enum literals at each per-OS
    /// site; the author owns only the mask-register bits (the match arms).
    /// A dynamic author call is a frontend error (R07).
    fn irq_source_set(&mut self, source: SerialIrqSource, state: IrqSourceState);
    /// Decode pending serviceable causes (and W1C-ack line-error causes as part
    /// of decoding — there is no separate ack op).
    fn irq_cause_decode(&mut self) -> CauseSet;
}
