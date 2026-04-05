# Task: track-a-validation-07 - PL011 Linux generated build and second-UART smoke proof

## Description

Prove that the generated PL011 Linux output can complete the first real
environment-backed guest smoke path:

- guest boot in the controlled PL011 environment
- module load, registration, and probe on the reserved second PL011 UART

This task is the first guest/environment-backed validation gate for Phase D.
The host-side generated-module build gate is handled earlier by
`track-a-validation-11`.

## Input Context

Read before starting:

- `.harness/tasks/regseq-linux-backend/02-pl011-linux-driver-emission-for-supported-slice.md`
- `.harness/tasks/track-a-validation/10-pl011-full-spec-lowering-completeness-and-backend-independence-proof.md`
- `.harness/tasks/track-a-validation/11-generated-pl011-linux-module-host-build-proof.md`
- `docs/discussions/evaluation-environment/pl011-linux-smoke-contract.md`
- `docs/discussions/evaluation-environment/pl011-project-controlled-kernel-strategy.md`
- `docs/discussions/evaluation-environment/current-resolutions-and-strategy.md`
- `docs/discussions/evaluation-environment/qemu-relative-evaluation-runbook.md`
- `experiments/pl011-eval-env/notes/phase-05-rootfs-strategy.md`
- `experiments/pl011-eval-env/notes/phase-06-qemu-boot-and-dtb-findings.md`
- `experiments/pl011-eval-env/notes/phase-07-owned-qemu-second-pl011-strategy.md`

## Output Specification

Modify:

- `tests/e2e/tests/`
- `scripts/` or `experiments/pl011-eval-env/scripts/` if narrow automation is
  needed for deterministic Loop A/B execution
- documentation snapshots or retained validation artifacts only where needed
  for reviewable proof

Add the first reviewer-facing proof that generated PL011 Linux output completes
the non-console second-UART smoke path on the controlled Ubuntu / QEMU
environment after the host-side compile gate has already passed.

## Implementation Notes

The validation environment should assume:

- Ubuntu 24.04 host
- project-controlled PL011 kernel tree at
  `~/regseq-eval/pl011/kernel-src/pl011-arm64-linux`
- pinned kernel revision `v6.18.21`
- Buildroot PL011 rootfs profile
- owned/patched QEMU `v8.2.2`
- reserved second PL011 UART via the AMBA-preserving DT strategy
- repo-local generated output under `output/linux/pl011-full-spec/`

Canonical owned-environment paths for this task are:

- kernel tree:
  - `~/regseq-eval/pl011/kernel-src/pl011-arm64-linux`
- kernel image:
  - `~/regseq-eval/pl011/kernel-src/pl011-arm64-linux/arch/arm64/boot/Image`
- patched QEMU:
  - `~/regseq-eval/pl011/qemu-src/qemu/build/qemu-system-aarch64`
- reserved-second-UART DTB:
  - `~/regseq-eval/pl011/qemu-src/qemu/virt-2uart-regseq.dtb`
- PL011 rootfs image:
  - `~/regseq-eval/pl011/rootfs-src/buildroot/output/images/rootfs.ext2`
- generated driver root:
  - `output/linux/pl011-full-spec/`
- host-share mount source:
  - `/home/nyengele/Documents/sekekama/regseq-compiler-asplos`

If the host is `x86_64` and the guest is `aarch64`, TCG remains acceptable for
this first smoke proof.

The canonical PL011 smoke boot command for this task is:

```bash
$HOME/regseq-eval/pl011/qemu-src/qemu/build/qemu-system-aarch64 \
  -machine virt \
  -cpu cortex-a53 \
  -m 1024 \
  -kernel $HOME/regseq-eval/pl011/kernel-src/pl011-arm64-linux/arch/arm64/boot/Image \
  -dtb $HOME/regseq-eval/pl011/qemu-src/qemu/virt-2uart-regseq.dtb \
  -append "root=/dev/vda rw rootwait console=ttyAMA0" \
  -drive file=$HOME/regseq-eval/pl011/rootfs-src/buildroot/output/images/rootfs.ext2,format=raw,if=virtio \
  -nic none \
  -virtfs local,path=/home/nyengele/Documents/sekekama/regseq-compiler-asplos,mount_tag=hostshare,security_model=none,readonly=off \
  -display none \
  -monitor none \
  -serial stdio \
  -serial tcp::4321,server,nowait
```

The canonical guest-side load-and-check sequence for this task is:

```sh
mkdir -p /mnt/host
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t 9p -o trans=virtio,version=9p2000.L hostshare /mnt/host

insmod "$(find /mnt/host/output/linux/pl011-full-spec -maxdepth 1 -name '*.ko' -print -quit)"

dmesg | grep -E 'pl011|regseq|amba'
find /sys/bus/amba/devices -maxdepth 1 -type l -printf '%f\n'
```

The implementing agent may wrap these commands in narrow scripts for
determinism, but it should not invent a different environment topology unless
the task is first updated.

The smoke proof should explicitly cover:

- Loop B:
  - boot the controlled PL011 VM
  - make the generated module available inside the guest
  - load the module
  - confirm registration
  - confirm probe on the reserved second PL011-compatible device
  - capture deterministic guest log evidence

This task may reuse retained build artifacts and logs from
`track-a-validation-11` instead of repeating the host-side compile proof
inline.

This task should treat the second-UART smoke path as the required gate.
It should not require:

- console takeover
- hot-path measurement
- in-tree kernel replacement

The environment already proved the second-UART AMBA reservation methodology
with a stand-in driver. This task should reuse that environment shape for the
generated-driver smoke path rather than redesigning the VM topology.

This task assumes the backend has already been made plan-driven. It is not the
place to compensate for missing lowering data with handwritten PL011 backend
logic.

## Test Plan

Add validation to verify:

1. the guest boots reproducibly in the PL011 environment
2. generated module insertion succeeds
3. generated registration and probe logs appear on the reserved second UART
4. retained logs/artifacts are sufficient for reviewer inspection

Test commands should include the relevant guest smoke commands or narrow
wrappers for them, and may reference retained host-build artifacts from
`track-a-validation-11`.

## Acceptance Criteria

- host-side generated-module build proof has already passed
- the guest smoke loop completes on the reserved second PL011 device
- registration and probe evidence is retained deterministically
- the proof follows the non-console second-UART smoke contract
- tests / scripted validation pass
