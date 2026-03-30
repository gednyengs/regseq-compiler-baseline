# Compiler Baseline Overview

## Purpose

This document is the compiler-facing baseline after:

- primitive mapping closure
- Pass 1 design closure
- `QG-01` PL011 full-device representability audit

It states what the compiler team should treat as accepted, what remains
implementation-facing rather than surface-defining, and where to look for
construct-local detail.

## Source of Truth

Normative and accepted sources:

- `../regseq-primitive-mapping-framework.md`
- `../primitive-mapping-tracker.md`
- `../construct-elaborations/`
- `../pass1-notes/`
- `../full-device-audit/pl011-representability-audit.md`

This overview is a projection of those files. If this document and the accepted
sources diverge, the accepted sources win and this overview should be patched.

## Scope

Accepted baseline covers:

- all validated constructs used by PL011
- all Pass 1 tasks `P1-01` through `P1-10`
- all E1000-only constructs accepted during the bridge task
- `QG-01` result:
  - `pass_with_recorded_caveats`

## Final Gate State

The current baseline is implementation-ready with explicit carry-forwards:

- no surfaced PL011 element remains uncovered
- no blocking ambiguity remains
- no overfit-risk row remains
- implementation still must honor the shared constraints captured in
  `compiler-constraints.md`

## Core Surface Families

### 1. Declarative metadata

Primary forms:

- `#[rseq::...]` attributes
- helper macros
- typed field declarations

Used for:

- driver identity and compatible matching
- resource declarations
- parameter declarations
- firmware-property declarations
- bound-handler declarations
- register block / bitfield structure

### 2. Trait callback families

Accepted callback families:

- `Lifecycle`
  - `init`, `deinit`
- `PowerManagement`
  - `suspend`, `resume`
- `Serial`
  - per-device-class callback family
- `SerialConsole`
  - optional companion trait for `console_write`
- `Network`
  - per-device-class callback family

Shared accepted callback conventions:

- all trait callbacks use `&mut self`
- `RegseqError` is the shared error type for standard fallible callbacks
- domain-specific returns remain allowed where already accepted:
  - e.g. `TxResult`, `NetStats`
- current compiler baseline should key on the accepted trait family names:
  - `Lifecycle`
  - `PowerManagement`
  - `Serial`
  - `SerialConsole`
  - `Network`
- any future naming polish is a compatibility follow-up, not a blocker for
  current implementation tasking

### 3. Attribute-bound handler methods

Accepted bound-handler baseline:

- used for Interrupt, Timer, and Work handler-shaped behavior
- not traits
- attribute + annotated method model
- single-resource auto-bind is allowed when the resource type is unique
- wrapper context recovery is separate from lock acquisition
- callback-context rules are constrained by the accepted Synchronization model

### 4. Typed-resource wrapper APIs

Accepted wrapper baseline:

- typed struct field is the normal resource declaration
- wrapper methods are the normal operation surface for resource-local behavior
- `&self` is the default receiver convention for wrapper families
- wrapper families include:
  - Register Access
  - Clock
  - ResetLine
  - PinCtrl
  - Synchronization
  - Timer
  - DMA handle surfaces
  - Atomic wrapper surfaces

Notable accepted exceptions:

- `regseq::poll::until(...) -> Result<(), RegseqError>`
  - accepted as a free-function intrinsic for Register Access polling
- explicit MMIO barrier/fence surface
  - excluded from the stable base surface
  - handled instead through the carried-forward MMIO barrier constraint in
    `compiler-constraints.md`

### 5. Compiler-managed primitives

Compiler-managed responsibilities include:

- registration glue
- resource acquisition / release
- generated preambles / postambles
- Layer 1 + Layer 2 joins
- firmware-property extraction
- parameter registration
- handler wrapper generation

Compiler-managed lifecycle participation is already recorded per primitive in
the accepted elaborations.

## Cross-Construct Baselines

### Callback and error conventions

Accepted in `P1-01`:

- shared callback naming/signature/error-model baseline for trait callback
  families
- device trait families aligned across Serial and Network

### Bound-handler model

Accepted in `P1-02`:

- shared bound-handler model for Interrupt and Timer (extended to Work in
  `P1-10`)
- attribute-driven binding with auto-bind for unique resource types

### Synchronization and callback context

Accepted in `P1-03`:

- three callback-entry context classes:
  - `F`: framework-managed entry
  - `S`: subsystem-serialized entry
  - `H`: explicit handler-managed entry
- bound-handler wrappers do not auto-acquire locks
- handler bodies manage lock scopes explicitly

### Wrapper alignment

Accepted in `P1-04`:

- wrapper-family conventions are aligned enough for compiler work
- two accepted Layer 1 / Layer 2 join patterns:
  - explicit attribute join
  - field-name join

### Init-side generated ordering reference

The accepted init-side ordering reference for PL011-class devices is:

| Step | Construct | Generated action |
|---|---|---|
| 1 | Driver Lifecycle | `create_device` + `associate_private_data` |
| 2 | Bus Resource Management | `get_resource_handle` |
| 3 | Register Access | `map_region` |
| 4 | Peripheral clock management | `acquire_clock` + enable |
| 5 | Hardware reset sequencing and post-reset polling | `acquire_reset` + deassert |
| 6 | Pin controller and GPIO configuration | `acquire_pinctrl` + apply default |
| 7 | Firmware property access | `read_property` |
| 8 | Kernel timer lifecycle management | `init_timer` |
| 9 | DMA Management | `alloc_coherent` + `request_channel` |
| 10 | Device interrupt setup and handling | `register_handler` |
| 11 | Driver Lifecycle | `Lifecycle::init()` body |
| 12 | Device interrupt setup and handling | `enable` |

This table is a package-level ordering aid. Construct-local docs still own the
primitive-specific lifecycle tables and lowering details.

### PM ordering and ownership

Accepted in `P1-05`:

- `#[rseq::pm(...)]` is the single PM gate
- stable PM-generated ordering:
  - suspend: driver body, then `Interrupt -> PinCtrl -> Clock`
  - resume: `Clock -> PinCtrl -> Interrupt -> shadow flush`, then driver body
- Timer and DMA remain PM interaction points, not PM-generated lifecycle
  primitives

### Timer lowering

Accepted in `P1-06`:

- stable timer surface:
  - `start(&self)`
  - `start_with(&self, duration: Duration)`
  - `stop(&self)`
- periodic rearm is compiler-generated
- `no_wake` is dropped from the stable Layer 1 surface

### DMA API shape

Accepted in `P1-07`:

- `DmaMapping` is the stable streaming-DMA handle surface
- `channel.configure()` is parameterless and declaration/L2-derived
- `prepare(&DmaMapping)` / `prepare_sg(&[&DmaMapping])` are the stable
  preparation surfaces
- `submit()` merges the validated `channel_submit` and `channel_issue_pending`
  semantic primitives at the stable API layer
- no DMA completion callback surface is part of the stable baseline yet

### Logging, firmware, and delay utilities

Accepted in `P1-08`:

- stable logging surface uses `regseq::log` macros
- firmware value resolution uses a stable three-source model
- `regseq::Duration` is the shared compile-time utility type

## E1000 Extension Surface

Accepted E1000-only constructs:

- Network Interface Management
- Parameter Registration
- Deferred work item scheduling

These extend the same baseline rather than introducing a second model.

## What Is Stable Enough to Implement Now

The compiler team can treat the following as implementation-ready:

- primitive ownership and construct boundaries
- accepted callback families
- accepted bound-handler model
- accepted wrapper-family surface conventions
- accepted PM ordering model
- accepted Timer and DMA stable surfaces
- accepted E1000-only construct additions

## What Is Still Implementation-Facing

These are not surface-definition blockers, but the compiler must account for
them explicitly:

- the shared implementation constraints in `compiler-constraints.md`
- the `QG-01` carried-forward caveats
- deferred deeper DMA callback-model work

## Construct Index

Per-construct compiler docs live in:

- `constructs/`

Each construct document should summarize:

- primitive inventory
- surface forms
- extraction signals
- lowering ownership
- construct-local constraints
- construct-local examples
