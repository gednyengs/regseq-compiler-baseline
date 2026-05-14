# Pre-Refactor Inventory

Generated: 2026-05-14 (Step 1 of harness refactor plan v4).

This file is the audit artifact for what `.harness/` looked like at the
moment the refactor opened. It is generated; do not hand-edit.

Snapshots in this directory:

- `tasks.json.snapshot` -- byte-copy of `.harness/tasks.json` at refactor open.
- `task-spec-status.yaml.snapshot` -- byte-copy of `.harness/task-spec-status.yaml` at refactor open.

## Table A. Groups by done / non-done counts

| Group | Done | Non-done |
| ----- | ---- | -------- |
| `regseq-config` | 2 | 0 |
| `cargo-regseq` | 5 | 0 |
| `regseq-driver` | 34 | 1 |
| `regseq-diagnostics` | 1 | 0 |
| `regseq-model` | 58 | 1 |
| `regseq-macros` | 4 | 0 |
| `regseq-analysis` | 6 | 0 |
| `regseq-backends` | 2 | 1 |
| `regseq-facade` | 24 | 0 |
| `regseq-lowering` | 11 | 1 |
| `regseq-linux-backend` | 6 | 2 |
| `regseq-zephyr-backend` | 0 | 0 |
| `track-a-pl011` | 7 | 0 |
| `track-a-e1000` | 0 | 0 |
| `track-a-validation` | 8 | 7 |
| `source-to-extraction` | 12 | 0 |
| `extraction-remediation` | 8 | 0 |
| `mir-schema-refactor` | 1 | 0 |
| `track-a-eval` | 0 | 0 |
| **TOTAL (19 groups)** | **189** | **13** |

## Table B. Non-done executable tasks (13)

Classification is informational per refactor plan v4. Step 8 archives all
non-done tasks uniformly regardless of classification; future phase-2 stages
may revive specific tasks via `task.py reset --force` if their content
remains applicable.

| Task ID | Status | Group | Deps (status) | Classification |
| ------- | ------ | ----- | ------------- | -------------- |
| `regseq-driver-35` | pending | `regseq-driver` | `track-a-validation-12` [pending] | OLD_SPEC |
| `regseq-model-63` | pending | `regseq-model` | `regseq-lowering-11` [pending] | OLD_SPEC |
| `regseq-backends-02` | pending | `regseq-backends` | `regseq-model-60` [done]<br>`regseq-model-64` [done]<br>`regseq-linux-backend-06` [done]<br>`regseq-linux-backend-07` [done]<br>`regseq-linux-backend-08` [done]<br>`regseq-lowering-12` [done] | OLD_SPEC |
| `regseq-lowering-11` | pending | `regseq-lowering` | `track-a-validation-13` [pending] | OLD_SPEC |
| `regseq-linux-backend-04` | pending | `regseq-linux-backend` | `regseq-backends-02` [pending]<br>`regseq-backends-03` [done]<br>`regseq-linux-backend-03` [done] | OLD_SPEC |
| `regseq-linux-backend-05` | pending | `regseq-linux-backend` | `regseq-backends-02` [pending]<br>`regseq-linux-backend-03` [done]<br>`regseq-linux-backend-04` [pending]<br>`regseq-linux-backend-06` [done]<br>`regseq-linux-backend-07` [done]<br>`regseq-linux-backend-08` [done] | DEFERRED_LINUX_BACKEND |
| `track-a-validation-07` | pending | `track-a-validation` | `pl011-linux-validation-11` [in_progress] | DEFERRED_LINUX_BACKEND |
| `track-a-validation-08` | pending | `track-a-validation` | `track-a-validation-07` [pending] | DEFERRED_LINUX_BACKEND |
| `pl011-linux-validation-11` | in_progress | `track-a-validation` | `regseq-linux-backend-02` [done]<br>`regseq-driver-23` [done] | DEFERRED_LINUX_BACKEND |
| `track-a-validation-11` | pending | `track-a-validation` | `regseq-model-60` [done] | OLD_SPEC |
| `track-a-validation-12` | pending | `track-a-validation` | `track-a-validation-11` [pending]<br>`regseq-linux-backend-05` [pending] | OLD_SPEC |
| `track-a-validation-13` | pending | `track-a-validation` | `regseq-driver-35` [pending] | OLD_SPEC |
| `track-a-validation-14` | pending | `track-a-validation` | `regseq-linux-backend-06` [done]<br>`regseq-linux-backend-07` [done]<br>`regseq-lowering-12` [done]<br>`regseq-lowering-11` [pending] | DEFERRED_LINUX_BACKEND |

## Table C. Tracker items (16)

| Tracker ID | Phase | Status | Deps |
| ---------- | ----- | ------ | ---- |
| `regseqmodel-phase-0` | 0 | tasked | (none) |
| `regseqmodel-phase-1` | 1 | tasked | `regseqmodel-phase-0` |
| `regseqmodel-phase-2` | 2 | tasked | `regseqmodel-phase-0`, `regseqmodel-phase-1` |
| `regseqmodel-phase-3a` | 3 | tasked | (none) |
| `regseqmodel-phase-3b` | 3 | tasked | (none) |
| `regseqmodel-phase-3c` | 3 | tasked | (none) |
| `regseqmodel-phase-3d` | 3 | tasked | `regseqmodel-phase-3b` |
| `regseqmodel-phase-4` | 4 | tasked | `regseqmodel-phase-3a`, `regseqmodel-phase-3b`, `regseqmodel-phase-3c`, `regseqmodel-phase-3d` |
| `regseqmodel-phase-5` | 5 | tasked | `regseqmodel-phase-1`, `regseqmodel-phase-2`, `regseqmodel-phase-4` |
| `regseqmodel-phase-6` | 6 | tasked | `regseqmodel-phase-5` |
| `regseqmodel-phase-7` | 7 | tasked | `regseqmodel-phase-6` |
| `device-generality-cleanup` | post-7 | tasked | `regseqmodel-phase-7` |
| `regseqmodel-phase-8` | 8 | ready_to_elaborate | `regseqmodel-phase-7` |
| `regseqmodel-phase-9` | 9 | not_ready | `regseqmodel-phase-8` |
| `pl011-fixture-cleanup` | post-7 | ready_to_elaborate | `user-data-init-path-a` |
| `user-data-init-path-a` | post-7 | tasked | (none) |

## Table D. Currently ready leaves (2)

Pending tasks whose dependencies are all `done` at refactor-time. After
Step 8 archives all 13 non-done tasks, this table goes to empty -- the
intended phase-1a/1b launch state.

| Task ID | Group |
| ------- | ----- |
| `regseq-backends-02` | `regseq-backends` |
| `track-a-validation-11` | `track-a-validation` |

---

Acceptance for Step 1 of the refactor:

- `diff .harness/tasks.json .harness/archive/2026-05-14-old-architecture/tasks.json.snapshot` is empty.
- `diff .harness/task-spec-status.yaml .harness/archive/2026-05-14-old-architecture/task-spec-status.yaml.snapshot` is empty.
- This `INVENTORY.md` exists and is non-empty.
