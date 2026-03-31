# Task: regseq-facade-07 - Interrupt resource and dispatch surface

## Description

Implement the shared interrupt facade surface needed before bound-handler
capture can be authored cleanly. This task owns the typed `Interrupt` resource
shell. `IrqResult` is already established by `regseq-facade-02`; this task
verifies that baseline remains aligned with the accepted dispatch contract.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- crates/regseq/src/irq.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/device-interrupt-setup-and-handling.md

## Output Specification

Create:

- crates/regseq/tests/interrupt.rs

Modify:

- crates/regseq/src/irq.rs
- crates/regseq/src/lib.rs

## Implementation Notes

Extend the existing interrupt module with the typed resource shell. `IrqResult`
already exists in `crates/regseq/src/irq.rs`; this task adds `Interrupt`
alongside it.

```rust
pub struct Interrupt {
    // opaque resource shell
}
```

Phase-B baseline:

- `Interrupt` is a typed resource shell only
- keep `Interrupt` free of driver-callable wrapper methods
- `Option<Interrupt>` and `[Interrupt; N]` remain valid declaration shapes
  through ordinary Rust type usage; no extra wrapper APIs are needed
- `IrqResult` remains a dispatch result, not a semantic error carrier

Do not include:

- line-level `enable` / `disable`
- registration glue
- vector allocation
- lowering behavior

## Test Plan

Create:

- crates/regseq/tests/interrupt.rs

Tests to implement:

1. Verify `Interrupt` is publicly available from `regseq`.
2. Verify `Option<Interrupt>` and `[Interrupt; N]` compile as resource field
   shapes.
3. Verify the existing `IrqResult` baseline still exposes the accepted
   `Handled`, `NotHandled`, and `DeferWork` variants.
4. Verify `Interrupt` itself exposes no driver-callable wrapper methods.

Test command: `cargo test -p regseq --test interrupt`

## Acceptance Criteria

- `Interrupt` exists as an explicit shared facade resource shell
- the existing `IrqResult` baseline remains aligned with the accepted dispatch
  contract
- the task stays separate from compiler-managed interrupt operations
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
