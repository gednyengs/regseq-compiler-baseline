# Compiler Constraints

## Purpose

This document collects shared implementation constraints that the compiler team
must honor even though they are not surface-definition blockers.

These constraints come primarily from:

- accepted Pass 1 notes
- `QG-01` PL011 full-device representability audit

## Constraint Classes

### Class A: Audit carry-forward constraints

These came directly out of `QG-01`.

#### IC-1: AMBA revision fallback

When Layer 2 does not supply `serial_uart.fifo_depth`, the AMBA/platform
backend must support a runtime fallback that queries the bus/PID revision path
and derives the effective FIFO depth. This remains compiler-managed and does
not introduce a new driver-authored primitive.

#### IC-2: Earlycon conditional emission

Linux earlycon registration is compiler-generated and conditional. The compiler
must emit the earlycon registration path only when the accepted serial surface
and target configuration require it.

The earlycon setup and write functions are derived from the accepted
`SerialConsole` surface and the declared register layout. This remains
compiler-generated rather than driver-authored.

#### IC-3: DMA pause/status multi-step lowering

Linux DMA lowering must preserve the accepted pause/status/terminate semantics
for active transfers.

At minimum, Linux lowering must support the pause-drain-stop protocol:

1. pause the transfer without destroying state
2. query residue / progress
3. drain data based on the observed residue
4. terminate the transfer

This constraint also covers the periodic residue-query use case that appears
outside the stop sequence in PL011. If later DMA callback work requires the
driver to observe transfer progress mid-flight as a first-class surface
behavior, evaluate a dedicated `dma.channel_query_residue` primitive then.

This remains the highest-risk deferred implementation detail carried forward
from the audit and should be handled early in DMA/backend work.

#### IC-4: nbcon console protocol

Linux `console_write` lowering must account for the nbcon cooperative
ownership-yield protocol where required.

This includes the per-character yield / reacquire pattern needed by the Linux
nbcon path, not just a coarse lock/unlock wrapper. This is a backend
realization detail of the accepted console-path model, not a new surface
primitive.

#### IC-5: MMIO barrier emission

Register Access lowering must make MMIO barrier emission explicit and correct
for target architecture and side-effect context.

These barriers are part of the lowering contract, not optional optimizations.
On ARM-family targets this includes data-memory ordering barriers and, where
required by control-register effects, instruction-synchronization barriers as
well. This is especially important for weakly ordered architectures.

#### IC-6: `push_rx_data` overflow handling

The backend must handle receive-buffer overflow internally when lowering
`serial_uart.push_rx_data`; the driver surface does not spell these checks
directly. This handling must not panic or block in ISR context.

#### IC-7: SoC power-domain ordering

Where a target SoC requires peripheral power-domain operations separate from
clock enable, the backend must emit them in the correct order:

- init / resume path: power-domain enable before clock enable
- suspend / deinit path: clock disable before power-domain disable

This remains a backend obligation under the accepted PM baseline; it does not
introduce a new surface primitive.

#### IC-8: Poll-loop yield hints

When lowering `register_access.poll`, the backend must emit the appropriate
poll-loop yield hint where required by the target environment rather than
treating the loop as a raw tight spin only.

For Linux PL011 this corresponds to the accepted `cpu_relax`-style lowering
detail rather than a separate driver-authored timed-delay primitive.

### Class B: Accepted non-blocking caveat families

These are accepted caveats, not blockers, but they should shape implementation
planning:

- synchronization variant collapse
- DMA lifecycle many-to-one collapse
- serial registration many-to-one collapse
- vendor-specific PM/power-domain mechanism widening
- Zephyr `irq_lock` / `irq_unlock` ownership split as a capture artifact
- concern-level misattribution rows that do not change construct ownership

### Class C: Explicitly deferred follow-up

These remain legitimate later-stage follow-up topics:

- deeper DMA callback / progress-query model
- any future reconsideration of AMBA revision as an explicit property
- any future naming cleanup after the already-accepted widening of the
  `clock_gating` semantic description is applied to the PM elaboration

## Implementation Guidance

Use these constraints in planning, code review, and backend tasking.

A good default rule is:

- if a compiler task touches Register Access, Serial console, DMA, or PM
  lowering, review this file first

## Relationship to Construct Docs

This document holds the cross-construct constraints.

Construct-specific compiler docs should only repeat a constraint when it is
central to understanding that construct’s implementation shape.
