# Task: regseq-driver-06 - Register/MMIO frontend capture scaffolding

## Description

Implement the narrow compiler-facing extraction scaffolding for the shared
Register Access foundation. This task should prove `regseq-driver` can reach
the accepted register/MMIO frontend forms without introducing normalized model
logic or lowering behavior.

## Input Context

Read before starting:

- crates/regseq-driver/src/callbacks.rs
- crates/regseq-driver/src/extract/mod.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/compiler-constraints.md
- docs/compiler-collateral/constructs/register-access.md
- docs/compiler-collateral/constructs/serial-uart-management.md

## Output Specification

Create:

- crates/regseq-driver/src/extract/register_access.rs
- crates/regseq-driver/tests/register_access.rs

Modify:

- crates/regseq-driver/src/extract/mod.rs
- crates/regseq-driver/src/callbacks.rs

## Implementation Notes

Recognize and extract only the shared register/MMIO frontend forms:

- `Mmio<T>` typed fields
- register-block declarations
- accepted register metadata attributes that belong to the register model
- wrapper-method call sites on known register wrapper types:
  - `read`
  - `write`
  - `modify`
  - `set`
- `regseq::poll::until(...)` call sites

For metadata reachability in this scaffolding task, cover the accepted
register-model properties:

- `access_width`
- `endian`
- `side_effects`
- `hw_updates`

And recover the generated register-descriptor facts needed for later analysis:

- register descriptor identity such as `FR_REG::Register`
- field offsets and widths from generated field constants
- named values from generated `FieldValue` constants

The extraction work may use HIR-visible macro expansion products and `TyCtxt`
queries where needed. This task should not assume the macro output is opaque.

Keep the output representation internal to `regseq-driver`. A narrow internal
record such as `RegisterAccessSurface` or split helper records is appropriate.

Do not include:

- normalized model structures
- backend lowering
- `regseq::ordered!`
- synchronization or interrupt capture

## Test Plan

Create:

- crates/regseq-driver/tests/register_access.rs

Tests to implement:

1. Verify a small fixture crate with `Mmio<T>` and a register-block declaration
   is recognized by the extraction helper.
2. Verify `regseq::poll::until(...)` call sites are recognized.
3. Verify wrapper-method call sites on known register wrapper types are
   recognized.
4. Verify generated register-descriptor facts such as field offsets/widths or
   named values are reachable from extraction.
5. Verify malformed or incomplete register/MMIO declarations are rejected
   deterministically.

Test command: `cargo test -p regseq-driver --test register_access`

## Acceptance Criteria

- `regseq-driver` can reach the accepted register/MMIO frontend forms
- the task stays limited to capture scaffolding
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
