# RALPH.md -- Overnight Codex Orchestration

You (Claude Code) are picking this up in a fresh session as the
orchestrator of an overnight Codex run. This file is the complete
operating manual. You do not need to read `.harness/SESSION.md` --
that file is for Codex.

## 1. Your Role

You are the **orchestrator**, not the implementer.

- You launch ONE Codex run per harness task, wait for it to finish,
  parse the result, and decide whether to continue.
- You do NOT write code, run tests, run reviews, or commit. Codex
  does all of that inside each run.
- You do NOT modify `.harness/tasks.json` or
  `.harness/task-spec-status.yaml` directly. The harness's `task.py`
  is the only writer.

Working directory: `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos`

## 2. State Lives Outside Chat

Do not maintain in-memory state. After any compaction or restart,
re-derive state from disk:

- Total progress: `uv run python scripts/python/task.py status`
- Recent completions (task IDs + SHAs): `git log --oneline -30`
- Next executable task: `uv run python scripts/python/task.py next-step`
- Working-tree cleanliness: `git status --porcelain`

Every completed task is a commit with subject
`<task-id>: <description>` -- that commit log IS the durable progress
record.

## 3. The Loop

Repeat for each task until a halt condition fires:

1. **Check halt conditions** (Section 5) before launching.

2. **Verify clean tree.** Run `git status --porcelain`. If non-empty,
   run `git stash push -m "ralph-auto-stash <timestamp>"` and log it.
   If stash fails, HALT.

3. **Confirm there is an executable task.** Run
   `uv run python scripts/python/task.py next-step`. If the output
   indicates a task-spec batch, brief-authoring batch, or no available
   action, HALT (nothing more to do tonight).

4. **Launch Codex in the background** using the exact command in
   Section 4. Use the Bash tool with `run_in_background: true` and
   `timeout: 600000` (the Bash tool's max).

5. **Wait for the completion notification.** It will arrive as a user
   message containing `<task-notification>...</task-notification>`.
   Do NOT poll or read `/tmp/codex_task_run.json` mid-flight.

6. **Read the result.** Read `/tmp/codex_task_run.json`. The
   `final_message` field is Codex's structured output (TASK_ID,
   STATUS, etc.). The wrapper JSON also has `exit_code`,
   `input_tokens`, `output_tokens`.

7. **Apply halt conditions to the result** (Section 5).

8. **Report briefly to the user** (one-paragraph summary: task ID,
   status, commit SHA, files changed) so the morning-user can scan
   chat history. Then loop.

## 4. The Codex Launch Command

Run this verbatim via the Bash tool with `run_in_background: true`:

```bash
cat <<'CODEX_PROMPT_EOF' | uv run python scripts/python/codex_runner.py --timeout 3600 > /tmp/codex_task_run.json 2> /tmp/codex_task_run.err
<task>
You are a worker agent for the Regseq compiler project. Your job in this
invocation is to execute exactly ONE harness task end-to-end and stop.

Procedure (canonical source: .harness/SESSION.md -- read it first):
1. Run `uv run python scripts/python/task.py doctor`.
2. Run `uv run python scripts/python/task.py next-step` to identify the
   next executable task. If `next-step` reports a task-spec batch or
   brief-authoring batch instead of an executable task, STOP and report
   that fact -- do not author specs or briefs in this invocation.
3. Claim the task with `uv run python scripts/python/task.py start <id>`
   and read the spec markdown plus every file listed in the Input
   Context section, plus any INBOUND_EXPECTATIONS file printed by
   `start`.
4. Implement the task following SESSION.md steps 6-10: public API stubs
   with `todo!()` first, tests against the stubs, then the real
   implementation. Run the test command from the task spec until all
   tests pass.
5. Run the Task Review loop (SESSION.md steps 11-13): invoke
   `scripts/python/codex_runner.py` per the template in SESSION.md
   section "Task Review", address every cited fix, and re-invoke until
   the verdict is APPROVED. Recursive Codex invocation is expected and
   fine. Do NOT skip this loop.
6. Finish per SESSION.md steps 14-16: `cargo fmt`, `cargo test
   --workspace`, `uv run python scripts/python/task.py done <id>`,
   then `git add <files>` and `git commit -m "<task-id>: <summary>"`.
   No author attribution in the commit message.
7. STOP. Do NOT run `task.py next-step` again. Do NOT pick up a
   follow-up task. Do NOT author specs or briefs.

You are one-shot: there is no session continuation after this run.
Everything above must happen in this single invocation without pausing
or waiting for input. If a step fails irrecoverably, run
`uv run python scripts/python/task.py fail <id> --reason "..."` and
report the failure -- do not silently abandon the task.

Scope boundaries (out of scope for this run):
- Any task other than the one returned by `next-step`.
- Modifications to crates not permitted by the task spec.
- Edits to .harness/tasks.json or .harness/task-spec-status.yaml
  directly (use `task.py` subcommands).
- Spec or brief authoring.
- Phase-Close Review (separate trigger).
</task>

<grounding_rules>
- Treat .harness/SESSION.md as authoritative for procedure; this prompt
  only narrows scope, it does not override SESSION.md.
- Treat the task spec markdown as authoritative for what to build,
  which files to touch, and the test plan.
- Cite file:line when you reference code in your final report.
- Do not speculate about code you did not read.
</grounding_rules>

<structured_output_contract>
At the end of the run, emit a final message in exactly this shape:

  TASK_ID: <id>
  STATUS: COMPLETED | FAILED
  CODEX_REVIEW: APPROVED (after N iterations) | N/A if FAILED
  COMMIT: <short sha> <subject line>   # omit if FAILED
  FILES_CHANGED:
    - <path>
    - <path>
  SUMMARY: <<=80-word description of what was built and verified>
  FAILURE_REASON: <only if STATUS = FAILED, the reason passed to task.py fail>

Total final message <= 300 words. No preamble before TASK_ID.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, execute, stop.
If something is genuinely ambiguous, pick the most spec-faithful
interpretation and note the choice in SUMMARY.
</default_follow_through_policy>
CODEX_PROMPT_EOF
```

`/tmp/codex_task_run.json` is overwritten each iteration. The Codex
runner's `--timeout 3600` gives each task up to one hour wall-clock;
the Bash tool's 10-minute ceiling does not apply when running in the
background.

## 5. Halt Conditions

Check these in order on every iteration result.

| Signal | Action |
|---|---|
| `final_message` contains `STATUS: COMPLETED` | Reset consecutive-failure counter to 0. Continue. |
| `final_message` contains `STATUS: FAILED` | Increment consecutive-failure counter. If counter >= 3, HALT. Else continue. |
| `codex_task_run.json` has `exit_code != 0` AND `/tmp/codex_task_run.err` contains any of `429`, `rate limit`, `rate_limit`, `quota exceeded`, `too many requests` (case-insensitive) | HALT. Do NOT retry. |
| `codex_task_run.json` has `exit_code != 0` AND stderr has no rate-limit signature | Retry the SAME launch once after `sleep 60`. If the retry also fails, HALT. |
| `task.py next-step` does not return an executable task (returns a task-spec batch, brief, or "no action") | HALT (nothing more to do tonight). |
| `git status --porcelain` non-empty between iterations AND `git stash push` fails | HALT. |
| You have completed 31+ tasks in this session | HALT (defensive cap; bigger than the expected remainder). |

On HALT: stop launching new tasks. Write a final-summary message to
the user (Section 7). Do NOT call `task.py fail` on the
already-running task -- Codex either committed or it didn't, and
either way the state is durable.

## 6. Rate-Limit Specifics

Rate limits propagate. The outer Codex and the inner review Codex
share auth and quota -- if one hits a limit, the other will too on
its next call. There are two paths to detect:

- **Outer Codex itself rate-limited.** `codex_runner.py` exits
  non-zero and stderr contains a rate-limit signature. Halt
  immediately per Section 5.
- **Inner review Codex rate-limited mid-task.** Outer Codex may
  surface this as `STATUS: FAILED` with a `FAILURE_REASON` mentioning
  rate limit, or it may itself fail. Either way, the outer-side rules
  in Section 5 will halt the loop within at most 3 iterations
  (consecutive-failure threshold).

Do NOT wait and retry rate-limited calls. The user's quota window may
be hours; let morning-user decide.

## 7. Halt / Morning Report

When halting, write ONE message to the user with this shape:

```
ORCHESTRATION HALTED

Halt reason: <one line>
Tasks completed this session: <N>
Tasks failed this session: <N> (IDs: ...)
Total project progress: <task.py status output>
Working tree: clean | dirty (stashed at <stash ref>) | dirty (HALTED)
Recent commits:
  <sha> <subject>
  ...

Next action for morning user:
  <one sentence>
```

Then stop responding. Wait for the morning user.

## 8. Resumption (Morning)

If the user wakes you up to continue, just read this file again and
restart the loop. The harness commands give you everything you need:
- `task.py status` for current progress
- `task.py next-step` for the next task
- `git log` for what was done overnight

No special "resume" state is needed.

## 9. What Not To Do

- Do NOT read the chat history of prior orchestrator iterations
  beyond the last 1-2 to reconstruct context. Use `git log` and
  `task.py status` instead -- they are authoritative and cheap.
- Do NOT modify any code, run tests, or commit. Those are Codex's
  responsibility per task.
- Do NOT modify `.harness/SESSION.md`, `.harness/RALPH.md`, or any
  task spec file during the orchestration run.
- Do NOT skip the halt-condition check on any iteration.
- Do NOT call `codex_runner.py` synchronously -- always
  `run_in_background: true`.
- Do NOT add a "wait, try one more time" branch for rate limits.
  Halt and let the user decide.
- Do NOT invent additional halt conditions beyond Section 5 in the
  middle of an overnight run. If you think one is needed, halt and
  surface it to the user instead.

## 10. Bootstrap

To start orchestrating, the user types something like:
"Read .harness/RALPH.md and begin orchestrating."

Your first action is to run the recovery commands in Section 2 to
print current state, then enter the loop at step 1 of Section 3.
