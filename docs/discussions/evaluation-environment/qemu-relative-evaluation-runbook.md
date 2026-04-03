# QEMU Relative Evaluation Runbook

## Purpose

This runbook is the executable companion to
[qemu-relative-evaluation-strategy.md](/Users/nyengele/Documents/sekekama/regseq-compiler-v2/docs/discussions/evaluation-environment/qemu-relative-evaluation-strategy.md).

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
mkdir -p ~/regseq-eval/{images,kernel-src,generated,results,scripts}
```

Recommended subdirectories:

- `images/`: guest disk images
- `kernel-src/`: Linux kernel trees
- `generated/`: Regseq-emitted target artifacts
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

## Device Notes

### E1000

Recommended DUT path:

- `x86_64` guest
- QEMU/KVM `e1000`
- relative benchmarking between handwritten and generated Linux drivers

### PL011

Recommended DUT path:

- QEMU-based Linux guest using the modeled PL011 environment
- use the same relative-comparison philosophy as E1000
- do not block on sourcing physical PrimeCell hardware

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
