# Task: regseq-facade-04 - Register support types and access wrappers

## Description

Implement the typed register-support surface that backs the accepted Register
Access wrapper baseline. This task owns the core support types such as
`ReadOnly`, `WriteOnly`, `ReadWrite`, `Reserved`, `Field`, and `FieldValue`,
along with the stable wrapper-method signatures for register-local read/write
operations and the composition contract needed by named register values.

## Input Context

Read before starting:

- crates/regseq/src/lib.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/register-access.md
- docs/compiler-collateral/constructs/serial-uart-management.md

## Output Specification

Create:

- crates/regseq/src/registers.rs
- crates/regseq/tests/register_types.rs

Modify:

- crates/regseq/src/lib.rs

## Implementation Notes

Add the stable support-type baseline needed for authored register access:

- `ReadOnly<T, R>`
- `WriteOnly<T, R>`
- `ReadWrite<T, R>`
- `Reserved<T>`
- `Field<T, R>`
- `FieldValue<T, R>`

Keep width explicit in the wrapper field types and support types. The accepted
baseline for this phase is the two-parameter wrapper form:

```rust
pub fr: ReadOnly<u32, FR_REG::Register>
```

not a collapsed single-parameter wrapper.

And the wrapper-method shape accepted in the collateral:

```rust
impl<T, R> ReadOnly<T, R> {
    fn read(&self, field: Field<T, R>) -> T;
}

impl<T, R> WriteOnly<T, R> {
    fn write(&self, values: FieldValue<T, R>);
    fn set(&self, values: FieldValue<T, R>);
}

impl<T, R> ReadWrite<T, R> {
    fn read(&self, field: Field<T, R>) -> T;
    fn write(&self, values: FieldValue<T, R>);
    fn modify(&self, values: FieldValue<T, R>);
    fn set(&self, values: FieldValue<T, R>);
}
```

Support-type contract for this phase:

- `Field<T, R>` is a const-constructible field descriptor
- `FieldValue<T, R>` is a const-constructible write-value carrier
- `FieldValue<T, R>` must support composition, for example through
  `Add<Output = Self>`, so authored code like
  `LCRH_REG::WLEN::WLEN_8 + LCRH_REG::FEN::SET` can type-check
- `Reserved<T>` and the register wrapper carriers should remain layout-sized so
  explicit `#[repr(C)]` register blocks stay mechanically coherent

Phase-B baseline for this task:

- keep these as compile-time facade types and method stubs only
- use `unimplemented!()` or equivalent non-runtime placeholder bodies where
  necessary
- do not add backend behavior, MMIO mapping, or macro expansion logic
- do not add `regseq::ordered!`

## Test Plan

Create:

- crates/regseq/tests/register_types.rs

Tests to implement:

1. Verify the support types are exported from `regseq`.
2. Verify the accepted per-type wrapper method names and signatures compile on
   `ReadOnly<T, R>`, `WriteOnly<T, R>`, and `ReadWrite<T, R>`.
3. Verify `ReadOnly`, `WriteOnly`, and `ReadWrite` remain distinct types for
   later access-legality checking.
4. Verify `Field` and `FieldValue` can be constructed in const contexts.
5. Verify `FieldValue` composition works for the accepted named-value style.

Test command: `cargo test -p regseq --test register_types`

## Acceptance Criteria

- `regseq` exports the accepted register support types
- the accepted wrapper-method surface compiles
- the task stays limited to support types and wrapper signatures
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
