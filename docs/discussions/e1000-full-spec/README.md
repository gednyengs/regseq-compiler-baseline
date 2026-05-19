# E1000 Full Spec Discussion

> **⚠ EVALUATION STRATEGY SUPERSEDED (2026-05-19).** The authoritative E1000
> evaluation strategy is `../e1000-evaluation-strategy/`. The scope pin,
> corpus, and Linux+Zephyr claim-bearing-backend model below are **retained**;
> `e1000-spec-requirements.md` is **retained, re-governed** (see
> `../e1000-evaluation-strategy/04-supersession-and-traceability.md` §3). The
> other evaluation docs in this directory are banner-marked superseded.

This directory is the working area for the full E1000 Regseq spec we intend to
carry through the compiler pipeline and, ultimately, the research evaluation
flow alongside the PL011 target.

The concrete target fixture for this discussion area is:

- `tests/fixtures/e1000-full-spec/`

For this version of the full E1000 target, we are intentionally pinning the
spec to a single concrete Intel E1000 configuration. We are not trying to model
family-wide variant selection, revision dispatch, or quirk/erratum machinery in
this version of the spec.

## Scope pin

- **Concrete variant**: Intel 82540EM. 32-bit parallel-PCI gigabit Ethernet
  controller with single MSI or legacy INTx, single TX queue, single RX queue.
  Matches QEMU's default `-device e1000` device model.
- **Out by variant pin**: e1000e family (82571/82573/ich8lan/80003es2lan/
  i210/82575), TSO, MSI-X, multi-queue, advanced RSS, PCIe link-state behavior,
  SR-IOV, virtual function support.
- **In by variant**: TX/RX descriptor rings, hardware TX/RX checksum offload,
  EEPROM access, basic VLAN, multicast hash filter, WoL, single-vector MSI or
  legacy INTx, RX interrupt moderation (`RDTR`/`RADV`), receive-address
  filtering.

## Corpus and backends

- **Corpus** (read in the four-dimension driver-analysis framework as policy
  source and comparison baseline for spec derivation):
  - Linux: `/Users/nyengele/driverbase/os/linux/drivers/net/ethernet/intel/e1000/`
  - FreeBSD: `/Users/nyengele/driverbase/os/freebsd/sys/dev/e1000/` (em(4)
    driver scoped to the 82540EM-relevant code paths)
  - ReactOS: `/Users/nyengele/driverbase/os/reactos/drivers/network/dd/e1000/`
  - Zephyr: `/Users/nyengele/driverbase/os/zephyr/drivers/ethernet/eth_e1000.c`
    (and adjacent files)
- **Backends in the paper claim**: Linux + Zephyr. Generated Regseq drivers
  vs native drivers compared on QEMU-relative functional, performance, and
  integration workloads. FreeBSD and ReactOS are corpus-only; no Regseq-
  generated FreeBSD or ReactOS drivers are produced or compared.

This mirrors the PL011 corpus model where Linux + FreeBSD + Zephyr + Windows
were all read for evidence but only Linux + Zephyr were claim-bearing
backends.

## What this discussion area is for

Use this discussion area to:

- write the per-OS driver-analysis task briefs and analyses across the four
  dimensions (functional, integration, performance, correctness)
- enumerate the concrete E1000 system-use cases the paper claim ranges over
- derive the comparison axes from those use cases
- author the requirements (axis / correctness / backend-contract) the spec
  must capture
- author the workloads that exercise each axis with explicit stimulus,
  observable, environment, verdict rule, and statistical contract
- author the experiment methodology that bridges abstract workloads to the
  concrete QEMU evaluation environment, including any required QEMU patches
  *(historical — the evaluation is now patch-free; see the supersession banner
  at the top of this file and `../e1000-evaluation-strategy/`)*
- record the exact scope and deferrals for the concrete target fixture
- record explicit deferrals, if any, even for the "full E1000" target

## Relationship to the PL011 work

The artifact set in this directory parallels `pl011-full-spec/` one-for-one:

| Role                                | PL011 artifact                       | E1000 artifact                       |
| ----------------------------------- | ------------------------------------ | ------------------------------------ |
| What is compared                    | `pl011-comparison-axes.md`           | `e1000-comparison-axes.md`           |
| How each axis is exercised          | `pl011-workloads.md`                 | `e1000-workloads.md`                 |
| Definitive requirement list         | `pl011-spec-requirements.md`         | `e1000-spec-requirements.md`         |
| Operational evaluation methodology  | `pl011-experiment-methodology.md`    | `e1000-experiment-methodology.md`    |
| Per-feature growth methodology      | `pl011-full-spec-methodology.md`     | `e1000-full-spec-methodology.md`     |
| Implementation state of the fixture | `pl011-full-spec-status.md`          | `e1000-full-spec-status.md`          |
| Driver analysis task brief (per OS) | `<os>-driver-analysis-task-brief.md` | `<os>-driver-analysis-task-brief.md` |
| Driver analysis output (per OS)     | `<os>-driver-analysis.md`            | `<os>-driver-analysis.md`            |

E1000 starts directly from the axes-derived methodology that PL011 converged
on after its 2026-04-20 reframing. There is no E1000 analogue of the older
feature-inventory-driven docs (`pl011-feature-matrix.md`,
`pl011-unified-spec-boundary.md`, `pl011-feature-to-construct-mapping.md`)
because that methodology was already retired before E1000 work began.

## Paper claim shape (provisional, to be finalized in `e1000-comparison-axes.md`)

> Regseq's semantic driver model expresses a single Intel 82540EM E1000
> specification, and its compiler generates Linux and Zephyr drivers that
> match native-driver behavior within predefined tolerance rules on
> QEMU-relative functional and performance workloads derived from concrete
> E1000 system use cases.

**SUPERSEDED (2026-05-19).** This provisional claim shape is replaced by the
authoritative claim in
`../e1000-evaluation-strategy/01-claim-and-evaluation-philosophy.md` §3, which
makes expressiveness and cross-OS portability the offensive claims and
performance a bounded guardrail.

## Status

DRAFT. Discussion-area skeleton stood up; load-bearing artifacts are
placeholders awaiting authoring and review. Driver-analysis task briefs are
draft, awaiting reviewer sign-off before the analyses are run.
