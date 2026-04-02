# Serial Device Class Specification

## Status

- task: `serial-device-class refinement`
- disposition: `compiler-facing synthesis complete`
- date: `2026-04-02`
- authority: `authoritative v1 Serial device-class spec for compiler work`

## Purpose

This document is the compiler-facing synthesis of the Serial/UART device-class
work. It closes the framework-first analysis and the first three challengers:

- Phase A framework analysis across Linux, Zephyr, Windows, FreeBSD, macOS
- PL011 challenger
- NS16550 / 8250 challenger
- STM32 challenger

This is the document the compiler team should use to implement the first stable
Serial device-class lowering.

## Inputs

- [current-baseline.md](/Users/nyengele/Documents/sekekama/regseq-taxonomy-work/research/construct-discovery-v3/track-a-handoff/serial-device-class/current-baseline.md)
- [serial-os-framework-baseline.md](/Users/nyengele/Documents/sekekama/regseq-taxonomy-work/research/construct-discovery-v3/track-a-handoff/serial-device-class/serial-os-framework-baseline.md)
- [serial-os-framework-matrix.md](/Users/nyengele/Documents/sekekama/regseq-taxonomy-work/research/construct-discovery-v3/track-a-handoff/serial-device-class/serial-os-framework-matrix.md)
- [pl011-challenge-findings.md](/Users/nyengele/Documents/sekekama/regseq-taxonomy-work/research/construct-discovery-v3/track-a-handoff/serial-device-class/device-challenges/pl011/pl011-challenge-findings.md)
- [ns16550-challenge-findings.md](/Users/nyengele/Documents/sekekama/regseq-taxonomy-work/research/construct-discovery-v3/track-a-handoff/serial-device-class/device-challenges/ns16550/ns16550-challenge-findings.md)
- [stm32-challenge-findings.md](/Users/nyengele/Documents/sekekama/regseq-taxonomy-work/research/construct-discovery-v3/track-a-handoff/serial-device-class/device-challenges/stm32/stm32-challenge-findings.md)
- [p1-01-serial-callback-conventions.md](/Users/nyengele/Documents/sekekama/regseq-taxonomy-work/research/construct-discovery-v3/track-a-handoff/pass1-notes/p1-01-serial-callback-conventions.md)

## Final decisions

1. `Serial` is the stable core device-class trait.
2. `SerialConsole` is the stable optional companion trait.
3. The stable v1 data-path surface remains byte-primitive:
   - `tx_byte`
   - `rx_char`
   - `tx_ready`
   - `tx_empty`
4. FIFO purge/reset stays in the model as explicit backend hooks:
   - `flush_rx_fifo`
   - `flush_tx_fifo`
5. Break control stays in the model as an explicit backend hook:
   - `send_break`
6. `SerialErrors` remains:
   - `overrun`
   - `parity`
   - `framing`
   - `break`
7. `noise` is a real audited hardware concern, but it is **not** part of the
   stable v1 `SerialErrors` surface yet.
8. Flow control, modem/control lines, RS-485, and DMA do **not** become
   separate stable core traits in v1.
9. Serial input/reference clocks are required when the hardware needs them,
   but they are sourced through the existing `Clock` construct and Layer 2
   bindings rather than a Serial-local duplicate property.

## 1. Layer 1 surface

### 1.1 Core trait family

```rust
trait Serial {
    /// Bring the serial port into service after device probe / initialization.
    fn port_open(&mut self) -> Result<(), RegseqError>;

    /// Take the port out of service before device teardown or file/session close.
    fn port_close(&mut self);

    /// Apply baud rate, framing, and flow-control configuration.
    fn configure(&mut self, config: &SerialConfig) -> Result<(), RegseqError>;

    /// Emit one byte to the transmit data path.
    ///
    /// Precondition: tx_ready() returned true.
    fn tx_byte(&mut self, byte: u8);

    /// Receive one character with inline per-character status.
    ///
    /// Returns None when no character is available.
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
```

### 1.2 Optional companion trait

```rust
trait SerialConsole {
    /// Write console bytes through the backend's polled / console-safe path.
    fn console_write(&mut self, bytes: &[u8]);
}
```

`SerialConsole` remains separate because:

- Linux, FreeBSD, Windows, and macOS all have a distinct console path
- Zephyr can still lower it by delegating to the polled path
- the compiler needs a place to hang console-specific lowering and registration

### 1.3 Supporting types

```rust
struct SerialConfig {
    baud_rate: u32,
    data_bits: DataBits,
    parity: Parity,
    stop_bits: StopBits,
    flow_control: FlowControl,
}

struct RxChar {
    byte: u8,
    errors: SerialErrors,
}

bitflags! {
    struct SerialErrors: u32 {
        const OVERRUN = 1 << 0;
        const PARITY  = 1 << 1;
        const FRAMING = 1 << 2;
        const BREAK   = 1 << 3;
    }
}

enum DataBits {
    Five,
    Six,
    Seven,
    Eight,
    Nine,
}

enum Parity {
    None,
    Even,
    Odd,
}

enum StopBits {
    One,
    Two,
}

enum FlowControl {
    None,
    RtsCts,
    DtrDsr,
}
```

### 1.4 Semantics

`tx_ready`
- means "the transmit path can accept another byte now"
- examples:
  - 8250 `THRE`
  - PL011 TX space / not-full
  - STM32 `TXE`

`tx_empty`
- means "transmission is fully complete, including bytes already shifted out"
- this is stronger than FIFO-space availability
- examples:
  - 8250 `TEMT`
  - PL011 `!BUSY`
  - STM32 `TC`

`rx_char`
- returns the next received byte plus inline per-character status
- backends that receive error and data information on separate channels must
  reconstruct `RxChar.errors`

`flush_rx_fifo` / `flush_tx_fifo`
- are explicit semantic hooks, not compiler inference from register metadata
- the device author provides the exact mechanism
- the compiler decides when to invoke them
- a device may implement them as:
  - real independent flush
  - combined purge behavior
  - no-op

`send_break`
- is a device-specific break-control hook
- the compiler/backends invoke it when the target OS/framework needs break
  transmission support
- devices may noop when break control is unsupported or unexercised

## 2. Layer 2 property catalog

The v1 Serial device class uses a mixed Layer 2 model:

- some properties are Serial-local
- some dependencies are satisfied through other constructs

### 2.1 Stable Serial-local Layer 2 properties

| Property | Status | Purpose |
|---|---|---|
| `has_fifo: bool` | stable | Whether the UART exposes FIFO buffering beyond a single holding register. |
| `fifo_depth: u32` | stable | FIFO capacity used by generated TX/RX loop sizing and heuristics. |
| `supports_flow_control: bool` | stable | Whether hardware-assisted flow control is available at all. |
| `supports_modem_signals: bool` | stable | Whether modem-status/control generation should be emitted. |
| `supports_break_control: bool` | stable | Whether break-control generation should be emitted. |
| `supports_console: bool` | stable | Whether `SerialConsole` lowering should be generated for this device/backend pairing. |

### 2.2 Stable external construct dependencies

These are required by serial lowering, but they are **not** modeled as
Serial-local properties.

| Dependency | Owner | Role in serial lowering |
|---|---|---|
| input/reference clock | `Clock` | Needed for divisor/baud programming. |
| register transport + access width/shift | Register Access + Bus Resource | Needed to read/write UART registers correctly. |
| interrupt line | Interrupt | Needed for interrupt-driven TX/RX lowering. |
| pin mux / pad configuration | PinCtrl | Needed when the UART depends on pinctrl selection. |
| reset line | ResetLine | Needed when the UART requires explicit reset sequencing. |
| DMA channels | DMA | Needed only when async/DMA serial lowering is emitted. |
| modem GPIO wiring | GPIO / Layer 2 binding | Needed when modem lines are routed through GPIO instead of native UART registers. |

### 2.3 Deferred Layer 2 property sets

These are real concerns, but they are not part of the stable core v1 surface.

| Property set | Status | Notes |
|---|---|---|
| RS-485 properties | deferred | DE polarity, assert/deassert timing, receive-during-transmit, addressing, hardware-native vs software-emulated direction switching. |
| DMA async-policy properties | deferred | Buffering/timeout policy remains backend-specific. |
| wide-data / 9-bit mode | deferred | STM32 shows it exists, but it is not promoted into the stable v1 surface. |
| noise detection support | deferred | STM32 proves the hardware concern exists, but it is not yet a stable `SerialErrors` bit. |

## 3. Compiler lowering contract

### 3.1 Core responsibilities

The compiler owns:

- OS framework registration glue
- resource acquisition / release joins
- TX/RX loop generation around byte primitives
- interrupt wrapper generation
- data/error reconstruction into `RxChar`
- modem-status propagation into the target OS framework
- `Clock` integration for baud/divisor programming
- console wrapper generation

The driver author owns:

- per-device register-level operations
- FIFO purge/reset mechanism
- break-control mechanism
- device-specific configuration logic inside `configure`

### 3.2 Lifecycle contract

The stable v1 sequencing contract is:

1. compiler acquires / joins Layer 2 resources and external constructs
2. device lifecycle init runs
3. `port_open()` brings the UART into service
4. `configure()` programs baud/framing/flow control
5. compiler connects interrupt, console, and optional DMA glue
6. data-plane operation begins
7. compiler uses `tx_empty()` for drain/wait semantics when required
8. compiler may invoke FIFO hooks or `send_break()` when the backend needs them
9. `port_close()` disables the session-level UART behavior
10. device lifecycle teardown runs

Exact backend-specific ordering around interrupt enablement, DMA start, or
console setup may vary, but the compiler must preserve these semantic roles.

### 3.3 Data-path lowering

The stable v1 lowering strategy is:

- `tx_byte` and `rx_char` are the only required device-authored data-plane
  primitives
- the compiler generates:
  - FIFO fill loops
  - FIFO drain loops
  - buffer management
  - wakeup / readiness integration
  - framework callback wrappers

This is an explicit design choice. v1 does **not** introduce a buffered
companion trait.

### 3.4 Error lowering

Backends must map hardware and framework status into:

- `OVERRUN`
- `PARITY`
- `FRAMING`
- `BREAK`

`noise`
- may be used internally by a backend
- may participate in recovery policy
- is **not** part of the stable v1 `SerialErrors` surface yet

### 3.5 Flow control lowering

Flow control stays:

- `SerialConfig.flow_control`
- `supports_flow_control`
- compiler/backend-managed policy

There is no stable v1 `SerialFlowControl` trait.

### 3.6 Modem/control lowering

Modem/control stays:

- `supports_modem_signals`
- optional GPIO / native-register Layer 2 bindings
- compiler/backend-generated state propagation

There is no stable v1 `SerialModemControl` trait.

### 3.7 Console lowering

`SerialConsole` lowers to a console-safe, polled path.

Backend requirements:

- preserve console-specific locking / context rules
- avoid reusing the normal interrupt-driven path directly when the target OS
  expects a distinct console registration or execution mode

`console_write(&[u8])` is the stable v1 surface because:

- it does not assume text encoding
- it matches the common compiler responsibility of writing an already-formed
  byte stream through the console-safe path

### 3.8 FIFO and break lowering

`flush_rx_fifo` / `flush_tx_fifo`
- compiler chooses when to call them
- device implementation decides how they work
- backends may ignore them when the platform never needs FIFO purge/reset

`send_break`
- compiler chooses when to call it
- devices/backends may noop it when unsupported or unexercised

## 4. Stable exclusions

The following are **not** part of the stable v1 Serial surface:

- buffered companion trait
- `noise` bit in `SerialErrors`
- `SerialFlowControl` trait
- `SerialModemControl` trait
- `SerialDma` trait
- RS-485 in the core `Serial` trait

These are deferred either because the evidence does not yet justify promotion
or because the current placement is sufficient for v1 implementation.

## 5. Important implementation notes

1. The compiler must not infer FIFO purge/reset sequences from register
   definitions alone.
2. The compiler must treat the `Clock` dependency as operational, not merely
   documentary.
3. The compiler must be prepared for:
   - native modem signals
   - GPIO-backed modem signals
   - flow-control support that is purely config/capability-driven
4. RS-485 remains an extension. Different devices lower it very differently:
   - software state machine
   - hardware-managed driver enable
   - no support
5. The absence of `noise` from the stable v1 `SerialErrors` surface does not
   mean the compiler should ignore hardware noise flags. It means those flags
   are backend-private until the public error surface is deliberately expanded.

## 6. Companion support matrix

The OS-facing support view for this spec is in:

- [serial-os-support-matrix.md](/Users/nyengele/Documents/sekekama/regseq-taxonomy-work/research/construct-discovery-v3/track-a-handoff/serial-device-class/serial-os-support-matrix.md)
