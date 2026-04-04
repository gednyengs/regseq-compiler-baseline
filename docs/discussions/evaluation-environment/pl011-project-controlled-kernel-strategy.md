# PL011 Project-Controlled Kernel Strategy

## Purpose

This note captures the environment decision that the first PL011 smoke path
should use a project-controlled `arm64` kernel tree rather than depending on
whatever distro kernel happens to ship inside a guest image.

This is an evaluation-environment decision, not a Linux codegen decision. It
exists to answer:

- where the `arm64` kernel build tree and headers come from
- how out-of-tree PL011 module builds should target the guest kernel
- how the project should control binding behavior for the emulated PL011
  device

## Core Decision

For the PL011 smoke path, the project should maintain a pinned host-side
`arm64` kernel tree and boot QEMU with that kernel directly.

That same kernel tree should also be the authoritative build target for
generated out-of-tree modules.

In shorthand:

1. prepare or build a project-controlled `arm64` kernel tree on the Ubuntu host
2. boot the QEMU `aarch64` `virt` guest with that kernel
3. build the generated PL011 module against that same kernel tree

## Why This Is The Right Default

This avoids several recurring problems:

- no ambiguity about which headers match the guest kernel
- no dependency on the guest distro's packaged kernel ABI
- no need to reverse-engineer the guest image's kernel/header provenance
- no confusion about whether host `x86_64` headers are being used by mistake
- more control over how the emulated PL011 device is claimed at boot

Most importantly, it lets the project control the competing in-kernel PL011
story. If the stock kernel already binds the emulated UART before the generated
module loads, the smoke loop becomes an environment-debugging problem instead
of a backend-validation problem.

## Separation Of Kernel And Root Filesystem

The guest root filesystem does not need to provide the authoritative kernel.

Recommended split:

- kernel: project-controlled host-side `arm64` kernel tree
- rootfs/disk image: simplest Linux guest image that supports module loading,
  SSH or file transfer, and log capture

This means the project can still use a convenient distro rootfs while keeping
kernel ABI and driver-binding behavior under project control.

## Recommended Host Tooling

On the Ubuntu 24.04 host, the minimum cross-toolchain should include:

```bash
sudo apt install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu
```

The host-side module build should support:

- `ARCH=arm64`
- `CROSS_COMPILE=aarch64-linux-gnu-`
- `KDIR=/path/to/project-controlled/arm64/kernel/build`

## Recommended Working-Directory Shape

Within `~/regseq-eval/`, reserve a location such as:

```text
~/regseq-eval/
  images/
  kernel-src/
    pl011-arm64/
  generated/
  results/
  scripts/
```

Where:

- `kernel-src/pl011-arm64/` is the pinned kernel tree used for the PL011 smoke
  path
- its build output or configured build directory is the canonical `KDIR`
  target for PL011 module builds

## Module Build Contract

Generated PL011 modules should be built against the project-controlled kernel
tree, not the host's running kernel and not whatever packaged headers happen to
exist inside the guest image.

The canonical shape is:

```bash
make \
  ARCH=arm64 \
  CROSS_COMPILE=aarch64-linux-gnu- \
  KDIR=$HOME/regseq-eval/kernel-src/pl011-arm64
```

If the kernel uses a separate out-of-tree build directory, `KDIR` should point
to that prepared build directory instead.

## Binding-Control Requirement

The environment should ensure that the generated PL011 driver has a realistic
chance to bind to the emulated device during smoke validation.

That means the project-controlled kernel must make an explicit choice about the
competing in-kernel PL011 path. Acceptable strategies include:

- disabling the competing in-kernel PL011 driver
- building it as a module and not loading it
- otherwise ensuring it does not claim the target device before the generated
  module is tested

The exact kernel-config choice belongs to environment setup, not backend
lowering.

## What This Solves

This strategy directly resolves:

- the `aarch64` cross-compilation contract
- the kernel-header provenance question
- the mismatch between host architecture and guest architecture
- the uncertainty around whether the default QEMU `virt` PL011 is already
  claimed by an in-kernel driver

## What This Does Not Settle

This note does not decide:

- the final Linux codegen layout
- the final PL011 callback lowering strategy
- the final benchmark methodology
- the exact kernel configuration fragment yet
- the exact rootfs image yet

It only fixes the environment-side kernel control strategy.

## Practical Rule For Follow-On Work

Until a stronger reason appears, evaluation-environment planning should assume:

- PL011 smoke uses a project-controlled `arm64` kernel tree
- QEMU boots that kernel directly
- generated PL011 modules are built against that same tree
- rootfs choice is secondary to kernel control

That is the cleanest way to keep PL011 smoke validation deterministic and
separate from backend debugging noise.
