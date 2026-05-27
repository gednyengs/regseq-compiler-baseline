# `.harness/generic-c/` — generic-C backend working state

Harness-side **working state** for the generic-C backend workstream:
live ledgers, status trackers, and task-spec scratchpads pertaining to
implementation. This is the day-to-day operational counterpart to the
design home at `docs/discussions/generic-c-backend/`.

## docs vs. harness split

- **Design / specs / the codegen-coverage matrix** (human-owned, durable)
  → `docs/discussions/generic-c-backend/`, plus the canonical C-emission
  spec `docs/discussions/regseq-model/generic-c-backend.md`.
- **Live derived state** (the codegen analog of the bridge workstream's
  `coverage_ledger.{json,md}` — pass/fail per construct, drift, verdict),
  working trackers, and in-progress task-spec drafts → **here**.

## Task specs and `task.py`

Task specs that get *registered* (via
`uv run python scripts/python/task.py add-task --group <g> --spec <path>`)
live under `.harness/tasks/<group>/` so `tasks.json` stays the canonical
task DB — consistent with every prior workstream. Use this folder for
**scratchpads and drafts** that incubate before promotion into a
registered task spec.

## Entry points

- Session-to-session resume: the latest **`.handoff/<timestamp>.md`**
  snapshot (session-handoff plugin), which points at the two durable
  workstream-reference docs:
  - **`.harness/generic-c/methodology.md`** — process / how we work.
  - **`docs/discussions/generic-c-backend/decisions-and-state.md`** —
    locked decisions / phase arc / orientation / file anchors.
- Design HQ: `docs/discussions/generic-c-backend/`.
- Prior (bridge) workstream record, now closed:
  `.harness/archive/2026-05-26-bridge-coverage-runner/HANDOFF.md`. The
  pre-2026-05-27 single-file generic-C handoff is at
  `.harness/archive/2026-05-27-generic-c-handoff.md`.
