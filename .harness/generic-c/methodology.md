# Generic-C workstream — methodology & operating manual

The **durable process-side reference** for the generic-C backend workstream:
the plan→spec→implement→assess loop, the task-spec template, the Codex review
recipe, tooling, conventions, the cold-start reading order, and the
first-action playbook. This is the "how we work" half of what used to live in
`.harness/HANDOFF.md` (the project decisions/state half is
`../../docs/discussions/generic-c-backend/decisions-and-state.md`; the volatile
session state is now in `.handoff/`, managed by the session-handoff plugin).

**Authoritative general-harness docs this builds on** (read for the broader
machinery; this doc is the workstream-specific adaptation):
- `../README.md` — the harness mental model (planning vs. execution layers,
  `task.py`, templates).
- `../task-spec-workflow.md` — tracker semantics, brief/spec authoring + update
  rules, phase closure.

> Update this doc only on a **methodology change** (it is otherwise stable).

---

## Handoff model (session-handoff plugin)

Session state lives in **`.handoff/<YYYY-MM-DD-HHMMSS>.md`** (project root),
managed by the `session-handoff` plugin — timestamped per-save snapshots, newest
wins, committed with the repo. The plugin's hooks auto-inject the latest handoff
at SessionStart, write a safety snapshot before compaction, and nudge to fill it.

The plugin's per-save template is intentionally generic. **The durable, must-
retain context lives in two version-controlled docs, and every saved handoff's
`## Resume instructions` (plus its top one-line note, for the 6 KB SessionStart
window) points at them:**
- `.harness/generic-c/methodology.md` — **this file** (process / how we work).
- `../../docs/discussions/generic-c-backend/decisions-and-state.md` — decisions
  locked / phase arc / orientation / file anchors.

So a `/save` captures the volatile state + the pointers; the heavy context is
retained in these two docs, not regenerated. (Closed workstreams + the
pre-2026-05-27 single-file HANDOFF live in `../archive/`.)

---

## Plan → spec → implement → assess loop

1. **Plan-first design doc** (pre-spec) → `docs/discussions/generic-c-backend/`
   or `.harness/audits/`: resolve forks, HEAD-grounded; Codex-review.
2. **Spec delta** into the canonical `generic-c-backend.md` (the 8-section task
   template below), Codex-reviewed (most specs take 2–4 rounds).
3. **File** via `task.py add-task` (see Tooling); commit + push; implementer
   picks up.
4. **Assess** the implementer's close (build/fmt/clippy/test + the codegen
   harness + diff-scope), trust-but-verify; **soundness pass via Codex on any
   load-bearing change** ([[assessment-soundness-pass]]). A soundness pass can
   also *over*-flag — verify findings against the relevant invariant (e.g. the
   discovery-roots invariant) and empirically before acting.

### The 8-section task-spec template

1. Description · 2. Input Context (design source / spec anchors / code anchors /
prior tasks / test surface) · 3. Implementation Notes (Change A/B/C…) ·
4. Non-Goals · 5. Test Plan · 6. Review (files / reviewer focus / four-block
adversarial prompt) · 7. Quality Gate · 8. Acceptance Criteria (numbered,
concrete).

### Codex invocation (canonical)

Use the **`codex-runner` Skill** (single source of truth; bundled runner is pure
stdlib — plain `python3`, no `uv run`). The repo's local
`scripts/python/codex_runner.py` is retired.

```bash
cat <<'EOF' | python3 "/home/nyengele/.claude/skills/codex-runner/scripts/codex_runner.py" --timeout 900 --message-only
<task>
... what to review/identify; scope boundaries; files to read ...
</task>

<grounding_rules>
- Cite file:line for every claim. Mark hypotheses as hypotheses.
- Don't speculate about code/spec you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
<per-check fields with PASS/FAIL + citation>
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
EOF
```

Bash `timeout` ≥ `950000` (Codex `--timeout 900` + margin). `--effort high`
default. Codex empirically probes code (runs `cargo`/tools) — trust findings.
Flags: `--message-only`, `--no-yolo`, `--model`, `--effort`, `--cwd`.

### Commit conventions

- Conventional prefixes (`feat:`, `fix:`, `gc-NN:` / group prefix, `chore:`).
- **No author attributions** (per global CLAUDE.md §1.3). HEREDOC for multi-line
  bodies.
- Commit/push only when the user asks; the project commits to `main` directly
  (single-architect workflow). The handoff snapshot is saved in the same session
  that did the work.

---

## Tooling specifics

### `task.py add-task`

```bash
uv run python scripts/python/task.py add-task \
  --id <group>-NN --name "<title>" \
  --spec .harness/tasks/<group>/NN-<slug>.md \
  --group <group> --depends-on <prev> --phase 1b
```

### Toolchain

- Pinned `nightly-2026-02-01` per `rust-toolchain.toml`.
- Repo uses **uv** for Python; test/dev deps via `uv add --dev`; tool runtimes
  stay stdlib-only ([[uv-python-tooling]]).
- Workspace members registered in `Cargo.toml` (alphabetical).

### Cargo test gotcha

`cargo test --workspace` in parallel mode can hit a rustc-driver stderr
interleaving quirk (E0463). Use
`RUST_TEST_THREADS=1 cargo test --workspace -- --test-threads=1` OR test
individual crates. Assessing an implementer close concurrently editing the tree?
verify the committed SHA in an **isolated `git worktree`** so the build isn't
polluted.

---

## Constraints + gotchas

- **No defensive fallbacks** (memory `feedback_no_defensive_fallbacks`).
- **No author attributions** in commits (global CLAUDE.md).
- **Playground is experimentation space** — never modify/commit
  `tests/fixtures/playground*` or `crates/regseq-driver/tests/fixtures/`.
- **The model is post-mono + rustc-free** — the backend walks `instances` and
  trusts layout/C-ABI as bridge certificates (don't recompute).
- **`FieldLayout` is indexed by source/MIR field order**, not memory order;
  offsets are from the containing aggregate.
- **Every interned nominal `Ty` gets an `InstanceKind` root** (bridge
  `nominal_adt_instances_pass`, `crates/regseq-driver/src/bridge/mod.rs:2375`),
  and `discover()` roots on all of them — so an aggregate reachable only behind a
  pointer is still named (no `NameTable` panic). Useful when triaging
  discovery/naming soundness claims.
- Run any new Python tooling's tests (`uv run pytest …`) after changes.

---

## Reading order for cold start

1. **The latest `.handoff/*.md`** (auto-injected at SessionStart) — the volatile
   "where we left off" + the pointers below.
2. **This file** — methodology / how we work.
3. **`../../docs/discussions/generic-c-backend/decisions-and-state.md`** —
   locked decisions / phase arc / orientation / file anchors.
4. Auto-memory (loads automatically) — role + binding rules + the
   bridge-feature-complete-gate milestone.
5. `docs/discussions/generic-c-backend/roadmap.md` — phase sequence + exit gates
   + the front-end↔backend dependency model.
6. **`docs/discussions/regseq-model/generic-c-backend.md` §§1–17 — the canonical
   C-emission spec; the COMPLETE input contract** (read before writing any task
   or code).
7. The Phase-1 plan docs `docs/discussions/generic-c-backend/phase-1/`: Doc 1
   (matrix + gap list), Doc 2 (D1–D12 decisions), Doc 3 (verification flow).
8. Only if needed: the archived bridge HANDOFF
   (`../archive/2026-05-26-bridge-coverage-runner/HANDOFF.md`) for methodology
   depth or front-end questions.

---

## First-action playbook (by user input)

**"Let's continue / what's next"** → read the latest `.handoff/` snapshot → its
Next steps. (As of the last close: all filed generic-C tasks — a, b, c1, 07, c2,
09 — are done + assessed; the next task to DRAFT is **d (bodies)** — §7.1
terminators / §10 Rvalue / §11 casts / §12 Const / §4-§5 places-operands; d must
also widen discovery roots to signature/local/static types; d→e is the critical
path to the first green fixture.)

**"Implementer is done"** → `git fetch && git log --oneline HEAD..origin/main` →
`git pull --ff-only` → read the task spec + any expectations → verify (build /
fmt / clippy / test / **codegen harness run**; isolated worktree if the tree is
concurrently edited) → diff-scope check → soundness Codex pass on load-bearing
changes (verify its findings before acting) → update the project doc + save a
`.handoff/` snapshot → commit.

**"File a new task"** → 8-section template → Codex (2–4 rounds) → apply fixes →
`task.py add-task` → commit → save a `.handoff/` snapshot.

**"Compact / hand off"** → `/session-handoff:save` (or the plugin's PreCompact
hook) — the snapshot captures the volatile state + points here and to the
project doc; the durable context is already in those two docs.
