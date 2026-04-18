# regseq-driver-29: Abstraction Type Registry Extraction

## Description

Implement the abstraction type registry extractor that collects all
`#[rseq::abstraction("...")]` attributes into a
`HashMap<DefId, String>` mapping each annotated type to its abstraction
identifier.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 4.1)
- `docs/discussions/models/regseq-model/spec.md` (Section 10.1: RegseqAbstraction)
- `crates/regseq-driver/src/extract/attr_validation.rs` (from regseq-driver-26)
- `crates/regseq/src/clock.rs` (clock)
- `crates/regseq/src/reset.rs` (reset)
- `crates/regseq/src/pinctrl.rs` (pinctrl)
- `crates/regseq/src/irq.rs` (interrupt)
- `crates/regseq/src/mmio.rs` (mmio)
- `crates/regseq/src/sync.rs` (mutex, spinlock, irq-spinlock, completion)

## Implementation Notes

### File

Create `crates/regseq-driver/src/extract/abstraction_types.rs`.

### Public API

```rust
pub struct AbstractionTypeRegistry {
    entries: HashMap<DefId, String>,
}

impl AbstractionTypeRegistry {
    pub fn build(tcx: TyCtxt<'_>) -> Self { ... }
    pub fn lookup(&self, def_id: DefId) -> Option<&str> { ... }
    pub fn len(&self) -> usize { ... }
    pub fn is_empty(&self) -> bool { ... }
}
```

### Expected entries

| Attribute value | Source type |
|----------------|------------|
| `clock` | `Clock` |
| `reset` | `ResetLine` |
| `pinctrl` | `PinCtrl` |
| `interrupt` | `Interrupt` |
| `mmio` | `Mmio<T>` |
| `mutex` | `Mutex<T>` |
| `spinlock` | `SpinLock<T>` |
| `irq-spinlock` | `IrqSpinLock<T>` |
| `completion` | `Completion` |

### Scanning strategy

Same pattern as tasks 27-28. Walk crate dependencies, check for
`rseq::abstraction` attribute, extract string value.

## Non-Goals

- No RawFrontendArtifact changes (task 32).
- No type classification logic.

## Test Plan

- Extract abstraction type registry from PL011 full-spec compilation.
- Verify 9 entries are discovered.
- Verify `clock`, `mmio`, `reset` are present.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to review. Verify all 9
abstraction types from spec.md Section 10.1 are discoverable. Iterate
until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `extract/abstraction_types.rs` created with AbstractionTypeRegistry.
- All 9 abstraction types discoverable.
- All tests pass. Codex review approved.
