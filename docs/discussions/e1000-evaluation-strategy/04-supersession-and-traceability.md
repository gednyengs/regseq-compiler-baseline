# 04 — Supersession and Traceability

This document is the formal old-to-new audit map. Every prior E1000
evaluation artifact, axis, and locked decision is accounted for here, so a
reviewer or thesis defender can trace why each was kept, reframed, or cut. No
prior decision is silently dropped (per the review discipline in the prior
`e1000-comparison-axes.md` §8).

## 1. Document disposition

| Prior document (`../e1000-full-spec/`) | Disposition | Action taken |
| --- | --- | --- |
| `e1000-comparison-axes.md` | **Superseded** (evaluation strategy) | Banner + change-history entry; U1–U7 carried forward here |
| `e1000-workloads.md` | **Superseded** | Banner + change-history entry; replaced by §02 §3–§4 |
| `e1000-experiment-methodology.md` | **Superseded** (incl. §3 QEMU patch catalog) | Banner + change-history entry; environment summarized in §02 §7, full runbook is a separate artifact |
| `e1000-full-spec-methodology.md` | **Partially superseded** | Banner: evaluation-strategy framing superseded; the per-feature *spec-growth* workflow is retained but must align with this strategy |
| `e1000-full-spec-status.md` | **Superseded** (staged plan) | Banner: the QEMU-gated Stage-1/2/3 plan no longer applies; tracking follows this strategy |
| `README.md` | **Scope-noted** | Banner pointing here for the evaluation strategy and the revised claim shape; corpus + scope pin retained |
| `e1000-spec-requirements.md` | **Retained, re-governed** | Scope note: spec requirements survive; their *claim-path / axis linkage* is governed here (§3 below) |

## 2. Axis and use-case disposition

**Use cases (D4): all retained.** U1 host endpoint, U2 bulk transfer, U3
low-latency req/resp, U4 interactive, U5 DHCP/bring-up, U6 link
state/reconfig, U7 observation/capture. Mapping: U5→W1; U1/U2→W2; U3/U4→W3;
U6→W4; U7→W5 (§02 §3).

| Prior axis | Disposition | Where it goes |
| --- | --- | --- |
| F1, F2 (TX/RX integrity) | Retained (functional) | Folded into W2 (payload-integrity check) |
| F3 (link autoneg/state) | Retained (functional, pass/fail) | W1, W4; timing explicitly not claimed (§03 §2.5) |
| F4 (MAC programming) | Retained (functional) | Exercised within W1 bring-up |
| F5 (RX filtering) | Retained (functional) | W5 |
| F6-TX (TX checksum) | Reframed → functional only | Exercised in W2; no perf claim (§03 §2.3) |
| F6-RX (RX checksum) | **Cut as runtime row**; retained as expressiveness | §03 §2.2; spec still declares RXCSUM |
| F7 (interrupt-driven service) | Reduced to "service works" | Folded into W1/W2; fine-grained cadence cut |
| F8 (lifecycle/reconfigure) | Retained (functional) | W4 |
| F9 (VLAN) | Deferred (D8) — unchanged | Not evaluated |
| F10 (promiscuous toggle) | Retained (functional) | W5 |
| P1, P2 (throughput) | Collapsed → single guardrail | W2 cost guardrail (§02 §4) |
| P3 (RX under CPU stress) | **Cut** | §03 §1 (emulator/scheduler pressure) |
| P4 (interrupts/pkt) | **Cut** | §03 §2.4 (emulator-artifact) |
| P5 (CPU/byte) | Collapsed → single guardrail | W2 cost guardrail |
| P6 (latency curve) | Collapsed → coarse RTT sanity | W3 (no curve) |
| P7 (ring efficiency) | **Cut** | §03 §2.4 (emulator-artifact) |
| IL1–IL4 (build/probe/netdev/ops) | Retained (integration) | Demonstrated by W1–W2 succeeding on Linux |
| IL5 (ethtool coalesce) | **Cut**; reframed as correctness expectation | §03 §2.4 (82540EM returns `-EOPNOTSUPP`; Regseq must match) |
| IL6 (sysfs/proc) | Smoke-log only | Not a results row |
| IZ1–IZ4 (Zephyr integration) | Retained (integration) | Demonstrated by the headline rows W1–W3 succeeding on Zephyr (and W4-Zephyr where `set_link` is available on the board harness) |

## 3. Locked-decision disposition

D1 **revised** (x86_64+KVM); D2 retained; D3 retained as spec evidence-gathering;
D4 retained; D5 retained/strengthened; **D6 reframed** (TX functional only,
RX cut as runtime row, both still expressible); **D7 cut as runtime row**
(still expressible); D8/D9/D10 retained unchanged. Rationale for each is in
[`01-claim-and-evaluation-philosophy.md`](01-claim-and-evaluation-philosophy.md)
§6 and [`03-cut-list-and-technical-justification.md`](03-cut-list-and-technical-justification.md)
§2.

**`e1000-spec-requirements.md` survival rule (precise).** The spec requirements
themselves — registers, descriptor layouts, correctness-only rules,
backend-contract rules, the deferred ledger (D8/D9/D10, variant pin) — are
**retained**; the DSL must still express them, and that expressiveness is
*evidence for the paper*, not against it. What changes is only their
*evaluation linkage*: the prior `[claim-path]` performance tags on
REQ-PERF-CSUM-\* and REQ-PERF-MODERATION-\* (D6/D7) no longer designate
benchmarked runtime rows. They become expressiveness/correctness requirements
with no runtime-measurement obligation. The QEMU-patch-gated Stage-2/3
machinery in that document is void (no patch program exists).

## 4. Change-history entry applied to each superseded prior document

> 2026-05-19: SUPERSEDED by `../e1000-evaluation-strategy/`. The
> full-coverage axis matrix and the QEMU device-model patch program are
> withdrawn; the E1000 evaluation is now minimal-sufficient and
> patch-free. See `../e1000-evaluation-strategy/04-supersession-and-traceability.md`
> for this document's specific disposition. Retained for provenance only;
> do not plan or report experiments from it.

## 5. What a reader should do

- Planning or reporting an E1000 experiment → use **only** this directory.
- Tracing the history of a decision → start here (§1–§3), follow the pointers.
- Working on the E1000 *spec* (not its evaluation) →
  `../e1000-full-spec/e1000-spec-requirements.md` remains live, read with §3
  above.
