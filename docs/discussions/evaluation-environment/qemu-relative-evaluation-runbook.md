# QEMU Relative Evaluation Runbook

## Purpose

This runbook is the executable companion to
[qemu-relative-evaluation-strategy.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/evaluation-environment/qemu-relative-evaluation-strategy.md).

It is written for this repo's actual intended workflow:

- Ubuntu 24.04 workstation is the primary host
- QEMU/KVM is the primary evaluation mechanism
- baseline and generated drivers are compared under identical VM conditions

## Scope

This runbook covers:

- Linux-native lowering validation on Ubuntu
- QEMU/KVM setup for comparative driver experiments
- artifact capture for reproducibility

It does not yet define:

- the exact final benchmark suite for the paper
- the final kernel integration patch shape for every backend

## Host Assumptions

Primary host:

- Ubuntu 24.04
- `x86_64` preferred for the E1000 workflow
- KVM available
- QEMU installed
- Linux bridge/TAP setup allowed

Secondary host:

- macOS may still be used for convenience work, but this runbook is not
  optimized around it and should not be treated as the source of truth for
  measured results

## Working Directory Convention

Use a working directory such as:

```bash
mkdir -p ~/regseq-eval/{pl011,e1000}
mkdir -p ~/regseq-eval/pl011/{images,kernel-src,qemu-src,rootfs-src,generated,results,scripts}
mkdir -p ~/regseq-eval/e1000/{images,kernel-src,qemu-src,rootfs-src,generated,results,scripts}
```

Recommended per-device subdirectories:

- `images/`: guest disk images
- `kernel-src/`: Linux kernel trees
- `qemu-src/`: device-specific QEMU source/build trees when needed
- `rootfs-src/`: device-specific guest rootfs source trees
- `generated/`: copied or archived Regseq-emitted target artifacts
- `results/`: logs, metrics, snapshots
- `scripts/`: helper scripts

## Evaluation Modes

### Mode 1: lowering/debug smoke

Use this first:

- validate generated collateral shape
- confirm the guest boots
- confirm the driver probes and basic I/O paths work

This mode should be designed so that an LLM agent can run the full compile/fix
loop on Ubuntu without needing to invent integration steps each time.

### Mode 2: relative benchmark runs

Use this only after Mode 1 is stable:

- same guest environment
- same VM topology
- same workloads
- baseline versus generated

## Host Networking

Use TAP-backed networking for comparative runs.

Example:

```bash
sudo ip link add br-regseq type bridge
sudo ip tuntap add tap-dut mode tap user "$USER"
sudo ip tuntap add tap-peer mode tap user "$USER"
sudo ip link set tap-dut master br-regseq
sudo ip link set tap-peer master br-regseq
sudo ip link set tap-dut up
sudo ip link set tap-peer up
sudo ip link set br-regseq up
```

Avoid `-nic user` for reported measurements.

## Guest Topology

Use:

- one DUT VM
- one peer endpoint

The peer may be:

- a second VM
- or a host namespace attached to the same bridge

For the shortest path, prefer:

- DUT VM
- peer VM or host namespace held constant across runs

## Driver Variants

For each device under evaluation, keep two variants:

1. handwritten baseline
2. Regseq-generated candidate

Keep everything else fixed.

## Integration Modes

Two supported integration modes:

### Option A: kernel-tree replacement

Use when:

- generated output closely mirrors an in-tree Linux driver structure
- kernel rebuild cost is acceptable

Best for:

- faithful integration experiments
- situations where module boundaries are awkward

### Option B: out-of-tree module

Use when:

- generated output can be wrapped as a self-contained Linux module
- iteration speed matters more than perfectly mimicking in-tree structure

Best for:

- early backend debugging
- frequent regenerate/rebuild/test loops

## Recommended Default

Default recommendation:

- use out-of-tree module style first for lowering-debug iteration
- move to tighter kernel-tree integration only if the backend shape or paper
  artifact requires it

Reason:

- shorter turnaround
- easier debugging
- fewer full-kernel rebuilds

This is also the better first setup for agent-driven development because it
provides the tightest regenerate/build/error/fix cycle.

## Compiler Output Contract

The compiler's default generated output root should be repo-relative:

- `output/linux/<driver-id>/`

That directory is the primary regenerate/build/debug artifact root during
compiler development.

If the Ubuntu evaluation workspace needs a copy, mirror selected generated
artifacts into:

- `~/regseq-eval/<device>/generated/`

But that evaluation workspace should be treated as a consumer/archive location,
not as the canonical compiler output root.

## Kernel Provenance Contract

For the PL011 smoke path, the kernel used by the guest should come from a
project-controlled `arm64` kernel tree on the Ubuntu host.

That same tree should also be the authoritative out-of-tree module build
target. In practice:

- the host maintains a pinned `arm64` kernel tree under
  `~/regseq-eval/pl011/kernel-src/`
- QEMU boots that kernel directly
- generated PL011 modules build against that same kernel tree via `KDIR`

This avoids trying to recover matching headers from a guest distro kernel after
the fact and gives the project control over device-binding behavior.

## Device Notes

### E1000

Recommended DUT path:

- `x86_64` guest
- QEMU/KVM `e1000`
- relative benchmarking between handwritten and generated Linux drivers
- guest networking enabled by design
- network setup treated as part of the DUT workflow

### PL011

Recommended DUT path:

- QEMU `aarch64` `virt` Linux guest using the modeled PL011 environment
- use the same relative-comparison philosophy as E1000
- do not block on sourcing physical PrimeCell hardware
- allow TCG for the first PL011 smoke path when host/guest architecture makes
  KVM unsuitable
- prefer a project-controlled `arm64` kernel over a guest distro kernel for
  the PL011 smoke path
- prefer a no-guest-NIC default workflow for PL011 bring-up
- prefer 9p as the default artifact-transfer path for PL011 bring-up
- do not treat the PL011 no-network workflow as a global rule for E1000 or
  other network-centric targets

For host-side build preparation, assume:

- `gcc-aarch64-linux-gnu`
- `g++-aarch64-linux-gnu`
- an `arm64` kernel build tree prepared for external module builds

Canonical module-build shape:

```bash
make \
  ARCH=arm64 \
  CROSS_COMPILE=aarch64-linux-gnu- \
  KDIR=$HOME/regseq-eval/pl011/kernel-src/pl011-arm64-linux
```

See also:

- [pl011-linux-smoke-contract.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/evaluation-environment/pl011-linux-smoke-contract.md)
- [pl011-project-controlled-kernel-strategy.md](/home/nyengele/Documents/sekekama/regseq-compiler-asplos/docs/discussions/evaluation-environment/pl011-project-controlled-kernel-strategy.md)

## PL011 Loop Hierarchy

The PL011 workflow should currently be understood as three loops.

### Loop A: Host Build Loop

Purpose:

- implement or fix Linux codegen
- regenerate the PL011 Linux artifact
- build it out-of-tree against the pinned PL011 kernel tree

Canonical shape:

1. implement or patch Linux codegen in the repo
2. generate the PL011 Linux output under `output/linux/pl011-full-spec/`
3. build the generated module against:
   - `KDIR=$HOME/regseq-eval/pl011/kernel-src/pl011-arm64-linux`
4. iterate until the module builds cleanly

This is the default fast agent-development loop.

### Loop B: Guest Smoke Loop

Purpose:

- boot the controlled PL011 guest environment
- load the generated or stand-in module
- confirm registration and probe on the reserved second PL011 device

Fixed environment assumptions:

- pinned kernel tree:
  - `~/regseq-eval/pl011/kernel-src/pl011-arm64-linux`
- pinned QEMU tree:
  - `~/regseq-eval/pl011/qemu-src/qemu`
- pinned Buildroot guest rootfs:
  - `~/regseq-eval/pl011/rootfs-src/buildroot`
- owned-QEMU two-PL011 topology
- reserved second-UART DT strategy using `arm,primecell-periphid`

Canonical shape:

1. boot the PL011 VM with the pinned kernel, rootfs, QEMU binary, and reserved
   second-UART DTB
2. make the module available in the guest via 9p
3. load the module
4. inspect `dmesg` and other guest-visible signals
5. retain logs and artifact identifiers

This is the first smoke-validation loop.

### Loop C: Hot-Path Functional Loop

Purpose:

- exercise the second PL011 UART as the device under test
- compare handwritten baseline versus generated driver under identical
  non-console conditions
- collect the main PL011 evaluation evidence expected to matter for the paper

Fixed comparison contract:

- same kernel tree
- same rootfs
- same patched QEMU binary
- same DTB
- same second-UART backend wiring
- same workload scripts
- same repetition count and logging policy

Expected workload focus:

- TX path behavior
- RX path behavior
- interrupt-driven behavior
- FIFO-related hot paths

Expected measurements:

- throughput
- latency
- guest CPU utilization
- interrupt counts
- run-to-run variance

Explicit deferral:

- console-driver takeover is not part of Loop C
- console-driver evaluation should remain a later stricter integration mode

## Metrics

Primary metrics:

- throughput
- latency
- guest CPU utilization
- interrupt count

Secondary metrics:

- packets per second
- retransmissions or drops
- run-to-run variance

Nice-to-have:

- MMIO operation counts
- per-packet CPU cost

## Artifact Capture

For each run, retain:

- generated collateral used
- kernel/module build artifact
- guest image identifier
- QEMU command line
- benchmark logs
- `/proc/interrupts` snapshots
- CPU-usage logs
- host metadata:
  - host kernel
  - QEMU version
  - KVM enabled/disabled
  - CPU model

## Fairness Controls

Hold fixed:

- guest kernel version
- guest userspace
- QEMU version
- VM CPU count
- VM memory size
- networking topology
- workload scripts
- run counts

Recommended:

- use the `performance` CPU governor
- minimize host background load
- pin vCPUs/QEMU processes if practical
- run at least 5 repetitions per workload

## Immediate Next Steps

1. decide the first concrete DUT workflow for Linux backend bring-up
   - likely out-of-tree module
2. define the initial PL011 smoke-validation loop
   - with a project-controlled `arm64` kernel tree
3. define the initial E1000 smoke-validation loop
4. script the benchmark and artifact-capture process under Ubuntu

## Future Harnessing Goal

The Ubuntu workflow should eventually expose a small scriptable surface such as:

- generate target collateral
- build PL011 module
- build E1000 module
- collect build logs deterministically

That will make it possible for an LLM agent to:

- generate
- compile
- catch errors
- patch
- retry

without relying on undocumented manual host setup.
