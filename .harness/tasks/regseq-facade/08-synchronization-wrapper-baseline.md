# Task: regseq-facade-08 - Synchronization wrapper baseline

## Description

Implement the shared synchronization wrapper baseline used across later frontend
slices. This task owns the lock/completion wrapper shells, the single generic
guard surface, and their stable method signatures.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- crates/regseq/src/error.rs
- crates/regseq/src/time.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/synchronization.md
- docs/discussions/regseq-error/regseq-error-spec.md

## Output Specification

Create:

- crates/regseq/src/sync.rs
- crates/regseq/tests/synchronization.rs

Modify:

- crates/regseq/src/lib.rs

## Implementation Notes

Add the accepted shared synchronization surface:

```rust
pub struct Mutex<T> { /* opaque */ }
pub struct SpinLock<T> { /* opaque */ }
pub struct IrqSpinLock<T> { /* opaque */ }
pub struct Completion { /* opaque */ }
pub struct Guard<'a, T> { /* opaque */ }

impl<T> Mutex<T> {
    pub fn lock(&self) -> Guard<'_, T> { unimplemented!() }
}

impl<T> SpinLock<T> {
    pub fn lock(&self) -> Guard<'_, T> { unimplemented!() }
}

impl<T> IrqSpinLock<T> {
    pub fn lock(&self) -> Guard<'_, T> { unimplemented!() }
}

impl Completion {
    pub fn wait(&self, timeout: Duration) -> Result<(), RegseqError> {
        unimplemented!()
    }

    pub fn signal(&self) { unimplemented!() }
    pub fn reset(&self) { unimplemented!() }
}
```

Phase-B baseline:

- `Guard<'a, T>` is the single stable guard type
- `Guard<'a, T>` should provide the ordinary access ergonomics needed by later
  authored code, namely `Deref<Target = T>` and `DerefMut`
- lock acquisition uses `&self`
- keep synchronization surfaces type-driven
- keep method bodies as stubs only
- keep `Completion::wait` on the shared `Result<(), RegseqError>` baseline

Do not include:

- `try_lock`
- backend backing-state layout commitments
- interrupt-specific legality checks
- backend lowering or compiler-managed init/destroy behavior

## Test Plan

Create:

- crates/regseq/tests/synchronization.rs

Tests to implement:

1. Verify `Mutex<T>`, `SpinLock<T>`, `IrqSpinLock<T>`, `Completion`, and
   `Guard<'_, T>` are publicly available from `regseq`.
2. Verify the accepted `.lock()` and `Completion::{wait, signal, reset}`
   signatures compile.
3. Verify `Guard<'_, T>` supports ordinary dereference access expected by later
   authored code.
4. Verify `Completion::wait` uses the current `Duration` and `RegseqError`
   baselines.

Test command: `cargo test -p regseq --test synchronization`

## Acceptance Criteria

- the accepted shared synchronization wrapper shells exist
- the single generic guard surface exists with ordinary dereference behavior
- the task stays limited to wrapper/type shape and signatures
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
