# Evaluation Environment Open Items And Closure Order

## Purpose

This note records what remains unresolved in the evaluation-environment
discussion after the Linux codegen discussion was closed for task authoring.

It exists so that the remaining environment work can be resolved one issue at a
time rather than reopened as a broad, moving target.

## Current State

The following environment decisions are already considered settled:

- Ubuntu 24.04 is the source-of-truth Phase D host
- PL011 smoke uses QEMU `aarch64` `virt`
- TCG is acceptable for the first PL011 smoke path on an `x86_64` host
- the simple-driver smoke baseline is a prerequisite before PL011 smoke
- the compiler output root is repo-relative `output/linux/<driver-id>/`
- `~/regseq-eval/generated/` is a consumer/archive location, not the canonical
  compiler output root
- PL011 smoke should prefer a project-controlled host-side `arm64` kernel tree
- generated PL011 modules should build against that same kernel tree

Those decisions are captured in the existing notes in this directory.

## Remaining Open Items

### 1. Exact PL011 binding strategy inside QEMU

This is the most important remaining environment question.

We need to decide how the generated PL011 driver will claim the intended test
device inside the QEMU environment without destabilizing the console path or
fighting a competing in-kernel driver.

Examples of candidate strategies include:

- DTB partitioning / dedicated test UART identity
- stock driver on console UART, generated driver on a separate test UART
- runtime unbind/rebind or driver override

This item should be resolved before finalizing the first PL011 smoke gate.

### 2. Exact project-controlled kernel workflow

We have decided to own the `arm64` kernel for the PL011 smoke path, but the
workflow is not yet fully fixed.

Still to decide:

- exact kernel source provenance strategy
- exact config strategy (`defconfig` plus fragment, or equivalent)
- exact options required for the first PL011 smoke path
- any explicit kernel-side choice needed for the competing stock PL011 path

### 3. Exact guest rootfs/image strategy

We still need to choose the first concrete guest image approach.

Still to decide:

- Buildroot versus a minimal distro image
- which tools/packages are required for first smoke
- which tools are deferred until later benchmarking

### 4. Exact first PL011 smoke success bar

We are close, but not final.

Still to decide:

- whether the first smoke proof requires only build + module load +
  registration
- or whether it requires a real probe on the intended test UART

This depends on Item 1.

### 5. Exact runbook-level loop

The strategy notes are in place, but we do not yet have one final concrete
workflow that ties together:

1. kernel build/preparation
2. rootfs/image preparation
3. QEMU launch shape
4. module transfer/load path
5. log/artifact capture path

This should be written after Items 1 through 4 are resolved.

## Closure Order

The recommended closure order is:

1. exact PL011 binding strategy inside QEMU
2. exact project-controlled kernel workflow
3. exact guest rootfs/image strategy
4. exact first PL011 smoke success bar
5. exact runbook-level loop

## Practical Rule

Do not try to finalize the full evaluation environment in one step.

Instead:

- close one item above
- update the relevant discussion notes
- then move to the next item

That is the intended working method for the remainder of the
evaluation-environment discussion.
