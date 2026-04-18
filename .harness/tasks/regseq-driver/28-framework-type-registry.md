# regseq-driver-28: Framework Type Registry Extraction

## Description

Implement the framework type registry extractor that collects all
`#[rseq::framework_type("...")]` attributes into a
`HashMap<DefId, String>` mapping each annotated type to its framework
type identifier.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 4.1)
- `docs/discussions/models/regseq-model/spec.md` (Section 3.4: FrameworkTypeKind)
- `crates/regseq-driver/src/extract/attr_validation.rs` (from regseq-driver-26)
- `crates/regseq/src/error.rs` (regseq-error)
- `crates/regseq/src/serial.rs` (serial-errors, rx-char, serial-config, data-bits, parity, stop-bits, flow-control)
- `crates/regseq/src/time.rs` (duration)
- `crates/regseq/src/irq.rs` (irq-result)

## Implementation Notes

### File

Create `crates/regseq-driver/src/extract/framework_types.rs`.

### Public API

```rust
pub struct FrameworkTypeRegistry {
    entries: HashMap<DefId, String>,
}

impl FrameworkTypeRegistry {
    pub fn build(tcx: TyCtxt<'_>) -> Self { ... }
    pub fn lookup(&self, def_id: DefId) -> Option<&str> { ... }
    pub fn len(&self) -> usize { ... }
    pub fn is_empty(&self) -> bool { ... }
}
```

### Expected entries

| Attribute value | Source type |
|----------------|------------|
| `regseq-error` | `RegseqError` |
| `serial-errors` | `SerialErrors` |
| `rx-char` | `RxChar` |
| `serial-config` | `SerialConfig` |
| `data-bits` | `DataBits` |
| `parity` | `Parity` |
| `stop-bits` | `StopBits` |
| `flow-control` | `FlowControl` |
| `duration` | `Duration` |
| `irq-result` | `IrqResult` |

### Scanning strategy

Same as task 27: walk crate dependencies, check for
`rseq::framework_type` attribute, extract string value. Framework types
are captured exclusively via the `#[rseq::framework_type("...")]`
attribute -- no path-based fallback. If cross-crate attribute visibility
regresses, that is a bug in task 26 to be fixed there, not a behavior
the registry should paper over.

## Non-Goals

- No RawFrontendArtifact changes (task 32).
- No type definition construction.

## Test Plan

- Extract framework type registry from PL011 full-spec compilation.
- Verify 10 entries are discovered.
- Verify `serial-config` and `regseq-error` are present.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to review. Verify all 10 framework
types from spec.md Section 3.4 are discoverable. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `extract/framework_types.rs` created with FrameworkTypeRegistry.
- All 10 framework types discoverable.
- All tests pass. Codex review approved.
