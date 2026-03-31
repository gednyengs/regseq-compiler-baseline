# Task: regseq-facade-03 - MMIO resource surface

## Description

Implement the minimal typed MMIO resource shell for the shared Register Access
foundation. This task owns only the `Mmio<T>` resource declaration surface and
its facade exports. It must also define the field-access contract that makes
`Mmio<RegisterBlock>` usable with explicit register-block structs. It must not
pull register support types, helper macros, or polling intrinsics into the
crate yet.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- crates/regseq/README.md
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/register-access.md
- docs/compiler-collateral/constructs/bus-resource-management.md

## Output Specification

Create:

- crates/regseq/src/mmio.rs
- crates/regseq/tests/mmio.rs

Modify:

- crates/regseq/src/lib.rs

## Implementation Notes

Add the stable typed MMIO declaration surface:

```rust
pub struct Mmio<T> {
    /* private fields */
}
```

and the field-access contract:

```rust
impl<T> Deref for Mmio<T> {
    type Target = T;
    /* ... */
}
```

Phase-B baseline for this task:

- keep `Mmio<T>` as a small opaque facade resource type
- make `Mmio<T>` implement `Deref<Target = T>` so authored code such as
  `self.regs.lcr_h.modify(...)` type-checks against explicit register-block
  structs
- derive only the traits that are uncontroversial for a typed field carrier
- do not add mapping logic, raw pointers, or backend-facing operations
- do not add register access methods here
- do not add a Rust trait bound on `T`; validity of `T` as a register block is
  a compiler-side extraction check, not a facade trait contract

This task establishes only the resource declaration shell needed by later
register support, polling, and extraction tasks.

## Test Plan

Create:

- crates/regseq/tests/mmio.rs

Tests to implement:

1. Verify `regseq::Mmio<T>` is publicly exported.
2. Verify a driver-shaped struct field such as `regs: Mmio<MyRegs>` compiles.
3. Verify `Mmio<T>` supports field access through `Deref<Target = T>`.
4. Verify the type stays opaque and does not require backend/runtime support.

Test command: `cargo test -p regseq --test mmio`

## Acceptance Criteria

- `regseq` exports `Mmio<T>`
- the task stays limited to the MMIO resource shell
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
