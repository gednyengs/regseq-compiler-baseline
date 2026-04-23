# PL011 Experiment Methodology -- How We Run the Comparisons in QEMU

**Role**: the operational bridge between the abstract workload
definitions in `pl011-workloads.md` and concrete execution on the
evaluation environment. This document answers, for every workload
row, "what exact QEMU setup and harness procedure runs this?"

**Relationship to existing evaluation docs**:

- `docs/discussions/evaluation-environment/reference-setup.md` --
  established throughput / latency / CPU / interrupt-count
  measurement methodology. This doc cites and uses that
  methodology; it does not duplicate it.
- `docs/discussions/evaluation-environment/current-resolutions-and-strategy.md`
  -- documented the patched QEMU v8.2.2 two-PL011 topology.
- `docs/discussions/evaluation-environment/qemu-relative-evaluation-strategy.md`
  -- established the QEMU-relative-only evaluation gate.
- `docs/discussions/evaluation-environment/zephyr-emulation-target-selection.md`
  -- board choice (`qemu_cortex_a53`).

This doc extends those docs with: per-workload execution
procedures, the QEMU patch set required for F5 / F9, the Zephyr
experimental harness, data archival + reproducibility contract,
and the pilot-run plan that precedes tolerance-window finalization.

## 1. Topology summary

### 1.1 Linux topology (from current-resolutions-and-strategy)

QEMU: patched v8.2.2, aarch64 `virt` machine. Two PL011 UARTs
exposed to the guest:

- `ttyAMA0` at `0x09000000` -- stock kernel console path; never
  driven by the driver under test.
- `ttyAMA1` at `0x09040000` -- reserved for the driver under test
  via device-tree `compatible = "regseq,pl011"` override that
  prevents the stock AMBA driver from binding.

Guest: project-controlled Linux v6.18.21 + Buildroot 2026.02
rootfs. No NIC on the PL011 profile.

Host-side peer: a chardev backend on the host (pipe, socket, or
TCP) connected to the reserved `ttyAMA1` so the test harness on
the host can read TX output and write RX input.

### 1.2 Zephyr topology

QEMU: stock QEMU with `qemu_cortex_a53` Zephyr board. The board
exposes a PL011 that the test application drives. Host-side peer
connects via the board's serial chardev backend.

Zephyr's `qemu_cortex_a53` does not by default expose two
independent PL011s the way the patched Linux setup does; the
Zephyr experimental harness instead runs in a single-PL011 mode
where the test app explicitly relinquishes the console to a
logging sink (RAM logger or semihosting) and uses the PL011 as the
device under test.

## 2. Per-workload execution plan

The table below maps every workload row in `pl011-workloads.md` to
its execution status using a granular taxonomy (replacing the
original binary `today` / `patch-required`):

- `today-byte-stream`: runs today via byte integrity through an
  unmodified QEMU chardev backend.
- `today-register-only`: executes today via register-state
  assertion only (QEMU does not emulate the wire-level behavior
  the workload might appear to claim; see Section 4 about QMP
  register capture as the primary assertion path).
- `today-with-existing-perf-tools`: runs today via `perf`,
  `/proc/interrupts`, debugfs counters, etc.
- `patch-required-error-injection`: needs QEMU PL011 error +
  OE injection patch (Section 3.1).
- `patch-required-flow-control`: needs QEMU PL011 modem-line +
  auto-flow-control patch (Section 3.2).
- `patch-required-ifls-semantics`: needs QEMU PL011 IFLS
  threshold-semantics patch (Section 3.3). QEMU v8.2.2
  hardcodes `read_trigger = 1` and ignores the `IFLS` register.
- `patch-required-qmp-introspection`: needs QEMU PL011
  `query-pl011-state` QMP command (Section 3.4) for independent
  register-state observation.
- `zephyr-harness`: needs the Zephyr experimental harness
  (Section 5) AND a proven auxiliary control channel (Section
  5.1).
- `optional-qemu-wire-fidelity`: would benefit from a
  baud-throttle / framing-enforcement QEMU patch but does not
  require one; the row is claim-bearing at the register-
  programming layer even without wire fidelity.

Multiple statuses may apply to a workload when its observables
span different mechanisms.

### Functional workloads (F)

| Workload | Status | Execution procedure |
|----------|--------|---------------------|
| W-F1-01 (TX integrity, 1 MiB) | today-byte-stream | Host peer reads from `ttyAMA1` chardev; compare to expected. Zephyr: analogous via test app + harness. |
| W-F2-01 (RX integrity, 1 MiB) | today-byte-stream | Host peer writes to `ttyAMA1` chardev; guest reads + compares. |
| W-F3-01 (divisor correctness) | today-register-only + patch-required-qmp-introspection | Programmed `IBRD`/`FBRD` read via QMP `query-pl011-state` (primary) and debugfs/shell (secondary); compare to TRM-derived expected. |
| W-F3-02 (baud throughput correlation) | optional-qemu-wire-fidelity | QEMU v8.2.2 does not throttle TX/RX by baud (`pl011_get_baudrate()` is tracing-only). Without a baud-throttle patch, throughput is identical across bauds in QEMU. DEMOTED to qualitative observation for the paper claim; F3 axis verdict rests on W-F3-01 (register correctness). If a baud-throttle patch is added, this row re-promotes to verdict-bearing. |
| W-F4-01 (line format, 7/8-bit) | today-register-only + patch-required-qmp-introspection + optional-qemu-wire-fidelity | Assert programmed `LCR_H` WLEN/STP2/PEN/EPS bits via QMP. Byte-exact round-trip still runs for completeness but QEMU does not enforce framing; mismatch at the wire layer would be a QEMU fidelity issue, not a driver issue. |
| W-F4-02 (stop bits) | today-register-only + patch-required-qmp-introspection | Same mechanism as W-F4-01; `STP2` assertion is the verdict. |
| W-F4-03 (5/6-bit widths) | today-register-only + patch-required-qmp-introspection | Same; plus `not-applicable` when the OS API rejects the width. |
| W-F5-01 (parity error detect) | patch-required-error-injection | QEMU PL011 must flag the byte + drive `PEIM` IRQ lifecycle per Section 3.1. |
| W-F5-02 (framing error detect) | patch-required-error-injection | Same, for `FEIM`. |
| W-F5-03 (break detect) | patch-required-error-injection | Same, for `BEIM`; duration via Section 3.1's break-assert QMP command. Unmodified QEMU's `CHR_IOCTL_SERIAL_SET_BREAK` does not guarantee break observation on TCP/socket peers. |
| W-F5-04 (overrun detect) | patch-required-error-injection | QEMU v8.2.2's `pl011_can_receive()` gates RX when FIFO is full and NEVER sets `INT_OE`. Natural overrun is not achievable; Section 3.1 must add explicit overrun emulation. |
| W-F6-01 (IRQ-driven RX) | today-byte-stream + today-with-existing-perf-tools | Standard RX workload with polling disabled; IMSC shadow consistency verified via QMP register capture. |
| W-F6-02 (IRQ-driven TX) | today-byte-stream + today-with-existing-perf-tools | Analogous for TX. |
| W-F6-03 (error-IRQ lifecycle) | patch-required-error-injection | All four error-IRQ sources (FE/PE/BE/OE) require the Section 3.1 patch. Overrun portion specifically depends on Section 3.1's explicit overrun emulation. |
| W-F7-01 (reconfigure around data) | today-byte-stream + today-register-only | Guest-side driver API calls bracket a data-bearing interval; register assertion via QMP. |
| W-F8-01 (break generation) | today-byte-stream (conditional on peer backend) | Guest emits break via driver API. Observability depends on peer chardev backend: mux / pipe may surface break; TCP/socket often does not. Report as platform-dependent with explicit chardev-backend call-out. |
| W-F9-01 (CTS-driven TX stall) | patch-required-flow-control | Section 3.2 extension must model `CR.CTSEN` gating, not just CTS state. |
| W-F9-02 (auto-RTS deassertion) | patch-required-flow-control | Section 3.2 must model `CR.RTSEN` + auto-RTS threshold behavior. |
| W-F10-01 (throttle with CRTSCTS) | patch-required-flow-control | Depends on F9 infrastructure. |
| W-F10-02 (throttle without CRTSCTS) | patch-required-error-injection | Relies on overrun reporting per W-F5-04. |

### Performance workloads (P)

| Workload | Status | Execution procedure |
|----------|--------|---------------------|
| W-P1-01 (sustained TX @ 921600) | today-byte-stream | `pv` / custom-harness driven stream; throughput measured via wall-clock on host side (single-clock, see Section 4.2). |
| W-P1-02 (sustained TX @ 115200) | today-byte-stream | Same methodology. |
| W-P2-01 (sustained RX @ 921600) | today-byte-stream | Same methodology. |
| W-P3-01 (RX under CPU stress) | today-byte-stream + today-with-existing-perf-tools | `stress-ng` guest-side background workload during sustained RX. |
| W-P4-01 (interrupts/KiB) | today-with-existing-perf-tools | Guest samples `/proc/interrupts` (Linux) before and after workload; delta divided by KiB. Zephyr: `CONFIG_INTERRUPT_METRICS` or custom probe. |
| W-P5-01 (guest CPU / byte) | today-with-existing-perf-tools (Linux); not-applicable (Zephyr, absent CPU-accounting primitive) | Linux: `perf stat -e cycles,instructions` during workload. |
| W-P6-01 (small-write latency) | today-byte-stream (redefined, see Section 4.2) | Host-only round-trip measurement: host writes byte; host times arrival of echoed byte. No cross-clock synchronization required. |
| W-P7-01 (FIFO threshold responsiveness) | patch-required-ifls-semantics | QEMU v8.2.2 `pl011_set_read_trigger()` forces `read_trigger = 1` and ignores `IFLS`. Without Section 3.3 IFLS patch, interrupt cadence is invariant with `IFLS` setting in QEMU; the row cannot distinguish Regseq from native. |
| W-P1-DMA-01, W-P2-DMA-01, W-P5-DMA-01 | today-byte-stream (once DMA lands) | Stage-3 items per `pl011-spec-requirements.md` Section 1.2; execute after DMA is captured in `driver.rs` (REQ-PERF-DMA-01/02) and both backends emit the DMA path; requires QEMU patch 3.3 for the paired P7 work in the same stage. |

### Integration workloads (I)

| Workload | Status | Execution procedure |
|----------|--------|---------------------|
| W-IL1-01 (module build + load) | today-with-existing-perf-tools | Standard out-of-tree module workflow against pinned kernel source. |
| W-IL2-01 (reserved-device binding) | today-with-existing-perf-tools | Per current-resolutions topology; confirm probe fires only on the `regseq,pl011` device. |
| W-IL3-01..05 (tty operations) | today-byte-stream + today-with-existing-perf-tools | Standard userspace tools (`stty`, `dd`, custom `read/write/poll` harness). |
| W-IL4-01 (serial_core counter updates) | today-with-existing-perf-tools | Read `/proc/tty/driver/ttyAMA` before + after workload; assert delta matches byte count. |
| W-IL5-01 (DT match) | today-with-existing-perf-tools | Inspect `dmesg` + `/sys/bus/amba/devices/`. |
| W-IZ1-01..W-IZ4-02 (Zephyr integration) | zephyr-harness | Per Section 5. |

## 3. QEMU extensions required

The current patched QEMU v8.2.2 gives us the two-PL011 topology
but does not give us error injection, overrun, IFLS semantics,
modem-line control, or an independent register-state read path.
Four patches close these gaps. Each patch is scoped to one
category so it can be developed, reviewed, and merged
independently.

### 3.1 PL011 error + overrun injection with IRQ lifecycle

**Purpose**: inject parity, framing, break, and overrun
conditions with full IRQ lifecycle (`RIS` / `MIS` / `ICR`
semantics) for W-F5-01..04, the error portion of W-F6-03, and
the overrun portion of W-F10-02.

**Why unmodified QEMU fails**: QEMU v8.2.2 `hw/char/pl011.c`
`pl011_can_receive()` gates RX when the FIFO is full and never
sets `INT_OE`; it also never sets `INT_PE`, `INT_FE`, or
`INT_BE` from chardev input. No FIFO entry carries per-byte
error status.

**Design** (~100-150 lines in `hw/char/pl011.c`):

- Extend FIFO entries with per-entry error bits (FE/PE/BE).
  Today a FIFO entry is a 32-bit register read; add the four
  error-status bits so when the guest reads DR/RSR the
  hardware-correct error flags come back.
- Add an `OE`-pending flag maintained independently of the
  FIFO; set when an enqueue happens while the FIFO is full
  (natural overrun) or when explicitly injected. Surface as
  `INT_OE` in `RIS`; cleared via `ICR`.
- Drive IRQ lifecycle: when an enqueue delivers a byte with
  an error flag, set the corresponding `RIS` bit according to
  the PL011 spec (`INT_FE`, `INT_PE`, `INT_BE`, `INT_OE`). The
  masked variant `MIS` follows via `RIS & IMSC`. Clearing the
  error IRQ via `ICR` behaves per the TRM.
- QMP commands (all use the standard selector
  `{qom-path: str | mmio-base: int}` per Section 3.4):
  - `pl011-inject-error`: selector plus `{error-type:
    enum(parity|framing|break), count: int=1}`. Flags the next
    `count` RX bytes and drives the corresponding IRQ on
    delivery.
  - `pl011-inject-overrun`: selector plus `{count: int=1}`.
    Forces the FIFO into overrun `count` times regardless of
    current FIFO state, drives `INT_OE`.
  - `pl011-assert-break`: selector plus `{duration-ms: int}`.
    Drives `BREAK` line and `INT_BE` for the duration;
    transitions cleanly.
- The patch ships with a QEMU unit test under `tests/qtest/`
  that drives each QMP command and asserts the expected RIS /
  MIS / ICR / DR behavior via the qtest bus.

### 3.2 PL011 modem-line + auto-flow-control semantics

**Purpose**: model CTS/RTS with the full PL011 flow-control
contract for W-F9-01, W-F9-02, W-F10-01. Not just line-state
I/O: must model `CR.CTSEN` gating of transmit, `CR.RTSEN`
auto-RTS deassertion at the FIFO-fill threshold, and the
modem-status interrupt deltas.

**Why unmodified QEMU fails**: QEMU PL011 v8.2.2 models
`CR.RTS` as a plain bit but does not model `CR.CTSEN` gating
of transmission, nor auto-RTS behavior, nor `FR.CTS` input
tracking, nor modem-status IRQs.

**Design** (~100-150 lines):

- Add device state: `fr_cts_in` (CTS input), auto-RTS
  threshold (1/2, 3/4, or tracked alongside IFLS).
- When `CR.CTSEN` is set, gate TX: the emulator only forwards
  bytes from the TX FIFO to the chardev when `FR.CTS` is
  asserted. TX FIFO fills normally; `FR.BUSY` reflects the
  gate state.
- When `CR.RTSEN` is set, model auto-RTS deassertion as the
  RX FIFO crosses the threshold: the emulator-driven RTS
  output line (exposed via QMP `query-pl011-state`, Section
  3.4) deasserts; on FIFO drain below threshold, re-asserts.
  A guest-visible modem-status IRQ delta (`INT_CTSM` /
  `INT_RIM` etc. per TRM) fires on each transition.
- Manual RTS (`CR.RTSEN == 0`, `CR.RTS` set/clear) still
  takes effect as line-drive state.
- QMP commands (all use the standard selector
  `{qom-path: str | mmio-base: int}` per Section 3.4):
  - `pl011-set-cts`: selector plus `{asserted: bool}`. Updates
    `fr_cts_in`; drives modem-status IRQ deltas if unmasked.
  - `pl011-get-modem-lines`: selector only. Diagnostic read-out
    of current CTS / RTS / DCD / DSR states for harness use.
  - `pl011-set-autorts-threshold`: integration test hook;
    selector plus threshold parameter.
- Unit test: qtest harness drives CTS transitions and asserts
  TX gating + modem-status IRQ lifecycle.

### 3.3 PL011 IFLS threshold semantics

**Purpose**: make the IFLS register control actual interrupt
thresholds for W-P7-01 and to correctly exercise FIFO-level IRQ
behavior in all RX/TX workloads.

**Why unmodified QEMU fails**: QEMU v8.2.2
`pl011_set_read_trigger()` hard-codes `read_trigger = 1`,
ignoring `IFLS`.

**Design** (~40-60 lines):

- Update `pl011_set_read_trigger()` to read the `IFLS` register
  and map the RX-level field to `1/8, 1/4, 1/2, 3/4, 7/8` of
  FIFO depth per TRM.
- Similarly update the TX trigger (if a TX trigger exists) to
  honor the TX-level field.
- When FIFO level crosses the threshold, drive `INT_RX` /
  `INT_TX` lifecycle.
- Unit test: at each of the five IFLS settings, RX a fixed
  stream and assert the IRQ cadence matches the expected
  crossing points.

### 3.4 PL011 register-state introspection (QMP)

**Purpose**: provide an independent observation path for
programmed register state (IBRD, FBRD, LCR_H, CR, IFLS, IMSC,
RIS, DMACR, and the modem-state bits added by 3.2). Used by
W-F3-01, W-F4-01..03, W-F6-* IMSC-shadow consistency checks,
and W-IZ3-02. Replaces the round-1 "driver self-inspection
hook" as the primary register assertion mechanism; guest
debugfs/shell stays as a secondary cross-check (see Section
4).

**Why not use existing QEMU trace events**: trace events log
writes but do not snapshot current state; a snapshot command is
simpler to integrate into a harness.

**Design** (~30-50 lines):

- QMP command `query-pl011-state`: argument is one of:
  - `qom-path: str` (e.g., `/machine/peripheral-anon/device[1]`),
    OR
  - `mmio-base: int` (e.g., `0x09040000` for the reserved
    `ttyAMA1` on the Linux aarch64 `virt` topology).
  The harness records the chosen selector in the per-run
  metadata (Section 7.1). Returns a struct with current register
  values (IBRD, FBRD, LCR_H, CR, IFLS, IMSC, RIS, DMACR,
  FR-including-modem-state, programmed-DMA-channel-info where
  3.2 / DMA work adds it).
  Note: the other QMP commands defined in Sections 3.1 and 3.2
  (`pl011-inject-error`, `pl011-inject-overrun`,
  `pl011-assert-break`, `pl011-set-cts`, `pl011-get-modem-lines`,
  `pl011-set-autorts-threshold`) use the same
  `{qom-path | mmio-base}` selector shape.
- Side-effect-free; pure read of device state.
- Unit test: drive the guest to program specific register
  values and assert the QMP snapshot matches.

### 3.5 Patch set catalog (summary)

| Patch | Scope | Unlocks |
|-------|-------|---------|
| 3.1 error + OE injection + IRQ lifecycle | ~100-150 LoC | W-F5-01..04, W-F6-03, W-F10-02 |
| 3.2 modem-line + auto-flow-control | ~100-150 LoC | W-F9-01, W-F9-02, W-F10-01 |
| 3.3 IFLS threshold semantics | ~40-60 LoC | W-P7-01, improves all RX/TX IRQ-cadence fidelity |
| 3.4 QMP register introspection | ~30-50 LoC | primary assertion path for W-F3-01, W-F4-*, W-F6-* shadow checks, W-IZ3-02 |

**Optional follow-on (not in current scope)**: baud-throttle +
framing-enforcement would re-promote W-F3-02 and raise
W-F4-*'s byte-stream round-trip from "for completeness" to
verdict-bearing. Scope is larger (~200+ LoC) and is not
required for the current paper claim because the F3/F4 axis
verdicts rest on register-state correctness (Patch 3.4). Flag
as future work.

### 3.6 Patch hygiene

All four patches live as git patches in
`experiments/pl011-eval-env/qemu-patches/`. Each patch
includes:

- A commit message documenting the feature, the workloads it
  unlocks, and links to this document.
- A build receipt (version pin, build flags).
- Unit-test harness output archived alongside the patch.

QEMU version pin for evaluation runs: the patched v8.2.2
baseline currently documented in current-resolutions, plus
patches 3.1, 3.2, 3.3, and 3.4. The repo records the exact
QEMU binary SHA-256 used for every evaluation campaign.

## 4. Measurement mechanisms (reuse of reference-setup methodology)

Per-workload measurements reuse the mechanisms validated in
`reference-setup.md`. The table below names the mechanism per
observable type so the workload rows can cite a single source of
truth.

All timing measurements use the single-clock host-only protocol
defined in Section 4.2. No cross-clock guest-host synchronization
is performed.

| Observable | Mechanism | Source |
|------------|-----------|--------|
| TX throughput (bytes/sec) | Host timestamps first and last byte arriving on the chardev endpoint; rate = payload-size / wall-clock-delta. Host clock only. | Section 4.2; reference-setup.md throughput section. |
| RX throughput (bytes/sec) | Host timestamps first-byte-sent and receives a completion acknowledgment over the auxiliary control channel after the guest has drained the expected payload count; rate = payload-size / wall-clock-delta. See Section 4.2 "RX completion bracket". | Section 4.2. |
| Small-write latency | Host round-trip: host writes byte, host times echoed-byte arrival. No guest timestamps. Bootstrap 95% CI on p50/p95/p99. | Section 4.2 W-P6-01 redefinition. |
| Interrupts per KiB | Linux: guest samples `/proc/interrupts` at workload boundaries (not per-byte). Zephyr: `CONFIG_INTERRUPT_METRICS` or driver-hook counter sampled at workload boundaries. No cross-clock align required. | reference-setup.md interrupts section. |
| CPU cycles / byte | Linux: `perf stat -e cycles,instructions` accumulates guest-side CPU; host times payload duration; ratio computed per side independently. Zephyr: not-applicable. | reference-setup.md CPU section. |
| Register-state assertions (IBRD/FBRD, LCR_H, CR, IFLS, IMSC, RIS, FR-including-modem) | PRIMARY: QMP `query-pl011-state` (Patch 3.4). SECONDARY: guest-side debugfs / shell readback (see 4.1). Primary is preferred because it observes device state independent of the driver under test and does not rely on driver-shadow correctness. | Patch 3.4 + Section 4.1. |
| Byte-exact integrity | Host peer captures full stream; compares to expected. Zero-loss-required verdicts fail on any mismatch. | Standard. |

### 4.1 Register state: QMP primary, driver readback secondary

Register-state assertions use a two-tier mechanism:

- **Primary (authoritative)**: QMP `query-pl011-state` (Patch
  3.4). Reads device state from the emulator itself, independent
  of the driver under test. Not subject to driver-shadow bugs;
  not subject to MMIO read-side-effect pitfalls.
- **Secondary (cross-check)**: guest-side readback of the driver's
  programmed-register view:
  - Linux: debugfs file `regseq_pl011/registers` (read-only).
  - Zephyr: shell command `regseq_pl011 regs`.
  Disagreement between primary and secondary is a driver bug
  (the driver's shadow differs from the actual device state).

Why QMP is primary, not driver readback: driver readback can
accidentally validate the driver against its own internal
assumptions. If the Regseq-generated driver has a bug in which
its local `uap->im` shadow disagrees with hardware `IMSC`, a
debugfs dump of the shadow would not expose it. QMP does.

Why QMP over a GDB stub: GDB pauses the VM to read memory; some
PL011 MMIO reads are side-effecting (e.g., reading `DR` consumes
a FIFO entry). A QMP snapshot is side-effect-free because it
reads the emulator's internal state directly.

### 4.2 Timing measurement: single-clock host-only protocol

All paper-load-bearing timing measurements (throughput, latency,
interrupts/KiB) use a single-clock host-side protocol. No
cross-clock synchronization between guest and host is performed
or relied on. This design decision:

- **TX throughput (W-P1-01, W-P1-02)**: host timestamps first
  and last byte of the payload on the chardev endpoint; rate is
  bytes per wall-clock second. The guest does not contribute
  timestamps. Measurement is independent of guest clock
  semantics (TCG virtual clock, KVM paravirt-clock, etc.).

- **RX throughput (W-P2-01, W-P3-01) -- RX completion bracket**:
  RX timing with only host write-side timestamps conflates
  chardev/QEMU buffering with guest receive completion and is
  not a valid throughput measurement. Instead, the harness uses
  a two-event bracket both timed on the host clock:
  1. **Start event**: host begins writing the payload to the
     chardev. Host records t0.
  2. **End event**: guest completes draining the expected
     payload count into the driver's RX buffer, then emits a
     completion marker over the AUXILIARY CONTROL CHANNEL
     (shell or semihosting; NEVER over the PL011 under test).
     Host reads this marker from the control channel and
     records t1.
  3. Rate = payload-size / (t1 - t0).
  
  The control-channel marker latency is a common-mode term
  across Regseq and native runs (both use the same harness),
  so the relative comparison remains valid. Absolute RX
  throughput includes the marker-round-trip as a small
  constant, documented in the per-run metadata.

- **Small-write latency (W-P6-01)**: REDEFINED as host round-trip.
  Host writes one byte; guest driver receives and echoes; host
  reads the echoed byte; host records wall-clock delta. The
  round-trip time includes both guest RX processing and guest TX
  processing, bracketed by two host timestamps. This measures a
  strictly larger quantity than "write-to-wire" latency, but it
  is measurable on one clock. Relative comparison (Regseq vs
  native) remains valid because both sides incur the same
  round-trip overhead.

  Why not guest-to-host cross-clock: QEMU's guest clock under
  TCG is not synchronously stepped with the host; aligning
  them requires either a KVM paravirt-clock export or a
  harness-side calibration protocol, both of which add
  significant complexity and confound interpretation. The
  round-trip redefinition is cleaner.

- **Interrupts/KiB (W-P4-01)**: guest samples
  `/proc/interrupts` at workload boundaries only, not at
  per-byte granularity; no clock alignment required.

- **CPU/byte (W-P5-01)**: `perf stat` accumulates guest CPU
  time; host times payload duration; ratio is a rate
  computable on each side independently.

Documentation requirement: every latency result published in
the comparison matrix cites this round-trip definition
explicitly.

## 5. Zephyr experimental harness

### 5.1 Control-channel proof (prerequisite)

The Zephyr harness requires a control channel DISTINCT from the
PL011 under test. Options:

- **Semihosting** (`CONFIG_SEMIHOST`): app routes shell + logs
  through QEMU semihosting interface.
- **Auxiliary emulated serial**: add a second emulated serial
  (e.g., a secondary PL011 explicitly mapped for the shell, or a
  platform-specific virtual console).
- **qemu_cortex_a53 with a modified DT** that adds a second
  virtual UART for the shell.

None of these has been validated for this project. Before any
Zephyr workload execution, a proof-of-viability run must
demonstrate:

1. Shell commands arrive on the control channel.
2. The PL011 under test is driven exclusively by the Regseq-
   generated driver, with no shell-traffic contamination.
3. Semihosting or aux channel does not introduce synchronization
   stalls that confound performance measurement.

This proof is a gate. If none of the options above pans out, the
Zephyr half of the campaign falls back to functional-only
(F + I axes) with performance rows marked `not-applicable`.

### 5.2 App shape (assuming control channel proven)

A minimal Zephyr sample app under `experiments/zephyr-pl011-eval/`:

- Includes: the generated PL011 driver (the driver under test).
- Relinquishes console output to the proven control channel,
  freeing the PL011 for the DUT role.
- Exposes a shell command set:
  - `regseq_pl011 tx <size>` -- emit `<size>` bytes pattern.
  - `regseq_pl011 rx <size>` -- receive `<size>` bytes; validate.
  - `regseq_pl011 config <baud> <framing>` -- set up.
  - `regseq_pl011 latency <count> <interval>` -- latency workload.
  - `regseq_pl011 regs` -- secondary register readback (QMP
    Patch 3.4 is primary, see Section 4.1).
- The app's shell is served on the proven control channel,
  never on the PL011 under test.

### 5.3 Host-side harness

Same harness binary as the Linux side, with a Zephyr-specific
backend:

- Connects to the PL011-under-test's QEMU chardev.
- Connects to the Zephyr shell's chardev for stimulus commands.
- Drives commands + captures data + records metrics.

### 5.4 Coverage across workloads

Once the control-channel proof (5.1) passes, F/P workloads run
on Zephyr with the same per-workload mechanisms used for Linux;
only the shell driver for command dispatch differs. Integration
workloads IZ1..IZ4 run as currently specified. If the
control-channel proof fails, performance workloads fall to
`not-applicable` on Zephyr; the campaign proceeds with
functional + integration only on that backend.

### 5.5 Topology fairness (native-vs-Regseq binding switch)

On Linux, the current two-PL011 topology reserves `ttyAMA1` for
Regseq via `compatible = "regseq,pl011"` and an overridden
`arm,primecell-periphid`. For the native-driver pilot run
(Section 6) and for native sentinel checkpoints in the real
campaign (Section 6.3), `ttyAMA1` must bind to the STOCK AMBA
PL011 driver under an IDENTICAL topology — same MMIO address,
same IRQ line, same chardev backend.

**Fairness rule**: the QEMU command line used for a native run
is identical to the Regseq run EXCEPT for the `-dtb` argument,
and the two DTBs must be binding-compatible (same MMIO
topology, same IRQs, same chardev wiring; the ONLY permitted
difference is the `compatible` string on `ttyAMA1` and the
`arm,primecell-periphid` override that reserves it for
Regseq).

Mechanism: maintain two DTBs:

- `pl011-regseq.dtb`: `ttyAMA1` binds to `regseq,pl011`.
- `pl011-native.dtb`: `ttyAMA1` binds to `arm,pl011` (stock).

Both DTBs produce the same MMIO topology and expose the same
chardev peer. The harness selects the DTB per run based on the
driver under test. Both DTB SHA-256 hashes are recorded in the
per-run metadata (Section 7.1) so fairness is verifiable. This
isolates driver behavior from topology confounds.

On Zephyr: analogous DT overlay selection; implementation
deferred to the Zephyr runbook (5.6).

### 5.6 Runbook pointer

A Zephyr-specific runbook analogous to
`qemu-relative-evaluation-runbook.md` is pending. Placeholder:
`docs/discussions/evaluation-environment/zephyr-pl011-runbook.md`.
Creating that runbook is on the critical path before the Zephyr
half of the evaluation campaign can start.

## 6. Pilot run (precedes tolerance-window finalization)

Per `pl011-workloads.md` Section 6 "Tolerance constants provenance",
the current tolerance constants are placeholders. The pilot run
fills that gap.

### 6.1 Pilot scope

Pilot does NOT evaluate Regseq. It runs the full Section 4 and
Section 5 workload suite against the NATIVE drivers only, with the
full statistical contract, on the evaluation topology. Output:

- Per workload, the native-side median, p95, p99, bootstrap CI,
  variance, and minimum viable run count.
- A tolerance-constants proposal derived from the observed
  native-side variance + a justified headroom factor.

### 6.2 Pilot deliverable

A pilot report artifact under
`docs/discussions/pl011-full-spec/pilot-report.md` that captures:

- Raw data pointers.
- Tolerance-constant proposals per workload, with rationale.
- High-variance rows flagged for methodology repair before the
  real campaign.

### 6.3 Pilot gating + real-campaign checkpoints

**Pre-campaign gate**: the real Regseq-vs-native campaign does
not start until the pilot report is reviewed and the tolerance
constants in `pl011-workloads.md` are updated from "PROVISIONAL"
to "pilot-derived".

**Per-batch checkpoints during the real campaign**: a one-time
pilot is not sufficient. Reviewers will distrust results if the
real campaign runs later under different host load. Therefore:

1. **Interleave native and Regseq runs within the same batch**
   rather than running all native then all Regseq. This defends
   against host-state drift biasing one side.
2. **Native sentinel workloads per batch**: at the start and
   end of every batch, re-run a small set of pilot sentinels
   on the native driver (at minimum W-P1-01 and W-P6-01).
   Compare sentinel batch-median to pilot-median:
   - Within pilot CI: proceed.
   - Outside pilot CI by <= 2x: flag, continue but annotate
     the batch as drift-suspect.
   - Outside pilot CI by > 2x: stop the batch; re-calibrate
     host environment; re-run sentinel before resuming.
3. **Per-row variance watch**: if a row's batch CI exceeds
   pilot CI by 1.5x, apply the high-variance escalation ladder
   (workloads.md Section 6).
4. **Host-metadata drift detection**: before each batch record
   host CPU governor, turbo state, thermal, load-average, and
   CPU-frequency sampling. Drift flags the batch.
5. **Stop-gate**: any row that fails its verdict AND is
   tagged claim-load-bearing triggers campaign pause, root
   cause investigation, and scope decision (fix vs demote vs
   shrink claim per workloads.md Section 6 escalation).

## 7. Data management and reproducibility

### 7.1 Per-run capture (reproducibility contract)

Every workload run records the following metadata. Missing any
of these invalidates the run for paper-facing reporting.

**QEMU environment**:
- QEMU binary SHA-256.
- Patch-set commit hashes (the four patches from Section 3).
- Full QEMU command line (machine, memory, CPU, chardev
  backends, DTB path, kernel/initrd paths, all flags).
- Acceleration mode (`-accel tcg` / `-accel kvm` / thread-tcg).
- `-smp` config (cpus, cores, threads).
- `-m` memory size.
- `-cpu` model (e.g., `cortex-a57`, `max`).
- `-trace` settings (enabled trace events, if any).
- QEMU thread pin map (vCPU threads pinned to host CPUs).

**Guest environment**:
- Kernel (Linux) / Zephyr build SHA + full config (config file
  hashed).
- DTB hash (selected per Section 5.5 for Linux;
  per overlay for Zephyr).
- Rootfs image SHA-256.
- Driver-under-test commit SHA.
- Guest kernel command-line (console, `earlycon`, `loglevel`,
  isolcpus if any).

**Host environment**:
- Host kernel version + CPU model + microcode.
- CPU governor (`performance` required).
- Turbo / frequency scaling (`intel_pstate=disable` or
  equivalent AMD).
- NUMA topology + placement decisions (QEMU / harness pinned to
  specific NUMA node, isolated from general host workloads).
- IRQ affinity rules (harness CPU isolated from device IRQs).
- Host load policy (no other interactive workloads during
  campaign windows; `stress-ng` etc. only as explicit guest-side
  workload stimuli).
- Thermal state at batch start (CPU package temperature).

**Workload-specific**:
- Parameters (baud, payload size, seeds, framing, IFLS).
- Raw data stream (per-byte timestamps where applicable; host
  clock only per Section 4.2).
- Derived summary (median, p95/p99, CI).
- QMP `query-pl011-state` captures before, during, after.
- PL011 device selector used for QMP calls (either the QOM path
  or the MMIO-base value; recorded verbatim so a later run can
  reproduce the QMP session deterministically).

All metadata fields go into a single JSON record per run,
versioned alongside the raw data.

### 7.2 Archive location

Per-campaign archive under `experiments/campaigns/<date>-<label>/`
containing one subdirectory per workload with the fields above.
Summary CSV per campaign, roll-up into
`pl011-comparison-matrix.md`.

### 7.3 Seed and repeatability

- All pseudo-random payloads use a named, documented seed.
- Every run is re-executable with the same seed + same QEMU
  binary + same guest image. If re-execution yields a different
  result, that is a variance event subject to the
  high-variance policy.

## 8. Execution sequencing

The campaign runs in two parallelizable tracks (non-DMA core and
DMA addendum). DMA has no prerequisite relationship to F/I axes
or non-DMA P axes; serializing everything behind DMA would block
most of the work unnecessarily.

### 8.1 Non-DMA core campaign

1. **QEMU patches** (Sections 3.1, 3.2, 3.3, 3.4): develop and
   validate with qtest-level unit tests. Four independent
   patches; can be parallelized across workstreams if available.
2. **Zephyr control-channel proof** (Section 5.1): demonstrates
   one of the control-channel options works. Critical gate for
   Zephyr workloads.
3. **Topology-fairness DTBs** (Section 5.5): produce
   `pl011-regseq.dtb` and `pl011-native.dtb` with verified
   topology parity.
4. **Pilot run** (Section 6.1-6.2): native drivers only, full
   non-DMA suite, finalize tolerance constants.
5. **Pre-campaign gate** (Section 6.3): tolerance constants
   promoted from PROVISIONAL to pilot-derived.
6. **Real non-DMA campaign**: Regseq vs native, interleaved
   batches with per-batch checkpoints. Fill the non-DMA rows of
   `pl011-comparison-matrix.md`.
7. **Report non-DMA results**.

Steps 1, 2, and 3 can progress in parallel. Step 4 gates step 5;
step 5 gates step 6.

### 8.2 DMA addendum campaign

1. **DMA internal stage** in `driver.rs` (`pl011-full-spec-status.md`
   pending item): add DMA to the Regseq spec + both backends emit
   DMA paths.
2. **Pilot run** (DMA-specific): native drivers on DMA, finalize
   DMA tolerance constants.
3. **Real DMA campaign**: Regseq-vs-native-with-DMA. Linux is
   quantitative; Zephyr is a feature-delta observation
   (`pl011-comparison-axes.md` Section 4).
4. **Report DMA addendum results**, joined into the main
   comparison matrix.

The DMA addendum can start as soon as step 1 (DMA in driver.rs)
completes; it does not require the non-DMA core campaign to be
complete. The final paper integrates both.

## 9. What this document is NOT

- Not a description of individual workloads. Those live in
  `pl011-workloads.md`.
- Not a description of what we measure conceptually. That lives
  in `pl011-comparison-axes.md`.
- Not a replacement for the existing evaluation-environment docs
  (`reference-setup.md`, `qemu-relative-evaluation-strategy.md`,
  etc.); it cites them.
- Not the Zephyr runbook. That is a pending separate artifact.

## 10. Change history

- 2026-04-20: initial version, created to close the "how do we
  run comparisons in QEMU" gap identified during the strategy
  artifact package review.
