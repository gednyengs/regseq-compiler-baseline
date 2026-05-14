#!/usr/bin/env python3
"""Regseq compiler harness -- task management CLI.

Usage:
    uv run python scripts/python/task.py next                     Find next available executable task
    uv run python scripts/python/task.py next-step                Determine the next executable or task-spec action
    uv run python scripts/python/task.py show <id>                Show task details
    uv run python scripts/python/task.py start <id>               Mark task as in_progress
    uv run python scripts/python/task.py done <id>                Run verification and mark done
    uv run python scripts/python/task.py fail <id> --reason "…"   Mark task as failed
    uv run python scripts/python/task.py status                   Show summary table
    uv run python scripts/python/task.py list [--status <s>]      List tasks with optional filter
    uv run python scripts/python/task.py blocked                  Show tasks with unmet dependencies
    uv run python scripts/python/task.py doctor                   Check prerequisites
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent.parent
TASKS_FILE = REPO_ROOT / ".harness" / "tasks.json"
TASK_SPEC_STATUS_FILE = REPO_ROOT / ".harness" / "task-spec-status.yaml"
LOGS_DIR = REPO_ROOT / "logs" / "verify"
TAIL_LINES = 50


def load_tasks() -> dict:
    with open(TASKS_FILE) as f:
        data = json.load(f)
    schema_version = data.get("schema_version", 1)
    if schema_version == 1:
        print(
            "warning: .harness/tasks.json is at schema_version 1; "
            "run scripts/python/migrate_tasks_v1_to_v2.py to migrate to v2",
            file=sys.stderr,
        )
    elif schema_version != 2:
        print(
            f"warning: .harness/tasks.json schema_version is {schema_version}; "
            "expected 1 or 2",
            file=sys.stderr,
        )
    return data


def save_tasks(data: dict) -> None:
    """Write tasks atomically: write to temp file then os.replace()."""
    tmp_fd, tmp_path = tempfile.mkstemp(
        dir=TASKS_FILE.parent, suffix=".tmp", prefix="tasks-"
    )
    try:
        with os.fdopen(tmp_fd, "w") as f:
            json.dump(data, f, indent=2)
            f.write("\n")
        os.replace(tmp_path, TASKS_FILE)
    except BaseException:
        # Clean up temp file on any failure (including KeyboardInterrupt)
        try:
            os.unlink(tmp_path)
        except OSError:
            pass
        raise


def find_task(data: dict, task_id: str) -> tuple[dict, dict] | None:
    """Return (group, task) for the given task ID, or None."""
    for group in data["groups"]:
        for task in group["tasks"]:
            if task["id"] == task_id:
                return group, task
    return None


def all_tasks(data: dict) -> list[dict]:
    """Return flat list of all tasks."""
    tasks = []
    for group in data["groups"]:
        for task in group["tasks"]:
            tasks.append(task)
    return tasks


def task_status_map(data: dict) -> dict[str, str]:
    """Return {task_id: status} for all tasks."""
    return {t["id"]: t["status"] for t in all_tasks(data)}


def deps_met(task: dict, status_map: dict[str, str]) -> bool:
    """Check if all dependencies of a task are done."""
    for dep_id in task.get("depends_on", []):
        if status_map.get(dep_id) != "done":
            return False
    return True


def cmd_next(args: argparse.Namespace) -> None:
    data = load_tasks()
    status_map = task_status_map(data)

    for task in all_tasks(data):
        if task["status"] == "pending" and deps_met(task, status_map):
            print(f"NEXT_TASK: {task['id']}")
            print(f"SPEC: {task['spec']}")
            print(f"ACTION: Run `uv run python scripts/python/task.py start {task['id']}` then read the spec file.")
            return

    print("NO_TASKS_AVAILABLE: All pending tasks have unmet dependencies or all tasks are done.")
    blocked = [t for t in all_tasks(data) if t["status"] == "pending" and not deps_met(t, status_map)]
    if blocked:
        print("BLOCKED_TASKS:")
        for t in blocked:
            unmet = [d for d in t.get("depends_on", []) if status_map.get(d) != "done"]
            unmet_strs = [f"{d} [{status_map.get(d, 'unknown')}]" for d in unmet]
            print(f"  - {t['id']} (blocked by: {', '.join(unmet_strs)})")
    print("ACTION: Run `uv run python scripts/python/task.py next-step` to determine whether a task-spec batch is ready.")


def load_task_spec_status() -> dict:
    try:
        import yaml
    except ModuleNotFoundError:
        print("ERROR: PyYAML is not installed.")
        print("ACTION: Run `uv sync` or invoke the harness with `uv run python scripts/python/task.py ...`.")
        sys.exit(1)

    if not TASK_SPEC_STATUS_FILE.exists():
        print(f"ERROR: Task-spec tracker not found at {TASK_SPEC_STATUS_FILE}.")
        sys.exit(1)

    with open(TASK_SPEC_STATUS_FILE) as f:
        data = yaml.safe_load(f)

    if not isinstance(data, dict) or not isinstance(data.get("items"), list):
        print("ERROR: Task-spec tracker is malformed. Expected a top-level mapping with an `items` list.")
        sys.exit(1)

    return data


def task_spec_status_map(data: dict) -> dict[str, str]:
    return {item["id"]: item["status"] for item in data["items"]}


def task_spec_deps_met(item: dict, status_map: dict[str, str]) -> bool:
    for dep_id in item.get("depends_on", []):
        if status_map.get(dep_id) != "tasked":
            return False
    return True


def first_ready_task_spec_batch(data: dict) -> tuple[dict | None, list[dict]]:
    status_map = task_spec_status_map(data)
    ready = []
    for item in data["items"]:
        if item["status"] == "ready_to_elaborate" and task_spec_deps_met(item, status_map):
            ready.append(item)
    return (ready[0] if ready else None, ready)


def brief_exists(item: dict) -> bool:
    brief = item.get("brief")
    if not brief:
        return False
    return (REPO_ROOT / brief).exists()


def cmd_next_step(args: argparse.Namespace) -> None:
    data = load_tasks()
    status_map = task_status_map(data)

    for task in all_tasks(data):
        if task["status"] == "pending" and deps_met(task, status_map):
            print("NEXT_ACTION: EXECUTABLE_TASK")
            print(f"TASK_ID: {task['id']}")
            print(f"SPEC: {task['spec']}")
            print(f"ACTION: Run `uv run python scripts/python/task.py start {task['id']}` then read the spec file.")
            return

    tracker = load_task_spec_status()
    item, ready_items = first_ready_task_spec_batch(tracker)
    if item is not None:
        next_action = "TASK_SPEC_BATCH" if brief_exists(item) else "BRIEF_AUTHORING_BATCH"
        print(f"NEXT_ACTION: {next_action}")
        print(f"TRACKER_ID: {item['id']}")
        print(f"PHASE: {item['phase']}")
        print(f"TITLE: {item['title']}")
        if item.get("brief"):
            label = "BRIEF" if brief_exists(item) else "EXPECTED_BRIEF"
            print(f"{label}: {item['brief']}")
        if item.get("target_harness_groups"):
            print(f"TARGET_HARNESS_GROUPS: {', '.join(item['target_harness_groups'])}")
        if item.get("source_refs"):
            print(f"SOURCE_REFS: {', '.join(item['source_refs'])}")
        print(f"SUMMARY: {item['summary']}")
        if item.get("spec_authoring_notes"):
            print("SPEC_AUTHORING_NOTES:")
            for note in item["spec_authoring_notes"]:
                print(f"  - {note}")
        if len(ready_items) > 1:
            print(f"WARNING: {len(ready_items)} task-spec batches are marked ready. Using the first listed item.")
        if next_action == "TASK_SPEC_BATCH":
            print("ACTION: Read `.harness/task-spec-workflow.md`, then elaborate this batch into harness task specs and update `.harness/task-spec-status.yaml`.")
        else:
            print("ACTION: Read `.harness/task-spec-workflow.md`, create the missing brief using `.harness/spec-batches/BRIEF_TEMPLATE.md`, then update `.harness/task-spec-status.yaml`.")
        return

    print("NO_NEXT_ACTION: No executable task and no dependency-satisfied task-spec batch are ready.")
    blocked = [item for item in tracker["items"] if item["status"] == "not_ready"]
    if blocked:
        print("TASK_SPEC_BATCHES_WAITING_ON_DEPENDENCIES:")
        for item in blocked[:10]:
            print(f"  - {item['id']}: depends_on={', '.join(item.get('depends_on', [])) or 'none'}")
    print("ACTION: End session or ask a human to review the task-spec tracker.")


def cmd_show(args: argparse.Namespace) -> None:
    data = load_tasks()
    result = find_task(data, args.id)
    if result is None:
        print(f"ERROR: Task '{args.id}' not found.")
        print("ACTION: Run `uv run python scripts/python/task.py list` to see available task IDs.")
        sys.exit(1)

    group, task = result
    print(f"TASK: {task['id']}")
    print(f"NAME: {task['name']}")
    print(f"GROUP: {group['name']}")
    print(f"STATUS: {task['status']}")
    print(f"PHASE: {task.get('phase')}")
    print(f"SPEC: {task['spec']}")
    print(f"SPEC_VERSION: {task.get('spec_version', 1)}")
    if task.get("spec_refs"):
        print("SPEC_REFS:")
        for ref in task["spec_refs"]:
            print(f"  - {ref}")
    if task.get("depends_on"):
        status_map = task_status_map(data)
        deps = ", ".join(f"{d} [{status_map.get(d, 'unknown')}]" for d in task["depends_on"])
        print(f"DEPENDS_ON: {deps}")
    if task.get("artifacts"):
        print(f"ARTIFACTS: {', '.join(task['artifacts'])}")
    if task.get("fail_reason"):
        print(f"FAIL_REASON: {task['fail_reason']}")
    history = task.get("history", [])
    if history:
        print("HISTORY (last 3):")
        for entry in history[-3:]:
            print(f"  - {entry}")
    print(f"ACTION: Read {task['spec']} for the full task specification.")


def cmd_start(args: argparse.Namespace) -> None:
    data = load_tasks()
    result = find_task(data, args.id)
    if result is None:
        print(f"ERROR: Task '{args.id}' not found.")
        print("ACTION: Run `uv run python scripts/python/task.py list` to see available task IDs.")
        sys.exit(1)

    _, task = result
    status_map = task_status_map(data)

    if task["status"] != "pending":
        print(f"ERROR: Cannot start '{args.id}'. Current status: {task['status']}.")
        print("ACTION: Only tasks with status 'pending' can be started.")
        sys.exit(1)

    if not deps_met(task, status_map):
        unmet = [d for d in task.get("depends_on", []) if status_map.get(d) != "done"]
        print(f"ERROR: Cannot start '{args.id}'. Dependencies not met.")
        for d in unmet:
            print(f"  - {d} [{status_map.get(d, 'unknown')}]")
        print("ACTION: Run `uv run python scripts/python/task.py next-step` to find the next executable or task-spec action.")
        sys.exit(1)

    task["status"] = "in_progress"
    save_tasks(data)
    print(f"STARTED: {args.id}")
    print(f"STATUS: in_progress")
    print(f"ACTION: Read {task['spec']} for your task specification.")

    expectations_path = REPO_ROOT / ".harness" / "expectations" / f"{args.id}.md"
    if expectations_path.exists():
        print(f"INBOUND_EXPECTATIONS: {expectations_path.relative_to(REPO_ROOT)}")


def workspace_has_members() -> bool:
    """Use cargo metadata to check if workspace has at least one package."""
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            capture_output=True, text=True, timeout=30, cwd=REPO_ROOT
        )
        if result.returncode != 0:
            return False
        metadata = json.loads(result.stdout)
        return len(metadata.get("packages", [])) > 0
    except (subprocess.TimeoutExpired, json.JSONDecodeError, FileNotFoundError):
        return False


def tail_lines(text: str, n: int) -> list[str]:
    """Return last n non-empty lines of text."""
    lines = text.strip().split("\n")
    return lines[-n:]


def persist_log(task_id: str, entries: list[dict]) -> Path:
    """Write full verification log and return its path."""
    LOGS_DIR.mkdir(parents=True, exist_ok=True)
    ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    log_path = LOGS_DIR / f"{task_id}-{ts}.log"
    with open(log_path, "w") as f:
        for entry in entries:
            f.write(f"=== {entry['command']} ===\n")
            f.write(f"exit_code: {entry['exit_code']}\n")
            f.write(f"cwd: {entry.get('cwd', 'N/A')}\n\n")
            if entry.get("stdout"):
                f.write("--- stdout ---\n")
                f.write(entry["stdout"])
                f.write("\n")
            if entry.get("stderr"):
                f.write("--- stderr ---\n")
                f.write(entry["stderr"])
                f.write("\n")
            f.write("\n")
    return log_path


def run_step(cmd: list[str], label: str) -> dict:
    """Run a verification step and return structured result."""
    result = subprocess.run(
        cmd, capture_output=True, text=True, timeout=300, cwd=REPO_ROOT
    )
    return {
        "command": label,
        "cmd": cmd,
        "exit_code": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
        "cwd": str(REPO_ROOT),
        "passed": result.returncode == 0,
    }


def format_failure(step: dict) -> list[str]:
    """Format a failed verification step for console output."""
    lines = []
    lines.append(f"  EXIT_CODE: {step['exit_code']}")
    stdout_tail = tail_lines(step["stdout"], TAIL_LINES) if step["stdout"].strip() else []
    stderr_tail = tail_lines(step["stderr"], TAIL_LINES) if step["stderr"].strip() else []
    if stdout_tail:
        lines.append("  --- stdout (last lines) ---")
        for line in stdout_tail:
            lines.append(f"  {line}")
    if stderr_tail:
        lines.append("  --- stderr (last lines) ---")
        for line in stderr_tail:
            lines.append(f"  {line}")
    return lines


def run_verification(task_id: str) -> tuple[bool, list[str]]:
    """Run cleanup steps, then workspace verification. Return (passed, messages)."""
    messages = []
    passed = True
    log_entries = []

    if not workspace_has_members():
        messages.append("VERIFICATION: Workspace has no packages. Skipping cargo checks.")
        return True, messages

    # cleanup: cargo fmt --all
    step = run_step(["cargo", "fmt", "--all"], "cargo fmt --all")
    log_entries.append(step)
    if step["passed"]:
        messages.append("CLEANUP: cargo fmt --all... PASSED")
    else:
        passed = False
        messages.append("CLEANUP: cargo fmt --all... FAILED")
        messages.extend(format_failure(step))

    # cleanup: cargo fix
    step = run_step(
        ["cargo", "fix", "--workspace", "--allow-dirty", "--allow-staged"],
        "cargo fix --workspace --allow-dirty --allow-staged"
    )
    log_entries.append(step)
    if step["passed"]:
        messages.append("CLEANUP: cargo fix --workspace... PASSED")
    else:
        passed = False
        messages.append("CLEANUP: cargo fix --workspace... FAILED")
        messages.extend(format_failure(step))

    # cleanup: cargo clippy --fix
    step = run_step(
        ["cargo", "clippy", "--fix", "--workspace", "--allow-dirty", "--allow-staged"],
        "cargo clippy --fix --workspace --allow-dirty --allow-staged"
    )
    log_entries.append(step)
    if step["passed"]:
        messages.append("CLEANUP: cargo clippy --fix --workspace... PASSED")
    else:
        passed = False
        messages.append("CLEANUP: cargo clippy --fix --workspace... FAILED")
        messages.extend(format_failure(step))

    # cleanup: re-run fmt after automated fixes mutate files
    step = run_step(["cargo", "fmt", "--all"], "cargo fmt --all")
    log_entries.append(step)
    if step["passed"]:
        messages.append("CLEANUP: final cargo fmt --all... PASSED")
    else:
        passed = False
        messages.append("CLEANUP: final cargo fmt --all... FAILED")
        messages.extend(format_failure(step))

    # cargo test
    step = run_step(["cargo", "test", "--workspace"], "cargo test --workspace")
    log_entries.append(step)
    if step["passed"]:
        messages.append("VERIFICATION: cargo test --workspace... PASSED")
    else:
        passed = False
        messages.append("VERIFICATION: cargo test --workspace... FAILED")
        messages.extend(format_failure(step))

    # cargo fmt --check
    step = run_step(
        ["cargo", "fmt", "--all", "--", "--check"],
        "cargo fmt --all -- --check"
    )
    log_entries.append(step)
    if step["passed"]:
        messages.append("VERIFICATION: cargo fmt --all -- --check... PASSED")
    else:
        passed = False
        messages.append("VERIFICATION: cargo fmt --all -- --check... FAILED")
        messages.extend(format_failure(step))

    # cargo clippy
    step = run_step(
        ["cargo", "clippy", "--workspace", "--", "-D", "warnings"],
        "cargo clippy --workspace -- -D warnings"
    )
    log_entries.append(step)
    if step["passed"]:
        messages.append("VERIFICATION: cargo clippy --workspace -- -D warnings... PASSED")
    else:
        passed = False
        messages.append("VERIFICATION: cargo clippy --workspace -- -D warnings... FAILED")
        messages.extend(format_failure(step))

    # Persist full log
    log_path = persist_log(task_id, log_entries)
    if not passed:
        messages.append(f"VERIFICATION_LOG: Full output at {log_path}")

    return passed, messages


def cmd_done(args: argparse.Namespace) -> None:
    data = load_tasks()
    result = find_task(data, args.id)
    if result is None:
        print(f"ERROR: Task '{args.id}' not found.")
        sys.exit(1)

    _, task = result

    if task["status"] != "in_progress":
        print(f"ERROR: Cannot complete '{args.id}'. Current status: {task['status']}.")
        print("ACTION: Only tasks with status 'in_progress' can be completed.")
        sys.exit(1)

    passed, messages = run_verification(args.id)
    for msg in messages:
        print(msg)

    if passed:
        task["status"] = "done"
        task["fail_reason"] = None
        save_tasks(data)
        print(f"DONE: {args.id}")
        print("ACTION: Run `uv run python scripts/python/task.py next-step` for your next action, or end session if none.")
    else:
        print(f"BLOCKED: {args.id} -- verification failed.")
        print("ACTION: Fix the failing checks and run `uv run python scripts/python/task.py done " + args.id + "` again.")


def cmd_fail(args: argparse.Namespace) -> None:
    data = load_tasks()
    result = find_task(data, args.id)
    if result is None:
        print(f"ERROR: Task '{args.id}' not found.")
        sys.exit(1)

    _, task = result

    if task["status"] != "in_progress":
        print(f"ERROR: Cannot fail '{args.id}'. Current status: {task['status']}.")
        print("ACTION: Only tasks with status 'in_progress' can be failed.")
        sys.exit(1)

    task["status"] = "failed"
    task["fail_reason"] = args.reason
    save_tasks(data)
    print(f"FAILED: {args.id}")
    print(f"REASON: {args.reason}")
    print("ACTION: End session. A human will review and reassign this task.")


def cmd_reset(args: argparse.Namespace) -> None:
    """Reset a task back to `pending` (recovery path for misuse of fail/start).

    Refuses to touch tasks already `pending`. For `done` tasks, requires
    `--force` so accidental resets don't wipe a completed result. Clears
    `fail_reason` as part of the transition. `artifacts` is left in place;
    remove them separately if the reset is meant to discard output.
    """
    data = load_tasks()
    result = find_task(data, args.id)
    if result is None:
        print(f"ERROR: Task '{args.id}' not found.")
        sys.exit(1)

    _, task = result
    current = task["status"]

    if current == "pending":
        print(f"ERROR: '{args.id}' is already pending; nothing to reset.")
        sys.exit(1)

    if current == "done" and not args.force:
        print(f"ERROR: '{args.id}' is done. Re-run with --force to reset a completed task.")
        print("ACTION: Consider whether downstream tasks already consumed its artifacts first.")
        sys.exit(1)

    task["status"] = "pending"
    task.pop("fail_reason", None)
    save_tasks(data)
    print(f"RESET: {args.id}")
    print(f"FROM: {current}")
    print(f"REASON: {args.reason}")
    if task.get("artifacts"):
        print("NOTE: `artifacts` field retained; clear manually if the reset should discard output.")
    print("ACTION: Task is now available for reassignment.")


LOCKED_SPEC_BASENAMES = {"spec.md", "pretty-printer.md", "generic-c-backend.md"}


def validate_spec_path(spec_str: str, group_id: str) -> str | None:
    """Return None if `spec_str` passes add-task spec validation, else a failure reason."""
    expected_prefix = f".harness/tasks/{group_id}/"
    if not spec_str.startswith(expected_prefix):
        return f"rule 1: path must start with `{expected_prefix}` (got `{spec_str}`)"
    if not spec_str.endswith(".md"):
        return f"rule 2: path must end with `.md` (got `{spec_str}`)"
    full = REPO_ROOT / spec_str
    if not full.exists():
        return f"rule 4: file does not exist on disk (`{full}`)"
    if full.is_symlink() or not full.is_file():
        return "rule 3: path must resolve to a regular file (not symlink, device, or directory)"
    return None


def validate_spec_ref(ref: str) -> str | None:
    """Return None if `ref` is a well-shaped spec reference, else a failure reason."""
    if "#" not in ref:
        return f"spec ref `{ref}` missing `#` separator"
    left = ref.split("#", 1)[0]
    basename = left.rsplit("/", 1)[-1] if "/" in left else left
    if basename not in LOCKED_SPEC_BASENAMES:
        return f"spec ref `{ref}` basename `{basename}` not in locked set {sorted(LOCKED_SPEC_BASENAMES)}"
    return None


def cmd_add_task(args: argparse.Namespace) -> None:
    """Canonical write path for new tasks. Refuses duplicate IDs and invalid inputs."""
    data = load_tasks()

    if find_task(data, args.id) is not None:
        print(f"ERROR: Task ID `{args.id}` already exists.")
        sys.exit(1)

    group = next((g for g in data["groups"] if g["id"] == args.group), None)
    if group is None:
        print(f"ERROR: Group `{args.group}` not found.")
        sys.exit(1)

    err = validate_spec_path(args.spec, args.group)
    if err:
        print(f"ERROR: Invalid --spec: {err}")
        sys.exit(1)

    for dep_id in args.depends_on or []:
        if find_task(data, dep_id) is None:
            print(f"ERROR: Dependency `{dep_id}` does not exist.")
            sys.exit(1)

    for ref in args.spec_refs or []:
        err = validate_spec_ref(ref)
        if err:
            print(f"ERROR: Invalid --spec-refs: {err}")
            sys.exit(1)

    now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    new_task = {
        "id": args.id,
        "name": args.name,
        "spec": args.spec,
        "status": "pending",
        "depends_on": list(args.depends_on or []),
        "artifacts": [],
        "fail_reason": None,
        "phase": args.phase,
        "spec_refs": list(args.spec_refs or []),
        "spec_version": 1,
        "history": [{"ts": now, "event": "added", "spec_version": 1}],
    }
    group["tasks"].append(new_task)
    save_tasks(data)
    print(f"ADDED: {args.id}")
    print(f"GROUP: {args.group}")
    print(f"PHASE: {args.phase}")
    print(f"SPEC: {args.spec}")
    print("SPEC_VERSION: 1")


def cmd_update_task_spec(args: argparse.Namespace) -> None:
    """Bump a task's spec_version and append a history entry. Allowed regardless of status."""
    data = load_tasks()
    result = find_task(data, args.id)
    if result is None:
        print(f"ERROR: Task `{args.id}` not found.")
        sys.exit(1)
    _, task = result
    task["spec_version"] = task.get("spec_version", 1) + 1
    now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    entry = {"ts": now, "event": "spec_edited", "spec_version": task["spec_version"]}
    if args.reason:
        entry["reason"] = args.reason
    task.setdefault("history", []).append(entry)
    save_tasks(data)
    print(f"UPDATED: {args.id}")
    print(f"SPEC_VERSION: {task['spec_version']}")
    if args.reason:
        print(f"REASON: {args.reason}")


def cmd_archive(args: argparse.Namespace) -> None:
    """Flip status to archived; set phase=archived atomically. Refuses already-archived or done tasks."""
    data = load_tasks()
    now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    targets = []
    for tid in args.ids:
        result = find_task(data, tid)
        if result is None:
            print(f"ERROR: Task `{tid}` not found.")
            sys.exit(1)
        _, task = result
        if task["status"] == "archived":
            print(f"ERROR: Task `{tid}` is already archived.")
            sys.exit(1)
        if task["status"] == "done":
            print(f"ERROR: Task `{tid}` is done; use `reset --force` first to revive, then archive.")
            sys.exit(1)
        targets.append(task)

    for task in targets:
        task["status"] = "archived"
        task["phase"] = "archived"
        task.setdefault("history", []).append({
            "ts": now,
            "event": "archived",
            "reason": args.reason,
        })
    save_tasks(data)
    print(f"ARCHIVED: {len(targets)} task(s)")
    for tid in args.ids:
        print(f"  - {tid}")
    print(f"REASON: {args.reason}")


def cmd_status(args: argparse.Namespace) -> None:
    data = load_tasks()
    tasks = all_tasks(data)

    counts = {"pending": 0, "in_progress": 0, "done": 0, "failed": 0, "archived": 0}
    for t in tasks:
        counts[t["status"]] = counts.get(t["status"], 0) + 1

    total = len(tasks)
    print("STATUS_SUMMARY:")
    print(f"  Total:       {total}")
    print(f"  Done:        {counts['done']}")
    print(f"  In Progress: {counts['in_progress']}")
    print(f"  Pending:     {counts['pending']}")
    print(f"  Failed:      {counts['failed']}")
    print(f"  Archived:    {counts['archived']}")

    if total > 0:
        pct = (counts["done"] / total) * 100
        print(f"  Progress:    {pct:.0f}%")


def cmd_list(args: argparse.Namespace) -> None:
    data = load_tasks()
    status_map = task_status_map(data)

    def passes_filters(task: dict) -> bool:
        if args.status and task["status"] != args.status:
            return False
        if args.phase and task.get("phase") != args.phase:
            return False
        return True

    if args.json:
        for group in data["groups"]:
            for t in group["tasks"]:
                if passes_filters(t):
                    print(json.dumps(t))
        return

    if args.quiet:
        for group in data["groups"]:
            for t in group["tasks"]:
                if passes_filters(t):
                    print(t["id"])
        return

    print("TASK_LIST:")
    for group in data["groups"]:
        tasks = [t for t in group["tasks"] if passes_filters(t)]
        if not tasks:
            continue

        print(f"\n  [{group['name']}]")
        for t in tasks:
            met = deps_met(t, status_map)
            blocked_marker = " (BLOCKED)" if t["status"] == "pending" and not met else ""
            print(f"    {t['id']:30s} {t['status']:12s} {t['name']}{blocked_marker}")


def natural_sort_key(s: str) -> tuple:
    """Sort key that handles numeric and dotted-numeric strings naturally.

    "1" < "2" < "10" < "Appendix"; "1.1" < "1.2" < "1.10".
    """
    parts = []
    for piece in re.split(r"(\d+)", s):
        if piece.isdigit():
            parts.append((0, int(piece), ""))
        elif piece:
            parts.append((1, 0, piece))
    return tuple(parts) if parts else ((1, 0, ""),)


def extract_anchors(spec_path: Path, include_subsections: bool = False) -> list[str]:
    """Return ordered anchor slugs from a markdown spec file.

    Anchor slug = first whitespace-delimited token of the heading text,
    with trailing `.` stripped. Examples:
    - "## 19. Place and projections" -> "19"
    - "### 1.1 What Regseq is" -> "1.1"
    - "## Appendix A: Bridge-rejection examples" -> "Appendix"
    """
    anchors: list[str] = []
    seen: set[str] = set()
    for raw in spec_path.read_text().splitlines():
        if raw.startswith("## ") or (include_subsections and raw.startswith("### ")):
            text = raw.lstrip("#").strip()
            if not text:
                continue
            first = text.split(None, 1)[0].rstrip(".")
            if first and first not in seen:
                anchors.append(first)
                seen.add(first)
    return anchors


def cmd_ready_leaves(args: argparse.Namespace) -> None:
    """Print every pending task whose `depends_on` are all done, one ID per line."""
    data = load_tasks()
    status_map = task_status_map(data)

    leaves = []
    for t in all_tasks(data):
        if t["status"] != "pending":
            continue
        if args.phase and t.get("phase") != args.phase:
            continue
        if deps_met(t, status_map):
            leaves.append(t["id"])
    for tid in sorted(leaves):
        print(tid)


def cmd_phase_status(args: argparse.Namespace) -> None:
    """PASS iff every phase-tagged task is done AND every dep is done."""
    data = load_tasks()
    status_map = task_status_map(data)

    phase_tasks = [t for t in all_tasks(data) if t.get("phase") == args.phase]
    if not phase_tasks:
        print(f"UNKNOWN_PHASE: No tasks tagged with phase `{args.phase}`")
        sys.exit(2)

    not_done = [t for t in phase_tasks if t["status"] != "done"]
    blockers = []
    phase_ids = {t["id"] for t in phase_tasks}
    for t in phase_tasks:
        for dep_id in t.get("depends_on", []):
            if status_map.get(dep_id) != "done":
                blockers.append((t["id"], dep_id, status_map.get(dep_id, "unknown")))

    counts: dict[str, int] = {}
    for t in phase_tasks:
        counts[t["status"]] = counts.get(t["status"], 0) + 1

    print(f"PHASE: {args.phase}")
    print(f"TOTAL: {len(phase_tasks)}")
    for st in sorted(counts):
        print(f"  {st}: {counts[st]}")

    if not_done or blockers:
        print("VERDICT: FAIL")
        if not_done:
            print("TASKS_NOT_DONE:")
            for t in not_done:
                print(f"  - {t['id']} [{t['status']}]")
        if blockers:
            print("EXTERNAL_DEP_BLOCKERS:")
            for tid, dep_id, dep_status in blockers:
                # only flag external blockers (deps not in this phase)
                if dep_id not in phase_ids:
                    print(f"  - {tid} depends on {dep_id} [{dep_status}]")
        sys.exit(1)

    print("VERDICT: PASS")
    sys.exit(0)


def cmd_graph(args: argparse.Namespace) -> None:
    """Emit dependency DAG as mermaid (default) or dot."""
    data = load_tasks()

    filtered_ids: set[str] = set()
    for t in all_tasks(data):
        if args.phase and t.get("phase") != args.phase:
            continue
        if args.status and t["status"] != args.status:
            continue
        filtered_ids.add(t["id"])

    if args.format == "mermaid":
        print("flowchart LR")
        print("  classDef done fill:#cfc,stroke:#3a3")
        print("  classDef archived fill:#eee,stroke:#888")
        print("  classDef pending fill:#fff,stroke:#777")
        print("  classDef in_progress fill:#ffe,stroke:#aa0")
        print("  classDef failed fill:#fcc,stroke:#a33")
        for t in all_tasks(data):
            if t["id"] not in filtered_ids:
                continue
            for dep_id in t.get("depends_on", []):
                if dep_id in filtered_ids:
                    print(f"  {dep_id} --> {t['id']}")
        by_status: dict[str, list[str]] = {}
        for t in all_tasks(data):
            if t["id"] not in filtered_ids:
                continue
            by_status.setdefault(t["status"], []).append(t["id"])
        for st, ids in by_status.items():
            print(f"  class {','.join(ids)} {st}")
    elif args.format == "dot":
        print("digraph tasks {")
        print('  rankdir=LR;')
        for t in all_tasks(data):
            if t["id"] not in filtered_ids:
                continue
            for dep_id in t.get("depends_on", []):
                if dep_id in filtered_ids:
                    print(f'  "{dep_id}" -> "{t["id"]}";')
        print("}")


def cmd_spec_coverage(args: argparse.Namespace) -> None:
    """Coverage report against a spec file: covered / uncovered / drift."""
    spec_path = Path(args.spec_file)
    if not spec_path.exists():
        print(f"ERROR: Spec file not found: {args.spec_file}", file=sys.stderr)
        sys.exit(1)
    anchors = extract_anchors(spec_path, args.include_subsections)
    anchor_set = set(anchors)

    data = load_tasks()
    basename = spec_path.name

    refs: set[str] = set()
    for t in all_tasks(data):
        if args.include_status == "done" and t["status"] != "done":
            continue
        if args.include_status == "pending" and t["status"] != "pending":
            continue
        for ref in t.get("spec_refs", []):
            if "#" not in ref:
                continue
            left, anchor = ref.split("#", 1)
            ref_basename = left.rsplit("/", 1)[-1] if "/" in left else left
            if ref_basename == basename:
                refs.add(anchor)

    covered = anchor_set & refs
    uncovered = anchor_set - refs
    drift = refs - anchor_set

    print(f"SPEC_FILE: {args.spec_file}")
    print(f"ANCHORS: {len(anchor_set)}")
    print(f"COVERED: {len(covered)}")
    for a in sorted(covered, key=natural_sort_key):
        print(f"  - {a}")
    print(f"UNCOVERED: {len(uncovered)}")
    for a in sorted(uncovered, key=natural_sort_key):
        print(f"  - {a}")
    print(f"DRIFT: {len(drift)}")
    for a in sorted(drift):
        print(f"  - {a}")


def cmd_read_spec_refs(args: argparse.Namespace) -> None:
    """Print anchor slugs of a markdown spec file, one per line."""
    spec_path = Path(args.spec_file)
    if not spec_path.exists():
        print(f"ERROR: Spec file not found: {args.spec_file}", file=sys.stderr)
        sys.exit(1)
    for a in extract_anchors(spec_path, args.include_subsections):
        print(a)


def cmd_blocked(args: argparse.Namespace) -> None:
    data = load_tasks()
    status_map = task_status_map(data)

    blocked = []
    for t in all_tasks(data):
        if t["status"] == "pending" and not deps_met(t, status_map):
            unmet = [d for d in t.get("depends_on", []) if status_map.get(d) != "done"]
            blocked.append((t, unmet))

    if not blocked:
        print("NO_BLOCKED_TASKS: All pending tasks have their dependencies met.")
        print("ACTION: Run `uv run python scripts/python/task.py next-step` to find the next action.")
        return

    print("BLOCKED_TASKS:")
    for t, unmet in blocked:
        deps_str = ", ".join(f"{d} [{status_map.get(d, 'unknown')}]" for d in unmet)
        print(f"  - {t['id']}: blocked by {deps_str}")


def cmd_doctor(args: argparse.Namespace) -> None:
    """Check that all prerequisites are installed and configured."""
    ok = True
    pinned_toolchain = None

    def check(label: str, cmd: list[str]) -> bool:
        try:
            result = subprocess.run(
                cmd, capture_output=True, text=True, timeout=15, cwd=REPO_ROOT
            )
            version = result.stdout.strip().split("\n")[0]
            if result.returncode == 0:
                print(f"  OK: {label}: {version}")
                return True
            else:
                print(f"  FAIL: {label}: command returned exit code {result.returncode}")
                return False
        except FileNotFoundError:
            print(f"  FAIL: {label}: not found. Install it and ensure it is on PATH.")
            return False
        except subprocess.TimeoutExpired:
            print(f"  FAIL: {label}: timed out")
            return False

    print("DOCTOR: Checking prerequisites...")

    ok = check("python", [sys.executable, "--version"]) and ok
    ok = check("uv", ["uv", "--version"]) and ok
    ok = check("rustup", ["rustup", "--version"]) and ok
    ok = check("cargo", ["cargo", "--version"]) and ok
    ok = check("rustc", ["rustc", "--version"]) and ok

    try:
        import yaml  # noqa: F401
        print("  OK: PyYAML: import succeeded")
    except ModuleNotFoundError:
        print("  FAIL: PyYAML: not found. Run `uv sync` to install Python dependencies.")
        ok = False

    # Check toolchain file is respected
    toolchain_file = REPO_ROOT / "rust-toolchain.toml"
    if toolchain_file.exists():
        try:
            with open(toolchain_file, "rb") as f:
                data = tomllib.load(f)
            pinned_toolchain = data.get("toolchain", {}).get("channel")
        except (OSError, tomllib.TOMLDecodeError):
            print("  FAIL: rust-toolchain.toml: could not parse pinned channel")
            ok = False

        try:
            result = subprocess.run(
                ["rustup", "show", "active-toolchain"],
                capture_output=True, text=True, timeout=15, cwd=REPO_ROOT
            )
            active = result.stdout.strip().split("\n")[0]
            print(f"  OK: active toolchain: {active}")
            if pinned_toolchain:
                active_short = active.split()[0]
                if not active_short.startswith(pinned_toolchain):
                    print(f"  FAIL: active toolchain '{active_short}' does not match pinned '{pinned_toolchain}'")
                    ok = False
                else:
                    print(f"  OK: toolchain matches pinned '{pinned_toolchain}'")
        except (FileNotFoundError, subprocess.TimeoutExpired):
            print("  FAIL: could not determine active toolchain")
            ok = False
    else:
        print("  WARNING: rust-toolchain.toml not found in repo root")

    # Check cargo metadata works
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            capture_output=True, text=True, timeout=30, cwd=REPO_ROOT
        )
        if result.returncode == 0:
            metadata = json.loads(result.stdout)
            n = len(metadata.get("packages", []))
            print(f"  OK: cargo metadata: {n} package(s) in workspace")
        else:
            print("  FAIL: cargo metadata failed")
            stderr_tail = result.stderr.strip().split("\n")[-5:]
            for line in stderr_tail:
                print(f"    {line}")
            ok = False
    except (FileNotFoundError, subprocess.TimeoutExpired, json.JSONDecodeError):
        print("  FAIL: cargo metadata failed")
        ok = False

    # Check cargo fmt is available
    if shutil.which("rustfmt") or shutil.which("cargo-fmt"):
        print("  OK: rustfmt: available")
    else:
        print("  FAIL: rustfmt: not found. Run `rustup component add rustfmt`")
        ok = False

    # Check cargo clippy is available
    if shutil.which("cargo-clippy"):
        print("  OK: clippy: available")
    else:
        print("  FAIL: clippy: not found. Run `rustup component add clippy`")
        ok = False

    # Check locked phase-1 spec files exist and are non-empty
    locked_specs = [
        REPO_ROOT / "docs/discussions/regseq-model/spec.md",
        REPO_ROOT / "docs/discussions/regseq-model/pretty-printer.md",
        REPO_ROOT / "docs/discussions/regseq-model/generic-c-backend.md",
    ]
    for spec_path in locked_specs:
        rel = spec_path.relative_to(REPO_ROOT)
        if spec_path.exists() and spec_path.stat().st_size > 0:
            print(f"  OK: locked spec: {rel} ({spec_path.stat().st_size} bytes)")
        else:
            print(f"  FAIL: locked spec: {rel} missing or empty")
            ok = False

    if ok:
        print("DOCTOR: All checks passed.")
    else:
        print("DOCTOR: Some checks failed. Fix the issues above before starting work.")
        sys.exit(1)


def main() -> None:
    parser = argparse.ArgumentParser(description="Regseq compiler harness task manager")
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser("next", help="Find next available task")
    subparsers.add_parser("next-step", help="Determine the next executable or task-spec action")

    show_parser = subparsers.add_parser("show", help="Show task details")
    show_parser.add_argument("id", help="Task ID")

    start_parser = subparsers.add_parser("start", help="Start a task")
    start_parser.add_argument("id", help="Task ID")

    done_parser = subparsers.add_parser("done", help="Complete a task")
    done_parser.add_argument("id", help="Task ID")

    fail_parser = subparsers.add_parser("fail", help="Fail a task")
    fail_parser.add_argument("id", help="Task ID")
    fail_parser.add_argument("--reason", required=True, help="Reason for failure")

    reset_parser = subparsers.add_parser(
        "reset",
        help="Reset a task back to pending (recovery path)",
    )
    reset_parser.add_argument("id", help="Task ID")
    reset_parser.add_argument(
        "--reason",
        required=True,
        help="Why the task is being reset (goes to console audit trail only)",
    )
    reset_parser.add_argument(
        "--force",
        action="store_true",
        help="Permit resetting a task whose current status is `done`",
    )

    add_task_parser = subparsers.add_parser(
        "add-task",
        help="Add a new task (canonical write path; direct tasks.json edits forbidden)",
    )
    add_task_parser.add_argument("--id", required=True, help="Task ID (must be unique)")
    add_task_parser.add_argument("--name", required=True, help="Task display name")
    add_task_parser.add_argument(
        "--spec",
        required=True,
        help="Path to task spec markdown (must be under .harness/tasks/<group>/)",
    )
    add_task_parser.add_argument("--group", required=True, help="Group ID the task belongs to")
    add_task_parser.add_argument(
        "--depends-on",
        nargs="*",
        default=[],
        help="Task IDs this depends on (all must already exist)",
    )
    add_task_parser.add_argument(
        "--spec-refs",
        nargs="*",
        default=[],
        help="Spec references in `<spec-file>#<anchor>` shape",
    )
    add_task_parser.add_argument(
        "--phase",
        default=None,
        help="Phase tag (e.g. `1a`, `1b`, or a phase-2 family tag)",
    )

    update_spec_parser = subparsers.add_parser(
        "update-task-spec",
        help="Bump a task's spec_version after editing its spec markdown",
    )
    update_spec_parser.add_argument("id", help="Task ID")
    update_spec_parser.add_argument(
        "--reason",
        help="Optional reason recorded in the history entry",
    )

    archive_parser = subparsers.add_parser(
        "archive",
        help="Flip task(s) to archived status; sets phase=archived atomically",
    )
    archive_parser.add_argument("ids", nargs="+", help="One or more task IDs to archive")
    archive_parser.add_argument(
        "--reason",
        required=True,
        help="Why these tasks are being archived (recorded in history)",
    )

    subparsers.add_parser("status", help="Show status summary")

    list_parser = subparsers.add_parser("list", help="List tasks")
    list_parser.add_argument(
        "--status",
        choices=["pending", "in_progress", "done", "failed", "archived"],
        help="Filter by status",
    )
    list_parser.add_argument("--phase", help="Filter by phase tag")
    list_parser.add_argument(
        "--json",
        action="store_true",
        help="Emit one JSON object per task on stdout, newline-separated",
    )
    list_parser.add_argument(
        "--quiet",
        action="store_true",
        help="Emit only task IDs, one per line (no group headers)",
    )

    ready_leaves_parser = subparsers.add_parser(
        "ready-leaves",
        help="Pending tasks whose dependencies are all done (sorted)",
    )
    ready_leaves_parser.add_argument("--phase", help="Filter by phase tag")

    phase_status_parser = subparsers.add_parser(
        "phase-status",
        help="PASS/FAIL report for a phase tag",
    )
    phase_status_parser.add_argument("phase", help="Phase tag (e.g. 1a, 1b)")

    graph_parser = subparsers.add_parser("graph", help="Emit dependency DAG")
    graph_parser.add_argument(
        "--format",
        choices=["mermaid", "dot"],
        default="mermaid",
    )
    graph_parser.add_argument("--phase", help="Filter by phase tag")
    graph_parser.add_argument(
        "--status",
        choices=["pending", "in_progress", "done", "failed", "archived"],
        help="Filter by status",
    )

    spec_coverage_parser = subparsers.add_parser(
        "spec-coverage",
        help="Coverage report for a spec file vs. tasks' spec_refs",
    )
    spec_coverage_parser.add_argument("spec_file", help="Path to spec markdown file")
    spec_coverage_parser.add_argument(
        "--include-subsections",
        action="store_true",
        help="Also collect `###` headings as anchors",
    )
    spec_coverage_parser.add_argument(
        "--include-status",
        choices=["done", "pending", "all"],
        default="all",
        help="Restrict spec_refs to tasks with this status (default: all)",
    )

    read_spec_refs_parser = subparsers.add_parser(
        "read-spec-refs",
        help="Print anchor slugs from a markdown spec file (one per line)",
    )
    read_spec_refs_parser.add_argument("spec_file")
    read_spec_refs_parser.add_argument(
        "--include-subsections",
        action="store_true",
        help="Also collect `###` headings as anchors",
    )

    subparsers.add_parser("blocked", help="Show blocked tasks")

    subparsers.add_parser("doctor", help="Check prerequisites")

    args = parser.parse_args()

    commands = {
        "next": cmd_next,
        "next-step": cmd_next_step,
        "show": cmd_show,
        "start": cmd_start,
        "done": cmd_done,
        "fail": cmd_fail,
        "reset": cmd_reset,
        "add-task": cmd_add_task,
        "update-task-spec": cmd_update_task_spec,
        "archive": cmd_archive,
        "status": cmd_status,
        "list": cmd_list,
        "ready-leaves": cmd_ready_leaves,
        "phase-status": cmd_phase_status,
        "graph": cmd_graph,
        "spec-coverage": cmd_spec_coverage,
        "read-spec-refs": cmd_read_spec_refs,
        "blocked": cmd_blocked,
        "doctor": cmd_doctor,
    }
    commands[args.command](args)


if __name__ == "__main__":
    main()
