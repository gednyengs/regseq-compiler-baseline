# regseq-facade-20: Framework type and abstraction attribute annotations

## Description

Add `#[rseq::framework_type("...")]` to all 10 framework types and
`#[rseq::abstraction("...")]` to all 9 abstraction types in the facade
crate. Remove the deprecated `ErrorKind` type alias.

These are inert tool attributes that do not change compilation. They
prepare the facade crate for attribute-based extraction in Phase 4.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 3.1, 3.2, 3.4)
- `docs/discussions/models/regseq-model/spec.md` (Sections 3.4, 10.1)
- `docs/discussions/models/regseq-model/compiler-knowledge.md` (Sections 1, 2)
- `crates/regseq/src/lib.rs`
- `crates/regseq/src/error.rs`
- `crates/regseq/src/serial.rs`
- `crates/regseq/src/clock.rs`
- `crates/regseq/src/reset.rs`
- `crates/regseq/src/pinctrl.rs`
- `crates/regseq/src/mmio.rs`
- `crates/regseq/src/irq.rs`
- `crates/regseq/src/sync.rs`
- `crates/regseq/src/time.rs`

## Implementation Notes

### Framework type annotations

Add `#[rseq::framework_type("...")]` directly above each type definition:

| File | Type | Attribute value |
|------|------|-----------------|
| `error.rs` | `RegseqError` | `"regseq-error"` |
| `serial.rs` | `SerialErrors` | `"serial-errors"` |
| `serial.rs` | `RxChar` | `"rx-char"` |
| `serial.rs` | `SerialConfig` | `"serial-config"` |
| `serial.rs` | `DataBits` | `"data-bits"` |
| `serial.rs` | `Parity` | `"parity"` |
| `serial.rs` | `StopBits` | `"stop-bits"` |
| `serial.rs` | `FlowControl` | `"flow-control"` |
| `time.rs` | `Duration` | `"duration"` |
| `irq.rs` | `IrqResult` | `"irq-result"` |

### Abstraction type annotations

Add `#[rseq::abstraction("...")]` directly above each type definition:

| File | Type | Attribute value |
|------|------|-----------------|
| `clock.rs` | `Clock` | `"clock"` |
| `reset.rs` | `ResetLine` | `"reset"` |
| `pinctrl.rs` | `PinCtrl` | `"pinctrl"` |
| `irq.rs` | `Interrupt` | `"interrupt"` |
| `mmio.rs` | `Mmio<T>` | `"mmio"` |
| `sync.rs` | `Mutex<T>` | `"mutex"` |
| `sync.rs` | `SpinLock<T>` | `"spinlock"` |
| `sync.rs` | `IrqSpinLock<T>` | `"irq-spinlock"` |
| `sync.rs` | `Completion` | `"completion"` |

### ErrorKind removal

- Remove `pub type ErrorKind = SerialErrors;` from `serial.rs`.
- Remove `ErrorKind` from the `pub use serial::{...}` line in `lib.rs`.
- Search for any remaining `ErrorKind` references in the workspace and
  update them to `SerialErrors`.

### Conventions

- Place the `#[rseq::]` attribute on the line immediately before the
  type's existing attributes (e.g., before `#[derive(...)]`).
- These are tool attributes in the `rseq` namespace, registered by
  `regseq-driver`. They are NOT proc macros.

## Non-Goals

- No intrinsic annotations on methods (regseq-facade-21).
- No register access restructure (phase-3b).
- No ordered! macro changes (phase-3c).
- No new types or methods.

## Test Plan

- `cargo build -p regseq` compiles without errors.
- `cargo build -p pl011-full-spec` (or the test fixture) compiles.
- `cargo test --workspace` passes (no regressions from attribute additions).
- Grep confirms no remaining `ErrorKind` references outside of archive/.

## Review

After tests pass, use `codex:rescue` to review. Verify every type in the
tables above has the correct attribute. Iterate until approved.

## Quality Gate

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

## Acceptance Criteria

- All 10 framework type annotations added.
- All 9 abstraction type annotations added.
- `ErrorKind` alias removed from serial.rs and lib.rs.
- Workspace compiles and all tests pass. Codex review approved.
