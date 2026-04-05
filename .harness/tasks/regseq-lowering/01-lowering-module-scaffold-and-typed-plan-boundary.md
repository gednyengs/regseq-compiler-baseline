# Task: regseq-lowering-01 - Lowering module scaffold and typed plan boundary

## Description

Establish the first real lowering boundary for Phase D by introducing a
portable typed lowering layer inside `regseq-backends`.

This task should create the initial lowering module structure, define the
portable `DriverLoweringPlan` and Linux-specific `LinuxBackendPlan` boundary,
and add the first planner entry points that consume the already-stable
`AnalysisInput`.

This task is about architecture and typed planning structure. It should not yet
lower the full PL011 fixture or emit buildable Linux driver artifacts.

## Input Context

Read before starting:

- `docs/discussions/linux-codegen/lowering-plan-interface-boundary.md`
- `docs/discussions/linux-codegen/pl011-linux-first-slice.md`
- `docs/discussions/evaluation-environment/current-resolutions-and-strategy.md`
- `archive/guide/lowering-contract.md`
- `crates/regseq-analysis/src/input.rs`
- `crates/regseq-model/src/joined/artifact.rs`
- `.harness/tasks/regseq-analysis/02-analysis-input-and-lowering-bootstrap.md`
- `.harness/tasks/regseq-analysis/03-lowering-input-readiness-and-consistency-checks.md`
- `.harness/tasks/track-a-validation/06-pl011-lowering-input-stability-proof.md`
- `crates/regseq-backends/src/`

## Output Specification

Modify:

- `crates/regseq-backends/src/`
- `crates/regseq-backends/tests/`

Add the first lowering-oriented modules, typed plan definitions, and boundary
tests needed to make later Linux backend work consume a stable internal plan
instead of re-reading `AnalysisInput` ad hoc.

## Implementation Notes

This task should follow these fixed decisions:

- `AnalysisInput` remains the stable lowering input boundary.
- `DriverLoweringPlan` is portable and compiler-owned.
- `LinuxBackendPlan` is Linux-specific and derived from
  `DriverLoweringPlan`, not from `AnalysisInput` directly.
- Both plan families may live inside `crates/regseq-backends` for the first
  implementation pass, but they must remain architecturally distinct.
- Do not introduce a JSON lowering manifest in this task.

The initial plan shapes should be rich enough to avoid a fake wrapper type.
At minimum, the task should define typed homes for:

- driver identity and provenance
- callback/control-flow inventory
- resource obligations:
  - MMIO
  - interrupts
  - properties
  - clocks
  - resets
  - pinctrl
  - bus identity
- acquisition and teardown ordering
- optional-resource policy
- device-class role obligations
- register-access and ordering obligations
- synchronization obligations
- callback-body lowering actions in a backend-usable structured form

For Linux-specific planning, define typed homes for:

- Linux driver registration shape
- Linux serial-core realization shape
- Linux probe/remove orchestration
- Linux resource-acquisition sequencing
- Linux artifact partitioning / emitted-file layout decisions

The task should also add the first planner API boundary, for example:

- a portable lowering entry point from `AnalysisInput` to `DriverLoweringPlan`
- a Linux planner entry point from `DriverLoweringPlan` to `LinuxBackendPlan`

Do not turn this task into:

- full PL011 lowering coverage
- Linux code emission
- compile/build smoke validation
- Zephyr work
- backend-independent IR redesign outside the accepted lowering boundary

## Test Plan

Add tests to verify:

1. the new lowering module compiles and is importable
2. `AnalysisInput` can reach a typed portable lowering-plan entry point
3. `LinuxBackendPlan` is typed separately from `DriverLoweringPlan`
4. the initial plan structures preserve provenance and ordering slots needed by
   later tasks
5. no Linux-specific realization types leak into the portable plan API

Test commands:

- `cargo test -p regseq-backends`
- `cargo clippy -p regseq-backends -- -D warnings`

## Acceptance Criteria

- `crates/regseq-backends` contains an explicit lowering module boundary
- a portable `DriverLoweringPlan` exists
- a distinct `LinuxBackendPlan` exists
- planner entry points are available for later tasks
- tests demonstrate the boundary is real rather than nominal
- tests pass
- `cargo test --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
