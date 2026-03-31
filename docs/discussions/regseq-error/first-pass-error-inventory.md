# First-Pass Error Inventory

## Purpose

This note records a first-pass inventory of fallible surfaces from the accepted
 compiler collateral. It is meant to support the next step of the `RegseqError`
 design:

- which surfaces unify on `RegseqError` at outermost authored fallible
  boundaries
- which operations should get intrinsic- or operation-local error types that
  later convert into `RegseqError`

This is an inventory note, not a final classification resolution.

## Classification Strategy

The current working split is:

1. **Trait-boundary fallible functions**
   - these are outermost authored fallible boundaries
   - they should return the shared `RegseqError` carrier

2. **Intrinsic or intrinsic-backed fallible operations**
   - these are the likely candidates for local error types
   - local error types should later convert into `RegseqError`

3. **Compiler-managed-only failures**
   - these do not surface as authored DSL fallible returns
   - they are still relevant for backend lowering, but not for the authored
     error carrier inventory

## A. Trait-Boundary Fallible Functions

These should remain unified on `RegseqError`.

| Construct | Surface | Current stable surface | Current classification |
|---|---|---|---|
| Driver Lifecycle | `Lifecycle::init` | `Result<(), RegseqError>` | trait-boundary |
| Driver Lifecycle | `Lifecycle::deinit` | `Result<(), RegseqError>` | trait-boundary |
| Device power state management | `PowerManagement::suspend` | `Result<(), RegseqError>` | trait-boundary |
| Device power state management | `PowerManagement::resume` | `Result<(), RegseqError>` | trait-boundary |
| Serial/UART Management | `Serial::port_open` | `Result<(), RegseqError>` | trait-boundary |
| Serial/UART Management | `Serial::configure` | `Result<(), RegseqError>` | trait-boundary |

### Notes

- `Serial::port_close`, `tx_byte`, `rx_char`, `tx_ready`, and
  `SerialConsole::console_write` are not part of the fallible shared
  `RegseqError` boundary.
- Additional trait-boundary fallible functions may appear later for non-PL011
  constructs, but this is the current accepted PL011-facing set.

## B. Intrinsic Or Intrinsic-Backed Fallible Operations

These are the main candidates for local error types.

The table intentionally distinguishes between:

- explicit free-function intrinsics
- wrapper methods that likely need an underlying intrinsic or operation-local
  error contract
- unresolved rows where the collateral still names the wrapper surface but the
  underlying intrinsic/error-family split has not yet been written down

| Construct | Surface | Current stable surface | Likely local error type | Confidence |
|---|---|---|---|---|
| Register Access | `regseq::poll::until(...)` | `Result<(), RegseqError>` | `PollError` | high |
| Timed CPU delay and sleep | `regseq::delay::busy_wait(Duration)` | infallible intrinsic | none | high |
| Timed CPU delay and sleep | `regseq::delay::sleep(Duration)` | infallible intrinsic | none | high |
| Serial/UART Management | `push_rx_data(...)` | intrinsic | unresolved | medium |
| Peripheral Clock Management | `Clock::enable(&self)` | `Result<(), RegseqError>` | `ClockError` | medium |
| Hardware Reset Sequencing | `ResetLine::assert(&self)` | `Result<(), RegseqError>` | `ResetError` | medium |
| Hardware Reset Sequencing | `ResetLine::deassert(&self)` | `Result<(), RegseqError>` | `ResetError` | medium |
| Hardware Reset Sequencing | `ResetLine::toggle(&self, delay)` | `Result<(), RegseqError>` | `ResetError` or `ResetToggleError` | medium |
| Synchronization | `Completion::wait(&self, timeout)` | `Result<(), RegseqError>` | `WaitError` / `CompletionError` | medium |
| DMA Management | `regseq::dma::map(...)` | `Result<DmaMapping, RegseqError>` | `DmaMapError` | medium |
| DMA Management | `DmaChannel::prepare(&self, mapping)` | `Result<(), RegseqError>` | `DmaPrepareError` | medium |
| DMA Management | `DmaChannel::prepare_sg(&self, mappings)` | `Result<(), RegseqError>` | `DmaPrepareError` or `DmaSgError` | medium |
| DMA Management | `DmaChannel::submit(&self)` | `Result<(), RegseqError>` | `DmaSubmitError` | medium |
| Pin Controller And GPIO Configuration | `PinCtrl::apply<S>(&self)` | `Result<(), RegseqError>` | `PinCtrlError` | medium |

### Notes

- `poll::until(...)` is the strongest candidate for a local error type because
  the accepted failure is already semantically narrow (`Timeout`).
- `Clock::enable`, `ResetLine::*`, `Completion::wait`, DMA operations, and
  `PinCtrl::apply` still need explicit design work to determine:
  - whether the wrapper is merely sugar over an intrinsic
  - what the local error type is
  - which local variants exist
  - how those variants map upward to `RegseqError`
- `busy_wait` and `sleep` are intrinsic-only but currently infallible in the
  accepted surface, so they are not part of the error inventory.

## C. Wrapper Methods That Appear Infallible In The Current Surface

These are wrapper or API surfaces that currently do not participate in the
 `RegseqError` discussion directly.

| Construct | Surface | Current stable surface | Notes |
|---|---|---|---|
| Peripheral Clock Management | `Clock::disable(&self)` | infallible | keep out of current error inventory |
| Peripheral Clock Management | `Clock::get_rate(&self)` | `u32` | may still need target notes, but not an error carrier |
| Register Access | `read` / `write` wrapper methods | infallible wrapper methods | ordering/side-effect concerns belong elsewhere |
| Synchronization | `.lock()` | infallible | legality/analysis issue, not current error-carrier issue |
| Synchronization | `.signal()` / `.reset()` | infallible | not part of current error inventory |
| DMA Management | `DmaChannel::configure(&self)` | infallible | declaration/L2-derived |
| DMA Management | `DmaChannel::stop(&self)` | infallible | not part of current error inventory |

## D. Compiler-Managed-Only Failure Zones

These operations may still fail at lowering time, but they are not authored DSL
 fallible surfaces and therefore are not part of the authored error-carrier
 inventory.

| Construct | Compiler-managed primitive | Notes |
|---|---|---|
| Driver Lifecycle | `create_device`, `associate_private_data` | generated preamble |
| Device power state management | `pm_registration`, generated PM ordering | generated glue |
| Register Access | `map_region` | generated preamble/postamble |
| Peripheral Clock Management | `acquire_clock` | generated preamble/postamble |
| Hardware Reset Sequencing | `acquire_reset` | generated preamble/postamble |
| Pin Controller And GPIO Configuration | `acquire_pinctrl` | generated init preamble |
| DMA Management | `alloc_coherent`, `request_channel`, `set_dma_mask` | generated preamble/postamble |
| Bus Resource Management | acquisition/mapping support | declarative/compiler-managed |
| Firmware Property Access | `read_property` | compiler-managed only |
| Device interrupt setup and handling | line-level `enable` / `disable` | compiler-managed |

### Notes

- These zones still matter for the full lowering contract.
- They should not be confused with authored semantic `RegseqError` flow.
- They may still need diagnostics and provenance tracking, but not through the
  same authored-carrier design.

## E. First Candidate Local Error Families

This is a candidate family list, not a final design.

| Candidate local error type | Likely owned surfaces | Status |
|---|---|---|
| `PollError` | `regseq::poll::until(...)` | strongest candidate |
| `ClockError` | `Clock::enable()` | needs variant design |
| `ResetError` | `ResetLine::assert/deassert/toggle` | needs variant design |
| `CompletionError` or `WaitError` | `Completion::wait(timeout)` | needs variant design |
| `PinCtrlError` | `PinCtrl::apply<S>()` | needs variant design |
| `DmaMapError` | `regseq::dma::map(...)` | needs variant design |
| `DmaPrepareError` | `DmaChannel::prepare/prepare_sg` | needs variant design |
| `DmaSubmitError` | `DmaChannel::submit()` | needs variant design |
| `PushRxDataError` or none | `push_rx_data(...)` | unresolved whether this should expose a fallible local error at all |

## F. Biggest Gaps Before Resolution

1. **Underlying intrinsic mapping**
- For wrapper surfaces like `Clock::enable`, `ResetLine::toggle`, and
  `PinCtrl::apply`, we still need to decide whether the wrapper itself is the
  operation-local error boundary or whether there is a deeper explicit
  intrinsic family beneath it.

2. **Local variant sets**
- Each candidate local error type still needs an explicit variant set.

3. **Upward mapping**
- Each local variant still needs an explicit `From<LocalError> for RegseqError`
  mapping.

4. **Preamble/authored overlap**
- Some constructs have both compiler-managed preamble operations and authored
  fallible operations in the same area (`Clock`, `Reset`, `DMA`). The error
  inventory must remain clear about which part belongs to authored semantic
  flow and which part remains generated/native.

## Recommended Next Step

Use this note to drive the next discussion pass:

1. confirm the trait-boundary inventory
2. pick the first local error family to finalize, likely `PollError`
3. then work construct by construct:
   - `ClockError`
   - `ResetError`
   - `CompletionError`
   - `PinCtrlError`
   - DMA families
