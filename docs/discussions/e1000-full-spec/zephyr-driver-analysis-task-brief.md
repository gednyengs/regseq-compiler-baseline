# E1000 Zephyr Stock Driver Analysis -- Task Brief

Status: APPROVED 2026-05-04 (D3 sign-off). Driver analysis work may begin.

This document is a rationale brief, not a work product. Its purpose is to let
a reviewer validate the approach and the analysis framework before we invest
effort in the actual analysis.

## 1. Purpose and Non-Purpose

**Purpose.** Produce a structured description of:

1. The functional and architectural capabilities of the Intel 82540EM E1000
   gigabit Ethernet controller as defined by its datasheet.
2. Which of those capabilities the Zephyr stock `eth_e1000` driver
   exercises, and how it exercises them.

The output is a descriptive reference document about the 82540EM device and
its Zephyr stock driver. It is intended to be consumed by downstream tasks
that fall outside this brief.

**Non-purpose.** This task is NOT:

- A design exercise for any DSL, compiler, or taxonomy.
- A workload enumeration or workload-driven scoping exercise.
- A comparison across operating systems. Only the Zephyr stock driver is
  analyzed.
- A platform-independence or portability exercise.
- A performance measurement or benchmarking exercise. Performance features
  are *catalogued* (Section 4.3); they are not measured.

## 2. Inputs

The analysis is driven entirely by two inputs:

1. The Intel 82540EM E1000 datasheet (Intel Software Developer's Manual
   for the PCI/PCI-X Family of Gigabit Ethernet Controllers, with
   82540EM-specific sections as the primary reference).
2. The Zephyr stock `eth_e1000` driver source as provided for this task,
   located under `/Users/nyengele/driverbase/os/zephyr/`:
   - `drivers/ethernet/eth_e1000.c` (main driver)
   - `drivers/ethernet/eth_e1000_priv.h` (private header)
   - `drivers/ethernet/Kconfig.e1000` (Kconfig)
   - `dts/bindings/ethernet/intel,e1000.yaml` (DT binding)

The agent analyzes all of the driver source provided. The Zephyr driver
is expected to be small (the main file is ~10 KiB) and to target a single
QEMU-emulated E1000 device. Where the datasheet and the driver disagree,
or where the driver appears to work around a hardware quirk not
documented in the datasheet, that discrepancy is recorded as a finding in
its own right.

## 3. Scope

- **In scope.** The 82540EM device behavior as defined by the datasheet;
  the behavior of the Zephyr stock `eth_e1000` driver, across the four
  analysis dimensions in Section 4, covering the full driver source
  provided.
- **Out of scope.** DSL / compiler / taxonomy design, workload definition,
  cross-OS comparison, portability analysis, performance benchmarking,
  any other Zephyr ethernet driver, any e1000e PCIe family driver
  (Zephyr does not ship one).

## 4. Analysis Framework -- Four Dimensions

Same four-dimension structure as the Linux brief.

### 4.1 Functional Analysis

**Question.** Which functional capabilities of the 82540EM device does
the Zephyr `eth_e1000` driver capture, and how does it capture them?

**Scope includes:** device reset and bring-up, EEPROM/NVM access (if
exercised), MAC address programming, receive filtering, receive control
(`RCTL`), transmit control (`TCTL`), descriptor-ring setup, TX/RX data
path, link auto-negotiation and link-state observation, PHY interaction
via `MDIC` (if exercised), interrupt management, hardware checksum
offload (if exercised), VLAN handling (if exercised), and any other
device-level behavior the driver exercises.

Capabilities NOT exercised by the Zephyr driver are recorded as such;
this is itself useful evidence about the minimum viable feature set for
an embedded ethernet driver.

### 4.2 Integration Analysis

**Question.** Which Zephyr subsystems does the stock driver integrate
the 82540EM device into, and how?

**Scope includes:** Zephyr `ethernet_api` participation, `net_if`
interface registration via `ETH_NET_DEVICE_DT_INST_DEFINE` (or older
init macro), DT binding consumption, PCI bus integration via Zephyr's
PCI subsystem, IRQ wiring via `IRQ_CONNECT` or DT IRQ properties, memory
allocation via Zephyr primitives (`k_malloc`, dedicated memory regions),
network L2 layer interaction (`ETHERNET_L2`), and any other Zephyr
subsystem touchpoints exposed by the driver.

### 4.3 Performance Analysis

**Question.** Which 82540EM performance features does the Zephyr driver
implement, and which Zephyr-side performance facilities does the driver
use?

**Scope includes:** RX/TX interrupt moderation if exercised,
descriptor-ring sizes and refill policy, RX buffer sizing, hardware
checksum offload (if exercised), and any other throughput or latency-
oriented mechanism implemented by the driver. Absence of a performance
feature is recorded.

### 4.4 Correctness Analysis

**Question.** What invariants, disciplines, and ordering constraints
govern *how* the Zephyr driver exercises the functional mechanisms
safely?

**Scope includes:**

- **Concurrency discipline.** Which state is shared between IRQ context,
  RX work-queue / fiber, TX submission context, and process context;
  Zephyr-style synchronization primitives (`k_sem`, `k_mutex`,
  `k_work`); what must not be called from which context.
- **Hardware-mandated ordering.** Descriptor write-back observation,
  RDT/TDT publish ordering with appropriate barriers, MAC reset
  sequence, PHY access protocol, EEPROM access protocol.
- **State-machine invariants.** Legal driver states and transitions
  (init / iface_init / start / stop), pre- and post-conditions of each
  entry point.
- **Resource safety and bounded work.** ISR bounded work; no allocations
  in atomic context; no leaks on error-path unwind.
- **Data integrity.** MMIO access discipline (`sys_read32`/`sys_write32`
  or equivalent), required barriers, DMA coherency on architectures
  where Zephyr maps the device coherently, cache coherency assumptions.
- **Error-path recovery.** What must be restored on partial failure
  during init or after a link change.

### 4.5 Variability (Cross-Cutting Note)

The Zephyr `eth_e1000` driver targets a single device profile (QEMU's
`-device e1000`, which is 82540EM). Where the driver branches on board,
DT property, or Kconfig option, that variation is recorded in the
section to which the affected feature belongs, and cross-referenced
under a dedicated Variability subsection in the deliverable.

## 5. Deliverable Shape

The analysis output is organized as four sections, one per dimension
(4.1 -- 4.4), each with a Variability subsection per Section 4.5.

For each feature the deliverable records:

- What the 82540EM datasheet says about it.
- What the Zephyr stock `eth_e1000` driver does about it, with file and
  function references into the driver source provided.
- Any datasheet-vs-driver discrepancies observed.
- Where a feature is NOT exercised by the Zephyr driver, this is
  recorded explicitly as evidence about the minimum viable embedded-
  driver feature set.

The deliverable does not propose, design, or reference any DSL, compiler,
or specification machinery beyond the Zephyr driver and the datasheet
themselves.
