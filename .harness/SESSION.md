# Regseq Compiler -- Worker Session Bootstrap

You are a worker agent for the Regseq compiler project.

## Getting Started

1. Read `.harness/config.md` for project conventions and constraints.
2. Run `uv run python scripts/python/task.py doctor` to verify prerequisites (toolchain, cargo, Python dependencies, etc.).
3. Run `uv run python scripts/python/task.py next-step` to determine the next action.
4. If the next action is an executable task:
   - Run `uv run python scripts/python/task.py start <id>` to claim your task.
   - If `start` prints `INBOUND_EXPECTATIONS: <path>`, read that file
     first; it contains forward refs from earlier tasks that you must
     address in this task.
   - Run `uv run python scripts/python/task.py show <id>` to see the task spec path and input context.
   - Read the task spec markdown file.
   - Read all files listed in the Input Context section.
5. If the next action is a task-spec batch:
   - Read `.harness/task-spec-workflow.md`.
   - Read the batch brief path printed by `next-step`.
   - Draft a proposed task decomposition and present it to the user for review before writing any files to disk. Do not create task spec files, modify `tasks.json`, or update `task-spec-status.yaml` until the user gives explicit approval of the proposal.
   - Do not end the session immediately unless the workflow document says there is nothing ready to elaborate.
6. If the next action is a brief-authoring batch:
   - Read `.harness/task-spec-workflow.md`.
   - Open `.harness/spec-batches/BRIEF_TEMPLATE.md`.
   - Create the missing batch brief shown by `next-step`.
   - Follow the tracker update rules in `.harness/task-spec-workflow.md` before ending the session.

## Doing the Work

7. If you are executing a real harness task, define the public API first: create modules with public type definitions,
   struct declarations, and function signatures using `todo!()` bodies. The
   task spec's Implementation Notes section specifies these signatures.
8. Write tests from the Test Plan against the API stubs. Tests should compile
   (though they would panic if run due to `todo!()`).
9. Replace `todo!()` with real implementation.
10. Run the test command from the task spec to verify.
11. Iterate until all tests pass.

## Review

The harness uses three review tiers, named *after* but **not the
same as** README §5.6's spec-side tiers (`docs/discussions/
regseq-model/README.md`). The spec-side tiers operate on spec
authoring (per-session / cross-session / top-level post-merge);
the harness-side tiers below operate on implementation against the
locked spec. Do not conflate them.

### Task Review (per-task, mandatory)

12. Once tests pass, request a Codex review of your implementation by
    invoking `scripts/python/codex_runner.py` via the Bash tool. Do NOT
    use the `codex:codex-rescue` agent / `/codex:rescue` skill -- that
    path goes through a background helper that has hung for hours in
    the past. `codex_runner.py` shells out to `codex exec --json`
    directly, parses the JSONL event stream, and returns a
    `CodexResult` as JSON on stdout. Invoke it like this (use the
    Bash tool's `timeout` parameter, max 600000 ms):

    ```bash
    cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
    <task>
    Review the implementation of <TASK_ID> against its task spec at
    <SPEC_PATH>. ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE
    TASK SPEC.

    Files to audit:
    - <file1>
    - <file2>
    ...

    Focus on:
    - Spec deviations (every rule in the Implementation Notes section).
    - Correctness issues and generality loss.
    - Design misalignment with the spec's public API / architecture.
    - Test coverage vs. the spec's Test Plan.
    Do NOT flag anything explicitly marked out of scope (Non-Goals,
    later task IDs).
    </task>

    <grounding_rules>
    Cite file:line for every claim and every required fix. Do not
    speculate about code you did not read. Mark hypotheses as
    hypotheses.
    </grounding_rules>

    <structured_output_contract>
    First line: VERDICT: APPROVED | REQUIRES FIXES
    If REQUIRES FIXES: numbered list, each entry exactly
      <file>:<line-range> -- <what is wrong> -- <what the spec requires>
    If APPROVED: one paragraph (<=120 words) summarizing what was
    verified and why any deliberate deviations are acceptable.
    Total response <= 400 words.
    </structured_output_contract>

    <default_follow_through_policy>
    Do not ask clarifying questions. Read the files, form a verdict,
    stop.
    </default_follow_through_policy>
    EOF
    ```

    Parse the `final_message` field from the returned JSON (or pass
    `--message-only` to print just that text). The first line carries
    the verdict. If Codex takes longer than 540 seconds you will hit
    the script's internal timeout; raise it with `--timeout N` (max
    ~595 to stay under the Bash tool's 10-minute ceiling).
13. If Codex returns REQUIRES FIXES, address each cited fix (unless
    you have strong reason to disagree -- in which case document that
    in the fix rationale) and re-invoke `codex_runner.py` with a
    fresh prompt.
14. Iterate (fix -> `codex_runner.py` review) until Codex returns
    VERDICT: APPROVED. Do not skip this step.

### Batch Review (per-batch cross-task, optional)

Invoked when a brief's `Review Tiers` section flags Batch Review for
a coherent multi-task feature. Use the same `codex_runner.py` pattern
as Task Review, with the `Files to audit` list expanded across all
tasks in the batch and the focus shifted to inter-task consistency
(shared types, invariants, integration shape). Not numbered into the
cold-session bootstrap flow; opt-in per-batch when the brief calls
for it.

### Phase-Close Review (per-phase, mandatory at phase boundary)

Triggered by `uv run python scripts/python/task.py phase-status
<phase>` returning PASS. File a phase-closure brief using
`.harness/spec-batches/PHASE_CLOSURE_BRIEF_TEMPLATE.md`. Phase-Close
Review covers the cumulative phase impl artifact (Rust code, test
output, `spec_refs` coverage against the locked spec), not spec
content -- it is distinct from README §5.6's Tier 3, which is the
top-level post-merge review of `spec.md` on the spec-authoring side.

## Finishing

15. Run `cargo fmt` to format code, then `cargo test --workspace` to check
    for regressions.
16. If you executed a real harness task, run `uv run python scripts/python/task.py done <id>` to complete the task.
    - This runs verification (tests + fmt check + clippy) automatically.
    - If verification fails, fix the issues and try again.
    - If you cannot resolve an issue, run:
      `uv run python scripts/python/task.py fail <id> --reason "description of the problem"`
17. Commit your changes:
    - Stage all files you created or modified: `git add <files>`
    - Commit with a message referencing the task ID:
      `git commit -m "<task-id>: brief description of what was done"`
    - Example: `git commit -m "cargo-regseq-02: CLI argument parsing with clap"`
18. If you authored or updated harness task specs or batch briefs instead of executing a real task:
    - Update `.harness/task-spec-status.yaml` to reflect the new spec-authoring status.
    - If you created executable harness tasks, ensure the corresponding `generated_task_ids` are recorded.
19. After completing either execution work or spec-authoring work, run `uv run python scripts/python/task.py next-step` to determine the next action.
    - If the next action is another executable task, continue.
    - If the next action is a task-spec batch, consult `.harness/task-spec-workflow.md`.
    - If the next action is a brief-authoring batch, create that brief first.
    - If there is no next action, end the session.

## Rules

- Follow all conventions in `.harness/config.md`.
- Do not modify `.harness/tasks.json` directly. Use `uv run python scripts/python/task.py` commands: `add-task` to create a new task, `update-task-spec` to bump a task's `spec_version` after editing its spec markdown, `archive` to retire a task, plus the existing `start` / `done` / `fail` / `reset` for status transitions.
- Do not use `.harness/task-spec-status.yaml` to track implementation execution status; it is only for harness task-spec authoring progress.
- Do not modify other crates unless your task spec explicitly allows it.
- If you encounter a problem outside your task's scope, note it in the fail
  reason and let a human handle it.
