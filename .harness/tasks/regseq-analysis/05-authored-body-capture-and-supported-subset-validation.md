# Task: regseq-analysis-05 - MIR body availability and supported-subset validation

## Description

Expose MIR body data through analysis and validate that the supported PL011
slice has enough captured executable-body structure for later lowering,
without manually reconstructing behavior in analysis code.

This task is the prerequisite that makes honest behavioral lowering possible.

## Input Context

Read before starting:

- `.harness/tasks/regseq-driver/22-authored-callback-and-helper-body-extraction.md`
- `.harness/tasks/regseq-model/13-authored-body-schema-and-normalization.md`
- `.harness/tasks/regseq-analysis/02-analysis-input-and-lowering-bootstrap.md`
- `.harness/tasks/regseq-analysis/03-lowering-input-readiness-and-consistency-checks.md`
- `tests/fixtures/pl011-full-spec/src/driver.rs`
- `crates/regseq-analysis/src/`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/mod.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/syntax.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/statement.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/mir/terminator.rs`
- `/home/nyengele/.rustup/toolchains/nightly-2026-02-01-x86_64-unknown-linux-gnu/lib/rustlib/rustc-src/rust/compiler/rustc_middle/src/query/mod.rs`

## Output Specification

Modify:

- `crates/regseq-analysis/src/`
- `crates/regseq-analysis/tests/`

Add analysis-visible MIR body capture and supported-subset validation for
callbacks and reachable helpers extracted from
`mir_drops_elaborated_and_const_checked`.

## Implementation Notes

Implementation continuity note:

- a previous implementation attempt was rejected because the analysis layer
  was still exposing and validating HIR-derived `BodyExpr` bodies
- do not treat the existing HIR-body view as an acceptable fallback for the
  reopened task
- the concrete migration goal is:
  - expose MIR-derived executable bodies through `AnalysisInput`
  - compute helper reachability from the MIR-backed body set
  - validate supported MIR forms and cleanup-bearing paths directly from MIR
    data rather than from the old HIR body pipeline

This task should expose, through `AnalysisInput` or equivalent analysis-owned
interfaces:

- per-function MIR bodies
- callback/helper identity
- helper reachability
- supported-subset validation status

The analysis layer should validate that the supported PL011 slice has:

- captured MIR bodies for all authored callbacks needed by the current slice
- captured MIR bodies for all reachable helpers needed by those callbacks
- supported MIR forms for the current slice
- visible cleanup/drop semantics for representative failure/scope-exit paths
- enough source provenance for later traceability proof

For the supported PL011 slice, supported-subset validation should explicitly
reason about MIR-level patterns that matter to correctness, including:

- `?` / fallible control flow as desugared MIR
  - typically `SwitchInt`-based result discrimination with explicit error path
    cleanup / return behavior
- barrier calls introduced by the source-level `ordered!` macro expansion
  - `ordered!` itself does not survive as a MIR node
  - if ordering semantics are needed later, they must be recognized from the
    expanded synchronization call pattern rather than from a preserved macro marker

The analysis task should enumerate the expected-supported vs
expected-unsupported MIR forms for the current slice rather than leaving the
subset implicit.

Critical negative constraints:

- do not manually construct PL011 body trees in analysis code
- do not manually reconstruct drop elaboration or cleanup insertion in analysis
  code; analysis should consume MIR that already contains those semantics
- do not silently accept a different MIR flavor; if analysis depends on the
  body query flavor, require `mir_drops_elaborated_and_const_checked`
- do not introduce analysis types that are specific to PL011 registers,
  PL011 interrupt masks, FIFO service policies, or Linux serial APIs
- do not skip unsupported body constructs silently; surface them explicitly
- do not preserve source-level `ordered!` as if it survived macro expansion;
  only the expanded synchronization behavior may be recognized later

This task should make the captured MIR bodies available for lowering, but
should not itself perform portable semantic lowering.

## Test Plan

Add tests to verify:

1. `AnalysisInput` exposes per-function MIR body capture
2. all reachable helpers required by the supported PL011 slice are visible
3. supported-subset validation catches missing/unsupported MIR features
4. cleanup/drop semantics needed for later lowering are visible in analysis data
5. body data remains traceable to authored source/provenance
6. representative fallible control-flow patterns (`?`-style MIR) are validated
   deterministically
7. no analysis-local manual reconstruction is needed to populate the body view

Test commands:

- `cargo test -p regseq-analysis`
- `cargo clippy -p regseq-analysis -- -D warnings`

## Acceptance Criteria

- analysis exposes MIR-derived callback/helper bodies needed by the supported slice
- supported-subset validation is explicit and deterministic
- body data remains device-neutral and OS-neutral in representation
- later lowering can consume captured MIR bodies instead of synthesizing PL011 behavior
- cleanup/drop semantics required for correct lowering are available without
  redoing Rust's drop elaboration in analysis
- the supported MIR subset for the current slice is explicit rather than implicit
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
