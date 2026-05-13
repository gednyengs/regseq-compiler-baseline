# E1000 Spec Requirements

**Status**: SKELETON. Awaiting authoring once
`e1000-comparison-axes.md` locks the axes and the per-OS driver analyses
inform the correctness and backend-contract requirement classes.

**Role**: definitive list of requirements the Regseq E1000 driver
specification must capture. Every requirement traced to one of three
evidence sources. No requirement carried in the spec without traceable
justification.

**Evidence sources** (mirrors `pl011-spec-requirements.md`):

- **axis**: requirement is derived from one or more comparison axes
  defined in `e1000-comparison-axes.md`. Realizing the axis's workloads
  requires this capability in the spec.
- **correctness**: requirement is derived from E1000 hardware or Regseq
  semantic correctness/safety rules that the compiler cannot safely
  infer. Examples (provisional): descriptor-ring head/tail ordering,
  descriptor write-back semantics, link-up handshake ordering, MAC reset
  sequence.
- **backend-contract**: requirement is derived from what a generated
  backend needs the spec to declare in order to emit correct code.
  Examples: resource declarations (MMIO BAR, IRQ, DMA-coherent buffers),
  capability flags for optional features.

Scope boundary: this document enumerates what the SPEC must capture. It
does not specify how each backend realizes that requirement; backend-
specific realization lives in backend lowering notes.

## 1. Trace legend (to be authored)

Each requirement will be tagged with `[source: ...]` where source is one
or more of `axis:<id>`, `correctness:<tag>`, `backend-contract:<tag>`.
Requirements that are paper-load-bearing for the current claim are tagged
`[claim-path]`. Requirements that are deferred are tagged `[deferred]`
and do not block the current claim.

**Implementation-stage tags**: every claim-path requirement will carry
exactly one of `[stage-1]`, `[stage-2]`, or `[stage-3]`, reflecting the
progressive implementation plan. Stage definitions to be derived from the
axis set and QEMU/observability gating once
`e1000-experiment-methodology.md` enumerates required emulator
extensions.

## 1.1 Stage definitions (provisional, to be authored)

Provisional stage shape (parallels PL011):

- **Stage 1**: paper-min core. Compiler pipe-clean end-to-end, Rust-only
  fixture work, no QEMU patches required. Aims to unlock the largest
  subset of axes that QEMU's stock `-device e1000` already exercises
  faithfully (most of F1, F2, F4, F5, F6 RX/TX checksum where QEMU
  models them, F7, F8; P1-P6).
- **Stage 2**: features that require QEMU/observability extensions for
  honest claim-bearing comparison (e.g., link-state injection fidelity,
  VLAN-tag observability, interrupt-moderation observability if not
  already faithful in QEMU's e1000 model).
- **Stage 3**: performance extensions (e.g., richer descriptor-ring
  efficiency observation, fairness across moderation settings).

Authoritative stage tags to be set per requirement once requirements are
authored.

## 1.2 Stage membership summary (to be authored)

(populated after stage tags are assigned)

## 2. Functional requirements (derived from F-axes) (to be authored)

To be authored from `e1000-comparison-axes.md` Section 3.

Provisional structure (subsections to be confirmed):

- 2.1 TX/RX data path
- 2.2 Link management and PHY
- 2.3 Receive filtering (unicast, multicast, broadcast, promiscuous)
- 2.4 MAC address programming
- 2.5 Descriptor-ring management
- 2.6 Hardware checksum offload (TX, RX)
- 2.7 Interrupt management (`ICR`, `ICS`, `IMS`, `IMC`)
- 2.8 Lifecycle (reset, enable, disable, reconfigure)
- 2.9 VLAN handling (if in scope)

## 3. Performance requirements (derived from P-axes) (to be authored)

Provisional structure:

- REQ-PERF-MODERATION-* : interrupt moderation via `RDTR` / `RADV` /
  `ITR`
- REQ-PERF-RING-* : descriptor-ring sizing and refill policy
- REQ-PERF-CSUM-* : hardware checksum offload as performance feature
- REQ-PERF-LATENCY-* : avoid mandating unnecessary serialization on the
  hot path

Tolerance-provenance note (to be authored): every numeric tolerance
constant referenced by performance requirements inherits its bound from
the matching workload in `e1000-workloads.md`. Constants are PROVISIONAL
until the native-only pilot run completes.

## 4. Integration requirements (derived from I-axes) (to be authored)

### 4.1 Identity and binding

- REQ-ID-01 (provisional): the spec must carry the portable Linux PCI
  identity for 82540EM (vendor 0x8086, device 0x100E). Authoritative
  identity tuple to be confirmed against the corpus driver match tables.
- REQ-ID-02 (provisional): the spec must carry the Zephyr DT binding
  identity `intel,e1000` for Zephyr lowering.

### 4.2 Resources (optional, per binding)

To be enumerated. Provisional anchors:

- REQ-RES-MMIO-* : MMIO BAR resource declaration for the E1000 register
  block.
- REQ-RES-IRQ-* : IRQ resource declaration.
- REQ-RES-DMA-* : DMA coherent buffer / descriptor-ring memory.
- REQ-RES-MAC-* : MAC address property (from EEPROM or DT/ACPI override).

### 4.3 OS subsystem participation

- REQ-INT-LIN-* : Linux `net_device` registration via `netdev_ops`,
  `ethtool_ops`, NAPI participation.
- REQ-INT-ZEP-* : Zephyr `ETH_NET_DEVICE_DT_INST_DEFINE` participation
  via the `ethernet_api` and `net_if` interfaces.

### 4.4 Deferred integration (not in current claim)

- REQ-INT-PM (deferred): suspend/resume hooks. Considered future scope.
- REQ-INT-XDP (deferred): XDP / AF_XDP fast-path hooks. Considered
  future scope.

## 5. Correctness-only requirements (not tied to an axis) (to be authored)

Anchors (to be filled in after the four-OS correctness analyses land):

- REQ-CORR-RESET-* : MAC reset sequence and post-reset register
  restoration.
- REQ-CORR-RING-ORDER-* : descriptor-ring head/tail update ordering and
  memory-barrier discipline.
- REQ-CORR-WRITEBACK-* : descriptor write-back observation discipline
  (status bits, error bits).
- REQ-CORR-ICR-* : `ICR` clear-on-read semantics interaction with
  `IMS`/`IMC`.
- REQ-CORR-LINK-* : link-up handshake ordering with PHY.

## 6. Backend-contract-only requirements (not tied to an axis) (to be authored)

Anchors:

- REQ-BC-RES-TABLE: every optional resource declared as optional in a
  stable shape so both Linux and Zephyr lowerings can emit conditional
  acquisition code.
- REQ-BC-NAPI-CONTRACT (Linux side): hot-path budget contract for NAPI.
- REQ-BC-ZEPHYR-RX-FIBER (Zephyr side): RX fiber/dispatch contract.

## 7. Requirements explicitly deferred (to be authored)

Provisional:

- REQ-DEF-E1000E (variant pin): e1000e family. Out of scope.
- REQ-DEF-MSIX (variant pin): MSI-X. Out of scope (82540EM single-vector).
- REQ-DEF-MULTIQUEUE (variant pin): multi-queue. Out of scope.
- REQ-DEF-TSO (variant pin): TX segmentation offload. Out of scope.
- REQ-DEF-SRIOV (variant pin): SR-IOV / VF support. Out of scope.
- REQ-DEF-PM: suspend/resume. Deferred.
- REQ-DEF-XDP: XDP / AF_XDP. Deferred.
- REQ-DEF-VLAN (D8 lock 2026-05-04): 802.1Q VLAN tag insertion /
  stripping (`CTRL.VME`, TX descriptor VLAN field, RX descriptor
  `RXD_SPC_VLAN`, VFTA filter table). No use-case anchor in the locked
  U1-U7 set; comparison fairness preserved by skipping on both sides;
  Zephyr `eth_e1000` likely lacks VLAN support (Linux-only feature
  delta otherwise); spec surface non-trivial (VFTA is a 128 x 32-bit
  filter table). Re-evaluate if a future use case U8 (e.g., corporate
  / multi-tenant LAN endpoint) is added via a new D-decision.
- REQ-DEF-FLOW-CONTROL (D9 lock 2026-05-04): 802.3x link-level flow
  control (`CTRL.RFCE`/`CTRL.TFCE`, `FCAL`/`FCAH`/`FCT`,
  `FCRTL`/`FCRTH`, `FCTTV`). Anchored only weakly by U2; QEMU emulation
  cost substantially higher than PL011's analogous Stage 2 patch
  because PAUSE-frame round-trip needs a smarter chardev peer, not just
  register state; Zephyr `eth_e1000` likely lacks flow-control support
  (Linux-only feature delta otherwise); fairness preserved by symmetric
  skip on both sides. Re-evaluate if the paper claim shape requires
  Stage 2 symmetry with PL011 CRTSCTS.
- REQ-DEF-JUMBO (D10 lock 2026-05-04): jumbo frames (`RCTL.LPE`,
  `RCTL.BSIZE`, `RCTL.BSEX` and the 82540EM-specific jumbo errata
  workarounds). Single-axis U2 anchor without tension; D6 + D7 already
  provide the principal per-packet overhead amortization mechanisms;
  82540EM-specific erratum complexity would expand spec surface and
  methodology footprint; fairness preserved by symmetric skip at
  standard 1500 MTU. Re-evaluate if U2 needs to range over jumbo
  deployment envelope.
- (additional deferrals to be added as they emerge from analyses)

## 8. Requirement-to-axis coverage table (to be authored)

(populated after axes lock and requirements are authored)

## 9. Change history

- 2026-05-04: skeleton stood up.
- 2026-05-04: D6 locked. Hardware checksum offload (TX + RX) is
  claim-path. Spec surface to be carried: TX context descriptor variant
  (with `TXD_CMD_IXSM` / `TXD_OPTS_IXSM` / `TXD_OPTS_TXSM`); RX
  descriptor status bits (`RXD_STAT_IXSM`, `RXD_STAT_IPCS`,
  `RXD_STAT_TCPCS`, `RXD_STAT_UDPCS`); `RXCSUM` register. REQ-PERF-CSUM-*
  rows to be authored after axes lock; not in the deferred ledger
  (Section 7).
- 2026-05-04: D7 locked. Interrupt moderation (`RDTR` / `RADV` / `ITR`)
  is claim-path. Spec surface to be carried: the three named registers,
  exposed as authored / DT-overridable values so backends can lower to
  Linux module parameters (Linux) and DT properties (Zephyr; `itr` is
  already in the `intel,e1000` binding YAML). REQ-PERF-MODERATION-* rows
  to be authored after axes lock. Anchored by U2 vs U3 tension. QEMU
  fidelity audit flagged: stock `hw/net/e1000.c` may not honor
  RDTR/RADV/ITR -- patch likely needed (~50-100 LoC), tracked under
  `e1000-experiment-methodology.md` Section 3.
- 2026-05-04: D8 locked. VLAN tag insertion / stripping is deferred
  (REQ-DEF-VLAN added to Section 7). No use-case anchor in U1-U7;
  fairness preserved by symmetric skip; Zephyr likely lacks support
  anyway. Re-evaluate if a future U8 corporate / multi-tenant LAN
  use case is added.
- 2026-05-04: D9 locked. 802.3x flow control is deferred
  (REQ-DEF-FLOW-CONTROL added to Section 7). Weak U2 anchor; QEMU
  emulation cost much higher than PL011 CRTSCTS due to PAUSE-frame
  round-trip modeling; backend asymmetry (Zephyr lacks support);
  fairness preserved by symmetric skip. Re-evaluate if PL011 Stage 2
  symmetry is desired.
- 2026-05-04: D10 locked. Jumbo frames are deferred (REQ-DEF-JUMBO
  added to Section 7). Single-axis U2 anchor without tension; 82540EM
  jumbo errata complexity; D6+D7 already provide the principal
  per-packet overhead amortization. Fairness preserved by symmetric
  skip at 1500 MTU.
