# Phase 2 Kernel Config Policy

## Purpose

This note records the first proposed kernel-configuration policy for the PL011
evaluation-environment experiment.

The intent is not to optimize for final benchmarking yet. The intent is to
produce a stable QEMU `aarch64` `virt` environment with:

- a reliable stock PL011 console path
- support for out-of-tree module loading
- a simple virtio-backed rootfs boot path
- enough flexibility to experiment with a custom PL011-like driver on a test
  UART later

## Baseline

Start from the upstream ARM64 default configuration:

```bash
make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- defconfig
```

Then verify or adjust the options below.

For the current `v6.18.21` experiment tree, the practical outcome after
inspecting `arm64 defconfig` is:

- no additional `.config` changes are required for the first boot/integration
  attempt
- the important first-pass policy is to pin and verify the relevant symbols,
  not to add a large config delta on top of `defconfig`

## Core Policy Decisions

### 1. Stock PL011 driver built in

Preferred setting:

- `CONFIG_SERIAL_AMBA_PL011=y`
- `CONFIG_SERIAL_AMBA_PL011_CONSOLE=y`

Reason:

- the console UART should work from boot
- the experiment should keep a stable known-good stock path available
- console availability should not depend on loading a module later
- `ttyAMA0` should stay on the stock built-in driver during environment bring-up

### 2. Custom/generated PL011 path remains out-of-tree

The custom or generated PL011-like path should be an out-of-tree module for
this experiment, not an in-tree kernel modification.

Reason:

- faster iteration
- cleaner environment/integration learning
- avoids premature kernel-tree coupling

The working assumption for this experiment is:

- the stock built-in PL011 driver owns the console UART
- test-UART ownership is solved via DTB-based device partitioning
- runtime sysfs unbind/rebind is not the primary strategy

### 3. Module support enabled

Required setting:

- `CONFIG_MODULES=y`

Reason:

- the experiment needs to load a custom PL011-like module in the guest

### 4. QEMU `virt` boot support built in

Preferred settings:

- `CONFIG_VIRTIO=y`
- `CONFIG_VIRTIO_MMIO=y`
- `CONFIG_VIRTIO_BLK=y`

Reason:

- simplest first rootfs path is a virtio block device
- boot-critical storage should not depend on module loading

### 5. devtmpfs support enabled

Preferred settings:

- `CONFIG_DEVTMPFS=y`
- `CONFIG_DEVTMPFS_MOUNT=y`

Reason:

- simplifies bring-up for a minimal rootfs
- reduces early userspace friction

### 6. Host/guest file-sharing support optional but likely useful

Convenience settings:

- `CONFIG_NET_9P=y`
- `CONFIG_9P_FS=y`

Reason:

- convenient for iterative module transfer
- not strictly required if another transfer path is used
- worth enabling in the experiment if it does not complicate the build

## PL011-Specific Policy

### 7. Keep AMBA support intact

Any required AMBA/PrimeCell support for the stock PL011 path should remain
enabled.

Reason:

- the stock driver path is part of the experiment
- the experiment should not artificially degrade or disable the stock console
  path

### 8. Keep DMA support available for the stock path

Preferred settings:

- `CONFIG_DMA_ENGINE=y`

Reason:

- later stock-versus-custom comparison should not force the stock driver into a
  degraded mode unnecessarily
- harmless enough to keep in the initial experiment config if available

Note:

- `CONFIG_AMBA_PL011_DMA` does not exist as a separate symbol in the inspected
  `v6.18.21` tree, so there is nothing additional to enable here beyond
  `CONFIG_DMA_ENGINE=y`

### 9. Do not disable the stock PL011 driver yet

The first config should not remove or modularize the stock console driver in an
attempt to make the custom path easier.

Reason:

- the current experiment should first prove the stable stock-console path
- test-UART ownership should be solved through DTB or binding strategy rather
  than by making the console fragile

## Summary Of Preferred Settings

Strongly preferred:

- `CONFIG_MODULES=y`
- `CONFIG_SERIAL_AMBA_PL011=y`
- `CONFIG_SERIAL_AMBA_PL011_CONSOLE=y`
- `CONFIG_ARM_AMBA=y`
- `CONFIG_OF=y`
- `CONFIG_VIRTIO=y`
- `CONFIG_VIRTIO_MMIO=y`
- `CONFIG_VIRTIO_BLK=y`
- `CONFIG_DEVTMPFS=y`
- `CONFIG_DEVTMPFS_MOUNT=y`
- `CONFIG_DMA_ENGINE=y`
- `CONFIG_MAGIC_SYSRQ=y`

Convenience:

- `CONFIG_NET_9P=y`
- `CONFIG_9P_FS=y`

Already enabled by `arm64 defconfig` in the inspected `v6.18.21` tree:

- `CONFIG_MODULES=y`
- `CONFIG_SERIAL_AMBA_PL011=y`
- `CONFIG_SERIAL_AMBA_PL011_CONSOLE=y`
- `CONFIG_ARM_AMBA=y`
- `CONFIG_OF=y`
- `CONFIG_VIRTIO=y`
- `CONFIG_VIRTIO_MMIO=y`
- `CONFIG_VIRTIO_BLK=y`
- `CONFIG_DEVTMPFS=y`
- `CONFIG_DEVTMPFS_MOUNT=y`
- `CONFIG_DMA_ENGINE=y`
- `CONFIG_MAGIC_SYSRQ=y`
- `CONFIG_NET_9P=y`
- `CONFIG_9P_FS=y`

Also present in `defconfig`, but not first-order drivers of the experiment:

- `CONFIG_PM_SLEEP=y`
- `CONFIG_ACPI_SPCR_TABLE=y`
- `CONFIG_PINCTRL=y`

Current first-pass config delta on top of `arm64 defconfig`:

- none

## Planned Build Target

Once configured:

```bash
make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- -j$(nproc) Image modules dtbs
```

## Next Step

Use `arm64 defconfig` as the first concrete kernel-config baseline, then move
to kernel build and boot experimentation. Revisit the config only if later DTB,
rootfs, or driver-binding experiments expose a specific missing option.
