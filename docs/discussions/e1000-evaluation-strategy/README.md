# E1000 Evaluation Strategy (Authoritative)

**Status**: AUTHORITATIVE. This directory is the single source of truth for how
the E1000 (Intel 82540EM) case study is evaluated in the Regseq paper.

**Supersession**: This directory **supersedes the prior E1000 evaluation
strategy** that lived in `../e1000-full-spec/` (`e1000-comparison-axes.md`,
`e1000-workloads.md`, `e1000-experiment-methodology.md`, and the
evaluation-strategy portions of `e1000-full-spec-methodology.md`,
`e1000-full-spec-status.md`, and `README.md`). Those documents carried a
full-coverage axis matrix (F1-F10 / P1-P7 / IL1-IL6 / IZ1-IZ4) and a QEMU
device-model patch program. They are retained, banner-marked as superseded, for
provenance only and **must not be used to plan or report experiments**. The
formal old-to-new disposition of every prior artifact and locked decision is in
[`04-supersession-and-traceability.md`](04-supersession-and-traceability.md).

`../e1000-full-spec/e1000-spec-requirements.md` is **not** superseded: the spec
requirements (what the DSL must express) survive. Only their *evaluation /
claim-path linkage* is re-governed here (see §04).

## Why this exists

The prior strategy was a coverage exercise — it sought to test every feature the
82540EM exposes. Two findings forced a reframing:

1. The paper's contribution is a **device-driver specification DSL + compiler**,
   not a NIC. For that contribution, performance is a **defensive guardrail, not
   an offensive result**.
2. Three prior claim-path rows (RX-checksum offload, interrupt-moderation, and
   descriptor-ring/IRQ efficiency) cannot be measured on stock QEMU v8.2.2
   without patching its e1000 device model — i.e. they would measure the
   emulator patch, not the DSL. See
   [`03-cut-list-and-technical-justification.md`](03-cut-list-and-technical-justification.md).

The result is a **minimal-sufficient** evaluation: the smallest evidence set
that defends the actual claim and that a skeptical reviewer would accept,
biased toward how a NIC is used in practice rather than microarchitectural
discrimination.

## The claim, in one paragraph

One semantic Intel 82540EM specification, written once in Regseq, compiles to
Linux **and** Zephyr drivers that build, integrate with each OS's networking
stack, and preserve native-driver behavior on realistic NIC use-case workloads,
at no pathological cost. Expressiveness and cross-OS portability are the
offensive claims; cost is a bounded guardrail. No absolute-hardware,
codegen-performance, or microarchitectural claim is made.

## Index

- [`01-claim-and-evaluation-philosophy.md`](01-claim-and-evaluation-philosophy.md)
  — the contribution, the four evaluation criteria, why performance is
  defensive, QEMU-relative scope and its honest limits, relationship to the
  taxonomy/representability thesis.
- [`02-minimal-sufficient-evaluation.md`](02-minimal-sufficient-evaluation.md)
  — spec-as-artifact evidence, the practical-use workload set, the single cost
  guardrail, result tables, statistical contract, environment.
- [`03-cut-list-and-technical-justification.md`](03-cut-list-and-technical-justification.md)
  — every cut with its reviewer-facing rationale, and the QEMU-device-model /
  Linux-stock-driver cross-reference evidence (file:line) that justifies
  removing RX-checksum / interrupt-moderation / ring-efficiency and the entire
  QEMU device-model patch program.
- [`04-supersession-and-traceability.md`](04-supersession-and-traceability.md)
  — formal old-to-new map for every prior artifact, axis, and locked decision.
- [`05-linux-native-pilot-runbook.md`](05-linux-native-pilot-runbook.md)
  — concrete, reproducible x86_64+KVM environment build and native
  (stock-driver) baseline collection procedure; preregisters the guardrail
  band. Operational realization of §02 §7.

## Scope pin (unchanged)

Single concrete Intel 82540EM (vendor 0x8086, device 0x100E), QEMU's default
`-device e1000`. e1000e family, TSO, MSI-X, multi-queue, RSS, SR-IOV remain out
by the variant pin. VLAN / 802.3x flow control / jumbo remain deferred (D8/D9/
D10). The 82540EM corpus and the Linux+Zephyr claim-bearing-backend model are
unchanged from `../e1000-full-spec/README.md`.

## Change history

- 2026-05-19: Directory created. Minimal-sufficient evaluation adopted;
  prior full-coverage axis matrix and QEMU device-model patch program
  superseded. See §04 for the per-artifact disposition.
