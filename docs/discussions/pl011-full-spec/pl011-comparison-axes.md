# PL011 Comparison Axes

**Role**: load-bearing artifact for the Regseq PL011 evaluation strategy.

This document defines WHAT we compare when we compare a Regseq-generated
ARM PrimeCell PL011 driver to the corresponding OS-native PL011 driver.
The axes documented here are the sole source of "comparability" for the
paper's central claim. Every requirement in
`pl011-spec-requirements.md`, every workload in `pl011-workloads.md`,
and every row in `pl011-feature-matrix.md` traces back to an axis
defined here (or to correctness/safety or backend-contract rules,
which are tracked separately).

**Derivation discipline**: axes are derived from concrete PL011
system-use cases, NOT from what Regseq happens to be able to do. This
is the anti-gaming rule that defends the claim at peer review. Any
axis added to this document must cite at least one system-use case
from Section 2 that implies it. Any system-use case considered and
dropped from scope must appear in the "dropped" ledger with rationale.

## 1. Thesis and scope

**Thesis** (current Regseq PL011 strategy):

> Regseq's semantic driver model expresses a single ARM PrimeCell
> PL011 specification, and its compiler generates Linux and Zephyr
> drivers that match native-driver behavior within predefined
> tolerance rules on QEMU-relative functional and performance
> workloads derived from concrete PL011 system use cases.

**Scope**:

- Single device: ARM PrimeCell PL011. No vendor variants (ST, SBSA,
  QDF2400) in the claim.
- Two backends in the claim: Linux and Zephyr.
- Corpus (for evidence, policy reference, and comparison baselines,
  not inclusion gating): Linux `amba-pl011.c`, Zephyr
  `drivers/serial/uart_pl011.c`, FreeBSD
  `sys/dev/uart/uart_dev_pl011.c`, Windows 10 IoT Core
  `ms-iot/rpi-iotcore/.../serPL011`. Files cached under
  `reference-drivers/pl011/`.
- Comparisons are QEMU-relative only: Regseq-generated driver versus
  native driver under the same QEMU setup. Absolute-physical claims
  are captured in `pl011-spec-requirements.md` as deferred items and
  are not paper-load-bearing.

## 2. System-use cases for PL011

These are the concrete deployed roles in which a PL011 UART actually
does work in a system. They are the reason the hardware exists. The
axes in Sections 3-5 are derived from what these use cases demand.

| ID  | Use case                            | Summary                                                                                                                                                     |
| --- | ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| U1  | Interactive shell TTY               | A human types at a terminal; the driver mediates echo, line editing, and control characters between a shell and the wire.                                   |
| U2  | System console output               | Kernel or firmware writes diagnostic text to the UART during boot and runtime. The driver must deliver bytes reliably under contention.                     |
| U3  | Inter-device serial link            | Two devices exchange data via the UART (e.g., SBC talking to a micro). Bidirectional byte stream at negotiated baud.                                        |
| U4  | Diagnostic / debug port             | A developer tool (JTAG probe, OEM diagnostics, log capture) attaches to the UART to extract logs or interact with device under test.                        |
| U5  | Firmware update channel             | A bootloader or recovery agent receives a firmware image over UART. Bulk RX at a sustained rate, usually with handshaking and error detection.              |
| U6  | Sensor or peripheral data ingestion | An embedded device receives structured data from an attached peripheral (GPS NMEA at 9600, modem AT-set, 3D printer G-code, industrial sensor).             |
| U7  | Machine-to-machine framed protocol  | A framed protocol (SLIP, PPP, custom binary protocol) runs over the UART stream. Requires byte-exact transport and occasionally framing signals like BREAK. |

Use cases considered and deferred:

- **U-DEF-1** (Kernel printk console registration on Linux): OS-specific
  integration role that spans multiple kernel subsystems. Captured as a
  requirement-document deferred item; not paper-load-bearing for the
  current claim.
- **U-DEF-2** (IrDA / infrared over SIR encoding): not UART-mode, PL011
  supports it in hardware but rarely deployed today. Out of scope per
  the PrimeCell scope pin interpretation.
- **U-DEF-3** (RS-485 half-duplex on a board with an RS-485 transceiver):
  defensible use case, but requires platform-specific direction-control
  hardware and is rare on available evaluation platforms. Deferred.
- **U-DEF-4** (Loopback as a system role): loopback is a diagnostic
  tool rather than a system-use case; used to self-test F1/F2
  workloads but not its own axis.

## 3. Functional axes (derived)

Each axis names a functional property of the driver that can be
independently observed and compared. Each axis is tagged with the
system-use cases that imply it. Failures on a functional axis mean
the driver does not correctly realize a use case.

| ID  | Axis                                  | Implied by             | Primary observable                                                                                                                                                          |
| --- | ------------------------------------- | ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F1  | TX path byte integrity                | U1, U2, U3, U4, U6, U7 | Byte-for-byte match between guest-side write stream and host-side capture.                                                                                                  |
| F2  | RX path byte integrity                | U1, U3, U5, U6, U7     | Byte-for-byte match between host-side injected stream and guest-side read stream.                                                                                           |
| F3  | Baud rate configuration correctness   | U1, U3, U4, U5, U6     | Programmed baud produces the expected wire rate (inferred from throughput matching).                                                                                        |
| F4  | Line format configuration correctness | U1, U6, U7             | Word length, stop bits, parity configurable and honored; frames not mangled.                                                                                                |
| F5  | Line-error reporting                  | U5, U6, U7             | Parity, framing, break, and overrun conditions are detected and surfaced to the OS.                                                                                         |
| F6  | Interrupt-driven service lifecycle    | U1, U2, U3, U5         | IRQ sources enabled, serviced, and cleared without loss under normal load.                                                                                                  |
| F7  | Enable/disable/reconfigure lifecycle  | U1, U3, U4             | Driver can be opened, reconfigured, and closed without data corruption around transitions.                                                                                  |
| F8  | Break generation                      | U7                     | Driver can emit a break condition when asked; peer observes the break.                                                                                                      |
| F9  | Hardware flow control (CRTSCTS)       | U3, U5, U6             | When enabled, auto-RTS and CTS-driven transmit pacing behave as the native driver behaves.                                                                                  |
| F10 | RX throttle / unthrottle              | U1, U5                 | Driver can stop accepting bytes under buffer pressure, then resume, without data loss on the peer side if flow control is active, and with controlled loss rules otherwise. |

Notes:

- F9 and F10 depend on the board having the relevant lines wired and
  the target OS exposing the control. We compare only where both
  native and Regseq drivers can exercise the axis.
- Modem-status inputs (DCD, DSR, CTS, RI) are observable only if
  the evaluation platform exposes them. Tracked as requirements, not
  as a paper-load-bearing axis unless the platform supports it.

## 4. Performance axes (derived, QEMU-relative)

Performance axes compare Regseq-generated vs native driver under
identical QEMU setups. All comparisons are relative. Absolute
wire-rate claims are not paper-load-bearing.

| ID  | Axis                                     | Implied by     | Primary observable                                                                                |
| --- | ---------------------------------------- | -------------- | ------------------------------------------------------------------------------------------------- |
| P1  | Sustained TX throughput                  | U2, U3, U4, U5 | Bytes/second of a sustained TX workload at fixed baud.                                            |
| P2  | Sustained RX throughput                  | U3, U5, U6     | Bytes/second of a sustained RX workload at fixed baud.                                            |
| P3  | RX robustness under CPU stress           | U2, U3, U5     | Byte loss rate of a sustained RX stream while guest runs a CPU-bound background workload.         |
| P4  | Interrupts per KiB (batching efficiency) | U3, U5         | ISR invocation count per KiB transferred under sustained RX or TX.                                |
| P5  | Guest CPU cost per byte                  | U2, U5         | Guest CPU time per byte of a sustained workload (where QEMU / guest perf counters are available). |
| P6  | Small-write latency                      | U1, U7         | p50/p95/p99 wall time from driver write call to peer observation of byte, under light load.       |
| P7  | FIFO threshold responsiveness            | U3, U5         | Observed interrupt cadence correlates with the configured IFLS level.                             |

DMA-bearing performance: P1, P2, and P5 extend to DMA paths once
DMA is internally staged. DMA is not a separate axis; it is a
realization option for P1/P2/P5 inherited from use cases U2, U3,
U4, U5 (their TX/RX throughput and CPU-cost observations carry
over directly to DMA paths). Under DMA:

- The Linux comparison is quantitative: Regseq-Linux-with-DMA vs
  native-Linux-with-DMA.
- The Zephyr comparison is NOT quantitative for DMA. Native Zephyr
  has no DMA path on the generic PL011 driver, so there is no
  like-for-like baseline. DMA on Zephyr is reported as a
  **feature-delta observation only** ("Regseq-generated Zephyr
  exposes a DMA path absent from the native Zephyr PL011 driver"),
  not as a performance-equivalence claim.

## 5. Integration axes (derived)

Integration axes verify that the generated driver participates in
target-OS frameworks the way a native driver does. Integration
failures mean a user-space program expecting a working UART cannot
find or use the driver.

Linux integration axes:

| ID  | Axis                                 | Implied by | Observable                                                                                                                       |
| --- | ------------------------------------ | ---------- | -------------------------------------------------------------------------------------------------------------------------------- |
| IL1 | Out-of-tree module builds and loads  | all        | `.ko` builds against pinned kernel; `insmod` / `modprobe` succeeds.                                                              |
| IL2 | AMBA probe binds the reserved device | U1, U3, U4 | Driver's probe runs on the reserved second PL011 without stealing the console UART.                                              |
| IL3 | Usable tty device                    | U1, U4     | `/dev/ttyAMA<N>` is created; `stty`, `open`, `read`, `write`, `poll`/`select`, and `close` work.                                 |
| IL4 | serial_core integration hooks        | U1, U5     | `uart_ops` callbacks invoked as expected for start_tx / stop_tx / startup / shutdown / set_termios etc.; serial counters update. |
| IL5 | Devicetree / ACPI binding match      | all        | Binding to the intended device via the platform's configuration mechanism.                                                       |

Zephyr integration axes:

| ID  | Axis                                         | Implied by | Observable                                                                                                 |
| --- | -------------------------------------------- | ---------- | ---------------------------------------------------------------------------------------------------------- |
| IZ1 | Builds in canonical app on `qemu_cortex_a53` | all        | Zephyr sample app builds and boots with the generated driver.                                              |
| IZ2 | Devicetree `arm,pl011` binding               | all        | Node matches the generated driver; init sequence runs.                                                     |
| IZ3 | `uart_driver_api` participation              | U1, U3     | `uart_poll_in`, `uart_poll_out`, `uart_configure`, `uart_irq_*` APIs work against the generated driver.    |
| IZ4 | Interrupt-driven API path                    | U1, U3, U5 | `uart_irq_callback_set` + `uart_irq_rx_enable` / `uart_irq_tx_enable` behave against the generated driver. |

Axes intentionally not included in the current claim:

- Linux console / earlycon participation: deferred (U-DEF-1).
  Re-evaluate when the console integration story is in scope.
- Suspend / resume: deferred along with PM (matches existing
  `pl011-full-spec-status.md` status).
- sysfs-driven unbind: administrative, not a driver-behavior axis.

## 6. Tolerance and verdict discipline

Axes here enumerate WHAT is compared. `pl011-workloads.md` defines
HOW each axis is exercised and WITH WHAT TOLERANCE.

Tolerance rules (to be refined in `pl011-workloads.md`):

- Functional axes: pass / fail. No tolerance; the generated driver
  must realize the functional behavior or the row fails.
- Performance axes: tolerance window, two-sided where meaningful.
  Starting rule: generated median within 10-20% of native median for
  throughput and latency; no worse than 1.5x for interrupts/byte.
  Statistical contract: median + p95/p99 with confidence intervals,
  minimum run count to be defined in `pl011-workloads.md`.
- Integration axes: pass / fail.

## 7. Dropped-axes ledger

Axes that were considered and explicitly dropped, with rationale.
This ledger exists so reviewers can see the derivation was
principled rather than selective.

| **Dropped axis**                                       | **Rationale**                                                                                                              |
| ------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| IrDA / SIR throughput                                  | Not UART-mode; deferred along with the SIR encoding scope pin (U-DEF-2).                                                   |
| RS-485 half-duplex                                     | Board-dependent (requires RS-485 transceiver and direction-control hardware); rare on available eval. platforms. Deferred. |
| Wire-rate absolute throughput on physical PL011        | Requires absolute-physical measurement; paper uses QEMU-relative evaluation. Captured as a deferred requirement.           |
| Interrupt latency under kernel panic / oops path       | Linux-specific, timing-critical integration; adjacent to console takeover, which is deferred.                              |
| Full modem signaling (DTR, OUT1, OUT2) as a paper axis | Board-wiring dependent; evaluation platforms may not expose the lines. Modeled in the spec, not claim-load-bearing.        |
| Loopback as an independent axis                        | Diagnostic tool, not a system-use case (U-DEF-4). Used to drive F1/F2 workloads but not reported separately.               |
| Console takeover (kernel printk)                       | OS-specific deep integration; deferred with the Linux console use case (U-DEF-1).                                          |
| Per-chip revision quirks (`AMBA_REV` branches, etc.)   | Out of scope per the PrimeCell scope pin; variant work is deferred.                                                        |
| sysfs / procfs administrative interfaces               | Administrative, not driver-behavior.                                                                                       |

## 8. Review discipline

Additions to this document:

- New use case: add to Section 2 with summary and ID. If it implies
  new axes, add them to Sections 3-5.
- New axis: must cite at least one use case from Section 2. If no
  use case applies, either add the use case first or reject.
- Dropped axis: move to Section 7 with rationale. Do not delete.

Removal of use cases or axes:

- Any removal must be logged in a change history at the bottom of
  this document or in a git commit message. Axes that have been
  reported in a published paper cannot be silently removed.

Change history:

- 2026-04-20: initial version, derived from the strategy round-4
  Codex-reviewed synthesis. Four-OS corpus (Linux, Zephyr, FreeBSD,
  Windows) confirmed by `reference-drivers/pl011/`.
