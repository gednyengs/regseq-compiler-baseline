# Harness Tweaks v1 -- Assessment

Reviewer feedback in `logs/harness-tweaks-v1.md`. Decisions below.

## 1. Safe Task State Updates -- Partial Accept

**Accept:** Atomic writes via `os.replace()` to guard against Ctrl-C corruption.

**Skip:** File locking, ownership metadata, hostname, single-in-progress enforcement.
The harness uses a single-worker model (human-as-orchestrator). Concurrency
protection and ownership tracking solve a problem we don't have. If we move to
concurrent workers later, reconsider -- but adding the machinery now is
premature.

## 2. Verification That Explains Failures -- Full Accept

**Accept all:**
- Print both stdout and stderr on failure (last 50 lines each).
- Print exit code on failure.
- Persist full verification logs to `logs/verify/<task-id>-<timestamp>.log`.
- Use `cargo metadata` instead of string matching for workspace detection.

**Skip:** Configurable timeouts via env vars. 300s has not been a problem.

This is the highest-value change. The current 20-line stderr tail is
insufficient for debugging compiler test failures.

## 3. `cargo fmt --check` + Error-Type Guidance -- Full Accept

**Accept:**
- Add `cargo fmt --check --workspace` as a verification step.
- Update `config.md` to document the three verification checks (test + fmt + clippy).
- Fix error-type wording: `thiserror` in libraries, `anyhow` only in
  binary crate top-level orchestration.

## 4. Quick Verify + Context Command -- Skip

`verify --mode quick` creates a second verification path with maintenance cost
and ambiguity about what "verified" means. Workers can run `cargo test -p <crate>`
directly during development.

`context` duplicates what SESSION.md already instructs agents to do (read spec,
read input context). Another command for the same workflow adds surface area
without clear benefit.

## 5. `task.py doctor` -- Full Accept

Cheap to implement, prevents wasted debugging time when toolchain isn't set up.
Especially useful with a pinned nightly.

Checks: rustup, cargo, rustc, toolchain match, cargo metadata.

## Summary

| Proposal | Decision |
|----------|----------|
| 1. Locking/ownership | Atomic writes only |
| 2. Better verification output | Full implementation |
| 3. fmt check + error guidance | Full implementation |
| 4. Quick verify + context | Skip |
| 5. Doctor command | Implement |
