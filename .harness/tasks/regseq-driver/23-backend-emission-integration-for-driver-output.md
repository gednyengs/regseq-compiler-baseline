# Task: regseq-driver-23 - Backend emission integration for driver output

## Description

Wire the canonical Regseq compiler path in `regseq-driver` to the now-complete
lowering and Linux backend emission pipeline so `EmitSpec::Driver` produces a
real generated artifact set for `pl011-full-spec`.

This task exists because the backend and lowering layers can now generate
reviewable Linux output, but the wrapper-driven compiler path still stops at
`after_analysis` and does not invoke backend emission at all. Validation tasks
must not compensate for that missing production path by constructing backend
plans directly in tests.

## Input Context

Read before starting:

- `.harness/tasks/regseq-driver/03-config-and-compiler-setup.md`
- `.harness/tasks/regseq-driver/19-raw-frontend-artifact-emission.md`
- `.harness/tasks/regseq-analysis/05-authored-body-capture-and-supported-subset-validation.md`
- `.harness/tasks/regseq-lowering/04-pl011-serial-lifecycle-and-isr-behavior-lowering.md`
- `.harness/tasks/regseq-linux-backend/02-pl011-linux-driver-emission-for-supported-slice.md`
- `.harness/tasks/track-a-validation/10-pl011-full-spec-lowering-completeness-and-backend-independence-proof.md`
- `crates/regseq-driver/src/callbacks.rs`
- `crates/regseq-driver/src/config.rs`
- `crates/regseq-backends/src/lowering/planner.rs`
- `crates/regseq-backends/src/linux/planner.rs`
- `crates/regseq-backends/src/linux/emitter.rs`
- `tests/fixtures/pl011-full-spec/`

## Output Specification

Modify:

- `crates/regseq-driver/Cargo.toml`
- `crates/regseq-driver/src/`
- `tests/e2e/tests/`
- narrow supporting test fixtures only if needed for deterministic compiler-path
  validation

Implement the real `EmitSpec::Driver` path in `regseq-driver` so the compiler's
`after_analysis` callback can:

1. construct the canonical analysis/bootstrap input from the emitted raw
   frontend artifact and platform file
2. invoke portable lowering
3. invoke Linux backend planning
4. emit generated Linux artifacts under the configured output root

## Implementation Notes

This task owns production-path integration only. It should not re-implement
lowering or backend logic inside `regseq-driver`.

The canonical execution path for `EmitSpec::Driver` should be:

1. `regseq-driver` extracts the raw frontend artifact during `after_analysis`
2. `regseq-driver` builds the declaration + binding + joined/analysis input
   through the existing model/analysis crates
3. `regseq-driver` calls the lowering planner and Linux backend planner
4. `regseq-driver` calls `emit_linux_artifacts(...)`
5. the generated artifact set is written under the configured output root

Required integration properties:

- `regseq-driver` must depend on the crates it needs for this production path:
  - `regseq-analysis`
  - `regseq-backends`
  - `regseq-model` where directly required
- the production path must be target-OS aware and route Linux generation
  through the Linux backend planner/emitter
- `EmitSpec::Driver` must no longer be a placeholder that only reports
  `after_analysis`
- the generated output location must align with the configured `output_dir`
  and backend artifact layout for `pl011-full-spec`
- emitted output must come from the production compiler path, not from
  hand-built test fixtures or direct backend test helpers

Constant-resolution honesty:

- if the currently supported production path requires explicit authored
  interrupt-constant provisioning for `pl011-full-spec`, that step must be
  implemented explicitly and transparently in the driver/compiler path
- do not hide this behind test-only helpers
- if the production path cannot resolve or provision what the backend needs,
  it must fail explicitly with a reviewer-comprehensible error rather than
  silently degrading

Do not:

- construct `AnalysisInput` ad hoc inside validation tests as a substitute for
  the production compiler path
- duplicate backend test fixture builders in e2e tests as the primary emission
  mechanism
- patch generated output by hand after emission
- embed Linux backend logic directly into `regseq-driver`

## Test Plan

Add tests to verify:

1. `EmitSpec::Driver` through the real wrapper-driven compiler path produces a
   generated Linux artifact set for `pl011-full-spec`
2. the emitted artifact set lands under the configured output root
3. the production compiler path surfaces any explicit constant-provisioning
   step honestly and deterministically
4. the wrapper-driven path no longer stops at “reached after_analysis” without
   generating output
5. emitted file layout matches the Linux backend contract

Test commands:

- `cargo test -p regseq-driver`
- `cargo test -p regseq-e2e -- --ignored`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`

## Acceptance Criteria

- `regseq-driver` owns a real `EmitSpec::Driver` production path for Linux
  generated output
- wrapper-driven compiler execution can emit `pl011-full-spec` Linux artifacts
  under the configured output root
- any required explicit constant-provisioning step is implemented honestly in
  the production compiler path and is visible in reviewer-facing proof
- validation tasks no longer need to assemble backend plans directly in order
  to get generated output
- tests pass
