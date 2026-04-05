# Task: track-a-validation-08 - PL011 second-UART hot-path functional comparison scaffold

## Description

Define and prove the first Loop C validation path for PL011: a non-console,
second-UART hot-path functional comparison between the handwritten baseline and
the generated driver under fixed virtualized conditions.

This task should establish the benchmark-oriented functional scaffold needed
for later paper evidence, without requiring final paper packaging or console
takeover.

## Input Context

Read before starting:

- `.harness/tasks/track-a-validation/07-pl011-linux-generated-build-and-second-uart-smoke-proof.md`
- `docs/discussions/evaluation-environment/current-resolutions-and-strategy.md`
- `docs/discussions/evaluation-environment/qemu-relative-evaluation-runbook.md`
- `docs/discussions/evaluation-environment/pl011-linux-smoke-contract.md`
- `experiments/pl011-eval-env/notes/phase-07-owned-qemu-second-pl011-strategy.md`

## Output Specification

Modify:

- `tests/e2e/tests/`
- `scripts/` or `experiments/pl011-eval-env/scripts/` for Loop C automation
- reviewer-facing validation notes or retained benchmark scaffolds only where
  needed for deterministic review

Add the first hot-path functional comparison scaffold for PL011 using the
reserved second UART as the device under test.

## Implementation Notes

Loop C should be defined as a controlled relative-comparison loop. It should
keep the following fixed across baseline-versus-generated runs:

- pinned host kernel tree
- rootfs profile
- owned/patched QEMU binary
- second-UART VM topology
- DTB / reservation strategy
- UART backend wiring

The DUT should be the reserved second PL011 UART, not the console UART.

The scaffold should define:

- which baseline driver is used
- which generated candidate is used
- how the host drives or observes the second UART
- how TX, RX, interrupt-driven behavior, and FIFO-related paths are exercised
- what metrics are retained for review:
  - throughput
  - latency
  - guest CPU utilization
  - interrupt counts
  - run-to-run variance or repeatability signals

This task should stay honest about its scope:

- it should create a real hot-path functional loop
- it should not promise the final paper benchmark suite
- it should not require console-driver takeover
- it should not force PM, DMA, or unrelated optional PL011 features into the
  comparison

## Test Plan

Add validation to verify:

1. the Loop C environment can run against both baseline and generated drivers
2. the second UART remains the DUT and the console UART remains untouched
3. at least one TX/RX plus interrupt-oriented workload path is runnable
4. collected results are deterministic enough for relative comparison
5. retained artifacts/metrics are reviewable

## Acceptance Criteria

- a concrete Loop C hot-path comparison scaffold exists
- it uses the reserved second UART rather than the console UART
- baseline-versus-generated runs can be performed under fixed conditions
- reviewer-facing retained outputs document the first comparison loop
- validation passes
