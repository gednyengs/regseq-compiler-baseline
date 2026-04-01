# PL011 Spec Slice Discussion

## Purpose

This discussion note captures the current thinking about the first real
`track-a-pl011` frontend slice that Phase B-08 should assemble.

The goal is to make the slice boundary explicit before creating executable
B-08 task specs.

## Where This Fits

The overall PL011 work has three broad parts:

1. Frontend: Regseq-authored Layer 1 and Layer 2 specification for PL011
2. Compiler: analysis, internal Regseq-model generation, and pre-codegen work
3. Codegen: target-specific lowering and emission

Phase B-08 addresses only the **frontend** side, and even there only the first
real slice rather than the full PL011 specification.

So the purpose of B-08 is:

- write the first real Regseq-authored PL011-facing frontend artifact
- prove that accepted Phase B surfaces compose in a realistic device setting
- keep the work frontend-only
- stop before normalized join/model work and before lowering

## What B-08 Should Prove

The first PL011 slice should prove that Regseq can express a real PL011-facing
specification using the accepted surfaces from earlier Phase B work, not just
isolated snippet tests.

At minimum, the slice should show:

- a real `track-a-pl011` spec crate exists and compiles
- a real PL011 register block and field schema exists
- a real PL011 driver type exists
- the driver type uses accepted resource declarations
- the driver type implements accepted callback-family traits where needed by the
  slice
- the slice is extractable by the existing frontend capture machinery
- the slice stays within the accepted Phase B boundary

## What B-08 Is Not

B-08 is not:

- the full PL011 specification
- the full Layer 2 binding coverage for every PL011 deployment fact
- the final joined Layer 1 + Layer 2 model
- backend lowering or code generation
- raw-artifact closure
- final validation passes over the normalized compiler model

So the right mental model is:

- B-08 assembles the first **credible frontend specimen**
- later work turns that into normalized compiler inputs and then backend code

## First Slice Candidate Shape

The leading direction is a narrow but real PL011 slice that includes:

1. A spec crate under `track-a-pl011`
2. The PL011 register schema and register block declarations needed by the
   accepted slice
3. A PL011 driver type using accepted Regseq resource fields
4. A small but real callback surface for the slice
5. A matching Layer 2 platform fixture that stays within the currently
   supported B-07 format

The slice should be real enough to exercise:

- `register_bitfields!`
- `#[rseq::register_block(...)]`
- `#[rseq::register(...)]`
- `Mmio<T>`
- at least some real wrapper calls over PL011 registers
- at least one real callback-family trait implementation
- at least one real Layer 2 `[properties]` binding fixture

## Likely Contents of the First Slice

The current leading candidate is a PL011 slice centered on:

- register declarations for the subset needed by initialization / basic UART
  bring-up
- firmware property declarations needed by the current accepted property subset
- clock, reset, and pinctrl declarations if they are part of the accepted
  PL011-facing baseline for the slice
- lifecycle and serial callback impls only to the extent needed for the first
  credible frontend specimen

This suggests the first slice may focus on:

- initialization-facing register setup
- callback surfaces that are already supported by Phase B
- logging and delay only if the chosen slice naturally needs them

## Realistic Narrowness

The slice should remain narrow.

That means the first slice probably should **not** try to include all of:

- every PL011 interrupt path
- every modem-control feature
- every PM path
- every pinctrl state transition
- full console integration
- every error path and every optional callback family

The right test is not “is this all of PL011?”

The right test is:

- does this look like a real PL011-facing Regseq driver fragment rather than a
  synthetic demo?
- does it use the accepted surfaces in combination rather than isolation?
- does it stay within what the compiler can currently parse/extract?

## Layer 2 Boundary for the First Slice

B-08 should rely only on the Layer 2 subset already established in B-07.

That means:

- use the required platform-file header
- use `[properties]`
- avoid depending on future Layer 2 sections that do not yet have parser/model
  support unless the task explicitly limits them to checked-in placeholders for
  review purposes

So the first slice should not depend on full Layer 2 join support for:

- mmio base resolution
- interrupt binding resolution
- clock binding resolution
- reset binding resolution
- pinctrl state resolution

Those remain later work.


## Recommended First Slice

The current recommended first slice is a **polling-first PL011 bring-up slice**.

This slice should include:

- a real `track-a-pl011` spec crate
- a real PL011 register schema and register block for the subset needed by
  basic UART bring-up
- a real PL011 driver type
- the minimum real callback surface needed to make the driver feel credible
- a matching Layer 2 platform fixture that uses only the currently supported
  B-07 subset

### Recommended register subset

The current leading register subset is:

- `DR`
- `FR`
- `IBRD`
- `FBRD`
- `LCR_H`
- `CR`
- optionally `IMSC` / `ICR` only if the slice needs to make interrupt masking
  explicit during bring-up

This subset is enough to show:

- data access
- polling status checks
- baud-rate setup
- line-control / FIFO setup
- UART enable / TX / RX enable

### Recommended driver shape

The current leading driver shape is:

- `Mmio<Pl011Registers>`
- firmware properties such as:
  - `reg-io-width`
  - `current-speed`
- optional `Clock`
- optional `ResetLine`
- optional `PinCtrl`

This gives the slice a believable PL011-facing resource shape without forcing
 full Layer 2 join support for every resource family yet.

### Recommended callback surface

The current leading callback surface is:

- one real `Lifecycle` implementation for init/teardown shape
- one real `Serial` implementation for a polling UART path

The initial `Serial` path should cover:

- `configure`
- `tx_ready`
- `tx_byte`
- `rx_char`

This gives the slice a real UART-facing operational surface while staying
 within the already accepted Phase B callback families.

### What the slice should do

The first slice should show real authored PL011 logic for:

- UART configuration writes
- baud-rate programming sequence
- FIFO / line-control setup
- polling-based TX and RX readiness checks through `FR`
- data movement through `DR`

If included, `Clock`, `ResetLine`, and `PinCtrl` should be present only at the
 frontend-declaration/use level already supported by earlier batches.
 The first slice should not depend on fully modeled Layer 2 joins for those
 resources.

### What the first slice should leave out

The first slice should not try to include all PL011 behavior.

It should leave out:

- ISR handlers
- interrupt-driven RX/TX paths
- `PowerManagement`
- `SerialConsole`
- modem-control features
- DMA
- suspend/resume pinctrl state transitions
- full Layer 2 resource sections beyond `[properties]`

### Why this is the right first slice

This slice is a good first specimen because it:

- looks like a real UART driver fragment rather than a toy example
- exercises the most important accepted Phase B surfaces in combination
- avoids depending on not-yet-built join/model/lowering work
- gives strong frontend evidence that the Regseq DSL can express a real PL011
  bring-up and polling path

## Main Open Questions

1. What is the narrowest PL011 slice that still feels like a real device-facing
   spec?
2. Which callback-family traits are mandatory for the first slice:
   - `Lifecycle`
   - `Serial`
   - `SerialConsole`
   - `PowerManagement`
3. Which PL011 properties are essential to include in the first slice?
4. Should the first slice include interrupt declarations, or should interrupts
   wait for a later PL011 slice once more of the end-to-end path is ready?
5. Should the first slice include clock/reset/pinctrl fields now, even if their
   Layer 2 joins are not yet modeled end-to-end?
6. What exact compile/extraction evidence should B-08 require from the slice?

## Working Direction

The current working direction is:

- build a real `track-a-pl011` spec crate
- include a real PL011 register schema and block
- include a real PL011 driver type with accepted Regseq declarations
- implement the minimum real callback surface needed for a convincing frontend
  slice
- include only the Layer 2 property fixture support already available
- stop before full Layer 2 joining, normalized model work, and lowering

This note should be refined before B-08 task specs are authored.
