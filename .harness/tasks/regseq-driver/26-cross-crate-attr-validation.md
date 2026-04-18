# regseq-driver-26: Cross-crate attribute validation gate test

## Description

Validate that `#[rseq::intrinsic("...")]`, `#[rseq::framework_type("...")]`,
`#[rseq::abstraction("...")]`, and `#[rseq::pin_state(id = "...")]`
attributes placed on facade crate types are visible via
`tcx.get_attrs(def_id)` when extracting a dependent crate.

This is the gate test for Phase 4. If cross-crate attribute visibility
does not work, the rest of Phase 4 must fall back to path-based heuristics.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 4.0)
- `crates/regseq-driver/src/callbacks.rs` (extraction entry point)
- `crates/regseq/src/mmio.rs` (has `#[rseq::intrinsic("mmio_read")]` etc.)
- `crates/regseq/src/clock.rs` (has `#[rseq::abstraction("clock")]`)
- `crates/regseq/src/serial.rs` (has `#[rseq::framework_type("serial-config")]`)
- `tests/fixtures/pl011-full-spec/src/driver.rs` (has `#[rseq::pin_state(id = "default")]`)

## Implementation Notes

### Approach

Add a validation pass to the extraction callback that runs early
(before existing extraction surfaces). The pass:

1. Iterates over all `DefId`s from the facade crate (`regseq`).
2. For each `DefId`, calls `tcx.get_attrs(def_id)`.
3. Checks for the presence of `rseq::intrinsic`, `rseq::framework_type`,
   `rseq::abstraction` attributes.
4. Logs findings to a diagnostic output.

This does NOT need to be a permanent extraction module -- it can be a
temporary validation function that is removed after confirming the
approach works. However, if the validation is successful, the function
can evolve into the foundation for the registry extractors (tasks 27-29).

### What to validate

1. `Mmio::read` method has a `rseq::intrinsic` attribute with value
   `"mmio_read"`.
2. `Clock` struct has a `rseq::abstraction` attribute with value
   `"clock"`.
3. `SerialConfig` struct has a `rseq::framework_type` attribute with
   value `"serial-config"`.
4. At least one type from the target crate (PL011 fixture) has a
   `rseq::pin_state` attribute visible.

### Implementation location

Add `crates/regseq-driver/src/extract/attr_validation.rs`. Wire a
call into `callbacks.rs` that runs when a `--validate-attrs` flag is
present (or unconditionally during tests).

### Fallback plan

If cross-crate attributes are NOT visible:
- Document the limitation.
- Keep path-based heuristics as the primary recognition mechanism.
- Phase 4 tasks 27-29 become path-based registry builders instead.

## Non-Goals

- No full registry extraction (tasks 27-29).
- No changes to the RawFrontendArtifact.
- No golden file changes.

## Test Plan

- Run the PL011 full-spec extraction with the validation pass enabled.
- Verify the pass finds at least:
  - 4 `rseq::intrinsic` attributes from mmio.rs (mmio_read, mmio_write,
    mmio_read_field, mmio_write_field).
  - At least 1 `rseq::intrinsic` attribute from each of: clock.rs,
    reset.rs, pinctrl.rs, intrinsics/ (to catch partial cross-crate
    visibility failures).
  - 1 `rseq::abstraction` attribute (clock).
  - 1 `rseq::framework_type` attribute (serial-config or any framework type).
- If the pass finds zero attributes, document the failure and record
  the fallback strategy.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to review. Verify the validation
covers all four attribute namespaces. If cross-crate visibility fails,
verify the fallback plan is documented. Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- Validation pass implemented and results documented.
- If successful: green light for attribute-based registries (tasks 27-29).
- If failed: fallback plan documented, path-based heuristic strategy
  defined for tasks 27-29.
- Codex review approved.
