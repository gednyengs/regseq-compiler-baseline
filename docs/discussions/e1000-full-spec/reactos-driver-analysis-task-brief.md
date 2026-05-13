# E1000 ReactOS Stock Driver Analysis -- Task Brief

Status: APPROVED 2026-05-04 (D3 sign-off). Driver analysis work may begin.

This document is a rationale brief, not a work product. Its purpose is to let
a reviewer validate the approach and the analysis framework before we invest
effort in the actual analysis.

## 1. Purpose and Non-Purpose

**Purpose.** Produce a structured description of:

1. The functional and architectural capabilities of the Intel 82540EM E1000
   gigabit Ethernet controller as defined by its datasheet.
2. Which of those capabilities the ReactOS stock E1000 NDIS miniport
   driver exercises, and how it exercises them.

The output is a descriptive reference document about the 82540EM device and
its ReactOS stock driver. It is intended to be consumed by downstream tasks
that fall outside this brief.

**Non-purpose.** This task is NOT:

- A design exercise for any DSL, compiler, or taxonomy.
- A workload enumeration or workload-driven scoping exercise.
- A comparison across operating systems. Only the ReactOS stock driver is
  analyzed.
- A platform-independence or portability exercise.
- A performance measurement or benchmarking exercise. Performance features
  are *catalogued* (Section 4.3); they are not measured.
- A claim about Microsoft Windows. The ReactOS driver is an independent
  open-source NDIS-shape driver. Statements about NDIS API contracts are
  to be sourced from public NDIS documentation and from how the ReactOS
  driver uses those APIs, not from any proprietary Windows source.

## 2. Inputs

The analysis is driven entirely by two inputs:

1. The Intel 82540EM E1000 datasheet (Intel Software Developer's Manual
   for the PCI/PCI-X Family of Gigabit Ethernet Controllers, with
   82540EM-specific sections as the primary reference).
2. The ReactOS stock E1000 NDIS miniport driver source as provided for
   this task, located at
   `/Users/nyengele/driverbase/os/reactos/drivers/network/dd/e1000/`:
   - `hardware.c` (device-level operations)
   - `interrupt.c` (ISR / DPC)
   - `send.c` (TX path)
   - `ndis.c` (NDIS miniport entry points)
   - `info.c`
   - `debug.c`, `debug.h`
   - `nic.h`, `e1000hw.h`
   - `e1000.rc`, `nete1000.inf`, `CMakeLists.txt`

The agent analyzes all of the driver source provided. Where the datasheet
and the driver disagree, or where the driver appears to work around a
hardware quirk not documented in the datasheet, that discrepancy is
recorded as a finding in its own right.

## 3. Scope

- **In scope.** The 82540EM device behavior as defined by the datasheet;
  the behavior of the ReactOS stock E1000 NDIS miniport driver, across
  the four analysis dimensions in Section 4, covering the full driver
  source provided.
- **Out of scope.** DSL / compiler / taxonomy design, workload definition,
  cross-OS comparison, portability analysis, performance benchmarking,
  any speculation about what a portable driver specification would look
  like, and any reverse-engineering claim about proprietary Windows
  drivers.

## 4. Analysis Framework -- Four Dimensions

Same four-dimension structure as the Linux brief. Each dimension asks a
different question about the driver. A single device feature is expected
to contribute to more than one dimension.

### 4.1 Functional Analysis

**Question.** Which functional capabilities of the 82540EM device does
the ReactOS driver capture, and how does it capture them?

**Scope includes:** device reset and bring-up, EEPROM/NVM access, MAC
address programming, receive filtering, receive control (`RCTL`),
transmit control (`TCTL`), descriptor-ring setup, TX/RX data path, link
auto-negotiation and link-state observation, PHY interaction via `MDIC`,
interrupt management, hardware checksum offload (if exercised), VLAN
handling (if exercised), and any other device-level behavior the driver
exercises.

The ReactOS driver is expected to be substantially smaller and more
limited than the Linux or FreeBSD drivers. Capabilities NOT exercised by
ReactOS are recorded as such; this is itself useful evidence about the
minimum viable feature set for an NDIS-shape driver.

### 4.2 Integration Analysis

**Question.** Which ReactOS / NDIS subsystems does the stock driver
integrate the 82540EM device into, and how?

**Scope includes:** NDIS miniport driver registration, `MiniportXxx`
entry points (`MiniportInitializeEx`, `MiniportHaltEx`, `MiniportReset`,
`MiniportSendNetBufferLists`, `MiniportReturnNetBufferLists`,
`MiniportInterrupt`, `MiniportInterruptDPC`, `MiniportOidRequest`, etc.),
PCI bus integration (resource allocation, BAR mapping, MSI/INTx setup
through NDIS), DMA buffer allocation through NDIS shared-memory
primitives, OID handling, link-state propagation, INF binding, and any
other NDIS subsystem touchpoints exposed by the driver.

### 4.3 Performance Analysis

**Question.** Which 82540EM performance features does the ReactOS driver
implement, and which NDIS-side performance facilities does the driver
use?

**Scope includes:** RX/TX interrupt moderation (`RDTR`/`RADV`/`ITR`)
configuration if exercised, descriptor-ring sizes and refill policy,
DPC-side batching, hardware checksum offload (if exercised), and any
other throughput or latency-oriented mechanism implemented by the
driver. Absence of a performance feature is recorded.

### 4.4 Correctness Analysis

**Question.** What invariants, disciplines, and ordering constraints
govern *how* the ReactOS driver exercises the functional mechanisms
safely?

**Scope includes:**

- **Concurrency discipline.** Which state is shared between
  `MiniportInterrupt` (high IRQL), `MiniportInterruptDPC` (DISPATCH_LEVEL
  DPC), `MiniportSendNetBufferLists` (DISPATCH_LEVEL), and PASSIVE_LEVEL
  control paths; spin-lock discipline; what must not be called at which
  IRQL.
- **Hardware-mandated ordering.** Descriptor write-back observation,
  RDT/TDT publish ordering with appropriate barriers, MAC reset
  sequence, PHY access protocol, EEPROM access protocol.
- **State-machine invariants.** Legal miniport states and transitions
  (Initialize / Halt / Pause / Restart / Reset), pre- and
  post-conditions of each NDIS callback.
- **Resource safety and bounded work.** ISR must be brief and not
  allocate; DPC must terminate in bounded work; no leaks on error-path
  unwind (resource list, NDIS handles, shared-memory allocations).
- **Data integrity.** Volatile / register-mapped access discipline,
  required barriers, NDIS shared-memory coherency assumptions, cache
  coherency assumptions.
- **Error-path recovery.** What must be restored on partial failure
  during initialize, on reset, or on link change.

### 4.5 Variability (Cross-Cutting Note)

The ReactOS driver may target only a narrow subset of the E1000 family
(potentially 82540EM only, or a small set of legacy variants). Where the
driver branches on PCI device ID, mac.type, or other variability, that
is recorded in the section to which the affected feature belongs, and
cross-referenced under a dedicated Variability subsection in the
deliverable.

## 5. Deliverable Shape

The analysis output is organized as four sections, one per dimension
(4.1 -- 4.4), each with a Variability subsection per Section 4.5.

For each feature the deliverable records:

- What the 82540EM datasheet says about it.
- What the ReactOS stock driver does about it, with file and function
  references into the driver source provided.
- Any datasheet-vs-driver discrepancies observed.
- Where a feature is NOT exercised by the ReactOS driver, this is
  recorded explicitly as evidence about the minimum viable NDIS-shape
  feature set.

The deliverable does not propose, design, or reference any DSL, compiler,
or specification machinery beyond the ReactOS driver and the datasheet
themselves.
