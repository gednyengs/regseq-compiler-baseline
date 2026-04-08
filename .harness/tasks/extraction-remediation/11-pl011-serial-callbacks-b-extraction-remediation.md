# Task: extraction-remediation-11 - PL011 serial-callbacks B extraction remediation

## Description

Implement the remediation defined in:

- `docs/validation/01-pl011/checks/remediation/11-serial-callbacks-extraction-b.md`

This task improves the compiler-side extraction boundary for the second serial
callback batch so that the remaining failing body predicates preserve exact
field-level identity, symbolic `FieldValue` identity, ordered value use, and
branch-local value mapping rather than only structural register/call shape.

This is an implementation task, not a proof-only task.

## Input Context

Read before starting:

- `docs/validation/01-pl011/README.md`
- `docs/validation/01-pl011/01-authored-source.md`
- `docs/validation/01-pl011/checks/11-serial-callbacks-extraction-b.md`
- `docs/validation/01-pl011/checks/results/11-serial-callbacks-extraction-b.md`
- `docs/validation/01-pl011/checks/remediation/11-serial-callbacks-extraction-b.md`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs`
- `crates/regseq-driver/src/extract/mir_extract.rs`
- `crates/regseq-driver/src/extract/body.rs`
- `crates/regseq-driver/src/extract/identity_enrichment.rs`
- `crates/regseq-driver/src/extract/register_access.rs`
- `crates/regseq-driver/src/mir_repr.rs`

Read only if needed:

- `tests/fixtures/pl011-full-spec/src/registers.rs`
- `crates/regseq-model/src/`

## Output Specification

Primary validation artifacts:

- `crates/regseq-driver/tests/source_to_extraction_11_serial_callbacks_b.rs`
- `docs/validation/01-pl011/checks/results/11-serial-callbacks-extraction-b.md`

Code changes may modify:

- `crates/regseq-driver/src/`
- `crates/regseq-model/src/` only if preserving exact `Field` / `FieldValue`
  symbolic identity requires extending shared MIR/body structures without
  weakening existing consumers

The task should update the validation report to reflect the new extraction
behavior after the implementation lands and the test is rerun.

If the implementation is incomplete or still leaves failing checks:

- keep the report honest about what still fails
- record a short descriptive `fail_reason` in the harness
- summarize the remaining gap rather than weakening the check contract

## Constraints

- Do not weaken the extraction check contract.
- Do not change authored-source facts to make the task easier.
- Do not edit unrelated `source-to-extraction-*` test files or reports.
- Keep changes local to the serial-callback body extraction boundary and its
  supporting data structures.
- Only upgrade a failed check to `pass` if the new exercised evidence actually
  proves it at the required specificity.
- Do not replace exact callback-body field/value identity with source-only
  inference in the test or report.
- Preserve the currently passing facts for `flush_rx_fifo` and for callback
  identity / trait role / signature / return type across the whole batch.
- Do not broaden into unrelated helper, backend, or non-callback extraction
  work.
- Do not add PL011-specific branches, helper-name pattern matches, or
  fixture-path special cases in the compiler.
- Any extraction improvement must generalize to structurally similar authored
  code outside the PL011 fixture.
- Use PL011 as the validation target, not as the implementation dispatch key.
- Do not add OS-specific branches or backend-specific shortcuts to satisfy the
  extraction contract.
- If this is solved via a richer HIR-side semantic layer or other pre-MIR
  recovery path, surface it as normal extracted compiler data available to
  standard consumers; do not add test-only metadata channels,
  callback-specific side paths, or proof-only recovery hooks.

## Implementation Notes

This task should:

- strengthen extraction so the task-owned validation can prove exact symbolic
  identity for `Field` and `FieldValue` constants used in callback bodies, not
  only register-module type arguments and control-flow shape
- preserve enough general operand or constant metadata to prove:
  - `tx_ready` uses `FR_REG::TXFF::FIELD`
  - `tx_empty` uses `FR_REG::BUSY::FIELD`
  - `flush_tx_fifo` performs two ordered `modify` calls with
    `LCRH_REG::FEN::DISABLED` then `LCRH_REG::FEN::ENABLED`
  - `send_break` maps `on == true` to `LCRH_REG::BRK::ENABLED` and
    `on == false` to `LCRH_REG::BRK::DISABLED`
- prefer improving the compiler-side representation of constant-bearing call
  arguments and branch-local value flow rather than compensating with weaker
  test heuristics
- keep field/value identity modeling internally coherent with existing
  register-description extraction rather than creating a second conflicting
  notion of register-field identity
- keep the currently passing callback facts intact for the rest of the serial
  callbacks B batch, including:
  - exact identity / trait / signature / return-type facts
  - `flush_rx_fifo` loop and drain proof
  - existing structural proof that these bodies read or modify the expected
    register families in the expected control-flow shape
- update the task-owned validation test only as needed to reflect the stronger
  extraction surface
- update the validation report after rerunning the task-specific test

This task should not:

- rewrite the validation check file
- accept coarse register-family or call-count proofs where the contract
  requires exact field/value identity
- rely on PL011-specific path or constant-name recognition in production code
- make unrelated refactors in `regseq-driver` or `regseq-model`

## Test Plan

Run at minimum:

- `CARGO_TARGET_DIR=target/extraction-remediation-11 cargo test -p regseq-driver --test source_to_extraction_11_serial_callbacks_b -- --nocapture`
- `CARGO_TARGET_DIR=target/extraction-remediation-11 cargo test -p regseq-driver --test source_to_extraction_10_serial_callbacks_a -- --nocapture`
- `CARGO_TARGET_DIR=target/extraction-remediation-11 cargo test -p regseq-driver --test source_to_extraction_08_helper_methods_b -- --nocapture`
- `cargo test --workspace`

Use additional targeted tests only if needed to support the implementation.

## Acceptance Criteria

- the extraction surface preserves enough exact field/value symbolic identity
  and branch-local correspondence for the task-owned validation to prove:
  - `tx_ready`
  - `tx_empty`
  - `flush_tx_fifo`
  - `send_break`
- any new field/value identity used to satisfy those proofs is represented as
  normal extracted compiler data with coherent producer/consumer semantics,
  not as inert proof-only or test-only metadata
- currently passing serial-callbacks-B facts remain passing, including
  `flush_rx_fifo`
- `source_to_extraction_11_serial_callbacks_b` passes
- `docs/validation/01-pl011/checks/results/11-serial-callbacks-extraction-b.md`
  is updated to reflect the new outcome accurately
