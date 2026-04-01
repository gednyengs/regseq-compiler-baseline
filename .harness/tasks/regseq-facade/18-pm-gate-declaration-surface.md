# Task: regseq-facade-18 - PM gate declaration surface

## Description

Implement the facade-side declaration baseline for the accepted
 power-management gate metadata. This task exists to make the PM declaration
 shape authorable in a `regseq`-using crate without reopening the
 `PowerManagement` trait API or inventing any new wrapper surface.

This is a declaration-surface task, not a callback-trait task.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- crates/regseq/src/power.rs
- crates/regseq/tests/power.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/device-power-state-management.md

## Output Specification

Create:

- crates/regseq/tests/pm.rs

Modify only if needed:

- crates/regseq/src/power.rs
- crates/regseq/src/lib.rs

## Implementation Notes

The accepted PM declaration gate is:

```rust
#[rseq::pm(idle = false, wakeup_capable = false)]
struct Pl011 {
    // resources
}
```

Important boundary for this task:

- `#[rseq::pm(...)]` is a compiler-recognized inert tool attribute
- it is not a proc macro
- it is not a wrapper API
- it is not a replacement for the already-established `PowerManagement` trait
- this task must not redesign `PowerManagement::{suspend,resume}`

Phase-B declaration-side baseline to support:

- `#[rseq::pm(idle = ...)]`
- `#[rseq::pm(wakeup_capable = ...)]`
- combined use in one attribute

Use the same integration-test pattern as other declaration-surface tasks:

```rust
#![feature(register_tool)]
#![register_tool(rseq)]
```

Keep this task bounded:

- do not introduce any PM wrapper/resource type
- do not introduce PM registration APIs
- do not encode PM ordering/lowering
- do not change the `PowerManagement` trait signatures
- do not require the trait from the facade layer alone; declaration-side
  enforcement belongs in `regseq-driver`

If a small doc clarification in `crates/regseq/src/power.rs` helps make the
 gate-vs-trait split explicit, that is acceptable, but the task should remain
 primarily test-driven.

## Test Plan

Create:

- crates/regseq/tests/pm.rs

Tests to implement:

1. Verify representative `#[rseq::pm(...)]` declarations compile on a driver
   type using:
   - `idle = false`
   - `wakeup_capable = false`
2. Verify a driver type can combine `#[rseq::pm(...)]` with a valid
   `impl PowerManagement for Driver`.
3. Verify the declaration surface does not introduce any new PM wrapper or
   helper API beyond the existing `PowerManagement` trait.
4. Verify the existing `PowerManagement` trait remains the callback-family
   surface while `#[rseq::pm(...)]` remains declarative metadata.

Test command: `cargo test -p regseq --test pm`

## Acceptance Criteria

- the accepted PM gate declaration shape is authorable from a `regseq`-using
  crate
- the task does not reopen the callback-trait API or invent PM wrapper
  surfaces
- the gate-vs-trait split remains explicit
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
