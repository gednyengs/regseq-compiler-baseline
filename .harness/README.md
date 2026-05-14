# .harness/ -- Regseq Compiler Harness

The harness coordinates implementation work on the Regseq compiler: a
two-layer scaffolding that separates batch-level planning from
single-task execution, with a CLI (`task.py`) that mediates every
mutation of `tasks.json` and a small set of templates that constrain
how new work is authored. The planning tracker
(`task-spec-status.yaml`) is hand-edited per the rules in
`task-spec-workflow.md`; the CLI does not mediate tracker writes.

This README is the conceptual entry point. Detailed mechanics live in
the files this README points at; this file's job is to give a fresh
worker (human or agent) the right mental model in one read.

---

## Maintenance note

**If the harness architecture changes -- new CLI subcommands, schema
field additions, template structure changes, new artifact types, new
review tiers, or shifts in the lifecycle states -- update this README
in the same commit.** The README is the load-bearing orientation
document; drift here propagates to every cold session that lands in
the directory. Volatile details (exact CLI flags, exact section
counts, exact algorithm output formats) belong in the concrete files
this README points at; structural and conceptual content belongs
here.

---

## Two layers: planning (briefs) vs. execution (task specs)

The harness keeps batch-level planning and single-task execution
separate. Workers at one layer should not need to reason about the
other:

- **Planning layer**: briefs and the tracker. Defines what work
  batches exist and what each batch covers. State in
  `task-spec-status.yaml`.
- **Execution layer**: task specs and `tasks.json`. Defines what
  individual workers (human or agent) must build. State in
  `tasks.json`.

**One brief spawns N task specs** (typically 5-15 per brief). The
brief is authored and reviewed first; once approved, it gets
decomposed into executable task specs via `task.py add-task`.

| Aspect | Brief | Task spec |
|--------|-------|-----------|
| Scope | Batch of N coherent tasks | One isolated task |
| Tracked in | `task-spec-status.yaml` (planning) | `tasks.json` (execution) |
| Markdown lives at | `spec-batches/<id>.md` | `tasks/<group>/<NN-name>.md` |
| Defines public API? | No -- defers to task specs | Yes |
| Has Test Plan? | No -- defers to task specs | Yes |
| Has Quality Gate? | No | Yes |
| Decomposition guidance? | Yes ("split by file", etc.) | No -- already split |
| Review unit | Batch Review (opt-in per brief); collective Phase-Close Review at phase boundary | Task Review per task |
| Lifecycle | `not_ready` -> `ready_to_elaborate` -> `in_elaboration` -> `tasked` -> `archived`; `blocked` is a stop state reachable from any non-terminal | `pending` -> `in_progress` -> `done` / `failed` / `archived` |

### A note on review-tier names

The harness uses three review tiers named **Task Review**, **Batch
Review**, and **Phase-Close Review**. These names are *after* but
**not the same as** the spec-side tiers in
`docs/discussions/regseq-model/README.md` §5.6 (per-session /
cross-session / top-level post-merge). The spec-side tiers operate
on spec authoring; the harness-side tiers operate on implementation
against the locked spec. Do not conflate them. SESSION.md's Review
section is the canonical reference for the harness-side tiers.

---

## Briefs: the planning-layer artifact

A brief is a *batch planning contract*. It bounds the scope of
decomposition, names the spec surface the batch covers, and prevents
drift between batches. It is the unit of review at the planning
layer.

### Brief tracker metadata

One entry per brief in `task-spec-status.yaml`:

```yaml
- id: regseqmodel-phase-1a
  phase: "1a"
  title: RegseqModel bridge + pretty-print implementation
  status: ready_to_elaborate
  depends_on: []
  target_harness_groups: [regseq-model, regseq-driver]
  source_refs:
    - docs/discussions/regseq-model/spec.md
    - docs/discussions/regseq-model/pretty-printer.md
  summary: >
    One-paragraph batch description.
  brief: .harness/spec-batches/regseqmodel-phase-1a.md
  spec_authoring_notes:
    - constraints that shape the resulting task specs
  generated_task_ids: []
```

Field semantics live at `task-spec-status.yaml`'s top-level
`status_definitions` block and in `task-spec-workflow.md` (`How to
Use a Tracker Item` section). Valid statuses are enumerated in both
places.

### Brief markdown structure

Every brief in `.harness/spec-batches/<id>.md` must contain exactly
these 10 sections, in this order (per
`spec-batches/BRIEF_TEMPLATE.md`):

1. **Description** -- 2-4 paragraphs: what the batch is for,
   executable-impl vs spec-authoring, concrete outcome.
2. **Input Context** -- exact files a cold worker must read before
   starting (SESSION.md, workflow.md, status.yaml, the locked spec
   sections being implemented).
3. **Output Specification** -- exact files to create or modify;
   what to update when done; whether the output is executable
   tasks, harness metadata, a new brief, or some combination.
4. **Implementation Notes** -- the *decomposition contract*. Fixed
   decisions the worker must follow: how to split into tasks,
   naming constraints, sequencing, dependency constraints. This is
   where the brief says *how* to break the batch into task specs.
5. **Spec References** -- the full `<spec-file>#<anchor>` set this
   batch covers; per-task `spec_refs` are subsets. Authors drive
   this via `task.py read-spec-refs <spec-file>`.
6. **Phase** -- single phase tag (`1a`, `1b`, `2-mmio`, ...);
   inherited by every task this brief spawns.
7. **Review Tiers** -- table naming which harness-side review tiers
   apply at batch close (Task Review always; Batch Review opt-in
   for coherent cross-task features; Phase-Close Review mandatory
   at phase boundary).
8. **Non-Goals** -- explicit scope boundary; prevents the batch
   from absorbing adjacent work.
9. **Acceptance Criteria** -- concrete completion conditions
   (specific files created, tracker item statused,
   `generated_task_ids` recorded).
10. **Update Instructions** -- exactly how `task-spec-status.yaml`
    updates when the batch closes.

Pointers: `spec-batches/BRIEF_TEMPLATE.md` (canonical template),
`task-spec-workflow.md` (authoring rules and lifecycle transitions).

---

## Task specs: the execution-layer artifact

A task spec is a *single-task execution contract*. It defines
exactly what a worker must do for one task in isolation: the public
API to build, the tests to write, the acceptance gate. It is the
unit of work at the execution layer.

### Task spec `tasks.json` metadata

One entry per task in `tasks.json` (schema version 2):

```json
{
  "id": "regseq-model-100",
  "name": "Place and Projection",
  "spec": ".harness/tasks/regseq-model/100-place-and-projection.md",
  "status": "pending",
  "depends_on": ["regseq-model-99"],
  "artifacts": [],
  "fail_reason": null,

  "phase": "1a",
  "spec_refs": ["spec.md#19"],
  "spec_version": 1,
  "history": [
    { "ts": "2026-05-14T...", "event": "added", "spec_version": 1 }
  ]
}
```

`task.py show <id>` surfaces every field with the last 3 `history`
entries inline. Schema is managed by `task.py` and the
`migrate_tasks_v1_to_v2.py` script; do not edit `tasks.json`
directly.

### Task spec markdown structure

The canonical task spec at `tasks/<group>/<NN-name>.md` contains
these 8 sections in this order; new phase-1a/1b task specs should
follow this shape. (The 189 historical done examples are not
uniformly 8-section -- some omit Non-Goals, Review, or Quality
Gate. Treat the canonical shape as the forward-going contract.)

1. **Description** -- 1-2 paragraphs: what the task implements at a
   high level.
2. **Input Context** -- files the worker must read before starting
   (relevant spec sections, type definitions from earlier tasks).
3. **Implementation Notes** -- the heart of the spec. Typically
   includes:
   - **Public API** -- Rust signatures (struct fields, enum
     variants, function signatures) the worker must define.
     Copy-paste-able from the spec.
   - **Conventions** -- internal rules (serde shape, naming,
     invariants).
4. **Non-Goals** -- what this task explicitly does *not* cover
   (often "implemented by task N+1").
5. **Test Plan** -- explicit tiers expected per the harness-side
   discipline:
   - construction + serde round-trip unit tests on data shapes,
   - validator-pass coverage assertions (where applicable),
   - insta snapshot of pretty-printed form for at least one
     canonical instance.
   Phase-1b tasks add a fourth tier: captured-stdout vs. golden.
6. **Review** -- the Task Review invocation pattern (per
   `SESSION.md` Review section). The four-block `codex_runner.py`
   prompt template is reproduced here so the worker can pipe it
   directly.
7. **Quality Gate** -- per-task dev-time check commands that mirror
   what `task.py done` will verify at completion; workers run them
   during development to anticipate the verification pass without
   invoking the full `done` flow:
   ```
   cargo test -p <crate>
   cargo fmt --all -- --check
   cargo clippy -p <crate> -- -D warnings
   ```
   **The authoritative gate is `task.py done`**, which runs cleanup
   plus workspace-wide test/fmt/clippy per `SESSION.md` Testing
   Requirements. Quality Gate commands in the spec are informational
   guidance, not a separate gate.
8. **Acceptance Criteria** -- concrete completion conditions:
   files created, tests pass, Codex review APPROVED.

Pointers: `SESSION.md` (execution flow and Review tier details),
`tasks/<group>/<NN-name>.md` files (189 done examples retained as
historical reference).

---

## How briefs and task specs relate (worked example)

A hypothetical phase-1a brief covering `spec.md` §19 (Place and
projections), §20 (Operand vocabulary), §21 (Rvalue vocabulary):

**Brief at `.harness/spec-batches/regseqmodel-phase-1a.md`** carries:

- `Spec References`: `spec.md#19`, `spec.md#20`, `spec.md#21`
- `Implementation Notes`: "Decomposition: one task per spec section;
  each task lands as `regseq-model-<NN>-<topic>.md` under the
  `regseq-model` group."
- `Phase`: `1a`
- `Review Tiers`: Task Review (mandatory), Phase-Close Review
  (mandatory at phase-1a close), Batch Review (skip -- tasks are
  independent enough)

**Three task specs spawned from that brief**, each registered via
`task.py add-task --phase 1a --spec-refs ...`:

- `regseq-model-100`  Place + Projection                            `spec_refs: ["spec.md#19"]`
- `regseq-model-101`  Operand + Constant                            `spec_refs: ["spec.md#20"]`
- `regseq-model-102`  Rvalue + supporting types                     `spec_refs: ["spec.md#21"]`

The brief is the contract that says "these three tasks exist and
collectively cover §19-§21." The task specs are executable units
that define exactly what each task delivers in isolation. `task.py
spec-coverage docs/discussions/regseq-model/spec.md` cross-references
the cumulative per-task `spec_refs` arrays against the spec file's
heading anchors and reports gaps or drift.

---

## CLI surface (`scripts/python/task.py`)

Every mutation of `tasks.json` (execution state) routes through the
CLI; direct edits to `tasks.json` are forbidden. The planning tracker
(`task-spec-status.yaml`) is hand-edited per the rules in
`task-spec-workflow.md` and has no CLI mediator. Run `uv run python
scripts/python/task.py <cmd> --help` for flag-level details.

**Query / orientation:**

- `doctor` -- check prerequisites (toolchain, locked spec files,
  Python deps).
- `next-step` -- determine the next executable or task-spec action;
  cold-session entry point.
- `next` -- find next available executable task.
- `show <id>` -- show task details, including new fields (`phase`,
  `spec_refs`, `spec_version`, last 3 history entries).
- `status` -- summary table of task counts by status.
- `list [--status] [--phase] [--json] [--quiet]` -- list tasks with
  filters and output modes.
- `blocked` -- show tasks with unmet dependencies.

**Status transitions** (mutate `tasks.json` atomically):

- `start <id>` -- claim a task; transitions to `in_progress`.
  Surfaces inbound expectations if `expectations/<id>.md` exists.
- `done <id>` -- complete a task; runs cleanup + verification gate.
- `fail <id> --reason "..."` -- mark task as failed with reason.
- `reset <id> --reason "..." [--force]` -- recovery path back to
  `pending`.

**Write path** (canonical entry points for creating / amending
tasks):

- `add-task --id --name --spec --group [--depends-on ...]
  [--spec-refs ...] [--phase]` -- create a new task. Validates that
  `--spec` resolves to a regular file under
  `.harness/tasks/<group>/` (no traversal, no symlinks, no devices).
- `update-task-spec <id> [--reason "..."]` -- bump `spec_version`
  and append a `spec_edited` history entry; allowed regardless of
  status.
- `archive <ids...> --reason "..."` -- flip status to `archived`
  and set `phase: "archived"` atomically; appends `archived` history
  entry. Refuses already-archived or done tasks (use `reset --force`
  to revive a done task first).

**Analysis:**

- `ready-leaves [--phase]` -- pending tasks whose `depends_on` are
  all done, sorted.
- `phase-status <phase>` -- PASS / FAIL / unknown for a phase tag.
- `graph [--format mermaid|dot] [--phase] [--status]` -- dependency
  DAG export.
- `spec-coverage <spec-file> [--include-subsections]
  [--include-status]` -- coverage report (covered / uncovered /
  drift) between a spec file's heading anchors and tasks'
  `spec_refs`.
- `read-spec-refs <spec-file> [--include-subsections]` -- emit
  anchor slugs from a markdown spec file, one per line; the
  canonical source brief authors pipe from when filling
  `Spec References`.

---

## Directory layout

```
.harness/
  README.md                     -- this file (orientation + project doctrine)
  SESSION.md                    -- worker bootstrap (execution layer; carries project overview, conventions, testing requirements)
  task-spec-workflow.md         -- planning workflow (brief authoring)
  task-spec-status.yaml         -- planning tracker (briefs + lifecycle)
  tasks.json                    -- execution state (task entries + metadata)

  spec-batches/
    BRIEF_TEMPLATE.md           -- 10-section brief template
    PHASE_CLOSURE_BRIEF_TEMPLATE.md -- 5-section impl-side phase closure
    <batch-id>.md               -- brief markdowns

  tasks/<group>/                -- task spec markdowns, one per task
    <NN-name>.md

  expectations/                 -- impl-side inbound forward refs
    README.md                   -- file-shape / creation / consumption rules
    <target-task-id>.md         -- expectations filed against a target task

  archive/<date>-old-architecture/
    tasks.json.snapshot         -- byte-copy at refactor open
    task-spec-status.yaml.snapshot
    INVENTORY.md                -- audit tables (groups, non-done, tracker, ready leaves)
    MANIFEST.md                 -- enumerates moves and status flips
    spec-batches/               -- archived batch briefs
```

---

## Reading order for a fresh agent

1. **This README** -- conceptual entry point.
2. **`SESSION.md`** -- project overview, workspace, coding rules,
   testing requirements, worker bootstrap, Review tier discipline,
   Finishing flow. Read every session.
3. **`task-spec-workflow.md`** -- only if authoring a brief or
   decomposing a batch.
4. **If executing a task:**
   - Read the task spec at the path printed by `task.py show <id>`.
   - Read all files in the task spec's Input Context section.
   - If `task.py start <id>` prints `INBOUND_EXPECTATIONS:`, read
     that file first.
5. **If authoring a brief:**
   - Read `spec-batches/BRIEF_TEMPLATE.md`.
   - Read the relevant sections of
     `docs/discussions/regseq-model/{spec,pretty-printer,generic-c-backend}.md`
     (the locked phase-1 spec).
   - Use `task.py read-spec-refs` to enumerate anchors for
     `Spec References`.
6. **On demand:** `task-spec-status.yaml` (planning state),
   `tasks.json` via `task.py show` (execution state),
   `archive/<date>-old-architecture/MANIFEST.md` (pre-refactor
   history).

---

## What this README is NOT

- **Not the binding methodology.** Project-wide binding rules live
  at `docs/discussions/regseq-model/README.md` §2 and §5. The
  harness implements that discipline; it does not revise it.
- **Not the CLI manual.** `task.py <cmd> --help` is authoritative
  for flag-level detail.
- **Not the spec.** Implementation targets the locked phase-1 spec
  at `docs/discussions/regseq-model/{spec,pretty-printer,generic-c-backend}.md`.
- **Not append-only.** Update in place when the harness architecture
  changes.
