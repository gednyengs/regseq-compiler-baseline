# Task: regseq-driver-05 - Callback-context baseline scaffolding

## Description

Implement the canonical internal baseline for callback-entry contexts used by
later frontend capture tasks:

- `F`: framework-managed entry
- `S`: subsystem-serialized entry
- `H`: explicit handler-managed entry

This task is intentionally narrow. It does not yet capture `Lifecycle`,
`PowerManagement`, `Serial`, or `#[rseq::isr]` surfaces directly. Instead, it
creates the single internal representation and helper APIs that later trait and
bound-handler extraction tasks will rely on.

## Input Context

Read before starting:

- crates/regseq-driver/src/lib.rs
- crates/regseq-driver/src/extract/mod.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/compiler-constraints.md
- docs/compiler-collateral/constructs/driver-lifecycle.md
- docs/compiler-collateral/constructs/device-interrupt-setup-and-handling.md

## Output Specification

Create:

- crates/regseq-driver/src/extract/callback_context.rs
- crates/regseq-driver/tests/callback_context.rs

Modify:

- crates/regseq-driver/src/extract/mod.rs
- crates/regseq-driver/src/lib.rs

## Implementation Notes

### Canonical internal type

Add one internal enum or equivalent canonical representation for callback-entry
contexts. The representation must:

- preserve the three accepted classes `F`, `S`, and `H`
- expose a stable short-code view usable by later extraction and snapshot work
- avoid embedding trait-specific or handler-specific capture logic in this task

An internal shape like the following is acceptable:

```rust
pub enum CallbackContext {
    FrameworkManaged,
    SubsystemSerialized,
    HandlerManaged,
}
```

with explicit helpers that map to the accepted short labels `F`, `S`, and `H`.

### Scope boundary

This task should define the baseline model and helper APIs only. It must not:

- capture `Lifecycle` impls
- capture `#[rseq::isr]` bindings
- enforce synchronization legality rules beyond the shared context split
- introduce raw-artifact serialization

## Test Plan

Create:

- crates/regseq-driver/tests/callback_context.rs

Tests to implement:

1. Verify the canonical internal type round-trips through the accepted short
   labels `F`, `S`, and `H`.
2. Verify the helper APIs expose deterministic names suitable for later
   diagnostics and snapshots.
3. Verify the module can be imported from later extraction code without
   redefining the context model.

Test command: `cargo test -p regseq-driver --test callback_context`

## Acceptance Criteria

- `regseq-driver` has exactly one canonical internal callback-context baseline
- the baseline preserves the accepted `F` / `S` / `H` split
- the task stays limited to scaffolding, not full trait or handler extraction
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes

