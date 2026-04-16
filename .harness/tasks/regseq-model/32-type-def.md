# regseq-model-32: TypeDef, FieldDef, VariantDef, Attrs

## Description

Implement the type definition model types: `TypeDef` representing struct
and enum definitions with classification, `FieldDef` for struct/variant
fields, `VariantDef` for enum variants, and `Attrs` as an extensible
attribute bag.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 2, step 2.2)
- `docs/discussions/models/regseq-model/spec.md` (Section 3.4: Type Definitions, Section 8: Attrs)
- `crates/regseq-model/src/model/framework.rs` (TypeDefClassification)
- `crates/regseq-model/src/identity/` (TypeDefId, Symbol)
- `crates/regseq-model/src/ir/types.rs` (RegseqType)
- `crates/regseq-model/src/ir/operand.rs` (LiteralValue)

## Implementation Notes

Create `crates/regseq-model/src/model/type_def.rs`. Update `model/mod.rs`.

### Public API

```rust
// model/type_def.rs

/// A struct or enum type definition.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct TypeDef {
    pub id: TypeDefId,
    pub name: Symbol,
    pub kind: TypeDefKind,
    pub classification: TypeDefClassification,
    pub attrs: Attrs,
}

/// Whether the type is a struct or enum.
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum TypeDefKind {
    Struct {
        fields: Vec<FieldDef>,
    },
    Enum {
        variants: Vec<VariantDef>,
    },
}

/// A named field in a struct or enum variant.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct FieldDef {
    pub name: Symbol,
    pub ty: RegseqType,
}

/// An enum variant.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct VariantDef {
    pub name: Symbol,
    pub fields: Vec<FieldDef>,
    pub discriminant: Option<LiteralValue>,
}

/// Extensible attribute bag for semantic annotations.
///
/// Captures #[rseq::attrs(..)] annotations generically. No concrete
/// attribute keys are defined at this time -- the mechanism exists for
/// future semantic annotations (ISR metadata, callback context,
/// inlining hints, etc.).
#[derive(Clone, Eq, PartialEq, Debug, Default)]
pub struct Attrs {
    pub entries: Vec<AttrEntry>,
}

/// A single key-value attribute entry.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct AttrEntry {
    pub key: String,
    pub value: AttrValue,
}

/// The value of an attribute entry.
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum AttrValue {
    /// A string value.
    String(String),
    /// A boolean value.
    Bool(bool),
    /// No value (presence-only attribute).
    Unit,
}
```

### Conventions

- `TypeDef` contains Symbol fields, so no standalone serde derives.
  Serde will be added when model-level serialization is built.
- `Attrs` is `Default` (empty attrs is the common case).
- `VariantDef.fields` is empty for unit variants, populated for data
  variants.
- `VariantDef.discriminant` is `Some` only for explicit discriminant
  values.

## Non-Goals

- No register descriptor or block types (regseq-model-33).
- No serde on Symbol-containing types.
- No attribute parsing logic.

## Test Plan

- Construct a `TypeDef` for a simple struct (UserDefined, 2 fields).
- Construct a `TypeDef` for an enum with unit and data variants.
- Construct a `TypeDef` with `Framework(FrameworkTypeKind::Duration)`.
- Construct a `TypeDef` with `Abstraction(RegseqAbstraction::Clock)`.
- Verify `Attrs::default()` has empty entries.
- Construct `Attrs` with mixed entry types (String, Bool, Unit).
- `cargo test -p regseq-model` passes.

## Review

After tests pass, use `codex:rescue` to review against spec.md
Section 3.4 and Section 8 (Attrs). Verify struct/enum distinction,
classification variants, field/variant structure. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `model/type_def.rs` created with all types.
- Re-exported from `model/mod.rs`.
- All tests pass. Codex review approved.
