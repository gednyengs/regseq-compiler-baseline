# Task: extraction-remediation-10 - PL011 serial-callbacks A extraction remediation

## Description

Implement the remediation defined in:

- `docs/validation/01-pl011/checks/remediation/10-serial-callbacks-extraction-a.md`

This task improves the compiler-side extraction boundary for the first serial
callback batch so that the remaining failing body predicate in `rx_char`
preserves exact symbolic constant identity and branch-local value mapping,
rather than only the lowered branch/count structure.

This is an implementation task, not a proof-only task.

## Input Context

Read before starting:

- `docs/validation/01-pl011/README.md`
- `docs/validation/01-pl011/01-authored-source.md`
- `docs/validation/01-pl011/checks/10-serial-callbacks-extraction-a.md`
- `docs/validation/01-pl011/checks/results/10-serial-callbacks-extraction-a.md`
- `docs/validation/01-pl011/checks/remediation/10-serial-callbacks-extraction-a.md`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `crates/regseq-driver/src/`
- `crates/regseq-model/src/`
- `crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs`

## Output Specification

Primary validation artifacts:

- `crates/regseq-driver/tests/source_to_extraction_10_serial_callbacks_a.rs`
- `docs/validation/01-pl011/checks/results/10-serial-callbacks-extraction-a.md`

Code changes may modify:

- `crates/regseq-driver/src/`
- `crates/regseq-model/src/` only if preserving exact symbolic constant
  identity requires extending shared MIR/body structures without weakening
  existing consumers

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
- Do not replace exact callback-body constant identity with source-only
  inference in the test or report.
- Preserve the currently passing facts for `port_open`, `port_close`,
  `configure`, and `tx_byte`, and preserve the already-strong structural proof
  for `rx_char`.
- Do not broaden into unrelated helper, backend, or non-callback extraction
  work.
- Do not add PL011-specific branches, helper-name pattern matches, or
  fixture-path special cases in the compiler.
- Any extraction improvement must generalize to structurally similar authored
  code outside the PL011 fixture.
- Use PL011 as the validation target, not as the implementation dispatch key.
- Do not add OS-specific branches or backend-specific shortcuts to satisfy the
  extraction contract.
- If this is solved via a richer HIR-side semantic layer, surface it as normal
  extracted compiler data available to standard consumers; do not add test-only
  metadata channels, callback-specific side paths, or proof-only recovery
  hooks.

## Implementation Notes

This task should:

- strengthen extraction so the task-owned validation can prove exact symbolic
  constant identity in callback bodies, not only structural branch/count facts
- preserve enough general expression, operand, or constant metadata to tie:
  - a tested condition
  - to the correct named constant or symbolic value used in the corresponding
    branch-local accumulation or assignment site
- prefer improving the compiler-side representation of named constants or
  constant-bearing body operations rather than compensating with weaker test
  heuristics
- keep the currently passing callback facts intact for the rest of the serial
  callbacks A batch, including:
  - identity / trait / signature / return-type facts
  - helper sequencing and `?` propagation for `port_open` and `configure`
  - parameter opacity for `configure`
  - staged vs fast-path branching for `tx_byte`
  - the already-passing structural `rx_char` facts: empty-FIFO branch, single
    raw `DR` read, repeated `field_from_raw`, error accumulation shape, and
    final `RxChar::new` / `Option` construction
- update the task-owned validation test only as needed to reflect the stronger
  extraction surface
- update the validation report after rerunning the task-specific test

This task should preserve enough general extraction detail to support:

- symbolic identity for named constants or enum/bitflag-like values used inside
  extracted callback bodies
- path-sensitive correspondence between a tested condition and the named value
  applied on the corresponding branch
- body-level validation that can distinguish specific named constants rather
  than only their lowered scalar residue when that residue loses authored
  meaning

This task should not:

- rewrite the validation check file
- accept coarse count-only proofs where the contract requires exact named
  flag-to-error mapping
- rely on PL011-specific path or constant-name recognition in production code
- make unrelated refactors in `regseq-driver` or `regseq-model`

## Test Plan

Run at minimum:

- `CARGO_TARGET_DIR=target/extraction-remediation-10 cargo test -p regseq-driver --test source_to_extraction_10_serial_callbacks_a -- --nocapture`
- `CARGO_TARGET_DIR=target/extraction-remediation-10 cargo test -p regseq-driver --test source_to_extraction_08_helper_methods_b -- --nocapture`
- `CARGO_TARGET_DIR=target/extraction-remediation-10 cargo test -p regseq-driver --test source_to_extraction_11_serial_callbacks_b -- --nocapture`
- `cargo test --workspace`

Use additional targeted tests only if needed to support the implementation.

The task-owned validation must still prove the PL011-specific authored facts
for the serial callbacks in group A, and must newly prove all four exact
PL011 `rx_char` error mappings:

- `OE -> OVERRUN`
- `BE -> BREAK`
- `PE -> PARITY`
- `FE -> FRAMING`

while preserving the already-passing proofs for:

- `port_open`
- `port_close`
- `configure`
- `tx_byte`
- the non-mapping structural facts already proved for `rx_char`

## Acceptance Criteria

- the extraction surface preserves enough exact symbolic constant identity and
  branch-local correspondence for the task-owned validation to prove all four
  authored `rx_char` error-flag to `ErrorKind` pairings
- currently passing serial-callbacks-A facts remain passing
- `source_to_extraction_10_serial_callbacks_a` passes
- `docs/validation/01-pl011/checks/results/10-serial-callbacks-extraction-a.md`
  is updated to reflect the new outcome accurately
