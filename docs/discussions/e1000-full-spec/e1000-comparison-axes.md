# E1000 Comparison Axes

> **⚠ SUPERSEDED (2026-05-19).** The authoritative E1000 evaluation strategy
> is `../e1000-evaluation-strategy/`. The full-coverage axis matrix here is
> withdrawn in favor of a minimal-sufficient, patch-free evaluation. Retained
> for provenance only — do not plan or report experiments from it. Per-axis
> disposition (U1–U7 are carried forward):
> `../e1000-evaluation-strategy/04-supersession-and-traceability.md`.

**Status**: SUPERSEDED (was SKELETON). See banner above.

**Role**: load-bearing artifact for the Regseq E1000 evaluation strategy.

This document will define WHAT we compare when we compare a Regseq-generated
Intel 82540EM E1000 driver to the corresponding OS-native E1000 driver.
The axes documented here will be the sole source of "comparability" for
the paper's central claim. Every requirement in
`e1000-spec-requirements.md`, every workload in `e1000-workloads.md` will
trace back to an axis defined here (or to correctness/safety or
backend-contract rules, which are tracked separately).

**Derivation discipline** (mirrors `pl011-comparison-axes.md`): axes are
derived from concrete E1000 system-use cases, NOT from what Regseq happens
to be able to do. This is the anti-gaming rule that defends the claim at
peer review. Any axis added to this document must cite at least one
system-use case from Section 2 that implies it. Any system-use case
considered and dropped from scope must appear in the "dropped" ledger
with rationale.

## 1. Thesis and scope (to be authored)

**Thesis** (provisional):

> Regseq's semantic driver model expresses a single Intel 82540EM E1000
> specification, and its compiler generates Linux and Zephyr drivers that
> match native-driver behavior within predefined tolerance rules on
> QEMU-relative functional and performance workloads derived from concrete
> E1000 system use cases.

**Scope** (provisional):

- Single device: Intel 82540EM. No e1000e variants and no other legacy
  variants (82541, 82543-7) in the claim.
- Two backends in the claim: Linux and Zephyr.
- Corpus (for evidence, policy reference, and comparison baselines, not
  inclusion gating): Linux `drivers/net/ethernet/intel/e1000/`, FreeBSD
  `sys/dev/e1000/` (em(4) scoped to 82540EM-relevant code), ReactOS
  `drivers/network/dd/e1000/`, Zephyr
  `drivers/ethernet/eth_e1000.c`. Files cached under
  `/Users/nyengele/driverbase/os/<os>/...`.
- Comparisons are QEMU-relative only: Regseq-generated driver versus
  native driver under the same QEMU setup. Absolute-physical claims are
  out of scope for the current version.

## 2. System-use cases for E1000

**Locked (D4, 2026-05-04)**.

These are the concrete deployed roles in which a 82540EM E1000 NIC
actually does work in a system. They are the reason the hardware exists.
The axes in Sections 3-5 are derived from what these use cases demand.

| ID  | Use case                       | Summary                                                                                                                                                                                                              |
| --- | ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| U1  | Host networking endpoint       | TCP/UDP traffic through the host network stack, either guest-initiated (client) or externally-initiated and served by the guest (server). The driver mediates packet I/O between user-space and the wire.           |
| U2  | Bulk file transfer             | Sustained large-payload TX or RX (HTTP download, NFS, scp). Descriptor-ring efficiency, hardware-checksum offload, and interrupt-moderation policy dominate.                                                         |
| U3  | Low-latency request-response   | Small-packet latency-critical workloads (RPC, DB query, ping). Per-packet IRQ cadence and the small-write-to-wire latency dominate.                                                                                  |
| U4  | Interactive shell over network | Mixed small-packet bidirectional traffic (SSH, telnet). Touches both latency and modest throughput simultaneously.                                                                                                   |
| U5  | DHCP / boot-time bring-up      | The driver must complete link bring-up plus early packet exchange during the OS boot sequence. Exercises link auto-negotiation, MAC address programming, and broadcast / unicast filtering during a cold start.      |
| U6  | Link state and reconfiguration | The driver responds to link-up / link-down transitions, MAC address changes, and reconfiguration without packet corruption around the transition. Exercises link-state IRQ lifecycle and reconfigure-around-data.   |
| U7  | Network observation / packet capture | Userspace tools (tcpdump, libpcap-based monitors) capture network traffic via promiscuous mode and raw sockets. The driver delivers all RX traffic to userspace regardless of unicast/multicast filtering. |

Use cases considered and deferred:

- **U-DEF-1**: kernel hardware-accelerated forwarding (XDP / AF_XDP).
  e1000e / PCIe-class territory; XDP was not designed against legacy
  PCI E1000 parts. Out of scope by the variant pin.
- **U-DEF-2**: SR-IOV VF role. Out of scope by the variant pin (82540EM
  has no VF support).
- **U-DEF-3**: PXE network boot. Realized by pre-OS firmware UNDI
  drivers, not by an OS-resident driver. Not a Regseq target.
- **U-DEF-4**: jumbo frames beyond the 82540EM native MTU envelope. A
  bandwidth feature, not a distinct deployment role; if in scope it is
  realized through U2's throughput observation, not as its own use
  case.
- **U-DEF-5**: Wake-on-LAN as an active deployment role. PM-adjacent;
  deferred symmetrically with PL011's PM deferral.
- **U-DEF-6**: Guest-kernel network bridging / forwarding. The driver's
  role is endpoint (U1, U7); bridging is a kernel-stack role above the
  driver and is not a driver-behavior axis.

Use cases reframed during D4:

- The originally sketched "service-level forwarding" use case (provisional
  U5 in the pre-lock skeleton) is folded into U1. The driver-level
  behavior is identical whether the userspace endpoint is a client or a
  server; the merged U1 explicitly covers both.

## 3. Functional axes (derived) (to be authored)

Provisional sketch. Each axis names a functional property of the driver
that can be independently observed and compared. Each axis must be tagged
with the system-use cases that imply it. Failures on a functional axis
mean the driver does not correctly realize a use case.

| ID   | Axis (provisional)                          | Implied by (TBD) |
| ---- | ------------------------------------------- | ---------------- |
| F1   | TX path packet integrity                    | TBD              |
| F2   | RX path packet integrity                    | TBD              |
| F3   | Link auto-negotiation and link-state report | TBD              |
| F4   | MAC address programming                     | TBD              |
| F5   | Receive filtering (unicast / multicast)     | TBD              |
| F6   | Hardware checksum offload (TX / RX)         | TBD              |
| F7   | Interrupt-driven service lifecycle          | TBD              |
| F8   | Enable / disable / reconfigure lifecycle    | TBD              |
| F9   | VLAN tag insertion / stripping (if in scope)| TBD              |
| F10  | Promiscuous-mode toggle                     | TBD              |

To be reviewed and finalized after the per-OS driver analyses land.

## 4. Performance axes (derived, QEMU-relative) (to be authored)

Provisional sketch. All comparisons relative; absolute wire-rate claims
not paper-load-bearing.

| ID  | Axis (provisional)                     | Implied by (TBD) |
| --- | -------------------------------------- | ---------------- |
| P1  | Sustained TX throughput                | TBD              |
| P2  | Sustained RX throughput                | TBD              |
| P3  | RX robustness under CPU stress         | TBD              |
| P4  | Interrupts per packet (moderation)     | TBD              |
| P5  | Guest CPU cost per byte                | TBD              |
| P6  | Small-packet round-trip latency        | TBD              |
| P7  | Descriptor-ring fill / drain efficiency| TBD              |

To be reviewed and finalized.

## 5. Integration axes (derived) (to be authored)

Linux integration axes:

| ID  | Axis (provisional)                        | Observable (TBD) |
| --- | ----------------------------------------- | ---------------- |
| IL1 | Out-of-tree module builds and loads       | TBD              |
| IL2 | PCI probe binds the reserved device       | TBD              |
| IL3 | Usable netdev (ifconfig/ip up/down)       | TBD              |
| IL4 | net_device_ops integration                | TBD              |
| IL5 | ethtool baseline operations               | TBD              |
| IL6 | sysfs / /proc/net visibility              | TBD              |

Zephyr integration axes:

| ID  | Axis (provisional)                        | Observable (TBD) |
| --- | ----------------------------------------- | ---------------- |
| IZ1 | Builds in canonical app on chosen QEMU board | TBD           |
| IZ2 | Devicetree `intel,e1000` binding match    | TBD              |
| IZ3 | `ethernet_api` participation              | TBD              |
| IZ4 | `net_if` interface registration           | TBD              |

To be reviewed and finalized.

## 6. Tolerance and verdict discipline (to be authored)

Mirrors `pl011-comparison-axes.md` Section 6. Functional axes are
pass/fail with no tolerance. Performance axes use a tolerance window with
the constants set per workload in `e1000-workloads.md`. Integration axes
are pass/fail.

## 7. Dropped-axes ledger (to be authored)

Axes considered and dropped, with rationale. To be filled in as the
derivation progresses.

| Dropped axis | Rationale |
| ------------ | --------- |
| TBD          | TBD       |

## 8. Review discipline

Additions to this document:

- New use case: add to Section 2 with summary and ID. If it implies new
  axes, add them to Sections 3-5.
- New axis: must cite at least one use case from Section 2. If no use case
  applies, either add the use case first or reject.
- Dropped axis: move to Section 7 with rationale. Do not delete.

Removal of use cases or axes:

- Any removal must be logged in a change history at the bottom of this
  document or in a git commit message. Axes that have been reported in a
  published paper cannot be silently removed.

## Change history

- 2026-05-04: skeleton stood up. Provisional thesis, system-use case
  candidates, and axis sketches recorded. Authoritative content awaits
  authoring after the per-OS driver analyses land.
- 2026-05-04: D4 locked. Section 2 (system-use cases) finalized. Seven
  use cases (U1-U7) plus six deferred use cases (U-DEF-1 .. U-DEF-6).
  Original provisional U5 (service-level forwarding) folded into U1
  since the driver-level behavior is endpoint-equivalent. Original
  provisional U6 (DHCP bring-up) renumbered to U5; original U7 (link
  state) renumbered to U6. New U7 (network observation / packet
  capture) added to provide the use-case anchor for receive-filter
  axes (multicast / promiscuous).
- 2026-05-04: D5 locked. Anti-gaming derivation discipline confirmed
  unchanged from PL011: axes derive from system-use cases, not from
  QEMU fidelity. QEMU fidelity gaps go to the patches catalog
  (`e1000-experiment-methodology.md` Section 3), not to silent axis
  exclusions. High-variance escalation ladder (methodology repair ->
  qualitative observation -> claim shrink) applies the same way as in
  `pl011-workloads.md` Section 6.
- 2026-05-04: D6 locked. Hardware checksum offload (TX + RX) is
  claim-path. Anchored by U2 (primary), U1, U7. See
  `e1000-spec-requirements.md` change history for spec-surface impact.
- 2026-05-04: D7 locked. Interrupt moderation (`RDTR` / `RADV` / `ITR`)
  is claim-path. Anchored by U2 vs U3 tension; also U7. See
  `e1000-spec-requirements.md` change history for spec-surface impact.
  QEMU patch likely required for fidelity.
- 2026-05-04: D8 locked. VLAN tag insertion / stripping is deferred.
  No anchor in U1-U7; deferral preserves comparison fairness symmetrically.
  See `e1000-spec-requirements.md` Section 7 (REQ-DEF-VLAN).
- 2026-05-04: D9 locked. 802.3x flow control is deferred. Weak U2
  anchor; QEMU emulation cost prohibitive vs PL011 analog; Zephyr
  feature-delta on its own; fairness preserved by symmetric skip.
  See `e1000-spec-requirements.md` Section 7 (REQ-DEF-FLOW-CONTROL).
- 2026-05-04: D10 locked. Jumbo frames are deferred. Single-axis U2
  anchor without tension; 82540EM jumbo errata complexity; fairness
  preserved by symmetric skip at 1500 MTU.
- 2026-05-19: SUPERSEDED by `../e1000-evaluation-strategy/`. The
  full-coverage axis matrix and the QEMU device-model patch program are
  withdrawn; the E1000 evaluation is now minimal-sufficient and patch-free.
  U1–U7 are carried forward. See
  `../e1000-evaluation-strategy/04-supersession-and-traceability.md` for the
  per-axis disposition. Retained for provenance only; do not plan or report
  experiments from it.
  See `e1000-spec-requirements.md` Section 7 (REQ-DEF-JUMBO).
