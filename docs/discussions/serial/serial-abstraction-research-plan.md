# Serial Abstraction Research Plan

## Purpose

This note captures a research-oriented plan for moving beyond the current
PL011-adequate `Serial` abstraction toward a serial abstraction that can make a
much stronger cross-device and cross-OS claim.

The immediate motivation is:

- the current `Serial` taxonomy is strong for PL011-class UARTs
- but it is narrower than what would support a broader research claim such as
  “captures most serial devices”

So the goal of this plan is not to redesign the abstraction immediately. The
goal is to define a credible research methodology for deriving and validating a
broader serial abstraction.

## Research Goal

The research goal is to derive a serial abstraction that:

1. captures the essential authored semantics of a large portion of real serial
   devices
2. separates:
   - portable authored surface
   - target-specific lowering concerns
   - optional or device-family-specific extensions
3. is evaluated against a meaningful corpus of:
   - serial device families
   - operating-system driver frameworks
4. can support a publication-quality claim stronger than:
   - “works for PL011”

The intended outcome is not necessarily one monolithic trait. A more plausible
and publishable outcome is:

- a small mandatory serial core
- plus optional extension surfaces

## Non-Goal

This plan is not trying to prove:

- one small fixed trait can represent every serial device

That is likely the wrong target. A better target is:

- derive a minimal core abstraction
- identify principled optional extensions
- quantify coverage over a real corpus

## Current Limitation

The current `Serial` construct in:

- [serial-uart-management.md](/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/compiler-collateral/constructs/serial-uart-management.md)

is best understood as:

- a PL011-optimized UART core abstraction

The recent cross-platform study showed:

- `PL011`: strong fit
- broader serial device space: workable with gaps

This is acceptable for the current implementation stream, but not yet the
strongest possible research position.

## Research Questions

The study should answer at least these questions:

1. What operations recur across major serial/UART driver frameworks?
2. Which of those operations are genuinely authored semantics, rather than
   backend glue?
3. Which operations belong in:
   - a mandatory serial core
   - optional extension traits
   - Layer 2 binding properties
   - compiler intrinsics
   - backend-only lowering
4. What fraction of sampled devices are:
   - fully expressible with the core
   - expressible with the core plus extensions
   - only expressible with backend-specific escape hatches
   - not expressible at all
5. What abstraction boundary is credible to claim in a research publication?

## Proposed Methodology

The right methodology is:

- comparative systems interface analysis
- followed by taxonomy synthesis
- followed by corpus-based coverage evaluation

In practical terms:

### 1. Define the study population

Select a device-and-OS corpus large enough to make the claim meaningful.

The corpus should include:

- UART IP families:
  - PL011
  - 8250 / 16550-style UARTs
  - STM32 USART
  - NXP / LPUART variants
  - DesignWare APB UART
  - at least a few devices with modem-control or DMA-heavy behavior
- OS/framework families:
  - Linux
  - Zephyr
  - Windows
  - FreeBSD
  - macOS / DriverKit / IOKit where relevant
- operation modes:
  - polling
  - interrupt-driven
  - DMA-backed
  - console mode
  - modem-control use cases
  - flow-control use cases

The corpus must not be limited to only devices that already fit the current
taxonomy.

### 2. Build a structured interface inventory

For each device/OS pair, record:

- driver callbacks
- driver-visible operations
- device capabilities
- deployment properties
- compiler-managed / framework-managed glue
- interrupt interaction
- DMA interaction
- modem-control behavior
- error-reporting behavior

This should be captured in a structured matrix, not only prose.

A recommended table shape:

- OS / framework
- device family
- operation name
- semantic role
- callback vs intrinsic vs property vs backend glue
- required / optional
- notes

### 3. Perform semantic clustering

Cluster operations by meaning, not by API spelling.

Examples:

- `startup`, `open`, `activate` -> open/session-start family
- `set_termios`, `configure`, `HwProgramUART` -> configuration family
- `fifo_fill`, `write_buffer`, `transmit`, `start_tx` -> TX family
- `receive`, `read_buffer`, `rx_ready`, `push_rx_data` -> RX delivery family
- `set_mctrl`, `line_ctrl_set`, `HwProgramMCR` -> modem-control family
- `flush`, `purge`, `reset_fifo` -> flush/purge family
- `break_ctl`, `send_break` -> break-control family

This step is the heart of the abstraction work.

### 4. Separate core from extensions

Do not force every clustered semantic into one trait.

Instead derive:

- a mandatory core
- optional extensions

Possible outcome shape:

- `SerialCore`
- `SerialConsole`
- `SerialModemControl`
- `SerialFlowControl`
- `SerialBufferedIo`
- `SerialDma`
- `SerialErrorReporting`

This is likely more defensible than widening the current `Serial` trait until
it becomes too broad.

### 5. Classify what belongs where

For each semantic family, explicitly classify whether it belongs in:

- Layer 1 surface
- Layer 2 binding property
- intrinsic
- backend-only lowering
- out-of-scope extension

This classification is essential because many apparent “gaps” are really:

- compiler/lowering adapter work

not:

- missing authored abstraction

### 6. Validate on held-out cases

After deriving the candidate abstraction from part of the corpus, validate it
on held-out device/OS cases not used to design it.

This is what makes the research claim stronger than anecdotal design.

For each held-out case, score:

- core-only fit
- core + extensions fit
- backend-only adaptation required
- not representable

### 7. Quantify coverage

The final study should report:

- what percentage of sampled devices fit the core
- what percentage require optional extensions
- what percentage require backend-only adaptation
- what percentage remain out of scope

This is the evidence needed for publication-quality claims.

## Recommended Evidence Shape

The strongest publication story would likely be:

1. a corpus of real serial device/framework interfaces
2. a derived abstraction with:
   - small core
   - optional extensions
3. an explicit layering policy:
   - authored abstraction
   - binding properties
   - intrinsics
   - backend-only lowering
4. a coverage evaluation over the corpus
5. a case study:
   - PL011 as a strong-fit example

## Role of LLM Agents

LLM agents can be extremely useful for this work, but they should be used as
research accelerators, not as the final source of truth.

### What LLMs can do well

- survey OS/framework interfaces
- extract candidate callbacks and properties
- build comparative tables
- cluster semantically similar operations
- propose candidate core/extension decompositions
- identify apparent gaps
- generate held-out evaluation cases

### What LLMs should not be trusted to do alone

- guarantee factual completeness
- guarantee exactness in niche kernel/framework behavior
- justify final coverage claims without verification

### Recommended LLM-assisted workflow

1. Assign different OS families to different agents.
2. Require structured outputs, not prose only.
3. Compare the outputs and identify disagreements.
4. Human-verify the disagreements and the most important claims against primary
   sources.
5. Derive the abstraction from the verified corpus.
6. Use agents again to test the abstraction against held-out cases.

This gives:

- speed
- breadth
- some internal redundancy

without sacrificing publication credibility.

## Suggested Study Phases

### Phase 1: Corpus Construction

Deliverables:

- device/framework inventory
- operation/property matrix
- initial gap log

### Phase 2: Semantic Family Derivation

Deliverables:

- clustered operation families
- candidate core + extension decomposition
- boundary decisions for Layer 1 / Layer 2 / intrinsic / backend-only

### Phase 3: Validation

Deliverables:

- held-out evaluation
- coverage metrics
- failure cases
- explicit out-of-scope cases

### Phase 4: Publication Framing

Deliverables:

- final abstraction proposal
- evidence tables
- claims the evidence supports
- claims the evidence does not support

## What This Suggests For Regseq Right Now

Near-term implication:

- do not widen the current `Serial` trait reactively based on one or two gaps

Instead:

- keep the current PL011-adequate surface
- treat this research plan as the path to a broader future serial abstraction
- record candidate future extension areas:
  - modem control
  - flow control / throttle / purge / break
  - richer RX error reporting
  - buffered or DMA-oriented serial semantics

## Success Criterion

This research plan succeeds if it lets us make a credible claim like:

- “We derived a serial abstraction family from a comparative study of real
  serial driver frameworks, and the resulting core-plus-extension taxonomy
  captures a large fraction of sampled UART-class devices across major OS
  families.”

That is a much stronger and more defensible research contribution than simply
claiming the current PL011-focused `Serial` trait is already general enough.
