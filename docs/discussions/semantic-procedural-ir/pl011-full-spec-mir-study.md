# PL011 Full-Spec MIR Study

## Purpose

This note captures a curated set of real MIR patterns from the current
`pl011-full-spec` fixture.

The goal is to anchor semantic-procedural-IR design in:

- actual post-drop-elaboration MIR as captured by Regseq
- actual helper/callback/ISR bodies from the current driver spec
- recurring patterns that the semantic IR must absorb before backend emission

This note is descriptive, not normative. It records what the current compiler
captures so we can reason about the next IR boundary from evidence.

## Source Of Truth

The current captured MIR comes from:

- [`tests/fixtures/pl011-full-spec/expected/raw-frontend.json`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/expected/raw-frontend.json)

The authored driver source is:

- [`tests/fixtures/pl011-full-spec/src/driver.rs`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs)

Within the raw artifact, executable bodies currently live under:

- `bodies.functions`

Each function entry contains:

- `name`
- `def_path`
- `kind`
- `receiver`
- `params`
- `return_type`
- `body`
- `helper_calls`

The `body` contains:

- `local_decls`
- `arg_count`
- `basic_blocks`
- `source_scopes`

## Curated Function Set

This study focuses on the following authored bodies because they exercise the
main MIR shapes we care about for semantic-IR design:

- `program_polling_port`
- `set_baud_rate`
- `uart_clock_hz`
- `enable_clock_if_present`
- `toggle_reset_if_present`
- `clear_explicit_interrupts`
- `service_rx_fifo`
- `service_tx_fifo`
- `init`
- `handle_irq`

## High-Level Pattern Inventory

Across the curated set, the current captured MIR prominently uses:

- explicit locals for params, temporaries, return place, and intermediates
- `Call` terminators for:
  - wrapper operations
  - helper calls
  - trait calls
  - structural `core::*` calls
  - `Try` machinery
- `SwitchInt` for:
  - `Option` discrimination
  - `Result` / `Try` control flow
  - boolean predicates
  - interrupt-mask tests
- `BinaryOp` and `Cast` assignments for arithmetic and bit tests
- `Ref` and `Deref` structure around:
  - `self`
  - `Mmio`
  - register wrappers
  - optional resources
- unevaluated constants in places where authored associated consts are used
- helper-call graphs that are directly visible in MIR call sites

## Function Studies

### `program_polling_port`

Authored source intent:

- call a sequence of helpers bracketed by `ordered! { ... }`

Observed MIR call sequence:

- `driver::Pl011FullSpec::disable_uart`
- `driver::Pl011FullSpec::disable_fifo`
- `regseq::intrinsics::sync::memory_barrier`
- `driver::Pl011FullSpec::set_baud_rate`
- `driver::Pl011FullSpec::configure_line_control`
- `driver::Pl011FullSpec::configure_interrupt_levels`
- `driver::Pl011FullSpec::mask_all_interrupts`
- `driver::Pl011FullSpec::clear_all_interrupts`
- `driver::Pl011FullSpec::enable_uart`
- `driver::Pl011FullSpec::enable_rx_interrupts`
- `regseq::intrinsics::sync::memory_barrier`

Main MIR shape:

- mostly linear call blocks
- paired barrier calls around the ordered region
- explicit helper-call graph visible in `helper_calls`

Semantic-IR implication:

- `ordered!` does not survive as a node
- the semantic IR must reconstruct an ordered region from barrier-bracketed
  call sequences

### `set_baud_rate`

Authored source intent:

- arithmetic on `uart_clk_hz` and `baud_rate`
- derive `ibrd` and `fbrd`
- write field values through register wrappers

Observed MIR call sequence:

- `core::ops::Deref::deref`
- `driver::field_val`
- `regseq::ReadWrite::<T, R>::write`
- `core::ops::Deref::deref`
- `driver::field_val`
- `regseq::ReadWrite::<T, R>::write`

Observed MIR structure:

- 32 locals and 10 basic blocks
- many arithmetic temporaries:
  - `U64` widening
  - shifts
  - masks
  - casts back to `u32`
- wrapper access requires:
  - deref of `Mmio`
  - field projection to register wrapper
  - field-value helper construction
  - wrapper write call

Semantic-IR implication:

- arithmetic temporaries are real semantic temps, not just noise
- register-write lowering needs to absorb:
  - wrapper deref
  - field-value constructor
  - wrapper write
into a clean register-write semantic form

### `uart_clock_hz`

Authored source intent:

- `self.clock.as_ref().map(Clock::get_rate).unwrap_or(DEFAULT_UART_CLOCK_HZ)`

Observed MIR call sequence:

- `core::option::Option::<T>::as_ref`
- `core::option::Option::<T>::map`
- `core::option::Option::<T>::unwrap_or`

Observed MIR structure:

- 5 locals, 4 blocks
- local 4 takes `&self.clock`
- `as_ref` produces an `Option<&Clock>`
- `map` takes `Clock::get_rate`
- `unwrap_or` uses:
  - destination local `_0`
  - unevaluated associated const:
    - `unevaluated:driver::Pl011FullSpec::DEFAULT_UART_CLOCK_HZ`

Semantic-IR implication:

- this is a canonical example of a value-producing helper body that must not be
  handled as a simple statement list
- the semantic IR needs:
  - optional/resource-read-aware value expressions
  - real return-with-value semantics
  - a strategy for unevaluated associated consts

### `enable_clock_if_present`

Authored source intent:

- if `self.clock` is `Some(clock)`, call `clock.enable()?`
- then `Ok(())`

Observed MIR call sequence:

- `regseq::Clock::enable`
- `core::ops::Try::branch`
- `core::ops::FromResidual::from_residual`

Observed MIR structure:

- discriminant on `Option<Clock>`
- `SwitchInt` over `Some`/`None`
- inside `Some` arm:
  - `Clock::enable`
  - `Try::branch`
  - `ControlFlow`/`Result` branching
  - residual conversion on error path

Semantic-IR implication:

- optional-resource gating and `?` lowering are separate concerns
- semantic IR must preserve:
  - optional resource condition
  - resource operation
  - fallible control flow
  - successful unit return

### `toggle_reset_if_present`

Authored source intent:

- if `self.reset` is `Some(reset)`, call `reset.toggle(Duration::from_us(10))?`

Observed MIR call sequence:

- `regseq::Duration::from_us`
- `regseq::ResetLine::toggle`
- `core::ops::Try::branch`
- `core::ops::FromResidual::from_residual`

Observed MIR structure:

- `Option<ResetLine>` discrimination by `SwitchInt`
- duration constructor as an explicit `Call`
- resource op followed by `Try` machinery

Semantic-IR implication:

- duration constructors are visible MIR calls, not hidden constants
- semantic IR must decide whether duration constructors are:
  - normalized into delay/resource parameters
  - or retained as portable value expressions

### `clear_explicit_interrupts`

Authored source intent:

- `clear_mask = status & EXPLICIT_ISR_CLEAR_MASK`
- if nonzero, write that mask to `ICR`

Observed MIR call sequence:

- `core::ops::Deref::deref`
- `driver::raw_reg_val`
- `regseq::WriteOnly::<T, R>::write`

Observed MIR structure:

- `BinaryOp(BitAnd)` using associated const
- `SwitchInt` for `clear_mask != 0`
- wrapper write through `raw_reg_val`

Semantic-IR implication:

- this is the minimal canonical pattern for:
  - interrupt-class clear
  - conditional register write
- semantic IR should capture:
  - status-source value
  - class mask or clear mask
  - destination clear register
  - conditional emission boundary

### `service_rx_fifo`

Authored source intent:

- loop while `rx_char().is_some()`
- set `handled = true`

Observed MIR call sequence:

- `regseq::Serial::rx_char`
- `core::option::Option::<T>::is_some`

Observed MIR structure:

- helper returns `bool`
- local `handled` initialized to false
- repeated call-and-test pattern
- `SwitchInt` loop test over `Option::is_some`

Semantic-IR implication:

- semantic IR needs a loop model that can carry:
  - helper predicate call
  - loop-carried local update
  - final bool return

### `service_tx_fifo`

Authored source intent:

- if `pending_tx` is `Some(byte)`:
  - if `tx_ready()`:
    - write byte to `DR`
    - clear `pending_tx`
    - disable tx interrupts
    - return true
  - return false
- else:
  - disable tx interrupts
  - return true

Observed MIR call sequence:

- `regseq::Serial::tx_ready`
- `core::ops::Deref::deref`
- `driver::field_val`
- `regseq::ReadWrite::<T, R>::write`
- `driver::Pl011FullSpec::disable_tx_interrupts`
- `driver::Pl011FullSpec::disable_tx_interrupts`

Observed MIR structure:

- `Option<u8>` discriminant on `pending_tx`
- nested branch on trait call `tx_ready`
- cast `u8 -> u32`
- field-value helper and wrapper write
- assignment of `pending_tx = None`
- duplicate helper call in separate branches

Semantic-IR implication:

- semantic IR must distinguish:
  - helper calls
  - trait/device-class calls
  - register wrapper calls
  - option-state updates
- this function is a good anchor for local/value naming policy because it uses:
  - option payload extraction
  - arithmetic cast temp
  - helper call branch
  - field assignment to an authored driver field

### `init`

Authored source intent:

- call several fallible helpers with `?`
- call `program_polling_port(self.uart_clock_hz(), self.current_speed)`
- return `Ok(())`

Observed MIR call sequence:

- `driver::Pl011FullSpec::toggle_reset_if_present`
- `core::ops::Try::branch`
- `driver::Pl011FullSpec::apply_default_pins_if_present`
- `core::ops::FromResidual::from_residual`
- `core::ops::Try::branch`
- `driver::Pl011FullSpec::enable_clock_if_present`
- `core::ops::FromResidual::from_residual`
- `core::ops::Try::branch`
- `driver::Pl011FullSpec::uart_clock_hz`
- `core::ops::FromResidual::from_residual`
- `driver::Pl011FullSpec::program_polling_port`

Observed MIR structure:

- 35 locals and 20 blocks
- repeated pattern:
  - helper call
  - `Try::branch`
  - discriminant/`SwitchInt`
  - break/continue path via `ControlFlow`
  - `FromResidual` on error path
- helper result/value feed into later call arguments

Semantic-IR implication:

- this is the canonical full lifecycle example for:
  - helper sequencing
  - fallible helper chaining
  - cleanup/error propagation shape
  - nested helper/value flow

### `handle_irq`

Authored source intent:

- read raw MIS status
- if zero, return `IrqResult::None`
- test RX/TX/error masks
- service RX/TX helpers as appropriate
- clear explicit interrupt sources
- return the accumulated `IrqResult`

Observed MIR call sequence:

- `core::ops::Deref::deref`
- `regseq::ReadOnly::<T, R>::read_raw`
- `driver::Pl011FullSpec::service_rx_fifo`
- `driver::Pl011FullSpec::service_tx_fifo`
- `driver::Pl011FullSpec::clear_explicit_interrupts`

Observed MIR structure:

- raw register-read path through `Mmio` deref and `ReadOnly::read_raw`
- early zero-status test (`status == 0`)
- bitmask tests using `BinaryOp(BitAnd)` with unevaluated associated consts:
  - `RX_INTERRUPT_MASK`
  - `TX_INTERRUPT_MASK`
  - `EXPLICIT_ISR_CLEAR_MASK`
- helper-call branches for RX/TX servicing
- accumulated local result updated with `BitOr`
- final `IrqResult` aggregate construction

Semantic-IR implication:

- ISR lowering cannot be a generic opaque branch tree
- semantic IR needs explicit constructs for:
  - raw interrupt-status source
  - interrupt-class tests
  - interrupt-class clear behavior
  - accumulated IRQ-result state
  - helper side effects within ISR branches

## Derived MIR Shape Families

The curated functions expose a small number of important recurring MIR families.

### 1. Wrapper-Based Register Access

Observed forms:

- `Deref(Mmio) -> register field ref -> wrapper call`
- `field_val(...)` or `raw_reg_val(...)`
- `ReadOnly/WriteOnly/ReadWrite::<T, R>::{read_raw, read, write, modify}`

Implication:

- wrapper-specific Rust paths should not survive beyond semantic elaboration
- semantic IR should recognize these as canonical register operations

### 2. Optional Resource Gating

Observed forms:

- `Ref` to `Option<Resource>`
- `Discriminant`
- `SwitchInt`
- downcast/projection into `Some`
- resource method call in the `Some` arm

Implication:

- semantic IR should have an explicit optional-resource control-flow/value model

### 3. `?` / Fallible Control Flow

Observed forms:

- helper or resource call returning `Result`
- `Try::branch`
- discriminant or control-flow split
- `FromResidual::from_residual`

Implication:

- semantic IR must preserve fallible operations as first-class semantic nodes,
  not merely branch syntax

### 4. Value-Producing `Option` Chains

Observed forms:

- `as_ref`
- `map`
- `unwrap_or`
- associated const default

Implication:

- semantic IR needs a real value-expression model, not just action statements

### 5. Arithmetic / Cast Temporaries

Observed forms:

- `BinaryOp`
- `Cast`
- shift/mask arithmetic
- integer widening/narrowing

Implication:

- semantic IR must give temporaries stable semantic identities
- these cannot be treated as mere formatting artifacts

### 6. Helper-Call CFG Structure

Observed forms:

- direct helper calls
- helper-call graphs visible from MIR call terminators
- helper sequencing mixed with fallibility and value flow

Implication:

- helper calls must remain explicit semantic nodes
- helper-call arguments and return flow must remain explicit

### 7. Ordered Region By Barrier Pair

Observed forms:

- `regseq::intrinsics::sync::memory_barrier`
- barrier before and after a helper sequence

Implication:

- semantic IR should recover an ordered-region concept from barrier pairs

### 8. ISR-Specific Bitmask Logic

Observed forms:

- raw status read
- zero-status early return
- `BitAnd` tests against associated const masks
- helper calls in per-class branches
- explicit clear helper
- accumulated IRQ-result local

Implication:

- semantic IR likely needs ISR-aware nodes or at least well-defined semantic
  patterns for interrupt-class discrimination and result accumulation

## Immediate Design Pressure On Semantic Procedural IR

From the current MIR evidence, the backend-facing semantic IR must be able to
model at least:

- locals and params with stable semantic identity
- return-with-value
- helper calls with explicit args
- register ops
- resource ops
- resource reads
- option-based value chains
- fallible operations
- branches and loops
- ordered regions
- interrupt-class tests and clears
- driver-field assignment to authored fields such as `pending_tx`
- associated-const references in a normalized backend-safe form

## Recommended Next Discussion Step

Use this MIR study alongside:

- [`semantic-procedural-ir-discussion.md`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/semantic-procedural-ir/semantic-procedural-ir-discussion.md)

to decide:

1. whether the existing `BehaviorPlan` family should be extended into the full
   semantic procedural IR
2. what new value/type/local constructs are required
3. which MIR families must be normalized before backend planning begins

This note should be updated if the captured MIR query or fixture changes
substantially.
