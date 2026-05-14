# Archive Manifest: 2026-05-14 (old architecture)

This manifest enumerates every file moved and every status flip applied
during Step 8 of the harness refactor (plan v4). The snapshots in
`tasks.json.snapshot` and `task-spec-status.yaml.snapshot` (this
directory) preserve the pre-archive state byte-for-byte; rollback
instructions are in the Step 8 commit message.

## Section 1. Batch briefs moved (OLD_SPEC_BRIEF)

Moved from `.harness/spec-batches/` to `.harness/archive/2026-05-14-old-architecture/spec-batches/`
(this directory).

| File | Old path | New path |
| ---- | -------- | -------- |
| `device-generality-cleanup.md` | `.harness/spec-batches/device-generality-cleanup.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/device-generality-cleanup.md` |
| `pl011-fixture-cleanup.md` | `.harness/spec-batches/pl011-fixture-cleanup.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/pl011-fixture-cleanup.md` |
| `regseqmodel-phase-0.md` | `.harness/spec-batches/regseqmodel-phase-0.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/regseqmodel-phase-0.md` |
| `regseqmodel-phase-1.md` | `.harness/spec-batches/regseqmodel-phase-1.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/regseqmodel-phase-1.md` |
| `regseqmodel-phase-2.md` | `.harness/spec-batches/regseqmodel-phase-2.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/regseqmodel-phase-2.md` |
| `regseqmodel-phase-3a.md` | `.harness/spec-batches/regseqmodel-phase-3a.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/regseqmodel-phase-3a.md` |
| `regseqmodel-phase-3b.md` | `.harness/spec-batches/regseqmodel-phase-3b.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/regseqmodel-phase-3b.md` |
| `regseqmodel-phase-3c.md` | `.harness/spec-batches/regseqmodel-phase-3c.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/regseqmodel-phase-3c.md` |
| `regseqmodel-phase-3d.md` | `.harness/spec-batches/regseqmodel-phase-3d.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/regseqmodel-phase-3d.md` |
| `regseqmodel-phase-4.md` | `.harness/spec-batches/regseqmodel-phase-4.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/regseqmodel-phase-4.md` |
| `regseqmodel-phase-5.md` | `.harness/spec-batches/regseqmodel-phase-5.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/regseqmodel-phase-5.md` |
| `regseqmodel-phase-6.md` | `.harness/spec-batches/regseqmodel-phase-6.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/regseqmodel-phase-6.md` |
| `regseqmodel-phase-7.md` | `.harness/spec-batches/regseqmodel-phase-7.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/regseqmodel-phase-7.md` |
| `regseqmodel-phase-8.md` | `.harness/spec-batches/regseqmodel-phase-8.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/regseqmodel-phase-8.md` |
| `regseqmodel-phase-9.md` | `.harness/spec-batches/regseqmodel-phase-9.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/regseqmodel-phase-9.md` |
| `user-data-init-path-a.md` | `.harness/spec-batches/user-data-init-path-a.md` | `.harness/archive/2026-05-14-old-architecture/spec-batches/user-data-init-path-a.md` |

Total: **16** batch briefs.

## Section 2. Tracker items flipped (OLD_TRACKER_ITEM)

`.harness/task-spec-status.yaml` -- every tracker item's `status` field
flipped from its prior value to `archived` with a trailing comment
`# superseded by phase-1a/1b plan in locked spec`. No item content
removed; the YAML body is preserved as a historical record of the
old-architecture planning surface.

| Tracker item | Old status | New status |
| ------------ | ---------- | ---------- |
| `regseqmodel-phase-0` | tasked | archived |
| `regseqmodel-phase-1` | tasked | archived |
| `regseqmodel-phase-2` | tasked | archived |
| `regseqmodel-phase-3a` | tasked | archived |
| `regseqmodel-phase-3b` | tasked | archived |
| `regseqmodel-phase-3c` | tasked | archived |
| `regseqmodel-phase-3d` | tasked | archived |
| `regseqmodel-phase-4` | tasked | archived |
| `regseqmodel-phase-5` | tasked | archived |
| `regseqmodel-phase-6` | tasked | archived |
| `regseqmodel-phase-7` | tasked | archived |
| `device-generality-cleanup` | tasked | archived |
| `regseqmodel-phase-8` | ready_to_elaborate | archived |
| `regseqmodel-phase-9` | not_ready | archived |
| `pl011-fixture-cleanup` | ready_to_elaborate | archived |
| `user-data-init-path-a` | tasked | archived |

Total: **16** tracker items.

## Section 3. Executable tasks archived (OLD_SPEC_EXECUTABLE_TASK)

Via `task.py archive` (canonical write path; atomic per-task update
with `phase: "archived"` and an `archived` history entry citing the
sweep reason).

| Task ID | Group | Old status | New status |
| ------- | ----- | ---------- | ---------- |
| `regseq-driver-35` | `regseq-driver` | pending | archived |
| `regseq-model-63` | `regseq-model` | pending | archived |
| `regseq-backends-02` | `regseq-backends` | pending | archived |
| `regseq-lowering-11` | `regseq-lowering` | pending | archived |
| `regseq-linux-backend-04` | `regseq-linux-backend` | pending | archived |
| `regseq-linux-backend-05` | `regseq-linux-backend` | pending | archived |
| `track-a-validation-07` | `track-a-validation` | pending | archived |
| `track-a-validation-08` | `track-a-validation` | pending | archived |
| `pl011-linux-validation-11` | `track-a-validation` | in_progress | archived |
| `track-a-validation-11` | `track-a-validation` | pending | archived |
| `track-a-validation-12` | `track-a-validation` | pending | archived |
| `track-a-validation-13` | `track-a-validation` | pending | archived |
| `track-a-validation-14` | `track-a-validation` | pending | archived |

Total: **13** executable tasks.

## Section 4. Reason

All entries share the same reason: **old-spec; superseded by phase-1a/
1b plan in locked spec**. The locked phase-1 spec lives at
`docs/discussions/regseq-model/{spec,pretty-printer,generic-c-backend}.md`
(README §1.3). Phase-1 implementation is split into phase-1a
(RegseqModel + pretty-print) followed by phase-1b (generic-C codegen).
None of the archived material targets the new spec; some entries may
be revived as inputs to phase-2 (Linux backend / domain extensions) via
`task.py reset --force` if their content remains applicable.

## Section 5. Non-touched state

- The 189 `done` executable tasks remain in `.harness/tasks.json` as
  the project audit trail. Their spec markdowns under
  `.harness/tasks/<group>/` are unchanged.
- `.harness/spec-batches/BRIEF_TEMPLATE.md` and
  `.harness/spec-batches/PHASE_CLOSURE_BRIEF_TEMPLATE.md` remain in
  place as the live templates for the next phase-1a/1b authoring step.
- `.harness/tasks/<group>/` directories and the executable task spec
  markdowns for archived entries stay at their referenced paths so
  `task.py show <archived-id>` continues to resolve.
