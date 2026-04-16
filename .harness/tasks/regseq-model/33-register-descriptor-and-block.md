# regseq-model-33: RegisterDescriptor and RegisterBlock

## Description

Implement the register descriptor and register block model types. Register
descriptors define bitfield layouts for individual registers. Register
blocks define ordered memory-mapped register layouts that reference
descriptors.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 2, step 2.3)
- `docs/discussions/models/regseq-model/spec.md` (Section 3.5: Register Descriptors, Section 3.6: Register Blocks)
- `crates/regseq-model/src/identity/` (RegisterDescriptorId, RegisterBlockId, Symbol)

## Implementation Notes

Create `crates/regseq-model/src/model/register.rs`. Update `model/mod.rs`.

### Public API

```rust
// model/register.rs

/// A bitfield definition for an individual register.
///
/// Register descriptors are standalone, reusable items -- they are not
/// owned by any single register block and may be referenced by multiple
/// blocks.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RegisterDescriptor {
    pub id: RegisterDescriptorId,
    pub name: Symbol,
    /// Register width in bits (e.g., 32 from u32).
    pub access_width: u32,
    pub fields: Vec<BitfieldDef>,
}

/// A single bitfield within a register descriptor.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct BitfieldDef {
    pub name: Symbol,
    /// Bit offset within the register.
    pub offset: u32,
    /// Number of bits.
    pub width: u32,
    /// Optional named constants for this bitfield.
    pub named_values: Vec<NamedValue>,
}

/// A named constant value for a bitfield (e.g., ENABLED = 1).
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct NamedValue {
    pub name: Symbol,
    pub value: u64,
}

/// A memory-mapped register layout definition.
///
/// Register blocks reference register descriptors but do not own them.
/// The same descriptor may be referenced by multiple blocks.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RegisterBlock {
    pub id: RegisterBlockId,
    pub name: Symbol,
    pub block_attributes: RegisterBlockAttributes,
    pub fields: Vec<RegisterBlockField>,
}

/// Block-level properties from #[rseq::register_block(..)].
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RegisterBlockAttributes {
    /// Byte order (e.g., "little", "big").
    pub endian: Symbol,
    /// Default access width in bits (e.g., 32).
    pub access_width: u32,
}

/// A field in a register block layout.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RegisterBlockField {
    pub name: Symbol,
    pub access_kind: AccessKind,
    /// Reference to a register descriptor, if not Reserved.
    pub descriptor: Option<RegisterDescriptorId>,
    /// Byte offset within the block.
    pub byte_offset: u32,
    /// For reserved gaps expressed as arrays, the element count.
    /// None for single fields.
    pub array_count: Option<usize>,
}

/// Access kind for a register block field.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum AccessKind {
    ReadWrite,
    ReadOnly,
    WriteOnly,
    Reserved,
}
```

### Conventions

- `RegisterDescriptor` and `RegisterBlock` contain Symbol fields, so
  no standalone serde derives on those types. `AccessKind` can derive
  serde (no Symbol fields).
- `NamedValue.value` is `u64` to accommodate registers up to 64 bits.
- `RegisterBlockField.descriptor` is `None` for Reserved fields.
- `RegisterBlockField.byte_offset` is derived from field ordering and
  access width during extraction.
- The descriptor-block reference forms a DAG (descriptors shared,
  blocks reference them).

## Non-Goals

- No transport resource types (regseq-model-36).
- No register access intrinsic types (already in ir/intrinsic.rs).

## Test Plan

- Construct a `RegisterDescriptor` with 3 fields, one with named values.
- Construct a `RegisterBlock` with 3 fields: a ReadWrite register, a
  Reserved gap, and a ReadOnly register.
- Verify `RegisterBlockField.descriptor` is `None` for the Reserved field.
- Verify byte offsets are as set.
- Serde round-trip for `AccessKind` all 4 variants.
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against spec.md
Sections 3.5 and 3.6. Verify the descriptor-block reference DAG,
field structures, and all access kinds. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `model/register.rs` created with all types.
- Re-exported from `model/mod.rs`.
- All tests pass. Codex review approved.
