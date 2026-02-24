#!/usr/bin/env python3
"""Regseq compiler harness -- task management CLI.

Usage:
    python task.py next                     Find next available task
    python task.py show <id>                Show task details
    python task.py start <id>               Mark task as in_progress
    python task.py done <id>                Run verification and mark done
    python task.py fail <id> --reason "…"   Mark task as failed
    python task.py status                   Show summary table
    python task.py list [--status <s>]      List tasks with optional filter
    python task.py blocked                  Show tasks with unmet dependencies
"""

import argparse
import json
import subprocess
import sys
from pathlib import Path

TASKS_FILE = Path(__file__).parent / ".harness" / "tasks.json"


def load_tasks() -> dict:
    with open(TASKS_FILE) as f:
        return json.load(f)


def save_tasks(data: dict) -> None:
    with open(TASKS_FILE, "w") as f:
        json.dump(data, f, indent=2)
        f.write("\n")


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
            print(f"ACTION: Run `python task.py start {task['id']}` then read the spec file.")
            return

    print("NO_TASKS_AVAILABLE: All pending tasks have unmet dependencies or all tasks are done.")
    blocked = [t for t in all_tasks(data) if t["status"] == "pending" and not deps_met(t, status_map)]
    if blocked:
        print("BLOCKED_TASKS:")
        for t in blocked:
            unmet = [d for d in t.get("depends_on", []) if status_map.get(d) != "done"]
            print(f"  - {t['id']} (blocked by: {', '.join(f'{d} [{status_map.get(d, 'unknown')}]' for d in unmet)})")
    print("ACTION: End session. No work available.")


def cmd_show(args: argparse.Namespace) -> None:
    data = load_tasks()
    result = find_task(data, args.id)
    if result is None:
        print(f"ERROR: Task '{args.id}' not found.")
        print("ACTION: Run `python task.py list` to see available task IDs.")
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
        print("ACTION: Run `python task.py list` to see available task IDs.")
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
        print("ACTION: Run `python task.py next` to find a task with all dependencies met.")
        sys.exit(1)

    task["status"] = "in_progress"
    save_tasks(data)
    print(f"STARTED: {args.id}")
    print(f"STATUS: in_progress")
    print(f"ACTION: Read {task['spec']} for your task specification.")


def run_verification() -> tuple[bool, list[str]]:
    """Run cargo test and clippy. Return (passed, messages)."""
    messages = []
    passed = True

    # Check if workspace has any members
    try:
        with open("Cargo.toml") as f:
            content = f.read()
        if 'members = []' in content:
            messages.append("VERIFICATION: Workspace has no members. Skipping cargo checks.")
            return True, messages
    except FileNotFoundError:
        messages.append("VERIFICATION: No Cargo.toml found. Skipping cargo checks.")
        return True, messages

    # cargo test
    result = subprocess.run(
        ["cargo", "test", "--workspace"],
        capture_output=True, text=True, timeout=300
    )
    if result.returncode == 0:
        messages.append("VERIFICATION: cargo test --workspace... PASSED")
    else:
        passed = False
        messages.append("VERIFICATION: cargo test --workspace... FAILED")
        stderr_lines = result.stderr.strip().split("\n")
        # Show last 20 lines of output for context
        for line in stderr_lines[-20:]:
            messages.append(f"  {line}")

    # cargo clippy
    result = subprocess.run(
        ["cargo", "clippy", "--workspace", "--", "-D", "warnings"],
        capture_output=True, text=True, timeout=300
    )
    if result.returncode == 0:
        messages.append("VERIFICATION: cargo clippy --workspace... PASSED")
    else:
        passed = False
        messages.append("VERIFICATION: cargo clippy --workspace... FAILED")
        stderr_lines = result.stderr.strip().split("\n")
        for line in stderr_lines[-20:]:
            messages.append(f"  {line}")

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

    passed, messages = run_verification()
    for msg in messages:
        print(msg)

    if passed:
        task["status"] = "done"
        task["fail_reason"] = None
        save_tasks(data)
        print(f"DONE: {args.id}")
        print("ACTION: Run `python task.py next` for your next task, or end session if none.")
    else:
        print(f"BLOCKED: {args.id} -- verification failed.")
        print("ACTION: Fix the failing checks and run `python task.py done " + args.id + "` again.")


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
        print("ACTION: Run `python task.py next` to find available work.")
        return

    print("BLOCKED_TASKS:")
    for t, unmet in blocked:
        deps_str = ", ".join(f"{d} [{status_map.get(d, 'unknown')}]" for d in unmet)
        print(f"  - {t['id']}: blocked by {deps_str}")


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
    }
    commands[args.command](args)


if __name__ == "__main__":
    main()
