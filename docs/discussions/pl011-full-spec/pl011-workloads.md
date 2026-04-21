# PL011 Workloads and Tolerance Rules

**Role**: realization layer for the comparison axes defined in
`pl011-comparison-axes.md`. Each axis is exercised by one or more
concrete workloads. Each workload has a fixed shape (stimulus,
observable, environment, verdict rule) and the statistical contract
under which results are reported.

This document is the input to `pl011-comparison-matrix.md`. Every
matrix row is an instance of a workload defined here.

## 1. Workload shape discipline

Every workload in this document follows the same shape:

- **Stimulus**: what is done to drive the axis (one well-defined
  input sequence or condition).
- **Observable**: what is measured (one well-defined output metric
  or pass/fail condition).
- **Environment**: QEMU configuration + guest OS image + driver
  binding topology; held identical across native and Regseq runs.
- **Verdict rule**: how pass / fail is decided, including tolerance
  window for performance workloads.
- **Statistical contract**: how many runs, what summary statistics,
  what confidence intervals.

Row-granularity rule: one workload instance = one stimulus + one
observable + one environment + one verdict rule. A workload that
combines two stimuli or two observables is split into two rows.

## 2. Shared environment skeleton

Unless overridden, all workloads use this environment:

- **QEMU machine**: `virt` on aarch64 (Linux) or `qemu_cortex_a53`
  (Zephyr), per the existing evaluation-environment choices.
- **Driver binding**: reserved secondary PL011 (the primary PL011
  remains the boot / console UART and is not taken over by the
  driver under test).
- **Backpressure**: host side reads the guest's TX and writes the
  guest's RX at line rate unless the workload explicitly models
  backpressure.
- **Repetition**: minimum 5 runs per workload for functional and 10
  for performance; results reported as median + p95/p99 with
  bootstrap 95% CI where applicable.
- **Reporting**: every matrix cell records run count, median,
  p95/p99, CI half-width, and the native baseline value used for
  tolerance check.

## 3. Functional workloads

Each functional workload exercises one axis from Section 3 of
`pl011-comparison-axes.md`.

### W-F1-01: TX payload integrity

- **Axis**: F1 (TX path byte integrity).
- **Stimulus**: guest driver writes a deterministic 1 MiB
  pseudo-random payload (fixed seed) at 115200 8N1.
- **Observable**: host-side captured stream; byte-for-byte
  comparison against the expected payload.
- **Verdict rule**: PASS iff exact match, FAIL otherwise. No
  tolerance.
- **Statistical contract**: 5 runs; all must pass.

### W-F2-01: RX payload integrity

- **Axis**: F2 (RX path byte integrity).
- **Stimulus**: host sends the same 1 MiB pseudo-random payload to
  the guest at 115200 8N1.
- **Observable**: guest driver's received byte stream; byte-for-byte
  comparison against expected.
- **Verdict rule**: PASS iff exact match, FAIL otherwise.
- **Statistical contract**: 5 runs; all must pass.

### W-F3-01: Baud-rate divisor correctness

- **Axis**: F3 (baud correctness).
- **Stimulus**: at each of {9600, 38400, 115200, 921600}, configure
  the driver to that baud with the PL011's declared input clock.
- **Observable**: programmed `IBRD` and `FBRD` register values
  match the TRM-derived expected integer and fractional divisor
  for that baud + input clock.
- **Verdict rule**: PASS iff (IBRD, FBRD) match expected for every
  baud in the matrix, with no tolerance (divisor is an integer
  register computation).
- **Statistical contract**: 1 run per baud; all must match.

### W-F3-02: Baud throughput correlation (QEMU-relative)

- **Axis**: F3 (baud correctness, throughput side).
- **Stimulus**: at each of {9600, 115200, 921600}, sustained TX
  of a fixed payload for 5 s.
- **Observable**: bytes/second rate observed on the host side.
- **Verdict rule**: rate ordering strictly increases with baud
  (9600 < 115200 < 921600); ratio of Regseq observed rate to
  native observed rate within 15% per baud.
- **Statistical contract**: 3 runs per baud; median reported.
  Note: this is QEMU-relative; we do not claim absolute wire-rate
  correctness.

### W-F4-01: Line format matrix (7- and 8-bit word lengths)

- **Axis**: F4 (line format correctness for 7- and 8-bit widths).
- **Stimulus**: at 115200 baud, for each framing in
  `{8N1, 8N2, 8E1, 8O1, 7E1, 7O1}`, run a 10 KiB round-trip
  integrity test. 5-bit and 6-bit word lengths are owned by
  W-F4-03 below and are not part of this row.
- **Observable**: byte-exact match per framing, AND programmed
  `LCR_H` WLEN/STP2/PEN/EPS bits match the expected encoding for
  that framing.
- **Verdict rule**: PASS iff every framing in this row's fixed
  set matches on both byte stream and LCR_H bits.
- **Statistical contract**: 3 runs per framing; all must pass.

### W-F4-02: Stop-bit-specific integrity

- **Axis**: F4 (specifically stop-bit correctness).
- **Stimulus**: at 115200 8N1 and 8N2, run a 10 KiB round-trip
  with a peer configured to expect the same number of stop bits.
- **Observable**: byte-exact match AND `LCR_H.STP2` bit matches
  the configuration.
- **Verdict rule**: PASS iff stream integrity and register state
  match for both configurations.
- **Statistical contract**: 3 runs per configuration.

### W-F4-03: Narrow word-length word lengths (5-bit and 6-bit)

- **Axis**: F4 (specifically word lengths below CS7).
- **Stimulus**: at 115200, for each of `{5N1, 6N1}`, configure
  the driver via the target OS's width-configuration API and run
  a 1 KiB round-trip with a peer configured identically.
  - Linux: `cflag |= CS5` or `CS6`, round-trip via tty.
  - Zephyr: `uart_config.data_bits = UART_CFG_DATA_BITS_5`
    or `UART_CFG_DATA_BITS_6`, round-trip via `uart_poll_*`.
- **Observable**: for each width: (a) programmed `LCR_H.WLEN`
  matches the TRM-expected encoding (`00` for 5-bit, `01` for
  6-bit); (b) round-trip stream matches byte-exact, masked to
  the relevant low-N bits (the upper unused bits are driver-
  defined and not compared).
- **Verdict rule**: PASS iff both (a) and (b) hold for each width.
  If the target-OS API does not expose the width configuration
  (e.g., a Zephyr port whose subsystem rejects < 7-bit), the
  width is `not-applicable` and excluded from scoring with
  annotation.
- **Statistical contract**: 3 runs per width.

### W-F5-01: Parity-error detection

- **Axis**: F5 (line-error reporting — parity).
- **Stimulus**: peer sends bytes with deliberate parity-bit
  inversion. Injection mechanism: QEMU chardev-mux `-chardev`
  with `parity=yes` or direct PL011 RSR emulation path; if
  unavailable on the active QEMU build, row is
  `not-applicable`.
- **Observable**: driver reports parity error via Linux
  `icount.parity` counter or Zephyr `UART_ERROR_PARITY` callback
  for each injected erroneous byte.
- **Verdict rule**: PASS iff every injected parity error is
  reported; FAIL on silent drop or mis-classification.
- **Statistical contract**: 10 injections per run; 3 runs.

### W-F5-02: Framing-error detection

- **Axis**: F5 (line-error reporting — framing).
- **Stimulus**: peer sends bytes with deliberate stop-bit violation.
  Injection mechanism: QEMU chardev backend supporting framing-error
  injection, or direct PL011 RSR emulation; else `not-applicable`.
- **Observable**: driver reports framing error via
  `icount.frame` (Linux) or `UART_ERROR_FRAMING` (Zephyr).
- **Verdict rule**: same PASS rule as W-F5-01.
- **Statistical contract**: same.

### W-F5-03: Break detection

- **Axis**: F5 (line-error reporting — break).
- **Stimulus**: peer asserts a break condition for a fixed
  duration (at least 2 frame times). Injection mechanism: peer
  UART break-send API, or QEMU chardev backend `send_break`.
- **Observable**: driver reports break via `icount.brk` (Linux)
  or `UART_ERROR_BREAK` (Zephyr).
- **Verdict rule**: PASS iff break is detected each time; FAIL
  on miss or misclassification.
- **Statistical contract**: 5 break events per run; 3 runs.

### W-F5-04: Overrun detection

- **Axis**: F5 (line-error reporting — overrun).
- **Stimulus**: peer sends bytes at sustained rate while the
  driver's RX path is blocked (IRQ masked or deliberately
  delayed) for long enough to overrun the PL011 RX FIFO + DR.
- **Observable**: driver reports overrun via `icount.overrun`
  (Linux) or `UART_ERROR_OVERRUN` (Zephyr).
- **Verdict rule**: PASS iff at least one overrun is reported
  for each induced condition.
- **Statistical contract**: 3 induced conditions per run; 3 runs.

### W-F6-01: Interrupt-driven RX service

- **Axis**: F6 (IRQ service lifecycle — RX path).
- **Stimulus**: host streams a 1 MiB payload at 115200 while the
  driver operates in interrupt-driven mode only (no polling).
- **Observable**: byte-exact match; no dropped/reordered bytes;
  no stuck-IRQ condition after workload end (IMSC shadow reflects
  hardware IMSC).
- **Verdict rule**: PASS iff byte-exact and no residual IRQ state.
- **Statistical contract**: 5 runs; all must pass.

### W-F6-02: Interrupt-driven TX service

- **Axis**: F6 (IRQ service lifecycle — TX path).
- **Stimulus**: driver transmits a 1 MiB payload at 115200 while
  operating in interrupt-driven mode (TXIM-based FIFO refill,
  no polling refill).
- **Observable**: host receives all 1 MiB byte-exact; TXIM is
  asserted and cleared according to the driver's documented TX
  IRQ lifecycle; no stuck TXIM at workload end.
- **Verdict rule**: PASS iff byte-exact on host side AND TXIM
  lifecycle is clean (no leaked asserted TXIM post-workload).
- **Statistical contract**: 5 runs; all must pass.

### W-F6-03: Error-IRQ servicing lifecycle

- **Axis**: F6 (IRQ service lifecycle — error sources).
- **Stimulus**: peer or harness induces one parity, one framing,
  one break, and one overrun event during a low-rate RX stream
  (reuses mechanisms from W-F5-01/02/03/04).
- **Observable**: each error IRQ source (FEIM/PEIM/BEIM/OEIM)
  fires for its corresponding induced event; ICR is W1C-cleared
  appropriately for each; no IRQ storm; IMSC shadow remains
  consistent with hardware IMSC after each clear.
- **Verdict rule**: PASS iff every expected IRQ fires, is cleanly
  cleared, and the IMSC shadow consistency is preserved.
- **Platform scoping**: overrun portion requires the injection
  mechanism from W-F5-04 (masked-IRQ-induced overflow); parity
  and framing portions require the injection mechanisms from
  W-F5-01/02. If any required injection mechanism is
  `not-applicable`, that portion is scored separately and the
  overall row is scored on the portions that ran.
- **Statistical contract**: 3 runs.

### W-F7-01: Reconfigure-around-data

- **Axis**: F7 (enable/disable/reconfigure lifecycle).
- **Stimulus**: driver transmits 10 KiB; pauses; changes baud from
  115200 to 921600; transmits another 10 KiB.
- **Observable**: first chunk matches at 115200 on the host side;
  second chunk matches at 921600; no corruption across the
  transition.
- **Verdict rule**: PASS iff both chunks byte-match at their
  respective baud rates and no spurious bytes appear in the
  transition window.
- **Statistical contract**: 5 runs; all must pass.

### W-F8-01: Break generation

- **Axis**: F8 (break generation).
- **Stimulus**: driver issues a break via its configured break
  API for 100 ms (at least 2 frame times at 115200), preceded
  and followed by 10 valid bytes.
- **Observable**: peer detects a break condition of measured
  duration; peer receives the 10 bytes before and 10 bytes after
  the break, byte-exact.
- **Verdict rule**: PASS iff peer break-duration measurement is
  within 20% of 100 ms AND the surrounding 20 bytes match
  byte-exact. The 20% tolerance accounts for QEMU timing and
  peer-side break-detection resolution.
- **Statistical contract**: 5 runs; all must pass.

### W-F9-01: CRTSCTS flow control (CTS-driven TX stall)

- **Axis**: F9 (hardware flow control).
- **Stimulus**: with CRTSCTS enabled in the driver and the PL011
  CTS line physically/virtually driven by a test harness (QEMU
  PL011 modem-line injection where supported, otherwise physical
  harness), deassert CTS mid-TX for 500 ms while the driver is
  actively transmitting a 1 MiB stream; then re-assert CTS.
- **Observable**: (a) `CR.CTSEN` is set in the driver-programmed
  register state; (b) TX bytes stop appearing on the wire while
  CTS is deasserted; (c) transmission resumes byte-exact after
  CTS is re-asserted; (d) no bytes lost across the stall.
- **Verdict rule**: PASS iff (a), (b), (c), and (d) all hold.
- **Platform scoping**: scored only on platforms that expose CTS
  control either via QEMU PL011 modem-line extension or via
  physical-hardware harness. If neither available,
  `not-applicable`.
- **Statistical contract**: 5 runs; all must pass.

### W-F9-02: CRTSCTS auto-RTS deassertion under RX pressure

- **Axis**: F9 (hardware flow control, RTS side).
- **Stimulus**: with CRTSCTS enabled and the driver's RX path
  receiving a sustained stream, induce a back-pressure condition
  (e.g., stop reading from the upstream tty/uart_driver_api)
  until the PL011 RX FIFO crosses its auto-RTS threshold.
- **Observable**: (a) `CR.RTSEN` is set in the driver-programmed
  register state; (b) the RTS line, as observed by the test
  harness peer, transitions from asserted to deasserted when the
  threshold is crossed; (c) the peer, which honors RTS, stops
  sending; (d) no RX bytes are dropped between threshold-cross
  and peer-stop.
- **Verdict rule**: PASS iff (a), (b), (c), and (d) all hold.
- **Platform scoping**: requires observing the RTS line state
  from the peer side — DIFFERENT from W-F9-01, which requires
  driving CTS. Options: (1) QEMU PL011 modem-line extension that
  surfaces RTS state to a chardev; (2) physical-hardware harness
  with a logic analyzer or looped-back peer UART that observes
  RTS. If neither available, row is `not-applicable`.
- **Statistical contract**: 5 runs; all must pass.

### W-F10-01: RX throttle / unthrottle under CRTSCTS

- **Axis**: F10 (RX throttle/unthrottle with flow control).
- **Stimulus**: with CRTSCTS enabled, host streams 100 KiB at
  115200; driver throttles RX at byte 50000, unthrottles at byte
  50000 + 500 ms.
- **Observable**: (a) IMSC RX sources are masked during the
  throttle window; (b) RTS deasserts, peer stops sending;
  (c) after unthrottle, IMSC is restored and RTS is reasserted;
  (d) exactly 100 KiB received, byte-exact.
- **Verdict rule**: PASS iff (a)-(d) all hold.
- **Platform scoping**: requires CTS/RTS control (same as F9).
- **Statistical contract**: 5 runs; all must pass.

### W-F10-02: RX throttle / unthrottle without CRTSCTS

- **Axis**: F10 (RX throttle/unthrottle without flow control).
- **Stimulus**: with CRTSCTS disabled, same stream as W-F10-01;
  same throttle/unthrottle timing.
- **Observable**: (a) IMSC RX sources are masked during throttle;
  (b) peer continues sending; the PL011 FIFO fills and overruns
  are reported via `icount.overrun` (Linux) /
  `UART_ERROR_OVERRUN` (Zephyr); (c) after unthrottle, IMSC is
  restored and driver resumes draining from the FIFO.
- **Verdict rule**: PASS iff (a)-(c) all hold. Byte loss during
  the throttle window is expected (no flow control); the
  specific loss count is NOT verdict-bearing because it depends
  on PL011 FIFO timing and QEMU RX pacing.
- **Statistical contract**: 5 runs; all must pass (a)-(c).

## 4. Performance workloads

All performance workloads are QEMU-relative: Regseq-generated vs
native driver under identical QEMU setup. Results are reported as
`(median, p95, p99, CI, n-runs, native-baseline-median)`.

### W-P1-01: Sustained TX throughput at 921600

- **Axis**: P1.
- **Stimulus**: driver transmits 50 MiB of pseudo-random data at
  921600 8N1 continuously.
- **Observable**: effective TX throughput in bytes/second.
- **Verdict rule**: Regseq median within 20% of native median (lower
  bound: 0.80 * native; upper bound: 1.20 * native). Report actual
  ratio.
- **Statistical contract**: 10 runs; median + p95/p99 + bootstrap CI.

### W-P1-02: Sustained TX throughput at 115200

- **Axis**: P1.
- **Stimulus**: same as W-P1-01 but at 115200.
- **Observable**: same.
- **Verdict rule**: Regseq median within 15% of native median
  (tighter because 115200 is well below any QEMU PL011 performance
  ceiling).
- **Statistical contract**: 10 runs.

### W-P2-01: Sustained RX throughput at 921600

- **Axis**: P2.
- **Stimulus**: host sends 50 MiB of pseudo-random data at 921600;
  driver reads.
- **Observable**: effective RX throughput; zero byte loss.
- **Verdict rule**: Regseq median within 20% of native median; any
  byte loss fails the row regardless of throughput.
- **Statistical contract**: 10 runs.

### W-P3-01: RX under CPU stress

- **Axis**: P3 (RX robustness under CPU stress).
- **Stimulus**: guest runs a CPU-bound background task (e.g., `dd`
  of `/dev/zero` into `/dev/null` at full CPU); host streams 10 MiB
  at 921600; driver reads.
- **Observable**: byte loss count.
- **Verdict rule**: Regseq byte loss count within 1.5x native byte
  loss count; zero loss for both is an accepted pass.
- **Statistical contract**: 10 runs.

### W-P4-01: Interrupts per KiB during sustained RX

- **Axis**: P4 (batching efficiency).
- **Stimulus**: same as W-P2-01.
- **Observable**: IRQ invocation count per KiB received.
- **Verdict rule**: Regseq ratio no worse than 1.5x native ratio
  (fewer is better).
- **Statistical contract**: 10 runs.

### W-P5-01: Guest CPU per byte during sustained TX

- **Axis**: P5.
- **Stimulus**: same as W-P1-01.
- **Observable**: guest CPU time per byte (sampled via guest-side
  perf counters or /proc/stat where available).
- **Verdict rule**: Regseq median within 20% of native median.
- **Statistical contract**: 10 runs; `not-applicable` on Zephyr
  absent a CPU accounting primitive.

### W-P6-01: Small-write latency

- **Axis**: P6.
- **Stimulus**: driver writes a single byte every 50 ms for 60 s,
  with no other workload running.
- **Observable**: wall-time from write call to host observation of
  byte; p50 / p95 / p99.
- **Verdict rule**: Regseq p95 within 1.5x native p95; p50 within
  1.3x native p50.
- **Statistical contract**: one run; 1200 samples; p95/p99 with
  bootstrap CI.

### W-P7-01: FIFO threshold responsiveness

- **Axis**: P7.
- **Stimulus**: drive sustained RX at 115200 with the driver
  configured to the default IFLS level; then reconfigure to a
  higher-threshold setting and repeat.
- **Observable**: observed interrupt cadence correlates with the
  configured IFLS level (monotonic trend).
- **Verdict rule**: PASS iff cadence ordering matches expected IFLS
  ordering; numeric comparison reported but not verdict-bearing.
- **Statistical contract**: 5 runs per IFLS setting.

### W-P1-DMA-01: Sustained TX throughput at 921600 via DMA

- **Axis**: P1 (DMA-realization extension; P-DMA is not a separate
  axis per `pl011-comparison-axes.md` Section 4).
- **Stimulus**: driver transmits 100 MiB at 921600 using the
  DMA-path realization.
- **Observable**: effective TX throughput; guest CPU cost per byte
  (where perf counters are available).
- **Verdict rule** (per-backend):
  - **Linux**: Regseq-Linux-with-DMA median throughput within 20%
    of native-Linux-with-DMA median; CPU/byte within 20%.
  - **Zephyr**: feature-delta observation only. Report
    Regseq-Zephyr-with-DMA median; record that native Zephyr
    generic PL011 has no DMA path (no quantitative comparison).
- **Statistical contract**: 10 runs per backend.
- **Internal staging note**: paper-load-bearing only after DMA is
  added to `driver.rs` and both backends can lower the DMA path.
  Becomes claim-load-bearing at that point.

### W-P2-DMA-01: Sustained RX throughput at 921600 via DMA

- **Axis**: P2 (DMA-realization extension).
- **Stimulus**: host sends 100 MiB at 921600; driver receives via
  DMA-path realization.
- **Observable**: effective RX throughput; byte loss count.
- **Verdict rule** (per-backend): same per-backend split as
  W-P1-DMA-01, with any byte loss failing the row.
- **Statistical contract**: 10 runs per backend.

### W-P5-DMA-01: Guest CPU cost per byte during DMA TX

- **Axis**: P5 (DMA-realization extension).
- **Stimulus**: same as W-P1-DMA-01.
- **Observable**: guest CPU time per byte (where available).
- **Verdict rule** (Linux): Regseq-Linux-with-DMA median within
  20% of native-Linux-with-DMA median. Zephyr: `not-applicable`
  absent CPU accounting primitive.
- **Statistical contract**: 10 runs.

## 5. Integration workloads

Integration workloads verify that the generated driver participates
in target-OS frameworks as a native driver does. Each row is
one-stimulus-one-observable per Section 1 discipline.

### Linux

#### W-IL1-01: out-of-tree module build + load

- **Stimulus**: `make` the out-of-tree module against the pinned
  kernel source; `insmod` the resulting `.ko`.
- **Observable**: build exits 0, `insmod` exits 0, `lsmod` lists
  the module, `dmesg` records probe-success messages.
- **Environment**: pinned kernel; clean module-namespace (no prior
  load).
- **Verdict rule**: PASS iff all three checks pass.
- **Statistical contract**: 3 runs; all must pass.

#### W-IL2-01: reserved-device binding

- **Stimulus**: boot guest with the primary PL011 serving as
  console and a reserved secondary PL011; load the Regseq-generated
  driver.
- **Observable**: driver's probe fires exactly once on the
  reserved device; the console UART remains on the native path
  (kernel printk continues unaffected).
- **Environment**: two-PL011 topology per evaluation harness.
- **Verdict rule**: PASS iff probe count equals 1 for the reserved
  device AND console continues working.
- **Statistical contract**: 3 runs.

#### W-IL3-01: `/dev/ttyAMA<N>` creation

- **Stimulus**: after module load, inspect `/dev`.
- **Observable**: `/dev/ttyAMA<N>` exists with character-device
  semantics and correct major/minor.
- **Verdict rule**: PASS iff node exists with expected major.
- **Statistical contract**: 3 runs.

#### W-IL3-02: open/close round-trip

- **Stimulus**: userspace program opens `/dev/ttyAMA<N>`, closes
  it.
- **Observable**: `open` returns a valid fd; `close` returns 0;
  no kernel oops.
- **Verdict rule**: PASS iff both syscalls succeed.
- **Statistical contract**: 3 runs.

#### W-IL3-03: read/write round-trip

- **Stimulus**: userspace opens `/dev/ttyAMA<N>`, writes 1 KiB,
  reads 1 KiB looped back via the evaluation harness.
- **Observable**: `write` and `read` byte counts match;
  written and read streams are byte-exact.
- **Verdict rule**: PASS iff both counts match and streams agree.
- **Statistical contract**: 3 runs.

#### W-IL3-04: poll/select responsiveness

- **Stimulus**: userspace opens `/dev/ttyAMA<N>`, calls
  `poll(POLLIN)` with a 1 s timeout while the harness injects
  bytes at 500 ms.
- **Observable**: `poll` returns POLLIN before timeout.
- **Verdict rule**: PASS iff `poll` returns POLLIN with
  `revents & POLLIN` set.
- **Statistical contract**: 3 runs.

#### W-IL3-05: stty configuration round-trip

- **Stimulus**: run `stty 115200 cs8 -parenb -cstopb` against
  `/dev/ttyAMA<N>`, then `stty -a` to read back.
- **Observable**: readback reports the configured settings.
- **Verdict rule**: PASS iff readback matches input.
- **Statistical contract**: 3 runs.

#### W-IL4-01: serial_core counter updates

- **Stimulus**: transmit and receive 10 KiB through the tty.
- **Observable**: `/proc/tty/driver/ttyAMA` tx/rx counters
  advance by the expected amounts (± 0; these are exact byte
  counts).
- **Verdict rule**: PASS iff counter delta equals byte count.
- **Statistical contract**: 3 runs.

#### W-IL5-01: devicetree match

- **Stimulus**: boot guest with a DT node for the reserved PL011;
  load the generated driver.
- **Observable**: driver binds to that DT node; no other
  bindings reported in `dmesg`.
- **Verdict rule**: PASS iff exact DT match with exactly one bind.
- **Statistical contract**: 3 runs.

### Zephyr

#### W-IZ1-01: canonical app build + boot

- **Stimulus**: build the Zephyr sample app for
  `qemu_cortex_a53` with the generated driver; boot the image.
- **Observable**: build succeeds; `main()` entry reached; no
  fault prior to main.
- **Verdict rule**: PASS iff both.
- **Statistical contract**: 3 runs.

#### W-IZ2-01: DT binding match

- **Stimulus**: DT `compatible = "arm,pl011"` node present in
  overlay; boot.
- **Observable**: `DEVICE_DT_GET` for that node returns a valid
  device pointer and `device_is_ready()` returns true.
- **Verdict rule**: PASS iff device is ready.
- **Statistical contract**: 3 runs.

#### W-IZ3-01: `uart_poll_in` / `uart_poll_out`

- **Stimulus**: app calls `uart_poll_out` for a 256-byte payload,
  then `uart_poll_in` in a loop to receive the same payload (via
  loopback harness).
- **Observable**: all 256 bytes received byte-exact.
- **Verdict rule**: PASS iff byte-exact.
- **Statistical contract**: 3 runs.

#### W-IZ3-02: `uart_configure`

- **Stimulus**: app calls `uart_configure` with
  `{baudrate: 115200, parity: UART_CFG_PARITY_NONE, stop_bits:
  UART_CFG_STOP_BITS_1, data_bits: UART_CFG_DATA_BITS_8,
  flow_ctrl: UART_CFG_FLOW_CTRL_NONE}`.
- **Observable**: return code 0; programmed `IBRD`/`FBRD`/`LCR_H`
  match expected values.
- **Verdict rule**: PASS iff rc == 0 AND register state matches.
- **Statistical contract**: 3 runs.

#### W-IZ4-01: `uart_irq_callback_set` and RX IRQ

- **Stimulus**: app registers IRQ callback via
  `uart_irq_callback_set`, enables RX IRQ via
  `uart_irq_rx_enable`; harness injects 1 KiB.
- **Observable**: callback fires; `uart_fifo_read` drains all
  bytes byte-exact.
- **Verdict rule**: PASS iff byte-exact.
- **Statistical contract**: 3 runs.

#### W-IZ4-02: TX IRQ

- **Stimulus**: app enables TX IRQ via `uart_irq_tx_enable`; app
  drives a 1 KiB transmit from the TX-ready callback.
- **Observable**: harness receives 1 KiB byte-exact; TX IRQ is
  disabled after completion.
- **Verdict rule**: PASS iff byte-exact AND clean IRQ lifecycle.
- **Statistical contract**: 3 runs.

## 6. Statistical contract (detail)

- **Repetitions**: minimum run counts defined per workload. Higher
  counts are acceptable; lower counts invalidate the row.
- **Summary statistics**: report median for central tendency,
  p95 + p99 for tail, bootstrap 95% CI for median. Mean and std
  optional.
- **Environment metadata**: every run records QEMU version, guest
  kernel / Zephyr version + config, host kernel + CPU, driver
  commit SHA, and seed where applicable.
- **Tolerance constants provenance**: every numeric tolerance
  constant in Section 4 (e.g., "within 20%", "within 1.5x") is
  PROVISIONAL until pilot data is collected. A pilot run of the
  full Section 4 suite against native drivers (no Regseq changes)
  is required before tolerances are finalized. The pilot informs
  two things: (a) the native-side variance (CI half-width), which
  sets the minimum defensible tolerance window; (b) the tolerance
  constant itself, which is the variance window plus a headroom
  factor to accommodate legitimate backend differences. The
  current constants are plausible defaults based on QEMU-PL011
  timing characteristics, not measured.
- **High-variance policy** (escalation ladder when a row's
  native-side CI half-width exceeds 15% of the median):
  1. **Methodology repair first**: increase run count (up to 30);
     isolate the variance source (CPU contention, QEMU
     scheduling, I/O noise); if repairable, re-report with the
     original verdict rule.
  2. **Promote to qualitative observation** if variance persists:
     drop the numeric verdict rule; report the row as a
     directional observation ("Regseq and native are within the
     same order of magnitude") without pass/fail.
  3. **Shrink the claim** if the row is claim-load-bearing and
     cannot be repaired or cleanly demoted: document that the
     axis this row realizes is no longer part of the paper claim
     and adjust the thesis / matrix accordingly.
- **Verdict rollup**: a matrix row is `comparable` iff the
  workload's verdict rule passes; `non-comparable` otherwise;
  with explicit `not-applicable` reserved for platform/API-missing
  cases; with `qualitative` reserved for rows demoted per the
  high-variance policy.

## 7. Change history

- 2026-04-20: initial version paired with
  `pl011-comparison-axes.md` and `pl011-spec-requirements.md`.
