# Serial Taxonomy Gap Study

> Note:
> This document is retained as supporting research context.
> For compiler-facing decisions, use
> [serial-device-class-spec.md](/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/discussions/device-class-serial/serial-device-class-spec.md)
> as the current authority.

## Purpose

This note captures a small cross-platform research study of the current
`Serial/UART Management` taxonomy against what major target OS families provide
 for serial devices.

The goal is not to redesign the taxonomy immediately. The goal is to understand:

- whether the current `Serial` construct is sufficient for the current ASPLOS
  scope
- where the current taxonomy is strong
- where the current taxonomy is thin
- which gaps are true taxonomy gaps versus backend/lowering concerns

Primary reference for the current taxonomy:

- [serial-uart-management.md](/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/compiler-collateral/constructs/serial-uart-management.md)

## Current Serial Boundary

The current serial construct boundary is roughly:

- Layer 1:
  - `Serial`
    - `port_open`
    - `port_close`
    - `configure`
    - `tx_byte`
    - `rx_char`
    - `tx_ready`
  - optional `SerialConsole`
    - `console_write`
  - `push_rx_data` intrinsic
- Layer 2:
  - `default_baud_rate`
  - `fifo_depth`
  - `supports_flow_control`
  - `supports_modem_signals`
- compiler-managed:
  - serial registration/wiring
  - target-specific callback adaptation
  - target-specific join with other constructs

The study question was:

- is this enough to capture and lower UART-class serial devices across Linux,
  Zephyr, FreeBSD, macOS, and Windows?

## Study Inputs

Researchers reviewed the current taxonomy against:

- Linux + Zephyr serial/UART driver model
- FreeBSD + macOS serial/UART driver model
- Windows serial/UART driver model

## Overall Result

Across all three reports, the common judgment was:

- `workable_with_gaps`

Across all three reports, the PL011-specific judgment was:

- `strong fit`

So the current taxonomy should be understood as:

- strong for the current PL011-class UART scope
- workable, but not fully general, for broader serial device families

## What Maps Cleanly

The following parts of the taxonomy mapped cleanly across the reviewed OS
families:

### Serial session/configuration core

- `port_open`
- `port_close`
- `configure`

These map cleanly to OS/framework lifecycle or activation/configuration entry
points, even when the target folds or renames them.

### Console companion surface

- `SerialConsole::console_write`

This maps cleanly to the console-style write path across the reviewed targets.

### Compiler-managed registration

- `serial_uart.register_driver`

All targets have framework- or subsystem-level serial registration that is
better treated as compiler-generated glue rather than authored surface.

### Layer 2 deployment/capability facts

- `default_baud_rate`
- `fifo_depth`
- `supports_flow_control`
- `supports_modem_signals`

These correspond to real deployment or hardware capability facts on the
reviewed targets.

### Cross-construct decomposition

The current separation of concerns remains strong:

- serial callbacks stay in `Serial`
- hardware register interaction stays in `Register Access`
- RX delivery from ISR paths can flow through `push_rx_data`
- locking stays in `Synchronization`
- PM stays in `PowerManagement`
- interrupt delivery stays in `Interrupt`

The researchers did not argue for collapsing those constructs together.

## Main Gaps

The study surfaced four recurring areas where the current taxonomy is thinner
than real OS serial frameworks.

### 1. Buffer/FIFO-oriented TX and RX behavior

Real OS serial frameworks are typically:

- buffer-oriented
- FIFO-oriented
- interrupt/batch oriented

Examples:

- Linux serial drivers drain or fill hardware FIFOs in loops
- Zephyr has FIFO-fill and async/DMA-oriented UART APIs
- Windows SerCx2 is buffer-oriented
- FreeBSD/macOS also expose buffer/drain-oriented TX/RX behavior

Current Regseq surface:

- `tx_byte(&mut self, byte: u8)`
- `rx_char(&mut self) -> Option<RxChar>`
- `tx_ready(&mut self) -> bool`

### Study conclusion

This is a real mismatch, but for PL011 it is mainly a lowering concern.

The backend can synthesize:

- FIFO drain/fill loops
- framework buffer interaction
- IRQ-driven batching
- start/stop signaling patterns

from the existing authored surface plus register operations.

### Classification

- `backend-only / lowering concern`

## 2. Modem-control operations are not explicit in Layer 1

Real serial stacks often expose operations for:

- DTR / RTS outputs
- CTS / DCD / DSR / RI input state
- modem-status change handling

Current Regseq taxonomy has only:

- `supports_modem_signals` as a capability fact

It does not define explicit Layer 1 operations for:

- setting modem control lines
- reading modem status lines

### Study conclusion

For PL011 specifically, this is usually not a blocker:

- these are just register reads/writes through `CR` / `FR` / related registers
- so the behavior belongs naturally to `Register Access`

For broader UART coverage, however, this may eventually require optional serial
trait extensions.

### Classification

- `backend-only` for PL011
- possible future `Layer 1` extension for broader serial support

## 3. Flow-control / throttle / flush / break operations are not explicit

Real serial stacks often expose operations such as:

- throttle / unthrottle
- FIFO purge / flush
- break control
- modem-interrupt enable or related line-control hooks

Current Regseq serial surface does not model these explicitly.

### Study conclusion

For PL011 specifically, these mostly collapse to register-level operations:

- flow-control configuration via control registers
- FIFO flush via register manipulation
- break control via line-control register bits

So they do not currently force a widening of the `Serial` trait for the PL011
scope.

### Classification

- mostly `backend-only` for PL011
- possible future `Layer 1` expansion outside the current ASPLOS scope

## 4. RX error status handling is underspecified

This was the most interesting gap from the study.

Real serial frameworks often surface RX-side error information such as:

- parity error
- framing error
- overrun
- break

Current Regseq surface has:

- `push_rx_data(...)`
- `rx_char() -> Option<RxChar>`

But the taxonomy does not yet clearly say how RX-side error status is surfaced
through those abstractions.

### Study conclusion

This does not look like a broad architecture failure, but it does look like a
real surface/detail gap.

The likely resolution path is not widening the whole `Serial` trait. It is
refining one of:

- `RxChar`
- `push_rx_data`

so that RX error metadata can be carried where needed.

### Classification

- `Layer 1 refinement`

## Per-OS Summary

### Linux and Zephyr

Judgment:

- `workable_with_gaps`

PL011 verdict:

- `strong_fit`

Important findings:

- TX behavior is batch/FIFO oriented in reality, but can be synthesized by
  lowering from `tx_byte`
- modem control and flow control are backend/register concerns for PL011
- Zephyr API-mode differences look like backend selection concerns, not a
  Layer 1 redesign requirement

### FreeBSD and macOS

Judgment:

- `workable_with_gaps`

PL011 verdict:

- `strong_fit`

Important findings:

- modem control is a real framework-level concept on these systems
- FIFO flush and break control are explicit concepts
- for PL011 these still mostly collapse to register operations
- RX error-status propagation deserves clearer modeling

### Windows

Judgment:

- `workable_with_gaps`

PL011 verdict:

- `strong fit`

Important findings:

- Windows SerCx2 is more buffer/callback oriented than the Regseq authored
  surface
- this still looks like backend adapter work, not a fatal taxonomy gap
- DMA integration appears as a future backend join issue rather than a current
  Layer 1 surface problem for PL011

## Interpretation

The current taxonomy does **not** justify the stronger claim:

- “the current `Serial` trait is already a complete cross-OS serial abstraction
  for most serial devices”

But it **does** justify the narrower claim:

- “the current serial construct boundary is sufficient for the PL011-class
  UART scope and can plausibly be lowered across the reviewed OS families with
  backend-specific adaptation”

That is a meaningful and useful result for the current implementation plan.

## Recommended Current Stance

Do **not** widen the `Serial` trait immediately.

Instead:

1. Keep the current serial surface for the current ASPLOS / PL011 scope.
2. Record explicitly that the current serial surface is optimized for
   PL011-class UARTs.
3. Treat the following as tracked future work, not current blockers:
   - optional modem-control serial extensions
   - optional flow-control / throttle / purge / break extensions
   - refinement of `RxChar` or `push_rx_data` for RX error metadata

## Recommended Documentation Follow-Up

The most useful immediate documentation clarification would be:

- note that modem control and hardware flow control for PL011-class devices are
  treated as `Register Access` concerns rather than `Serial` trait methods
- note that richer serial semantics may require optional future extensions
  beyond the current PL011-focused baseline

## Current Decision Support

For current planning, this study supports:

- continuing with the current `Serial` / `SerialConsole` baseline in
  `phase-b-04`
- not widening the trait surface before PL011 frontend closure
- treating remaining gaps as:
  - mostly backend/lowering concerns for PL011
  - one likely future Layer 1 refinement area around RX error status
