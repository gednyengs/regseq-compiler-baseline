# regseq-macros-04: register_block! proc macro

## Description

Implement the `register_block!` proc macro that replaces both
`register_bitfields!` and the manual `#[repr(C)]` struct with a single
unified definition producing a scoped module.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 3.6)
- `docs/discussions/models/regseq-model/spec.md` (Section 10.2, register block definition)
- `crates/regseq-macros/src/lib.rs`
- `crates/regseq-macros/src/register_bitfields.rs` (reference for field syntax)
- `crates/regseq/src/registers.rs` (RegisterOf, Field, FieldValue, RegisterId types)
- `crates/regseq/src/mmio.rs` (Mmio methods from regseq-facade-23, needed for type-check tests)

## Implementation Notes

### Macro syntax

```rust
regseq::register_block! {
    #[endian = "little", access_width = 32]
    pub block_name {
        reg_name: AccessKind {
            FIELD_NAME OFFSET(n) NUMBITS(n) [],
            FIELD_NAME OFFSET(n) NUMBITS(n) [VALUE_A = 0, VALUE_B = 1],
        },
        _reserved0: Reserved<u32> [count],
        // ...
    }
}
```

`AccessKind` is one of `ReadOnly`, `ReadWrite`, `WriteOnly`.
Reserved fields use the syntax `_reserved_N: Reserved<T> [count]` and
generate a `[Reserved<T>; count]` array gap in the layout struct.

### Generated output

For `register_block! { ... pub pl011 { dr: ReadWrite { ... }, fr: ReadOnly { ... }, ... } }`:

```rust
pub mod pl011 {
    // Layout struct
    #[repr(C)]
    #[rseq::register_block(endian = "little", access_width = 32)]
    pub struct Regs {
        pub dr: ReadWrite<u32, dr::Register>,
        pub fr: ReadOnly<u32, fr::Register>,
        // ...
    }

    // Per-register module
    pub mod dr {
        /// Unique marker type for this register.
        pub enum Register {}

        /// Byte offset within the block.
        pub const OFFSET: u32 = 0;

        /// RegisterId for full-register operations.
        pub const REG: RegisterId<u32, Register> = RegisterId::new();

        // Field constants
        pub const DATA: Field<u32, Register> = Field::new(0x000000ff, 0);
        pub const OE: Field<u32, Register> = Field::new(0x00000800, 11);

        // Named values as FieldValue constants (per-field sub-modules).
        // Path: dr::OE::SOME_VALUE (if OE had named values).
        // Only generated for fields that have named values.

        // RegisterOf impl
        impl RegisterOf<super::Regs> for Register {
            type Access = ReadWriteAccess;
        }
    }

    pub mod fr {
        pub enum Register {}
        pub const OFFSET: u32 = 4; // computed from field order + access_width
        pub const REG: RegisterId<u32, Register> = RegisterId::new();
        // ...field constants...
        impl RegisterOf<super::Regs> for Register {
            type Access = ReadOnlyAccess;
        }
    }
}
```

### Key behaviors

- Byte offsets are computed from field declaration order and access width.
  Reserved gaps contribute their `count * width_bytes` to the offset.
- `Field::new(mask, shift)` -- mask is computed from `OFFSET` and `NUMBITS`.
- Named values (e.g., `[DISABLED = 0, ENABLED = 1]`) generate
  `FieldValue` constants under the field name within the register module.
  The path convention is `reg_module::FIELD_NAME::VALUE_NAME` (e.g.,
  `cr::UARTEN::ENABLED`). Each field with named values gets a sub-module
  named after the field.
- The `#[rseq::register_block(...)]` attribute on the generated struct
  carries `endian` and `access_width` so the extractor can read them.

### Implementation approach

Study `register_bitfields.rs` for the field-parsing logic (OFFSET,
NUMBITS, named values). The new macro reuses the same field syntax but
wraps it in a block-level structure that also generates the layout struct
and per-register modules.

## Non-Goals

- No removal of `register_bitfields!` (kept during transition, removed
  in Phase 9).
- No PL011 fixture conversion (phase-3d).
- No extraction changes.

## Test Plan

- Small test block: 2 registers (one ReadOnly, one ReadWrite) with 2-3
  fields each, including named values. Verify the module structure exists:
  `block::Regs`, `block::reg1::Register`, `block::reg1::FIELD_NAME`,
  `block::reg1::REG`, `block::reg1::OFFSET`.
- Verify `RegisterOf` impls: `reg1::Register` implements
  `RegisterOf<block::Regs>` with correct `Access` type.
- Verify byte offsets are correct for sequential registers.
- Verify reserved gaps advance offsets correctly.
- Verify `Mmio<block::Regs>` can call `.read_field(reg1::FIELD)` (type
  checks via RegisterOf bounds).
- Verify named value paths: `reg_module::FIELD_NAME::VALUE_NAME` exists
  and is a `FieldValue` constant.
- Compile-fail: writing a ReadOnly register should fail to compile.
- Compile-fail: using a field from block A with `Mmio<block_B::Regs>`
  should fail to compile (type-level block mismatch).
- Serde is not relevant for proc macros.

## Review

After tests pass, use `codex:rescue` to review. Pay special attention
to offset computation and named value generation. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-macros
cargo test -p regseq
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

## Acceptance Criteria

- `register_block!` macro implemented in `regseq-macros`.
- Re-exported from `regseq` crate.
- Test block compiles and produces correct module structure.
- RegisterOf bounds work with Mmio<T> methods.
- Codex review approved.
