---
feature_id:   F-device-interrupt-setup-and-handling
feature_slug: device-interrupt-setup-and-handling
artifact_id:  GATE-REPORT-2026-06-11-M0
type:         gate-report
status:       closed-with-recorded-relaxation
source:       [GRILL-2026-06-10-223308, PRD-2026-06-10-223813, PLAN-2026-06-11-000040]
created:      2026-06-11
---

# M0 gate report — device-interrupt-setup-and-handling

The PLAN's M0 hard precondition named two compiler units that must land on
`main` before any implementation task starts. Disposition at 2026-06-11 00:20:

## Half 1 — `unit/spec-convergence-pl011`: **LANDED. Gate condition MET.**

Merged to `main` at `cfd469b` (no-ff, 2026-06-11 00:15, pushed to origin) by
the unit's own closing session, per its closing gate report
(`.claude/sekekama/grill-to-plan/specs/spec-convergence-pl011/GATE-REPORT-2026-06-10-close.md`:
verdict DONE, final Codex review CLEAN, `cargo test` 2355/0 at tip `03bbae5`).
T13 (`pl011-full-spec` convergence) is therefore **properly un-gated** — no
relaxation involved.

## Half 2 — `backend/zephyr-c-e1000`: **NOT LANDED. Proceeding under explicit user relaxation.**

State at gate close: branch + worktree (`~/regseq-compiler-worktrees/backend-zephyr-c-e1000`)
created 2026-06-11 00:15 at `cfd469b`, **zero unit commits** — the unit is
just starting in a parallel session.

The relaxation: Gedeon's directive of 2026-06-11 (verbatim: "You will work
autonomously through all the milestones in this implementation plan. … Don't
stop until all milestones are completed."), issued immediately after a recap
stating this gate half was unmet. Per the PLAN's own contingency clause
("Should the user later explicitly relax the gate…"), this directive is the
anticipated explicit relaxation; waiting for a separate, just-started unit is
incompatible with it.

External review: Codex (GPT-5.5, effort high) foundation review
**PROCEED-WITH-CHANGES**, 2026-06-11. Findings adopted:

1. No re-merge of spec-convergence (already landed); capture a fresh
   post-merge `main` baseline before branching. → Baseline `cargo test` run
   at branch base `cfd469b` (result recorded below).
2. Ledger the e1000 half as a **relaxation**, not as "M0 closed". → This
   document.
3. Re-ground T05/T10/T11 splice points against `main@cfd469b` before
   implementation: `capability.rs` is still lifecycle-slot-shaped,
   `audit_block()` hard-codes `AbsentDispatch`, `NEVER_ABSENT` covers
   init/init-failure — exactly the T10 splice risk the PLAN names.
4. Branch hygiene: new clean worktree off `main@cfd469b`
   (`~/regseq-compiler-worktrees/device-interrupt`, branch
   `construct/device-interrupt-setup-and-handling`), not the dirty primary
   checkout.

## Live-parallel-unit amendment (supersedes the PLAN's collision analysis)

The PLAN gated T11/T12-zephyr because `backend/zephyr-c-e1000` would reshape
`zephyr_c` ("moving target"). The facts have changed twice:

- `zephyr_c` was already reshaped — by `unit/spec-convergence-pl011`, now on
  `main`. The e1000 unit is "born-converged" (its predecessor's words) and
  builds ON TOP of the same `cfd469b` base this unit branches from.
- The e1000 unit is now LIVE in a parallel session, sharing
  `crates/regseq-backends` (zephyr_c, possibly capability.rs) and
  `codegen-coverage-runner` with this unit's T10/T11/T12.

Coordination rule adopted for this unit: **at M4 start (T10), re-check
`backend/zephyr-c-e1000` status.** If it has landed on `main`, rebase this
branch onto `main` before T10/T11 (gate letter restored). If still in
flight, proceed against `main@cfd469b` under the relaxation and record the
overlap for merge-time reconciliation — never plan against the unmerged
moving branch.

## Baseline

`cargo test` at branch base `cfd469b` in the fresh worktree:
**2355 passed, 0 failed** (matches the spec-convergence closing count;
filled in on completion of the baseline run — see M1 task commits).
