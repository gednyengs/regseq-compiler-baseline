# Phase 4 Kernel Build

## Purpose

This note records the first full kernel build for the PL011 evaluation
environment experiment.

The goal of this phase was to validate that the pinned Linux source and the
chosen `arm64 defconfig` baseline actually build on the Ubuntu host with the
installed cross toolchain.

## Build Inputs

- kernel source: `v6.18.21`
- source tree: `~/regseq-eval/pl011/kernel-src/pl011-arm64-linux`
- configuration baseline: `arm64 defconfig`
- cross toolchain: `aarch64-linux-gnu-*`

## Build Command

```bash
make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- -j24 Image modules dtbs \
  2>&1 | tee build.log
```

## Result

- status: success
- completion timestamp: `2026-04-03T22:59:08-07:00`
- no additional `.config` changes were required beyond `arm64 defconfig`
- no new missing host dependencies were exposed during the build

## Key Artifacts

- kernel image: `arch/arm64/boot/Image`
- build log: `build.log`
- built modules: full module tree produced
- built device trees: full `dtbs` target produced

Observed artifact details:

- `arch/arm64/boot/Image`: approximately `48M`
- built module count: `1425` `.ko` files

## Immediate Takeaway

The current host and toolchain are sufficient to build the pinned `v6.18.21`
ARM64 kernel, modules, and device trees from the chosen baseline.

This closes the first practical question behind the kernel-config policy:
`arm64 defconfig` is not just theoretically acceptable for this experiment, it
is buildable as-is on the current host.

## Next Step

Move to the rootfs strategy and build phase, then use the resulting kernel
artifacts for the first QEMU boot attempt.
