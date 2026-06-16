---
feature_id:   F-device-interrupt-setup-and-handling
feature_slug: device-interrupt-setup-and-handling
artifact_id:  GATE-REPORT-2026-06-11-close
type:         gate-report
status:       closed
source:       [GRILL-2026-06-10-223308, PRD-2026-06-10-223813, PLAN-2026-06-11-000040, GATE-REPORT-2026-06-11-M0]
created:      2026-06-11
---

# Closing gate-report — device-interrupt-setup-and-handling

**Unit verdict: DONE. All five milestones (M0–M5, tasks T01–T14) closed;
final holistic Codex review CLEAN ("I would not block merge to main").**

`cargo test` at tip: **2424 passed, 0 failed** (baseline 2355 at the branch
base `cfd469b`; every pre-existing golden byte-stable; +69 from the unit's
own tests). 20 commits on `construct/device-interrupt-setup-and-handling`,
one per task close plus review fixes; eval-side T14 commit `5edbc56` on the
eval construct branch.

## Milestone ledger (the per-milestone Codex loop, as directed)

| milestone | verdict | disposition |
|---|---|---|
| M0 gate | PROCEED-WITH-CHANGES | all four changes adopted (no re-merge; baseline; relaxation ledgered; splice re-grounding) |
| M1 (T01–T07) | FINDINGS-MAJOR | ProvenReachable hardened twice (exact-set + termination proof) → **RESOLVED** |
| M2 (T08) | FINDINGS-MAJOR | end-to-end present-but-unreachable warning pin → **RESOLVED** |
| M3 (T09) | FINDINGS-MAJOR | IRQF_NO_AUTOEN\|IRQF_SHARED is -EINVAL: shared=true emit-rejected + negative → **RESOLVED** |
| M4 (T10–T12) | FINDINGS-MINOR | stale-pin audit covers StructurallyAbsent; zephyr gates hardened; west irq leg → applied |
| M5 (T13–T14) + holistic | **CLEAN** | — |

## What was built (headline)

The first slice of construct #6, frozen surface with ZERO extensions: the
typed `Interrupt` + `#[rseq::isr]` → `IrqResult` surface flows recognition →
model (append-only discriminants + proof-grade arm reachability) → the
`[interrupt.<id>]` L2 tenant → THREE backends consuming ONE ordering source
(`interrupt_order`, R17/D14): generic_c (the C05 host line model, synchronous
fire, the `irq-pl011` golden = the C01 grammar observable), linux_c
(`request_irq(IRQF_NO_AUTOEN)`/gated `enable_irq`/manual
`disable_irq`→`free_irq`, irqreturn_t wrapper, kbuild-gated), zephyr_c
(`IRQ_CONNECT` without the stock enable, split enable, `irq_disable` + the
`RSEQ-ABSENT-OP` audit block at the deregister slot, west-gated). The
capability table grew its first non-lifecycle row
(`CAP-ZEPHYR-STATICIRQ-DEREGISTER-1`, `Disposition::StructurallyAbsent` —
O5) with the conformance schema widened (`structurally_absent_slots`). The
full C06 negative corpus (11 reject fixtures) + the R11 warn-on-unknown
shared-discipline warning. T14 handed Tier-2 off: whitelists, the
single-vCPU pin, the surface-convergence doc.

## Open-item disposition (O1–O5, all discharged)

- **O1** minted in T06: `IRQ fire line=<n> inert`, goldens assert positively.
- **O2** stayed free: `interrupt_order` is the harness-owned representation.
- **O3** minted in T03/T08 within `IRQ-ERR-*` (+`IRQ-WARN-SHARED-NOTHANDLED`).
- **O4** resolved fixture-local (as pre-registered).
- **O5** minted in T10: `Disposition::StructurallyAbsent`.

## Binding amendments (carried in the surface-convergence doc)

1. D07 synchronous fire supersedes the buildout "host thread" note.
2. **Shared-line registration**: `IRQF_NO_AUTOEN | IRQF_SHARED` is `-EINVAL`
   — the PLAN's flag formula is unlowerable; `shared = true` is
   emit-rejected on both OS backends pending a design amendment (owner: the
   shared-irq fixture unit, e1000).
3. **T13 residual**: the full-spec model-JSON suite never existed (orphaned
   legacy); interrupt-surface convergence delivered (canonical spellings,
   no-Debug); full-spec end-to-end blocked on clock/reset/pinctrl, pinned by
   `pl011_full_spec_irq_convergence.rs`.

## Gate coordination postscript

`backend/zephyr-c-e1000` remained at ZERO commits through unit close
(checked at M0 and at M4 start) — the recorded relaxation was never
exercised against live parallel work; the overlap is theoretical. That unit
inherits: the conformance-schema widening, the capability-row precedent, the
PCI-irq + shared-line amendments (its e1000 is the witnessed shared device).

## What the successor inherits

- The shared-line design amendment (registered-enabled + device-level
  masking is the stock idiom to ratify).
- The PCI irq leg (`pdev->irq`) behind an e1000 irq fixture.
- Option/array lowering (recognized whole; emit-rejected loudly).
- The Synchronization construct's obligation: lift the single-vCPU pin,
  wrappers acquire declared locks (D11/R18).
- The full-spec convergence flip when clock/reset/pinctrl land.

## Merge readiness

The branch is merge-ready (`--no-ff` per convention) at the user's call;
NOT merged or pushed by this unit (branch discipline). Eval-side: the design
docs + spec chain remain uncommitted in the eval worktree per standing
instruction; the T14 artifacts are committed there (`5edbc56`).
