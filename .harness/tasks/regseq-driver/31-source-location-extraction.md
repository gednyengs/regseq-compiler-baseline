# regseq-driver-31: Source location extraction

## Description

Add span extraction for all items with a DefId. Source locations are
stored in the RawFrontendArtifact and later populate
`RegseqModel.source_locations` during normalization.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 4.3)
- `docs/discussions/models/regseq-model/spec.md` (Section 7: Identity Scheme)
- `crates/regseq-model/src/identity/source.rs` (SourceLocation, SourceOrigin)
- `crates/regseq-driver/src/callbacks.rs` (extraction callback)
- `crates/regseq-driver/src/extract/mod.rs`

## Implementation Notes

### File

Create `crates/regseq-driver/src/extract/source_locations.rs`.

### Public API

```rust
/// Source location information for a single item.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawSourceLocation {
    /// File path (relative to crate root or absolute).
    pub file: String,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Collected source locations for all extracted items.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RawSourceLocationSurface {
    /// Map from item path to source location. BTreeMap for
    /// deterministic JSON output ordering.
    pub locations: BTreeMap<String, RawSourceLocation>,
}
```

### Extraction logic

```rust
pub fn extract_source_locations(tcx: TyCtxt<'_>, /* ... */) -> RawSourceLocationSurface {
    // For each DefId in the target crate:
    // 1. Get the span via tcx.def_span(def_id)
    // 2. Resolve to file/line/col via source_map
    // 3. Record as RawSourceLocationEntry
}
```

### What to extract spans for

- All struct and enum definitions
- All function definitions (free functions, methods)
- All const/static definitions
- All impl blocks
- All module definitions
- The driver struct itself

### Integration

Wire into `callbacks.rs` as a new extraction surface call, alongside
the existing extraction calls.

## Non-Goals

- No body-level source locations (those come from BodySourceMap in
  Phase 5).
- No RawFrontendArtifact expansion (task 32).
- No normalization changes.

## Test Plan

- Extract source locations from PL011 full-spec compilation.
- Verify at least 10 items have source locations.
- Verify the driver struct has a source location.
- Verify source locations have non-zero line numbers.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to review. Verify the extraction
covers all item kinds listed above. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `extract/source_locations.rs` created.
- Source locations extractable for all item kinds.
- All tests pass. Codex review approved.
