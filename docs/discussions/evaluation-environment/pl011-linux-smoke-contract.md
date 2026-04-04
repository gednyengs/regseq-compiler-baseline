# PL011 Linux Smoke Contract

## Purpose

This note defines the first PL011-specific smoke-validation contract for Phase
D Linux work.

It exists because the repo already has:

- a general QEMU relative-evaluation strategy
- a simple-driver Ubuntu/QEMU baseline note

but it did not yet have one concise note stating the first PL011 smoke target,
environment contract, and success signals.

## Prerequisite

The simple-driver baseline in
[`qemu-kvm-simple-linux-driver-smoke-baseline.md`](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/evaluation-environment/qemu-kvm-simple-linux-driver-smoke-baseline.md)
should be treated as a prerequisite environment proof before PL011 smoke work
is considered stable.

PL011 smoke validation should not be the first point where the project learns
whether the Ubuntu/QEMU feedback loop works at all.

## Host Contract

The source-of-truth host is:

- Ubuntu 24.04

The first PL011 smoke loop assumes:

- Linux module build tooling available on the host
- QEMU available on the host
- artifact/log capture paths are deterministic
- a project-controlled `arm64` kernel tree is available on the host for the
  PL011 guest and module builds

## Guest Contract

The first PL011 smoke target should be:

- QEMU `aarch64`
- `virt` machine

Reason:

- that is the natural first virtual environment for PL011-compatible hardware

## KVM Versus TCG

KVM acceleration is not required for the first PL011 smoke proof.

If the Ubuntu host is `x86_64` and the guest is `aarch64`, using TCG for the
PL011 smoke path is acceptable.

This is a smoke-validation contract, not a performance-evaluation contract.

## Kernel And Header Provenance

For the PL011 smoke path, the project should not rely on the guest distro
kernel as the authoritative kernel source.

Instead:

- the Ubuntu host should maintain a project-controlled `arm64` kernel tree
- QEMU should boot that kernel directly for the PL011 smoke path
- generated modules should build against that same kernel tree

This should be treated as the canonical answer to the kernel-headers question.
It keeps guest-kernel ABI, module-build provenance, and device-binding
behavior under project control.

See also:

- [pl011-project-controlled-kernel-strategy.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/evaluation-environment/pl011-project-controlled-kernel-strategy.md)

## Cross-Compilation Note

When the host is `x86_64` and the PL011 guest is `aarch64`, the out-of-tree
module build requires:

- an `aarch64` cross-compiler
- an `arm64` kernel build tree prepared for external module builds
- `ARCH=arm64`
- `CROSS_COMPILE=aarch64-linux-gnu-`

The minimum expected Ubuntu host packages are:

```bash
sudo apt install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu
```

## Generated Artifact Contract

The compiler's default generated output root should remain repo-relative:

- `output/linux/<driver-id>/`

For the PL011 fixture, that means:

- `output/linux/pl011-full-spec/`

If evaluation scripts or runbooks need a copy in a separate Ubuntu workspace
such as `~/regseq-eval/generated/`, that location should be treated as a
consumer/archive location rather than the compiler's primary output root.

## First Smoke Goals

The first PL011 smoke path must prove:

1. generated PL011 Linux artifacts build as an out-of-tree module
2. the resulting module metadata is sane enough for inspection
3. the guest boots reproducibly
4. the module can be made available inside the guest
5. module load succeeds
6. driver registration succeeds
7. probe runs once the environment intentionally allows the generated driver to
   claim the target PL011-compatible device
8. logs can be captured deterministically

This is enough for the first smoke proof.

## First Smoke Success Signals

The first PL011 smoke contract should consider the loop successful when the
following signals are available:

- host-side build success for the generated out-of-tree module
- retained build log
- sane `modinfo` or equivalent module-metadata inspection
- guest boot success
- successful module insertion
- expected registration log messages
- probe log messages once the environment is configured so the generated driver
  can own the target device
- deterministic artifact/log capture locations

The first smoke proof does not require full UART I/O benchmarking.

## What This Contract Does Not Require Yet

The first PL011 smoke contract does not require:

- KVM acceleration for the PL011 path
- final paper benchmark packaging
- full serial-subsystem behavioral proof
- PM validation
- DMA validation
- in-tree kernel replacement

Those belong to later validation or evaluation work.

## Recommended Artifact Retention

For each PL011 smoke run, retain at minimum:

- generated artifact directory identifier
- build log
- module build outputs
- guest image identifier
- QEMU command line
- host kernel/QEMU metadata
- guest kernel version
- relevant guest log excerpt

These should be retained in stable reviewable locations, whether repo-local or
under an external evaluation workspace.

## Practical Rule For Task Authors

When authoring `phase-d-01` validation tasks:

- require the simple-driver baseline as prerequisite context
- use Ubuntu 24.04 as the source-of-truth host
- use QEMU `aarch64` `virt` as the first PL011 smoke target
- allow TCG when the host/guest architecture combination makes KVM unsuitable
- treat the project-controlled `arm64` kernel tree as the canonical guest
  kernel and module-build target
- treat compile/build/module-load/registration/probe-if-available as the first
  smoke gate

That is the intended first PL011 Linux validation contract.
