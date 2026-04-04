# QEMU Relative Evaluation Strategy

## Purpose

This note defines the repo-local evaluation strategy for the ASPLOS artifact
 and paper.

It updates the earlier external E1000-only notes into a project-local strategy
 that covers both target devices:

- PL011
- E1000

The central decision is:

- use QEMU-based evaluation as the primary paper methodology
- make only **controlled relative-comparison claims**
- avoid claiming absolute physical-hardware performance

## Paper Claim Boundary

The paper should treat the evaluation as:

- handwritten driver versus Regseq-generated driver
- under the same guest/kernel/toolchain/workload environment
- with the same virtual device model
- with the same host-side networking setup

This supports claims about:

- relative throughput
- relative latency
- relative guest CPU cost
- relative interrupt cost
- whether Regseq preserves the important fast paths closely enough to remain
  competitive with a handwritten baseline

This does **not** support claims about:

- bare-metal peak performance
- physical-device absolute efficiency
- direct extrapolation to legacy physical hardware

## Why QEMU Is The Right Default

For both devices, the main practical issue is hardware availability and
 repeatability:

- E1000 physical hardware is old and inconvenient to source
- PL011 hardware in the exact modeled form is also inconvenient to obtain

QEMU solves the important paper problem:

- it gives us a stable, reproducible environment
- it lets us hold the platform constant and swap only the driver
- it makes relative comparison feasible for both devices using the same
  evaluation philosophy

## Host Strategy

### Primary evaluation host

Use the Ubuntu 24.04 workstation as the source-of-truth environment for:

- Phase D lowering development
- generated-collateral debugging
- Linux-native build/smoke validation
- paper measurements

### Secondary host

macOS may still be useful for:

- code authoring
- artifact generation
- non-performance snapshot checks
- limited guest bring-up experiments

But it should **not** be the primary paper-evaluation host.

## Why macOS should not be the primary measurement host

This repository is currently being developed on an Apple Silicon Mac.

That matters:

- PL011 experiments can plausibly run in an `aarch64` guest environment on
  Apple Silicon with reasonable bring-up potential
- E1000 evaluation is much more naturally tied to an `x86_64` Linux guest
  environment
- on Apple Silicon, `x86_64` guest evaluation is not the clean KVM-style path
  we want for paper measurements
- host networking, TAP/bridge setup, and repeatable Linux-native benchmarking
  are all easier and more defensible on Ubuntu than on macOS

So the clean recommendation is:

- use macOS only for convenience workflows
- use Ubuntu for all reported evaluation runs

## Environment Policy

### Development/debug loop

The development/debug loop for lowering should happen primarily on Ubuntu:

- generate target collateral
- integrate it into the guest/kernel/module workflow
- boot the guest under QEMU/KVM
- debug failures there

This environment should also support an agentic compile/fix loop:

- generate Linux-target driver collateral
- place it into the chosen Linux integration layout
- compile it against the selected kernel tree or module headers
- capture compiler errors and warnings deterministically
- patch lowering/backend logic
- regenerate and rebuild

The key requirement is a deterministic build harness, not manual ad hoc steps.

### Measurement loop

The measurement loop should also happen on Ubuntu:

- same QEMU version
- same guest images
- same workload scripts
- same host-side networking configuration
- baseline and generated variants swapped under otherwise identical conditions

## Device-Specific Guidance

### E1000

Use QEMU/KVM `e1000` as the primary evaluation vehicle.

This is a controlled relative-comparison environment for:

- handwritten Linux `e1000`
- generated Linux `e1000`

### PL011

Use a QEMU-based Linux guest setup for PL011 as well, rather than attempting
 to find specific physical PrimeCell hardware.

This keeps the methodology aligned with E1000:

- same paper evaluation philosophy
- same host-side validation environment
- same relative-comparison framing

For the first PL011 smoke path, the environment should prefer a
project-controlled `arm64` kernel tree over a guest distro kernel. That gives
the project control over:

- the module-build target headers/tree
- the guest kernel ABI
- whether a competing in-kernel PL011 driver claims the emulated device first

See also:

- [pl011-project-controlled-kernel-strategy.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/evaluation-environment/pl011-project-controlled-kernel-strategy.md)

## Validation Layers

The evaluation strategy should separate three layers:

1. local structural validation
- deterministic snapshots
- shape/readability checks
- lowering-input traceability

2. Linux-native build and smoke validation
- generated collateral integrates and builds in the Ubuntu workflow
- guest boots
- driver probes
- minimal workload succeeds

3. benchmark validation
- repeated workload runs under controlled conditions
- baseline versus generated comparison

## Agent-Friendly Development Loop

The Linux-first strategy should explicitly support LLM-agent development during
Phase D.

The intended loop is:

1. generate Linux collateral
2. integrate it into the selected wrapper/layout
3. compile on Ubuntu
4. collect errors and warnings
5. patch the lowering/backend implementation
6. regenerate and recompile

To make that feasible, the project should later standardize:

- one canonical generated-artifact output location
- one canonical Linux kernel tree or headers target
- one canonical integration mode for early iteration
- one canonical build command per target driver
- deterministic log/artifact capture locations

The likely first default for iteration is:

- out-of-tree module style build

because it gives faster compile/debug turnaround than full kernel replacement.

## Fairness Rules

For every reported comparison, hold constant:

- guest kernel version
- guest userspace
- guest image baseline
- vCPU count
- guest memory size
- QEMU command line except the driver under test
- host networking topology
- benchmark tooling and scripts

Only the driver implementation should vary.

## Preferred Next Notes

The follow-on repo-local notes should define:

- the concrete QEMU/KVM topology and runbook
- the Linux-native lowering debug workflow
- the metric set and benchmark methodology
- artifact capture requirements for reproducibility
