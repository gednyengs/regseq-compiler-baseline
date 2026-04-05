# Evaluation Environment Current Resolutions And Strategy

## Purpose

This note is the current consolidated state of the evaluation-environment
discussion.

It exists so the project can recover the main decisions, experiment-backed
findings, and remaining open items without needing the full conversational
history.

## Current Resolved Decisions

The following decisions are currently considered resolved.

### Host and overall methodology

- Ubuntu 24.04 is the source-of-truth Phase D host.
- QEMU-based evaluation is the default methodology.
- Paper claims should be framed as controlled relative comparisons, not
  absolute hardware-performance claims.
- Generated driver outputs remain repo-local; external eval workspaces are
  consumer/archive locations.

### PL011 kernel and guest direction

- PL011 smoke should use a project-controlled host-side `arm64` kernel tree.
- Generated or stand-in out-of-tree modules should build against that same
  kernel tree.
- The current canonical kernel location is:
  - `~/regseq-eval/pl011/kernel-src/pl011-arm64-linux`
- The pinned Linux source used in the current experiment is:
  - `v6.18.21`
  - commit `44c944a679974c2d18ee9b87070456d34193f3d4`
- The canonical kernel configuration baseline is:
  - `arm64 defconfig`
- No extra kernel config delta has been required so far for the current
  two-PL011 PL011 environment.
- Future kernel-config changes should be recorded as a small explicit fragment
  rather than left as ad hoc `.config` drift.
- QEMU should boot the `Image` produced by that same pinned tree, and
  out-of-tree modules should always build against that same `KDIR`.

### Rootfs direction

- Buildroot is the first rootfs strategy.
- The pinned Buildroot source used in the current experiment is:
  - `2026.02`
  - commit `52ee2f5644da5389634b2465f1dc31deb5a4807c`
- The first-pass image is a minimal bring-up image, not a benchmark-heavy
  guest image.
- For the PL011 guest profile, the concrete no-network default is now fixed:
  - `BR2_SYSTEM_DHCP=""`
  - generated `/etc/network/interfaces` contains only loopback
  - `-nic none` no longer causes the guest to stall waiting for `eth0`
- The current PL011 rootfs baseline is frozen as:
  - Buildroot `qemu_aarch64_virt_defconfig`
  - plus the repo-local fragment in
    [buildroot-pl011-rootfs.fragment](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/experiments/pl011-eval-env/artifacts/buildroot-pl011-rootfs.fragment)

### QEMU direction

- PL011 smoke uses QEMU `aarch64` `virt`.
- TCG is acceptable for the first PL011 smoke path on an `x86_64` host.
- The default Ubuntu-packaged `virt` machine exposes only one guest-visible
  PL011 by default.
- For the current experiment, owning and patching QEMU is a viable strategy.
- The pinned QEMU source used in the current experiment is:
  - `v8.2.2`
  - commit `11aa0b1ff115b86160c4d37e7c37e6a6b13b77ea`

## Experiment-Backed PL011 Findings

The current experiment established the following concrete PL011 results.

### Two-PL011 topology on `virt`

By patching QEMU `v8.2.2`, the experiment successfully exposed two real
guest-visible PL011 UARTs on the `virt` machine:

- `0x9000000` for the console UART
- `0x9040000` for the test UART

This required:

- instantiating the existing `VIRT_SECURE_UART` slot as a second normal-world
  PL011 when `secure = off`
- emitting DT aliases so numbering stays deterministic:
  - `serial0 -> /pl011@9000000`
  - `serial1 -> /pl011@9040000`

With that aliasing in place, guest boot confirmed:

- `9000000.pl011 -> ttyAMA0`
- `9040000.pl011 -> ttyAMA1`
- console remains on `ttyAMA0`

### Reserving the second UART for a custom AMBA driver

The experiment also established that the second UART can remain on the AMBA
bus while being reserved for a custom out-of-tree driver.

This was done by changing the second DT node to:

- `compatible = "regseq,pl011", "arm,primecell"`
- `arm,primecell-periphid = <0x00abc123>`

and then loading a tiny out-of-tree `amba_driver` whose `amba_id` table
matches `0x00abc123`.

Observed result:

- stock `amba-pl011` still claims only the console UART
- the custom out-of-tree probe driver successfully claims `9040000.pl011`
- the probe sees the overridden `periphid=0x00abc123`

This matters because it proves:

- the second UART does not need to be downgraded to a platform-only test
  device
- the environment can preserve AMBA semantics for the second UART
- a generated or stand-in `amba_driver` can claim the test UART without
  fighting the stock console driver

## Current Recommended PL011 Environment Shape

The current recommended experiment-backed shape for PL011 is:

1. Ubuntu 24.04 host
2. project-controlled Linux `v6.18.21` `arm64` kernel
3. Buildroot guest rootfs
4. owned/patched QEMU `v8.2.2`
5. two guest-visible PL011 UARTs on `virt`
6. stock built-in PL011 on `ttyAMA0` for console
7. second PL011 on `ttyAMA1` reserved for the generated or stand-in custom
   driver
8. preserve AMBA semantics for the second UART by overriding
   `arm,primecell-periphid` in DT rather than removing `"arm,primecell"`

## Current PL011 Validation Priority

The PL011 validation hierarchy should currently be understood as:

1. non-console second-UART smoke and integration loop
2. second-UART hot-path functional loop
3. console-driver integration last

The first loop is the default agent-development path.

The second loop is expected to provide most of the PL011 evidence needed for
the paper.

The third loop should be treated as a later stricter integration mode rather
than a prerequisite for Phase D bring-up.

## Current Recommended Guest Workflow Profiles

The project should not assume one guest-workflow policy fits every device
family.

### PL011 guest workflow profile

For PL011 bring-up and serial-driver iteration, the preferred workflow is:

- no guest NIC by default
- 9p-first for host/guest artifact sharing
- no automatic `eth0` bring-up in the normal PL011 profile
- manual network bring-up only when a specific experiment needs it

This keeps the serial console cleaner and avoids boot stalls caused by a guest
waiting for a NIC that is not present. It also matches the fact that guest
networking is not on the critical path for PL011 driver work.

### E1000 guest workflow profile

For E1000, the workflow should remain networking-oriented:

- guest NIC present by design
- guest network bring-up enabled
- network topology and workload setup treated as part of the device-under-test
  environment

So the PL011 no-network default should be treated as a device-specific profile,
not as a global rule for the entire evaluation environment.

## What Is Still Not Fully Closed

The following environment questions are still open or only partially closed.

### Final runbook

The project still needs one final runbook-quality procedure tying together:

1. kernel build/preparation
2. rootfs preparation
3. QEMU launch
4. DTB selection
5. module transfer/load path
6. log/artifact capture

## Pointers To Supporting Notes

- [qemu-relative-evaluation-strategy.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/evaluation-environment/qemu-relative-evaluation-strategy.md)
- [qemu-relative-evaluation-runbook.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/evaluation-environment/qemu-relative-evaluation-runbook.md)
- [pl011-project-controlled-kernel-strategy.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/evaluation-environment/pl011-project-controlled-kernel-strategy.md)
- [pl011-linux-smoke-contract.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/evaluation-environment/pl011-linux-smoke-contract.md)
- [open-items-and-closure-order.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/evaluation-environment/open-items-and-closure-order.md)

Experiment evidence:

- [README.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/experiments/pl011-eval-env/README.md)
- [phase-06-qemu-boot-and-dtb-findings.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/experiments/pl011-eval-env/notes/phase-06-qemu-boot-and-dtb-findings.md)
- [phase-07-owned-qemu-second-pl011-strategy.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/experiments/pl011-eval-env/notes/phase-07-owned-qemu-second-pl011-strategy.md)
