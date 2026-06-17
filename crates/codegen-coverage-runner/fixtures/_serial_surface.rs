// Serial device-class AUTHOR surface (construct #7) — shared across the
// `serial-*` single-file fixtures. The framework-pure 6-trait surface's v1
// in-scope subset (GRILL D02/D03): base `Serial` (C04),
// `SerialRuntimeLineConfig` with only `line_format_apply` (C03), and
// `SerialInterruptDriven` (C01).
//
// Recognition is SIGNATURE-BASED + NAME-VALIDATED (no `#[rseq::framework_type]`
// tags needed): the bridge recognizes the traits by name, reads the support
// types off the recognized method signatures, and validates the enum variant
// names against the closed vocab. The including `input.rs` pulls this in with
//   include!("../../_serial_surface.rs");
// alongside `_lifecycle_surface.rs`, `_interrupt_surface.rs`, and
// `_serial_pl011_surface.rs`.

// Bitset bit positions are plain module-level `u32` consts (the bridge's const
// translator handles scalar-int consts but not a single-field newtype struct
// whose layout collapses to a scalar — so `RxError`/`CauseSet` are only ever
// built via a body aggregate `from_bits(..)`, like `FieldValue::new`).

/// `RxError` bit: framing error.
pub const RX_ERR_FRAMING: u32 = 1 << 0;
/// `RxError` bit: parity error.
pub const RX_ERR_PARITY: u32 = 1 << 1;
/// `RxError` bit: hardware overrun.
pub const RX_ERR_HW_OVERRUN: u32 = 1 << 2;
/// `RxError` bit: software overrun.
pub const RX_ERR_SW_OVERRUN: u32 = 1 << 3;
/// `RxError` bit: break condition.
pub const RX_ERR_BREAK: u32 = 1 << 4;

/// `CauseSet` bit: TX path can accept more data.
pub const CAUSE_TX_READY: u32 = 1 << 0;
/// `CauseSet` bit: RX data available.
pub const CAUSE_RX_DATA: u32 = 1 << 1;
/// `CauseSet` bit: RX idle-timeout with residual data.
pub const CAUSE_RX_TIMEOUT: u32 = 1 << 2;
/// `CauseSet` bit: line error.
pub const CAUSE_LINE_ERROR: u32 = 1 << 3;
/// `CauseSet` bit: modem-status change (present-but-unarmed in v1).
pub const CAUSE_MODEM_DELTA: u32 = 1 << 4;

// -- base data-path support types (C04) --------------------------------------

/// Port-service scope for `Serial::port_service_set`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PortScope {
    /// Whole port (UART enable, both directions).
    Block,
    /// Transmit path only.
    Tx,
    /// Receive path only.
    Rx,
}

/// Readiness direction for `Serial::data_ready_query`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReadyDir {
    /// TX path can accept at least one more unit.
    TxSpace,
    /// RX path has at least one unit available.
    RxData,
}

/// Previous on/off state returned by `port_service_set`. Discarded by the
/// compiler at scatter sites in v1.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PrevState {
    /// Whether the scope was on before the call.
    pub was_on: bool,
}
impl PrevState {
    /// Construct a previous-state marker.
    pub const fn new(was_on: bool) -> Self {
        Self { was_on }
    }
}

/// Inline per-unit RX error vocabulary (I04):
/// `{framing, parity, hw_overrun, sw_overrun, break}`. Built via
/// [`RxError::from_bits`] using the `RX_ERR_*` bit consts.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RxError {
    bits: u32,
}
impl RxError {
    /// The empty error set.
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }
    /// Build from a raw bit set (the `RX_ERR_*` consts).
    pub const fn from_bits(bits: u32) -> Self {
        Self { bits }
    }
    /// Raw bits.
    pub const fn bits(self) -> u32 {
        self.bits
    }
}

/// One received data unit with inline per-unit errors (C04).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RxDatum {
    /// The received data unit (PL011 carries an 8-bit datum in the low byte).
    pub unit: u32,
    /// Inline per-unit error status.
    pub errors: RxError,
}
impl RxDatum {
    /// Construct an RX datum.
    pub const fn new(unit: u32, errors: RxError) -> Self {
        Self { unit, errors }
    }
}

// -- line-format support types (C03) -----------------------------------------

/// Data bits per frame.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DataBits {
    /// 5 bits.
    Five,
    /// 6 bits.
    Six,
    /// 7 bits.
    Seven,
    /// 8 bits.
    Eight,
    /// 9 bits (unsupported on PL011).
    Nine,
}

/// Parity mode.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    /// No parity.
    None,
    /// Even parity.
    Even,
    /// Odd parity.
    Odd,
}

/// Stop bits per frame.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StopBits {
    /// One stop bit.
    One,
    /// Two stop bits.
    Two,
}

/// Requested line format (C03). `line_format_apply` commits this; it does not
/// read back (I03).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LineFormat {
    /// Baud rate (bits/second).
    pub baud: u32,
    /// Data bits per frame.
    pub data_bits: DataBits,
    /// Parity mode.
    pub parity: Parity,
    /// Stop bits per frame.
    pub stop_bits: StopBits,
}

/// Result of committing a line format (C03). `Unsupported` is surfaced, never
/// silently defaulted (I03).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConfigResult {
    /// The requested format was committed.
    Ok,
    /// The requested format is unsupported by this device.
    Unsupported,
}

// -- interrupt-driven support types (C01) ------------------------------------

/// A portable interrupt source the compiler arms/disarms (C01/D03). Backends
/// couple `rx_timeout` into `RxData` (Zephyr Z11).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SerialIrqSource {
    /// TX-ready source.
    TxReady,
    /// RX-data source (couples rx_timeout).
    RxData,
    /// Line-error source.
    LineError,
}

/// Arm/disarm state for a `SerialIrqSource` (C01/D03).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IrqSourceState {
    /// Armed (unmasked).
    Armed,
    /// Disarmed (masked).
    Disarmed,
}

/// Decoded set of pending serviceable causes (C01). A bitset over
/// `{tx_ready, rx_data, rx_timeout, line_error, modem_delta}`; `modem_delta` is
/// present-but-unarmed in v1. Built via [`CauseSet::from_bits`] using the
/// `CAUSE_*` bit consts.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CauseSet {
    bits: u32,
}
impl CauseSet {
    /// The empty cause set.
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }
    /// Build from a raw bit set (the `CAUSE_*` consts).
    pub const fn from_bits(bits: u32) -> Self {
        Self { bits }
    }
    /// Raw bits.
    pub const fn bits(self) -> u32 {
        self.bits
    }
}

// -- the framework-pure serial traits (recognized by name) -------------------

/// Base serial data path (C04).
pub trait Serial {
    /// Enable/service the port at `scope`; returns the previous on-state.
    fn port_service_set(&mut self, scope: PortScope, on: bool) -> PrevState;
    /// True iff the path in `dir` is ready (TX space / RX data).
    fn data_ready_query(&self, dir: ReadyDir) -> bool;
    /// Emit one data unit to TX (precondition: TX space).
    fn data_unit_write(&mut self, unit: u32);
    /// Read one data unit with inline per-unit error status.
    fn data_unit_read(&mut self) -> RxDatum;
}

/// Runtime line-format control (C03); v1 in-scope: only `line_format_apply`.
/// The deferred sibling members (`line_format_readback`/`break_set`/
/// `flow_auto_set`, C03) are NOT declared here (they would emit dead types into
/// the generic-c oracle); their reject is exercised by the dedicated
/// `_serial_deferred_surface.rs` reject corpus (T04).
pub trait SerialRuntimeLineConfig {
    /// Attempt to commit the requested line format (procedural; does not read
    /// back, I03); returns `Unsupported` for an unrepresentable format.
    fn line_format_apply(&mut self, f: LineFormat) -> ConfigResult;
}

/// Interrupt-driven operation (C01) — the construct's confirmed contribution.
pub trait SerialInterruptDriven {
    /// BACKEND-CALLED per-source interrupt mask; the author owns the bits, the
    /// compiler owns WHEN. A dynamic author call is a frontend error (R07).
    fn irq_source_set(&mut self, source: SerialIrqSource, state: IrqSourceState);
    /// Decode pending serviceable causes (and W1C-ack line-error causes as part
    /// of decoding — no separate ack op).
    fn irq_cause_decode(&mut self) -> CauseSet;
}

/// Queue/drain control (C05); v1 in-scope: only `tx_drain_query` (T09 INCLUDE).
/// The deferred sibling `fifo_purge_hw` is NOT declared here; its reject is
/// exercised by `_serial_deferred_surface.rs` (T04).
pub trait SerialQueueControl {
    /// True iff the transmitter is fully drained (TX FIFO + shift register
    /// empty) — shift-register-inclusive completion. BACKEND-CALLED at the
    /// close/drain sites: lowers to Linux `uart_ops.tx_empty` and Zephyr
    /// `uart_driver_api.irq_tx_complete`.
    fn tx_drain_query(&self) -> bool;
}
