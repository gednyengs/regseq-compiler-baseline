# E1000 Workloads and Tolerance Rules

**Status**: SKELETON. Awaiting authoring once `e1000-comparison-axes.md`
locks the axis set.

**Role**: realization layer for the comparison axes defined in
`e1000-comparison-axes.md`. Each axis will be exercised by one or more
concrete workloads. Each workload has a fixed shape (stimulus, observable,
environment, verdict rule) and a statistical contract under which results
are reported.

## 1. Workload shape discipline (mirrors PL011)

Every workload in this document follows the same shape:

- **Stimulus**: what is done to drive the axis (one well-defined input
  sequence or condition).
- **Observable**: what is measured (one well-defined output metric or
  pass/fail condition).
- **Environment**: QEMU configuration + guest OS image + driver binding
  topology; held identical across native and Regseq runs.
- **Verdict rule**: how pass / fail is decided, including tolerance window
  for performance workloads.
- **Statistical contract**: how many runs, what summary statistics, what
  confidence intervals.

Row-granularity rule: one workload instance = one stimulus + one
observable + one environment + one verdict rule. A workload that combines
two stimuli or two observables is split into two rows.

## 2. Shared environment skeleton (to be authored)

To be filled in after `e1000-experiment-methodology.md` Section 1
(topology) is authored. Provisional anchors:

- **QEMU machine**: aarch64 `virt` (Linux); chosen Zephyr board (TBD).
  E1000 device at a reserved PCI slot.
- **Driver binding topology**: a reserved E1000 device for the driver
  under test, distinct from any device used by the boot/management path.
- **Repetition**: minimum 5 runs for functional, 10 for performance;
  results reported as median + p95/p99 with bootstrap 95% CI.
- **Reporting**: every matrix cell records run count, median, p95/p99,
  CI half-width, and the native baseline value used for tolerance check.

## 3. Functional workloads (to be authored)

One workload per functional axis (more rows per axis where needed). Each
row uses the five-part shape above. To be authored after
`e1000-comparison-axes.md` Section 3 is locked.

Provisional axes-to-workload skeleton:

| Axis | Workload IDs (provisional) |
| ---- | -------------------------- |
| F1   | W-F1-01 (TX integrity, sustained)         |
| F2   | W-F2-01 (RX integrity, sustained)         |
| F3   | W-F3-01 (link autoneg + state report)     |
| F4   | W-F4-01 (unicast MAC programming)         |
| F5   | W-F5-01 (multicast filter), W-F5-02 (promiscuous) |
| F6   | W-F6-01 (TX checksum offload), W-F6-02 (RX checksum offload) |
| F7   | W-F7-01 (interrupt-driven RX), W-F7-02 (interrupt-driven TX), W-F7-03 (link IRQ) |
| F8   | W-F8-01 (reconfigure-around-data)         |
| F9   | W-F9-01 (VLAN tag, if in scope)           |
| F10  | W-F10-01 (promiscuous toggle)             |

To be reviewed once the axes lock.

## 4. Performance workloads (to be authored)

QEMU-relative; tolerance constants PROVISIONAL until pilot run finalizes
per Section 6.

Provisional skeleton:

| Axis | Workload IDs (provisional) |
| ---- | -------------------------- |
| P1   | W-P1-01 (sustained TX, large frame), W-P1-02 (sustained TX, small frame) |
| P2   | W-P2-01 (sustained RX, large frame), W-P2-02 (sustained RX, small frame) |
| P3   | W-P3-01 (RX under CPU stress)             |
| P4   | W-P4-01 (interrupts per packet at default moderation), W-P4-02 (per-packet at tuned moderation) |
| P5   | W-P5-01 (CPU cost per byte during sustained TX) |
| P6   | W-P6-01 (small-packet round-trip latency) |
| P7   | W-P7-01 (descriptor-ring efficiency)      |

## 5. Integration workloads (to be authored)

Linux: W-IL1-01..W-IL6-NN once integration axes lock.

Zephyr: W-IZ1-01..W-IZ4-NN once integration axes lock.

## 6. Statistical contract (to be authored)

Mirrors `pl011-workloads.md` Section 6. Tolerance constants are
PROVISIONAL until a native-only pilot run informs them; high-variance
escalation ladder applies.

## 7. Change history

- 2026-05-04: skeleton stood up.
