# regseq-facade-21: Non-register intrinsic annotations

## Description

Add named `#[rseq::intrinsic("...")]` attributes to all non-register
intrinsic methods and functions: Clock (3), ResetLine (3), PinCtrl (1),
timed_delay (2), logging (4), poll_until (1) = 14 total.

Register intrinsic annotations on Mmio<T> are NOT part of this task --
those methods do not exist yet and are created in phase-3b.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 3.3)
- `docs/discussions/models/regseq-model/spec.md` (Section 10.2)
- `docs/discussions/models/regseq-model/compiler-knowledge.md` (Sections 2, 3)
- `crates/regseq/src/clock.rs`
- `crates/regseq/src/reset.rs`
- `crates/regseq/src/pinctrl.rs`
- `crates/regseq/src/intrinsics/timed_delay.rs`
- `crates/regseq/src/intrinsics/diagnostic_logging.rs`
- `crates/regseq/src/intrinsics/register_access.rs`

## Implementation Notes

Add `#[rseq::intrinsic("canonical_name")]` to each method/function:

| File | Method/Function | Canonical name |
|------|----------------|----------------|
| `clock.rs` | `Clock::enable` | `"clock_enable"` |
| `clock.rs` | `Clock::disable` | `"clock_disable"` |
| `clock.rs` | `Clock::get_rate` | `"clock_get_rate"` |
| `reset.rs` | `ResetLine::assert` | `"reset_assert"` |
| `reset.rs` | `ResetLine::deassert` | `"reset_deassert"` |
| `reset.rs` | `ResetLine::toggle` | `"reset_toggle"` |
| `pinctrl.rs` | `PinCtrl::apply` | `"pinctrl_apply"` |
| `intrinsics/timed_delay.rs` | `busy_wait` | `"busy_wait"` |
| `intrinsics/timed_delay.rs` | `sleep` | `"sleep"` |
| `intrinsics/diagnostic_logging.rs` | `error` | `"log_error"` |
| `intrinsics/diagnostic_logging.rs` | `warn` | `"log_warn"` |
| `intrinsics/diagnostic_logging.rs` | `info` | `"log_info"` |
| `intrinsics/diagnostic_logging.rs` | `debug` | `"log_debug"` |
| `intrinsics/register_access.rs` | `poll_until` | `"poll_until"` |

If any of these already carry a bare `#[rseq::intrinsic]` (without a
name string), replace it with the named form.

### Conventions

- Place the intrinsic attribute on the line immediately before `pub fn`.
- The canonical name is a string literal inside the attribute.

## Non-Goals

- No register intrinsic annotations on Mmio<T> (phase-3b).
- No new methods or type changes.
- No extraction pipeline changes.

## Test Plan

- `cargo build -p regseq` compiles.
- `cargo build -p pl011-full-spec` compiles.
- `cargo test --workspace` passes.

## Review

After tests pass, use `codex:rescue` to review. Verify every entry in
the table above has the correct attribute. Iterate until approved.

## Quality Gate

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```

## Acceptance Criteria

- All 14 intrinsic annotations added with correct canonical names.
- Workspace compiles and tests pass. Codex review approved.
