# Regseq Compiler -- Worker Session Bootstrap

You are a worker agent for the Regseq compiler project.

## Getting Started

1. Read `.harness/config.md` for project conventions and constraints.
2. Run `uv run python scripts/python/task.py doctor` to verify prerequisites (toolchain, cargo, Python dependencies, etc.).
3. Run `uv run python scripts/python/task.py next-step` to determine the next action.
4. If the next action is an executable task:
   - Run `uv run python scripts/python/task.py start <id>` to claim your task.
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
12. Run `cargo fmt` to format code, then `cargo test --workspace` to check
    for regressions.

## Finishing

13. If you executed a real harness task, run `uv run python scripts/python/task.py done <id>` to complete the task.
    - This runs verification (tests + fmt check + clippy) automatically.
    - If verification fails, fix the issues and try again.
    - If you cannot resolve an issue, run:
      `uv run python scripts/python/task.py fail <id> --reason "description of the problem"`
14. Commit your changes:
    - Stage all files you created or modified: `git add <files>`
    - Commit with a message referencing the task ID:
      `git commit -m "<task-id>: brief description of what was done"`
    - Example: `git commit -m "cargo-regseq-02: CLI argument parsing with clap"`
15. If you authored or updated harness task specs or batch briefs instead of executing a real task:
    - Update `.harness/task-spec-status.yaml` to reflect the new spec-authoring status.
    - If you created executable harness tasks, ensure the corresponding `generated_task_ids` are recorded.
16. After completing either execution work or spec-authoring work, run `uv run python scripts/python/task.py next-step` to determine the next action.
    - If the next action is another executable task, continue.
    - If the next action is a task-spec batch, consult `.harness/task-spec-workflow.md`.
    - If the next action is a brief-authoring batch, create that brief first.
    - If there is no next action, end the session.

## Rules

- Follow all conventions in `.harness/config.md`.
- Do not modify `.harness/tasks.json` directly. Use `uv run python scripts/python/task.py` commands.
- Do not use `.harness/task-spec-status.yaml` to track implementation execution status; it is only for harness task-spec authoring progress.
- Do not modify other crates unless your task spec explicitly allows it.
- If you encounter a problem outside your task's scope, note it in the fail
  reason and let a human handle it.
