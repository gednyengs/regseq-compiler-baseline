# PL011 Unified Spec Boundary

## REFRAMING (2026-04-20)

This note has been reframed by the round-4 Codex-reviewed strategy
synthesis. The inclusion rule has moved from
"real hardware or integration capabilities evidenced in at least
one corpus driver" to an outcome-oriented, axis-derived inclusion
rule. The new authoritative artifacts are:

- `pl011-comparison-axes.md` -- the load-bearing document defining
  WHAT is compared (system-use cases + derived comparison axes).
- `pl011-workloads.md` -- HOW each axis is exercised (stimulus,
  observable, environment, verdict rule, statistical contract).
- `pl011-spec-requirements.md` -- the definitive requirements list,
  each requirement traced to `axis`, `correctness`, or
  `backend-contract` evidence.

The sections below are preserved as historical context. Where they
conflict with the new artifacts, the new artifacts win. The
current corpus is four OSes (Linux, FreeBSD, Zephyr, Windows 10 IoT
Core) as collected in `reference-drivers/pl011/`; Windows has been
added since the original version of this note.

The current Regseq paper claim, per the round-4 synthesis:

> Regseq's semantic driver model expresses a single ARM PrimeCell
> PL011 specification, and its compiler generates Linux and Zephyr
> drivers that match native-driver behavior within predefined
> tolerance rules on QEMU-relative functional and performance
> workloads derived from concrete PL011 system use cases.

Internal staging is the 3-stage progressive plan (Stage 1
paper-min core with no QEMU patches; Stage 2 flow control +
error injection + independent register observation; Stage 3
performance extensions including DMA). Stage membership for
every REQ-* is authoritatively defined in
`pl011-spec-requirements.md` Section 1.2. The paper presents
the cumulative F+I+P result at Stage-3 closure.

---

# Historical content (below)

This note proposes a candidate boundary for the unified Regseq PL011
spec based on the current cross-OS corpus evidence from:

- Linux PL011
- FreeBSD PL011
- Zephyr PL011

There is no Windows PL011 driver in the current corpus.
(Historical statement; a Windows PL011 driver has since been added
to `reference-drivers/pl011/windows/`.)

The aim is to separate:

- what belongs in the unified PL011 hardware spec
- what should be represented as optional capabilities or integration
  properties
- what should stay in backend lowering rather than in the unified spec

## Core principle

The unified PL011 spec should include:

- stable PL011 hardware semantics
- driver-author decisions that the compiler cannot infer
- optional capabilities that are real hardware or integration features
  evidenced in at least one corpus driver
- the portable bus/fabric identity when it is part of the true device identity

The unified PL011 spec should exclude:

- pure OS mechanism details
- framework glue with no hardware meaning
- compiler-internal orchestration that the driver author should not
  declare directly

## 1. Mandatory core of the unified PL011 spec

These features are evidenced across all three OSes and should be
treated as the mandatory PL011 core:

1. TX and RX data path, including both polled and interrupt-driven use
2. baud-rate programming via integer and fractional divisors
3. line-format configuration:
   - word length
   - parity
   - stop bits
4. FIFO enable/disable and interrupt-threshold configuration
5. interrupt status, mask, acknowledge, and service lifecycle
6. UART enable/disable sequencing
7. receive-path error detection and reporting
8. revision / variant-aware configuration is evidenced in the broader corpus,
   but remains deferred for the current single-variant ARM PL011 target

These form the minimum cross-OS PL011 hardware surface that Regseq
should model directly.

## 2. Optional but real PL011 capabilities that should be spec-visible

These are not universal in the current drivers, but they are real
hardware or integration capabilities and should be represented in the
unified spec as optional features or conditional properties:

1. DMA TX support
2. DMA RX support
3. hardware RTS/CTS flow control
4. modem status / modem control signals
5. RS-485 half-duplex support
6. break generation
7. pinctrl integration
8. reset-controller integration
9. clock integration and platform clock-selection properties
10. PM-aware save/restore or suspend/resume state handling
11. early-console / boot-console support where the target backend needs
    a dedicated early path
12. board / SoC-specific quirk hooks and variant-specific register
    behavior

These should not all be mandatory for every generated driver. For the current
concrete full-spec fixture, variant/revision/quirk machinery and PM-aware
behavior remain intentionally deferred even though they are evidenced in the
broader corpus.

## 3. Features that should stay out of the unified hardware spec

These should remain in lowering/backends, compiler support, or platform
integration layers rather than being first-class PL011 hardware-spec
items:

1. OS-specific IRQ registration APIs
2. OS-specific lock APIs and lock object shapes
3. MMIO mapping APIs and bus-resource acquisition details
4. device-object registration and driver binding glue
5. generic logging / tracing mechanisms
6. generic timer/workqueue mechanics used to support a feature
7. platform-framework names for pinctrl, clock, reset, or PM services

These are important for generated drivers, but they are backend
mechanisms, not PL011 semantics.

## 4. Recommended shape of the unified Regseq PL011 spec

The unified PL011 spec should probably expose four classes of content:

### A. Core PL011 operations

Examples:

- initialize / enable / disable UART
- configure line settings
- configure baud rate
- read RX status
- receive characters
- transmit characters
- enable / disable / acknowledge interrupts
- configure FIFO thresholds

### B. Declarable PL011 properties

Examples:

- input clock frequency
- FIFO depth or variant profile where not inferable
- register width / access mode overrides
- interrupt wiring mode
- default line format
- optional flow-control support
- optional DMA channel availability
- optional RS-485 configuration
- optional modem-control availability
- optional quirk flags or variant selectors

### B1. Driver identity and bus attachment

For the current unified PL011 target, the authored identity should retain both:

- the concrete firmware-facing compatible identity
- the portable bus/fabric identity

Recommended authored shape:

```rust
#[rseq::driver(
    name = "pl011",
    compatible = ["arm,pl011"],
    bus = "amba"
)]
```

Interpretation:

- `compatible = ["arm,pl011"]` remains the concrete device identity used by
  firmware-facing matching paths
- `bus = "amba"` expresses that PL011 is not merely a generic integrated MMIO
  UART, but an ARM AMBA / PrimeCell-family device

This is stronger than treating PL011 as a generic `integrated` device because
the AMBA/PrimeCell identity is materially relevant to:

- stock Linux realization shape
- AMBA revision / FIFO-depth fallback considerations
- later benchmarking against the stock `amba-pl011` path

At the same time, the authored Layer 1 spec should not carry deployment-level
AMBA binding numbers directly. AMBA deployment facts such as peripheral ID or
revision should live in Layer 2 bus metadata rather than as generic firmware
properties.

### C. Optional integration capabilities

Examples:

- clock source / gate hookup
- reset line hookup
- pinctrl states
- low-power save/restore support
- early-console eligibility

These are not raw PL011 register semantics, but they are still
driver-author visible and affect correct generated drivers.

### D. Compiler/backend responsibilities

Examples:

- map interrupts to target-OS registration APIs
- lower MMIO operations to each OS
- generate synchronization patterns
- route optional features through the target framework
- handle compiler-managed startup / teardown sequencing
- map the portable bus identity into each target OS's native registration model

## 5. Practical interpretation for the compiler team

The safest design stance is:

- use the common three-OS baseline as the non-negotiable hardware core, while
  allowing the concrete target fixture to defer variant/revision machinery
- add Linux- and Zephyr-only features as optional spec-visible
  capabilities
- avoid baking FreeBSD's current omissions into the unified spec as if
  they were hardware limitations
- treat PL011 as `bus = "amba"` rather than as a merely generic integrated
  device
- retain `compatible = ["arm,pl011"]` alongside `bus = "amba"` because bus
  family and concrete firmware-facing device identity are complementary, not
  interchangeable

That means the unified spec should be **maximal-but-structured**, not
minimal and not a naive union.

## 6. Best paper story supported by this boundary

The corpus supports the following claim:

- existing PL011 drivers each capture different subsets of the hardware
  feature surface
- Regseq can recover the maximal feature set evidenced across those
  drivers
- once encoded in a unified spec, that hardware knowledge becomes
  reusable across backends
- therefore, a thinner target OS implementation can inherit features
  already discovered elsewhere without manually rediscovering PL011
  behavior from scratch

That is the strongest compiler-and-reuse interpretation of the current
PL011 evidence.
