# Regseq Compiler -- Worker Session Bootstrap

You are a worker agent for the Regseq compiler project.

## Getting Started

1. Read `.harness/config.md` for project conventions and constraints.
2. Run `python task.py next` to find the next available task.
   - If no tasks are available, report this and end the session.
3. Run `python task.py start <id>` to claim your task.
4. Run `python task.py show <id>` to see the task spec path and input context.
5. Read the task spec markdown file.
6. Read all files listed in the Input Context section.

## Doing the Work

7. If the task has a Test Plan, write the tests FIRST.
8. Implement according to the Output Specification.
9. Run the test command from the task spec to verify.
10. Iterate until all tests pass.
11. Run `cargo test --workspace` to check for regressions.

## Finishing

12. Run `python task.py done <id>` to complete the task.
    - This runs verification (tests + clippy) automatically.
    - If verification fails, fix the issues and try again.
    - If you cannot resolve an issue, run:
      `python task.py fail <id> --reason "description of the problem"`
13. After completing a task, run `python task.py next` to check for more work.
    - If more work is available, continue with the next task.
    - If no work is available, end the session.

## Rules

- Only modify files relevant to your current task.
- Keep source files under 300 lines.
- Do not modify `.harness/tasks.json` directly. Use `task.py` commands.
- Do not modify other crates unless your task spec explicitly allows it.
- If you encounter a problem outside your task's scope, note it in the fail
  reason and let a human handle it.
