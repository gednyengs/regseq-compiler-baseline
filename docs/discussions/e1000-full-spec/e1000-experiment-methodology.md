# E1000 Experiment Methodology -- How We Run the Comparisons in QEMU

**Status**: SKELETON. Awaiting authoring once
`e1000-comparison-axes.md` and `e1000-workloads.md` lock the axis and
workload set.

**Role**: operational bridge between the abstract workload definitions in
`e1000-workloads.md` and concrete execution on the evaluation
environment. Will answer, for every workload row, "what exact QEMU setup
and harness procedure runs this?"

**Relationship to existing evaluation docs** (to be confirmed and cited):

- `docs/discussions/evaluation-environment/reference-setup.md` --
  established throughput / latency / CPU / interrupt-count measurement
  methodology. This doc will cite and reuse, not duplicate.
- `docs/discussions/evaluation-environment/qemu-relative-evaluation-strategy.md`
  -- established the QEMU-relative-only evaluation gate.
- (E1000-specific topology document equivalent to PL011's
  `current-resolutions-and-strategy.md` to be authored or selected
  during methodology authoring.)

## 1. Topology summary (to be authored)

### 1.1 Linux topology

**Locked (D1, 2026-05-04)**: aarch64 `virt`, sharing the PL011 evaluation
substrate.

Rationale: substrate parity with the PL011 campaign minimizes methodology
surface area. The Linux `e1000` driver is architecture-independent; QEMU's
`-device e1000` (82540EM) emulates the device identically regardless of
guest CPU. PCI is architecture-portable.

Topology anchors (to be detailed during methodology authoring):

- **QEMU machine**: aarch64 `virt` (same machine type as PL011 evaluation).
- **QEMU binary**: patched v8.2.2 (PL011 baseline) plus any e1000-specific
  patches enumerated in Section 3.
- **Kernel and rootfs pins**: Linux v6.18.21 + Buildroot 2026.02 (PL011
  baseline). Single kernel build serves both campaigns.
- **Console / boot path**: existing PL011 (`ttyAMA0`) as the kernel
  console; the E1000 is the device under test, never the boot device.
  Boot disk: virtio-blk or initrd.
- **E1000 device**: one reserved E1000 PCI device on the virt machine's
  PCI host bridge for the driver under test. Vendor 0x8086, device
  0x100E (82540EM).
- **Host-side peer**: tap or socket chardev connected to the reserved
  E1000 so the test harness can inject and observe packets. Selection
  between tap / socket / unix-socket to be made during methodology
  authoring (Section 4 measurement mechanisms).
- **Topology-fairness mechanism**: differs from PL011's DT-`compatible`-
  override pattern because Linux PCI drivers bind by PCI ID, not by DT
  compatible string. The fairness rule (identical QEMU command line and
  kernel; only the driver under test differs) is preserved through
  module-load discipline -- the regseq-generated module is loaded on
  Regseq runs and the stock `e1000` module is loaded on native runs,
  via blacklist + `driver_override` or equivalent. Detailed mechanism
  to be specified in Section 5.5.

### 1.2 Zephyr topology

**Locked (D2, 2026-05-04)**: `qemu_cortex_a53` board with
`QEMU_CORTEX_A53_ETH_E1000=y`; native PL011 (the board's default UART)
serves as the control channel.

Rationale: the Zephyr `eth_e1000` driver ships pre-enabled for
`qemu_cortex_a53`. The board's `qemu_cortex_a53.dts` already contains an
`intel,e1000` DT node and the Kconfig option `QEMU_CORTEX_A53_ETH_E1000`
selects `ETH_E1000`. This:

- preserves substrate parity with PL011's Zephyr campaign (same board)
- preserves architecture parity with the Linux side (D1 = aarch64 virt)
- collapses the PL011 control-channel-proof gate: for E1000, the board's
  default PL011 is a separate device from the E1000 DUT, so the shell /
  log path can use PL011 directly without semihosting or aux-serial
  gymnastics

Topology anchors:

- **QEMU board**: `qemu_cortex_a53`.
- **DUT**: the `intel,e1000` device exposed by the board's DT.
- **Control channel (logs + shell)**: the qemu_cortex_a53 board's default
  PL011 UART. Distinct device from the E1000 DUT; no proof-of-viability
  gate required.
- **Zephyr DT binding identity**: `intel,e1000` (confirmed against the
  driver's `DT_DRV_COMPAT intel_e1000` definition).
- **Topology-fairness mechanism**: native and Regseq runs use identical
  QEMU command line and identical Zephyr SDK / kernel build; the
  difference is only the driver source linked into the Zephyr image.
  Detailed mechanism in Section 5.5.

Fairness rule, build-config items to align between native and Regseq:

- `CONFIG_ETH_E1000_PTP_CLOCK`: the Zephyr Kconfig comment marks it
  "still a dummy that is only used for debugging and currently does not
  do any real PTP activities". Disable on both sides to avoid
  measuring a no-op.
- `CONFIG_ETH_E1000_RX_QUEUE_SIZE` / `CONFIG_ETH_E1000_TX_QUEUE_SIZE`:
  match between sides.
- DT properties `rdmts` and `itr` exposed by the `intel,e1000` binding:
  match between sides; if the Regseq lowering does not yet honor these,
  flag as a backend-contract gap (REQ-INT-ZEP-*) before claim runs.

## 2. Per-workload execution plan (to be authored)

Will use a granular taxonomy parallel to `pl011-experiment-methodology.md`
Section 2:

- `today-byte-stream` -- runs today via packet-level integrity through
  unmodified QEMU.
- `today-register-only` -- executes today via register-state assertion
  only.
- `today-with-existing-perf-tools` -- runs today via `perf`,
  `/proc/interrupts`, ethtool counters, etc.
- `patch-required-<feature>` -- needs a QEMU e1000 model patch
  (Section 3).
- `zephyr-harness` -- needs the Zephyr experimental harness AND a proven
  auxiliary control channel.
- `optional-qemu-wire-fidelity` -- would benefit from a QEMU patch but
  does not require one.

## 3. QEMU extensions required (to be authored)

QEMU's stock `-device e1000` (82540EM) gives us a working data plane but
has known fidelity gaps. The patch set required will be enumerated here
once each axis is mapped to its observability needs. Provisional patch
catalog to confirm:

- 3.1 Link-state control + IRQ lifecycle: ability to assert/deassert
  link state from the host harness with full E1000 IRQ lifecycle (LSC
  bit) for axes that exercise link bring-up and re-negotiation.
- 3.2 Register-state introspection (QMP `query-e1000-state`): independent
  observation path for programmed register state (RCTL, TCTL, IMS,
  RDH/RDT, TDH/TDT, RDTR/RADV/ITR, RAR/MTA, etc.). Same role as PL011
  Patch 3.4. Useful for divergence-detection between driver-shadow and
  hardware state.
- 3.3 Interrupt-moderation observability: confirm `RDTR`/`RADV`/`ITR`
  actually shape interrupt cadence in QEMU's e1000 model; patch if
  needed for honest P4 / P7 claims.
- 3.4 Per-descriptor write-back fidelity: confirm QEMU writes back
  status bits correctly for hardware-checksum-offload assertions; patch
  if needed.
- 3.5 VLAN tag insertion/stripping (if F9 in scope): observability of
  CTRL.VME and per-descriptor VLAN bits.

To be confirmed against the actual QEMU `hw/net/e1000.c` source during
authoring; some of these may already be faithful and not need patches.

## 4. Measurement mechanisms (to be authored)

Per-workload measurements will reuse the mechanisms validated in
`reference-setup.md`. The table mapping observable to mechanism will be
authored alongside.

Single-clock host-only protocol: paper-load-bearing timing measurements
will use a single-clock host-side protocol (mirroring PL011 Section 4.2)
to avoid cross-clock guest-host synchronization confounds.

## 5. Zephyr experimental harness (to be authored)

### 5.1 Control-channel proof (prerequisite)

The Zephyr harness requires a control channel DISTINCT from the E1000
under test. Same gate shape as PL011 Section 5.1: a proof-of-viability
run must demonstrate the chosen channel works before any Zephyr workload
runs. If the proof fails, Zephyr falls back to functional-only
(F + I axes) with performance rows marked `not-applicable`.

### 5.2 App shape (assuming control channel proven)

To be authored.

### 5.3 Host-side harness

To be authored.

### 5.4 Coverage across workloads

To be authored.

### 5.5 Topology fairness (native-vs-Regseq binding switch)

Native and Regseq runs must differ ONLY in the driver under test. Same
fairness rule as PL011: identical QEMU command line except the
binding-affecting argument; both binding artifacts (DTBs, overlays, or
ACPI tables) recorded with SHA-256 in per-run metadata.

## 6. Pilot run (precedes tolerance-window finalization)

Per `e1000-workloads.md` Section 6, current tolerance constants will be
PROVISIONAL until a native-only pilot run completes. Pilot scope, gating,
and per-batch sentinels will be authored here following the PL011
template.

## 7. Data management and reproducibility (to be authored)

Per-run capture, archive location, and seed/repeatability rules to be
authored following the PL011 template (Section 7).

## 8. Execution sequencing (to be authored)

Campaign sequencing (QEMU patches -> Zephyr control-channel proof ->
topology-fairness binding artifacts -> pilot -> real campaign) to be
authored following the PL011 template (Section 8).

## 9. What this document is NOT

- Not a description of individual workloads. Those will live in
  `e1000-workloads.md`.
- Not a description of what we measure conceptually. That will live in
  `e1000-comparison-axes.md`.
- Not a replacement for the existing evaluation-environment docs; it will
  cite them.

## 10. Change history

- 2026-05-04: skeleton stood up.
- 2026-05-04: D1 locked. Linux evaluation host architecture is aarch64
  `virt`, sharing the PL011 evaluation substrate (patched QEMU v8.2.2,
  Linux v6.18.21, Buildroot 2026.02). Topology-fairness mechanism flagged
  as differing from PL011 due to PCI-ID binding; detail deferred to
  Section 5.5.
- 2026-05-04: D2 locked. Zephyr evaluation board is `qemu_cortex_a53`
  with `QEMU_CORTEX_A53_ETH_E1000=y`; native PL011 serves as the control
  channel. PL011's control-channel-proof gate (Section 5.1 of
  `pl011-experiment-methodology.md`) does not apply to E1000 because the
  PL011 console and the E1000 DUT are separate devices on
  `qemu_cortex_a53`. PTP clock and queue-size Kconfig items flagged for
  fairness alignment.
