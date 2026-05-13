# E1000 FreeBSD Stock Driver Analysis -- Task Brief

Status: APPROVED 2026-05-04 (D3 sign-off). Driver analysis work may begin.

This document is a rationale brief, not a work product. Its purpose is to let
a reviewer validate the approach and the analysis framework before we invest
effort in the actual analysis.

## 1. Purpose and Non-Purpose

**Purpose.** Produce a structured description of:

1. The functional and architectural capabilities of the Intel 82540EM E1000
   gigabit Ethernet controller as defined by its datasheet.
2. Which of those capabilities the FreeBSD stock `em(4)` driver exercises,
   and how it exercises them, scoped to the 82540EM-relevant code paths.

The output is a descriptive reference document about the 82540EM device and
its FreeBSD stock driver. It is intended to be consumed by downstream tasks
that fall outside this brief.

**Non-purpose.** This task is NOT:

- A design exercise for any DSL, compiler, or taxonomy. No DSL, compiler
  pipeline, or construct-level modeling is in scope.
- A workload enumeration or workload-driven scoping exercise.
- A comparison across operating systems. Only the FreeBSD stock driver is
  analyzed.
- A platform-independence or portability exercise.
- A performance measurement or benchmarking exercise. Performance features
  are *catalogued* (Section 4.3); they are not measured.

## 2. Inputs

The analysis is driven entirely by two inputs:

1. The Intel 82540EM E1000 datasheet (Intel Software Developer's Manual
   for the PCI/PCI-X Family of Gigabit Ethernet Controllers, with
   82540EM-specific sections as the primary reference).
2. The FreeBSD stock `em(4)` driver source as provided for this task,
   located at `/Users/nyengele/driverbase/os/freebsd/sys/dev/e1000/`.
   Files in scope (82540EM-relevant subset of the shared HAL):
   - `if_em.c`, `if_em.h` (main driver)
   - `em_txrx.c` (TX/RX path)
   - `e1000_api.c`, `e1000_api.h` (HAL dispatch)
   - `e1000_82540.c` (82540-specific)
   - `e1000_mac.c`, `e1000_mac.h` (shared MAC)
   - `e1000_phy.c`, `e1000_phy.h` (shared PHY)
   - `e1000_nvm.c`, `e1000_nvm.h` (shared NVM)
   - `e1000_osdep.c`, `e1000_osdep.h` (OS abstraction)
   - `e1000_defines.h`, `e1000_regs.h`, `e1000_hw.h`

   Files OUT of scope (other family members, not exercised by 82540EM):
   - `e1000_82541.c`, `e1000_82541.h`
   - `e1000_82542.c`
   - `e1000_82543.c`, `e1000_82543.h`
   - `e1000_82571.c`, `e1000_82571.h`
   - `e1000_82575.c`, `e1000_82575.h`
   - `e1000_80003es2lan.c`, `e1000_80003es2lan.h`
   - `e1000_ich8lan.c`, `e1000_ich8lan.h`
   - `e1000_i210.c`, `e1000_i210.h`
   - `e1000_base.c`, `e1000_base.h`
   - `e1000_vf.c`, `e1000_vf.h`
   - `e1000_mbx.c`, `e1000_mbx.h`
   - `e1000_manage.c`, `e1000_manage.h`
   - `igb_txrx.c`

The agent analyzes all in-scope source. Because `if_em.c` and `em_txrx.c`
cover both legacy and PCIe variants, the agent SHOULD focus on the
82540EM-relevant code paths and explicitly note where the driver branches
on `hw->mac.type` or related capability flags; those branches are
documented as variability per Section 4.5 but are not the primary analysis
subject.

Where the datasheet and the driver disagree, or where the driver appears
to work around a hardware quirk not documented in the datasheet, that
discrepancy is recorded as a finding in its own right.

## 3. Scope

- **In scope.** The 82540EM device behavior as defined by the datasheet;
  the behavior of the FreeBSD stock `em(4)` driver, across the four
  analysis dimensions in Section 4, covering the in-scope source files
  listed in Section 2.
- **Out of scope.** DSL / compiler / taxonomy design, workload definition,
  cross-OS comparison, portability analysis, performance benchmarking,
  the FreeBSD `igb(4)` driver, and the out-of-scope source files listed
  in Section 2.

## 4. Analysis Framework -- Four Dimensions

Same four-dimension structure as the Linux brief. Each dimension asks a
different question about the driver. A single device feature is expected
to contribute to more than one dimension.

### 4.1 Functional Analysis

**Question.** Which functional capabilities of the 82540EM device does
the FreeBSD `em(4)` driver capture, and how does it capture them?

**Scope includes:** device reset and bring-up, EEPROM/NVM access, MAC
address programming, receive filtering (unicast, multicast hash,
broadcast, promiscuous), receive control (`RCTL`), transmit control
(`TCTL`), descriptor-ring setup, TX/RX data path (including the iflib
shim where applicable), link auto-negotiation and link-state observation,
PHY interaction via `MDIC`, interrupt management, hardware checksum
offload, VLAN handling, wake-on-LAN, flow control, and any other
device-level behavior the driver exercises.

### 4.2 Integration Analysis

**Question.** Which FreeBSD kernel subsystems does the stock driver
integrate the 82540EM device into, and how?

**Scope includes:** iflib integration (if used; `em(4)` is iflib-based in
modern FreeBSD), `if_t`/ifnet registration, PCI bus integration
(probe/attach/detach, BAR mapping, INTx/MSI), bus_dma allocation for
descriptor rings and buffers, sysctl visibility, ifconfig integration,
link-state propagation, BPF (Berkeley Packet Filter) hooks, and any other
kernel-subsystem touchpoints exposed by the driver.

### 4.3 Performance Analysis

**Question.** Which 82540EM performance features does the FreeBSD driver
implement, and which FreeBSD-side performance facilities does the driver
use?

**Scope includes:** RX interrupt moderation (`RDTR`, `RADV`), TX
interrupt moderation, `ITR` usage, descriptor-ring sizes and refill
policy, iflib-side TX/RX batching, RX buffer sizing strategies, hardware
checksum offload (as performance feature), and any other throughput or
latency-oriented mechanism implemented by the driver.

### 4.4 Correctness Analysis

**Question.** What invariants, disciplines, and ordering constraints
govern *how* the FreeBSD driver exercises the functional mechanisms
safely?

**Scope includes:**

- **Concurrency discipline.** Which state is shared between interrupt
  filters, TX/RX taskqueues, ifconfig context, and process context; lock
  hierarchies (driver lock, iflib locks); what must not be called under
  which lock.
- **Hardware-mandated ordering.** Descriptor write-back observation,
  RDT/TDT publish ordering with appropriate barriers, MAC reset
  sequence, PHY access protocol, EEPROM access protocol.
- **State-machine invariants.** Legal driver states and transitions
  (init/up/down/detach), pre- and post-conditions of each entry point.
- **Resource safety and bounded work.** Interrupt-filter constraints,
  taskqueue usage, no leaks on error-path unwind (bus_dma, BAR,
  resources).
- **Data integrity.** `bus_space` access discipline, required barriers,
  bus_dmamap_sync usage for descriptors and buffers, cache coherency
  assumptions.
- **Error-path recovery.** What must be restored on partial failure.

### 4.5 Variability (Cross-Cutting Note)

The 82540EM is one of several mac.type values handled by the same `em(4)`
driver via the shared Intel HAL. Where the driver branches on
`hw->mac.type`, PHY type, or board-specific quirks, that variation is
recorded in the section to which the affected feature belongs, and
cross-referenced under a dedicated Variability subsection in the
deliverable.

## 5. Deliverable Shape

The analysis output is organized as four sections, one per dimension
(4.1 -- 4.4), each with a Variability subsection per Section 4.5.

For each feature the deliverable records:

- What the 82540EM datasheet says about it.
- What the FreeBSD stock `em(4)` driver does about it, with file and
  function references into the in-scope driver source.
- Any datasheet-vs-driver discrepancies observed.

The deliverable does not propose, design, or reference any DSL, compiler,
or specification machinery beyond the FreeBSD driver and the datasheet
themselves.
