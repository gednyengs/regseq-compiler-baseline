# 02 — The Minimal-Sufficient Evaluation

This is the operative test plan. It is intentionally small: it is the smallest
evidence set that defends the central claim of §01 §3 and that a skeptical
reviewer would accept.

## 1. Expressiveness vs. runtime measurement (read this first)

A capability can be **expressible** in the DSL without being a **benchmarked
runtime row**. These are different kinds of evidence:

- *Expressiveness evidence* answers "can the DSL say it?" — defended by the
  spec-as-artifact analysis (§2) and by the generated driver advertising/using
  the capability where the platform supports it.
- *Runtime evidence* answers "does the compiled driver behave correctly/at
  acceptable cost?" — defended by the workloads (§3) and guardrail (§4).

Checksum offload and interrupt moderation remain **expressible** (the spec
declares them; the generated driver programs them); they are simply not
benchmarked runtime rows, because the substrate cannot exhibit them honestly
(§03). The evaluation must present this distinction explicitly so a reviewer
does not mistake a measurement-scope decision for a capability gap.

## 2. Spec-as-artifact evidence (offensive: expressiveness)

This is the cheapest and most under-weighted evidence in the prior strategy,
and for a DSL paper it is primary.

- **Specification size and declarativeness.** Report E1000 spec source size
  (`cloc`) and a count of declared registers / descriptor layouts / protocol
  constructs, against the line count of the hand-written Linux `e1000` driver
  it stands in for. The point is the *ratio* and the *declarative fraction*,
  not absolute lines.
- **Single-source proof.** Show the Linux and Zephyr drivers are generated
  from the *same spec artifact* (same content hash). Enumerate every point of
  divergence between the two generated drivers and demonstrate each is
  **backend lowering** (PCI/module/netdev/NAPI vs. DT/`ethernet_api`/`net_if`),
  not re-specified device behavior.
- **Taxonomy-construct coverage.** One table mapping the constructs the E1000
  spec uses (data path, link/PHY, filtering, descriptor rings, interrupts,
  lifecycle, backend resources) to the taxonomy. This ties the case study back
  to the representability thesis.
- **Escape-hatch boundary (acceptance rule for "expressiveness").** §01 §1
  requires the DSL to express the driver "without escaping to hand-written
  code for the parts that matter." This is made falsifiable: enumerate every
  hand-written escape hatch and every backend-glue boundary in each generated
  driver, with LOC, and apply the rule **every device-semantic behavior must
  trace to the shared spec; hand-written code is permitted only for backend
  glue, never for device semantics.** A device-semantic behavior found only in
  hand-written code is a failure of the expressiveness claim, reported as such,
  not hidden. The artifact table (§5) carries an explicit escape-hatch row.

## 3. Practical-use functional workloads (offensive: correctness + portability)

Five end-to-end scenarios, observed at the **socket / netdev level** (where
practical use lives), not at the register-bit level. Each maps to the locked
U1–U7 use cases (D4). Verdict is pass/fail unless a relative metric is named.

| # | Workload | Stimulus | Observable | Defends | OS scope |
| - | -------- | -------- | ---------- | ------- | -------- |
| W1 | Cold bring-up | Boot guest, probe device, bring link up, acquire address (DHCP or static), `ping` a peer | Link reports up; address acquired; ICMP echo succeeds | Spec lowers to a *usable OS network device*; OS-stack integration; U5 | Linux **+ Zephyr** |
| W2 | Bulk TCP transfer | Bidirectional sustained TCP at 1500 MTU, TX checksum offload enabled, with payload integrity check | Transfer completes; received payload bit-identical to sent | Real working data path; correctness under load; TX-checksum path exercised; U1/U2 | Linux **+ Zephyr** |
| W3 | Interactive / request-response | Small-message request/response stream + `ping` RTT sample | Responses correct; RTT distribution has no pathological tail vs. native | No pathological latency for interactive/RPC use; U3/U4 | Linux **+ Zephyr** |
| W4 | Operational dynamics | Link down then up during active traffic; re-acquire address; resume transfer | No corruption across the transition; traffic resumes; no driver hang | Survives link-flap / reconfigure-around-data; U6 | Linux + Zephyr if link injection is available without altering device semantics; else Linux + qualitative Zephyr |
| W5 | Filtering / promiscuous | Inject unicast, multicast, and non-matching traffic; toggle promiscuous mode; observe with a capture tool | Normal filtering delivers only matching frames; promiscuous delivers all | Receive-filter and promiscuous correctness; U7 | Linux-only (no Zephyr packet-capture analogue) |

**Headline vs. auxiliary classification (load-bearing — see §5).**

- **W1, W2, W3 are the headline cross-OS rows**: each is fully comparable on
  *both* Linux and Zephyr (native and Regseq), and only these carry the
  cross-OS portability claim.
- **W4 is Linux-comparable + Zephyr-conditional**: link down/up is driven by
  the patch-free QMP `set_link` host command (stock QEMU raises the E1000 LSC
  interrupt on link-status change — `hw/net/e1000.c`, link-status→LSC path;
  no device-model patch). W4 is pass/fail (no negotiation-timing claim), so
  `set_link` suffices. On Zephyr, W4 is comparable if the board harness can
  drive `set_link` against the E1000 DUT; otherwise W4-Zephyr is a qualitative
  observation. W4 is **auxiliary correctness evidence, not a headline
  portability row.**
- **W5 is Linux-only auxiliary**: no Zephyr packet-capture analogue exists, so
  W5 corroborates filtering correctness on Linux only and is **never**
  presented as cross-OS evidence.

Notes:

- W2 folds in what was F1/F2 (TX/RX integrity) and the *functional* part of
  F6 (TX checksum: offload enabled, data still correct). It makes **no**
  checksum-performance claim.
- No workload depends on a modification to QEMU's *device model*. Host-side
  control-plane commands (QMP `set_link`, standard monitor) are not
  device-model patches and are permitted (§03 §3).

## 4. The cost guardrail (defensive: acceptable cost)

A single, coarse, honestly-framed comparison:

- **Sustained throughput** and **guest CPU cost per byte**, native vs. Regseq,
  measured during the W2 bulk transfer (no separate microbenchmark).
- **Preregistered tolerance rule.** The guardrail band is fixed *before* any
  Regseq-vs-native comparison and is written down (preregistered) in the
  runbook. It is derived mechanically from the native-only pilot: the band is
  the native-side run-to-run variation (its 95% CI) widened by a single,
  stated headroom multiple `k` (the value of `k` is committed in the runbook
  before Regseq numbers are collected, not chosen after seeing them).
  "Pathological regression" is defined as: Regseq's median falling outside
  that preregistered band. There is no post-hoc band adjustment; if the band
  is exceeded, that is reported as a finding, not retuned.
- Reported as: "Regseq is within the preregistered guardrail band of native;
  no pathological regression." It is **not** reported as a performance result,
  a curve, or a win.
- Latency/interrupt/ring microbenchmarks (former P3/P4/P6/P7) are **not**
  collected. Rationale in §03.

## 5. Result tables (what actually goes in the paper)

The result presentation is **deliberately split** so that no Linux-only or
qualitative row is read as cross-OS portability evidence.

**Headline portability table (the money figure).** Rows = **W1, W2, W3
only**. Columns = {native Linux, Regseq Linux, native Zephyr, Regseq Zephyr}
pass/fail, plus a relative-metric column for W2/W3. Every cell is a real,
comparable result on both OSes. A reviewer reading this one table concludes:
one spec, two OSes, behaves like native on real use. The cross-OS portability
claim is read off **this table only**.

**Auxiliary correctness table.** Rows = W4 (Linux native/Regseq; Zephyr
native/Regseq *or* "qualitative" if `set_link` is unavailable on the Zephyr
harness) and W5 (Linux native/Regseq only; Zephyr column marked
"n/a — no analogue"). This table corroborates correctness under operational
dynamics and filtering; it is explicitly labeled as **not** cross-OS
portability evidence.

**Artifact table.** Spec LOC and declared-construct count; generated Linux
LOC; generated Zephyr LOC; hand-written Linux `e1000` LOC; shared spec content
hash; enumerated backend-only divergence points; **enumerated hand-written
escape hatches / backend-glue boundaries with LOC** (per the §2 acceptance
rule); taxonomy-construct coverage checklist.

These three tables, plus the preregistered W2/W3 guardrail sentence, are the
complete quantitative footprint of the E1000 evaluation.

## 6. Statistical contract (scoped to the small matrix)

Carried from the prior locked contract, applied only to the rows above:

- Functional workloads (W1, W4, W5): pass/fail; minimum 5 independent runs;
  any non-determinism reported, not averaged away.
- Workloads with a relative metric (W2 throughput/CPU, W3 RTT): minimum 10
  runs; report median and p95/p99 with a bootstrap 95% CI.
- **Host metadata captured from run 1** (host kernel, CPU model, governor,
  KVM on, QEMU build hash, guest kernel, rootfs hash, spec hash, binding
  artifacts) so the native-only pilot can be retro-validated and so the full
  drift-control template can be adopted before the comparative campaign
  without re-baselining.
- The native-only pilot sets the W2/W3 guardrail band per the **preregistered
  tolerance rule in §4** (native 95% CI widened by a committed headroom
  multiple `k`), fixed and written down before any Regseq-vs-native
  comparison; provisional bands are not reported and the band is never
  retuned post-hoc.

## 7. Environment (summary; full runbook is a separate artifact)

- **Linux**: x86_64 guest under **KVM** on the Ubuntu host (revised D1).
- **Binding/fairness**: a reserved DUT e1000 on a pinned PCI slot plus a
  separate virtio-net management NIC; stock `e1000` blacklisted and bound via
  `driver_override`; per-run `lspci -nnk` + dmesg capture proving no prior
  probe. "Only the driver under test differs" is enforced here.
- **Peer**: TAP-bridge transport (project rule); peer in a host network
  namespace with tap/bridge offloads (gso/gro/tso) disabled and pinned.
- **Zephyr**: `qemu_cortex_a53` + `QEMU_CORTEX_A53_ETH_E1000` (D2), PL011
  control channel.
- **No QEMU device-model patches are required or permitted** for any row in
  §3–§4 (§03 §3).

The concrete runbook (build steps, exact commands, collection scripts) is
authored separately and must not contradict this section.
