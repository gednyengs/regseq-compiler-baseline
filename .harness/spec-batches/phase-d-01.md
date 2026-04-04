# Spec Batch: phase-d-01 - PL011 Linux lowering and backend task batch

## Description

Author the first Phase D batch for the ASPLOS implementation stream. This is
task-spec authoring work, not executable implementation work.

This batch should define the first lowering-oriented executable tasks that turn
the stabilized PL011 lowering input into Linux-specific output. The batch is
Linux-first by design: it should establish the shared lowering boundary,
PL011-to-Linux realization, Linux backend emission, and the first build/smoke
validation path without coupling Zephyr follow-up work into the same batch.

The concrete outcome should be a small executable task set that lets later
workers implement one reviewable path from the existing Phase C joined artifact
to generated PL011 Linux collateral, with a typed portable lowering plan,
typed Linux backend plan, and a realistic Ubuntu/QEMU-oriented validation loop.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`
- `docs/handoff.md`
- `docs/plans/asplos-implementation-plan.md`
- `archive/guide/lowering-contract.md`
- `experiments/mir-structured-codegen/report.md`
- `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
- `docs/discussions/device-class-serial/serial-device-class-spec.md`
- `docs/discussions/evaluation-environment/README.md`
- `docs/discussions/evaluation-environment/qemu-relative-evaluation-strategy.md`
- `docs/discussions/evaluation-environment/qemu-relative-evaluation-runbook.md`
- `docs/discussions/evaluation-environment/qemu-kvm-simple-linux-driver-smoke-baseline.md`
- `docs/discussions/evaluation-environment/pl011-linux-smoke-contract.md`
- `docs/discussions/linux-codegen/README.md`
- `docs/discussions/linux-codegen/linux-driver-layout-strategy.md`
- `docs/discussions/linux-codegen/lowering-plan-interface-boundary.md`
- `docs/discussions/linux-codegen/pl011-linux-first-slice.md`
- `tests/e2e/tests/pl011_lowering_input_stability.rs`
- `.harness/tasks/regseq-analysis/03-lowering-input-readiness-and-consistency-checks.md`
- `.harness/tasks/track-a-validation/06-pl011-lowering-input-stability-proof.md`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-lowering/`
- `.harness/tasks/regseq-linux-backend/`
- `.harness/tasks/track-a-validation/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs plus the
required harness metadata updates.

## Implementation Notes

Follow these fixed decisions:

- This is the first lowering-oriented task batch. Treat the existing Phase C
  lowering-input contract as fixed input context; do not reopen frontend,
  normalization, join, or readiness scope here.
- Linux comes first. Do not include Zephyr task specs in this batch. Zephyr
  follow-up belongs to `phase-d-02`.
- Keep `regseq-lowering` and `regseq-linux-backend` as distinct harness groups
  even if both land in `crates/regseq-backends/`.
- The first supported PL011 Linux slice should cover the already-validated
  PL011 behavior fixed by `tests/fixtures/pl011-full-spec` and
  `docs/discussions/linux-codegen/pl011-linux-first-slice.md`: lifecycle/probe
  registration, serial device-class integration, MMIO/register realization,
  interrupt registration, firmware-property reads, clock/reset/pinctrl
  handling, synchronization realization, logging, and timed polling/delay
  where the current declaration and readiness evidence already justify them.
- Later backend tasks should consume a stable typed lowering boundary rather
  than re-reading joined artifacts ad hoc.
- Use a portable `DriverLoweringPlan` and an OS-specific `LinuxBackendPlan` as
  the planning split. Do not describe Linux-specific realization structures as
  the global lowering contract.
- A generated JSON lowering-manifest or other external debug artifact may be
  added later if it proves useful, but it is not required for the first Linux
  compile/build/smoke bring-up.
- Use `archive/guide/lowering-contract.md` as the authority for compiler-owned
  Linux realization responsibilities, especially:
  - Driver Lifecycle
  - Register Access
  - Serial/UART Management
  - Device interrupt setup and handling
  - Diagnostic Logging
  - Synchronization
  - Peripheral clock management
  - Firmware property access
  - Hardware reset sequencing and post-reset polling
  - Pin controller and GPIO configuration
  - Timed CPU delay and sleep
- Use `experiments/mir-structured-codegen/report.md` as required reference
  material for readable C emission, cleanup-path lowering, per-backend
  specialization, and AST extensions needed for production Linux output.
- Before finalizing Linux backend and environment-backed validation task specs,
  the surrounding discussion material should make two prerequisites explicit:
  - the first Linux generated-driver layout and generated-versus-handwritten
    boundary
  - the Ubuntu/QEMU/KVM feedback loop, proven first with a simple Linux driver
    if needed before PL011-specific smoke validation is specified
- Validation task specs should assume Ubuntu 24.04 as the canonical Phase D
  host and should target a deterministic Linux build/smoke loop. The default
  first integration mode should be out-of-tree module style unless a cited
  reason requires tighter kernel-tree integration.
- The compiler's default generated output root should be repo-relative
  `output/linux/<driver-id>/`. If evaluation workflows need a copy in
  `~/regseq-eval/generated/`, that location should be treated as a consumer or
  archive location rather than the primary compiler output root.
- The first PL011 smoke-validation contract should follow
  `docs/discussions/evaluation-environment/pl011-linux-smoke-contract.md`:
  Ubuntu 24.04 host, QEMU `aarch64` `virt` guest, TCG acceptable for first
  smoke when KVM is unsuitable, and compile/build plus module-load,
  registration, and probe-if-available as the first smoke gate.
- Keep the batch narrow and decomposed. A typical split should separate:
  - shared typed lowering-boundary work
  - PL011-to-Linux lowering realization from the stable input contract
  - Linux backend emission/rendering
  - PL011 Linux build and smoke validation proof
- Task specs in this batch should preserve the paper's evaluation boundary:
  controlled relative comparison under fixed virtualized conditions, not
  absolute bare-metal performance claims.

## Non-Goals

Do not include:

- Zephyr lowering or Zephyr validation task specs
- E1000-facing work
- new DSL primitives or reopened frontend extraction design
- new normalized-model or joined-artifact design work
- PL011 PM-aware path support
- PL011 variant, revision, or quirk machinery
- modem-control/status, RS-485, loopback, or IrDA/SIR expansion
- final paper benchmark packaging or full benchmarking methodology
- physical-hardware performance claims
- making in-tree kernel replacement a prerequisite for the first Linux
  validation loop

## Acceptance Criteria

This batch is complete when:

- executable harness task specs exist under `.harness/tasks/regseq-lowering/`,
  `.harness/tasks/regseq-linux-backend/`, and any needed
  `.harness/tasks/track-a-validation/` entries
- the task split clearly establishes a portable typed lowering-plan boundary
  and a Linux backend-plan boundary for PL011 Linux lowering
- the task specs cover Linux-only lowering realization and Linux backend
  emission for the supported PL011 slice
- at least one validation task spec covers build-oriented proof and the first
  smoke-validation path for generated PL011 Linux output on the Ubuntu/QEMU
  workflow
- `tasks.json` references the new task IDs under the appropriate groups with
  dependency order that preserves Linux-first sequencing
- `phase-d-01` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-d-01` to `tasked`
   - populate `generated_task_ids` with the new task IDs
   - move `phase-d-02` to `ready_to_elaborate`
2. Update `.harness/tasks.json` with the new tasks and dependencies.
3. Do not modify unrelated tracker items.
