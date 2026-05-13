# E1000 Linux Stock Driver Analysis -- Task Brief

Status: APPROVED 2026-05-04 (D3 sign-off). Driver analysis work may begin.

This document is a rationale brief, not a work product. Its purpose is to let
a reviewer validate the approach and the analysis framework before we invest
effort in the actual analysis.

## 1. Purpose and Non-Purpose

**Purpose.** Produce a structured description of:

1. The functional and architectural capabilities of the Intel 82540EM E1000
   gigabit Ethernet controller as defined by its datasheet.
2. Which of those capabilities the Linux stock `e1000` driver exercises, and
   how it exercises them.

The output is a descriptive reference document about the 82540EM device and
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

1. The Intel 82540EM E1000 datasheet (Intel Software Developer's Manual
   for the PCI/PCI-X Family of Gigabit Ethernet Controllers, with
   82540EM-specific sections as the primary reference).
2. The Linux stock `e1000` driver source as provided for this task, located
   at `/Users/nyengele/driverbase/os/linux/drivers/net/ethernet/intel/e1000/`:
   - `e1000_main.c` (main driver, probe/remove, lifecycle)
   - `e1000_hw.c`, `e1000_hw.h` (hardware abstraction layer)
   - `e1000_ethtool.c` (ethtool ops)
   - `e1000_param.c` (module parameters)
   - `e1000.h` (private header)
   - `e1000_osdep.h` (OS abstraction)
   - `Makefile`

The agent analyzes all of the driver source provided. Because the `e1000`
driver covers 82540/82541/82543-7, the agent SHOULD focus on the 82540EM-
relevant code paths and explicitly note where the driver branches on other
mac.type values; those branches are documented as variability per Section
4.5 but are not the primary analysis subject.

The analysis does not pre-select portions of the source to include or
exclude beyond the 82540EM scoping above; the scope of the source is fixed
by what is handed to it. Where the datasheet and the driver disagree, or
where the driver appears to work around a hardware quirk not documented in
the datasheet, that discrepancy is recorded as a finding in its own right.

## 3. Scope

- **In scope.** The 82540EM device behavior as defined by the datasheet;
  the behavior of the Linux stock `e1000` driver, across the four analysis
  dimensions in Section 4, covering the 82540EM-relevant code paths in the
  driver source provided.
- **Out of scope.** DSL / compiler / taxonomy design, workload definition,
  cross-OS comparison, portability analysis, performance benchmarking, and
  any speculation about what a portable driver specification would look
  like. The `e1000e` driver (PCIe family) is also out of scope.

## 4. Analysis Framework -- Four Dimensions

The analysis is partitioned along four dimensions. Each dimension asks a
different question about the driver. A single device feature is expected to
contribute to more than one dimension, because each dimension asks a
different question about it. For example, the RX descriptor ring shows up
under Functional as "what the device provides and how the driver
configures it," under Performance as "what the driver does to batch work
and reduce IRQ rate via NAPI and interrupt moderation," and under
Correctness as "what ordering and barrier rules govern descriptor
ownership and head/tail updates." A feature appearing in multiple sections
is the expected outcome, not a classification failure.

### 4.1 Functional Analysis

**Question.** Which functional capabilities of the 82540EM device does the
Linux driver capture, and how does it capture them?

**Scope includes:** device reset and bring-up, EEPROM/NVM access, MAC
address programming (`RAL`/`RAH`), receive filtering (unicast, multicast
hash, broadcast, promiscuous via `RCTL.MPE`/`UPE`/`BAM`), receive control
(`RCTL`), transmit control (`TCTL`), descriptor-ring setup (RX:
`RDBA`/`RDLEN`/`RDH`/`RDT`; TX: `TDBA`/`TDLEN`/`TDH`/`TDT`), TX/RX data
path, link auto-negotiation and link-state observation (`STATUS`,
`CTRL`), PHY interaction via `MDIC`, interrupt management
(`ICR`/`ICS`/`IMS`/`IMC`), hardware checksum offload (TX context
descriptors, RX status bits), VLAN handling (`CTRL.VME`, per-descriptor
VLAN), wake-on-LAN, flow control (`CTRL.RFCE`/`TFCE`, `FCAL`/`FCAH`/`FCT`,
`FCRTL`/`FCRTH`), and any other device-level behavior the driver
exercises.

### 4.2 Integration Analysis

**Question.** Which Linux kernel subsystems does the stock driver
integrate the 82540EM device into, and how?

**Scope includes:** `net_device` registration via `netdev_ops`, NAPI
participation, `ethtool_ops`, PCI bus integration (probe/remove, BAR
mapping, MSI/INTx setup), DMA-coherent allocation for descriptor rings
and buffers, sysfs / `/proc/net` visibility, ndo lifecycle
(`ndo_open`/`ndo_stop`/`ndo_start_xmit`/`ndo_set_rx_mode`/etc.), watchdog
timer, link-state propagation to userspace, power management hooks (if
exercised), and any other kernel-subsystem touchpoints exposed by the
driver.

### 4.3 Performance Analysis

**Question.** Which 82540EM performance features does the Linux driver
implement, and which Linux-side performance facilities does the driver
use to improve the throughput, latency, or CPU cost of device operation?

The Performance dimension catalogs features whose implementation or
configuration affects performance, described from the perspective of what
the driver implements and how that choice affects performance in general.
No external reference (workload, service contract, or benchmark) is used.
Where a feature has both functional and performance aspects, it appears
in both Section 4.1 and Section 4.3, with the relevant question asked in
each.

**Scope includes:** RX interrupt moderation (`RDTR`, `RADV`), TX interrupt
moderation, throttle register (`ITR`) usage, NAPI poll budget and weight,
descriptor-ring sizes and refill policy, RX buffer sizing strategies,
hardware checksum offload (as performance feature), TX completion
batching, interrupt coalescing in `ICR` polling, prefetch hints, and any
other throughput or latency-oriented mechanism implemented by the driver.

### 4.4 Correctness Analysis

**Question.** What invariants, disciplines, and ordering constraints
govern *how* the Linux driver exercises the functional mechanisms safely?

This dimension is not about what the driver does, but about what it must
not do, and what must hold across interleavings, error paths, and
hardware quirks.

**Scope includes:**

- **Concurrency discipline.** Which state is shared between hard-IRQ,
  NAPI poll context, ndo_start_xmit context, and process context; which
  lock covers which state; what must not be called under which lock;
  netif_tx_lock / xmit_lock interaction.
- **Hardware-mandated ordering.** Examples: descriptor write-back
  observation discipline (status bit polling vs. interrupt); RDT/TDT
  publish-via-MMIO-after-descriptor-store ordering with appropriate
  barriers (e.g., `wmb()` before tail write); MAC reset sequence and
  required register restoration ordering; PHY register access protocol
  via `MDIC`; EEPROM access protocol.
- **State-machine invariants.** Legal driver states and transitions
  (e.g., cannot xmit before open, cannot reconfigure rings while RCTL.EN
  is set, link-up handshake ordering), and the pre- and post-conditions
  of each entry point.
- **Resource safety and bounded work.** IRQ handlers must terminate in
  bounded work; NAPI budget must be honored; no allocations in atomic
  context; no resource leaks on error-path unwind (DMA mapping,
  descriptor-ring allocation, buffer pool, MSI/IRQ acquisition).
- **Data integrity.** Volatile / `__iomem` requirements, required
  barriers between MMIO accesses, DMA coherency assumptions for
  descriptors and buffers, cache flush/invalidate where applicable.
- **Error-path recovery.** What must be restored when a multi-step
  operation (e.g., ring reconfigure, MAC reset, link reset) fails
  partway through.

### 4.5 Variability (Cross-Cutting Note)

The 82540EM is one of several mac.type values handled by the same `e1000`
driver. Where the driver branches on `mac.type`, PHY type, or board-
specific quirks, that variation is recorded in the section to which the
affected feature belongs, and cross-referenced under a dedicated
Variability subsection in the deliverable. This is not a fifth analysis
dimension; it is a cross-cutting axis every dimension must account for.

## 5. Deliverable Shape

The analysis output is organized as four sections, one per dimension
(4.1 -- 4.4), each with a Variability subsection per Section 4.5.

For each feature the deliverable records:

- What the 82540EM datasheet says about it.
- What the Linux stock `e1000` driver does about it, with file and
  function references into the driver source provided.
- Any datasheet-vs-driver discrepancies observed.

The deliverable does not propose, design, or reference any DSL, compiler,
or specification machinery beyond the Linux driver and the datasheet
themselves.
