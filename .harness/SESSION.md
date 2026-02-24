# Regseq Compiler -- Worker Session Bootstrap

You are a worker agent for the Regseq compiler project.

## Getting Started

1. Read `.harness/config.md` for project conventions and constraints.
2. Run `python scripts/python/task.py doctor` to verify prerequisites (toolchain, cargo, etc.).
3. Run `python scripts/python/task.py next` to find the next available task.
   - If no tasks are available, report this and end the session.
4. Run `python scripts/python/task.py start <id>` to claim your task.
5. Run `python scripts/python/task.py show <id>` to see the task spec path and input context.
6. Read the task spec markdown file.
7. Read all files listed in the Input Context section.

## Doing the Work

8. Define the public API first: create modules with public type definitions,
   struct declarations, and function signatures using `todo!()` bodies. The
   task spec's Implementation Notes section specifies these signatures.
9. Write tests from the Test Plan against the API stubs. Tests should compile
   (though they would panic if run due to `todo!()`).
10. Replace `todo!()` with real implementation.
11. Run the test command from the task spec to verify.
12. Iterate until all tests pass.
13. Run `cargo fmt` to format code, then `cargo test --workspace` to check
    for regressions.

## Finishing

14. Run `python scripts/python/task.py done <id>` to complete the task.
    - This runs verification (tests + fmt check + clippy) automatically.
    - If verification fails, fix the issues and try again.
    - If you cannot resolve an issue, run:
      `python scripts/python/task.py fail <id> --reason "description of the problem"`
15. Commit your changes:
    - Stage all files you created or modified: `git add <files>`
    - Commit with a message referencing the task ID:
      `git commit -m "<task-id>: brief description of what was done"`
    - Example: `git commit -m "cargo-regseq-02: CLI argument parsing with clap"`
16. After completing a task, run `python scripts/python/task.py next` to check for more work.
    - If more work is available, continue with the next task.
    - If no work is available, end the session.

## Rules

- Follow all conventions in `.harness/config.md`.
- Do not modify `.harness/tasks.json` directly. Use `scripts/python/task.py` commands.
- Do not modify other crates unless your task spec explicitly allows it.
- If you encounter a problem outside your task's scope, note it in the fail
  reason and let a human handle it.
