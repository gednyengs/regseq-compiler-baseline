# PL011 Evaluation Environment Experiment

## Purpose

This experiment exists to de-risk and clarify the PL011 evaluation environment
before Phase D task specs are finalized.

The goal is not to start with paper-quality benchmarking. The goal is to learn
the real environment and integration constraints first, then use that evidence
to close the remaining evaluation-environment discussion with confidence.

This experiment should answer:

- what host-side toolchain and packages are actually needed
- how the project-controlled `arm64` kernel should be sourced and configured
- what rootfs/image strategy is simplest and stable enough for bring-up
- how the QEMU `aarch64` `virt` environment should be launched
- how a custom PL011-like driver can claim a non-console test UART cleanly
- what the first reliable PL011 smoke-validation loop should be

## Scope

In scope:

- host-side cross-compilation setup for `aarch64`
- pinned Linux source selection for the PL011 environment
- building a project-controlled `arm64` kernel
- building a minimal rootfs
- booting QEMU `aarch64` `virt`
- DTB inspection and modification if needed
- experimenting with stock-versus-custom PL011 driver ownership of console/test
  UARTs
- recording what works and what fails

Out of scope for the first pass:

- full benchmark automation
- final paper-quality throughput/latency claims
- E1000 evaluation
- final Regseq-generated PL011 driver integration

## Relationship To Discussion Notes

This experiment is the evidence-producing companion to the evaluation
environment notes under:

- `docs/discussions/evaluation-environment/`

In particular, it is intended to help close the open items recorded in:

- `docs/discussions/evaluation-environment/open-items-and-closure-order.md`

The experiment should update those notes once a phase has produced a clear
answer.

## Working Assumptions

Current working assumptions carried into this experiment:

- Ubuntu 24.04 is the source-of-truth host
- PL011 smoke uses QEMU `aarch64` `virt`
- TCG is acceptable for first smoke on an `x86_64` host
- the PL011 path should use a project-controlled host-side `arm64` kernel tree
- generated/compiler output remains repo-local; external eval workspaces are
  consumer/archive locations

## Progressive Phases

### Phase 1: Toolchain

Questions:

- which host packages are actually required for `aarch64` kernel and module
  builds
- does the expected cross toolchain work cleanly on the Ubuntu host

Expected output:

- exact package list
- exact verification commands
- notes on any missing tools

### Phase 2: Kernel Source

Questions:

- what exact Linux source version should be used

Expected output:

- exact source provenance
- exact clone command and verification

### Phase 3: Kernel Configuration

Questions:

- what configuration baseline is best for QEMU `virt`
- what extra config is needed for PL011 experimentation

Expected output:

- exact configuration baseline
- notes on config-sensitive PL011 features
- decision on any config delta beyond `arm64 defconfig`

### Phase 4: Kernel Build

Questions:

- does the host environment build the pinned `arm64` kernel cleanly
- what exact build command should be treated as canonical for this experiment
- what artifacts and logs do we get from the first successful build

Expected output:

- exact build commands
- notes on resulting kernel artifacts
- build log notes

### Phase 5: Rootfs

Questions:

- is Buildroot the best first rootfs strategy
- what is the minimum package/tool set needed for first smoke

Expected output:

- chosen rootfs strategy
- exact build steps
- image artifact notes

### Phase 6: QEMU Boot

Questions:

- what exact QEMU launch shape is reliable for the project-controlled kernel
  and chosen rootfs
- what serial and storage setup is simplest

Expected output:

- canonical first boot command
- boot log notes
- confirmed console behavior

### Phase 7: Driver Integration Trials

Questions:

- can a modified stock PL011 driver be built as an out-of-tree module and made
  to claim the intended test UART
- what integration path is least fragile

Expected output:

- working driver integration path
- notes on what did not work
- first reliable load/bind/probe loop

## Subdirectories

- `notes/`: progressive findings, failures, and decisions
- `scripts/`: helper scripts created during the experiment
- `artifacts/`: small checked-in experiment artifacts only when appropriate

Do not check in large generated kernels, rootfs images, or build trees.

## Success Condition

This experiment is successful when it produces enough evidence to close the
remaining PL011 evaluation-environment discussion items and lets `phase-d-01`
task authors write validation tasks against a concrete environment strategy
instead of a hypothetical one.
