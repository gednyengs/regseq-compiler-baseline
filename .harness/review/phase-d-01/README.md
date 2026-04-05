# Phase D-01 Review Packet

These are the authored harness task specs for `phase-d-01`.

They are ready for researcher review before implementation begins.

## Context Summary

This batch starts the first lowering-oriented Phase D work for PL011 Linux.

The task set reflects three decisions that were stabilized after the original
brief was written:

1. PL011 should be treated as `bus = "amba"`, not as a generic Linux
   `platform_driver` case.
2. The default PL011 evaluation path is the non-console second-UART path.
3. The current controlled evaluation environment is the device-scoped
   `~/regseq-eval/pl011/...` workspace with a pinned kernel, Buildroot rootfs,
   and owned/patched QEMU.

## Proposed Task Set

1. `regseq-lowering-01` - lowering module scaffold and typed plan boundary
2. `regseq-lowering-02` - PL011 supported-slice lowering into plans
3. `regseq-linux-backend-01` - Linux artifact layout and backend emission scaffold
4. `regseq-linux-backend-02` - PL011 Linux driver emission for supported slice
5. `track-a-validation-07` - PL011 Linux generated build and second-UART smoke proof
6. `track-a-validation-08` - PL011 second-UART hot-path functional comparison scaffold

## Dependency Order

- `regseq-lowering-01`
- `regseq-lowering-02` depends on `regseq-lowering-01`
- `regseq-linux-backend-01` depends on `regseq-lowering-02`
- `regseq-linux-backend-02` depends on `regseq-linux-backend-01`
- `track-a-validation-07` depends on `regseq-linux-backend-02`
- `track-a-validation-08` depends on `track-a-validation-07`

Full dependency details, including cross-batch prerequisites, are captured in
[proposed-tasks.json](./proposed-tasks.json).

## Review Focus

Researchers should review whether the batch:

- preserves the portable `DriverLoweringPlan` versus `LinuxBackendPlan` split
- reflects the AMBA-specific PL011 decision consistently
- keeps Linux-first scope narrow enough for `phase-d-01`
- uses the right first validation gate:
  non-console second-UART smoke
- scopes Loop C honestly as a hot-path scaffold rather than full paper
  benchmarking

## Key Supporting Notes

- [phase-d-01 brief](../../spec-batches/phase-d-01.md)
- [Lowering plan boundary](../../../docs/discussions/linux-codegen/lowering-plan-interface-boundary.md)
- [PL011 Linux first slice](../../../docs/discussions/linux-codegen/pl011-linux-first-slice.md)
- [PL011 smoke contract](../../../docs/discussions/evaluation-environment/pl011-linux-smoke-contract.md)
- [Evaluation current resolutions](../../../docs/discussions/evaluation-environment/current-resolutions-and-strategy.md)
- [PL011 unified spec boundary](../../../docs/discussions/pl011-full-spec/pl011-unified-spec-boundary.md)
- [Bus resource management](../../../docs/compiler-collateral/constructs/bus-resource-management.md)

## Draft Artifacts

- [proposed-tasks.json](./proposed-tasks.json)
- [regseq-lowering/01-lowering-module-scaffold-and-typed-plan-boundary.md](../../tasks/regseq-lowering/01-lowering-module-scaffold-and-typed-plan-boundary.md)
- [regseq-lowering/02-pl011-supported-slice-lowering-into-plans.md](../../tasks/regseq-lowering/02-pl011-supported-slice-lowering-into-plans.md)
- [regseq-linux-backend/01-linux-artifact-layout-and-backend-emission-scaffold.md](../../tasks/regseq-linux-backend/01-linux-artifact-layout-and-backend-emission-scaffold.md)
- [regseq-linux-backend/02-pl011-linux-driver-emission-for-supported-slice.md](../../tasks/regseq-linux-backend/02-pl011-linux-driver-emission-for-supported-slice.md)
- [track-a-validation/07-pl011-linux-generated-build-and-second-uart-smoke-proof.md](../../tasks/track-a-validation/07-pl011-linux-generated-build-and-second-uart-smoke-proof.md)
- [track-a-validation/08-pl011-second-uart-hot-path-functional-comparison-scaffold.md](../../tasks/track-a-validation/08-pl011-second-uart-hot-path-functional-comparison-scaffold.md)

## Capture Plan

If the researchers approve this batch, the next step is implementation work in
dependency order, starting with `regseq-lowering-01`.
