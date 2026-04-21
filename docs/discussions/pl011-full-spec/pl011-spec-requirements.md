# PL011 Spec Requirements

**Role**: the definitive list of requirements the Regseq PL011
driver specification must capture. Every requirement is traced to
one of three evidence sources. No requirement is carried in the
spec without a traceable justification.

**Evidence sources** (per the round-3 Codex review outcome):

- **axis**: requirement is derived from one or more comparison axes
  defined in `pl011-comparison-axes.md`. Realizing the axis's
  workloads requires this capability in the spec.
- **correctness**: requirement is derived from PL011 hardware or
  Regseq semantic correctness/safety rules that the compiler cannot
  safely infer. Examples: register-write ordering, reserved-bit
  discipline, interrupt-mask lifecycle invariants.
- **backend-contract**: requirement is derived from what a generated
  backend needs the spec to declare in order for the backend to
  emit correct code. Examples: resource declarations, capability
  flags for optional features.

Scope boundary: this document enumerates what the SPEC must capture.
It does not specify how each backend realizes that requirement;
backend-specific realization lives in backend lowering notes.

## 1. Trace legend

Each requirement is tagged with `[source: ...]` where source is one
or more of `axis:<id>`, `correctness:<tag>`, `backend-contract:<tag>`.
Requirements that are paper-load-bearing for the current claim are
tagged `[claim-path]`. Requirements that are deferred (e.g., absolute-
only workloads, deferred use cases) are tagged `[deferred]` and do
not block the current claim.

## 2. Functional requirements (derived from F-axes)

### 2.1 TX/RX data path

- **REQ-TX-01** `[source: axis:F1, claim-path]`: the spec must
  express byte-at-a-time TX via the `DR` register guarded by the
  `TXFF` condition in the `FR` register.
- **REQ-TX-02** `[source: axis:F1, claim-path]`: the spec must
  express a TX path usable from polled and interrupt-driven contexts.
- **REQ-RX-01** `[source: axis:F2, claim-path]`: the spec must
  express byte-at-a-time RX via the `DR` register guarded by the
  `RXFE` condition in the `FR` register.
- **REQ-RX-02** `[source: axis:F2, axis:F5, claim-path]`: the spec
  must express RX error detection via the error-status bits co-resident
  on the `DR` word (overrun, break, parity, framing).
- **REQ-RX-03** `[source: axis:F2, correctness:read-dispose,
  claim-path]`: the spec must preserve the invariant that reading
  `DR` consumes one FIFO entry; duplicate reads are a protocol
  violation.

### 2.2 Baud and line format

- **REQ-CFG-01** `[source: axis:F3, claim-path]`: the spec must
  express baud configuration as a requested-baud value plus an
  input-clock property, and must program both `IBRD` and `FBRD`.
- **REQ-CFG-02** `[source: axis:F3, correctness:register-order,
  claim-path]`: the spec must enforce the PL011 rule that `IBRD`
  and `FBRD` writes precede any `LCR_H` write; an authored ordered
  sequence is required (current fixture uses `regseq::ordered!`).
- **REQ-CFG-03** `[source: axis:F4, claim-path]`: the spec must
  express word length (5-8), stop bits (1 or 2), and parity
  (none/odd/even; mark/space optional via SPS).
- **REQ-CFG-04** `[source: axis:F4, correctness:register-order,
  claim-path]`: the spec must enforce `LCR_H` being written after
  divisor programming and before UART-enable.

### 2.3 FIFO

- **REQ-FIFO-01** `[source: axis:F2, axis:P4, claim-path]`: the
  spec must express FIFO enable/disable via `LCR_H.FEN`.
- **REQ-FIFO-02** `[source: axis:P7, claim-path]`: the spec must
  expose FIFO interrupt-threshold configuration via the `IFLS`
  register.
- **REQ-FIFO-03** `[source: axis:F7, backend-contract:flush,
  claim-path]`: the spec must provide explicit TX/RX FIFO flush
  hooks (the current fixture has `flush_tx_fifo`, `flush_rx_fifo`).

### 2.4 Enable / disable / lifecycle

- **REQ-LIFE-01** `[source: axis:F6, axis:F7, claim-path]`: the
  spec must express UART enable/disable via `CR.UARTEN` with the
  surrounding documented ordering.
- **REQ-LIFE-02** `[source: axis:F7, correctness:register-order,
  claim-path]`: the spec must express the disable-before-reconfigure
  ordering required by the PL011 TRM for safe reconfiguration.
- **REQ-LIFE-03** `[source: axis:F7, axis:F9,
  correctness:register-order, claim-path]`: the spec must preserve
  `CR.RTS` state across disable/enable cycles so that auto-RTS
  behavior survives reconfiguration (required for F9). DTR
  preservation is NOT claim-path here; per
  `pl011-comparison-axes.md` Section 7, full modem outputs are
  out of the paper claim, and DTR preservation is covered under
  `REQ-DEF-MODEM-OUT` as an optional spec-surface behavior.

### 2.5 Interrupts

- **REQ-IRQ-01** `[source: axis:F6, claim-path]`: the spec must
  express the interrupt-mask register `IMSC`, raw-status `RIS`,
  masked-status `MIS`, and interrupt-clear `ICR` with W1C semantics
  for `ICR`.
- **REQ-IRQ-02** `[source: axis:F6, claim-path]`: the spec must
  express an ISR entry point that services RX/TX/error/modem IRQ
  sources.
- **REQ-IRQ-03** `[source: axis:F6, correctness:mask-shadow,
  claim-path]`: the spec must express a local shadow of the IMSC
  mask to avoid RMW on the live register in the hot path (this is
  the convergent pattern across corpus drivers).
- **REQ-IRQ-04** `[source: axis:F5, axis:F6, claim-path]`: the spec
  must enumerate and individually manipulate the named IRQ sources
  (RXIM, RTIM, TXIM, FEIM, PEIM, BEIM, OEIM, and modem sources
  where relevant).

### 2.6 Break and flow control

- **REQ-BRK-01** `[source: axis:F8, claim-path]`: the spec must
  express break generation via `LCR_H.BRK` (already captured in
  the fixture's `send_break`).
- **REQ-FLOW-01** `[source: axis:F9, axis:F10, claim-path]`: the
  spec must express CRTSCTS hardware flow control via `CR.CTSEN`
  and `CR.RTSEN`, with the auto-RTS-disable-before-drop sequencing
  rule.
- **REQ-FLOW-02** `[source: axis:F10, claim-path]`: the spec must
  express RX throttle/unthrottle hooks that configure the RX-side
  interrupt masks appropriately.

## 3. Performance requirements (derived from P-axes)

- **REQ-PERF-BATCH-01** `[source: axis:P4, correctness:hot-path,
  claim-path]`: the spec must allow backends to batch multi-byte
  TX and RX operations in the hot path. The fixture currently
  expresses byte-at-a-time paths; the generated backend is
  responsible for emitting batched loops. The spec must not
  encode a byte-at-a-time ISR contract that prevents batching.
- **REQ-PERF-IFLS-01** `[source: axis:P7, claim-path]`: spec must
  allow authored selection of `IFLS` levels per platform profile.
- **REQ-PERF-DMA-01** `[source: axis:P1, axis:P2, axis:P5,
  claim-path]`: the spec must express PL011 DMA participation via
  `DMACR` (RXDMAE, TXDMAE, DMAONERR) and the associated
  DMA-channel hookup contract. (DMA is realized through P1/P2/P5,
  not a separate axis; see `pl011-comparison-axes.md` Section 4.)
  Currently `pending` in `pl011-full-spec-status.md`; this is the
  next implementation target.
- **REQ-PERF-DMA-02** `[source: axis:P1, axis:P2, correctness:
  dma-policy, claim-path]`: the spec must express the PIO-DMA
  selection policy (the "DMA only above FIFO/2" threshold; single-
  char XON/XOFF path that bypasses DMA; residue-granularity
  capability check for RX DMA).
- **REQ-PERF-LATENCY-01** `[source: axis:P6, claim-path]`: the
  spec must avoid mandating unnecessary serialization primitives
  on the hot TX/RX paths that would introduce latency spikes.

## 4. Integration requirements (derived from I-axes)

### 4.1 Identity and binding

- **REQ-ID-01** `[source: axis:IL5, axis:IZ2, backend-contract:
  identity, claim-path]`: the spec must carry the portable
  `arm,pl011` compatible identity.
- **REQ-ID-02** `[source: axis:IL2, backend-contract:identity,
  claim-path]`: the spec must carry `bus = "amba"` to express the
  AMBA/PrimeCell family identity; this drives Linux emission to
  the AMBA driver registration path rather than generic platform.

### 4.2 Resources (optional, per binding)

- **REQ-RES-CLK-01** `[source: backend-contract:resource,
  claim-path]`: optional `Clock` resource; when absent, a default
  input clock frequency (property) is used.
- **REQ-RES-CLK-02** `[source: correctness:clock-lifecycle,
  claim-path]`: clock prepare+enable in the driver's startup path,
  disable+unprepare in the shutdown path, with error-unwind on
  startup failure.
- **REQ-RES-RST-01** `[source: backend-contract:resource,
  claim-path]`: optional reset-controller resource; when present,
  assert+release on startup.
- **REQ-RES-PIN-01** `[source: backend-contract:resource,
  claim-path]`: optional pinctrl states (default, sleep); default
  on open, sleep on close.
- **REQ-RES-IRQ-01** `[source: axis:F6, backend-contract:resource,
  claim-path]`: IRQ resource declaration; backends realize
  `request_irq`/`uart_irq_callback_set` accordingly.
- **REQ-RES-MMIO-01** `[source: backend-contract:resource,
  claim-path]`: MMIO resource declaration for the PL011 register
  block; backends realize `ioremap`/`DEVICE_MMIO_*` accordingly.

### 4.3 OS subsystem participation

- **REQ-INT-LIN-01** `[source: axis:IL3, axis:IL4, claim-path]`:
  the generated Linux driver must register with `serial_core` as a
  `uart_driver` + `uart_port`, exposing the `uart_ops` methods
  needed by axes IL3 and IL4.
- **REQ-INT-ZEP-01** `[source: axis:IZ3, axis:IZ4, claim-path]`:
  the generated Zephyr driver must participate in `uart_driver_api`
  (both polling and interrupt-driven subsets).

### 4.4 Deferred integration (not in current claim)

- **REQ-INT-CONSOLE-LINUX** `[source: axis-deferred:U-DEF-1,
  deferred]`: console/earlycon participation on Linux; considered
  future scope.
- **REQ-INT-PM** `[source: deferred]`: Linux suspend/resume and
  Zephyr PM integration; considered future scope.
- **REQ-INT-SYSFS-ADMIN** `[source: deferred]`: sysfs-driven
  bind/unbind and runtime PM administrative paths; deferred.

## 5. Correctness-only requirements (not tied to an axis)

These are required for the spec to be safely compilable and
lowerable, independent of which benchmarks are run.

- **REQ-CORR-REGORDER-01** `[source: correctness:register-order]`:
  PL011 TRM requires `IBRD`/`FBRD` writes before `LCR_H`; any
  baud/line-format change sequence must respect this. Already
  encoded in the fixture via `regseq::ordered!`.
- **REQ-CORR-REGORDER-02** `[source: correctness:register-order]`:
  `LCR_H` must be written with the FIFO enable bit after divisor
  programming; the compiler cannot infer this.
- **REQ-CORR-RESERVED-01** `[source: correctness:reserved-bits]`:
  reserved bits in CR, LCR_H, IFLS, IMSC, ICR, DMACR must be
  preserved on RMW or written as specified; authored register
  descriptors must encode the reserved-bit policy.
- **REQ-CORR-ICR-W1C** `[source: correctness:icr-semantics]`: ICR
  has write-one-to-clear semantics; authored sequences must
  respect this.
- **REQ-CORR-MASK-SHADOW** `[source: correctness:mask-shadow]`:
  the local IMSC shadow must stay consistent with the hardware
  IMSC after every mutation; hot-path mutations must write-through
  the shadow.

## 6. Backend-contract-only requirements (not tied to an axis)

- **REQ-BC-RES-TABLE** `[source: backend-contract:resource]`: every
  optional resource must be declared as optional in a stable shape
  so both Linux and Zephyr lowerings can emit conditional
  acquisition code.
- **REQ-BC-ISR-ENTRY** `[source: backend-contract:isr-entry]`: the
  ISR entry point must be a stable-named authored function with a
  documented return convention (the current `#[rseq::isr]`).
- **REQ-BC-LIFECYCLE** `[source: backend-contract:lifecycle]`:
  lifecycle methods (startup, shutdown, configure) must have
  documented preconditions about which resources are acquired by
  the compiler vs. by the authored body.

## 7. Requirements explicitly deferred

The following were considered during derivation and are NOT in
the current spec target. Each is captured so the deferral is
traceable and can be revisited without re-deriving.

- **REQ-DEF-IRDA** `[source: dropped-axis:IrDA]`: IrDA / SIR
  encoding via SIREN and associated SIRLP/paranoid timing. Out of
  scope (not UART-mode).
- **REQ-DEF-RS485** `[source: dropped-axis:RS-485]`: RS-485
  half-duplex direction control with pre/post-send delay. Out of
  scope for current claim; platform-dependent.
- **REQ-DEF-MODEM-OUT** `[source: dropped-axis:modem]`: full modem
  outputs (DTR, OUT1, OUT2). Modeled in the spec surface as an
  optional capability but not claim-load-bearing.
- **REQ-DEF-CONSOLE-LINUX** `[source: dropped-axis:console]`:
  Linux `struct console` registration + nbcon. Deferred.
- **REQ-DEF-WIRE-ABSOLUTE** `[source: dropped-axis:wire-rate]`:
  absolute-physical wire-rate throughput benchmarks. Deferred per
  QEMU-relative gate.
- **REQ-DEF-PM** `[source: deferred-use-case]`: suspend/resume, PM
  runtime. Deferred.
- **REQ-DEF-VARIANTS** `[source: scope-pin]`: vendor variants
  (ST, SBSA, QDF2400) and their quirk machinery. Out of scope.

## 8. Requirement-to-axis coverage table

The reverse-direction trace: for each axis, which requirements
realize it. Useful for audit.

| Axis | Realizing requirements |
|------|------------------------|
| F1 | REQ-TX-01, REQ-TX-02 |
| F2 | REQ-RX-01, REQ-RX-02, REQ-RX-03, REQ-FIFO-01 |
| F3 | REQ-CFG-01, REQ-CFG-02 |
| F4 | REQ-CFG-03, REQ-CFG-04 |
| F5 | REQ-RX-02, REQ-IRQ-04 |
| F6 | REQ-IRQ-01, REQ-IRQ-02, REQ-IRQ-03, REQ-IRQ-04, REQ-LIFE-01, REQ-RES-IRQ-01 |
| F7 | REQ-LIFE-01, REQ-LIFE-02, REQ-LIFE-03, REQ-FIFO-03 |
| F8 | REQ-BRK-01 |
| F9 | REQ-FLOW-01, REQ-LIFE-03 |
| F10 | REQ-FLOW-01, REQ-FLOW-02 |
| P1 | REQ-PERF-BATCH-01, REQ-PERF-DMA-01, REQ-PERF-DMA-02 |
| P2 | REQ-PERF-BATCH-01, REQ-PERF-DMA-01, REQ-PERF-DMA-02 |
| P3 | REQ-PERF-BATCH-01 |
| P4 | REQ-PERF-BATCH-01, REQ-FIFO-01, REQ-PERF-IFLS-01 |
| P5 | REQ-PERF-BATCH-01, REQ-PERF-DMA-01 |
| P6 | REQ-PERF-LATENCY-01 |
| P7 | REQ-FIFO-02, REQ-PERF-IFLS-01 |
| IL1 | (backend-emission concern; not spec-requirement) |
| IL2 | REQ-ID-02 |
| IL3 | REQ-INT-LIN-01 |
| IL4 | REQ-INT-LIN-01 |
| IL5 | REQ-ID-01 |
| IZ1 | (backend-emission concern) |
| IZ2 | REQ-ID-01 |
| IZ3 | REQ-INT-ZEP-01 |
| IZ4 | REQ-INT-ZEP-01 |

Reverse-table derivation notes:
- F2 includes REQ-FIFO-01 because FIFO enable is required for
  RX path byte integrity at baud rates where the 1-byte RBR is
  insufficient (TRM FIFO-enable rule).
- F9 includes REQ-LIFE-03 for RTS preservation across
  disable/enable (needed for auto-RTS continuity).
- F10 includes REQ-FLOW-01 because throttle/unthrottle policy
  branches on whether CRTSCTS is enabled.
- P4 realizing requirements: REQ-FIFO-01 (FIFO enable is the
  precondition for batching), REQ-PERF-BATCH-01 (hot-path
  batching allowance), REQ-PERF-IFLS-01 (threshold selection
  drives interrupt cadence). REQ-FIFO-02 realizes P7
  specifically, not P4.

## 9. Change history

- 2026-04-20: initial version derived from
  `pl011-comparison-axes.md` per round-4 Codex-reviewed strategy.
  Three-class evidence sourcing (axis / correctness /
  backend-contract) applied per round-3 Codex C2.
