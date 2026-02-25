#!/usr/bin/env python3
"""Regseq compiler harness -- task management CLI.

Usage:
    python scripts/python/task.py next                     Find next available task
    python scripts/python/task.py show <id>                Show task details
    python scripts/python/task.py start <id>               Mark task as in_progress
    python scripts/python/task.py done <id>                Run verification and mark done
    python scripts/python/task.py fail <id> --reason "…"   Mark task as failed
    python scripts/python/task.py status                   Show summary table
    python scripts/python/task.py list [--status <s>]      List tasks with optional filter
    python scripts/python/task.py blocked                  Show tasks with unmet dependencies
    python scripts/python/task.py doctor                   Check prerequisites
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
import tomllib
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).parent.parent.parent
TASKS_FILE = REPO_ROOT / ".harness" / "tasks.json"
LOGS_DIR = REPO_ROOT / "logs" / "verify"
TAIL_LINES = 50


def load_tasks() -> dict:
    with open(TASKS_FILE) as f:
        return json.load(f)


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
            print(f"ACTION: Run `python scripts/python/task.py start {task['id']}` then read the spec file.")
            return

    print("NO_TASKS_AVAILABLE: All pending tasks have unmet dependencies or all tasks are done.")
    blocked = [t for t in all_tasks(data) if t["status"] == "pending" and not deps_met(t, status_map)]
    if blocked:
        print("BLOCKED_TASKS:")
        for t in blocked:
            unmet = [d for d in t.get("depends_on", []) if status_map.get(d) != "done"]
            unmet_strs = [f"{d} [{status_map.get(d, 'unknown')}]" for d in unmet]
            print(f"  - {t['id']} (blocked by: {', '.join(unmet_strs)})")
    print("ACTION: End session. No work available.")


def cmd_show(args: argparse.Namespace) -> None:
    data = load_tasks()
    result = find_task(data, args.id)
    if result is None:
        print(f"ERROR: Task '{args.id}' not found.")
        print("ACTION: Run `python scripts/python/task.py list` to see available task IDs.")
        sys.exit(1)

    group, task = result
    print(f"TASK: {task['id']}")
    print(f"NAME: {task['name']}")
    print(f"GROUP: {group['name']}")
    print(f"STATUS: {task['status']}")
    print(f"SPEC: {task['spec']}")
    if task.get("depends_on"):
        status_map = task_status_map(data)
        deps = ", ".join(f"{d} [{status_map.get(d, 'unknown')}]" for d in task["depends_on"])
        print(f"DEPENDS_ON: {deps}")
    if task.get("artifacts"):
        print(f"ARTIFACTS: {', '.join(task['artifacts'])}")
    if task.get("fail_reason"):
        print(f"FAIL_REASON: {task['fail_reason']}")
    print(f"ACTION: Read {task['spec']} for the full task specification.")


def cmd_start(args: argparse.Namespace) -> None:
    data = load_tasks()
    result = find_task(data, args.id)
    if result is None:
        print(f"ERROR: Task '{args.id}' not found.")
        print("ACTION: Run `python scripts/python/task.py list` to see available task IDs.")
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
        print("ACTION: Run `python scripts/python/task.py next` to find a task with all dependencies met.")
        sys.exit(1)

    task["status"] = "in_progress"
    save_tasks(data)
    print(f"STARTED: {args.id}")
    print(f"STATUS: in_progress")
    print(f"ACTION: Read {task['spec']} for your task specification.")


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
    """Run cargo test, fmt check, and clippy. Return (passed, messages)."""
    messages = []
    passed = True
    log_entries = []

    if not workspace_has_members():
        messages.append("VERIFICATION: Workspace has no packages. Skipping cargo checks.")
        return True, messages

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
        messages.append("VERIFICATION: cargo clippy --workspace... PASSED")
    else:
        passed = False
        messages.append("VERIFICATION: cargo clippy --workspace... FAILED")
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
        print("ACTION: Run `python scripts/python/task.py next` for your next task, or end session if none.")
    else:
        print(f"BLOCKED: {args.id} -- verification failed.")
        print("ACTION: Fix the failing checks and run `python scripts/python/task.py done " + args.id + "` again.")


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


def cmd_status(args: argparse.Namespace) -> None:
    data = load_tasks()
    tasks = all_tasks(data)

    counts = {"pending": 0, "in_progress": 0, "done": 0, "failed": 0}
    for t in tasks:
        counts[t["status"]] = counts.get(t["status"], 0) + 1

    total = len(tasks)
    print("STATUS_SUMMARY:")
    print(f"  Total:       {total}")
    print(f"  Done:        {counts['done']}")
    print(f"  In Progress: {counts['in_progress']}")
    print(f"  Pending:     {counts['pending']}")
    print(f"  Failed:      {counts['failed']}")

    if total > 0:
        pct = (counts["done"] / total) * 100
        print(f"  Progress:    {pct:.0f}%")


def cmd_list(args: argparse.Namespace) -> None:
    data = load_tasks()
    status_map = task_status_map(data)

    print("TASK_LIST:")
    for group in data["groups"]:
        tasks = group["tasks"]
        if args.status:
            tasks = [t for t in tasks if t["status"] == args.status]
        if not tasks:
            continue

        print(f"\n  [{group['name']}]")
        for t in tasks:
            met = deps_met(t, status_map)
            blocked_marker = " (BLOCKED)" if t["status"] == "pending" and not met else ""
            print(f"    {t['id']:30s} {t['status']:12s} {t['name']}{blocked_marker}")


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
        print("ACTION: Run `python scripts/python/task.py next` to find available work.")
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
    ok = check("rustup", ["rustup", "--version"]) and ok
    ok = check("cargo", ["cargo", "--version"]) and ok
    ok = check("rustc", ["rustc", "--version"]) and ok

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

    if ok:
        print("DOCTOR: All checks passed.")
    else:
        print("DOCTOR: Some checks failed. Fix the issues above before starting work.")
        sys.exit(1)


def main() -> None:
    parser = argparse.ArgumentParser(description="Regseq compiler harness task manager")
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser("next", help="Find next available task")

    show_parser = subparsers.add_parser("show", help="Show task details")
    show_parser.add_argument("id", help="Task ID")

    start_parser = subparsers.add_parser("start", help="Start a task")
    start_parser.add_argument("id", help="Task ID")

    done_parser = subparsers.add_parser("done", help="Complete a task")
    done_parser.add_argument("id", help="Task ID")

    fail_parser = subparsers.add_parser("fail", help="Fail a task")
    fail_parser.add_argument("id", help="Task ID")
    fail_parser.add_argument("--reason", required=True, help="Reason for failure")

    subparsers.add_parser("status", help="Show status summary")

    list_parser = subparsers.add_parser("list", help="List tasks")
    list_parser.add_argument("--status", choices=["pending", "in_progress", "done", "failed"],
                             help="Filter by status")

    subparsers.add_parser("blocked", help="Show blocked tasks")

    subparsers.add_parser("doctor", help="Check prerequisites")

    args = parser.parse_args()

    commands = {
        "next": cmd_next,
        "show": cmd_show,
        "start": cmd_start,
        "done": cmd_done,
        "fail": cmd_fail,
        "status": cmd_status,
        "list": cmd_list,
        "blocked": cmd_blocked,
        "doctor": cmd_doctor,
    }
    commands[args.command](args)


if __name__ == "__main__":
    main()
