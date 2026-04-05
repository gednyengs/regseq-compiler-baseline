# Phase 5 Rootfs Strategy

## Purpose

This note records the first rootfs strategy for the PL011 evaluation
environment experiment.

The goal of this phase is not to produce a benchmark-heavy guest image. The
goal is to produce the simplest reproducible root filesystem that supports:

- booting on QEMU `aarch64` `virt`
- a stable PL011 console session
- kernel module loading and unloading
- basic inspection and debugging during bring-up

## Chosen Direction

Use Buildroot as the first rootfs implementation strategy.

Start from:

```bash
git clone --depth 1 --branch 2026.02 \
  https://github.com/buildroot/buildroot.git
cd buildroot
make qemu_aarch64_virt_defconfig
```

Pinned Buildroot revision:

- tag: `2026.02`
- commit: `52ee2f5644da5389634b2465f1dc31deb5a4807c`

This is the preferred first rootfs path because:

- the project already controls the kernel, so the rootfs does not need to own
  the kernel ABI
- Buildroot is reproducible and easy to reshape
- it is a good fit for a small bring-up-oriented guest image
- it avoids unnecessary distro-kernel coupling during early environment work
- a shallow clone is sufficient for this experiment because we need a pinned
  reproducible source tree, not Buildroot history

## Baseline Adjustment Strategy

The starting Buildroot baseline is:

```bash
make qemu_aarch64_virt_defconfig
```

But that defconfig is broader than what this experiment wants. In particular,
it enables:

- a Buildroot-built Linux kernel
- a Buildroot-hosted QEMU package
- a canned post-image QEMU launcher workflow

For this experiment, the rootfs should stay independent from the
project-controlled kernel and from the later hand-authored QEMU launch flow.

So the first-pass Buildroot config delta is:

- disable `BR2_LINUX_KERNEL`
- disable `BR2_PACKAGE_HOST_QEMU`
- enable `BR2_PACKAGE_KMOD`
- clear `BR2_ROOTFS_POST_IMAGE_SCRIPT`

Practical result:

- Buildroot focuses on producing the root filesystem image
- the project-controlled `v6.18.21` kernel remains authoritative
- QEMU launch policy remains under experiment control rather than inherited
  from Buildroot board scripts
- module loading support is explicitly available in the guest userspace

The current first-pass rootfs profile still keeps:

- Buildroot internal toolchain
- BusyBox init/userspace
- ext4 rootfs image generation
- console login on `console`

The internal toolchain is intentionally retained for the first pass because it
is the lower-risk Buildroot path, even if it is slower than reusing an external
toolchain.

## First-Pass Rootfs Goals

The first image should optimize for bring-up, not for final evaluation.

Required capabilities:

- boot with the project-controlled `arm64` kernel
- mount the root disk under QEMU `virt`
- expose a usable shell on the serial console
- support `insmod`, `modprobe`, `rmmod`, and `dmesg`
- support enough basic shell/file utilities to inspect the environment

Preferred but still lightweight:

- `kmod`
- `util-linux`
- simple process/file utilities when BusyBox coverage is insufficient

Optional convenience features:

- host/guest sharing support if the console-only workflow becomes too awkward
- SSH only if the serial-console workflow becomes a bottleneck

## Explicit Deferrals

Do not optimize the first image for benchmarking yet.

Defer the following until after first boot and module-loading success:

- `perf`
- `stress-ng`
- `pv`
- `iperf3`
- broader network-testing tools
- any package added only for later benchmark automation

## Relationship To Kernel Strategy

This rootfs strategy assumes:

- the project-controlled host-side `arm64` kernel remains the authoritative
  kernel
- the guest rootfs is a userspace/container for boot and testing
- kernel headers and module ABI should continue to come from the pinned host
  kernel tree, not from the rootfs

## Device-Specific Guest Workflow Policy

This Buildroot image should currently be understood as a PL011-oriented guest
profile, not as a universal profile for every evaluation target.

For the PL011 workflow, the preferred operating mode is:

- no guest NIC by default
- 9p-first for host/guest artifact sharing
- no automatic `eth0` bring-up in the default PL011 image
- manual network bring-up only when a specific experiment requires it

This recommendation follows from the current experiment results:

- PL011 development depends on the serial console and the second UART, not on
  guest networking
- `-nic none` avoids unnecessary console noise
- current Buildroot defaults still try to configure `eth0`, so the remaining
  cleanup is guest-side userspace policy rather than kernel or QEMU topology

This policy should not be overgeneralized to network-oriented devices such as
E1000, where guest networking is part of the device-under-test setup.

## Guest-Network Bring-Up Fix

The concrete Buildroot-side fix for the PL011 guest profile is:

- set `BR2_SYSTEM_DHCP=""`

This removes the auto-generated `eth0` DHCP stanza from:

- `/etc/network/interfaces`

so the default SysV `S40network` script brings up only loopback instead of
waiting for a missing NIC.

One incremental-build detail is worth recording:

- changing `BR2_SYSTEM_DHCP` in `.config` was not enough by itself to refresh
  the already-installed `ifupdown-scripts` payload in `output/target`
- the fix required forcing:
  - `make ifupdown-scripts-reinstall`
  - followed by a normal `make` to regenerate the rootfs image

After that reinstall, the generated `interfaces` file contained only:

```text
# interface file auto-generated by buildroot

auto lo
iface lo inet loopback
```

This is the intended default PL011 guest behavior.

## Frozen PL011 Rootfs Baseline

The rootfs direction is now narrow enough to freeze the current PL011 guest
baseline rather than continue treating it as open-ended exploration.

Baseline source:

- Buildroot `2026.02`
- start from `qemu_aarch64_virt_defconfig`

Current PL011-specific delta:

- `# BR2_LINUX_KERNEL is not set`
- `# BR2_PACKAGE_HOST_QEMU is not set`
- `BR2_PACKAGE_KMOD=y`
- `BR2_ROOTFS_POST_IMAGE_SCRIPT=""`
- `BR2_SYSTEM_DHCP=""`
- `BR2_TARGET_ROOTFS_EXT2_SIZE="60M"`

Repo-local baseline artifact:

- [buildroot-pl011-rootfs.fragment](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/experiments/pl011-eval-env/artifacts/buildroot-pl011-rootfs.fragment)

This should be treated as the current reproducible PL011 guest baseline for:

- module build/load/probe work
- no-NIC serial-console bring-up
- 9p-based artifact transfer

Still deferred from the baseline:

- benchmark-heavy tooling
- SSH-oriented workflow
- E1000-specific guest networking needs

## Practical Rule

For the first rootfs build:

1. keep the image minimal
2. prioritize module loading and serial-console bring-up
3. postpone benchmark tooling until the QEMU boot and PL011 integration path
   are working

## First Build Execution Notes

The first execution of the Buildroot image build surfaced two practical issues
that are worth recording for repeatability.

### 1. `LD_LIBRARY_PATH` environment issue

The first `make` attempt failed immediately because the host environment had:

```text
LD_LIBRARY_PATH=/usr/local/cuda/lib64:
```

The trailing `:` adds the current working directory to `LD_LIBRARY_PATH`.
Buildroot rejects this.

Observed failure shape:

```text
You seem to have the current working directory in your
LD_LIBRARY_PATH environment variable. This doesn't work.
```

Resolution:

- run the Buildroot build with `LD_LIBRARY_PATH` unset

Concrete command used:

```bash
env -u LD_LIBRARY_PATH make -j24
```

### 2. Hung `util-linux` source download

After the `LD_LIBRARY_PATH` fix, the build progressed but the download for:

- `util-linux-2.41.3.tar.xz`

stalled at `0` bytes inside Buildroot's temporary output path.

The URL itself was still reachable with standalone `curl` and `wget --spider`,
so this appeared to be a stuck Buildroot download instance rather than a dead
mirror.

Resolution:

1. stop the stuck Buildroot `wget`/`make` process chain
2. manually download the tarball into Buildroot's `dl/` cache:

```bash
wget -O ~/regseq-eval/pl011/rootfs-src/buildroot/dl/util-linux/util-linux-2.41.3.tar.xz \
  https://cdn.kernel.org/pub/linux/utils/util-linux/v2.41/util-linux-2.41.3.tar.xz
```

3. restart the Buildroot build:

```bash
env -u LD_LIBRARY_PATH make -j24
```

The restarted build resumed cleanly and completed successfully.

## First Build Outcome

The first rootfs image was built successfully after the two environment-level
fixes above.

Artifacts produced:

- `output/images/rootfs.ext2`
- `output/images/rootfs.ext4` (symlink to `rootfs.ext2`)

Image size:

- `60M`

## Next Step

Use the built rootfs image together with the project-controlled kernel for the
first QEMU boot attempt, then record:

- whether the guest reaches a stable login prompt
- whether `ttyAMA0` is the active PL011 console
- whether the current machine/DTB setup exposes more than one guest-visible
  PL011 UART
