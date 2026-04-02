# Task: track-a-validation-04 - Serial device-class v1 regression update

## Description

Update the validation evidence so the stable v1 Serial device-class spec, not
the older six-method baseline, becomes the tested authority.

This task exists because the serial contract changed shape after the original
Phase B serial work. The goal is to make the regression surface explicit and
reviewable rather than relying on incidental compile breakage.

## Input Context

Read before starting:

- `docs/discussions/device-class-serial/serial-device-class-spec.md`
- `docs/discussions/device-class-serial/serial-os-support-matrix.md`
- `crates/regseq/tests/serial.rs`
- `crates/regseq-driver/tests/traits.rs`
- `tests/e2e/tests/pl011_spec_slice.rs`
- `tests/e2e/tests/pl011_frontend_qc.rs`
- `tests/fixtures/pl011-spec-slice/src/driver.rs`
- `tests/fixtures/pl011-full-spec/src/driver.rs`

## Output Specification

Modify only the validation artifacts actually affected by the serial v1
alignment. Likely targets include:

- `crates/regseq/tests/serial.rs`
- `crates/regseq-driver/tests/traits.rs`
- relevant `tests/e2e/tests/` files if they encode the older serial contract

Do not widen the validation surface beyond what the serial alignment changes.

## Implementation Notes

This task should:

- replace older validation assumptions that hard-code the six-method serial
  baseline
- encode the stable v1 method set and `SerialConsole` byte-slice signature as
  the new regression authority
- keep the tests focused on contract conformance, not backend lowering

Where an older test still provides value but reflects the superseded baseline:

- update it rather than deleting it blindly
- preserve the original intent where possible

Pay attention to both checked-in PL011 artifacts:

- `pl011-spec-slice`
- `pl011-full-spec`

Neither should remain as a hidden holdout for the older six-method serial
baseline after this regression update lands.

Do not include:

- new lowering tests
- RS-485, DMA, or modem-control validation
- serial-model normalization

## Test Plan

Run the updated targeted checks plus the usual safety net:

- `cargo test -p regseq --test serial`
- `cargo test -p regseq-driver --test traits`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

## Acceptance Criteria

- validation reflects the stable v1 Serial device-class contract
- no remaining targeted validation artifact assumes the superseded six-method
  contract where the new spec differs
- workspace tests and clippy pass
