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
- `~/regseq-eval/<device>/generated/` is a consumer/archive location, not the
  canonical compiler output root
- PL011 smoke should prefer a project-controlled host-side `arm64` kernel tree
- generated PL011 modules should build against that same kernel tree
- the PL011 topology question inside QEMU `virt` is resolved by the owned-QEMU
  experiment path:
  - patched QEMU `v8.2.2`
  - two guest-visible PL011s
  - deterministic aliasing so console stays on `ttyAMA0` and the second UART
    appears as `ttyAMA1`
  - the second UART can remain on the AMBA bus and still be reserved for a
    custom driver by overriding `arm,primecell-periphid` in DT and matching it
    from a custom out-of-tree `amba_driver`
- the PL011 guest-workflow issue is resolved at the policy and rootfs-config
  level:
  - PL011 uses a no-guest-NIC default profile
  - 9p is the default artifact-transfer path
  - Buildroot was updated with `BR2_SYSTEM_DHCP=""` so `-nic none` no longer
    blocks guest startup waiting for `eth0`
- the first concrete PL011 guest rootfs/image baseline is resolved:
  - Buildroot `2026.02`
  - `qemu_aarch64_virt_defconfig` plus the PL011 fragment recorded in
    [buildroot-pl011-rootfs.fragment](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/experiments/pl011-eval-env/artifacts/buildroot-pl011-rootfs.fragment)
- the first PL011 smoke success bar is resolved:
  - it is a non-console second-UART smoke gate
  - it requires build, module load, registration, and probe on the reserved
    second PL011 device
  - it does not require console takeover
  - hot-path second-UART evaluation is the next mode after smoke
  - console integration is deferred to a later stricter phase
- the exact project-controlled kernel workflow is resolved:
  - pinned kernel tree at `~/regseq-eval/pl011/kernel-src/pl011-arm64-linux`
  - pinned revision `v6.18.21`
  - baseline config `arm64 defconfig`
  - build `Image`, modules, and `dtbs` from that same tree
  - boot QEMU with that same tree's `Image`
  - build all generated PL011 modules against that same `KDIR`
  - only introduce future config changes through a recorded fragment if needed

Those decisions are captured in the existing notes in this directory.

## Remaining Open Items

### 1. Exact runbook-level loop

The strategy notes are in place, but we do not yet have one final concrete
workflow that ties together:

1. kernel build/preparation
2. rootfs/image preparation
3. QEMU launch shape
4. module transfer/load path
5. log/artifact capture path

This is now the last remaining environment item.

## Closure Order

The recommended closure order is:

1. exact runbook-level loop

## Practical Rule

Do not try to finalize the full evaluation environment in one step.

Instead:

- close one item above
- update the relevant discussion notes
- then move to the next item

That is the intended working method for the remainder of the
evaluation-environment discussion.
