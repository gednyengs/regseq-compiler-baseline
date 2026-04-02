# Serial/UART Management

> Status note:
> This document remains the broad construct-level baseline for accepted
> Serial/UART compiler scope. Compiler-facing implementation decisions for the
> Serial device class are now refined in
> [serial-device-class-spec.md](/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/discussions/device-class-serial/serial-device-class-spec.md).
> Where this construct doc and the device-class serial spec differ, the
> device-class serial spec is authoritative for implementation.

## Purpose

`Serial/UART Management` is the accepted device-class construct for UART
 drivers. In compiler terms, it selects the serial subsystem surface, joins
 deployment-specific serial properties from Layer 2, validates the serial
 callback trait, and emits serial-subsystem registration glue.

## Accepted Primitive Inventory

### Layer 1

| Primitive | Kind | Stable surface |
|---|---|---|
| `serial_uart.port_open` | operation | `Serial::port_open(&mut self) -> Result<(), RegseqError>` |
| `serial_uart.port_close` | operation | `Serial::port_close(&mut self)` |
| `serial_uart.configure` | operation | `Serial::configure(&mut self, config: &SerialConfig) -> Result<(), RegseqError>` |
| `serial_uart.tx_byte` | operation | `Serial::tx_byte(&mut self, byte: u8)` |
| `serial_uart.rx_char` | operation | `Serial::rx_char(&mut self) -> Option<RxChar>` |
| `serial_uart.tx_ready` | operation | `Serial::tx_ready(&mut self) -> bool` |
| `serial_uart.console_write` | operation | `SerialConsole::console_write(&mut self, text: &str)` |
| `serial_uart.push_rx_data` | operation | intrinsic |

### Layer 2

| Primitive | Kind | Stable surface |
|---|---|---|
| `serial_uart.default_baud_rate` | property | hardware binding field |
| `serial_uart.fifo_depth` | property | hardware binding field |
| `serial_uart.supports_flow_control` | property | hardware binding field |
| `serial_uart.supports_modem_signals` | property | hardware binding field |

### Compiler-managed

| Primitive | Role |
|---|---|
| `serial_uart.register_driver` | emit serial-subsystem registration/wiring |

## Accepted Surface Baseline

### Trait baseline

Recommended device-class traits:

```rust
trait Serial {
    fn port_open(&mut self) -> Result<(), RegseqError>;
    fn port_close(&mut self);
    fn configure(&mut self, config: &SerialConfig) -> Result<(), RegseqError>;
    fn tx_byte(&mut self, byte: u8);
    fn rx_char(&mut self) -> Option<RxChar>;
    fn tx_ready(&mut self) -> bool;
}

trait SerialConsole {
    fn console_write(&mut self, text: &str);
}
```

- Callback entry context: `S` (subsystem-serialized entry)
- The OS serial framework holds the subsystem lock before invoking these
  callbacks. Compiler-generated wrappers do not acquire an additional lock;
  callback bodies run inside the framework's lock scope.
- Trait grouping: `per_device_class`
- `SerialConsole` is an optional companion trait, not part of the required core
  `Serial` baseline

### Intrinsic baseline

- `push_rx_data` remains an explicit intrinsic used by generated/driver code to
  move received bytes into upper serial buffering paths

### Property baseline

All four construct properties are Layer 2, deployment-specific hardware binding
 facts. None of them are stable Layer 1 item attributes.

## Extraction Baseline

The frontend must recognize:

- `driver_lifecycle.device_class = serial`
- `impl Serial for DriverType { ... }`
- optional `impl SerialConsole for DriverType { ... }`
- `push_rx_data(...)` intrinsic use
- Layer 2 hardware binding schema fields for serial deployment properties

Primary specification signals:

- trait impl surface for callback methods
- `device_class = serial` as the construct selector
- Layer 2 binding data for serial properties

Required extraction validations:

- a driver declaring `device_class = serial` must implement `Serial`
- `SerialConsole` is optional
- the four serial properties must be read from Layer 2, not invented from
  Layer 1 defaults unless a lowering rule explicitly says so

## Lowering Baseline

Lowering ownership:

- trait callbacks: backend realizes serial callback contracts
- Layer 2 properties: compiler performs Layer 1 / Layer 2 join
- `register_driver`: compiler-generated serial-subsystem wiring
- `push_rx_data`: backend/compiler intrinsic realization

### Generated lifecycle phase table

| Primitive | Init preamble | Deinit postamble | Suspend postamble | Resume preamble |
|---|---|---|---|---|
| `register_driver` | no | no | no | no |
| serial Layer 2 join | yes | no | no | no |

Notes:

- `port_open` / `port_close` are a nested serial session lifecycle, not aliases
  for `init` / `deinit`
- `register_driver` is separate from `driver_lifecycle.register_driver`
- backend folding is allowed where an OS does not expose open/close as distinct
  stages

## Cross-Construct Dependencies

Depends on:

- `Driver Lifecycle` for `device_class = serial`
- `Register Access` for UART register operations inside serial callbacks
- `Synchronization` for callback-context locking expectations

Related constraints:

- `Diagnostic Logging` and `console_write` may intersect with console paths
- `Device interrupt setup and handling` often feeds `rx_char` / `push_rx_data`
- `DMA Management` may participate in TX/RX fast paths but does not own the
  core serial callback surface

## Constraints and Caveats

- Keep the four serial properties in Layer 2. They are deployment facts, not
  per-driver declarations.
- `console_write` is a separate optional companion trait surface.
- `push_rx_data` is called from ISR-context receive paths. Backend handling of
  buffer-full conditions must stop draining the hardware FIFO, record an
  overrun condition through the target OS reporting path, and must not panic or
  block.
- `rx_char` is intentionally modeled as a data-returning callback rather than a
  v2.2-style pushed character callback.

## Representative Examples

```rust
#[rseq::device_class(serial)]
struct Pl011 {
    regs: Mmio<Pl011Registers>,
}

impl Serial for Pl011 {
    fn port_open(&mut self) -> Result<(), RegseqError> { /* ... */ }
    fn port_close(&mut self) { /* ... */ }
    fn configure(&mut self, config: &SerialConfig) -> Result<(), RegseqError> { /* ... */ }
    fn tx_byte(&mut self, byte: u8) { /* ... */ }
    fn rx_char(&mut self) -> Option<RxChar> { /* ... */ }
    fn tx_ready(&mut self) -> bool { /* ... */ }
}
```

Layer 2 examples:

- `default_baud_rate`
- `fifo_depth`
- `supports_flow_control`
- `supports_modem_signals`

## Implementation Checklist

- Treat `device_class = serial` as the gate for this construct.
- Enforce `Serial` implementation when that gate is present.
- Keep `SerialConsole` optional and distinct.
- Preserve the accepted callback return-shape distinctions across setup,
  teardown, data-plane write, data-plane read, and predicate operations.
- Join all four serial properties from Layer 2 binding data.
- Emit separate serial-subsystem registration glue.
- Support `push_rx_data` as an intrinsic with explicit overflow semantics.
