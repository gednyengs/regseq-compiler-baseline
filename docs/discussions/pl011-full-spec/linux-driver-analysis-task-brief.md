# PL011 Linux Stock Driver Analysis -- Task Brief

Status: DRAFT, awaiting reviewer sign-off before work begins.

This document is a rationale brief, not a work product. Its purpose is to let
a reviewer validate the approach and the analysis framework before we invest
effort in the actual analysis.

## 1. Purpose and Non-Purpose

**Purpose.** Produce a structured description of:

1. The functional and architectural capabilities of the ARM PrimeCell PL011
   UART device as defined by its datasheet.
2. Which of those capabilities the Linux stock PL011 driver exercises, and
   how it exercises them.

The output is a descriptive reference document about the PL011 device and
its Linux stock driver. It is intended to be consumed by downstream tasks
that fall outside this brief.

**Non-purpose.** This task is NOT:

- A design exercise for any DSL, compiler, or taxonomy. No DSL, compiler
  pipeline, or construct-level modeling is in scope. The analysis must be
  expressible and complete without referring to any such machinery.
- A workload enumeration or workload-driven scoping exercise. The analysis
  does not depend on any service contract, workload, or benchmark.
- A comparison across operating systems. Only the Linux stock driver is
  analyzed.
- A platform-independence or portability exercise. The task makes no claims
  about what a portable driver would look like and proposes no abstractions
  to support one.
- A performance measurement or benchmarking exercise. Performance features
  are *catalogued* (see Section 4.3); they are not measured.

## 2. Inputs

The analysis is driven entirely by two inputs:

1. The ARM PrimeCell PL011 Technical Reference Manual (datasheet).
2. The Linux stock PL011 driver source as provided for this task.

The agent analyzes all of the driver source provided. It does not pre-select
portions of the source to include or exclude; the scope of the source is
fixed by what is handed to it. Where the datasheet and the driver disagree,
or where the driver appears to work around a hardware quirk not documented
in the datasheet, that discrepancy is recorded as a finding in its own
right.

## 3. Scope

- **In scope.** The PL011 device behavior as defined by the datasheet; the
  behavior of the Linux stock driver, across the four analysis dimensions
  in Section 4, covering the full driver source provided.
- **Out of scope.** DSL / compiler / taxonomy design, workload definition,
  cross-OS comparison, portability analysis, performance benchmarking, and
  any speculation about what a portable driver specification would look
  like.

## 4. Analysis Framework -- Four Dimensions

The analysis is partitioned along four dimensions. Each dimension asks a
different question about the driver. A single device feature is expected to
contribute to more than one dimension, because each dimension asks a
different question about it. For example, the RX FIFO shows up under
Functional as "what the device provides and how the driver configures it,"
under Performance as "what the driver does to batch work and reduce IRQ
rate," and under Correctness as "what ordering and locking rules govern its
access." A feature appearing in multiple sections is the expected outcome,
not a classification failure.

### 4.1 Functional Analysis

**Question.** Which functional capabilities of the PL011 device does the
Linux driver capture, and how does it capture them?

**Scope includes:** device configuration (baud, data bits, parity, stop
bits), TX path, RX path, interrupt management, FIFO management, error
handling, modem control, flow control, and any other device-level behavior
the driver exercises.

### 4.2 Integration Analysis

**Question.** Which Linux kernel subsystems does the stock driver integrate
the PL011 device into, and how?

**Scope includes:** tty / serial core integration, console integration,
earlycon, kgdb, sysrq, device tree / ACPI bindings, probe / remove
lifecycle, power management hooks, and any other kernel-subsystem
touchpoints exposed by the driver.

### 4.3 Performance Analysis

**Question.** Which PL011 performance features does the Linux driver
implement, and which Linux-side performance facilities does the driver use
to improve the throughput, latency, or CPU cost of device operation?

The Performance dimension catalogs features whose implementation or
configuration affects performance, described from the perspective of what
the driver implements and how that choice affects performance in general.
No external reference (workload, service contract, or benchmark) is used.
Where a feature has both functional and performance aspects, it appears in
both Section 4.1 and Section 4.3, with the relevant question asked in each.

**Scope includes:** FIFO depth usage and threshold tuning, RX timeout IRQ,
DMA engines, batched IRQ handling, per-CPU buffers, and any other
throughput or latency-oriented mechanism implemented by the driver.

### 4.4 Correctness Analysis

**Question.** What invariants, disciplines, and ordering constraints
govern *how* the Linux driver exercises the functional mechanisms safely?

This dimension is not about what the driver does, but about what it must
not do, and what must hold across interleavings, error paths, and hardware
quirks.

**Scope includes:**

- **Concurrency discipline.** Which state is shared between hard-IRQ,
  soft-IRQ (tasklet / workqueue), and process context; which lock covers
  which state; what must not be called under which lock.
- **Hardware-mandated ordering.** Example: DR-before-RSR read sequence for
  PL011 error clearing; UARTEN=0 requirement before reprogramming
  LCR_H / IBRD / FBRD then re-enable; IMSC clear before IRQ release during
  shutdown; MMIO barriers.
- **State-machine invariants.** Legal driver states and transitions
  (e.g. cannot TX before port open, cannot configure while enabled), and
  the pre- and post-conditions of each entry point.
- **Resource safety and bounded work.** IRQ handlers must terminate in
  bounded work; no unbounded loops under spinlock; no allocations in
  atomic context; no resource leaks on error-path unwind.
- **Data integrity.** Volatile requirements, required barriers between
  register accesses, cache and coherency assumptions.
- **Error-path recovery.** What must be restored when a multi-step
  operation (e.g. reconfigure) fails partway through.

### 4.5 Variability (Cross-Cutting Note)

The PL011 exists in multiple revisions and integrations with platform-
specific differences (FIFO depth, PrimeCell revision, vendor quirks,
clock / reset assumptions, optional capabilities). Wherever the Linux
driver branches on such variation, that variation is recorded in the
section to which the affected feature belongs, and cross-referenced under
a dedicated Variability subsection in the deliverable. This is not a fifth
analysis dimension; it is a cross-cutting axis every dimension must
account for.

## 5. Deliverable Shape

The analysis output is organized as four sections, one per dimension
(4.1 -- 4.4), each with a Variability subsection per Section 4.5.

For each feature the deliverable records:

- What the PL011 datasheet says about it.
- What the Linux stock driver does about it, with file and function
  references into the driver source provided.
- Any datasheet-vs-driver discrepancies observed.

The deliverable does not propose, design, or reference any DSL, compiler,
or specification machinery beyond the Linux driver and the datasheet
themselves.
