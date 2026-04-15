# regseq-driver-25: Release-profile MIR compiler config

## Description

Update the compiler callback configuration to set release-profile MIR
semantics: `overflow-checks = false` and `debug-assertions = false`.

The IR spec requires these settings so that arithmetic uses
`Add`/`Sub`/`Mul` (not `AddWithOverflow`) and no debug-assertion
`Assert` terminators appear in MIR.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 3.8)
- `docs/discussions/models/regseq-model/ir-reference.md` (preamble, lines 7-17)
- `crates/regseq-driver/src/callbacks.rs`

## Implementation Notes

In `crates/regseq-driver/src/callbacks.rs`, within the compiler
configuration callback (where `mir_opt_level` and `always_encode_mir`
are set), add:

```rust
config.opts.cg.overflow_checks = Some(false);
config.opts.cg.debug_assertions = Some(false);
```

Or the equivalent fields depending on the rustc config API. The existing
code already sets:
```rust
config.opts.unstable_opts.always_encode_mir = true;
config.opts.unstable_opts.mir_opt_level = Some(0);
```

The new settings should be adjacent.

### Validation

After making the change, verify by examining a MIR dump of the PL011
fixture:
- No `Assert` terminators should appear (these are debug-assertion
  checks for arithmetic overflow and bounds).
- Arithmetic operations should be `Add`, `Sub`, `Mul` -- NOT
  `CheckedAdd`, `AddWithOverflow`, etc.

## Non-Goals

- No other callback changes.
- No extraction logic changes.
- No IR construction (Phase 5).

## Test Plan

- `cargo build -p regseq-driver` compiles.
- Existing extraction tests pass (`cargo test -p regseq-driver`).
- E2E extraction tests pass with the new config.
- Verify MIR dump of a PL011 fixture function shows no `Assert`
  terminators and uses `Add`/`Sub`/`Mul` (not `*WithOverflow` variants).

## Review

After tests pass, use `codex:rescue` to review. Verify the config
fields are set correctly. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `overflow_checks` and `debug_assertions` set to `false` in callbacks.
- Existing tests pass. Codex review approved.
