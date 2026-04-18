# regseq-driver-27: Intrinsic Registry Extraction

## Description

Implement the intrinsic registry extractor that collects all
`#[rseq::intrinsic("...")]` attributes into a `HashMap<DefId, String>`
mapping each annotated function/method to its canonical intrinsic name.

This replaces path-based intrinsic recognition with attribute-based
recognition.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 4.1)
- `docs/discussions/models/regseq-model/compiler-knowledge.md` (intrinsic canonical names)
- `crates/regseq-driver/src/extract/attr_validation.rs` (from regseq-driver-26)
- `crates/regseq-driver/src/callbacks.rs`
- `crates/regseq/src/mmio.rs` (mmio_read, mmio_write, mmio_read_field, mmio_write_field)
- `crates/regseq/src/clock.rs` (clock_enable, clock_disable, clock_get_rate)
- `crates/regseq/src/reset.rs` (reset_assert, reset_deassert, reset_toggle)
- `crates/regseq/src/pinctrl.rs` (pinctrl_apply)
- `crates/regseq/src/intrinsics/` (busy_wait, sleep, log_*, poll_until)

## Implementation Notes

### File

Create `crates/regseq-driver/src/extract/intrinsic_registry.rs`.

### Public API

```rust
use rustc_hir::def_id::DefId;
use std::collections::HashMap;

/// Registry mapping DefIds of intrinsic-annotated functions/methods
/// to their canonical intrinsic names.
pub struct IntrinsicRegistry {
    entries: HashMap<DefId, String>,
}

impl IntrinsicRegistry {
    /// Build the registry by scanning all DefIds for #[rseq::intrinsic("...")] attributes.
    pub fn build(tcx: TyCtxt<'_>) -> Self { ... }

    /// Look up the canonical intrinsic name for a callee DefId.
    pub fn lookup(&self, def_id: DefId) -> Option<&str> { ... }

    /// Number of registered intrinsics.
    pub fn len(&self) -> usize { ... }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool { ... }
}
```

### Expected entries

The registry should discover all 18 intrinsics annotated in Phase 3:

Note: `poll_until` is annotated with `#[rseq::intrinsic("poll_until")]`
in Phase 3 even though its IR construction support is deferred until
closure support is added. Phase 4 only builds the registry -- it makes
no IR decisions. The annotation exists, so the registry finds it.

| Canonical name | Source |
|---------------|--------|
| `mmio_read` | `Mmio::read` |
| `mmio_read_field` | `Mmio::read_field` |
| `mmio_write` | `Mmio::write` |
| `mmio_write_field` | `Mmio::write_field` |
| `clock_enable` | `Clock::enable` |
| `clock_disable` | `Clock::disable` |
| `clock_get_rate` | `Clock::get_rate` |
| `reset_assert` | `ResetLine::assert` |
| `reset_deassert` | `ResetLine::deassert` |
| `reset_toggle` | `ResetLine::toggle` |
| `pinctrl_apply` | `PinCtrl::apply` |
| `busy_wait` | `busy_wait()` |
| `sleep` | `sleep()` |
| `log_error` | `error()` |
| `log_warn` | `warn()` |
| `log_info` | `info()` |
| `log_debug` | `debug()` |
| `poll_until` | `poll_until()` |

### Scanning strategy

Walk all crate dependencies (particularly `regseq` facade crate),
iterate over all items with DefIds, check for `rseq::intrinsic`
attribute via `tcx.get_attrs(def_id)`, extract the string value.

If cross-crate attribute visibility failed in task 26, use path-based
fallback instead (match known function paths to canonical names).

## Non-Goals

- No RawFrontendArtifact changes (task 32).
- No IR construction (Phase 5).

## Test Plan

- Extract intrinsic registry from PL011 full-spec compilation.
- Verify at least 18 entries are discovered.
- Verify `mmio_read`, `clock_enable`, `poll_until` are present.
- Verify `lookup()` returns correct canonical name for a known DefId.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to review. Verify all 18 intrinsic
canonical names from the execution plan are discoverable. Iterate until
approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `extract/intrinsic_registry.rs` created with IntrinsicRegistry.
- All expected intrinsics discoverable.
- All tests pass. Codex review approved.
