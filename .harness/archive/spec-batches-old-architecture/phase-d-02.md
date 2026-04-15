# Spec Batch: phase-d-02 - PL011 Zephyr lowering and validation task batch

## Description

Author the Zephyr follow-up task batch for PL011 lowering after the Linux
lowering batch (phase-d-01) has stabilized. This is task-spec authoring work,
not executable implementation work.

This batch should define executable tasks that turn the same stabilized PL011
lowering input already used by the Linux path into Zephyr-specific output. The
portable `DriverLoweringPlan` established by phase-d-01 is shared input; this
batch adds a `ZephyrBackendPlan` as the OS-specific realization boundary and
covers Zephyr backend emission and structural validation.

The concrete outcome should be a small executable task set that lets later
workers implement one reviewable path from the existing shared lowering plan to
generated PL011 Zephyr collateral, with a typed Zephyr backend plan, static
device registration, and a structural build/link validation loop.

## Input Context

Read before starting:

- `.harness/config.md`
- `.harness/SESSION.md`
- `.harness/task-spec-workflow.md`
- `.harness/task-spec-status.yaml`
- `.harness/tasks.json`
- `docs/plans/asplos-implementation-plan.md`
- `archive/guide/lowering-contract.md`
- `experiments/mir-structured-codegen/report.md`
- `.harness/spec-batches/phase-d-01.md`
- `docs/discussions/pl011-full-spec/pl011-full-spec-status.md`
- `docs/discussions/evaluation-environment/README.md`
- `.harness/tasks/regseq-lowering/`
- `.harness/tasks/regseq-linux-backend/`
- `.harness/tasks/track-a-validation/`

## Output Specification

Create or modify:

- `.harness/tasks/regseq-zephyr-backend/`
- `.harness/tasks/track-a-validation/`
- `.harness/tasks.json`

Update when done:

- `.harness/task-spec-status.yaml`

The output of this batch should be executable harness task specs plus the
required harness metadata updates. Any shared lowering extensions needed for
Zephyr should land in `regseq-lowering` task specs if they are OS-neutral, or
in `regseq-zephyr-backend` if they are Zephyr-specific.

## Implementation Notes

Follow these fixed decisions:

- This batch reuses the portable lowering-input contract and shared
  `DriverLoweringPlan` from phase-d-01 as fixed input. Do not reopen frontend,
  normalization, join, or readiness scope here.
- Do not re-derive shared lowering work already covered by phase-d-01. The
  portable `DriverLoweringPlan` is an input, not an output of this batch.
- Introduce a `ZephyrBackendPlan` as the Zephyr-specific realization boundary
  parallel to the Linux `LinuxBackendPlan` from phase-d-01.
- Use `archive/guide/lowering-contract.md` as the authority for Zephyr-specific
  realization responsibilities across all supported constructs:
  - **MMIO**: device_map / DEVICE_MMIO_RAM_PTR (static DT mapping)
  - **Register ops**: sys_read32 / sys_write32
  - **Parameters**: Kconfig CONFIG_* symbols only (compile-time, no runtime
    modification); writable params generate warnings
  - **Interrupts**: irq_config_func (static, no shared interrupt chains)
  - **Synchronization**: k_spin_lock / k_mutex_lock (no IrqSpinLock variant);
    binary semaphore for completion
  - **Clocks**: DT static lookup + clock_control_on/off
  - **Properties**: DT_INST_PROP_OR (compile-time DT property extraction)
  - **Reset**: reset_line_toggle_dt (DT-driven, direct toggle)
  - **Logging**: LOG_ERR / LOG_INF + LOG_MODULE_REGISTER (macro-based)
  - **Device lifecycle**: DEVICE_DT_INST_DEFINE (static registration, no
    dynamic probe/remove, no hotplug)
  - **Delays**: k_busy_wait / k_sleep
- Zephyr has no goto-cleanup pattern; drop-terminator lowering must use the
  Zephyr DEVICE_DEFINE wrapper and static initialization conventions.
- Zephyr init functions return int (error codes), not ERR_PTR.
- Keep `regseq-zephyr-backend` as a distinct harness group even though it
  shares the `crates/regseq-backends/` crate with the Linux backend.
- The same PL011 fixture (`tests/fixtures/pl011-full-spec`) is the specimen
  for both Linux and Zephyr paths.
- Keep the batch narrow and decomposed. A typical split should separate:
  - Zephyr backend plan construction from the shared lowering plan
  - Zephyr backend emission/rendering (C source generation)
  - PL011 Zephyr structural build validation
- Validation task specs should prove structural correctness: Zephyr output
  compiles and links, device registration via DEVICE_DT_INST_DEFINE is
  present, correct cleanup ordering, static DT property reads, and no runtime
  parameter mechanisms invoked. Performance parity with Linux is not required.
- The compiler's default generated output root for Zephyr should be
  repo-relative `output/zephyr/<driver-id>/`.
- Zephyr build validation should prove that generated source compiles against
  Zephyr headers (a Zephyr SDK or header stubs). Full Zephyr application build
  and flash validation is out of scope for the first structural proof.

## Non-Goals

Do not include:

- Linux lowering or Linux backend work (covered by phase-d-01)
- E1000-facing work
- new DSL primitives or reopened frontend extraction design
- new normalized-model or joined-artifact design work
- PL011 PM-aware Zephyr path support
- dynamic device registration or hot-plug mechanisms
- modem-control/status, RS-485, loopback, or IrDA/SIR expansion
- full Zephyr application build, flash, and runtime validation
- bare-metal or on-device performance testing
- paper benchmark packaging or benchmarking methodology
- changes to the shared portable lowering plan established by phase-d-01

## Acceptance Criteria

This batch is complete when:

- executable harness task specs exist under `.harness/tasks/regseq-zephyr-backend/`
  and any needed `.harness/tasks/track-a-validation/` entries
- the task split clearly establishes a Zephyr backend-plan boundary parallel to
  the Linux backend-plan boundary from phase-d-01
- the task specs cover Zephyr-specific backend emission for the supported PL011
  slice using the Zephyr OS mechanisms from the lowering contract
- at least one validation task spec covers structural build-oriented proof for
  generated PL011 Zephyr output (compile against Zephyr headers)
- `tasks.json` references the new task IDs under the appropriate groups with
  dependency order that preserves the shared-lowering-first sequencing
- `phase-d-02` can be marked `tasked`

## Update Instructions

After finishing this batch:

1. Update `.harness/task-spec-status.yaml`:
   - set `phase-d-02` to `tasked`
   - populate `generated_task_ids` with the new task IDs
   - move `phase-e-01` to `ready_to_elaborate` (if all its dependencies are
     satisfied)
2. Update `.harness/tasks.json` with the new tasks and dependencies.
3. Do not modify unrelated tracker items.
