// Serial device-class DEFERRED/FULL author surface — used ONLY by the T04
// negative-reject corpus (`_serial/serial-*-reject/`). It declares the SAME
// in-scope traits as `_serial_surface.rs` PLUS the v1-DEFERRED capabilities:
//   * the deferred MEMBERS inside the in-scope traits
//     (`SerialRuntimeLineConfig::{line_format_readback,break_set,flow_auto_set}`,
//     `SerialQueueControl::fifo_purge_hw`) — declared with DEFAULT bodies so a
//     reject fixture overriding exactly ONE of them stays a complete trait impl;
//   * the deferred COMPANION traits (`SerialModemControl`, `SerialConsole`,
//     `SerialBulkTransfer`).
// The frontend REJECTS any impl/override of these (T04 reject matrix:
// SER-ERR-DEFERRED-MEMBER / SER-ERR-DEFERRED-TRAIT), so a reject fixture never
// reaches emit — hence this surface is deliberately kept OUT of the in-scope
// `_serial_surface.rs` (which the generic-c oracle golden compiles), avoiding
// dead deferred-type emission there.
//
// Self-contained (it is the ONLY serial surface a reject fixture includes; it
// must not be combined with `_serial_surface.rs` — the trait names would clash).

// -- base data-path support types (C04) --------------------------------------

/// Port-service scope for `Serial::port_service_set`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PortScope {
    /// Whole port.
    Block,
    /// Transmit path only.
    Tx,
    /// Receive path only.
    Rx,
}

/// Readiness direction for `Serial::data_ready_query`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ReadyDir {
    /// TX path can accept a unit.
    TxSpace,
    /// RX path has a unit.
    RxData,
}

/// Previous on/off state returned by `port_service_set`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PrevState {
    /// Whether the scope was on before.
    pub was_on: bool,
}
impl PrevState {
    /// Construct a previous-state marker.
    pub const fn new(was_on: bool) -> Self {
        Self { was_on }
    }
}

/// Inline per-unit RX error vocabulary (I04).
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
}

/// One received data unit with inline per-unit errors (C04).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RxDatum {
    /// The received data unit.
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
    /// 9 bits.
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

/// Requested line format (C03).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LineFormat {
    /// Baud rate.
    pub baud: u32,
    /// Data bits.
    pub data_bits: DataBits,
    /// Parity.
    pub parity: Parity,
    /// Stop bits.
    pub stop_bits: StopBits,
}

/// Result of committing a line format (C03).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConfigResult {
    /// Committed.
    Ok,
    /// Unsupported.
    Unsupported,
}

// -- interrupt-driven support types (C01) ------------------------------------

/// A portable interrupt source (C01/D03).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SerialIrqSource {
    /// TX-ready source.
    TxReady,
    /// RX-data source.
    RxData,
    /// Line-error source.
    LineError,
}

/// Arm/disarm state (C01/D03).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IrqSourceState {
    /// Armed.
    Armed,
    /// Disarmed.
    Disarmed,
}

/// Decoded set of pending serviceable causes (C01).
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
}

// -- v1-DEFERRED support types (C03/C05/C06/C08) -----------------------------

/// DEFERRED line-break state (C03/D10).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BreakState {
    /// Drive the break condition.
    Set,
    /// Release the break condition.
    Clear,
}

/// DEFERRED auto-flow-control mechanism (C03/D05).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FlowControl {
    /// No auto-flow control.
    None,
    /// RTS/CTS hardware flow control.
    RtsCts,
    /// XON/XOFF software flow control.
    XonXoff,
    /// DTR/DSR hardware flow control.
    DtrCts,
    /// RS-485 direction control.
    Rs485,
}

/// DEFERRED hardware FIFO purge direction (C05/D09).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PurgeDir {
    /// Purge RX.
    Rx,
    /// Purge TX.
    Tx,
    /// Purge both.
    RxTx,
}

/// DEFERRED modem-line bitset (C06).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ModemLines {
    /// Raw TIOCM-shaped bits.
    pub bits: u32,
}

/// DEFERRED console execution-context mode (C08/D08).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConsoleMode {
    /// Atomic context.
    Atomic,
    /// Threaded context.
    Threaded,
    /// Polled context.
    Polled,
}

// -- the framework-pure serial traits (recognized by name) -------------------

/// Base serial data path (C04).
pub trait Serial {
    /// Enable/service the port at `scope`.
    fn port_service_set(&mut self, scope: PortScope, on: bool) -> PrevState;
    /// True iff the path in `dir` is ready.
    fn data_ready_query(&self, dir: ReadyDir) -> bool;
    /// Emit one data unit to TX.
    fn data_unit_write(&mut self, unit: u32);
    /// Read one data unit with inline error status.
    fn data_unit_read(&mut self) -> RxDatum;
}

/// Runtime line-format control (C03). The deferred members carry DEFAULT bodies
/// so a reject fixture overriding exactly one stays a complete impl; any
/// override is frontend-rejected (T04, SER-ERR-DEFERRED-MEMBER).
pub trait SerialRuntimeLineConfig {
    /// In-scope: attempt to commit the requested line format.
    fn line_format_apply(&mut self, f: LineFormat) -> ConfigResult;
    /// DEFERRED (D04): report the observable live line format.
    fn line_format_readback(&mut self) -> LineFormat {
        LineFormat {
            baud: 0,
            data_bits: DataBits::Eight,
            parity: Parity::None,
            stop_bits: StopBits::One,
        }
    }
    /// DEFERRED (D10): drive the TX line break state.
    fn break_set(&mut self, _state: BreakState) {}
    /// DEFERRED (D05): enable/disable an auto-flow mechanism.
    fn flow_auto_set(&mut self, _mechanism: FlowControl, _on: bool) {}
}

/// Interrupt-driven operation (C01).
pub trait SerialInterruptDriven {
    /// BACKEND-CALLED per-source interrupt mask (R07: no dynamic author call).
    fn irq_source_set(&mut self, source: SerialIrqSource, state: IrqSourceState);
    /// Decode pending serviceable causes.
    fn irq_cause_decode(&mut self) -> CauseSet;
}

/// Queue/drain control (C05). `fifo_purge_hw` carries a DEFAULT body; any
/// override is frontend-rejected (T04, SER-ERR-DEFERRED-MEMBER).
pub trait SerialQueueControl {
    /// In-scope: shift-register-inclusive TX-drain completion.
    fn tx_drain_query(&self) -> bool;
    /// DEFERRED (C05/D09): hardware FIFO purge.
    fn fifo_purge_hw(&mut self, _dir: PurgeDir) {}
}

// -- v1-DEFERRED companion traits (C06/C07/C08) ------------------------------
// Any impl on a serial-class driver is frontend-rejected (SER-ERR-DEFERRED-TRAIT).

/// DEFERRED modem-control plane (C06).
pub trait SerialModemControl {
    /// Drive the modem output lines.
    fn modem_outputs_set(&mut self, out: ModemLines);
    /// Read the modem input lines.
    fn modem_inputs_get(&mut self) -> ModemLines;
}

/// DEFERRED console companion (C08).
pub trait SerialConsole {
    /// Report the console execution-context contract.
    fn console_mode(&self) -> ConsoleMode;
}

/// DEFERRED bulk/DMA transfer plane (C07; un-frozen, D07).
pub trait SerialBulkTransfer {
    /// Bind/unbind the bulk engine.
    fn bulk_engine_ctl(&mut self, on: bool);
    /// Submit a bulk transfer.
    fn bulk_xfer(&mut self);
}
