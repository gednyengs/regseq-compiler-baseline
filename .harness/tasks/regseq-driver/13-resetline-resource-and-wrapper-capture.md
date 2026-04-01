# Task: regseq-driver-13 - ResetLine resource and wrapper capture

## Description

Implement the narrow compiler-facing extraction scaffolding for the accepted
`ResetLine` resource field shapes and wrapper-method calls. This task should
prove `regseq-driver` can recover the immediate frontend shape of the reset
construct without introducing Layer 2 join work or lowering behavior.

## Input Context

Read before starting:

- crates/regseq-driver/src/extract/mod.rs
- crates/regseq-driver/src/extract/register_access.rs
- docs/compiler-collateral/compiler-baseline-overview.md
- docs/compiler-collateral/constructs/hardware-reset-sequencing-and-post-reset-polling.md

## Output Specification

Create:

- crates/regseq-driver/src/extract/reset.rs
- crates/regseq-driver/tests/reset.rs

Modify:

- crates/regseq-driver/src/extract/mod.rs

## Implementation Notes

Recognize and extract only the immediate shared reset frontend forms:

- `ResetLine` and `Option<ResetLine>` typed fields
- wrapper-method call sites:
  - `assert()`
  - `deassert()`
  - `toggle(Duration)`

The extraction output should remain internal to `regseq-driver`. A narrow
record such as `ResetSurface`, `ResetField`, and `ResetCallSite` is
appropriate.

Required capture details:

- field name
- whether the field is required (`ResetLine`) or optional (`Option<ResetLine>`)
- wrapper method identity and containing function/method name
- whether `toggle` was called with a `Duration`-typed argument

Required structural validations for this task:

- recognize only fields whose type is `ResetLine` or `Option<ResetLine>`
- `toggle` must take the shared `Duration` type
- only recognize wrapper-method calls whose receiver resolves to a known reset
  field or borrowed reset field

Keep this task bounded:

- do not perform field-name join into Layer 2 reset resources
- do not implement compiler-managed `acquire_reset`
- do not validate `reset_shared` / `reset_line_id`
- do not pull post-reset polling into this task

## Test Plan

Create:

- crates/regseq-driver/tests/reset.rs

Tests to implement:

1. Verify `ResetLine` and `Option<ResetLine>` fields are recognized.
2. Verify required vs optional field shape is preserved.
3. Verify wrapper-method call sites for `assert`, `deassert`, and `toggle` are
   recognized on known reset fields.
4. Verify `toggle` rejects non-`Duration` arguments deterministically.
5. Verify unrelated methods with the same names are not misclassified as reset
   wrapper calls.

Test command: `cargo test -p regseq-driver --test reset`

## Acceptance Criteria

- `regseq-driver` can reach the accepted reset resource and wrapper-call forms
- the task stays limited to immediate frontend capture and structural
  validation
- the new tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
