# PL011 Linux First Slice

## Purpose

This note fixes the first supported PL011-to-Linux lowering slice for
`phase-d-01`.

It exists so that task authors and implementers do not have to infer the
required feature boundary from scattered discussion notes. The concrete target
fixture is:

- `tests/fixtures/pl011-full-spec`

This note defines what that fixture means for the first Linux backend bring-up:

- what must be supported
- what may be deferred
- what the compiler/backend is expected to realize first

## Concrete Target

The first Linux lowering path should target the existing fixture:

- `tests/fixtures/pl011-full-spec/src/lib.rs`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `tests/fixtures/pl011-full-spec/src/registers.rs`

This is not an abstract PL011 subset discussion. It is the first Linux
realization target for the concrete `pl011-full-spec` fixture already present
in the repo.

## Required Driver Shape

The first Linux backend path must support the fixture as a platform-driver
shaped UART device with:

- one MMIO-backed PL011 register block
- one interrupt resource
- declared properties with authored defaults:
  - `current-speed`
  - `fifo-size`
- and any other already-captured property obligation that the current lowering
  input and readiness evidence treat as part of the concrete fixture surface
- optional clock participation
- optional reset-line participation
- optional pinctrl participation

The first backend path should also preserve the authored top-level module
partitioning implied by the fixture:

- `driver.rs`
- `registers.rs`
- `lib.rs` only if it owns meaningful emitted content

## Required Behavioral Surface

The first Linux backend path must cover the behavior already expressed in the
fixture and already treated as part of the concrete target:

- lifecycle initialization and teardown
- UART enable/disable sequencing
- baud-rate configuration
- line-format configuration
- FIFO configuration
- firmware-property reads with fallback-to-authored-default behavior for the
  declared properties in the fixture
- MMIO register reads/writes and register helper realization
- interrupt registration and ISR realization for the authored `#[rseq::isr]`
  path
- RX-path handling, including RX error observation/clearing behavior already
  present in the fixture
- TX and RX data-path support expressed by the fixture
- serial callback realization for the authored serial surface
- synchronization and ordering obligations already present in the fixture and
  accepted lowering contract
- explicit lowering of the authored `ordered!` usage in the fixture so the
  required sequencing semantics are preserved
- timed polling and delay obligations where already justified by the current
  construct set and lowering contract
- diagnostic logging anchors already present in the captured frontend surface

For RX error handling specifically:

- the backend must not silently drop the authored RX error signal carried by
  the fixture
- first-pass Linux realization may either:
  - map captured RX error flags into the appropriate Linux serial/TTY error
    path, or
  - treat exact Linux error-flag propagation as an explicit first-pass deferral
    while still preserving reviewable evidence that RX error information was
    captured and considered

The important rule is that RX error handling must be explicit, not silently
omitted.

## Linux Realization Required In First Pass

The first Linux backend pass must realize enough Linux scaffolding to compile
and support the first smoke path:

- backend-owned `platform_driver` registration
- backend-owned match-table definitions
- backend-owned probe/remove orchestration
- backend-owned `uart_driver` definition
- backend-owned `uart_ops` definition
- out-of-tree module build packaging

The Linux backend should not rely on handwritten per-driver wrapper glue to
supply any of those responsibilities.

## Explicit Deferrals

The following are out of scope for the first PL011 Linux bring-up even though
some are discussed elsewhere in the broader corpus:

- `SerialConsole`
- PM-aware suspend/resume or save/restore paths
- DMA
- hardware flow control
- modem status/control
- RS-485
- loopback
- IrDA/SIR
- family-wide PL011 variant/revision/quirk machinery
- in-tree Linux replacement as a prerequisite

These may become later Phase D or later-track work, but they are not required
for the first Linux compile/build/smoke proof.

## Practical Interpretation For Task Authors

When writing `phase-d-01` tasks:

- treat `tests/fixtures/pl011-full-spec` as the required lowering target
- do not reduce the target to a smaller hypothetical PL011 toy slice
- do not silently expand the target to include deferred PM/variant/console/DMA
  work
- make required-vs-deferred scope explicit in each task spec

## Exit Condition For This Slice

The first PL011 Linux slice is satisfied when the backend can lower the
concrete `pl011-full-spec` fixture into generated Linux artifacts that:

- reflect the required behavioral surface above
- compile in the first supported out-of-tree module workflow
- participate in the first PL011 smoke-validation loop defined separately in
  the evaluation-environment discussion

That is the right scope for `phase-d-01`.
