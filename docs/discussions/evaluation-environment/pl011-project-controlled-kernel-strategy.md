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

## Exact Workflow Contract

The current project-controlled kernel workflow for PL011 is:

1. maintain one pinned kernel tree at:
   - `~/regseq-eval/pl011/kernel-src/pl011-arm64-linux`
2. pin one exact upstream kernel revision:
   - `v6.18.21`
   - commit `44c944a679974c2d18ee9b87070456d34193f3d4`
3. use one baseline kernel configuration:
   - `arm64 defconfig`
4. record any future kernel-config changes as a small explicit fragment rather
   than letting ad hoc edits accumulate in `.config`
5. build all required kernel artifacts from that same tree:
   - `Image`
   - modules
   - `dtbs`
6. build all generated or stand-in out-of-tree PL011 modules against that same
   tree via `KDIR`
7. boot QEMU only with the `Image` produced by that same pinned tree

The invariant is:

- host kernel build tree
- guest boot kernel
- out-of-tree module build target

must all come from the same pinned kernel provenance.

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
  pl011/
    images/
    kernel-src/
      pl011-arm64-linux/
    qemu-src/
    rootfs-src/
    generated/
    results/
    scripts/
```

Where:

- `kernel-src/pl011-arm64-linux/` is the pinned kernel tree used for the PL011
  smoke path
- its build output or configured build directory is the canonical `KDIR`
  target for PL011 module builds

This device-scoped layout is preferred because PL011 and E1000 are expected to
use different kernels, QEMU setups, rootfs policies, and benchmark workflows.

## Canonical Commands

Kernel configure:

```bash
cd ~/regseq-eval/pl011/kernel-src/pl011-arm64-linux
make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- defconfig
```

Kernel build:

```bash
cd ~/regseq-eval/pl011/kernel-src/pl011-arm64-linux
make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- -j$(nproc) Image modules dtbs
```

## Module Build Contract

Generated PL011 modules should be built against the project-controlled kernel
tree, not the host's running kernel and not whatever packaged headers happen to
exist inside the guest image.

The canonical shape is:

```bash
make \
  ARCH=arm64 \
  CROSS_COMPILE=aarch64-linux-gnu- \
  KDIR=$HOME/regseq-eval/pl011/kernel-src/pl011-arm64-linux
```

If the kernel uses a separate out-of-tree build directory, `KDIR` should point
to that prepared build directory instead.

## Binding-Control Requirement

The environment should ensure that the generated PL011 driver has a realistic
chance to bind to the emulated device during smoke validation.

For the current PL011 environment, that control is achieved by environment
topology rather than by disabling the stock console driver:

- keep stock built-in PL011 available for the console UART on `ttyAMA0`
- reserve the second PL011 test UART for the generated or stand-in driver
  through the patched-QEMU plus DT `arm,primecell-periphid` strategy

So the kernel workflow does not currently require a special PL011-disabling
config fragment just to make smoke validation possible.

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
- the exact rootfs image yet

It only fixes the environment-side kernel control strategy.

## Practical Rule For Follow-On Work

Until a stronger reason appears, evaluation-environment planning should assume:

- PL011 smoke uses a project-controlled `arm64` kernel tree
- QEMU boots that kernel directly
- generated PL011 modules are built against that same tree
- rootfs choice is secondary to kernel control
- `arm64 defconfig` is the baseline unless a recorded fragment becomes
  necessary

That is the cleanest way to keep PL011 smoke validation deterministic and
separate from backend debugging noise.
