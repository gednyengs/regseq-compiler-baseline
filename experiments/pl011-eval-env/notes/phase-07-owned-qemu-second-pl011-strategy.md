# Phase 7 Owned QEMU Second-PL011 Strategy

## Purpose

This note records the experiment for obtaining a non-console PL011 target
inside the evaluation environment by owning and patching QEMU.

The immediate motivation is the Phase 6 finding that the default QEMU `virt`
machine configuration exposes only one guest-visible PL011 UART.

## Why This Strategy Is Being Considered

Phase 6 established the following facts:

- the current `virt` DTB contains only one PL011 node
- the guest only probes one PL011 device (`ttyAMA0`)
- extra `-serial` backends do not create additional guest PL011 hardware
- adding DTB nodes alone would not create real hardware behind those nodes

So if the experiment wants a clean separation between:

- stock built-in PL011 on the console UART
- modified/custom PL011 on a separate test UART

then the current default machine model is not sufficient.

## Candidate Strategy

Own QEMU for this experiment and patch the `virt` machine model so that it
instantiates a second PL011 UART in hardware.

If this works, the intended shape becomes:

- `ttyAMA0`: stock built-in PL011, used for the console
- `ttyAMA1`: second PL011, reserved for custom-driver experiments

Only after the second PL011 is proven should the experiment revisit DTB
partitioning to keep the stock driver off the test UART.

## Version Pin

For this experiment, the QEMU source should be pinned to upstream `v8.2.2`.

Why:

- the host already uses QEMU `8.2.2`
- using upstream `v8.2.2` keeps the experiment close to the current host
  behavior while still giving us a clean pinned source base
- moving to a newer series such as `9.x` would introduce unnecessary
  variables at this stage

Verified upstream tag:

- tag: `v8.2.2`
- dereferenced commit: `11aa0b1ff115b86160c4d37e7c37e6a6b13b77ea`

Host QEMU currently in use:

- `QEMU emulator version 8.2.2 (Debian 1:8.2.2+ds-0ubuntu1.14)`

## Constraints And Reservations

This strategy should only be treated as viable if the source-level inspection
confirms the assumptions behind it.

Important reservations:

- do not assume the patch point without inspecting the actual `v8.2.2` source
- do not assume the `virt` machine still defines the second UART slot exactly
  as described in external notes
- do not assume DTB editing alone solves the problem
- do not assume `-serial` backends create hardware

This is an experiment, not yet a committed final environment decision.

## What Was Verified In Source

Inspection of the pinned `v8.2.2` QEMU source showed:

- the `virt` machine model source file is `hw/arm/virt.c`
- there is no existing non-secure `VIRT_UART1`
- the machine already defines a second UART-shaped slot as `VIRT_SECURE_UART`
- that slot already has:
  - memmap entry at `0x09040000`
  - IRQ assignment `8`
- the existing `create_uart(...)` helper already handles:
  - PL011 instantiation
  - MMIO mapping
  - IRQ wiring
  - DT node generation
- the default single-UART behavior comes from the machine init path only
  creating `VIRT_UART` unconditionally and creating `VIRT_SECURE_UART` only
  when `vms->secure` is enabled

This meant the smallest viable experiment patch was not to invent a new
`VIRT_UART1`, but to repurpose the existing `VIRT_SECURE_UART` slot as a
second normal-world PL011 when `secure = off`.

## Implemented Patch Strategy

The experiment patch did two things:

1. instantiate `VIRT_SECURE_UART` even when `secure = off`, using normal
   `sysmem` rather than `secure_sysmem`
2. emit explicit DT aliases so Linux numbering stays deterministic:
   - `serial0 -> /pl011@9000000`
   - `serial1 -> /pl011@9040000`

The alias addition was required because without it Linux initially enumerated:

- `9040000.pl011 -> ttyAMA0`
- `9000000.pl011 -> ttyAMA1`

which moved the console to the TCP-backed UART instead of keeping it on the
stdio-backed UART.

## Build Result

The patched QEMU built successfully.

Artifacts:

- source tree: `~/regseq-eval/pl011/qemu-src/qemu`
- binary: `~/regseq-eval/pl011/qemu-src/qemu/build/qemu-system-aarch64`
- build log: `~/regseq-eval/pl011/qemu-src/qemu/build.log`

## DTB Result

Dumping the DTB from the patched binary showed:

- two PL011 nodes:
  - `pl011@9000000`
  - `pl011@9040000`
- `stdout-path = "/pl011@9000000"`
- explicit aliases:
  - `serial0 = "/pl011@9000000"`
  - `serial1 = "/pl011@9040000"`

This confirmed that the machine model now exposes the intended two-PL011
topology and that numbering is stable.

## Guest Boot Result

Booting the existing project-controlled kernel and Buildroot rootfs with the
patched QEMU produced the desired UART topology:

- `9000000.pl011: ttyAMA0`
- `9040000.pl011: ttyAMA1`
- `console [ttyAMA0] enabled`

This is the core success condition for the experiment:

- stable stock PL011 console on `ttyAMA0`
- second guest-visible PL011 on `ttyAMA1`

## AMBA Binding Reservation Trial

The next question after proving the two-PL011 topology was whether the second
UART could remain on the AMBA bus while still being reserved for a custom
driver instead of being claimed by the stock `amba-pl011` driver.

This was tested with a second custom DTB and a tiny out-of-tree AMBA probe
driver.

### Custom DTB shape

The second UART node was changed to:

- `compatible = "regseq,pl011", "arm,primecell"`
- `arm,primecell-periphid = <0x00abc123>`

while keeping:

- the node on the AMBA/PrimeCell path
- `serial0 -> /pl011@9000000`
- `serial1 -> /pl011@9040000`
- `stdout-path = "/pl011@9000000"`

This is the important point:

- changing only the `compatible` string is not enough while `arm,primecell`
  remains present, because Linux will still create an AMBA device
- overriding `arm,primecell-periphid` is what prevents stock `amba-pl011`
  from matching the second UART

### Out-of-tree AMBA test driver

A tiny out-of-tree module was built under:

- `experiments/pl011-eval-env/artifacts/regseq-pl011-amba-probe/`

The test module:

- registers an `amba_driver`
- matches `amba_id = 0x00abc123` with mask `0x00ffffff`
- logs probe information to `dmesg`

### Boot and load result

Booting with the custom DTB and loading the module produced:

- stock PL011 still claimed only `9000000.pl011` as `ttyAMA0`
- the custom module successfully probed `9040000.pl011`
- the probe log showed:
  - `periphid=0x00abc123`
  - base `0x9040000`
  - the expected device-tree node `/pl011@9040000`

Observed `dmesg` lines:

- `9000000.pl011: ttyAMA0 at MMIO 0x9000000 ...`
- `regseq-pl011-probe 9040000.pl011: regseq probe matched: periphid=0x00abc123 ...`

This proves that the second UART can stay on the AMBA bus while being reserved
for a custom out-of-tree AMBA driver by using a DT override for
`arm,primecell-periphid`.

### What This Does And Does Not Prove

This trial proves:

- the second UART does not have to be downgraded to a platform device just to
  avoid stock-driver binding
- the environment can keep AMBA semantics for the second UART
- a generated or stand-in driver can bind as an `amba_driver` to the second
  UART without fighting the stock console driver

This trial does not yet prove:

- serial-core registration for the custom driver
- full PL011 callback behavior
- IRQ-driven UART behavior through the custom driver

The test module was intentionally only a probe logger. It does not register a
UART port, so the absence of `/dev/ttyAMA1` during this trial is expected.

## Remaining Rough Edges

The successful two-PL011 result does not mean the entire guest workflow is
fully polished yet.

The remaining rough edges observed during this phase were:

- with the guest NIC present, `virtio_net` watchdog messages could spam the
  console and make login awkward
- with `-nic none`, the current Buildroot init scripts wait for `eth0`, which
  is a userspace/rootfs issue rather than a UART-topology issue

These are environment-polish problems, not failures of the two-PL011 strategy.

For the purposes of resolving the PL011 topology question, they do not block
closure. They can be handled later by:

- adjusting Buildroot network startup policy
- using a shell-oriented boot path such as `init=/bin/sh` when needed
- or choosing a different artifact-transfer strategy such as 9p rather than
  relying on guest networking

## Outcome

This experiment path is viable.

It resolved the core question that motivated it:

- the project can own a patched QEMU `virt` machine and use it to obtain a
  stable console PL011 plus a second non-console PL011 for later custom-driver
  experiments
- the project can keep that second UART on the AMBA bus and reserve it for a
  custom AMBA driver by overriding `arm,primecell-periphid` in DT

The networking/init rough edges should be tracked separately from the PL011
topology result.

## Relationship To Other Options

This is the cleanest currently known path to preserve:

- stable stock PL011 console
- separate PL011 test target

But it is not the only possible path.

If this fails or proves too costly, the fallback options remain:

- patch the stock in-kernel PL011 driver and use it as the custom-driver
  stand-in
- pursue a one-PL011 integration strategy instead of a two-PL011 strategy

## Next Step

Use this result to treat the "exact PL011 binding strategy inside QEMU"
question as resolved at the topology and AMBA-reservation level, then move on
to:

- environment-polish follow-up for guest workflow
- custom-driver serial-core registration on the second UART
