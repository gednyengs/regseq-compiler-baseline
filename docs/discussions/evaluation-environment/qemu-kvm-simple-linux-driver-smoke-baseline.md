# QEMU/KVM Simple Linux Driver Smoke Baseline

## Purpose

This note defines the recommended pre-PL011 smoke baseline for the Ubuntu
QEMU/KVM development environment.

It exists because the Phase D Linux validation story should not begin by
debugging generated PL011 output and the host/guest environment at the same
time. Before PL011-specific smoke validation is specified, the project should
prove that the Ubuntu feedback loop itself works with a deliberately simple
Linux driver.

This is an environment-establishment note, not a claim that the simple driver
resembles the final PL011 artifact.

## Why This Is Needed

The existing evaluation notes already establish that:

- Ubuntu 24.04 is the source-of-truth host
- QEMU/KVM is the default environment
- out-of-tree module style is the likely first integration mode

What remains missing is a concrete proof that the basic loop works:

1. build a Linux module on Ubuntu
2. boot a guest under QEMU/KVM
3. move the module into the guest or expose it via a shared path
4. load the module
5. observe deterministic success signals and logs
6. unload and repeat

Without that baseline, later PL011 smoke validation will conflate:

- backend bugs
- integration-layout mistakes
- guest-bring-up failures
- artifact-capture problems

## Recommended Baseline Driver

The first smoke baseline should use a deliberately simple synthetic platform
driver rather than a PL011-specific driver.

Recommended shape:

- one out-of-tree module
- a toy `platform_driver`
- a matching synthetic `platform_device` created by the same module or by a
  tiny companion helper
- probe/remove callbacks that log deterministic messages

This is better than a pure `hello world` module because it exercises the same
basic Linux driver model family that the first PL011 path will use:

- module build
- module insertion
- driver registration
- device match/probe
- module removal
- dmesg/log capture

If desired, a plain hello-module check may still be used as a stage-zero sanity
check, but it should not be treated as the main environment proof.

## Recommended Host And Guest Shape

Primary host:

- Ubuntu 24.04
- KVM enabled
- QEMU installed
- Linux kernel headers available for out-of-tree module builds

Initial guest recommendation:

- one Linux guest only
- no benchmark workload yet
- simple artifact transfer path

For this baseline, simplicity matters more than measurement fidelity.

Acceptable first-pass artifact-transfer mechanisms:

- shared folder setup such as `virtiofs` or 9p
- SSH/SCP over a simple forwarded port

This baseline does not need the final comparative networking topology yet.

## Recommended Smoke Loop

The first proven loop should be:

1. build the simple out-of-tree module on Ubuntu
2. capture the exact build command and build log
3. boot the QEMU/KVM Linux guest
4. make the built module available inside the guest
5. `insmod` the module
6. confirm:
   - module load success
   - driver registration success
   - synthetic device probe fired
   - expected log lines appeared
7. `rmmod` the module
8. confirm:
   - remove/unregister path ran
   - expected teardown log lines appeared
9. repeat deterministically

## What This Baseline Must Prove

Before the environment is considered ready for generated-driver work, this
simple-driver baseline should prove:

- the host can build out-of-tree Linux modules reproducibly
- the QEMU/KVM guest boots reproducibly
- guest logs can be captured deterministically
- module insertion and removal are scriptable
- driver probe success has a simple, reviewable signal
- artifacts and logs can be retained in stable locations

## Artifact Capture

For each baseline run, retain:

- module source used
- module build log
- module build outputs
- guest image identifier
- QEMU command line
- guest kernel version
- `dmesg` or journal excerpt showing:
  - module load
  - probe success
  - remove success
- host metadata:
  - host kernel
  - QEMU version
  - whether KVM acceleration was enabled

## Exit Criteria

The Ubuntu/QEMU baseline is ready for PL011-oriented validation planning when:

1. the simple platform-driver module builds repeatedly on Ubuntu
2. the guest boots repeatedly under QEMU/KVM
3. the synthetic device probe succeeds repeatedly
4. logs and artifacts are captured in deterministic locations
5. the loop is scriptable enough that an LLM agent would not need to invent
   steps by hand each time

## What This Does Not Prove

This baseline does not prove:

- PL011 correctness
- serial subsystem integration correctness
- MMIO or interrupt behavior for the real artifact device
- benchmark readiness
- final paper measurement topology

It exists only to de-risk the environment and feedback loop.

## Immediate Recommendation

Before finalizing PL011 Linux smoke-validation task specs, the project should:

1. stand up the Ubuntu 24.04 QEMU/KVM baseline
2. prove the out-of-tree module loop with a simple synthetic platform driver
3. capture the exact commands, artifacts, and success signals in a stable
   runbook

After that, PL011-specific smoke validation can reuse the same host and guest
workflow rather than inventing it at the same time as backend debugging.
