# regseq-driver-30: Pin-state attribute extraction

## Description

Extend the pin control extraction to recognize `#[rseq::pin_state(id = "...")]`
attributes on marker types used for pin state identification.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 4.2)
- `docs/discussions/models/regseq-model/spec.md` (Section 4.2: PinCtrl)
- `crates/regseq-driver/src/extract/pinctrl.rs` (existing pin control extraction)
- `tests/fixtures/pl011-full-spec/src/driver.rs` (DefaultState with `#[rseq::pin_state(id = "default")]`, SleepState with `#[rseq::pin_state(id = "sleep")]`)

## Implementation Notes

### Extend existing extractor

Update `crates/regseq-driver/src/extract/pinctrl.rs` to:

1. Scan for types annotated with `#[rseq::pin_state(id = "...")]`.
2. Collect pin state identifiers and their associated type DefIds.
3. Make this information available to the pin control surface output.

### Pin state discovery

During extraction, when visiting type definitions:
- Check for `rseq::pin_state` attribute.
- Extract the `id` value (e.g., `"default"`, `"sleep"`).
- Record the mapping: type DefId -> pin state id string.

This information is used during normalization to associate pin states
with PinCtrl resources and during IR construction to resolve
`PinCtrl::apply` calls with the correct state_id metadata.

### Output type

Add a `pin_states` field to the pin control surface output:

```rust
pub struct PinStateInfo {
    /// Fully-qualified path of the marker type (e.g., "pl011_full_spec::driver::DefaultState").
    /// Uses String for serialization, consistent with how tasks 27-29
    /// convert DefId to String via method_path/type_path.
    pub type_path: String,
    /// The pin state identifier from the attribute.
    pub state_id: String,
}
```

## Non-Goals

- No changes to RawFrontendArtifact structure (task 32).
- No normalization logic changes.
- No IR construction changes.

## Test Plan

- Extract pin control surface from PL011 full-spec compilation.
- Verify 2 pin states are discovered: "default" and "sleep".
- Verify each is associated with the correct marker type.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to review. Verify the pin state
extraction covers both marker types from the PL011 fixture. Iterate
until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `extract/pinctrl.rs` updated with pin-state attribute extraction.
- Pin states discoverable from PL011 fixture.
- All tests pass. Codex review approved.
