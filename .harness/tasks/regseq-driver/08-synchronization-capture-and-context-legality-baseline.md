# Task: regseq-driver-08 - Synchronization capture and context-legality baseline

## Description

Implement the narrow compiler-facing extraction scaffolding for the shared
synchronization surface and the minimum callback-context legality checks needed
before later slices can rely on these wrappers.

## Input Context

Read before starting:

- crates/regseq-driver/src/callbacks.rs
- crates/regseq-driver/src/extract/callback_context.rs
- crates/regseq-driver/src/extract/mod.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/device-interrupt-setup-and-handling.md
- docs/compiler-collateral/constructs/synchronization.md

## Output Specification

Create:

- crates/regseq-driver/src/extract/synchronization.rs
- crates/regseq-driver/tests/synchronization.rs

Modify:

- crates/regseq-driver/src/extract/mod.rs
- crates/regseq-driver/src/callbacks.rs

## Implementation Notes

Recognize and extract only the shared synchronization frontend forms:

- synchronization field types:
  - `Mutex<T>`
  - `SpinLock<T>`
  - `IrqSpinLock<T>`
  - `Completion`
- protected payload types and guard-bearing lock shapes
- wrapper-method call sites:
  - `.lock()`
  - `.wait(timeout)`
  - `.signal()`
  - `.reset()`
- enough guard-scope structure to support later analysis work

The extraction output should remain internal to `regseq-driver`. A narrow
record such as `SynchronizationSurface`, `LockOperation`, `CompletionOperation`,
and `GuardScope` is appropriate.

Bounded legality baseline for this task:

- use the ISR handler identification produced by `regseq-driver-07` to decide
  which method bodies are `H` interrupt-handler context for these checks
- only `IrqSpinLock` lock operations are accepted directly inside
  `#[rseq::isr]` handler bodies
- `Completion::wait(...)` is rejected inside `#[rseq::isr]` handler bodies
- preserve guard-scope structure for later passes, but do not attempt full
  guard/data escape analysis yet
- preserve enough callback-context linkage to extend the checks later when the
  trait callback surfaces land in `phase-b-04`

Do not include:

- subsystem-locking lowering
- full guard-escape proofs
- framework-call-in-guard-scope proofs
- backend lowering or primitive lifecycle behavior

## Test Plan

Create:

- crates/regseq-driver/tests/synchronization.rs

Tests to implement:

1. Verify synchronization field types and payload types are recognized.
2. Verify `.lock()`, `.wait(...)`, `.signal()`, and `.reset()` call sites are
   recognized.
3. Verify direct use of `Mutex` or `SpinLock` lock acquisition inside an ISR
   body is rejected deterministically.
4. Verify `Completion::wait(...)` inside an ISR body is rejected
   deterministically.
5. Verify extracted guard-scope metadata is available for later analysis even
   though full escape checking is deferred.

Test command: `cargo test -p regseq-driver --test synchronization`

## Acceptance Criteria

- `regseq-driver` can reach the accepted synchronization frontend forms
- the bounded ISR-context legality baseline is implemented
- the task stays separate from full synchronization analysis and lowering
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
