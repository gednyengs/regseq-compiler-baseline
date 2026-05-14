# Inbound Expectations

Impl-side companion to the stage-side "Forward expectations" rule in
`docs/discussions/regseq-model/README.md` §2. The spec-side mechanism
files forward refs at `stage-N/expectations.md`; this directory holds
the parallel mechanism for executable tasks.

When a task surfaces a concern that an earlier-numbered task must
account for, the earlier task's worker needs a structured place to
discover the concern when they claim the task. This directory is that
place.

## File shape

One file per target task. Path:
`.harness/expectations/<target-task-id>.md`.

Each file's content:

```markdown
# Inbound expectations: <target-task-id>

## From <originating-task-id> (<ISO-Z timestamp>)

<Concern body: what the originating task discovered that the target
task must address. Cite spec sections, code paths, or invariants as
needed.>

## From <originating-task-id> (<ISO-Z timestamp>)

<Another concern, same shape.>
```

One `##`-level subsection per inbound expectation. Subsections
accumulate in chronological order; the target-task worker reads them
top-to-bottom on claim.

## Creation rule

Any task may file an expectation against any later-numbered task by
creating or appending to the target's file. Lazy creation: the file
is created at the first inbound expectation, not preemptively. If
the file already exists, append a new `##`-level subsection rather
than rewriting existing content -- earlier expectations remain
visible as historical context.

The originating task should commit the new expectations file as
part of its own commit, with a message body that names which target
task is affected.

## Consumption rule

`uv run python scripts/python/task.py start <target-task-id>` prints
`INBOUND_EXPECTATIONS: <path>` to stdout when the file exists. The
worker reads the file before beginning implementation (per
`.harness/SESSION.md` step 4). Expectations are not formally
acknowledged in any tracker; the worker tracks resolution in their
own commits and the file's content stands as historical record.

If an expectation is no longer applicable (e.g., the originating
task's concern was addressed at a different layer), the target-task
worker may leave the entry as-is or annotate it with a `> resolved:
<note>` quote line. The file is not deleted on resolution; the
historical trail is preserved.
