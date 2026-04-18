# regseq-driver-32: RawFrontendArtifact expansion and callback wiring

## Description

Expand `RawFrontendArtifact` with new surface fields for the intrinsic
registry, framework type registry, abstraction type registry, and source
locations. Wire the new extraction modules into `callbacks.rs`.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 4.4)
- `crates/regseq-driver/src/raw_artifact.rs` (current RawFrontendArtifact)
- `crates/regseq-driver/src/callbacks.rs` (extraction entry point)
- `crates/regseq-driver/src/extract/intrinsic_registry.rs` (from task 27)
- `crates/regseq-driver/src/extract/framework_types.rs` (from task 28)
- `crates/regseq-driver/src/extract/abstraction_types.rs` (from task 29)
- `crates/regseq-driver/src/extract/pinctrl.rs` (updated in task 30)
- `crates/regseq-driver/src/extract/source_locations.rs` (from task 31)

## Implementation Notes

### RawFrontendArtifact changes

Add new fields to `RawFrontendArtifact` in `raw_artifact.rs`:

```rust
pub struct RawFrontendArtifact {
    // ... existing fields ...

    /// Intrinsic registry: canonical intrinsic name -> method path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intrinsic_registry: Option<RawIntrinsicRegistrySurface>,

    /// Framework type registry: framework type id -> type path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub framework_types: Option<RawFrameworkTypeSurface>,

    /// Abstraction type registry: abstraction id -> type path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abstraction_types: Option<RawAbstractionTypeSurface>,

    /// Source locations for all items.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_locations: Option<RawSourceLocationSurface>,
}
```

### Serializable surface types

The registry types from tasks 27-29 use `DefId` internally, which is
not serializable. Create serializable surface representations:

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawIntrinsicRegistrySurface {
    pub entries: Vec<RawIntrinsicEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawIntrinsicEntry {
    pub canonical_name: String,
    pub method_path: String,
}

// Similar for framework types and abstraction types.
```

### Callback wiring

In `callbacks.rs`, add calls to the new extraction modules in the
extraction sequence. Order:
1. Build registries (intrinsic, framework, abstraction) -- these run
   first as read-only context.
2. Source locations -- runs after all items are known.
3. Existing extraction surfaces (register_access, clock, reset, pinctrl,
   etc.) continue to use their current path-based heuristics. Registries
   do NOT replace heuristics in Phase 4 -- that happens in Phase 5 IR
   construction.
4. Convert registries to serializable surfaces when building the
   RawFrontendArtifact.

### Existing surface compatibility

The existing extraction surfaces (register_access, clock, reset, etc.)
continue to work as before. The new fields are `Option` with
`skip_serializing_if`, so existing golden files are NOT affected until
the new surfaces are wired in.

## Non-Goals

- No unit test specimen updates (task 33).
- No golden file regeneration (task 34).
- No normalization changes.

## Test Plan

- Build PL011 full-spec with `--emit raw-artifact`.
- Verify the output JSON contains the new fields when present.
- Verify the output JSON does NOT contain the new fields when None
  (skip_serializing_if works).
- Verify existing surfaces are unchanged.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to review. Verify the artifact
expansion is backward-compatible, new fields are properly gated, and
callback ordering is correct. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `RawFrontendArtifact` expanded with 4 new fields.
- New extraction modules wired into `callbacks.rs`.
- Serializable surface types created.
- Backward-compatible with existing golden files.
- All tests pass. Codex review approved.
