# regseq-driver-33: Unit test specimen attribute updates

## Description

Update the 33+ unit test specimens in `crates/regseq-driver/tests/` to
carry `#[rseq::]` attributes where needed. Test specimens are minimal
Rust driver specs that exercise individual extraction surfaces.

## Input Context

- `docs/discussions/models/regseq-model/execution-plan.md` (Phase 4.5)
- `crates/regseq-driver/tests/` (33+ test files)
- `crates/regseq/src/` (facade crate with updated attributes)
- `tests/fixtures/pl011-full-spec/src/driver.rs` (reference for attribute usage)

## Implementation Notes

### What needs updating

Test specimens that reference facade types may need to be updated
because:

1. `ErrorKind` was renamed to `SerialErrors` (already done in task
   regseq-driver-25, but verify all specimens are updated).
2. Register access patterns changed from wrapper methods to Mmio methods.
3. `#[rseq::pin_state]` attributes should be present on pin state
   marker types in specimens that test pin control.
4. All resource field attributes should be present per spec.md
   Section 4.2: `#[rseq::clock(id = "...")]` on Clock fields,
   `#[rseq::reset(id = "...")]` on ResetLine fields,
   `#[rseq::mmio(id = "...")]` on Mmio fields,
   `#[rseq::interrupt(id = "...")]` on Interrupt fields,
   `#[rseq::pinctrl(id = "...")]` on PinCtrl fields.

### Approach

1. Run `cargo test -p regseq-driver` to identify any compilation
   failures from the facade changes.
2. For each failing specimen, update it to use the new surface:
   - Replace `ErrorKind` with `SerialErrors` if not already done.
   - Replace wrapper method register access with Mmio method access
     if the specimen uses registers.
   - Add missing `#[rseq::]` attributes as needed.
3. For specimens that don't fail but should have new attributes for
   completeness (e.g., pin_state specimens), add the attributes.

### Scope

This task updates the test specimens to compile and run with the
current facade. It does NOT update expected outputs or golden files
(that's task 34).

## Non-Goals

- No golden file regeneration (task 34).
- No new test specimens.
- No extraction logic changes.

## Test Plan

- All 33+ test specimens compile.
- `cargo test -p regseq-driver` passes with no compilation failures
  from test specimens.
- Specimens using pin control have `#[rseq::pin_state]` attributes.
- Specimens using reset have `#[rseq::reset(id = "...")]` attributes.

## Review

After tests pass, use `codex:rescue` to review. Verify all specimens
compile and attribute updates are consistent with the facade changes.
Iterate until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
```

## Acceptance Criteria

- All test specimens compile with updated facade.
- Attribute additions are consistent with spec.
- All tests pass. Codex review approved.
