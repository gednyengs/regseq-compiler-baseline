# regseq-model-21: Declaration IDs, body-level IDs, and source locations

## Description

Implement the typed ID scheme (`DeclId` with 9 typed wrappers) and
body-level IDs (`LocalId`, `BasicBlockId`), plus the source location
types (`SourceLocation`, `SourceOrigin`).

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 0, steps 0.2-0.3)
- `docs/discussions/models/regseq-model/spec.md` (Section 7: Identity Scheme)
- `docs/discussions/models/regseq-model/ir-reference.md` (Section 7: Source Locations)
- `crates/regseq-model/src/identity/symbol.rs` (from regseq-model-20)

## Implementation Notes

Create `crates/regseq-model/src/identity/decl_id.rs` and
`crates/regseq-model/src/identity/source.rs`. Update `identity/mod.rs`
to re-export all types.

### Public API -- decl_id.rs

```rust
/// Base declaration identifier. Single flat namespace.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash,
         Serialize, Deserialize)]
pub struct DeclId(u32);

// 9 typed wrappers -- all newtype over DeclId:
pub struct ModuleId(pub DeclId);
pub struct ConstantId(pub DeclId);
pub struct FunctionId(pub DeclId);
pub struct TypeDefId(pub DeclId);
pub struct RegisterDescriptorId(pub DeclId);
pub struct RegisterBlockId(pub DeclId);
pub struct ResourceId(pub DeclId);
pub struct PropertyId(pub DeclId);
pub struct UserDataId(pub DeclId);

// Body-level IDs -- NOT DeclId-based:
pub struct LocalId(pub u32);
pub struct BasicBlockId(pub u32);
```

All types: `Copy + Clone + Eq + PartialEq + Hash + Debug + Serialize + Deserialize`.

Provide `DeclId::new(u32) -> Self` and `DeclId::index(&self) -> u32`.
Same for `LocalId` and `BasicBlockId`.

Each typed wrapper provides `From<DeclId>` and derefs or exposes the
inner `DeclId` for generic operations.

### Public API -- source.rs

```rust
/// A source location range with interned file path.
#[derive(Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: FileId,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// The origin of an IR node.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum SourceOrigin {
    Source(SourceLocation),
    CompilerGenerated,
}
```

### Conventions

- Serde: `DeclId` and wrappers serialize as plain integers. `SourceOrigin`
  uses adjacently-tagged enum format.
- `Debug` for DeclId wrappers should include the wrapper name
  (e.g., `FunctionId(DeclId(3))`).

## Non-Goals

- No DeclId allocator or counter (that is Phase 5 IR construction).
- No SymbolInterner changes.
- No RegseqType.

## Test Plan

- Construct each of the 9 typed wrappers from a `DeclId`.
- Typed wrappers with the same inner `DeclId` are equal.
- Different typed wrappers are distinct Rust types (compile-time check).
- `LocalId` and `BasicBlockId` are independent of `DeclId`.
- Serde round-trip for `DeclId`, each wrapper, `LocalId`, `BasicBlockId`.
- Serde round-trip for `SourceLocation` and both `SourceOrigin` variants.

## Review

After tests pass, use the `codex:rescue` tool to review against this
spec. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `identity/decl_id.rs` and `identity/source.rs` created.
- `identity/mod.rs` re-exports all types.
- All tests pass. Codex review approved.
