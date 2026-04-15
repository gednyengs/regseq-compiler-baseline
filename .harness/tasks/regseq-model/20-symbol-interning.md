# regseq-model-20: Symbol interning and string identity

## Description

Implement the `SymbolInterner` and its handle types (`Symbol`, `FileId`)
that provide O(1) string equality and compact storage for all identifiers
and file paths in the RegseqModel.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 0, step 0.1)
- `docs/discussions/models/regseq-model/ir-reference.md` (Section 7: Symbol Interning)
- `crates/regseq-model/src/lib.rs`

## Implementation Notes

Create `crates/regseq-model/src/identity/symbol.rs` and
`crates/regseq-model/src/identity/mod.rs`. Add `pub mod identity;` to
`crates/regseq-model/src/lib.rs`.

### Public API

```rust
// identity/symbol.rs

/// An interned string identifier. Copy, Eq, Hash -- all O(1).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Symbol(u32);

/// An interned file path. Distinct newtype over the same pool.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct FileId(u32);

/// The string intern table.
pub struct SymbolInterner {
    // Internal: Vec<String> + HashMap<String, u32>
}

impl SymbolInterner {
    pub fn new() -> Self;
    pub fn intern(&mut self, s: &str) -> Symbol;
    pub fn intern_file(&mut self, path: &str) -> FileId;
    pub fn resolve(&self, sym: Symbol) -> &str;
    pub fn resolve_file(&self, file: FileId) -> &str;
}
```

### Serde

Symbol and FileId do NOT implement Serialize/Deserialize standalone.
Serialization happens only at the RegseqModel boundary, where a
model-level serializer resolves symbols to strings. Individual
Symbol/FileId values are opaque handles -- they have no meaning outside
the interner that created them. Do not implement standalone serde for
Symbol or FileId.

### Conventions

- Single shared pool for both `Symbol` and `FileId` (same backing
  `Vec<String>`). The newtypes prevent accidental mixing.
- Debug impl for Symbol/FileId should show the raw index (e.g.,
  `Symbol(3)`, `FileId(7)`). Human-readable resolution requires the
  interner and is done at diagnostic/display sites, not in Debug.
- `SymbolInterner` implements `Default`.

## Non-Goals

- No DeclId or typed wrappers (regseq-model-21).
- No RegseqType (regseq-model-22).
- No file I/O or source location types.

## Test Plan

- `intern("foo")` returns the same `Symbol` on repeated calls.
- Two different strings produce different `Symbol` values.
- `resolve(intern("bar"))` returns `"bar"`.
- `intern_file` and `resolve_file` work symmetrically.
- `Symbol` and `FileId` from the same string are not equal (different
  types, but backed by the same index -- this is fine, they're distinct
  newtypes).
- FileId resolve: `resolve_file(intern_file("path/to/file"))` returns
  `"path/to/file"`.
- Empty interner: `new()` creates an interner with no entries.

## Review

After tests pass, use the `codex:rescue` tool to review the
implementation against this task spec. Iterate until Codex confirms the
implementation matches.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

## Acceptance Criteria

- `identity/symbol.rs` created with all public API items.
- `identity/mod.rs` re-exports `Symbol`, `FileId`, `SymbolInterner`.
- `lib.rs` updated with `pub mod identity;`.
- All tests pass. Codex review approved.
