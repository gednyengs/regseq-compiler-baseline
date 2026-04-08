# Task: regseq-linux-backend-01 - Linux artifact layout and backend emission scaffold

## Description

Define the first real Linux backend-emission scaffold for generated PL011
output.

This task should establish the emitted artifact contract under
`output/linux/pl011-full-spec/`, the backend-side file/layout boundary, and the
first Linux emitter entry points that consume `LinuxBackendPlan`.

This is emission scaffolding and artifact-layout work. It should not yet claim
full PL011 driver completeness.

## Continuity Note

This task remains accepted after the MIR-body pivot and the reopening of
downstream lowering/backend tasks.

What remains valid from this task:

- the typed `LinuxBackendPlan` to emitter boundary
- the emitted artifact contract and repo-local output layout
- the typed emitted-artifact manifest / scaffold entry point
- the rule that Linux emission consumes `LinuxBackendPlan`, not
  `AnalysisInput` directly

What this task does **not** claim after the MIR-body pivot:

- correctness of full callback/helper behavior emission
- correctness of cleanup-bearing path emission
- freedom from device-specific hardcoding in concrete Linux emitter logic

Those concerns are owned by the downstream reopened chain:

- `regseq-lowering-04`
- `track-a-validation-10`
- `regseq-linux-backend-02`
- `pl011-linux-validation-11`
- `track-a-validation-07`

So implementers and reviewers should treat this task as a valid scaffold/layout
result, not as proof that the Linux backend is behavior-complete or
backend-independent by itself.

## Input Context

Read before starting:

- `.harness/tasks/regseq-lowering/04-pl011-serial-lifecycle-and-isr-behavior-lowering.md`
- `docs/discussions/linux-codegen/linux-driver-layout-strategy.md`
- `docs/discussions/linux-codegen/lowering-plan-interface-boundary.md`
- `experiments/mir-structured-codegen/report.md`
- `docs/discussions/evaluation-environment/pl011-linux-smoke-contract.md`
- `crates/regseq-backends/src/linux.rs`

## Output Specification

Modify:

- `crates/regseq-backends/src/`
- `crates/regseq-backends/tests/`
- `output/linux/` fixtures or golden files only if narrow checked-in scaffolds
  are needed for deterministic tests

Add the Linux backend scaffold needed to turn `LinuxBackendPlan` into a stable
generated artifact set under `output/linux/pl011-full-spec/`.

## Implementation Notes

This task should define:

- the first generated Linux artifact set and directory layout for
  `pl011-full-spec`
- the emitter entry point from `LinuxBackendPlan` to generated artifacts
- the typed representation of emitted artifacts / output manifest returned by
  the emitter
- readable C/code-structure helpers needed for later Linux emission
- module/build collateral boundaries for out-of-tree build packaging

The first Linux artifact contract should be explicit about which generated files
exist, for example:

- generated driver source
- generated register/helper source if split out
- generated header(s) if needed
- `Makefile` or other out-of-tree build collateral
- optional metadata/helpers only if required by the first compile/smoke loop

The scaffold should preserve the authored top-level partitioning where it
meaningfully helps readability, especially around:

- driver-oriented code
- register-oriented code
- backend-owned build files

Readable C emission matters in this task. Use the MIR structured-codegen report
as the authority for:

- structured control-flow emission
- cleanup-path lowering
- avoiding opaque string-concatenation emitters where a small AST or templated
  renderer is justified

Do not turn this task into:

- full PL011 behavior emission
- guest build/smoke validation
- Zephyr emission
- JSON manifest requirements
- benchmark harness authoring

This task must not be interpreted as evidence that Linux emission is already
behavior-complete or free of device-specific hardcoding. It establishes the
emission/layout scaffold only; behavior-correct emission and backend
independence are downstream validation boundaries.

## Test Plan

Add tests to verify:

1. `LinuxBackendPlan` can reach a backend-emission entry point
2. the emitted artifact set and paths are deterministic
3. the generated output root is repo-local `output/linux/pl011-full-spec/`
4. the emitted file partitioning matches the declared backend contract
5. smoke-level golden or snapshot output remains readable and stable

Test commands:

- `cargo test -p regseq-backends`
- `cargo clippy -p regseq-backends -- -D warnings`

## Acceptance Criteria

- Linux backend emission has a real typed scaffold
- the generated artifact layout for PL011 is explicit and stable
- the backend consumes `LinuxBackendPlan`, not `AnalysisInput` directly
- repo-local output paths are fixed for the first PL011 Linux path
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
