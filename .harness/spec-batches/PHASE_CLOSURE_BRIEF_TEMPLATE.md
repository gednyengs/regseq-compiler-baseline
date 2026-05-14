# Phase Closure Brief: <phase-tag>

Impl-side phase closure brief, filed when
`uv run python scripts/python/task.py phase-status <phase>` returns
PASS for a phase tag. Companion to but not a mirror of README §5.5
(which integrates spec-side artifacts at stage close); phase-1
implementation runs against the locked phase-1 spec and produces no
spec content to integrate, so this template is deliberately slimmer
than the spec-side closure protocol.

Five sections; each must be filled before the phase is declared
closed.

## Phase identification

State the phase tag and the spec sections this phase implemented.

Include:

- the literal `phase` value (e.g. `1a`, `1b`)
- the cumulative set of `spec_refs` covered by phase-tagged tasks,
  emittable via:
  `uv run python scripts/python/task.py list --phase <p> --json |
  python3 -c "import sys, json; refs=set(); [refs.update(json.loads(l)['spec_refs']) for l in sys.stdin if l.strip()]; [print(r) for r in sorted(refs)]"`

## Tasks in phase

Capture the output of:

```bash
uv run python scripts/python/task.py list --phase <p> --quiet
```

Every listed task must be `done` for `phase-status` to return PASS.

## Verification status

Capture the output of:

```bash
uv run python scripts/python/task.py phase-status <p>
```

VERDICT must be `PASS`. If the phase's closure depends on a
cumulative test suite (e.g., phase-1b's
DSL -> IR -> C -> captured-stdout suite), capture the workspace-wide
test command and its summary here as well.

## Phase-Close Review trail

Capture the `codex_runner.py` invocation(s) and verdict(s) used to
run the harness-side Phase-Close Review for this phase. Phase-Close
Review surveys the cumulative phase impl artifact (Rust code, test
output, `spec_refs` coverage against the locked spec); it is distinct
from README §5.6's spec-side Tier 3 (top-level post-merge review of
`spec.md`) and does not replace it.

Iterate review rounds to `VERDICT: APPROVED` per the per-tier
discipline in `.harness/SESSION.md`.

## Outbound expectations

List any entries this phase filed into later phases'
`.harness/expectations/<task-id>.md` files (forward refs from
in-phase tasks to downstream phase tasks). Announce the next phase
entry point:

- For phase-1a close: next phase opens at phase-1b.
- For phase-1b close: next phase opens at the first phase-2 stage
  (initial allocation per README §1.3: regseq-trait scaffolding +
  Lifecycle + driver-struct discovery + RegseqError).
- For phase-2 stages: next phase per the queue declared in README
  §1.3 and §2 "Domain extensions iterate per stage in phase 2".
