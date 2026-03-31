# Task: regseq-macros-02 - Register bitfields helper macro

## Description

Implement `register_bitfields!` as the explicit helper macro surface for the
shared Register Access foundation. This task owns only the helper macro and its
proc-macro wiring. It must not claim ownership of inert `#[rseq::...]`
attributes.

## Input Context

Read before starting:

- crates/regseq-macros/src/lib.rs
- crates/regseq-macros/README.md
- docs/compiler-collateral/constructs/register-access.md
- docs/compiler-collateral/constructs/serial-uart-management.md

## Output Specification

Create:

- crates/regseq-macros/src/register_bitfields.rs
- crates/regseq-macros/tests/register_bitfields.rs

Modify:

- crates/regseq-macros/src/lib.rs

## Implementation Notes

This task should establish the helper macro surface and enough expansion shape
for later register-model and extraction work to build on. Keep the scope
bounded:

- own `register_bitfields!` explicitly
- do not add `regseq::ordered!` in this batch
- do not add ownership of inert `#[rseq::...]` attributes
- do not mix backend lowering logic into the macro crate

The tests should prove the macro can define a register-layout surface suitable
for later `Mmio<T>` and wrapper tasks.

Representative minimum expectation:

```rust
register_bitfields! [
    u32,

    DR_REG [
        DATA OFFSET(0) NUMBITS(8) []
    ],

    FR_REG [
        TXFF OFFSET(5) NUMBITS(1) [],
        BUSY OFFSET(3) NUMBITS(1) []
    ]
];
```

The macro should expand into HIR-visible Rust items that define register
descriptor schemas, not full MMIO blocks. At minimum, for a register like
`FR_REG`, the generated surface should include:

- `FR_REG::Register` as a distinct marker type
- `FR_REG::BUSY::FIELD` as a `Field<u32, FR_REG::Register>` constant
- named value constants such as `LCRH_REG::WLEN::WLEN_8` as
  `FieldValue<u32, LCRH_REG::Register>` constants

The exact internal expansion strategy can vary, but the generated items must:

- be usable by the facade-side register support types from `regseq-facade-04`
- remain structurally predictable for later driver extraction work
- not hide the register block itself; the explicit `#[rseq::register_block]`
  struct remains the authored layout surface

Clarification on ownership:

- `register_bitfields!` is generative
- `#[rseq::register_block(...)]` and `#[rseq::register(...)]` remain inert
  compiler-recognized metadata and are not owned by this crate

## Test Plan

Create:

- crates/regseq-macros/tests/register_bitfields.rs

Tests to implement:

1. Verify `register_bitfields!` is exported by `regseq-macros`.
2. Verify a representative register-layout declaration compiles.
3. Verify the expansion shape is usable by the facade-side register support
   types in later tasks and can serve as the `T` in `Mmio<T>`.
4. Verify the generated marker types, field constants, and named value
   constants are accessible in authored code.

Test command: `cargo test -p regseq-macros --test register_bitfields`

## Acceptance Criteria

- `register_bitfields!` exists as an explicit proc-macro surface
- the task stays separate from inert tool-attribute ownership
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
